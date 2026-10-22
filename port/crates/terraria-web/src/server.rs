//! The browser build's dedicated server: what `terraria.exe -server` is natively (terraria-app/src/server.rs), run by
//! a second Worker of the page (web/www/server-worker.js) on its own instance of this module. "Host & Play" of the
//! game starts it (`Main.HostAndPlay` -> `Process.Start("TerrariaServer.exe")`, terraria-host sys/misc.rs: the page's
//! `tw.server_start`), and the hosting client of the same page joins it.
//!
//! Exports (HOST_ABI.md "Server"):
//! - `tw_server_init() -> i32`: the game's launch parameters come as ONE string in the WASI arguments (`argv[1]`, the
//!   page passes Host & Play's `-autoshutdown -password ".." -lang n -world "/saves/Worlds/x.wld" ...` as the game
//!   wrote it; [`split_args`]), then what server.rs does up to the main loop: `Main.dedServ = true`,
//!   `Program.LaunchGame` / `RunGame` (`launch::launch_game`). 0: started.
//! - `tw_server_tick(now_ms) -> f64`: one poll of `Main.DedServ`'s resumable form (tools/cs2rs DESIGN.md 8f) on the
//!   main stack, up to its next `Thread.Sleep(ms)`: returns `ms`, after which the Worker calls again (the page runs
//!   meanwhile: connections arrive, bytes move). The world load and anything else inside one poll give the page its
//!   turn at the main stack's yield points (`main_yield`, `tw.yield_frame`). A negative result: the server ended
//!   (`-1 - exit code`).
//!
//! Differences to the native server, each for a reason:
//! - The main loop is a state machine parked at every `Thread.Sleep` (`threading::poll_main_machine`) instead of a
//!   blocking loop: a Worker cannot block, and between two polls no game frame is on any stack, so the collector's
//!   mark phases can end (a suspended JSPI stack cannot be scanned, WEB_PORT.md 2.2). The server's threads
//!   (`Netplay.ServerLoop`, `BroadcastThread`, `TcpSocket.ListenLoop`) are resumable too (`gen::resumable`).
//! - The collector runs between polls (server.rs runs it inside the sleeps): a cycle after 48 MB of allocation or
//!   3,600 polls (about a minute; the client driver's 3,600 ticks: a hosting server allocates about 0.1 MB/s, so
//!   server.rs's 20,000 sleeps would leave its garbage for minutes), 1.5 ms slices, a mark phase ends only while no
//!   work is suspended mid-poll.
//! - The listener is the page's (terraria-host sys/net.rs, "The browser build"); the LAN announcement sends nothing.
//! - Console input: the page's console box is the server's stdin (WASI fd 0, HOST_ABI.md "Server console"), its
//!   stdout the box's output. The original starts its input thread (`Main.startDedInput`: "Server Input Thread",
//!   `startDedInputCallBack`, the `help` / `say` / `save` / `exit` commands) only without `-autoshutdown`, and Host &
//!   Play always passes it (the original's server window then takes no commands). Here the page has the console, so
//!   the driver starts that same translated thread when DedServ reaches its main loop (`Main.gameMenu` false) with
//!   `-autoshutdown` given; without it DedServ starts it itself as in the original. The thread runs its resumable
//!   form (tools/cs2rs game.json): parked while no line was typed (`Console.ReadLine` awaited), so it keeps no stack.

use std::cell::RefCell;
use std::future::Future;
use std::pin::Pin;

use terraria_gen_game::gen::terraria;
use terraria_gen_game::host::launch;
use terraria_gen_game::rt::{self, Gc, Rooted};
use terraria_host::sys::threading;

use crate::{log, mb};

struct Server {
    _main: Rooted<Gc<terraria::main::Main>>,
    machine: Option<Pin<Box<dyn Future<Output = ()>>>>,
    machine_root: u64,
    t0: std::time::Instant,
    polls: u64,
    sleeps_since_cycle: u64,
    cycles: u64,
    freed_bytes: usize,
    max_pause_ms: f32,
    last_report: std::time::Instant,
    exit: Option<i32>,
    seen: Vec<String>,
    /// the console's input thread was started (see the module docs, "Console input")
    console_started: bool,
}

