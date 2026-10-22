//! `object`: what a handle of static type `object` / interface can be asked at run time.
//!
//! - strings as objects (`(object)"x"`, `o as string`),
//! - nullable boxing (`(object)(int?)x` is null or a boxed `int`),
//! - interface tests (`o is IFoo`) through the generated `implements` table of the interface,
//! - downcasts to runtime types (arrays, lists, delegates) checked with Rust's `TypeId`,
//! - `typeof(T)` objects, `enum.ToString()`.

use super::gc::{box_value, invalid_cast, is_boxed, null_reference, rt_type_of, tid, type_id_of, Delegate, DelegateData, Gc, Nullable, Obj, Rooted, Trace};
use super::string::Str;
use super::{Arr, Arr2};
use std::any::TypeId;
use std::cell::RefCell;
use std::collections::HashMap;

/// `(object)s`: a string object (null stays null).
pub fn str_obj(s: Str) -> Obj {
    if s.is_null() {
        Gc::NULL
    } else {
        box_value(s, tid::STRING)
    }
}

/// `(string)o`: null stays null, anything but a string throws `InvalidCastException`.
pub fn obj_str(o: Obj) -> Str {
    if o.is_null() {
        return Str::NULL;
    }
    if !is_boxed(o, tid::STRING) {
        invalid_cast();
    }
    // SAFETY: objects with the STRING id hold a `Str`.
    unsafe { *(o.0 as *const Str) }
}

/// `o as string`.
pub fn obj_as_str(o: Obj) -> Str {
    if is_boxed(o, tid::STRING) {
        // SAFETY: as above.
        unsafe { *(o.0 as *const Str) }
    } else {
        Str::NULL
    }
}

/// `(object)n` for a `T?`: null, or a boxed `T`.
pub fn box_nullable<T: Trace + Copy + 'static>(n: Nullable<T>, type_id: u32) -> Obj {
    if n.has_value {
        box_value(n.value, type_id)
    } else {
        Gc::NULL
    }
}

/// `(T?)o`: null gives null, a boxed `T` its value, anything else `InvalidCastException`.
///
/// # Safety
/// `type_id` must be the id values of type `T` are boxed with.
pub unsafe fn unbox_nullable<T: Copy>(o: Obj, type_id: u32) -> Nullable<T> {
    if o.is_null() {
        return core::mem::zeroed();
    }
    if type_id_of(o.0 as *const u8) != type_id {
        invalid_cast();
    }
    Nullable::some(*(o.0 as *const T))
}

/// `o is IFoo`: `implements` is the generated table of the interface (type id -> implements it).
pub fn is_iface(o: Obj, implements: fn(u32) -> bool) -> bool {
    // SAFETY: every non-null Gc comes from `alloc`.
    !o.is_null() && implements(unsafe { type_id_of(o.0 as *const u8) })
}

/// `o as IFoo`.
pub fn as_iface(o: Obj, implements: fn(u32) -> bool) -> Obj {
    if is_iface(o, implements) {
        o
    } else {
        Gc::NULL
    }
}

/// `(IFoo)o`.
pub fn cast_iface(o: Obj, implements: fn(u32) -> bool) -> Obj {
    if o.is_null() || is_iface(o, implements) {
        o
    } else {
        invalid_cast()
    }
}

/// A handle type of the runtime (array, collection, delegate) that an `object` can be cast back to.
pub trait RtHandle: Copy {
    /// The heap type behind the handle; its `TypeId` is what the cast checks.
    type Data: 'static;
    /// # Safety
    /// `o` must be null or point at a `Self::Data`.
    unsafe fn from_obj(o: Obj) -> Self;
}
impl<T: 'static> RtHandle for Arr<T> {
    type Data = super::arr::ArrData<T>;
    unsafe fn from_obj(o: Obj) -> Self {
        Arr::from_obj(o)
    }
}
impl<T: 'static> RtHandle for Arr2<T> {
    type Data = super::arr::Arr2Data<T>;
    unsafe fn from_obj(o: Obj) -> Self {
        Arr2::from_obj(o)
    }
}
impl<F: ?Sized + 'static> RtHandle for Delegate<F> {
    type Data = DelegateData<F>;
    unsafe fn from_obj(o: Obj) -> Self {
        Delegate::from_obj(o)
    }
}
/// Every class the runtime or a host crate implements natively (`List<T>`, `rt::io::Stream`, a type of
/// `terraria-host`): the object's Rust type is the class.
impl<T: 'static> RtHandle for Gc<T> {
    type Data = T;
    unsafe fn from_obj(o: Obj) -> Self {
        o.up()
    }
}

/// `o as H` where several C# classes share the Rust type behind `H` (`nativeTypes` entry with `"is"`): null
/// unless the object is that Rust type AND `is` accepts it.
pub fn as_rt_if<H: RtHandle>(o: Obj, is: impl FnOnce(H) -> bool) -> H {
    // SAFETY: as `as_rt`.
    unsafe {
        if !o.is_null() && rt_type_of(o.0 as *const u8) == TypeId::of::<H::Data>() && is(H::from_obj(o)) {
            H::from_obj(o)
        } else {
            H::from_obj(Gc::NULL)
        }
    }
}

/// `(H)o` with the same rule: null stays null, anything else that is not accepted throws `InvalidCastException`.
pub fn cast_rt_if<H: RtHandle>(o: Obj, is: impl FnOnce(H) -> bool) -> H {
    // SAFETY: as `as_rt`.
    unsafe {
        if o.is_null() {
            return H::from_obj(Gc::NULL);
        }
        if rt_type_of(o.0 as *const u8) != TypeId::of::<H::Data>() || !is(H::from_obj(o)) {
            invalid_cast();
        }
        H::from_obj(o)
    }
}

/// `o as H` for a runtime handle type: null unless `o` is exactly that type.
pub fn as_rt<H: RtHandle>(o: Obj) -> H {
    // SAFETY: the TypeId of the object's heap type was compared before the handle is retyped.
    unsafe {
        if !o.is_null() && rt_type_of(o.0 as *const u8) == TypeId::of::<H::Data>() {
            H::from_obj(o)
        } else {
            H::from_obj(Gc::NULL)
        }
    }
}

/// `(H)o` for a runtime handle type: null stays null, another type throws `InvalidCastException`.
/// Deviation: array covariance (`(object[])stringArray`) is not modelled; such a cast throws here.
pub fn cast_rt<H: RtHandle>(o: Obj) -> H {
    // SAFETY: as `as_rt`.
    unsafe {
        if o.is_null() {
            return H::from_obj(Gc::NULL);
        }
        if rt_type_of(o.0 as *const u8) != TypeId::of::<H::Data>() {
            invalid_cast();
        }
        H::from_obj(o)
    }
}

/// A `System.Type` object: one per type name, compared by identity like the CLR's.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct TypeObject {
    pub name: Str,
    /// [`type_flags`] bits.
    pub flags: u8,
    /// Boxing id of the type's values (0 when it has none): what `Convert.ChangeType` converts to.
    pub tid: u32,
}

/// What reflection on a [`TypeObject`] can answer.
pub mod type_flags {
    pub const PRIMITIVE: u8 = 1;
    pub const VALUE_TYPE: u8 = 2;
    pub const GENERIC: u8 = 4;
    pub const NULLABLE: u8 = 8;
    /// An enum: `tid` is the enum's own id (`EnumInfo`).
    pub const ENUM: u8 = 16;
    /// Flags are known (set by `typeof` sites; a name-only type object answers nothing).
    pub const KNOWN: u8 = 128;
}
unsafe impl Trace for TypeObject {
    const NEEDS_TRACE: bool = false;
    unsafe fn trace(&self, _m: &mut super::Marker) {}
}

thread_local! {
    static TYPES: RefCell<HashMap<&'static str, Rooted<Obj>>> = RefCell::new(HashMap::new());
}

/// `typeof(T)` for a type the translator knows: `full_name` is the C# full name, `flags` are [`type_flags`]
/// (without `KNOWN`), `type_id` the boxing id of its values or 0.
pub fn type_of(full_name: &'static str, flags: u8, type_id: u32) -> Obj {
    TYPES.with(|t| {
        t.borrow_mut()
            .entry(full_name)
            .or_insert_with(|| Rooted::new(super::gc::alloc_init_id(TypeObject { name: Str::lit(full_name), flags: flags | type_flags::KNOWN, tid: type_id }, tid::TYPE).up()))
            .get()
    })
}

