//! The Bevy shell for `terraria-host`: runs a [`HostedGame`] the way XNA's `Game.Run` runs a `Game` subclass, with
//! Bevy supplying the window, input, GPU and audio device.
//!
//! A binary includes this module with `#[path = "../shell/mod.rs"] mod shell;` (the crate has no library target, so
//! the module pulls the renderer and the Bevy adapters in itself) and calls [`run`], or adds [`ShellPlugin`] to its
//! own `App` after `DefaultPlugins` and [`render::GfxPlugin`].
//!
//! # Threading
//! The host's objects (`Gc<T>`, the thread-local `game::HostState`, the translated game's statics) are bound to ONE
//! thread. Bevy runs systems on a thread pool, but a system that takes a `NonSend` / `NonSendMut` resource is pinned to
//! the thread that owns the `World` (the thread that called `App::run`). The game, its state and the audio handle live
//! in one `NonSend` resource ([`ShellState`]) and the whole frame protocol is ONE system that takes it, so every call
//! into the game (`initialize`, `load_content`, `update`, `draw`, `unload`, the host driver functions, the readback hook
//! that `GetData` runs in the middle of `draw`) happens on that thread, in a fixed order. An exclusive system would pin
//! the same way but would also forbid the other system parameters (input readers, queries) that the frame needs.
//!
//! # Frame sequence (one Bevy `Update` = one XNA `Game.Tick`)
//! First tick only, `Game.RunGame` (reference/xna/Game.cs:247-259):
//! `game::init` (`CreateDevice`) -> [`HostedGame::initialize`] (`Initialize`; `LoadContent` runs at its end, Game.cs:503-514,
//! see [`HostedGame`]) -> [`HostedGame::begin_run`] (`BeginRun`) -> one `Update` with `ElapsedGameTime` zero (Game.cs:260-263).
//!
//! Every tick, `Game.Tick` (Game.cs:312-410) and `DrawFrame` (Game.cs:526-545):
//! 1. gather input / window state (keyboard, mouse, wheel, text, game pads, size, focus, monitor) -> `InputSnapshot`, `WindowState`;
//!    an inactive window sleeps `InactiveSleepTime` first (Game.cs:318-321; not in a hidden run);
//! 2. `game::begin_frame` (clock step, 500 ms clamp, fixed-step accumulator and catch-up, `IsRunningSlowly`; a changed
//!    window size resets the back buffer -> `ClientSizeChanged`; focus change -> `Activated` / `Deactivated`);
//! 3. `game::dispatch_events` (the queued events, typed characters to the IME listeners);
//! 4. `while let Some(t) = begin_update() { update(t); end_update() }` (the catch-up loop, Game.cs:354-371; `end_update`
//!    adds `SuppressDraw` bookkeeping, game time and the queued "thread" work);
//! 5. `DrawFrame`: skipped when no update ran, every update suppressed the draw, the game exited, or the window is
//!    minimised (`Window.IsMinimized`); otherwise `begin_draw` (`BeginDraw`) -> [`HostedGame::draw`] ->
//!    [`HostedGame::end_draw`] (the game's `EndDraw` override: Terraria paces its frames there; not in a fixed-step
//!    run) -> `end_draw` (the base `EndDraw` = `Present`);
//! 6. the `FrameOutput`: frame -> [`render::PublishedFrames`] (replayed by the render world), audio commands -> mixer,
//!    title, mouse visibility, `WindowRequest` (`ApplyChanges`), `WindowCommand`s (`System.Windows.Forms` calls), exit;
//! 7. the collector: a cycle starts once `gc_bytes` were allocated or `gc_interval` ticks passed; every tick after
//!    that gives it one slice of `gc_slice_us` (`rt::gc_step`) until the cycle is done. A mark phase does not end
//!    while queued work is suspended
//!    (between frames, as the host requires: handles the game keeps across
//!    frames must be rooted).
//!
//! Shutdown (Game.cs `RunGame` after `host.Run`, `Dispose`): [`HostedGame::end_run`] (`EndRun`) -> `game::raise_exiting`
//! (`Exiting`) -> [`HostedGame::unload`] (`UnloadContent`, run by `Dispose` through `DeviceDisposing`, Game.cs:726-745,765-790).
//! It runs when the game calls `Game.Exit`, when the window is closed, and for a scripted `quit`.
//!
//! # Environment
//! `TERRARIA_HIDDEN=1` (no visible window, focus assumed, no fullscreen / decoration changes), `TERRARIA_FIXED_DT=1` (every
//! tick is exactly `TargetElapsedTime`: deterministic, faster than real time), `TERRARIA_INPUT_SCRIPT=<file>` (see
//! [`script`]; implies a fixed step), `TERRARIA_SCREENSHOT=<png>` + `TERRARIA_SCREENSHOT_FRAME=<n>` (window screenshot at
//! Bevy frame `n`, exit 30 frames later), `TERRARIA_MUTE=1` / `TERRARIA_VOLUME=<0..1>` (mixer, see `host/audio.rs`).
//!
//! A visible window that never takes the focus or a monitor (test runs driven by window messages):
//! `TERRARIA_NOACTIVATE=1` (shown without activation, no task bar button, never asks for the foreground, fullscreen /
//! maximise / minimise / borderless requests of the game are logged and not applied, message boxes are answered
//! by the host instead of shown), `TERRARIA_WINDOW_POS=<x>,<y>` (position of the window, physical pixels; may be
//! outside every monitor), `TERRARIA_FRAME_LOG=<file.tsv>` (one line per shell tick: time, interval, updates run,
//! drawn, active, minimised, client size, milliseconds in the game's `EndDraw`, milliseconds of the whole tick), `TERRARIA_COMMAND_FILE=<file>` (the shell reads `dump ...` / `aim ...`
//! lines from this file when it appears, runs them through [`HostedGame::command`] and deletes it: reads of game
//! state for a test driver; nothing is injected, `aim` does not move anything here).
//! `TERRARIA_NO_GAMEPAD=1`: connected game pads are not reported to the game (keyboard and mouse only).
//! `TERRARIA_FAKE_MONITOR=<x>,<y>,<w>,<h> | auto` (with `TERRARIA_NOACTIVATE`): the game is told that this rectangle
//! of the virtual screen is its only monitor and its window requests (borderless, maximise, fullscreen, moves,
//! sizes) are applied for real inside it (`window.rs`, `Apply::Fake`). The rectangle must lie outside the real
//! desktop; `auto` is 1920x1080 below and right of it.
//! `TERRARIA_COARSE_TIMER=1`: no `timeBeginPeriod(1)` (a measuring aid).
//!
//! # What comes straight from the window system (`native.rs`)
//! A run with a real window and no script puts a message hook on its own window: typed text is every `WM_CHAR`,
//! keys are the virtual keys of the key messages, `Game.IsActive` follows `WM_ACTIVATEAPP` (XNA's rule), and a
//! normal run reads the pointer with `GetCursorPos` while its window is in front (`native::mouse_source`). Bevy's
//! events are the source where there is no hook (`host/input.rs`).
//! `TERRARIA_PRESENT_MODE=vsync | novsync`: the present mode whatever the game asks for (a measuring aid).

#![allow(dead_code, unused_imports)] // each binary uses part of the surface

#[path = "../host/audio.rs"]
pub mod host_audio;
#[path = "../host/input.rs"]
pub mod host_input;
#[path = "../gfx/render.rs"]
pub mod render;
pub mod native;
pub mod pad;
pub mod script;
pub mod window;

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

use bevy::app::AppExit;
use bevy::ecs::system::SystemParam;
use bevy::input::gamepad::{GamepadRumbleRequest, RawGamepadEvent};
use bevy::prelude::*;
use bevy::render::renderer::{RenderDevice, RenderQueue};
use bevy::render::view::screenshot::{save_to_disk, Screenshot};
use bevy::window::{CursorOptions, PrimaryWindow, RawHandleWrapper, WindowResolution};
use terraria_gen_rt::rt;
use terraria_host::game::{self, GameTime, WindowState};
use terraria_host::input::InputSnapshot;
use terraria_host::winforms;

use host_audio::{AudioBackend, AudioStats};
use host_input::HostInputState;
use render::{BackBufferImage, GfxPlugin, PublishedFrames, SharedGpu, WgpuReadback};

pub use terraria_gen_rt::rt::Gc;

