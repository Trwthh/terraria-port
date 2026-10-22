//! The browser build of the port: `game.wasm`, a wasm32-wasip1 reactor driven by the page's Worker
//! (web/www/worker.js). Contract: web/www/HOST_ABI.md (imports `wasi_snapshot_preview1` + `tw`, exports `tw_init`,
//! `tw_frame`) and web/www/FRAME_FORMAT.md (what `tw.gfx_frame` receives).
//!
//! The frame sequence is the native shell's (`terraria-app/src/shell/mod.rs` `shell_tick`, which follows XNA's
//! `Game.Tick`): `begin_frame` -> events -> the catch-up loop of `Main.Update` + queued thread work -> `Main.Draw`
//! -> `end_draw` (Present), then a collector slice. Differences, each for a reason:
//! - `Main.EndDraw` (the game's busy-wait frame pacing) is not called: the browser paces by requestAnimationFrame and
//!   a busy wait would freeze the page. The native fixed-step runs skip it the same way.
//! - Queued "thread" work (world generation, load, save) runs on a fiber of its own, as natively: a JSPI stack with
//!   its own shadow stack (terraria-host `threading::wasm_fibers`, the page's web/www/lib/fibers.js). It gives the
//!   frame back after `WORK_SLICE_MS` (`YieldMode::Millis`), so the loading screen animates and the sound goes on.
//!   While work is suspended no collector mark phase ends (`gc_step(.., may_finish = false)`).
//! - The game's files: `/install` is the player's own install as the page imported it (Content + Terraria.exe),
//!   `/saves` the save root; both through WASI over the browser's private file system (web/www/wasi.js). The
//!   embedded resources are read from `/install/Terraria.exe` itself (`rt::resources::install_from_assembly`).
//! - Sound: the native client's mixer (`terraria_host::audio::mixer`), run here inside `tw_frame` instead of on a
//!   thread: every frame's audio commands go to it in order, then it mixes as many frames as `tw.audio_free` asks
//!   for and hands them to the AudioWorklet (`tw.audio_push`, 44,100 Hz stereo f32). It mixes in the native
//!   thread's 1,024-frame blocks (`BlockReader`), so the samples are the native ones for the same commands; the
//!   page paces `audio_free` to real time, also before the AudioContext exists (HOST_ABI.md, "Audio").
//! - Multiplayer: a `TcpClient`'s connection is a WebSocket to a local bridge (tools/ws-bridge), which opens the
//!   TCP connection the game asked for (`tw.net_*`, HOST_ABI.md "Network"; terraria-host `sys::net::wasm_net`).
//!   The game still types an address and a port; the page knows where the bridge is (`?bridge=`, or the header
//!   field). The network thread of the game (`Netplay.TcpClientLoop`) runs as a state machine made by the
//!   translator (`gen::resumable`): one poll per pump on a fiber, parked between packets with no stack, so the
//!   collector's mark phases end while connected (WEB_PORT.md 2.2).
//! - Host & Play: the game's `Process.Start("TerrariaServer.exe")` asks the page for a second Worker, which runs this
//!   same module as the dedicated server ([`server`]: `tw_server_init` / `tw_server_tick` instead of `tw_init` /
//!   `tw_frame`). The server's listener is the page's; the hosting client reaches it inside the page, other players
//!   through the bridge in reverse mode (HOST_ABI.md "Network", "Server").

mod frame;
pub mod input;
pub mod server;

/// wasm: when an allocation made the linear memory grow, the memory grows by this much more at once (an allocation
/// of this size, freed at once: it stays with the allocator as free space at the top). wasi-libc's malloc grows the
/// memory by exactly what one allocation needs, and every `memory.grow` costs V8 about 1.3 ms whatever its size
/// (web/spikes/memory-grow.mjs: 4,576 grows of 256 KB took 6.1 s, 18 grows of 64 MB 24 ms): the title's content loading spent
/// 11.6 of 34.9 s in `sbrk` (CPU profile, work/logs/title_load_profile.txt).
#[cfg(target_family = "wasm")]
const GROW_STEP: usize = 64 << 20;

#[cfg(target_family = "wasm")]
#[inline]
fn pages() -> usize {
    core::arch::wasm32::memory_size(0)
}
#[cfg(target_family = "wasm")]
#[cold]
fn grow_ahead() {
    use std::alloc::GlobalAlloc as _;
    let l = std::alloc::Layout::from_size_align(GROW_STEP, 16).unwrap();
    // SAFETY: a fresh block of a valid layout, freed at once; not counted (no one uses it).
    unsafe {
        let q = std::alloc::System.alloc(l);
        if !q.is_null() {
            std::alloc::System.dealloc(q, l);
        }
    }
}

