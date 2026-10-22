//! Proof 2: world generation of the translated game against the real game's, pass by pass.
//!
//! Reference: `refdata/worldgen/` (seed text "12345", small world, 106 passes; README.md there has the call
//! sequence of the harness `tools/refdump_worldgen/Gen.cs` with line references). This test performs the same
//! sequence with translated code only:
//!
//! a. `Program.SavePath`, `Main.dedServ = true`, language, `new Main()`, `Lang.InitializeLegacyLocalization()`
//! b. the launch parameters BEFORE `Initialize()`: `maxTilesX/Y = 4200/1200`, `autoGen`, `worldName`,
//!    `new WorldFileData(path, false).SetAsActive()`, `autoGenFileLocation`, `AutogenSeedName = "12345"`
//! c. `Main.rand = new UnifiedRandom(0)`, `main.Initialize()`
//! d. the autogen block of `WorldFile.LoadWorld` (WorldFile.cs:679-705): `CreateMetadata`, the seed text handling,
//!    `ActiveWorldFileData.SetSeed("12345")`
//! e. `WorldGen.GenerateWorld` (WorldGen.cs:11080-11116) statement by statement, with the loop of
//!    `WorldGenerator.GenerateWorld` / `RunPass` (WorldGenerator.cs:421-517) written out here so that a record can
//!    be taken where the harness takes it (after `pass.Apply`, before `progress.End()` and `genRand.Next()`)
//! f. `Finish()` and the `finally` block: the last row
//!
//! Compared per row (108 rows): the whole-map tile hash, the hash of the 4200 column hashes (on a mismatch the
//! differing columns are listed from `ref_small.columns.bin`), the active / wall / liquid tile counts, the value
//! `Next()` would return for `Main.rand`, `worldSurface`, `rockLayer`, spawn and dungeon position, the chest count.
//!
//! Not compared here: the other statics of the rows (`worldgen_static`, `genvars`), the chest / sign / NPC content
//! hash, the 15 tile snapshots (use them to bisect a diverging pass).
//!
//! `WORLDGEN_REAL=1`: instead of d - f, `WorldGen.serverLoadWorldCallBack()` as ONE call, the dedicated server's own
//! path (the harness's `mode real`): the autogen block of `WorldFile.LoadWorld`, `WorldGen.GenerateWorld`,
//! `WorldFile.SaveNewWorld()` and the load of the file just written. The file is left at
//! `<target>/tmp/worldgen-save/Worlds/ref_small.wld`; compare it with the real game's:
//! `python tools/refdump_worldgen/wld_diff.py <that file> work/refsave/Worlds/ref_small.wld`.
//!
//! ```text
//! cd port && CARGO_PROFILE_DEV_OPT_LEVEL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0 CARGO_TARGET_DIR=target-gen/fast \
//!   cargo test -p terraria-gen-game -j 4 --test worldgen -- --nocapture
//! ```
//! `WORLDGEN_PASSES=<n>`: stop after pass index n - 1 (default: all 106). `WORLDGEN_REQUIRE=<n>`: the test fails
//! unless the first n rows are identical (default 0: the test reports and passes; it fails on a stop only;
//! `WORLDGEN_REQUIRE=all`: every row of the reference).
//!
//! `WORLDGEN_WORLD=<tag>`: another world of the matrix `refdata/worldgen/matrix/` (the real game run by
//! `tools/refdump_worldgen/GenX.cs`, which has the source references of every parameter): seed text, size, game
//! mode, evil and special-seed flags come from `<tag>.summary.json`, the rows from `<tag>.passes.jsonl` /
//! `<tag>.columns.bin`. The launch parameters are then performed as that harness performs them:
//! `main.autoCreate("1|2|3")`, `main.SetWorldName`, `new WorldFileData(path, false).SetAsActive()`,
//! `Main.GameMode = n`, `WorldGenerationOptions.TryEnablingFlagFrom("seed_<flag>=1")`, `Main.AutogenSeedName`,
//! `WorldGen.WorldGenParam_Evil` (a flag `secret.<name>` is a secret seed: `WorldGen.SecretSeed.Enable(<the seed whose
//! Localization is "SecretSeedDescription.<name>">, false)`, as the server console's seed entry); in the autogen block every option with `AutoGenEnabled` gets `Enabled = true`
//! (WorldFile.cs:690-696). A pass disabled by `DisablePassesForSpecialSeeds` has no row on either side.
//! With `WORLDGEN_REAL=1` the file is `<target>/tmp/worldgen-save-<tag>/Worlds/<tag>.wld`; the real game's is
//! `refdata/worldgen/matrix/<tag>.wld`.