/// What XNA's `Game.Run` calls on a `Game` subclass. The shell is the base class: it owns the clock, the window and
/// the loop; the implementor is `Main`.
///
/// `Game.Initialize`'s base body ends with `LoadContent()` (Game.cs:503-514). The shell does that: it calls
/// [`load_content`](Self::load_content) once, right after [`initialize`](Self::initialize) returns, which is what an
/// override that calls `base.Initialize()` last gets. (The translated `Main.Initialize` calls `base.Initialize()` in the
/// middle: the mapped `base.Initialize()` is `game::base_noop()`, so `LoadContent` then runs after the rest of
/// `Initialize`. README.md lists this as a difference to settle when `Main` is connected.)
///
/// Handles to host objects that a game keeps across frames must be rooted (`rt::Rooted`): the shell collects garbage
/// between frames.
pub trait HostedGame {
    /// `Game.Initialize` override.
    fn initialize(&mut self);
    /// `Game.LoadContent` override.
    fn load_content(&mut self);
    /// `Game.BeginRun` override.
    fn begin_run(&mut self) {}
    /// `Game.Update(gameTime)` override.
    fn update(&mut self, game_time: Gc<GameTime>);
    /// `Game.Draw(gameTime)` override.
    fn draw(&mut self, game_time: Gc<GameTime>);
    /// `Game.EndDraw` override, called after [`draw`](Self::draw) of every frame that was drawn (`DrawFrame`,
    /// Game.cs:526-545: `if (BeginDraw()) { Draw(gameTime); EndDraw(); }`); the base `EndDraw` (`Present`) is the
    /// shell's. Not called in a fixed-step run (`TERRARIA_FIXED_DT`, scripts): Terraria's override is the game's
    /// own frame pacing, a wait on the real clock until 1/60 s has passed since the last frame
    /// (Main.cs `EndDraw`, `FrameSkipMode.Subtle`), and a fixed-step run is meant to be faster than real time.
    /// Without it the game runs one `Update` per presented frame: as fast as the display refreshes.
    fn end_draw(&mut self) {}
    /// `Game.EndRun` override.
    fn end_run(&mut self) {}
    /// `Game.UnloadContent` override.
    fn unload(&mut self);
    /// Process exit code once the game has ended (tests report failure through it).
    fn exit_code(&self) -> u8 {
        0
    }
    /// A `dump` / `aim` / `game` / `set` line of the input script, before the `Update` of shell tick `tick`
    /// (`script.rs`). Returns where the scripted cursor has to go, if the command moves it.
    fn command(&mut self, tick: u64, cmd: &str) -> Option<(i32, i32)> {
        eprintln!("[shell] tick {tick}: this game has no script command {cmd:?}");
        None
    }
    /// A script's `wait <seconds> <condition>`: does the condition hold now? (default: never)
    fn condition(&mut self, _condition: &str) -> bool {
        false
    }
    /// A script could not do what it says (a `wait` timed out): the run has failed.
    fn script_failed(&mut self, _text: &str) {}
}

/// Settings of the shell; [`ShellConfig::from_env`] reads the environment variables listed in the module docs.
#[derive(Clone, Debug)]
pub struct ShellConfig {
    pub hidden: bool,
    pub fixed_dt: bool,
    pub script: Option<PathBuf>,
    pub screenshot: Option<PathBuf>,
    pub screenshot_frame: u32,
    /// A collector cycle starts at the latest every n ticks (0: never). `TERRARIA_GC_TICKS`, default 3600 (a minute
    /// of game time).
    pub gc_interval: u64,
    /// ... and earlier, as soon as this many bytes were allocated since the last cycle started. `TERRARIA_GC_MB`,
    /// default 48 (in play the game allocates about 140 KB per tick: a cycle every 6 seconds).
    pub gc_bytes: usize,
    /// Microseconds the collector may use per tick while a cycle runs (`rt::gc_step`). `TERRARIA_GC_SLICE_US`,
    /// default 1500. A whole cycle is about 0.1 s of work with a small world loaded (5.8 million objects), so it
    /// takes 60 to 100 ticks. When the game allocates faster than the collector works (more than twice `gc_bytes`
    /// since the cycle started), the slice is four times as long.
    pub gc_slice_us: u64,
    /// How queued work (world load, save, generation: the original's worker threads) gives the frame back so that
    /// the loading screen is drawn while it runs (`terraria_host::sys::threading::set_yield_mode`).
    /// `TERRARIA_LOAD_SLICE=<n>ms | <n>loops | off`. Default: 8 ms of work per tick; with a fixed step (scripted
    /// runs) one million loop iterations per tick, which is repeatable. While work is suspended a script's clock
    /// stands still (its `frame N` lines count ticks in which no work was suspended), so a script does not have
    /// to know how many ticks a load takes.
    pub yield_mode: terraria_host::sys::threading::YieldMode,
    /// `TERRARIA_GC_FULL=1`: the collector of the earlier stages, one whole collection in one tick
    /// (`rt::collect`), for comparison.
    pub gc_full: bool,
    /// `TERRARIA_GC_VERIFY=1`: `rt::set_verify` (the end of every mark phase is checked by a second, independent
    /// walk of the heap; slow).
    pub gc_verify: bool,
    /// Bevy frames to wait for the back-buffer image to reach the GPU before the game starts
    pub gpu_wait_frames: u32,
    /// `TERRARIA_DISPLAY=<W>x<H>`: the game is told about ONE monitor of this size instead of the machine's. A
    /// scripted run defaults to 1920x1080, so that what the game derives from the display (its default UI scale,
    /// `Main.TryPickingDefaultUIScale`) is the same on every machine.
    pub display: Option<(i32, i32)>,
    /// The working directory at start-up: relative screenshot paths (`TERRARIA_SCREENSHOT`, a script's
    /// `shot <file>`) are relative to it, also when the game changes the working directory later.
    pub launch_dir: PathBuf,
    /// `TERRARIA_NOACTIVATE=1`: a visible window that is created without activation and never asks for the
    /// foreground (module docs, `window.rs`).
    pub noactivate: bool,
    /// `TERRARIA_WINDOW_POS=<x>,<y>`: where the window is created (physical pixels of the virtual screen).
    pub window_pos: Option<(i32, i32)>,
    /// `TERRARIA_FRAME_LOG=<file.tsv>`: one line per shell tick (module docs).
    pub frame_log: Option<PathBuf>,
    /// `TERRARIA_COMMAND_FILE=<file>`: `dump` / `aim` lines for the game adapter, polled (module docs).
    pub command_file: Option<PathBuf>,
    /// `TERRARIA_NO_GAMEPAD=1`: connected game pads are not reported to the game.
    pub no_gamepad: bool,
    /// `TERRARIA_PRESENT_MODE=vsync | novsync`: the present mode whatever the game asks for
    /// (`GraphicsDeviceManager.SynchronizeWithVerticalRetrace`); a measuring aid.
    pub present_override: Option<bevy::window::PresentMode>,
    /// `TERRARIA_FAKE_MONITOR=<x>,<y>,<w>,<h> | auto`: the one monitor the game is told about, outside the real
    /// desktop; window requests are applied inside it (`window::Apply::Fake`). Needs `TERRARIA_NOACTIVATE`.
    pub fake_monitor: Option<window::Rect4>,
}

impl ShellConfig {
    /// What kind of run this is for the real pointer and the real focus (`native::RunKind`).
    pub fn run_kind(&self) -> native::RunKind {
        if self.hidden || self.noactivate || self.script.is_some() {
            native::RunKind::Test
        } else {
            native::RunKind::Normal
        }
    }
    /// How window requests of the game are carried out.
    pub fn apply_mode(&self) -> window::Apply {
        if self.hidden {
            window::Apply::Hidden
        } else if let Some(monitor) = self.fake_monitor {
            window::Apply::Fake { monitor, desktop: native::virtual_screen() }
        } else if self.noactivate {
            window::Apply::Passive
        } else {
            window::Apply::Real
        }
    }