/// Counts the bytes the module has allocated (live and peak): wasm memory only grows, so its size says nothing about
/// what is in use; these do (logged every 300 ticks).
struct Counting;
static LIVE: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
static PEAK: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
/// Live bytes and blocks by size class (class k: sizes in [2^k, 2^(k+1))): where the memory is (logged with the
/// counters).
static CLASS_BYTES: [std::sync::atomic::AtomicUsize; 33] = [const { std::sync::atomic::AtomicUsize::new(0) }; 33];
static CLASS_COUNT: [std::sync::atomic::AtomicUsize; 33] = [const { std::sync::atomic::AtomicUsize::new(0) }; 33];
fn class_of(n: usize) -> usize {
    (usize::BITS - 1 - n.max(1).leading_zeros()) as usize
}
fn class_add(n: usize) {
    let k = class_of(n);
    CLASS_BYTES[k].fetch_add(n, std::sync::atomic::Ordering::Relaxed);
    CLASS_COUNT[k].fetch_add(1, std::sync::atomic::Ordering::Relaxed);
}
fn class_sub(n: usize) {
    let k = class_of(n);
    CLASS_BYTES[k].fetch_sub(n, std::sync::atomic::Ordering::Relaxed);
    CLASS_COUNT[k].fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
}
/// The size classes from 64 KB up that hold memory: "class: blocks / MB".
fn classes_summary() -> String {
    let mut out = String::new();
    for k in 16..33 {
        let (c, b) = (CLASS_COUNT[k].load(std::sync::atomic::Ordering::Relaxed), CLASS_BYTES[k].load(std::sync::atomic::Ordering::Relaxed));
        if c > 0 {
            let lo = 1usize << k;
            let label = if lo >= 1 << 20 { format!("{}M", lo >> 20) } else { format!("{}K", lo >> 10) };
            out.push_str(&format!("{label}+: {c} / {:.1} MB; ", mb(b)));
        }
    }
    let small: usize = (0..16).map(|k| CLASS_BYTES[k].load(std::sync::atomic::Ordering::Relaxed)).sum();
    out.push_str(&format!("under 64K: {:.1} MB", mb(small)));
    out
}
/// The allocator under the counters. wasm: blocks of up to 64 bytes (alignment up to 16) come from a pool of
/// size classes of 16, 32, 48 and 64 bytes ([`small`]); larger ones from the system allocator (wasi-libc's dlmalloc),
/// which grows the memory ahead ([`grow_ahead`]). Elsewhere: the system allocator.
mod raw {
    use std::alloc::{GlobalAlloc, Layout, System};

    #[cfg(target_family = "wasm")]
    pub unsafe fn alloc(l: Layout, zeroed: bool) -> *mut u8 {
        if let Some(c) = super::small::class(l) {
            let p = super::small::alloc(c);
            if zeroed && !p.is_null() {
                core::ptr::write_bytes(p, 0, super::small::size(c));
            }
            return p;
        }
        let before = super::pages();
        let p = if zeroed { System.alloc_zeroed(l) } else { System.alloc(l) };
        if super::pages() != before {
            super::grow_ahead();
        }
        p
    }
    #[cfg(target_family = "wasm")]
    pub unsafe fn dealloc(p: *mut u8, l: Layout) {
        match super::small::class(l) {
            Some(c) => super::small::free(p, c),
            None => System.dealloc(p, l),
        }
    }
    #[cfg(target_family = "wasm")]
    pub unsafe fn realloc(p: *mut u8, l: Layout, new: usize) -> *mut u8 {
        let nl = Layout::from_size_align_unchecked(new, l.align());
        let (from, to) = (super::small::class(l), super::small::class(nl));
        if from.is_none() && to.is_none() {
            let before = super::pages();
            let q = System.realloc(p, l, new);
            if super::pages() != before {
                super::grow_ahead();
            }
            return q;
        }
        if from.is_some() && from == to {
            return p;
        }
        let q = alloc(nl, false);
        if !q.is_null() {
            core::ptr::copy_nonoverlapping(p, q, l.size().min(new));
            dealloc(p, l);
        }
        q
    }

    #[cfg(not(target_family = "wasm"))]
    pub unsafe fn alloc(l: Layout, zeroed: bool) -> *mut u8 {
        if zeroed {
            System.alloc_zeroed(l)
        } else {
            System.alloc(l)
        }
    }
    #[cfg(not(target_family = "wasm"))]
    pub unsafe fn dealloc(p: *mut u8, l: Layout) {
        System.dealloc(p, l)
    }
    #[cfg(not(target_family = "wasm"))]
    pub unsafe fn realloc(p: *mut u8, l: Layout, new: usize) -> *mut u8 {
        System.realloc(p, l, new)
    }
}

/// wasm: a pool for small blocks. dlmalloc keeps a 4-byte size word per block and rounds blocks to 16 bytes, so a
/// collector object of 16 + 14 bytes (a `Tile`: the header and its 14 bytes of fields) takes 48 bytes; here it takes
/// 32. A large world has 20 million tiles and about 39 million objects in all. Free blocks go on a free list per
/// class and are used again by blocks of that class only (as the collector frees and makes objects of the same
/// kinds); slabs are never given back (wasm memory never shrinks anyway). One thread (wasm32-wasip1): no locking.
#[cfg(target_family = "wasm")]
mod small {
    use std::alloc::{GlobalAlloc, Layout, System};

    const CLASSES: usize = 4;
    const SLAB: usize = 1 << 20;
    struct Pool {
        free: [*mut u8; CLASSES],
        next: [usize; CLASSES],
        end: [usize; CLASSES],
        slabs: usize,
    }
    struct Cell(core::cell::UnsafeCell<Pool>);
    // SAFETY: wasm32-wasip1 has one thread.
    unsafe impl Sync for Cell {}
    static POOL: Cell = Cell(core::cell::UnsafeCell::new(Pool { free: [core::ptr::null_mut(); CLASSES], next: [0; CLASSES], end: [0; CLASSES], slabs: 0 }));

