//! Draw output against the ORIGINAL client: the calls the translated game makes at the XNA graphics API boundary,
//! frame by frame, compared line by line with what the real `Terraria.exe` makes in the same scenes
//! (`tools/refdump_draw/RefDumpDraw.cs`, data in `refdata/draw`; format: `terraria_host::gfx::trace`).
//!
//! The scenario is the reference harness's `Scenario()`, step for step, with the same pins: idle input (mouse at
//! 640, 360), `Environment.TickCount`, `DateTime.Now` and the stopwatch clock fixed, window 1280 x 720 active on a 1920 x 1080 display, 1/60 s per tick,
//! config.json with RGB devices off and music / sound / ambient volume 0, `-savedirectory <scratch>`,
//! `Main.SkipAssemblyLoad`, the asset loader's queue served 64 actions after every `Update` (the host's default).
//!
//! Output: `target-gen/draw/<scene>.trace` (the port's lines), `target-gen/draw/run.digest` (every frame: frame,
//! lines, FNV-1a 64), one `==== draw <scene>:` line per scene, and the first frame whose digest differs from the
//! reference's. The test fails on any difference in a recorded scene.
//!
//! `DRAW_SCENES=a,b`: only these scenes are compared (all are still run: they follow one another).
//! `DRAW_REF=<dir>`: the reference directory (default `refdata/draw`, `refdata/audio` in the audio mode).
//!
//! `DRAW_MODE=audio`: the audio run (`RefDumpDraw.exe <dir> audio`, refdata/audio): the same scenario with sound,
//! music and ambience on (config.json volumes 0.75 / 1 / 0.5), menu clicks after the title scene and the audio scene
//! at the end (walking on grass, mining, an enemy hit, item use, music across day / night and biomes; scripted
//! input through the input snapshot as the reference scripts XNA's Mouse / Keyboard). Besides the draw scenes it
//! compares every call the game's audio makes at the engine boundary (`terraria_host::audio::trace`, AudioRec.cs:
//! SoundEffect / instance / XACT calls with the derived XAudio2 parameters), all frames of the run, line by line
//! (`target-gen/audio/run.audio`), and the per-frame audio digest; and it requires that no XACT cue ran out by itself
//! (the reference engine model has no such event, AudioRec.cs).
//!
//! `DRAW_MODE=scenes`: the scenes run (`RefDumpDraw.exe <dir> scenes`, refdata/scenes): the multiplayer menu, the
//! character and world lists, world creation, nine biomes, rain / blood moon / eclipse / slime rain / goblin army /
//! sandstorm, the settings pages, crafting, housing, NPC chat and shop, bestiary, emotes, a chest, camera mode, a
//! mount, pets and a minion, the smart cursor, the retro / trippy / white lighting modes and all 18 bosses; each scene
//! 2 frames call by call, and the digest of every frame of the run.
//!
//! `DRAW_MODE=net`: the net run's client (`RefDumpDraw.exe <dir> net <pipe>`, NetClient.cs, refdata/net; tests/draw/net.rs):
//! the real client joined the original server (tools/refdump_net) in lock step; here the server is what it sent
//! (client_in.stream), and every frame's bytes the client sends are compared with the original client's
//! (client_out.stream), besides the scenes, digests and state lines.
//!
//! `DRAW_MODE=interact`: the interaction run (`RefDumpDraw.exe <dir> interact`, Interact.cs, refdata/interact;
//! tests/draw/interact.rs): every held / channelled weapon used, a wired contraption, chest operations, the Old One's
//! Army, shimmer, town NPCs arriving, the player's states and UI details, fishing.
//!
//! `DRAW_MODE=gallery`: the gallery run (`RefDumpDraw.exe <dir> gallery`, Gallery.cs, refdata/gallery; the steps in
//! tests/draw/gallery.rs): the old background drawing, every particle, every ambient sky entity, every projectile and
//! NPC type drawn next to the player, world events (meteor, a shaken tree, hardmode, an altar, the pillars), the
//! player's death and respawn; scenes, frame digests and state lines as in the scenes run.

#![allow(unused_mut, unused_variables, unused_assignments, unused_unsafe, unused_parens, unused_braces, non_snake_case, unused_imports)]

use std::cell::Cell;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use terraria_gen_game as game;
use terraria_gen_game::gen::*;
use terraria_gen_game::rt::{self, *};
use terraria_gen_game::s;
use terraria_host::game as hg;
use terraria_host::audio::trace as atrace;
use terraria_host::gfx::trace;

const WIDTH: i32 = 1280;
const HEIGHT: i32 = 720;
/// RefDumpDraw.cs `TickCount`
const TICK_COUNT: i32 = 1_000_000;

fn window() -> hg::WindowState {
    hg::WindowState { width: WIDTH, height: HEIGHT, x: 0, y: 0, active: true, handle: 0, display_width: 1920, display_height: 1080, scale: 1.0 }
}

fn project_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..").canonicalize().expect("project root")
}

/// The scenario's input (RefDumpDraw.cs `InMouseX` / `InMouseY` / `InLeft` / `InKeys`; idle by default).
struct Input {
    x: i32,
    y: i32,
    left: bool,
    /// `Keys` values (D 68, D1 49, D2 50, D4 52)
    keys: Vec<i32>,
}

struct Run {
    main: Gc<terraria::main::Main>,
    frame: u32,
    digest: String,
    audio: bool,
    /// the scenes run (`DRAW_MODE=scenes`)
    scenes: bool,
    /// the gallery run (`DRAW_MODE=gallery`, tests/draw/gallery.rs)
    gallery: bool,
    /// the net run (`DRAW_MODE=net`, tests/draw/net.rs)
    net: Option<NetRun>,
    /// the interaction run (`DRAW_MODE=interact`, tests/draw/interact.rs)
    interact: bool,
    /// the coverage run (`DRAW_MODE=cover`, tests/draw/cover.rs)
    cover: bool,
    /// the probe run (`DRAW_MODE=probe`, tests/draw/probe.rs)
    probe: bool,
    /// the scenes run's state lines (`state_line`)
    state: String,
    audio_digest: String,
    input: Input,
    out_dir: PathBuf,
    ref_dir: PathBuf,
    only: Option<Vec<String>>,
    summary: Vec<String>,
    failures: Vec<String>,
}

thread_local! {
    static LABELLED_AT: Cell<i32> = const { Cell::new(-1) };
}

/// RefDumpDraw.cs `LabelAssets`: content textures, effects and font pages by their name in the asset repository.
fn label_assets() {
    unsafe {
        let assets = (*terraria::main::s()).assets;
        if !rt::obj_is::<re_logic::content::asset_repository::AssetRepository>(assets) {
            return;
        }
        let repo = Gc(assets.0 as *mut re_logic::content::asset_repository::AssetRepository);
        let loaded = (*repo.ptr()).loaded_assets__backing;
        if loaded == LABELLED_AT.get() {
            return;
        }
        LABELLED_AT.set(loaded);
        let pairs = Dictionary::to_pair_array((*repo.ptr())._assets);
        for kv in pairs.as_slice() {
            let v = kv.value;
            let name = kv.key.as_str().replace('\\', "/");
            if rt::obj_is::<re_logic::content::asset__texture2_d::Asset>(v) {
                let t = (*(v.0 as *mut re_logic::content::asset__texture2_d::Asset)).value__backing;
                if !t.is_null() {
                    trace::set_texture_label((*t.ptr()).id, &name);
                }
            } else if rt::obj_is::<re_logic::content::asset__effect::Asset>(v) {
                let e = (*(v.0 as *mut re_logic::content::asset__effect::Asset)).value__backing;
                if !e.is_null() {
                    trace::set_effect_label(e, &name);
                }
            } else if rt::obj_is::<re_logic::content::asset__dynamic_sprite_font::Asset>(v) {
                let f = (*(v.0 as *mut re_logic::content::asset__dynamic_sprite_font::Asset)).value__backing;
                if f.is_null() {
                    continue;
                }
                let chars = (*f.ptr())._sprite_characters;
                if chars.is_null() {
                    continue;
                }
                let mut seen: Vec<Gc<terraria_host::gfx::Texture2D>> = Vec::new();
                for c in chars.as_slice() {
                    if c.is_null() {
                        continue;
                    }
                    let t = (*c.ptr()).texture;
                    if t.is_null() || seen.contains(&t) {
                        continue;
                    }
                    seen.push(t);
                    trace::set_texture_label((*t.ptr()).id, &format!("{name}#{}", seen.len() - 1));
                }
            }
        }
    }
}

