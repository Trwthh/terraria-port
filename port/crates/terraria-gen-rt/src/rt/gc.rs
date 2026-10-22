//! Object handles (`Gc<T>`, `Nullable<T>`, `Delegate<F>`, boxes) and the collector (DESIGN.md section 9).
//!
//! A `Gc<T>` is a raw pointer to a heap object with a 16-byte header in front of it: the object's type id, a
//! liveness word and a pointer to its [`TypeInfo`] (how to trace and free it).
//!
//! # The collector
//!
//! Precise mark-sweep, stop-the-world, run only when the host calls [`collect`]. The contract of `collect`:
//!
//! 1. No generated function is on the stack (the host calls it between simulation ticks). Handles in locals
//!    of generated code are therefore never roots, and `Copy` handles need neither write barriers nor stack maps.
//! 2. Roots are: the `Statics` struct of every class whose static initialiser has run (each `cctor` registers
//!    its tracer with [`register_statics`]) and every value the host keeps alive in a [`Rooted`] or through
//!    [`add_root`]. A handle the host holds anywhere else is dangling after `collect`.
//!
//! Every type that can live in an object implements [`Trace`]; the translator emits the impl for each struct it
//! generates, so the heap is traced exactly (no conservative scanning, nothing kept alive by accident).
//!
//! Each thread has its own heap. Generated code runs on one thread (`rt::enter`), so in practice there is one.
//!
//! # Debugging a use-after-free
//!
//! [`set_poison`]`(true)`: swept objects are filled with `0xDD`, marked dead and never handed back to the
//! allocator, so a stale handle can never alias a new object: it reads poison (a handle read from poison is the
//! non-canonical address `0xDDDD_DDDD_DDDD_DDDD` and faults on use), and with the cargo feature `gc-check`
//! every `Gc::ptr()` verifies the liveness word and panics with "use after free". The next `collect` also
//! panics if it reaches a dead object from a root.

use super::exc::{exc, throw};
use super::string::Str;
use std::alloc::{alloc_zeroed, dealloc, handle_alloc_error, Layout};
use std::cell::{Cell, RefCell};
use std::marker::PhantomData;

const HEADER: usize = 16;
const LIVE: u32 = 0x4C49_5644;
/// Black: reached in the running cycle and its contents traced (or queued as an array range, see `Marker::trace_slice`).
const MARKED: u32 = LIVE | 1;
/// Gray: reached in the running cycle, waiting on the mark stack.
const GRAY: u32 = LIVE | 2;
const DEAD: u32 = 0xDEAD_DEAD;

/// Number of heaps (threads) whose collector is in the mark phase: the fast path of the store barrier ([`wb`]).
static MARKING: core::sync::atomic::AtomicUsize = core::sync::atomic::AtomicUsize::new(0);

#[repr(C, align(16))]
struct Header {
    type_id: u32,
    state: u32,
    info: &'static TypeInfo,
}

/// How the collector treats objects of one Rust type.
pub struct TypeInfo {
    /// Payload size in bytes of this object (strings are variable-sized, everything else is `size_of::<T>()`).
    size_of: unsafe fn(*const u8) -> usize,
    trace: unsafe fn(*const u8, &mut Marker),
    drop: unsafe fn(*mut u8),
    needs_trace: bool,
    /// Stores into objects of this type are not all behind the store barrier ([`Trace::RESCAN`]).
    rescan: bool,
    /// Rust name of the payload type (diagnostics: [`heap_histogram`], the verifier's report).
    name: fn() -> &'static str,
    /// Plain data (no `Drop`): may be copied bytewise by `MemberwiseClone`.
    plain: bool,
    /// Rust type of the payload, for checked downcasts from `object` to runtime types (`rt::cast_rt`).
    rust_type: fn() -> core::any::TypeId,
    /// The object's `Equals(object)`, `GetHashCode()` and `ToString()`: the virtual methods of `System.Object`,
    /// reached through any handle whatever its static type (`Trace::obj_equals` ...).
    equals: unsafe fn(*const u8, Obj) -> bool,
    hash: unsafe fn(*const u8) -> i32,
    to_string: unsafe fn(*const u8) -> Str,
    /// Contents of a runtime container, for code that holds it as `object` (`Trace::obj_reflect`).
    reflect: unsafe fn(*const u8) -> super::object::Reflect,
    /// `IDisposable.Dispose()` reached through an interface handle (`Trace::obj_dispose`).
    dispose: unsafe fn(*mut u8),
    /// `ICollection<T>.Clear()` reached through an interface handle (`Trace::obj_clear`); false: not a collection.
    clear: unsafe fn(*mut u8) -> bool,
    /// The payload can be larger than 4096 bytes, or the object owns a buffer ([`Trace::obj_buffer`]): the
    /// conservative resolution looks at every such object (`Marker::resolve_candidates`).
    wide: bool,
    /// The buffer the object owns outside its payload (start, bytes); (0, 0) for none.
    buffer: unsafe fn(*const u8) -> (usize, usize),
    /// [`Trace::HAS_BUFFER`]: objects of this type go on the heap's list of wide objects (`Heap::wide`).
    has_buffer: bool,
    /// [`Trace::BUFFER_FIXED`].
    fixed_buffer: bool,
}

struct InfoOf<T>(PhantomData<T>);
impl<T: Trace + 'static> InfoOf<T> {
    const INFO: TypeInfo = TypeInfo {
        size_of: size_thunk::<T>,
        trace: trace_thunk::<T>,
        drop: drop_thunk::<T>,
        needs_trace: T::NEEDS_TRACE,
        rescan: T::NEEDS_TRACE && T::RESCAN,
        name: core::any::type_name::<T>,
        plain: !core::mem::needs_drop::<T>(),
        rust_type: core::any::TypeId::of::<T>,
        equals: equals_thunk::<T>,
        hash: hash_thunk::<T>,
        to_string: to_string_thunk::<T>,
        reflect: reflect_thunk::<T>,
        dispose: dispose_thunk::<T>,
        clear: clear_thunk::<T>,
        wide: core::mem::size_of::<T>() > 4096 || T::HAS_BUFFER,
        buffer: buffer_thunk::<T>,
        has_buffer: T::HAS_BUFFER,
        fixed_buffer: T::HAS_BUFFER && T::BUFFER_FIXED,
    };
}
unsafe fn buffer_thunk<T: Trace>(p: *const u8) -> (usize, usize) {
    T::obj_buffer(p as *const T)
}
unsafe fn no_buffer_fn(_p: *const u8) -> (usize, usize) {
    (0, 0)
}
unsafe fn clear_thunk<T: Trace>(p: *mut u8) -> bool {
    T::obj_clear(p as *mut T)
}
unsafe fn no_clear_fn(_p: *mut u8) -> bool {
    false
}

/// `((ICollection<T>)o).Clear()` for a runtime collection held through an interface (`IDictionary<K, V>`,
/// `ICollection<T>`, `IList<T>`): the collection's own `Clear` ([`Trace::obj_clear`]). Null throws
/// `NullReferenceException`; an object that is no runtime collection is recorded as an unimplemented member.
pub fn collection_clear(o: Obj) {
    let p = o.ptr() as *mut u8;
    // SAFETY: a non-null handle points at a payload from `alloc`.
    unsafe {
        let info = (*(p.sub(HEADER) as *const Header)).info;
        if !(info.clear)(p) {
            crate::host::unimplemented_clear((info.name)());
        }
    }
}
unsafe fn dispose_thunk<T: Trace>(p: *mut u8) {
    T::obj_dispose(p as *mut T)
}
unsafe fn no_dispose_fn(_p: *mut u8) {}

/// `((IDisposable)o).Dispose()` for an object that is not a translated class: the runtime or host type's own
/// `Dispose` ([`Trace::obj_dispose`]). Null throws `NullReferenceException`.
pub fn dispose_obj(o: Obj) {
    let p = o.ptr() as *mut u8;
    // SAFETY: a non-null handle points at a payload from `alloc`.
    unsafe { ((*(p.sub(HEADER) as *const Header)).info.dispose)(p) }
}
unsafe fn reflect_thunk<T: Trace>(p: *const u8) -> super::object::Reflect {
    T::obj_reflect(p as *const T)
}
unsafe fn no_reflect_fn(_p: *const u8) -> super::object::Reflect {
    super::object::Reflect::Other
}

/// The contents of `o` when it is a runtime container (array, list, set, dictionary, delegate).
pub fn obj_reflect(o: Obj) -> super::object::Reflect {
    if o.is_null() {
        null_reference();
    }
    // SAFETY: non-null objects have a header.
    unsafe { ((*(o.0 as *const u8).sub(HEADER).cast::<Header>()).info.reflect)(o.0 as *const u8) }
}
unsafe fn equals_thunk<T: Trace>(p: *const u8, other: Obj) -> bool {
    T::obj_equals(p as *const T, other)
}
unsafe fn hash_thunk<T: Trace>(p: *const u8) -> i32 {
    T::obj_hash(p as *const T)
}
unsafe fn to_string_thunk<T: Trace>(p: *const u8) -> Str {
    T::obj_to_string(p as *const T)
}
unsafe fn size_thunk<T>(_p: *const u8) -> usize {
    core::mem::size_of::<T>()
}
unsafe fn trace_thunk<T: Trace>(p: *const u8, m: &mut Marker) {
    (*(p as *const T)).trace(m)
}
unsafe fn drop_thunk<T>(p: *mut u8) {
    core::ptr::drop_in_place(p as *mut T)
}

/// A type the collector can look into. `trace` must report every handle the value holds (`m.mark(..)`, or
/// `field.trace(m)` for nested values) and must not allocate or call generated code.
///
/// # Safety
/// An impl that misses a handle lets the collector free a reachable object.
pub unsafe trait Trace {
    /// `false` for types that can never hold a handle (numbers, plain XNA structs): arrays of them are skipped.
    const NEEDS_TRACE: bool = true;
    /// `true` (the default, the safe answer): code may store a handle into a value of this type WITHOUT the store
    /// barrier ([`wb`]), so the incremental collector traces heap objects of this type a second time when it ends
    /// its mark phase. `false` is a promise: every store of a handle into this type's memory, after the object was
    /// created, goes through [`wb`] (translated code: the translator emits it on every assignment; a runtime
    /// container: its mutating methods call it), or the type is immutable. Handles themselves are `false`.
    const RESCAN: bool = true;
    /// How a value of this type becomes an `object` in generic code (`(object)t` for a type parameter `T`).
    const BOX: BoxKind = BoxKind::None;
    /// # Safety
    /// `self` must be a valid value of the type (all-zero is valid).
    unsafe fn trace(&self, m: &mut Marker);

    /// A hint to the CPU that [`Trace::trace`] of this value comes soon: a handle asks for its object's header.
    /// Arrays call it a few elements ahead while they are traced (the tile map is one array of 5 million handles to
    /// objects scattered over the heap: the mark phase is bound by those cache misses). No effect on behaviour.
    #[inline(always)]
    fn prefetch(&self) {}

    /// `(object)value` where the static type is a type parameter: boxing for value types, the same pointer for handles.
    fn to_obj(&self) -> Obj
    where
        Self: Copy + Sized + 'static,
    {
        match Self::BOX {
            BoxKind::Value(id) => box_value(*self, id),
            BoxKind::Handle => {
                assert!(core::mem::size_of::<Self>() == core::mem::size_of::<Obj>());
                // SAFETY: handle types are one pointer (`#[repr(transparent)]` over `Gc`).
                unsafe { core::mem::transmute_copy(self) }
            }
            BoxKind::None | BoxKind::Nullable => panic!("cs2rs runtime: (object) conversion of {} in generic code has no rule", core::any::type_name::<Self>()),
        }
    }

    /// `(T)o` where `T` is a type parameter: unboxing for value types (`InvalidCastException` on another type).
    /// For handles the pointer is retyped WITHOUT a check (deviation: C# throws `InvalidCastException` when the
    /// object is not a `T`; generic code of the game does not rely on that).
    ///
    /// # Safety
    /// For handle types `o` must be null or an object of that type.
    unsafe fn from_obj(o: Obj) -> Self
    where
        Self: Copy + Sized + 'static,
    {
        match Self::BOX {
            BoxKind::Value(id) => unbox::<Self>(o, id),
            BoxKind::Handle => {
                assert!(core::mem::size_of::<Self>() == core::mem::size_of::<Obj>());
                core::mem::transmute_copy(&o)
            }
            BoxKind::None | BoxKind::Nullable => panic!("cs2rs runtime: cast from object to {} in generic code has no rule", core::any::type_name::<Self>()),
        }
    }

    /// `this.Equals(other)` as `System.Object` declares it, for a heap object whose payload is this type: a
    /// translated class (the generator overrides this when the class or a base overrides `Equals`), a boxed
    /// value (value equality), a native class. Default: reference equality, as `Object.Equals`.
    ///
    /// # Safety
    /// `this` must point at a live object from [`alloc`] with this payload type.
    unsafe fn obj_equals(this: *const Self, other: Obj) -> bool
    where
        Self: Sized,
    {
        this as usize == other.addr()
    }

    /// `this.GetHashCode()`. Default: `Object.GetHashCode` (a number per object; .NET's value is arbitrary too).
    ///
    /// # Safety
    /// As [`Trace::obj_equals`].
    unsafe fn obj_hash(this: *const Self) -> i32
    where
        Self: Sized,
    {
        identity_hash(this as usize)
    }

    /// `this.ToString()`. Default: the type's name, as `Object.ToString` (for a native type the Rust name: no
    /// C# name is known; translated classes print their C# full name).
    ///
    /// # Safety
    /// As [`Trace::obj_equals`].
    unsafe fn obj_to_string(_this: *const Self) -> Str
    where
        Self: Sized,
    {
        Str::lit(core::any::type_name::<Self>())
    }

    /// `Dispose()` of this object reached as `IDisposable` (a `using`, the end of a `foreach`, an explicit call
    /// through the interface). Runtime and host types that have a `Dispose` override this. The default is loud:
    /// the call is recorded as an unimplemented host member (`host::unimplemented`) with the Rust type's name, so
    /// a type whose `Dispose` matters cannot be skipped silently.
    ///
    /// # Safety
    /// As [`Trace::obj_equals`].
    unsafe fn obj_dispose(_this: *mut Self)
    where
        Self: Sized,
    {
        crate::host::unimplemented_dispose(core::any::type_name::<Self>())
    }

    /// `(object)value` for an element of a runtime container that code holds as `object` ([`Trace::obj_reflect`]):
    /// a fresh box for a value type, the same object for a handle. Bitwise copy: every value of the runtime is
    /// `Copy` by the model's rule (DESIGN.md section 1) even where the Rust type does not say so.
    ///
    /// # Safety
    /// `self` must be a valid value of a type that the model allows in an array or a collection.
    unsafe fn box_ref(&self) -> Obj
    where
        Self: Sized + 'static,
    {
        match Self::BOX {
            BoxKind::Value(id) => {
                let g = alloc::<Self>(id);
                core::ptr::copy_nonoverlapping(self as *const Self, g.0, 1);
                g.up()
            }
            BoxKind::Handle => {
                assert!(core::mem::size_of::<Self>() == core::mem::size_of::<Obj>());
                core::mem::transmute_copy(self)
            }
            BoxKind::None | BoxKind::Nullable => panic!("cs2rs runtime: (object) conversion of {} has no rule", core::any::type_name::<Self>()),
        }
    }

    /// The object owns memory outside its payload that code may hold a pointer into while it holds no pointer to
    /// the object itself (an array's elements, a list's items): [`Trace::obj_buffer`] tells where. Only the
    /// conservative scan of suspended stacks uses it.
    const HAS_BUFFER: bool = false;
    /// The buffer of [`Trace::HAS_BUFFER`] stays where it is for the object's life (an array's elements; NOT a
    /// list's items, which move when the list grows): the collector caches its range (`Heap::wide`).
    const BUFFER_FIXED: bool = false;
    /// See [`Trace::HAS_BUFFER`]: (start address, length in bytes) of that memory now; (0, 0) for none.
    ///
    /// # Safety
    /// `this` points at a live object of this type.
    unsafe fn obj_buffer(_this: *const Self) -> (usize, usize)
    where
        Self: Sized,
    {
        (0, 0)
    }

    /// `ICollection<T>.Clear()` of a runtime collection reached through an interface handle
    /// ([`collection_clear`]). Default: `false`, this type is not a collection.
    ///
    /// # Safety
    /// `this` points at a live object of this type.
    unsafe fn obj_clear(_this: *mut Self) -> bool
    where
        Self: Sized,
    {
        false
    }

    /// The contents of a runtime container, boxed (see [`super::object::Reflect`]). Default: not a container.
    ///
    /// # Safety
    /// As [`Trace::obj_equals`].
    unsafe fn obj_reflect(_this: *const Self) -> super::object::Reflect
    where
        Self: Sized,
    {
        super::object::Reflect::Other
    }

    /// `EqualityComparer<T>.Default.Equals(a, b)` for two non-null handles of STATIC type `T` = this class:
    /// `IEquatable<T>.Equals(T)` when the class implements it (the generator overrides this), else the virtual
    /// `Equals(object)`.
    ///
    /// # Safety
    /// Both handles must be non-null and point at live objects.
    unsafe fn key_eq(a: Gc<Self>, b: Gc<Self>) -> bool
    where
        Self: Sized,
    {
        obj_equals(a.up(), b.up())
    }

    /// `a.CompareTo(b)` through `IComparable<T>` on a type parameter (`where T : IComparable<T>`): defined for
    /// the primitives and strings; any other type panics (a translated `CompareTo` is called directly instead).
    fn cs_compare(&self, _other: &Self) -> i32
    where
        Self: Sized,
    {
        panic!("cs2rs runtime: IComparable<T>.CompareTo on {} in generic code has no rule", core::any::type_name::<Self>())
    }
}