/// `typeof(T)` for a type parameter: what the runtime knows about `T` (its boxing rule). The name is Rust's.
/// Deviation: two type parameters instantiated with the same C# type are the same `Type` here only if they are
/// the same Rust type, which they are; `typeof(T) == typeof(int)` against a `typeof` site compares by name and
/// is therefore false. The game only asks such a type for `IsPrimitive` / `IsClass` / nullable-ness.
pub fn type_of_val<T: super::Val>() -> Obj {
    use super::gc::BoxKind;
    let (flags, id) = match T::BOX {
        BoxKind::Value(id) if (tid::BOOL..=tid::F64).contains(&id) => (type_flags::PRIMITIVE | type_flags::VALUE_TYPE, id),
        BoxKind::Value(id) => (type_flags::VALUE_TYPE, id),
        BoxKind::Handle => (0, 0),
        BoxKind::Nullable => (type_flags::VALUE_TYPE | type_flags::GENERIC | type_flags::NULLABLE, 0),
        // string, sequences: reference types
        BoxKind::None => (0, 0),
    };
    type_of(core::any::type_name::<T>(), flags, id)
}

fn type_object(t: Obj) -> TypeObject {
    if t.is_null() {
        null_reference();
    }
    if !is_boxed(t, tid::TYPE) {
        invalid_cast();
    }
    // SAFETY: objects with the TYPE id hold a TypeObject.
    let o = unsafe { *(t.0 as *const TypeObject) };
    assert!(o.flags & type_flags::KNOWN != 0, "reflection on a Type object the runtime only knows by name");
    o
}

/// `type.IsPrimitive`.
pub fn type_is_primitive(t: Obj) -> bool {
    type_object(t).flags & type_flags::PRIMITIVE != 0
}
/// `type.IsValueType`.
pub fn type_is_value_type(t: Obj) -> bool {
    type_object(t).flags & type_flags::VALUE_TYPE != 0
}
/// `type.IsClass` (deviation: true for interfaces too, which the runtime does not tell apart).
pub fn type_is_class(t: Obj) -> bool {
    type_object(t).flags & type_flags::VALUE_TYPE == 0
}
/// `type.IsGenericType`.
pub fn type_is_generic(t: Obj) -> bool {
    type_object(t).flags & type_flags::GENERIC != 0
}
/// `type.GetGenericTypeDefinition()`: known for `Nullable<T>` only (`InvalidOperationException` for a
/// non-generic type, as in .NET; other generic types panic: not modelled).
pub fn type_generic_definition(t: Obj) -> Obj {
    let o = type_object(t);
    if o.flags & type_flags::NULLABLE != 0 {
        return type_of("System.Nullable`1", type_flags::VALUE_TYPE | type_flags::GENERIC, 0);
    }
    if o.flags & type_flags::GENERIC == 0 {
        super::throw(super::exc("System.InvalidOperationException", Str::lit("This operation is only valid on generic types.")));
    }
    panic!("cs2rs runtime: GetGenericTypeDefinition of {} is not modelled", o.name.as_str())
}

/// `Convert.ChangeType(value, type)` for the conversions `IConvertible` defines between the primitives:
/// the same type gives the same object; integers convert with a range check (`OverflowException`), to and from
/// floating point as `Convert.ToXxx` does (to an integer: round half to even).
pub fn convert_change_type(value: Obj, target: Obj) -> Obj {
    let t = type_object(target);
    if value.is_null() {
        if t.flags & type_flags::VALUE_TYPE != 0 {
            invalid_cast();
        }
        return value;
    }
    // SAFETY: non-null objects have a header.
    let have = unsafe { type_id_of(value.0 as *const u8) };
    if t.tid == have {
        return value;
    }
    // a boxed enum converts as its underlying integer (IConvertible of System.Enum)
    let have = enum_info(have).map(|i| i.underlying).unwrap_or(have);
    if t.tid == have || t.tid == 0 {
        if t.tid == 0 && t.flags & type_flags::VALUE_TYPE != 0 {
            panic!("cs2rs runtime: Convert.ChangeType to {} is not modelled", t.name.as_str());
        }
        return value;
    }
    // SAFETY: the id says which primitive the box holds.
    let as_f64: f64 = unsafe {
        match have {
            tid::BOOL => *(value.0 as *const bool) as u8 as f64,
            tid::U8 => *(value.0 as *const u8) as f64,
            tid::I8 => *(value.0 as *const i8) as f64,
            tid::I16 => *(value.0 as *const i16) as f64,
            tid::U16 | tid::CHAR => *(value.0 as *const u16) as f64,
            tid::I32 => *(value.0 as *const i32) as f64,
            tid::U32 => *(value.0 as *const u32) as f64,
            tid::F32 => *(value.0 as *const f32) as f64,
            tid::F64 => *(value.0 as *const f64),
            // 64-bit integers do not fit a double exactly: converted below
            tid::I64 | tid::U64 => 0.0,
            _ => invalid_cast(),
        }
    };
    let as_i128: Option<i128> = unsafe {
        match have {
            tid::I64 => Some(*(value.0 as *const i64) as i128),
            tid::U64 => Some(*(value.0 as *const u64) as i128),
            tid::F32 | tid::F64 => None,
            _ => Some(as_f64 as i128),
        }
    };
    let overflow = || -> ! { super::throw(super::exc("System.OverflowException", Str::lit("Value was either too large or too small."))) };
    let int = |lo: i128, hi: i128| -> i128 {
        let v = match as_i128 {
            Some(v) => v,
            None => {
                if as_f64.is_nan() {
                    overflow()
                }
                super::math::round(as_f64) as i128
            }
        };
        if v < lo || v > hi {
            overflow()
        }
        v
    };
    let float = || match as_i128 {
        Some(v) if matches!(have, tid::I64 | tid::U64) => v as f64,
        _ => as_f64,
    };
    match t.tid {
        tid::BOOL => box_value(float() != 0.0, tid::BOOL),
        tid::U8 => box_value(int(0, u8::MAX as i128) as u8, tid::U8),
        tid::I8 => box_value(int(i8::MIN as i128, i8::MAX as i128) as i8, tid::I8),
        tid::I16 => box_value(int(i16::MIN as i128, i16::MAX as i128) as i16, tid::I16),
        tid::U16 => box_value(int(0, u16::MAX as i128) as u16, tid::U16),
        tid::I32 => box_value(int(i32::MIN as i128, i32::MAX as i128) as i32, tid::I32),
        tid::U32 => box_value(int(0, u32::MAX as i128) as u32, tid::U32),
        tid::I64 => box_value(int(i64::MIN as i128, i64::MAX as i128) as i64, tid::I64),
        tid::U64 => box_value(int(0, u64::MAX as i128) as u64, tid::U64),
        tid::F32 => box_value(float() as f32, tid::F32),
        tid::F64 => box_value(float(), tid::F64),
        _ => invalid_cast(),
    }
}

/// `Type.FullName` of a `typeof` object.
pub fn type_full_name(t: Obj) -> Str {
    if t.is_null() {
        null_reference();
    }
    if !is_boxed(t, tid::TYPE) {
        invalid_cast();
    }
    // SAFETY: objects with the TYPE id hold a TypeObject.
    unsafe { (*(t.0 as *const TypeObject)).name }
}

/// What the runtime knows about one enum type (generated: `gen::enums`). A boxed enum value carries `tid` in its
/// header, so `o is E`, `(E)o`, `o.ToString()`, `Enum.GetName` see the enum and not its underlying integer.
pub struct EnumInfo {
    pub tid: u32,
    /// C# full name (`Terraria.Enums.NPCTargetType`).
    pub name: &'static str,
    /// (value as unsigned 64-bit pattern, name), sorted by value; equal values in declaration order.
    pub names: &'static [(u64, &'static str)],
    pub flags: bool,
    pub signed: bool,
    /// Boxing id of the underlying integer type (`tid::I32` ...).
    pub underlying: u32,
}

thread_local! {
    static ENUMS: RefCell<HashMap<u32, &'static EnumInfo>> = RefCell::new(HashMap::new());
}

/// Makes `info` known by its id (every boxing and every `typeof` of the enum does this).
pub fn enum_register(info: &'static EnumInfo) {
    ENUMS.with(|e| {
        e.borrow_mut().entry(info.tid).or_insert(info);
    })
}