impl Run {
    /// One tick of the driver protocol with an idle input (tests/client.rs `frames_with`, no script).
    unsafe fn tick(&mut self) {
        self.net_pre();
        self.frame += 1;
        trace::frame_start();
        atrace::frame_start();
        if self.audio {
            atrace::write_raw(&format!("frame {}\n", self.frame));
        }
        let snapshot = terraria_host::input::InputSnapshot { mouse_x: self.input.x, mouse_y: self.input.y, left: self.input.left, keys_down: self.input.keys.clone(), ..Default::default() };
        hg::begin_frame(snapshot, window(), 1.0 / 60.0);
        hg::dispatch_events();
        while let Some(t) = hg::begin_update() {
            terraria::main::update(self.main.ptr(), t);
            hg::end_update();
        }
        let dt = hg::begin_draw();
        if let Some(t) = dt {
            terraria::main::draw(self.main.ptr(), t);
        }
        let _ = hg::end_draw(dt.is_some());
        let (lines, hash) = trace::frame_digest();
        let _ = writeln!(self.digest, "{} {lines} {hash:016x}", self.frame);
        if self.audio {
            let (lines, hash) = atrace::frame_digest();
            let _ = writeln!(self.audio_digest, "{} {lines} {hash:016x}", self.frame);
        }
        // the collector between frames (as the app shell; never while queued work is suspended)
        if self.frame % 120 == 0 && !terraria_host::sys::threading::has_suspended_work() {
            rt::collect();
        }
        self.fx_trace();
        self.net_post();
    }
    /// RefDumpDraw.cs `FxTrace` (`DRAW_FXTRACE=<first>,<last>`, diagnosis): every active gore and dust after the frame.
    unsafe fn fx_trace(&mut self) {
        let Some((first, last)) = std::env::var("DRAW_FXTRACE").ok().and_then(|v| {
            let mut p = v.split(',');
            Some((p.next()?.parse::<u32>().ok()?, p.next()?.parse::<u32>().ok()?))
        }) else {
            return;
        };
        if self.frame < first || self.frame > last {
            return;
        }
        let m = terraria::main::s();
        let fb = |f: f32| format!("{:08x}", f.to_bits());
        let mut out = format!("frame {}\n", self.frame);
        for (i, g) in (*m).gore.as_slice().iter().enumerate() {
            if g.is_null() || !(*g.ptr()).active {
                continue;
            }
            let g = &*g.ptr();
            out.push_str(&format!("gore {i} {} {},{} {},{} {} {} {}\n", g.type_, fb(g.position.x), fb(g.position.y), fb(g.velocity.x), fb(g.velocity.y), fb(g.rotation), fb(g.scale), g.alpha));
        }
        for (i, d) in (*m).dust.as_slice().iter().enumerate() {
            if d.is_null() || !(*d.ptr()).active {
                continue;
            }
            let d = &*d.ptr();
            out.push_str(&format!(
                "dust {i} {} {},{} {},{} {} {} g{} f{} a{} fr{},{} c{:08x}\n",
                d.type_,
                fb(d.position.x),
                fb(d.position.y),
                fb(d.velocity.x),
                fb(d.velocity.y),
                fb(d.rotation),
                fb(d.scale),
                d.no_gravity as i32,
                fb(d.fade_in),
                d.alpha,
                d.frame.x,
                d.frame.y,
                d.color.packed_value()
            ));
        }
        use std::io::Write as _;
        let mut f = std::fs::OpenOptions::new().create(true).append(true).open(self.out_dir.join("fx.trace")).expect("fx.trace");
        let _ = f.write_all(out.as_bytes());
    }
    /// `DRAW_EXTRA=<first>,<count>` (diagnosis): the lines of those frames go to `extra.trace` (RefDumpDraw.cs the same).
    unsafe fn frames(&mut self, n: u32) {
        let extra: Option<(u32, u32)> = std::env::var("DRAW_EXTRA").ok().and_then(|v| {
            let mut p = v.split(',');
            Some((p.next()?.parse().ok()?, p.next()?.parse().ok()?))
        });
        for _ in 0..n {
            let rec = extra.is_some_and(|(first, count)| self.frame + 1 >= first && self.frame + 1 < first + count);
            if !rec {
                self.tick();
                continue;
            }
            trace::set_full(true);
            let _ = trace::take_output();
            let head = format!("frame {}\n", self.frame + 1);
            self.tick();
            trace::set_full(false);
            use std::io::Write as _;
            let mut f = std::fs::OpenOptions::new().create(true).append(true).open(self.out_dir.join("extra.trace")).expect("extra.trace");
            let _ = f.write_all(head.as_bytes());
            let _ = f.write_all(&trace::take_output());
        }
    }
    fn note(&mut self, s: String) {
        let s = format!("[{}] {s}", self.frame);
        eprintln!("{s}");
        self.summary.push(s);
    }

    /// RefDumpDraw.cs `Record`: `n` frames with every line kept, then compared with the reference.
    unsafe fn record(&mut self, scene: &str, n: u32) {
        trace::set_full(true);
        let _ = trace::take_output();
        let mut body: Vec<u8> = Vec::new();
        for _ in 0..n {
            body.extend_from_slice(format!("frame {}\n", self.frame + 1).as_bytes());
            self.tick();
            body.extend_from_slice(&trace::take_output());
        }
        trace::set_full(false);
        std::fs::write(self.out_dir.join(format!("{scene}.trace")), &body).expect("write trace");
        if self.scenes || self.gallery || self.interact || self.cover || self.probe || self.net.is_some() {
            self.state.push_str(&state_line(scene));
        }
        self.compare_scene(scene, body, &format!("{n} frames"));
    }

    /// The lines of a scene against the reference's `<scene>.trace.zlib` (Record and the probe run's direct draws).
    fn compare_scene(&mut self, scene: &str, body: Vec<u8>, what: &str) {
        let port = String::from_utf8(body).expect("utf-8");
        let wanted = self.only.as_ref().is_none_or(|o| o.iter().any(|s| s == scene));
        if !wanted {
            self.note(format!("draw {scene}: not compared (DRAW_SCENES)"));
            return;
        }
        let ref_path = self.ref_dir.join(format!("{scene}.trace.zlib"));
        let reference = match std::fs::read(&ref_path) {
            Ok(z) => String::from_utf8(miniz_oxide::inflate::decompress_to_vec_zlib(&z).expect("zlib")).expect("utf-8"),
            Err(e) => {
                self.failures.push(format!("{scene}: no reference {} ({e})", ref_path.display()));
                return;
            }
        };
        let (r, p): (Vec<&str>, Vec<&str>) = (reference.lines().collect(), port.lines().collect());
        let calls = r.iter().filter(|l| !l.starts_with("frame ")).count();
        let mut frame = "";
        let mut first = None;
        for i in 0..r.len().max(p.len()) {
            let (a, b) = (r.get(i).copied(), p.get(i).copied());
            if let Some(l) = a {
                if l.starts_with("frame ") {
                    frame = l;
                }
            }
            if a != b {
                first = Some((i, a.unwrap_or("<end>").to_owned(), b.unwrap_or("<end>").to_owned()));
                break;
            }
        }
        match first {
            None => {
                let s = format!("==== draw {scene}: {calls} of {calls} calls exact ({what})");
                eprintln!("{s}");
                self.summary.push(s);
            }
            Some((i, a, b)) => {
                let exact = r[..i].iter().filter(|l| !l.starts_with("frame ")).count();
                let s = format!("==== draw {scene}: {exact} of {calls} calls exact, first divergence at line {} ({frame}):\n  original: {}\n  port:     {}", i + 1, a.chars().take(300).collect::<String>(), b.chars().take(300).collect::<String>());
                eprintln!("{s}");
                self.summary.push(s.clone());
                self.failures.push(s);
            }
        }
    }
}

unsafe fn local_player() -> Gc<terraria::player::Player> {
    let m = terraria::main::s();
    *(*m).player.at((*m).my_player)
}

