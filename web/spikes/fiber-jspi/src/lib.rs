//! A model of `terraria-host/src/sys/threading.rs` on wasm32-unknown-unknown.
//!
//! - A work item (`work_entry`, exported; run.mjs wraps it with `WebAssembly.promising`) runs deep recursive code
//!   with address-taken locals (those live on the SHADOW stack in linear memory) and values held only in wasm
//!   locals (those live on the engine's own stack), and calls `yield_point()` in its loops, as translated code does.
//! - An armed yield point calls the import `host.suspend` (wrapped with `WebAssembly.Suspending`): the engine
//!   suspends the whole wasm stack of the work item, the frame loop goes on and the main "fiber" calls `frame()`,
//!   which uses the shadow stack too.
//! - The work is resumed at the next pump. Each fiber has its own shadow stack: run.mjs swaps `__stack_pointer`
//!   (exported with `-C link-arg=--export=__stack_pointer`). `run.mjs noswap` is the negative control.
use std::hint::black_box;

#[link(wasm_import_module = "host")]
extern "C" {
    /// gives the frame back (JSPI suspends the caller's wasm stack here); `fiber` is the caller's id
    fn suspend(fiber: u32);
}

static mut YIELD_EVERY: u32 = 0;
static mut LEFT: u32 = 1;
static mut CURRENT: u32 = 0;
static mut YIELDS: u32 = 0;

/// `rt::yield_point()`: one load and a branch unless armed
#[inline(always)]
fn yield_point() {
    unsafe {
        if YIELD_EVERY == 0 {
            return;
        }
        LEFT -= 1;
        if LEFT == 0 {
            LEFT = YIELD_EVERY;
            YIELDS += 1;
            let me = CURRENT;
            suspend(me);
            // another fiber may have run in between: this one is current again
            CURRENT = me;
        }
    }
}

#[no_mangle]
pub extern "C" fn set_yield_every(n: u32) {
    unsafe {
        YIELD_EVERY = n;
        LEFT = n.max(1);
    }
}

#[no_mangle]
pub extern "C" fn yields() -> u32 {
    unsafe { YIELDS }
}

/// A fiber's shadow stack: `size` bytes; returns the initial stack pointer (its top, 16-aligned). Leaked on purpose.
#[no_mangle]
pub extern "C" fn alloc_stack(size: u32) -> u32 {
    let v: Vec<u8> = vec![0xAA; size as usize];
    let top = v.as_ptr() as usize + v.len();
    std::mem::forget(v);
    (top & !15) as u32
}

/// Recursion `depth` deep; every frame keeps an address-taken array (shadow stack) and a value only in a local.
#[inline(never)]
fn deep(fiber: u32, depth: u32, seed: u64) -> u64 {
    let mut frame = [0u64; 8];
    for (i, f) in frame.iter_mut().enumerate() {
        *f = seed.wrapping_mul(0x9E37_79B9_7F4A_7C15).wrapping_add(i as u64 + depth as u64);
    }
    let frame = black_box(&mut frame);
    // held in a wasm local across the suspensions below (never stored to memory by this code)
    let local = seed ^ (fiber as u64).rotate_left(17) ^ 0x5151_0000_0000_0000;
    let mut acc = 0u64;
    for i in 0..4u64 {
        yield_point();
        acc = acc.wrapping_add(frame[i as usize] ^ local);
    }
    if depth > 0 {
        acc = acc.wrapping_add(deep(fiber, depth - 1, seed.wrapping_add(1)));
    }
    for f in frame.iter() {
        acc = acc.rotate_left(5) ^ *f;
    }
    acc ^ local
}

/// The work item. Its result must equal `reference(fiber, depth)` (the same computation without suspension).
#[no_mangle]
pub extern "C" fn work_entry(fiber: u32, depth: u32) -> u64 {
    unsafe {
        CURRENT = fiber;
    }
    deep(fiber, depth, fiber as u64 * 1000)
}

#[no_mangle]
pub extern "C" fn reference(fiber: u32, depth: u32) -> u64 {
    let saved = unsafe { YIELD_EVERY };
    unsafe {
        YIELD_EVERY = 0;
    }
    let r = deep(fiber, depth, fiber as u64 * 1000);
    unsafe {
        YIELD_EVERY = saved;
    }
    r
}