    pub fn from_env() -> ShellConfig {
        let on = |k: &str| std::env::var_os(k).is_some_and(|v| !v.is_empty() && v != "0");
        let script = std::env::var_os("TERRARIA_INPUT_SCRIPT").filter(|v| !v.is_empty()).map(PathBuf::from);
        let has_script = script.is_some();
        ShellConfig {
            hidden: std::env::var_os("TERRARIA_HIDDEN").is_some(),
            fixed_dt: on("TERRARIA_FIXED_DT") || script.is_some(),
            script,
            screenshot: std::env::var_os("TERRARIA_SCREENSHOT").map(PathBuf::from),
            screenshot_frame: std::env::var("TERRARIA_SCREENSHOT_FRAME").ok().and_then(|v| v.parse().ok()).unwrap_or(120),
            gc_interval: std::env::var("TERRARIA_GC_TICKS").ok().and_then(|v| v.parse().ok()).unwrap_or(3600),
            gc_bytes: std::env::var("TERRARIA_GC_MB").ok().and_then(|v| v.parse::<usize>().ok()).unwrap_or(48) << 20,
            yield_mode: {
                use terraria_host::sys::threading::YieldMode;
                let fixed = on("TERRARIA_FIXED_DT") || has_script;
                match std::env::var("TERRARIA_LOAD_SLICE").ok().as_deref().map(str::trim) {
                    None | Some("") => {
                        if fixed {
                            YieldMode::Loops(1_000_000)
                        } else {
                            YieldMode::Millis(8)
                        }
                    }
                    Some("off") | Some("0") => YieldMode::Off,
                    Some(v) => {
                        let parsed = v.strip_suffix("ms").map(|n| n.trim().parse().map(YieldMode::Millis)).or_else(|| v.strip_suffix("loops").map(|n| n.trim().parse().map(YieldMode::Loops)));
                        match parsed {
                            Some(Ok(m)) => m,
                            _ => {
                                eprintln!("TERRARIA_LOAD_SLICE={v}: expected <n>ms, <n>loops or off");
                                std::process::exit(2);
                            }
                        }
                    }
                }
            },
            gc_slice_us: std::env::var("TERRARIA_GC_SLICE_US").ok().and_then(|v| v.parse().ok()).unwrap_or(1500),
            gc_full: std::env::var_os("TERRARIA_GC_FULL").is_some_and(|v| !v.is_empty() && v != "0"),
            gc_verify: std::env::var_os("TERRARIA_GC_VERIFY").is_some_and(|v| !v.is_empty() && v != "0"),
            gpu_wait_frames: 600,
            launch_dir: std::env::current_dir().unwrap_or_default(),
            noactivate: on("TERRARIA_NOACTIVATE"),
            no_gamepad: on("TERRARIA_NO_GAMEPAD"),
            present_override: match std::env::var("TERRARIA_PRESENT_MODE").ok().as_deref() {
                Some("vsync") => Some(bevy::window::PresentMode::AutoVsync),
                Some("novsync") => Some(bevy::window::PresentMode::AutoNoVsync),
                None | Some("") => None,
                Some(v) => {
                    eprintln!("TERRARIA_PRESENT_MODE={v}: expected vsync or novsync");
                    std::process::exit(2);
                }
            },
            window_pos: match std::env::var("TERRARIA_WINDOW_POS") {
                Ok(v) if !v.is_empty() => {
                    let parsed = v.split_once(',').and_then(|(x, y)| Some((x.trim().parse().ok()?, y.trim().parse().ok()?)));
                    Some(parsed.unwrap_or_else(|| {
                        eprintln!("TERRARIA_WINDOW_POS={v}: expected <x>,<y>");
                        std::process::exit(2);
                    }))
                }
                _ => None,
            },
            fake_monitor: match std::env::var("TERRARIA_FAKE_MONITOR") {
                Ok(v) if !v.is_empty() => {
                    let fail = |why: String| -> ! {
                        eprintln!("TERRARIA_FAKE_MONITOR={v}: {why}");
                        std::process::exit(2);
                    };
                    if !on("TERRARIA_NOACTIVATE") || std::env::var_os("TERRARIA_HIDDEN").is_some() {
                        fail("only for a TERRARIA_NOACTIVATE window (a visible test window that is never activated)".to_owned());
                    }
                    let desktop = native::virtual_screen();
                    let rect = if v == "auto" {
                        window::fake_monitor_for(desktop, (1920, 1080))
                    } else {
                        let n: Vec<i32> = v.split(',').filter_map(|t| t.trim().parse().ok()).collect();
                        match n[..] {
                            [x, y, w, h] => (x, y, w, h),
                            _ => fail("expected <x>,<y>,<w>,<h> or auto".to_owned()),
                        }
                    };
                    if let Err(why) = window::check_fake_monitor(rect, desktop) {
                        fail(why);
                    }
                    Some(rect)
                }
                _ => None,
            },
            frame_log: std::env::var_os("TERRARIA_FRAME_LOG").filter(|v| !v.is_empty()).map(PathBuf::from),
            command_file: std::env::var_os("TERRARIA_COMMAND_FILE").filter(|v| !v.is_empty()).map(PathBuf::from),
            display: match std::env::var("TERRARIA_DISPLAY") {
                Ok(v) if !v.is_empty() => {
                    let (w, h) = v.split_once(['x', 'X']).unwrap_or_else(|| panic!("TERRARIA_DISPLAY: <W>x<H> expected, got {v:?}"));
                    Some((w.trim().parse().unwrap_or_else(|_| panic!("TERRARIA_DISPLAY: bad width {w:?}")), h.trim().parse().unwrap_or_else(|_| panic!("TERRARIA_DISPLAY: bad height {h:?}"))))
                }
                _ => has_script.then_some((1920, 1080)),
            },
        }
    }
}

thread_local! {
    static AUDIO_STATS: RefCell<Option<Arc<AudioStats>>> = const { RefCell::new(None) };
}

/// The mixer's counters (blocks written, peak, voices started), for a game or test that wants to show that sound was
/// produced. `None` before the shell started.
pub fn audio_stats() -> Option<Arc<AudioStats>> {
    AUDIO_STATS.with(|s| s.borrow().clone())
}

/// The game and everything else that must stay on the main thread.
pub struct ShellState<G: HostedGame> {
    make: fn() -> G,
    config: ShellConfig,
    game: Option<G>,
    audio: AudioBackend,
    script: Option<script::Script>,
    started: bool,
    finished: bool,
    tick: u64,
    waited: u32,
    /// the next tick runs with zero elapsed time (the clock restarts after start-up work: `ResetElapsedTime`)
    reset_clock: bool,
    shadow: window::WindowShadow,
    last_size: (i32, i32),
    monitors: Vec<winforms::Monitor>,
    modes: Vec<(i32, i32)>,
    last_title: String,
    exit_code: u8,
    /// milliseconds of every collector pause (a slice, the start of a cycle, or a whole collection with
    /// `gc_full`), and the heap right after each cycle (tick, objects, bytes)
    gc_ms: Vec<f32>,
    /// the pauses in which a mark phase ended (also in `gc_ms`), and per cycle: slices, objects traced again at the
    /// end of the mark phase, passes over the object list for conservative scans, objects freed
    gc_end_ms: Vec<f32>,
    gc_cycles: Vec<(u32, usize, usize, usize)>,
    gc_slices: u32,
    gc_verify_set: bool,
    /// the script's clock (see `ShellConfig::yield_mode`) and the ticks it stood still in a row / in total
    script_tick: u64,
    script_stalled: u32,
    script_stalled_total: u64,
    /// `wait` of the script: the condition and the moment it times out; the clock stands still meanwhile
    waiting: Option<(String, std::time::Instant)>,
    /// `loadshots` of the script: (file pattern, every, count), suspended ticks seen since armed, shots taken
    load_shots: Option<(String, u32, u32)>,
    load_shot_ticks: u32,
    load_shots_taken: u32,
    /// CPU cycles of the game thread in each pause (`QueryThreadCycleTime`), parallel to `gc_ms`: tells a pause the
    /// collector made from one in which the thread was not running (the machine was busy)
    gc_cycles_cpu: Vec<u64>,
    /// what each pause was, parallel to `gc_ms`: 0 the start of a cycle (roots), 1 a mark slice, 2 the slice that
    /// ended a mark phase, 3 a sweep slice, 4 a whole collection (`gc_full`)
    gc_kind: Vec<u8>,
    heap_log: Vec<(u64, usize, usize)>,
    last_gc_tick: u64,
    /// per collection: ticks since the previous one, bytes allocated in between
    gc_allocated: Vec<(u64, usize)>,
    /// milliseconds between shell ticks (Bevy frames) and the `Update` calls of each, for the pacing summary
    frame_ms: Vec<f32>,
    frame_updates: u64,
    frame_draws: u64,
    frame_seconds: f64,
    frame_log: Option<std::io::BufWriter<std::fs::File>>,
    run_start: Option<std::time::Instant>,
    /// the four game pad slots, fed from Bevy's raw gamepad events (`pad.rs`)
    pads: pad::PadAdapter,
    /// the window's native handle (0: not known yet / none) and whether the message hook is on it (`native.rs`)
    hwnd: isize,
    hook: bool,
    /// what the window's frame adds to its client size, seen last while it had one (`window::Apply::Fake`)
    frame: (i32, i32),
    /// `Mouse.SetPosition` that moved nothing real: the game's own cursor is there until the next mouse message
    pending_mouse: Option<(i32, i32)>,
    /// things said once (log lines that would otherwise repeat every tick)
    said: Vec<String>,
    /// the last state told to the log: (active, minimised, pointer followed)
    told: (Option<bool>, bool, bool),
}

impl<G: HostedGame> ShellState<G> {
    fn say_once(&mut self, text: String) {
        if !self.said.contains(&text) {
            eprintln!("[shell] {text}");
            self.said.push(text);
        }
    }