/// See [`Trace::BOX`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum BoxKind {
    /// A value type, boxed with this type id.
    Value(u32),
    /// A handle (class reference, array, delegate): the object itself.
    Handle,
    /// `T?`: null, or the boxed value (its impl overrides `to_obj` / `from_obj`).
    Nullable,
    /// The type has its own `to_obj` / `from_obj` (`string`, `T?`), or cannot be an `object` in generic code.
    None,
}

macro_rules! no_trace {
    ($($t:ty),* $(,)?) => { $(
        unsafe impl Trace for $t {
            const NEEDS_TRACE: bool = false;
            const RESCAN: bool = false;
            #[inline(always)]
            unsafe fn trace(&self, _m: &mut Marker) {}
        }
    )* };
}
/// Value types with a boxing id; `cmp`: the type's `IComparable<T>.CompareTo`.
macro_rules! no_trace_boxed {
    ($($t:ty => $id:expr $(, cmp $c:expr)?;)*) => { $(
        unsafe impl Trace for $t {
            const NEEDS_TRACE: bool = false;
            const RESCAN: bool = false;
            const BOX: BoxKind = BoxKind::Value($id);
            #[inline(always)]
            unsafe fn trace(&self, _m: &mut Marker) {}
            $( fn cs_compare(&self, other: &Self) -> i32 { ($c)(self, other) } )?
            // a boxed value: `Equals(object)` is "the same type and the same value" (`ValueType` / the primitives)
            unsafe fn obj_equals(this: *const Self, other: Obj) -> bool {
                !other.is_null() && type_id_of(other.0 as *const u8) == type_id_of(this as *const u8) && super::CsKey::cs_eq(&*this, &*(other.0 as *const Self))
            }
            unsafe fn obj_hash(this: *const Self) -> i32 {
                super::CsKey::cs_hash_code(&*this)
            }
            unsafe fn obj_to_string(this: *const Self) -> Str {
                super::object::boxed_to_string(type_id_of(this as *const u8), &*this)
            }
        }
    )* };
}
fn cmp_ord<T: PartialOrd>(a: &T, b: &T) -> i32 {
    if a < b {
        -1
    } else if a > b {
        1
    } else {
        0
    }
}
/// `float.CompareTo`: NaN is smaller than every number and equal to NaN.
fn cmp_float<T: PartialOrd + Copy>(a: &T, b: &T) -> i32 {
    #[allow(clippy::eq_op)]
    let (an, bn) = (a != a, b != b);
    if a < b {
        -1
    } else if a > b {
        1
    } else if a == b {
        0
    } else if an {
        if bn {
            0
        } else {
            -1
        }
    } else {
        1
    }
}
no_trace!(usize, (), &'static str, terraria_core::UnifiedRandom);
// `char` is a `u16` in the runtime: in generic code it boxes as a `ushort` (deviation).
no_trace_boxed! {
    bool => tid::BOOL, cmp cmp_ord::<bool>;
    u8 => tid::U8, cmp cmp_ord::<u8>;
    i8 => tid::I8, cmp cmp_ord::<i8>;
    u16 => tid::U16, cmp cmp_ord::<u16>;
    i16 => tid::I16, cmp cmp_ord::<i16>;
    u32 => tid::U32, cmp cmp_ord::<u32>;
    i32 => tid::I32, cmp cmp_ord::<i32>;
    u64 => tid::U64, cmp cmp_ord::<u64>;
    i64 => tid::I64, cmp cmp_ord::<i64>;
    f32 => tid::F32, cmp cmp_float::<f32>;
    f64 => tid::F64, cmp cmp_float::<f64>;
    // `IntPtr` (the host tables map it to `isize`)
    isize => tid::INT_PTR;
    terraria_core::xna::Color => tid::COLOR;
    terraria_core::xna::Vector2 => tid::VECTOR2;
    terraria_core::xna::Vector3 => tid::VECTOR3;
    terraria_core::xna::Vector4 => tid::VECTOR4;
    terraria_core::xna::Matrix => tid::MATRIX;
    terraria_core::xna::Rectangle => tid::RECTANGLE;
    terraria_core::xna::Point => tid::POINT;
}

unsafe impl<T> Trace for Gc<T> {
    const BOX: BoxKind = BoxKind::Handle;
    const RESCAN: bool = false;
    #[inline(always)]
    unsafe fn trace(&self, m: &mut Marker) {
        m.mark_payload(self.0 as *const u8)
    }
    #[inline(always)]
    fn prefetch(&self) {
        prefetch_read((self.0 as *const u8).wrapping_sub(HEADER));
    }
}

/// Asks the CPU to load the cache line at `p` (any address is allowed, null included: the instruction never faults).
#[inline(always)]
pub(crate) fn prefetch_read(p: *const u8) {
    #[cfg(target_arch = "x86_64")]
    // SAFETY: a prefetch does not access memory architecturally.
    unsafe {
        core::arch::x86_64::_mm_prefetch(p as *const i8, core::arch::x86_64::_MM_HINT_T0)
    }
    #[cfg(not(target_arch = "x86_64"))]
    let _ = p;
}
unsafe impl<T: Trace, const N: usize> Trace for [T; N] {
    const NEEDS_TRACE: bool = T::NEEDS_TRACE;
    const RESCAN: bool = T::RESCAN;
    unsafe fn trace(&self, m: &mut Marker) {
        if T::NEEDS_TRACE {
            for x in self {
                x.trace(m)
            }
        }
    }
}
unsafe impl<A: Trace, B: Trace> Trace for (A, B) {
    const NEEDS_TRACE: bool = A::NEEDS_TRACE || B::NEEDS_TRACE;
    const RESCAN: bool = field_rescan::<A>() || field_rescan::<B>();
    unsafe fn trace(&self, m: &mut Marker) {
        self.0.trace(m);
        self.1.trace(m);
    }
}
unsafe impl<T: Trace> Trace for Option<T> {
    const NEEDS_TRACE: bool = T::NEEDS_TRACE;
    const RESCAN: bool = T::RESCAN;
    unsafe fn trace(&self, m: &mut Marker) {
        if let Some(x) = self {
            x.trace(m)
        }
    }
}
unsafe impl<T: Trace> Trace for Vec<T> {
    const NEEDS_TRACE: bool = T::NEEDS_TRACE;
    unsafe fn trace(&self, m: &mut Marker) {
        if T::NEEDS_TRACE {
            for x in self {
                x.trace(m)
            }
        }
    }
}

/// `true` when a value of type `T` embedded as a field makes its owner a [`Trace::RESCAN`] object.
pub const fn field_rescan<T: Trace>() -> bool {
    T::NEEDS_TRACE && T::RESCAN
}

/// Arrays above this many elements are traced in pieces ([`Marker::trace_slice`]), so that one array (the tile
/// map: 5 million handles) cannot hold a mark slice for longer than a piece takes.
const RANGE_MIN: usize = 2048;
const RANGE_CHUNK: usize = 512;

/// The rest of a large array that is still to be traced.
struct Range {
    p: *const u8,
    n: usize,
    size: usize,
    f: unsafe fn(*const u8, usize, &mut Marker),
}

/// The second, independent traversal of [`set_verify`].
#[derive(Default)]
struct Verify {
    seen: std::collections::HashSet<usize>,
    missed: Vec<*mut Header>,
}

/// The mark phase's work list.
#[derive(Default)]
pub struct Marker {
    stack: Vec<*mut Header>,
    /// Large arrays in progress.
    ranges: Vec<Range>,
    /// Words found by conservative scans that may be handles: resolved in rounds ([`Marker::resolve_candidates`]).
    candidates: Vec<usize>,
    /// Registered Rust heap blocks already scanned in this collection ([`register_scan_block`]).
    blocks_seen: std::collections::HashSet<usize>,
    /// Conservative words already resolved in this cycle, sorted ([`Marker::resolve_candidates`]).
    resolved: Vec<usize>,
    /// A resolution spread over mark slices ([`Marker::resolve_candidates`]).
    pending: Option<Box<Resolution>>,
    /// Set while the verifier walks the heap: nothing is marked, reached objects are only checked.
    verify: Option<Box<Verify>>,
}

impl Marker {
    /// Marks the object a handle points to (null is ignored).
    #[inline(always)]
    pub fn mark<T>(&mut self, g: Gc<T>) {
        // SAFETY: a non-null Gc points at a payload from `alloc`.
        unsafe { self.mark_payload(g.0 as *const u8) }
    }

    /// # Safety
    /// `p` is null or the payload address of an object from [`alloc`] / [`alloc_var`].
    #[inline]
    pub unsafe fn mark_payload(&mut self, p: *const u8) {
        if p.is_null() {
            return;
        }
        let h = p.sub(HEADER) as *mut Header;
        if self.verify.is_some() {
            return self.verify_visit(h, p);
        }
        match (*h).state {
            LIVE => {
                if (*h).info.needs_trace {
                    (*h).state = GRAY;
                    self.stack.push(h);
                } else {
                    (*h).state = MARKED;
                }
            }
            MARKED | GRAY => {}
            _ => dangling(p),
        }
    }

    #[cold]
    unsafe fn verify_visit(&mut self, h: *mut Header, p: *const u8) {
        let v = self.verify.as_mut().unwrap();
        if !v.seen.insert(h as usize) {
            return;
        }
        match (*h).state {
            MARKED => {}
            LIVE | GRAY => v.missed.push(h),
            _ => dangling(p),
        }
        if (*h).info.needs_trace {
            self.stack.push(h);
        }
    }

    /// Traces the elements of an array buffer that neither moves nor is freed while its object is alive (`T[]`,
    /// `T[,]`, `T[,,]`; NOT a `Vec` that can reallocate). A large buffer is only queued and traced in pieces
    /// between budget checks. Because such an array can be half traced while the program runs, code that moves
    /// handles around INSIDE an array without [`wb`] (sort, reverse, an overlapping copy) must shade them.
    ///
    /// # Safety
    /// As [`Trace::trace`]; the buffer must stay where it is until its object is freed.
    #[inline]
    pub unsafe fn trace_slice<T: Trace>(&mut self, s: &[T]) {
        if !T::NEEDS_TRACE {
            return;
        }
        if s.len() > RANGE_MIN {
            self.ranges.push(Range { p: s.as_ptr() as *const u8, n: s.len(), size: core::mem::size_of::<T>(), f: slice_thunk::<T> });
        } else {
            slice_thunk::<T>(s.as_ptr() as *const u8, s.len(), self)
        }
    }

    fn is_empty(&self) -> bool {
        self.stack.is_empty() && self.ranges.is_empty() && self.candidates.is_empty() && self.pending.is_none()
    }
}

unsafe fn slice_thunk<T: Trace>(p: *const u8, n: usize, m: &mut Marker) {
    let s = core::slice::from_raw_parts(p as *const T, n);
    for (i, x) in s.iter().enumerate() {
        // the header of each element's object is asked for a few elements ahead: the tile map's 5 million tiles
        // are scattered over the heap and the mark phase is bound by those cache misses
        if let Some(ahead) = s.get(i + 8) {
            ahead.prefetch();
        }
        x.trace(m)
    }
}

impl Marker {
    /// Conservative scan of `len` bytes at `p`: every pointer-sized, pointer-aligned word that is the payload
    /// address of a live object (a `Gc`, `Arr`, collection or delegate handle, a `this` pointer) or the data
    /// address of a string object (`Str`) marks that object. For memory whose layout the tracer cannot know: the
    /// state machine of a suspended iterator method (`rt::iter`), which is a compiler-made `Future`.
    ///
    /// Limits, stated: a word that only looks like a handle keeps an object alive for one more collection
    /// (harmless); a handle that the memory holds only inside a Rust-heap buffer of its own is seen only when
    /// that buffer was declared with [`register_scan_block`] (the runtime's enumerators do).
    ///
    /// The words are only collected here; which of them are objects is decided for all scans of a round at once
    /// ([`Marker::resolve_candidates`], one pass over the object list), because a set of every object's address
    /// costs more to build than the rest of a collection (5.8 million objects with a world loaded).
    ///
    /// # Safety
    /// `p .. p + len` must be readable.
    pub unsafe fn mark_conservative(&mut self, p: *const u8, len: usize) {
        let word = core::mem::size_of::<usize>();
        let start = (p as usize + word - 1) & !(word - 1);
        let end = (p as usize + len) & !(word - 1);
        let mut a = start;
        while a + word <= end.max(start) && a < end {
            let w = (a as *const usize).read();
            a += word;
            // pointer-aligned words only (8 on x86-64, 4 on wasm32: a `Str` in a suspended machine sits 4 past 8 there)
            if w < 4096 || w % word != 0 {
                continue;
            }
            // a block of Rust heap memory that holds handles and is owned from the scanned bytes (the buffer of a
            // snapshot enumerator, a boxed enumerator of a lazy sequence): scanned too, once per collection
            let block = SCAN_BLOCKS.with(|b| b.borrow().get(&w).copied());
            if let Some(len) = block {
                if self.blocks_seen.insert(w) {
                    self.mark_conservative(w as *const u8, len);
                }
                continue;
            }
            self.candidates.push(w);
        }
    }

    /// Scans the stack memory of suspended code (a work fiber that gave the frame back, the main loop of a
    /// dedicated server asleep): EVERY word that could be an address is a candidate, aligned or not. Compiled code
    /// keeps handles, `this` pointers, pointers to a field of an object and pointers into an array's elements; the
    /// resolution ([`Marker::resolve_candidates`]) keeps the object alive in each of these cases.
    ///
    /// # Safety
    /// `p .. p + len` must be readable.
    pub unsafe fn mark_conservative_stack(&mut self, p: *const u8, len: usize) {
        let word = core::mem::size_of::<usize>();
        let start = (p as usize + word - 1) & !(word - 1);
        let end = (p as usize + len) & !(word - 1);
        let mut a = start;
        while a + word <= end {
            let w = (a as *const usize).read_volatile();
            a += word;
            // not an address of this process's heap: small integers, kernel-half values
            if !(HEAP_ADDRESSES).contains(&w) {
                continue;
            }
            let block = SCAN_BLOCKS.with(|b| b.borrow().get(&w).copied());
            if let Some(len) = block {
                if self.blocks_seen.insert(w) {
                    self.mark_conservative(w as *const u8, len);
                }
                continue;
            }
            self.candidates.push(w);
        }
    }

    /// Resolves the candidate words of the conservative scans: an object is referred to when a word points
    /// anywhere into its payload (its start: a handle or `this`; the bytes of a string: `Str`; a field), or into the
    /// buffer it owns ([`Trace::obj_buffer`]: an element of an array, an item of a list); those objects are marked.
    /// One pass over the addresses in the object list (a bit filter over the words' pages rejects nearly every object
    /// without reading its header), then over the wide objects (`Heap::wide`). The passes are done in pieces within
    /// the budget: the program may run between them (see [`Resolution`]).
    ///
    /// # Safety
    /// The marker's object list must be the heap's; it may only have grown since the resolution began.
    unsafe fn resolve_candidates(&mut self, objects: &[*mut Header], wide: &mut [Wide], budget: &mut Budget) -> Resolve {
        if self.pending.is_none() {
            if self.candidates.is_empty() {
                return Resolve::Idle;
            }
            let mut cands = core::mem::take(&mut self.candidates);
            cands.sort_unstable();
            cands.dedup();
            // Words already resolved in this cycle need no second look at payloads and fixed buffers: what such a
            // word pointed into then was marked then and stays marked, and an object made since was born marked.
            // Only a buffer that MOVES (a list's) can have come under an old word: those are checked with every
            // word. The verifier looks at everything.
            let fresh: Vec<usize> = if self.verify.is_some() {
                cands.clone()
            } else {
                let f: Vec<usize> = cands.iter().copied().filter(|c| self.resolved.binary_search(c).is_err()).collect();
                if !f.is_empty() {
                    self.resolved.extend_from_slice(&f);
                    self.resolved.sort_unstable();
                }
                f
            };
            let mut filter = vec![0u64; if fresh.is_empty() { 0 } else { FILTER_BITS / 64 }];
            for &w in &fresh {
                let k = (w >> 12) & (FILTER_BITS - 1);
                filter[k >> 6] |= 1 << (k & 63);
            }
            self.pending = Some(Box::new(Resolution { cands, fresh, filter, pos: 0, wpos: 0, read: 0, found: Vec::new(), us: 0 }));
        }
        let r = self.pending.as_mut().unwrap();
        let t0 = std::time::Instant::now();
        const PIECE: usize = 16 * 1024;
        // Every object: only its address is read (the list is sequential memory, the filter fits in the cache);
        // the header is touched only for an object with a fresh word on its pages. Reading every header was a
        // cache miss per object: 46 ms for 1.6 million objects in a multiplayer session.
        if r.fresh.is_empty() {
            r.pos = objects.len();
        }
        while r.pos < objects.len() {
            let end = (r.pos + PIECE).min(objects.len());
            for &h in &objects[r.pos..end] {
                if r.near(h as usize + HEADER) && refers(h, true, &r.fresh) {
                    r.found.push(h);
                }
            }
            r.pos = end;
            if r.pos < objects.len() && budget.spent(PIECE / 8) {
                r.us += t0.elapsed().as_micros() as u64;
                return Resolve::OutOfBudget;
            }
        }
        // The wide objects: a word further into a large payload or into an owned buffer. A known range is tested
        // without the header (two filter bits, a search for a range above a page); a fixed buffer's range is read
        // once and kept; a moving buffer is read every time and tested with every word.
        while r.wpos < wide.len() {
            let end = (r.wpos + PIECE).min(wide.len());
            for e in wide[r.wpos..end].iter_mut() {
                if !e.known {
                    r.read += 1;
                    let info = (*e.h).info;
                    if (*e.h).state & !3 == LIVE && info.fixed_buffer {
                        let (b, len) = (info.buffer)((e.h as usize + HEADER) as *const u8);
                        *e = Wide { h: e.h, lo: b, hi: b + len, known: true };
                    } else {
                        if refers(e.h, true, &r.cands) {
                            r.found.push(e.h);
                        }
                        continue;
                    }
                }
                if r.fresh.is_empty() || r.near(e.h as usize + HEADER) {
                    // nothing new, or done by the pass over every object
                    continue;
                }
                if e.hi > e.lo
                    && (r.hit(e.lo) || r.hit(e.hi - 1) || (e.hi - e.lo > 4096 && any_in(&r.fresh, e.lo, e.hi)))
                    && refers(e.h, false, &r.fresh)
                {
                    r.found.push(e.h);
                }
            }
            r.wpos = end;
            if r.wpos < wide.len() && budget.spent(PIECE / 8) {
                r.us += t0.elapsed().as_micros() as u64;
                return Resolve::OutOfBudget;
            }
        }
        let r = self.pending.take().unwrap();
        profile(GcProfile::Resolve {
            candidates: r.cands.len(),
            fresh: r.fresh.len(),
            objects: objects.len(),
            wide: wide.len(),
            read: r.read,
            found: r.found.len(),
            us: r.us + t0.elapsed().as_micros() as u64,
        });
        for &h in &r.found {
            self.mark_payload((h as *const u8).add(HEADER));
        }
        Resolve::Done
    }
}

/// What [`Marker::resolve_candidates`] did.
#[derive(PartialEq)]
enum Resolve {
    /// No word was waiting.
    Idle,
    /// A set of words is resolved and its objects marked.
    Done,
    /// The budget ran out in the middle of the passes; the resolution continues in the next step.
    OutOfBudget,
}

/// Bits of the page filter of a [`Resolution`] (keyed by page number).
const FILTER_BITS: usize = 1 << 20;

/// A resolution of a set of conservative words in progress. It may be spread over several mark slices with the
/// program running in between because, during a mark phase, the object list and the wide list only grow (objects
/// made since are born marked: the cursors may pass them or not), payloads and fixed buffers do not move, and a
/// moving (list) buffer that comes under one of these words later is caught by the end of the mark phase, which
/// tests every word still on a stack against every moving buffer.
struct Resolution {
    /// Every word of the set, sorted; `fresh`: those not resolved before in the cycle.
    cands: Vec<usize>,
    fresh: Vec<usize>,
    filter: Vec<u64>,
    /// Cursors in the object list and the wide list.
    pos: usize,
    wpos: usize,
    read: usize,
    found: Vec<*mut Header>,
    us: u64,
}

impl Resolution {
    #[inline]
    fn hit(&self, a: usize) -> bool {
        let k = (a >> 12) & (FILTER_BITS - 1);
        !self.filter.is_empty() && self.filter[k >> 6] & (1 << (k & 63)) != 0
    }
    /// A payload of at most a page lies on the page of its start and the next one.
    #[inline]
    fn near(&self, payload: usize) -> bool {
        self.hit(payload) || self.hit(payload + 4095)
    }
}

/// Is a word of `set` (sorted) in `lo .. hi`?
#[inline]
fn any_in(set: &[usize], lo: usize, hi: usize) -> bool {
    let i = set.partition_point(|&c| c < lo);
    i < set.len() && set[i] < hi
}

/// Does a word of `set` point into the object (`near`: look at a small payload too) or into its buffer?
///
/// # Safety
/// `h` is the header of an object of the heap that has not been freed.
unsafe fn refers(h: *mut Header, near: bool, set: &[usize]) -> bool {
    if (*h).state & !3 != LIVE {
        return false;
    }
    let info = (*h).info;
    let payload = h as usize + HEADER;
    let size = (info.size_of)(payload as *const u8).max(1);
    if (near || size > 4096) && any_in(set, payload, payload + size) {
        return true;
    }
    if info.wide {
        let (b, len) = (info.buffer)(payload as *const u8);
        return len > 0 && any_in(set, b, b + len);
    }
    false
}

/// The addresses a heap pointer of this process can have, for the conservative stack scan: above the first 64 KB,
/// below the kernel half on x86-64; on wasm32 the whole 32-bit linear memory above 64 KB (a word that is no object
/// resolves to nothing: the filter only saves work).
#[cfg(target_pointer_width = "64")]
const HEAP_ADDRESSES: core::ops::Range<usize> = 0x1_0000..0x8000_0000_0000;
#[cfg(target_pointer_width = "32")]
const HEAP_ADDRESSES: core::ops::Range<usize> = 0x1_0000..usize::MAX;

thread_local! {
    /// Rust heap blocks (start address -> length in bytes) that hold object handles and are owned by values which
    /// can sit inside a suspended iterator method's state ([`register_scan_block`]).
    static SCAN_BLOCKS: RefCell<std::collections::HashMap<usize, usize>> = RefCell::new(std::collections::HashMap::new());
}

/// Declares `len` bytes at `p` as memory that holds object handles and is owned (through a plain Rust pointer) by a
/// value the collector only finds by scanning bytes conservatively: the state of a suspended iterator method
/// (`rt::iter::Machine`) can own an enumerator with a `Vec` of handles or a boxed enumerator, and the handles in
/// those are in a heap block of their own, which a scan of the state's bytes would miss. A registered block is
/// scanned when a pointer to its start is found during a conservative scan. The owner must call
/// [`unregister_scan_block`] before the block is freed or moved.
pub fn register_scan_block(p: *const u8, len: usize) {
    if len != 0 {
        SCAN_BLOCKS.with(|b| b.borrow_mut().insert(p as usize, len));
    }
}

/// See [`register_scan_block`]. Safe to call for a block that was not registered, and during thread shutdown.
pub fn unregister_scan_block(p: *const u8) {
    let _ = SCAN_BLOCKS.try_with(|b| {
        if let Ok(mut b) = b.try_borrow_mut() {
            b.remove(&(p as usize));
        }
    });
}

#[cold]
#[inline(never)]
fn dangling(p: *const u8) -> ! {
    panic!("terraria-gen gc: a reachable handle points at a freed object ({p:p}): a handle was held outside the roots across rt::collect()")
}

/// Where this thread's collector is in its cycle.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum GcPhase {
    /// No cycle is running; every object is white.
    #[default]
    Idle,
    /// Marking, in slices ([`gc_step`]); the program runs in between and the store barrier ([`wb`]) is active.
    Mark,
    /// Freeing the objects the mark phase did not reach, in slices.
    Sweep,
}

