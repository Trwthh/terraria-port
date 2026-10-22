//! Client start-up (`Main.dedServ == false`) against the host layer with no GPU: the host records draw commands and
//! nobody replays them.
//!
//! Two modes:
//! - default: DIAGNOSTIC survey. Every top-level statement of the start-up functions runs caught on its own
//!   (`tests/client/steps.rs`, written by `tools/cs2rs/mk_client_survey.py` from the generated code), then
//!   `CLIENT_FRAMES` (default 30) ticks of `Main.Update` / `Main.Draw` through the host's driver protocol, each call
//!   caught. One build lists the first stop of every statement. Proves nothing: state after a failed step is incomplete.
//! - `CLIENT_WHOLE=1`: the sequence as `Program.LaunchGame` / `Game.Run` perform it, ONE call each, no catching between
//!   statements. The test fails on any stop, on any unimplemented `void` host member that is not declared inert, and on
//!   any gap.
//!
//! - `CLIENT_SCRIPT=<file>`: DIAGNOSTIC. The sequence of the app's `terraria` binary (single calls for the launch
//!   and `Main.Initialize`), then ticks with the input of an app input script (`terraria-app/scripts/*.input`, same
//!   tick numbers as under the shell; `shot` lines are ignored), every `Update` / `Draw` caught. Lists the stops of a
//!   scripted session (menu navigation, entering a world) without a GPU and without rebuilding the app.
//!
//! - `CLIENT_SAVE_FROM=<folder>` (with `CLIENT_SCRIPT`): the save root of an earlier session is copied into the
//!   scratch save root first, so the session starts with a config, a player and a world on disk.
//!
//! - `CLIENT_POISON=1` (with `CLIENT_SCRIPT`): the collector's poison mode for the whole session; build with
//!   `--features gc-check` to have every dereference of a freed object panic (the "gc-check session").
//!
//! Run: `sh tools/cs2rs/p1.sh --client` (survey) / `sh tools/cs2rs/p1.sh --client-whole`.

#![allow(unused_mut, unused_variables, unused_assignments, unused_unsafe, unused_parens, unused_braces, non_snake_case, unused_imports)]

use terraria_gen_game as game;
use terraria_gen_game::gen::*;
use terraria_gen_game::rt::{self, *};
use terraria_gen_game::s;
use terraria_host::game as hg;

fn panic_text(p: &Box<dyn std::any::Any + Send>) -> String {
    if let Some(s) = p.downcast_ref::<String>() {
        s.clone()
    } else if let Some(s) = p.downcast_ref::<&str>() {
        (*s).to_owned()
    } else {
        "<non-string panic payload>".to_owned()
    }
}

thread_local! {
    /// collections, objects freed, bytes freed, most live objects after a collection
    static COLLECTIONS: std::cell::RefCell<(u64, u64, u64, u64)> = const { std::cell::RefCell::new((0, 0, 0, 0)) };
}

static mut FAILED: u32 = 0;
static mut STEPS: u32 = 0;
/// (stop text, count, first statement)
static STOPS: std::sync::Mutex<Vec<(String, u32, String)>> = std::sync::Mutex::new(Vec::new());

fn summary() {
    let stops = STOPS.lock().unwrap();
    eprintln!("==== distinct stops: {}", stops.len());
    for (e, n, first) in stops.iter() {
        eprintln!("STOP x{n} {} | first: {}", e.chars().take(420).collect::<String>(), first.chars().take(110).collect::<String>());
    }
}

fn short(name: &str) -> String {
    let one: String = name.split_whitespace().collect::<Vec<_>>().join(" ");
    one.chars().take(150).collect()
}