/// The enum boxed values with header id `tid` belong to, if it is an enum id.
pub fn enum_info(tid: u32) -> Option<&'static EnumInfo> {
    ENUMS.with(|e| e.borrow().get(&tid).copied())
}

/// The boxing id of an enum; makes the enum known to the runtime on the way (so that a box with this id can
/// print its name).
#[inline]
pub fn enum_tid(info: &'static EnumInfo) -> u32 {
    enum_register(info);
    info.tid
}

/// `o.ToString()` as an operand of a string concatenation: null contributes the empty string.
pub fn obj_to_string_or_empty(o: Obj) -> Str {
    if o.is_null() {
        Str::lit("")
    } else {
        super::gc::obj_to_string(o)
    }
}

/// `(object)e` for an enum value `e` (`v` is its underlying integer).
pub fn box_enum<T: Trace + Copy + 'static>(v: T, info: &'static EnumInfo) -> Obj {
    enum_register(info);
    box_value(v, info.tid)
}

/// `typeof(E)` for an enum.
pub fn type_of_enum(info: &'static EnumInfo) -> Obj {
    enum_register(info);
    type_of(info.name, type_flags::VALUE_TYPE | type_flags::ENUM, info.tid)
}

/// `Enum.GetUnderlyingType(typeof(E))`: the `Type` of the enum's integer type (`ArgumentException` for a type that
/// is not an enum).
pub fn enum_get_underlying_type(t: Obj) -> Obj {
    let info = type_enum_info(t);
    let name = match info.underlying {
        tid::U8 => "System.Byte",
        tid::I8 => "System.SByte",
        tid::I16 => "System.Int16",
        tid::U16 => "System.UInt16",
        tid::I32 => "System.Int32",
        tid::U32 => "System.UInt32",
        tid::I64 => "System.Int64",
        tid::U64 => "System.UInt64",
        other => panic!("cs2rs runtime: enum {} with an underlying type id {other} that is not an integer", info.name),
    };
    type_of(name, type_flags::PRIMITIVE | type_flags::VALUE_TYPE, info.underlying)
}

/// The enum behind a `typeof(E)` object (`ArgumentException` when the type is not an enum, as `Enum.*` throws).
pub fn type_enum_info(t: Obj) -> &'static EnumInfo {
    let o = type_object(t);
    match if o.flags & type_flags::ENUM != 0 { enum_info(o.tid) } else { None } {
        Some(i) => i,
        None => super::throw(super::exc("System.ArgumentException", Str::lit("Type provided must be an Enum."))),
    }
}

/// Text of a boxed value of a runtime value type: the header id tells a `char` from a `ushort` and an enum from
/// its underlying integer.
pub trait BoxedText: Copy {
    fn text(self) -> Str;
    /// The value as an integer, for the enum rule; `None` for types that are no enum's underlying type.
    fn int(self) -> Option<i64> {
        None
    }
}
macro_rules! boxed_int {
    ($($t:ty),*) => {$( impl BoxedText for $t {
        fn text(self) -> Str { super::to_str(self) }
        fn int(self) -> Option<i64> { Some(self as i64) }
    } )*};
}
boxed_int!(i8, u8, i16, u16, i32, u32, i64, u64);
impl BoxedText for isize {
    fn text(self) -> Str {
        Str::new(&self.to_string())
    }
}
impl BoxedText for bool {
    fn text(self) -> Str {
        super::to_str(self)
    }
}
impl BoxedText for f32 {
    fn text(self) -> Str {
        super::to_str(self)
    }
}
impl BoxedText for f64 {
    fn text(self) -> Str {
        super::to_str(self)
    }
}
// XNA value types: `ToString()` as Microsoft.Xna.Framework writes it (`{X:1 Y:2}`; components with the current
// culture's `Single.ToString()`).
impl BoxedText for terraria_core::xna::Vector2 {
    fn text(self) -> Str {
        Str::new(&format!("{{X:{} Y:{}}}", super::string::ToStr::cs_to_string(self.x), super::string::ToStr::cs_to_string(self.y)))
    }
}
impl BoxedText for terraria_core::xna::Vector3 {
    fn text(self) -> Str {
        use super::string::ToStr;
        Str::new(&format!("{{X:{} Y:{} Z:{}}}", self.x.cs_to_string(), self.y.cs_to_string(), self.z.cs_to_string()))
    }
}
impl BoxedText for terraria_core::xna::Vector4 {
    fn text(self) -> Str {
        use super::string::ToStr;
        Str::new(&format!("{{X:{} Y:{} Z:{} W:{}}}", self.x.cs_to_string(), self.y.cs_to_string(), self.z.cs_to_string(), self.w.cs_to_string()))
    }
}
impl BoxedText for terraria_core::xna::Point {
    fn text(self) -> Str {
        Str::new(&format!("{{X:{} Y:{}}}", self.x, self.y))
    }
}
impl BoxedText for terraria_core::xna::Rectangle {
    fn text(self) -> Str {
        Str::new(&format!("{{X:{} Y:{} Width:{} Height:{}}}", self.x, self.y, self.width, self.height))
    }
}
impl BoxedText for terraria_core::xna::Color {
    fn text(self) -> Str {
        Str::new(&format!("{{R:{} G:{} B:{} A:{}}}", self.r(), self.g(), self.b(), self.a()))
    }
}
impl BoxedText for terraria_core::xna::Matrix {
    fn text(self) -> Str {
        use super::string::ToStr;
        let m = self;
        let row = |a: f32, b: f32, c: f32, d: f32, n: u8| format!("{{M{n}1:{} M{n}2:{} M{n}3:{} M{n}4:{}}}", a.cs_to_string(), b.cs_to_string(), c.cs_to_string(), d.cs_to_string());
        Str::new(&format!(
            "{{ {} {} {} {} }}",
            row(m.m11, m.m12, m.m13, m.m14, 1),
            row(m.m21, m.m22, m.m23, m.m24, 2),
            row(m.m31, m.m32, m.m33, m.m34, 3),
            row(m.m41, m.m42, m.m43, m.m44, 4)
        ))
    }
}

/// `ToString()` of a boxed runtime value whose header id is `type_id`.
pub fn boxed_to_string<T: BoxedText>(type_id: u32, v: &T) -> Str {
    if type_id == tid::CHAR {
        if let Some(c) = v.int() {
            return super::char_str(c as u16);
        }
    }
    if type_id >= tid::FIRST_GENERATED {
        if let (Some(info), Some(n)) = (enum_info(type_id), v.int()) {
            return enum_to_str(n, info.names, info.flags, info.signed);
        }
    }
    v.text()
}

/// `Type.Name` of a `typeof` object: the last segment of the full name.
pub fn type_name(t: Obj) -> Str {
    let full = type_full_name(t).as_str();
    let cut = full.find(|c| c == '<' || c == '[').unwrap_or(full.len());
    let head = &full[..cut];
    let start = head.rfind(|c| c == '.' || c == '+').map(|i| i + 1).unwrap_or(0);
    Str::new(&full[start..])
}