/// An entry of `Heap::wide`. `lo .. hi`: the memory beyond the payload's first two pages where a word can point
/// (a large payload: known at allocation; a fixed buffer: read from the object the first time it is needed);
/// `known == false`: not read yet, or a buffer that moves (read through the header every time).
#[derive(Clone, Copy)]
struct Wide {
    h: *mut Header,
    lo: usize,
    hi: usize,
    known: bool,
}

#[derive(Default)]
struct Heap {
    objects: Vec<*mut Header>,
    bytes: usize,
    statics: Vec<unsafe fn(&mut Marker)>,
    roots: Vec<(u64, *const u8, unsafe fn(*const u8, &mut Marker))>,
    dyn_roots: Vec<(u64, Box<dyn Fn(&mut Marker)>)>,
    next_root: u64,
    poison: bool,
    verify: bool,
    quarantined_bytes: usize,
    collections: u64,
    allocated_since_collect: usize,
    phase: GcPhase,
    marker: Marker,
    /// Live objects of [`Trace::RESCAN`] types: traced again when the mark phase ends.
    rescan: Vec<*mut Header>,
    /// Live objects a conservative word can refer to from beyond the page after their payload's start: a payload
    /// larger than 4096 bytes or a buffer of their own ([`Trace::HAS_BUFFER`]). The resolution of candidate words
    /// reads the header of these and of objects near a candidate only ([`Marker::resolve_candidates`]). Kept as
    /// `rescan` is: unmarked ones are dropped when a mark phase ends, new ones are added by `register`.
    wide: Vec<Wide>,
    /// The sweep first drops the unmarked entries of `wide` (`wide[wide_pos..wide_end]` still to be looked at,
    /// `wide[..wide_kept]` survived); `wide_end == usize::MAX`: done for this cycle.
    wide_pos: usize,
    wide_kept: usize,
    wide_end: usize,
    /// Sweep cursor: `objects[sweep_pos..sweep_end]` is still to be looked at, `objects[..sweep_kept]` survived;
    /// objects allocated during the sweep are behind `sweep_end`.
    sweep_pos: usize,
    sweep_kept: usize,
    sweep_end: usize,
    cycle: GcStats,
}

thread_local! {
    static HEAP: RefCell<Heap> = RefCell::new(Heap::default());
}

thread_local! {
    /// Diagnostics of the collector's unbounded steps ([`gc_profile`]).
    static PROFILE: RefCell<Vec<GcProfile>> = const { RefCell::new(Vec::new()) };
    /// Bytes and microseconds of the last scan of suspended stacks (`scan_roots`).
    static LAST_STACK_SCAN: Cell<(usize, u64)> = const { Cell::new((0, 0)) };
}

/// One unbounded step of the collector, timed (diagnostics: which part of a long pause costs what).
#[derive(Clone, Copy, Debug)]
pub enum GcProfile {
    /// A resolution of conservative words: distinct candidates, those not resolved before in the cycle, objects in the list, wide objects, wide objects
    /// whose header was read, objects found, microseconds.
    Resolve { candidates: usize, fresh: usize, objects: usize, wide: usize, read: usize, found: usize, us: u64 },
    /// The end of a mark phase: roots scanned again (stacks included), `RESCAN` objects traced again, the work list
    /// drained (resolutions included), total; microseconds.
    Finish { roots_us: u64, stack_bytes: usize, stacks_us: u64, rescan_us: u64, drain_us: u64, total_us: u64 },
}

/// Takes the collector's diagnostics recorded since the last call ([`GcProfile`]).
pub fn gc_profile() -> Vec<GcProfile> {
    PROFILE.with(|p| core::mem::take(&mut *p.borrow_mut()))
}

/// [`gc_profile`] as report lines: the resolutions summarised, every end of a mark phase above 4 ms in full.
pub fn gc_profile_report() -> Vec<String> {
    let all = gc_profile();
    let mut out = Vec::new();
    let mut us: Vec<u64> = Vec::new();
    let (mut cands, mut found, mut wide, mut objects, mut reads, mut no_fresh) = (Vec::new(), 0usize, 0usize, 0usize, 0usize, 0usize);
    for e in &all {
        if let GcProfile::Resolve { candidates, fresh: fr, objects: o, wide: w, read: r, found: f, us: u } = *e {
            us.push(u);
            if fr == 0 {
                no_fresh += 1;
            }
            reads = reads.max(r);
            cands.push(candidates);
            found += f;
            wide = wide.max(w);
            objects = objects.max(o);
        }
    }
    if !us.is_empty() {
        us.sort_unstable();
        cands.sort_unstable();
        let pick = |v: &Vec<u64>, q: f64| v[((v.len() - 1) as f64 * q) as usize];
        out.push(format!(
            "resolutions of conservative words: {} ({} without a new word); time p50 {} us, p99 {} us, max {} us; distinct candidates p50 {}, max {}; objects found {} in all; largest object list {}, largest wide list {}, most wide headers read in one {}",
            us.len(), no_fresh, pick(&us, 0.5), pick(&us, 0.99), us[us.len() - 1], cands[cands.len() / 2], cands[cands.len() - 1], found, objects, wide, reads
        ));
    }
    let mut n = 0;
    for e in &all {
        if let GcProfile::Finish { roots_us, stack_bytes, stacks_us, rescan_us, drain_us, total_us } = *e {
            n += 1;
            if total_us >= 4000 {
                out.push(format!("end of a mark phase: {total_us} us = roots {roots_us} (suspended stacks {stack_bytes} bytes in {stacks_us}) + rescan {rescan_us} + drain {drain_us}"));
            }
        }
    }
    out.push(format!("ends of a mark phase: {n}"));
    out
}

fn profile(e: GcProfile) {
    PROFILE.with(|p| {
        let mut p = p.borrow_mut();
        if p.len() < 100_000 {
            p.push(e)
        }
    })
}

/// What one collection did.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct GcStats {
    pub live_objects: usize,
    pub live_bytes: usize,
    pub freed_objects: usize,
    pub freed_bytes: usize,
    /// Objects of [`Trace::RESCAN`] types that were traced a second time when the mark phase ended.
    pub rescanned: usize,
    /// Passes over the object list to resolve conservatively scanned words (suspended iterator methods).
    pub resolve_passes: usize,
}

/// Number of live objects and their payload bytes (this thread's heap).
pub fn heap_size() -> (usize, usize) {
    HEAP.with(|h| {
        let h = h.borrow();
        (h.objects.len(), h.bytes)
    })
}

/// Collections finished so far on this thread (`GC.CollectionCount(0)`: there is one generation).
pub fn collection_count() -> u64 {
    HEAP.with(|h| h.borrow().collections)
}

/// Payload bytes allocated since the last collection: the host's trigger ("collect when above N MB").
pub fn allocated_since_collect() -> usize {
    HEAP.with(|h| h.borrow().allocated_since_collect)
}

/// Debug mode: freed objects are filled with `0xDD`, marked dead and kept out of the allocator for good.
pub fn set_poison(on: bool) {
    HEAP.with(|h| h.borrow_mut().poison = on)
}

/// Called by each generated static initialiser: `trace` reports the handles in that class's static fields.
pub fn register_statics(trace: unsafe fn(&mut Marker)) {
    HEAP.with(|h| h.borrow_mut().statics.push(trace))
}

/// Registers a host root; the closure reports the handles the host holds. Returns an id for [`remove_root`].
pub fn add_root(f: impl Fn(&mut Marker) + 'static) -> u64 {
    HEAP.with(|h| {
        let mut h = h.borrow_mut();
        h.next_root += 1;
        let id = h.next_root;
        h.dyn_roots.push((id, Box::new(f)));
        id
    })
}

pub fn remove_root(id: u64) {
    HEAP.with(|h| h.borrow_mut().dyn_roots.retain(|(k, _)| *k != id))
}

/// A value the host keeps alive across collections (a handle, or a struct of handles).
pub struct Rooted<T: Trace + Copy + 'static> {
    slot: Box<T>,
    id: u64,
}

impl<T: Trace + Copy + 'static> Rooted<T> {
    pub fn new(value: T) -> Rooted<T> {
        let slot = Box::new(value);
        let p = &*slot as *const T as *const u8;
        let id = HEAP.with(|h| {
            let mut h = h.borrow_mut();
            h.next_root += 1;
            let id = h.next_root;
            h.roots.push((id, p, trace_thunk::<T>));
            id
        });
        Rooted { slot, id }
    }
    #[inline(always)]
    pub fn get(&self) -> T {
        *self.slot
    }
    pub fn set(&mut self, v: T) {
        *self.slot = v;
    }
}

impl<T: Trace + Copy + 'static> Drop for Rooted<T> {
    fn drop(&mut self) {
        let id = self.id;
        // the heap is gone when a thread is being torn down
        let _ = HEAP.try_with(|h| h.borrow_mut().roots.retain(|(k, _, _)| *k != id));
    }
}

/// Runs a complete collection on this thread's heap, in one go: exactly the objects reachable from the roots at
/// the call survive. A cycle that [`gc_step`] left unfinished is completed first.
///
/// # Safety
/// No generated function may be on the stack (of any fiber), and every handle the host still uses must be
/// reachable from a registered root (module docs). Violating this frees objects that are in use.
pub unsafe fn collect() -> GcStats {
    with_heap(|heap| {
        let mut unlimited = Budget::unlimited();
        if heap.phase == GcPhase::Mark {
            mark_work(heap, &mut unlimited);
            finish_mark(heap);
        }
        if heap.phase == GcPhase::Sweep {
            sweep_work(heap, &mut unlimited);
        }
        begin(heap);
        mark_work(heap, &mut unlimited);
        finish_mark(heap);
        sweep_work(heap, &mut unlimited);
        heap.cycle
    })
}

/// How much one [`gc_step`] may do.
#[derive(Clone, Copy, Debug)]
pub enum GcBudget {
    /// Wall-clock microseconds (the clock is read every few dozen objects).
    Micros(u64),
    /// Units of work, about one per object looked at: repeatable, for tests.
    Work(usize),
    Unlimited,
}

/// What a [`gc_step`] did.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct GcStep {
    pub before: GcPhase,
    pub after: GcPhase,
    /// The mark phase ended in this step (roots and `RESCAN` objects traced again, everything drained: the one
    /// part of a cycle that is not bounded by the budget).
    pub mark_ended: bool,
    /// The cycle ended in this step.
    pub finished: Option<GcStats>,
}

struct Budget {
    deadline: Option<std::time::Instant>,
    work: usize,
}
impl Budget {
    fn unlimited() -> Budget {
        Budget { deadline: None, work: usize::MAX }
    }
    fn new(b: GcBudget) -> Budget {
        match b {
            GcBudget::Micros(us) => Budget { deadline: Some(std::time::Instant::now() + std::time::Duration::from_micros(us)), work: usize::MAX },
            GcBudget::Work(n) => Budget { deadline: None, work: n.min(usize::MAX - 1) },
            GcBudget::Unlimited => Budget::unlimited(),
        }
    }
    /// Accounts `units` of work; `true` when the budget is used up.
    #[inline]
    fn spent(&mut self, units: usize) -> bool {
        if let Some(d) = self.deadline {
            return std::time::Instant::now() >= d;
        }
        if self.work == usize::MAX {
            return false;
        }
        self.work = self.work.saturating_sub(units);
        self.work == 0
    }
}