/// RefDumpDraw.cs `PlacePlayer`: the player stands at tile (x, y) (its feet), not moving; the camera follows it.
unsafe fn place_player(tx: i32, ty: i32) {
    let p = local_player().ptr();
    let (w, h) = ((*p).base.width, (*p).base.height);
    (*p).base.position = terraria_core::xna::Vector2 { x: (tx * 16 + 8 - w / 2) as f32, y: (ty * 16 - h) as f32 };
    (*p).base.velocity = terraria_core::xna::Vector2 { x: 0.0, y: 0.0 };
    (*p).fall_start = ty - h / 16;
}

/// `SoundEngine.PlaySound(type, -1, -1, 1, 1f, 0f)`
unsafe fn play_sound(t: i32) {
    let _ = terraria::audio::sound_engine::play_sound__int_int_int_int_float_float(t, -1, -1, 1, 1.0, 0.0);
}

/// RefDumpDraw.cs `EnterWorld`.
unsafe fn enter_world(r: &mut Run) {
    let m = terraria::main::s();
    // the character creation's end, the player list's load and selection, the world list's PlayGame
    let player = (*m).pending_player;
    (*player.ptr()).name = s!("Ref");
    let created = terraria::io::player_file_data::create_and_save(player);
    let pfd = terraria::player::get_file_data(terraria::io::file_data::path(&mut (*created.ptr()).base), false);
    terraria::main::select_player(pfd);
    terraria::main::load_worlds();
    if r.scenes {
        scenes_world_menus(r);
    }
    let wfd = terraria::io::world_file::get_all_metadata(Str::new(&format!("{}\\ref_small.wld", (*m).world_path.as_str())), false);
    terraria::io::world_file_data::set_as_active(wfd.ptr());
    play_sound(10);
    terraria::main::clr_input();
    let _ = terraria::main::get_input_text(s!(""), false);
    (*m).menu_mode = 10;
    // WorldGen.playWorldCallBack, on this thread
    for i in 0..255 {
        if i != (*m).my_player {
            (*(*(*m).player.at(i)).ptr()).active = false;
        }
    }
    terraria::main::toggle_gameplay_updates(false);
    terraria::io::world_file::load_world();
    assert!(!(*terraria::world_gen::s()).load_failed, "the world did not load");
    if (*m).map_enabled {
        terraria::map::world_map::load((*m).map.ptr());
    }
    if (*m).net_mode != 2 {
        terraria::world_sections::set_all_sections_loaded((*m).section_manager.ptr());
    }
    if (*m).net_mode == 0 && (*(*m).angler_who_finished_today.ptr()).contains((*local_player().ptr()).name) {
        (*m).angler_quest_finished = true;
    }
    (*m).game_menu = false;
    terraria::player::spawn(local_player().ptr(), 1 /* PlayerSpawnContext.SpawningIntoWorld */);
    terraria::io::world_file::set_ongoing_to_temps();
    play_sound(11);
    (*m).reset_clouds = true;
}

unsafe fn set_time(day: bool, time: f64) {
    (*terraria::main::s()).day_time = day;
    (*terraria::main::s()).time = time;
}

/// RefDumpDraw.cs `Scenario`, step for step.
unsafe fn scenario(r: &mut Run) {
    // 1. start-up: splash (quick: music volume 0, assembly load skipped) into the title menu; its first frames recorded
    r.record("boot", 3);
    r.frames(297);
    let m = &*terraria::main::s();
    let s = format!("after start-up: showSplash {} gameMenu {} menuMode {}", if m.show_splash { "True" } else { "False" }, if m.game_menu { "True" } else { "False" }, m.menu_mode);
    r.note(s);
    // 2. title menu
    r.record("title", 3);
    if r.audio {
        menu_clicks(r);
    }
    if r.scenes {
        scenes_menus(r);
    }
    // 3. character creation: the player list opens (Main.LoadPlayers), then UICharacterSelect.NewCharacterClick
    terraria::main::load_players();
    play_sound(10);
    (*terraria::main::s()).pending_player = terraria::player::new();
    (*terraria::main::s()).menu_mode = 888;
    let cc = terraria::game_content::ui::states::ui_character_creation::new((*terraria::main::s()).pending_player);
    terraria::ui::user_interface::set_state((*terraria::main::s()).menu_ui.ptr(), cc.up());
    r.frames(30);
    r.record("charcreate", 3);
    if r.net.is_some() {
        net_scenario(r);
        return;
    }
    // 4. into the world (the new character, ref_small.wld), noon
    enter_world(r);
    let s = format!("in the world: gameMenu {} spawn {},{}", if (*terraria::main::s()).game_menu { "True" } else { "False" }, (*terraria::main::s()).spawn_tile_x, (*terraria::main::s()).spawn_tile_y);
    r.note(s);
    if r.scenes {
        scenes_world(r);
        return;
    }
    if r.gallery {
        gallery_world(r);
        return;
    }
    if r.interact {
        interact_world(r);
        r.record("interact_end", 2);
        return;
    }
    if r.probe {
        probe_world(r);
        r.record("probe_end", 2);
        return;
    }
    if r.cover {
        cover_world(r);
        r.record("cover_end", 2);
        return;
    }
    set_time(true, 27000.0);
    r.frames(60);
    r.record("world_day", 3);
    // 5. the same place at midnight (moon, stars)
    set_time(false, 16200.0);
    r.frames(60);
    r.record("world_night", 3);
    // 6. on the sea floor of the ocean at the left edge of the world (water, the surface above), noon
    set_time(true, 27000.0);
    place_player(150, 249);
    r.frames(60);
    r.record("ocean", 3);
    // 7. a cave below the rock layer (dark: the lighting engine)
    place_player(1999, 547);
    r.frames(60);
    r.record("cave", 3);
    // 8. the underworld (lava, its light)
    place_player(480, 1009);
    r.frames(60);
    r.record("underworld", 3);
    // 9. back at the spawn at night: the Eye of Cthulhu summoned (NPC.SpawnOnPlayer) and projectiles of the player
    set_time(false, 16200.0);
    place_player((*terraria::main::s()).spawn_tile_x, (*terraria::main::s()).spawn_tile_y);
    r.frames(30);
    terraria::npc::spawn_on_player((*terraria::main::s()).my_player, 4, 0.0, 0.0, 0.0, 0.0);
    {
        let c = (*local_player().ptr()).base.position;
        for (k, ty) in BOSS_PROJECTILES.iter().enumerate() {
            let k = k as f32;
            let _ = terraria::projectile::new_projectile__i_entity_source_vector2_vector2_int_int_float_int_float_float_float_new_projectile_modifier(
                terraria::data_structures::entity_source_debug_command::new().up::<rt::Object>(),
                terraria_core::xna::Vector2 { x: c.x + 10.0, y: c.y - 20.0 - 8.0 * k },
                terraria_core::xna::Vector2 { x: 4.0 + k, y: -1.5 * k },
                *ty,
                20,
                2.0,
                (*terraria::main::s()).my_player,
                0.0,
                0.0,
                0.0,
                rt::zero::<rt::Delegate<dyn Fn(Gc<terraria::projectile::Projectile>)>>(),
            );
        }
    }
    r.frames(20);
    r.record("boss", 3);
    // 10. the inventory open (Player.ToggleInv)
    terraria::player::toggle_inv(local_player().ptr());
    r.frames(10);
    r.record("inventory", 3);
    // 11. the full-screen map (Player.TryOpeningFullscreenMap; the minimap is in every scene above)
    terraria::player::try_opening_fullscreen_map(local_player().ptr());
    r.frames(10);
    r.record("map", 3);
    if r.audio {
        audio_scene(r);
    }
}