    /// Game.Exit / window closed / scripted quit: `EndRun`, `Exiting`, `UnloadContent`.
    fn finish(&mut self) {
        if self.finished || !self.started {
            self.finished = true;
            return;
        }
        self.finished = true;
        if let Some(w) = self.frame_log.as_mut() {
            use std::io::Write as _;
            let _ = w.flush();
        }
        if let Some(g) = self.game.as_mut() {
            g.end_run();
        }
        game::raise_exiting();
        if let Some(g) = self.game.as_mut() {
            g.unload();
        }
        self.exit_code = self.game.as_ref().map_or(0, |g| g.exit_code());
        if std::env::var_os("TERRARIA_FRAME_TIMES").is_some_and(|v| !v.is_empty() && v != "0") {
            let summary = |v: &mut Vec<f32>| -> String {
                if v.is_empty() {
                    return "no samples".to_owned();
                }
                v.sort_by(|a, b| a.total_cmp(b));
                let n = v.len();
                let at = |q: f64| v[((n as f64 - 1.0) * q) as usize];
                format!("{n} samples, mean {:.3} ms, p50 {:.3} ms, p99 {:.3} ms, max {:.3} ms", v.iter().sum::<f32>() / n as f32, at(0.5), at(0.99), v[n - 1])
            };
            let over = self.gc_ms.iter().filter(|x| **x > 4.0).count();
            // the longest pause of each kind (before the samples are sorted below)
            let by_kind: Vec<String> = ["cycle start", "mark slice", "end of a mark phase", "sweep slice", "whole collection"]
                .iter()
                .enumerate()
                .filter_map(|(k, name)| {
                    let v: Vec<f32> = self.gc_ms.iter().zip(&self.gc_kind).filter(|(_, kk)| **kk as usize == k).map(|(ms, _)| *ms).collect();
                    (!v.is_empty()).then(|| format!("{name}: {} pauses, max {:.3} ms, {} above 4 ms", v.len(), v.iter().fold(0f32, |a, b| a.max(*b)), v.iter().filter(|x| **x > 4.0).count()))
                })
                .collect();
            // CPU cycles per millisecond from the pauses themselves (the median ratio: most pauses are not
            // interrupted), then the pauses above 4 ms of wall time in CPU time
            let mut ratios: Vec<f64> = self.gc_ms.iter().zip(&self.gc_cycles_cpu).filter(|(ms, c)| **ms > 0.05 && **c > 0).map(|(ms, c)| *c as f64 / *ms as f64).collect();
            ratios.sort_by(|a, b| a.total_cmp(b));
            let cpu_note = if ratios.is_empty() {
                String::new()
            } else {
                let per_ms = ratios[ratios.len() / 2];
                let mut cpu_ms: Vec<f32> = self.gc_cycles_cpu.iter().map(|c| (*c as f64 / per_ms) as f32).collect();
                let over_cpu = cpu_ms.iter().filter(|x| **x > 4.0).count();
                cpu_ms.sort_by(|a, b| a.total_cmp(b));
                let n = cpu_ms.len();
                format!("; in CPU time of the game thread (QueryThreadCycleTime): p99 {:.3} ms, max {:.3} ms, {over_cpu} above 4 ms", cpu_ms[((n as f64 - 1.0) * 0.99) as usize], cpu_ms[n - 1])
            };
            if self.config.gc_full {
                eprintln!("[shell] collector (TERRARIA_GC_FULL: rt::collect after {} MB of allocation, at the latest every {} ticks): {}", self.config.gc_bytes >> 20, self.config.gc_interval, summary(&mut self.gc_ms));
            } else {
                eprintln!(
                    "[shell] collector pauses (incremental: a cycle after {} MB of allocation or {} ticks, {} us per tick while it runs): {}; {over} above 4 ms{cpu_note}",
                    self.config.gc_bytes >> 20,
                    self.config.gc_interval,
                    self.config.gc_slice_us,
                    summary(&mut self.gc_ms)
                );
                let n = self.gc_cycles.len().max(1);
                let sum = self.gc_cycles.iter().fold((0u64, 0usize, 0usize, 0usize), |a, c| (a.0 + c.0 as u64, a.1 + c.1, a.2 + c.2, a.3 + c.3));
                eprintln!(
                    "[shell] collector cycles: {}; per cycle {:.0} slices, {:.0} objects traced again at the end of the mark phase, {:.2} conservative passes, {:.0} objects freed; the pauses that ended a mark phase: {}",
                    self.gc_cycles.len(),
                    sum.0 as f64 / n as f64,
                    sum.1 as f64 / n as f64,
                    sum.2 as f64 / n as f64,
                    sum.3 as f64 / n as f64,
                    summary(&mut self.gc_end_ms)
                );
                eprintln!("[shell] collector pauses by kind: {}", by_kind.join("; "));
                for line in rt::gc_profile_report() {
                    eprintln!("[shell] gc profile: {line}");
                }
                let top: Vec<String> = rt::rescan_histogram().into_iter().take(8).map(|(n, c)| format!("{c} {n}")).collect();
                eprintln!("[shell] objects traced again at the end of a mark phase, by type: {}", top.join("; "));
            }
            eprintln!(
                "[shell] queued work ({:?}): gave the frame back {} times at yield points; the script clock stood still for {} ticks",
                self.config.yield_mode,
                terraria_host::sys::threading::yields(),
                self.script_stalled_total
            );
            if std::env::var_os("TERRARIA_HEAP_HISTOGRAM").is_some_and(|v| !v.is_empty() && v != "0") {
                for (name, count, bytes) in rt::heap_histogram().into_iter().take(40) {
                    eprintln!("[shell] heap: {count:>9} objects {:>8.1} MB  {name}", bytes as f64 / 1048576.0);
                }
            }
            // allocation rate in the second half of the run (the first half has start-up and the world load in it)
            let half = &self.gc_allocated[self.gc_allocated.len() / 2..];
            let (ticks, bytes) = half.iter().fold((0u64, 0usize), |a, x| (a.0 + x.0, a.1 + x.1));
            if ticks > 0 {
                eprintln!("[shell] allocation between collections, second half of the run: {:.1} KB per tick ({} collections in {} ticks)", bytes as f64 / 1024.0 / ticks as f64, half.len(), ticks);
            }
            // the heap after the collections: every 10th sample, and the last
            let n = self.heap_log.len();
            let picks: Vec<String> = self.heap_log.iter().enumerate().filter(|(i, _)| i % (n / 10).max(1) == 0 || *i + 1 == n).map(|(_, (t, o, b))| format!("tick {t}: {o} objects {:.1} MB", *b as f64 / 1048576.0)).collect();
            eprintln!("[shell] heap after a collection: {}", picks.join("; "));
            if let Some(a) = audio_stats() {
                eprintln!("[shell] audio mixer: {}", a.summary());
            }
            if self.frame_seconds > 0.0 {
                eprintln!(
                    "[shell] frame pacing (interval between shell ticks, real time): {}; {} updates and {} draws in {:.1} s = {:.2} updates/s, {:.2} draws/s",
                    summary(&mut self.frame_ms),
                    self.frame_updates,
                    self.frame_draws,
                    self.frame_seconds,
                    self.frame_updates as f64 / self.frame_seconds,
                    self.frame_draws as f64 / self.frame_seconds
                );
            }
            let mut replay = render::REPLAY_MS.lock().map(|v| v.clone()).unwrap_or_default();
            eprintln!("[shell] GPU replay, CPU side (render thread): {}", summary(&mut replay));
        }
    }
}

/// Adds the frame driver for the game `G`, built by `make` on the game thread once the host is initialised
/// (`new Main()` needs `game::init` to have run).
pub struct ShellPlugin<G: HostedGame + 'static> {
    make: fn() -> G,
    config: ShellConfig,
}

impl<G: HostedGame + 'static> ShellPlugin<G> {
    pub fn new(make: fn() -> G) -> Self {
        ShellPlugin { make, config: ShellConfig::from_env() }
    }
    pub fn with_config(make: fn() -> G, config: ShellConfig) -> Self {
        ShellPlugin { make, config }
    }
}

impl<G: HostedGame + 'static> Plugin for ShellPlugin<G> {
    fn build(&self, app: &mut App) {
        let script = self.config.script.as_ref().map(|p| match std::fs::read_to_string(p).map_err(|e| e.to_string()).and_then(|t| script::Script::parse(&t)) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("{}: {e}", p.display());
                std::process::exit(2);
            }
        });
        let audio = AudioBackend::start();
        AUDIO_STATS.with(|s| *s.borrow_mut() = Some(audio.stats.clone()));
        app.add_observer(keep_window_when_its_monitor_goes);
        app.insert_non_send(ShellState::<G> {
            make: self.make,
            config: self.config.clone(),
            game: None,
            audio,
            script,
            started: false,
            finished: false,
            tick: 0,
            waited: 0,
            reset_clock: false,
            shadow: window::WindowShadow::default(),
            last_size: (0, 0),
            monitors: Vec::new(),
            modes: Vec::new(),
            last_title: String::new(),
            exit_code: 0,
            gc_ms: Vec::new(),
            gc_end_ms: Vec::new(),
            gc_cycles: Vec::new(),
            gc_slices: 0,
            gc_verify_set: false,
            script_tick: 0,
            script_stalled: 0,
            script_stalled_total: 0,
            waiting: None,
            load_shots: None,
            load_shot_ticks: 0,
            load_shots_taken: 0,
            gc_cycles_cpu: Vec::new(),
            gc_kind: Vec::new(),
            heap_log: Vec::new(),
            last_gc_tick: 0,
            gc_allocated: Vec::new(),
            frame_ms: Vec::new(),
            frame_updates: 0,
            frame_draws: 0,
            frame_seconds: 0.0,
            frame_log: self.config.frame_log.as_ref().and_then(|p| match std::fs::File::create(p) {
                Ok(f) => {
                    use std::io::Write as _;
                    let mut w = std::io::BufWriter::new(f);
                    let _ = writeln!(w, "tick\ttime_ms\tinterval_ms\tupdates\tdrawn\tactive\tminimized\twidth\theight\tend_draw_ms\ttick_ms");
                    Some(w)
                }
                Err(e) => {
                    eprintln!("TERRARIA_FRAME_LOG {}: {e}", p.display());
                    None
                }
            }),
            run_start: None,
            pads: pad::PadAdapter::default(),
            hwnd: 0,
            hook: false,
            frame: (0, 0),
            pending_mouse: None,
            said: Vec::new(),
            told: (None, false, false),
        })
        .init_resource::<HostInputState>()
        .insert_resource(NoActivate(self.config.noactivate))
        .add_systems(First, track_window_focus)
        .add_systems(Startup, setup_back_buffer)
        .add_systems(Update, (sync_back_buffer, host_input::collect, shell_tick::<G>, capture_and_exit::<G>).chain())
        .add_systems(Last, shell_shutdown::<G>);
        // Sleeps of this process end on a 1 ms grid (the mixer thread's 2 ms sleeps, InactiveSleepTime, waits).
        // TERRARIA_COARSE_TIMER=1 leaves the system's resolution (a measuring aid).
        if std::env::var_os("TERRARIA_COARSE_TIMER").is_some_and(|v| !v.is_empty() && v != "0") {
            eprintln!("[shell] TERRARIA_COARSE_TIMER: the system's timer resolution is kept");
        } else if !native::fine_timer() && cfg!(windows) {
            eprintln!("[shell] timeBeginPeriod(1) was refused: sleeps keep the system's timer resolution");
        }
    }
}