/// The heap is moved out of its thread-local cell while the collector works on it: tracers and `Drop` impls of
/// freed objects may touch the cell (`unregister_scan_block`, a `Rooted` inside a freed object).
unsafe fn with_heap<R>(f: impl FnOnce(&mut Heap) -> R) -> R {
    let mut heap = HEAP.with(|h| core::mem::take(&mut *h.borrow_mut()));
    let r = f(&mut heap);
    // roots registered while collecting cannot exist (tracers do not call back), so plain move back
    HEAP.with(|h| *h.borrow_mut() = heap);
    r
}

/// The collector's phase on this thread.
pub fn gc_phase() -> GcPhase {
    HEAP.with(|h| h.borrow().phase)
}

/// Starts an incremental cycle (no effect when one is running): marks what the roots refer to. The pause is the
/// size of the roots, not of the heap.
///
/// # Safety
/// As [`gc_step`].
pub unsafe fn gc_begin() {
    with_heap(|heap| {
        if heap.phase == GcPhase::Idle {
            begin(heap)
        }
    })
}

/// One slice of the running cycle, at most `budget` long (see [`GcStep::mark_ended`] for the exception).
/// `may_finish`: the mark phase may end in this step. It must be `false` while generated code is suspended on
/// another fiber (its locals are not roots); marking and sweeping slices are still allowed then.
///
/// The incremental cycle: [`gc_begin`] marks from the roots; mark slices trace the heap while the program runs
/// in between. The program keeps the collector's invariant through the store barrier [`wb`] (a handle stored
/// anywhere while a mark phase runs is marked: Dijkstra's insertion barrier) and objects allocated during the mark
/// phase are queued for tracing. When the work list is empty the phase ends: roots and the objects of
/// [`Trace::RESCAN`] types are traced once more, with the program stopped. Then sweep slices free what was not
/// reached. Objects that became garbage after they were marked survive until the next cycle.
///
/// # Safety
/// No generated function may be on the stack of the running fiber, and every handle the host still uses must be
/// reachable from a registered root.
pub unsafe fn gc_step(budget: GcBudget, may_finish: bool) -> GcStep {
    with_heap(|heap| {
        let mut step = GcStep { before: heap.phase, ..GcStep::default() };
        let mut b = Budget::new(budget);
        if heap.phase == GcPhase::Mark && mark_work(heap, &mut b) && may_finish {
            finish_mark(heap);
            step.mark_ended = true;
        }
        if heap.phase == GcPhase::Sweep && !step.mark_ended && sweep_work(heap, &mut b) {
            step.finished = Some(heap.cycle);
        }
        step.after = heap.phase;
        step
    })
}

/// The verifier: when a mark phase ends, the heap is walked a second time from the roots, independently, and the
/// collection panics if that walk reaches an object the incremental marking left unmarked (a store that was not
/// behind the barrier), naming the object's type. Slow; for tests and survey sessions.
pub fn set_verify(on: bool) {
    HEAP.with(|h| h.borrow_mut().verify = on)
}

/// Live objects by Rust type: (name, count, payload bytes), largest count first. Diagnostics.
pub fn heap_histogram() -> Vec<(&'static str, usize, usize)> {
    HEAP.with(|h| {
        let h = h.borrow();
        let mut by: std::collections::HashMap<usize, (&'static str, usize, usize)> = std::collections::HashMap::new();
        for &o in &h.objects {
            // SAFETY: the object list holds live or not yet swept objects; their headers are readable.
            unsafe {
                let info = (*o).info;
                let e = by.entry(info as *const TypeInfo as usize).or_insert(((info.name)(), 0, 0));
                e.1 += 1;
                e.2 += (info.size_of)((o as *const u8).add(HEADER));
            }
        }
        // one Rust type can have several `TypeInfo` addresses (one per codegen unit)
        let mut names: std::collections::HashMap<&'static str, (usize, usize)> = std::collections::HashMap::new();
        for (_, (n, c, b)) in by {
            let e = names.entry(n).or_insert((0, 0));
            e.0 += c;
            e.1 += b;
        }
        let mut v: Vec<_> = names.into_iter().map(|(n, (c, b))| (n, c, b)).collect();
        v.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(b.0)));
        v
    })
}

/// Number of live objects of [`Trace::RESCAN`] types (what the end of a mark phase traces again), by type.
pub fn rescan_histogram() -> Vec<(&'static str, usize)> {
    HEAP.with(|h| {
        let h = h.borrow();
        let mut names: std::collections::HashMap<&'static str, usize> = std::collections::HashMap::new();
        for &o in &h.rescan {
            // SAFETY: as `heap_histogram`; the list is cleaned before each sweep.
            unsafe { *names.entry(((*o).info.name)()).or_insert(0) += 1 };
        }
        let mut v: Vec<_> = names.into_iter().collect();
        v.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(b.0)));
        v
    })
}

unsafe fn scan_roots(heap: &mut Heap) {
    for f in &heap.statics {
        f(&mut heap.marker);
    }
    for (_, p, f) in &heap.roots {
        f(*p, &mut heap.marker);
    }
    for (_, f) in &heap.dyn_roots {
        f(&mut heap.marker);
    }
    if let Some(stacks) = CONSERVATIVE_ROOTS.get() {
        let t0 = std::time::Instant::now();
        let marker = &mut heap.marker;
        let mut bytes = 0usize;
        stacks(&mut |p, len| {
            bytes += len;
            marker.mark_conservative_stack(p, len)
        });
        LAST_STACK_SCAN.set((bytes, t0.elapsed().as_micros() as u64));
    }
}

thread_local! {
    static CONSERVATIVE_ROOTS: Cell<Option<fn(&mut dyn FnMut(*const u8, usize))>> = const { Cell::new(None) };
}

/// The host's source of stack memory that holds handles of suspended generated code (work fibers that gave the
/// frame back; a blocking main loop asleep): `stacks` calls its argument with every range. Scanned with the roots:
/// when a cycle begins, when its mark phase ends and by the verifier
/// ([`Marker::mark_conservative_stack`]). With it installed a mark phase may end while such code is suspended
/// (`gc_step(.., may_finish: true)`), which is what lets a long load, a generation or a network session collect.
pub fn set_conservative_roots(stacks: Option<fn(&mut dyn FnMut(*const u8, usize))>) {
    CONSERVATIVE_ROOTS.set(stacks);
}

/// Is a source of conservative roots installed ([`set_conservative_roots`])?
pub fn has_conservative_roots() -> bool {
    CONSERVATIVE_ROOTS.get().is_some()
}

unsafe fn begin(heap: &mut Heap) {
    debug_assert!(heap.phase == GcPhase::Idle && heap.marker.is_empty());
    heap.phase = GcPhase::Mark;
    MARKING.fetch_add(1, core::sync::atomic::Ordering::Relaxed);
    heap.allocated_since_collect = 0;
    heap.cycle = GcStats::default();
    heap.marker.blocks_seen.clear();
    heap.marker.resolved.clear();
    scan_roots(heap);
}

/// Traces from the work list until it is empty (`true`) or the budget is used up (`false`).
unsafe fn mark_work(heap: &mut Heap, budget: &mut Budget) -> bool {
    let verifying = heap.marker.verify.is_some();
    loop {
        let mut n = 0usize;
        while let Some(h) = heap.marker.stack.pop() {
            if !verifying {
                (*h).state = MARKED;
            }
            ((*h).info.trace)((h as *const u8).add(HEADER), &mut heap.marker);
            n += 1;
            if n & 7 == 0 && budget.spent(8) {
                return false;
            }
        }
        if let Some(r) = heap.marker.ranges.last_mut() {
            let k = r.n.min(RANGE_CHUNK);
            let (p, f) = (r.p, r.f);
            r.p = r.p.add(k * r.size);
            r.n -= k;
            if r.n == 0 {
                heap.marker.ranges.pop();
            }
            f(p, k, &mut heap.marker);
            if budget.spent(k) {
                return false;
            }
            continue;
        }
        // the words the conservative scans of this round found (suspended stacks, iterator methods): the objects
        // among them are marked and traced in the next round
        match heap.marker.resolve_candidates(&heap.objects, &mut heap.wide, budget) {
            Resolve::Idle => return true,
            Resolve::OutOfBudget => return false,
            Resolve::Done => heap.cycle.resolve_passes += 1,
        }
    }
}

/// Ends the mark phase. The program is stopped and no generated code is on any stack, so after the roots and the
/// objects that can be written without the barrier are traced again and the work list is empty, every reachable
/// object is marked.
unsafe fn finish_mark(heap: &mut Heap) {
    let mut unlimited = Budget::unlimited();
    let t0 = std::time::Instant::now();
    scan_roots(heap);
    let t1 = std::time::Instant::now();
    let list = core::mem::take(&mut heap.rescan);
    for &h in &list {
        if (*h).state == MARKED {
            heap.cycle.rescanned += 1;
            ((*h).info.trace)((h as *const u8).add(HEADER), &mut heap.marker);
        }
    }
    let t2 = std::time::Instant::now();
    mark_work(heap, &mut unlimited);
    let t3 = std::time::Instant::now();
    let (stack_bytes, stacks_us) = LAST_STACK_SCAN.replace((0, 0));
    profile(GcProfile::Finish { roots_us: (t1 - t0).as_micros() as u64, stack_bytes, stacks_us, rescan_us: (t2 - t1).as_micros() as u64, drain_us: (t3 - t2).as_micros() as u64, total_us: (t3 - t0).as_micros() as u64 });
    // unmarked ones are about to be freed; objects of these types that are allocated during the sweep are added
    // behind (`register`)
    heap.rescan = list.into_iter().filter(|h| (**h).state == MARKED).collect();
    // the unmarked entries of `wide` are dropped by the first sweep slices (`sweep_work`), not in this pause
    heap.wide_pos = 0;
    heap.wide_kept = 0;
    heap.wide_end = heap.wide.len();
    if heap.verify {
        verify(heap);
    }
    MARKING.fetch_sub(1, core::sync::atomic::Ordering::Relaxed);
    heap.marker.blocks_seen.clear();
    heap.marker.resolved.clear();
    heap.phase = GcPhase::Sweep;
    heap.sweep_pos = 0;
    heap.sweep_kept = 0;
    heap.sweep_end = heap.objects.len();
}

/// See [`set_verify`].
unsafe fn verify(heap: &mut Heap) {
    heap.marker.verify = Some(Box::default());
    heap.marker.blocks_seen.clear();
    scan_roots(heap);
    mark_work(heap, &mut Budget::unlimited());
    let v = heap.marker.verify.take().unwrap();
    if !v.missed.is_empty() {
        let mut names: std::collections::BTreeMap<&'static str, usize> = Default::default();
        for h in &v.missed {
            *names.entry(((**h).info.name)()).or_insert(0) += 1;
        }
        panic!("terraria-gen gc: the incremental mark phase missed {} reachable object(s) (a handle was stored without rt::wb, in a type that does not ask for a rescan): {names:?}", v.missed.len());
    }
}

/// Frees unmarked objects from the sweep cursor on, until the list is done (`true`) or the budget is used up.
/// A live object costs one header access (prefetched a few objects ahead) and nothing else: its size is only
/// needed when it is freed, the live byte count is the old one minus what was freed.
unsafe fn sweep_work(heap: &mut Heap, budget: &mut Budget) -> bool {
    let poison = heap.poison;
    let end = heap.sweep_end;
    let mut i = heap.sweep_pos;
    let mut kept = heap.sweep_kept;
    let mut out = false;
    // a freed object costs far more than a kept one (its `Drop`, the allocator): counted as 8
    let mut work = 0usize;
    // first the entries of `wide` whose object is about to be freed (their headers are readable until then)
    if heap.wide_end != usize::MAX {
        let wend = heap.wide_end.min(heap.wide.len());
        while heap.wide_pos < wend {
            if heap.wide_pos + 12 < wend {
                prefetch_read(heap.wide[heap.wide_pos + 12].h as *const u8);
            }
            let e = heap.wide[heap.wide_pos];
            heap.wide_pos += 1;
            if (*e.h).state == MARKED {
                heap.wide[heap.wide_kept] = e;
                heap.wide_kept += 1;
            }
            work += 1;
            if work >= 128 {
                if budget.spent(work) {
                    return false;
                }
                work = 0;
            }
        }
        // entries of objects allocated during the sweep stay, behind the survivors
        let len = heap.wide.len();
        heap.wide.copy_within(wend..len, heap.wide_kept);
        heap.wide.truncate(heap.wide_kept + (len - wend));
        heap.wide_end = usize::MAX;
    }
    while i < end {
        if i + 12 < end {
            prefetch_read(heap.objects[i + 12] as *const u8);
        }
        let h = heap.objects[i];
        i += 1;
        if (*h).state == MARKED {
            (*h).state = LIVE;
            heap.objects[kept] = h;
            kept += 1;
            work += 1;
        } else {
            work += 8;
            let payload = (h as *mut u8).add(HEADER);
            let size = ((*h).info.size_of)(payload);
            heap.cycle.freed_objects += 1;
            heap.cycle.freed_bytes += size;
            heap.bytes -= size;
            ((*h).info.drop)(payload);
            (*h).state = DEAD;
            if poison {
                core::ptr::write_bytes(payload, 0xDD, size);
                heap.quarantined_bytes += size;
            } else {
                dealloc(h as *mut u8, Layout::from_size_align_unchecked(HEADER + size, HEADER));
            }
        }
        if work >= 128 {
            if budget.spent(work) {
                out = true;
                break;
            }
            work = 0;
        }
    }
    heap.sweep_pos = i;
    heap.sweep_kept = kept;
    if out && i < end {
        return false;
    }
    // close the gap between the survivors and the objects allocated during the sweep
    let len = heap.objects.len();
    heap.objects.copy_within(end..len, kept);
    heap.objects.truncate(kept + (len - end));
    heap.sweep_pos = 0;
    heap.sweep_kept = 0;
    heap.sweep_end = 0;
    heap.phase = GcPhase::Idle;
    heap.collections += 1;
    heap.cycle.live_objects = heap.objects.len();
    heap.cycle.live_bytes = heap.bytes;
    true
}

/// The store barrier. Call it with every value that is stored into memory the collector traces (a field of an
/// object, an array element, a collection, a captured variable's cell, anything behind a `ref`), before or after
/// the store. While a mark phase runs, the objects the value refers to are marked, so that a handle moved into
/// an object the collector has already traced is not lost (see [`gc_step`]). Otherwise: one load and a branch;
/// nothing at all for types without handles.
#[inline(always)]
pub fn wb<T: Trace>(v: &T) {
    if T::NEEDS_TRACE && MARKING.load(core::sync::atomic::Ordering::Relaxed) != 0 {
        wb_slow(v)
    }
}

/// [`wb`] as an identity function, for the value side of an assignment: `place = rt::wbv(value)`.
#[inline(always)]
pub fn wbv<T: Trace>(v: T) -> T {
    wb(&v);
    v
}

/// [`wb`] for every element of a slice (bulk copies inside the runtime).
#[inline(always)]
pub fn wb_slice<T: Trace>(s: &[T]) {
    if T::NEEDS_TRACE && MARKING.load(core::sync::atomic::Ordering::Relaxed) != 0 {
        for x in s {
            wb_slow(x)
        }
    }
}

#[cold]
#[inline(never)]
fn wb_slow<T: Trace>(v: &T) {
    HEAP.with(|h| {
        let mut h = h.borrow_mut();
        // another thread's heap may be the one that is marking
        if h.phase == GcPhase::Mark {
            // SAFETY: `v` is a valid value; tracers only mark.
            unsafe { v.trace(&mut h.marker) }
        }
    })
}

fn register(h: *mut Header, size: usize) {
    HEAP.with(|heap| {
        let mut heap = heap.borrow_mut();
        heap.objects.push(h);
        heap.bytes += size;
        heap.allocated_since_collect += size;
        // SAFETY: `h` is the header `alloc_raw` just wrote.
        unsafe {
            let info = (*h).info;
            if heap.phase == GcPhase::Mark {
                // born during a mark phase: survives it. A traceable object is queued, so what the code that
                // creates it writes into it (before the next slice) needs no barrier.
                if info.needs_trace {
                    (*h).state = GRAY;
                    heap.marker.stack.push(h);
                } else {
                    (*h).state = MARKED;
                }
            }
            if info.rescan {
                heap.rescan.push(h);
            }
            if info.has_buffer {
                heap.wide.push(Wide { h, lo: 0, hi: 0, known: false });
            } else if info.wide && size > 4096 {
                let p = h as usize + HEADER;
                heap.wide.push(Wide { h, lo: p, hi: p + size, known: true });
            }
        }
    })
}

/// Reference to a class instance. `Copy`; equality is identity; all-zero is `null`.
#[repr(transparent)]
pub struct Gc<T>(pub *mut T);

impl<T> Clone for Gc<T> {
    #[inline(always)]
    fn clone(&self) -> Self {
        *self
    }
}
impl<T> Copy for Gc<T> {}
impl<T> PartialEq for Gc<T> {
    #[inline(always)]
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}
impl<T> Eq for Gc<T> {}
impl<T> core::fmt::Debug for Gc<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "Gc({:p})", self.0)
    }
}

impl<T> Gc<T> {
    pub const NULL: Gc<T> = Gc(core::ptr::null_mut());

    #[inline(always)]
    pub fn is_null(self) -> bool {
        self.0.is_null()
    }

    /// Identity, for `(object)a == (object)b`.
    #[inline(always)]
    pub fn addr(self) -> usize {
        self.0 as *const u8 as usize
    }

    /// The pointer, after the null check C# performs on every member access (`NullReferenceException`).
    #[inline(always)]
    pub fn ptr(self) -> *mut T {
        if self.0.is_null() {
            null_reference();
        }
        #[cfg(feature = "gc-check")]
        // SAFETY: a non-null Gc points at a payload from `alloc`; quarantined headers stay readable.
        unsafe {
            if (*((self.0 as *const u8).sub(HEADER) as *const Header)).state & !3 != LIVE {
                use_after_free(self.0 as *const u8);
            }
        }
        self.0
    }