/// RefDumpDraw.cs `StateLine`: the scenes run's state after a recorded scene, the same bytes.
unsafe fn state_line(scene: &str) -> String {
    let m = terraria::main::s();
    if (*m).game_menu {
        return format!("state {scene} menu\n");
    }
    fn i4(mut h: u64, v: i32) -> u64 {
        for k in 0..4 {
            h ^= ((v >> (8 * k)) & 0xff) as u64;
            h = h.wrapping_mul(1099511628211);
        }
        h
    }
    let fb = |f: f32| f.to_bits() as i32;
    let (inext, sa) = (*(*m).rand.ptr()).state();
    let mut rh = 14695981039346656037u64;
    for v in sa {
        rh = i4(rh, v);
    }
    let pl = local_player().ptr();
    let (pp, pv) = ((*pl).base.position, (*pl).base.velocity);
    let (mut nh, mut ph) = (14695981039346656037u64, 14695981039346656037u64);
    let (mut nn, mut np) = (0, 0);
    for (i, n) in (*m).npc.as_slice().iter().enumerate() {
        let n = n.ptr();
        if !(*n).active {
            continue;
        }
        nn += 1;
        let (p, v) = ((*n).base.position, (*n).base.velocity);
        nh = i4(i4(i4(i4(i4(i4(i4(nh, i as i32), (*n).type_), fb(p.x)), fb(p.y)), fb(v.x)), fb(v.y)), (*n).life);
        for a in (*n).ai.as_slice() {
            nh = i4(nh, fb(*a));
        }
    }
    for (i, n) in (*m).projectile.as_slice().iter().enumerate() {
        let n = n.ptr();
        if !(*n).active {
            continue;
        }
        np += 1;
        let (p, v) = ((*n).base.position, (*n).base.velocity);
        ph = i4(i4(i4(i4(i4(i4(ph, i as i32), (*n).type_), fb(p.x)), fb(p.y)), fb(v.x)), fb(v.y));
        for a in (*n).ai.as_slice() {
            ph = i4(ph, fb(*a));
        }
        ph = i4(ph, (*n).time_left);
    }
    format!(
        "state {scene} rand {inext} {rh:016x} player {:08x},{:08x},{:08x},{:08x},{},{} npc {nn}:{nh:016x} proj {np}:{ph:016x}\n",
        fb(pp.x),
        fb(pp.y),
        fb(pv.x),
        fb(pv.y),
        (*pl).stat_life,
        (*pl).stat_mana
    )
}

// ================================================================== the scenes run (RefDumpDraw.cs ScenesMenus / ScenesWorldMenus / ScenesWorld)

/// RefDumpDraw.cs `ScenesMenus`.
unsafe fn scenes_menus(r: &mut Run) {
    let m = terraria::main::s();
    (*m).menu_mode = 12;
    r.frames(10);
    r.record("menu_multiplayer", 2);
    (*m).menu_mode = 0;
    r.frames(5);
    terraria::main::load_players();
    terraria::main::open_character_select_ui();
    r.frames(20);
    r.record("menu_characters", 2);
    terraria::ui::user_interface::set_state((*m).menu_ui.ptr(), Gc::NULL);
    (*m).menu_mode = 0;
    r.frames(5);
}

/// RefDumpDraw.cs `ScenesWorldMenus` (from enter_world, after the worlds are loaded).
unsafe fn scenes_world_menus(r: &mut Run) {
    let m = terraria::main::s();
    terraria::main::open_world_select_ui();
    r.frames(20);
    r.record("menu_worlds", 2);
    terraria::ui::user_interface::set_state((*m).menu_ui.ptr(), terraria::game_content::ui::states::ui_world_creation::new().up());
    r.frames(20);
    r.record("menu_worldcreation", 2);
    terraria::ui::user_interface::set_state((*m).menu_ui.ptr(), Gc::NULL);
    (*m).menu_mode = 0;
    r.frames(5);
}

/// RefDumpDraw.cs `Biomes`.
const BIOMES: [(&str, i32, i32); 9] = [
    ("crimson", 1408, 471),
    ("jungle", 309, 340),
    ("snow", 2499, 298),
    ("desert", 1720, 352),
    ("mushroom", 151, 886),
    ("dungeon", 3352, 295),
    ("temple", 790, 527),
    ("marble", 627, 859),
    ("granite", 2342, 446),
];
/// RefDumpDraw.cs `Bosses`.
const BOSSES: [(&str, i32); 18] = [
    ("kingslime", 50),
    ("eye", 4),
    ("brain", 266),
    ("eater", 13),
    ("queenbee", 222),
    ("skeletron", 35),
    ("deerclops", 668),
    ("wall", 113),
    ("queenslime", 657),
    ("twins", 125),
    ("destroyer", 134),
    ("prime", 127),
    ("plantera", 262),
    ("golem", 245),
    ("fishron", 370),
    ("empress", 636),
    ("cultist", 439),
    ("moonlord", 398),
];

unsafe fn clear_entities() {
    let m = terraria::main::s();
    for n in (*m).npc.as_slice() {
        (*n.ptr()).active = false;
    }
    for p in (*m).projectile.as_slice() {
        (*p.ptr()).active = false;
    }
}
unsafe fn new_npc_at(x: i32, y: i32, ty: i32) -> i32 {
    terraria::npc::new_npc(terraria::data_structures::entity_source_debug_command::new().up::<rt::Object>(), x, y, ty, 0, 0.0, 0.0, 0.0, 0.0, 255)
}

/// RefDumpDraw.cs `ScenesItems`.
unsafe fn scenes_items(r: &mut Run) {
    let m = terraria::main::s();
    let (sx, sy) = ((*m).spawn_tile_x, (*m).spawn_tile_y);
    let pl = local_player();
    set_time(true, 27000.0);
    place_player(sx, sy);
    r.frames(10);
    (*m).player_inventory = true;
    r.input.x = 40;
    r.input.y = 40;
    r.frames(5);
    r.record("ui_tooltip", 2);
    r.input.x = 640;
    r.input.y = 360;
    (*m).player_inventory = false;
    r.frames(5);
    terraria::main::open_hair_window();
    r.frames(10);
    r.record("ui_hair", 2);
    (*m).hair_window = false;
    r.frames(5);
    terraria::main::open_clothes_window();
    r.frames(10);
    r.record("ui_clothes", 2);
    (*m).clothes_window = false;
    r.frames(5);
    for sword in [757, 675, 674] {
        give_item(0, sword);
        r.press(KEY_D1);
        r.input.x = 900;
        r.input.y = 340;
        r.input.left = true;
        r.frames(30);
        r.input.left = false;
        r.record(&format!("item_sword_{sword}"), 2);
        r.frames(20);
    }
    r.input.x = 640;
    r.input.y = 360;
    terraria::item::set_defaults((*(*pl.ptr()).armor.at(3)).ptr(), 493, Gc::NULL);
    terraria::item::set_defaults((*(*pl.ptr()).armor.at(4)).ptr(), 54, Gc::NULL);
    r.frames(5);
    r.input.keys = vec![KEY_SPACE];
    r.frames(40);
    r.record("player_wings", 2);
    r.input.keys.clear();
    r.frames(60);
    r.input.keys = vec![KEY_D];
    r.frames(80);
    r.record("player_boots", 2);
    r.input.keys.clear();
    r.frames(20);
    place_player(sx, sy);
    give_item(0, 509);
    r.press(KEY_D1);
    r.frames(10);
    r.record("item_wrench", 2);
    terraria::player::open_boss_bag(pl.ptr(), 3318);
    r.frames(30);
    r.record("item_bossbag", 2);
    set_time(false, 16200.0);
    place_player(2499, 298);
    r.frames(90);
    r.record("biome_snow_night", 2);
    set_time(true, 27000.0);
    place_player(sx, sy);
    clear_entities();
    r.frames(30);
}
const KEY_SPACE: i32 = 32;