    #[inline]
    pub fn class(l: Layout) -> Option<usize> {
        (l.size() <= CLASSES * 16 && l.align() <= 16).then(|| (l.size().max(1) + 15) / 16 - 1)
    }
    #[inline]
    pub fn size(c: usize) -> usize {
        (c + 1) * 16
    }
    /// Bytes in slabs (used and free).
    pub fn slab_bytes() -> usize {
        // SAFETY: one thread; a plain read.
        unsafe { (*POOL.0.get()).slabs * SLAB }
    }
    pub unsafe fn alloc(c: usize) -> *mut u8 {
        let pool = &mut *POOL.0.get();
        let p = pool.free[c];
        if !p.is_null() {
            pool.free[c] = *(p as *mut *mut u8);
            return p;
        }
        let n = size(c);
        if pool.next[c] + n > pool.end[c] {
            let before = super::pages();
            let s = System.alloc(Layout::from_size_align_unchecked(SLAB, 16));
            if super::pages() != before {
                super::grow_ahead();
            }
            if s.is_null() {
                return s;
            }
            pool.slabs += 1;
            pool.next[c] = s as usize;
            pool.end[c] = s as usize + SLAB;
        }
        let p = pool.next[c] as *mut u8;
        pool.next[c] += n;
        p
    }
    pub unsafe fn free(p: *mut u8, c: usize) {
        let pool = &mut *POOL.0.get();
        *(p as *mut *mut u8) = pool.free[c];
        pool.free[c] = p;
    }
}

fn count_alloc(n: usize) {
    let live = LIVE.fetch_add(n, std::sync::atomic::Ordering::Relaxed) + n;
    PEAK.fetch_max(live, std::sync::atomic::Ordering::Relaxed);
    class_add(n);
}
fn count_free(n: usize) {
    LIVE.fetch_sub(n, std::sync::atomic::Ordering::Relaxed);
    class_sub(n);
}

// SAFETY: forwards to `raw` (the system allocator, and on wasm the small-block pool); counts.
unsafe impl std::alloc::GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: std::alloc::Layout) -> *mut u8 {
        let p = raw::alloc(l, false);
        if !p.is_null() {
            count_alloc(l.size());
        }
        p
    }
    unsafe fn alloc_zeroed(&self, l: std::alloc::Layout) -> *mut u8 {
        let p = raw::alloc(l, true);
        if !p.is_null() {
            count_alloc(l.size());
        }
        p
    }
    unsafe fn dealloc(&self, p: *mut u8, l: std::alloc::Layout) {
        count_free(l.size());
        raw::dealloc(p, l)
    }
    unsafe fn realloc(&self, p: *mut u8, l: std::alloc::Layout, new: usize) -> *mut u8 {
        let q = raw::realloc(p, l, new);
        if !q.is_null() {
            count_free(l.size());
            count_alloc(new);
        }
        q
    }
}
#[cfg(target_family = "wasm")]
#[global_allocator]
static ALLOC: Counting = Counting;

fn mb(n: usize) -> f64 {
    n as f64 / 1048576.0
}

use std::cell::RefCell;

use terraria_gen_game::gen::terraria;
use terraria_gen_game::host::launch;
use terraria_gen_game::rt::{self, Gc, Rooted};
use terraria_host::audio::mixer::{AudioStats, BlockReader, Mixer};
use terraria_host::audio::AudioCommand;
use terraria_host::game::{self, GameTime, WindowState};