thread_local! {
    static SERVER: RefCell<Option<Server>> = const { RefCell::new(None) };
}

/// The launch parameters as the server's `argv`: blanks separate, double quotes group (`-password ""` is an empty
/// argument), `\"` is a quote (the game's own quoting: Main.ConvertToSafeArgument / SanitizePathArgument).
pub fn split_args(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut any = false;
    let mut quoted = false;
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\\' if chars.peek() == Some(&'"') => {
                cur.push('"');
                chars.next();
                any = true;
            }
            '"' => {
                quoted = !quoted;
                any = true;
            }
            c if c.is_whitespace() && !quoted => {
                if any {
                    out.push(std::mem::take(&mut cur));
                    any = false;
                }
            }
            c => {
                cur.push(c);
                any = true;
            }
        }
    }
    if any {
        out.push(cur);
    }
    out
}

/// The main stack's yield hook of the server Worker: it has no sound and no display, it only gives the page its turn
/// (connections, bytes) and continues at the page's next tick.
#[cfg(target_family = "wasm")]
fn server_yield_hook() {
    // SAFETY: a suspending import with no arguments; called on the main stack inside tw_server_init / tw_server_tick
    // only (the window is open only there), i.e. under the promising exports.
    unsafe {
        crate::tw_yield_frame();
    }
}

fn poll_text(p: &Box<dyn std::any::Any + Send>) -> String {
    p.downcast_ref::<String>().cloned().or_else(|| p.downcast_ref::<&str>().map(|s| (*s).to_owned())).unwrap_or_else(|| "<non-string panic payload>".to_owned())
}

/// `tw_server_init()`: 0 started; 1 the install has no readable Terraria.exe; 2 the game could not be created; 3
/// called twice (or in a module that runs the client).
#[no_mangle]
pub extern "C" fn tw_server_init() -> i32 {
    if SERVER.with(|s| s.borrow().is_some()) || crate::client_started() {
        return 3;
    }
    // the launch parameters: the WASI arguments after the program name, as one string or already split
    let text = std::env::args().skip(1).collect::<Vec<_>>().join(" ");
    let argv = split_args(&text);
    log(2, &format!("[server] terraria -server {text}"));
    rt::enter();
    if let Err(code) = crate::install_resources_from_install() {
        return code;
    }
    // the host's Game state without a window (as server.rs): a server never draws
    terraria_host::game::init(terraria_host::game::WindowState { width: 800, height: 600, x: 0, y: 0, active: true, handle: 0, display_width: 1920, display_height: 1080, scale: 1.0 });
    crate::install_platform();
    #[cfg(target_family = "wasm")]
    terraria_host::sys::threading::main_yield::install(Some(server_yield_hook), crate::MAIN_SLICE_MS);
    let _ = std::fs::create_dir_all("/saves");
    let _ = std::env::set_current_dir("/saves");
    let argv_ref: Vec<&str> = argv.iter().map(String::as_str).collect();
    let mut seen = Vec::new();
    let mut main = Gc::NULL;
    crate::main_window(true);
    crate::guarded(&mut seen, "Program.LaunchGame (server)", || unsafe {
        // server.rs: Program.SavePath first (Main's static initialiser builds its paths from it), the server build's
        // Main.dedServ, no splash
        (*terraria::program::s()).save_path = match argv_ref.iter().position(|a| a.eq_ignore_ascii_case("-savedirectory")).and_then(|i| argv_ref.get(i + 1)) {
            Some(dir) => rt::Str::new(dir),
            None => (*terraria_host::os::platform_get::<rt::Gc<terraria_host::os::PathService>>().ptr()).get_storage_path(rt::Str::lit("Terraria")),
        };
        (*terraria::main::s()).ded_serv = true;
        (*terraria::main::s()).show_splash = false;
        main = launch::launch_game(&argv_ref);
    });
    crate::main_window(false);
    if main.is_null() {
        return 2;
    }
    let root = Rooted::new(main);
    // SAFETY: `main` is rooted for the life of the machine (`_main`).
    let machine: Pin<Box<dyn Future<Output = ()>>> = Box::pin(async move { unsafe { terraria::main::ded_serv__resumable(main.ptr()).await } });
    let p: *const dyn Future<Output = ()> = &*machine;
    // between polls the machine holds every value of DedServ's frame (its locals: the Stopwatch, the load task):
    // one heap block, scanned conservatively like a parked thread's machine (threading.rs "Resumable threads")
    let machine_root = rt::add_root(move |m| unsafe { m.mark_conservative(p as *const u8, core::mem::size_of_val(&*p)) });
    let now = std::time::Instant::now();
    SERVER.with(|s| *s.borrow_mut() = Some(Server { _main: root, machine: Some(machine), machine_root, t0: now, polls: 0, sleeps_since_cycle: 0, cycles: 0, freed_bytes: 0, max_pause_ms: 0.0, last_report: now, exit: None, seen, console_started: false }));
    log(2, &format!("[server] started: allocated {:.1} MB live", mb(crate::live_bytes())));
    0
}