/// RefDumpDraw.cs `ScenesWorld`.
unsafe fn scenes_world(r: &mut Run) {
    let m = terraria::main::s();
    let (sx, sy) = ((*m).spawn_tile_x, (*m).spawn_tile_y);
    let pl = local_player();
    set_time(true, 27000.0);
    place_player(sx, sy);
    give_item(0, 4);
    give_item(1, 1);
    give_item(2, 9);
    (*(*(*pl.ptr()).inventory.at(2)).ptr()).stack = 99;
    r.frames(30);
    for (name, x, y) in BIOMES {
        place_player(x, y);
        r.frames(90);
        r.record(&format!("biome_{name}"), 2);
    }
    place_player(sx, sy);
    r.frames(30);
    terraria::main::start_rain(false, rt::zero::<Nullable<f32>>(), false);
    r.frames(60);
    r.record("event_rain", 2);
    terraria::main::stop_rain(true);
    set_time(false, 16200.0);
    (*m).blood_moon = true;
    r.frames(60);
    r.record("event_bloodmoon", 2);
    (*m).blood_moon = false;
    set_time(true, 27000.0);
    (*m).eclipse = true;
    r.frames(60);
    r.record("event_eclipse", 2);
    (*m).eclipse = false;
    terraria::main::start_slime_rain(false);
    r.frames(90);
    r.record("event_slimerain", 2);
    terraria::main::stop_slime_rain(false);
    clear_entities();
    terraria::main::start_invasion(1);
    r.frames(120);
    r.record("event_goblins", 2);
    (*m).invasion_type = 0;
    (*m).invasion_size = 0;
    clear_entities();
    place_player(1720, 352);
    terraria::game_content::events::sandstorm::start_sandstorm();
    r.frames(120);
    r.record("event_sandstorm", 2);
    terraria::game_content::events::sandstorm::stop_sandstorm();
    place_player(sx, sy);
    r.frames(30);
    // in-game UI
    terraria::ingame_options::open();
    for c in 0..4 {
        (*terraria::ingame_options::s()).category = c;
        r.frames(4);
        r.record(&format!("ui_settings_{c}"), 2);
    }
    terraria::ingame_options::close__bool(true);
    r.frames(5);
    (*m).player_inventory = true;
    terraria::recipe::update_recipe_list();
    r.frames(10);
    r.record("ui_crafting", 2);
    (*m).equip_page = 1;
    r.frames(5);
    r.record("ui_housing", 2);
    (*m).equip_page = 0;
    (*m).player_inventory = false;
    let ppos = (*pl.ptr()).base.position;
    let guide = new_npc_at(ppos.x as i32 + 40, ppos.y as i32 + 40, 22);
    let merchant = new_npc_at(ppos.x as i32 - 40, ppos.y as i32 + 40, 17);
    r.frames(5);
    terraria::player::set_talk_npc__int_2(pl.ptr(), guide);
    (*m).npc_chat_text = terraria::npc::get_chat((*(*m).npc.at(guide)).ptr());
    r.frames(10);
    r.record("ui_chat", 2);
    terraria::player::set_talk_npc__int_2(pl.ptr(), merchant);
    (*m).npc_chat_text = s!("");
    terraria::main::set_npc_shop_index(1);
    (*m).player_inventory = true;
    r.frames(10);
    r.record("ui_shop", 2);
    terraria::player::set_talk_npc__int_2(pl.ptr(), -1);
    terraria::main::set_npc_shop_index(0);
    (*m).player_inventory = false;
    r.frames(5);
    terraria::ui::ingame_fancy_ui::open_ui_state__ui_state_bool((*m).bestiary_ui.up(), true);
    r.frames(20);
    r.record("ui_bestiary", 2);
    terraria::ui::ingame_fancy_ui::close(false);
    r.frames(5);
    terraria::ui::ingame_fancy_ui::open_ui_state__ui_state_bool(terraria::game_content::ui::states::ui_emotes_menu::new().up(), true);
    r.frames(10);
    r.record("ui_emotes", 2);
    terraria::ui::ingame_fancy_ui::close(false);
    r.frames(5);
    {
        let chests = (*m).chest;
        let mut ci = -1i32;
        for (i, c) in chests.as_slice().iter().enumerate() {
            if !c.is_null() && (*c.ptr()).y < 300 {
                ci = i as i32;
                break;
            }
        }
        let ch = *chests.at(ci);
        place_player((*ch.ptr()).x, (*ch.ptr()).y + 2);
        r.frames(10);
        (*pl.ptr()).chest = ci;
        (*m).player_inventory = true;
        r.frames(10);
        r.record("ui_chest", 2);
        (*pl.ptr()).chest = -1;
        (*m).player_inventory = false;
        place_player(sx, sy);
        r.frames(10);
    }
    let cm = (*terraria::graphics::capture::capture_manager::s()).instance;
    terraria::graphics::capture::capture_manager::set_active(cm.ptr(), true);
    r.frames(10);
    r.record("ui_camera", 2);
    terraria::graphics::capture::capture_manager::set_active(cm.ptr(), false);
    r.frames(5);
    let mount = (*pl.ptr()).mount;
    terraria::mount::set_mount(mount.ptr(), 3, pl);
    r.frames(30);
    r.record("player_mount", 2);
    terraria::mount::dismount(mount.ptr(), pl, false);
    r.frames(5);
    terraria::player::add_buff(pl.ptr(), 40, 3600, false);
    terraria::player::add_buff(pl.ptr(), 64, 3600, false);
    let _ = terraria::projectile::new_projectile__i_entity_source_vector2_vector2_int_int_float_int_float_float_float_new_projectile_modifier(
        terraria::data_structures::entity_source_debug_command::new().up::<rt::Object>(),
        terraria_core::xna::Vector2 { x: ppos.x, y: ppos.y - 40.0 },
        terraria_core::xna::Vector2 { x: 0.0, y: 0.0 },
        266,
        10,
        1.0,
        (*m).my_player,
        0.0,
        0.0,
        0.0,
        rt::zero::<rt::Delegate<dyn Fn(Gc<terraria::projectile::Projectile>)>>(),
    );
    r.frames(60);
    r.record("player_pets", 2);
    (*m).smart_cursor_wanted_mouse = true;
    r.press(KEY_D2);
    r.aim(ppos.x + 40.0, ppos.y + 56.0);
    r.frames(10);
    r.record("player_smartcursor", 2);
    (*m).smart_cursor_wanted_mouse = false;
    r.input.x = 640;
    r.input.y = 360;
    r.frames(5);
    place_player(1999, 547);
    // LightMode: White 0, Retro 1, Trippy 2, Color 3
    for (name, mode) in [("retro", 1), ("trippy", 2), ("white", 0)] {
        terraria::lighting::set_mode(mode);
        r.frames(20);
        r.record(&format!("light_{name}"), 2);
    }
    terraria::lighting::set_mode(3);
    place_player(sx, sy);
    r.frames(30);
    scenes_items(r);
    set_time(false, 16200.0);
    for (name, ty) in BOSSES {
        clear_entities();
        let pos = (*pl.ptr()).base.position;
        new_npc_at(pos.x as i32 + 300, pos.y as i32 - 200, ty);
        if ty == 125 {
            new_npc_at(pos.x as i32 - 300, pos.y as i32 - 200, 126);
        }
        r.frames(40);
        r.record(&format!("boss_{name}"), 2);
    }
    clear_entities();
    r.frames(10);
    r.record("scenes_end", 2);
}

// ================================================================== the audio run's own steps (RefDumpDraw.cs, same steps)

/// RefDumpDraw.cs `MenuSingleY` / `MenuMultiY` / `MenuSettingsY`: the title menu's items at x 640.
const MENU_SINGLE_Y: i32 = 240;
const MENU_MULTI_Y: i32 = 292;
const MENU_SETTINGS_Y: i32 = 450;
const KEY_D: i32 = 68;
const KEY_D1: i32 = 49;
const KEY_D2: i32 = 50;
const KEY_D4: i32 = 52;

impl Run {
    /// RefDumpDraw.cs `Press`: a key down for one frame, then up.
    unsafe fn press(&mut self, key: i32) {
        self.input.keys = vec![key];
        self.frames(1);
        self.input.keys.clear();
        self.frames(1);
    }
    /// RefDumpDraw.cs `Click`: the left button down for one frame, then up.
    unsafe fn click(&mut self) {
        self.input.left = true;
        self.frames(1);
        self.input.left = false;
        self.frames(1);
    }
    /// RefDumpDraw.cs `Aim`: the cursor on world position (x, y).
    unsafe fn aim(&mut self, x: f32, y: f32) {
        let sp = (*terraria::main::s()).screen_position;
        self.input.x = (x - sp.x) as i32;
        self.input.y = (y - sp.y) as i32;
    }
}

unsafe fn feet_tile() -> (i32, i32) {
    let p = local_player().ptr();
    let pos = (*p).base.position;
    (((pos.x + ((*p).base.width / 2) as f32) / 16.0) as i32, ((pos.y + (*p).base.height as f32) / 16.0) as i32)
}

/// RefDumpDraw.cs `GiveItem`: `Item.SetDefaults(type, null)` on an inventory slot of the local player.
unsafe fn give_item(slot: i32, ty: i32) {
    let item = *(*local_player().ptr()).inventory.at(slot);
    terraria::item::set_defaults(item.ptr(), ty, Gc::NULL);
}