#![allow(unused_mut, unused_variables, unused_assignments, unused_unsafe, unused_parens, unused_braces, non_snake_case, unused_imports)]

#[path = "p1/json.rs"]
mod json;
#[path = "worldgen_diag/dungeon.rs"]
mod dungeon_diag;

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

/// Runs `f`; a stop or an uncaught C# exception is returned as text.
fn step(name: &str, f: impl FnOnce()) -> Result<(), String> {
    let t0 = std::time::Instant::now();
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| rt::run(f))) {
        Ok(Ok(())) => {
            eprintln!("ok   {name}   ({:.2} s)", t0.elapsed().as_secs_f64());
            Ok(())
        }
        Ok(Err(e)) => Err(format!("{name}: C# exception: {e}")),
        Err(p) => Err(format!("{name}: stopped: {}", panic_text(&p))),
    }
}

const FNV: u64 = 0xcbf29ce484222325;
fn fnv(mut h: u64, bytes: &[u8]) -> u64 {
    for b in bytes {
        h = (h ^ *b as u64).wrapping_mul(0x100000001b3);
    }
    h
}

struct Tiles {
    whole: u64,
    cols: Vec<u64>,
    cols_hash: u64,
    active: i64,
    wall: i64,
    liquid: i64,
}

/// The tile map as the harness hashes it (refdata/worldgen/README.md, "Tile hash"): 14-byte records, column-major.
unsafe fn tiles() -> Tiles {
    let ms = &*terraria::main::s();
    let mut t = Tiles { whole: FNV, cols: Vec::with_capacity(ms.max_tiles_x as usize), cols_hash: FNV, active: 0, wall: 0, liquid: 0 };
    let mut col = vec![0u8; ms.max_tiles_y as usize * 14];
    for x in 0..ms.max_tiles_x {
        for y in 0..ms.max_tiles_y {
            let rec = &mut col[y as usize * 14..y as usize * 14 + 14];
            let h = *ms.tile.at(x, y);
            if h.is_null() {
                rec.fill(0);
                continue;
            }
            let h = &*h.ptr();
            rec[0..2].copy_from_slice(&h.type_.to_le_bytes());
            rec[2..4].copy_from_slice(&h.wall.to_le_bytes());
            rec[4] = h.liquid;
            rec[5..7].copy_from_slice(&h.s_tile_header.to_le_bytes());
            rec[7] = h.b_tile_header;
            rec[8] = h.b_tile_header2;
            rec[9] = h.b_tile_header3;
            rec[10..12].copy_from_slice(&h.frame_x.to_le_bytes());
            rec[12..14].copy_from_slice(&h.frame_y.to_le_bytes());
            t.active += (h.s_tile_header & 0x20 != 0) as i64;
            t.wall += (h.wall != 0) as i64;
            t.liquid += (h.liquid != 0) as i64;
        }
        t.whole = fnv(t.whole, &col);
        let c = fnv(FNV, &col);
        t.cols_hash = fnv(t.cols_hash, &c.to_le_bytes());
        t.cols.push(c);
    }
    t
}

