//! `Thread`, `ThreadPool`, `Task`, `TaskCompletionSource<T>`, `Monitor`, `Interlocked`, `ConcurrentQueue<T>`.
//!
//! # Model: one thread, deferred work
//! The translated game runs on one thread (`rt::enter`, DESIGN.md section 1). "Starting a thread" therefore QUEUES
//! the work; it runs later on the game thread, to completion, at one of these points:
//! - [`run_pending_work`], which the driver calls between `Update` calls (README.md, driver protocol);
//! - on demand, when the game asks for the result: `Task.Wait()`, `Task.IsCompleted`, `Thread.Join()`.
//!
//! Work is never run inside `Thread.Start` / `QueueUserWorkItem` / `StartNew` themselves: the game's callers go on
//! to set state the worker reads (e.g. `Main.menuMode = 10` after `ThreadPool.QueueUserWorkItem(playWorldCallBack)`).
//!
//! What this changes, by code path (decompile grep for `new Thread(`, `ThreadPool.QueueUserWorkItem`,
//! `Task.Factory.StartNew`):
//! - world load / save / generation (`WorldGen.playWorldCallBack`, `saveAndPlayCallBack`, `SaveAndQuitCallBack`,
//!   `CreateNewWorld`, `serverLoadWorldCallBack`): correct results, but the frame that pumps the work BLOCKS until
//!   it is done, so the loading screen / progress bar (drawn by the main thread while the worker runs) does not
//!   animate. No logic depends on the interleaving.
//! - `Main.RunOnMainThread(action)` followed by `.Wait()` from a worker: the action sits in the game's own
//!   `_mainThreadActions` queue, which only `Main.Update` drains; on one thread the wait can never be satisfied.
//!   `Task.Wait()` on an unfinished `TaskCompletionSource` task therefore panics with that explanation instead of
//!   hanging (`TaskCompletionSource` deadlock). Callers: asset loading off the main thread (not used with
//!   `AssetRequestMode.ImmediateLoad`, which the game's own code passes).
//! - wait loops (`while (flag) Thread.Sleep(..)` in a worker waiting for the main thread; `WorldGen.playWorldCallBack`
//!   waits like that for `Main.DoDraw` to draw the loaded map, `while (Main.loadMapLock) Thread.Sleep(0)`): on
//!   Windows every queued work item runs on a FIBER of the game thread (section "Fibers" below). After
//!   [`SLEEPS_PER_SLICE`] `Thread.Sleep` calls in one go the work is suspended, the frame goes on (`Update`,
//!   `Draw`), and the work continues at the next [`run_pending_work`]. Elsewhere (no fibers) `Thread.Sleep` inside
//!   pumped work panics after [`SLEEP_LIMIT`] calls in one work item, naming the model, instead of spinning.
//!
//! # Fibers (Windows)
//! A fiber is a second stack on the same OS thread: thread-local state, `rt::enter`'s owner check and the
//! "one thread runs generated code" contract all hold. Only one of them runs at a time and the switch points are
//! exactly two: the main fiber resumes a work item ([`run_pending_work`], `Task.Wait` ...), and a work item
//! gives control back (it finished, or it slept [`SLEEPS_PER_SLICE`] times). While a work item is suspended its
//! frames (with object handles in them) are alive on a stack the collector does not see, so the driver MUST NOT
//! collect then: [`has_suspended_work`] tells it (the app shell and `tests/client.rs` check it). A panic or a C#
//! exception that leaves the work is caught on the fiber and re-raised on the main fiber, at the resume point.
//! - `FastParallel.For` (lighting, world gen): the runtime's `rt::sys::fast_parallel_for` runs the range on the
//!   calling thread; results are identical because the game's ranges do not overlap.
//! - network / Steam / dedicated-server console threads (`Netplay`, `TcpSocket`, `CoreSocialModule`,
//!   `Main.startDedInputCallBack`): loops that never return; they must not be queued. Nothing here can tell them
//!   apart, so those subsystems need their own host design (not part of this layer). On wasm the client net loop
//!   and the dedicated-server console loop are translated as resumable state machines (game.json "resumable",
//!   DESIGN.md 8f) and park without a stack while they wait.
//!
//! - ReLogic's `AsyncActionDispatcher` (the asset loader's thread: `while (running) queue.Take()()`): a consumer
//!   thread that never returns, so it is a native class here ([`AsyncActionDispatcher`]): its queue is served on
//!   the game thread, at most [`dispatcher_budget`] actions per [`run_pending_work`] (the loader thread's
//!   throughput; the splash screen's progress depends on it), and on demand when the game busy-waits for the
//!   results (a second poll of an empty `ConcurrentQueue` in the same driver phase, see [`ConcurrentQueue`]).
//!
//! # Giving the frame back while work runs ([`set_yield_mode`])
//! Off by default (tests, tools): a work item runs until it finishes or waits in a sleep loop, as described above.
//! The app turns it on so that loading screens animate: the translator puts `rt::yield_point()` at the head of every
//! loop body, and while a work item runs on its fiber the host arms it: after a time slice ([`YieldMode::Millis`],
//! a timer thread sets the flag; it runs no game code) or after a fixed number of loop iterations
//! ([`YieldMode::Loops`], repeatable: scripted runs). The work item then switches to the main fiber, the frame goes
//! on (`Update`, `Draw` of the loading screen), and the next pump continues the work. This is the original's
//! concurrency (a worker thread next to the main thread) at coarser grain, with what real threads have:
//! - `[ThreadStatic]` fields have a value per work item (`rt::threads`: `Main.rand`, `Main._rngs`,
//!   `WorldGen.generatingWorldOnThisThread`): a work item starts with the defaults, as a new .NET thread, and the
//!   main thread's `Update` / `Draw` between its slices cannot disturb its random numbers. This holds with the
//!   yield mode off too.
//! - Monitors have owners ([`monitor_enter`]): the main fiber entering a lock that a suspended work item holds
//!   continues that work item until it leaves the lock; a work item gives the frame back until the lock is free.
//! - no yield inside a static initialiser (`rt::CctorGuard`).
//! What a yield cannot change: the work's own results. They depend on the work's code and its thread-static state
//! only, exactly as far as the original's worker does not read what the main thread writes meanwhile.
//!
//! `Interlocked` is plain arithmetic.
//!
//! # Threads that never end (networking)
//! `Netplay.ServerLoop`, `InnerClientLoop`, `TcpSocket.ListenLoop` and `Netplay.BroadcastThread` are loops on
//! threads of their own (`new Thread(..)`). They run as work items like everything else and need no special case:
//! each sleeps in every iteration, and a sleeping fiber is suspended.
//! - A fiber of a `Thread` object that sleeps one millisecond or more is suspended AT ONCE and is IDLE: it waits for
//!   time or for the network, not for the frame. Idle fibers do not stop a script's clock
//!   ([`has_suspended_foreground_work`]), and a sleep of 16 ms or more is honoured in real time (the LAN
//!   announcement sleeps a second between datagrams). `Thread.Sleep(0)` and the sleeps of pool work count towards
//!   [`SLEEPS_PER_SLICE`] as before.
//! - Every `Thread.Sleep` on a fiber first moves the sockets ([`super::net::pump`]) and runs the completion
//!   callbacks that became due inline (the original runs them on I/O threads while this thread sleeps): the client
//!   loop reads as fast as data arrives within its slice, not once per frame.
//! - A blocking wait of host code (`TcpListener.AcceptTcpClient`, `TcpClient.Connect`) is [`wait_a_frame`].
//! - `Thread.Abort()` ([`Task::abort`]) removes a suspended thread: its fiber is deleted where it stands (no
//!   `ThreadAbortException` runs its handlers; the one user, `Netplay.StopBroadCasting`, has none).
//! - A process whose main loop is the game's own (`Main.DedServ`: the dedicated server) calls
//!   [`set_blocking_main`]: `Thread.Sleep` on the main thread then pumps the work, runs the driver's idle hook
//!   (the collector) and really sleeps.
//!
//! # The collector and suspended fibers
//! A suspended fiber's frames hold object handles. [`suspended_stacks`] gives the collector their memory (from the
//! stack pointer at the switch, with the callee-saved registers spilled just above it, to the stack base) to scan
//! conservatively (`rt::set_conservative_roots`), so a collection can finish while work is suspended.
//!
//! # Fibers on wasm (the browser build)
//! The same structure on WebAssembly JSPI (web/WEB_PORT.md section 1, [`wasm_fibers`]): a work item runs on a
//! suspendable engine stack of its own (entered through a `WebAssembly.promising` export) with a shadow stack of its
//! own in linear memory; the main stack waits for it in a `WebAssembly.Suspending` import. The driver installs the
//! stack switch ([`wasm_fibers::install`]); without it work runs inline as on other platforms (the wasm unit tests).
//! A suspended wasm stack cannot be scanned (its locals are not in linear memory, WEB_PORT.md section 2.2):
//! [`suspended_stacks`] reports nothing for it, so no mark phase may end while work is suspended
//! (`gc_step(.., may_finish = false)`, the driver's part).
//!
//! # Resumable threads (the browser build's network thread)
//! A thread that never ends would keep a suspended stack for good, and on wasm then no mark phase would ever end.
//! The translator therefore gives listed thread bodies a second, resumable form (tools/cs2rs DESIGN.md section 8f:
//! an `async fn`; `Thread.Sleep` is [`sleep_async`], `try` is [`try_async`]), and a driver that cannot scan
//! suspended stacks installs their table ([`set_resumable_threads`]). Such a thread still runs on a fiber, but each
//! entry of the fiber is ONE poll of its state machine: up to its next `Thread.Sleep` of 1 ms or more, where the
//! machine returns `Pending` and the fiber's body returns, so its stack is gone. The thread is then PARKED: it is
//! in the suspended list (the pump continues it as before, after `wake_at`), but [`has_suspended_work`] does not
//! count it, so mark phases end. Everything the thread's frames held is in the machine, one heap block, which a
//! collector root scans conservatively (as a suspended iterator method, `rt::iter`); the roots are traced again at
//! the end of a mark phase, so values the machine loaded without a barrier are found. Inside a poll everything
//! else is as before: waits in host code (`TcpClient.Connect`), yield points, locks and short sleeps suspend the
//! fiber mid-poll, and while it is suspended mark phases do not end. The native shell installs no table: natively
//! every thread runs its plain form, unchanged.
//! - A thread body may be an instance method (`TcpSocket.ListenLoop`): its table entry gets the start delegate's
//!   receiver. A host wait inside a resumable form that should not keep a stack is awaited ([`park_async`]: the
//!   machine parks until the next pump; `TcpListener.AcceptTcpClient` of the browser build's server).
//! - The browser build's dedicated server runs its MAIN loop (`Main.DedServ`) as a machine too: the driver polls it
//!   on the main stack ([`poll_main_machine`]); every `Thread.Sleep` of it runs the queued work and parks it, so the
//!   driver can collect between polls and hand the page its turn.

use std::cell::{Cell, RefCell};

use terraria_gen_rt::rt::{self, Delegate, Gc, Marker, Nullable, Obj, Rooted, Str, Trace, Val};

use crate::gfx::states::throw;

/// `Action` / `ThreadStart`
pub type Action = Delegate<dyn Fn()>;
/// `WaitCallback`
pub type WaitCallback = Delegate<dyn Fn(Obj)>;

/// `Thread.Sleep` calls allowed inside one pumped work item before the wait loop is declared a deadlock (only
/// where work cannot be suspended: no fibers, or work run inline inside other work).
pub const SLEEP_LIMIT: u32 = 1000;
/// `Thread.Sleep` calls after which a work item running on a fiber is suspended until the next pump: enough for
/// the short sleeps of working code to pass at once, few enough that a wait loop costs nothing.
pub const SLEEPS_PER_SLICE: u32 = 100;
/// Resumptions of one suspended work item by a single `Task.Wait()` / `Thread.Join()` / `Task.IsCompleted` on the
/// main fiber before that is declared a deadlock (the work waits for the main thread, which waits for the work).
pub const RESUME_LIMIT: u32 = 100_000;

#[derive(Clone, Copy)]
enum Work {
    Action(Action),
    Callback(WaitCallback, Obj),
}
// SAFETY: every handle is reported.
unsafe impl Trace for Work {
    unsafe fn trace(&self, m: &mut Marker) {
        match self {
            Work::Action(a) => a.trace(m),
            Work::Callback(c, o) => {
                c.trace(m);
                m.mark(*o);
            }
        }
    }
}

/// `System.Threading.Tasks.Task` (also `Task<T>` without the result: see [`TaskCompletionSource`]) and
/// `System.Threading.Thread`.
pub struct Task {
    work: Option<Work>,
    pub completed: bool,
    running: bool,
    /// a `TaskCompletionSource` task: completed by `SetResult`, never by running work
    pub is_completion_source: bool,
    pub name: Str,
    pub is_background: bool,
    started: bool,
    /// the fiber this task's work is suspended on (Windows, wasm with JSPI; null otherwise), see the module docs
    fiber: *mut FiberState,
    /// a `Thread` object (`new Thread(..)`), not pool work: see the module docs, "Threads that never end"
    is_thread: bool,
}
// SAFETY: every handle is reported.
unsafe impl Trace for Task {
    unsafe fn trace(&self, m: &mut Marker) {
        if let Some(w) = &self.work {
            w.trace(m);
        }
        self.name.trace(m);
    }
}

thread_local! {
    /// What the main thread does for work that waits on it (see [`set_main_thread_pump`]).
    static MAIN_THREAD_PUMP: RefCell<Option<std::rc::Rc<dyn Fn()>>> = const { RefCell::new(None) };
    static QUEUE: RefCell<Vec<Rooted<Gc<Task>>>> = const { RefCell::new(Vec::new()) };
    static PUMP_DEPTH: Cell<u32> = const { Cell::new(0) };
    static SLEEPS: Cell<u32> = const { Cell::new(0) };
}

fn enqueue(t: Gc<Task>) {
    QUEUE.with(|q| q.borrow_mut().push(Rooted::new(t)));
}

/// One work item on its own fiber (module docs, "Fibers").
struct FiberState {
    /// the OS fiber (null until created / after it was deleted); on wasm: the base of its shadow stack
    fiber: *mut core::ffi::c_void,
    task: Rooted<Gc<Task>>,
    work: Work,
    finished: bool,
    /// what left the work by unwinding (a stop, or an uncaught C# exception): re-raised on the main fiber
    payload: Option<Box<dyn std::any::Any + Send>>,
    /// this work item's `[ThreadStatic]` values (`rt::threads` context)
    ctx: u64,
    /// the context that was running when the fiber last gave control back: `ctx`, or that of work it runs inline
    /// (a thread of its own in the original, see `run`), and the contexts of that inline work, innermost last
    current: u64,
    nested: Vec<u64>,
    /// while suspended: the stack memory in use (lowest address, one past the highest); 0 while running
    stack_low: usize,
    stack_high: usize,
    /// suspended in a sleep of a `Thread` object's loop or in a host wait: it waits for time / the network
    idle: bool,
    /// not to be resumed before this moment (a long sleep)
    wake_at: Option<std::time::Instant>,
    /// a thread run as a state machine (module docs, "Resumable threads"): its body, one poll per entry of the fiber
    machine: Option<std::pin::Pin<Box<dyn std::future::Future<Output = ()>>>>,
    /// the collector root that scans `machine` (0: none)
    machine_root: u64,
    /// the machine returned `Pending` at a sleep and its fiber ended: nothing of it is on any stack. The next resume
    /// enters the fiber afresh and polls again
    parked: bool,
}

