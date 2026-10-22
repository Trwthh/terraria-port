//! `List<T>`, `Dictionary<K, V>`, `IEnumerable<T>` (`Seq<T>`) and the enumerators `foreach` uses.
//!
//! Methods take `&mut self` / `&self`: generated code calls them as `(*list.ptr()).add(x)`, so the reference
//! lives only for the call, and nothing in here calls back into generated code except the LINQ helpers, which
//! take a snapshot first.

use super::arr::Arr;
use super::exc::{exc, throw};
use super::gc::{alloc_init, null_reference, Delegate, Gc, Marker, Trace};
use super::string::Str;
use super::Val;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};

fn arg_out_of_range() -> ! {
    throw(exc("System.ArgumentOutOfRangeException", crate::s!("Index was out of range. Must be non-negative and less than the size of the collection.")))
}
fn modified() -> ! {
    throw(exc("System.InvalidOperationException", crate::s!("Collection was modified; enumeration operation may not execute.")))
}

/// Equality and hashing as `EqualityComparer<T>.Default` of .NET does it (the comparer of `Dictionary`, `HashSet`,
/// `List.Contains` / `IndexOf` / `Remove`, `Distinct`): `IEquatable<T>.Equals(T)` when `T` implements it, else
/// `Equals(object)`; numbers and enums by value, strings by ordinal content, a class without `Equals` by
/// identity, a struct without `Equals` field by field.
pub trait CsKey: Val {
    fn cs_eq(&self, other: &Self) -> bool;
    /// Feeds the hash maps of the runtime. Must agree with `cs_eq`; by convention one `write_i32` of the
    /// type's `GetHashCode()`.
    fn cs_hash<H: Hasher>(&self, h: &mut H);
    /// `GetHashCode()` of the value (`EqualityComparer<T>.Default.GetHashCode(x)`). The provided version returns
    /// what `cs_hash` wrote when that was a single `write_i32`, else a fold of what it wrote (not .NET's number:
    /// override it where the game can observe the value).
    fn cs_hash_code(&self) -> i32 {
        let mut h = NetHash::default();
        self.cs_hash(&mut h);
        h.code()
    }
}

/// Collects what a `cs_hash` writes: see [`CsKey::cs_hash_code`].
#[derive(Default)]
pub struct NetHash {
    single: Option<i32>,
    writes: u32,
    acc: u32,
}
impl NetHash {
    pub fn code(&self) -> i32 {
        match (self.writes, self.single) {
            (1, Some(v)) => v,
            _ => self.acc as i32,
        }
    }
}
impl Hasher for NetHash {
    fn finish(&self) -> u64 {
        self.code() as u32 as u64
    }
    fn write(&mut self, bytes: &[u8]) {
        self.writes += 1;
        self.single = None;
        for b in bytes {
            self.acc = (self.acc ^ *b as u32).wrapping_mul(16777619);
        }
    }
    fn write_i32(&mut self, v: i32) {
        self.writes += 1;
        self.single = if self.writes == 1 { Some(v) } else { None };
        for b in v.to_le_bytes() {
            self.acc = (self.acc ^ b as u32).wrapping_mul(16777619);
        }
    }
}

// `GetHashCode` of the primitives as .NET Framework computes it (Int32.cs, Int16.cs, ... of the reference source).
macro_rules! key_by_value {
    ($($t:ty: $h:expr;)*) => {$( impl CsKey for $t {
        fn cs_eq(&self, o: &Self) -> bool { self == o }
        fn cs_hash<H: Hasher>(&self, h: &mut H) { h.write_i32(self.cs_hash_code()) }
        fn cs_hash_code(&self) -> i32 { ($h)(*self) }
    } )*};
}
key_by_value! {
    bool: |v: bool| v as i32;
    i8: |v: i8| (v as i32) ^ ((v as i32) << 8);
    u8: |v: u8| v as i32;
    i16: |v: i16| (v as u16 as i32) | ((v as i32) << 16);
    // `char` and `ushort` share `u16`: `Char.GetHashCode` is `c | (c << 16)`, `UInt16.GetHashCode` is the value.
    // Deviation: a `ushort` hashes like a `char` here (the value is only observable through `GetHashCode()`).
    u16: |v: u16| (v as i32) | ((v as i32) << 16);
    i32: |v: i32| v;
    u32: |v: u32| v as i32;
    i64: |v: i64| (v as i32) ^ ((v >> 32) as i32);
    u64: |v: u64| (v as i32) ^ ((v >> 32) as i32);
}
/// `IntPtr` (mapped to `isize` by the host tables): `IntPtr.GetHashCode()` is the value truncated to 32 bits.
impl CsKey for isize {
    fn cs_eq(&self, o: &Self) -> bool {
        self == o
    }
    fn cs_hash<H: Hasher>(&self, h: &mut H) {
        h.write_i32(*self as i32)
    }
}
impl CsKey for Str {
    fn cs_eq(&self, o: &Self) -> bool {
        super::string::str_eq(*self, *o)
    }
    fn cs_hash<H: Hasher>(&self, h: &mut H) {
        h.write_i32(self.cs_hash_code())
    }
    /// `String.GetHashCode` of the 32-bit .NET Framework 4 runtime (0 for null, as the default comparer gives).
    fn cs_hash_code(&self) -> i32 {
        match self.get() {
            None => 0,
            Some(s) => super::string::legacy_string_hash(s),
        }
    }
}
/// A class reference: the default comparer calls `IEquatable<T>.Equals(T)` or the virtual `Equals(object)`
/// and the virtual `GetHashCode()`; a class that overrides neither compares by identity (`Trace::key_eq`).
impl<T: Trace + 'static> CsKey for Gc<T> {
    fn cs_eq(&self, o: &Self) -> bool {
        if self.is_null() || o.is_null() {
            return self.is_null() && o.is_null();
        }
        // SAFETY: both handles are non-null objects.
        unsafe { T::key_eq(*self, *o) }
    }
    fn cs_hash<H: Hasher>(&self, h: &mut H) {
        h.write_i32(self.cs_hash_code())
    }
    fn cs_hash_code(&self) -> i32 {
        if self.is_null() {
            0
        } else {
            super::gc::obj_hash_code(self.up())
        }
    }
}
impl CsKey for f32 {
    fn cs_eq(&self, o: &Self) -> bool {
        self == o || (self.is_nan() && o.is_nan())
    }
    fn cs_hash<H: Hasher>(&self, h: &mut H) {
        h.write_i32(self.cs_hash_code())
    }
    /// `Single.GetHashCode`: the bits, 0 for both zeroes.
    fn cs_hash_code(&self) -> i32 {
        if *self == 0.0 {
            0
        } else {
            self.to_bits() as i32
        }
    }
}

// XNA value types: `IEquatable<T>` member-wise equality.
impl CsKey for terraria_core::xna::Point {
    fn cs_eq(&self, o: &Self) -> bool {
        self.x == o.x && self.y == o.y
    }
    fn cs_hash<H: Hasher>(&self, h: &mut H) {
        h.write_i32(self.cs_hash_code())
    }
    /// `Point.GetHashCode`: `X.GetHashCode() + Y.GetHashCode()`.
    fn cs_hash_code(&self) -> i32 {
        self.x.wrapping_add(self.y)
    }
}
impl CsKey for terraria_core::xna::Rectangle {
    fn cs_eq(&self, o: &Self) -> bool {
        self.x == o.x && self.y == o.y && self.width == o.width && self.height == o.height
    }
    fn cs_hash<H: Hasher>(&self, h: &mut H) {
        h.write_i32(self.cs_hash_code())
    }
    /// `Rectangle.GetHashCode`: the sum of the four `int` hashes.
    fn cs_hash_code(&self) -> i32 {
        self.x.wrapping_add(self.y).wrapping_add(self.width).wrapping_add(self.height)
    }
}
impl CsKey for terraria_core::xna::Color {
    fn cs_eq(&self, o: &Self) -> bool {
        self.packed_value() == o.packed_value()
    }
    fn cs_hash<H: Hasher>(&self, h: &mut H) {
        h.write_i32(self.cs_hash_code())
    }
    /// `Color.GetHashCode`: `packedValue.GetHashCode()`.
    fn cs_hash_code(&self) -> i32 {
        self.packed_value() as i32
    }
}