/// RefDumpDraw.cs `MenuClicks`.
unsafe fn menu_clicks(r: &mut Run) {
    r.input.x = 640;
    for y in [MENU_SINGLE_Y, MENU_MULTI_Y, MENU_SETTINGS_Y] {
        r.input.y = y;
        r.frames(10);
    }
    r.click();
    r.frames(10);
    let s = format!("settings: menuMode {}", (*terraria::main::s()).menu_mode);
    r.note(s);
    play_sound(11);
    (*terraria::main::s()).menu_mode = 0;
    r.frames(10);
    let s = format!("back: menuMode {}", (*terraria::main::s()).menu_mode);
    r.note(s);
    r.input.x = 640;
    r.input.y = 360;
    r.frames(5);
}

/// RefDumpDraw.cs `AudioScene`.
unsafe fn audio_scene(r: &mut Run) {
    let m = terraria::main::s();
    (*m).map_fullscreen = false;
    (*m).player_inventory = false;
    for n in (*m).npc.as_slice() {
        (*n.ptr()).active = false;
    }
    for p in (*m).projectile.as_slice() {
        (*p.ptr()).active = false;
    }
    set_time(true, 27000.0);
    place_player((*m).spawn_tile_x, (*m).spawn_tile_y);
    give_item(0, 4);
    give_item(1, 1);
    give_item(3, 28);
    r.frames(120);
    let s = format!("audio: day at the spawn, curMusic {}", (*m).cur_music);
    r.note(s);
    r.input.keys = vec![KEY_D];
    r.frames(90);
    r.input.keys.clear();
    r.frames(30);
    let (fx, fy) = feet_tile();
    r.note(format!("audio: walked to {fx},{fy}"));
    r.press(KEY_D2);
    let (fx, fy) = feet_tile();
    let (gx, gy) = (fx + 1, fy);
    r.aim((gx * 16 + 8) as f32, (gy * 16 + 8) as f32);
    r.input.left = true;
    r.frames(90);
    r.input.left = false;
    r.frames(10);
    let s = format!("audio: mined at {gx},{gy}, selected {}", terraria::player::selected_item(local_player().ptr()));
    r.note(s);
    {
        let pos = (*local_player().ptr()).base.position;
        let slot = terraria::npc::new_npc(terraria::data_structures::entity_source_debug_command::new().up::<rt::Object>(), pos.x as i32 + 36, pos.y as i32 + 20, 1, 0, 0.0, 0.0, 0.0, 0.0, 255);
        r.press(KEY_D1);
        let slime = *(*m).npc.at(slot);
        for _ in 0..90 {
            let sp = (*slime.ptr()).base.position;
            r.aim(sp.x + 8.0, sp.y + 8.0);
            r.input.left = true;
            r.frames(1);
        }
        r.input.left = false;
        r.frames(30);
        let s = format!("audio: slime {slot} active {} life {}", if (*slime.ptr()).active { "True" } else { "False" }, (*slime.ptr()).life);
        r.note(s);
    }
    {
        let item = *(*local_player().ptr()).inventory.at(3);
        r.press(KEY_D4);
        r.input.x = 700;
        r.input.y = 360;
        r.click();
        r.frames(40);
        let s = format!("audio: potion stack {}", (*item.ptr()).stack);
        r.note(s);
    }
    r.input.x = 640;
    r.input.y = 360;
    set_time(false, 16200.0);
    r.frames(120);
    let s = format!("audio: night, curMusic {}", (*m).cur_music);
    r.note(s);
    place_player((*m).spawn_tile_x, 60);
    r.frames(120);
    let s = format!("audio: space, curMusic {}", (*m).cur_music);
    r.note(s);
    set_time(true, 27000.0);
    place_player(150, 249);
    r.frames(120);
    let s = format!("audio: ocean, curMusic {}", (*m).cur_music);
    r.note(s);
    place_player(480, 1009);
    r.frames(120);
    let s = format!("audio: underworld, curMusic {}", (*m).cur_music);
    r.note(s);
    r.record("audio_end", 3);
}


/// The audio run's calls against the reference's (`run.audio.zlib`), line by line over the whole run, and the
/// per-frame audio digest; no XACT cue may have run out by itself (module docs).
fn compare_audio(r: &mut Run, ref_dir: &Path) {
    let port = String::from_utf8(atrace::take_output()).expect("utf-8");
    std::fs::write(r.out_dir.join("run.audio"), &port).expect("run.audio");
    std::fs::write(r.out_dir.join("audio.digest"), &r.audio_digest).expect("audio.digest");
    let reference = match std::fs::read(ref_dir.join("run.audio.zlib")) {
        Ok(z) => String::from_utf8(miniz_oxide::inflate::decompress_to_vec_zlib(&z).expect("zlib")).expect("utf-8"),
        Err(e) => {
            r.failures.push(format!("audio: no reference run.audio.zlib ({e})"));
            return;
        }
    };
    let (a, b): (Vec<&str>, Vec<&str>) = (reference.lines().collect(), port.lines().collect());
    let calls = a.iter().filter(|l| !l.starts_with("frame ")).count();
    let mut frame = "";
    let mut first = None;
    for i in 0..a.len().max(b.len()) {
        let (x, y) = (a.get(i).copied(), b.get(i).copied());
        if let Some(l) = x {
            if l.starts_with("frame ") {
                frame = l;
            }
        }
        if x != y {
            first = Some((i, x.unwrap_or("<end>").to_owned(), y.unwrap_or("<end>").to_owned()));
            break;
        }
    }
    match first {
        None => {
            let s = format!("==== audio: {calls} of {calls} calls exact ({} frames)", r.frame + 1);
            eprintln!("{s}");
            r.summary.push(s);
        }
        Some((i, x, y)) => {
            let exact = a[..i].iter().filter(|l| !l.starts_with("frame ")).count();
            let s = format!("==== audio: {exact} of {calls} calls exact, first divergence at line {} ({frame}):\n  original: {}\n  port:     {}", i + 1, x.chars().take(300).collect::<String>(), y.chars().take(300).collect::<String>());
            eprintln!("{s}");
            r.summary.push(s.clone());
            r.failures.push(s);
        }
    }
    if let Ok(reference) = std::fs::read_to_string(ref_dir.join("audio.digest")) {
        match reference.lines().zip(r.audio_digest.lines()).find(|(x, y)| x != y) {
            Some((x, y)) => {
                eprintln!("==== audio digest: first different frame: original {x} / port {y}");
                r.failures.push(format!("audio digest: original {x} / port {y}"));
            }
            None => eprintln!("==== audio digest: {} frames, all equal", r.audio_digest.lines().count().min(reference.lines().count())),
        }
    }
    mixer_check(r, &reference);
    let ran_out = terraria_host::audio::engine::cues_ran_out();
    eprintln!("==== audio: XACT cues that ran out by themselves: {ran_out}");
    if ran_out != 0 {
        r.failures.push(format!("audio: {ran_out} XACT cue(s) ran out by themselves (the reference model has no such event)"));
    }
}