/// `tw_server_tick(now_ms)`: milliseconds until the next call, or `-1 - exit code` once the server has ended.
#[no_mangle]
pub extern "C" fn tw_server_tick(_now_ms: f64) -> f64 {
    let taken = SERVER.with(|s| s.borrow_mut().as_mut().map(|sv| (sv.machine.take(), sv.exit)));
    let Some((machine, exit)) = taken else { return -3.0 };
    if let Some(code) = exit {
        return -1.0 - code as f64;
    }
    let Some(mut machine) = machine else { return -3.0 };
    // one poll on the main stack; the machine is out of the server state meanwhile (the poll may run for long and the
    // yield hook may hand the page its turn in the middle)
    crate::main_window(true);
    let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| rt::try_(|| threading::poll_main_machine(machine.as_mut()))));
    crate::main_window(false);
    let r = SERVER.with(|s| {
        let mut b = s.borrow_mut();
        let sv = b.as_mut().expect("the server state");
        sv.polls += 1;
        let wait = match r {
            Ok(Ok(Some(ms))) => {
                sv.machine = Some(machine);
                Some(ms)
            }
            Ok(Ok(None)) => {
                sv.exit = Some(0);
                None
            }
            Ok(Err(e)) => {
                let text = format!("[server] unhandled C# exception: {}", rt::exc_text(e));
                log(4, &text);
                sv.seen.push(text);
                sv.exit = Some(1);
                None
            }
            Err(p) => {
                log(4, &format!("[server] stopped: {}", poll_text(&p)));
                sv.exit = Some(1);
                None
            }
        };
        if wait.is_none() {
            // the machine (and with it DedServ's frame) is gone: its root goes too
            drop(machine_drop_guard(sv));
        }
        collect(sv);
        report(sv, wait.is_none());
        match (wait, sv.exit) {
            (Some(ms), _) => ms as f64,
            (None, Some(code)) => -1.0 - code as f64,
            (None, None) => -1.0,
        }
    });
    if r >= 0.0 {
        start_console_input_if_due();
    }
    r
}

/// The console's input thread (module docs, "Console input"): `Main.startDedInput()` once DedServ runs its main loop,
/// when it did not start it itself (`-autoshutdown`). Between polls, on the main stack: it only creates and starts the
/// `Thread` (its work is queued; the next pump of DedServ's sleep runs it).
fn start_console_input_if_due() {
    let due = SERVER.with(|s| s.borrow().as_ref().is_some_and(|sv| !sv.console_started && sv.exit.is_none() && sv.machine.is_some()))
        // SAFETY: the game's statics, between polls.
        && unsafe { !(*terraria::main::s()).game_menu && (*terraria::main::s()).auto_shutdown && !(*terraria::netplay::s()).disconnect };
    if due {
        SERVER.with(|s| s.borrow_mut().as_mut().expect("the server state").console_started = true);
        // SAFETY: between polls on the main stack (as the collector's slice): the game code only queues the thread.
        match rt::try_(|| unsafe { terraria::main::start_ded_input() }) {
            Ok(()) => log(2, "[server] console: the server's input thread runs (commands typed into the page's console box: help, say, save, exit ...)"),
            Err(e) => log(4, &format!("[server] console: Main.startDedInput threw {}", rt::exc_text(e))),
        }
    }
}

