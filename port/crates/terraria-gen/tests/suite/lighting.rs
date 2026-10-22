//! The translated lighting engine (`Terraria.Lighting`, `Graphics.Light.LightingEngine`, `TileLightScanner`,
//! `LightMap`) against the real game: ten areas of the reference world (`tools/refdump_light`; file layout mirrored
//! from `terraria-sim/tests/lighting_reference_world.rs`).
//!
//! Per area, as the harness did (RefDumpLight.cs:400-480): write the dumped tiles, set the recorded `Main` state,
//! `Lighting.Initialize()`, seed the three `FastRandom`s of the engine, queue the lights, set
//! `SceneMetrics.PerspectivePlayer`, then per cycle `engine._state = Scan` and `Lighting.LightTiles(area)` twice.
//! Compared for every tile of the processed area: the engine's raw `Vector3` (bit for bit) and
//! `Lighting.GetColor(x, y).PackedValue`.

use crate::game::{self, Report, RECORD};
use terraria_core::xna::{Color, Rectangle};
use terraria_gen::gen::terraria;
use terraria_gen::rt::Arr2;

const NAMES: [&str; 10] =
    ["spawn", "cavern", "hell_t0", "hell", "edge_left", "edge_bottom", "scramble_surface", "scramble_deep", "scramble_hell", "scramble_shimmer"];

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
    /// (x, y, raw, packed)
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

struct AreaResult {
    tiles: usize,
    raw_exact: usize,
    packed_exact: usize,
    max_ulp: u32,
    worst_abs: f32,
    non_black: usize,
    first_raw: Option<String>,
    first_packed: Option<String>,
}

unsafe fn run_area(name: &str, light: &Light, notes: &mut Vec<String>) -> AreaResult {
    let main = terraria::main::s();
    // ---- the documented state (RefDumpLight.cs:416-427)
    (*main).game_menu = false;
    (*main).net_mode = 0;
    (*main).game_paused = false;
    (*main).map_enabled = false;
    (*main).day_time = light.day_time;
    (*main).time = light.time;
    (*main).tile_color = light.tile_color;
    (*main).mouse_text_color = light.mouse_text_color;
    (*main).time_for_visual_effects = light.time_for_visual_effects;
    (*main).global_time_wrapped_hourly = light.global_time_wrapped_hourly;
    (*main).water_style = light.water_style;
    (*main)._game_update_count = light.game_update_count;
    // ---- Main statics the harness only read: the translated initialisers must have produced the same values
    let mut note = |what: &str, same: bool| {
        if !same {
            notes.push(format!("{name}: {what} differs from the value the game held"));
        }
    };
    note("Main.demonTorch", (*main).demon_torch == light.demon_torch);
    note("Main.DiscoR/G/B", ((*main).disco_r, (*main).disco_g, (*main).disco_b) == light.disco);
    note("Main.sundialCooldown", (*main).sundial_cooldown == light.sundial_cooldown);
    note("Main.moondialCooldown", (*main).moondial_cooldown == light.moondial_cooldown);
    note("Main.shimmerDarken", (*main).shimmer_darken == light.shimmer_darken);
    note("Main.cageFrames", (*main).cage_frames == light.cage_frames);
    note("Main.raining", (*main).raining == light.raining);
    note("Main.myPlayer", (*main).my_player == light.my_player);
    note("Main.mapDelay", (*main).map_delay == light.map_delay);
    note("Main.liquidAlpha", (*main).liquid_alpha.as_slice() == light.liquid_alpha.as_slice());
    // Main.jellyfishCageMode is animation state the game had advanced; the harness recorded it, use the game's
    let jc: Arr2<u8> = (*main).jellyfish_cage_mode;
    for (i, row) in light.jellyfish_cage_mode.iter().enumerate() {
        for (j, v) in row.iter().enumerate() {
            *jc.at(i as i32, j as i32) = *v;
        }
    }
    terraria::lighting::initialize();
    let engine = (*terraria::lighting::s()).new_engine;
    let e = engine.ptr();
    (*(*e)._tile_scanner.ptr())._random = terraria::utilities::fast_random::new__ulong(light.seeds.0);
    (*(*e)._active_light_map.ptr())._random = terraria::utilities::fast_random::new__ulong(light.seeds.1);
    (*(*e)._working_light_map.ptr())._random = terraria::utilities::fast_random::new__ulong(light.seeds.2);
    for &(x, y, r, g, b) in &light.lights {
        terraria::lighting::add_light__int_int_float_float_float(x, y, r, g, b);
    }
    // RefDumpLight.cs:462-470: SceneMetrics.PerspectivePlayer = Main.player[Main.myPlayer]
    let metrics = terraria::main::scene_metrics();
    (*metrics.ptr()).perspective_player__backing = *(*main).player.at((*main).my_player);
    for _ in 0..light.cycles {
        (*e)._state = 2; // EngineState.Scan
        terraria::lighting::light_tiles(light.area); // Scan
        terraria::lighting::light_tiles(light.area); // Blur + Present
    }
    let mut res = AreaResult { tiles: light.tiles.len(), raw_exact: 0, packed_exact: 0, max_ulp: 0, worst_abs: 0.0, non_black: 0, first_raw: None, first_packed: None };
    for &(x, y, v, packed) in &light.tiles {
        let mine = terraria::graphics::light::lighting_engine::get_color(e, x, y);
        if mine.x != 0.0 || mine.y != 0.0 || mine.z != 0.0 {
            res.non_black += 1;
        }
        let m = [mine.x, mine.y, mine.z];
        if m.iter().zip(&v).all(|(a, b)| a.to_bits() == b.to_bits()) {
            res.raw_exact += 1;
        } else {
            for k in 0..3 {
                res.max_ulp = res.max_ulp.max((m[k].to_bits() as i64 - v[k].to_bits() as i64).unsigned_abs() as u32);
                res.worst_abs = res.worst_abs.max((m[k] - v[k]).abs());
            }
            if res.first_raw.is_none() {
                res.first_raw = Some(format!("({x},{y}) here {m:?} game {v:?}"));
            }
        }
        let p = terraria::lighting::get_color__int_int(x, y).packed_value();
        if p == packed {
            res.packed_exact += 1;
        } else if res.first_packed.is_none() {
            res.first_packed = Some(format!("({x},{y}) here {p:08x} game {packed:08x}"));
        }
    }
    let _ = light.processed;
    let _ = light.scene_state;
    res
}