impl Drop for FiberState {
    fn drop(&mut self) {
        if self.machine_root != 0 {
            rt::remove_root(self.machine_root);
        }
    }
}

#[cfg(windows)]
mod fibers {
    use core::ffi::c_void;
    #[link(name = "kernel32")]
    extern "system" {
        pub fn ConvertThreadToFiber(parameter: *mut c_void) -> *mut c_void;
        pub fn CreateFiberEx(commit: usize, reserve: usize, flags: u32, start: unsafe extern "system" fn(*mut c_void), parameter: *mut c_void) -> *mut c_void;
        pub fn SwitchToFiber(fiber: *mut c_void);
        pub fn DeleteFiber(fiber: *mut c_void);
        pub fn IsThreadAFiber() -> i32;
    }
    /// `GetCurrentFiber()` (a macro of the Windows headers: `NtCurrentTeb()->NtTib.FiberData`).
    #[cfg(target_arch = "x86_64")]
    pub unsafe fn current_fiber() -> *mut c_void {
        let p: *mut c_void;
        core::arch::asm!("mov {}, gs:[0x20]", out(reg) p, options(nostack, readonly, preserves_flags));
        p
    }
    /// Stack reserved for a work item: world loading and generation recurse (flood fills), the game thread's own
    /// stack is the linker default; the pages are committed on demand.
    pub const STACK_RESERVE: usize = 256 << 20;
}

/// Work fibers on WebAssembly (module docs, "Fibers on wasm"). The driver (terraria-web) owns the JSPI imports and
/// the fiber entry export and installs them here as a [`Backend`](wasm_fibers::Backend); this module owns the
/// shadow stacks.
#[cfg(target_family = "wasm")]
pub mod wasm_fibers {
    use std::cell::Cell;

    /// The stack switch of the driver.
    #[derive(Clone, Copy)]
    pub struct Backend {
        /// Main stack: starts fiber `handle` with its shadow stack pointer at `stack_top` (non-zero: first entry,
        /// the engine stack is created by calling the driver's promising entry export, which must call [`entry`]),
        /// or continues it (`stack_top` 0), until it gives control back. Returns 0 (it gave control back:
        /// [`Backend::suspend`]), 1 (it finished: [`entry`] returned) or 2 (it trapped: its stack is gone).
        pub resume: unsafe fn(handle: usize, stack_top: usize) -> i32,
        /// Work fiber `handle`: gives control back to the main stack; returns when it is resumed.
        pub suspend: unsafe fn(handle: usize),
        /// Fiber `handle` will never be resumed (`Thread.Abort`): the driver forgets it.
        pub forget: unsafe fn(handle: usize),
    }

    thread_local! {
        pub(super) static BACKEND: Cell<Option<Backend>> = const { Cell::new(None) };
    }

    /// Driver: turns work fibers on (`Some`) or off. Only while no work is suspended.
    pub fn install(backend: Option<Backend>) {
        assert!(!super::has_suspended_work(), "host threading: the fiber backend changed while work is suspended");
        BACKEND.set(backend);
    }

    /// Shadow stack of a work fiber (its address-taken locals; the engine keeps the rest). The main stack of the
    /// browser build has 64 MB (web/ci/build_game_wasm.sh) because world generation recurses on it; a work item gets
    /// half of that, and its high-water mark is logged when it ends.
    pub const SHADOW_STACK: usize = 32 << 20;
    /// The lowest bytes of a shadow stack hold this pattern; it is checked at every switch. An overflow of a shadow
    /// stack is not detected by the engine (WEB_PORT.md 1.5, `depth.mjs`): below it lies the heap.
    pub const GUARD: usize = 64 << 10;
    const GUARD_BYTE: u8 = 0xA5;

    fn layout() -> std::alloc::Layout {
        std::alloc::Layout::from_size_align(SHADOW_STACK, 16).unwrap()
    }

    thread_local! {
        /// Shadow stacks of ended fibers, zero above their guard again, for the next fiber: a network completion
        /// callback is a fiber of its own, thousands per session, and a fresh zeroed 32 MB block plus a full scan
        /// for its high-water mark cost about 3 ms each (a joining player: stretches of 2-6 s of the server Worker,
        /// CPU profile work/logs/hpw_server_join_profile.txt)
        static POOL: std::cell::RefCell<Vec<usize>> = const { std::cell::RefCell::new(Vec::new()) };
    }
    /// Shadow stacks kept for reuse at most.
    const POOL_MAX: usize = 4;
    /// The high-water scan stops after this many zero bytes in a row below the last used byte.
    const SCAN_GAP: usize = 64 << 10;

    /// A shadow stack (zero above the guard, so the high-water mark can be found): its base. A pooled one if any.
    pub(super) fn alloc_stack() -> *mut u8 {
        if let Some(b) = POOL.with(|p| p.borrow_mut().pop()) {
            return b as *mut u8;
        }
        // SAFETY: non-zero size.
        let base = unsafe { std::alloc::alloc_zeroed(layout()) };
        if base.is_null() {
            std::alloc::handle_alloc_error(layout());
        }
        // SAFETY: the first GUARD bytes of the new block.
        unsafe { core::ptr::write_bytes(base, GUARD_BYTE, GUARD) };
        base
    }

    /// Panics when the fiber's shadow stack has grown into its guard.
    ///
    /// # Safety
    /// `base` comes from [`alloc_stack`] and is not freed.
    pub(super) unsafe fn check_guard(base: *mut u8) {
        let g = core::slice::from_raw_parts(base, GUARD);
        if let Some(i) = g.iter().rposition(|b| *b != GUARD_BYTE) {
            panic!("host threading: a work fiber's shadow stack overflowed ({} MB, the guard was written {} bytes below its top). Raise wasm_fibers::SHADOW_STACK.", SHADOW_STACK >> 20, GUARD - i);
        }
    }

    /// Gives a shadow stack back (to the pool, zeroed where it was used, or to the allocator); returns the bytes it
    /// used at most. The scan goes down from the top and stops at [`SCAN_GAP`] zero bytes in a row, so it costs what
    /// the fiber used, not 32 MB (a deeper gap of zeros only makes a later mark read low; nothing depends on it).
    ///
    /// # Safety
    /// As [`check_guard`]; no fiber runs on it any more.
    pub(super) unsafe fn free_stack(base: *mut u8) -> usize {
        let top = base as usize + SHADOW_STACK;
        let floor = base as usize + GUARD;
        let mut low = top; // the lowest used byte found so far
        let mut at = top;
        while at > floor {
            let from = at.saturating_sub(4096).max(floor);
            let page = core::slice::from_raw_parts(from as *const u8, at - from);
            if let Some(i) = page.iter().position(|b| *b != 0) {
                low = from + i;
            } else if low - from >= SCAN_GAP {
                break;
            }
            at = from;
        }
        let high = top - low;
        let pooled = POOL.with(|p| {
            let mut p = p.borrow_mut();
            if p.len() < POOL_MAX {
                core::ptr::write_bytes(low as *mut u8, 0, top - low);
                p.push(base as usize);
                true
            } else {
                false
            }
        });
        if !pooled {
            std::alloc::dealloc(base, layout());
        }
        high
    }

    /// The body of a work fiber: the driver's promising entry export calls this with the `handle` it was started
    /// with, on the fiber's own engine stack and shadow stack. Nothing unwinds out of it.
    pub fn entry(handle: usize) {
        let st = handle as *mut super::FiberState;
        // SAFETY: the handle is the live box `resume` passed to the backend.
        unsafe {
            let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| super::run_fiber_body(st)));
            if let Err(p) = r {
                (*st).payload = Some(p);
                (*st).parked = false;
            }
            (*st).finished = !(*st).parked;
        }
    }
}

/// Are work fibers available (Windows x64: always; wasm: when the driver installed the stack switch)?
#[cfg(all(windows, target_arch = "x86_64"))]
#[inline]
fn fibers_on() -> bool {
    true
}
#[cfg(target_family = "wasm")]
#[inline]
fn fibers_on() -> bool {
    wasm_fibers::BACKEND.get().is_some()
}

thread_local! {
    /// the game thread's own fiber, once it has resumed a work item
    static MAIN_FIBER: Cell<*mut core::ffi::c_void> = const { Cell::new(core::ptr::null_mut()) };
    /// the work item whose fiber is running now (null on the main fiber)
    static CURRENT_FIBER: Cell<*mut FiberState> = const { Cell::new(core::ptr::null_mut()) };
    /// work items that are suspended (started, not finished), in start order
    static SUSPENDED: RefCell<Vec<*mut FiberState>> = const { RefCell::new(Vec::new()) };
}

/// Is a work item suspended in the middle of its body (module docs, "Fibers")? The driver must not collect
/// garbage while this is true: the suspended frames hold handles the collector cannot see.
pub fn has_suspended_work() -> bool {
    // SAFETY: the list holds live boxes. A parked machine has no frames anywhere (module docs, "Resumable threads").
    SUSPENDED.with(|s| s.borrow().iter().any(|st| unsafe { !(**st).parked }))
}

/// Is a work item suspended that the frame is waiting for (a load, a save, a generation: suspended at a yield
/// point or in a wait loop)? Fibers that are idle (module docs, "Threads that never end") do not count: a hosted
/// or joined game has them all the time. A script's clock stands still while this is true.
pub fn has_suspended_foreground_work() -> bool {
    // SAFETY: the list holds live boxes.
    SUSPENDED.with(|s| s.borrow().iter().any(|st| unsafe { !(**st).idle }))
}

/// The stack memory of every suspended work item, for the collector's conservative scan (module docs).
pub fn suspended_stacks(f: &mut dyn FnMut(*const u8, usize)) {
    SUSPENDED.with(|s| {
        for st in s.borrow().iter() {
            // SAFETY: the list holds live boxes; the range was recorded when the fiber switched away and the
            // fiber has not run since.
            let (lo, hi) = unsafe { ((**st).stack_low, (**st).stack_high) };
            if lo != 0 && hi > lo {
                f(lo as *const u8, hi - lo);
            }
        }
    });
    // a blocking main loop asleep (`set_blocking_main`): the game's own frames on the main stack
    let (lo, hi) = MAIN_STACK.get();
    if lo != 0 && hi > lo {
        f(lo as *const u8, hi - lo);
    }
}

thread_local! {
    /// While the main thread of a blocking main loop sleeps: its stack in use (lowest address, base).
    static MAIN_STACK: Cell<(usize, usize)> = const { Cell::new((0, 0)) };
}

/// Writes the callee-saved registers (rbx, rbp, rdi, rsi, r12..r15: whatever the callers keep in them across the
/// call) into `regs`, which lives in the CALLER's frame, and returns that frame's stack pointer and the base of
/// the running stack (`NT_TIB.StackBase`: the TEB's stack limits are the running fiber's). From the stack pointer
/// to the base is then everything a conservative scan has to look at.
#[cfg(all(windows, target_arch = "x86_64"))]
#[inline(always)]
unsafe fn spill_registers(regs: &mut [usize; 8]) -> (usize, usize) {
    let (sp, base): (usize, usize);
    core::arch::asm!(
        "mov [{r}], rbx",
        "mov [{r} + 8], rbp",
        "mov [{r} + 16], rdi",
        "mov [{r} + 24], rsi",
        "mov [{r} + 32], r12",
        "mov [{r} + 40], r13",
        "mov [{r} + 48], r14",
        "mov [{r} + 56], r15",
        "mov {sp}, rsp",
        "mov {base}, gs:[0x08]",
        r = in(reg) regs.as_mut_ptr(),
        sp = out(reg) sp,
        base = out(reg) base,
        options(nostack, preserves_flags)
    );
    (sp, base)
}

/// A work fiber gives control back to the main fiber, EVERY suspension goes through here: the callee-saved
/// registers are written to this frame and the stack range in use is recorded, so the collector can find the
/// handles the suspended frames hold ([`suspended_stacks`]).
#[cfg(all(windows, target_arch = "x86_64"))]
#[inline(never)]
unsafe fn switch_to_main(idle: bool, wake_at: Option<std::time::Instant>) {
    let st = CURRENT_FIBER.get();
    debug_assert!(!st.is_null());
    let mut regs = [0usize; 8];
    let (sp, base) = spill_registers(&mut regs);
    core::hint::black_box(&mut regs);
    (*st).stack_low = sp;
    (*st).stack_high = base;
    (*st).idle = idle;
    (*st).wake_at = wake_at;
    fibers::SwitchToFiber(MAIN_FIBER.get());
    // running again
    let st = CURRENT_FIBER.get();
    (*st).stack_low = 0;
    (*st).stack_high = 0;
    (*st).idle = false;
    (*st).wake_at = None;
    core::hint::black_box(&regs);
}

/// wasm: a work fiber gives control back to the main stack through the driver's suspending import. Nothing is
/// recorded for the collector: a suspended wasm stack cannot be scanned (module docs, "Fibers on wasm").
#[cfg(target_family = "wasm")]
#[inline(never)]
unsafe fn switch_to_main(idle: bool, wake_at: Option<std::time::Instant>) {
    let st = CURRENT_FIBER.get();
    debug_assert!(!st.is_null());
    (*st).idle = idle;
    (*st).wake_at = wake_at;
    let b = wasm_fibers::BACKEND.get().expect("host threading: a work fiber runs without a fiber backend");
    (b.suspend)(st as usize);
    // running again
    let st = CURRENT_FIBER.get();
    (*st).idle = false;
    (*st).wake_at = None;
}

/// A blocking wait inside host code that only a thread of the game calls (`AcceptTcpClient`, `Connect`): on a
/// work fiber the frame goes on and the caller continues at the next pump (true); elsewhere nothing happens
/// (false: the caller sleeps a moment itself).
pub fn wait_a_frame() -> bool {
    #[cfg(any(all(windows, target_arch = "x86_64"), target_family = "wasm"))]
    {
        if !CURRENT_FIBER.get().is_null() {
            SLEEPS.set(0);
            // SAFETY: on a work fiber the main fiber exists (it resumed this one).
            unsafe { switch_to_main(true, None) };
            return true;
        }
    }
    false
}

fn run_work(work: Work) {
    match work {
        Work::Action(a) => a.invoke(|f| f()),
        Work::Callback(c, state) => c.invoke(|f| f(state)),
    }
}

/// What a work fiber runs: the work, or one poll of a resumable thread's state machine (module docs, "Resumable
/// threads"). A machine that returns `Pending` is parked: the fiber ends, the machine stays.
///
/// # Safety
/// `st` is the live state of the fiber this runs on.
#[cfg(any(all(windows, target_arch = "x86_64"), target_family = "wasm"))]
unsafe fn run_fiber_body(st: *mut FiberState) {
    let machine = match (*st).machine.as_mut() {
        Some(m) => m as *mut std::pin::Pin<Box<dyn std::future::Future<Output = ()>>>,
        None => return run_work((*st).work),
    };
    let mut cx = std::task::Context::from_waker(std::task::Waker::noop());
    // the machine is only polled here, from the fiber of its own thread; `sleep_async` sets `parked`
    (*st).parked = false;
    if (*machine).as_mut().poll(&mut cx).is_pending() {
        assert!((*st).parked, "host threading: a resumable thread suspended outside Thread.Sleep");
    }
}

// ------------------------------------------------------------------------------------------- resumable threads