#[cfg(target_family = "wasm")]
#[link(wasm_import_module = "tw")]
extern "C" {
    #[link_name = "gfx_frame"]
    fn tw_gfx_frame(ptr: *const u8, len: usize);
    #[link_name = "gfx_frame_chunk"]
    fn tw_gfx_frame_chunk(ptr: *const u8, len: usize);
    #[link_name = "gfx_texture"]
    fn tw_gfx_texture(id: u32, width: u32, height: u32, ptr: *const u8, len: usize);
    #[link_name = "input_next"]
    fn tw_input_next(out: *mut u8, cap: usize) -> i32;
    #[link_name = "log"]
    fn tw_log(level: i32, ptr: *const u8, len: usize);
    #[link_name = "audio_free"]
    fn tw_audio_free() -> i32;
    #[link_name = "audio_push"]
    fn tw_audio_push(ptr: *const f32, frames: usize) -> i32;
    // work fibers (HOST_ABI.md "Fibers"; terraria-host sys/threading.rs `wasm_fibers`)
    #[link_name = "fiber_resume"]
    fn tw_fiber_resume(handle: usize, stack_top: usize) -> i32;
    #[link_name = "fiber_suspend"]
    fn tw_fiber_suspend(handle: usize);
    #[link_name = "fiber_forget"]
    fn tw_fiber_forget(handle: usize);
    // the main stack gives the display frame back (HOST_ABI.md "Frames"; terraria-host threading `main_yield`)
    #[link_name = "yield_frame"]
    fn tw_yield_frame() -> f64;
    // connections through the WebSocket bridge (HOST_ABI.md "Network"; terraria-host sys/net.rs `wasm_net`)
    #[link_name = "net_connect"]
    fn tw_net_connect(host: *const u8, len: usize, port: i32) -> i32;
    #[link_name = "net_status"]
    fn tw_net_status(handle: i32) -> i32;
    #[link_name = "net_send"]
    fn tw_net_send(handle: i32, ptr: *const u8, len: usize) -> i32;
    #[link_name = "net_recv"]
    fn tw_net_recv(handle: i32, ptr: *mut u8, cap: usize, peek: i32) -> i32;
    #[link_name = "net_close"]
    fn tw_net_close(handle: i32) -> i32;
    // the page's listeners and its dedicated server Worker (HOST_ABI.md "Network", "Server")
    #[link_name = "net_listen"]
    fn tw_net_listen(port: i32) -> i32;
    #[link_name = "net_accept"]
    fn tw_net_accept(listener: i32) -> i32;
    #[link_name = "net_unlisten"]
    fn tw_net_unlisten(listener: i32) -> i32;
    #[link_name = "net_peer"]
    fn tw_net_peer(handle: i32, out: *mut u8, cap: usize) -> i32;
    #[link_name = "server_start"]
    fn tw_server_start(args: *const u8, len: usize) -> i32;
    #[link_name = "server_status"]
    fn tw_server_status(id: i32) -> i32;
    #[link_name = "server_kill"]
    fn tw_server_kill(id: i32) -> i32;
    // the game window's title and the game's mouse (HOST_ABI.md "Window"): the page's document title, its cursor style,
    // and the cursor-mapping tests
    #[link_name = "window_title"]
    fn tw_window_title(ptr: *const u8, len: usize);
    #[link_name = "frame_info"]
    fn tw_frame_info(mouse_x: i32, mouse_y: i32, flags: i32);
}

/// After each presented frame: the window title when it changed (`Game.Window.Title`, set by the game), and
/// `PlayerInput.MouseX` / `MouseY`, `IsMouseVisible` (bit 0), `Main.gameMenu` (bit 1) and `Main.showSplash` (bit 2: the
/// Re-Logic splash and the content loading behind it).
fn report_window(d: &mut Driver, title: &str, mouse_visible: bool) {
    if title != d.title {
        d.title = title.to_owned();
        #[cfg(target_family = "wasm")]
        // SAFETY: as `log`.
        unsafe {
            tw_window_title(d.title.as_ptr(), d.title.len())
        }
    }
    // SAFETY: the game's statics, read between frames on the game thread.
    // PlayerInput.MouseX / MouseY: the mouse in back-buffer pixels (Main.mouseX is the same divided by the UI scale
    // while the UI is drawn, so it is not the pixel under the pointer)
    let (mx, my, menu, splash) = unsafe { ((*terraria::game_input::player_input::s()).mouse_x, (*terraria::game_input::player_input::s()).mouse_y, (*terraria::main::s()).game_menu, (*terraria::main::s()).show_splash) };
    let flags = mouse_visible as i32 | (menu as i32) << 1 | (splash as i32) << 2;
    #[cfg(target_family = "wasm")]
    // SAFETY: as `log`.
    unsafe {
        tw_frame_info(mx, my, flags)
    }
    #[cfg(not(target_family = "wasm"))]
    let _ = (mx, my, flags);
}

/// The bridge connections for terraria-host's `TcpClient`.
#[cfg(target_family = "wasm")]
fn net_backend() -> terraria_host::sys::net::wasm_net::Backend {
    unsafe fn connect(host: *const u8, len: usize, port: i32) -> i32 {
        tw_net_connect(host, len, port)
    }
    unsafe fn status(h: i32) -> i32 {
        tw_net_status(h)
    }
    unsafe fn send(h: i32, ptr: *const u8, len: usize) -> i32 {
        tw_net_send(h, ptr, len)
    }
    unsafe fn recv(h: i32, ptr: *mut u8, cap: usize, peek: i32) -> i32 {
        tw_net_recv(h, ptr, cap, peek)
    }
    unsafe fn close(h: i32) -> i32 {
        tw_net_close(h)
    }
    unsafe fn listen(port: i32) -> i32 {
        tw_net_listen(port)
    }
    unsafe fn accept(l: i32) -> i32 {
        tw_net_accept(l)
    }
    unsafe fn unlisten(l: i32) -> i32 {
        tw_net_unlisten(l)
    }
    unsafe fn peer(h: i32, out: *mut u8, cap: usize) -> i32 {
        tw_net_peer(h, out, cap)
    }
    unsafe fn server_start(args: *const u8, len: usize) -> i32 {
        tw_server_start(args, len)
    }
    unsafe fn server_status(id: i32) -> i32 {
        tw_server_status(id)
    }
    unsafe fn server_kill(id: i32) -> i32 {
        tw_server_kill(id)
    }
    terraria_host::sys::net::wasm_net::Backend { connect, status, send, recv, close, listen, accept, unlisten, peer, server_start, server_status, server_kill }
}

/// The body of a work fiber: the page calls this export through `WebAssembly.promising` (a new engine stack) with
/// `__stack_pointer` set to the fiber's shadow stack, from inside `tw.fiber_resume` (HOST_ABI.md "Fibers").
#[cfg(target_family = "wasm")]
#[no_mangle]
pub extern "C" fn tw_fiber_entry(handle: usize) {
    terraria_host::sys::threading::wasm_fibers::entry(handle)
}

