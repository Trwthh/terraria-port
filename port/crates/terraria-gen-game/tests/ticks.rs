//! Proof 3 (start): the translated per-tick simulation against the real server's, scenario 2 of
//! `refdata/ticks` (one scripted remote player, 600 ticks; harness `tools/refdump_ticks/Ticks.cs`).
//!
//! The test performs the harness's call sequence with translated code only (same `netMode`, same server path,
//! same pins, same control flags per tick) and compares the player's state after every tick with
//! `refdata/ticks/player.per_tick.jsonl`. It prints the number of ticks that are identical in every compared
//! field, the first differing tick with its fields, and per field the first tick it differs at.
//!
//! What is NOT reproduced of the harness: its reflection walk that re-seeds 41 other `UnifiedRandom` instances
//! (`Cloud.rand`, `HitTile.rand`, the sound-style generators ...: `summary.json` says none of them reaches tiles,
//! NPCs, items or projectiles; if one reaches the player this comparison shows it).
//!
//! `TICKS_SCENARIO=world` runs scenario 1 instead (no player, 3600 ticks; `TICKS_LIMIT=<n>` stops earlier). In both
//! scenarios, after every one of the first 120 ticks the value `Next()` would return is compared for `Main.rand`
//! and every `Main._rngs` stream (`<scenario>.rand_per_tick.jsonl`), and at the harness's record ticks (0, 1, 2,
//! 3, 10, every 60th) the tile-map hash, the tile / wall / liquid counts, time and weather statics, the liquid
//! counters, the entity counts and (full rows) the NPCs (`<scenario>.ticks.jsonl`).
//!
//! `TICKS_REQUIRE=<n>`: fail unless at least the first n ticks are identical (default 0: report only);
//! `TICKS_REQUIRE_WORLD=<n>`: the same for the world comparisons (RNG streams and record rows).
//! Run: `CARGO_TARGET_DIR=target-gen/fast CARGO_PROFILE_DEV_OPT_LEVEL=0 CARGO_PROFILE_DEV_DEBUG=0 cargo test -p
//! terraria-gen-game --test ticks -- --nocapture`

#[path = "p1/json.rs"]
mod json;

use json::Json;
use terraria_gen_game::gen::{meta, terraria};
use terraria_gen_game::rt::{self, Gc, Str};

fn panic_text(p: &Box<dyn std::any::Any + Send>) -> String {
    if let Some(s) = p.downcast_ref::<String>() {
        s.clone()
    } else if let Some(s) = p.downcast_ref::<&str>() {
        (*s).to_owned()
    } else {
        "<non-string panic payload>".to_owned()
    }
}

fn step(name: &str, f: impl FnOnce()) {
    let t0 = std::time::Instant::now();
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| rt::run(f))) {
        Ok(Ok(())) => eprintln!("ok   {name}   ({:.2} s)", t0.elapsed().as_secs_f64()),
        Ok(Err(e)) => panic!("{name}: C# exception: {e}"),
        Err(p) => panic!("{name}: stopped: {}", panic_text(&p)),
    }
}

trait AsF64 {
    fn f(self) -> f64;
}
macro_rules! as_f64 {
    ($($t:ty),*) => {$( impl AsF64 for $t { fn f(self) -> f64 { self as f64 } } )*};
}
as_f64!(i32, u32, i64, u8, f32, f64);
impl AsF64 for bool {
    fn f(self) -> f64 {
        self as u8 as f64
    }
}

/// FNV-1a 64 of the tile map as the harness hashes it (refdata/worldgen/README.md): 14-byte records, column-major.
unsafe fn tile_hash() -> (u64, i64, i64, i64) {
    let ms = &*terraria::main::s();
    let (mut h, mut active, mut wall, mut liquid) = (0xcbf29ce484222325u64, 0i64, 0i64, 0i64);
    for x in 0..ms.max_tiles_x {
        for y in 0..ms.max_tiles_y {
            let t = *ms.tile.at(x, y);
            let mut rec = [0u8; 14];
            if !t.is_null() {
                let t = &*t.ptr();
                rec[0..2].copy_from_slice(&t.type_.to_le_bytes());
                rec[2..4].copy_from_slice(&t.wall.to_le_bytes());
                rec[4] = t.liquid;
                rec[5..7].copy_from_slice(&t.s_tile_header.to_le_bytes());
                rec[7] = t.b_tile_header;
                rec[8] = t.b_tile_header2;
                rec[9] = t.b_tile_header3;
                rec[10..12].copy_from_slice(&t.frame_x.to_le_bytes());
                rec[12..14].copy_from_slice(&t.frame_y.to_le_bytes());
                active += (t.s_tile_header & 0x20 != 0) as i64;
                wall += (t.wall != 0) as i64;
                liquid += (t.liquid != 0) as i64;
            }
            for b in rec {
                h = (h ^ b as u64).wrapping_mul(0x100000001b3);
            }
        }
    }
    (h, active, wall, liquid)
}