/// `Enum.ToString()` as .NET Framework does it. `names` holds (value as unsigned 64-bit pattern, name) sorted by
/// value. Not `[Flags]`: the name of the value, else the number. `[Flags]`: the names of the set bits joined
/// with ", " (`Enum.InternalFlagsFormat`), else the number.
/// Deviation: for an enum with two names for one value .NET's choice is an artefact of its binary search; this
/// returns the first declared.
pub fn enum_to_str(v: i64, names: &[(u64, &'static str)], flags: bool, signed: bool) -> Str {
    let value = v as u64;
    let number = || if signed { Str::new(&v.to_string()) } else { Str::new(&value.to_string()) };
    if !flags {
        return match names.iter().find(|(k, _)| *k == value) {
            Some((_, n)) => Str::lit(n),
            None => number(),
        };
    }
    let mut result = value;
    let mut parts: Vec<&'static str> = Vec::new();
    let mut index = names.len();
    while index > 0 {
        index -= 1;
        let (k, n) = names[index];
        if index == 0 && k == 0 {
            break;
        }
        if result & k == k {
            result -= k;
            parts.insert(0, n);
        }
    }
    if result != 0 {
        return number();
    }
    if value == 0 {
        return match names.first() {
            Some((0, n)) => Str::lit(n),
            _ => Str::lit("0"),
        };
    }
    Str::new(&parts.join(", "))
}

// ---------------------------------------------------------------------------------------------- reflection

/// One field of a translated type as reflection sees it (generated: `gen::meta`).
pub struct FieldMeta {
    pub name: &'static str,
    pub public: bool,
    /// `const` (`FieldInfo.IsLiteral`).
    pub literal: bool,
    /// `readonly` (`FieldInfo.IsInitOnly`).
    pub init_only: bool,
    pub is_static: bool,
    /// `typeof` of the field's type (the same object a `typeof` expression of that type gives).
    pub field_type: unsafe fn() -> Obj,
    /// The value, boxed (`FieldInfo.GetValue(obj)`). `obj` is the instance (a class object, or a boxed struct);
    /// ignored for a static field, whose access runs the class's static initialiser like any static access.
    pub get: unsafe fn(Obj) -> Obj,
    /// `FieldInfo.SetValue(obj, value)`; `None` for constants and for fields whose type cannot be unboxed.
    pub set: Option<unsafe fn(Obj, Obj)>,
}

/// One property of a translated type (generated: `gen::meta`).
pub struct PropMeta {
    pub name: &'static str,
    /// The property has a public getter (`GetGetMethod(false) != null`).
    pub public: bool,
    pub is_static: bool,
    /// `typeof` of the property's type.
    pub prop_type: unsafe fn() -> Obj,
    /// `PropertyInfo.GetValue(obj, null)`: runs the translated getter (virtual dispatch as a call would).
    /// `None`: no getter, an indexer, or a getter the generator did not emit.
    pub get: Option<unsafe fn(Obj) -> Obj>,
}

/// One method of a translated type (generated: `gen::meta`, for the types the config's `reflectMethods` selects).
pub struct MethodMeta {
    pub name: &'static str,
    pub public: bool,
    pub is_static: bool,
    /// Fresh instances of the custom attributes declared on the method whose classes are translated.
    pub attrs: Option<unsafe fn() -> Vec<Obj>>,
    /// A delegate of the method's own signature bound to `target` (`Delegate.CreateDelegate(type, target, method)`);
    /// `None` for a method whose signature has no delegate form here (generic, `ref` parameters).
    pub bind: Option<unsafe fn(Obj) -> Obj>,
}

/// What reflection can ask a translated type. `fields` always holds the STATIC fields and constants in declaration
/// order. Instance fields and properties are present only when `members` is true (types selected by the config's
/// `reflect` and what they reach); otherwise asking for them is loud, never an empty answer.
pub struct TypeMeta {
    /// C# full name, nested types with `.` (`Terraria.ID.NPCID.Sets`).
    pub name: &'static str,
    /// CLR full name, nested types with `+` (`Type.FullName`).
    pub clr_name: &'static str,
    pub flags: u8,
    /// Boxing id of the type's values (value types), 0 otherwise.
    pub tid: u32,
    /// Type id in the header of instances (classes and boxed structs), 0 for static classes and interfaces.
    pub class_id: u32,
    pub fields: &'static [FieldMeta],
    /// The type declares instance fields (`fields` lists them only when `members`).
    pub instance_fields: bool,
    pub members: bool,
    pub props: &'static [PropMeta],
    /// Fresh instances of the custom attributes declared on the type whose classes are translated, in
    /// declaration order (`GetCustomAttributes(false)`); attributes of library classes are not modelled.
    pub attrs: Option<unsafe fn() -> Vec<Obj>>,
    /// Reflection data of the base class when it is a translated type with data.
    pub base: Option<&'static TypeMeta>,
    /// The ordinary methods the type declares, in declaration order; `None` when they are not generated
    /// (`reflectMethods` in the translator config): asking is then loud, never an empty answer.
    pub methods: Option<&'static [MethodMeta]>,
}

thread_local! {
    static METAS: RefCell<HashMap<&'static str, &'static TypeMeta>> = RefCell::new(HashMap::new());
    static METAS_BY_ID: RefCell<HashMap<u32, &'static TypeMeta>> = RefCell::new(HashMap::new());
}

/// Makes reflection data known to the runtime (by name and by the type id of instances).
pub fn meta_register(meta: &'static TypeMeta) {
    METAS.with(|m| {
        m.borrow_mut().entry(meta.name).or_insert(meta);
    });
    if meta.class_id != 0 {
        METAS_BY_ID.with(|m| {
            m.borrow_mut().entry(meta.class_id).or_insert(meta);
        });
    }
}

/// `typeof(T)` for a translated type with reflection data.
pub fn type_of_meta(meta: &'static TypeMeta) -> Obj {
    meta_register(meta);
    type_of(meta.name, meta.flags, meta.tid)
}

thread_local! {
    /// Parameterless constructors by type name (`gen::meta::CTORS`): public, constructor.
    static CTORS: RefCell<HashMap<&'static str, (bool, unsafe fn() -> Obj)>> = RefCell::new(HashMap::new());
}

/// Makes the parameterless constructor of a translated class known (`Activator.CreateInstance(type, nonPublic)`).
pub fn ctor_register(name: &'static str, public: bool, ctor: unsafe fn() -> Obj) {
    CTORS.with(|c| {
        c.borrow_mut().entry(name).or_insert((public, ctor));
    });
}

/// `Activator.CreateInstance(type, nonPublic)`: a new instance of a translated class with reflection data through its
/// parameterless constructor (a non-public one only when `non_public`, as the CLR). Any other type (no data, no
/// such constructor, a library type) is a host member that is not implemented.
pub fn activator_create_instance(t: Obj, non_public: bool) -> Obj {
    let o = type_object(t);
    let name = o.name.as_str();
    match CTORS.with(|c| c.borrow().get(name).copied()) {
        // SAFETY: a generated constructor (`gen::meta`).
        Some((public, ctor)) if public || non_public => unsafe { ctor() },
        _ => crate::host::unimplemented_value(Box::leak(format!("System.Activator.CreateInstance(Type, bool) of {name} (no registered parameterless constructor{})", if non_public { "" } else { " that is public" }).into_boxed_str())),
    }
}

/// Reflection data of the runtime type of `o` (`o.GetType()`), when the type is registered.
pub fn meta_of_obj(o: Obj) -> Option<&'static TypeMeta> {
    if o.is_null() {
        return None;
    }
    // SAFETY: non-null objects have a header.
    let id = unsafe { type_id_of(o.0 as *const u8) };
    METAS_BY_ID.with(|m| m.borrow().get(&id).copied())
}

/// Reflection data behind a `Type` object, when the type has any.
pub fn meta_of_type(t: Obj) -> Option<&'static TypeMeta> {
    let o = type_object(t);
    METAS.with(|m| m.borrow().get(o.name.as_str()).copied())
}

/// What a `Type` object says about itself: (full name, [`type_flags`], boxing id).
pub fn type_info(t: Obj) -> (Str, u8, u32) {
    let o = type_object(t);
    (o.name, o.flags, o.tid)
}

/// `System.Reflection.FieldInfo` of a field of a translated type.
#[derive(Clone, Copy)]
pub struct FieldInfo {
    pub meta: &'static FieldMeta,
}
unsafe impl Trace for FieldInfo {
    const NEEDS_TRACE: bool = false;
    unsafe fn trace(&self, _m: &mut super::Marker) {}
}

// ---------------------------------------------------------------------------------------------- anonymous types

/// An instance of a C# anonymous type (`new { Count = n, Name = s }`): the property names in declaration order
/// and the values, boxed. Immutable, compared by value (`Equals` / `GetHashCode` of every property), printed as
/// `{ Count = 3, Name = x }`, as the compiler-generated class does.
pub struct AnonObj {
    pub names: &'static [&'static str],
    pub values: Vec<Obj>,
}
unsafe impl Trace for AnonObj {
    unsafe fn trace(&self, m: &mut super::Marker) {
        for v in &self.values {
            m.mark(*v);
        }
    }
    unsafe fn obj_equals(this: *const Self, other: Obj) -> bool {
        if !super::gc::obj_is::<AnonObj>(other) {
            return false;
        }
        let (a, b) = (&*this, &*(other.0 as *const AnonObj));
        a.names.as_ptr() == b.names.as_ptr() && a.values.len() == b.values.len() && a.values.iter().zip(&b.values).all(|(x, y)| super::gc::object_equals(*x, *y))
    }
    unsafe fn obj_hash(this: *const Self) -> i32 {
        // the compiler's formula: a seed per type, then `hash * -1521134295 + EqualityComparer<T>.Default.GetHashCode(value)`
        let mut h: i32 = super::legacy_string_hash(&(*this).names.join(","));
        for v in &(*this).values {
            h = h.wrapping_mul(-1521134295).wrapping_add(if v.is_null() { 0 } else { super::gc::obj_hash_code(*v) });
        }
        h
    }
    unsafe fn obj_to_string(this: *const Self) -> Str {
        let a = &*this;
        let parts: Vec<String> = a.names.iter().zip(&a.values).map(|(n, v)| format!("{n} = {}", obj_to_string_or_empty(*v).as_str())).collect();
        Str::new(&if parts.is_empty() { "{ }".to_owned() } else { format!("{{ {} }}", parts.join(", ")) })
    }
}