/// Removes the machine's root (the machine itself was dropped by `tw_server_tick`).
fn machine_drop_guard(sv: &mut Server) -> u64 {
    let id = core::mem::replace(&mut sv.machine_root, 0);
    if id != 0 {
        rt::remove_root(id);
    }
    id
}

/// server.rs `idle`, between polls: one collector slice.
fn collect(sv: &mut Server) {
    let t0 = std::time::Instant::now();
    // SAFETY: between polls on the main stack: no generated function is on it (the machine is parked). Work suspended
    // mid-poll (a fiber in a host wait, a yield point) holds handles no one can scan: no mark phase ends meanwhile.
    unsafe {
        if rt::gc_phase() == rt::GcPhase::Idle {
            sv.sleeps_since_cycle += 1;
            if rt::allocated_since_collect() >= 48 << 20 || sv.sleeps_since_cycle >= 3_600 {
                sv.sleeps_since_cycle = 0;
                rt::gc_begin();
            }
        } else if let Some(st) = rt::gc_step(rt::GcBudget::Micros(1500), !threading::has_suspended_work()).finished {
            sv.cycles += 1;
            sv.freed_bytes += st.freed_bytes;
            log(2, &format!("[server] collector cycle {} finished: {} objects ({:.1} MB) live, {} objects ({:.1} MB) freed; allocated {:.1} MB live", rt::collection_count(), st.live_objects, mb(st.live_bytes), st.freed_objects, mb(st.freed_bytes), mb(crate::live_bytes())));
        }
    }
    sv.max_pause_ms = sv.max_pause_ms.max(t0.elapsed().as_secs_f32() * 1000.0);
}

/// Every 10 s, and at the end: memory and progress (the evidence that a hosted session's memory stays flat).
fn report(sv: &mut Server, last: bool) {
    if !last && sv.last_report.elapsed() < std::time::Duration::from_secs(10) {
        return;
    }
    sv.last_report = std::time::Instant::now();
    let (objects, bytes) = rt::heap_size();
    // SAFETY: the game's statics, between polls.
    let (updates, players) = unsafe {
        let arr = (*terraria::main::s()).player;
        let players = if arr.is_null() { 0 } else { arr.as_slice().iter().filter(|q| !q.is_null() && (*q.ptr()).active).count() };
        ((*terraria::main::s())._game_update_count, players)
    };
    log(
        2,
        &format!(
            "[server] {:.0} s: {} polls, game update count {updates}, {players} player(s) active; allocated {:.1} MB live, {:.1} MB peak; collector heap {objects} objects, {:.1} MB; {} cycles ({:.1} MB freed), phase {:?}, longest slice {:.2} ms; work suspended {}",
            sv.t0.elapsed().as_secs_f64(),
            sv.polls,
            mb(crate::live_bytes()),
            mb(crate::peak_bytes()),
            mb(bytes),
            sv.cycles,
            mb(sv.freed_bytes),
            rt::gc_phase(),
            sv.max_pause_ms,
            threading::has_suspended_work()
        ),
    );
    if last {
        let problems = terraria_gen_game::rt::host::unimplemented_reached().into_iter().filter(|(m, _)| !m.contains("..ctor")).count() + rt::gaps().len();
        log(2, &format!("[server] ended (exit code {}) after {:.1} s; {problems} unimplemented host member(s) / gap(s) reached", sv.exit.unwrap_or(0), sv.t0.elapsed().as_secs_f64()));
    }
}

#[cfg(test)]
mod tests {
    use super::split_args;

    #[test]
    fn host_and_play_arguments_are_split_as_the_server_receives_them() {
        let a = split_args(r#"-autoshutdown -password "" -lang 1 -world "/saves/Worlds/My World.wld" -worldrollbackstokeep 2 -hosttoken abc+/="#);
        assert_eq!(a, vec!["-autoshutdown", "-password", "", "-lang", "1", "-world", "/saves/Worlds/My World.wld", "-worldrollbackstokeep", "2", "-hosttoken", "abc+/="]);
        assert_eq!(split_args(r#"-password "a\"b""#), vec!["-password", "a\"b"]);
        assert!(split_args("   ").is_empty());
    }
}