/// The value `Next()` would return, on a copy (as the harness computes `next`).
unsafe fn next_of(g: Gc<terraria_core::UnifiedRandom>) -> i64 {
    (*g.ptr()).clone().next_raw() as i64
}

/// Differences of the RNG streams after a tick.
unsafe fn compare_rngs(want: &Json) -> Vec<String> {
    let ms = &*terraria::main::s();
    let mut d = Vec::new();
    let w = want.get("main_rand").and_then(|r| r.get("next")).map(|j| j.int()).expect("main_rand.next");
    let g = next_of(ms.rand);
    if w != g {
        d.push(format!("Main.rand: real next {w}, here {g}"));
    }
    let rngs = want.get("rngs").expect("rngs").obj();
    for (key, state) in rngs {
        let w = state.get("next").map(|j| j.int()).expect("next");
        let k = Str::new(key);
        if ms._rngs.is_null() || !(*ms._rngs.ptr()).contains_key(k) {
            d.push(format!("{key}: real next {w}, here the stream does not exist"));
            continue;
        }
        let g = next_of((*ms._rngs.ptr()).get_item(k));
        if w != g {
            d.push(format!("{key}: real next {w}, here {g}"));
        }
    }
    if !ms._rngs.is_null() {
        for k in rt::Dictionary::keys(ms._rngs).to_vec() {
            if !rngs.iter().any(|(name, _)| name == k.as_str()) {
                d.push(format!("{}: exists here, not in the real server", k.as_str()));
            }
        }
    }
    d
}