/// The JSPI stack switch for terraria-host's work fibers.
#[cfg(target_family = "wasm")]
fn fiber_backend() -> terraria_host::sys::threading::wasm_fibers::Backend {
    unsafe fn resume(handle: usize, stack_top: usize) -> i32 {
        tw_fiber_resume(handle, stack_top)
    }
    unsafe fn suspend(handle: usize) {
        tw_fiber_suspend(handle)
    }
    unsafe fn forget(handle: usize) {
        tw_fiber_forget(handle)
    }
    terraria_host::sys::threading::wasm_fibers::Backend { resume, suspend, forget }
}

/// Time slice of queued work (world generation, loading, saving) per pump: the rest of the display frame draws the
/// loading screen and mixes the sound. The native shell uses the same 8 ms.
const WORK_SLICE_MS: u64 = 8;

/// The main stack's slice: game code on the main stack (tw_init, a frame's Update / Draw / pumped work) that runs
/// longer than this gives the display frame back at its next yield point (terraria-host threading `main_yield`):
/// the title's content loading and the first frame in a world no longer freeze the page for seconds. Ordinary
/// frames are far shorter, so they are never cut (a cut costs a display tick).
const MAIN_SLICE_MS: u64 = 50;

/// `main_yield`'s hook: the sound goes on, then the whole main stack waits for the page's next tick
/// (`tw.yield_frame`, a JSPI suspension). Nothing else runs meanwhile, so for the game this is one slow frame.
#[cfg(target_family = "wasm")]
fn main_yield_hook() {
    AUDIO.with(|a| {
        if let Ok(mut a) = a.try_borrow_mut() {
            if let Some(a) = a.as_mut() {
                a.pump();
            }
        }
    });
    // SAFETY: a suspending import with no arguments; called on the main stack inside tw_init / tw_frame only
    // (the window is open only there), i.e. under the promising exports.
    unsafe {
        tw_yield_frame();
    }
}

/// Times the main stack gave the display frame back so far.
fn main_yields() -> u64 {
    #[cfg(target_family = "wasm")]
    return terraria_host::sys::threading::main_yield::count();
    #[cfg(not(target_family = "wasm"))]
    0
}

/// Opens / closes the window in which the main stack may give the frame back (no-op off wasm).
fn main_window(on: bool) {
    #[cfg(target_family = "wasm")]
    terraria_host::sys::threading::main_yield::window(on);
    #[cfg(not(target_family = "wasm"))]
    let _ = on;
}

/// A line for the page's log panel and the console (level: 0 trace .. 4 error).
pub fn log(level: i32, text: &str) {
    #[cfg(target_family = "wasm")]
    // SAFETY: a valid byte range of this module's memory for the duration of the call.
    unsafe {
        tw_log(level, text.as_ptr(), text.len())
    }
    #[cfg(not(target_family = "wasm"))]
    eprintln!("[web {level}] {text}");
}

/// One part of the frame being sent (FRAME_FORMAT.md v2): `last` replays it.
fn send_frame(bytes: &[u8], last: bool) {
    #[cfg(target_family = "wasm")]
    // SAFETY: as `log`.
    unsafe {
        if last {
            tw_gfx_frame(bytes.as_ptr(), bytes.len())
        } else {
            tw_gfx_frame_chunk(bytes.as_ptr(), bytes.len())
        }
    }
    #[cfg(not(target_family = "wasm"))]
    let _ = (bytes, last);
}

fn drain_input(input: &mut input::Input) {
    #[cfg(target_family = "wasm")]
    {
        let mut buf = [0u8; 32 * 128];
        loop {
            // SAFETY: the buffer is ours and `cap` bytes long.
            let n = unsafe { tw_input_next(buf.as_mut_ptr(), buf.len()) };
            if n <= 0 {
                break;
            }
            for r in buf[..n as usize].chunks_exact(32) {
                input.apply(r);
            }
        }
    }
    #[cfg(not(target_family = "wasm"))]
    let _ = input;
}

/// The mixer and what it has not handed over yet (`pending`: interleaved stereo frames mixed but refused by a full
/// buffer; they go first next time, so no sample is lost or repeated).
struct Audio {
    reader: BlockReader,
    stats: AudioStats,
    buf: Vec<f32>,
    pending: usize,
    pushed: u64,
}

impl Audio {
    fn new() -> Audio {
        let stats = AudioStats::default();
        stats.format.store(1, std::sync::atomic::Ordering::Relaxed); // f32 to the AudioWorklet
        Audio { reader: BlockReader::new(Mixer::new(1.0)), stats, buf: Vec::new(), pending: 0, pushed: 0 }
    }
    fn submit(&mut self, commands: Vec<AudioCommand>) {
        for c in commands {
            self.reader.mixer.apply(c, &self.stats);
        }
    }
    /// Mixes and pushes as many frames as the page asks for.
    fn pump(&mut self) {
        #[cfg(target_family = "wasm")]
        {
            // SAFETY: plain imports, numbers only.
            let free = unsafe { tw_audio_free() }.max(0) as usize;
            if free == 0 {
                return;
            }
            if self.buf.len() < free * 2 {
                self.buf.resize(free * 2, 0.0);
            }
            if self.pending < free {
                let (from, to) = (self.pending * 2, free * 2);
                self.reader.read(&mut self.buf[from..to], &self.stats);
                self.pending = free;
            }
            // SAFETY: `buf` is a Vec<f32> (4-byte aligned) holding at least `pending` stereo frames.
            let took = unsafe { tw_audio_push(self.buf.as_ptr(), self.pending) };
            if took < 0 {
                log(3, &format!("[web] audio_push: error {took}"));
                return;
            }
            let took = (took as usize).min(self.pending);
            self.buf.copy_within(took * 2..self.pending * 2, 0);
            self.pending -= took;
            self.pushed += took as u64;
        }
    }
}