/// The 14-byte records of column `x`, as [`tiles`] hashes them.
unsafe fn column_records(x: i32) -> Vec<u8> {
    let ms = &*terraria::main::s();
    let mut col = vec![0u8; ms.max_tiles_y as usize * 14];
    for y in 0..ms.max_tiles_y {
        let rec = &mut col[y as usize * 14..y as usize * 14 + 14];
        let h = *ms.tile.at(x, y);
        if h.is_null() {
            continue;
        }
        let h = &*h.ptr();
        rec[0..2].copy_from_slice(&h.type_.to_le_bytes());
        rec[2..4].copy_from_slice(&h.wall.to_le_bytes());
        rec[4] = h.liquid;
        rec[5..7].copy_from_slice(&h.s_tile_header.to_le_bytes());
        rec[7] = h.b_tile_header;
        rec[8] = h.b_tile_header2;
        rec[9] = h.b_tile_header3;
        rec[10..12].copy_from_slice(&h.frame_x.to_le_bytes());
        rec[12..14].copy_from_slice(&h.frame_y.to_le_bytes());
    }
    col
}

fn text_of(j: Option<&Json>) -> String {
    match j {
        Some(Json::Str(s)) => s.clone(),
        Some(Json::Num(n)) => n.clone(),
        Some(Json::Bool(b)) => b.to_string(),
        _ => String::new(),
    }
}
fn json_strings(j: Option<&Json>) -> Vec<String> {
    match j {
        Some(Json::Arr(a)) => a.iter().map(|f| text_of(Some(f))).collect(),
        _ => Vec::new(),
    }
}
fn hex(j: &Json, key: &str) -> u64 {
    u64::from_str_radix(&text_of(j.get(key)), 16).unwrap_or(0)
}
fn num(j: &Json, path: &[&str]) -> f64 {
    let mut j = j;
    for p in path {
        match j.get(p) {
            Some(v) => j = v,
            None => return f64::NAN,
        }
    }
    text_of(Some(j)).parse().unwrap_or(f64::NAN)
}

/// The world of this run: the reference scenario, or one of the matrix (`WORLDGEN_WORLD`).
struct World {
    tag: String,
    seed_text: String,
    /// 1 small, 2 medium, 3 large (the argument of `Main.autoCreate`)
    size: i32,
    width: usize,
    height: usize,
    game_mode: i32,
    evil: i32,
    flags: Vec<String>,
    rows: Vec<Json>,
    columns: Vec<u8>,
    /// the reference scenario: parameters written as the first harness (Gen.cs) wrote them
    reference_scenario: bool,
}

fn load_world(root: &std::path::Path) -> World {
    let tag = std::env::var("WORLDGEN_WORLD").ok().filter(|t| !t.is_empty() && t != "ref_small");
    let (dir, tag, reference_scenario) = match tag {
        Some(t) => (root.join("refdata/worldgen/matrix"), t, false),
        None => (root.join("refdata/worldgen"), "ref_small".to_owned(), true),
    };
    let rows: Vec<Json> = std::fs::read_to_string(dir.join(format!("{tag}.passes.jsonl"))).unwrap_or_else(|e| panic!("{tag}.passes.jsonl: {e}")).lines().map(json::parse).collect();
    let columns = std::fs::read(dir.join(format!("{tag}.columns.bin"))).unwrap_or_else(|e| panic!("{tag}.columns.bin: {e}"));
    let mut w = World { tag: tag.clone(), seed_text: "12345".to_owned(), size: 1, width: 4200, height: 1200, game_mode: 0, evil: -1, flags: Vec::new(), rows, columns, reference_scenario };
    if !reference_scenario {
        let summary = json::parse(&std::fs::read_to_string(dir.join(format!("{tag}.summary.json"))).unwrap_or_else(|e| panic!("{tag}.summary.json: {e}")));
        w.seed_text = text_of(summary.get("seed_text"));
        w.width = num(&summary, &["max_tiles_x"]) as usize;
        w.height = num(&summary, &["max_tiles_y"]) as usize;
        w.size = match w.width {
            4200 => 1,
            6400 => 2,
            8400 => 3,
            other => panic!("{tag}.summary.json: max_tiles_x {other}"),
        };
        w.game_mode = num(&summary, &["game_mode"]) as i32;
        w.evil = num(&summary, &["worldgen_param_evil"]) as i32;
        w.flags = json_strings(summary.get("flags"));
    }
    assert_eq!(w.columns.len(), w.rows.len() * w.width * 8, "{tag}.columns.bin does not hold {} rows of {} columns", w.rows.len(), w.width);
    w
}