/// Differences of one record row of `<scenario>.ticks.jsonl`.
unsafe fn compare_record(rec: &Json) -> Vec<String> {
    let ms = &*terraria::main::s();
    let mut d = Vec::new();
    let (h, active, wall, liquid) = tile_hash();
    let want_hash = rec.get("tiles_fnv64").map(|j| j.text()).expect("tiles_fnv64");
    let want_hash = want_hash.trim_matches(|c| c == '"').to_owned();
    if want_hash != format!("{h:016x}") {
        d.push(format!("tiles_fnv64: real {want_hash}, here {h:016x}"));
    }
    let mut n = |name: &str, section: Option<&str>, got: f64| {
        let src = match section {
            Some(s) => rec.get(s).unwrap_or_else(|| panic!("record has no {s}")),
            None => rec,
        };
        let w = num(src.get(name).unwrap_or_else(|| panic!("record has no {name}")));
        if w != got {
            d.push(format!("{}{name}: real {w:?}, here {got:?}", section.map(|s| format!("{s}.")).unwrap_or_default()));
        }
    };
    n("n_active", None, active as f64);
    n("n_wall", None, wall as f64);
    n("n_liquid", None, liquid as f64);
    macro_rules! m {
        ($($name:literal => $e:expr),* $(,)?) => {$( n($name, Some("main"), AsF64::f($e)); )*};
    }
    m!("time" => ms.time, "dayTime" => ms.day_time, "moonPhase" => ms.moon_phase, "raining" => ms.raining, "rainTime" => ms.rain_time,
       "maxRaining" => ms.max_raining, "cloudAlpha" => ms.cloud_alpha, "windSpeedCurrent" => ms.wind_speed_current,
       "windSpeedTarget" => ms.wind_speed_target, "numClouds" => ms.num_clouds, "windCounter" => ms.wind_counter,
       "extremeWindCounter" => ms.extreme_wind_counter, "weatherCounter" => ms.weather_counter, "cloudBGActive" => ms.cloud_bg_active,
       "slimeRain" => ms.slime_rain, "slimeRainTime" => ms.slime_rain_time, "invasionType" => ms.invasion_type,
       "checkForSpawns" => ms.check_for_spawns, "dayRate" => ms.day_rate, "netPlayCounter" => ms.net_play_counter,
       "ambientCounter" => ms.ambient_counter, "_gameUpdateCount" => ms._game_update_count);
    let lq = &*terraria::liquid::s();
    macro_rules! l {
        ($($name:literal => $e:expr),* $(,)?) => {$( n($name, Some("liquid"), AsF64::f($e)); )*};
    }
    l!("Liquid.numLiquid" => lq.num_liquid, "Liquid.wetCounter" => lq.wet_counter, "Liquid.panicCounter" => lq.panic_counter,
       "Liquid.stuckCount" => lq.stuck_count, "Liquid.stuckAmount" => lq.stuck_amount, "Liquid.quickSettle" => lq.quick_settle,
       "Liquid.cycles" => lq.cycles, "Liquid.curMaxLiquid" => lq.cur_max_liquid, "Liquid.skipCount" => lq.skip_count,
       "Liquid.stuck" => lq.stuck, "Liquid.panicMode" => lq.panic_mode, "Liquid.panicY" => lq.panic_y,
       "LiquidBuffer.numLiquidBuffer" => (*terraria::liquid_buffer::s()).num_liquid_buffer);
    // entity counts
    let mut npcs = Vec::new();
    for k in 0..ms.npc.len() {
        let np = *ms.npc.at(k);
        if !np.is_null() && (*np.ptr()).active {
            npcs.push((k, np));
        }
    }
    let mut items = 0;
    for k in 0..ms.item.len() {
        let w = *ms.item.at(k);
        if !w.is_null() && terraria::world_item::active(w.ptr()) {
            items += 1;
        }
    }
    let mut projectiles = 0;
    for k in 0..ms.projectile.len() {
        let w = *ms.projectile.at(k);
        if !w.is_null() && (*w.ptr()).active {
            projectiles += 1;
        }
    }
    n("npcs", Some("counts"), npcs.len() as f64);
    n("items", Some("counts"), items as f64);
    n("projectiles", Some("counts"), projectiles as f64);
    // the NPCs of a full row
    if let Some(list) = rec.get("npcs") {
        for e in list.arr() {
            let index = e.get("index").map(|j| j.int()).expect("index") as i32;
            let v = e.get("v").expect("v");
            let Some((_, np)) = npcs.iter().find(|(k, _)| *k == index) else {
                d.push(format!("npc[{index}]: active in the real server, not here"));
                continue;
            };
            let np = &*np.ptr();
            let mut c = |name: &str, w: f64, g: f64| {
                if w != g {
                    d.push(format!("npc[{index}].{name}: real {w:?}, here {g:?}"));
                }
            };
            let g = |k: &str| num(v.get(k).unwrap_or_else(|| panic!("npc has no {k}")));
            let g2 = |k: &str, s: &str| num(v.get(k).and_then(|j| j.get(s)).unwrap_or_else(|| panic!("npc has no {k}.{s}")));
            c("type", g("type"), np.type_ as f64);
            c("position.X", g2("position", "X"), np.base.position.x as f64);
            c("position.Y", g2("position", "Y"), np.base.position.y as f64);
            c("velocity.X", g2("velocity", "X"), np.base.velocity.x as f64);
            c("velocity.Y", g2("velocity", "Y"), np.base.velocity.y as f64);
            for (a, wj) in v.get("ai").expect("ai").arr().iter().enumerate() {
                c(&format!("ai[{a}]"), num(wj), *np.ai.at(a as i32) as f64);
            }
            for (a, wj) in v.get("localAI").expect("localAI").arr().iter().enumerate() {
                c(&format!("localAI[{a}]"), num(wj), *np.local_ai.at(a as i32) as f64);
            }
            c("life", g("life"), np.life as f64);
            c("direction", g("direction"), np.base.direction as f64);
            c("directionY", g("directionY"), np.direction_y as f64);
            c("target", g("target"), np.target as f64);
            c("spriteDirection", g("spriteDirection"), np.sprite_direction as f64);
            c("rotation", g("rotation"), np.rotation as f64);
            c("timeLeft", g("timeLeft"), np.time_left as f64);
            c("homeTileX", g("homeTileX"), np.home_tile_x as f64);
        }
    }
    d
}