/// The resumable form of a thread body (tools/cs2rs DESIGN.md section 8f; the translator's `gen::resumable`), given
/// the receiver of the thread's start delegate (an instance method group's object; 0 for a static one).
pub type ResumableBody = fn(usize) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()>>>;

thread_local! {
    /// (method id of the thread's `ThreadStart`, resumable form): see [`set_resumable_threads`]
    static RESUMABLE: Cell<&'static [(u64, ResumableBody)]> = const { Cell::new(&[]) };
}

/// Driver: threads whose start delegate is one of these method groups run as state machines from now on (module
/// docs, "Resumable threads"). The browser build installs the translator's table; the native shell installs
/// nothing (its fibers' stacks are scanned), so natively every thread runs as before.
pub fn set_resumable_threads(table: &'static [(u64, ResumableBody)]) {
    RESUMABLE.set(table);
}

/// The resumable form of a thread whose start delegate is `a`, if the driver installed one, and the delegate's
/// receiver (0 for a static method group).
fn resumable_body(a: Action) -> Option<(ResumableBody, usize)> {
    let table = RESUMABLE.get();
    if table.is_empty() || a.is_null() {
        return None;
    }
    // a method-group delegate carries (method id, receiver); a closure carries (0, 0), which no entry has
    let rt::Reflect::Delegate { targets } = rt::obj_reflect(a.up()) else { return None };
    let [(id, recv)] = targets.as_slice() else { return None };
    table.iter().find(|(tid, _)| tid == id).map(|(_, body)| (*body, *recv))
}

/// `Thread.Sleep(ms)` inside the resumable form of a thread (the translator's config "resumable"). On the thread's
/// machine a sleep of 1 ms or more is what `thread_sleep` does on a thread's fiber (the sockets move, due callbacks
/// run inline, the thread waits for time or the network: idle), except that the machine RETURNS to its poll instead
/// of suspending a stack. Anywhere else it is `thread_sleep`.
pub fn sleep_async(milliseconds: i32) -> SleepAsync {
    SleepAsync { milliseconds, slept: false }
}

/// See [`sleep_async`].
pub struct SleepAsync {
    milliseconds: i32,
    slept: bool,
}

impl std::future::Future for SleepAsync {
    type Output = ();
    fn poll(mut self: std::pin::Pin<&mut Self>, _cx: &mut std::task::Context<'_>) -> std::task::Poll<()> {
        if self.slept {
            return std::task::Poll::Ready(());
        }
        // the driver's main machine (the browser build's dedicated server, [`poll_main_machine`]): every sleep parks
        // it, after what a sleep of a blocking main loop does first (the queued work, the sockets, the threads)
        if MAIN_MACHINE.get() && PUMP_DEPTH.get() == 0 && is_main_stack() {
            run_pending_work();
            MAIN_WAKE.set(Some(self.milliseconds.max(0)));
            self.slept = true;
            return std::task::Poll::Pending;
        }
        #[cfg(any(all(windows, target_arch = "x86_64"), target_family = "wasm"))]
        {
            let st = CURRENT_FIBER.get();
            // SAFETY: the state of the fiber this code runs on.
            if self.milliseconds >= 1 && !st.is_null() && unsafe { (*st).machine.is_some() } {
                sleep_delivers();
                SLEEPS.set(0);
                let ms = self.milliseconds;
                // SAFETY: as above.
                unsafe {
                    (*st).idle = true;
                    (*st).wake_at = if ms >= 16 { Some(std::time::Instant::now() + std::time::Duration::from_millis(ms as u64)) } else { None };
                    (*st).parked = true;
                }
                self.slept = true;
                return std::task::Poll::Pending;
            }
        }
        thread_sleep(self.milliseconds);
        std::task::Poll::Ready(())
    }
}

/// The `try` of a resumable form (the translator's config "resumable"): [`rt::try_`] around every poll of the body,
/// so a C# exception thrown in any slice of it is caught as in the plain form.
pub fn try_async<R: Copy, F: std::future::Future<Output = rt::Flow<R>>>(body: F) -> TryAsync<F> {
    TryAsync { body }
}

/// The main stack, not a work fiber.
fn is_main_stack() -> bool {
    #[cfg(any(all(windows, target_arch = "x86_64"), target_family = "wasm"))]
    return CURRENT_FIBER.get().is_null();
    #[cfg(not(any(all(windows, target_arch = "x86_64"), target_family = "wasm")))]
    true
}

/// A blocking wait of host code inside a resumable form (the translator's config "resumable" / "await":
/// `TcpListener.AcceptTcpClient` of `TcpSocket.ListenLoop`): the caller loops `{ try; park_async().await }`. On the
/// fiber of a resumable thread the machine PARKS (idle; the next pump polls it again), so no stack is kept while it
/// waits. Anywhere else it is [`wait_a_frame`] (or a 1 ms sleep where there is no frame to give back).
pub fn park_async() -> ParkAsync {
    ParkAsync { parked: false }
}

/// See [`park_async`].
pub struct ParkAsync {
    parked: bool,
}

impl std::future::Future for ParkAsync {
    type Output = ();
    fn poll(mut self: std::pin::Pin<&mut Self>, _cx: &mut std::task::Context<'_>) -> std::task::Poll<()> {
        if self.parked {
            return std::task::Poll::Ready(());
        }
        #[cfg(any(all(windows, target_arch = "x86_64"), target_family = "wasm"))]
        {
            let st = CURRENT_FIBER.get();
            // SAFETY: the state of the fiber this code runs on.
            if !st.is_null() && unsafe { (*st).machine.is_some() } {
                SLEEPS.set(0);
                // SAFETY: as above.
                unsafe {
                    (*st).idle = true;
                    (*st).wake_at = None;
                    (*st).parked = true;
                }
                self.parked = true;
                return std::task::Poll::Pending;
            }
        }
        if !wait_a_frame() {
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        std::task::Poll::Ready(())
    }
}

thread_local! {
    /// The driver polls its main machine now ([`poll_main_machine`]).
    static MAIN_MACHINE: Cell<bool> = const { Cell::new(false) };
    /// The sleep the main machine parked at in this poll (milliseconds).
    static MAIN_WAKE: Cell<Option<i32>> = const { Cell::new(None) };
}

/// Driver of a process whose main loop is the game's own, where the main stack cannot block (the browser build's
/// dedicated server: `Main.DedServ`, which paces itself with `Stopwatch` and `Thread.Sleep`). `machine` is that
/// loop's resumable form (tools/cs2rs DESIGN.md section 8f). One poll runs it on the main stack up to its next
/// `Thread.Sleep(ms)`: the sleep first does what a sleep of a blocking main loop does ([`set_blocking_main`]: the
/// queued work, the sockets, the network threads), then the machine returns `Pending`, so nothing of the game is on
/// any stack when this returns: the driver may run the collector, give the page its turn and poll again after
/// `ms`. Returns `Some(ms)` (parked at that sleep) or `None` (the loop ended). Waits inside a poll (yield points,
/// `Task.IsCompleted` of the world load) stay inside it.
pub fn poll_main_machine(machine: std::pin::Pin<&mut dyn std::future::Future<Output = ()>>) -> Option<i32> {
    assert!(is_main_stack() && PUMP_DEPTH.get() == 0, "host threading: the main machine is polled from the main stack only");
    struct Flag;
    impl Drop for Flag {
        fn drop(&mut self) {
            MAIN_MACHINE.set(false);
        }
    }
    MAIN_MACHINE.set(true);
    MAIN_WAKE.set(None);
    let _flag = Flag;
    let mut cx = std::task::Context::from_waker(std::task::Waker::noop());
    match machine.poll(&mut cx) {
        std::task::Poll::Ready(()) => None,
        std::task::Poll::Pending => Some(MAIN_WAKE.take().expect("host threading: the main machine suspended outside Thread.Sleep")),
    }
}

/// See [`try_async`].
pub struct TryAsync<F> {
    body: F,
}

impl<R: Copy, F: std::future::Future<Output = rt::Flow<R>>> std::future::Future for TryAsync<F> {
    type Output = Result<rt::Flow<R>, Gc<rt::Exception>>;
    fn poll(self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<Self::Output> {
        // SAFETY: structural pinning: `body` is never moved out of a pinned `TryAsync`.
        let body = unsafe { self.map_unchecked_mut(|s| &mut s.body) };
        match rt::try_(|| body.poll(cx)) {
            Ok(std::task::Poll::Ready(r)) => std::task::Poll::Ready(Ok(r)),
            Ok(std::task::Poll::Pending) => std::task::Poll::Pending,
            Err(e) => std::task::Poll::Ready(Err(e)),
        }
    }
}

#[cfg(all(windows, target_arch = "x86_64"))]
unsafe extern "system" fn fiber_main(parameter: *mut core::ffi::c_void) {
    let st = parameter as *mut FiberState;
    // nothing may unwind out of a fiber's start routine
    let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| run_fiber_body(st)));
    if let Err(p) = r {
        (*st).payload = Some(p);
        (*st).parked = false;
    }
    (*st).finished = !(*st).parked;
    loop {
        // back to whoever resumed this fiber; a finished (or parked) fiber is never resumed again (it is deleted)
        fibers::SwitchToFiber(MAIN_FIBER.get());
    }
}

/// Main fiber: runs (or continues) `st` until it finishes or suspends itself. Returns true when it finished.
#[cfg(any(all(windows, target_arch = "x86_64"), target_family = "wasm"))]
fn resume(st: *mut FiberState) -> bool {
    // SAFETY: `st` is a live box (freed below, only here); the fiber API is used from one thread.
    unsafe {
        // a parked machine (module docs, "Resumable threads"): its fiber is entered afresh
        let reenter = core::mem::replace(&mut (*st).parked, false);
        if reenter {
            (*st).idle = false;
            (*st).wake_at = None;
        }
        #[cfg(all(windows, target_arch = "x86_64"))]
        {
            if MAIN_FIBER.get().is_null() {
                let main = if fibers::IsThreadAFiber() != 0 { fibers::current_fiber() } else { fibers::ConvertThreadToFiber(core::ptr::null_mut()) };
                assert!(!main.is_null(), "host threading: ConvertThreadToFiber failed");
                MAIN_FIBER.set(main);
            }
            if (*st).fiber.is_null() {
                let f = fibers::CreateFiberEx(0, fibers::STACK_RESERVE, 0, fiber_main, st as *mut core::ffi::c_void);
                assert!(!f.is_null(), "host threading: CreateFiberEx failed");
                (*st).fiber = f;
            }
        }
        #[cfg(target_family = "wasm")]
        let backend = wasm_fibers::BACKEND.get().expect("host threading: a work fiber without a fiber backend");
        #[cfg(target_family = "wasm")]
        let stack_top = if (*st).fiber.is_null() {
            let base = wasm_fibers::alloc_stack();
            (*st).fiber = base as *mut core::ffi::c_void;
            base as usize + wasm_fibers::SHADOW_STACK
        } else if reenter {
            // the shadow stack of a parked machine is kept (nothing lives on it): a new engine stack starts on it
            (*st).fiber as usize + wasm_fibers::SHADOW_STACK
        } else {
            0
        };
        let (depth, sleeps) = (PUMP_DEPTH.get(), SLEEPS.get());
        PUMP_DEPTH.set(1);
        SLEEPS.set(0);
        CURRENT_FIBER.set(st);
        // the thread-static values of what was running on the fiber come in, the main thread's go to its context
        rt::threads::context_switch((*st).current);
        arm_yield();
        #[cfg(all(windows, target_arch = "x86_64"))]
        fibers::SwitchToFiber((*st).fiber);
        #[cfg(target_family = "wasm")]
        {
            let r = (backend.resume)(st as usize, stack_top);
            if r == 2 {
                // a trap (not a panic: those are caught on the fiber): its stack is gone, the driver logged why
                (*st).finished = true;
                (*st).payload = Some(Box::new(String::from("host threading: a work fiber trapped (see the console for the wasm trap)")));
            }
        }
        disarm_yield();
        // the fiber may have stopped inside work it runs inline: that context continues at the next resume
        (*st).current = rt::threads::context_current();
        rt::threads::context_switch(0);
        CURRENT_FIBER.set(core::ptr::null_mut());
        PUMP_DEPTH.set(depth);
        SLEEPS.set(sleeps);
        #[cfg(target_family = "wasm")]
        wasm_fibers::check_guard((*st).fiber as *mut u8);
        if (*st).parked {
            // the fiber's body returned: its engine stack is gone (wasm), or it is deleted here (Windows). Only the
            // machine remains, in the heap, scanned by its root
            #[cfg(all(windows, target_arch = "x86_64"))]
            {
                fibers::DeleteFiber((*st).fiber);
                (*st).fiber = core::ptr::null_mut();
            }
            return false;
        }
        if !(*st).finished {
            return false;
        }
        #[cfg(all(windows, target_arch = "x86_64"))]
        fibers::DeleteFiber((*st).fiber);
        #[cfg(target_family = "wasm")]
        {
            let high = wasm_fibers::free_stack((*st).fiber as *mut u8);
            (*st).fiber = core::ptr::null_mut();
            // real work (a load, a save, a generation) leaves a mark of a kilobyte or more on its shadow stack; the
            // network's completion callbacks, thousands per session, use a few hundred bytes and are not logged
            if high >= 1024 {
                terraria_gen_rt::host::log(&format!("[host] work fiber ended: shadow stack high-water mark {} KB of {} MB", high >> 10, wasm_fibers::SHADOW_STACK >> 20));
            }
        }
        rt::threads::context_drop((*st).ctx);
        release_locks_of((*st).ctx);
        SUSPENDED.with(|s| s.borrow_mut().retain(|x| *x != st));
        let mut st = Box::from_raw(st);
        let payload = st.payload.take();
        let t = st.task.get();
        let task = &mut *t.ptr();
        task.fiber = core::ptr::null_mut();
        task.running = false;
        task.completed = true;
        task.work = None;
        drop(st);
        if let Some(p) = payload {
            // the stop / exception continues on the main fiber, where the driver's guards are
            std::panic::resume_unwind(p);
        }
        true
    }
}