pub fn run(report: &mut Report) {
    let dir = game::root().join("refdata/light");
    if !dir.join("spawn.light.bin").exists() {
        report.fail(format!("no dump in {} (tools\\refdump_light\\run.cmd)", dir.display()));
        return;
    }
    let Some((w, h)) = (unsafe { game::load_reference_world(report) }) else { return };
    unsafe { game::apply_liquid("ref_small") };
    report.line(format!(
        "{:<18} {:>7} {:>10} {:>10} {:>9} {:>11} {:>10}   {}",
        "area", "tiles", "raw exact", "raw diff", "max ulp", "max |diff|", "GetColor", "GetColor mismatches"
    ));
    let mut notes: Vec<String> = Vec::new();
    let (mut total, mut total_packed_bad) = (0usize, 0usize);
    for name in NAMES {
        let light = read_light(&std::fs::read(dir.join(format!("{name}.light.bin"))).unwrap());
        let tiles_bytes = std::fs::read(dir.join(format!("{name}.tiles.bin"))).unwrap();
        let mut c = Cur { b: &tiles_bytes, at: 0 };
        assert_eq!(c.take(4), b"TILE");
        assert_eq!(c.i32(), 1);
        let (tx, ty, tw, th) = (c.i32(), c.i32(), c.i32(), c.i32());
        assert!(tx >= 0 && ty >= 0 && tx + tw <= w && ty + th <= h, "{name}: dumped tile region outside the world");
        // the tile input is the dump (scrambled areas exist only there)
        unsafe {
            for x in tx..tx + tw {
                for y in ty..ty + th {
                    game::write_record(game::tile(x, y).ptr(), c.take(RECORD));
                }
            }
        }
        assert_eq!(c.at, tiles_bytes.len());
        match game::run(|| unsafe { run_area(name, &light, &mut notes) }) {
            Err(e) => report.fail(format!("lighting {name}: {e}")),
            Ok(r) => {
                total += r.tiles;
                total_packed_bad += r.tiles - r.packed_exact;
                report.line(format!(
                    "{:<18} {:>7} {:>10} {:>10} {:>9} {:>11.3e} {:>10}   {}",
                    name,
                    r.tiles,
                    r.raw_exact,
                    r.tiles - r.raw_exact,
                    r.max_ulp,
                    r.worst_abs,
                    r.packed_exact,
                    r.tiles - r.packed_exact
                ));
                if let Some(f) = &r.first_raw {
                    report.line(format!("    first raw difference: {f}"));
                }
                if let Some(f) = &r.first_packed {
                    report.line(format!("    first GetColor difference: {f}"));
                }
                if r.non_black == 0 {
                    report.fail(format!("lighting {name}: every tile is black: the engine did not run"));
                }
            }
        }
    }
    notes.sort();
    notes.dedup();
    for n in &notes {
        report.line(format!("  note: {n}"));
    }
    report.line(format!("total: {total} tiles in {} areas; Lighting.GetColor differs on {total_packed_bad}", NAMES.len()));
    if total_packed_bad != 0 {
        report.fail(format!("lighting: Lighting.GetColor differs from the real game on {total_packed_bad} of {total} tiles"));
    }
}