    /// Reference conversion the C# compiler already proved (derived to base, or a generic parameter erased
    /// to its class constraint). Classes are `#[repr(C)]` with the base as first field, so the address is the same.
    #[inline(always)]
    pub fn up<U>(self) -> Gc<U> {
        Gc(self.0 as *mut U)
    }
}

#[cfg(feature = "gc-check")]
#[cold]
#[inline(never)]
fn use_after_free(p: *const u8) -> ! {
    panic!("terraria-gen gc: use after free ({p:p})")
}

#[cold]
#[inline(never)]
pub(crate) fn null_reference() -> ! {
    throw(exc("System.NullReferenceException", crate::s!("Object reference not set to an instance of an object.")))
}

/// Target of references to types the tool does not translate (`object`, interfaces, unmapped classes).
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Object {
    _opaque: u8,
}
pub type Obj = Gc<Object>;
unsafe impl Trace for Object {
    const NEEDS_TRACE: bool = false;
    const RESCAN: bool = false;
    unsafe fn trace(&self, _m: &mut Marker) {}
    // a handle of static type `object` / an interface as a dictionary key: the virtual Equals of the object
    unsafe fn obj_to_string(_this: *const Self) -> Str {
        Str::lit("System.Object")
    }
}

/// `Object.GetHashCode()` of an object at `addr`: stable for the object's life (objects never move).
#[inline]
pub fn identity_hash(addr: usize) -> i32 {
    ((addr >> 4) as u32).wrapping_mul(0x9E37_79B1) as i32 & 0x7FFF_FFFF
}

/// `a.Equals(b)` (virtual; `NullReferenceException` when `a` is null).
pub fn obj_equals(a: Obj, b: Obj) -> bool {
    if a.is_null() {
        null_reference();
    }
    // SAFETY: every non-null Gc comes from `alloc`; its header names the payload type.
    unsafe { ((*(a.0 as *const u8).sub(HEADER).cast::<Header>()).info.equals)(a.0 as *const u8, b) }
}

/// `object.Equals(a, b)`: same reference, or neither null and `a.Equals(b)`.
pub fn object_equals(a: Obj, b: Obj) -> bool {
    if a.addr() == b.addr() {
        return true;
    }
    if a.is_null() || b.is_null() {
        return false;
    }
    obj_equals(a, b)
}

/// `o.GetHashCode()` (virtual).
pub fn obj_hash_code(o: Obj) -> i32 {
    if o.is_null() {
        null_reference();
    }
    // SAFETY: as `obj_equals`.
    unsafe { ((*(o.0 as *const u8).sub(HEADER).cast::<Header>()).info.hash)(o.0 as *const u8) }
}

/// `o.ToString()` (virtual).
pub fn obj_to_string(o: Obj) -> Str {
    if o.is_null() {
        null_reference();
    }
    // SAFETY: as `obj_equals`.
    unsafe { ((*(o.0 as *const u8).sub(HEADER).cast::<Header>()).info.to_string)(o.0 as *const u8) }
}

unsafe fn alloc_raw(size: usize, type_id: u32, info: &'static TypeInfo) -> *mut u8 {
    let layout = Layout::from_size_align(HEADER + size, HEADER).expect("layout");
    let base = alloc_zeroed(layout);
    if base.is_null() {
        handle_alloc_error(layout);
    }
    (base as *mut Header).write(Header { type_id, state: LIVE, info });
    register(base as *mut Header, size);
    base.add(HEADER)
}

/// Allocates a zeroed `T` with the given type id (0 = runtime object, not a translated class).
pub fn alloc<T: Trace + 'static>(type_id: u32) -> Gc<T> {
    const { assert!(core::mem::align_of::<T>() <= HEADER) };
    // SAFETY: the header is written before the pointer escapes; a zeroed payload is valid for every type
    // this is called with (generated classes) or is overwritten at once (`alloc_init`).
    unsafe { Gc(alloc_raw(core::mem::size_of::<T>(), type_id, &InfoOf::<T>::INFO) as *mut T) }
}

/// Allocates and moves `value` in: for runtime types whose zero pattern is not valid (ones holding a `Vec`)
/// and for natively mapped classes (`new UnifiedRandom(seed)`).
pub fn alloc_init<T: Trace + 'static>(value: T) -> Gc<T> {
    alloc_init_id(value, 0)
}

pub fn alloc_init_id<T: Trace + 'static>(value: T, type_id: u32) -> Gc<T> {
    let g = alloc::<T>(type_id);
    // SAFETY: fresh allocation of the right size and alignment.
    unsafe { g.0.write(value) };
    if T::HAS_BUFFER && T::BUFFER_FIXED {
        // the buffer's range into the entry `register` just pushed: arrays are made and dropped by the hundred
        // thousand per cycle in a session, and a resolution would otherwise read each new one's header once
        // SAFETY: the object is initialised.
        let (b, len) = unsafe { T::obj_buffer(g.0) };
        let h = unsafe { (g.0 as *mut u8).sub(HEADER) as *mut Header };
        HEAP.with(|heap| {
            if let Ok(mut heap) = heap.try_borrow_mut() {
                if let Some(e) = heap.wide.last_mut().filter(|e| e.h == h) {
                    *e = Wide { h, lo: b, hi: b + len, known: true };
                }
            }
        });
    }
    g
}

/// A variable-sized, untraced byte object (string storage): `[len: usize][bytes]`. Returns the address of the bytes.
pub(crate) fn alloc_bytes(bytes: &[u8]) -> *const u8 {
    static INFO: TypeInfo = TypeInfo {
        size_of: bytes_size,
        trace: no_trace_fn,
        drop: no_drop_fn,
        needs_trace: false,
        rescan: false,
        name: bytes_name,
        plain: false,
        rust_type: core::any::TypeId::of::<[u8]>,
        equals: bytes_equals,
        hash: bytes_hash,
        to_string: bytes_to_string,
        reflect: no_reflect_fn,
        dispose: no_dispose_fn,
        clear: no_clear_fn,
        wide: true,
        buffer: no_buffer_fn,
        has_buffer: false,
        fixed_buffer: false,
    };
    // string storage is never an object of its own
    unsafe fn bytes_equals(p: *const u8, other: Obj) -> bool {
        p as usize == other.addr()
    }
    unsafe fn bytes_hash(p: *const u8) -> i32 {
        identity_hash(p as usize)
    }
    unsafe fn bytes_to_string(_p: *const u8) -> Str {
        Str::lit("")
    }
    fn bytes_name() -> &'static str {
        "string bytes"
    }
    unsafe fn bytes_size(p: *const u8) -> usize {
        core::mem::size_of::<usize>() + *(p as *const usize)
    }
    unsafe fn no_trace_fn(_p: *const u8, _m: &mut Marker) {}
    unsafe fn no_drop_fn(_p: *mut u8) {}
    // SAFETY: the object is `usize` + `bytes.len()` bytes; the length is written before `size_of` can run.
    unsafe {
        let p = alloc_raw(core::mem::size_of::<usize>() + bytes.len(), 0, &INFO);
        (p as *mut usize).write(bytes.len());
        let data = p.add(core::mem::size_of::<usize>());
        core::ptr::copy_nonoverlapping(bytes.as_ptr(), data, bytes.len());
        data
    }
}

/// Marks the byte object whose data starts at `data` (from [`alloc_bytes`]).
pub(crate) unsafe fn mark_bytes(m: &mut Marker, data: *const u8) {
    m.mark_payload(data.sub(core::mem::size_of::<usize>()))
}

/// `new object()`.
pub fn new_object() -> Obj {
    alloc::<Object>(0)
}

/// Type id of the object `p` points to.
///
/// # Safety
/// `p` must be a non-null pointer obtained from [`alloc`] (any `Gc`, or `this` of a translated class).
#[inline(always)]
pub unsafe fn type_id_of(p: *const u8) -> u32 {
    (*(p.sub(HEADER) as *const Header)).type_id
}

/// Rust type of the object `p` points to.
///
/// # Safety
/// As [`type_id_of`].
pub(crate) unsafe fn rt_type_of(p: *const u8) -> core::any::TypeId {
    ((*(p.sub(HEADER) as *const Header)).info.rust_type)()
}

/// Is the payload of the object `o` (non-null) a value of the Rust type `T`? For hosts and tests that hold an
/// `object` and know which native types it may be (a boxed native struct, a native class).
pub fn obj_is<T: 'static>(o: Obj) -> bool {
    // SAFETY: every non-null Gc comes from `alloc`.
    !o.is_null() && unsafe { rt_type_of(o.0 as *const u8) } == core::any::TypeId::of::<T>()
}

/// `object.MemberwiseClone()`: a new object of the same run-time type with every field copied.
///
/// # Safety
/// `p` must point at a live object from [`alloc`] whose fields are all `Copy` data (any translated class).
pub unsafe fn memberwise_clone(p: *const u8) -> Obj {
    let h = &*(p.sub(HEADER) as *const Header);
    assert!(h.info.plain, "MemberwiseClone of a runtime object that owns memory");
    let size = (h.info.size_of)(p);
    let q = alloc_raw(size, h.type_id, h.info);
    core::ptr::copy_nonoverlapping(p, q, size);
    Gc(q as *mut Object)
}

fn derives(mut id: u32, target: u32, parents: &[u32]) -> bool {
    while id != 0 {
        if id == target {
            return true;
        }
        id = parents.get(id as usize).copied().unwrap_or(0);
    }
    false
}

/// `x is T` for a translated class `T` (`parents` is `gen::types::PARENT`).
pub fn is_instance<T>(o: Gc<T>, target: u32, parents: &[u32]) -> bool {
    // SAFETY: every non-null Gc comes from `alloc`.
    !o.is_null() && derives(unsafe { type_id_of(o.0 as *const u8) }, target, parents)
}

/// `x as T`.
pub fn as_class<T, U>(o: Gc<T>, target: u32, parents: &[u32]) -> Gc<U> {
    if is_instance(o, target, parents) {
        Gc(o.0 as *mut U)
    } else {
        Gc::NULL
    }
}

#[cold]
#[inline(never)]
pub(crate) fn invalid_cast() -> ! {
    throw(exc("System.InvalidCastException", crate::s!("Specified cast is not valid.")))
}

/// `(T)x`: null stays null, a wrong type throws `InvalidCastException`.
pub fn cast_class<T, U>(o: Gc<T>, target: u32, parents: &[u32]) -> Gc<U> {
    if o.is_null() || is_instance(o, target, parents) {
        Gc(o.0 as *mut U)
    } else {
        invalid_cast()
    }
}

// ---------------------------------------------------------------------------------------------- boxing

/// Type ids of boxed values of runtime types. Translated classes and structs start at [`tid::FIRST_GENERATED`].
pub mod tid {
    pub const BOOL: u32 = 1;
    pub const U8: u32 = 2;
    pub const I8: u32 = 3;
    pub const I16: u32 = 4;
    pub const U16: u32 = 5;
    pub const I32: u32 = 6;
    pub const U32: u32 = 7;
    pub const I64: u32 = 8;
    pub const U64: u32 = 9;
    pub const F32: u32 = 10;
    pub const F64: u32 = 11;
    /// `char` (a `u16` in the runtime): boxed with its own id, so `(object)'a'` is not a `ushort`.
    pub const CHAR: u32 = 12;
    pub const STRING: u32 = 13;
    pub const COLOR: u32 = 14;
    pub const VECTOR2: u32 = 15;
    pub const VECTOR3: u32 = 16;
    pub const VECTOR4: u32 = 17;
    pub const RECTANGLE: u32 = 18;
    pub const POINT: u32 = 19;
    pub const MATRIX: u32 = 20;
    /// A `System.Type` object (`typeof`).
    pub const TYPE: u32 = 21;
    /// `System.TimeSpan` (`rt::time::TimeSpan`).
    pub const TIME_SPAN: u32 = 22;
    /// `System.DateTime` (`rt::time::DateTime`).
    pub const DATE_TIME: u32 = 23;
    /// `System.IntPtr` (`isize`).
    pub const INT_PTR: u32 = 24;
    /// A boxed value of a library struct the runtime has no id for.
    pub const OPAQUE: u32 = 63;
    pub const FIRST_GENERATED: u32 = 64;
    /// Ids from here up are for value types of host crates (`terraria-host`): the generator never assigns them
    /// (it counts up from 64) and the parent / interface tables treat an unknown id as "no translated class".
    /// A host crate numbers its boxed structs `HOST_FIRST + n` in one place of its own.
    pub const HOST_FIRST: u32 = 0x4000_0000;
}

/// The id values of `T` are boxed with (`Trace::BOX`), for natively mapped structs (`nativeTypes`, kind "struct").
pub const fn box_id<T: Trace>() -> u32 {
    match T::BOX {
        BoxKind::Value(id) => id,
        _ => panic!("cs2rs runtime: a natively mapped struct must set Trace::BOX = BoxKind::Value(id) to be boxed"),
    }
}

/// As [`box_id`], 0 for a struct that declares no box id (`typeof(T)` does not box).
pub const fn box_id_or_zero<T: Trace>() -> u32 {
    match T::BOX {
        BoxKind::Value(id) => id,
        _ => 0,
    }
}

/// Boxing conversion: a value type as `object` (a fresh object holding a copy, as in the CLR).
pub fn box_value<T: Trace + Copy + 'static>(v: T, type_id: u32) -> Obj {
    alloc_init_id(v, type_id).up()
}

/// `o is T` for a boxed value type.
pub fn is_boxed(o: Obj, type_id: u32) -> bool {
    // SAFETY: every non-null Gc comes from `alloc`.
    !o.is_null() && unsafe { type_id_of(o.0 as *const u8) } == type_id
}

/// Unboxing conversion `(T)o`: null throws `NullReferenceException`, another type `InvalidCastException`.
///
/// # Safety
/// `type_id` must be the id values of type `T` are boxed with.
pub unsafe fn unbox<T: Copy>(o: Obj, type_id: u32) -> T {
    if o.is_null() {
        null_reference();
    }
    let have = type_id_of(o.0 as *const u8);
    if have != type_id && !unbox_compatible(have, type_id) {
        invalid_cast();
    }
    *(o.0 as *const T)
}

/// The CLR unboxes an enum as its underlying integer type and the other way round (and one enum as another of
/// the same underlying type): `(int)(object)Color.Red`, `(Color)(object)1`.
#[cold]
fn unbox_compatible(have: u32, want: u32) -> bool {
    let norm = |id: u32| super::object::enum_info(id).map(|i| i.underlying).unwrap_or(id);
    (have >= tid::FIRST_GENERATED || want >= tid::FIRST_GENERATED) && norm(have) == norm(want) && norm(have) < tid::FIRST_GENERATED
}

/// `o as T?` / `o is T x` for a boxed value type.
///
/// # Safety
/// As [`unbox`].
pub unsafe fn unbox_or_null<T: Copy>(o: Obj, type_id: u32) -> Nullable<T> {
    if is_boxed(o, type_id) {
        Nullable::some(*(o.0 as *const T))
    } else {
        core::mem::zeroed()
    }
}

/// `T?`. All-zero is `null`.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct Nullable<T: Copy> {
    pub has_value: bool,
    pub value: T,
}
unsafe impl<T: Copy + Trace + 'static> Trace for Nullable<T> {
    const NEEDS_TRACE: bool = T::NEEDS_TRACE;
    const RESCAN: bool = T::RESCAN;
    const BOX: BoxKind = BoxKind::Nullable;
    #[inline(always)]
    unsafe fn trace(&self, m: &mut Marker) {
        self.value.trace(m)
    }
    unsafe fn box_ref(&self) -> Obj {
        if self.has_value {
            self.value.box_ref()
        } else {
            Gc::NULL
        }
    }
    /// A `T?` boxes as null or as a boxed `T`.
    fn to_obj(&self) -> Obj {
        if self.has_value {
            self.value.to_obj()
        } else {
            Gc::NULL
        }
    }
    unsafe fn from_obj(o: Obj) -> Self {
        if o.is_null() {
            core::mem::zeroed()
        } else {
            Nullable::some(T::from_obj(o))
        }
    }
}

impl<T: Copy> Nullable<T> {
    #[inline(always)]
    pub fn some(value: T) -> Self {
        Nullable { has_value: true, value }
    }
    /// `.Value`: throws `InvalidOperationException` when null.
    #[inline(always)]
    pub fn value(self) -> T {
        if !self.has_value {
            throw(exc("System.InvalidOperationException", crate::s!("Nullable object must have a value.")))
        }
        self.value
    }
    /// `.GetValueOrDefault()`: the stored value, which is zero when null.
    #[inline(always)]
    pub fn value_or_default(self) -> T {
        self.value
    }
    #[inline(always)]
    pub fn value_or(self, d: T) -> T {
        if self.has_value {
            self.value
        } else {
            d
        }
    }
    #[inline(always)]
    pub fn is_null(self) -> bool {
        !self.has_value
    }
    /// Lifted unary / conversion: null stays null.
    #[inline(always)]
    pub fn map<U: Copy>(self, f: impl FnOnce(T) -> U) -> Nullable<U> {
        if self.has_value {
            Nullable::some(f(self.value))
        } else {
            // SAFETY: all-zero is the null Nullable.
            unsafe { core::mem::zeroed() }
        }
    }
    /// Lifted binary arithmetic: null if either side is null.
    #[inline(always)]
    pub fn lift2<B: Copy, U: Copy>(self, b: Nullable<B>, f: impl FnOnce(T, B) -> U) -> Nullable<U> {
        if self.has_value && b.has_value {
            Nullable::some(f(self.value, b.value))
        } else {
            // SAFETY: all-zero is the null Nullable.
            unsafe { core::mem::zeroed() }
        }
    }
    /// Lifted ordering comparison (`<`, `>=` ...): false if either side is null.
    #[inline(always)]
    pub fn cmp2<B: Copy>(self, b: Nullable<B>, f: impl FnOnce(T, B) -> bool) -> bool {
        self.has_value && b.has_value && f(self.value, b.value)
    }
}

