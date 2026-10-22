//! Whole-area check of the lighting port against the real game.
//!
//! `tools/refdump_light` (`run.cmd`) loads `work/refsave/Worlds/ref_small.wld` with the game's own
//! `WorldFile.LoadWorld`, then for each area of its list (spawn, cavern, hell, world edges, and three areas
//! overwritten with a pseudo-random tile pattern so every tile type, wall, paint, liquid and frame is reached):
//! dumps the tiles it is about to light (`<name>.tiles.bin`), sets the Main state recorded in the header of
//! `<name>.light.bin`, runs `Lighting.Initialize()`, seeds the three `FastRandom`s of the engine, queues the lights
//! listed in the header and calls `Lighting.LightTiles(area)` for two scan + blur cycles. It dumps the engine's raw
//! `Vector3` and `Lighting.GetColor(x, y)` of every tile of the processed area (the area inflated by 28).
//!
//! This test replays exactly that through the Rust port and compares every tile bit for bit. The tile input of
//! the Rust run is the `.tiles.bin` dump; for the areas that are plain world (not scrambled) the test first checks the
//! tiles the Rust loader produced against the dump (only the liquid settle of `WorldFile.LoadWorld`, which is not
//! ported, may differ) so the world loader is covered as well.
//!
//! What "equal" means. `Lighting.GetColor(x, y)` (the packed colour the renderer uses) must be identical on every tile of
//! every area. The raw `Vector3` of the light map must be bit-identical in `spawn`, `cavern` and `hell_t0` (plain air, walls,
//! tiles, a few lights) and within `REL_TOLERANCE` of the value everywhere else. The reference harness is the game's x86
//! build: its JIT (x87) keeps excess precision in float expressions that a strict-f32 translation rounds at every operation
//! (for example `num2 = num4 * 0.6f` after `num4 = 0.55f + (float)Math.Sin(..) * 0.08f` uses the unrounded `num4`, and
//! `zero.X *= LightDecayThroughWater.X * num` multiplies by an unrounded product). Those differences are 1 ulp of the
//! value, never more, and they are all that is left. `scramble_shimmer` contains tiles that call
//! `LiquidRenderer.GetShimmerBaseColor`, whose `Math.Sin` argument is about 1000 radians, so an x87-rounding difference of
//! 1e-7 in `(x + y / 6f) / 10f` becomes 1e-5 in the colour; that area is compared with `SHIMMER_TOLERANCE`.
//!
//! Skipped with a message when the world or the dumps are missing.

use std::path::PathBuf;

use terraria_core::xna::{Color, Rectangle, Vector3};
use terraria_core::Tile;
use terraria_sim::graphics::light::ILightingEngine;
use terraria_sim::io::world_file::load_world;
use terraria_sim::{lighting, Game};

/// Largest raw difference outside the exact and shimmer areas, relative to the value. One x87-versus-f32 rounding
/// difference is 6e-8 of the value; a blur line applies up to ~100 multiplications, each of which can add one.
const REL_TOLERANCE: f32 = 3e-6;
/// Largest absolute raw difference in `scramble_shimmer` (see the module doc).
const SHIMMER_TOLERANCE: f32 = 1e-4;
/// Areas whose raw light map must be bit-identical.
const EXACT: [&str; 3] = ["spawn", "cavern", "hell_t0"];

fn ulps(a: f32, b: f32) -> u32 {
    // both finite and non-negative in a light map; compare the integer bit patterns
    (a.to_bits() as i64 - b.to_bits() as i64).unsigned_abs() as u32
}

const NAMES: [&str; 10] =
    ["spawn", "cavern", "hell_t0", "hell", "edge_left", "edge_bottom", "scramble_surface", "scramble_deep", "scramble_hell", "scramble_shimmer"];

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..")
}

struct Cur<'a> {
    b: &'a [u8],
    at: usize,
}