impl CsKey for f64 {
    fn cs_eq(&self, o: &Self) -> bool {
        self == o || (self.is_nan() && o.is_nan())
    }
    fn cs_hash<H: Hasher>(&self, h: &mut H) {
        h.write_i32(self.cs_hash_code())
    }
    /// `Double.GetHashCode`: the two halves of the bits xored, 0 for both zeroes.
    fn cs_hash_code(&self) -> i32 {
        if *self == 0.0 {
            0
        } else {
            let b = self.to_bits();
            (b as i32) ^ ((b >> 32) as i32)
        }
    }
}
macro_rules! key_by_float_fields {
    ($($t:ty: $($f:ident),+;)*) => {$( impl CsKey for $t {
        fn cs_eq(&self, o: &Self) -> bool { $(self.$f.cs_eq(&o.$f))&&+ }
        fn cs_hash<H: Hasher>(&self, h: &mut H) { h.write_i32(self.cs_hash_code()) }
        // VectorN / Matrix `GetHashCode`: the sum of the component hashes
        fn cs_hash_code(&self) -> i32 { 0i32 $(.wrapping_add(self.$f.cs_hash_code()))+ }
    } )*};
}
key_by_float_fields! {
    terraria_core::xna::Vector2: x, y;
    terraria_core::xna::Vector3: x, y, z;
    terraria_core::xna::Vector4: x, y, z, w;
    terraria_core::xna::Matrix: m11, m12, m13, m14, m21, m22, m23, m24, m31, m32, m33, m34, m41, m42, m43, m44;
}
// Handles that are not `Gc<T>`: identity, as for any reference.
macro_rules! key_by_identity {
    ($($t:ident<$($p:ident),+>),*) => {$( impl<$($p: 'static),+> CsKey for $t<$($p),+> {
        fn cs_eq(&self, o: &Self) -> bool { self.addr() == o.addr() }
        fn cs_hash<H: Hasher>(&self, h: &mut H) { self.addr().hash(h) }
    } )*};
}
key_by_identity!(Arr<T>, Seq<T>);
impl<T: 'static> CsKey for super::Arr2<T> {
    fn cs_eq(&self, o: &Self) -> bool {
        self.addr() == o.addr()
    }
    fn cs_hash<H: Hasher>(&self, h: &mut H) {
        self.addr().hash(h)
    }
}
impl<T: 'static> CsKey for super::Arr3<T> {
    fn cs_eq(&self, o: &Self) -> bool {
        self.addr() == o.addr()
    }
    fn cs_hash<H: Hasher>(&self, h: &mut H) {
        self.addr().hash(h)
    }
}
impl<F: ?Sized + 'static> CsKey for Delegate<F> {
    fn cs_eq(&self, o: &Self) -> bool {
        self == o
    }
    fn cs_hash<H: Hasher>(&self, h: &mut H) {
        // equal method-group delegates are different objects: hash nothing that distinguishes them
        h.write_u8(0)
    }
}
impl<T: CsKey> CsKey for super::Nullable<T> {
    fn cs_eq(&self, o: &Self) -> bool {
        self.has_value == o.has_value && (!self.has_value || self.value.cs_eq(&o.value))
    }
    fn cs_hash<H: Hasher>(&self, h: &mut H) {
        h.write_i32(self.cs_hash_code())
    }
    /// `Nullable<T>.GetHashCode`: the value's, 0 for null.
    fn cs_hash_code(&self) -> i32 {
        if self.has_value {
            self.value.cs_hash_code()
        } else {
            0
        }
    }
}
impl<K: CsKey, V: CsKey> CsKey for KeyValuePair<K, V> {
    fn cs_eq(&self, o: &Self) -> bool {
        self.key.cs_eq(&o.key) && self.value.cs_eq(&o.value)
    }
    fn cs_hash<H: Hasher>(&self, h: &mut H) {
        self.key.cs_hash(h);
        self.value.cs_hash(h);
    }
}

#[derive(Clone, Copy)]
struct KeyW<K>(K);
impl<K: CsKey> PartialEq for KeyW<K> {
    fn eq(&self, o: &Self) -> bool {
        self.0.cs_eq(&o.0)
    }
}
impl<K: CsKey> Eq for KeyW<K> {}
impl<K: CsKey> Hash for KeyW<K> {
    fn hash<H: Hasher>(&self, h: &mut H) {
        self.0.cs_hash(h)
    }
}

// ---------------------------------------------------------------------------------------------- List<T>

/// `System.Collections.Generic.List<T>`. Always behind a `Gc` (a zeroed `Vec` is not valid).
pub struct List<T> {
    items: Vec<T>,
    version: u32,
    /// `Capacity` as .NET Framework's `List<T>` would have it (the size of its `_items` array): 0 for `new List<T>()`,
    /// the argument of `new List<T>(n)`, then 4 and doubling when an `Add` / `Insert` finds it full
    /// (`EnsureCapacity`), the element count for a list made from a collection. Observable through `Capacity`
    /// (ReLogic's `ChromaShader` loops over it).
    cap: usize,
}

/// `List<T>.EnsureCapacity(min)`: `_items.Length == 0 ? 4 : _items.Length * 2`, at least `min`.
fn grown_capacity(cap: usize, min: usize) -> usize {
    if min <= cap {
        return cap;
    }
    let n = if cap == 0 { 4 } else { cap * 2 };
    n.max(min)
}

impl<T: Val> List<T> {
    /// `new List<T>()`.
    pub fn new_gc() -> Gc<List<T>> {
        alloc_init(List { items: Vec::new(), version: 0, cap: 0 })
    }
    /// `new List<T>(capacity)`.
    pub fn with_capacity_gc(capacity: i32) -> Gc<List<T>> {
        if capacity < 0 {
            arg_out_of_range();
        }
        alloc_init(List { items: Vec::with_capacity(capacity as usize), version: 0, cap: capacity as usize })
    }
    /// `Capacity` (get)
    pub fn capacity(&self) -> i32 {
        self.cap as i32
    }
    /// `Capacity` (set): `ArgumentOutOfRangeException` below `Count`.
    pub fn set_capacity(&mut self, value: i32) {
        if value < 0 || (value as usize) < self.items.len() {
            arg_out_of_range();
        }
        self.cap = value as usize;
    }
    pub fn count(&self) -> i32 {
        self.items.len() as i32
    }
    pub fn add(&mut self, v: T) {
        super::gc::wb(&v);
        self.cap = grown_capacity(self.cap, self.items.len() + 1);
        self.items.push(v);
        self.version = self.version.wrapping_add(1);
    }
    /// Address of item `i` in the list's buffer (valid until the list changes): for runtime and host code that
    /// reads items in place. `ArgumentOutOfRangeException` outside the list.
    pub fn item_ptr(&self, i: i32) -> *const T {
        match self.items.get(i as usize) {
            Some(v) if i >= 0 => v as *const T,
            _ => arg_out_of_range(),
        }
    }
    /// `list[i]` (get): `ArgumentOutOfRangeException` outside the list.
    pub fn get_item(&self, i: i32) -> T {
        match self.items.get(i as usize) {
            Some(v) if i >= 0 => *v,
            _ => arg_out_of_range(),
        }
    }
    pub fn set_item(&mut self, i: i32, v: T) {
        super::gc::wb(&v);
        if i < 0 || i as usize >= self.items.len() {
            arg_out_of_range();
        }
        self.items[i as usize] = v;
        self.version = self.version.wrapping_add(1);
    }
    pub fn clear(&mut self) {
        self.items.clear();
        self.version = self.version.wrapping_add(1);
    }
    pub fn remove_at(&mut self, i: i32) {
        if i < 0 || i as usize >= self.items.len() {
            arg_out_of_range();
        }
        self.items.remove(i as usize);
        self.version = self.version.wrapping_add(1);
    }
    pub fn insert(&mut self, i: i32, v: T) {
        super::gc::wb(&v);
        if i < 0 || i as usize > self.items.len() {
            arg_out_of_range();
        }
        self.cap = grown_capacity(self.cap, self.items.len() + 1);
        self.items.insert(i as usize, v);
        self.version = self.version.wrapping_add(1);
    }
    /// `RemoveRange(index, count)`
    pub fn remove_range(&mut self, index: i32, count: i32) {
        if index < 0 || count < 0 {
            arg_out_of_range();
        }
        if self.items.len() < index as usize || self.items.len() - (index as usize) < count as usize {
            super::throw(super::exc("System.ArgumentException", super::Str::lit("Offset and length were out of bounds for the array or count is greater than the number of elements from index to the end of the source collection.")));
        }
        if count > 0 {
            self.items.drain(index as usize..(index + count) as usize);
            self.version = self.version.wrapping_add(1);
        }
    }
    /// `GetRange(index, count)`: a new list with a copy of the range.
    pub fn get_range(&self, index: i32, count: i32) -> Gc<List<T>> {
        if index < 0 || count < 0 {
            arg_out_of_range();
        }
        if self.items.len() < index as usize || self.items.len() - (index as usize) < count as usize {
            super::throw(super::exc("System.ArgumentException", super::Str::lit("Offset and length were out of bounds for the array or count is greater than the number of elements from index to the end of the source collection.")));
        }
        let items = self.items[index as usize..(index + count) as usize].to_vec();
        let cap = items.len();
        alloc_init(List { items, version: 0, cap })
    }
    /// `AddRange`: the source is read completely before anything is appended (it may be this list).
    pub fn add_range(&mut self, src: Seq<T>) {
        let snapshot = src.to_vec();
        super::gc::wb_slice(&snapshot);
        if src.kind == 1 || src.kind == 2 {
            // an ICollection<T>: InsertRange makes room for all of it at once
            if !snapshot.is_empty() {
                self.cap = grown_capacity(self.cap, self.items.len() + snapshot.len());
            }
        } else {
            // any other sequence is added element by element
            for k in 1..=snapshot.len() {
                self.cap = grown_capacity(self.cap, self.items.len() + k);
            }
        }
        self.items.extend(snapshot);
        self.version = self.version.wrapping_add(1);
    }
    pub fn to_array(&self) -> Arr<T> {
        Arr::from_slice(&self.items)
    }
    /// `new List<T>(collection)`.
    pub fn from_seq_gc(src: Seq<T>) -> Gc<List<T>> {
        let items = src.to_vec();
        // an ICollection<T> (list, array) is copied into an array of its size; anything else is added one by one
        let mut cap = if src.kind == 1 || src.kind == 2 { items.len() } else { 0 };
        for k in 1..=items.len() {
            cap = grown_capacity(cap, k);
        }
        alloc_init(List { items, version: 0, cap })
    }
    /// `l.Reverse()`.
    pub fn reverse(&mut self) {
        self.items.reverse();
        self.version = self.version.wrapping_add(1);
    }
    pub(crate) fn snapshot(&self) -> Vec<T> {
        self.items.clone()
    }
    pub(crate) fn replace(&mut self, items: Vec<T>) {
        for k in 1..=items.len() {
            self.cap = grown_capacity(self.cap, k);
        }
        self.items = items;
        self.version = self.version.wrapping_add(1);
    }
}
impl<T: CsKey> List<T> {
    pub fn index_of(&self, v: T) -> i32 {
        self.items.iter().position(|x| x.cs_eq(&v)).map(|i| i as i32).unwrap_or(-1)
    }
    pub fn contains(&self, v: T) -> bool {
        self.index_of(v) >= 0
    }
    pub fn remove(&mut self, v: T) -> bool {
        match self.items.iter().position(|x| x.cs_eq(&v)) {
            Some(i) => {
                self.items.remove(i);
                self.version = self.version.wrapping_add(1);
                true
            }
            None => false,
        }
    }
}

