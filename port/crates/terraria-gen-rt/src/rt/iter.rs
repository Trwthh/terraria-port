//! `yield` iterator methods (DESIGN.md section 8d).
//!
//! The translator emits the body of an iterator method unchanged inside an `async` block; `yield return v` is
//! `__y.put(v).await` and `yield break` is `return`. rustc turns the block into a state machine, exactly what the C#
//! compiler does to an iterator method, so the semantics come for free: nothing runs before the first `MoveNext`,
//! each `MoveNext` runs to the next `yield`, locals keep their values in between, an exception leaves through
//! `MoveNext` and ends the iterator. [`Machine::move_next`] is one `poll` with a waker that does nothing.
//!
//! Collector: the state machine is a compiler-made `Future`, so its layout is unknown to the tracer. A machine
//! that is suspended is scanned conservatively ([`Marker::mark_conservative`]); a finished one holds nothing.
//! - An `IEnumerator` object ([`IterObj`], what a method returning `IEnumerator` / `IEnumerator<T>` gives) is a
//!   heap object and scans its machine when it is reached.
//! - An enumerator made from an `IEnumerable<T>` iterator method ([`enumerable`]) lives in a Rust `Box` owned by
//!   whoever enumerates (a `foreach` on the stack, or another suspended machine). It registers itself as a root
//!   for as long as it exists.
//!
//! Not modelled: `IEnumerator.Reset()` (throws `NotSupportedException`, as compiler-made iterators do);
//! `Dispose()` of a suspended iterator ends it and drops its state, but `finally` blocks around the `yield` it is
//! suspended at do not run (the translator refuses `yield` inside `try`, so there are none).

use super::collections::{Enumerator, Seq};
use super::gc::{alloc_init, Marker, Obj, Trace};
use super::Val;
use std::any::TypeId;
use std::cell::{Cell, RefCell};
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll, Waker};

/// Handed to the iterator body: where `yield return` puts its value.
pub struct Yielder<T: Val> {
    slot: *const Cell<Option<T>>,
}
impl<T: Val> Clone for Yielder<T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T: Val> Copy for Yielder<T> {}

impl<T: Val> Yielder<T> {
    /// `yield return v;` is `__y.put(v).await;`
    pub fn put(self, v: T) -> YieldPoint {
        // SAFETY: the slot is owned by the machine that polls the future holding this yielder.
        unsafe { (*self.slot).set(Some(v)) };
        YieldPoint(false)
    }
}

/// Suspends the state machine once.
pub struct YieldPoint(bool);
impl Future for YieldPoint {
    type Output = ();
    fn poll(mut self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<()> {
        if self.0 {
            Poll::Ready(())
        } else {
            self.0 = true;
            Poll::Pending
        }
    }
}

/// One running iterator method.
pub struct Machine<T: Val> {
    fut: Option<Pin<Box<dyn Future<Output = ()>>>>,
    slot: Box<Cell<Option<T>>>,
    cur: T,
    root: u64,
}

impl<T: Val> Machine<T> {
    pub fn new<F: Future<Output = ()> + 'static>(make: impl FnOnce(Yielder<T>) -> F) -> Machine<T> {
        let slot: Box<Cell<Option<T>>> = Box::new(Cell::new(None));
        let y = Yielder { slot: &*slot as *const Cell<Option<T>> };
        // SAFETY: the zero pattern is `default(T)` (rt::zero).
        Machine { fut: Some(Box::pin(make(y))), slot, cur: unsafe { core::mem::zeroed() }, root: 0 }
    }
    /// # Safety
    /// As [`Trace::trace`].
    unsafe fn trace_machine(&self, m: &mut Marker) {
        self.cur.trace(m);
        if let Some(v) = self.slot.get() {
            v.trace(m);
        }
        if let Some(f) = &self.fut {
            let r: &dyn Future<Output = ()> = &**f;
            m.mark_conservative(r as *const dyn Future<Output = ()> as *const u8, core::mem::size_of_val(r));
        }
    }
}