/// `new { .. }`
pub fn anon_new(names: &'static [&'static str], values: &[Obj]) -> Obj {
    assert!(names.len() == values.len());
    super::alloc_init(AnonObj { names, values: values.to_vec() }).up()
}
fn anon<'a>(o: Obj) -> &'a AnonObj {
    if o.is_null() {
        null_reference();
    }
    assert!(super::gc::obj_is::<AnonObj>(o), "rt::anon_get on an object that is not an anonymous type instance");
    // SAFETY: type checked.
    unsafe { &*(o.0 as *const AnonObj) }
}
/// Property `index` of an anonymous object (boxed).
pub fn anon_get(o: Obj, index: usize) -> Obj {
    anon(o).values[index]
}

thread_local! {
    /// `Type` name of an anonymous type -> its property names.
    static ANON_TYPES: RefCell<HashMap<&'static str, &'static [&'static str]>> = RefCell::new(HashMap::new());
}
fn anon_type(names: &'static [&'static str]) -> Obj {
    let name = ANON_TYPES.with(|t| {
        let mut t = t.borrow_mut();
        if let Some((k, _)) = t.iter().find(|(_, v)| v.as_ptr() == names.as_ptr() && v.len() == names.len()) {
            return *k;
        }
        let k: &'static str = Box::leak(format!("<>f__AnonymousType<{}>", names.join(",")).into_boxed_str());
        t.insert(k, names);
        k
    });
    type_of(name, 0, 0)
}

// ---------------------------------------------------------------------------------------------- TypeDescriptor

/// `PropertyDescriptorCollection`: the properties of one type (an anonymous type's, or a translated type's with
/// generated members).
#[derive(Clone, Copy)]
pub struct PropColl {
    anon: Option<&'static [&'static str]>,
    meta: Option<&'static TypeMeta>,
}
unsafe impl Trace for PropColl {
    const NEEDS_TRACE: bool = false;
    unsafe fn trace(&self, _m: &mut super::Marker) {}
}
/// `PropertyDescriptor`
#[derive(Clone, Copy)]
pub struct PropDesc {
    pub name: &'static str,
    anon_index: i32,
    prop: Option<&'static PropMeta>,
}
unsafe impl Trace for PropDesc {
    const NEEDS_TRACE: bool = false;
    unsafe fn trace(&self, _m: &mut super::Marker) {}
}

/// `TypeDescriptor.GetProperties(Type)`: the public instance properties. Served for anonymous types and for
/// translated types whose members are generated (`reflect`); any other type is a host member that is not implemented.
pub fn type_descriptor_get_properties_of_type(t: Obj) -> Gc<PropColl> {
    let o = type_object(t);
    let name = o.name.as_str();
    if let Some(names) = ANON_TYPES.with(|a| a.borrow().get(name).copied()) {
        return super::alloc_init(PropColl { anon: Some(names), meta: None });
    }
    match METAS.with(|m| m.borrow().get(name).copied()) {
        Some(meta) if meta.members => super::alloc_init(PropColl { anon: None, meta: Some(meta) }),
        _ => crate::host::unimplemented_value(no_meta("System.ComponentModel.TypeDescriptor.GetProperties", name)),
    }
}
/// `TypeDescriptor.GetProperties(object)`
pub fn type_descriptor_get_properties(o: Obj) -> Gc<PropColl> {
    type_descriptor_get_properties_of_type(obj_get_type(o))
}
/// `collection.Find(name, ignoreCase)` / `collection[name]`: null when there is no such property.
pub fn prop_coll_find(c: Gc<PropColl>, name: Str, ignore_case: bool) -> Gc<PropDesc> {
    // SAFETY: `ptr()` checks null.
    let c = unsafe { *c.ptr() };
    let want = name.as_str();
    let same = |n: &str| if ignore_case { n.eq_ignore_ascii_case(want) } else { n == want };
    if let Some(names) = c.anon {
        return match names.iter().position(|n| same(n)) {
            Some(i) => super::alloc_init(PropDesc { name: names[i], anon_index: i as i32, prop: None }),
            None => Gc::NULL,
        };
    }
    let mut level = c.meta;
    while let Some(mt) = level {
        if let Some(p) = mt.props.iter().find(|p| p.public && !p.is_static && same(p.name)) {
            return super::alloc_init(PropDesc { name: p.name, anon_index: -1, prop: Some(p) });
        }
        level = mt.base;
    }
    Gc::NULL
}
impl PropDesc {
    /// `descriptor.GetValue(component)`
    pub fn get_value(&self, component: Obj) -> Obj {
        if self.anon_index >= 0 {
            return anon_get(component, self.anon_index as usize);
        }
        match self.prop.and_then(|p| p.get) {
            // SAFETY: generated getter thunk; the component is an instance of the type the descriptor came from.
            Some(get) => unsafe { get(component) },
            None => crate::host::unimplemented_value("System.ComponentModel.PropertyDescriptor.GetValue for a property without a generated getter"),
        }
    }
    pub fn name(&self) -> Str {
        Str::lit(self.name)
    }
}

/// `System.Reflection.MethodInfo` of a method of a translated type.
#[derive(Clone, Copy)]
pub struct MethodInfo {
    pub meta: &'static MethodMeta,
}
unsafe impl Trace for MethodInfo {
    const NEEDS_TRACE: bool = false;
    unsafe fn trace(&self, _m: &mut super::Marker) {}
}

/// `o.GetType()`: the `Type` of a translated class instance (or boxed translated struct) that has reflection data.
/// The data of every type must be registered (`gen::meta::register_all()`, the driver's job at start-up). Any
/// other object (runtime containers, boxed primitives, host objects) is a host member that is not implemented.
pub fn obj_get_type(o: Obj) -> Obj {
    if o.is_null() {
        null_reference();
    }
    if super::gc::obj_is::<AnonObj>(o) {
        return anon_type(anon(o).names);
    }
    match meta_of_obj(o) {
        Some(m) => type_of_meta(m),
        None => crate::host::unimplemented_value(Box::leak(
            format!("System.Object.GetType() of an object without registered reflection data (type id {}, Rust type {})", unsafe { type_id_of(o.0 as *const u8) }, super::gc::obj_to_string(o).as_str()).into_boxed_str(),
        )),
    }
}

/// `type.GetMethods(flags)`: the methods the type declares, in declaration order (the CLR's order for one class).
/// Only for types whose method data is generated; without `DeclaredOnly` the base classes follow, most derived
/// first, and every one of them needs data too (never an incomplete list).
pub fn type_get_methods(t: Obj, flags: i32) -> Arr<Gc<MethodInfo>> {
    let o = type_object(t);
    let Some(meta) = METAS.with(|m| m.borrow().get(o.name.as_str()).copied()) else {
        return crate::host::unimplemented_value(no_meta("System.Type.GetMethods(BindingFlags)", o.name.as_str()));
    };
    let mut out: Vec<Gc<MethodInfo>> = Vec::new();
    let mut level = Some(meta);
    while let Some(mt) = level {
        let Some(methods) = mt.methods else {
            return crate::host::unimplemented_value(Box::leak(format!("System.Type.GetMethods(BindingFlags) of {}: methods are not generated (\"reflectMethods\" in the translator config)", mt.name).into_boxed_str()));
        };
        for m in methods {
            let scope = if m.is_static { binding_flags::STATIC } else { binding_flags::INSTANCE };
            let vis = if m.public { binding_flags::PUBLIC } else { binding_flags::NON_PUBLIC };
            if flags & scope != 0 && flags & vis != 0 {
                out.push(super::alloc_init(MethodInfo { meta: m }));
            }
        }
        if flags & binding_flags::DECLARED_ONLY != 0 {
            break;
        }
        level = mt.base;
    }
    Arr::from_slice(&out)
}

