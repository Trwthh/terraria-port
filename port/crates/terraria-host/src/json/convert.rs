//! Conversions between the `Linq` DOM and .NET values: `JToken.ToObject<T>()`, `(int)token`, `Value<T>()`,
//! `JsonConvert.DeserializeObject<T>(json)` for the types that need no type metadata, `JToken.FromObject(value)` and
//! `JsonConvert.SerializeObject(value)` for primitives, strings, `JToken`s, arrays, lists and dictionaries.
//!
//! Numeric conversions follow the explicit operators of `JToken` (`Convert.ToInt32(object, InvariantCulture)` after
//! `ValidateToken`), checked against the dll for 18 values x 9 target types (json_probe.ps1 / probe in this
//! module's test): floats round half to even, `"7"` parses, a string `"1.5"` does not parse as an integer, `true`
//! is 1, `null` is `ArgumentException: Can not convert Null to Int32.`, 3000000000 to `int` is `OverflowException`.
//!
//! Game classes: the generator implements [`FromJToken`] / [`ToJToken`] for `Gc<Class>` from the member table in
//! REQUESTS.md; the helpers [`member`] and [`set_member`] are the building blocks it calls.

use terraria_gen_rt::rt::{self, tid, Arr, Dictionary, Gc, List, Obj, Str, Val};

use super::number;
use super::reader::{self, DateVal, Scalar, Zone};
use super::token::*;

fn throw_exc(kind: &'static str, msg: &str) -> ! {
    rt::throw(rt::exc(kind, Str::new(msg)))
}

/// `JTokenType` name as in `Can not convert Array to Int32.`
fn type_name(t: &JToken) -> &'static str {
    match t.kind {
        T_OBJECT => "Object",
        T_ARRAY => "Array",
        T_CONSTRUCTOR => "Constructor",
        T_PROPERTY => "Property",
        T_COMMENT => "Comment",
        T_INTEGER => "Integer",
        T_FLOAT => "Float",
        T_STRING => "String",
        T_BOOLEAN => "Boolean",
        T_NULL => "Null",
        T_UNDEFINED => "Undefined",
        T_DATE => "Date",
        T_RAW => "Raw",
        T_BYTES => "Bytes",
        _ => "None",
    }
}

/// The scalar a number-like conversion starts from (after `ValidateToken(v, NumberTypes, false)`).
fn number_source(t: &JToken, target: &str) -> Scalar {
    match t.kind {
        T_INTEGER | T_FLOAT | T_STRING | T_COMMENT | T_RAW | T_BOOLEAN => t.value.clone(),
        _ => throw_exc("System.ArgumentException", &format!("Can not convert {} to {}.", type_name(t), target)),
    }
}

fn overflow(what: &str) -> ! {
    throw_exc("System.OverflowException", &format!("Value was either too large or too small for {what}."))
}
fn format_exc() -> ! {
    throw_exc("System.FormatException", "Input string was not in a correct format.")
}

/// `Convert.ToInt64(object)`-style: the integer value of the source within `[min, max]`.
fn to_integer(t: &JToken, target: &str, what: &str, min: i128, max: i128) -> i128 {
    let small = matches!(target, "Byte" | "SByte" | "Int16" | "UInt16");
    let v: i128 = match number_source(t, target) {
        Scalar::Int(i) => i as i128,
        Scalar::Big(s) => match s.parse::<i128>() {
            Ok(v) if !small => v,
            // Byte, SByte, Int16 and UInt16 go through Convert.ToInt32 in the original
            _ if small => overflow("an Int32"),
            Err(_) => overflow(what),
            Ok(v) => v,
        },
        Scalar::Float(f) => {
            let r = f.round_ties_even();
            if small && (!r.is_finite() || r.abs() > i32::MAX as f64) {
                overflow("an Int32")
            }
            if !r.is_finite() || r < -9.3e18 || r > 1.85e19 {
                if target == "Int64" {
                    throw_exc("System.OverflowException", "Arithmetic operation resulted in an overflow.")
                }
                overflow(what)
            }
            r as i128
        }
        Scalar::Bool(b) => b as i128,
        Scalar::Str(s) | Scalar::Comment(s) | Scalar::Raw(s) => {
            let tr = s.trim_matches(|c: char| c.is_whitespace());
            let digits = tr.strip_prefix(['+', '-']).unwrap_or(tr);
            if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
                format_exc()
            }
            match tr.parse::<i128>() {
                Ok(v) => v,
                Err(_) => overflow(what),
            }
        }
        _ => throw_exc("System.ArgumentException", &format!("Can not convert {} to {}.", type_name(t), target)),
    };
    if v < min || v > max {
        overflow(what)
    }
    v
}

