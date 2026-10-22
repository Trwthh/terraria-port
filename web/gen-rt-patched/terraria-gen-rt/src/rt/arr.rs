//! Arrays: `T[]` is `Arr<T>`, `T[,]` is `Arr2<T>`. Handles to a fixed-length zero-initialised buffer.

use super::exc::{exc, throw};
use super::gc::{alloc_init, null_reference, Gc, Marker, Trace};
use std::alloc::{alloc_zeroed, dealloc, handle_alloc_error, Layout};

#[cold]
#[inline(never)]
fn out_of_range() -> ! {
    throw(exc("System.IndexOutOfRangeException", crate::s!("Index was outside the bounds of the array.")))
}

/// Alignment of every array buffer: at least 16, whatever the element type. Translated `unsafe` code reads a `byte[]`
/// through a pointer to a struct (`MapHelper.LoadMapVersionCompressed`: `(MapTile*)ptr`); the CLR aligns array data
/// to the pointer size, so that is defined there and must be here.
const fn buffer_align<T>() -> usize {
    let a = core::mem::align_of::<T>();
    if a > 16 {
        a
    } else {
        16
    }
}

fn buffer<T>(len: usize) -> *mut T {
    let size = len.checked_mul(core::mem::size_of::<T>()).unwrap_or(usize::MAX);
    if size == 0 {
        return core::ptr::NonNull::<T>::dangling().as_ptr();
    }
    let layout = match Layout::from_size_align(size, buffer_align::<T>()) {
        Ok(l) => l,
        Err(_) => throw(exc("System.OutOfMemoryException", crate::rt::Str::NULL)),
    };
    // SAFETY: non-zero size.
    let p = unsafe { alloc_zeroed(layout) };
    if p.is_null() {
        handle_alloc_error(layout);
    }
    p as *mut T
}

fn checked_len(n: i32) -> usize {
    if n < 0 {
        // `new T[-1]` throws OverflowException
        throw(exc("System.OverflowException", crate::s!("Arithmetic operation resulted in an overflow.")))
    }
    n as usize
}

/// Buffers of this many bytes and more are handed to the allocator by a helper thread ([`free_buffer`]).
pub const LARGE_FREE: usize = 256 << 10;

struct LargeBlock(*mut u8, Layout);
// SAFETY: the block belongs to nobody any more: the array that owned it is being dropped by the collector.
unsafe impl Send for LargeBlock {}

static LARGE_FREES: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static LARGE_FREED_BYTES: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// Number and total size of the array buffers freed through the helper thread so far (statistics).
pub fn large_frees() -> (u64, u64) {
    use std::sync::atomic::Ordering::Relaxed;
    (LARGE_FREES.load(Relaxed), LARGE_FREED_BYTES.load(Relaxed))
}

fn send_large(block: LargeBlock) -> Result<(), LargeBlock> {
    static TX: std::sync::OnceLock<std::sync::Mutex<std::sync::mpsc::Sender<LargeBlock>>> = std::sync::OnceLock::new();
    thread_local! {
        static MINE: std::cell::OnceCell<std::sync::mpsc::Sender<LargeBlock>> = const { std::cell::OnceCell::new() };
    }
    let global = TX.get_or_init(|| {
        let (tx, rx) = std::sync::mpsc::channel::<LargeBlock>();
        // the thread runs no code of the program: it only gives memory back to the allocator
        let _ = std::thread::Builder::new().name("terraria-gen-rt large free".into()).spawn(move || {
            for LargeBlock(p, layout) in rx {
                // SAFETY: the block came from `alloc_zeroed` with this layout and was handed over by its only owner.
                unsafe { dealloc(p, layout) };
            }
        });
        std::sync::Mutex::new(tx)
    });
    let mut block = Some(block);
    // (the thread-local is gone while a thread is torn down: the caller frees the block itself then)
    let sent = MINE.try_with(|m| {
        let tx = m.get_or_init(|| global.lock().unwrap_or_else(|e| e.into_inner()).clone());
        match tx.send(block.take().expect("taken once")) {
            Ok(()) => None,
            Err(e) => Some(e.0),
        }
    });
    match sent {
        Ok(None) => Ok(()),
        Ok(Some(b)) => Err(b),
        Err(_) => Err(block.take().expect("not sent")),
    }
}