impl<'a> Cur<'a> {
    fn take(&mut self, n: usize) -> &'a [u8] {
        let s = &self.b[self.at..self.at + n];
        self.at += n;
        s
    }
    fn i32(&mut self) -> i32 {
        i32::from_le_bytes(self.take(4).try_into().unwrap())
    }
    fn u32(&mut self) -> u32 {
        u32::from_le_bytes(self.take(4).try_into().unwrap())
    }
    fn u64(&mut self) -> u64 {
        u64::from_le_bytes(self.take(8).try_into().unwrap())
    }
    fn f32(&mut self) -> f32 {
        f32::from_le_bytes(self.take(4).try_into().unwrap())
    }
    fn f64(&mut self) -> f64 {
        f64::from_le_bytes(self.take(8).try_into().unwrap())
    }
    fn u8(&mut self) -> u8 {
        self.take(1)[0]
    }
}

fn read_tile(rec: &[u8]) -> Tile {
    let u16_at = |o: usize| u16::from_le_bytes([rec[o], rec[o + 1]]);
    Tile {
        type_: u16_at(0),
        wall: u16_at(2),
        liquid: rec[4],
        s_tile_header: u16_at(5),
        b_tile_header: rec[7],
        b_tile_header2: rec[8],
        b_tile_header3: rec[9],
        frame_x: u16_at(10) as i16,
        frame_y: u16_at(12) as i16,
    }
}

struct Light {
    area: Rectangle,
    processed: Rectangle,
    seeds: (u64, u64, u64),
    cycles: i32,
    game_update_count: u32,
    tile_color: Color,
    mouse_text_color: u8,
    time_for_visual_effects: f64,
    global_time_wrapped_hourly: f32,
    water_style: i32,
    day_time: bool,
    time: f64,
    lights: Vec<(i32, i32, f32, f32, f32)>,
    demon_torch: f32,
    disco: (i32, i32, i32),
    sundial_cooldown: i32,
    moondial_cooldown: i32,
    shimmer_darken: f32,
    cage_frames: i32,
    raining: bool,
    my_player: i32,
    map_delay: i32,
    liquid_alpha: Vec<f32>,
    jellyfish_cage_mode: Vec<Vec<u8>>,
    scene_state: (f32, f32),
    /// (x, y, raw x, raw y, raw z, packed)
    tiles: Vec<(i32, i32, [f32; 3], u32)>,
}

fn read_light(bytes: &[u8]) -> Light {
    let mut c = Cur { b: bytes, at: 0 };
    assert_eq!(c.take(4), b"LGHT");
    assert_eq!(c.i32(), 2);
    let area = Rectangle::new(c.i32(), c.i32(), c.i32(), c.i32());
    let processed = Rectangle::new(c.i32(), c.i32(), c.i32(), c.i32());
    let seeds = (c.u64(), c.u64(), c.u64());
    let cycles = c.i32();
    let game_update_count = c.u32();
    let tile_color = Color::new(c.u8(), c.u8(), c.u8(), c.u8());
    let mouse_text_color = c.u8();
    c.take(3);
    let time_for_visual_effects = c.f64();
    let global_time_wrapped_hourly = c.f32();
    let water_style = c.i32();
    let day_time = c.u8() != 0;
    c.take(3);
    let time = c.f64();
    let n = c.i32();
    let lights = (0..n).map(|_| (c.i32(), c.i32(), c.f32(), c.f32(), c.f32())).collect();
    let demon_torch = c.f32();
    let disco = (c.i32(), c.i32(), c.i32());
    let sundial_cooldown = c.i32();
    let moondial_cooldown = c.i32();
    let shimmer_darken = c.f32();
    let cage_frames = c.i32();
    let raining = c.u8() != 0;
    c.take(3);
    let my_player = c.i32();
    let map_delay = c.i32();
    let n = c.i32();
    let liquid_alpha = (0..n).map(|_| c.f32()).collect();
    let (rows, cols) = (c.i32(), c.i32());
    let jellyfish_cage_mode = (0..rows).map(|_| (0..cols).map(|_| c.u8()).collect()).collect();
    let scene_state = (c.f32(), c.f32());
    let mut tiles = Vec::new();
    for x in processed.x..processed.x + processed.width {
        for y in processed.y..processed.y + processed.height {
            let v = [c.f32(), c.f32(), c.f32()];
            tiles.push((x, y, v, c.u32()));
        }
    }
    assert_eq!(c.at, bytes.len(), "trailing bytes in the light dump");
    Light {
        area,
        processed,
        seeds,
        cycles,
        game_update_count,
        tile_color,
        mouse_text_color,
        time_for_visual_effects,
        global_time_wrapped_hourly,
        water_style,
        day_time,
        time,
        lights,
        demon_torch,
        disco,
        sundial_cooldown,
        moondial_cooldown,
        shimmer_darken,
        cage_frames,
        raining,
        my_player,
        map_delay,
        liquid_alpha,
        jellyfish_cage_mode,
        scene_state,
        tiles,
    }
}