fn to_double(t: &JToken, target: &str) -> f64 {
    match number_source(t, target) {
        Scalar::Int(i) => i as f64,
        Scalar::Big(s) => s.parse::<f64>().unwrap_or(f64::NAN),
        Scalar::Float(f) => f,
        Scalar::Float32(f) => f as f64,
        Scalar::Bool(b) => b as i32 as f64,
        Scalar::Str(s) | Scalar::Comment(s) | Scalar::Raw(s) => match number::parse_float(s.trim()) {
            Some(f) => f,
            None => format_exc(),
        },
        _ => unreachable!("validated"),
    }
}

pub fn to_bool(t: &JToken) -> bool {
    match number_source(t, "Boolean") {
        Scalar::Int(i) => i != 0,
        Scalar::Big(_) => overflow("an Int32"),
        Scalar::Float(f) => f != 0.0,
        Scalar::Float32(f) => f != 0.0,
        Scalar::Bool(b) => b,
        Scalar::Str(s) | Scalar::Comment(s) | Scalar::Raw(s) => match s.trim().to_ascii_lowercase().as_str() {
            "true" => true,
            "false" => false,
            _ => throw_exc("System.FormatException", "String was not recognized as a valid Boolean."),
        },
        _ => unreachable!("validated"),
    }
}

/// `(string)token`: null for a JSON null.
pub fn to_string_value(t: &JToken) -> Str {
    match t.kind {
        T_NULL | T_UNDEFINED => Str::NULL,
        T_DATE | T_INTEGER | T_FLOAT | T_STRING | T_COMMENT | T_RAW | T_BOOLEAN | T_BYTES => match &t.value {
            Scalar::Str(s) | Scalar::Comment(s) | Scalar::Raw(s) => Str::new(s),
            Scalar::Int(i) => Str::new(&i.to_string()),
            Scalar::Big(s) => Str::new(s),
            Scalar::Float(f) => Str::new(&number::format_g(*f, 15)),
            Scalar::Float32(f) => Str::new(&number::format_g(*f as f64, 7)),
            Scalar::Bool(b) => Str::lit(if *b { "True" } else { "False" }),
            Scalar::Date(d) => Str::new(&invariant_date(d)),
            Scalar::Bytes(b) => Str::new(&crate::sys::text::base64(b)),
            _ => Str::NULL,
        },
        _ => throw_exc("System.ArgumentException", &format!("Can not convert {} to String.", type_name(t))),
    }
}

/// `DateTime.ToString(CultureInfo.InvariantCulture)`: `MM/dd/yyyy HH:mm:ss`.
fn invariant_date(d: &DateVal) -> String {
    let iso = reader::format_date(&DateVal { ticks: d.ticks, zone: Zone::Unspecified });
    // iso is yyyy-MM-ddTHH:mm:ss[.fffffff]
    let (date, time) = iso.split_once('T').expect("iso date");
    let mut p = date.split('-');
    let (y, m, dd) = (p.next().unwrap_or(""), p.next().unwrap_or(""), p.next().unwrap_or(""));
    format!("{m}/{dd}/{y} {}", time.split('.').next().unwrap_or(time))
}

// ------------------------------------------------------------------------------------------------ traits

/// A type `JToken.ToObject<T>()` can produce.
pub trait FromJToken: Sized + Copy {
    fn from_jtoken(t: Gc<JToken>) -> Self;
}
/// A value `JToken.FromObject` can turn into a token.
pub trait ToJToken {
    fn to_jtoken(self) -> Gc<JToken>;
}

/// A game CLASS that Newtonsoft (de)serialises. Generated code implements this for the class type itself: Rust's
/// orphan rule does not let another crate implement [`FromJToken`] for `Gc<Class>` (`Gc` is the runtime's type),
/// so the two impls below carry it over. A game STRUCT implements [`FromJToken`] / [`ToJToken`] directly.
pub trait JsonObject: Sized + 'static {
    /// JSON null gives the null reference; otherwise a new object filled from the members of `t`.
    fn from_jtoken(t: Gc<JToken>) -> Gc<Self>;
    fn to_jtoken(this: Gc<Self>) -> Gc<JToken>;
}
impl<T: JsonObject> FromJToken for Gc<T> {
    fn from_jtoken(t: Gc<JToken>) -> Self {
        T::from_jtoken(t)
    }
}
impl<T: JsonObject> ToJToken for Gc<T> {
    fn to_jtoken(self) -> Gc<JToken> {
        T::to_jtoken(self)
    }
}