struct Driver {
    main: Rooted<Gc<terraria::main::Main>>,
    input: input::Input,
    encoder: frame::Encoder,
    size: (i32, i32),
    last_ms: Option<f64>,
    tick: u64,
    last_gc_tick: u64,
    failed: bool,
    seen: Vec<String>,
    /// (`Main.gameMenu`, `Main.netMode`) at the end of the last frame: a change is logged (scripted runs wait for it)
    mode: (bool, i32),
    /// the window title last reported to the page
    title: String,
}

thread_local! {
    static DRIVER: RefCell<Option<Driver>> = const { RefCell::new(None) };
    /// The mixer, apart from the driver: the main stack's yield hook pumps it while the driver is borrowed.
    static AUDIO: RefCell<Option<Audio>> = const { RefCell::new(None) };
}

fn with_audio(f: impl FnOnce(&mut Audio)) {
    AUDIO.with(|a| {
        if let Some(a) = a.borrow_mut().as_mut() {
            f(a)
        }
    })
}

/// Runs translated code as `Program.RunGame`'s `try { .. } catch (Exception e) { DisplayException(e); }` surrounds
/// it (bin/terraria.rs `guarded`): an uncaught C# exception or a stop is logged, written to
/// `/saves/port-crashlog.txt` once per text, and the game goes on (a normal run, not a scripted one). `false`: it failed.
fn guarded(seen: &mut Vec<String>, what: &str, f: impl FnOnce()) -> bool {
    let text = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| rt::try_(f))) {
        Ok(Ok(())) => return true,
        Ok(Err(e)) => format!("C# exception in {what}: {}", rt::exc_text(e)),
        Err(p) => format!("stopped in {what}: {}", p.downcast_ref::<String>().cloned().or_else(|| p.downcast_ref::<&str>().map(|s| (*s).to_owned())).unwrap_or_default()),
    };
    if !seen.contains(&text) {
        log(4, &text);
        let mut entry = format!("{text}\nlast lines of the host log:\n");
        for l in terraria_gen_rt::host::log_tail() {
            entry.push_str("    ");
            entry.push_str(&l);
            entry.push('\n');
        }
        use std::io::Write as _;
        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open("/saves/port-crashlog.txt") {
            let _ = f.write_all(entry.as_bytes());
        }
        seen.push(text);
    }
    false
}

fn window_state(size: (i32, i32), active: bool) -> WindowState {
    WindowState { width: size.0, height: size.1, x: 0, y: 0, active, handle: 0, display_width: size.0, display_height: size.1, scale: 1.0 }
}

/// The embedded resources of the player's own Terraria.exe (`/install/Terraria.exe`); `Err(1)` when it cannot be read.
fn install_resources_from_install() -> Result<(), i32> {
    let exe = match std::fs::read("/install/Terraria.exe") {
        Ok(b) => b,
        Err(e) => {
            log(4, &format!("cannot read /install/Terraria.exe (import the game folder again): {e}"));
            return Err(1);
        }
    };
    match rt::resources::install_from_assembly(exe) {
        Ok(n) => {
            log(2, &format!("{n} embedded resources read from the player's Terraria.exe"));
            Ok(())
        }
        Err(e) => {
            log(4, &format!("/install/Terraria.exe: {e}"));
            Err(1)
        }
    }
}

/// What the client and the server Worker install alike: work fibers, resumable threads, the work slice, the network.
fn install_platform() {
    // queued work runs on fibers of its own (JSPI) and gives the frame back after a time slice: the loading screen
    // animates and the sound goes on while a world is generated or loaded
    #[cfg(target_family = "wasm")]
    terraria_host::sys::threading::wasm_fibers::install(Some(fiber_backend()));
    // the network threads (the client's Netplay.TcpClientLoop, the server's ServerLoop / ListenLoop /
    // BroadcastThread) never end while connected: they run as the translator's state machines, which park at every
    // sleep instead of keeping a suspended stack, so collector cycles end during a session (terraria-host threading
    // "Resumable threads", tools/cs2rs DESIGN.md 8f)
    #[cfg(target_family = "wasm")]
    terraria_host::sys::threading::set_resumable_threads(terraria_gen_game::gen::resumable::THREADS);
    terraria_host::sys::threading::set_yield_mode(terraria_host::sys::threading::YieldMode::Millis(WORK_SLICE_MS));
    // Join via IP: TcpClient connections go through the page's WebSocket bridge (or, to the page's own server, through
    // the page); a server's listener is the page's
    #[cfg(target_family = "wasm")]
    terraria_host::sys::net::wasm_net::install(Some(net_backend()));
}