/// Runs `t`'s work if it has not run: to completion, or (on a fiber) until it suspends itself. A suspended task is
/// continued. Inside other work the task runs inline, on the same stack.
fn run(t: Gc<Task>) {
    // SAFETY: tasks are kept alive by the queue root or by the caller's handle; `ptr()` checks null.
    let task = unsafe { &mut *t.ptr() };
    if task.completed {
        return;
    }
    #[cfg(any(all(windows, target_arch = "x86_64"), target_family = "wasm"))]
    {
        if !task.fiber.is_null() {
            // suspended: continue it, unless this is the work itself asking about its own task
            if CURRENT_FIBER.get().is_null() {
                resume(task.fiber);
            }
            return;
        }
    }
    if task.running {
        return;
    }
    let Some(work) = task.work else { return };
    task.running = true;
    QUEUE.with(|q| q.borrow_mut().retain(|r| r.get() != t));
    #[cfg(any(all(windows, target_arch = "x86_64"), target_family = "wasm"))]
    {
        if CURRENT_FIBER.get().is_null() && PUMP_DEPTH.get() == 0 && fibers_on() {
            // from the main fiber: on a fiber of its own, so that it can wait for the main thread
            let st = Box::into_raw(Box::new({
                let ctx = rt::threads::context_new();
                FiberState { fiber: core::ptr::null_mut(), task: Rooted::new(t), work, finished: false, payload: None, ctx, current: ctx, nested: Vec::new(), stack_low: 0, stack_high: 0, idle: false, wake_at: None, machine: None, machine_root: 0, parked: false }
            }));
            // a thread with a resumable form (module docs, "Resumable threads")
            let body = match work {
                Work::Action(a) if task.is_thread => resumable_body(a),
                _ => None,
            };
            if let Some((body, recv)) = body {
                // SAFETY: the fresh box; the root lives as long as the box (`Drop for FiberState`). The receiver is
                // kept alive by the task's start delegate (the task is rooted) and by the machine's own copy.
                unsafe {
                    (*st).machine = Some(body(recv));
                    (*st).machine_root = rt::add_root(move |m| {
                        // SAFETY: the box outlives its root. Between polls the machine holds every value of the
                        // thread's frames (its locals, the nested machines of the resumable methods it awaits):
                        // the bytes of the one heap block are scanned like a suspended iterator method's
                        // (`rt::iter`). During a poll that is suspended no mark phase ends (`has_suspended_work`).
                        if let Some(f) = &(*st).machine {
                            let r: &dyn std::future::Future<Output = ()> = &**f;
                            m.mark_conservative(r as *const dyn std::future::Future<Output = ()> as *const u8, core::mem::size_of_val(r));
                        }
                    });
                }
            }
            task.fiber = st;
            SUSPENDED.with(|s| s.borrow_mut().push(st));
            resume(st);
            return;
        }
    }
    // inline: inside other work (its sleeps suspend the fiber it runs on), or where there are no fibers.
    // keep the task alive while its work runs (a collection may not run now anyway: generated code is on the stack)
    let _root = Rooted::new(t);
    let (depth, sleeps) = (PUMP_DEPTH.get(), SLEEPS.get());
    PUMP_DEPTH.set(depth + 1);
    SLEEPS.set(0);
    {
        // a thread of its own in the original: its own thread-static values, also when the work unwinds
        struct Ctx(u64, u64);
        impl Drop for Ctx {
            fn drop(&mut self) {
                rt::threads::context_switch(self.0);
                rt::threads::context_drop(self.1);
                release_locks_of(self.1);
                let st = CURRENT_FIBER.get();
                if !st.is_null() {
                    // SAFETY: the state of the fiber this code runs on.
                    unsafe { (*st).nested.retain(|c| *c != self.1) };
                }
            }
        }
        let ctx = Ctx(rt::threads::context_current(), rt::threads::context_new());
        let st = CURRENT_FIBER.get();
        if !st.is_null() {
            // SAFETY: the state of the fiber this code runs on. The fiber owns this context while the work runs:
            // a lock the inline work holds is the fiber's to release (`monitor_enter`).
            unsafe { (*st).nested.push(ctx.1) };
        }
        rt::threads::context_switch(ctx.1);
        run_work(work);
    }
    PUMP_DEPTH.set(depth);
    SLEEPS.set(sleeps);
    // SAFETY: as above (the reference above ended before the work ran any generated code).
    let task = unsafe { &mut *t.ptr() };
    task.running = false;
    task.completed = true;
    task.work = None;
}

/// Main fiber, a caller that needs `t` finished (`Task.Wait`, `Thread.Join`): continues a suspended task until it
/// is done. Each resumption lets it sleep [`SLEEPS_PER_SLICE`] more times; a task that never finishes is waiting
/// for the main thread, which is waiting here.
fn run_to_completion(t: Gc<Task>) {
    run(t);
    #[cfg(any(all(windows, target_arch = "x86_64"), target_family = "wasm"))]
    {
        if !CURRENT_FIBER.get().is_null() {
            // work waiting for other work that is suspended on its own fiber: this one sleeps (gives the frame
            // back) until the other has finished; the main fiber continues both at every pump
            // SAFETY: as `run`; on a work fiber the main fiber exists.
            while unsafe { !(*t.ptr()).completed && !(*t.ptr()).fiber.is_null() } {
                SLEEPS.set(0);
                unsafe { switch_to_main(false, None) };
            }
            return;
        }
        let mut n = 0;
        // SAFETY: as `run`.
        while unsafe { !(*t.ptr()).completed && !(*t.ptr()).fiber.is_null() } {
            n += 1;
            if n > RESUME_LIMIT {
                panic!("host threading model: the main thread waits (Task.Wait / Thread.Join) for work that is itself waiting for the main thread (it slept {} times). See terraria-host/src/sys/threading.rs.", RESUME_LIMIT as u64 * SLEEPS_PER_SLICE as u64);
            }
            run(t);
        }
    }
}

// ------------------------------------------------------------------------------------------------ yield points

/// When a running work item gives the frame back at an `rt::yield_point()` (module docs).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum YieldMode {
    /// never: work runs until it finishes or sleeps
    Off,
    /// after this many loop iterations of translated code per slice (repeatable)
    Loops(u64),
    /// after this many milliseconds per slice
    Millis(u64),
}

thread_local! {
    static YIELD_MODE: Cell<YieldMode> = const { Cell::new(YieldMode::Off) };
    static YIELD_LEFT: Cell<u64> = const { Cell::new(0) };
    static YIELDS: Cell<u64> = const { Cell::new(0) };
}
/// Nanoseconds since [`yield_clock`] started at which the timer arms the yield flag (`u64::MAX`: never).
static YIELD_DEADLINE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(u64::MAX);

fn yield_clock() -> u64 {
    static START: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
    START.get_or_init(std::time::Instant::now).elapsed().as_nanos() as u64
}

/// Driver: how running work gives the frame back (module docs). Call on the game thread.
pub fn set_yield_mode(mode: YieldMode) {
    YIELD_MODE.set(mode);
    rt::threads::set_yield_hook(if mode == YieldMode::Off { None } else { Some(on_yield_point) });
    // wasm has no timer thread: an armed yield point reads the clock every `check` passes instead
    #[cfg(not(target_family = "wasm"))]
    if let YieldMode::Millis(_) = mode {
        static TIMER: std::sync::Once = std::sync::Once::new();
        TIMER.call_once(|| {
            // the timer: it only sets a flag; no game code runs on this thread
            let _ = std::thread::Builder::new().name("terraria-host yield timer".into()).spawn(|| loop {
                std::thread::sleep(std::time::Duration::from_millis(1));
                if yield_clock() >= YIELD_DEADLINE.load(std::sync::atomic::Ordering::Relaxed) {
                    YIELD_DEADLINE.store(u64::MAX, std::sync::atomic::Ordering::Relaxed);
                    rt::threads::YIELD_ARMED.store(true, std::sync::atomic::Ordering::Relaxed);
                }
            });
        });
    }
}

/// Number of times a work item gave the frame back at a yield point (statistics).
pub fn yields() -> u64 {
    YIELDS.get()
}

/// A work item is about to run on its fiber.
fn arm_yield() {
    use std::sync::atomic::Ordering::Relaxed;
    match YIELD_MODE.get() {
        YieldMode::Off => {}
        YieldMode::Loops(n) => {
            YIELD_LEFT.set(n.max(1));
            rt::threads::YIELD_ARMED.store(true, Relaxed);
        }
        YieldMode::Millis(ms) => {
            YIELD_DEADLINE.store(yield_clock() + ms * 1_000_000, Relaxed);
            #[cfg(target_family = "wasm")]
            {
                YIELD_LEFT.set(check::start(false));
                rt::threads::YIELD_ARMED.store(true, Relaxed);
            }
        }
    }
}

/// wasm, [`YieldMode::Millis`] and the main stack's slices: how many armed yield points pass between two reads of the
/// clock (a read is an import call). Adaptive, one count for the main stack and one for work: when the passes between
/// two reads took more than 2 ms the count shrinks in proportion (aiming at 1 ms, at least 4), under 0.5 ms it doubles
/// (up to 1,024). A fixed
/// 1,024 let loops with heavy passes run for seconds between two reads (the server sending world sections to a
/// joining player: stretches of 1-4 s without a page turn measured).
#[cfg(target_family = "wasm")]
mod check {
    use std::cell::Cell;
    const MIN: u64 = 4;
    const MAX: u64 = 1024;
    thread_local! {
        static EVERY: [Cell<u64>; 2] = const { [Cell::new(MAX), Cell::new(MAX)] };
        static LAST: [Cell<u64>; 2] = const { [Cell::new(0), Cell::new(0)] };
    }
    /// A new run of yield points starts now (main stack: `main`): the passes until the first read.
    pub fn start(main: bool) -> u64 {
        let i = main as usize;
        LAST.with(|l| l[i].set(super::yield_clock()));
        EVERY.with(|e| e[i].get())
    }
    /// The clock is read now (the count ran out): adapt, and the passes until the next read; also the time.
    pub fn read(main: bool) -> (u64, u64) {
        let i = main as usize;
        let now = super::yield_clock();
        let took = now.saturating_sub(LAST.with(|l| l[i].replace(now)));
        let every = EVERY.with(|e| {
            let v = e[i].get();
            // too slow: in proportion, aiming at 1 ms between reads; fast: doubling back
            let v = if took > 2_000_000 { ((v as u128 * 1_000_000 / took as u128) as u64).clamp(MIN, MAX) } else if took < 500_000 { (v * 2).min(MAX) } else { v };
            e[i].set(v);
            v
        });
        (every, now)
    }
}

/// Control is back on the main fiber.
fn disarm_yield() {
    use std::sync::atomic::Ordering::Relaxed;
    YIELD_DEADLINE.store(u64::MAX, Relaxed);
    rt::threads::YIELD_ARMED.store(false, Relaxed);
    // the main stack's own slices (wasm) go on. Its next yield point reads the clock at once: a main-stack loop
    // that waits for work (the dedicated server's `while (!task.IsCompleted)` around its world load) runs the work
    // for a whole slice in every pass, so counting a thousand passes first would keep the page waiting for
    // 1,024 slices (8.7 s measured)
    #[cfg(target_family = "wasm")]
    if main_yield::open() {
        YIELD_LEFT.set(1);
        rt::threads::YIELD_ARMED.store(true, Relaxed);
    }
}

/// Yields of the MAIN stack (the browser build only): the game's own long main-thread frames (the title's content
/// loading in `DrawSplash` / `LoadContent_Deferred`, `Initialize_AlmostEverything`, entering a world) give the
/// display frame back at a yield point once their slice is over. The driver's hook gives the frame back
/// (`tw.yield_frame`, a JSPI suspension of the whole main stack until the page's next tick) and keeps the sound
/// going; nothing else runs meanwhile (no Update, no Draw, no work, no collector), so for the game this is a slow
/// frame, as natively, cut into pieces the page can present between. The driver opens a window around the game
/// code it runs ([`main_yield::window`]); outside one, and natively (no hook), a main-stack yield point does nothing.
#[cfg(target_family = "wasm")]
pub mod main_yield {
    use std::cell::Cell;

    thread_local! {
        static HOOK: Cell<Option<fn()>> = const { Cell::new(None) };
        static SLICE_NS: Cell<u64> = const { Cell::new(0) };
        static OPEN: Cell<bool> = const { Cell::new(false) };
        static DEADLINE: Cell<u64> = const { Cell::new(u64::MAX) };
        static COUNT: Cell<u64> = const { Cell::new(0) };
    }

    /// Driver: the hook that gives the display frame back (`None`: main-stack yields off) and the main stack's
    /// slice. Needs [`super::set_yield_mode`] with [`super::YieldMode::Millis`] (the yield hook of `rt::threads`).
    pub fn install(hook: Option<fn()>, slice_ms: u64) {
        HOOK.set(hook);
        SLICE_NS.set(slice_ms.max(1) * 1_000_000);
    }

    /// Driver: game code runs on the main stack from now on (`true`: its slice starts now) or not any more.
    pub fn window(on: bool) {
        use std::sync::atomic::Ordering::Relaxed;
        let on = on && HOOK.get().is_some();
        OPEN.set(on);
        if on {
            DEADLINE.set(super::yield_clock() + SLICE_NS.get());
            super::YIELD_LEFT.set(super::check::start(true));
            terraria_gen_rt::rt::threads::YIELD_ARMED.store(true, Relaxed);
        } else if super::CURRENT_FIBER.get().is_null() {
            terraria_gen_rt::rt::threads::YIELD_ARMED.store(false, Relaxed);
        }
    }

    pub(super) fn open() -> bool {
        OPEN.get()
    }

    /// Times the main stack gave the display frame back (statistics).
    pub fn count() -> u64 {
        COUNT.get()
    }

    /// A yield point of host code (a loop over many game actions, e.g. the asset loader's queue): as a translated
    /// yield point, but reads the clock at once. Only on the main stack inside a window.
    pub fn check() {
        if OPEN.get() && super::CURRENT_FIBER.get().is_null() && !terraria_gen_rt::rt::threads::in_cctor() && super::yield_clock() >= DEADLINE.get() {
            if let Some(h) = HOOK.get() {
                COUNT.set(COUNT.get() + 1);
                h();
            }
            DEADLINE.set(super::yield_clock() + SLICE_NS.get());
        }
    }

    /// An armed yield point on the main stack inside a window: every `check` passes the clock is read;
    /// past the slice the hook runs (the stack is suspended until the next tick) and a new slice starts.
    pub(super) fn on_main_yield_point() {
        let left = super::YIELD_LEFT.get().saturating_sub(1);
        if left > 0 {
            super::YIELD_LEFT.set(left);
            return;
        }
        let (every, now) = super::check::read(true);
        super::YIELD_LEFT.set(every);
        if now < DEADLINE.get() {
            return;
        }
        if let Some(h) = HOOK.get() {
            COUNT.set(COUNT.get() + 1);
            h();
        }
        DEADLINE.set(super::yield_clock() + SLICE_NS.get());
    }
}

/// `rt::yield_point()` with the flag armed (the hook of `rt::threads`).
fn on_yield_point() {
    #[cfg(any(all(windows, target_arch = "x86_64"), target_family = "wasm"))]
    {
        use std::sync::atomic::Ordering::Relaxed;
        if CURRENT_FIBER.get().is_null() {
            // the browser build's main stack inside a driver window: its own slices (`main_yield`)
            #[cfg(target_family = "wasm")]
            if main_yield::open() {
                main_yield::on_main_yield_point();
                return;
            }
            // the main fiber has nobody to give the frame to (the timer fired just after the work stopped)
            rt::threads::YIELD_ARMED.store(false, Relaxed);
            return;
        }
        if let YieldMode::Loops(_) = YIELD_MODE.get() {
            let left = YIELD_LEFT.get().saturating_sub(1);
            YIELD_LEFT.set(left);
            if left > 0 {
                return;
            }
        }
        #[cfg(target_family = "wasm")]
        if let YieldMode::Millis(_) = YIELD_MODE.get() {
            let left = YIELD_LEFT.get().saturating_sub(1);
            if left > 0 {
                YIELD_LEFT.set(left);
                return;
            }
            let (every, now) = check::read(false);
            YIELD_LEFT.set(every);
            if now < YIELD_DEADLINE.load(Relaxed) {
                return;
            }
        }
        YIELDS.set(YIELDS.get() + 1);
        SLEEPS.set(0);
        // SAFETY: on a work fiber the main fiber exists (it resumed this one); `resume` arms the flag again.
        unsafe { switch_to_main(false, None) };
    }
}

// ------------------------------------------------------------------------------------------------ Monitor