// ---------------------------------------------------------------------------------------------- Queue<T>

/// `System.Collections.Generic.Queue<T>`.
pub struct Queue<T> {
    items: std::collections::VecDeque<T>,
}

impl<T: Val> Queue<T> {
    /// `new Queue<T>()`.
    pub fn new_gc() -> Gc<Queue<T>> {
        alloc_init(Queue { items: std::collections::VecDeque::new() })
    }
    pub fn count(&self) -> i32 {
        self.items.len() as i32
    }
    pub fn enqueue(&mut self, v: T) {
        super::gc::wb(&v);
        self.items.push_back(v);
    }
    /// `q.Peek()`: `InvalidOperationException` when empty.
    pub fn peek(&self) -> T {
        match self.items.front() {
            Some(v) => *v,
            None => throw(exc("System.InvalidOperationException", crate::s!("Queue empty."))),
        }
    }
    pub fn clear(&mut self) {
        self.items.clear();
    }
    /// `q.Dequeue()`: `InvalidOperationException` when empty.
    pub fn dequeue(&mut self) -> T {
        match self.items.pop_front() {
            Some(v) => v,
            None => throw(exc("System.InvalidOperationException", crate::s!("Queue empty."))),
        }
    }
}

// ---------------------------------------------------------------------------------------------- Dictionary<K, V>

#[repr(C)]
#[derive(Clone, Copy)]
pub struct KeyValuePair<K, V> {
    pub key: K,
    pub value: V,
}

/// `System.Collections.Generic.Dictionary<K, V>` with .NET's enumeration order: entries in slot order, where
/// `Add` fills the most recently freed slot first and otherwise appends.
pub struct Dictionary<K, V> {
    index: HashMap<KeyW<K>, usize>,
    entries: Vec<Option<(K, V)>>,
    free: Vec<usize>,
    version: u32,
}

impl<K: CsKey, V: Val> Dictionary<K, V> {
    /// `new Dictionary<K, V>()`.
    pub fn new_gc() -> Gc<Dictionary<K, V>> {
        alloc_init(Dictionary { index: HashMap::new(), entries: Vec::new(), free: Vec::new(), version: 0 })
    }
    pub fn count(&self) -> i32 {
        self.index.len() as i32
    }
    pub fn contains_key(&self, k: K) -> bool {
        self.index.contains_key(&KeyW(k))
    }
    fn insert(&mut self, k: K, v: V, add: bool) {
        super::gc::wb(&k);
        super::gc::wb(&v);
        if let Some(&slot) = self.index.get(&KeyW(k)) {
            if add {
                throw(exc("System.ArgumentException", crate::s!("An item with the same key has already been added.")))
            }
            self.entries[slot] = Some((k, v));
            self.version = self.version.wrapping_add(1);
            return;
        }
        let slot = match self.free.pop() {
            Some(s) => {
                self.entries[s] = Some((k, v));
                s
            }
            None => {
                self.entries.push(Some((k, v)));
                self.entries.len() - 1
            }
        };
        self.index.insert(KeyW(k), slot);
        self.version = self.version.wrapping_add(1);
    }
    /// `d.Add(k, v)`: `ArgumentException` on a duplicate key.
    pub fn add(&mut self, k: K, v: V) {
        self.insert(k, v, true)
    }
    /// `d[k] = v`.
    pub fn set_item(&mut self, k: K, v: V) {
        self.insert(k, v, false)
    }
    /// `d[k]`: `KeyNotFoundException` when missing.
    pub fn get_item(&self, k: K) -> V {
        match self.index.get(&KeyW(k)) {
            Some(&slot) => self.entries[slot].expect("indexed slot is occupied").1,
            None => throw(exc("System.Collections.Generic.KeyNotFoundException", crate::s!("The given key was not present in the dictionary."))),
        }
    }
    /// `d.TryGetValue(k, out v)`: writes `default(V)` when missing.
    ///
    /// # Safety
    /// `out` must be a valid pointer (the address of the `out` argument).
    pub unsafe fn try_get_value(&self, k: K, out: *mut V) -> bool {
        match self.index.get(&KeyW(k)) {
            Some(&slot) => {
                // the `out` variable may be a field of an object the collector has traced already
                *out = super::gc::wbv(self.entries[slot].expect("indexed slot is occupied").1);
                true
            }
            None => {
                *out = core::mem::zeroed();
                false
            }
        }
    }
    pub fn remove(&mut self, k: K) -> bool {
        match self.index.remove(&KeyW(k)) {
            Some(slot) => {
                self.entries[slot] = None;
                self.free.push(slot);
                self.version = self.version.wrapping_add(1);
                true
            }
            None => false,
        }
    }
    pub fn clear(&mut self) {
        self.index.clear();
        self.entries.clear();
        self.free.clear();
        self.version = self.version.wrapping_add(1);
    }
}

// ---------------------------------------------------------------------------------------------- IEnumerable<T>

/// `IEnumerable<T>`: a tagged handle to the collection it was converted from. All-zero is `null`.
#[repr(C)]
pub struct Seq<T> {
    kind: u32, // 0 null, 1 List<T>, 2 T[]
    ptr: *mut u8,
    _t: core::marker::PhantomData<T>,
}
impl<T> Clone for Seq<T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T> Copy for Seq<T> {}