/// Frees an array buffer. A small one at once; one of [`LARGE_FREE`] bytes or more on a helper thread, because
/// giving tens of megabytes back to the system takes milliseconds and the collector frees inside a time slice of
/// the game thread (the buffers of `Main.tile`-sized maps, light maps, the world file's byte arrays).
unsafe fn free_buffer<T>(p: *mut T, len: usize) {
    let size = len * core::mem::size_of::<T>();
    if size != 0 {
        let layout = Layout::from_size_align_unchecked(size, buffer_align::<T>());
        if size >= LARGE_FREE {
            use std::sync::atomic::Ordering::Relaxed;
            LARGE_FREES.fetch_add(1, Relaxed);
            LARGE_FREED_BYTES.fetch_add(size as u64, Relaxed);
            if send_large(LargeBlock(p as *mut u8, layout)).is_ok() {
                return;
            }
        }
        dealloc(p as *mut u8, layout);
    }
}

#[repr(C)]
pub struct ArrData<T> {
    len: i32,
    data: *mut T,
}
impl<T> Drop for ArrData<T> {
    fn drop(&mut self) {
        // SAFETY: `data` came from `buffer::<T>(len)` and is freed once, by the collector.
        unsafe { free_buffer(self.data, self.len as usize) }
    }
}
unsafe impl<T: Trace + 'static> Trace for ArrData<T> {
    const NEEDS_TRACE: bool = T::NEEDS_TRACE;
    // the elements live in a buffer of their own: a pointer to an element must keep the array alive
    const HAS_BUFFER: bool = true;
    // allocated with the array and never replaced
    const BUFFER_FIXED: bool = true;
    unsafe fn obj_buffer(this: *const Self) -> (usize, usize) {
        ((*this).data as usize, ((*this).len as usize) * core::mem::size_of::<T>())
    }
    // element stores are assignments of translated code (behind the barrier) or runtime helpers that call `wb`
    const RESCAN: bool = false;
    unsafe fn obj_reflect(this: *const Self) -> super::object::Reflect {
        let items = core::slice::from_raw_parts((*this).data, (*this).len as usize).iter().map(|x| x.box_ref()).collect();
        super::object::Reflect::Seq { kind: "Array", items }
    }
    unsafe fn trace(&self, m: &mut Marker) {
        m.trace_slice(core::slice::from_raw_parts(self.data, self.len as usize))
    }
}
unsafe impl<T> Trace for Arr<T> {
    const BOX: super::gc::BoxKind = super::gc::BoxKind::Handle;
    const RESCAN: bool = false;
    #[inline(always)]
    unsafe fn trace(&self, m: &mut Marker) {
        m.mark(self.0)
    }
}
unsafe impl<T> Trace for Arr2<T> {
    const BOX: super::gc::BoxKind = super::gc::BoxKind::Handle;
    const RESCAN: bool = false;
    #[inline(always)]
    unsafe fn trace(&self, m: &mut Marker) {
        m.mark(self.0)
    }
}

/// `T[]`. `Copy`, identity equality, all-zero is `null`.
#[repr(transparent)]
pub struct Arr<T>(Gc<ArrData<T>>);

impl<T> Clone for Arr<T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T> Copy for Arr<T> {}
impl<T> PartialEq for Arr<T> {
    fn eq(&self, o: &Self) -> bool {
        self.0 == o.0
    }
}

impl<T> Arr<T> {
    pub const NULL: Arr<T> = Arr(Gc::NULL);
    /// The array as `object` / `Array`.
    pub fn up<U>(self) -> Gc<U> {
        self.0.up()
    }
    /// # Safety
    /// `o` must be null or an array of exactly this element type.
    pub unsafe fn from_obj<U>(o: Gc<U>) -> Self {
        Arr(o.up())
    }
    /// Identity, for `(object)a == (object)b`.
    #[inline(always)]
    pub fn addr(self) -> usize {
        self.0.addr()
    }
}