/// `Attribute.GetCustomAttribute(member, attributeType)`: the attribute of that type on the member, or null
/// (`AmbiguousMatchException` for more than one).
pub fn attribute_get_custom_attribute<T>(member: Gc<T>, attribute_type: Obj, parents: &[u32]) -> Obj {
    let all = member_get_custom_attributes_of(member, attribute_type, true, parents);
    match all.len() {
        0 => Gc::NULL,
        1 => unsafe { *all.at(0) },
        _ => super::throw(super::exc("System.Reflection.AmbiguousMatchException", Str::lit("Multiple custom attributes of the same type found."))),
    }
}

/// `Delegate.CreateDelegate(type, firstArgument, method, throwOnBindFailure)`: the delegate of the method's own
/// signature bound to the target. The caller's cast to its delegate type is the signature check (`rt::cast_rt` /
/// `as_rt` compare the Rust delegate type), which is where a mismatch surfaces: C# returns null / throws here.
pub fn delegate_create<T>(_delegate_type: Obj, target: Gc<T>, method: Gc<MethodInfo>, _throw_on_bind_failure: bool) -> Obj {
    // SAFETY: MethodInfo objects are made by `type_get_methods`.
    let meta = unsafe { (*method.ptr()).meta };
    match meta.bind {
        // SAFETY: generated thunk; the target's type is the declaring type (the method came from its own Type).
        Some(bind) => unsafe { bind(target.up()) },
        None => crate::host::unimplemented_value(Box::leak(format!("System.Delegate.CreateDelegate for the method {} (its signature has no delegate form in the generated reflection data)", meta.name).into_boxed_str())),
    }
}

/// `type.IsEnum`
pub fn type_is_enum(t: Obj) -> bool {
    type_object(t).flags & type_flags::ENUM != 0
}

/// `BindingFlags` bits.
pub mod binding_flags {
    pub const DECLARED_ONLY: i32 = 2;
    pub const INSTANCE: i32 = 4;
    pub const STATIC: i32 = 8;
    pub const PUBLIC: i32 = 16;
    pub const NON_PUBLIC: i32 = 32;
    /// `Type.GetFields()` without flags.
    pub const DEFAULT: i32 = INSTANCE | STATIC | PUBLIC;
}

fn no_meta(what: &str, t: &str) -> &'static str {
    Box::leak(format!("{what} of {t} (no reflection data: add the type to \"reflect\" in the translator config)").into_boxed_str())
}

/// `type.GetFields(flags)` in the CLR's order (see below). A type the runtime has no data for, or a request that
/// would include instance fields of a type whose instance members are not generated, is a host member that is not
/// implemented (never an incomplete list). Deviation: inherited public fields of base classes are listed after the
/// type's own, base by base (the CLR's order for inherited fields is the same: most derived first).
pub fn type_get_fields(t: Obj, flags: i32) -> Arr<Gc<FieldInfo>> {
    let o = type_object(t);
    let meta = METAS.with(|m| m.borrow().get(o.name.as_str()).copied());
    let Some(meta) = meta else {
        return crate::host::unimplemented_value(no_meta("System.Type.GetFields()", o.name.as_str()));
    };
    if flags & binding_flags::INSTANCE != 0 && meta.instance_fields && !meta.members {
        return crate::host::unimplemented_value(Box::leak(format!("System.Type.GetFields() of {}: instance fields are not generated (\"reflect\" in the translator config)", meta.name).into_boxed_str()));
    }
    let mut out: Vec<Gc<FieldInfo>> = Vec::new();
    let mut level = Some(meta);
    let mut own = true;
    while let Some(mt) = level {
        // The CLR lists the fields that have storage first and the constants (metadata only) after them, each
        // group in declaration order (observed on .NET 8 by the snippet `Equality`; the mechanism, fields from the
        // method table and then literals from metadata, is the same in .NET Framework).
        for literal in [false, true] {
            for f in mt.fields.iter().filter(|f| f.literal == literal) {
                let scope = if f.is_static { binding_flags::STATIC } else { binding_flags::INSTANCE };
                let vis = if f.public { binding_flags::PUBLIC } else { binding_flags::NON_PUBLIC };
                // inherited: instance fields that are not private (statics need FlattenHierarchy, which the game never passes)
                if flags & scope != 0 && flags & vis != 0 && (own || (!f.is_static && f.public)) {
                    out.push(super::alloc_init(FieldInfo { meta: f }));
                }
            }
        }
        if flags & binding_flags::DECLARED_ONLY != 0 || flags & binding_flags::INSTANCE == 0 {
            break;
        }
        level = mt.base;
        own = false;
    }
    Arr::from_slice(&out)
}

/// `type.GetField(name)` (public, static or instance): null when there is none.
pub fn type_get_field(t: Obj, name: Str) -> Gc<FieldInfo> {
    let o = type_object(t);
    let Some(meta) = METAS.with(|m| m.borrow().get(o.name.as_str()).copied()) else {
        return crate::host::unimplemented_value(no_meta("System.Type.GetField(string)", o.name.as_str()));
    };
    let want = name.as_str();
    let mut level = Some(meta);
    while let Some(mt) = level {
        if let Some(f) = mt.fields.iter().find(|f| f.public && f.name == want) {
            return super::alloc_init(FieldInfo { meta: f });
        }
        if mt.instance_fields && !mt.members {
            return crate::host::unimplemented_value(Box::leak(format!("System.Type.GetField(string) of {}: instance fields are not generated", mt.name).into_boxed_str()));
        }
        level = mt.base;
    }
    Gc::NULL
}

/// `type.GetCustomAttributes(inherit)`: fresh attribute objects, as the CLR creates them on every call.
/// Only attributes whose class is translated exist here; the game never asks for a library attribute this way
/// (the call sites are listed in DESIGN.md, "Reflection").
pub fn type_get_custom_attributes(t: Obj, _inherit: bool) -> Arr<Obj> {
    let o = type_object(t);
    let Some(meta) = METAS.with(|m| m.borrow().get(o.name.as_str()).copied()) else {
        return crate::host::unimplemented_value(no_meta("System.Reflection.MemberInfo.GetCustomAttributes(bool)", o.name.as_str()));
    };
    match meta.attrs {
        // SAFETY: generated constructor calls.
        Some(f) => Arr::from_slice(&unsafe { f() }),
        None => Arr::from_slice(&[]),
    }
}

/// `member.GetCustomAttributes(inherit)` for the kinds of `MemberInfo` the runtime has: a `Type` (its attributes)
/// and a `FieldInfo` (no translated attribute is declared on a field the game reflects over: checked by the
/// generator, which refuses to emit data for such a field).
pub fn member_get_custom_attributes<T>(m: Gc<T>, inherit: bool) -> Arr<Obj> {
    let m: Obj = m.up();
    if m.is_null() {
        null_reference();
    }
    if is_boxed(m, tid::TYPE) {
        return type_get_custom_attributes(m, inherit);
    }
    if super::gc::obj_is::<MethodInfo>(m) {
        // SAFETY: type checked.
        return match unsafe { (*(m.0 as *const MethodInfo)).meta.attrs } {
            // SAFETY: generated constructor calls.
            Some(f) => Arr::from_slice(&unsafe { f() }),
            None => Arr::from_slice(&[]),
        };
    }
    let _: Gc<FieldInfo> = cast_rt(m);
    Arr::from_slice(&[])
}

/// `member.GetCustomAttributes(attributeType, inherit)`: the attributes that are instances of `attribute_type`
/// (`parents` is `gen::types::PARENT`). An attribute type without reflection data (a library attribute) is a
/// host member that is not implemented: library attributes are not modelled, so the answer would be a guess.
pub fn member_get_custom_attributes_of<T>(m: Gc<T>, attribute_type: Obj, inherit: bool, parents: &[u32]) -> Arr<Obj> {
    let Some(at) = meta_of_type(attribute_type) else {
        return crate::host::unimplemented_value(no_meta("System.Reflection.MemberInfo.GetCustomAttributes(Type, bool)", type_full_name(attribute_type).as_str()));
    };
    let all = member_get_custom_attributes(m, inherit);
    let keep: Vec<Obj> = all.as_slice().iter().copied().filter(|a| super::is_instance(*a, at.class_id, parents)).collect();
    Arr::from_slice(&keep)
}