impl<T: Val> Enumerator for Machine<T> {
    type Item = T;
    fn move_next(&mut self) -> bool {
        // taken out while it runs: an exception (a panic) drops the state machine, so the iterator is finished,
        // as in C#
        let Some(mut fut) = self.fut.take() else { return false };
        let mut cx = Context::from_waker(Waker::noop());
        match fut.as_mut().poll(&mut cx) {
            Poll::Pending => {
                match self.slot.take() {
                    Some(v) => self.cur = v,
                    None => panic!("rt::iter: the iterator suspended without a yield"),
                }
                self.fut = Some(fut);
                true
            }
            Poll::Ready(()) => false,
        }
    }
    fn current(&self) -> T {
        self.cur
    }
}

type ScanEntry = (u64, *const u8, unsafe fn(*const u8, &mut Marker));

thread_local! {
    /// Machines owned by Rust boxes (see [`enumerable`]): address + scan thunk, by root id.
    static BOXED: RefCell<Vec<ScanEntry>> = const { RefCell::new(Vec::new()) };
    static BOXED_ROOT: Cell<u64> = const { Cell::new(0) };
    static NEXT: Cell<u64> = const { Cell::new(0) };
}

unsafe fn scan_thunk<T: Val>(p: *const u8, m: &mut Marker) {
    (*(p as *const Machine<T>)).trace_machine(m)
}

impl<T: Val> Drop for Machine<T> {
    fn drop(&mut self) {
        if self.root != 0 {
            let id = self.root;
            let _ = BOXED.try_with(|b| b.borrow_mut().retain(|(k, _, _)| *k != id));
        }
    }
}

fn register_boxed<T: Val>(b: &mut Box<Machine<T>>) {
    if BOXED_ROOT.get() == 0 {
        // one collector root for all boxed machines of this thread
        BOXED_ROOT.set(super::gc::add_root(|m| {
            BOXED.with(|b| {
                for (_, p, f) in b.borrow().iter() {
                    // SAFETY: an entry is removed when its machine is dropped.
                    unsafe { f(*p, m) }
                }
            })
        }));
    }
    let id = NEXT.get() + 1;
    NEXT.set(id);
    b.root = id;
    BOXED.with(|l| l.borrow_mut().push((id, &**b as *const Machine<T> as *const u8, scan_thunk::<T>)));
}

/// An iterator method returning `IEnumerable<T>`: every `GetEnumerator()` starts the body again with the
/// arguments as they were at the call (`make` captured them by value). `tracer` reports the handles `make` holds.
pub fn enumerable<T: Val, F: Future<Output = ()> + 'static>(make: impl Fn(Yielder<T>) -> F + 'static, tracer: impl Fn(&mut Marker) + 'static) -> Seq<T> {
    Seq::lazy(
        move || {
            let mut b = Box::new(Machine::new(&make));
            register_boxed(&mut b);
            b
        },
        tracer,
    )
}

/// What an [`IterObj`] holds: an enumerator whose element type is erased.
trait AnyEnum {
    fn move_next(&mut self) -> bool;
    fn current_obj(&self) -> Obj;
    /// Writes the current value to `out` when `ty` is the element type.
    unsafe fn current_raw(&self, out: *mut u8, ty: TypeId) -> bool;
    unsafe fn trace(&self, m: &mut Marker);
    fn dispose(&mut self);
}

impl<T: Val> AnyEnum for Machine<T> {
    fn move_next(&mut self) -> bool {
        Enumerator::move_next(self)
    }
    fn current_obj(&self) -> Obj {
        self.cur.to_obj()
    }
    unsafe fn current_raw(&self, out: *mut u8, ty: TypeId) -> bool {
        if ty != TypeId::of::<T>() {
            return false;
        }
        (out as *mut T).write(self.cur);
        true
    }
    unsafe fn trace(&self, m: &mut Marker) {
        self.trace_machine(m)
    }
    fn dispose(&mut self) {
        self.fut = None;
    }
}

/// Any runtime enumerator as an object, with the tracer of what it holds (`collection.GetEnumerator()` kept in a
/// variable of type `IEnumerator<T>`).
struct Wrapped<E: Enumerator> {
    e: E,
    tracer: Box<dyn Fn(&mut Marker)>,
}
impl<E: Enumerator> AnyEnum for Wrapped<E> {
    fn move_next(&mut self) -> bool {
        self.e.move_next()
    }
    fn current_obj(&self) -> Obj {
        self.e.current().to_obj()
    }
    unsafe fn current_raw(&self, out: *mut u8, ty: TypeId) -> bool {
        if ty != TypeId::of::<E::Item>() {
            return false;
        }
        (out as *mut E::Item).write(self.e.current());
        true
    }
    unsafe fn trace(&self, m: &mut Marker) {
        (self.tracer)(m);
        self.e.current().trace(m);
    }
    fn dispose(&mut self) {}
}