thread_local! {
    /// locked object -> (owner: `rt::threads` context, depth)
    static LOCKS: RefCell<std::collections::HashMap<usize, (u64, u32)>> = RefCell::new(std::collections::HashMap::new());
}

fn release_locks_of(ctx: u64) {
    LOCKS.with(|l| l.borrow_mut().retain(|_, v| v.0 != ctx));
}

/// Takes the lock when it is free or already this thread's (monitors are re-entrant).
fn lock_try(key: usize) -> bool {
    let me = rt::threads::context_current();
    LOCKS.with(|l| {
        let mut l = l.borrow_mut();
        match l.get_mut(&key) {
            Some((owner, depth)) if *owner == me => {
                *depth += 1;
                true
            }
            Some(_) => false,
            None => {
                l.insert(key, (me, 1));
                true
            }
        }
    })
}

/// `Monitor.Enter(obj)` / `lock (obj)`: waits while another thread holds the lock. The main fiber continues the
/// suspended work item that holds it until the lock is free; a work item gives the frame back until it is.
pub fn monitor_enter(obj: Obj) -> bool {
    if obj.is_null() {
        throw("System.ArgumentNullException", "obj");
    }
    let key = obj.addr();
    let mut n = 0u32;
    while !lock_try(key) {
        n += 1;
        let owner_now = LOCKS.with(|l| l.borrow().get(&key).map(|v| v.0));
        // The main thread never waits, so a lock it still holds after a work item has given it three whole frames
        // was not left by the original either: `CaptureCamera.DrawTick` enters `_captureLock` every frame and
        // returns without leaving it when no capture runs (monitors are re-entrant, and only the main thread
        // draws). A work item that comes to such a lock goes on, as it did when locks had no owners.
        if owner_now == Some(0) && n > 3 {
            static TOLD: std::sync::Once = std::sync::Once::new();
            TOLD.call_once(|| terraria_gen_rt::host::log("[host] Monitor.Enter: a lock the main thread holds across frames (an Enter without Exit in the original) was entered by a work item"));
            return true;
        }
        if n > RESUME_LIMIT {
            panic!(
                "host threading model: a lock is never released (waited {RESUME_LIMIT} slices; held by context {owner_now:?}, wanted by context {} on {}): the thread that holds it waits for the one that wants it. See terraria-host/src/sys/threading.rs.",
                rt::threads::context_current(),
                if CURRENT_FIBER.get().is_null() { "the main fiber" } else { "a work fiber" }
            );
        }
        #[cfg(any(all(windows, target_arch = "x86_64"), target_family = "wasm"))]
        {
            let owner = LOCKS.with(|l| l.borrow().get(&key).map(|v| v.0));
            if CURRENT_FIBER.get().is_null() {
                let st = SUSPENDED.with(|s| s.borrow().iter().copied().find(|st| owner.is_some_and(|o| unsafe { (**st).ctx == o || (**st).nested.contains(&o) })));
                match st {
                    Some(st) => {
                        resume(st);
                    }
                    // the owner is not a suspended work item (it ended without leaving the lock): the lock is free
                    None => {
                        LOCKS.with(|l| l.borrow_mut().remove(&key));
                    }
                }
            } else {
                SLEEPS.set(0);
                // SAFETY: on a work fiber the main fiber exists.
                unsafe { switch_to_main(false, None) };
            }
            continue;
        }
        #[allow(unreachable_code)]
        {
            // no fibers: nothing else can be running
            LOCKS.with(|l| l.borrow_mut().remove(&key));
        }
    }
    true
}

/// `Monitor.TryEnter(obj)`: false when another thread holds the lock.
pub fn monitor_try_enter(obj: Obj) -> bool {
    if obj.is_null() {
        throw("System.ArgumentNullException", "obj");
    }
    lock_try(obj.addr())
}

/// `Monitor.Exit(obj)`. Deviation: leaving a lock this thread does not hold is ignored (.NET throws
/// `SynchronizationLockException`).
pub fn monitor_exit(obj: Obj) {
    if obj.is_null() {
        throw("System.ArgumentNullException", "obj");
    }
    let me = rt::threads::context_current();
    LOCKS.with(|l| {
        let mut l = l.borrow_mut();
        if let Some((owner, depth)) = l.get_mut(&obj.addr()) {
            if *owner == me {
                *depth -= 1;
                if *depth == 0 {
                    l.remove(&obj.addr());
                }
            }
        }
    })
}

// ------------------------------------------------------------------------------------------------ AsyncActionDispatcher

/// `ReLogic.Threading.AsyncActionDispatcher`: a queue of actions with a dedicated consumer thread in the original
/// (`Start` creates it, `Stop` cancels and joins it). Here the queue is consumed on the game thread (module docs).
pub struct AsyncActionDispatcher {
    queue: std::collections::VecDeque<Action>,
    running: bool,
    disposed: bool,
}
// SAFETY: every handle is reported.
unsafe impl Trace for AsyncActionDispatcher {
    unsafe fn trace(&self, m: &mut Marker) {
        for a in &self.queue {
            a.trace(m);
        }
    }
    unsafe fn obj_dispose(this: *mut Self) {
        AsyncActionDispatcher::dispose(Gc(this))
    }
}

thread_local! {
    /// Started dispatchers (their "threads" are alive): rooted, as a running thread keeps its object alive.
    static DISPATCHERS: RefCell<Vec<Rooted<Gc<AsyncActionDispatcher>>>> = const { RefCell::new(Vec::new()) };
    static DISPATCHER_BUDGET: Cell<usize> = const { Cell::new(64) };
    /// Driver phase (one `Update`, one `Draw`, one `end_update`): see [`ConcurrentQueue::try_dequeue`].
    static PHASE: Cell<u64> = const { Cell::new(0) };
    static EMPTY_POLLS: RefCell<(u64, Vec<usize>)> = const { RefCell::new((0, Vec::new())) };
}

/// Actions a started dispatcher runs per [`run_pending_work`] (default 64). It stands for the loader thread's
/// throughput per frame, which in the original depends on the disk; the game's logic does not depend on it, the
/// number of frames the splash screen lasts does.
pub fn dispatcher_budget() -> usize {
    DISPATCHER_BUDGET.get()
}
pub fn set_dispatcher_budget(actions_per_tick: usize) {
    DISPATCHER_BUDGET.set(actions_per_tick.max(1));
}
/// Driver: a new phase starts (`begin_update`, `begin_draw`, `end_update`).
pub fn next_phase() {
    PHASE.set(PHASE.get() + 1);
}

/// Runs one queued action of a started dispatcher. False when none has work.
pub fn pump_dispatcher_action() -> bool {
    let next = DISPATCHERS.with(|d| {
        for r in d.borrow().iter() {
            let g = r.get();
            // SAFETY: rooted, live object; the reference ends before the action runs.
            let x = unsafe { &mut *g.ptr() };
            if x.running {
                if let Some(a) = x.queue.pop_front() {
                    return Some(a);
                }
            }
        }
        None
    });
    match next {
        Some(a) => {
            let depth = PUMP_DEPTH.get();
            PUMP_DEPTH.set(depth + 1);
            a.invoke(|f| f());
            PUMP_DEPTH.set(depth);
            true
        }
        None => false,
    }
}

impl AsyncActionDispatcher {
    pub fn new_gc() -> Gc<AsyncActionDispatcher> {
        rt::alloc_init(AsyncActionDispatcher { queue: Default::default(), running: false, disposed: false })
    }
    /// `Queue(action)`
    pub fn queue(&mut self, action: Action) {
        self.queue.push_back(action);
    }
    /// `ActionsRemaining`
    pub fn actions_remaining(&self) -> i32 {
        self.queue.len() as i32
    }
    pub fn is_running(&self) -> bool {
        self.running
    }
    pub fn is_disposed(&self) -> bool {
        self.disposed
    }
    /// `Start()`
    pub fn start(this: Gc<AsyncActionDispatcher>) {
        // SAFETY: `ptr()` checks null; the reference ends with this function.
        let x = unsafe { &mut *this.ptr() };
        if x.running {
            throw("System.InvalidOperationException", "AsyncActionDispatcher is already started.");
        }
        x.running = true;
        DISPATCHERS.with(|d| d.borrow_mut().push(Rooted::new(this)));
    }
    /// `Stop()`: the thread is cancelled and joined; actions still queued stay queued.
    pub fn stop(this: Gc<AsyncActionDispatcher>) {
        // SAFETY: as above.
        let x = unsafe { &mut *this.ptr() };
        if !x.running {
            throw("System.InvalidOperationException", "AsyncActionDispatcher is already stopped.");
        }
        x.running = false;
        DISPATCHERS.with(|d| d.borrow_mut().retain(|r| r.get() != this));
    }
    /// `Dispose()`
    pub fn dispose(this: Gc<AsyncActionDispatcher>) {
        // SAFETY: as above.
        let x = unsafe { &mut *this.ptr() };
        if x.disposed {
            return;
        }
        if x.running {
            Self::stop(this);
        }
        // SAFETY: as above (re-borrowed after `stop`).
        unsafe { (*this.ptr()).disposed = true };
    }
}

/// Runs everything queued so far (work queued while running is run too). Returns the number of items run.
/// The driver calls this between `Update` calls.
pub fn run_pending_work() -> usize {
    let mut n = 0;
    #[cfg(any(all(windows, target_arch = "x86_64"), target_family = "wasm"))]
    {
        // work suspended in an earlier pump continues first, each item one slice, in start order
        if CURRENT_FIBER.get().is_null() && fibers_on() {
            // the sockets move first: what completes is queued and runs in this pump
            super::net::pump();
            super::net::deliver(&mut |cb, result| {
                thread_pool_queue_user_work_item(cb, result);
            });
            let now = std::time::Instant::now();
            let suspended: Vec<*mut FiberState> = SUSPENDED.with(|s| s.borrow().clone());
            for st in suspended {
                // SAFETY: checked to be in the list still (a live box).
                if SUSPENDED.with(|s| s.borrow().contains(&st)) && unsafe { (*st).wake_at.is_none_or(|t| t <= now) } {
                    resume(st);
                    n += 1;
                }
            }
        }
    }
    loop {
        let next = QUEUE.with(|q| q.borrow().first().map(|r| r.get()));
        let Some(t) = next else { break };
        run(t);
        n += 1;
    }
    n
}

/// [`run_pending_work`] plus the started dispatchers' share for this tick ([`dispatcher_budget`] actions).
pub fn run_pending_work_and_dispatchers() -> usize {
    let mut n = run_pending_work();
    for _ in 0..dispatcher_budget() {
        if !pump_dispatcher_action() {
            break;
        }
        n += 1;
        // the browser build's main stack: a long run of actions gives the display frame back between two of them
        #[cfg(target_family = "wasm")]
        main_yield::check();
    }
    // what the actions queued (threads started by a loaded asset's callback) runs in the same tick
    n + run_pending_work()
}

/// Number of queued work items.
pub fn pending_work() -> usize {
    QUEUE.with(|q| q.borrow().len())
}

impl Task {
    fn make(work: Option<Work>, is_completion_source: bool) -> Gc<Task> {
        rt::alloc_init(Task { work, completed: false, running: false, is_completion_source, name: Str::NULL, is_background: false, started: false, fiber: core::ptr::null_mut(), is_thread: false })
    }

    /// `new Thread(ThreadStart)`
    pub fn new_thread(start: Action) -> Gc<Task> {
        if start.is_null() {
            throw("System.ArgumentNullException", "start");
        }
        let t = Self::make(Some(Work::Action(start)), false);
        // SAFETY: fresh object.
        unsafe { (*t.ptr()).is_thread = true };
        t
    }
    /// `Thread.Abort()`: a thread that has not run is taken from the queue; one that is suspended is removed where
    /// it stands (its fiber is deleted: no `ThreadAbortException`, no `finally` of the thread runs; locks it holds
    /// are released). The thread calling it for itself is not supported (nothing in the game does).
    pub fn abort(&mut self) {
        let t = Gc(self as *mut Task);
        QUEUE.with(|q| q.borrow_mut().retain(|r| r.get() != t));
        #[cfg(any(all(windows, target_arch = "x86_64"), target_family = "wasm"))]
        {
            let st = self.fiber;
            if !st.is_null() {
                if CURRENT_FIBER.get() == st {
                    throw("System.NotSupportedException", "Thread.Abort() of the running thread itself is not supported by the host threading model.");
                }
                // SAFETY: a suspended work item's state is a live box, freed only here and in `resume`.
                unsafe {
                    if !(*st).fiber.is_null() {
                        #[cfg(all(windows, target_arch = "x86_64"))]
                        fibers::DeleteFiber((*st).fiber);
                        #[cfg(target_family = "wasm")]
                        {
                            // the engine drops the suspended stack once nothing refers to it
                            if let Some(b) = wasm_fibers::BACKEND.get() {
                                (b.forget)(st as usize);
                            }
                            wasm_fibers::free_stack((*st).fiber as *mut u8);
                        }
                    }
                    rt::threads::context_drop((*st).ctx);
                    release_locks_of((*st).ctx);
                    for c in (*st).nested.clone() {
                        rt::threads::context_drop(c);
                        release_locks_of(c);
                    }
                    SUSPENDED.with(|s| s.borrow_mut().retain(|x| *x != st));
                    drop(Box::from_raw(st));
                }
                self.fiber = core::ptr::null_mut();
            }
        }
        self.running = false;
        self.completed = true;
        self.work = None;
    }
    /// `Thread.Start()`: queues the thread's body (module docs).
    pub fn start(&mut self) {
        if self.started {
            throw("System.Threading.ThreadStateException", "Thread is running or terminated; it cannot restart.");
        }
        self.started = true;
        enqueue(Gc(self as *mut Task));
    }
    /// `Thread.Join()`: runs the body now if it is still queued.
    pub fn join(&mut self) {
        if !self.started {
            throw("System.Threading.ThreadStateException", "Thread has not been started.");
        }
        run_to_completion(Gc(self as *mut Task));
    }
    pub fn set_name(&mut self, v: Str) {
        self.name = v;
    }
    pub fn set_is_background(&mut self, v: bool) {
        self.is_background = v;
    }
    /// `new Thread(ParameterizedThreadStart)`: the argument arrives with [`Task::start_with`].
    pub fn new_thread_param(start: WaitCallback) -> Gc<Task> {
        if start.is_null() {
            throw("System.ArgumentNullException", "start");
        }
        let t = Self::make(Some(Work::Callback(start, Gc::NULL)), false);
        // SAFETY: fresh object.
        unsafe { (*t.ptr()).is_thread = true };
        t
    }
    /// `Thread.Start(object)`: queues the body with its argument. `Start()` on such a thread passes null, as .NET does.
    pub fn start_with(&mut self, parameter: Obj) {
        if let Some(Work::Callback(c, _)) = self.work {
            self.work = Some(Work::Callback(c, parameter));
        } else if !self.started {
            throw("System.InvalidOperationException", "The thread was created with a ThreadStart delegate that does not accept a parameter.");
        }
        self.start();
    }
    /// `Thread.CurrentThread`: translated code only ever runs on the game thread (module docs), so this is the one
    /// object standing for it ("Main Thread" once `Program.LaunchGame` named it). Never queued, never completed.
    pub fn current_thread() -> Gc<Task> {
        thread_local! {
            static MAIN: Rooted<Gc<Task>> = Rooted::new(rt::alloc_init(Task { work: None, completed: false, running: true, is_completion_source: false, name: Str::NULL, is_background: false, started: true, fiber: core::ptr::null_mut(), is_thread: true }));
        }
        MAIN.with(|m| m.get())
    }
    /// `Thread.ManagedThreadId`: 1 for the game thread (the CLR's main thread id), an id per object otherwise.
    pub fn managed_thread_id(&self) -> i32 {
        if Gc(self as *const Task as *mut Task) == Self::current_thread() {
            1
        } else {
            2 + (rt::identity_hash(self as *const Task as usize) & 0xFFFF)
        }
    }
    /// `Thread.IsAlive`: started and not finished.
    pub fn is_alive(&self) -> bool {
        self.started && !self.completed
    }