/// `member.Name` for the two kinds of `MemberInfo` the runtime has: a `Type` and a `FieldInfo`.
pub fn member_name(m: Obj) -> Str {
    if m.is_null() {
        null_reference();
    }
    if is_boxed(m, tid::TYPE) {
        return type_name(m);
    }
    if super::gc::obj_is::<MethodInfo>(m) {
        // SAFETY: type checked.
        return Str::lit(unsafe { (*(m.0 as *const MethodInfo)).meta.name });
    }
    let f: Gc<FieldInfo> = cast_rt(m);
    // SAFETY: checked cast.
    Str::lit(unsafe { (*f.ptr()).meta.name })
}

impl FieldInfo {
    pub fn field_type(&self) -> Obj {
        // SAFETY: generated getter.
        unsafe { (self.meta.field_type)() }
    }
    /// `GetValue(obj)`: `obj` is ignored for a static field, as in .NET; an instance field of null throws
    /// `TargetException` in .NET, `NullReferenceException` here.
    pub fn get_value(&self, obj: Obj) -> Obj {
        if !self.meta.is_static && obj.is_null() {
            null_reference();
        }
        // SAFETY: generated getter.
        unsafe { (self.meta.get)(obj) }
    }
    /// `SetValue(obj, value)`.
    pub fn set_value(&self, obj: Obj, value: Obj) {
        if !self.meta.is_static && obj.is_null() {
            null_reference();
        }
        match self.meta.set {
            // SAFETY: generated setter.
            Some(f) => unsafe { f(obj, value) },
            None => crate::host::unimplemented(Box::leak(format!("System.Reflection.FieldInfo.SetValue of {} (no setter generated)", self.meta.name).into_boxed_str())),
        }
    }
    pub fn is_literal(&self) -> bool {
        self.meta.literal
    }
    pub fn is_init_only(&self) -> bool {
        self.meta.init_only
    }
    pub fn is_public(&self) -> bool {
        self.meta.public
    }
    pub fn is_static(&self) -> bool {
        self.meta.is_static
    }
}

/// What a runtime container says about its contents to code that only holds an `object` (`foreach (object e in
/// (IEnumerable)o)`, `IDictionary`, `Array.GetValue`, `Delegate.GetInvocationList`): [`obj_reflect`].
pub enum Reflect {
    /// Not a runtime container.
    Other,
    /// `T[]`, `List<T>`, `Queue<T>`, `Stack<T>`, `HashSet<T>`: the elements, boxed, in enumeration order.
    /// `kind` is the C# type name without arguments (`"Array"`, `"List"`, `"HashSet"` ...).
    Seq { kind: &'static str, items: Vec<Obj> },
    /// `T[,]` / `T[,,]`: the dimensions and the elements, boxed, in row-major order.
    Multi { dims: Vec<usize>, items: Vec<Obj> },
    /// `Dictionary<K, V>`: (key, value) boxed, in enumeration order.
    Dict { items: Vec<(Obj, Obj)> },
    /// A delegate: (method id, receiver address) of each target of the invocation list; (0, 0) for a lambda.
    Delegate { targets: Vec<(u64, usize)> },
}

/// Boxes every element (`(object)x`).
pub fn box_all<T: super::Val>(items: &[T]) -> Vec<Obj> {
    items.iter().map(|x| x.to_obj()).collect()
}

fn convert_number(value: Obj) -> Result<i128, f64> {
    if value.is_null() {
        return Ok(0);
    }
    // SAFETY: non-null objects have a header; the id says which primitive the box holds.
    unsafe {
        let have = type_id_of(value.0 as *const u8);
        let have = enum_info(have).map(|i| i.underlying).unwrap_or(have);
        match have {
            tid::BOOL => Ok(*(value.0 as *const bool) as i128),
            tid::U8 => Ok(*(value.0 as *const u8) as i128),
            tid::I8 => Ok(*(value.0 as *const i8) as i128),
            tid::I16 => Ok(*(value.0 as *const i16) as i128),
            tid::U16 | tid::CHAR => Ok(*(value.0 as *const u16) as i128),
            tid::I32 => Ok(*(value.0 as *const i32) as i128),
            tid::U32 => Ok(*(value.0 as *const u32) as i128),
            tid::I64 => Ok(*(value.0 as *const i64) as i128),
            tid::U64 => Ok(*(value.0 as *const u64) as i128),
            tid::F32 => Err(*(value.0 as *const f32) as f64),
            tid::F64 => Err(*(value.0 as *const f64)),
            tid::STRING => match obj_str(value).as_str().trim().parse::<i128>() {
                Ok(v) => Ok(v),
                Err(_) => super::throw(super::exc("System.FormatException", Str::lit("Input string was not in a correct format."))),
            },
            _ => invalid_cast(),
        }
    }
}

fn convert_int(value: Obj, lo: i128, hi: i128) -> i128 {
    let v = match convert_number(value) {
        Ok(v) => v,
        Err(f) if f.is_nan() => i128::MAX,
        // Convert.ToInt32(double): round half to even
        Err(f) => super::math::round(f) as i128,
    };
    if v < lo || v > hi {
        super::throw(super::exc("System.OverflowException", Str::lit("Value was either too large or too small.")))
    }
    v
}

/// `Convert.ToInt32(object)`: null is 0; numbers convert with a range check, floating point rounds half to even.
pub fn convert_to_i32(value: Obj) -> i32 {
    convert_int(value, i32::MIN as i128, i32::MAX as i128) as i32
}
/// `Convert.ToInt64(object)`.
pub fn convert_to_i64(value: Obj) -> i64 {
    convert_int(value, i64::MIN as i128, i64::MAX as i128) as i64
}

#[cfg(test)]
mod anon_tests {
    use super::*;
    use crate::rt;