/// Runs `f` caught; returns the failure text.
fn step(name: &str, f: impl FnOnce()) -> Option<String> {
    let t0 = std::time::Instant::now();
    unsafe { STEPS += 1 };
    let r = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| rt::run(f))) {
        Ok(Ok(())) => None,
        Ok(Err(e)) => Some(format!("C# exception: {e}")),
        Err(p) => Some(format!("stopped: {}", panic_text(&p))),
    };
    match &r {
        None => {
            if t0.elapsed().as_secs_f64() > 0.25 {
                eprintln!("ok   {}   ({:.2} s)", short(name), t0.elapsed().as_secs_f64())
            }
        }
        Some(e) => {
            unsafe { FAILED += 1 };
            let mut stops = STOPS.lock().unwrap();
            match stops.iter_mut().find(|(k, _, _)| k == e) {
                Some(x) => x.1 += 1,
                None => stops.push((e.clone(), 1, short(name))),
            }
            if std::env::var_os("CLIENT_VERBOSE").is_some() {
                eprintln!("FAIL {}: {}", short(name), e.chars().take(600).collect::<String>())
            }
        }
    }
    r
}

macro_rules! st {
    ($($e:tt)*) => { let _ = step(stringify!($($e)*), || { $($e)*; }); };
}

include!("client/steps.rs");

/// The app's input script parser (no Bevy in it): the same file drives the hidden app and this test.
#[allow(dead_code)]
#[path = "../../terraria-app/src/shell/script.rs"]
mod script;
/// The script language's debug-state commands (the same file the app's `terraria` binary includes).
#[path = "../src/host/debug_state.rs"]
mod debug_state;

thread_local! {
    /// the scripted window: client size, has the focus, is minimised (`resize`, `focus`, `minimise` of a script)
    static WINDOW: std::cell::Cell<(i32, i32, bool, bool)> = const { std::cell::Cell::new((1280, 720, true, false)) };
}

fn window() -> hg::WindowState {
    let (width, height, focused, minimised) = WINDOW.get();
    hg::WindowState { width, height, x: 0, y: 0, active: focused && !minimised, handle: 0, display_width: 1920, display_height: 1080, scale: 1.0 }
}

unsafe fn install() {
    let root = terraria_host::sys::io::project_dir();
    game::host::launch::install_resources(&root);
    // the save root of this test: a scratch folder of the build directory, emptied first
    let save = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("client-save");
    let _ = std::fs::remove_dir_all(&save);
    std::fs::create_dir_all(&save).expect("save root");
    // `CLIENT_WORLD=<file.wld>`: a world for the world list (copied into the scratch save root before the launch)
    if let Some(w) = std::env::var_os("CLIENT_WORLD") {
        let w = std::path::PathBuf::from(w);
        std::fs::create_dir_all(save.join("Worlds")).expect("Worlds");
        std::fs::copy(&w, save.join("Worlds").join(w.file_name().expect("world file name"))).unwrap_or_else(|e| panic!("CLIENT_WORLD {}: {e}", w.display()));
    }
    // `CLIENT_SAVE_FROM=<folder>`: an existing save root (config.json, Players, Worlds of an earlier session) is copied
    // into the scratch one: the "second start" paths (no language menu, an existing player and world)
    if let Some(from) = std::env::var_os("CLIENT_SAVE_FROM") {
        fn copy_dir(from: &std::path::Path, to: &std::path::Path) {
            std::fs::create_dir_all(to).expect("create");
            for e in std::fs::read_dir(from).unwrap_or_else(|e| panic!("CLIENT_SAVE_FROM {}: {e}", from.display())) {
                let e = e.expect("entry");
                let (src, dst) = (e.path(), to.join(e.file_name()));
                if src.is_dir() {
                    copy_dir(&src, &dst)
                } else {
                    std::fs::copy(&src, &dst).expect("copy");
                }
            }
        }
        copy_dir(std::path::Path::new(&from), &save);
    }
    std::env::set_var("TERRARIA_SAVE_DIR", &save);
    hg::init(window());
    // `o.GetType()` of a translated object needs the reflection data of its type by type id
    meta::register_all();
    meta::register_json();
}