    /// `Task.Factory.StartNew(Action)` / `Task.Run(Action)`
    pub fn start_new(action: Action) -> Gc<Task> {
        if action.is_null() {
            throw("System.ArgumentNullException", "action");
        }
        let t = Self::make(Some(Work::Action(action)), false);
        // SAFETY: fresh object.
        unsafe { (*t.ptr()).started = true };
        enqueue(t);
        t
    }
    /// `Task.IsCompleted`: runs the task first if it is still queued, so `while (!task.IsCompleted)` ends.
    pub fn is_completed(&mut self) -> bool {
        run(Gc(self as *mut Task));
        self.completed
    }
    /// `Task.Wait()`
    pub fn wait(&mut self) {
        run_to_completion(Gc(self as *mut Task));
        if !self.completed && self.is_completion_source {
            // A worker waits for something it handed to the main thread (`Main.RunOnMainThread(..).Wait()`: the
            // action sits in the game's own queue, which `Main.Update` drains). The worker IS the main thread
            // here, so the main thread's part is run now, through the pump the driver registered.
            let pump = MAIN_THREAD_PUMP.with(|p| p.borrow().clone());
            if let Some(pump) = pump {
                // the main thread's part: with the main thread's thread-static values and as the owner of the
                // main thread's locks (context 0), also when the pump unwinds
                struct Back(u64);
                impl Drop for Back {
                    fn drop(&mut self) {
                        rt::threads::context_switch(self.0);
                    }
                }
                let _back = Back(rt::threads::context_current());
                rt::threads::context_switch(0);
                pump();
            }
        }
        if !self.completed {
            if self.is_completion_source {
                panic!(
                    "host threading model: Task.Wait() on a TaskCompletionSource task that has no result yet. On one thread nothing else can run to provide it (typically Main.RunOnMainThread(..).Wait() from work that the host is running on the main thread). See terraria-host/src/sys/threading.rs."
                );
            }
            panic!("host threading model: Task.Wait() on a task that is already running (it waits for itself)");
        }
    }
}

/// `TaskFactory` (`Task.Factory`): stateless.
pub struct TaskFactory;
// SAFETY: no handles inside.
unsafe impl Trace for TaskFactory {
    const NEEDS_TRACE: bool = false;
    unsafe fn trace(&self, _m: &mut Marker) {}
}
impl TaskFactory {
    /// `Task.Factory`: a fresh stateless object per call.
    pub fn get() -> Gc<TaskFactory> {
        rt::alloc_init(TaskFactory)
    }
    /// `StartNew(Action)`
    pub fn start_new(&self, action: Action) -> Gc<Task> {
        Task::start_new(action)
    }
}

/// `ThreadPool.QueueUserWorkItem(callback, state)`: always true.
pub fn thread_pool_queue_user_work_item(callback: WaitCallback, state: Obj) -> bool {
    if callback.is_null() {
        throw("System.ArgumentNullException", "callBack");
    }
    let t = Task::make(Some(Work::Callback(callback, state)), false);
    // SAFETY: fresh object.
    unsafe { (*t.ptr()).started = true };
    enqueue(t);
    true
}

/// Driver: registers how the main thread serves requests that queued work waits for. For the game this is
/// `Main.ConsumeAllMainThreadActions()` (the queue behind `Main.RunOnMainThread` / `QueueMainThreadAction`, which
/// `Main.Update` drains once per tick). `Task.Wait()` on an unfinished `TaskCompletionSource` task calls it: in the
/// original the worker thread blocks until the main thread's next `Update` has run the action; on one thread the
/// action runs at the wait. Deviation: whatever else that `Update` would have done before draining the queue has
/// not happened yet when the action runs.
pub fn set_main_thread_pump(f: impl Fn() + 'static) {
    MAIN_THREAD_PUMP.with(|p| *p.borrow_mut() = Some(std::rc::Rc::new(f)));
}

/// `ThreadPool.SetMinThreads(worker, io)`: there is no pool (module docs); the request is accepted.
pub fn thread_pool_set_min_threads(_worker_threads: i32, _completion_port_threads: i32) -> bool {
    true
}

thread_local! {
    /// The main loop is the game's own (`Main.DedServ`): what a main-thread sleep runs besides the pump.
    static BLOCKING_MAIN: RefCell<Option<std::rc::Rc<dyn Fn()>>> = const { RefCell::new(None) };
}

/// Driver of a process whose main loop is the game's own (the dedicated server: `Main.DedServ` paces itself with
/// `Stopwatch` and `Thread.Sleep`): from now on `Thread.Sleep` on the main thread runs the queued work
/// ([`run_pending_work`]: sockets, the network threads), then `idle` (the collector), then really sleeps.
pub fn set_blocking_main(idle: impl Fn() + 'static) {
    BLOCKING_MAIN.with(|b| *b.borrow_mut() = Some(std::rc::Rc::new(idle)));
}

/// `Thread.Sleep(ms)`: returns at once on the game thread (frame pacing belongs to the driver; but see
/// [`set_blocking_main`]); inside pumped work it counts towards [`SLEEP_LIMIT`] (module docs, wait loops).
pub fn thread_sleep(milliseconds: i32) {
    if PUMP_DEPTH.get() == 0 {
        let idle = BLOCKING_MAIN.with(|b| b.borrow().clone());
        if let Some(idle) = idle {
            run_pending_work();
            // the collector may run in `idle`: the frames of the game's own loop below this call hold handles
            #[cfg(all(windows, target_arch = "x86_64"))]
            {
                let mut regs = [0usize; 8];
                // SAFETY: plain register and TEB reads.
                MAIN_STACK.set(unsafe { spill_registers(&mut regs) });
                idle();
                MAIN_STACK.set((0, 0));
                core::hint::black_box(&regs);
            }
            #[cfg(not(all(windows, target_arch = "x86_64")))]
            idle();
            if milliseconds > 0 {
                std::thread::sleep(std::time::Duration::from_millis(milliseconds as u64));
            } else {
                std::thread::yield_now();
            }
        }
        return;
    }
    {
        let n = SLEEPS.get() + 1;
        SLEEPS.set(n);
        #[cfg(any(all(windows, target_arch = "x86_64"), target_family = "wasm"))]
        {
            let st = CURRENT_FIBER.get();
            if !st.is_null() {
                sleep_delivers();
                // SAFETY: the state of the fiber this code runs on.
                let is_thread = unsafe { (*(*st).task.get().ptr()).is_thread };
                if is_thread && milliseconds >= 1 {
                    // a loop of a thread of its own that waits for time or for the network (module docs)
                    SLEEPS.set(0);
                    let wake = if milliseconds >= 16 { Some(std::time::Instant::now() + std::time::Duration::from_millis(milliseconds as u64)) } else { None };
                    // SAFETY: on a work fiber the main fiber exists (it resumed this one).
                    unsafe { switch_to_main(true, wake) };
                } else if n >= SLEEPS_PER_SLICE {
                    // a wait loop: the frame goes on; this work continues at the next pump (module docs, "Fibers")
                    SLEEPS.set(0);
                    // SAFETY: as above.
                    unsafe { switch_to_main(false, None) };
                }
                return;
            }
        }
        if n > SLEEP_LIMIT {
            panic!("host threading model: {SLEEP_LIMIT} Thread.Sleep calls inside one queued work item: it is waiting for something only the main thread does, which cannot happen while the work runs on the main thread. See terraria-host/src/sys/threading.rs.");
        }
    }
}

/// While a thread of the game sleeps on a fiber the I/O threads of the original deliver what arrived: the sockets
/// move and the completion callbacks that became due run now, inline (module docs).
#[cfg(any(all(windows, target_arch = "x86_64"), target_family = "wasm"))]
fn sleep_delivers() {
    if super::net::pending_operations() > 0 {
        super::net::pump();
    }
    super::net::deliver(&mut |cb, result| {
        let t = Task::make(Some(Work::Callback(cb, result)), false);
        // SAFETY: fresh object.
        unsafe { (*t.ptr()).started = true };
        // inside other work: runs inline, as a thread of its own (`run`)
        run(t);
    });
}

/// `ReLogic.Threading.ThreadUtilities.HighPrecisionSleep(seconds)`: the game's frame limiter; pacing is the driver's.
pub fn high_precision_sleep(_seconds: f64) {}

/// `Interlocked.Increment(ref int)`
///
/// # Safety
/// `location` must be a valid place (a `ref` argument of generated code).
pub unsafe fn interlocked_increment(location: *mut i32) -> i32 {
    *location = (*location).wrapping_add(1);
    *location
}
/// `Interlocked.Decrement(ref int)`
///
/// # Safety
/// As [`interlocked_increment`].
pub unsafe fn interlocked_decrement(location: *mut i32) -> i32 {
    *location = (*location).wrapping_sub(1);
    *location
}
/// `Interlocked.Exchange(ref int, int)`
///
/// # Safety
/// As [`interlocked_increment`].
pub unsafe fn interlocked_exchange(location: *mut i32, value: i32) -> i32 {
    core::mem::replace(&mut *location, value)
}

/// `Interlocked.Add(ref int, int)`: the new value.
///
/// # Safety
/// As [`interlocked_increment`].
pub unsafe fn interlocked_add(location: *mut i32, value: i32) -> i32 {
    *location = (*location).wrapping_add(value);
    *location
}

// ------------------------------------------------------------------------------------------------ TaskCompletionSource<T>

/// `TaskCompletionSource<TResult>`
pub struct TaskCompletionSource<T: Val> {
    pub task: Gc<Task>,
    pub result: Nullable<T>,
}
// SAFETY: the handle and the result are reported.
unsafe impl<T: Val> Trace for TaskCompletionSource<T> {
    unsafe fn trace(&self, m: &mut Marker) {
        m.mark(self.task);
        self.result.trace(m);
    }
}
impl<T: Val> TaskCompletionSource<T> {
    pub fn new_gc() -> Gc<Self> {
        // SAFETY: all-zero is a valid `Nullable<T>` (no value).
        rt::alloc_init(TaskCompletionSource { task: Task::make(None, true), result: unsafe { rt::zero() } })
    }
    /// `Task`
    pub fn task(&self) -> Gc<Task> {
        self.task
    }
    /// `SetResult(result)`: `InvalidOperationException` when the task is already completed.
    pub fn set_result(&mut self, result: T) {
        // SAFETY: the source keeps its task alive.
        let t = unsafe { &mut *self.task.ptr() };
        if t.completed {
            throw("System.InvalidOperationException", "An attempt was made to transition a task to a final state when it had already completed.");
        }
        self.result = Nullable::some(result);
        t.completed = true;
    }
    /// `Task<T>.Result` through the source.
    pub fn result(&self) -> T {
        // SAFETY: as above.
        unsafe { (*self.task.ptr()).wait() };
        self.result.value()
    }
}

// ------------------------------------------------------------------------------------------------ ThreadLocal<T>

/// `ThreadLocal<T>`: one value, because translated code runs on one thread (module docs). `Value` is created by
/// the factory on first read, or is `default(T)` without one.
pub struct ThreadLocal<T: Val> {
    factory: Delegate<dyn Fn() -> T>,
    value: Nullable<T>,
    disposed: bool,
}
// SAFETY: every handle is reported.
unsafe impl<T: Val> Trace for ThreadLocal<T> {
    unsafe fn trace(&self, m: &mut Marker) {
        self.factory.trace(m);
        self.value.trace(m);
    }
    unsafe fn obj_dispose(this: *mut Self) {
        (*this).dispose()
    }
}
impl<T: Val> ThreadLocal<T> {
    /// `new ThreadLocal<T>()`
    pub fn new_gc() -> Gc<Self> {
        // SAFETY: all-zero is a valid `Nullable<T>` (no value).
        rt::alloc_init(ThreadLocal { factory: Delegate::NULL, value: unsafe { rt::zero() }, disposed: false })
    }
    /// `new ThreadLocal<T>(valueFactory)`
    pub fn with_factory_gc(factory: Delegate<dyn Fn() -> T>) -> Gc<Self> {
        if factory.is_null() {
            throw("System.ArgumentNullException", "valueFactory");
        }
        // SAFETY: as above.
        rt::alloc_init(ThreadLocal { factory, value: unsafe { rt::zero() }, disposed: false })
    }
    fn check(&self) {
        if self.disposed {
            throw("System.ObjectDisposedException", "The ThreadLocal object has been disposed.");
        }
    }
    /// `IsValueCreated`
    pub fn is_value_created(&self) -> bool {
        self.check();
        self.value.has_value
    }
    /// `Value` (get). Takes the handle: the factory is translated code and may touch this object.
    pub fn value(this: Gc<Self>) -> T {
        // SAFETY: `ptr()` checks null; no reference is alive while the factory runs.
        unsafe {
            (*this.ptr()).check();
            if !(*this.ptr()).value.has_value {
                let f = (*this.ptr()).factory;
                let v: T = if f.is_null() { rt::zero() } else { f.invoke(|f| f()) };
                (*this.ptr()).value = Nullable::some(v);
            }
            (*this.ptr()).value.value
        }
    }
    /// `Value` (set)
    pub fn set_value(&mut self, v: T) {
        self.check();
        self.value = Nullable::some(v);
    }
    pub fn dispose(&mut self) {
        self.disposed = true;
        // SAFETY: all-zero is a valid `Nullable<T>`.
        self.value = unsafe { rt::zero() };
        self.factory = Delegate::NULL;
    }
}

// ------------------------------------------------------------------------------------------------ ConcurrentQueue<T>

/// `ConcurrentQueue<T>`: a FIFO (one thread: no concurrency to handle).
pub struct ConcurrentQueue<T: Val> {
    items: std::collections::VecDeque<T>,
}
// SAFETY: every element is reported.
unsafe impl<T: Val> Trace for ConcurrentQueue<T> {
    unsafe fn trace(&self, m: &mut Marker) {
        for i in &self.items {
            i.trace(m);
        }
    }
}
impl<T: Val> ConcurrentQueue<T> {
    pub fn new_gc() -> Gc<Self> {
        rt::alloc_init(ConcurrentQueue { items: Default::default() })
    }
    pub fn enqueue(&mut self, item: T) {
        self.items.push_back(item);
    }
    /// `TryDequeue(out T result)`: `default(T)` and false when empty.
    ///
    /// The queue is the channel on which a worker hands results to the game thread. A game thread that polls an
    /// empty queue AGAIN within the same driver phase is busy-waiting for the worker (`AssetRepository.SetSources`:
    /// `while (loader.Remaining > 0) loader.TransferCompleted();`); on one thread the worker can only make progress
    /// if it is run now, so the second and later empty polls of a phase run queued dispatcher actions until
    /// something arrives or nothing is left to run. The first empty poll of a phase just answers false (the
    /// per-frame `TransferCompleted` in `Main.Draw`).
    ///
    /// # Safety
    /// `result` must be a valid place (an `out` argument of generated code).
    pub unsafe fn try_dequeue(&mut self, result: *mut T) -> bool {
        if self.items.is_empty() {
            let me = self as *const Self as usize;
            let again = EMPTY_POLLS.with(|p| {
                let mut p = p.borrow_mut();
                if p.0 != PHASE.get() {
                    p.0 = PHASE.get();
                    p.1.clear();
                }
                if p.1.contains(&me) {
                    true
                } else {
                    p.1.push(me);
                    false
                }
            });
            if again {
                // `self` is not touched while translated code runs (the worker enqueues through its own handle)
                let this = self as *mut Self;
                while (*this).items.is_empty() && pump_dispatcher_action() {}
            }
        }
        match self.items.pop_front() {
            Some(v) => {
                *result = v;
                true
            }
            None => {
                *result = rt::zero();
                false
            }
        }
    }
    pub fn count(&self) -> i32 {
        self.items.len() as i32
    }
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}

