//! What "another thread" needs in a runtime where every thread of the original program is a fiber of one OS
//! thread (`terraria-host` `sys/threading.rs`): `[ThreadStatic]` fields with a value per thread, and a place where
//! long-running work can give the processor back.
//!
//! # `[ThreadStatic]`
//! A thread-static field lives in its class's `Statics` struct like any static field, and holds the value of the
//! thread that is running. Each static initialiser registers its thread-static fields ([`register_thread_static`]).
//! A *context* is one thread's set of values while that thread is not running: the host creates one per work item
//! ([`context_new`]: every field is `default(T)`, as on a new .NET thread) and swaps on every switch
//! ([`context_switch`]). Context 0 is the main thread's. The values of suspended threads are collector roots.
//!
//! # Yield points
//! The translator puts [`yield_point`] at the head of every loop body. It is one load of a flag that is false
//! unless the host armed it ([`YIELD_ARMED`]: a work item is running and should give the frame back); then the
//! host's hook runs ([`set_yield_hook`]) and may switch to the main fiber. Never inside a static initialiser
//! ([`CctorGuard`]): on .NET another thread that touches the class waits for the initialiser to finish; here it
//! would see the class half initialised.

use super::gc::{Marker, Trace};
use std::cell::{Cell, RefCell};
use std::sync::atomic::{AtomicBool, Ordering};

struct Slot {
    p: *mut u8,
    size: usize,
    /// offset in a context's buffer (16-byte aligned)
    offset: usize,
    trace: unsafe fn(*const u8, &mut Marker),
}

#[derive(Default)]
struct State {
    slots: Vec<Slot>,
    /// bytes a context needs for the slots registered so far
    len: usize,
    /// context id -> the values of that thread while it does not run (u128: aligned for any field)
    contexts: std::collections::HashMap<u64, Vec<u128>>,
    next: u64,
    current: u64,
    root: bool,
}

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
    static CCTOR_DEPTH: Cell<u32> = const { Cell::new(0) };
    static YIELD_HOOK: Cell<Option<fn()>> = const { Cell::new(None) };
}

unsafe fn trace_thunk<T: Trace>(p: *const u8, m: &mut Marker) {
    (*(p as *const T)).trace(m)
}

/// Generated static initialisers: `field` is a `[ThreadStatic]` field of the class's statics.
///
/// # Safety
/// `field` must point at a static field that lives for the rest of the program.
pub unsafe fn register_thread_static<T: Trace + 'static>(field: *mut T) {
    const { assert!(core::mem::align_of::<T>() <= 16) };
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        if s.slots.iter().any(|x| x.p == field as *mut u8) {
            return;
        }
        let offset = s.len;
        let size = core::mem::size_of::<T>();
        s.len += size.div_ceil(16) * 16;
        s.slots.push(Slot { p: field as *mut u8, size, offset, trace: trace_thunk::<T> });
        if !s.root {
            s.root = true;
            drop(s);
            // the values of the threads that are not running
            super::gc::add_root(|m| {
                STATE.with(|s| {
                    let s = s.borrow();
                    for buf in s.contexts.values() {
                        for slot in &s.slots {
                            if (slot.offset + slot.size).div_ceil(16) <= buf.len() {
                                // SAFETY: the buffer holds a valid (or zero) value of the slot's type at its offset.
                                unsafe { (slot.trace)((buf.as_ptr() as *const u8).add(slot.offset), m) }
                            }
                        }
                    }
                })
            });
        }
    })
}

/// A new thread's context: every thread-static field is `default(T)`.
pub fn context_new() -> u64 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.next += 1;
        let id = s.next;
        s.contexts.insert(id, Vec::new());
        id
    })
}

/// The thread whose values are in the static fields now (0: the main thread).
pub fn context_current() -> u64 {
    STATE.with(|s| s.borrow().current)
}

/// The running thread stops and thread `to` runs: the fields' values go to the running thread's context, the
/// values of `to` come into the fields. No effect when `to` is the running thread.
pub fn context_switch(to: u64) {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        let s = &mut *s;
        if s.current == to {
            return;
        }
        let words = s.len / 16;
        let mut out = s.contexts.remove(&s.current).unwrap_or_default();
        out.resize(words, 0);
        let mut inp = s.contexts.remove(&to).unwrap_or_default();
        // fields registered after the context was saved have their default value in it
        inp.resize(words, 0);
        for slot in &s.slots {
            // SAFETY: the slots are registered static fields; the buffers are large enough and aligned.
            unsafe {
                core::ptr::copy_nonoverlapping(slot.p, (out.as_mut_ptr() as *mut u8).add(slot.offset), slot.size);
                core::ptr::copy_nonoverlapping((inp.as_ptr() as *const u8).add(slot.offset), slot.p, slot.size);
            }
        }
        s.contexts.insert(s.current, out);
        s.current = to;
    })
}