impl<T: Val> Seq<T> {
    /// Implicit conversion `List<T>` -> `IEnumerable<T>` (null stays null).
    pub fn from_list(l: Gc<List<T>>) -> Seq<T> {
        Seq { kind: if l.is_null() { 0 } else { 1 }, ptr: l.0 as *mut u8, _t: core::marker::PhantomData }
    }
    /// Implicit conversion `T[]` -> `IEnumerable<T>`.
    pub fn from_arr(a: Arr<T>) -> Seq<T> {
        // SAFETY: Arr<T> is a transparent wrapper around one pointer.
        let p: *mut u8 = unsafe { core::mem::transmute_copy(&a) };
        Seq { kind: if a.is_null() { 0 } else { 2 }, ptr: p, _t: core::marker::PhantomData }
    }
    pub fn is_null(self) -> bool {
        self.kind == 0
    }
    fn as_list(self) -> Gc<List<T>> {
        Gc(self.ptr as *mut List<T>)
    }
    fn as_arr(self) -> Arr<T> {
        // SAFETY: kind 2 was built from an Arr<T> by `from_arr`.
        unsafe { core::mem::transmute_copy(&self.ptr) }
    }
    /// Snapshot of the elements (null: `ArgumentNullException`-like null failure).
    pub fn to_vec(self) -> Vec<T> {
        match self.kind {
            // SAFETY: kind 1 was built from a non-null list; the reference ends with this statement.
            1 => unsafe { (*self.as_list().ptr()).items.clone() },
            2 => self.as_arr().as_slice().to_vec(),
            3 => {
                let mut v = Vec::new();
                let mut e = self.enumerator();
                while e.move_next() {
                    v.push(e.current());
                }
                v
            }
            _ => null_reference(),
        }
    }
}

// ---------------------------------------------------------------------------------------------- collector support

/// Elements of a collection in enumeration order, boxed.
unsafe fn reflect_items<C: Enumerable>(c: C) -> Vec<super::Obj>
where
    <C::Enum as Enumerator>::Item: Trace + 'static,
{
    let mut e = c.enumerator();
    let mut out = Vec::new();
    while e.move_next() {
        out.push(e.current().box_ref());
    }
    out
}
unsafe impl<T: Val> Trace for List<T> {
    unsafe fn obj_clear(this: *mut Self) -> bool {
        (*this).clear();
        true
    }
    // the items live in the Vec's buffer: a pointer to an item must keep the list alive
    const HAS_BUFFER: bool = true;
    unsafe fn obj_buffer(this: *const Self) -> (usize, usize) {
        ((*this).items.as_ptr() as usize, (*this).items.capacity() * core::mem::size_of::<T>())
    }
    const NEEDS_TRACE: bool = T::NEEDS_TRACE;
    // every method that stores an element calls the store barrier (`gc::wb`); the buffer is traced in one piece
    const RESCAN: bool = false;
    unsafe fn obj_reflect(this: *const Self) -> super::object::Reflect {
        super::object::Reflect::Seq { kind: "List", items: reflect_items(Gc(this as *mut Self)) }
    }
    unsafe fn trace(&self, m: &mut Marker) {
        self.items.trace(m)
    }
}
unsafe impl<T: Val> Trace for Queue<T> {
    unsafe fn obj_clear(this: *mut Self) -> bool {
        (*this).clear();
        true
    }
    const NEEDS_TRACE: bool = T::NEEDS_TRACE;
    // every method that stores an element calls the store barrier (`gc::wb`); the buffer is traced in one piece
    const RESCAN: bool = false;
    unsafe fn obj_reflect(this: *const Self) -> super::object::Reflect {
        super::object::Reflect::Seq { kind: "Queue", items: reflect_items(Gc(this as *mut Self)) }
    }
    unsafe fn trace(&self, m: &mut Marker) {
        if T::NEEDS_TRACE {
            for x in &self.items {
                x.trace(m)
            }
        }
    }
}
unsafe impl<T: Val> Trace for Stack<T> {
    unsafe fn obj_clear(this: *mut Self) -> bool {
        (*this).clear();
        true
    }
    const NEEDS_TRACE: bool = T::NEEDS_TRACE;
    // every method that stores an element calls the store barrier (`gc::wb`); the buffer is traced in one piece
    const RESCAN: bool = false;
    unsafe fn obj_reflect(this: *const Self) -> super::object::Reflect {
        super::object::Reflect::Seq { kind: "Stack", items: reflect_items(Gc(this as *mut Self)) }
    }
    unsafe fn trace(&self, m: &mut Marker) {
        self.items.trace(m)
    }
}
unsafe impl<K: Val, V: Val> Trace for KeyValuePair<K, V> {
    const NEEDS_TRACE: bool = K::NEEDS_TRACE || V::NEEDS_TRACE;
    const RESCAN: bool = super::gc::field_rescan::<K>() || super::gc::field_rescan::<V>();
    unsafe fn trace(&self, m: &mut Marker) {
        self.key.trace(m);
        self.value.trace(m);
    }
}
unsafe impl<K: CsKey, V: Val> Trace for Dictionary<K, V> {
    unsafe fn obj_clear(this: *mut Self) -> bool {
        (*this).clear();
        true
    }
    const NEEDS_TRACE: bool = K::NEEDS_TRACE || V::NEEDS_TRACE;
    // every method that stores an element calls the store barrier (`gc::wb`); the buffer is traced in one piece
    const RESCAN: bool = false;
    unsafe fn obj_reflect(this: *const Self) -> super::object::Reflect {
        let mut e = Gc(this as *mut Self).enumerator();
        let mut items = Vec::new();
        while e.move_next() {
            let kv = e.current();
            items.push((kv.key.box_ref(), kv.value.box_ref()));
        }
        super::object::Reflect::Dict { items }
    }
    unsafe fn trace(&self, m: &mut Marker) {
        self.entries.trace(m)
    }
}
unsafe impl<T: CsKey> Trace for HashSet<T> {
    unsafe fn obj_clear(this: *mut Self) -> bool {
        (*this).clear();
        true
    }
    const NEEDS_TRACE: bool = T::NEEDS_TRACE;
    // every method that stores an element calls the store barrier (`gc::wb`); the buffer is traced in one piece
    const RESCAN: bool = false;
    unsafe fn obj_reflect(this: *const Self) -> super::object::Reflect {
        super::object::Reflect::Seq { kind: "HashSet", items: reflect_items(Gc(this as *mut Self)) }
    }
    unsafe fn trace(&self, m: &mut Marker) {
        self.entries.trace(m)
    }
}
unsafe impl<T> Trace for Seq<T> {
    const RESCAN: bool = false;
    #[inline(always)]
    unsafe fn trace(&self, m: &mut Marker) {
        m.mark_payload(self.ptr)
    }
}
unsafe impl<T: Val> Trace for SeqDyn<T> {
    // immutable: a factory closure and the tracer of what it captured
    const RESCAN: bool = false;
    unsafe fn trace(&self, m: &mut Marker) {
        if let Some(t) = &self.tracer {
            t(m)
        }
    }
}

// ---------------------------------------------------------------------------------------------- Stack<T>

/// `System.Collections.Generic.Stack<T>`. `foreach` goes from the top down, as in .NET.
pub struct Stack<T> {
    items: Vec<T>,
    version: u32,
}

impl<T: Val> Stack<T> {
    pub fn new_gc() -> Gc<Stack<T>> {
        alloc_init(Stack { items: Vec::new(), version: 0 })
    }
    pub fn count(&self) -> i32 {
        self.items.len() as i32
    }
    pub fn push(&mut self, v: T) {
        super::gc::wb(&v);
        self.items.push(v);
        self.version = self.version.wrapping_add(1);
    }
    /// `s.Pop()`: `InvalidOperationException` when empty.
    pub fn pop(&mut self) -> T {
        self.version = self.version.wrapping_add(1);
        match self.items.pop() {
            Some(v) => v,
            None => throw(exc("System.InvalidOperationException", crate::s!("Stack empty."))),
        }
    }
    pub fn peek(&self) -> T {
        match self.items.last() {
            Some(v) => *v,
            None => throw(exc("System.InvalidOperationException", crate::s!("Stack empty."))),
        }
    }
    pub fn clear(&mut self) {
        self.items.clear();
        self.version = self.version.wrapping_add(1);
    }
    pub fn to_array(&self) -> Arr<T> {
        let v: Vec<T> = self.items.iter().rev().copied().collect();
        Arr::from_slice(&v)
    }
}
impl<T: CsKey> Stack<T> {
    pub fn contains(&self, v: T) -> bool {
        self.items.iter().any(|x| x.cs_eq(&v))
    }
}

// ---------------------------------------------------------------------------------------------- HashSet<T>