impl<T: Copy + Trace + 'static> Arr<T> {

    /// `new T[n]`: every element zero.
    pub fn new(n: i32) -> Arr<T> {
        let len = checked_len(n);
        Arr(alloc_init(ArrData { len: n, data: buffer::<T>(len) }))
    }

    /// `new T[] { a, b, c }`.
    pub fn from_slice(items: &[T]) -> Arr<T> {
        let a = Arr::<T>::new(items.len() as i32);
        // SAFETY: the buffer holds exactly `items.len()` elements and does not overlap `items`.
        unsafe { core::ptr::copy_nonoverlapping(items.as_ptr(), (*a.0 .0).data, items.len()) };
        a
    }

    #[inline(always)]
    pub fn is_null(self) -> bool {
        self.0.is_null()
    }

    /// `a.Length`.
    #[inline(always)]
    pub fn len(self) -> i32 {
        if self.0.is_null() {
            null_reference();
        }
        // SAFETY: non-null arrays point at a live header.
        unsafe { (*self.0 .0).len }
    }

    /// Address of element `i`: the only accessor. Read `*a.at(i)`, write `*a.at(i) = v`, `ref a[i]` is `a.at(i)`.
    /// Null array: `NullReferenceException`. Bad index: `IndexOutOfRangeException`.
    #[inline(always)]
    pub fn at(self, i: i32) -> *mut T {
        if self.0.is_null() {
            null_reference();
        }
        // SAFETY: non-null arrays point at a live header; the index is checked against its length.
        unsafe {
            let d = &*self.0 .0;
            if (i as u32) >= (d.len as u32) {
                out_of_range();
            }
            d.data.add(i as usize)
        }
    }

    /// `(T[])a.Clone()`: a new array with the same elements (shallow).
    pub fn clone_arr(self) -> Arr<T> {
        Arr::from_slice(self.as_slice())
    }

    /// Snapshot view for runtime code. Do not hold it across calls into generated code.
    pub fn as_slice<'a>(self) -> &'a [T] {
        let n = self.len() as usize;
        // SAFETY: `n` elements are initialised (zeroed or written) and never freed.
        unsafe { core::slice::from_raw_parts((*self.0 .0).data, n) }
    }
}

#[repr(C)]
pub struct Arr2Data<T> {
    d0: i32,
    d1: i32,
    data: *mut T,
}
impl<T> Drop for Arr2Data<T> {
    fn drop(&mut self) {
        // SAFETY: as ArrData.
        unsafe { free_buffer(self.data, self.d0 as usize * self.d1 as usize) }
    }
}
unsafe impl<T: Trace + 'static> Trace for Arr2Data<T> {
    const NEEDS_TRACE: bool = T::NEEDS_TRACE;
    // the elements live in a buffer of their own: a pointer to an element must keep the array alive
    const HAS_BUFFER: bool = true;
    // allocated with the array and never replaced
    const BUFFER_FIXED: bool = true;
    unsafe fn obj_buffer(this: *const Self) -> (usize, usize) {
        ((*this).data as usize, ((*this).d0 as usize * (*this).d1 as usize) * core::mem::size_of::<T>())
    }
    // element stores are assignments of translated code (behind the barrier) or runtime helpers that call `wb`
    const RESCAN: bool = false;
    unsafe fn obj_reflect(this: *const Self) -> super::object::Reflect {
        let d = &*this;
        let items = core::slice::from_raw_parts(d.data, d.d0 as usize * d.d1 as usize).iter().map(|x| x.box_ref()).collect();
        super::object::Reflect::Multi { dims: vec![d.d0 as usize, d.d1 as usize], items }
    }
    unsafe fn trace(&self, m: &mut Marker) {
        // `Main.tile` is one of these: 5 million handles, traced in pieces
        m.trace_slice(core::slice::from_raw_parts(self.data, self.d0 as usize * self.d1 as usize))
    }
}

/// `T[,]`, row-major like the CLR.
#[repr(transparent)]
pub struct Arr2<T>(Gc<Arr2Data<T>>);

impl<T> Clone for Arr2<T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T> Copy for Arr2<T> {}
impl<T> PartialEq for Arr2<T> {
    fn eq(&self, o: &Self) -> bool {
        self.0 == o.0
    }
}

impl<T> Arr2<T> {
    pub const NULL: Arr2<T> = Arr2(Gc::NULL);
    /// The array as `object` / `Array`.
    pub fn up<U>(self) -> Gc<U> {
        self.0.up()
    }
    /// # Safety
    /// `o` must be null or a 2-D array of exactly this element type.
    pub unsafe fn from_obj<U>(o: Gc<U>) -> Self {
        Arr2(o.up())
    }
    /// Identity, for `(object)a == (object)b`.
    #[inline(always)]
    pub fn addr(self) -> usize {
        self.0.addr()
    }
}