/// The thread of context `id` has ended (it must not be the running one).
pub fn context_drop(id: u64) {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        assert!(s.current != id, "cs2rs runtime: the running thread's context was dropped");
        s.contexts.remove(&id);
    })
}

/// `true` while the host wants the running code to stop at its next [`yield_point`].
pub static YIELD_ARMED: AtomicBool = AtomicBool::new(false);

/// The head of every translated loop body.
#[inline(always)]
pub fn yield_point() {
    if YIELD_ARMED.load(Ordering::Relaxed) {
        yield_slow()
    }
}

#[cold]
#[inline(never)]
fn yield_slow() {
    if CCTOR_DEPTH.get() != 0 {
        return;
    }
    if let Some(h) = YIELD_HOOK.get() {
        h()
    }
}

/// The host's reaction to an armed [`yield_point`] on this thread (it decides whether to switch fibers and
/// disarms the flag).
pub fn set_yield_hook(hook: Option<fn()>) {
    YIELD_HOOK.set(hook)
}

/// Held by a running static initialiser: no yield inside (module docs).
pub struct CctorGuard(());
impl CctorGuard {
    #[inline]
    pub fn enter() -> CctorGuard {
        CCTOR_DEPTH.set(CCTOR_DEPTH.get() + 1);
        CctorGuard(())
    }
}
impl Drop for CctorGuard {
    #[inline]
    fn drop(&mut self) {
        CCTOR_DEPTH.set(CCTOR_DEPTH.get() - 1);
    }
}

/// Is a static initialiser running on this thread?
pub fn in_cctor() -> bool {
    CCTOR_DEPTH.get() != 0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rt::{alloc, collect, Gc, Rooted};

    #[repr(C)]
    #[derive(Clone, Copy)]
    struct Leaf {
        v: i32,
    }
    unsafe impl Trace for Leaf {
        const NEEDS_TRACE: bool = false;
        const RESCAN: bool = false;
        unsafe fn trace(&self, _m: &mut Marker) {}
    }

    #[test]
    fn thread_static_values_follow_the_context() {
        // two "static fields": a handle and a number
        let field_h: &'static mut Gc<Leaf> = Box::leak(Box::new(Gc::NULL));
        let field_n: &'static mut i32 = Box::leak(Box::new(0));
        unsafe {
            // as the class's `__trace_statics` does for the real ones
            let hp = field_h as *mut Gc<Leaf> as usize;
            let root = crate::rt::add_root(move |m| m.mark(*(hp as *const Gc<Leaf>)));
            register_thread_static(field_h as *mut Gc<Leaf>);
            let a = alloc::<Leaf>(0);
            (*a.0).v = 7;
            *field_h = a;
            let worker = context_new();
            // a field registered after the context was made
            register_thread_static(field_n as *mut i32);
            *field_n = 5;
            context_switch(worker);
            assert!(field_h.is_null() && *field_n == 0, "a new thread sees default values");
            let b = alloc::<Leaf>(0);
            (*b.0).v = 9;
            *field_h = b;
            *field_n = 11;
            // the main thread's values are roots while the worker runs
            let keep = Rooted::new(Gc::<Leaf>::NULL);
            crate::rt::set_poison(true);
            assert_eq!(collect().freed_objects, 0);
            context_switch(0);
            assert_eq!(((*field_h.ptr()).v, *field_n), (7, 5));
            // and the suspended worker's are too
            assert_eq!(collect().freed_objects, 0);
            context_switch(worker);
            assert_eq!(((*field_h.ptr()).v, *field_n), (9, 11));
            context_switch(0);
            context_drop(worker);
            assert_eq!(collect().freed_objects, 1, "the ended thread's value is garbage");
            assert_eq!((*field_h.ptr()).v, 7);
            *field_h = Gc::NULL;
            crate::rt::remove_root(root);
            drop(keep);
        }
    }

    #[test]
    fn yield_point_calls_the_hook_only_when_armed_and_outside_static_initialisers() {
        thread_local! { static HITS: Cell<u32> = const { Cell::new(0) }; }
        fn hook() {
            HITS.set(HITS.get() + 1)
        }
        set_yield_hook(Some(hook));
        // the flag is process-wide; this thread's hook only counts this thread's calls
        yield_slow();
        assert_eq!(HITS.get(), 1);
        {
            let _g = CctorGuard::enter();
            assert!(in_cctor());
            yield_slow();
            assert_eq!(HITS.get(), 1);
        }
        yield_slow();
        assert_eq!(HITS.get(), 2);
        set_yield_hook(None);
        yield_slow();
        assert_eq!(HITS.get(), 2);
    }
}