/// `System.Collections.Generic.HashSet<T>` with .NET's enumeration order (slot order; `Add` reuses the most
/// recently freed slot first, otherwise appends), the same scheme as [`Dictionary`].
pub struct HashSet<T> {
    index: HashMap<KeyW<T>, usize>,
    entries: Vec<Option<T>>,
    free: Vec<usize>,
    version: u32,
}

impl<T: CsKey> HashSet<T> {
    pub fn new_gc() -> Gc<HashSet<T>> {
        alloc_init(HashSet { index: HashMap::new(), entries: Vec::new(), free: Vec::new(), version: 0 })
    }
    /// `new HashSet<T>(collection)`.
    pub fn from_seq_gc(src: Seq<T>) -> Gc<HashSet<T>> {
        let h = Self::new_gc();
        for x in src.to_vec() {
            // SAFETY: fresh object, no other reference.
            unsafe { (*h.0).add(x) };
        }
        h
    }
    pub fn count(&self) -> i32 {
        self.index.len() as i32
    }
    pub fn contains(&self, v: T) -> bool {
        self.index.contains_key(&KeyW(v))
    }
    /// `h.Add(v)`: false when already present.
    pub fn add(&mut self, v: T) -> bool {
        super::gc::wb(&v);
        if self.index.contains_key(&KeyW(v)) {
            return false;
        }
        let slot = match self.free.pop() {
            Some(s) => {
                self.entries[s] = Some(v);
                s
            }
            None => {
                self.entries.push(Some(v));
                self.entries.len() - 1
            }
        };
        self.index.insert(KeyW(v), slot);
        self.version = self.version.wrapping_add(1);
        true
    }
    pub fn remove(&mut self, v: T) -> bool {
        match self.index.remove(&KeyW(v)) {
            Some(slot) => {
                self.entries[slot] = None;
                self.free.push(slot);
                self.version = self.version.wrapping_add(1);
                true
            }
            None => false,
        }
    }
    pub fn clear(&mut self) {
        self.index.clear();
        self.entries.clear();
        self.free.clear();
        self.version = self.version.wrapping_add(1);
    }
    fn snapshot(&self) -> Vec<T> {
        self.entries.iter().flatten().copied().collect()
    }
    /// `h.UnionWith(other)`.
    pub fn union_with(&mut self, other: Seq<T>) {
        for x in other.to_vec() {
            self.add(x);
        }
    }
    /// `h.ExceptWith(other)`.
    pub fn except_with(&mut self, other: Seq<T>) {
        for x in other.to_vec() {
            self.remove(x);
        }
    }
    pub fn to_array(&self) -> Arr<T> {
        Arr::from_slice(&self.snapshot())
    }
}

pub struct HashSetEnum<T: CsKey> {
    h: Gc<HashSet<T>>,
    i: usize,
    version: u32,
    cur: T,
}
impl<T: CsKey> Enumerator for HashSetEnum<T> {
    type Item = T;
    fn move_next(&mut self) -> bool {
        // SAFETY: null-checked when the enumerator was made; the reference ends with this call.
        let h = unsafe { &*self.h.0 };
        if h.version != self.version {
            modified();
        }
        while self.i < h.entries.len() {
            let e = h.entries[self.i];
            self.i += 1;
            if let Some(v) = e {
                self.cur = v;
                return true;
            }
        }
        false
    }
    fn current(&self) -> T {
        self.cur
    }
}
impl<T: CsKey> Enumerable for Gc<HashSet<T>> {
    type Enum = HashSetEnum<T>;
    fn enumerator(self) -> HashSetEnum<T> {
        // SAFETY: `ptr` null-checks; the reference ends with this statement.
        let version = unsafe { (*self.ptr()).version };
        // SAFETY: zero is a valid T (runtime invariant).
        HashSetEnum { h: self, i: 0, version, cur: unsafe { core::mem::zeroed() } }
    }
}

/// Enumerator over a snapshot (`Stack<T>`, `Queue<T>`, `Dictionary.Keys` / `.Values`): the collection's version
/// is still checked through `check`, so modification during `foreach` throws as in .NET.
pub struct VecEnum<T: Val> {
    items: Vec<T>,
    i: usize,
    cur: T,
    check: Option<Box<dyn Fn()>>,
}
impl<T: Val> VecEnum<T> {
    pub fn new(items: Vec<T>) -> Self {
        Self::register(&items);
        // SAFETY: zero is a valid T (runtime invariant).
        VecEnum { items, i: 0, cur: unsafe { core::mem::zeroed() }, check: None }
    }
    fn checked(items: Vec<T>, check: Box<dyn Fn()>) -> Self {
        Self::register(&items);
        // SAFETY: zero is a valid T (runtime invariant).
        VecEnum { items, i: 0, cur: unsafe { core::mem::zeroed() }, check: Some(check) }
    }
    /// The snapshot's buffer holds handles in a heap block of its own: declared to the collector, which finds this
    /// enumerator only by scanning the bytes of a suspended iterator method (`gc::register_scan_block`). The
    /// buffer is never reallocated (the vector is only read).
    fn register(items: &Vec<T>) {
        if T::NEEDS_TRACE {
            super::gc::register_scan_block(items.as_ptr() as *const u8, core::mem::size_of_val(items.as_slice()));
        }
    }
}
impl<T: Val> Drop for VecEnum<T> {
    fn drop(&mut self) {
        if T::NEEDS_TRACE {
            super::gc::unregister_scan_block(self.items.as_ptr() as *const u8);
        }
    }
}
impl<T: Val> Enumerator for VecEnum<T> {
    type Item = T;
    fn move_next(&mut self) -> bool {
        if let Some(c) = &self.check {
            c()
        }
        match self.items.get(self.i) {
            Some(v) => {
                self.cur = *v;
                self.i += 1;
                true
            }
            None => false,
        }
    }
    fn current(&self) -> T {
        self.cur
    }
}
impl<T: Val> Enumerable for Gc<Stack<T>> {
    type Enum = VecEnum<T>;
    fn enumerator(self) -> VecEnum<T> {
        // SAFETY: `ptr` null-checks; the references end with each statement.
        unsafe {
            let version = (*self.ptr()).version;
            let items: Vec<T> = (*self.0).items.iter().rev().copied().collect();
            VecEnum::checked(items, Box::new(move || if (*self.0).version != version { modified() }))
        }
    }
}
impl<T: Val> Enumerable for Gc<Queue<T>> {
    type Enum = VecEnum<T>;
    fn enumerator(self) -> VecEnum<T> {
        // SAFETY: `ptr` null-checks; the reference ends with this statement.
        VecEnum::new(unsafe { (*self.ptr()).items.iter().copied().collect() })
    }
}

impl<K: CsKey, V: Val> Dictionary<K, V> {
    /// `ConcurrentDictionary<K, V>.ToArray()`: a snapshot of the pairs in enumeration order.
    pub fn to_pair_array(this: Gc<Self>) -> Arr<KeyValuePair<K, V>> {
        let mut e = this.enumerator();
        let mut out = Vec::new();
        while e.move_next() {
            out.push(e.current());
        }
        Arr::from_slice(&out)
    }
    /// `d.Keys` as a sequence in enumeration order; modification of the dictionary while it is enumerated throws.
    pub fn keys(this: Gc<Self>) -> Seq<K> {
        let _ = this.ptr();
        Seq::lazy(
            move || {
                // SAFETY: non-null (checked above); references end with each statement.
                let (version, items) = unsafe { ((*this.0).version, (*this.0).entries.iter().flatten().map(|e| e.0).collect()) };
                Box::new(VecEnum::checked(items, Box::new(move || if unsafe { (*this.0).version } != version { modified() })))
            },
            move |m| m.mark(this),
        )
    }
    /// `d.Values`.
    pub fn values(this: Gc<Self>) -> Seq<V> {
        let _ = this.ptr();
        Seq::lazy(
            move || {
                // SAFETY: as `keys`.
                let (version, items) = unsafe { ((*this.0).version, (*this.0).entries.iter().flatten().map(|e| e.1).collect()) };
                Box::new(VecEnum::checked(items, Box::new(move || if unsafe { (*this.0).version } != version { modified() })))
            },
            move |m| m.mark(this),
        )
    }
    /// The dictionary as `IEnumerable<KeyValuePair<K, V>>`.
    pub fn as_seq(this: Gc<Self>) -> Seq<KeyValuePair<K, V>> {
        if this.is_null() {
            return Seq::NULL;
        }
        Seq::lazy(move || Box::new(this.enumerator()), move |m| m.mark(this))
    }
}