/// Compares the state with row `row` of the reference; returns the differences (empty: identical).
unsafe fn compare(row: usize, want: &Json, columns: &[u8], width: usize) -> Vec<String> {
    let mut d = Vec::new();
    let t = tiles();
    let ms = &*terraria::main::s();
    if t.whole != hex(want, "tiles_fnv64") || t.cols_hash != hex(want, "cols_fnv64") {
        let base = row * width * 8;
        let bad: Vec<usize> = (0..t.cols.len().min(width)).filter(|x| u64::from_le_bytes(columns[base + x * 8..base + x * 8 + 8].try_into().unwrap()) != t.cols[*x]).collect();
        let (first, last) = (bad.first().copied().unwrap_or(0), bad.last().copied().unwrap_or(0));
        d.push(format!("tiles: {} of {} columns differ (x {first}..={last}; first ones {:?})", bad.len(), t.cols.len(), &bad[..bad.len().min(12)]));
    }
    let mut numbers: Vec<(&str, f64, Vec<&str>)> = Vec::new();
    let mut n = |name: &'static str, got: f64, path: &[&'static str]| numbers.push((name, got, path.to_vec()));
    n("n_active", t.active as f64, &["n_active"]);
    n("n_wall", t.wall as f64, &["n_wall"]);
    n("n_liquid", t.liquid as f64, &["n_liquid"]);
    let r = ms.rand;
    n("rand.next", if r.is_null() { f64::NAN } else { (*r.ptr()).clone().next_raw() as f64 }, &["rand", "next"]);
    n("worldSurface", ms.world_surface, &["main", "worldSurface"]);
    n("rockLayer", ms.rock_layer, &["main", "rockLayer"]);
    n("spawnTileX", ms.spawn_tile_x as f64, &["main", "spawnTileX"]);
    n("spawnTileY", ms.spawn_tile_y as f64, &["main", "spawnTileY"]);
    n("dungeonX", ms.dungeon_x as f64, &["main", "dungeonX"]);
    n("dungeonY", ms.dungeon_y as f64, &["main", "dungeonY"]);
    let chests = ms.chest.as_slice().iter().filter(|c| !c.is_null()).count();
    n("chests", chests as f64, &["counts", "chests"]);
    for (name, got, path) in numbers {
        let w = num(want, &path);
        if got.to_bits() != w.to_bits() && !(got == w) {
            d.push(format!("{name}: {got} here, {w} in the game"));
        }
    }
    d
}