fn tok(t: Gc<JToken>) -> &'static JToken {
    // SAFETY: a live node
    unsafe { &*t.ptr() }
}

macro_rules! int_conv {
    ($t:ty, $name:literal, $what:literal) => {
        impl FromJToken for $t {
            fn from_jtoken(t: Gc<JToken>) -> $t {
                to_integer(tok(t), $name, $what, <$t>::MIN as i128, <$t>::MAX as i128) as $t
            }
        }
        impl ToJToken for $t {
            fn to_jtoken(self) -> Gc<JToken> {
                // a value the serialiser writes with `WriteValue(int)` (and the smaller integer types, which
                // `BsonWriter` widens to Int32): see `JToken::int32`. Deviation: a `short` / `byte` that goes
                // through `JToken.FromObject` first would be Int64 in BSON; nothing in the game does that.
                match i32::try_from(self as i64) {
                    Ok(v) if core::mem::size_of::<$t>() <= 4 => JToken::new_int32(v),
                    _ => JToken::new_value(Scalar::Int(self as i64)),
                }
            }
        }
    };
}
int_conv!(i8, "SByte", "a signed byte");
int_conv!(u8, "Byte", "an unsigned byte");
int_conv!(i16, "Int16", "an Int16");
int_conv!(u16, "UInt16", "a UInt16");
int_conv!(i32, "Int32", "an Int32");
int_conv!(u32, "UInt32", "a UInt32");
int_conv!(i64, "Int64", "an Int64");
impl FromJToken for u64 {
    fn from_jtoken(t: Gc<JToken>) -> u64 {
        to_integer(tok(t), "UInt64", "a UInt64", 0, u64::MAX as i128) as u64
    }
}
impl ToJToken for u64 {
    fn to_jtoken(self) -> Gc<JToken> {
        JToken::new_value(if self > i64::MAX as u64 { Scalar::Big(self.to_string()) } else { Scalar::Int(self as i64) })
    }
}
impl FromJToken for f64 {
    fn from_jtoken(t: Gc<JToken>) -> f64 {
        to_double(tok(t), "Double")
    }
}
impl ToJToken for f64 {
    fn to_jtoken(self) -> Gc<JToken> {
        JToken::new_value(Scalar::Float(self))
    }
}
impl FromJToken for f32 {
    fn from_jtoken(t: Gc<JToken>) -> f32 {
        to_double(tok(t), "Single") as f32
    }
}
impl ToJToken for f32 {
    fn to_jtoken(self) -> Gc<JToken> {
        JToken::new_value(Scalar::Float32(self))
    }
}
impl FromJToken for bool {
    fn from_jtoken(t: Gc<JToken>) -> bool {
        to_bool(tok(t))
    }
}
impl ToJToken for bool {
    fn to_jtoken(self) -> Gc<JToken> {
        JToken::new_value(Scalar::Bool(self))
    }
}
impl FromJToken for Str {
    fn from_jtoken(t: Gc<JToken>) -> Str {
        to_string_value(tok(t))
    }
}
impl ToJToken for Str {
    fn to_jtoken(self) -> Gc<JToken> {
        JToken::new_str(self)
    }
}
impl FromJToken for Gc<JToken> {
    fn from_jtoken(t: Gc<JToken>) -> Gc<JToken> {
        t
    }
}
impl ToJToken for Gc<JToken> {
    fn to_jtoken(self) -> Gc<JToken> {
        if self.is_null() {
            return JToken::new_value(Scalar::Null);
        }
        if tok(self).parent.is_null() {
            self
        } else {
            JToken::deep_clone(self)
        }
    }
}

fn check_kind(t: &JToken, want: i32, what: &str) {
    if t.kind != want {
        throw_exc("Newtonsoft.Json.JsonSerializationException", &format!("Cannot deserialize the current JSON {} into type '{what}'.", if t.kind == T_ARRAY { "array" } else if t.kind == T_OBJECT { "object" } else { "value" }));
    }
}