impl<T: Copy + Trace + 'static> Arr2<T> {

    /// `new T[d0, d1]`.
    pub fn new(d0: i32, d1: i32) -> Arr2<T> {
        let (a, b) = (checked_len(d0), checked_len(d1));
        Arr2(alloc_init(Arr2Data { d0, d1, data: buffer::<T>(a.checked_mul(b).unwrap_or(usize::MAX)) }))
    }

    /// `new T[,] { { a, b }, { c, d } }`.
    pub fn from_rows(rows: &[&[T]]) -> Arr2<T> {
        let d1 = rows.first().map(|r| r.len()).unwrap_or(0);
        let a = Arr2::<T>::new(rows.len() as i32, d1 as i32);
        for (i, r) in rows.iter().enumerate() {
            assert_eq!(r.len(), d1, "ragged 2-D initialiser");
            for (j, v) in r.iter().enumerate() {
                // SAFETY: in range by construction.
                unsafe { *a.at(i as i32, j as i32) = *v };
            }
        }
        a
    }

    #[inline(always)]
    pub fn is_null(self) -> bool {
        self.0.is_null()
    }

    /// `a.Length`: total element count.
    pub fn len(self) -> i32 {
        if self.0.is_null() {
            null_reference();
        }
        // SAFETY: live header.
        unsafe { (*self.0 .0).d0.wrapping_mul((*self.0 .0).d1) }
    }

    /// `a.GetLength(d)`.
    pub fn dim(self, d: i32) -> i32 {
        if self.0.is_null() {
            null_reference();
        }
        // SAFETY: live header.
        unsafe {
            match d {
                0 => (*self.0 .0).d0,
                1 => (*self.0 .0).d1,
                _ => out_of_range(),
            }
        }
    }

    /// `(T[,])a.Clone()`: a new array with the same elements (shallow).
    pub fn clone_arr(self) -> Arr2<T> {
        let (d0, d1) = (self.dim(0), self.dim(1));
        let b = Arr2::<T>::new(d0, d1);
        let n = d0 as usize * d1 as usize;
        if n > 0 {
            // SAFETY: both buffers hold `n` elements and do not overlap.
            unsafe { core::ptr::copy_nonoverlapping((*self.0 .0).data, (*b.0 .0).data, n) };
        }
        b
    }

    /// Address of element `[i, j]`.
    #[inline(always)]
    pub fn at(self, i: i32, j: i32) -> *mut T {
        if self.0.is_null() {
            null_reference();
        }
        // SAFETY: live header; both indices are checked.
        unsafe {
            let d = &*self.0 .0;
            if (i as u32) >= (d.d0 as u32) || (j as u32) >= (d.d1 as u32) {
                out_of_range();
            }
            d.data.add(i as usize * d.d1 as usize + j as usize)
        }
    }
}

#[repr(C)]
pub struct Arr3Data<T> {
    d: [i32; 3],
    data: *mut T,
}
impl<T> Drop for Arr3Data<T> {
    fn drop(&mut self) {
        // SAFETY: as ArrData.
        unsafe { free_buffer(self.data, self.d[0] as usize * self.d[1] as usize * self.d[2] as usize) }
    }
}
unsafe impl<T: Trace + 'static> Trace for Arr3Data<T> {
    const NEEDS_TRACE: bool = T::NEEDS_TRACE;
    // the elements live in a buffer of their own: a pointer to an element must keep the array alive
    const HAS_BUFFER: bool = true;
    // allocated with the array and never replaced
    const BUFFER_FIXED: bool = true;
    unsafe fn obj_buffer(this: *const Self) -> (usize, usize) {
        ((*this).data as usize, ((*this).d[0] as usize * (*this).d[1] as usize * (*this).d[2] as usize) * core::mem::size_of::<T>())
    }
    // element stores are assignments of translated code (behind the barrier) or runtime helpers that call `wb`
    const RESCAN: bool = false;
    unsafe fn obj_reflect(this: *const Self) -> super::object::Reflect {
        let d = &*this;
        let items = core::slice::from_raw_parts(d.data, d.d[0] as usize * d.d[1] as usize * d.d[2] as usize).iter().map(|x| x.box_ref()).collect();
        super::object::Reflect::Multi { dims: vec![d.d[0] as usize, d.d[1] as usize, d.d[2] as usize], items }
    }
    unsafe fn trace(&self, m: &mut Marker) {
        m.trace_slice(core::slice::from_raw_parts(self.data, self.d[0] as usize * self.d[1] as usize * self.d[2] as usize))
    }
}