/// `Program.LaunchGame` + `Program.RunGame` up to `main.Run()` (Program.cs:178-234), with `SocialMode.None`
/// (terraria-host REQUESTS.md section 11).
unsafe fn program(main: &mut Gc<terraria::main::Main>) {
    st!((*terraria_host::sys::threading::Task::current_thread().ptr()).set_name(s!("Main Thread")));
    st!((*terraria::program::s()).launch_parameters = terraria::utils::parse_arguements(Arr::<Str>::from_slice(&[])));
    st!((*terraria::program::s()).save_path = (*terraria_host::os::platform_get::<Gc<terraria_host::os::PathService>>().ptr()).get_storage_path(s!("Terraria")));
    st!(let _ = terraria_host::sys::threading::thread_pool_set_min_threads(8, 8));
    st!(terraria::program::initialize_console_output());
    st!(terraria::program::setup_logging());
    st!(terraria::localization::language_manager::set_language__game_culture((*terraria::localization::language_manager::s()).instance.ptr(), terraria::localization::game_culture::default_culture()));
    st!(*main = terraria::main::new());
    st!(terraria::lang::initialize_legacy_localization());
    st!(terraria::social::social_api::initialize(Nullable::some(0 /* SocialMode.None */)));
    st!(terraria::initializers::launch_initializer::load_parameters(*main));
    st!({
        let p = core::ptr::addr_of_mut!((*terraria::main::s()).on_engine_preload__event);
        *p = Delegate::combine(*p, Delegate::<dyn Fn()>::new(Box::new(move || unsafe { terraria::program::start_force_load() })))
    });
}

fn report() {
    eprintln!("gaps reached: {}", rt::gaps().len());
    for (g, n) in rt::gaps() {
        eprintln!("    gap {g}: {n}");
    }
    eprintln!("host members reached: {}", game::host::reached().len());
    for (g, n) in game::host::reached() {
        eprintln!("    host {g}: {n}");
    }
    eprintln!("host members unimplemented (void / opaque): {}", game::rt::host::unimplemented_reached().len());
    for (g, n) in game::rt::host::unimplemented_reached() {
        eprintln!("    unimpl {g}: {n}");
    }
    eprintln!("inert: {:?}", game::rt::host::inert_reached());
    eprintln!("stubs: {:?}", rt::stubs_hit());
}

/// `frames` ticks of the driver protocol (terraria-host README "Driver protocol") with an idle input.
unsafe fn frames(main: Gc<terraria::main::Main>, frames: u32, catch_each: bool) -> Vec<String> {
    frames_with(main, frames, catch_each, None)
}