#[cfg(test)]
mod dispatcher_tests {
    use super::*;
    use std::rc::Rc;

    fn act(log: &Rc<RefCell<Vec<i32>>>, n: i32, results: Gc<ConcurrentQueue<i32>>) -> Action {
        let log = log.clone();
        rt::Delegate::new_traced(
            Box::new(move || {
                log.borrow_mut().push(n);
                // SAFETY: the queue is rooted by the test.
                unsafe { (*results.ptr()).enqueue(n * 10) };
            }),
            Box::new(move |m: &mut Marker| m.mark(results)),
        )
    }

    /// The loader-thread model: nothing runs before `Start`, a tick runs at most the budget, a second poll of an
    /// empty result queue in the same phase runs the worker until a result arrives, `Stop` leaves the rest queued.
    #[test]
    fn dispatcher_serves_its_queue_by_budget_and_on_demand() {
        let log = Rc::new(RefCell::new(Vec::new()));
        let results = ConcurrentQueue::<i32>::new_gc();
        let _r1 = Rooted::new(results);
        let d = AsyncActionDispatcher::new_gc();
        let _r2 = Rooted::new(d);
        // SAFETY: fresh, rooted objects.
        unsafe {
            for n in 1..=6 {
                (*d.ptr()).queue(act(&log, n, results));
            }
            assert_eq!(run_pending_work_and_dispatchers(), 0, "not started: the thread does not exist");
            AsyncActionDispatcher::start(d);
            set_dispatcher_budget(2);
            next_phase();
            assert_eq!(run_pending_work_and_dispatchers(), 2);
            assert_eq!(*log.borrow(), vec![1, 2]);
            assert_eq!((*d.ptr()).actions_remaining(), 4);
            let mut out = 0;
            assert!((*results.ptr()).try_dequeue(&mut out) && out == 10);
            assert!((*results.ptr()).try_dequeue(&mut out) && out == 20);
            // first empty poll of the phase: no result, nothing runs
            assert!(!(*results.ptr()).try_dequeue(&mut out));
            assert_eq!(log.borrow().len(), 2);
            // second empty poll of the same phase: a busy wait, the worker runs one action
            assert!((*results.ptr()).try_dequeue(&mut out) && out == 30);
            assert_eq!(*log.borrow(), vec![1, 2, 3]);
            // a new phase starts with a plain answer again
            next_phase();
            assert!(!(*results.ptr()).try_dequeue(&mut out));
            assert_eq!(log.borrow().len(), 3);
            AsyncActionDispatcher::stop(d);
            assert_eq!(run_pending_work_and_dispatchers(), 0);
            assert_eq!((*d.ptr()).actions_remaining(), 3);
            AsyncActionDispatcher::dispose(d);
            assert!((*d.ptr()).is_disposed() && !(*d.ptr()).is_running());
            set_dispatcher_budget(64);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::rc::Rc;

    fn action(log: &Rc<RefCell<Vec<&'static str>>>, name: &'static str) -> Action {
        let log = log.clone();
        Delegate::new(Box::new(move || log.borrow_mut().push(name)))
    }

    #[test]
    fn work_is_deferred_and_runs_in_order_or_on_demand() {
        let log = Rc::new(RefCell::new(Vec::new()));
        let thread = Task::new_thread(action(&log, "thread"));
        unsafe {
            (*thread.ptr()).set_name(Str::lit("Server Input Thread"));
            (*thread.ptr()).start();
        }
        let l2 = log.clone();
        thread_pool_queue_user_work_item(Delegate::new(Box::new(move |_o: Obj| l2.borrow_mut().push("pool"))), Gc::NULL);
        let task = Task::start_new(action(&log, "task"));
        assert!(log.borrow().is_empty(), "nothing runs when it is queued");
        assert_eq!(pending_work(), 3);
        // on demand, out of order
        unsafe { assert!((*task.ptr()).is_completed()) };
        assert_eq!(*log.borrow(), ["task"]);
        assert_eq!(run_pending_work(), 2);
        assert_eq!(*log.borrow(), ["task", "thread", "pool"]);
        unsafe {
            (*task.ptr()).wait();
            (*thread.ptr()).join();
            assert!(!(*thread.ptr()).is_alive());
        }
        assert_eq!((log.borrow().len(), pending_work()), (3, 0));
    }

    #[test]
    fn work_queued_by_work_runs_in_the_same_pump() {
        let log = Rc::new(RefCell::new(Vec::new()));
        let inner = action(&log, "inner");
        let l = log.clone();
        Task::start_new(Delegate::new(Box::new(move || {
            l.borrow_mut().push("outer");
            Task::start_new(inner);
        })));
        assert_eq!(run_pending_work(), 2);
        assert_eq!(*log.borrow(), ["outer", "inner"]);
    }

    #[test]
    fn completion_source_queue_monitor_interlocked() {
        let tcs = TaskCompletionSource::<i32>::new_gc();
        unsafe {
            let t = (*tcs.ptr()).task();
            assert!(!(*t.ptr()).is_completed());
            (*tcs.ptr()).set_result(7);
            assert!((*t.ptr()).is_completed());
            assert_eq!((*tcs.ptr()).result(), 7);
            let e = rt::try_(|| (*tcs.ptr()).set_result(8)).unwrap_err();
            assert!(rt::exc_is(e, "System.InvalidOperationException"));

            let q = ConcurrentQueue::<i32>::new_gc();
            (*q.ptr()).enqueue(1);
            (*q.ptr()).enqueue(2);
            let mut v = 0;
            assert!((*q.ptr()).try_dequeue(&mut v) && v == 1);
            assert!((*q.ptr()).try_dequeue(&mut v) && v == 2);
            v = 9;
            assert!(!(*q.ptr()).try_dequeue(&mut v) && v == 0);

            let mut n = i32::MAX;
            assert_eq!(interlocked_increment(&mut n), i32::MIN);
            assert_eq!(interlocked_decrement(&mut n), i32::MAX);
        }
        assert!(monitor_try_enter(rt::new_object()));
        thread_sleep(5);
    }

    #[test]
    fn waiting_for_the_main_thread_is_reported_not_hung() {
        let tcs = TaskCompletionSource::<i32>::new_gc();
        let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| unsafe { (*(*tcs.ptr()).task().ptr()).wait() }));
        let payload = r.unwrap_err();
        let msg = payload.downcast_ref::<String>().cloned().or_else(|| payload.downcast_ref::<&str>().map(|s| s.to_string())).unwrap();
        assert!(msg.contains("TaskCompletionSource"), "{msg}");
    }

    /// `while (flag) Thread.Sleep(0)` in queued work that waits for the main thread (WorldGen.playWorldCallBack
    /// waiting for Main.DoDraw to draw the loaded map): the work is suspended, the "frame" goes on, the work
    /// continues at the next pump and finishes once the main thread has done its part.
    #[cfg(all(windows, target_arch = "x86_64"))]
    #[test]
    fn a_wait_loop_in_queued_work_gives_the_frame_back_and_finishes_later() {
        let lock = Rc::new(Cell::new(true));
        let log: Rc<RefCell<Vec<&'static str>>> = Rc::new(RefCell::new(Vec::new()));
        let (l2, g2) = (lock.clone(), log.clone());
        let t = Task::start_new(Delegate::new(Box::new(move || {
            g2.borrow_mut().push("work: started");
            while l2.get() {
                thread_sleep(0);
            }
            g2.borrow_mut().push("work: the main thread did its part");
        })));
        assert!(!has_suspended_work());
        assert_eq!(run_pending_work(), 1);
        // suspended in the loop, not finished, not hung
        assert!(has_suspended_work());
        assert!(unsafe { !(*t.ptr()).completed && (*t.ptr()).is_alive() });
        assert_eq!(*log.borrow(), ["work: started"]);
        // IsCompleted continues it by one slice and still says no
        assert!(unsafe { !(*t.ptr()).is_completed() });
        // frames pass; nothing changes until the main thread releases the lock
        for _ in 0..5 {
            run_pending_work();
        }
        assert!(has_suspended_work());
        lock.set(false);
        run_pending_work();
        assert!(!has_suspended_work());
        assert!(unsafe { (*t.ptr()).completed });
        assert_eq!(*log.borrow(), ["work: started", "work: the main thread did its part"]);

        // short sleeps of working code do not suspend it
        let done = Rc::new(Cell::new(0));
        let d2 = done.clone();
        Task::start_new(Delegate::new(Box::new(move || {
            for _ in 0..SLEEPS_PER_SLICE - 1 {
                thread_sleep(1);
            }
            d2.set(1);
        })));
        run_pending_work();
        assert_eq!((done.get(), has_suspended_work()), (1, false));

        // Thread.Join from the main thread continues a suspended thread until it is done
        let n = Rc::new(Cell::new(0u32));
        let n2 = n.clone();
        let th = Task::new_thread(Delegate::new(Box::new(move || {
            for _ in 0..3 * SLEEPS_PER_SLICE {
                thread_sleep(0);
                n2.set(n2.get() + 1);
            }
        })));
        unsafe {
            (*th.ptr()).start();
            run_pending_work();
            assert!(has_suspended_work() && n.get() < 3 * SLEEPS_PER_SLICE);
            (*th.ptr()).join();
        }
        assert_eq!((n.get(), has_suspended_work()), (3 * SLEEPS_PER_SLICE, false));

        // work that waits for other work: both are suspended and both finish
        let order: Rc<RefCell<Vec<&'static str>>> = Rc::new(RefCell::new(Vec::new()));
        let gate = Rc::new(Cell::new(true));
        let (o1, g1) = (order.clone(), gate.clone());
        let inner = Task::start_new(Delegate::new(Box::new(move || {
            while g1.get() {
                thread_sleep(0);
            }
            o1.borrow_mut().push("inner");
        })));
        let o2 = order.clone();
        let outer = Task::start_new(Delegate::new(Box::new(move || {
            unsafe { (*inner.ptr()).wait() };
            o2.borrow_mut().push("outer");
        })));
        run_pending_work();
        run_pending_work();
        assert!(order.borrow().is_empty() && has_suspended_work());
        gate.set(false);
        run_pending_work();
        run_pending_work();
        assert_eq!(*order.borrow(), ["inner", "outer"]);
        assert!(unsafe { (*outer.ptr()).completed } && !has_suspended_work());

        // a panic (a stop, an uncaught C# exception) inside suspended work surfaces at the pump that continued it
        let fuse = Rc::new(Cell::new(true));
        let f2 = fuse.clone();
        Task::start_new(Delegate::new(Box::new(move || {
            while f2.get() {
                thread_sleep(0);
            }
            panic!("boom in work");
        })));
        run_pending_work();
        fuse.set(false);
        let payload = std::panic::catch_unwind(run_pending_work).unwrap_err();
        assert_eq!(payload.downcast_ref::<&str>().copied(), Some("boom in work"));
        assert!(!has_suspended_work());
    }

    /// A thread with a resumable form (module docs, "Resumable threads"; the browser build's network thread): it
    /// parks at every sleep, so between pumps nothing is suspended and incremental collections END while it lives;
    /// what only its machine holds survives them (poison mode: a freed array would read 0xDD..); a C# exception
    /// in a later slice of a `try` is caught by [`try_async`]; Thread.Join continues it to the end. The negative
    /// control: an array nothing holds IS freed by the same collections.
    #[cfg(all(windows, target_arch = "x86_64"))]
    #[test]
    fn a_resumable_thread_parks_at_its_sleeps_and_collections_end_while_it_lives() {
        const ID: u64 = 0x5eed_0001_0000_0001;
        thread_local! {
            static SEEN: RefCell<Vec<i32>> = const { RefCell::new(Vec::new()) };
            static CAUGHT: Cell<bool> = const { Cell::new(false) };
        }
        fn body(_recv: usize) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()>>> {
            Box::pin(async {
                // held only by the machine across its sleeps
                let a = rt::Arr::<i32>::new(64);
                unsafe { *a.at(7) = 4242 };
                for _ in 0..6 {
                    sleep_async(1).await;
                    // a freed header reads 0xDD.. (poison mode); only a live one is indexed
                    SEEN.with(|s| s.borrow_mut().push(if a.len() == 64 { unsafe { *a.at(7) } } else { a.len() }));
                }
                let r = try_async::<(), _>(async {
                    sleep_async(1).await;
                    if SEEN.with(|s| s.borrow().len()) == 6 {
                        rt::throw(rt::exc("System.InvalidOperationException", Str::NULL));
                    }
                    rt::Flow::Next
                })
                .await;
                CAUGHT.set(matches!(r, Err(e) if rt::exc_is(e, "System.InvalidOperationException")));
                // a short sleep is the plain Thread.Sleep (counts towards a slice, does not park)
                sleep_async(0).await;
            })
        }
        static TABLE: [(u64, ResumableBody); 1] = [(ID, body)];
        rt::set_poison(true);
        set_resumable_threads(&TABLE);
        let th = Task::new_thread(Action::from_method(Box::new(|| panic!("the plain form ran")), ID, 0, None));
        let control = rt::Arr::<i32>::new(64);
        unsafe {
            *control.at(7) = 4242;
            (*th.ptr()).start();
        }
        let mut cycles = 0;
        for _ in 0..4 {
            run_pending_work();
            // parked: nothing suspended, alive
            assert!(!has_suspended_work() && unsafe { (*th.ptr()).is_alive() });
            assert!(!has_suspended_foreground_work());
            // one whole incremental cycle between two pumps, as the browser driver runs it
            unsafe {
                rt::gc_begin();
                loop {
                    let s = rt::gc_step(rt::GcBudget::Micros(50), !has_suspended_work());
                    if s.finished.is_some() {
                        cycles += 1;
                        break;
                    }
                }
            }
        }
        assert_eq!(cycles, 4);
        assert_eq!(control.len(), 0xDDDD_DDDDu32 as i32, "the negative control: an array nothing holds is freed (its header is poisoned)");
        unsafe { (*th.ptr()).join() };
        set_resumable_threads(&[]);
        rt::set_poison(false);
        assert!(unsafe { (*th.ptr()).completed } && !has_suspended_work());
        assert_eq!(SEEN.with(|s| s.borrow().clone()), vec![4242; 6]);
        assert!(CAUGHT.get(), "the exception of a later slice was caught by the async try");
    }