impl<T: FromJToken + Val> FromJToken for Arr<T> {
    fn from_jtoken(t: Gc<JToken>) -> Arr<T> {
        if t.is_null() || tok(t).kind == T_NULL {
            return Arr::NULL;
        }
        check_kind(tok(t), T_ARRAY, "array");
        let items: Vec<T> = tok(t).children.iter().map(|c| T::from_jtoken(*c)).collect();
        Arr::from_slice(&items)
    }
}
impl<T: ToJToken + Val> ToJToken for Arr<T> {
    fn to_jtoken(self) -> Gc<JToken> {
        if self.is_null() {
            return JToken::new_value(Scalar::Null);
        }
        let a = JToken::new_array();
        for v in self.as_slice() {
            JToken::add(a, v.to_jtoken());
        }
        a
    }
}
impl<T: FromJToken + Val> FromJToken for Gc<List<T>> {
    fn from_jtoken(t: Gc<JToken>) -> Gc<List<T>> {
        if t.is_null() || tok(t).kind == T_NULL {
            return Gc::NULL;
        }
        check_kind(tok(t), T_ARRAY, "System.Collections.Generic.List");
        let l = List::<T>::new_gc();
        for c in &tok(t).children {
            // SAFETY: fresh list
            unsafe { (*l.ptr()).add(T::from_jtoken(*c)) };
        }
        l
    }
}
impl<T: ToJToken + Val> ToJToken for Gc<List<T>> {
    fn to_jtoken(self) -> Gc<JToken> {
        if self.is_null() {
            return JToken::new_value(Scalar::Null);
        }
        let a = JToken::new_array();
        // SAFETY: a live list
        let l = unsafe { &*self.ptr() };
        for i in 0..l.count() {
            JToken::add(a, l.get_item(i).to_jtoken());
        }
        a
    }
}
impl<V: FromJToken + Val> FromJToken for Gc<Dictionary<Str, V>> {
    fn from_jtoken(t: Gc<JToken>) -> Gc<Dictionary<Str, V>> {
        if t.is_null() || tok(t).kind == T_NULL {
            return Gc::NULL;
        }
        check_kind(tok(t), T_OBJECT, "System.Collections.Generic.Dictionary");
        let d = Dictionary::<Str, V>::new_gc();
        for p in &tok(t).children {
            let p = tok(*p);
            let v = p.children.first().copied().unwrap_or(Gc::NULL);
            let v = if v.is_null() { JToken::new_value(Scalar::Null) } else { v };
            // SAFETY: fresh dictionary; a duplicate key cannot occur (a JObject has unique names)
            unsafe { (*d.ptr()).set_item(Str::new(&p.name), V::from_jtoken(v)) };
        }
        d
    }
}
impl<V: ToJToken + Val> ToJToken for Gc<Dictionary<Str, V>> {
    fn to_jtoken(self) -> Gc<JToken> {
        if self.is_null() {
            return JToken::new_value(Scalar::Null);
        }
        let o = JToken::new_object();
        for kv in Dictionary::as_seq(self).to_vec() {
            JToken::add_property(o, kv.key, kv.value.to_jtoken());
        }
        o
    }
}

// ---- `object`: Newtonsoft's dynamic types (long, double, bool, string, DateTime, JObject, JArray)

/// What a JSON token becomes when the target type is `object`.
pub fn token_to_obj(t: Gc<JToken>) -> Obj {
    if t.is_null() {
        return Obj::NULL;
    }
    let n = tok(t);
    match (&n.value, n.kind) {
        (_, T_OBJECT | T_ARRAY | T_CONSTRUCTOR | T_PROPERTY) => t.up(),
        (Scalar::Null | Scalar::Undefined, _) => Obj::NULL,
        (Scalar::Int(i), _) => rt::box_value(*i, tid::I64),
        (Scalar::Float(f), _) => rt::box_value(*f, tid::F64),
        (Scalar::Float32(f), _) => rt::box_value(*f, tid::F32),
        (Scalar::Bool(b), _) => rt::box_value(*b, tid::BOOL),
        (Scalar::Str(s) | Scalar::Comment(s) | Scalar::Raw(s), _) => rt::str_obj(Str::new(s)),
        (Scalar::Date(d), _) => {
            // `DateTime` boxed with the runtime's id; kind: 0 unspecified, 1 utc
            let kind = if d.zone == Zone::Utc { 1 } else { 0 };
            rt::box_value(crate::sys::time::DateTime::from_ticks_kind(d.ticks, kind), tid::DATE_TIME)
        }
        (Scalar::Big(_), _) => rt::throw(rt::exc("System.NotSupportedException", Str::lit("System.Numerics.BigInteger values are not modelled by the host (JSON integer beyond the range of a long)"))),
        (Scalar::Bytes(b), _) => rt::alloc_init(b.clone()).up(),
    }
}
impl FromJToken for Obj {
    fn from_jtoken(t: Gc<JToken>) -> Obj {
        token_to_obj(t)
    }
}