/// C# lifted equality: two nulls are equal, null never equals a value.
impl<T: Copy + PartialEq> PartialEq for Nullable<T> {
    fn eq(&self, o: &Self) -> bool {
        match (self.has_value, o.has_value) {
            (false, false) => true,
            (true, true) => self.value == o.value,
            _ => false,
        }
    }
}

// ---------------------------------------------------------------------------------------------- delegates

/// Heap part of a delegate: one target (`single`) or an invocation list (`targets`, each of them single).
pub struct DelegateData<F: ?Sized + 'static> {
    single: Option<Box<F>>,
    targets: Vec<Delegate<F>>,
    /// (method, receiver) for delegates made from a method group: C# compares those by value, which is what
    /// makes `evt -= Handler` find the delegate `evt += Handler` added. (0, 0) for lambdas (identity only). The
    /// method is the translator's 64-bit id of the C# method (`u64`: a `usize` would cut it on wasm32).
    ident: (u64, usize),
    /// Reports the handles the closure captured (the translator lists them).
    tracer: Option<Box<dyn Fn(&mut Marker)>>,
}
unsafe impl<F: ?Sized + 'static> Trace for DelegateData<F> {
    // immutable once made (`combine` / `remove` make new objects); captured variables that are reassigned live in
    // heap cells of their own
    const RESCAN: bool = false;
    unsafe fn obj_reflect(this: *const Self) -> super::object::Reflect {
        let d = &*this;
        let targets = if d.single.is_some() { vec![d.ident] } else { d.targets.iter().map(|t| (*t.0).ident).collect() };
        super::object::Reflect::Delegate { targets }
    }
    unsafe fn trace(&self, m: &mut Marker) {
        if let Some(t) = &self.tracer {
            t(m);
        }
        for d in &self.targets {
            d.trace(m);
        }
    }
}

/// Delegate: handle to a boxed closure. `F` is `dyn Fn(A, B) -> R`. All-zero is `null`.
pub struct Delegate<F: ?Sized + 'static>(*mut DelegateData<F>);

impl<F: ?Sized + 'static> Clone for Delegate<F> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<F: ?Sized + 'static> Copy for Delegate<F> {}
/// C# delegate equality: same object, or the same method group on the same receiver.
impl<F: ?Sized + 'static> PartialEq for Delegate<F> {
    fn eq(&self, o: &Self) -> bool {
        if self.0 == o.0 {
            return true;
        }
        if self.0.is_null() || o.0.is_null() {
            return false;
        }
        // SAFETY: non-null delegates point at live DelegateData.
        unsafe {
            let (a, b) = (&*self.0, &*o.0);
            if a.single.is_some() && b.single.is_some() {
                return a.ident != (0, 0) && a.ident == b.ident;
            }
            a.targets.len() == b.targets.len() && !a.targets.is_empty() && a.targets.iter().zip(&b.targets).all(|(x, y)| x == y)
        }
    }
}
unsafe impl<F: ?Sized + 'static> Trace for Delegate<F> {
    const BOX: BoxKind = BoxKind::Handle;
    const RESCAN: bool = false;
    #[inline(always)]
    unsafe fn trace(&self, m: &mut Marker) {
        m.mark_payload(self.0 as *const u8)
    }
}

impl<F: ?Sized + 'static> Delegate<F> {
    pub const NULL: Delegate<F> = Delegate(core::ptr::null_mut());

    fn make(single: Option<Box<F>>, targets: Vec<Delegate<F>>, ident: (u64, usize), tracer: Option<Box<dyn Fn(&mut Marker)>>) -> Self {
        Delegate(alloc_init(DelegateData { single, targets, ident, tracer }).0)
    }
    /// A closure that captured no handles.
    pub fn new(f: Box<F>) -> Self {
        Self::make(Some(f), Vec::new(), (0, 0), None)
    }
    /// A closure with captured handles: `tracer` reports them to the collector.
    pub fn new_traced(f: Box<F>, tracer: Box<dyn Fn(&mut Marker)>) -> Self {
        Self::make(Some(f), Vec::new(), (0, 0), Some(tracer))
    }
    /// A method group `recv.Method` (`recv` = 0 for static methods); `method` is the translator's id of the method.
    pub fn from_method(f: Box<F>, method: u64, recv: usize, tracer: Option<Box<dyn Fn(&mut Marker)>>) -> Self {
        Self::make(Some(f), Vec::new(), (method, recv), tracer)
    }
    #[inline(always)]
    pub fn is_null(self) -> bool {
        self.0.is_null()
    }
    #[inline(always)]
    pub fn addr(self) -> usize {
        self.0 as *const u8 as usize
    }
    /// Handle as `object` / `Delegate` / `MulticastDelegate`.
    pub fn up<U>(self) -> Gc<U> {
        Gc(self.0 as *mut U)
    }
    /// # Safety
    /// `o` must be null or a delegate of exactly this signature.
    pub unsafe fn from_obj<U>(o: Gc<U>) -> Self {
        Delegate(o.0 as *mut DelegateData<F>)
    }

    fn list(self) -> Vec<Delegate<F>> {
        if self.0.is_null() {
            return Vec::new();
        }
        // SAFETY: live DelegateData.
        unsafe {
            if (*self.0).single.is_some() {
                vec![self]
            } else {
                (*self.0).targets.clone()
            }
        }
    }

    /// Invocation: `call` is applied to each target in order; the result of the last one is returned.
    /// A null delegate throws `NullReferenceException`.
    #[inline]
    pub fn invoke<R>(self, call: impl Fn(&F) -> R) -> R {
        if self.0.is_null() {
            null_reference();
        }
        // SAFETY: non-null delegates point at live DelegateData; delegates are immutable after creation and the
        // collector does not run while generated code is on the stack.
        unsafe {
            if let Some(f) = &(*self.0).single {
                return call(f);
            }
            let targets = &(*self.0).targets;
            let mut i = 0;
            loop {
                let t = targets[i];
                let f = (*t.0).single.as_deref().expect("flat invocation list");
                if i + 1 == targets.len() {
                    return call(f);
                }
                call(f);
                i += 1;
            }
        }
    }

    /// `Delegate.Combine(a, b)` (`a + b`, `evt += h`).
    pub fn combine(a: Self, b: Self) -> Self {
        if a.is_null() {
            return b;
        }
        if b.is_null() {
            return a;
        }
        let mut l = a.list();
        l.extend(b.list());
        Self::make(None, l, (0, 0), None)
    }

    /// `Delegate.Remove(a, b)` (`a - b`, `evt -= h`): removes the last occurrence of `b`'s invocation list from `a`'s.
    pub fn remove(a: Self, b: Self) -> Self {
        if a.is_null() || b.is_null() {
            return a;
        }
        let (la, lb) = (a.list(), b.list());
        if lb.len() > la.len() {
            return a;
        }
        let mut start = la.len() - lb.len();
        loop {
            if la[start..start + lb.len()].iter().zip(&lb).all(|(x, y)| x == y) {
                let mut rest: Vec<Delegate<F>> = la[..start].to_vec();
                rest.extend_from_slice(&la[start + lb.len()..]);
                return match rest.len() {
                    0 => Delegate::NULL,
                    1 => rest[0],
                    _ => Self::make(None, rest, (0, 0), None),
                };
            }
            if start == 0 {
                return a;
            }
            start -= 1;
        }
    }
}