/// A window with the game's back buffer, plus the shell. `size` is the client size in pixels (no DPI scaling: the game
/// works in client pixels).
pub fn run<G: HostedGame + 'static>(title: &str, size: (u32, u32), make: fn() -> G) {
    if !terraria_host::content::install_dir().join("Content").is_dir() {
        eprintln!("Terraria content folder not found at {} (set TERRARIA_DIR)", terraria_host::content::install_dir().display());
        std::process::exit(1);
    }
    let config = ShellConfig::from_env();
    eprintln!("[shell] window: {}x{} client pixels{}{}", size.0, size.1, if config.noactivate { ", TERRARIA_NOACTIVATE" } else { "" }, config.window_pos.map_or(String::new(), |(x, y)| format!(", at {x},{y}")));
    let code = App::new()
        .add_plugins(DefaultPlugins.set(ImagePlugin::default_nearest()).set(WindowPlugin {
            primary_window: Some(Window {
                title: title.into(),
                resolution: WindowResolution::from(size).with_scale_factor_override(1.0),
                visible: !config.hidden,
                // TERRARIA_NOACTIVATE: winit shows the window with SW_SHOWNOACTIVATE and it gets no task bar button
                focused: !config.noactivate,
                skip_taskbar: config.noactivate,
                position: config.window_pos.map_or(bevy::window::WindowPosition::Automatic, |(x, y)| bevy::window::WindowPosition::At(IVec2::new(x, y))),
                ..default()
            }),
            ..default()
        }))
        .add_plugins(GfxPlugin)
        // Bevy updates a window without the focus only when an event arrives or every 1/60 s (and then once more
        // for every input message). A TERRARIA_NOACTIVATE window never has the OS focus: it runs as a focused game
        // window does (one update after the other, paced by the present and by the game's own EndDraw).
        .insert_resource(if config.noactivate {
            bevy::winit::WinitSettings { focused_mode: bevy::winit::UpdateMode::Continuous, unfocused_mode: bevy::winit::UpdateMode::Continuous }
        } else {
            bevy::winit::WinitSettings::game()
        })
        .add_plugins(ShellPlugin::with_config(make, config))
        .run();
    if let AppExit::Error(c) = code {
        std::process::exit(c.get() as i32);
    }
}

/// A window must outlive its monitor. Bevy 0.19 links every window to the monitor it is on (`OnMonitor`, a
/// relationship with `linked_spawn`) and despawns the monitor's entity when the display goes away (bevy_winit
/// `create_monitors`): the display powers down or is unplugged, and every window on it is despawned with it, the
/// app exits (`exit_on_all_closed`) and the game is gone without `Exiting` / a save. Seen in the multiplayer proof:
/// two hidden clients ended in the same millisecond with "Monitor removed", "No windows are open, exiting".
/// Nothing here reads the link (the monitor list comes from the `Monitor` entities), so it is taken off again as
/// soon as it is put on.
fn keep_window_when_its_monitor_goes(link: On<Insert, bevy::window::OnMonitor>, mut commands: Commands) {
    commands.entity(link.entity).try_remove::<bevy::window::OnMonitor>();
}

#[derive(Resource)]
struct NoActivate(bool);

/// The focus as the window system reports it (`host_input::track_focus`, which also explains why a
/// `TERRARIA_NOACTIVATE` window must not keep `Window::focused` set).
fn track_window_focus(no_activate: Res<NoActivate>, mut state: ResMut<HostInputState>, mut focus: MessageReader<bevy::window::WindowFocused>, mut window: Single<&mut Window, With<PrimaryWindow>>) {
    host_input::track_focus(&mut state, &mut focus, &mut window, no_activate.0);
}

#[derive(Resource)]
struct BackBuffer {
    width: u32,
    height: u32,
}

#[derive(Component)]
struct BackBufferSprite;

fn setup_back_buffer(mut commands: Commands, mut images: ResMut<Assets<Image>>, window: Single<&Window, With<PrimaryWindow>>) {
    let (w, h) = (window.physical_width().max(1), window.physical_height().max(1));
    let image = images.add(render::new_target_image(w, h));
    commands.insert_resource(BackBufferImage(image.clone()));
    commands.spawn(Camera2d);
    commands.spawn((Sprite { image, custom_size: Some(Vec2::new(window.width(), window.height())), ..default() }, BackBufferSprite));
    commands.insert_resource(BackBuffer { width: w, height: h });
}

/// The back-buffer image follows the window's client size (a minimised window reports 0 x 0: keep the last one).
fn sync_back_buffer(
    window: Single<&Window, With<PrimaryWindow>>,
    mut back: ResMut<BackBuffer>,
    mut images: ResMut<Assets<Image>>,
    mut back_image: ResMut<BackBufferImage>,
    mut sprite: Single<&mut Sprite, With<BackBufferSprite>>,
) {
    let (w, h) = (window.physical_width(), window.physical_height());
    if w == 0 || h == 0 {
        return;
    }
    if (w, h) != (back.width, back.height) {
        let image = images.add(render::new_target_image(w, h));
        back_image.0 = image.clone();
        sprite.image = image;
        *back = BackBuffer { width: w, height: h };
    }
    sprite.custom_size = Some(Vec2::new(window.width(), window.height()));
}

#[derive(SystemParam)]
struct InputParams<'w, 's> {
    state: ResMut<'w, HostInputState>,
    buttons: Res<'w, ButtonInput<MouseButton>>,
    raw_pads: MessageReader<'w, 's, RawGamepadEvent>,
    rumble: Option<ResMut<'w, Messages<GamepadRumbleRequest>>>,
}

#[derive(SystemParam)]
struct GpuParams<'w> {
    published: Res<'w, PublishedFrames>,
    shared: Res<'w, SharedGpu>,
    device: Option<Res<'w, RenderDevice>>,
    queue: Option<Res<'w, RenderQueue>>,
}

/// The native handle of the window (`HWND`), 0 where there is none.
fn native_handle(raw: Option<&RawHandleWrapper>) -> isize {
    match raw.map(|r| r.get_window_handle()) {
        Some(raw_window_handle::RawWindowHandle::Win32(h)) => h.hwnd.get(),
        _ => 0,
    }
}