/// A .NET `object` as a token: boxed primitives, strings, `JToken`s, and the collections listed in
/// [`obj_to_jtoken`]; anything else is `NotSupportedException` (needs the type metadata of REQUESTS.md).
pub fn obj_to_jtoken(o: Obj) -> Gc<JToken> {
    if o.is_null() {
        return JToken::new_value(Scalar::Null);
    }
    // SAFETY (unbox): the id was checked just before
    macro_rules! boxed {
        ($t:ty, $id:expr) => {
            if rt::is_boxed(o, $id) {
                return unsafe { rt::unbox::<$t>(o, $id) }.to_jtoken();
            }
        };
    }
    boxed!(bool, tid::BOOL);
    boxed!(u8, tid::U8);
    boxed!(i8, tid::I8);
    boxed!(i16, tid::I16);
    boxed!(u16, tid::U16);
    boxed!(i32, tid::I32);
    boxed!(u32, tid::U32);
    boxed!(i64, tid::I64);
    boxed!(u64, tid::U64);
    boxed!(f32, tid::F32);
    boxed!(f64, tid::F64);
    if rt::is_boxed(o, tid::STRING) {
        return rt::obj_str(o).to_jtoken();
    }
    if rt::is_boxed(o, tid::CHAR) {
        // SAFETY: id checked
        let c = unsafe { rt::unbox::<u16>(o, tid::CHAR) };
        return JToken::new_value(Scalar::Str(String::from_utf16_lossy(&[c])));
    }
    if rt::is_boxed(o, tid::DATE_TIME) {
        // SAFETY: id checked
        let d = unsafe { rt::unbox::<crate::sys::time::DateTime>(o, tid::DATE_TIME) };
        let zone = if d.kind() == 1 { Zone::Utc } else { Zone::Unspecified };
        return JToken::new_value(Scalar::Date(DateVal { ticks: d.ticks(), zone }));
    }
    macro_rules! handle {
        ($t:ty) => {
            let h: $t = rt::as_rt::<$t>(o);
            if !h.is_null_handle() {
                return h.to_jtoken();
            }
        };
    }
    handle!(Gc<JToken>);
    handle!(Gc<Dictionary<Str, Obj>>);
    handle!(Gc<Dictionary<Str, Str>>);
    handle!(Gc<Dictionary<Str, bool>>);
    handle!(Gc<Dictionary<Str, i32>>);
    handle!(Gc<Dictionary<Str, Gc<Dictionary<Str, bool>>>>);
    handle!(Gc<Dictionary<Str, Gc<List<Str>>>>);
    handle!(Gc<List<Obj>>);
    handle!(Gc<List<Str>>);
    handle!(Gc<List<i32>>);
    handle!(Gc<List<f32>>);
    handle!(Arr<Str>);
    handle!(Arr<i32>);
    handle!(Arr<Obj>);
    // SAFETY: a non-null object from the runtime's allocator.
    let id = unsafe { rt::type_id_of(o.0 as *const u8) };
    // a boxed enum: its number (Newtonsoft's default; `[JsonConverter(typeof(StringEnumConverter))]` is a member rule)
    if rt::enum_info(id).is_some() {
        return JToken::new_value(Scalar::Int(rt::convert_to_i64(o)));
    }
    // an instance of a game class / a boxed game struct with a generated member table
    if let Some(write) = OBJECT_WRITERS.with(|w| w.borrow().get(&id).copied()) {
        return write(o);
    }
    // any other runtime container (`Dictionary<string, byte>`, `List<bool>`, `ushort[]` ...): by its contents, as
    // Newtonsoft writes an IDictionary with string keys as an object and any other IEnumerable as an array
    match rt::obj_reflect(o) {
        rt::Reflect::Seq { items, .. } => {
            let a = JToken::new_array();
            for it in items {
                JToken::add(a, obj_to_jtoken(it));
            }
            return a;
        }
        rt::Reflect::Dict { items } => {
            let j = JToken::new_object();
            for (k, v) in items {
                // a key that is not a string is written with its ToString(), as Newtonsoft does for primitive keys
                let key = if rt::is_boxed(k, tid::STRING) { rt::obj_str(k) } else { rt::obj_to_string(k) };
                JToken::add_property(j, key, obj_to_jtoken(v));
            }
            return j;
        }
        _ => {}
    }
    rt::throw(rt::exc(
        "System.NotSupportedException",
        Str::new(&format!("Newtonsoft.Json: an object of type id {id} ({}) held as `object` has no conversion here: it needs generated type metadata (REQUESTS.md, \"Newtonsoft.Json\")", rt::obj_to_string(o).as_str())),
    ))
}
impl ToJToken for Obj {
    fn to_jtoken(self) -> Gc<JToken> {
        obj_to_jtoken(self)
    }
}

trait NullHandle {
    fn is_null_handle(&self) -> bool;
}
impl<T> NullHandle for Gc<T> {
    fn is_null_handle(&self) -> bool {
        self.is_null()
    }
}
impl<T: Val> NullHandle for Arr<T> {
    fn is_null_handle(&self) -> bool {
        self.is_null()
    }
}