/// The port's mixer against a model of XAudio2's per-voice mixing, for every distinct voice start of the reference
/// run: the mixer (`terraria_host::audio::mixer`) gets what the game set (Volume, Pan, Pitch, IsLooped) and derives
/// its own output matrix and frequency ratio; the model gets the XAudio2 parameters XNA's REAL kernel code computed
/// in the reference run (`v=`, `m=`, `r=` of the trace) and mixes as XAudio2 is documented to: per source channel the
/// sample (linear interpolation at `n * r * source rate / 44100`, the loop region when looped) times the voice volume,
/// then into each output channel through the matrix (`out[d] = sum_s m[d * S + s] * x[s]`). What the model does not
/// know is XAudio2's sample-rate converter (a filter, not public: linear here too) and its 10 ms quantum.
/// Fails when a sample differs by more than 1e-6 of full scale (the two sum in different orders).
fn mixer_check(r: &mut Run, reference: &str) {
    use std::collections::{BTreeMap, HashMap};
    use std::sync::Arc;
    use terraria_host::audio::mixer::{AudioStats, Mixer, BLOCK_FRAMES};
    use terraria_host::audio::{AudioCommand, Sound};
    #[derive(Clone, Default)]
    struct V {
        label: String,
        vol: f32,
        pan: f32,
        pitch: f32,
        v: f32,
        m: Vec<f32>,
        ratio: f32,
        loop_count: i32,
        state: i32,
    }
    let hexf = |s: &str| f32::from_bits(u32::from_str_radix(s, 16).expect("hex float"));
    let mut voices: HashMap<String, V> = HashMap::new();
    // (label, volume, pan, pitch, looped) -> (v, matrix, ratio), how many starts
    let mut starts: BTreeMap<(String, u32, u32, u32, bool), (f32, Vec<f32>, f32, u32)> = BTreeMap::new();
    for line in reference.lines() {
        let f: Vec<&str> = line.split(' ').collect();
        match f[0] {
            "si.create" => {
                voices.insert(f[1].to_owned(), V { label: f[2].to_owned(), state: 4, ratio: 1.0, v: 1.0, vol: 1.0, ..Default::default() });
            }
            "si.volume" => {
                let v = voices.get_mut(f[1]).expect("voice");
                v.vol = hexf(f[2]);
                v.v = hexf(&f[3][2..]);
            }
            "si.pitch" => {
                let v = voices.get_mut(f[1]).expect("voice");
                v.pitch = hexf(f[2]);
                v.ratio = hexf(&f[3][2..]);
            }
            "si.pan" => {
                let v = voices.get_mut(f[1]).expect("voice");
                v.pan = hexf(f[2]);
                v.m = f[3][2..].split(',').map(hexf).collect();
            }
            "si.submit" => voices.get_mut(f[1]).expect("voice").loop_count = f[4].parse().expect("loop count"),
            "si.play" => {
                let v = voices.get_mut(f[1]).expect("voice");
                if v.state == 4 {
                    let e = starts.entry((v.label.clone(), v.vol.to_bits(), v.pan.to_bits(), v.pitch.to_bits(), v.loop_count != 0)).or_insert((v.v, v.m.clone(), v.ratio, 0));
                    e.3 += 1;
                }
                v.state = f[2].parse().expect("state");
            }
            "si.stop" | "si.pause" => {
                let v = voices.get_mut(f[1]).expect("voice");
                v.state = f[f.len() - 1].parse().expect("state");
            }
            "si.state" => voices.get_mut(f[1]).expect("voice").state = f[2].parse().expect("state"),
            _ => {}
        }
    }
    // the sounds, read from the install as the game's content reader reads them
    let install = terraria_host::content::install_dir();
    let mut sounds: HashMap<String, Arc<Sound>> = HashMap::new();
    let mut load = |label: &str| -> Arc<Sound> {
        sounds
            .entry(label.to_owned())
            .or_insert_with(|| {
                let bytes = std::fs::read(install.join("Content").join(format!("{label}.xnb"))).expect("sound file");
                let x = terraria_formats::xnb::read_xnb(&bytes).expect("xnb");
                let terraria_formats::xnb::Asset::SoundEffect(s) = &x.asset else { panic!("{label}: not a sound effect") };
                let samples = match s.format_tag {
                    1 => s.data.chunks_exact(2).map(|c| i16::from_le_bytes([c[0], c[1]])).collect(),
                    _ => terraria_host::audio::decode_ms_adpcm(&s.data, s.channels as usize, s.block_align as usize),
                };
                Arc::new(Sound { name: label.to_owned(), channels: s.channels, sample_rate: s.sample_rate, samples, loop_start: s.loop_start.max(0) as u32, loop_length: s.loop_length.max(0) as u32 })
            })
            .clone()
    };
    const FRAMES: usize = 2 * BLOCK_FRAMES;
    let (mut worst, mut same, mut total, mut voices_n) = (0f64, 0usize, 0usize, 0u32);
    let mut worst_at = String::new();
    for ((label, vol, pan, pitch, looped), (v, m, ratio, n)) in &starts {
        let sound = load(label);
        // the port's mixer, one voice
        let stats = AudioStats::default();
        let mut mx = Mixer::new(1.0);
        mx.apply(AudioCommand::Play { voice: 1, sound: sound.clone(), volume: f32::from_bits(*vol), pan: f32::from_bits(*pan), pitch: f32::from_bits(*pitch), looped: *looped }, &stats);
        let mut port = vec![0f32; FRAMES * 2];
        for b in port.chunks_mut(BLOCK_FRAMES * 2) {
            mx.mix(b);
        }
        // the model
        let ch = sound.channels.max(1) as usize;
        let frames = sound.frames();
        let (ls, ll) = (sound.loop_start as usize, sound.loop_length as usize);
        let range = if ll > 0 && ls + ll <= frames { (ls, ls + ll) } else { (0, frames) };
        let step = sound.sample_rate as f64 / 44100.0 * *ratio as f64;
        let at = |f: usize, c: usize| sound.samples.get(f * ch + c).copied().unwrap_or(0) as f32 / 32768.0;
        let mut model = vec![0f32; FRAMES * 2];
        let mut pos = 0f64;
        for n in 0..FRAMES {
            let mut i = pos as usize;
            if *looped && i >= range.1 && range.1 > range.0 {
                pos = range.0 as f64 + (pos - range.1 as f64) % (range.1 - range.0) as f64;
                i = pos as usize;
            }
            if i >= frames {
                break;
            }
            let t = (pos - i as f64) as f32;
            let next = if i + 1 < frames { i + 1 } else { i };
            let x: Vec<f32> = (0..ch.min(2)).map(|c| (at(i, c) + (at(next, c) - at(i, c)) * t) * v).collect();
            for d in 0..2 {
                model[n * 2 + d] = (0..x.len()).map(|s| m.get(d * x.len() + s).copied().unwrap_or(0.0) * x[s]).sum();
            }
            pos += step;
        }
        for (k, (a, b)) in port.iter().zip(&model).enumerate() {
            let diff = (*a as f64 - *b as f64).abs();
            total += 1;
            if a.to_bits() == b.to_bits() {
                same += 1;
            }
            if diff > worst {
                worst = diff;
                worst_at = format!("{label} vol {:08x} pan {:08x} pitch {:08x}{} sample {k}: mixer {a} model {b}", vol, pan, pitch, if *looped { " looped" } else { "" });
            }
        }
        voices_n += n;
    }
    let s = format!("==== mixer: {voices_n} voice starts, {} distinct (sound, volume, pan, pitch, loop), {FRAMES} frames each: {same} of {total} samples bit-identical to the XAudio2 model, largest difference {worst:.3e} of full scale{}", starts.len(), if worst_at.is_empty() { String::new() } else { format!(" ({worst_at})") });
    eprintln!("{s}");
    r.summary.push(s.clone());
    if worst > 1e-6 {
        r.failures.push(s);
    }
}

/// RefDumpDraw.cs `BossProjectiles`
const BOSS_PROJECTILES: [i32; 6] = [1, 14, 15, 27, 16, 88];

// the gallery run's steps (Gallery.cs)
include!("draw/gallery.rs");
// the interaction run's steps (Interact.cs)
include!("draw/interact.rs");
// the coverage run's steps (Cover.cs)
include!("draw/cover.rs");
include!("draw/probe.rs");
include!("draw/probe_a.rs");
include!("draw/probe_b.rs");
include!("draw/probe_c.rs");
include!("draw/probe_d.rs");
include!("draw/probe_e.rs");
include!("draw/probe_f.rs");
include!("draw/probe_g.rs");
include!("draw/probe_h.rs");
include!("draw/probe_i.rs");
include!("draw/probe_j.rs");
include!("draw/probe_k.rs");
include!("draw/probe_l.rs");
include!("draw/probe_m.rs");
include!("draw/probe_n.rs");
include!("draw/probe_o.rs");
include!("draw/probe_p.rs");
include!("draw/probe_q.rs");
include!("draw/probe_r.rs");
include!("draw/probe_s.rs");
// the net run's client (NetClient.cs) and what it shares with tests/net.rs
#[path = "net/common.rs"]
mod netc;
include!("draw/net.rs");