/// The one system that drives the game (see the module docs for the order and for why it is a single system).
#[allow(clippy::too_many_arguments)]
fn shell_tick<G: HostedGame + 'static>(
    mut shell: NonSendMut<ShellState<G>>,
    mut input: InputParams,
    gpu: GpuParams,
    monitors: window::MonitorQuery,
    mut window_q: Single<(&mut Window, &mut CursorOptions, Option<&RawHandleWrapper>), With<PrimaryWindow>>,
    time: Res<Time<Real>>,
    mut commands: Commands,
    mut exit: MessageWriter<AppExit>,
) {
    let sh = &mut *shell;
    if sh.finished {
        return;
    }
    let tick_started = std::time::Instant::now();
    let (window, cursor, raw_handle) = &mut *window_q;
    let hidden = sh.config.hidden;
    // how window requests of the game are carried out (recorded only, applied, applied inside a fake monitor)
    let apply = sh.config.apply_mode();
    let kind = sh.config.run_kind();
    let scripted = sh.script.is_some();

    // the window's native handle; a run that takes its input from the window gets the message hook (native.rs)
    if sh.hwnd == 0 {
        sh.hwnd = native_handle(*raw_handle);
        if sh.hwnd != 0 && !scripted && native::install(sh.hwnd) {
            sh.hook = true;
            let hwnd = sh.hwnd;
            // what WM_ACTIVATEAPP and WM_SIZE said before the hook existed. A TERRARIA_NOACTIVATE window was
            // never activated: it is inactive until a WM_ACTIVATEAPP is posted to it
            let active = !sh.config.noactivate && native::app_is_foreground();
            native::with_state(|m| {
                m.focus.active = active;
                m.focus.minimized = native::is_iconic(hwnd);
            });
            eprintln!("[shell] message hook on the window: text from WM_CHAR, keys from the key messages, Game.IsActive from WM_ACTIVATEAPP (now {active})");
        }
    }
    let facts = if kind == native::RunKind::Normal { native::facts(sh.hwnd) } else { native::WindowFacts::default() };
    if let Some(p) = sh.pending_mouse.take() {
        input.state.mouse = Some(p);
    }

    // 1. input and window state. The readers are drained even when a script supplies the input.
    let (mut snapshot, mut ws) = host_input::gather(&mut input.state, &input.buttons, window);
    let mut hook_focus = None;
    if sh.hook {
        // text, keys and focus are the window's own messages; what Bevy's events said was dropped by `gather`
        let (text, keys, mut focus) = native::with_state(|m| (m.take_text(), m.keys_down(), m.focus));
        snapshot.text = text;
        snapshot.keys_down = keys;
        if kind == native::RunKind::Normal && facts.foreground && !focus.active {
            // the window IS the foreground window: a WM_ACTIVATEAPP was missed (it cannot be, and if it is the
            // game must not stay asleep in front of the player)
            focus.active = true;
            native::with_state(|m| m.focus.active = true);
            eprintln!("[shell] focus: the window is the foreground window but no WM_ACTIVATEAPP said so: active from now on");
        }
        ws.active = focus.active;
        hook_focus = Some(focus);
    }
    match native::mouse_source(kind, &facts) {
        native::MouseSource::Pointer(x, y) => {
            (snapshot.mouse_x, snapshot.mouse_y) = (x, y);
            input.state.mouse = Some((x, y));
            if !sh.told.2 {
                sh.told.2 = true;
                eprintln!("[shell] mouse: following the pointer (GetCursorPos, also outside the client area) while the window is in front");
            }
        }
        native::MouseSource::Messages => sh.told.2 = false,
    }
    // Form.ActiveForm: the foreground window, where the shell may ask (a normal run)
    winforms::set_foreground((kind == native::RunKind::Normal && sh.hwnd != 0).then_some(facts.foreground));
    // the input method follows IImeService.Enable / Disable (a text field is being edited), as the original's
    // WindowsIme does; never in a test run
    if kind == native::RunKind::Normal {
        let want = sh.started && terraria_host::os::ime_enabled();
        if window.ime_enabled != want {
            window.ime_enabled = want;
        }
        terraria_host::os::set_ime_composition(&input.state.composition);
    }
    for e in input.raw_pads.read() {
        if let Some((id, ev)) = pad::from_bevy(e) {
            if matches!(ev, pad::PadEvent::Connected | pad::PadEvent::Disconnected) {
                eprintln!("[shell] game pad {id:#x}: {ev:?}{}", if sh.config.no_gamepad { " (TERRARIA_NO_GAMEPAD: not reported to the game)" } else { "" });
            }
            sh.pads.apply(id, ev);
        }
    }
    if !sh.config.no_gamepad {
        snapshot.game_pads = sh.pads.snapshot();
    }
    // after the frame was switched on / off the client area keeps its size (window.rs, "Borderless")
    if let Some((w, h)) = window::enforce_client_size(window, &mut sh.shadow) {
        (ws.width, ws.height) = (w as i32, h as i32);
    }
    let mut minimized = window.physical_width() == 0 || window.physical_height() == 0 || hook_focus.is_some_and(|f| f.minimized);
    // the frame of the window, for placements inside a fake monitor
    if window.decorations && !minimized {
        if let Some((_, _, w, h, cw, ch)) = native::window_rect(sh.hwnd) {
            if cw > 0 && ch > 0 {
                sh.frame = (w - cw, h - ch);
            }
        }
    }
    if minimized && sh.last_size.0 > 0 {
        // a minimised window reports 0 x 0: the game keeps the size it had
        (ws.width, ws.height) = sh.last_size;
    } else {
        sh.last_size = (ws.width, ws.height);
    }
    if hidden {
        // a hidden window never has focus; automation runs count as active so the loop is not throttled
        ws.active = true;
    }
    let pos = match window.position {
        bevy::window::WindowPosition::At(p) => Some(p),
        _ => None,
    };
    if let Some(m) = sh.config.fake_monitor {
        // TERRARIA_FAKE_MONITOR: the one monitor of a test window, outside the real desktop. The game sees it at
        // (0, 0), as a primary monitor is, and the window's position relative to it
        (ws.display_width, ws.display_height) = window::single_monitor((0, 0, m.2, m.3), &mut sh.monitors);
        (ws.x, ws.y) = (ws.x - m.0, ws.y - m.1);
    } else if let Some((w, h)) = sh.config.display {
        // a fixed, single monitor (scripted runs: what the game derives from the display must not depend on the
        // machine: `Main.TryPickingDefaultUIScale` takes the UI scale from the display height)
        (ws.display_width, ws.display_height) = window::single_monitor((0, 0, w, h), &mut sh.monitors);
    } else if let Some((w, h)) = window::sync_monitors(monitors.iter(), pos, &mut sh.monitors, &mut sh.modes) {
        (ws.display_width, ws.display_height) = (w, h);
    }
    let mut fx = script::Effects::default();
    if sh.started {
        if let Some(s) = sh.script.as_mut() {
            // the script's clock stands still while queued work is suspended at a yield point (a load in
            // progress); a work item that never ends must not stop the script for good
            const STALL_LIMIT: u32 = 100_000;
            let yielding = sh.config.yield_mode != terraria_host::sys::threading::YieldMode::Off;
            let mut load_shot: Option<PathBuf> = None;
            // `wait`: the clock stands still until the game adapter says the condition holds
            let mut held = false;
            if let Some((cond, deadline)) = sh.waiting.clone() {
                let ok = sh.game.as_mut().is_some_and(|g| g.condition(&cond));
                if ok {
                    eprintln!("[shell] wait {cond:?}: holds (script tick {}, shell tick {})", sh.script_tick, sh.tick);
                    sh.waiting = None;
                } else if std::time::Instant::now() >= deadline {
                    let text = format!("script wait {cond:?} timed out at script tick {}", sh.script_tick);
                    eprintln!("[shell] {text}");
                    if let Some(g) = sh.game.as_mut() {
                        g.script_failed(&text);
                    }
                    sh.waiting = None;
                } else {
                    held = true;
                    // the other process needs real time: do not spin
                    std::thread::sleep(std::time::Duration::from_millis(4));
                }
            }
            // (work that only waits for time or the network does not count: the threads of a hosted / joined game)
            if held {
            } else if yielding && terraria_host::sys::threading::has_suspended_foreground_work() && sh.script_stalled < STALL_LIMIT {
                sh.script_stalled += 1;
                sh.script_stalled_total += 1;
                if sh.script_stalled == STALL_LIMIT {
                    eprintln!("[shell] queued work has been suspended for {STALL_LIMIT} ticks: the script goes on");
                }
                // `loadshots`: the loading screen, which no `frame N shot` can reach (the clock stands still)
                if let Some((pattern, every, count)) = sh.load_shots.as_ref() {
                    sh.load_shot_ticks += 1;
                    if sh.load_shots_taken < *count && sh.load_shot_ticks % *every == 0 {
                        load_shot = Some(PathBuf::from(pattern.replace("{}", &format!("{:02}", sh.load_shots_taken))));
                        sh.load_shots_taken += 1;
                    }
                }
            } else {
                if !terraria_host::sys::threading::has_suspended_foreground_work() {
                    sh.script_stalled = 0;
                }
                sh.script_tick += 1;
            }
            let (snap, effects) = s.step(sh.script_tick);
            snapshot = snap;
            fx = effects;
            if let Some((secs, cond)) = fx.wait.take() {
                eprintln!("[shell] wait {cond:?}: up to {secs} s (script tick {})", sh.script_tick);
                sh.waiting = Some((cond, std::time::Instant::now() + std::time::Duration::from_secs(secs as u64)));
            }
            if let Some(l) = fx.load_shots.take() {
                sh.load_shots = Some(l);
                sh.load_shot_ticks = 0;
                sh.load_shots_taken = 0;
            }
            if let Some(path) = load_shot {
                eprintln!("[shell] loadshots: {} (suspended tick {})", path.display(), sh.load_shot_ticks);
                fx.shot = true;
                fx.shot_path = Some(path);
                fx.commands.push("dump ui".to_owned());
            }
            // the scripted window: `focus lost`, `minimise` (a hidden window is otherwise always active)
            let (focused, minimised) = s.window_state();
            if !focused || minimised {
                ws.active = false;
            }
            minimized |= minimised;
            // game-adapter commands run before this tick's Update; `aim` moves the scripted cursor at once
            if let Some(g) = sh.game.as_mut() {
                for c in &fx.commands {
                    if let Some((x, y)) = g.command(sh.script_tick, c) {
                        s.set_mouse(x, y);
                        (snapshot.mouse_x, snapshot.mouse_y) = (x, y);
                    }
                }
            }
        }
    }
    if !sh.hook && (sh.shadow.minimized || minimized) {
        // Without the window's own messages (no hook: a script's `minimise`, other systems) minimised counts as
        // deactivated, which is what happens to a window a person minimises. With the hook the two are separate,
        // as in XNA: `Game.IsActive` is WM_ACTIVATEAPP alone, and the game's own FocusHelper reads the form's
        // window state, reported below.
        ws.active = false;
    }
    winforms::report_minimized(sh.shadow.minimized || minimized);
    if sh.hook && sh.config.noactivate && (sh.told.0, sh.told.1) != (Some(ws.active), minimized) {
        eprintln!("[shell] window: Game.IsActive = {}, minimised = {minimized}", ws.active);
        (sh.told.0, sh.told.1) = (Some(ws.active), minimized);
    }

    // start-up: wait until the GPU has the back buffer, so draws of the first frames (and a readback during
    // `load_content`) have somewhere to land
    if !sh.started {
        if !gpu.shared.has_back_buffer() && sh.waited < sh.config.gpu_wait_frames {
            sh.waited += 1;
            return;
        }
        // `Game.RunGame`
        rt::enter();
        game::init(ws);
        // a window that really takes the sizes the game asks for: ApplyChanges resizes it at once, as XNA's form
        // (host docs). Hidden / scripted and plain TERRARIA_NOACTIVATE runs keep the window they were started with
        game::set_window_follows_apply_changes(matches!(apply, window::Apply::Real | window::Apply::Fake { .. }) && !scripted);
        terraria_host::sys::threading::set_yield_mode(sh.config.yield_mode);
        // handles in the frames of suspended work (a load, a generation, the network threads) are found by scanning
        // those stacks conservatively: collections finish while such work exists
        if !std::env::var_os("TERRARIA_GC_FIBER_SCAN").is_some_and(|v| v == "0") {
            rt::set_conservative_roots(Some(|f: &mut dyn FnMut(*const u8, usize)| terraria_host::sys::threading::suspended_stacks(f)));
        }
        // a visible window with a person in front of it: the game's message boxes are real dialogs
        if kind == native::RunKind::Normal {
            winforms::set_message_box_presenter(Some(native_message_box));
        }
        sh.shadow.full_screen = false;
        if let (Some(d), Some(q)) = (gpu.device.as_ref(), gpu.queue.as_ref()) {
            let backend = Rc::new(RefCell::new(WgpuReadback::new(&gpu.shared, &gpu.published, d, q)));
            terraria_host::readback::install_sync(game::graphics_device(), backend);
        }
        let mut g = (sh.make)();
        g.initialize();
        g.load_content();
        g.begin_run();
        // the first Update, with ElapsedGameTime zero (Game.cs:260-263); the game time object starts at zero
        let t = game::roots().game_time;
        g.update(t);
        terraria_host::sys::threading::run_pending_work();
        sh.game = Some(g);
        sh.started = true;
        sh.reset_clock = true;
        apply_outputs(sh, window, cursor, apply, &mut input.rumble, &mut exit, &gpu.published, &mut commands, game::end_draw(false), fx.shot, fx.shot_path.clone());
        return;
    }

    sh.tick += 1;
    if fx.quit {
        // the window is closed: same path as a user closing it
        sh.finish();
        exit_with(sh, &mut exit);
        return;
    }
    if let Some((w, h)) = fx.resize {
        // the new size reaches the game through `WindowState` on the next tick, like a user's drag
        window.resolution.set_physical_resolution(w, h);
    }

    // Game.Tick: an inactive window sleeps first (Game.cs:318-321)
    if !ws.active && !hidden {
        let ms = game::inactive_sleep_time().ticks / 10_000;
        std::thread::sleep(std::time::Duration::from_millis(ms.max(0) as u64));
    }
    let elapsed = if sh.reset_clock {
        sh.reset_clock = false;
        0.0
    } else if sh.config.fixed_dt {
        game::target_elapsed_time().ticks as f64 / 1e7
    } else {
        time.delta_secs_f64()
    };

    let g = sh.game.as_mut().expect("started");
    // TERRARIA_COMMAND_FILE: state reads for a test driver (looked for every 4th tick)
    if sh.tick % 4 == 0 {
        if let Some(text) = sh.config.command_file.as_ref().and_then(|p| {
            let text = std::fs::read_to_string(p).ok()?;
            let _ = std::fs::remove_file(p);
            Some(text)
        }) {
            for c in text.lines().map(str::trim).filter(|l| !l.is_empty()) {
                if c.starts_with("dump ") || c.starts_with("aim ") {
                    // the cursor position an `aim` returns is only written to the state log: nothing moves
                    let _ = g.command(sh.tick, c);
                } else {
                    eprintln!("[shell] TERRARIA_COMMAND_FILE: only dump / aim lines are run, not {c:?}");
                }
            }
        }
    }
    let frame_active = ws.active;
    let frame_size = (ws.width, ws.height);
    let mut frame_updates = 0u32;
    // 2.-3.
    game::begin_frame(snapshot, ws, elapsed);
    game::dispatch_events();
    // 4. the catch-up loop
    while let Some(t) = game::begin_update() {
        frame_updates += 1;
        g.update(t);
        // queued "thread" work runs here and is translated code: an exception that leaves it is an unhandled
        // exception on a worker thread, which ends the process in the original too
        if host_step("queued thread work (game::end_update)", game::end_update).is_none() {
            return fail(sh, &mut exit);
        }
    }
    // 5. DrawFrame (a minimised window draws nothing)
    let draw_time = if minimized { None } else { game::begin_draw() };
    let mut end_draw_ms = 0f32;
    if let Some(t) = draw_time {
        g.draw(t);
        if !sh.config.fixed_dt {
            // The game's own frame pacing: a busy wait until 1/60 s after the last frame (one core at 100 %, as the
            // original). Sleeping through most of it instead was measured (10 s blocks of each in turn in one
            // run): 57.5 instead of 60.1 updates per second on a quiet machine, 47.1 instead of 54.9 under a
            // synthetic CPU load. The spin stays.
            let t0 = std::time::Instant::now();
            g.end_draw();
            end_draw_ms = t0.elapsed().as_secs_f32() * 1000.0;
        }
    }
    if !sh.config.fixed_dt {
        let start = *sh.run_start.get_or_insert_with(std::time::Instant::now);
        let ms = (elapsed * 1000.0) as f32;
        sh.frame_ms.push(ms);
        sh.frame_updates += frame_updates as u64;
        sh.frame_draws += draw_time.is_some() as u64;
        sh.frame_seconds += elapsed;
        if let Some(w) = sh.frame_log.as_mut() {
            use std::io::Write as _;
            let _ = writeln!(
                w,
                "{}\t{:.3}\t{:.3}\t{}\t{}\t{}\t{}\t{}\t{}\t{:.3}\t{:.3}",
                sh.tick,
                start.elapsed().as_secs_f64() * 1000.0,
                ms,
                frame_updates,
                draw_time.is_some() as u8,
                frame_active as u8,
                minimized as u8,
                frame_size.0,
                frame_size.1,
                end_draw_ms,
                tick_started.elapsed().as_secs_f32() * 1000.0
            );
            if sh.tick % 30 == 0 {
                let _ = w.flush();
            }
        }
    }
    // Present refuses a frame that a failed Draw left with a render target bound
    let Some(out) = host_step("Present (game::end_draw)", || game::end_draw(draw_time.is_some())) else {
        return fail(sh, &mut exit);
    };
    apply_outputs(sh, window, cursor, apply, &mut input.rumble, &mut exit, &gpu.published, &mut commands, out, fx.shot, fx.shot_path.clone());
}