#[test]
fn world_generation_matches_the_real_game() {
    let run = || unsafe {
        std::panic::set_hook(Box::new(|_| {}));
        let root = terraria_host::sys::io::project_dir();
        terraria_gen_game::host::launch::install_resources(&root);
        terraria_host::game::init(terraria_host::game::WindowState { width: 800, height: 600, x: 0, y: 0, active: true, handle: 0, display_width: 1920, display_height: 1080, scale: 1.0 });
        meta::register_all();
        meta::register_json();
        let w = load_world(&root);
        let (rows, columns) = (&w.rows, &w.columns);
        if w.reference_scenario {
            assert_eq!((rows.len(), columns.len()), (108, 108 * 4200 * 8));
        }
        eprintln!("==== world {}: seed text {:?}, {} x {}, game mode {}, evil {}, flags {:?}, {} rows", w.tag, w.seed_text, w.width, w.height, w.game_mode, w.evil, w.flags, rows.len());

        let save = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join(if w.reference_scenario { "worldgen-save".to_owned() } else { format!("worldgen-save-{}", w.tag) });
        let _ = std::fs::remove_dir_all(&save);
        std::fs::create_dir_all(save.join("Worlds")).expect("save root");
        let world = save.join("Worlds").join(format!("{}.wld", w.tag));
        std::env::set_var("TERRARIA_SAVE_DIR", &save);

        let stop = std::cell::RefCell::new(None::<String>);
        let mut main = Gc::NULL;
        let mut st = |name: &str, f: &mut dyn FnMut()| {
            if stop.borrow().is_none() {
                if let Err(e) = step(name, || f()) {
                    eprintln!("STOP {e}");
                    *stop.borrow_mut() = Some(e);
                }
            }
        };
        // ---- a
        st("Program.SavePath = <scratch>", &mut || (*terraria::program::s()).save_path = Str::new(&save.to_string_lossy()));
        st("Main.dedServ = true; Main.showSplash = false", &mut || {
            (*terraria::main::s()).ded_serv = true;
            (*terraria::main::s()).show_splash = false;
        });
        st("LanguageManager.Instance.SetLanguage(GameCulture.DefaultCulture)", &mut || {
            let lm = (*terraria::localization::language_manager::s()).instance;
            terraria::localization::language_manager::set_language__game_culture(lm.ptr(), terraria::localization::game_culture::default_culture());
        });
        st("main = new Main()", &mut || main = terraria::main::new());
        st("Lang.InitializeLegacyLocalization()", &mut || terraria::lang::initialize_legacy_localization());
        // ---- b (Main.cs:5062-5065, 5315; LaunchInitializer.cs:181-182, 205)
        if w.reference_scenario {
            st("launch parameters: -autocreate 1 -worldname ref_small -world <path> -seed 12345", &mut || {
                let ms = terraria::main::s();
                (*ms).max_tiles_x = 4200;
                (*ms).max_tiles_y = 1200;
                (*ms).auto_gen = true;
                (*ms).world_name = Str::lit("ref_small");
                let wfd = terraria::io::world_file_data::new__string_bool(Str::new(&world.to_string_lossy()), false);
                terraria::io::world_file_data::set_as_active(wfd.ptr());
                (*ms).auto_gen_file_location = Str::new(&world.to_string_lossy());
                (*ms).autogen_seed_name = Str::lit("12345");
            });
        } else {
            // the steps of tools/refdump_worldgen/GenX.cs (RunGenX), in its order
            st(&format!("launch parameters: -autocreate {} -worldname {} -world <path>, difficulty={}, flags {:?}, -seed {}, WorldGenParam_Evil {}", w.size, w.tag, w.game_mode, w.flags, w.seed_text, w.evil), &mut || {
                let ms = terraria::main::s();
                terraria::main::auto_create(main.ptr(), Str::new(&w.size.to_string()));
                terraria::main::set_world_name(main.ptr(), Str::new(&w.tag));
                let wfd = terraria::io::world_file_data::new__string_bool(Str::new(&world.to_string_lossy()), false);
                terraria::io::world_file_data::set_as_active(wfd.ptr());
                (*ms).auto_gen_file_location = Str::new(&world.to_string_lossy());
                terraria::main::set_game_mode(w.game_mode);
                let mut options_named = 0;
                for f in &w.flags {
                    if let Some(name) = f.strip_prefix("secret.") {
                        // GenX.cs: a secret seed, named by its localization key, enabled as the server console does
                        // (WorldGen.SecretSeed.Enable(seed, playSound: false))
                        let key = format!("SecretSeedDescription.{name}");
                        let all = (*terraria::world_gen::secret_seed::s()).all_secret_seeds;
                        let seed = (0..(*all.ptr()).count()).map(|k| (*all.ptr()).get_item(k)).find(|s| (*s.ptr()).localization.as_str() == key);
                        let seed = seed.unwrap_or_else(|| panic!("no secret seed has the localization key {key}"));
                        terraria::world_gen::secret_seed::enable(seed, false);
                        assert!(terraria::world_gen::secret_seed::enabled(seed.ptr()), "{key} is not enabled");
                        continue;
                    }
                    options_named += 1;
                    terraria::world_building::world_generation_options::try_enabling_flag_from(Str::new(&format!("seed_{f}=1")));
                }
                let options = (*terraria::world_building::world_generation_options::s())._options;
                let auto: usize = (0..(*options.ptr()).count()).filter(|k| (*(*options.ptr()).get_item(*k).ptr()).auto_gen_enabled).count();
                assert_eq!(auto, options_named, "every flag of the summary names one world generation option");
                (*ms).autogen_seed_name = Str::new(&w.seed_text);
                (*terraria::world_gen::s()).world_gen_param_evil = w.evil;
                assert_eq!(((*ms).max_tiles_x as usize, (*ms).max_tiles_y as usize), (w.width, w.height));
            });
        }
        // ---- c
        st("Main.rand = new UnifiedRandom(0)", &mut || (*terraria::main::s()).rand = rt::alloc_init(terraria_core::UnifiedRandom::new(0)));
        st("main.Initialize()", &mut || terraria::main::initialize(main.ptr()));
        if std::env::var_os("WORLDGEN_REAL").is_some_and(|v| !v.is_empty() && v != "0") {
            st("WorldGen.serverLoadWorldCallBack() (generate, save, load)", &mut || {
                terraria::world_gen::server_load_world_call_back();
                assert!(!(*terraria::world_gen::s()).load_failed, "WorldGen.loadFailed");
            });
            let failed = stop.borrow().clone();
            eprintln!("==== WORLDGEN_REAL: {}; world file {}", failed.clone().unwrap_or_else(|| "generated, saved and loaded".to_owned()), world.display());
            assert!(failed.is_none() && world.is_file(), "the world file was not written");
            return;
        }
        // ---- d (WorldFile.cs:679-705)
        st(&format!("WorldFile.CreateMetadata + the seed text handling + SetSeed({:?})", w.seed_text), &mut || {
            let ms = terraria::main::s();
            (*ms).active_world_file_data = terraria::io::world_file::create_metadata((*ms).world_name, false, terraria::main::game_mode());
            let text = Str::new(&w.seed_text);
            let (mut a, mut b, mut c) = (Str::NULL, Str::NULL, Gc::NULL);
            assert!(!terraria::io::world_file_data::try_applying_copied_seed(text, true, &mut a, &mut b, &mut c), "the seed text is not a copied seed");
            let text = terraria::utils::trim_user_string(text, (*terraria::io::world_file_data::s()).max_user_seed_text_length);
            let option = terraria::world_building::world_generation_options::get_option_from_seed_text(text);
            assert!(option.is_null(), "the seed text selects no special-seed option (the game would draw a random seed)");
            terraria::world_building::world_generation_options::select_option(option);
            // WorldFile.cs:690-696
            let options = (*terraria::world_building::world_generation_options::s())._options;
            for k in 0..(*options.ptr()).count() {
                let o = (*options.ptr()).get_item(k);
                if (*o.ptr()).auto_gen_enabled {
                    terraria::world_building::a_world_generation_option::set_enabled(o.ptr(), true);
                }
            }
            terraria::io::world_file_data::set_seed((*ms).active_world_file_data.ptr(), text);
        });
        // ---- e (WorldGen.cs:11082-11095)
        let seed = if stop.borrow().is_none() { terraria::io::world_file_data::seed((*terraria::main::s()).active_world_file_data.ptr()) } else { 0 };
        st("GenerateWorld up to the pass loop: configuration, new WorldGenerator, clearWorld, Reset, AddPasses, DisablePassesForSpecialSeeds", &mut || {
            let wg = terraria::world_gen::s();
            (*wg).generating_world = true;
            (*wg).is_generating_or_loading_world = true;
            (*wg).generating_world_on_this_thread = true;
            let gv = terraria::world_building::gen_vars::s();
            (*gv).configuration = terraria::world_building::world_gen_configuration::from_embedded_path(Str::lit("Terraria.GameContent.WorldBuilding.Configuration.json"));
            terraria::world_gen::hooks::process_world_gen_config(core::ptr::addr_of_mut!((*gv).configuration));
            (*wg)._generator = terraria::world_building::world_generator::new(seed, (*gv).configuration, (*terraria::main::s()).autogen_progress, Gc::NULL);
            terraria::world_gen::clear_world();
            terraria::world_gen::reset();
            terraria::world_gen::add_passes();
            terraria::world_gen::disable_passes_for_special_seeds((*(*wg)._generator.ptr())._passes);
        });
        let limit: usize = std::env::var("WORLDGEN_PASSES").ok().and_then(|v| v.parse().ok()).unwrap_or(usize::MAX);
        let mut identical = 0usize;
        let mut first_bad: Option<(usize, String, Vec<String>)> = None;
        let mut done = 0usize;
        let mut skipped = 0usize;
        let mut total = 0usize;
        if stop.borrow().is_none() {
            let last_row = rows.len() - 1;
            let mut row = |r: usize, name: &str| {
                if r >= rows.len() {
                    eprintln!("row {r:3} {name}: the reference has only {} rows", rows.len());
                    if first_bad.is_none() {
                        first_bad = Some((r, name.to_owned(), vec!["no such row in the reference".to_owned()]));
                    }
                    return;
                }
                let d = compare(r, &rows[r], columns, w.width);
                // diagnosis: WORLDGEN_DUMPCOLS=<row>:<x>,<x>,.. writes those columns' 14-byte tile records (the
                // harness's snapshot layout, tools/refdump_worldgen/GenX.cs) to worldgen_cols_<row>.bin in the
                // current directory, to compare cell by cell with a reference snapshot (.tfrm.z)
                if let Ok(spec) = std::env::var("WORLDGEN_DUMPCOLS") {
                    if let Some((sr, xs)) = spec.split_once(':') {
                        if sr.parse::<usize>().ok() == Some(r) {
                            let mut out = Vec::new();
                            for x in xs.split(',').filter_map(|x| x.parse::<i32>().ok()) {
                                out.extend_from_slice(&column_records(x));
                            }
                            std::fs::write(format!("worldgen_cols_{r}.bin"), out).unwrap();
                        }
                    }
                }
                let want = text_of(rows[r].get("name"));
                if r != 0 && r != last_row && want != name {
                    eprintln!("row {r}: pass {name:?} here, {want:?} in the game");
                }
                if d.is_empty() {
                    if first_bad.is_none() {
                        identical += 1;
                    }
                    eprintln!("row {r:3} {name}: identical");
                } else {
                    eprintln!("row {r:3} {name}: DIFFERENT: {}", d.join("; "));
                    if first_bad.is_none() {
                        first_bad = Some((r, name.to_owned(), d));
                    }
                }
            };
            row(0, "(after clearWorld + Reset, before pass 0)");
            let gen = (*terraria::world_gen::s())._generator;
            let passes = (*gen.ptr())._passes;
            let n = (*passes.ptr()).count();
            eprintln!("{n} passes, seed {seed}");
            total = n as usize;
            let progress = (*gen.ptr())._progress;
            // WorldGenerator.GenerateWorld (421-427)
            (*terraria::world_building::world_generator::s()).current_generation_progress = progress;
            for i in 0..n {
                if i as usize >= limit {
                    break;
                }
                let pass = (*passes.ptr()).get_item(i);
                let name = (*pass.ptr()).name.as_str().to_owned();
                // RunPass: a disabled pass is skipped (WorldGenerator.cs:490-497): no row on either side
                if !terraria::world_building::gen_pass::enabled(pass.ptr()) {
                    eprintln!("pass {i} {name}: disabled");
                    skipped += 1;
                    continue;
                }
                // diagnosis: WORLDGEN_DUNGEON=<pass index> writes the dungeons of that pass (worldgen_diag/dungeon.rs)
                let dungeon_watch = std::env::var("WORLDGEN_DUNGEON").ok().and_then(|v| v.parse::<i32>().ok()) == Some(i);
                if dungeon_watch {
                    dungeon_diag::start(&format!("worldgen_{}.dungeon.txt", w.tag));
                }
                // RunPass (488-517)
                let r = step(&format!("pass {i} {name}"), || {
                    (*terraria::main::s()).rand = rt::alloc_init(terraria_core::UnifiedRandom::new(seed));
                    terraria::world_building::generation_progress::start(progress.ptr(), (*pass.ptr()).weight);
                    let cfg = terraria::world_building::world_gen_configuration::get_pass_configuration((*gen.ptr())._configuration.ptr(), Str::new(&name));
                    terraria::world_building::gen_pass::apply(pass.ptr(), progress, cfg);
                });
                if dungeon_watch {
                    dungeon_diag::stop();
                }
                if let Err(e) = r {
                    eprintln!("STOP {e}");
                    *stop.borrow_mut() = Some(e);
                    break;
                }
                done += 1;
                row(done, &name);
                let r = step(&format!("after pass {i}: progress.End(), genRand.Next()"), || {
                    terraria::world_building::generation_progress::end(progress.ptr());
                    let _ = (*(*terraria::main::s()).rand.ptr()).next_raw();
                });
                if let Err(e) = r {
                    *stop.borrow_mut() = Some(e);
                    break;
                }
            }
            if stop.borrow().is_none() && done + skipped == n as usize {
                // ---- f
                if let Err(e) = step("Finish() and the finally block of GenerateWorld", || {
                    terraria::world_gen::finish();
                    terraria::world_gen::restore_temporary_state_changes();
                    let wg = terraria::world_gen::s();
                    (*wg).generating_world = false;
                    (*wg).is_generating_or_loading_world = false;
                    (*wg).generating_world_on_this_thread = false;
                    // WorldGen.cs:11107-11114
                    (*wg).drunk_world_gen = false;
                    (*wg).not_the_bees = false;
                    (*wg).no_traps_world_gen = false;
                    (*wg).get_good_world_gen = false;
                    (*wg).tenth_anniversary_world_gen = false;
                    (*wg).dont_starve_world_gen = false;
                    (*wg).remix_world_gen = false;
                    (*wg).everything_world_gen = false;
                    (*wg).skyblock_world_gen = false;
                }) {
                    *stop.borrow_mut() = Some(e);
                } else {
                    row(done + 1, "(after GenerateWorld)");
                }
            }
        }
        eprintln!("==== Proof 2 ({}): {done} of {} passes ran ({skipped} disabled); rows identical from the start: {identical} of {}", w.tag, total - skipped, rows.len());
        match &first_bad {
            Some((r, name, d)) => eprintln!("==== first difference: row {r} ({name}): {}", d.join("; ")),
            None => eprintln!("==== no difference in the rows that were compared"),
        }
        if let Some(e) = stop.borrow().as_ref() {
            eprintln!("==== stopped: {e}");
        }
        let require: usize = match std::env::var("WORLDGEN_REQUIRE").ok().as_deref() {
            Some("all") => rows.len(),
            v => v.and_then(|v| v.parse().ok()).unwrap_or(0),
        };
        assert!(identical >= require, "{identical} rows identical, {require} required");
        assert!(stop.borrow().is_none(), "world generation stopped: {}", stop.borrow().clone().unwrap_or_default());
    };
    // world generation recurses deeply (flood fills): the game's generation thread has a large stack too
    terraria_host::run_with_stack(1 << 30, "the test thread", run);
}