/// A captured local that the lambda or the method reassigns: both sides share one heap cell (what the C#
/// compiler's closure class does). `*cell.ptr()` is the variable.
pub fn new_cell<T: Trace + Copy + 'static>(v: T) -> Gc<T> {
    alloc_init(v)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rt::{try_, Arr, List, Str};

    #[repr(C)]
    #[derive(Clone, Copy)]
    struct Base {
        a: i32,
    }
    unsafe impl Trace for Base {
        unsafe fn trace(&self, _m: &mut Marker) {}
    }
    #[repr(C)]
    #[derive(Clone, Copy)]
    struct Derived {
        base: Base,
        b: i64,
    }
    unsafe impl Trace for Derived {
        unsafe fn trace(&self, _m: &mut Marker) {}
    }

    /// A node of an arbitrary graph: two handles, an array of handles, a string, a list.
    #[repr(C)]
    #[derive(Clone, Copy)]
    struct Node {
        id: u32,
        left: Gc<Node>,
        right: Gc<Node>,
        kids: Arr<Gc<Node>>,
        name: Str,
        list: Gc<List<Gc<Node>>>,
    }
    unsafe impl Trace for Node {
        // the tests store into nodes through `wbv`, as translated code does
        const RESCAN: bool = false;
        unsafe fn trace(&self, m: &mut Marker) {
            self.left.trace(m);
            self.right.trace(m);
            self.kids.trace(m);
            self.name.trace(m);
            self.list.trace(m);
        }
    }

    #[test]
    fn alloc_is_zeroed_and_typed() {
        let g = alloc::<Derived>(2);
        unsafe {
            assert_eq!((*g.ptr()).base.a, 0);
            assert_eq!((*g.ptr()).b, 0);
            assert_eq!(type_id_of(g.0 as *const u8), 2);
        }
    }

    #[test]
    fn memberwise_clone_copies_runtime_type() {
        let g = alloc::<Derived>(2);
        unsafe {
            (*g.ptr()).base.a = 5;
            (*g.ptr()).b = 6;
            // cloned through a base-typed pointer: the whole derived object is copied
            let c = memberwise_clone(g.up::<Base>().0 as *const u8);
            assert!(c.0 as usize != g.0 as usize);
            assert_eq!(type_id_of(c.0 as *const u8), 2);
            let d: Gc<Derived> = c.up();
            assert_eq!(((*d.ptr()).base.a, (*d.ptr()).b), (5, 6));
            (*d.ptr()).b = 7;
            assert_eq!((*g.ptr()).b, 6);
        }
    }

    #[test]
    fn aliasing_and_upcast() {
        let g = alloc::<Derived>(2);
        let alias = g;
        let b: Gc<Base> = g.up();
        unsafe {
            (*alias.ptr()).base.a = 7;
            assert_eq!((*b.ptr()).a, 7);
        }
        assert!(g == alias);
    }

    #[test]
    fn casts_follow_parent_table() {
        let parents = [0u32, 0, 1];
        let d = alloc::<Derived>(2);
        let b = alloc::<Base>(1);
        assert!(is_instance(d, 1, &parents));
        assert!(!is_instance(b, 2, &parents));
        assert!(as_class::<_, Derived>(b, 2, &parents).is_null());
        assert!(!cast_class::<_, Base>(d, 1, &parents).is_null());
        assert!(cast_class::<_, Derived>(Gc::<Base>::NULL, 2, &parents).is_null());
        let r = try_(|| cast_class::<_, Derived>(b, 2, &parents));
        assert!(exc_kind(r) == "System.InvalidCastException");
    }

    fn exc_kind<R>(r: Result<R, Gc<crate::rt::Exception>>) -> &'static str {
        match r {
            Ok(_) => "no exception",
            Err(e) => unsafe { (*e.ptr()).kind },
        }
    }

    #[test]
    fn null_dereference_is_an_exception() {
        let r = try_(|| unsafe { (*Gc::<Base>::NULL.ptr()).a });
        assert_eq!(exc_kind(r), "System.NullReferenceException");
    }

    #[test]
    fn nullable_semantics() {
        let n: Nullable<i32> = unsafe { crate::rt::zero() };
        assert!(n == unsafe { crate::rt::zero() });
        assert!(n != Nullable::some(0));
        assert_eq!(n.value_or_default(), 0);
        assert_eq!(Nullable::some(3).value(), 3);
        assert_eq!(exc_kind(try_(|| n.value())), "System.InvalidOperationException");
        // lifted operators
        assert!(Nullable::some(6).lift2(Nullable::some(3), |a, b| a / b) == Nullable::some(2));
        assert!(Nullable::some(6).lift2(n, |a, b| a / b).is_null());
        assert!(!n.cmp2(Nullable::some(1), |a, b| a < b));
        assert!(Nullable::some(0).cmp2(Nullable::some(1), |a, b| a < b));
        assert!(n.map(|x| x as f32).is_null());
    }

    #[test]
    fn boxing_copies_and_checks_the_type() {
        let o = box_value(5i32, tid::I32);
        unsafe {
            assert_eq!(unbox::<i32>(o, tid::I32), 5);
            assert!(is_boxed(o, tid::I32) && !is_boxed(o, tid::F32) && !is_boxed(Gc::NULL, tid::I32));
            assert_eq!(exc_kind(try_(|| unbox::<f32>(o, tid::F32))), "System.InvalidCastException");
            assert_eq!(exc_kind(try_(|| unbox::<i32>(Gc::NULL, tid::I32))), "System.NullReferenceException");
            assert!(unbox_or_null::<i32>(o, tid::I32) == Nullable::some(5));
            assert!(unbox_or_null::<f32>(o, tid::F32).is_null());
        }
        // two boxes of the same value are different objects
        assert!(box_value(5i32, tid::I32) != o);
    }

    #[test]
    fn delegate_calls_closure() {
        let k = 5;
        let d = Delegate::<dyn Fn(i32) -> i32>::new(Box::new(move |x| x + k));
        assert_eq!(d.invoke(|f| f(1)), 6);
        let n: Delegate<dyn Fn()> = unsafe { crate::rt::zero() };
        assert!(n.is_null());
        assert_eq!(exc_kind(try_(|| n.invoke(|f| f()))), "System.NullReferenceException");
    }

    #[test]
    fn multicast_combine_and_remove() {
        use std::cell::RefCell;
        use std::rc::Rc;
        type D = Delegate<dyn Fn(i32) -> i32>;
        let log = Rc::new(RefCell::new(Vec::new()));
        let mk = |tag: i32, method: u64| -> D {
            let log = log.clone();
            Delegate::from_method(Box::new(move |x| { log.borrow_mut().push(tag * 100 + x); tag }), method, 0, None)
        };
        let (a, b, c) = (mk(1, 11), mk(2, 22), mk(3, 33));
        let abc = D::combine(D::combine(a, b), c);
        // every target runs in order; the value of the last one is the result
        assert_eq!(abc.invoke(|f| f(7)), 3);
        assert_eq!(*log.borrow(), vec![107, 207, 307]);
        // `-=` with a NEW delegate for the same method group removes the entry (value equality)
        let b2 = mk(2, 22);
        assert!(b2 == b && b2.addr() != b.addr());
        let ac = D::remove(abc, b2);
        log.borrow_mut().clear();
        assert_eq!(ac.invoke(|f| f(1)), 3);
        assert_eq!(*log.borrow(), vec![101, 301]);
        // the last occurrence goes first: a b a - a = a b
        let aba = D::combine(D::combine(a, b), a);
        log.borrow_mut().clear();
        D::remove(aba, a).invoke(|f| f(0));
        assert_eq!(*log.borrow(), vec![100, 200]);
        // removing everything gives null; removing something absent returns the same delegate
        assert!(D::remove(a, mk(1, 11)).is_null());
        assert!(D::remove(abc, mk(9, 99)).addr() == abc.addr());
        assert!(D::combine(D::NULL, a).addr() == a.addr() && D::combine(a, D::NULL).addr() == a.addr());
        // lambdas (no method identity) are only equal to themselves
        let l1: D = Delegate::new(Box::new(|x| x));
        let l2: D = Delegate::new(Box::new(|x| x));
        assert!(l1 != l2 && l1 == l1);
    }

    fn node(id: u32) -> Gc<Node> {
        let n = alloc::<Node>(0);
        unsafe { (*n.0).id = id };
        n
    }

    #[test]
    fn collect_frees_only_unreachable_objects() {
        unsafe {
            let (n0, _) = heap_size();
            // root -> a -> b -> a (cycle), plus garbage cycle c <-> d
            let a = node(1);
            let b = node(2);
            (*a.0).left = b;
            (*b.0).right = a;
            (*a.0).kids = Arr::from_slice(&[b, Gc::NULL, a]);
            (*b.0).name = Str::new("kept");
            (*b.0).list = List::new_gc();
            (*(*b.0).list.0).add(node(3));
            let c = node(100);
            let d = node(101);
            (*c.0).left = d;
            (*d.0).left = c;
            (*d.0).name = Str::new("garbage");
            let root = Rooted::new(a);
            let s = collect();
            // freed: c, d, the string of d
            assert_eq!(s.freed_objects, 3);
            assert_eq!(heap_size().0, n0 + 6); // a, b, kids array, name, list, node 3
            assert_eq!((*(*(*root.get().0).left.0).right.0).id, 1);
            assert_eq!((*(*a.0).left.0).name.as_str(), "kept");
            assert_eq!((*(*(*b.0).list.0).get_item(0).0).id, 3);
            // a second collection frees nothing
            assert_eq!(collect().freed_objects, 0);
            drop(root);
            let s = collect();
            assert_eq!(s.freed_objects, 6);
            assert_eq!(heap_size().0, n0);
        }
    }

    thread_local! {
        /// The "suspended stack" of `conservative_words_keep_objects_by_any_inner_address`.
        static FAKE_STACK: RefCell<Vec<usize>> = const { RefCell::new(Vec::new()) };
    }
    fn fake_stacks(f: &mut dyn FnMut(*const u8, usize)) {
        FAKE_STACK.with(|s| {
            let s = s.borrow();
            f(s.as_ptr() as *const u8, s.len() * core::mem::size_of::<usize>())
        })
    }

    /// The resolution of conservative words keeps an object alive by its start, by an address inside a large
    /// payload far from the start (a string of 20,000 bytes), by an address deep inside an owned buffer (element
    /// 90,000 of an array, item 1,500 of a list) and by an element of a small array; the wide objects that no word
    /// is near are found through `Heap::wide`. Negative control: the same objects without the words are freed.
    #[test]
    fn conservative_words_keep_objects_by_any_inner_address() {
        unsafe {
            set_conservative_roots(Some(fake_stacks));
            let (n0, _) = heap_size();
            let node_a = node(7);
            let big = Arr::<i32>::new(100_000);
            *big.at(90_000) = 99;
            let small = Arr::<i32>::new(16);
            *small.at(3) = 5;
            let text = "x".repeat(20_000);
            let s = Str::new(&text);
            let list = List::<i32>::new_gc();
            for k in 0..2000 {
                (*list.0).add(k);
            }
            let words = vec![
                node_a.0 as usize,
                big.at(90_000) as usize,
                small.at(3) as usize,
                s.as_str().as_ptr() as usize + 15_000,
                (*list.0).item_ptr(1500) as usize,
            ];
            // the deep words are many pages away from their object's start: only the wide list finds them
            assert!(words[1] - big.at(0) as usize > 8192 && words[3] - s.as_str().as_ptr() as usize > 8192);
            FAKE_STACK.with(|f| *f.borrow_mut() = words);
            let st = collect();
            assert_eq!(st.freed_objects, 0, "every object a word refers into survives");
            assert_eq!(heap_size().0, n0 + 5);
            assert_eq!((*node_a.0).id, 7);
            assert_eq!(*big.at(90_000), 99);
            assert_eq!(*small.at(3), 5);
            assert_eq!(s.as_str().len(), 20_000);
            assert_eq!(*(*list.0).item_ptr(1500), 1500);
            // a second, incremental cycle in small slices: the same words resolve the same way
            gc_begin();
            while gc_step(GcBudget::Work(64), true).finished.is_none() {}
            assert_eq!(heap_size().0, n0 + 5);
            // the control: no words, everything is garbage
            FAKE_STACK.with(|f| f.borrow_mut().clear());
            let st = collect();
            assert_eq!(st.freed_objects, 5);
            assert_eq!(heap_size().0, n0);
            // the freed arrays' entries are gone with them: new arrays (perhaps at the same addresses) resolve by
            // their own buffers, in a cycle with a resolution before and one after the cache is filled
            for round in 0..3 {
                let a = Arr::<i64>::new(50_000 + round);
                *a.at(40_000) = round as i64;
                FAKE_STACK.with(|f| *f.borrow_mut() = vec![a.at(40_000) as usize]);
                collect();
                gc_begin();
                while gc_step(GcBudget::Work(64), true).finished.is_none() {}
                assert_eq!(heap_size().0, n0 + 1, "round {round}");
                assert_eq!(*a.at(40_000), round as i64);
                FAKE_STACK.with(|f| f.borrow_mut().clear());
                assert_eq!(collect().freed_objects, 1);
            }
            set_conservative_roots(None);
        }
    }

    /// A word resolved once in a cycle is not looked at again for payloads and fixed buffers, but a buffer that
    /// moves can come under it later: two lists that only the word keeps, the word into the first one's items;
    /// after the cycle's first resolution the lists swap their contents (as a list whose buffer is reallocated), so
    /// the old word now points into the SECOND list, which nothing has marked. The end of the mark phase must find it.
    #[test]
    fn an_old_word_under_a_moved_list_buffer_keeps_the_list() {
        unsafe {
            set_conservative_roots(Some(fake_stacks));
            let (n0, _) = heap_size();
            let l1 = List::<i32>::new_gc();
            let l2 = List::<i32>::new_gc();
            for k in 0..500 {
                (*l1.0).add(k);
                (*l2.0).add(1000 + k);
            }
            let w = (*l1.0).item_ptr(250) as usize;
            FAKE_STACK.with(|f| *f.borrow_mut() = vec![w]);
            gc_begin();
            // the first slices resolve `w` (into l1) and drain
            for _ in 0..1000 {
                gc_step(GcBudget::Work(64), false);
            }
            core::ptr::swap(l1.0, l2.0);
            // the end of the mark phase: `w` is an old word now, and it points into l2's items
            while gc_step(GcBudget::Work(64), true).finished.is_none() {}
            assert_eq!(heap_size().0, n0 + 2, "both lists survive");
            assert_eq!(*(w as *const i32), 250);
            assert_eq!(*(*l2.0).item_ptr(250), 250);
            FAKE_STACK.with(|f| f.borrow_mut().clear());
            assert_eq!(collect().freed_objects, 2);
            set_conservative_roots(None);
        }
    }

    /// A resolution spread over several mark slices (more objects than one piece of `resolve_candidates`), with the
    /// program allocating between the slices: what the words point into survives, the garbage does not, and the
    /// resolution really was spread (seen pending between two slices).
    #[test]
    fn a_resolution_over_several_slices_keeps_what_the_words_hold() {
        unsafe {
            set_conservative_roots(Some(fake_stacks));
            let (n0, _) = heap_size();
            for k in 0..50_000 {
                node(k);
            }
            let keep = node(7);
            let arr = Arr::<i32>::new(20_000);
            *arr.at(15_000) = 3;
            let list = List::<i32>::new_gc();
            for k in 0..300 {
                (*list.0).add(k);
            }
            FAKE_STACK.with(|f| *f.borrow_mut() = vec![keep.0 as usize, arr.at(15_000) as usize, (*list.0).item_ptr(200) as usize]);
            gc_begin();
            let mut spread = false;
            let mut made = 0;
            loop {
                let st = gc_step(GcBudget::Work(256), true);
                if st.finished.is_some() {
                    break;
                }
                spread |= HEAP.with(|h| h.borrow().marker.pending.is_some());
                // the program runs between slices: new objects (born marked during the mark phase)
                node(1_000_000 + made);
                made += 1;
            }
            assert!(spread, "the resolution was finished inside one slice");
            assert_eq!(heap_size().0, n0 + 3 + made as usize, "the three held objects and the ones made during the cycle survive");
            assert_eq!((*keep.0).id, 7);
            assert_eq!(*arr.at(15_000), 3);
            assert_eq!(*(*list.0).item_ptr(200), 200);
            FAKE_STACK.with(|f| f.borrow_mut().clear());
            collect();
            assert_eq!(heap_size().0, n0);
            set_conservative_roots(None);
        }
    }

    #[test]
    fn delegates_keep_their_captures_alive() {
        unsafe {
            let a = node(7);
            let d: Delegate<dyn Fn() -> u32> = Delegate::new_traced(Box::new(move || (*a.0).id), Box::new(move |m| m.mark(a)));
            let lone = node(8);
            let untraced: Delegate<dyn Fn() -> u32> = Delegate::new(Box::new(|| 1));
            let both = Delegate::combine(d, untraced);
            let root = Rooted::new(both);
            let s = collect();
            assert_eq!(s.freed_objects, 1, "only `lone` is unreachable");
            let _ = lone;
            assert_eq!(root.get().invoke(|f| f()), 1);
            assert_eq!(d.invoke(|f| f()), 7);
            drop(root);
            assert_eq!(collect().freed_objects, 4); // a, d, untraced, both
        }
    }

    #[test]
    fn statics_and_dynamic_roots() {
        thread_local! { static SLOT: std::cell::Cell<usize> = const { std::cell::Cell::new(0) }; }
        unsafe fn trace_static(m: &mut Marker) {
            m.mark_payload(SLOT.with(|s| s.get()) as *const u8)
        }
        unsafe {
            let a = node(1);
            SLOT.with(|s| s.set(a.0 as usize));
            register_statics(trace_static);
            let b = node(2);
            let id = add_root(move |m| m.mark(b));
            assert_eq!(collect().freed_objects, 0);
            remove_root(id);
            assert_eq!(collect().freed_objects, 1);
            SLOT.with(|s| s.set(0));
            assert_eq!(collect().freed_objects, 1);
        }
    }

    #[test]
    fn poison_mode_makes_stale_handles_visible() {
        unsafe {
            set_poison(true);
            let a = node(42);
            (*a.0).left = node(43);
            let stale = a; // a handle the host forgot to root
            let keep = Rooted::new(node(1));
            let s = collect();
            assert_eq!(s.freed_objects, 2);
            // the memory was not handed back: it reads as poison, never as another object's data
            assert_eq!((*stale.0).id, 0xDDDD_DDDD);
            assert_eq!((*stale.0).left.0 as usize, usize::from_ne_bytes([0xDD; core::mem::size_of::<usize>()]));
            // new objects never reuse a quarantined address
            for _ in 0..1000 {
                assert!(node(0).0 != stale.0);
            }
            // a root that still points at the dead object is caught by the next collection
            let bad = Rooted::new(stale);
            let r = std::panic::catch_unwind(|| collect());
            assert!(r.is_err(), "marking a freed object must panic");
            drop(bad);
            drop(keep);
        }
    }

    #[cfg(feature = "gc-check")]
    #[test]
    fn gc_check_catches_use_after_free_on_access() {
        unsafe {
            set_poison(true);
            let a = node(1);
            collect();
            let r = std::panic::catch_unwind(|| (*a.ptr()).id);
            assert!(r.is_err());
        }
    }

    /// Random graph mutation against a model: after each collection exactly the objects reachable from the
    /// roots in the model survive, and their contents are intact.
    #[test]
    fn stress_random_graphs() {
        struct Rng(u64);
        impl Rng {
            fn next(&mut self, n: usize) -> usize {
                self.0 ^= self.0 << 13;
                self.0 ^= self.0 >> 7;
                self.0 ^= self.0 << 17;
                (self.0 % n as u64) as usize
            }
        }
        unsafe {
            set_poison(true);
            let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
            const ROOTS: usize = 8;
            let mut roots: Vec<Rooted<Gc<Node>>> = (0..ROOTS).map(|_| Rooted::new(Gc::NULL)).collect();
            // model: id -> (left, right, kids) by id; 0 = null
            let mut model: std::collections::HashMap<u32, (u32, u32, Vec<u32>)> = Default::default();
            let mut handle: std::collections::HashMap<u32, Gc<Node>> = Default::default();
            let mut root_ids = [0u32; ROOTS];
            let mut next_id = 1u32;
            let (base_objects, _) = heap_size();
            for round in 0..60 {
                for _ in 0..400 {
                    let live: Vec<u32> = handle.keys().copied().collect();
                    let pick = |rng: &mut Rng| if live.is_empty() || rng.next(5) == 0 { 0 } else { live[rng.next(live.len())] };
                    match rng.next(6) {
                        0 | 1 => {
                            let id = next_id;
                            next_id += 1;
                            let n = node(id);
                            (*n.0).name = Str::new(&format!("n{id}"));
                            handle.insert(id, n);
                            model.insert(id, (0, 0, Vec::new()));
                            let r = rng.next(ROOTS);
                            // hang the old root under the new node so chains form
                            (*n.0).left = roots[r].get();
                            model.get_mut(&id).unwrap().0 = root_ids[r];
                            roots[r].set(n);
                            root_ids[r] = id;
                        }
                        2 => {
                            let (a, b) = (pick(&mut rng), pick(&mut rng));
                            if a != 0 {
                                (*handle[&a].0).left = if b == 0 { Gc::NULL } else { handle[&b] };
                                model.get_mut(&a).unwrap().0 = b;
                            }
                        }
                        3 => {
                            let (a, b) = (pick(&mut rng), pick(&mut rng));
                            if a != 0 {
                                (*handle[&a].0).right = if b == 0 { Gc::NULL } else { handle[&b] };
                                model.get_mut(&a).unwrap().1 = b;
                            }
                        }
                        4 => {
                            let a = pick(&mut rng);
                            if a != 0 {
                                let ks: Vec<u32> = (0..rng.next(5)).map(|_| pick(&mut rng)).collect();
                                let hs: Vec<Gc<Node>> = ks.iter().map(|k| if *k == 0 { Gc::NULL } else { handle[k] }).collect();
                                (*handle[&a].0).kids = Arr::from_slice(&hs);
                                model.get_mut(&a).unwrap().2 = ks;
                            }
                        }
                        _ => {
                            let r = rng.next(ROOTS);
                            let b = pick(&mut rng);
                            roots[r].set(if b == 0 { Gc::NULL } else { handle[&b] });
                            root_ids[r] = b;
                        }
                    }
                }
                // reachability in the model
                let mut seen = std::collections::HashSet::new();
                let mut work: Vec<u32> = root_ids.iter().copied().filter(|x| *x != 0).collect();
                while let Some(id) = work.pop() {
                    if !seen.insert(id) {
                        continue;
                    }
                    let (l, r, ks) = &model[&id];
                    work.extend([*l, *r].into_iter().chain(ks.iter().copied()).filter(|x| *x != 0));
                }
                let stats = collect();
                handle.retain(|id, _| seen.contains(id));
                model.retain(|id, _| seen.contains(id));
                // every survivor is intact and points where the model says
                let mut arrays = 0;
                for (id, h) in &handle {
                    let n = &*h.ptr();
                    assert_eq!(n.id, *id, "round {round}");
                    assert_eq!(n.name.as_str(), format!("n{id}"));
                    let (l, r, ks) = &model[id];
                    assert_eq!(if n.left.is_null() { 0 } else { (*n.left.ptr()).id }, *l);
                    assert_eq!(if n.right.is_null() { 0 } else { (*n.right.ptr()).id }, *r);
                    if !n.kids.is_null() {
                        arrays += 1;
                        let got: Vec<u32> = n.kids.as_slice().iter().map(|k| if k.is_null() { 0 } else { (*k.ptr()).id }).collect();
                        assert_eq!(&got, ks);
                    } else {
                        assert!(ks.is_empty());
                    }
                }
                // node + its name string per survivor, plus the kids arrays: nothing else is left
                assert_eq!(stats.live_objects - base_objects, handle.len() * 2 + arrays, "round {round}");
            }
            roots.clear();
            collect();
            assert_eq!(heap_size().0, base_objects);
        }
    }

    fn state_of<T>(g: Gc<T>) -> u32 {
        unsafe { (*((g.0 as *const u8).sub(HEADER) as *const Header)).state }
    }

    /// `n` nodes linked through `left`, head first.
    fn chain(n: usize) -> Vec<Gc<Node>> {
        let v: Vec<Gc<Node>> = (0..n).map(|i| node(i as u32)).collect();
        for i in 0..n - 1 {
            unsafe { (*{ v[i] }.0).left = v[i + 1] };
        }
        v
    }

    unsafe fn finish_cycle(work: usize) -> GcStats {
        loop {
            if let Some(s) = gc_step(GcBudget::Work(work), true).finished {
                return s;
            }
        }
    }

    /// The situation the store barrier exists for: a handle is moved from a part of the heap the collector has not
    /// traced yet into an object it has already traced, and the old reference is cut.
    unsafe fn move_tail_into_traced_head(with_barrier: bool) -> (Vec<Gc<Node>>, Rooted<Gc<Node>>) {
        let nodes = chain(200);
        let root = Rooted::new(nodes[0]);
        let w = nodes[199];
        gc_begin();
        let st = gc_step(GcBudget::Work(32), false);
        assert_eq!((st.before, st.after, st.mark_ended), (GcPhase::Mark, GcPhase::Mark, false));
        assert_eq!(state_of(nodes[0]), MARKED);
        assert_eq!(state_of(w), LIVE);
        (*{ nodes[0] }.0).right = if with_barrier { wbv(w) } else { w };
        (*{ nodes[198] }.0).left = Gc::NULL;
        (nodes, root)
    }

    #[test]
    fn incremental_cycle_with_store_barrier() {
        unsafe {
            set_poison(true);
            set_verify(true);
            let (n0, _) = heap_size();
            let (nodes, root) = move_tail_into_traced_head(true);
            // an object made during the mark phase is filled without a barrier; it takes over the middle of the chain
            let fresh = node(1000);
            (*fresh.0).left = nodes[150];
            (*{ nodes[149] }.0).left = Gc::NULL;
            (*{ nodes[0] }.0).kids = wbv(Arr::from_slice(&[fresh]));
            // garbage made during the mark phase survives this cycle (it is born marked) and goes in the next
            let floating = node(2000);
            let s = finish_cycle(16);
            assert_eq!(gc_phase(), GcPhase::Idle);
            assert_eq!(s.freed_objects, 0);
            for (i, n) in nodes.iter().enumerate() {
                assert_eq!((*n.ptr()).id, i as u32);
                assert_eq!(state_of(*n), LIVE);
            }
            assert_eq!((*(*{ nodes[0] }.0).right.ptr()).id, 199);
            assert_eq!((*(*(*(*{ nodes[0] }.0).kids.at(0)).ptr()).left.ptr()).id, 150);
            assert_eq!(state_of(floating), LIVE);
            assert_eq!(collect().freed_objects, 1);
            assert_eq!(state_of(floating), DEAD);
            drop(root);
            collect();
            assert_eq!(heap_size().0, n0);
        }
    }

    /// Negative control of the test above: the same moves without the barrier lose the object.
    #[test]
    fn without_the_store_barrier_the_moved_object_is_freed() {
        unsafe {
            set_poison(true);
            let (nodes, _root) = move_tail_into_traced_head(false);
            let s = finish_cycle(16);
            assert_eq!(s.freed_objects, 1);
            assert_eq!(state_of(nodes[199]), DEAD);
            assert_eq!((*{ nodes[199] }.0).id, 0xDDDD_DDDD);
        }
    }

    /// And the verifier names it.
    #[test]
    fn verifier_reports_a_store_without_barrier() {
        unsafe {
            set_verify(true);
            let (_nodes, _root) = move_tail_into_traced_head(false);
            let r = std::panic::catch_unwind(|| finish_cycle(16));
            let msg = *r.expect_err("the verifier must panic").downcast::<String>().unwrap();
            assert!(msg.contains("missed 1 reachable object") && msg.contains("Node"), "{msg}");
        }
    }

    #[test]
    fn large_arrays_are_traced_in_pieces() {
        unsafe {
            set_poison(true);
            set_verify(true);
            let n = (3 * RANGE_MIN) as i32;
            let arr = Arr::<Gc<Node>>::new(n);
            for i in 0..n {
                *arr.at(i) = node(i as u32);
            }
            let root = Rooted::new(arr);
            gc_begin();
            gc_step(GcBudget::Work(RANGE_CHUNK), false);
            assert_ne!(state_of(*arr.at(0)), LIVE);
            assert_eq!(state_of(*arr.at(n - 1)), LIVE);
            // from the part that is still to be traced into the part that was: through the barrier
            let (first, last) = (*arr.at(0), *arr.at(n - 1));
            *arr.at(0) = wbv(last);
            *arr.at(n - 1) = Gc::NULL;
            let mut steps = 0;
            let s = loop {
                steps += 1;
                if let Some(s) = gc_step(GcBudget::Work(RANGE_CHUNK), true).finished {
                    break s;
                }
            };
            assert!(steps > 10, "the array must take several slices ({steps})");
            // `first` was marked before it was dropped from the array: it goes in the next cycle
            assert_eq!(s.freed_objects, 0);
            assert_eq!((*last.ptr()).id, (n - 1) as u32);
            assert_eq!(collect().freed_objects, 1);
            assert_eq!(state_of(first), DEAD);
            drop(root);
            collect();
        }
    }

    /// `collect()` in the middle of an incremental cycle completes it and then collects exactly.
    #[test]
    fn collect_during_an_incremental_cycle() {
        unsafe {
            set_poison(true);
            let (n0, _) = heap_size();
            let nodes = chain(100);
            let root = Rooted::new(nodes[0]);
            gc_begin();
            gc_step(GcBudget::Work(32), false);
            (*{ nodes[49] }.0).left = Gc::NULL;
            let s = collect();
            assert_eq!((s.live_objects - n0, gc_phase()), (50, GcPhase::Idle));
            // and in the middle of a sweep
            (*{ nodes[24] }.0).left = Gc::NULL;
            gc_begin();
            while gc_phase() == GcPhase::Mark {
                gc_step(GcBudget::Work(8), true);
            }
            assert_eq!(gc_phase(), GcPhase::Sweep);
            let during_sweep = node(7);
            (*{ nodes[0] }.0).right = wbv(during_sweep);
            gc_step(GcBudget::Work(1), true);
            let s = collect();
            assert_eq!(s.live_objects - n0, 26);
            assert_eq!((*(*{ nodes[0] }.0).right.ptr()).id, 7);
            drop(root);
            collect();
            assert_eq!(heap_size().0, n0);
        }
    }

    /// The model test of `stress_random_graphs` with the collector running in small slices between the
    /// mutations, the verifier on: nothing reachable is ever freed, and a full collection at the end of a round
    /// leaves exactly the reachable objects.
    #[test]
    fn stress_random_graphs_incremental() {
        struct Rng(u64);
        impl Rng {
            fn next(&mut self, n: usize) -> usize {
                self.0 ^= self.0 << 13;
                self.0 ^= self.0 >> 7;
                self.0 ^= self.0 << 17;
                (self.0 % n as u64) as usize
            }
        }
        type Model = std::collections::HashMap<u32, (u32, u32, Vec<u32>)>;
        fn reachable(model: &Model, root_ids: &[u32]) -> std::collections::HashSet<u32> {
            let mut seen = std::collections::HashSet::new();
            let mut work: Vec<u32> = root_ids.iter().copied().filter(|x| *x != 0).collect();
            while let Some(id) = work.pop() {
                if !seen.insert(id) {
                    continue;
                }
                let (l, r, ks) = &model[&id];
                work.extend([*l, *r].into_iter().chain(ks.iter().copied()).filter(|x| *x != 0));
            }
            seen
        }
        unsafe {
            set_poison(true);
            set_verify(true);
            let mut rng = Rng(0x2545_F491_4F6C_DD1D);
            const ROOTS: usize = 6;
            let mut roots: Vec<Rooted<Gc<Node>>> = (0..ROOTS).map(|_| Rooted::new(Gc::NULL)).collect();
            let mut model: Model = Default::default();
            let mut handle: std::collections::HashMap<u32, Gc<Node>> = Default::default();
            let mut root_ids = [0u32; ROOTS];
            let mut next_id = 1u32;
            let (base_objects, _) = heap_size();
            let (mut cycles, mut slices) = (0, 0);
            for round in 0..40 {
                for _ in 0..300 {
                    // a program can only use what it can reach
                    let live: Vec<u32> = {
                        let mut v: Vec<u32> = reachable(&model, &root_ids).into_iter().collect();
                        v.sort_unstable();
                        v
                    };
                    let pick = |rng: &mut Rng| if live.is_empty() || rng.next(5) == 0 { 0 } else { live[rng.next(live.len())] };
                    let h = |id: u32| if id == 0 { Gc::NULL } else { handle[&id] };
                    match rng.next(6) {
                        0 | 1 => {
                            let id = next_id;
                            next_id += 1;
                            let n = node(id);
                            (*n.0).name = Str::new(&format!("n{id}"));
                            handle.insert(id, n);
                            model.insert(id, (0, 0, Vec::new()));
                            let r = rng.next(ROOTS);
                            (*n.0).left = roots[r].get();
                            model.get_mut(&id).unwrap().0 = root_ids[r];
                            roots[r].set(n);
                            root_ids[r] = id;
                        }
                        2 => {
                            let (a, b) = (pick(&mut rng), pick(&mut rng));
                            if a != 0 {
                                (*handle[&a].0).left = wbv(h(b));
                                model.get_mut(&a).unwrap().0 = b;
                            }
                        }
                        3 => {
                            let (a, b) = (pick(&mut rng), pick(&mut rng));
                            if a != 0 {
                                (*handle[&a].0).right = wbv(h(b));
                                model.get_mut(&a).unwrap().1 = b;
                            }
                        }
                        4 => {
                            let a = pick(&mut rng);
                            if a != 0 {
                                let ks: Vec<u32> = (0..rng.next(5)).map(|_| pick(&mut rng)).collect();
                                let hs: Vec<Gc<Node>> = ks.iter().map(|k| h(*k)).collect();
                                (*handle[&a].0).kids = wbv(Arr::from_slice(&hs));
                                model.get_mut(&a).unwrap().2 = ks;
                            }
                        }
                        _ => {
                            let r = rng.next(ROOTS);
                            let b = pick(&mut rng);
                            roots[r].set(h(b));
                            root_ids[r] = b;
                        }
                    }
                    // the collector, a few objects at a time
                    if gc_phase() == GcPhase::Idle {
                        if rng.next(3) == 0 {
                            gc_begin();
                            cycles += 1;
                        }
                    } else {
                        slices += 1;
                        gc_step(GcBudget::Work(1 + rng.next(40)), true);
                    }
                    // nothing reachable is dead, whatever the phase
                    if rng.next(8) == 0 {
                        for id in reachable(&model, &root_ids) {
                            assert_ne!(state_of(handle[&id]), DEAD, "round {round}: node {id} is reachable and was freed");
                        }
                    }
                }
                let seen = reachable(&model, &root_ids);
                let stats = collect();
                handle.retain(|id, _| seen.contains(id));
                model.retain(|id, _| seen.contains(id));
                let mut arrays = 0;
                for (id, h) in &handle {
                    let n = &*h.ptr();
                    assert_eq!(n.id, *id, "round {round}");
                    assert_eq!(n.name.as_str(), format!("n{id}"));
                    let (l, r, ks) = &model[id];
                    assert_eq!(if n.left.is_null() { 0 } else { (*n.left.ptr()).id }, *l);
                    assert_eq!(if n.right.is_null() { 0 } else { (*n.right.ptr()).id }, *r);
                    if !n.kids.is_null() {
                        arrays += 1;
                        let got: Vec<u32> = n.kids.as_slice().iter().map(|k| if k.is_null() { 0 } else { (*k.ptr()).id }).collect();
                        assert_eq!(&got, ks);
                    } else {
                        assert!(ks.is_empty());
                    }
                }
                assert_eq!(stats.live_objects - base_objects, handle.len() * 2 + arrays, "round {round}");
            }
            assert!(cycles > 100 && slices > 3000, "{cycles} cycles, {slices} slices");
            roots.clear();
            collect();
            assert_eq!(heap_size().0, base_objects);
        }
    }

    /// Pause times on a heap shaped like the game's with a world loaded: 5.04 million leaf objects behind one
    /// two-dimensional array, 770,000 other objects. `cargo test --release -p terraria-gen-rt --lib bench_pauses
    /// -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn bench_pauses() {
        #[repr(C)]
        #[derive(Clone, Copy)]
        struct Leaf {
            a: [u16; 7],
        }
        unsafe impl Trace for Leaf {
            const NEEDS_TRACE: bool = false;
            const RESCAN: bool = false;
            unsafe fn trace(&self, _m: &mut Marker) {}
        }
        fn pct(v: &mut Vec<f64>, p: f64) -> f64 {
            v.sort_by(|a, b| a.partial_cmp(b).unwrap());
            v[((v.len() - 1) as f64 * p) as usize]
        }
        unsafe {
            let tiles = crate::rt::Arr2::<Gc<Leaf>>::new(4200, 1200);
            let others = Arr::<Gc<Node>>::new(385_000);
            // interleave the allocations, as a world load does
            let mut k = 0;
            for i in 0..4200 {
                for j in 0..1200 {
                    *tiles.at(i, j) = alloc::<Leaf>(0);
                    if (i * 1200 + j) % 13 == 0 && k < 385_000 {
                        let n = node(k as u32);
                        (*n.0).name = Str::new("some name of a thing");
                        if k > 0 {
                            (*n.0).left = *others.at(k - 1);
                        }
                        *others.at(k) = n;
                        k += 1;
                    }
                }
            }
            let _r1 = Rooted::new(tiles);
            let _r2 = Rooted::new(others);
            let (objects, bytes) = heap_size();
            println!("heap: {objects} objects, {:.1} MB", bytes as f64 / 1048576.0);
            let mut full = Vec::new();
            for _ in 0..5 {
                let t = std::time::Instant::now();
                collect();
                full.push(t.elapsed().as_secs_f64() * 1000.0);
            }
            println!("full collection (one pause), ms: {:?}", full.iter().map(|x| (x * 10.0).round() / 10.0).collect::<Vec<_>>());
            for budget in [500u64, 1000, 2000] {
                let (mut begin, mut slice, mut end) = (Vec::new(), Vec::new(), Vec::new());
                let mut tick = 0u32;
                let (mut worst, mut over) = ((0f64, 0f64), 0u32);
                for _cycle in 0..4 {
                    let t = std::time::Instant::now();
                    gc_begin();
                    begin.push(t.elapsed().as_secs_f64() * 1000.0);
                    loop {
                        // one tick of the program: 139 KB of garbage, a few stores
                        for g in 0..1400 {
                            let n = node(g);
                            (*n.0).name = Str::new("garbage garbage garbage garbage garbage garbage");
                            if g % 100 == 0 {
                                (*(*others.at(((tick * 7919 + g) % 385_000) as i32)).0).right = wbv(n);
                            }
                        }
                        tick += 1;
                        let t = std::time::Instant::now();
                        let st = gc_step(GcBudget::Micros(budget), true);
                        let ms = t.elapsed().as_secs_f64() * 1000.0;
                        if st.mark_ended {
                            end.push(ms)
                        } else {
                            slice.push(ms);
                            if st.before == GcPhase::Mark {
                                worst.0 = worst.0.max(ms)
                            } else {
                                worst.1 = worst.1.max(ms)
                            }
                            over += (ms > 4.0) as u32;
                        }
                        if st.finished.is_some() {
                            break;
                        }
                    }
                }
                let n = slice.len();
                let mean = slice.iter().sum::<f64>() / n as f64;
                println!(
                    "budget {budget} us: {} slices per cycle, slice mean {mean:.2} p50 {:.2} p99 {:.2} max {:.2} ms; begin max {:.3} ms; mark end max {:.3} ms; worst mark slice {:.2}, worst sweep slice {:.2}, {over} of {n} slices above 4 ms",
                    n / 4,
                    pct(&mut slice, 0.5),
                    pct(&mut slice, 0.99),
                    pct(&mut slice, 1.0),
                    pct(&mut begin, 1.0),
                    pct(&mut end, 1.0),
                    worst.0,
                    worst.1
                );
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------- native base classes

/// The base-class part of a translated class that derives from a natively implemented class (`class ClusterGroup :
/// List<Cluster>`, `Main : Game`): the native value itself, embedded as the FIRST field, so that a handle to the
/// derived object is also a valid handle to the native class (the same rule as for translated bases). The slot is
/// empty until the base constructor ran; an empty slot is neither traced nor dropped. NATIVE_SCHEMA.md section 3a.
#[repr(C)]
pub struct NativeSlot<T> {
    value: core::mem::MaybeUninit<T>,
    init: bool,
}
impl<T: Trace> NativeSlot<T> {
    /// `: base(args)`: moves the value out of the object the native constructor template returned.
    ///
    /// # Safety
    /// `slot` must point at the slot of a live object; `made` must be a fresh object nothing else refers to.
    pub unsafe fn init(slot: *mut NativeSlot<T>, made: Gc<T>) {
        if made.is_null() {
            null_reference();
        }
        assert!(!(*slot).init, "cs2rs runtime: the base constructor of a native base class ran twice");
        // the temporary object keeps its memory until the next sweep, but no longer owns the value
        let h = &mut *((made.0 as *mut u8).sub(HEADER) as *mut Header);
        (*slot).value.write(core::ptr::read(made.0));
        h.info = &MOVED_OUT;
        (*slot).init = true;
        // the value's handles now sit in another object, which the collector may have traced already
        wb(&*slot);
    }
}
static MOVED_OUT: TypeInfo = TypeInfo {
    size_of: moved_size,
    trace: moved_trace,
    drop: moved_drop,
    needs_trace: false,
    rescan: false,
    name: moved_name,
    plain: false,
    rust_type: core::any::TypeId::of::<()>,
    equals: moved_equals,
    hash: moved_hash,
    to_string: moved_to_string,
    reflect: no_reflect_fn,
    dispose: no_dispose_fn,
    clear: no_clear_fn,
    wide: false,
    buffer: no_buffer_fn,
    has_buffer: false,
    fixed_buffer: false,
};
fn moved_name() -> &'static str {
    "moved-out native base"
}
unsafe fn moved_size(_p: *const u8) -> usize {
    0
}
unsafe fn moved_trace(_p: *const u8, _m: &mut Marker) {}
unsafe fn moved_drop(_p: *mut u8) {}
unsafe fn moved_equals(p: *const u8, other: Obj) -> bool {
    p as usize == other.addr()
}
unsafe fn moved_hash(p: *const u8) -> i32 {
    identity_hash(p as usize)
}
unsafe fn moved_to_string(_p: *const u8) -> Str {
    Str::lit("")
}
impl<T> Drop for NativeSlot<T> {
    fn drop(&mut self) {
        if self.init {
            // SAFETY: `init` says the value was written.
            unsafe { self.value.assume_init_drop() }
        }
    }
}
unsafe impl<T: Trace> Trace for NativeSlot<T> {
    const NEEDS_TRACE: bool = T::NEEDS_TRACE;
    const RESCAN: bool = T::RESCAN;
    unsafe fn trace(&self, m: &mut Marker) {
        if self.init {
            self.value.assume_init_ref().trace(m)
        }
    }
}

thread_local! {
    static OVERRIDES: RefCell<std::collections::HashMap<(u32, &'static str), (core::any::TypeId, usize)>> = RefCell::new(std::collections::HashMap::new());
}

/// A native class whose virtual members translated subclasses may override. `VIRTUALS` lists the members (by
/// documentation id) whose native entry points ask [`native_override`] first; registering an override of any
/// other member panics, so an override the native code would never call cannot go unnoticed.
pub trait NativeVirtuals {
    const VIRTUALS: &'static [&'static str];
}

/// Generated code: objects with type id `type_id` override virtual `slot` of native base `B` with `f`
/// (an `unsafe fn(Obj, args..) -> R`).
pub fn native_register_override<B: NativeVirtuals, F: Copy + 'static>(type_id: u32, slot: &'static str, f: F) {
    assert!(core::mem::size_of::<F>() == core::mem::size_of::<usize>(), "an override is a function pointer");
    assert!(B::VIRTUALS.contains(&slot), "cs2rs runtime: native class {} does not dispatch virtual member {slot}; a translated override would never be called", core::any::type_name::<B>());
    // SAFETY: size checked above.
    let addr: usize = unsafe { core::mem::transmute_copy(&f) };
    OVERRIDES.with(|o| {
        o.borrow_mut().insert((type_id, slot), (core::any::TypeId::of::<F>(), addr));
    })
}

/// Native code: the translated override of virtual `slot` for the object whose base part is `this` (a native
/// method's `self`), or `None` for a plain native object. `F` must be the function type the generator registers
/// (`unsafe fn(Obj, args..) -> R`); another type panics.
///
/// # Safety
/// `this` must be the address of a native object allocated by the runtime (any `&self` of a native class).
pub unsafe fn native_override<F: Copy + 'static>(this: *const u8, slot: &'static str) -> Option<F> {
    let id = type_id_of(this);
    if id < tid::FIRST_GENERATED {
        return None;
    }
    OVERRIDES.with(|o| {
        o.borrow().get(&(id, slot)).map(|(ty, addr)| {
            assert!(*ty == core::any::TypeId::of::<F>(), "cs2rs runtime: override of {slot} has another signature than the native caller expects");
            core::mem::transmute_copy::<usize, F>(addr)
        })
    })
}