fn num(j: &Json) -> f64 {
    match j {
        Json::Num(s) => s.parse::<f64>().unwrap_or_else(|_| panic!("number {s:?}")),
        Json::Bool(b) => *b as u8 as f64,
        other => panic!("not a number: {}", other.text()),
    }
}

/// (field name, value in the reference, value here) for every compared field that differs.
unsafe fn compare(p: Gc<terraria::player::Player>, want: &Json) -> Vec<(String, f64, f64)> {
    let pl = &*p.ptr();
    let e = &pl.base;
    let mut d = Vec::new();
    let mut f = |name: &str, got: f64| {
        let w = match name.split_once('.') {
            Some((a, b)) => num(want.get(a).unwrap_or_else(|| panic!("reference has no {a}")).get(b).unwrap_or_else(|| panic!("reference has no {name}"))),
            None => num(want.get(name).unwrap_or_else(|| panic!("reference has no {name}"))),
        };
        // floats are compared as the doubles the harness printed (an f32 widened): exact
        if w != got && !(w.is_nan() && got.is_nan()) {
            d.push((name.to_owned(), w, got));
        }
    };
    f("position.X", e.position.x as f64);
    f("position.Y", e.position.y as f64);
    f("velocity.X", e.velocity.x as f64);
    f("velocity.Y", e.velocity.y as f64);
    f("oldPosition.X", e.old_position.x as f64);
    f("oldPosition.Y", e.old_position.y as f64);
    f("oldVelocity.X", e.old_velocity.x as f64);
    f("oldVelocity.Y", e.old_velocity.y as f64);
    f("direction", e.direction as f64);
    f("oldDirection", e.old_direction as f64);
    f("jump", pl.jump as f64);
    f("wingTime", pl.wing_time as f64);
    f("rocketTime", pl.rocket_time as f64);
    f("fallStart", pl.fall_start as f64);
    f("fallStart2", pl.fall_start2 as f64);
    f("gravDir", pl.grav_dir as f64);
    f("wet", e.wet as u8 as f64);
    f("lavaWet", e.lava_wet as u8 as f64);
    f("honeyWet", e.honey_wet as u8 as f64);
    f("statLife", pl.stat_life as f64);
    f("statMana", pl.stat_mana as f64);
    f("controlLeft", pl.control_left as u8 as f64);
    f("controlRight", pl.control_right as u8 as f64);
    f("controlUp", pl.control_up as u8 as f64);
    f("controlDown", pl.control_down as u8 as f64);
    f("controlJump", pl.control_jump as u8 as f64);
    f("releaseJump", pl.release_jump as u8 as f64);
    f("justJumped", pl.just_jumped as u8 as f64);
    f("sliding", pl.sliding as u8 as f64);
    f("slideDir", pl.slide_dir as f64);
    f("fullRotation", pl.full_rotation as f64);
    f("bodyFrame.Y", pl.body_frame.y as f64);
    f("legFrame.Y", pl.leg_frame.y as f64);
    f("breath", pl.breath as f64);
    f("maxFallSpeed", pl.max_fall_speed as f64);
    f("moveSpeed", pl.move_speed as f64);
    f("runAcceleration", pl.run_acceleration as f64);
    f("maxRunSpeed", pl.max_run_speed as f64);
    f("runSlowdown", pl.run_slowdown as f64);
    f("gravity", pl.gravity as f64);
    f("dead", pl.dead as u8 as f64);
    f("active", pl.active as u8 as f64);
    f("ghost", pl.ghost as u8 as f64);
    f("pulley", pl.pulley as u8 as f64);
    f("autoJump", pl.auto_jump as u8 as f64);
    d
}