/// An `IEnumerable<T>` that is not a `List<T>` or `T[]`: a factory of enumerators (lazy LINQ operators,
/// other collections, iterator methods).
pub struct SeqDyn<T> {
    make: Box<dyn Fn() -> Box<dyn Enumerator<Item = T>>>,
    tracer: Option<Box<dyn Fn(&mut Marker)>>,
}

impl<T: Val> Seq<T> {
    pub const NULL: Seq<T> = Seq { kind: 0, ptr: core::ptr::null_mut(), _t: core::marker::PhantomData };

    /// A sequence defined by an enumerator factory; `tracer` reports the handles the factory captured.
    pub fn lazy(make: impl Fn() -> Box<dyn Enumerator<Item = T>> + 'static, tracer: impl Fn(&mut Marker) + 'static) -> Seq<T> {
        let g = alloc_init(SeqDyn { make: Box::new(make), tracer: Some(Box::new(tracer)) });
        Seq { kind: 3, ptr: g.0 as *mut u8, _t: core::marker::PhantomData }
    }
    /// A sequence over a fixed snapshot.
    pub fn from_vec(items: Vec<T>) -> Seq<T> {
        Seq::from_arr(Arr::from_slice(&items))
    }
    pub fn from_hash_set(h: Gc<HashSet<T>>) -> Seq<T>
    where
        T: CsKey,
    {
        if h.is_null() {
            return Seq::NULL;
        }
        Seq::lazy(move || Box::new(h.enumerator()), move |m| m.mark(h))
    }
    pub fn from_stack(h: Gc<Stack<T>>) -> Seq<T> {
        if h.is_null() {
            return Seq::NULL;
        }
        Seq::lazy(move || Box::new(h.enumerator()), move |m| m.mark(h))
    }
    pub fn from_queue(h: Gc<Queue<T>>) -> Seq<T> {
        if h.is_null() {
            return Seq::NULL;
        }
        Seq::lazy(move || Box::new(h.enumerator()), move |m| m.mark(h))
    }
}

impl<T> Seq<T> {
    /// Identity of the underlying object, for `(object)a == (object)b` and conversion to `object`.
    pub fn addr(self) -> usize {
        self.ptr as usize
    }
    pub fn up<U>(self) -> Gc<U> {
        Gc(self.ptr as *mut U)
    }
}

// ---------------------------------------------------------------------------------------------- foreach

/// What `foreach` drives: `while e.move_next() { let x = e.current(); ... }`.
pub trait Enumerator {
    type Item: Val;
    fn move_next(&mut self) -> bool;
    fn current(&self) -> Self::Item;
}
pub trait Enumerable {
    type Enum: Enumerator;
    fn enumerator(self) -> Self::Enum;
}
/// `collection.GetEnumerator()` (a null collection throws `NullReferenceException`).
pub fn get_enumerator<C: Enumerable>(c: C) -> C::Enum {
    c.enumerator()
}

pub struct ArrEnum<T: Val> {
    a: Arr<T>,
    i: i32,
    cur: T,
}
impl<T: Val> Enumerator for ArrEnum<T> {
    type Item = T;
    fn move_next(&mut self) -> bool {
        // C# reads `a.Length` on every iteration; arrays never change length
        if self.i < self.a.len() {
            // SAFETY: index checked by `at`.
            self.cur = unsafe { *self.a.at(self.i) };
            self.i += 1;
            true
        } else {
            false
        }
    }
    fn current(&self) -> T {
        self.cur
    }
}
impl<T: Val> Enumerable for Arr<T> {
    type Enum = ArrEnum<T>;
    fn enumerator(self) -> ArrEnum<T> {
        let _ = self.len(); // null check
        // SAFETY: zero is a valid T (runtime invariant).
        ArrEnum { a: self, i: 0, cur: unsafe { core::mem::zeroed() } }
    }
}

/// `foreach (char c in s)`: UTF-16 units.
impl Enumerable for Str {
    type Enum = VecEnum<u16>;
    fn enumerator(self) -> VecEnum<u16> {
        VecEnum::new(self.as_str().encode_utf16().collect())
    }
}
/// `foreach` over `T[,]`: row-major, like the CLR.
pub struct Arr2Enum<T: Val> {
    a: super::Arr2<T>,
    i: i32,
    cur: T,
}
impl<T: Val> Enumerator for Arr2Enum<T> {
    type Item = T;
    fn move_next(&mut self) -> bool {
        let d1 = self.a.dim(1);
        if self.i < self.a.len() {
            // SAFETY: both indices are in range by construction.
            self.cur = unsafe { *self.a.at(self.i / d1, self.i % d1) };
            self.i += 1;
            true
        } else {
            false
        }
    }
    fn current(&self) -> T {
        self.cur
    }
}
impl<T: Val> Enumerable for super::Arr2<T> {
    type Enum = Arr2Enum<T>;
    fn enumerator(self) -> Arr2Enum<T> {
        let _ = self.len(); // null check
        // SAFETY: zero is a valid T (runtime invariant).
        Arr2Enum { a: self, i: 0, cur: unsafe { core::mem::zeroed() } }
    }
}

/// `List<T>.Enumerator`: also a C# value of its own (`List<T>.Enumerator e = list.GetEnumerator()`), so it is
/// `Copy` and reports its handles like every value type of the runtime.
pub struct ListEnum<T: Val> {
    l: Gc<List<T>>,
    i: usize,
    version: u32,
    cur: T,
}
impl<T: Val> Clone for ListEnum<T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T: Val> Copy for ListEnum<T> {}
impl<T: Val> ListEnum<T> {
    /// The list this enumerator walks.
    pub fn list(&self) -> Gc<List<T>> {
        self.l
    }
}
// SAFETY: the list and the current element are reported.
unsafe impl<T: Val> Trace for ListEnum<T> {
    unsafe fn trace(&self, m: &mut Marker) {
        m.mark(self.l);
        self.cur.trace(m);
    }
}
impl<T: Val> Enumerator for ListEnum<T> {
    type Item = T;
    fn move_next(&mut self) -> bool {
        // SAFETY: the list was null-checked when the enumerator was made; the reference ends with this call.
        let l = unsafe { &*self.l.0 };
        if l.version != self.version {
            modified();
        }
        match l.items.get(self.i) {
            Some(v) => {
                self.cur = *v;
                self.i += 1;
                true
            }
            None => false,
        }
    }
    fn current(&self) -> T {
        self.cur
    }
}
impl<T: Val> Enumerable for Gc<List<T>> {
    type Enum = ListEnum<T>;
    fn enumerator(self) -> ListEnum<T> {
        // SAFETY: `ptr` null-checks; the reference ends with this statement.
        let version = unsafe { (*self.ptr()).version };
        // SAFETY: zero is a valid T (runtime invariant).
        ListEnum { l: self, i: 0, version, cur: unsafe { core::mem::zeroed() } }
    }
}

pub enum SeqEnum<T: Val> {
    List(ListEnum<T>),
    Arr(ArrEnum<T>),
    Dyn(DynEnum<T>),
}

/// The boxed enumerator of a lazy sequence. Its state (handles of the source, of delegates, a current element) is
/// in a heap block of its own: declared to the collector for the time it exists, see `gc::register_scan_block`.
pub struct DynEnum<T: Val>(Box<dyn Enumerator<Item = T>>);
impl<T: Val> DynEnum<T> {
    fn new(e: Box<dyn Enumerator<Item = T>>) -> Self {
        let r: &dyn Enumerator<Item = T> = &*e;
        super::gc::register_scan_block(r as *const dyn Enumerator<Item = T> as *const u8, core::mem::size_of_val(r));
        DynEnum(e)
    }
}
impl<T: Val> Drop for DynEnum<T> {
    fn drop(&mut self) {
        let r: &dyn Enumerator<Item = T> = &*self.0;
        super::gc::unregister_scan_block(r as *const dyn Enumerator<Item = T> as *const u8);
    }
}
impl<T: Val> Enumerator for SeqEnum<T> {
    type Item = T;
    fn move_next(&mut self) -> bool {
        match self {
            SeqEnum::List(e) => e.move_next(),
            SeqEnum::Arr(e) => e.move_next(),
            SeqEnum::Dyn(e) => e.0.move_next(),
        }
    }
    fn current(&self) -> T {
        match self {
            SeqEnum::List(e) => e.current(),
            SeqEnum::Arr(e) => e.current(),
            SeqEnum::Dyn(e) => e.0.current(),
        }
    }
}
impl<T: Val> Enumerable for Seq<T> {
    type Enum = SeqEnum<T>;
    fn enumerator(self) -> SeqEnum<T> {
        match self.kind {
            1 => SeqEnum::List(self.as_list().enumerator()),
            2 => SeqEnum::Arr(self.as_arr().enumerator()),
            // SAFETY: kind 3 was built by `lazy` from a live SeqDyn<T>.
            3 => SeqEnum::Dyn(DynEnum::new(unsafe { ((*(self.ptr as *const SeqDyn<T>)).make)() })),
            _ => null_reference(),
        }
    }
}