/// A host driver call that can run translated code or throw a C# exception: `None` (after printing why) instead
/// of a panic inside a Bevy system.
fn host_step<R>(what: &str, f: impl FnOnce() -> R) -> Option<R> {
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| rt::try_(f))) {
        Ok(Ok(r)) => Some(r),
        Ok(Err(e)) => {
            eprintln!("[shell] unhandled C# exception in {what}: {}", rt::exc_text(e));
            None
        }
        Err(p) => {
            let text = p.downcast_ref::<String>().cloned().or_else(|| p.downcast_ref::<&str>().map(|s| (*s).to_owned())).unwrap_or_else(|| "<non-string panic payload>".to_owned());
            eprintln!("[shell] stopped in {what}: {text}");
            None
        }
    }
}

/// Ends the run with exit code 1 after a failed host step (the game is shut down as far as it lets itself be).
fn fail<G: HostedGame>(sh: &mut ShellState<G>, exit: &mut MessageWriter<AppExit>) {
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = rt::try_(|| sh.finish());
    }));
    sh.finished = true;
    sh.exit_code = sh.exit_code.max(1);
    exit_with(sh, exit);
}

#[allow(clippy::too_many_arguments)]
fn apply_outputs<G: HostedGame>(
    sh: &mut ShellState<G>,
    window: &mut Mut<Window>,
    cursor: &mut Mut<CursorOptions>,
    apply: window::Apply,
    rumble: &mut Option<ResMut<Messages<GamepadRumbleRequest>>>,
    exit: &mut MessageWriter<AppExit>,
    published: &PublishedFrames,
    commands: &mut Commands,
    out: game::FrameOutput,
    shot: bool,
    shot_path: Option<PathBuf>,
) {
    // 6.
    sh.audio.submit(out.audio);
    published.push(out.frame);
    if !out.title.is_empty() && out.title != sh.last_title {
        window.title = out.title.clone();
        sh.last_title = out.title;
    }
    if sh.config.noactivate && cursor.visible != out.is_mouse_visible {
        eprintln!("[shell] Game.IsMouseVisible = {}: the system cursor is {} over the client area", out.is_mouse_visible, if out.is_mouse_visible { "shown" } else { "hidden" });
    }
    window::apply_cursor(out.is_mouse_visible, cursor);
    if let Some(r) = out.window_request {
        window::apply_request(r, window, &mut sh.shadow, apply, sh.frame, game::display_mode());
    }
    window::apply_commands(winforms::take_commands(), window, &mut sh.shadow, apply, sh.frame);
    if let Some(m) = sh.config.present_override {
        if window.present_mode != m {
            window.present_mode = m;
        }
    }
    // `Mouse.SetPosition` (native.rs, `set_position_action`): the real pointer moves only for the foreground
    // window of a normal run, as the original moves it; otherwise the game's own cursor is there until the next
    // mouse message, and a scripted pointer follows
    if let Some((x, y)) = out.mouse_position_request {
        if let Some(s) = sh.script.as_mut() {
            s.set_mouse(x, y);
        } else {
            match native::set_position_action(sh.config.run_kind(), &native::facts(sh.hwnd)) {
                native::SetPositionAction::MovePointer => {
                    if !native::move_pointer(sh.hwnd, x, y) {
                        sh.pending_mouse = Some((x, y));
                        sh.say_once("Mouse.SetPosition: the window stopped being the foreground window: the pointer was not moved".to_owned());
                    }
                }
                native::SetPositionAction::Virtual(why) => {
                    sh.pending_mouse = Some((x, y));
                    sh.say_once(format!("Mouse.SetPosition: the real pointer is not moved ({why}); the game's own cursor goes there"));
                }
            }
        }
    }
    // `GamePad.SetVibration`: to the device of that slot (Bevy / gilrs force feedback); a scripted pad has none
    let vibration = terraria_host::input::take_vibration_requests();
    if vibration.iter().any(Option::is_some) {
        let device = sh.script.is_none() && rumble.is_some();
        let (requests, inert) = pad::rumble_requests(vibration, &sh.pads, |_| device);
        if !requests.is_empty() {
            sh.say_once("GamePad.SetVibration: handed to Bevy's gamepad rumble (gilrs force feedback); never felt on a device".to_owned());
        }
        if let Some(r) = rumble.as_mut() {
            for q in requests {
                r.write(q);
            }
        }
        for line in inert {
            sh.say_once(line);
        }
    }
    if shot {
        if let Some(path) = shot_path.or_else(|| sh.config.screenshot.clone()) {
            let path = if path.is_relative() { sh.config.launch_dir.join(path) } else { path };
            commands.spawn(Screenshot::primary_window()).observe(save_to_disk(path));
        }
    }
    if out.exit {
        sh.finish();
        exit_with(sh, exit);
        return;
    }
    // 7.
    if !sh.gc_verify_set {
        sh.gc_verify_set = true;
        rt::set_verify(sh.config.gc_verify);
    }
    if sh.config.gc_interval == 0 {
        return;
    }
    let allocated = rt::allocated_since_collect();
    let due = sh.tick - sh.last_gc_tick >= sh.config.gc_interval || allocated >= sh.config.gc_bytes;
    // SAFETY (all collector calls below): between frames, on the game thread; the game roots what it keeps
    // (HostedGame docs). A work item suspended in the middle of its body has frames that hold handles the
    // collector cannot see (terraria-host sys/threading.rs, "Fibers"): no whole collection and no end of a mark
    // phase while one exists; marking and sweeping slices are safe (rt::gc_step).
    // With the fiber stacks scanned conservatively (rt::set_conservative_roots, installed at start-up unless
    // TERRARIA_GC_FIBER_SCAN=0) suspended work is no obstacle any more.
    let suspended = terraria_host::sys::threading::has_suspended_work() && !rt::has_conservative_roots();
    let c0 = thread_cycles();
    let t0 = std::time::Instant::now();
    if sh.config.gc_full {
        if due && !suspended {
            unsafe { rt::collect() };
            sh.gc_ms.push(t0.elapsed().as_secs_f32() * 1000.0);
            sh.gc_cycles_cpu.push(thread_cycles().wrapping_sub(c0));
            sh.gc_kind.push(4);
            sh.gc_allocated.push((sh.tick - sh.last_gc_tick, allocated));
            sh.last_gc_tick = sh.tick;
            let (objects, bytes) = rt::heap_size();
            sh.heap_log.push((sh.tick, objects, bytes));
        }
        return;
    }
    if rt::gc_phase() == rt::GcPhase::Idle {
        if due {
            sh.gc_allocated.push((sh.tick - sh.last_gc_tick, allocated));
            sh.last_gc_tick = sh.tick;
            sh.gc_slices = 0;
            unsafe { rt::gc_begin() };
            sh.gc_ms.push(t0.elapsed().as_secs_f32() * 1000.0);
            sh.gc_cycles_cpu.push(thread_cycles().wrapping_sub(c0));
            sh.gc_kind.push(0);
        }
        return;
    }
    let behind = allocated >= 2 * sh.config.gc_bytes;
    let budget = sh.config.gc_slice_us * if behind { 4 } else { 1 };
    let step = unsafe { rt::gc_step(rt::GcBudget::Micros(budget), !suspended) };
    let ms = t0.elapsed().as_secs_f32() * 1000.0;
    sh.gc_ms.push(ms);
    sh.gc_cycles_cpu.push(thread_cycles().wrapping_sub(c0));
    sh.gc_kind.push(if step.mark_ended { 2 } else if step.before == rt::GcPhase::Mark { 1 } else { 3 });
    sh.gc_slices += 1;
    if step.mark_ended {
        sh.gc_end_ms.push(ms);
    }
    if let Some(st) = step.finished {
        sh.gc_cycles.push((sh.gc_slices, st.rescanned, st.resolve_passes, st.freed_objects));
        sh.heap_log.push((sh.tick, st.live_objects, st.live_bytes));
    }
}