#[test]
fn draw() {
    let run = move || unsafe {
        std::panic::set_hook(Box::new(|info| eprintln!("panic: {info}")));
        let root = project_root();
        game::host::launch::install_resources(&root);
        // the scratch save folder (below the build directory): config.json and the world, as RefDumpDraw.cs writes them
        let audio = std::env::var("DRAW_MODE").is_ok_and(|v| v == "audio");
        let scenes = std::env::var("DRAW_MODE").is_ok_and(|v| v == "scenes");
        let gallery = std::env::var("DRAW_MODE").is_ok_and(|v| v == "gallery");
        let net = std::env::var("DRAW_MODE").is_ok_and(|v| v == "net");
        let interact = std::env::var("DRAW_MODE").is_ok_and(|v| v == "interact");
        let cover = std::env::var("DRAW_MODE").is_ok_and(|v| v == "cover");
        let probe = std::env::var("DRAW_MODE").is_ok_and(|v| v == "probe");
        let volumes = if audio { "  \"VolumeMusic\": 0.75,\n  \"VolumeSound\": 1.0,\n  \"VolumeAmbient\": 0.5,\n" } else { "  \"VolumeMusic\": 0.0,\n  \"VolumeSound\": 0.0,\n  \"VolumeAmbient\": 0.0,\n" };
        let save = Path::new(env!("CARGO_TARGET_TMPDIR")).join(if audio { "audio-save" } else if scenes { "scenes-save" } else if gallery { "gallery-save" } else if interact { "interact-save" } else if cover { "cover-save" } else if probe { "probe-save" } else if net { "net-save" } else { "draw-save" });
        let _ = std::fs::remove_dir_all(&save);
        std::fs::create_dir_all(save.join("Worlds")).expect("save folder");
        std::fs::write(
            save.join("config.json"),
            format!("{{\n  \"LastLaunchedVersion\": 326,\n  \"Fullscreen\": false,\n  \"WindowMaximized\": false,\n  \"WindowBorderless\": false,\n  \"DisplayWidth\": 1280,\n  \"DisplayHeight\": 720,\n  \"ResetDefaultUIScale\": false,\n  \"UIScale\": 1.0,\n  \"Zoom\": 1.0,\n{volumes}  \"Language\": \"en-US\",\n  \"UseRazerRGB\": false,\n  \"UseCorsairRGB\": false,\n  \"UseLogitechRGB\": false,\n  \"UseSteelSeriesRGB\": false\n}}\n"),
        )
        .expect("config.json");
        std::fs::copy(root.join("work/refsave/Worlds/ref_small.wld"), save.join("Worlds/ref_small.wld")).expect("world");
        // the probe run: input profiles saved by the game itself (RefDumpDraw.cs Run, the same file)
        if probe {
            std::fs::copy(root.join("tools/refdump_draw/probe_input_profiles.json"), save.join("input profiles.json")).expect("input profiles");
            // the probe run's resource pack (RefDumpDraw.cs the same)
            std::fs::create_dir_all(save.join("ResourcePacks/ProbePack")).expect("resource pack folder");
            std::fs::copy(root.join("tools/refdump_draw/probe_pack/pack.json"), save.join("ResourcePacks/ProbePack/pack.json")).expect("resource pack");
        }
        let out_dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(if audio { "../audio" } else if scenes { "../scenes" } else if gallery { "../gallery" } else if interact { "../interact" } else if cover { "../cover" } else if probe { "../probe" } else if net { "../net" } else { "../draw" });
        std::fs::create_dir_all(&out_dir).expect("out dir");
        let ref_dir = std::env::var_os("DRAW_REF").map(PathBuf::from).unwrap_or_else(|| root.join(if audio { "refdata/audio" } else if scenes { "refdata/scenes" } else if gallery { "refdata/gallery" } else if interact { "refdata/interact" } else if cover { "refdata/cover" } else if probe { "refdata/probe" } else if net { "refdata/net" } else { "refdata/draw" }));
        let only = std::env::var("DRAW_SCENES").ok().map(|v| v.split(',').map(str::to_owned).collect());

        hg::init(window());
        terraria_host::sys::time::set_fixed_tick_count(Some(TICK_COUNT));
        // RefDumpDraw.cs `FakeNewGuid`: the game thread's GUIDs are Guid.Empty (the net run's client UUID, message 68)
        terraria_host::sys::misc::set_new_guid_empty(true);
        // RefDumpDraw.cs `FakeTimestamp`: the stopwatch clock stands still
        terraria_host::sys::time::set_fixed_timestamp(Some(1_000_000_000));
        // RefDumpDraw.cs `ClockTicks`: 2026-06-15 12:00:00
        terraria_host::sys::time::set_fixed_now(Some(terraria_host::sys::time::DateTime::from_ymd(2026, 6, 15).ticks() + 12 * 36_000_000_000));
        trace::set_missing_hook(Some(std::rc::Rc::new(label_assets)));
        trace::set_active(true);
        if audio {
            atrace::set_active(true);
            atrace::set_full(true);
            atrace::write_raw("frame 0\n");
        }
        let save_str = save.to_string_lossy().into_owned();
        let main = game::host::launch::launch_game(&["-savedirectory", &save_str]);
        // pinned: no force-load thread
        (*terraria::main::s()).skip_assembly_load = true;
        game::host::launch::initialize(main);
        // the start-up force-load (Program.ForceLoadThread: every static initialiser run) on this thread, before the
        // first frame (RefDumpDraw.cs the same)
        terraria::program::force_load_thread(Gc::NULL);

        let mut r = Run { main, frame: 0, digest: String::new(), audio, scenes, gallery, net: None, interact, cover, probe, state: String::new(), audio_digest: String::new(), input: Input { x: 640, y: 360, left: false, keys: Vec::new() }, out_dir: out_dir.clone(), ref_dir: ref_dir.clone(), only, summary: Vec::new(), failures: Vec::new() };
        if net {
            // NetClient.cs: the client's network is virtual (NetVirt.cs; terraria-host sys::net::set_virtual)
            terraria_host::sys::net::set_virtual(true);
            r.net = Some(NetRun {
                server: netc::read_stream(&ref_dir.join("client_in.stream")),
                client: netc::read_stream(&ref_dir.join("client_out.stream")),
                active: false,
                frame: 0,
                pending: Vec::new(),
                exact: 0,
                first: None,
                sent_types: Default::default(),
            });
        }
        scenario(&mut r);
        net_report(&mut r);
        std::fs::write(out_dir.join("run.digest"), &r.digest).expect("digest");
        // the first frame whose digest differs from the reference run's
        if let Ok(reference) = std::fs::read_to_string(ref_dir.join("run.digest")) {
            let first = reference.lines().zip(r.digest.lines()).find(|(a, b)| a != b)
                .or_else(|| (reference.lines().count() != r.digest.lines().count()).then_some(("(frame count)", "(frame count)")));
            match first {
                Some((a, b)) => {
                    eprintln!("==== digest: first different frame: original {a} / port {b}");
                    // every frame counts, not only the recorded ones (DRAW_EXTRA=<frame>,1 writes that frame's lines)
                    r.failures.push(format!("digest: original {a} / port {b}"));
                }
                None => eprintln!("==== digest: {} frames, all equal", r.digest.lines().count().min(reference.lines().count())),
            }
        }
        if audio {
            compare_audio(&mut r, &ref_dir);
        }
        if scenes || gallery || net || interact || cover || probe {
            std::fs::write(out_dir.join("scenes.state"), &r.state).expect("scenes.state");
            let reference = std::fs::read_to_string(ref_dir.join("scenes.state")).unwrap_or_default();
            let (a, b): (Vec<&str>, Vec<&str>) = (reference.lines().collect(), r.state.lines().collect());
            match (0..a.len().max(b.len())).find(|&i| a.get(i) != b.get(i)) {
                None => eprintln!("==== state: {} of {} scenes equal", b.len(), a.len()),
                Some(i) => {
                    let s = format!("==== state: {} of {} scenes equal, first difference:\n  original: {}\n  port:     {}", i, a.len(), a.get(i).unwrap_or(&"<end>"), b.get(i).unwrap_or(&"<end>"));
                    eprintln!("{s}");
                    r.failures.push(s);
                }
            }
        }
        std::fs::write(out_dir.join("summary.txt"), r.summary.join("\n") + "\n").expect("summary");
        eprintln!("draw: {} failure(s)", r.failures.len());
        assert!(r.failures.is_empty(), "draw output differs from the original");
    };
    terraria_host::run_with_stack(512 << 20, "draw", run);
}