// ------------------------------------------------------------------------------------------------ entry points

/// `token.ToObject<T>()`
pub fn to_object<T: FromJToken>(t: Gc<JToken>) -> T {
    if t.is_null() {
        throw_exc("System.NullReferenceException", "Object reference not set to an instance of an object.");
    }
    T::from_jtoken(t)
}

/// `token.Value<T>(key)` for an object / array token.
pub fn value_of<T: FromJToken>(t: Gc<JToken>, key: Str) -> T {
    let v = tok(t).get(key);
    if v.is_null() {
        // Value<T> on a missing key: default(T) for reference types, an exception for value types in the original
        // (`ToObject` of a null): the zeroed handle
        // SAFETY: handles and numbers are valid when zeroed
        return unsafe { core::mem::zeroed() };
    }
    T::from_jtoken(v)
}

/// `JsonConvert.DeserializeObject<T>(json)`: `T` without metadata needs: primitives, strings, `JToken`, `object`,
/// arrays, `List<T>`, `Dictionary<string, T>`. Empty text gives `default(T)`.
pub fn deserialize<T: FromJToken>(json: Str) -> T {
    let t = JToken::deserialize(json);
    if t.is_null() {
        // SAFETY: handles and numbers are valid when zeroed
        return unsafe { core::mem::zeroed() };
    }
    T::from_jtoken(t)
}

/// `JsonConvert.DeserializeObject<Dictionary<string, Dictionary<string, string>>>(json)` (the localisation files):
/// what the runtime's minimal reader can be mapped to.
pub fn deserialize_string_map(json: Str) -> Gc<Dictionary<Str, Gc<Dictionary<Str, Str>>>> {
    deserialize(json)
}

/// `JToken.FromObject(value)`
pub fn from_object<T: ToJToken>(v: T) -> Gc<JToken> {
    v.to_jtoken()
}

/// `JsonConvert.SerializeObject(value)` / `(value, Formatting)`: 0 `None`, 1 `Indented`.
pub fn serialize(v: Obj, formatting: i32) -> Str {
    Str::new(&tok(obj_to_jtoken(v)).to_json(formatting))
}

/// `T?` (`Nullable<T>`): JSON null is "no value", anything else converts as `T`; "no value" is written as null.
impl<T: FromJToken + Val> FromJToken for rt::Nullable<T> {
    fn from_jtoken(t: Gc<JToken>) -> Self {
        if is_null_token(t) {
            // SAFETY: all-zero is the Nullable without a value.
            unsafe { rt::zero() }
        } else {
            rt::Nullable::some(T::from_jtoken(t))
        }
    }
}
impl<T: ToJToken + Val> ToJToken for rt::Nullable<T> {
    fn to_jtoken(self) -> Gc<JToken> {
        if self.has_value {
            self.value.to_jtoken()
        } else {
            null_token()
        }
    }
}

thread_local! {
    /// Type id of a game class (or of a boxed game struct) -> its generated `to_jtoken`, for values that reach the
    /// serialiser as `object` (`Preferences`: `Dictionary<string, object>`).
    static OBJECT_WRITERS: std::cell::RefCell<std::collections::HashMap<u32, fn(Obj) -> Gc<JToken>>> = std::cell::RefCell::new(std::collections::HashMap::new());
}

/// Registers the generated member table of a game type for objects held as `object` (called for every such type by
/// the generated `gen::meta::register_json()`, which the driver runs at start-up).
/// `StringEnumConverter.ReadJson`: the enum value of a token: a string is a member's name (ignoring case, or
/// names joined by commas for flags), an integer is the value itself. Anything else, or an unknown name, is
/// `JsonSerializationException`, as in Newtonsoft.
pub fn enum_from_token(t: Gc<JToken>, info: &'static rt::EnumInfo) -> i64 {
    let tk = tok(t);
    match &tk.value {
        Scalar::Str(s) if tk.kind == T_STRING => match rt::sys::enum_parse_text(info, s, true) {
            Some(v) => v as i64,
            None => throw_exc("Newtonsoft.Json.JsonSerializationException", &format!("Error converting value \"{s}\" to type '{}'.", info.name)),
        },
        Scalar::Int(i) if tk.kind == T_INTEGER => *i,
        _ => throw_exc("Newtonsoft.Json.JsonSerializationException", &format!("Unexpected token {} when parsing enum.", type_name(tk))),
    }
}