#[test]
fn scripted_player_matches_the_real_server() {
    let run = || unsafe {
        std::panic::set_hook(Box::new(|_| {}));
        let root = terraria_host::sys::io::project_dir();
        terraria_gen_game::host::launch::install_resources(&root);
        terraria_host::game::init(terraria_host::game::WindowState { width: 800, height: 600, x: 0, y: 0, active: true, handle: 0, display_width: 1920, display_height: 1080, scale: 1.0 });
        meta::register_all();
        meta::register_json();

        // scratch save root with a copy of the reference world (Ticks.cs Run)
        let save = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("ticks-save");
        let _ = std::fs::remove_dir_all(&save);
        std::fs::create_dir_all(save.join("Worlds")).expect("save root");
        let world = save.join("Worlds").join("ref_small.wld");
        std::fs::copy(root.join("work/refsave/Worlds/ref_small.wld"), &world).expect("copy of the reference world");
        std::env::set_var("TERRARIA_SAVE_DIR", &save);

        // ---- InitGame (Common.cs:118-147)
        let mut main = Gc::NULL;
        step("Program.SavePath = <scratch>", || (*terraria::program::s()).save_path = Str::new(&save.to_string_lossy()));
        step("Main.dedServ = true; Main.showSplash = false", || {
            (*terraria::main::s()).ded_serv = true;
            (*terraria::main::s()).show_splash = false;
        });
        step("LanguageManager.Instance.SetLanguage(GameCulture.DefaultCulture)", || {
            let lm = (*terraria::localization::language_manager::s()).instance;
            terraria::localization::language_manager::set_language__game_culture(lm.ptr(), terraria::localization::game_culture::default_culture());
        });
        step("main = new Main()", || main = terraria::main::new());
        step("Lang.InitializeLegacyLocalization()", || terraria::lang::initialize_legacy_localization());
        step("-world <existing>: WorldFile.GetAllMetadata(path, false).SetAsActive()", || {
            let wfd = terraria::io::world_file::get_all_metadata(Str::new(&world.to_string_lossy()), false);
            assert!(!wfd.is_null(), "GetAllMetadata returned null");
            terraria::io::world_file_data::set_as_active(wfd.ptr());
        });
        step("Main.rand = new UnifiedRandom(0)", || (*terraria::main::s()).rand = rt::alloc_init(terraria_core::UnifiedRandom::new(0)));
        step("main.Initialize()", || terraria::main::initialize(main.ptr()));
        eprintln!("netMode after Initialize = {}, dedServ = {}, gameMenu = {}", (*terraria::main::s()).net_mode, (*terraria::main::s()).ded_serv, (*terraria::main::s()).game_menu);

        // ---- the load, with the load thread's Main.rand pinned (Main.rand is [ThreadStatic] in the original: the
        // load thread has its own; here there is one, and the main thread's is replaced right after anyway)
        step("WorldGen.serverLoadWorldCallBack() with Main.rand = new UnifiedRandom(1357)", || {
            (*terraria::main::s()).rand = rt::alloc_init(terraria_core::UnifiedRandom::new(1357));
            terraria::world_gen::server_load_world_call_back();
            assert!(!(*terraria::world_gen::s()).load_failed, "WorldGen.loadFailed");
        });
        eprintln!("loaded {}x{} time={} dayTime={}", (*terraria::main::s()).max_tiles_x, (*terraria::main::s()).max_tiles_y, (*terraria::main::s()).time, (*terraria::main::s()).day_time);

        // ---- server state without sockets (Netplay.cs:254-263), Main.gameMenu = false (Main.cs:5906)
        step("server state: myPlayer = 255; netMode = 2; Netplay.Disconnect = false; Clients[i] = new RemoteClient", || {
            (*terraria::main::s()).my_player = 255;
            (*terraria::main::s()).net_mode = 2;
            (*terraria::netplay::s()).disconnect = false;
            let clients = (*terraria::netplay::s()).clients;
            for i in 0..256 {
                let c = terraria::remote_client::new();
                terraria::remote_client::reset(c.ptr());
                (*c.ptr()).id = i;
                (*c.ptr()).read_buffer = rt::Arr::<u8>::new(1024);
                *clients.at(i) = c;
            }
        });
        (*terraria::main::s()).game_menu = false;

        // ---- pins (Ticks.cs:121-137)
        (*terraria::main::s()).rand = rt::alloc_init(terraria_core::UnifiedRandom::new(424242));
        (*terraria::main::s()).unpaused_update_seed__backing = 123456789;
        (*terraria::main::s()).tile_frame_seed = 987654321;
        (*terraria::main::s()).wind_counter = 1500;
        (*terraria::main::s()).extreme_wind_counter = 25;

        let scenario = std::env::var("TICKS_SCENARIO").unwrap_or_else(|_| "player".to_owned());
        assert!(scenario == "player" || scenario == "world", "TICKS_SCENARIO=player|world");
        let with_player = scenario == "player";
        let records: Vec<Json> = std::fs::read_to_string(root.join(format!("refdata/ticks/{scenario}.ticks.jsonl"))).expect("ticks.jsonl").lines().map(json::parse).collect();
        let rand_rows: Vec<Json> = std::fs::read_to_string(root.join(format!("refdata/ticks/{scenario}.rand_per_tick.jsonl"))).expect("rand_per_tick.jsonl").lines().map(json::parse).collect();
        let mut world_first: Option<(u32, Vec<String>)> = None;
        let mut world_checks = 0u32;
        let mut world_identical = 0u32;
        let mut world_identical_prefix_tick = 0u32;
        let mut world_check = |t: u32, what: &str, d: Vec<String>| {
            world_checks += 1;
            if d.is_empty() {
                world_identical += 1;
                if world_first.is_none() {
                    world_identical_prefix_tick = t;
                }
            } else {
                if world_first.is_none() {
                    eprintln!("WORLD: first difference at tick {t} ({what}), {} value(s):", d.len());
                    for l in d.iter().take(24) {
                        eprintln!("    {l}");
                    }
                    world_first = Some((t, d.clone()));
                } else if std::env::var_os("TICKS_VERBOSE").is_some() {
                    eprintln!("world: tick {t} ({what}): {} value(s) differ, first: {}", d.len(), d[0]);
                }
            }
        };

        // ---- SetupPlayer (Ticks.cs:446-477)
        let p0 = *(*terraria::main::s()).player.at(0);
        if with_player {
        step("player 0: active, 100 life, 20 mana, Player.Spawn(SpawningIntoWorld)", || {
            let pl = &mut *p0.ptr();
            pl.active = true;
            pl.name = Str::lit("Ref");
            pl.base.who_am_i = 0;
            pl.stat_life_max = 100;
            pl.stat_life_max2 = 100;
            pl.stat_life = 100;
            pl.stat_mana_max = 20;
            pl.stat_mana_max2 = 20;
            pl.stat_mana = 20;
            // PlayerSpawnContext.SpawningIntoWorld = 1 (ReviveFromDeath, SpawningIntoWorld, RecallFromItem, TeamSwap)
            terraria::player::spawn(p0.ptr(), 1);
            (*p0.ptr()).active = true;
            (*p0.ptr()).dead = false;
        });
        let e = &(*p0.ptr()).base;
        eprintln!("player 0 at ({}, {}) size {}x{}", e.position.x, e.position.y, e.width, e.height);
        }
        // row 0: the state right after the load, before any tick
        world_check(0, "record", compare_record(&records[0]));

        // ---- the ticks
        let last_tick = records.last().and_then(|r| r.get("tick")).map(|j| j.int()).expect("last record") as u32;
        let limit: u32 = std::env::var("TICKS_LIMIT").ok().and_then(|v| v.parse().ok()).unwrap_or(last_tick).min(last_tick);
        let reference = if with_player { std::fs::read_to_string(root.join("refdata/ticks/player.per_tick.jsonl")).expect("refdata/ticks/player.per_tick.jsonl") } else { String::new() };
        let player_lines: Vec<&str> = reference.lines().collect();
        let mut matches_next_record = 0u32;
        let mut identical_prefix = 0u32;
        let mut identical_total = 0u32;
        let mut first: Option<(u32, Vec<(String, f64, f64)>)> = None;
        let mut first_per_field: Vec<(String, u32, f64, f64)> = Vec::new();
        let mut wind_resets = 0;
        let mut ticks = 0u32;
        let t0 = std::time::Instant::now();
        for t in 1..=limit {
            // ScriptPlayer (Ticks.cs:491-497)
            if with_player {
                let pl = &mut *p0.ptr();
                pl.control_left = (241..=420).contains(&t);
                pl.control_right = (1..=150).contains(&t);
                pl.control_jump = (121..=150).contains(&t) || t == 201 || t == 211 || t == 221 || (361..=420).contains(&t);
                pl.control_up = false;
                pl.control_down = false;
            }
            let wind_before = (*terraria::main::s()).wind_counter;
            let gt = terraria_host::game::GameTime::new_gc(rt::zero(), rt::zero());
            let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| rt::run(|| terraria::main::update(main.ptr(), gt))));
            match r {
                Ok(Ok(())) => {}
                Ok(Err(e)) => {
                    eprintln!("TICK {t} THREW {e}");
                    break;
                }
                Err(p) => {
                    eprintln!("TICK {t} STOPPED: {}", panic_text(&p));
                    break;
                }
            }
            (*main.ptr())._is_drawing_or_updating = false;
            terraria_host::sys::threading::run_pending_work();
            if (*terraria::main::s()).wind_counter > wind_before {
                (*terraria::main::s()).wind_counter = 900 + (wind_resets * 773) % 1801;
                wind_resets += 1;
            }
            ticks = t;
            if t <= 120 {
                let row = &rand_rows[t as usize - 1];
                assert_eq!(row.get("tick").map(|j| j.int()), Some(t as i64));
                world_check(t, "RNG streams", compare_rngs(row));
            }
            if let Some(rec) = records.iter().find(|r| r.get("tick").map(|j| j.int()) == Some(t as i64)) {
                world_check(t, "record", compare_record(rec));
            }
            if !with_player {
                continue;
            }
            let rec = json::parse(player_lines[t as usize - 1]);
            assert_eq!(rec.get("tick").map(|j| j.int()), Some(t as i64));
            // negative control: the state after tick t against the record of tick t + 1
            if (t as usize) < player_lines.len() && compare(p0, json::parse(player_lines[t as usize]).get("player").expect("player")).is_empty() {
                matches_next_record += 1;
            }
            let d = compare(p0, rec.get("player").expect("player"));
            if d.is_empty() {
                identical_total += 1;
                if first.is_none() {
                    identical_prefix = t;
                }
            } else {
                for (name, w, g) in &d {
                    if !first_per_field.iter().any(|(n, ..)| n == name) {
                        first_per_field.push((name.clone(), t, *w, *g));
                    }
                }
                if first.is_none() {
                    first = Some((t, d));
                }
            }
        }
        eprintln!("scenario {scenario}: ticks run: {ticks} in {:.1} s ({:.2} ms per tick, the comparisons included)", t0.elapsed().as_secs_f64(), t0.elapsed().as_secs_f64() * 1000.0 / ticks.max(1) as f64);
        eprintln!("WORLD comparisons (RNG streams after each of the first 120 ticks, record rows): {world_identical} of {world_checks} identical; identical up to tick {world_identical_prefix_tick}{}", match &world_first { None => " (no difference)".to_owned(), Some((t, d)) => format!("; first difference at tick {t}: {} value(s)", d.len()) });
        let need_world: u32 = std::env::var("TICKS_REQUIRE_WORLD").ok().and_then(|v| v.parse().ok()).unwrap_or(0);
        assert!(world_identical_prefix_tick >= need_world || world_first.is_none(), "the world comparisons are identical only up to tick {world_identical_prefix_tick}, {need_world} required");
        if !with_player {
            return;
        }
        eprintln!("PLAYER: ticks identical to the real server in every compared field: the first {identical_prefix}; {identical_total} of {ticks} in all");
        eprintln!("negative control: the state after tick t equals the record of tick t + 1 in {matches_next_record} of {} ticks (the player stands still in those)", ticks.saturating_sub(1));
        match &first {
            None => eprintln!("no difference"),
            Some((t, d)) => {
                eprintln!("first differing tick: {t}");
                for (name, w, g) in d {
                    eprintln!("    {name}: real {w:?} (f32 bits {:08x}), here {g:?} (f32 bits {:08x})", (*w as f32).to_bits(), (*g as f32).to_bits());
                }
            }
        }
        if !first_per_field.is_empty() {
            eprintln!("first difference per field:");
            for (name, t, w, g) in &first_per_field {
                eprintln!("    tick {t:4}  {name}: real {w:?}, here {g:?}");
            }
        }
        let e = &(*p0.ptr()).base;
        eprintln!("player at the end: ({:?}, {:?}) velocity ({:?}, {:?})", e.position.x, e.position.y, e.velocity.x, e.velocity.y);
        let need: u32 = std::env::var("TICKS_REQUIRE").ok().and_then(|v| v.parse().ok()).unwrap_or(0);
        assert!(identical_prefix >= need, "only the first {identical_prefix} ticks are identical, {need} required");
    };
    // generated code keeps static state and recurses deeply: one thread with a large stack
    terraria_host::run_with_stack(512 << 20, "the ticks test", run);
}