#[test]
fn lighting_matches_the_real_game() {
    let root = root();
    let wld = root.join("work/refsave/Worlds/ref_small.wld");
    let dir = root.join("refdata/light");
    let Ok(wld_bytes) = std::fs::read(&wld) else {
        eprintln!("SKIP: {} missing", wld.display());
        return;
    };
    if !dir.join("spawn.light.bin").exists() {
        eprintln!("SKIP: no dump in {} (run tools\\refdump_light\\run.cmd)", dir.display());
        return;
    }

    let world = terraria_formats::wld::read_world(&wld_bytes).expect("read ref_small.wld");
    let mut g = Game::new();
    let _world = load_world(&mut g, world);
    let loaded_tiles = g.main.tile.as_slice().to_vec();
    let height = g.main.tile.height;

    let mut total_bad = 0usize;
    let mut summary = String::new();
    for name in NAMES {
        let light = read_light(&std::fs::read(dir.join(format!("{name}.light.bin"))).unwrap());
        let tiles_bytes = std::fs::read(dir.join(format!("{name}.tiles.bin"))).unwrap();
        let mut c = Cur { b: &tiles_bytes, at: 0 };
        assert_eq!(c.take(4), b"TILE");
        assert_eq!(c.i32(), 1);
        let (tx, ty, tw, th) = (c.i32(), c.i32(), c.i32(), c.i32());

        // ---- the tile input: the dump; for plain world areas also compare it with what the Rust loader made
        let scrambled = name.starts_with("scramble");
        let (mut liquid_only, mut other) = (0usize, 0usize);
        for x in tx..tx + tw {
            for y in ty..ty + th {
                let t = read_tile(c.take(14));
                if !scrambled {
                    let mine = loaded_tiles[(x * height + y) as usize];
                    if mine != t {
                        let mut a = mine;
                        let mut b = t;
                        for q in [&mut a, &mut b] {
                            q.liquid = 0;
                            q.b_tile_header &= !0x60;
                            q.b_tile_header3 &= !0x18;
                        }
                        if a == b {
                            liquid_only += 1;
                        } else {
                            other += 1;
                        }
                    }
                }
                *g.main.tile.get_mut(x, y) = t;
            }
        }
        assert_eq!(c.at, tiles_bytes.len());
        assert_eq!(other, 0, "{name}: {other} tiles of the loaded world differ from the game's beyond the liquid settle");

        // ---- the same state and calls as the harness
        g.main.game_menu = false;
        g.main.net_mode = 0;
        g.main.game_paused = false;
        g.main.map_enabled = false;
        g.main.day_time = light.day_time;
        g.main.time = light.time;
        g.main.tile_color = light.tile_color;
        g.main.mouse_text_color = light.mouse_text_color;
        g.main.time_for_visual_effects = light.time_for_visual_effects;
        g.main.global_time_wrapped_hourly = light.global_time_wrapped_hourly;
        g.main.water_style = light.water_style;
        g.main.game_update_count = light.game_update_count;
        // Main statics the harness only read: the port's initial values must be what the game holds
        let fresh = Game::new();
        assert_eq!(fresh.main.demon_torch, light.demon_torch, "Main.demonTorch initial value");
        assert_eq!((fresh.main.disco_r, fresh.main.disco_g, fresh.main.disco_b), light.disco, "Main.DiscoR/G/B initial values");
        assert_eq!(fresh.main.sundial_cooldown, light.sundial_cooldown);
        assert_eq!(fresh.main.moondial_cooldown, light.moondial_cooldown);
        assert_eq!(fresh.main.shimmer_darken, light.shimmer_darken);
        assert_eq!(fresh.main.cage_frames, light.cage_frames);
        assert_eq!(fresh.main.raining, light.raining);
        assert_eq!(fresh.main.my_player, light.my_player);
        assert_eq!(fresh.main.map_delay, light.map_delay);
        assert_eq!(fresh.main.liquid_alpha.to_vec(), light.liquid_alpha, "Main.liquidAlpha initial values");
        assert_eq!(fresh.main.scene_state_air_light_decay, light.scene_state.0);
        assert_eq!(fresh.main.scene_state_solid_light_decay, light.scene_state.1);
        let jc: Vec<Vec<u8>> = fresh.main.jellyfish_cage_mode.iter().map(|r| r.to_vec()).collect();
        if jc != light.jellyfish_cage_mode {
            eprintln!("note: Main.jellyfishCageMode differs from the port's initial (all zero) value in the game; using the game's");
        }
        for (i, row) in light.jellyfish_cage_mode.iter().enumerate() {
            g.main.jellyfish_cage_mode[i].copy_from_slice(row);
        }
        lighting::initialize(&mut g);
        g.lighting.new_engine_mut().set_random_seeds(light.seeds.0, light.seeds.1, light.seeds.2);
        for &(x, y, r, g_, b) in &light.lights {
            lighting::add_light(&mut g, x, y, r, g_, b);
        }
        for _ in 0..light.cycles * 4 {
            lighting::light_tiles(&mut g, light.area);
        }

        // ---- compare every tile of the processed area
        assert_eq!(g.lighting.new_engine().active_processed_area(), light.processed);
        let (mut bad_raw, mut bad_packed, mut first_raw, mut first_packed) = (0usize, 0usize, None, None);
        let mut non_black = 0usize;
        let mut worst = 0f32;
        let mut worst_ulps = 0u32;
        let mut out_of_tolerance = 0usize;
        let tolerance_ok = |mine: Vector3, v: [f32; 3], d: f32, u: u32| -> bool {
            if name == "scramble_shimmer" {
                d <= SHIMMER_TOLERANCE
            } else if EXACT.contains(&name) {
                u == 0
            } else {
                [mine.x, mine.y, mine.z].iter().zip(v.iter()).all(|(a, b)| (a - b).abs() <= REL_TOLERANCE * a.abs().max(b.abs()))
            }
        };
        for &(x, y, v, packed) in &light.tiles {
            let mine = g.lighting.new_engine().get_color(x, y);
            if mine != Vector3::ZERO {
                non_black += 1;
            }
            let same = mine.x.to_bits() == v[0].to_bits() && mine.y.to_bits() == v[1].to_bits() && mine.z.to_bits() == v[2].to_bits();
            if !same {
                if std::env::var("LIGHT_DEBUG").is_ok() && ((mine.x - v[0]).abs() > 1e-6 || (mine.y - v[1]).abs() > 1e-6 || (mine.z - v[2]).abs() > 1e-6) {
                    let t = g.main.tile.get(x, y);
                    eprintln!("DBG {name} ({x},{y}) rust {:?} game {:?} type {} wall {} liq {} s {:04x} b {:02x} b3 {:02x} fx {} fy {}", mine, v, t.type_, t.wall, t.liquid, t.s_tile_header, t.b_tile_header, t.b_tile_header3, t.frame_x, t.frame_y);
                }
                bad_raw += 1;
                first_raw.get_or_insert((x, y, mine, v));
                let d = (mine.x - v[0]).abs().max((mine.y - v[1]).abs()).max((mine.z - v[2]).abs());
                let u = ulps(mine.x, v[0]).max(ulps(mine.y, v[1])).max(ulps(mine.z, v[2]));
                worst = worst.max(d);
                worst_ulps = worst_ulps.max(u);
                if !tolerance_ok(mine, v, d, u) {
                    out_of_tolerance += 1;
                }
            }
            let p = lighting::get_color(&g, x, y).packed_value();
            if p != packed {
                bad_packed += 1;
                first_packed.get_or_insert((x, y, p, packed));
            }
        }
        summary += &format!(
            "{name}: {} tiles, {non_black} lit, loader liquid-only diffs {liquid_only}, raw mismatches {bad_raw} (beyond tolerance {out_of_tolerance}), packed mismatches {bad_packed}, worst abs diff {worst} ({worst_ulps} ulp)\n",
            light.tiles.len()
        );
        if let Some(f) = first_raw {
            summary += &format!("  first raw mismatch at ({}, {}): rust {:?} game {:?}\n", f.0, f.1, f.2, f.3);
        }
        if let Some(f) = first_packed {
            summary += &format!("  first packed mismatch at ({}, {}): rust {:08x} game {:08x}\n", f.0, f.1, f.2, f.3);
        }
        total_bad += out_of_tolerance + bad_packed;
    }
    eprintln!("{summary}");
    assert_eq!(total_bad, 0, "lighting differs from the real game:\n{summary}");
}