/// `T[,,]`, row-major like the CLR.
#[repr(transparent)]
pub struct Arr3<T>(Gc<Arr3Data<T>>);

impl<T> Clone for Arr3<T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T> Copy for Arr3<T> {}
impl<T> PartialEq for Arr3<T> {
    fn eq(&self, o: &Self) -> bool {
        self.0 == o.0
    }
}
unsafe impl<T> Trace for Arr3<T> {
    const BOX: super::gc::BoxKind = super::gc::BoxKind::Handle;
    const RESCAN: bool = false;
    #[inline(always)]
    unsafe fn trace(&self, m: &mut Marker) {
        m.mark(self.0)
    }
}
impl<T> Arr3<T> {
    pub const NULL: Arr3<T> = Arr3(Gc::NULL);
    pub fn up<U>(self) -> Gc<U> {
        self.0.up()
    }
    #[inline(always)]
    pub fn is_null(self) -> bool {
        self.0.is_null()
    }
    #[inline(always)]
    pub fn addr(self) -> usize {
        self.0.addr()
    }
}
impl<T: Copy + Trace + 'static> Arr3<T> {
    /// `new T[d0, d1, d2]`.
    pub fn new(d0: i32, d1: i32, d2: i32) -> Arr3<T> {
        let n = checked_len(d0).checked_mul(checked_len(d1)).and_then(|x| x.checked_mul(checked_len(d2))).unwrap_or(usize::MAX);
        Arr3(alloc_init(Arr3Data { d: [d0, d1, d2], data: buffer::<T>(n) }))
    }
    /// `new T[,,] { { { a, b }, .. }, .. }`.
    pub fn from_nested(planes: &[&[&[T]]]) -> Arr3<T> {
        let d1 = planes.first().map(|p| p.len()).unwrap_or(0);
        let d2 = planes.first().and_then(|p| p.first()).map(|r| r.len()).unwrap_or(0);
        let a = Arr3::<T>::new(planes.len() as i32, d1 as i32, d2 as i32);
        for (i, p) in planes.iter().enumerate() {
            assert_eq!(p.len(), d1, "ragged 3-D initialiser");
            for (j, r) in p.iter().enumerate() {
                assert_eq!(r.len(), d2, "ragged 3-D initialiser");
                for (k, v) in r.iter().enumerate() {
                    // SAFETY: in range by construction.
                    unsafe { *a.at(i as i32, j as i32, k as i32) = *v };
                }
            }
        }
        a
    }
    /// `a.Length`.
    pub fn len(self) -> i32 {
        if self.0.is_null() {
            null_reference();
        }
        // SAFETY: live header.
        unsafe { (*self.0 .0).d.iter().fold(1i32, |n, d| n.wrapping_mul(*d)) }
    }
    /// `a.GetLength(d)`.
    pub fn dim(self, d: i32) -> i32 {
        if self.0.is_null() {
            null_reference();
        }
        // SAFETY: live header.
        unsafe {
            match (*self.0 .0).d.get(d as usize) {
                Some(n) if d >= 0 => *n,
                _ => out_of_range(),
            }
        }
    }
    /// Address of element `[i, j, k]`.
    #[inline(always)]
    pub fn at(self, i: i32, j: i32, k: i32) -> *mut T {
        if self.0.is_null() {
            null_reference();
        }
        // SAFETY: live header; all three indices are checked.
        unsafe {
            let h = &*self.0 .0;
            if (i as u32) >= (h.d[0] as u32) || (j as u32) >= (h.d[1] as u32) || (k as u32) >= (h.d[2] as u32) {
                out_of_range();
            }
            h.data.add((i as usize * h.d[1] as usize + j as usize) * h.d[2] as usize + k as usize)
        }
    }
}

/// Index of type `uint` / `long` / `ulong`: values that do not fit an `int` become -1 (which then throws).
#[inline(always)]
pub fn index_from<I: TryInto<i32>>(i: I) -> i32 {
    i.try_into().unwrap_or(-1)
}