/// The main thread's frame (`Update` / `Draw`): uses the shadow stack itself and checks its own result
/// (1 = consistent). With a shared shadow stack it overwrites the suspended fibers' frames.
#[no_mangle]
pub extern "C" fn frame(n: u32) -> u32 {
    #[inline(never)]
    fn rec(d: u32) -> u64 {
        let mut buf = [d as u64; 32];
        let b = black_box(&mut buf);
        b[d as usize % 32] = b[0].wrapping_mul(3);
        let s: u64 = b.iter().sum();
        if d == 0 {
            s
        } else {
            s ^ rec(d - 1)
        }
    }
    let saved = unsafe { YIELD_EVERY };
    unsafe {
        YIELD_EVERY = 0;
    }
    let a = rec(64 + n % 16);
    let b = rec(64 + n % 16);
    unsafe {
        YIELD_EVERY = saved;
    }
    (a == b) as u32
}

/// Recursion `n` deep, each frame keeping 8 values live across the call (wasm locals, not the shadow stack):
/// depth.mjs searches the deepest `n` that fits the engine's stack, on the main stack and on a JSPI stack.
#[no_mangle]
pub extern "C" fn probe_depth(n: u32) -> u64 {
    #[inline(never)]
    fn r(n: u32, a: u64) -> u64 {
        if n == 0 {
            return a;
        }
        // eight values in wasm locals (no address taken: nothing goes to the shadow stack)
        let m = n as u64;
        let (v0, v1, v2, v3) = (a ^ m, a.rotate_left(7) ^ m, a.wrapping_mul(m | 1), a.wrapping_add(m << 3));
        let (v4, v5, v6, v7) = (v0 ^ v3, v1.wrapping_mul(v2 | 1), v2 ^ (m << 11), v3.rotate_left(29));
        let inner = r(n - 1, v0 ^ v7);
        inner.rotate_left(3) ^ v0 ^ v1.rotate_left(5) ^ v2 ^ v3.rotate_left(9) ^ v4 ^ v5.rotate_left(13) ^ v6 ^ v7
    }
    r(n, 1)
}

/// The same from inside a work item that has suspended once first (so it runs on a JSPI secondary stack).
#[no_mangle]
pub extern "C" fn probe_depth_on_fiber(n: u32) -> u64 {
    unsafe { suspend(99) };
    probe_depth(n)
}

// ------------------------------------------------------------------------------------------------ scheduler in wasm
// The shape of the real code: the MAIN fiber is itself inside wasm (`Main.Update` -> `run_pending_work` -> `resume`,
// threading.rs) when it continues a work item. sched.mjs wraps `main_frame` with `WebAssembly.promising` and makes
// `host.resume` a `WebAssembly.Suspending` import: the main stack is suspended while the work item runs, and the work
// item's own suspension resumes it.

#[link(wasm_import_module = "host")]
extern "C" {
    /// start or continue work item `fiber` until it gives the frame back (0) or finishes (1)
    fn resume(fiber: u32) -> u32;
}

static mut DONE: [bool; 4] = [false; 4];
static mut RESULTS: [u64; 4] = [0; 4];

/// `work_entry` that leaves its result where `main_frame` can read it.
#[no_mangle]
pub extern "C" fn work_entry_stored(fiber: u32, depth: u32) {
    let r = work_entry(fiber, depth);
    unsafe { RESULTS[fiber as usize] = r };
}

#[no_mangle]
pub extern "C" fn result_of(fiber: u32) -> u64 {
    unsafe { RESULTS[fiber as usize] }
}

/// One frame of the main fiber: Update/Draw work on the main shadow stack, then run_pending_work. Returns the
/// number of work items still unfinished, or 1000 + n if the main stack's own check failed.
#[no_mangle]
pub extern "C" fn main_frame(n: u32) -> u32 {
    // a value of the main fiber's frame that must survive its own suspension inside `resume`
    let mut mine = [n as u64; 16];
    let mine = black_box(&mut mine);
    let ok = frame(n);
    let mut alive = 0;
    for f in 1..=3u32 {
        unsafe {
            if DONE[f as usize] {
                continue;
            }
            if resume(f) == 1 {
                DONE[f as usize] = true;
            } else {
                alive += 1;
            }
        }
    }
    let intact = mine.iter().all(|v| *v == n as u64);
    if ok == 1 && intact { alive } else { 1000 + alive }
}