    /// The behaviour of a C# anonymous type instance and of `TypeDescriptor` over it, as `Utils.FormatWith` and
    /// `LocalizedText.FormatWith` use it.
    #[test]
    fn anonymous_objects_have_value_semantics_and_properties() {
        static NAMES: [&str; 2] = ["Count", "Name"];
        let a = anon_new(&NAMES, &[rt::box_value(3i32, rt::tid::I32), rt::str_obj(Str::new("x"))]);
        let b = anon_new(&NAMES, &[rt::box_value(3i32, rt::tid::I32), rt::str_obj(Str::new("x"))]);
        let c = anon_new(&NAMES, &[rt::box_value(4i32, rt::tid::I32), rt::str_obj(Str::new("x"))]);
        assert!(rt::obj_equals(a, b) && !rt::obj_equals(a, c));
        assert_eq!(rt::obj_hash_code(a), rt::obj_hash_code(b));
        assert_eq!(rt::obj_to_string(a).as_str(), "{ Count = 3, Name = x }");
        assert_eq!(obj_get_type(a), obj_get_type(b));
        let props = type_descriptor_get_properties(a);
        let p = prop_coll_find(props, Str::new("Count"), false);
        assert!(!p.is_null());
        assert_eq!(unsafe { rt::unbox::<i32>((*p.ptr()).get_value(c), rt::tid::I32) }, 4);
        assert!(prop_coll_find(props, Str::new("count"), false).is_null());
        assert!(!prop_coll_find(props, Str::new("count"), true).is_null());
        assert!(prop_coll_find(props, Str::new("Missing"), false).is_null());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rt::{try_, List};

    #[test]
    fn strings_as_objects() {
        let o = str_obj(Str::lit("abc"));
        assert_eq!(obj_str(o).as_str(), "abc");
        assert_eq!(obj_as_str(o).as_str(), "abc");
        assert!(str_obj(Str::NULL).is_null());
        assert!(obj_str(Gc::NULL).is_null());
        let i = box_value(5i32, tid::I32);
        assert!(obj_as_str(i).is_null());
        assert!(try_(|| obj_str(i)).is_err());
    }

    #[test]
    fn nullable_boxes_as_its_value() {
        let n: Nullable<i32> = unsafe { crate::rt::zero() };
        assert!(box_nullable(n, tid::I32).is_null());
        let o = box_nullable(Nullable::some(4i32), tid::I32);
        unsafe {
            assert_eq!(crate::rt::unbox::<i32>(o, tid::I32), 4);
            assert!(unbox_nullable::<i32>(o, tid::I32) == Nullable::some(4));
            assert!(unbox_nullable::<i32>(Gc::NULL, tid::I32).is_null());
            assert!(try_(|| unbox_nullable::<f32>(o, tid::F32)).is_err());
        }
    }

    #[test]
    fn runtime_downcasts_check_the_element_type() {
        let a = Arr::<i32>::from_slice(&[1, 2]);
        let o: Obj = a.up();
        assert!(as_rt::<Arr<i32>>(o) == a);
        assert!(as_rt::<Arr<f32>>(o).is_null());
        assert!(cast_rt::<Arr<i32>>(Gc::NULL).is_null());
        assert!(try_(|| cast_rt::<Arr<u8>>(o)).is_err());
        let l = List::<i32>::new_gc();
        assert!(as_rt::<Gc<List<i32>>>(l.up()) == l);
        assert!(as_rt::<Gc<List<i64>>>(l.up()).is_null());
        assert!(as_rt::<Arr<i32>>(l.up()).is_null());
        let d: Delegate<dyn Fn() -> i32> = Delegate::new(Box::new(|| 1));
        assert!(as_rt::<Delegate<dyn Fn() -> i32>>(d.up()) == d);
        assert!(as_rt::<Delegate<dyn Fn()>>(d.up()).is_null());
    }

    #[test]
    fn interface_tests_use_the_table() {
        fn implements(id: u32) -> bool {
            matches!(id, 70 | 71)
        }
        let a = crate::rt::alloc::<crate::rt::Object>(70).up::<crate::rt::Object>();
        let b = crate::rt::alloc::<crate::rt::Object>(72).up::<crate::rt::Object>();
        assert!(is_iface(a, implements) && !is_iface(b, implements) && !is_iface(Gc::NULL, implements));
        assert!(as_iface(b, implements).is_null() && as_iface(a, implements) == a);
        assert!(cast_iface(Gc::NULL, implements).is_null());
        assert!(try_(|| cast_iface(b, implements)).is_err());
    }

    #[test]
    fn typeof_is_one_object_per_type() {
        let a = type_of("Terraria.Item", 0, 0);
        assert!(a == type_of("Terraria.Item", 0, 0));
        assert!(a != type_of("Terraria.NPC", 0, 0));
        assert_eq!(type_full_name(a).as_str(), "Terraria.Item");
        assert!(type_is_class(a) && !type_is_value_type(a) && !type_is_primitive(a) && !type_is_generic(a));
    }

    #[test]
    fn type_parameters_answer_reflection_by_their_boxing_rule() {
        assert!(type_is_primitive(type_of_val::<i32>()) && type_is_value_type(type_of_val::<f32>()));
        assert!(type_is_class(type_of_val::<Str>()) && type_is_class(type_of_val::<Gc<crate::rt::Object>>()));
        let n = type_of_val::<Nullable<bool>>();
        assert!(type_is_generic(n) && !type_is_primitive(n) && !type_is_class(n));
        assert!(type_generic_definition(n) == type_of("System.Nullable`1", type_flags::VALUE_TYPE | type_flags::GENERIC, 0));
        assert!(try_(|| type_generic_definition(type_of_val::<i32>())).is_err());
        assert!(type_of_val::<i32>() == type_of_val::<i32>() && type_of_val::<i32>() != type_of_val::<u32>());
    }

    #[test]
    fn generic_boxing_round_trips() {
        use crate::rt::Trace;
        unsafe {
            assert_eq!(i32::from_obj(7i32.to_obj()), 7);
            assert!(is_boxed(1.5f32.to_obj(), tid::F32));
            assert_eq!(Str::from_obj(Str::lit("x").to_obj()).as_str(), "x");
            let n: Nullable<i16> = crate::rt::zero();
            assert!(n.to_obj().is_null());
            assert!(Nullable::<i16>::from_obj(Gc::NULL).is_null());
            assert!(Nullable::<i16>::from_obj(Nullable::some(4i16).to_obj()) == Nullable::some(4));
            assert!(is_boxed(Nullable::some(true).to_obj(), tid::BOOL));
            let a = Arr::<u8>::new(1);
            assert!(Arr::<u8>::from_obj(Trace::to_obj(&a)) == a);
            assert!(try_(|| i32::from_obj(1.0f32.to_obj())).is_err());
        }
        assert_eq!(Trace::cs_compare(&2, &3), -1);
        assert_eq!(Trace::cs_compare(&f32::NAN, &0.0), -1);
        assert_eq!(Trace::cs_compare(&Str::lit("b"), &Str::lit("a")), 1);
    }

    #[test]
    fn change_type_converts_between_primitives() {
        let int = type_of("System.Int32", type_flags::PRIMITIVE | type_flags::VALUE_TYPE, tid::I32);
        let byte = type_of("System.Byte", type_flags::PRIMITIVE | type_flags::VALUE_TYPE, tid::U8);
        let float = type_of("System.Single", type_flags::PRIMITIVE | type_flags::VALUE_TYPE, tid::F32);
        unsafe {
            let o = box_value(5i16, tid::I16);
            assert_eq!(crate::rt::unbox::<i32>(convert_change_type(o, int), tid::I32), 5);
            assert_eq!(crate::rt::unbox::<u8>(convert_change_type(o, byte), tid::U8), 5);
            assert_eq!(crate::rt::unbox::<f32>(convert_change_type(o, float), tid::F32), 5.0);
            // same type: the same object
            let i = box_value(9i32, tid::I32);
            assert!(convert_change_type(i, int) == i);
            // Convert.ToInt32(2.5) is 2, Convert.ToInt32(3.5) is 4 (round half to even)
            assert_eq!(crate::rt::unbox::<i32>(convert_change_type(box_value(2.5f64, tid::F64), int), tid::I32), 2);
            assert_eq!(crate::rt::unbox::<i32>(convert_change_type(box_value(3.5f64, tid::F64), int), tid::I32), 4);
            assert!(try_(|| convert_change_type(box_value(300i32, tid::I32), byte)).is_err());
            assert!(try_(|| convert_change_type(Gc::NULL, int)).is_err());
        }
    }

    #[test]
    fn enum_names_like_net() {
        const DAYS: &[(u64, &str)] = &[(0, "None"), (1, "Mon"), (2, "Tue"), (4, "Wed")];
        assert_eq!(enum_to_str(2, DAYS, false, true).as_str(), "Tue");
        assert_eq!(enum_to_str(3, DAYS, false, true).as_str(), "3");
        assert_eq!(enum_to_str(-1, DAYS, false, true).as_str(), "-1");
        assert_eq!(enum_to_str(3, DAYS, true, true).as_str(), "Mon, Tue");
        assert_eq!(enum_to_str(7, DAYS, true, true).as_str(), "Mon, Tue, Wed");
        assert_eq!(enum_to_str(0, DAYS, true, true).as_str(), "None");
        assert_eq!(enum_to_str(8, DAYS, true, true).as_str(), "8");
        assert_eq!(enum_to_str(9, DAYS, true, true).as_str(), "9");
        const NOZERO: &[(u64, &str)] = &[(1, "A"), (2, "B")];
        assert_eq!(enum_to_str(0, NOZERO, true, false).as_str(), "0");
        assert_eq!(enum_to_str(255, NOZERO, false, false).as_str(), "255");
    }
}

#[cfg(test)]
mod underlying_type_tests {
    use super::*;

    #[test]
    fn enum_underlying_type_is_the_primitive_type_object() {
        static INFO: EnumInfo = EnumInfo { tid: 900_001, name: "Test.ByteEnum", names: &[(0, "A"), (1, "B")], flags: false, signed: false, underlying: tid::U8 };
        let t = type_of_enum(&INFO);
        let u = enum_get_underlying_type(t);
        assert!(u == type_of("System.Byte", type_flags::PRIMITIVE | type_flags::VALUE_TYPE, tid::U8));
        assert!(type_is_primitive(u) && !type_is_enum(u));
        let not_enum = type_of("System.Int32", type_flags::PRIMITIVE | type_flags::VALUE_TYPE, tid::I32);
        assert!(crate::rt::try_(|| enum_get_underlying_type(not_enum).is_null()).is_err());
    }
}