/// `MessageBox.Show` as a real dialog (`winforms::set_message_box_presenter`). The `MessageBoxButtons` /
/// `MessageBoxIcon` values are the Win32 `MB_*` flags and `DialogResult` is the Win32 button id.
fn native_message_box(m: &winforms::MessageBoxRecord) -> Option<i32> {
    #[cfg(windows)]
    {
        #[link(name = "user32")]
        extern "system" {
            fn MessageBoxW(owner: *mut core::ffi::c_void, text: *const u16, caption: *const u16, flags: u32) -> i32;
        }
        let wide = |s: &str| -> Vec<u16> { s.encode_utf16().chain(std::iter::once(0)).collect() };
        let (text, caption) = (wide(&m.text), wide(&m.caption));
        const MB_SETFOREGROUND: u32 = 0x1_0000;
        // SAFETY: two NUL-terminated UTF-16 strings that live across the call.
        let r = unsafe { MessageBoxW(core::ptr::null_mut(), text.as_ptr(), caption.as_ptr(), (m.buttons as u32 & 0xF) | (m.icon as u32 & 0xF0) | MB_SETFOREGROUND) };
        (r > 0).then_some(r)
    }
    #[cfg(not(windows))]
    {
        let _ = m;
        None
    }
}

/// CPU cycles the calling thread has used (0 where the system has no such counter).
fn thread_cycles() -> u64 {
    #[cfg(windows)]
    {
        #[link(name = "kernel32")]
        extern "system" {
            fn GetCurrentThread() -> *mut core::ffi::c_void;
            fn QueryThreadCycleTime(thread: *mut core::ffi::c_void, cycles: *mut u64) -> i32;
        }
        let mut c = 0u64;
        // SAFETY: the pseudo handle of the calling thread and a valid out pointer.
        unsafe { QueryThreadCycleTime(GetCurrentThread(), &mut c) };
        c
    }
    #[cfg(not(windows))]
    0
}

fn exit_with<G: HostedGame>(sh: &ShellState<G>, exit: &mut MessageWriter<AppExit>) {
    exit.write(match std::num::NonZeroU8::new(sh.exit_code) {
        Some(c) => AppExit::Error(c),
        None => AppExit::Success,
    });
}

/// The window was closed (or something else ended the app): the same shutdown as `Game.Exit`.
fn shell_shutdown<G: HostedGame + 'static>(mut shell: NonSendMut<ShellState<G>>, mut exits: MessageReader<AppExit>) {
    if exits.read().next().is_some() && !shell.finished {
        shell.finish();
    }
}

/// `TERRARIA_SCREENSHOT=<file.png>`: save the window contents at Bevy frame `TERRARIA_SCREENSHOT_FRAME`, then quit 30
/// frames later. (Not used when a script ends the run with `shot` / `quit`.)
fn capture_and_exit<G: HostedGame + 'static>(
    shell: NonSend<ShellState<G>>,
    mut commands: Commands,
    mut frame: Local<u32>,
    mut exit: MessageWriter<AppExit>,
) {
    if shell.script.is_some() {
        return;
    }
    let Some(path) = shell.config.screenshot.clone() else { return };
    *frame += 1;
    let at = shell.config.screenshot_frame;
    if *frame == at {
        commands.spawn(Screenshot::primary_window()).observe(save_to_disk(path));
    } else if *frame == at + 30 {
        exit.write(AppExit::Success);
    }
}