/// The token of a game object through the member table registered for its RUN-TIME type, `None` when there is
/// none. Generated member tables call it for an object that is not exactly of their class (Newtonsoft writes an
/// object by its run-time type).
pub fn write_registered(o: Obj) -> Option<Gc<JToken>> {
    if o.is_null() {
        return Some(JToken::new_value(Scalar::Null));
    }
    // SAFETY: a non-null object from the runtime's allocator.
    let id = unsafe { rt::type_id_of(o.0 as *const u8) };
    OBJECT_WRITERS.with(|w| w.borrow().get(&id).copied()).map(|write| write(o))
}

pub fn register_object_writer(type_id: u32, write: fn(Obj) -> Gc<JToken>) {
    OBJECT_WRITERS.with(|w| {
        w.borrow_mut().insert(type_id, write);
    });
}

// ------------------------------------------------------------------------------------------------ class members

/// Reads `obj[name]` for a generated `FromJToken` of a game class: `None` when the member is absent (the field
/// keeps the value the constructor gave it, as Newtonsoft does).
pub fn member<T: FromJToken>(obj: Gc<JToken>, name: &str) -> Option<T> {
    let o = tok(obj);
    // Newtonsoft matches the exact name first, then ignoring case
    let mut v = o.get(Str::new(name));
    if v.is_null() {
        if let Some(p) = o.children.iter().find(|p| tok(**p).name.to_lowercase() == name.to_lowercase()) {
            v = tok(*p).children.first().copied().unwrap_or(Gc::NULL);
        }
    }
    if v.is_null() {
        None
    } else {
        Some(T::from_jtoken(v))
    }
}
/// Is `obj[name]` present (exact name, then ignoring case)? For members a generated table cannot convert: it must
/// stop only when the JSON actually carries the member.
pub fn has_member(obj: Gc<JToken>, name: &str) -> bool {
    let o = tok(obj);
    !o.get(Str::new(name)).is_null() || o.children.iter().any(|p| tok(*p).name.to_lowercase() == name.to_lowercase())
}
/// JSON `null` (or no token at all): a generated `from_jtoken` answers the null reference / `default(T)`.
pub fn is_null_token(t: Gc<JToken>) -> bool {
    t.is_null() || tok(t).token_type() == super::token::T_NULL
}
/// The token of a null reference.
pub fn null_token() -> Gc<JToken> {
    JToken::new_value(Scalar::Null)
}
/// Adds `name: value` to an object token for a generated `ToJToken`.
pub fn set_member<T: ToJToken>(obj: Gc<JToken>, name: &str, value: T) {
    JToken::add_property(obj, Str::new(name), value.to_jtoken());
}

// ------------------------------------------------------------------------------------------------ settings, serializer, BSON streams

use terraria_gen_rt::rt::io::Stream;

/// `JsonSerializerSettings`: `Formatting` is honoured; `TypeNameHandling`, `MetadataPropertyHandling`,
/// `DefaultValueHandling` and the contract resolver only matter for objects of game classes (`$type`, member
/// tables: REQUESTS.md) and are kept as written.
pub struct JsonSerializerSettings {
    pub formatting: i32,
    pub type_name_handling: i32,
    pub metadata_property_handling: i32,
    pub default_value_handling: i32,
    pub contract_resolver: Obj,
}
// SAFETY: the one handle is reported.
unsafe impl rt::Trace for JsonSerializerSettings {
    unsafe fn trace(&self, m: &mut rt::Marker) {
        m.mark(self.contract_resolver);
    }
}
impl JsonSerializerSettings {
    pub fn new_gc() -> Gc<JsonSerializerSettings> {
        rt::alloc_init(JsonSerializerSettings { formatting: 0, type_name_handling: 0, metadata_property_handling: 0, default_value_handling: 0, contract_resolver: Obj::NULL })
    }
}

/// A custom `IContractResolver` changes which members of a game class are (de)serialised (the game's one resolver,
/// `EasyDeserializationJsonContractResolver`, takes every field, private ones too). The generated member tables
/// follow the default contract only, so converting with such settings stops instead of producing other JSON.
fn refuse_custom_resolver(settings: Gc<JsonSerializerSettings>, what: &'static str) {
    // SAFETY: a live settings object.
    if !settings.is_null() && unsafe { !(*settings.ptr()).contract_resolver.is_null() } {
        let _: () = terraria_gen_rt::host::unimplemented_value(what);
    }
}