    /// The browser build's dedicated server (terraria-web server.rs): its main loop is a machine the driver polls
    /// (`poll_main_machine`), parked at EVERY `Thread.Sleep` (0 too) after the queued work ran, so nothing of it is
    /// on a stack between polls; a thread whose start delegate is an INSTANCE method group gets its receiver
    /// (`TcpSocket.ListenLoop`); `park_async` (its `AcceptTcpClient` wait) parks the thread's machine, which the
    /// next pump polls again. Collections end between the polls while the thread lives.
    #[cfg(all(windows, target_arch = "x86_64"))]
    #[test]
    fn the_main_machine_parks_at_every_sleep_and_instance_threads_get_their_receiver() {
        const ID: u64 = 0x5eed_0002_0000_0001;
        thread_local! {
            static LOG: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
            static WAITS: Cell<u32> = const { Cell::new(0) };
        }
        fn body(recv: usize) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()>>> {
            Box::pin(async move {
                // the receiver: the object the method group was taken from
                let me = Gc(recv as *mut Task);
                LOG.with(|l| l.borrow_mut().push(format!("thread on {}", unsafe { (*me.ptr()).name.as_str() })));
                // an AcceptTcpClient that has nothing to accept twice: the machine parks each time
                while WAITS.get() < 2 {
                    WAITS.set(WAITS.get() + 1);
                    park_async().await;
                }
                LOG.with(|l| l.borrow_mut().push("thread accepted".to_owned()));
                sleep_async(1).await;
            })
        }
        static TABLE: [(u64, ResumableBody); 1] = [(ID, body)];
        set_resumable_threads(&TABLE);
        let holder = Task::new_thread(Action::new(Box::new(|| {})));
        unsafe { (*holder.ptr()).set_name(Str::lit("the listener")) };
        let _keep = Rooted::new(holder);
        let t = Task::new_thread(Action::from_method(Box::new(|| panic!("the plain form ran")), ID, holder.addr(), None));
        let _keep_t = Rooted::new(t);
        let mut machine: std::pin::Pin<Box<dyn std::future::Future<Output = ()>>> = Box::pin(async move {
            LOG.with(|l| l.borrow_mut().push("main 1".to_owned()));
            sleep_async(0).await;
            unsafe { (*t.ptr()).start() };
            LOG.with(|l| l.borrow_mut().push("main 2".to_owned()));
            // the sleep runs the queued work first: the thread starts and parks in its accept
            sleep_async(25).await;
            sleep_async(1).await;
            sleep_async(1).await;
            LOG.with(|l| l.borrow_mut().push("main 3".to_owned()));
        });
        let mut polls = Vec::new();
        let mut cycles = 0;
        loop {
            let r = poll_main_machine(machine.as_mut());
            polls.push(r);
            // between polls: nothing suspended (the thread is parked), a whole collection can run
            assert!(!has_suspended_work());
            unsafe {
                rt::gc_begin();
                loop {
                    if rt::gc_step(rt::GcBudget::Micros(50), !has_suspended_work()).finished.is_some() {
                        cycles += 1;
                        break;
                    }
                }
            }
            if r.is_none() {
                break;
            }
        }
        assert_eq!(polls, vec![Some(0), Some(25), Some(1), Some(1), None]);
        assert_eq!(cycles, 5);
        unsafe { (*t.ptr()).join() };
        set_resumable_threads(&[]);
        assert_eq!(LOG.with(|l| l.borrow().clone()), vec!["main 1", "main 2", "thread on the listener", "thread accepted", "main 3"]);
        assert_eq!(WAITS.get(), 2);
        assert!(unsafe { (*t.ptr()).completed } && !has_suspended_work());
    }

    /// Without fibers a wait loop cannot be served: it is reported, not hung.
    #[cfg(not(all(windows, target_arch = "x86_64")))]
    #[test]
    fn a_wait_loop_in_queued_work_is_reported_not_hung() {
        Task::start_new(Delegate::new(Box::new(|| loop {
            thread_sleep(1);
        })));
        let r = std::panic::catch_unwind(run_pending_work);
        assert!(r.is_err());
    }
}

#[cfg(all(test, windows, target_arch = "x86_64"))]
mod yield_tests {
    use super::*;
    use std::rc::Rc;

    /// The yield flag is one per process (the game has one game thread); the tests of this module take turns.
    static TURN: std::sync::Mutex<()> = std::sync::Mutex::new(());
    fn turn() -> std::sync::MutexGuard<'static, ()> {
        TURN.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Objects that only a SUSPENDED work item's frames refer to survive a collection when the fiber stacks are
    /// scanned (`rt::set_conservative_roots` with [`suspended_stacks`]): a class object by its handle, an array
    /// that the frame holds only a pointer INTO (an element address; once deep inside a large array), a list held by an item address. Negative
    /// control: without the scan the same collection frees them (poison mode shows it).
    #[test]
    fn a_collection_keeps_what_only_a_suspended_fiber_holds() {
        let _turn = turn();
        set_yield_mode(YieldMode::Off);
        // a traced object type of this test
        struct Node {
            value: u64,
            next: Gc<Node>,
        }
        // SAFETY: the one handle is reported.
        unsafe impl Trace for Node {
            unsafe fn trace(&self, m: &mut Marker) {
                m.mark(self.next)
            }
        }
        #[inline(never)]
        fn work(report: Rc<Cell<(u64, u64, i32, i32, i32)>>) {
            // a chain of two objects, an array and a list that nothing but this frame knows
            let tail = rt::alloc_init(Node { value: 0x1111_2222_3333_4444, next: Gc::NULL });
            let head = rt::alloc_init(Node { value: 0x5555_6666_7777_8888, next: tail });
            let arr = rt::Arr::<i32>::new(1000);
            // SAFETY: fresh, live objects of this thread.
            let elem = unsafe {
                *arr.at(500) = 77;
                arr.at(500)
            };
            // a pointer 360 KB into a large array's elements: far from the object's own pages (the list of wide
            // objects is what finds it)
            let big = rt::Arr::<i32>::new(100_000);
            // SAFETY: as above.
            let deep = unsafe {
                *big.at(90_000) = 99;
                big.at(90_000)
            };
            let list = rt::List::<i32>::new_gc();
            // SAFETY: as above.
            let item = unsafe {
                for k in 0..100 {
                    (*list.ptr()).add(k);
                }
                (*list.ptr()).item_ptr(42)
            };
            let (elem, item, deep) = (core::hint::black_box(elem), core::hint::black_box(item), core::hint::black_box(deep));
            // the handles of the array and of the list are dead from here on: only `elem` and `item` are used
            assert!(wait_a_frame(), "the work runs on a fiber");
            // SAFETY: alive if the collector saw this frame; in poison mode freed memory reads 0xDD..
            let got = unsafe { ((*head.ptr()).value, (*(*head.ptr()).next.ptr()).value, *elem, *item, *deep) };
            report.set(got);
        }
        for scan in [true, false] {
            rt::set_poison(true);
            rt::set_conservative_roots(if scan { Some(|f: &mut dyn FnMut(*const u8, usize)| suspended_stacks(f)) } else { None });
            let report = Rc::new(Cell::new((0u64, 0u64, 0i32, 0i32, 0i32)));
            let r = report.clone();
            Task::start_new(Delegate::new(Box::new(move || work(r.clone()))));
            run_pending_work();
            assert!(has_suspended_work() && !has_suspended_foreground_work(), "suspended in a host wait: idle");
            // SAFETY: no generated code runs on the main fiber; the suspended fiber is scanned (or not: the control)
            let st = unsafe { rt::collect() };
            if scan {
                run_pending_work();
                assert!(!has_suspended_work());
                assert_eq!(report.get(), (0x5555_6666_7777_8888, 0x1111_2222_3333_4444, 77, 42, 99), "everything the fiber held is intact");
            } else {
                // the control must not dereference freed memory for real: the collection's own count says it
                assert!(st.freed_objects >= 5, "without the scan the collector frees what only the fiber holds ({} objects freed)", st.freed_objects);
                // the fiber is dropped where it stands
                let t = SUSPENDED.with(|s| unsafe { (**s.borrow().first().expect("still suspended")).task.get() });
                // SAFETY: the task object is rooted by its fiber state.
                unsafe { (*t.ptr()).abort() };
                assert!(!has_suspended_work());
            }
        }
        rt::set_conservative_roots(None);
        rt::set_poison(false);
    }

    /// A loop of translated code as the translator emits it: the yield point at the head of the body.
    fn spin(n: u32, count: &Cell<u32>) {
        for _ in 0..n {
            rt::yield_point();
            count.set(count.get() + 1);
        }
    }

    /// With `YieldMode::Loops` running work gives the frame back every n loop iterations and continues at the next
    /// pump; with the mode off it runs to completion. The work has its own value of a thread-static field, and
    /// nothing yields inside a static initialiser.
    #[test]
    fn running_work_gives_the_frame_back_at_yield_points() {
        let _turn = turn();
        // a `[ThreadStatic] static int` (leaked: lives like a static)
        let field: &'static mut i32 = Box::leak(Box::new(0));
        let fp = field as *mut i32 as usize;
        unsafe { rt::register_thread_static(fp as *mut i32) };
        unsafe { *(fp as *mut i32) = 42 };
        let count = Rc::new(Cell::new(0u32));
        let seen = Rc::new(Cell::new(-1));

        set_yield_mode(YieldMode::Off);
        let c = count.clone();
        Task::start_new(Delegate::new(Box::new(move || spin(1000, &c))));
        assert_eq!(run_pending_work(), 1);
        assert_eq!((count.get(), has_suspended_work()), (1000, false), "off: to completion");

        set_yield_mode(YieldMode::Loops(100));
        count.set(0);
        let (c, s) = (count.clone(), seen.clone());
        Task::start_new(Delegate::new(Box::new(move || {
            // a new thread: default value; what it writes is its own
            s.set(unsafe { *(fp as *const i32) });
            unsafe { *(fp as *mut i32) = 7 };
            {
                // a static initialiser that loops: never interrupted
                let _g = rt::CctorGuard::enter();
                spin(250, &c);
            }
            spin(250, &c);
            s.set(s.get() * 100 + unsafe { *(fp as *const i32) });
        })));
        let y0 = yields();
        run_pending_work();
        // the 250 iterations of the initialiser and 99 more: the yield is at the head of the 100th
        assert_eq!((count.get(), has_suspended_work(), seen.get()), (349, true, 0));
        assert_eq!(unsafe { *(fp as *const i32) }, 42, "the main thread keeps its value while the work is suspended");
        let mut pumps = 1;
        while has_suspended_work() {
            run_pending_work();
            pumps += 1;
            assert!(pumps < 10);
        }
        assert_eq!((count.get(), pumps, yields() - y0), (500, 3, 2));
        assert_eq!(seen.get(), 7, "the work saw its own value after being suspended");
        assert_eq!(unsafe { *(fp as *const i32) }, 42);
        // on the main fiber an armed flag does nothing
        rt::threads::YIELD_ARMED.store(true, std::sync::atomic::Ordering::Relaxed);
        spin(10, &count);
        assert_eq!(count.get(), 510);
        set_yield_mode(YieldMode::Off);
    }

    /// A lock held by a suspended work item: the main thread entering it continues the work until the lock is
    /// free (on .NET the main thread would block that long), so it never sees the half-done state.
    #[test]
    fn a_lock_held_by_suspended_work_is_waited_for() {
        let _turn = turn();
        set_yield_mode(YieldMode::Loops(10));
        let lock = rt::new_object();
        let _r = Rooted::new(lock);
        let state = Rc::new(Cell::new(0u32));
        let count = Rc::new(Cell::new(0u32));
        let (s, c) = (state.clone(), count.clone());
        Task::start_new(Delegate::new(Box::new(move || {
            monitor_enter(lock);
            s.set(1); // half done
            spin(35, &c);
            s.set(2);
            monitor_exit(lock);
            spin(35, &c);
            s.set(3);
        })));
        run_pending_work();
        assert_eq!((state.get(), has_suspended_work()), (1, true), "suspended inside the lock");
        assert!(!monitor_try_enter(lock), "TryEnter does not wait");
        assert!(monitor_enter(lock));
        assert_eq!(state.get(), 2, "the work ran until it left the lock, not further than its next yield");
        assert!(has_suspended_work());
        // re-entrant for the owner
        assert!(monitor_try_enter(lock));
        monitor_exit(lock);
        monitor_exit(lock);
        while has_suspended_work() {
            run_pending_work();
        }
        assert_eq!((state.get(), count.get()), (3, 70));
        set_yield_mode(YieldMode::Off);
    }

    /// Work that a work item runs inline (a thread of its own in the original) and that gives the frame back while
    /// it holds a lock: it continues with ITS thread-static values and can leave its lock (it once came back
    /// under the outer work's context, could not, and the lock stayed taken for good).
    #[test]
    fn inline_work_that_yields_keeps_its_context_and_its_lock() {
        let _turn = turn();
        set_yield_mode(YieldMode::Loops(10));
        let field: &'static mut i32 = Box::leak(Box::new(0));
        let fp = field as *mut i32 as usize;
        unsafe { rt::register_thread_static(fp as *mut i32) };
        let lock = rt::new_object();
        let _r = Rooted::new(lock);
        let count = Rc::new(Cell::new(0u32));
        let seen = Rc::new(Cell::new(0));
        let (c, sn) = (count.clone(), seen.clone());
        Task::start_new(Delegate::new(Box::new(move || {
            unsafe { *(fp as *mut i32) = 1 }; // the outer work's value
            let (c2, sn2) = (c.clone(), sn.clone());
            let inner = Task::start_new(Delegate::new(Box::new(move || {
                unsafe { *(fp as *mut i32) = 2 };
                monitor_enter(lock);
                spin(25, &c2); // yields twice inside the lock
                sn2.set(unsafe { *(fp as *const i32) });
                monitor_exit(lock);
            })));
            // Task.Wait(): the inner work runs inline, on this fiber
            unsafe { (*inner.ptr()).wait() };
            sn.set(sn.get() * 10 + unsafe { *(fp as *const i32) });
        })));
        run_pending_work();
        assert!(has_suspended_work());
        assert!(!monitor_try_enter(lock), "the inline work holds the lock while it is suspended");
        while has_suspended_work() {
            run_pending_work();
        }
        assert_eq!((count.get(), seen.get()), (25, 21), "inner saw 2, outer saw 1 again");
        assert!(monitor_try_enter(lock), "the lock was released");
        monitor_exit(lock);
        set_yield_mode(YieldMode::Off);
    }

    /// `YieldMode::Millis`: the timer arms the flag after the slice. The work runs until it has been continued
    /// three times (or ten seconds have passed: then the timer never fired and the first pump finishes it), so the
    /// test does not depend on how busy the machine is.
    #[test]
    fn a_time_slice_ends_running_work() {
        let _turn = turn();
        set_yield_mode(YieldMode::Millis(5));
        let pumps = Rc::new(Cell::new(0u32));
        let p = pumps.clone();
        Task::start_new(Delegate::new(Box::new(move || {
            let t = std::time::Instant::now();
            while p.get() < 4 && t.elapsed() < std::time::Duration::from_secs(10) {
                rt::yield_point();
            }
        })));
        pumps.set(1);
        run_pending_work();
        assert!(has_suspended_work(), "the work was not interrupted by its time slice");
        while has_suspended_work() {
            pumps.set(pumps.get() + 1);
            run_pending_work();
            assert!(pumps.get() < 100);
        }
        assert_eq!(pumps.get(), 4, "one slice per pump");
        set_yield_mode(YieldMode::Off);
    }
}
