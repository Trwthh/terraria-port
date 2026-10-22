//! `terraria.exe -server <launch parameters>`: the translated game as a dedicated server (`Main.DedServ`), in the
//! same executable as the client. No window, no GPU, no Bevy; console output on stdout / stderr.
//!
//! It exists because "Host & Play" of the original starts `TerrariaServer.exe` as a second process and joins it
//! (`Main.HostAndPlay`, Main.cs: `tServer.StartInfo.FileName = "TerrariaServer.exe"`, arguments
//! `-autoshutdown -password "<pw>" -lang <id> -world "<path>" -worldrollbackstokeep <n> -hosttoken <token>`).
//! The host's `Process.Start()` runs this executable with `-server` in front of those arguments
//! (`terraria_host::sys::misc::Process::start`). It can also be started by hand:
//!
//! ```text
//! terraria.exe -server -world <file.wld> -port 7777 -ip 127.0.0.1 -password "" [-autoshutdown]
//! ```
//!
//! What runs: `Main.dedServ = true` (the field's value in the original's server build), then `Program.LaunchGame`
//! / `RunGame` up to the main loop (`terraria_gen_game::host::launch::launch_game`: `new Main()`,
//! `LaunchInitializer.LoadParameters`), then the translated `Main.DedServ()`: world load, `Netplay.StartServer()`
//! and the server's own loop, which paces itself with `Stopwatch` and `Thread.Sleep`. The host makes those sleeps
//! real and uses them to run the network threads and the collector (`threading::set_blocking_main`).
//!
//! The listener is the game's: `IPAddress.Any` unless `-ip <address>` is given (TcpSocket.cs `StartListening`),
//! port `-port` (default 7777). `TERRARIA_NET_LOOPBACK_ONLY=1` (set by every automated test) forces 127.0.0.1 and
//! suppresses the LAN announcement (`terraria_host::sys::net`).
//!
//! Console input (`Main.startDedInput`: the `help` / `exit` / `save` commands typed into the server's console) is
//! read from stdin by the translated code only when `-autoshutdown` is NOT given, as in the original.

use terraria_gen_game::gen::terraria;
use terraria_gen_game::host::launch;
use terraria_gen_game::rt::{self, Rooted};
use terraria_host::sys::threading;

fn panic_text(p: &Box<dyn std::any::Any + Send>) -> String {
    if let Some(s) = p.downcast_ref::<String>() {
        s.clone()
    } else if let Some(s) = p.downcast_ref::<&str>() {
        (*s).to_owned()
    } else {
        "<non-string panic payload>".to_owned()
    }
}

/// The stack memory of suspended generated code, for the collector (`rt::set_conservative_roots`).
pub fn suspended_stacks(f: &mut dyn FnMut(*const u8, usize)) {
    threading::suspended_stacks(f)
}

thread_local! {
    static GC: std::cell::RefCell<GcState> = const { std::cell::RefCell::new(GcState { cycles: 0, freed_objects: 0, freed_bytes: 0, pauses_over_4ms: 0, max_pause_ms: 0.0, sleeps_since_cycle: 0 }) };
}
struct GcState {
    cycles: u64,
    freed_objects: usize,
    freed_bytes: usize,
    pauses_over_4ms: u32,
    max_pause_ms: f32,
    sleeps_since_cycle: u64,
}

/// What the server does while its main loop sleeps: one slice of the collector (a cycle starts after 48 MB of
/// allocation, as in the client's shell; `TERRARIA_GC_MB`, `TERRARIA_GC_SLICE_US`).
fn idle() {
    let mb: usize = std::env::var("TERRARIA_GC_MB").ok().and_then(|v| v.parse().ok()).unwrap_or(48);
    let slice: u64 = std::env::var("TERRARIA_GC_SLICE_US").ok().and_then(|v| v.parse().ok()).unwrap_or(1500);
    let t0 = std::time::Instant::now();
    // SAFETY: on the main thread, inside `Thread.Sleep` of the server loop: the generated frames below (Main.DedServ
    // and its callers) and the suspended fibers are scanned conservatively (`suspended_stacks`).
    unsafe {
        if rt::gc_phase() == rt::GcPhase::Idle {
            let due = GC.with(|g| {
                let mut g = g.borrow_mut();
                g.sleeps_since_cycle += 1;
                g.sleeps_since_cycle >= 20_000
            });
            if rt::allocated_since_collect() >= mb << 20 || due {
                GC.with(|g| g.borrow_mut().sleeps_since_cycle = 0);
                rt::gc_begin();
            } else {
                return;
            }
        } else if let Some(st) = rt::gc_step(rt::GcBudget::Micros(slice), true).finished {
            GC.with(|g| {
                let mut g = g.borrow_mut();
                g.cycles += 1;
                g.freed_objects += st.freed_objects;
                g.freed_bytes += st.freed_bytes;
            });
        }
    }
    let ms = t0.elapsed().as_secs_f32() * 1000.0;
    GC.with(|g| {
        let mut g = g.borrow_mut();
        g.max_pause_ms = g.max_pause_ms.max(ms);
        if ms > 4.0 {
            g.pauses_over_4ms += 1;
        }
    });
}

