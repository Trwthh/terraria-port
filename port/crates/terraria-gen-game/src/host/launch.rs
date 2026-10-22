//! What replaces `Program.Main` for a driver of the translated client (the test `tests/client.rs`, the app's
//! `terraria` binary): the start-up sequence of `Program.LaunchGame` / `Program.RunGame` (Program.cs:178-234), written
//! out statement by statement with translated code, with the differences a port has to make:
//!
//! - `SocialAPI.Initialize(SocialMode.None)` instead of `SocialAPI.Initialize()` (Steam): terraria-host REQUESTS.md 11;
//! - `main.Run()` is the driver's loop (the host's driver protocol), so this stops before it and [`initialize`] is the
//!   first thing `Game.Run` does;
//! - `Platform.Get<IWindowService>().SetQuickEditEnabled(false)` (the console window's quick-edit mode) is left out:
//!   there is no console window to configure.

use crate::gen::{meta, terraria};
use crate::rt::{self, Arr, Delegate, Gc, Nullable, Str};
use crate::s;

/// The game assembly's embedded resources (localisation JSON, data tables), read from the player's own install as the
/// original reads them from its own assembly: the manifest resources of `<install>/Terraria.exe`
/// (`terraria_host::content::install_dir()`, read only), in its manifest order (`rt::resources::install_from_assembly`,
/// as the browser build does). `TERRARIA_RESOURCES=decompile` (diagnosis only): the extracted copies of
/// `<project>/decompile/Terraria` by the names of `tests/data/resources.txt` instead, as before 2026-10-16.
pub fn install_resources(project_root: &std::path::Path) {
    if std::env::var("TERRARIA_RESOURCES").is_ok_and(|v| v == "decompile") {
        let names: Vec<String> = include_str!("../../tests/data/resources.txt").lines().filter(|l| !l.starts_with('#') && !l.is_empty()).map(str::to_owned).collect();
        rt::resources::install(project_root.join("decompile/Terraria"), names);
        return;
    }
    let exe = terraria_host::content::install_dir().join("Terraria.exe");
    let bytes = std::fs::read(&exe).unwrap_or_else(|e| panic!("the game's embedded resources: {}: {e}", exe.display()));
    if let Err(e) = rt::resources::install_from_assembly(bytes) {
        panic!("the game's embedded resources: {}: {e}", exe.display());
    }
}

/// `Program.LaunchGame(args)` + `Program.RunGame()` up to `main.Run()`. `terraria_host::game::init` must have run
/// (`new Main()` creates the graphics device manager through the host). Returns the game object, which the caller
/// must root.
///
/// # Safety
/// Generated code: the simulation thread only.
pub unsafe fn launch_game(args: &[&str]) -> Gc<terraria::main::Main> {
    // `o.GetType()` of a translated object and `Type.GetType(name)` need the reflection data of every type
    meta::register_all();
    // game objects held as `object` by the JSON serialiser (Preferences) are written through their member tables
    meta::register_json();
    // Program.LaunchGame (Program.cs:178)
    (*terraria_host::sys::threading::Task::current_thread().ptr()).set_name(s!("Main Thread"));
    let argv: Vec<Str> = args.iter().map(|a| Str::new(a)).collect();
    (*terraria::program::s()).launch_parameters = terraria::utils::parse_arguements(Arr::<Str>::from_slice(&argv));
    (*terraria::program::s()).save_path = if (*(*terraria::program::s()).launch_parameters.ptr()).contains_key(s!("-savedirectory")) {
        (*(*terraria::program::s()).launch_parameters.ptr()).get_item(s!("-savedirectory"))
    } else {
        (*terraria_host::os::platform_get::<Gc<terraria_host::os::PathService>>().ptr()).get_storage_path(s!("Terraria"))
    };
    let _ = terraria_host::sys::threading::thread_pool_set_min_threads(8, 8);
    terraria::program::initialize_console_output();
    terraria::program::setup_logging();
    // Program.RunGame (Program.cs:195)
    terraria::localization::language_manager::set_language__game_culture((*terraria::localization::language_manager::s()).instance.ptr(), terraria::localization::game_culture::default_culture());
    // (Platform.IsWindows) Main.OnEngineLoad += delegate { Platform.Get<IMouseNotifier>()?.AddMouseHandler(..) }: the
    // host has no IMouseNotifier service (terraria_host::os::PlatformService for Obj), so the handler would do nothing
    // when it runs; it is still subscribed, as in the original
    {
        let p = core::ptr::addr_of_mut!((*terraria::main::s()).on_engine_load__event);
        *p = Delegate::combine(
            *p,
            Delegate::<dyn Fn()>::new(Box::new(move || {
                let notifier = terraria_host::os::platform_get::<rt::Obj>();
                if !notifier.is_null() {
                    let _: () = crate::host::unimplemented_value("IMouseNotifier.AddMouseHandler (the host registered a mouse notifier service without the launch code knowing how to use it)");
                }
            })),
        );
    }
    // work that waits for the main thread (WorldGen.playWorldCallBack: Main.RunOnMainThread(..).Wait()) is served by
    // the game's own main-thread action queue (terraria_host::sys::threading::set_main_thread_pump)
    terraria_host::sys::threading::set_main_thread_pump(|| unsafe { terraria::main::consume_all_main_thread_actions() });
    let main = terraria::main::new();
    terraria::lang::initialize_legacy_localization();
    terraria::social::social_api::initialize(Nullable::some(0 /* SocialMode.None */));
    terraria::initializers::launch_initializer::load_parameters(main);
    {
        let p = core::ptr::addr_of_mut!((*terraria::main::s()).on_engine_preload__event);
        *p = Delegate::combine(*p, Delegate::<dyn Fn()>::new(Box::new(move || unsafe { terraria::program::start_force_load() })));
    }
    main
}

/// The first step of `Game.Run`: `Initialize()`. `Main.Initialize` calls `base.Initialize()` in the middle, which is
/// where XNA calls the virtual `LoadContent()`: the override is handed to the host first.
///
/// # Safety
/// Generated code: the simulation thread only. `main` must be rooted by the caller.
pub unsafe fn initialize(main: Gc<terraria::main::Main>) {
    terraria_host::game::set_load_content(move || unsafe { terraria::main::load_content(main.ptr()) });
    terraria::main::initialize(main.ptr());
}

/// What the run reached that is not translated game code, for a report at exit: unimplemented `void` host members
/// and opaque constructors, gaps, host members by hit count, inert members, configured values.
pub fn reached_report() -> String {
    use std::fmt::Write as _;
    let mut s = String::new();
    let _ = writeln!(s, "unimplemented host members reached (void: continued; constructors: opaque objects): {}", rt::host::unimplemented_reached().len());
    for (m, n) in rt::host::unimplemented_reached() {
        let _ = writeln!(s, "    unimpl {m}: {n}");
    }
    let _ = writeln!(s, "gaps reached (unported void calls, refused bodies): {}", rt::gaps().len());
    for (m, n) in rt::gaps() {
        let _ = writeln!(s, "    gap {m}: {n}");
    }
    let _ = writeln!(s, "inert members (translator config, with reasons): {}", rt::host::inert_reached().len());
    for (m, n) in rt::host::inert_reached() {
        let _ = writeln!(s, "    inert {m}: {n}");
    }
    let _ = writeln!(s, "configured values: {}", rt::stubs_hit().len());
    for (m, n) in rt::stubs_hit() {
        let _ = writeln!(s, "    stub {m}: {n}");
    }
    s
}