unsafe fn frames_with(main: Gc<terraria::main::Main>, frames: u32, catch_each: bool, mut input: Option<&mut script::Script>) -> Vec<String> {
    let mut seen: Vec<String> = Vec::new();
    let mut note = |what: &str, tick: u32, r: Option<String>| {
        if let Some(e) = r {
            let key = format!("{what}: {e}");
            if !seen.contains(&key) {
                eprintln!("     first at tick {tick}");
                seen.push(key);
            }
        }
    };
    // as the app's shell: a script's clock stands still while queued work is suspended (a load in progress)
    let mut script_tick = 0u64;
    let mut stalled = 0u32;
    let mut tick = 0u32;
    while script_tick < frames as u64 {
        tick += 1;
        if input.is_some() && terraria_host::sys::threading::has_suspended_foreground_work() && stalled < 100_000 {
            stalled += 1;
        } else {
            stalled = 0;
            script_tick += 1;
        }
        let snapshot = match input.as_mut() {
            Some(s) => {
                let (mut snap, fx) = s.step(script_tick);
                let (focused, minimised) = s.window_state();
                let (w, h) = fx.resize.map(|(w, h)| (w as i32, h as i32)).unwrap_or((WINDOW.get().0, WINDOW.get().1));
                WINDOW.set((w, h, focused, minimised));
                if fx.quit {
                    eprintln!("script: quit at tick {tick}");
                    break;
                }
                // the debug-state commands of the script language, as the app runs them (before the Update)
                for c in &fx.commands {
                    let mut result = Err("the command did not run".to_owned());
                    let r = step(&format!("script command {c}"), || result = debug_state::command(c));
                    match (r, result) {
                        (None, Ok(reply)) => {
                            if let Some((x, y)) = reply.mouse {
                                s.set_mouse(x, y);
                                (snap.mouse_x, snap.mouse_y) = (x, y);
                            }
                            if let Some(j) = reply.json {
                                eprintln!("state tick {tick} {c}: {}", j.chars().take(std::env::var("CLIENT_STATE_WIDTH").ok().and_then(|v| v.parse().ok()).unwrap_or(600)).collect::<String>());
                            }
                        }
                        (None, Err(e)) => eprintln!("state tick {tick} {c}: FAILED {e}"),
                        (Some(e), _) => eprintln!("state tick {tick} {c}: FAILED {e}"),
                    }
                }
                snap
            }
            None => terraria_host::input::InputSnapshot { mouse_x: 640, mouse_y: 360, ..Default::default() },
        };
        hg::begin_frame(snapshot, window(), 1.0 / 60.0);
        hg::dispatch_events();
        while let Some(t) = hg::begin_update() {
            if catch_each {
                let r = step(&format!("Main.Update tick {tick}"), || terraria::main::update(main.ptr(), t));
                // a stop inside Update leaves the re-entrancy guard set
                (*main.ptr())._is_drawing_or_updating = false;
                note("update", tick, r);
            } else {
                terraria::main::update(main.ptr(), t);
            }
            if catch_each {
                let r = step(&format!("game::end_update (queued thread work) tick {tick}"), || hg::end_update());
                note("end_update", tick, r);
            } else {
                hg::end_update();
            }
        }
        // a minimised window draws nothing (the shell, step 5)
        let dt = if WINDOW.get().3 { None } else { hg::begin_draw() };
        if let Some(t) = dt {
            if catch_each {
                let r = step(&format!("Main.Draw tick {tick}"), || terraria::main::draw(main.ptr(), t));
                (*main.ptr())._is_drawing_or_updating = false;
                note("draw", tick, r);
            } else {
                terraria::main::draw(main.ptr(), t);
            }
        }
        // a stop inside Draw can leave a batch open: Present would then fail too, and that must not end the survey
        let mut exit = false;
        if catch_each {
            let r = step(&format!("game::end_draw (Present) tick {tick}"), || exit = hg::end_draw(dt.is_some()).exit);
            note("end_draw", tick, r);
        } else {
            exit = hg::end_draw(dt.is_some()).exit;
        }
        // the collector as the app shell runs it: a cycle every 120 ticks, a slice per tick while it runs, the
        // mark phase never ended while a work item is suspended. `CLIENT_GC=full`: one whole collection instead.
        let suspended = terraria_host::sys::threading::has_suspended_work() && !rt::has_conservative_roots();
        let full = std::env::var("CLIENT_GC").is_ok_and(|v| v == "full");
        let count = |st: rt::GcStats| {
            COLLECTIONS.with(|c| {
                let mut c = c.borrow_mut();
                c.0 += 1;
                c.1 += st.freed_objects as u64;
                c.2 += st.freed_bytes as u64;
                c.3 = c.3.max(st.live_objects as u64);
            })
        };
        if full {
            if tick % 120 == 0 && !suspended {
                let r = step(&format!("rt::collect() tick {tick}"), || count(rt::collect()));
                note("collect", tick, r);
            }
        } else if rt::gc_phase() == rt::GcPhase::Idle {
            if tick % 120 == 0 {
                let r = step(&format!("rt::gc_begin() tick {tick}"), || rt::gc_begin());
                note("collect", tick, r);
            }
        } else {
            let r = step(&format!("rt::gc_step() tick {tick}"), || {
                if let Some(st) = rt::gc_step(rt::GcBudget::Work(100_000), !suspended).finished {
                    count(st)
                }
            });
            note("collect", tick, r);
        }
        if exit {
            eprintln!("the game asked to exit at tick {tick}");
            break;
        }
    }
    seen
}