/// The client runs in this instance (`tw_init` was called).
fn client_started() -> bool {
    DRIVER.with(|d| d.borrow().is_some())
}

/// Bytes the module has allocated and not freed / the most at any time.
fn live_bytes() -> usize {
    LIVE.load(std::sync::atomic::Ordering::Relaxed)
}
fn peak_bytes() -> usize {
    PEAK.load(std::sync::atomic::Ordering::Relaxed)
}

/// `tw_init(width, height)`: `Game.Run` up to the first presented frame. 0: started; 1: the install has no readable
/// Terraria.exe; 2: the game could not be created; 3: called twice.
#[no_mangle]
pub extern "C" fn tw_init(width: i32, height: i32) -> i32 {
    if DRIVER.with(|d| d.borrow().is_some()) {
        return 3;
    }
    rt::enter();
    if let Err(code) = install_resources_from_install() {
        return code;
    }
    let size = (width.max(1), height.max(1));
    game::init(window_state(size, true));
    // the game's ApplyChanges size becomes the window's (as the native shell's real window): otherwise the game sees
    // a window of another size every frame, applies its own again and rebuilds its render targets each time (which
    // also restarts the lighting engine: the world was drawn unlit)
    game::set_window_follows_apply_changes(true);
    install_platform();
    // long main-stack stretches give the display frame back too (the title's content loading)
    AUDIO.with(|a| *a.borrow_mut() = Some(Audio::new()));
    #[cfg(target_family = "wasm")]
    terraria_host::sys::threading::main_yield::install(Some(main_yield_hook), MAIN_SLICE_MS);
    // the pixels of a new content texture go to the replayer at once (HOST_ABI gfx_texture), not in the frame
    #[cfg(target_family = "wasm")]
    terraria_host::gfx::draw_list::set_upload_sink(Some(Box::new(|id, w, h, rgba: &[u8]| unsafe { tw_gfx_texture(id.0, w, h, rgba.as_ptr(), rgba.len()) })));
    // the original writes a few files by relative path (client-crashlog.txt): they land in the save root
    let _ = std::fs::create_dir_all("/saves");
    let _ = std::env::set_current_dir("/saves");
    let mut seen = Vec::new();
    let mut main = Gc::NULL;
    main_window(true);
    guarded(&mut seen, "Program.LaunchGame", || unsafe { main = launch::launch_game(&[]) });
    if main.is_null() {
        main_window(false);
        return 2;
    }
    let main = Rooted::new(main);
    let m = main.get();
    guarded(&mut seen, "Main.Initialize", || unsafe { launch::initialize(m) });
    // the first Update, with ElapsedGameTime zero (Game.cs:260-263)
    let t: Gc<GameTime> = game::roots().game_time;
    guarded(&mut seen, "Main.Update", || unsafe { terraria::main::update(m.ptr(), t) });
    terraria_host::sys::threading::run_pending_work();
    main_window(false);
    let mut d = Driver { main, input: input::Input::new(), encoder: frame::Encoder::default(), size, last_ms: None, tick: 0, last_gc_tick: 0, failed: false, seen, mode: (true, 0), title: String::new() };
    let out = game::end_draw(false);
    if let Some(r) = &out.window_request {
        d.size = (r.width.max(1), r.height.max(1));
    }
    d.encoder.encode(&out.frame, &mut send_frame);
    report_window(&mut d, &out.title, out.is_mouse_visible);
    with_audio(|a| {
        a.submit(out.audio);
        a.pump();
    });
    log(2, &format!("[web] tw_init done: allocated {:.1} MB live, {:.1} MB peak", mb(LIVE.load(std::sync::atomic::Ordering::Relaxed)), mb(PEAK.load(std::sync::atomic::Ordering::Relaxed))));
    DRIVER.with(|x| *x.borrow_mut() = Some(d));
    0
}