/// `LiquidRenderer.GetShimmerBaseColor(x, y)` against the game for a grid of tiles (`shimmer.bin`). The sine's argument is
/// about 1000 radians, so the x87 excess precision of `(x + y / 6f) / 10f` shows up at 1e-5; the strict-f32 port stays
/// within `SHIMMER_TOLERANCE`. Evaluating the same expression in f64 (which is what a longer x87 register amounts to) brings
/// the error down by two orders of magnitude, which shows the difference is the rounding of that sub-expression and not
/// the formula.
#[test]
fn shimmer_base_color_matches_the_real_game() {
    let path = root().join("refdata/light/shimmer.bin");
    let Ok(bytes) = std::fs::read(&path) else {
        eprintln!("SKIP: {} missing (run tools\refdump_light\run.cmd)", path.display());
        return;
    };
    let mut c = Cur { b: &bytes, at: 0 };
    assert_eq!(c.take(4), b"SHMR");
    assert_eq!(c.i32(), 1);
    let time_for_visual_effects = c.f64();
    let n = c.i32();
    let mut g = Game::new();
    g.main.time_for_visual_effects = time_for_visual_effects;
    let (mut worst, mut worst_wide) = (0f32, 0f32);
    for _ in 0..n {
        let (x, y) = (c.f32(), c.f32());
        let want = [c.f32(), c.f32(), c.f32(), c.f32()];
        let got = terraria_sim::light_support::get_shimmer_base_color(&g, x, y);
        for (a, b) in [got.x, got.y, got.z, got.w].iter().zip(want.iter()) {
            worst = worst.max((a - b).abs());
        }
        // the same formula with the float sub-expression evaluated in f64
        let arg = (((x as f64 + y as f64 / 6.0) / 10.0) - time_for_visual_effects / 360.0) * 6.2831854820251465_f64;
        let wave = arg.sin() as f32;
        let t = 0.1f32 + wave * 0.4f32;
        let lerp = |a: f32, b: f32| a + (b - a) * t;
        let wide = [lerp(0.64705884, 41.0 / 51.0), lerp(26.0 / 51.0, 41.0 / 51.0), lerp(14.0 / 15.0, 1.0), 1.0f32];
        for (a, b) in wide.iter().zip(want.iter()) {
            worst_wide = worst_wide.max((a - b).abs());
        }
    }
    eprintln!("{n} points: strict f32 worst abs error {worst}, f64 sub-expression worst abs error {worst_wide}");
    assert!(worst <= SHIMMER_TOLERANCE, "strict port differs from the game by {worst}");
    assert!(worst_wide < worst / 10.0, "the f64 evaluation should explain the difference ({worst_wide} vs {worst})");
}