pub struct DictEnum<K: CsKey, V: Val> {
    d: Gc<Dictionary<K, V>>,
    i: usize,
    version: u32,
    cur: KeyValuePair<K, V>,
}
impl<K: CsKey, V: Val> Enumerator for DictEnum<K, V> {
    type Item = KeyValuePair<K, V>;
    fn move_next(&mut self) -> bool {
        // SAFETY: null-checked when the enumerator was made; the reference ends with this call.
        let d = unsafe { &*self.d.0 };
        if d.version != self.version {
            modified();
        }
        while self.i < d.entries.len() {
            let e = d.entries[self.i];
            self.i += 1;
            if let Some((key, value)) = e {
                self.cur = KeyValuePair { key, value };
                return true;
            }
        }
        false
    }
    fn current(&self) -> KeyValuePair<K, V> {
        self.cur
    }
}
impl<K: CsKey, V: Val> Enumerable for Gc<Dictionary<K, V>> {
    type Enum = DictEnum<K, V>;
    fn enumerator(self) -> DictEnum<K, V> {
        // SAFETY: `ptr` null-checks; the reference ends with this statement.
        let version = unsafe { (*self.ptr()).version };
        // SAFETY: zero is a valid K and V (runtime invariant).
        DictEnum { d: self, i: 0, version, cur: unsafe { core::mem::zeroed() } }
    }
}

// ---------------------------------------------------------------------------------------------- LINQ (eager operators)

/// `System.Linq.Enumerable` operators that produce a value at once. The predicate may run generated code,
/// so each operator enumerates through the same enumerators as `foreach` (no Rust reference is held).
pub mod linq {
    use super::*;

    /// `Enumerable.Empty<T>()`.
    pub fn empty<T: Val>() -> Seq<T> {
        Seq::from_arr(Arr::<T>::new(0))
    }

    /// `source.Any(predicate)`.
    pub fn any<T: Val>(source: Seq<T>, predicate: Delegate<dyn Fn(T) -> bool>) -> bool {
        let mut e = get_enumerator(source);
        while e.move_next() {
            if predicate.invoke(|f| f(e.current())) {
                return true;
            }
        }
        false
    }

    /// `source.Any()`.
    pub fn any0<T: Val>(source: Seq<T>) -> bool {
        get_enumerator(source).move_next()
    }

    /// `source.SingleOrDefault(predicate)`: `InvalidOperationException` when more than one element matches.
    pub fn single_or_default<T: Val>(source: Seq<T>, predicate: Delegate<dyn Fn(T) -> bool>) -> T {
        // SAFETY: zero is `default(T)`.
        let mut found: T = unsafe { core::mem::zeroed() };
        let mut n = 0;
        let mut e = get_enumerator(source);
        while e.move_next() {
            let x = e.current();
            if predicate.invoke(|f| f(x)) {
                found = x;
                n += 1;
                if n > 1 {
                    throw(exc("System.InvalidOperationException", crate::s!("Sequence contains more than one matching element")))
                }
            }
        }
        found
    }

    /// `source.FirstOrDefault(predicate)`.
    pub fn first_or_default<T: Val>(source: Seq<T>, predicate: Delegate<dyn Fn(T) -> bool>) -> T {
        let mut e = get_enumerator(source);
        while e.move_next() {
            let x = e.current();
            if predicate.invoke(|f| f(x)) {
                return x;
            }
        }
        // SAFETY: zero is `default(T)`.
        unsafe { core::mem::zeroed() }
    }

    /// `source.Where(predicate)`: lazy; the source is enumerated again each time the result is.
    pub fn where_<T: Val>(source: Seq<T>, predicate: Delegate<dyn Fn(T) -> bool>) -> Seq<T> {
        if source.is_null() || predicate.is_null() {
            throw(exc("System.ArgumentNullException", crate::s!("source")));
        }
        struct WhereEnum<T: Val> {
            inner: SeqEnum<T>,
            predicate: Delegate<dyn Fn(T) -> bool>,
        }
        impl<T: Val> Enumerator for WhereEnum<T> {
            type Item = T;
            fn move_next(&mut self) -> bool {
                while self.inner.move_next() {
                    let x = self.inner.current();
                    if self.predicate.invoke(|f| f(x)) {
                        return true;
                    }
                }
                false
            }
            fn current(&self) -> T {
                self.inner.current()
            }
        }
        Seq::lazy(move || Box::new(WhereEnum { inner: source.enumerator(), predicate }), move |m| unsafe {
            source.trace(m);
            predicate.trace(m);
        })
    }

    /// `source.Select(selector)`: lazy.
    pub fn select<T: Val, R: Val>(source: Seq<T>, selector: Delegate<dyn Fn(T) -> R>) -> Seq<R> {
        if source.is_null() || selector.is_null() {
            throw(exc("System.ArgumentNullException", crate::s!("source")));
        }
        struct SelectEnum<T: Val, R: Val> {
            inner: SeqEnum<T>,
            selector: Delegate<dyn Fn(T) -> R>,
            cur: R,
        }
        impl<T: Val, R: Val> Enumerator for SelectEnum<T, R> {
            type Item = R;
            fn move_next(&mut self) -> bool {
                if self.inner.move_next() {
                    let x = self.inner.current();
                    self.cur = self.selector.invoke(|f| f(x));
                    true
                } else {
                    false
                }
            }
            fn current(&self) -> R {
                self.cur
            }
        }
        // SAFETY: zero is a valid R (runtime invariant).
        Seq::lazy(move || Box::new(SelectEnum { inner: source.enumerator(), selector, cur: unsafe { core::mem::zeroed() } }), move |m| unsafe {
            source.trace(m);
            selector.trace(m);
        })
    }

    /// `first.Concat(second)`: lazy.
    pub fn concat<T: Val>(first: Seq<T>, second: Seq<T>) -> Seq<T> {
        if first.is_null() || second.is_null() {
            throw(exc("System.ArgumentNullException", crate::s!("first")));
        }
        struct ConcatEnum<T: Val> {
            a: SeqEnum<T>,
            b: Seq<T>,
            second: Option<SeqEnum<T>>,
        }
        impl<T: Val> Enumerator for ConcatEnum<T> {
            type Item = T;
            fn move_next(&mut self) -> bool {
                if self.second.is_none() {
                    if self.a.move_next() {
                        return true;
                    }
                    self.second = Some(self.b.enumerator());
                }
                self.second.as_mut().map(|e| e.move_next()).unwrap_or(false)
            }
            fn current(&self) -> T {
                match &self.second {
                    Some(e) => e.current(),
                    None => self.a.current(),
                }
            }
        }
        Seq::lazy(move || Box::new(ConcatEnum { a: first.enumerator(), b: second, second: None }), move |m| unsafe {
            first.trace(m);
            second.trace(m);
        })
    }

    /// `source.OrderBy(keySelector)` for keys with a default ordering (numbers, strings): a stable sort, as LINQ's is.
    /// Computed when called (.NET defers it until the result is enumerated).
    pub fn order_by<T: Val, K: crate::rt::sys::CsOrd>(source: Seq<T>, key_selector: Delegate<dyn Fn(T) -> K>) -> Seq<T> {
        if source.is_null() || key_selector.is_null() {
            throw(exc("System.ArgumentNullException", crate::s!("source")));
        }
        let mut keyed: Vec<(K, T)> = source.to_vec().into_iter().map(|x| (key_selector.invoke(|f| f(x)), x)).collect();
        keyed.sort_by(|a, b| a.0.cs_cmp(&b.0).cmp(&0));
        Seq::from_vec(keyed.into_iter().map(|(_, x)| x).collect())
    }

    /// `Enumerable.Range(start, count)`.
    pub fn range(start: i32, count: i32) -> Seq<i32> {
        if count < 0 || (start as i64 + count as i64 - 1) > i32::MAX as i64 {
            throw(exc("System.ArgumentOutOfRangeException", crate::s!("count")));
        }
        Seq::from_vec((0..count).map(|i| start + i).collect())
    }