/// `JsonConvert.SerializeObject(value, settings)`
pub fn serialize_with(v: Obj, settings: Gc<JsonSerializerSettings>) -> Str {
    refuse_custom_resolver(settings, "Newtonsoft.Json.JsonConvert.SerializeObject(object, JsonSerializerSettings) with a custom ContractResolver");
    let f = if settings.is_null() { 0 } else { unsafe { (*settings.ptr()).formatting } };
    serialize(v, f)
}
/// `JsonConvert.DeserializeObject<T>(json, settings)`
pub fn deserialize_with<T: FromJToken>(json: Str, settings: Gc<JsonSerializerSettings>) -> T {
    refuse_custom_resolver(settings, "Newtonsoft.Json.JsonConvert.DeserializeObject<T>(string, JsonSerializerSettings) with a custom ContractResolver");
    deserialize(json)
}

/// `new BsonWriter(stream)` / `new BsonReader(stream)`: the stream the serializer writes to / reads from.
pub struct BsonStream {
    pub stream: Gc<Stream>,
}
// SAFETY: the stream is reported.
unsafe impl rt::Trace for BsonStream {
    unsafe fn trace(&self, m: &mut rt::Marker) {
        m.mark(self.stream);
    }
}
impl BsonStream {
    pub fn new_gc(stream: Gc<Stream>) -> Gc<BsonStream> {
        if stream.is_null() {
            throw_exc("System.ArgumentNullException", "stream");
        }
        rt::alloc_init(BsonStream { stream })
    }
    /// `Flush()` / `Close()` / `Dispose()`: the document is written by `JsonSerializer.Serialize`.
    pub fn flush(&self) {}
}

/// `JsonSerializer.Create(settings)`: only the settings are kept.
pub struct JsonSerializer {
    pub settings: Gc<JsonSerializerSettings>,
}
// SAFETY: the one handle is reported.
unsafe impl rt::Trace for JsonSerializer {
    unsafe fn trace(&self, m: &mut rt::Marker) {
        m.mark(self.settings);
    }
}
impl JsonSerializer {
    pub fn create(settings: Gc<JsonSerializerSettings>) -> Gc<JsonSerializer> {
        rt::alloc_init(JsonSerializer { settings })
    }
    /// `Serialize(BsonWriter, object)`
    pub fn serialize_bson(&self, writer: Gc<BsonStream>, value: Obj) {
        let t = obj_to_jtoken(value);
        match super::bson::write(t) {
            Ok(b) => unsafe { (*(*writer.ptr()).stream.ptr()).write_slice(&b) },
            Err(e) => throw_exc("Newtonsoft.Json.JsonWriterException", &e),
        }
    }
    /// `Deserialize<T>(BsonReader)`: the rest of the stream is one BSON document.
    pub fn deserialize_bson<T: FromJToken>(&self, reader: Gc<BsonStream>) -> T {
        // SAFETY: live objects
        let s = unsafe { &mut *(*reader.ptr()).stream.ptr() };
        let mut bytes = vec![0u8; (s.length() - s.position()).max(0) as usize];
        let n = s.read_slice(&mut bytes);
        bytes.truncate(n);
        if bytes.is_empty() {
            // SAFETY: handles and numbers are valid when zeroed
            return unsafe { core::mem::zeroed() };
        }
        match super::bson::read(&bytes, false) {
            Ok(t) => T::from_jtoken(t),
            Err(e) => throw_json(e),
        }
    }
}

/// `JToken.Item(object key)` (a `JToken`-typed receiver): a string key for an object, an `int` for an array.
pub fn token_item(this: Gc<JToken>, key: Obj) -> Gc<JToken> {
    let t = tok(this);
    if rt::is_boxed(key, tid::I32) {
        // SAFETY: id checked
        return t.get_at(unsafe { rt::unbox::<i32>(key, tid::I32) });
    }
    t.get(rt::obj_str(key))
}

/// `JArray.FromObject(value)` / `JToken.FromObject(value)`
pub fn from_object_obj(v: Obj) -> Gc<JToken> {
    obj_to_jtoken(v)
}

#[cfg(test)]
mod object_tests {
    use super::*;

    /// `Preferences` holds its values as `object`: containers of any element type must serialise by content.
    #[test]
    fn containers_held_as_object_serialise_by_content() {
        let d = Dictionary::<Str, u8>::new_gc();
        // SAFETY: fresh objects.
        unsafe {
            (*d.ptr()).add(Str::new("a"), 3u8);
            (*d.ptr()).add(Str::new("b"), 250u8);
        }
        assert_eq!(serialize(d.up(), 0).as_str(), "{\"a\":3,\"b\":250}");
        let l = List::<bool>::new_gc();
        unsafe {
            (*l.ptr()).add(true);
            (*l.ptr()).add(false);
        }
        assert_eq!(serialize(l.up(), 0).as_str(), "[true,false]");
    }
}