/// `fixed (T* p = array)`: the address of the first element; null for a null or an empty array (C# 7.3 spec,
/// "The fixed statement"). The buffer of an array never moves and lives as long as the array is reachable.
#[inline(always)]
pub fn fixed_arr<T: Copy + Trace + 'static>(a: Arr<T>) -> *mut T {
    if a.is_null() || a.len() == 0 {
        core::ptr::null_mut()
    } else {
        a.at(0)
    }
}

/// Element of a native const table mapped in the config (`nativeSlices`), with C#'s bounds exception.
#[inline(always)]
pub fn slice_get<T: Copy>(table: &[T], i: i32) -> T {
    match table.get(i as usize) {
        Some(v) if i >= 0 => *v,
        _ => out_of_range(),
    }
}

/// `.Length` of a native const table.
#[inline(always)]
pub fn slice_len<T>(table: &[T]) -> i32 {
    table.len() as i32
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rt::try_;

    #[test]
    fn new_is_zeroed_and_aliases() {
        let a = Arr::<i32>::new(4);
        let b = a;
        unsafe {
            assert_eq!(*a.at(3), 0);
            *b.at(3) = 9;
            assert_eq!(*a.at(3), 9);
        }
        assert!(a == b);
        assert_eq!(a.len(), 4);
        let c = a.clone_arr();
        assert!(c != a);
        unsafe { assert_eq!(*c.at(3), 9) };
    }

    #[test]
    fn bounds_and_null_throw() {
        let a = Arr::<u8>::from_slice(&[1, 2, 3]);
        let kind = |r: Result<u8, Gc<crate::rt::Exception>>| unsafe { (*r.unwrap_err().ptr()).kind };
        assert_eq!(kind(try_(|| unsafe { *a.at(3) })), "System.IndexOutOfRangeException");
        assert_eq!(kind(try_(|| unsafe { *a.at(-1) })), "System.IndexOutOfRangeException");
        assert_eq!(kind(try_(|| unsafe { *Arr::<u8>::NULL.at(0) })), "System.NullReferenceException");
        assert!(try_(|| Arr::<u8>::new(-1)).is_err());
        assert_eq!(Arr::<u8>::new(0).len(), 0);
    }

    #[test]
    fn three_dimensional() {
        let a = Arr3::<i32>::from_nested(&[&[&[1, 2, 3], &[4, 5, 6]], &[&[7, 8, 9], &[10, 11, 12]]]);
        unsafe {
            assert_eq!((*a.at(0, 0, 0), *a.at(0, 1, 2), *a.at(1, 0, 1), *a.at(1, 1, 2)), (1, 6, 8, 12));
            *a.at(1, 1, 0) = 99;
            assert_eq!(*a.at(1, 1, 0), 99);
        }
        assert_eq!((a.dim(0), a.dim(1), a.dim(2), a.len()), (2, 2, 3, 12));
        assert!(try_(|| unsafe { *a.at(0, 2, 0) }).is_err());
        assert!(try_(|| unsafe { *a.at(0, 0, 3) }).is_err());
        assert!(try_(|| unsafe { *Arr3::<i32>::NULL.at(0, 0, 0) }).is_err());
        assert_eq!(Arr3::<u8>::new(2, 3, 4).len(), 24);
    }

    #[test]
    fn two_dimensional() {
        let a = Arr2::<i32>::from_rows(&[&[1, 2, 3], &[4, 5, 6]]);
        unsafe {
            assert_eq!(*a.at(1, 0), 4);
            assert_eq!(*a.at(0, 2), 3);
        }
        assert_eq!((a.dim(0), a.dim(1), a.len()), (2, 3, 6));
        assert!(try_(|| unsafe { *a.at(0, 3) }).is_err());
        assert!(try_(|| unsafe { *a.at(2, 0) }).is_err());
    }

    #[test]
    fn native_tables() {
        static T: [bool; 3] = [false, true, false];
        assert!(slice_get(&T, 1));
        assert_eq!(slice_len(&T), 3);
        assert!(try_(|| slice_get(&T, 3)).is_err());
        assert!(try_(|| slice_get(&T, -1)).is_err());
        assert_eq!(index_from(5u32), 5);
        assert_eq!(index_from(u64::MAX), -1);
    }
}