fn frame_count() -> u32 {
    std::env::var("CLIENT_FRAMES").ok().and_then(|v| v.parse().ok()).unwrap_or(30)
}

#[test]
fn client_startup() {
    let whole = std::env::var("CLIENT_WHOLE").map(|v| v == "1").unwrap_or(false);
    let run = move || unsafe {
        std::panic::set_hook(Box::new(|_| {}));
        install();
        let mut main: Gc<terraria::main::Main> = Gc::NULL;
        if let Some(path) = std::env::var_os("CLIENT_SCRIPT") {
            let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("CLIENT_SCRIPT {}: {e}", std::path::Path::new(&path).display()));
            let mut sc = script::Script::parse(&text).unwrap_or_else(|e| panic!("{e}"));
            eprintln!("==== launch (single calls, as the app)");
            let _ = step("Program.LaunchGame up to main.Run()", || main = game::host::launch::launch_game(&[]));
            let _ = step("Main.Initialize()", || game::host::launch::initialize(main));
            eprintln!("==== scripted ticks ({} at most)", sc.last_tick + 60);
            let n = (sc.last_tick + 60) as u32;
            // `CLIENT_POISON=1`: freed objects are filled with 0xDD and never reused (rt::set_poison), so a handle
            // that outlives its object reads garbage instead of a recycled object; with the crate feature `gc-check`
            // every dereference of a freed object panics. The gc-check session of the in-game work is this mode.
            let poison = std::env::var_os("CLIENT_POISON").is_some_and(|v| !v.is_empty() && v != "0");
            rt::set_poison(poison);
            // `CLIENT_GC_VERIFY=1`: every mark phase is checked by a second walk of the heap (rt::set_verify)
            rt::set_verify(std::env::var_os("CLIENT_GC_VERIFY").is_some_and(|v| !v.is_empty() && v != "0"));
            // the stacks of suspended work are scanned conservatively, as in the app (`CLIENT_GC_FIBER_SCAN=0`: not,
            // and no mark phase ends while work is suspended)
            if !std::env::var("CLIENT_GC_FIBER_SCAN").is_ok_and(|v| v == "0") {
                rt::set_conservative_roots(Some(|f: &mut dyn FnMut(*const u8, usize)| terraria_host::sys::threading::suspended_stacks(f)));
            }
            // queued work gives the frame back as in a scripted run of the app (`CLIENT_LOAD_SLICE=off`: never)
            if !std::env::var("CLIENT_LOAD_SLICE").is_ok_and(|v| v == "off") {
                terraria_host::sys::threading::set_yield_mode(terraria_host::sys::threading::YieldMode::Loops(1_000_000));
            }
            let seen = frames_with(main, n, true, Some(&mut sc));
            let (count, freed_objects, freed_bytes, max_live) = COLLECTIONS.with(|c| *c.borrow());
            eprintln!(
                "client gc: poison {poison}, gc-check feature {}, {count} collections, {freed_objects} objects / {:.1} MB freed in all, at most {max_live} live objects after a collection",
                cfg!(feature = "gc-check"),
                freed_bytes as f64 / 1048576.0
            );
            eprintln!(
                "client script: {} distinct stop(s); showSplash {} gameMenu {} menuMode {}",
                seen.len(),
                (*terraria::main::s()).show_splash,
                (*terraria::main::s()).game_menu,
                (*terraria::main::s()).menu_mode
            );
            report();
            summary();
        } else if whole {
            let mut failures: Vec<String> = Vec::new();
            let mut one = |name: &str, f: &mut dyn FnMut()| {
                if let Some(e) = step(name, || f()) {
                    failures.push(format!("{name}: {e}"));
                }
            };
            // the sequence the app's `terraria` binary performs (src/host/launch.rs)
            one("Program.LaunchGame up to main.Run()", &mut || main = game::host::launch::launch_game(&[]));
            one("Main.Initialize() (LoadContent inside base.Initialize())", &mut || game::host::launch::initialize(main));
            one("frames", &mut || {
                let _ = frames(main, frame_count(), false);
            });
            report();
            if FAILED > 0 {
                failures.push(format!("{} statement(s) of the launch sequence failed", FAILED));
            }
            // Opaque library objects that are created and never looked into (any use of one stops the run), the same
            // two that Proof 1 accepts: `new TcpClient()` in `TcpSocket..ctor` (no socket host) and
            // `new UTF8Encoding(true, true)` in `FavoritesFile` / `Preferences` (passed to nothing on this path).
            const OPAQUE_UNUSED: [&str; 2] = ["System.Net.Sockets.TcpClient..ctor()", "System.Text.UTF8Encoding..ctor(bool, bool)"];
            for (m, n) in game::rt::host::unimplemented_reached() {
                if OPAQUE_UNUSED.contains(&m) {
                    eprintln!("    opaque object (never looked into): {m} (x{n})");
                    continue;
                }
                failures.push(format!("unimplemented host member reached: {m} (x{n})"));
            }
            for (m, n) in rt::gaps() {
                failures.push(format!("gap reached: {m} (x{n})"));
            }
            eprintln!("client whole: {} failure(s)", failures.len());
            for f in &failures {
                eprintln!("    FAILURE {}", f.chars().take(400).collect::<String>());
            }
            assert!(failures.is_empty(), "client start-up failed");
        } else {
            eprintln!("==== Program");
            program(&mut main);
            eprintln!("==== Main.Initialize");
            initialize(main.ptr());
            let after_init = FAILED;
            // phase 1: the splash screen while assets load. It ends at splashCounter 620 (Main.cs:53400, 53452), where
            // DrawSplash calls Initialize_AlmostEverything + PostContentLoadInitialize as ONE call each; the survey
            // stops before that and runs their statements one by one instead.
            eprintln!("==== frames (splash, 600 ticks)");
            let mut seen = frames(main, 600, true);
            eprintln!(
                "client survey: after the splash ticks: pending assets {}, load process done {}, loaded everything {}",
                re_logic::content::i_asset_repository::pending_assets__v((*terraria::main::s()).assets.ptr() as *mut re_logic::content::i_asset_repository::IAssetRepository),
                (*terraria::main::s())._is_async_load_complete,
                (*terraria::program::s()).loaded_everything
            );
            if (*terraria::main::s()).show_splash {
                eprintln!("==== Main.Initialize_AlmostEverything (end of the splash)");
                initialize_almost_everything(main.ptr());
                eprintln!("==== Main.PostContentLoadInitialize");
                post_content_load_initialize(main.ptr());
                (*terraria::main::s()).show_splash = false;
                (*terraria::main::s()).fade_counter = 120;
                (*(*terraria::main::s()).splash_timer.ptr()).stop();
            }
            eprintln!("==== frames (menu, {} ticks)", frame_count());
            seen.extend(frames(main, frame_count(), true));
            eprintln!("client survey: {} statement(s) failed before the first frame; {} step(s) failed in all; {} distinct stop(s) in the frames", after_init, FAILED, seen.len());
            // Program.ForceStaticInitializers (the force-load "thread"): every static initialiser of the list on its own
            eprintln!("==== static initialisers (gen::types::STATIC_INITS)");
            let before = FAILED;
            for (name, init) in types::STATIC_INITS.iter() {
                let _ = step(&format!("static initialiser of {name}"), || init());
            }
            eprintln!("client survey: {} of {} static initialiser(s) failed", FAILED - before, types::STATIC_INITS.len());
            eprintln!(
                "client survey: showSplash {} gameMenu {} menuMode {} loaded {}",
                (*terraria::main::s()).show_splash,
                (*terraria::main::s()).game_menu,
                (*terraria::main::s()).menu_mode,
                (*terraria::program::s()).loaded_everything
            );
            report();
            summary();
        }
    };
    terraria_host::run_with_stack(512 << 20, "client", run);
}