/// `tw_frame(now_ms)`: one `Game.Tick` and its presented frame.
#[no_mangle]
pub extern "C" fn tw_frame(now_ms: f64) {
    DRIVER.with(|x| {
        let mut b = x.borrow_mut();
        let Some(d) = b.as_mut() else { return };
        if d.failed {
            return;
        }
        d.tick += 1;
        let elapsed = match d.last_ms {
            Some(l) => ((now_ms - l) / 1000.0).clamp(0.0, 0.5),
            None => 0.0,
        };
        d.last_ms = Some(now_ms);
        drain_input(&mut d.input);
        if let Some(s) = d.input.size.take() {
            d.size = (s.0.max(1), s.1.max(1));
        }
        let snapshot = d.input.snapshot();
        let ws = window_state(d.size, d.input.active);
        let m = d.main.get();
        let seen = &mut d.seen;
        main_window(true);
        let t0 = std::time::Instant::now();
        let yields0 = main_yields();
        game::begin_frame(snapshot, ws, elapsed);
        game::dispatch_events();
        while let Some(t) = game::begin_update() {
            guarded(seen, "Main.Update", || unsafe { terraria::main::update(m.ptr(), t) });
            if !guarded(seen, "queued thread work (game::end_update)", game::end_update) {
                main_window(false);
                d.failed = true;
                return;
            }
        }
        let t_update = t0.elapsed();
        let drew = match game::begin_draw() {
            Some(t) => {
                if !guarded(seen, "Main.Draw", || unsafe { terraria::main::draw(m.ptr(), t) }) {
                    // put the device back so that the next frames can be drawn (bin/terraria.rs survey rule)
                    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        let _ = rt::try_(|| unsafe { (*game::graphics_device().ptr()).set_render_target(Gc::NULL) });
                        let sb = unsafe { (*terraria::main::s()).sprite_batch };
                        if !sb.is_null() {
                            let _ = rt::try_(|| unsafe { (*sb.ptr()).end() });
                        }
                    }));
                    unsafe { (*m.ptr())._is_drawing_or_updating = false };
                }
                true
            }
            None => false,
        };
        let t_draw = t0.elapsed();
        let mut out = None;
        if !guarded(seen, "Present (game::end_draw)", || out = Some(game::end_draw(drew))) {
            main_window(false);
            d.failed = true;
            return;
        }
        main_window(false);
        if let Some(out) = out {
            if let Some(r) = &out.window_request {
                d.size = (r.width.max(1), r.height.max(1));
            }
            d.encoder.encode(&out.frame, &mut send_frame);
            report_window(d, &out.title, out.is_mouse_visible);
            with_audio(|a| a.submit(out.audio));
            if out.exit {
                d.failed = true;
                log(2, "the game asked to exit");
            }
        }
        with_audio(|a| a.pump());
        // a frame over 100 ms of the module's time: where it went, and the loading screen's text (a world generation
        // pass names itself there)
        let total = t0.elapsed();
        if total.as_millis() > 100 {
            // SAFETY: the game's statics, between frames on the game thread
            let status = unsafe { (*terraria::main::s()).status_text };
            let status = if status.is_null() { "" } else { status.as_str() };
            log(2, &format!("[web] tick {}: slow frame {} ms (update + work {} ms, draw {} ms, present + encode {} ms; {} main-stack yields); status \"{}\"", d.tick, total.as_millis(), t_update.as_millis(), (t_draw - t_update).as_millis(), (total - t_draw).as_millis(), main_yields() - yields0, status.trim()));
        }
        // SAFETY: the game's statics, read between frames on the game thread.
        let mode = unsafe { ((*terraria::main::s()).game_menu, (*terraria::main::s()).net_mode) };
        if mode != d.mode {
            d.mode = mode;
            let what = if mode.0 { "the menu" } else { "the world" };
            let net = match mode.1 {
                0 => "single player",
                1 => "multiplayer client",
                _ => "server",
            };
            log(2, &format!("[web] tick {}: now in {what} ({net}, Main.netMode {})", d.tick, mode.1));
        }
        if d.tick % 300 == 0 {
            let main_yields = main_yields();
            with_audio(|a| log(2, &format!("[web] tick {}: audio mixer: {}; {} frames pushed; work suspended {}, {} work yields, {} main-stack yields so far", d.tick, a.stats.summary(), a.pushed, terraria_host::sys::threading::has_suspended_work(), terraria_host::sys::threading::yields(), main_yields)));
            let (objects, bytes) = rt::heap_size();
            log(2, &format!("[web] tick {}: allocated {:.1} MB live, {:.1} MB peak; collector heap {objects} objects, {:.1} MB; {} collections; phase {:?}; allocated since the last cycle {:.1} MB", d.tick, mb(LIVE.load(std::sync::atomic::Ordering::Relaxed)), mb(PEAK.load(std::sync::atomic::Ordering::Relaxed)), mb(bytes), rt::collection_count(), rt::gc_phase(), mb(rt::allocated_since_collect())));
            #[cfg(target_family = "wasm")]
            let slabs = small::slab_bytes();
            #[cfg(not(target_family = "wasm"))]
            let slabs = 0;
            log(2, &format!("[web] tick {}: live blocks by size: {}; small-block pool {:.1} MB in slabs", d.tick, classes_summary(), mb(slabs)));
        }
        // the collector: a cycle after 48 MB of allocation or 3,600 ticks, 1.5 ms per tick while it runs (shell)
        let due = d.tick - d.last_gc_tick >= 3600 || rt::allocated_since_collect() >= 48 << 20;
        // SAFETY: between frames, on the game thread; no generated function is on the main stack. A work item
        // suspended on its fiber holds handles in frames no one can scan (a suspended JSPI stack, WEB_PORT.md 2.2):
        // marking and sweeping slices go on, but no mark phase may end while one exists (as the native shell
        // without conservative roots).
        let suspended = terraria_host::sys::threading::has_suspended_work();
        unsafe {
            if rt::gc_phase() == rt::GcPhase::Idle {
                if due {
                    d.last_gc_tick = d.tick;
                    rt::gc_begin();
                }
            } else {
                let step = rt::gc_step(rt::GcBudget::Micros(1500), !suspended);
                if let Some(c) = step.finished {
                    log(2, &format!("[web] tick {}: collector cycle {} finished: {} objects ({:.1} MB) live, {} objects ({:.1} MB) freed", d.tick, rt::collection_count(), c.live_objects, mb(c.live_bytes), c.freed_objects, mb(c.freed_bytes)));
                }
            }
        }
    });
}