/// An `IEnumerator` / `IEnumerator<T>` object of the runtime.
pub struct IterObj {
    e: Box<dyn AnyEnum>,
}
unsafe impl Trace for IterObj {
    unsafe fn trace(&self, m: &mut Marker) {
        self.e.trace(m)
    }
    unsafe fn obj_dispose(this: *mut Self) {
        (*this).e.dispose()
    }
}

/// An iterator method returning `IEnumerator` / `IEnumerator<T>`.
pub fn enumerator<T: Val, F: Future<Output = ()> + 'static>(make: impl FnOnce(Yielder<T>) -> F) -> Obj {
    alloc_init(IterObj { e: Box::new(Machine::new(make)) }).up()
}

/// A runtime enumerator as an `IEnumerator<T>` object; `tracer` reports the collection it walks.
pub fn enumerator_of<E: Enumerator + 'static>(e: E, tracer: impl Fn(&mut Marker) + 'static) -> Obj {
    alloc_init(IterObj { e: Box::new(Wrapped { e, tracer: Box::new(tracer) }) }).up()
}

/// A `List<T>.Enumerator` struct boxed as `IEnumerator<T>`: an enumerator object over a copy of it.
pub fn enumerator_of_list_enum<T: Val>(e: super::ListEnum<T>) -> Obj {
    let l = e.list();
    enumerator_of(e, move |m| m.mark(l))
}

/// `seq.GetEnumerator()` as an object.
pub fn enumerator_of_seq<T: Val>(s: Seq<T>) -> Obj {
    use super::collections::Enumerable;
    let e = s.enumerator();
    enumerator_of(e, move |m| unsafe { s.trace(m) })
}

/// Enumerator over an object's own `GetEnumerator()` result (see [`seq_of_object`]).
struct ObjectEnum<T: Val> {
    e: Obj,
    move_next: fn(Obj) -> bool,
    current: fn(Obj) -> T,
    root: u64,
}
impl<T: Val> Enumerator for ObjectEnum<T> {
    type Item = T;
    fn move_next(&mut self) -> bool {
        (self.move_next)(self.e)
    }
    fn current(&self) -> T {
        (self.current)(self.e)
    }
}
impl<T: Val> Drop for ObjectEnum<T> {
    fn drop(&mut self) {
        super::gc::remove_root(self.root);
    }
}

/// A translated class that implements `IEnumerable<T>` seen as a sequence: every enumeration calls the class's
/// `GetEnumerator()` (`get`) and drives the object it returns with `move_next` / `current` (the dispatchers of
/// `IEnumerator` / `IEnumerator<T>`). The enumerator object is a root while the enumeration exists (it lives in a
/// Rust box, not in a traced object). Null stays null.
pub fn seq_of_object<C: 'static, T: Val>(source: super::Gc<C>, get: fn(super::Gc<C>) -> Obj, move_next: fn(Obj) -> bool, current: fn(Obj) -> T) -> Seq<T> {
    if source.is_null() {
        return Seq::NULL;
    }
    Seq::lazy(
        move || {
            let e = get(source);
            let root = super::gc::add_root(move |m| m.mark(e));
            Box::new(ObjectEnum { e, move_next, current, root })
        },
        move |m| m.mark(source),
    )
}

/// Is `o` an enumerator object of the runtime?
pub fn is_iter(o: Obj) -> bool {
    super::gc::obj_is::<IterObj>(o)
}

fn it<'a>(o: Obj) -> &'a mut IterObj {
    if !is_iter(o) {
        // null: NullReferenceException
        let _ = o.ptr();
        panic!("rt::iter: IEnumerator member called on an object that is not a runtime enumerator ({})", super::gc::obj_to_string(o).get().unwrap_or("?"));
    }
    // SAFETY: type checked above; the reference ends with the caller's statement (generated code keeps handles only).
    unsafe { &mut *(o.0 as *mut IterObj) }
}