/// Runs the dedicated server with the game's launch parameters `args` (without `-server`). Returns the exit code.
pub fn run(args: &[String]) -> i32 {
    let project = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let project = std::env::var_os("TERRARIA_PROJECT_DIR").map(std::path::PathBuf::from).unwrap_or(project);
    launch::install_resources(&project);
    // as the client: files the original writes by relative path land in the save root
    let save_root = terraria_host::sys::io::save_root();
    let save_root = std::path::absolute(&save_root).unwrap_or(save_root);
    std::env::set_var("TERRARIA_SAVE_DIR", &save_root);
    if let Err(e) = std::fs::create_dir_all(&save_root).and_then(|_| std::env::set_current_dir(&save_root)) {
        eprintln!("[server] cannot use the save root {} as the working directory: {e}", save_root.display());
    }
    eprintln!("[server] terraria -server {}", args.join(" "));
    eprintln!("[server] pid {}; save root {}", std::process::id(), save_root.display());
    rt::enter();
    // the host's Game state without a window (the client's shell passes the real one): a server never draws
    terraria_host::game::init(terraria_host::game::WindowState { width: 800, height: 600, x: 0, y: 0, active: true, handle: 0, display_width: 1920, display_height: 1080, scale: 1.0 });
    if !std::env::var_os("TERRARIA_GC_FIBER_SCAN").is_some_and(|v| v == "0") {
        rt::set_conservative_roots(Some(suspended_stacks));
    }
    threading::set_blocking_main(idle);
    let argv: Vec<&str> = args.iter().map(String::as_str).collect();
    let t0 = std::time::Instant::now();
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        rt::try_(|| unsafe {
            // Program.SavePath first (Program.cs:187; `launch_game` sets it again, the same way): the static
            // initialiser of Main, which the next line runs, builds its paths from it (Main.WorldPath ...)
            (*terraria::program::s()).save_path = match argv.iter().position(|a| a.eq_ignore_ascii_case("-savedirectory")).and_then(|i| argv.get(i + 1)) {
                Some(dir) => rt::Str::new(dir),
                None => (*terraria_host::os::platform_get::<rt::Gc<terraria_host::os::PathService>>().ptr()).get_storage_path(rt::Str::lit("Terraria")),
            };
            // the server build's value of the field (the client build's is false); what `Main.DedServ` reads before
            // it sets it itself: `new Main()` and `LaunchInitializer.LoadParameters` branch on it
            (*terraria::main::s()).ded_serv = true;
            (*terraria::main::s()).show_splash = false;
            let main = launch::launch_game(&argv);
            let _root = Rooted::new(main);
            terraria::main::ded_serv(main.ptr());
        })
    }));
    let code = match outcome {
        Ok(Ok(())) => 0,
        Ok(Err(e)) => {
            eprintln!("[server] unhandled C# exception: {}", rt::exc_text(e));
            1
        }
        Err(p) => {
            eprintln!("[server] stopped: {}", panic_text(&p));
            1
        }
    };
    let (objects, bytes) = rt::heap_size();
    GC.with(|g| {
        let g = g.borrow();
        eprintln!(
            "[server] ran {:.1} s; heap at exit {objects} objects, {:.1} MB; collector: {} cycles, {} objects / {:.1} MB freed, longest slice {:.2} ms, {} above 4 ms",
            t0.elapsed().as_secs_f64(),
            bytes as f64 / 1048576.0,
            g.cycles,
            g.freed_objects,
            g.freed_bytes as f64 / 1048576.0,
            g.max_pause_ms,
            g.pauses_over_4ms
        );
    });
    for line in rt::gc_profile_report() {
        eprintln!("[server] gc profile: {line}");
    }
    if std::env::var_os("TERRARIA_HOST_REPORT").is_some_and(|v| !v.is_empty() && v != "0") {
        eprint!("[server] {}", launch::reached_report().replace('\n', "\n[server] "));
        eprintln!();
    }
    let problems = terraria_gen_game::rt::host::unimplemented_reached().into_iter().filter(|(m, _)| !m.contains("..ctor")).count() + rt::gaps().len();
    if problems > 0 {
        eprintln!("[server] {problems} unimplemented host member(s) / gap(s) were reached");
    }
    code
}