    /// `source.ToArray()`.
    pub fn to_array<T: Val>(source: Seq<T>) -> Arr<T> {
        Arr::from_slice(&source.to_vec())
    }

    /// `source.ToList()`.
    pub fn to_list<T: Val>(source: Seq<T>) -> Gc<List<T>> {
        List::from_seq_gc(source)
    }

    /// `source.First()`: `InvalidOperationException` when empty.
    pub fn first<T: Val>(source: Seq<T>) -> T {
        let mut e = get_enumerator(source);
        if e.move_next() {
            e.current()
        } else {
            throw(exc("System.InvalidOperationException", crate::s!("Sequence contains no elements")))
        }
    }

    /// `source.Last()`: `InvalidOperationException` when empty.
    pub fn last<T: Val>(source: Seq<T>) -> T {
        match source.to_vec().last() {
            Some(v) => *v,
            None => throw(exc("System.InvalidOperationException", crate::s!("Sequence contains no elements"))),
        }
    }

    /// `source.Contains(value)` with the default equality comparer.
    pub fn contains<T: CsKey>(source: Seq<T>, value: T) -> bool {
        let mut e = get_enumerator(source);
        while e.move_next() {
            if e.current().cs_eq(&value) {
                return true;
            }
        }
        false
    }

    /// `source.Distinct()`: first occurrences, in order (computed when called; .NET defers it).
    pub fn distinct<T: CsKey>(source: Seq<T>) -> Seq<T> {
        let mut out: Vec<T> = Vec::new();
        let mut seen: std::collections::HashSet<KeyW<T>> = Default::default();
        for x in source.to_vec() {
            if seen.insert(KeyW(x)) {
                out.push(x);
            }
        }
        Seq::from_vec(out)
    }

    /// `source.Count()`.
    pub fn count<T: Val>(source: Seq<T>) -> i32 {
        let mut n = 0i32;
        let mut e = get_enumerator(source);
        while e.move_next() {
            n = n.wrapping_add(1);
        }
        n
    }
}

#[cfg(test)]
mod capacity_tests {
    use super::*;

    /// `List<T>.Capacity` as the .NET Framework reports it (reference source, List.cs: `EnsureCapacity`).
    #[test]
    fn list_capacity_follows_dotnet() {
        unsafe {
            let l = List::<i32>::new_gc();
            assert_eq!((*l.ptr()).capacity(), 0);
            (*l.ptr()).add(1);
            assert_eq!((*l.ptr()).capacity(), 4);
            for i in 0..4 {
                (*l.ptr()).add(i);
            }
            assert_eq!((*l.ptr()).capacity(), 8);
            (*l.ptr()).clear();
            assert_eq!((*l.ptr()).capacity(), 8);
            let two = List::<i32>::with_capacity_gc(2);
            assert_eq!((*two.ptr()).capacity(), 2);
            (*two.ptr()).add(1);
            (*two.ptr()).add(2);
            assert_eq!((*two.ptr()).capacity(), 2);
            (*two.ptr()).add(3);
            assert_eq!((*two.ptr()).capacity(), 4);
            let from = List::<i32>::from_seq_gc(Seq::from_arr(Arr::from_slice(&[1, 2, 3])));
            assert_eq!((*from.ptr()).capacity(), 3);
            (*from.ptr()).add_range(Seq::from_arr(Arr::from_slice(&[4, 5, 6, 7, 8])));
            assert_eq!((*from.ptr()).capacity(), 8);
            (*from.ptr()).set_capacity(20);
            assert_eq!((*from.ptr()).capacity(), 20);
            // RemoveRange / GetRange
            (*from.ptr()).remove_range(1, 3);
            assert_eq!((*from.ptr()).to_array().as_slice(), &[1, 5, 6, 7, 8]);
            assert_eq!((*(*from.ptr()).get_range(3, 2).ptr()).to_array().as_slice(), &[7, 8]);
            assert_eq!((*from.ptr()).capacity(), 20);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rt::try_;
    use crate::s;

    #[test]
    fn list_basics_and_alias() {
        let l = List::<i32>::new_gc();
        let alias = l;
        unsafe {
            (*l.ptr()).add(1);
            (*alias.ptr()).add(2);
            assert_eq!((*l.ptr()).count(), 2);
            assert_eq!((*l.ptr()).get_item(1), 2);
            assert!(try_(|| (*l.ptr()).get_item(2)).is_err());
            assert!((*l.ptr()).contains(2));
            assert!((*l.ptr()).remove(1));
            assert_eq!((*l.ptr()).index_of(2), 0);
        }
    }

    #[test]
    fn foreach_detects_modification() {
        let l = List::<i32>::new_gc();
        unsafe {
            (*l.ptr()).add(1);
            (*l.ptr()).add(2);
        }
        let r = try_(|| {
            let mut e = get_enumerator(l);
            while e.move_next() {
                unsafe { (*l.ptr()).add(9) };
            }
        });
        assert!(r.is_err());
        let mut sum = 0;
        let mut e = get_enumerator(Seq::from_list(l));
        while e.move_next() {
            sum += e.current();
        }
        assert_eq!(sum, 12);
        assert!(Seq::from_list(Gc::<List<i32>>::NULL).is_null());
        assert!(try_(|| get_enumerator(Gc::<List<i32>>::NULL).move_next()).is_err());
    }

    #[test]
    fn dictionary_order_matches_dotnet() {
        let d = Dictionary::<i32, Str>::new_gc();
        let keys = |d: Gc<Dictionary<i32, Str>>| {
            let mut v = Vec::new();
            let mut e = get_enumerator(d);
            while e.move_next() {
                v.push(e.current().key);
            }
            v
        };
        unsafe {
            for k in [5, 3, 9, 1] {
                (*d.ptr()).add(k, s!("x"));
            }
            assert_eq!(keys(d), [5, 3, 9, 1]);
            (*d.ptr()).remove(3);
            (*d.ptr()).remove(9);
            (*d.ptr()).add(7, s!("y")); // reuses the slot of 9 (last freed)
            (*d.ptr()).add(8, s!("y")); // then the slot of 3
            (*d.ptr()).add(2, s!("y")); // then appends
            assert_eq!(keys(d), [5, 8, 7, 1, 2]);
            assert!(try_(|| (*d.ptr()).add(5, s!("dup"))).is_err());
            (*d.ptr()).set_item(5, s!("z"));
            assert_eq!((*d.ptr()).get_item(5).as_str(), "z");
            assert!(try_(|| (*d.ptr()).get_item(100)).is_err());
            let mut out = s!("before");
            assert!(!(*d.ptr()).try_get_value(100, &mut out));
            assert!(out.is_null());
            assert_eq!((*d.ptr()).count(), 5);
        }
    }

    #[test]
    fn queue_is_fifo() {
        let q = Queue::<i32>::new_gc();
        unsafe {
            (*q.ptr()).enqueue(1);
            (*q.ptr()).enqueue(2);
            assert_eq!((*q.ptr()).count(), 2);
            assert_eq!((*q.ptr()).dequeue(), 1);
            assert_eq!((*q.ptr()).dequeue(), 2);
            assert!(try_(|| (*q.ptr()).dequeue()).is_err());
        }
    }

    #[test]
    fn string_keys_compare_by_content() {
        let d = Dictionary::<Str, i32>::new_gc();
        unsafe {
            (*d.ptr()).add(s!("ab"), 1);
            assert!((*d.ptr()).contains_key(crate::rt::concat(&[s!("a"), s!("b")])));
        }
    }

    #[test]
    fn linq_eager() {
        let a = Arr::from_slice(&[1, 2, 3, 4]);
        let even = Delegate::<dyn Fn(i32) -> bool>::new(Box::new(|x| x % 2 == 0));
        let three = Delegate::<dyn Fn(i32) -> bool>::new(Box::new(|x| x == 3));
        let none = Delegate::<dyn Fn(i32) -> bool>::new(Box::new(|x| x > 9));
        let s = Seq::from_arr(a);
        assert!(linq::any(s, even));
        assert!(!linq::any(s, none));
        assert_eq!(linq::single_or_default(s, three), 3);
        assert_eq!(linq::single_or_default(s, none), 0);
        assert!(try_(|| linq::single_or_default(s, even)).is_err());
        assert_eq!(linq::count(linq::empty::<i32>()), 0);
        assert_eq!(linq::first_or_default(s, even), 2);
    }
}