/// `IEnumerator.MoveNext()`
pub fn move_next(o: Obj) -> bool {
    it(o).e.move_next()
}
/// `IEnumerator.Current` (boxed)
pub fn current_obj(o: Obj) -> Obj {
    it(o).e.current_obj()
}
/// `IEnumerator<T>.Current`. A non-generic iterator (`IEnumerator`, element type `object`) read as `T` unboxes.
pub fn current<T: Val>(o: Obj) -> T {
    let e = it(o);
    // SAFETY: `current_raw` writes a `T` only when the element type is `T`; otherwise the boxed value is converted
    // as a C# cast would.
    unsafe {
        let mut out = core::mem::MaybeUninit::<T>::zeroed();
        if e.e.current_raw(out.as_mut_ptr() as *mut u8, TypeId::of::<T>()) {
            out.assume_init()
        } else {
            T::from_obj(e.e.current_obj())
        }
    }
}
/// `IDisposable.Dispose()` of an enumerator object.
pub fn dispose(o: Obj) {
    it(o).e.dispose()
}
/// `IEnumerator.Reset()`
pub fn reset(_o: Obj) -> ! {
    super::throw(super::exc("System.NotSupportedException", super::Str::lit("Specified method is not supported.")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rt::{self, Arr, Gc, List, Str};

    fn count_to(n: i32) -> Seq<i32> {
        enumerable(
            move |y: Yielder<i32>| async move {
                let mut i = 0;
                while i < n {
                    y.put(i).await;
                    i += 1;
                }
            },
            |_| {},
        )
    }

    /// A suspended iterator method whose only references to some objects are (a) inside the buffer of a snapshot
    /// enumerator (`foreach` over a Queue / Stack / Dictionary keys: `VecEnum`) and (b) inside the boxed enumerator
    /// of a lazy sequence (`foreach` over a LINQ result): the collector scans the iterator's state by its bytes,
    /// and both live in heap blocks of their own (`gc::register_scan_block`). With poison on, a freed object would
    /// read back as 0xDD bytes (and with the `gc-check` feature the dereference would panic).
    #[test]
    fn handles_owned_by_enumerators_inside_a_suspended_iterator_survive_a_collection() {
        use crate::rt::{linq, Delegate, Queue};
        rt::set_poison(true);
        let e = enumerator(move |y: Yielder<i32>| async move {
            // objects reachable only through the queue, the queue only through its enumerator's snapshot
            let q = Queue::<Gc<List<i32>>>::new_gc();
            for k in 0..40 {
                let l = List::<i32>::new_gc();
                unsafe {
                    (*l.ptr()).add(1000 + k);
                    (*q.ptr()).enqueue(l);
                }
            }
            let mut snapshot = rt::get_enumerator(q);
            while rt::Enumerator::move_next(&mut snapshot) {
                let l = rt::Enumerator::current(&snapshot);
                y.put(unsafe { (*l.ptr()).get_item(0) }).await;
            }
            // a lazy sequence over fresh lists: its boxed enumerator holds the source and the current element
            let src = List::<Gc<List<i32>>>::new_gc();
            for k in 0..40 {
                let l = List::<i32>::new_gc();
                unsafe {
                    (*l.ptr()).add(2000 + k);
                    (*src.ptr()).add(l);
                }
            }
            let lazy = linq::where_(Seq::from_list(src), Delegate::new(Box::new(|l: Gc<List<i32>>| unsafe { (*l.ptr()).get_item(0) % 2 == 0 })));
            let mut it = rt::get_enumerator(lazy);
            while rt::Enumerator::move_next(&mut it) {
                let l = rt::Enumerator::current(&it);
                y.put(unsafe { (*l.ptr()).get_item(0) }).await;
            }
        });
        let _root = rt::Rooted::new(e);
        let mut got = Vec::new();
        while move_next(e) {
            got.push(current::<i32>(e));
            // garbage that would reuse freed memory, then a collection while the iterator is suspended
            for _ in 0..50 {
                let _ = List::<i32>::new_gc();
            }
            unsafe { rt::collect() };
        }
        rt::set_poison(false);
        let want: Vec<i32> = (0..40).map(|k| 1000 + k).chain((0..40).filter(|k| k % 2 == 0).map(|k| 2000 + k)).collect();
        assert_eq!(got, want);
    }

    #[test]
    fn enumerable_restarts_and_is_lazy() {
        let s = count_to(4);
        assert_eq!(s.to_vec(), vec![0, 1, 2, 3]);
        assert_eq!(s.to_vec(), vec![0, 1, 2, 3]);
        assert_eq!(count_to(0).to_vec(), Vec::<i32>::new());
    }

    #[test]
    fn enumerator_object_steps_one_yield_at_a_time() {
        let log = List::<i32>::new_gc();
        let e = enumerator(move |y: Yielder<Obj>| async move {
            unsafe { (*log.ptr()).add(1) };
            y.put(Gc::NULL).await;
            unsafe { (*log.ptr()).add(2) };
            y.put(rt::Trace::to_obj(&Str::new("x"))).await;
            unsafe { (*log.ptr()).add(3) };
        });
        let _root = rt::Rooted::new(e);
        let _root2 = rt::Rooted::new(log);
        let len = || unsafe { (*log.ptr()).count() };
        assert_eq!(len(), 0, "nothing runs before the first MoveNext");
        assert!(move_next(e));
        assert_eq!(len(), 1);
        assert!(current_obj(e).is_null());
        assert!(move_next(e));
        assert_eq!(len(), 2);
        assert!(!move_next(e));
        assert_eq!(len(), 3);
        assert!(!move_next(e));
    }

    #[test]
    fn exception_ends_the_iterator() {
        let e = enumerator(|y: Yielder<i32>| async move {
            y.put(1).await;
            rt::throw(rt::exc("System.InvalidOperationException", Str::lit("boom")));
        });
        assert!(move_next(e));
        assert_eq!(current::<i32>(e), 1);
        let r = rt::try_(|| -> rt::Flow<()> {
            move_next(e);
            rt::Flow::Next
        });
        assert!(r.is_err());
        assert!(!move_next(e));
    }

    #[test]
    fn suspended_machine_keeps_its_locals_alive() {
        rt::set_poison(true);
        let e = enumerator(|y: Yielder<i32>| async move {
            // handles that live only in the state machine across the yields
            let text = Str::new(&format!("kept {}", 41 + 1));
            let arr = Arr::<i32>::from_slice(&[7, 8, 9]);
            let list = List::<Str>::new_gc();
            unsafe { (*list.ptr()).add(Str::new(&format!("in {}", "list"))) };
            y.put(1).await;
            assert_eq!(text.get().unwrap(), "kept 42");
            y.put(unsafe { *arr.at(2) }).await;
            assert_eq!(unsafe { (*list.ptr()).get_item(0) }.get().unwrap(), "in list");
            y.put(3).await;
        });
        let _root = rt::Rooted::new(e);
        assert!(move_next(e));
        // garbage, so that a freed handle would be reused or poisoned
        for i in 0..1000 {
            let _ = Str::new(&format!("garbage {i}"));
        }
        unsafe { rt::collect() };
        assert!(move_next(e));
        assert_eq!(current::<i32>(e), 9);
        unsafe { rt::collect() };
        assert!(move_next(e));
        assert!(!move_next(e));
        // finished: nothing is scanned any more, and the collection still works
        unsafe { rt::collect() };
        rt::set_poison(false);
    }

    #[test]
    fn boxed_machine_inside_a_foreach_is_a_root_while_it_lives() {
        let make = || {
            enumerable(
                |y: Yielder<Str>| async move {
                    let a = Str::new(&format!("a{}", 1));
                    y.put(a).await;
                    let b = Str::new(&format!("b{}", 2));
                    y.put(b).await;
                    y.put(a).await;
                },
                |_| {},
            )
        };
        let s = make();
        let _root = rt::Rooted::new(s);
        use crate::rt::Enumerable;
        let mut e = s.enumerator();
        assert!(e.move_next());
        unsafe { rt::collect() };
        assert!(e.move_next());
        assert_eq!(e.current().get().unwrap(), "b2");
        unsafe { rt::collect() };
        assert!(e.move_next());
        assert_eq!(e.current().get().unwrap(), "a1");
        assert!(!e.move_next());
        drop(e);
        BOXED.with(|b| assert!(b.borrow().is_empty(), "a dropped machine is no root"));
    }
}
