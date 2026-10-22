//! `string`: an immutable, nullable, `Copy` handle to UTF-8 text. `Length` and indexing count UTF-16 units,
//! as in C#. Literals are static data; strings built at run time are collector objects (`rt::gc`).

use super::exc::{exc, throw};
use super::gc::{alloc_bytes, mark_bytes, null_reference, Marker, Trace};

/// C# `string`. All-zero is `null`. Compare with [`str_eq`] (ordinal, like `==` on strings in C#).
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Str {
    ptr: *const u8,
    len: u32,
    ascii: bool,
    /// The bytes are a collector object (not static data).
    heap: bool,
}

unsafe impl Trace for Str {
    const RESCAN: bool = false;
    #[inline(always)]
    unsafe fn trace(&self, m: &mut Marker) {
        if self.heap {
            mark_bytes(m, self.ptr)
        }
    }
    fn to_obj(&self) -> super::Obj {
        super::str_obj(*self)
    }
    unsafe fn box_ref(&self) -> super::Obj {
        super::str_obj(*self)
    }
    unsafe fn from_obj(o: super::Obj) -> Self {
        super::obj_str(o)
    }
    /// Deviation: ordinal (`rt::sys::compare_ordinal`), not the current culture's collation.
    fn cs_compare(&self, other: &Self) -> i32 {
        super::sys::compare_ordinal(*self, *other)
    }
    // a string object (`(object)"x"`): `String.Equals(object)` is ordinal content equality with another string
    unsafe fn obj_equals(this: *const Self, other: super::Obj) -> bool {
        super::gc::is_boxed(other, super::tid::STRING) && str_eq(*this, *(other.0 as *const Str))
    }
    unsafe fn obj_hash(this: *const Self) -> i32 {
        legacy_string_hash((*this).as_str())
    }
    unsafe fn obj_to_string(this: *const Self) -> Str {
        *this
    }
}

/// `String.GetHashCode()` of the 32-bit .NET Framework 4 runtime (the game is an x86 process without
/// `UseRandomizedStringHashAlgorithm`): two interleaved multiplicative hashes over the UTF-16 units, read two at
/// a time (String.cs of the reference source, the `WIN32` branch).
pub fn legacy_string_hash(s: &str) -> i32 {
    let mut units: Vec<u16> = s.encode_utf16().collect();
    let mut len = units.len() as i32;
    // the CLR reads past an odd length into the terminating 0
    units.push(0);
    units.push(0);
    units.push(0);
    let pair = |i: usize| -> i32 { (units[i] as u32 | ((units[i + 1] as u32) << 16)) as i32 };
    let mut hash1: i32 = (5381 << 16) + 5381;
    let mut hash2: i32 = hash1;
    let mut i = 0usize;
    while len > 2 {
        hash1 = (hash1.wrapping_shl(5).wrapping_add(hash1).wrapping_add(hash1 >> 27)) ^ pair(i);
        hash2 = (hash2.wrapping_shl(5).wrapping_add(hash2).wrapping_add(hash2 >> 27)) ^ pair(i + 2);
        i += 4;
        len -= 4;
    }
    if len > 0 {
        hash1 = (hash1.wrapping_shl(5).wrapping_add(hash1).wrapping_add(hash1 >> 27)) ^ pair(i);
    }
    hash1.wrapping_add(hash2.wrapping_mul(1566083941))
}

const fn is_ascii(b: &[u8]) -> bool {
    let mut i = 0;
    while i < b.len() {
        if b[i] >= 0x80 {
            return false;
        }
        i += 1;
    }
    true
}

impl Str {
    pub const NULL: Str = Str { ptr: core::ptr::null(), len: 0, ascii: false, heap: false };

    /// A string literal (no allocation).
    pub const fn lit(s: &'static str) -> Str {
        Str { ptr: s.as_ptr(), len: s.len() as u32, ascii: is_ascii(s.as_bytes()), heap: false }
    }

    /// A string built at run time: a collector object, alive while a traced value holds it.
    pub fn new(s: &str) -> Str {
        if s.is_empty() {
            return Str::lit("");
        }
        Str { ptr: alloc_bytes(s.as_bytes()), len: s.len() as u32, ascii: is_ascii(s.as_bytes()), heap: true }
    }

    /// A string built at run time that lives for ever (host tables that are not collector roots).
    pub fn new_static(s: &str) -> Str {
        if s.is_empty() {
            return Str::lit("");
        }
        let leaked: &'static str = Box::leak(s.to_owned().into_boxed_str());
        Str::lit(leaked)
    }

    #[inline(always)]
    pub fn is_null(self) -> bool {
        self.ptr.is_null()
    }

    /// The string itself as the receiver of a member call: null throws `NullReferenceException`.
    #[inline]
    pub fn not_null(self) -> Str {
        if self.ptr.is_null() {
            null_reference();
        }
        self
    }

    /// The text; a null string throws `NullReferenceException` (member access on null).
    /// The `'static` is the runtime's usual contract, not Rust's: the text is valid until a collection frees
    /// the string, so runtime and host code must not keep the slice across `rt::collect()`.
    #[inline]
    pub fn as_str(self) -> &'static str {
        if self.ptr.is_null() {
            null_reference();
        }
        // SAFETY: non-null `Str` always points at `len` bytes of valid UTF-8 that are never freed.
        unsafe { core::str::from_utf8_unchecked(core::slice::from_raw_parts(self.ptr, self.len as usize)) }
    }

    /// The text, or `None` for null.
    pub fn get(self) -> Option<&'static str> {
        if self.ptr.is_null() {
            None
        } else {
            Some(self.as_str())
        }
    }

    /// `s.Length`: number of UTF-16 code units.
    pub fn len(self) -> i32 {
        let s = self.as_str();
        if self.ascii {
            s.len() as i32
        } else {
            s.encode_utf16().count() as i32
        }
    }

    /// `s[i]`: the UTF-16 code unit at `i` (`IndexOutOfRangeException` outside the string).
    pub fn char_at(self, i: i32) -> u16 {
        let s = self.as_str();
        let unit = if i < 0 {
            None
        } else if self.ascii {
            s.as_bytes().get(i as usize).map(|b| *b as u16)
        } else {
            s.encode_utf16().nth(i as usize)
        };
        match unit {
            Some(u) => u,
            None => throw(exc("System.IndexOutOfRangeException", crate::s!("Index was outside the bounds of the array."))),
        }
    }
}

impl core::fmt::Debug for Str {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self.get() {
            Some(s) => write!(f, "{s:?}"),
            None => write!(f, "null"),
        }
    }
}

/// `a == b` on strings: ordinal comparison of contents; null equals only null.
pub fn str_eq(a: Str, b: Str) -> bool {
    match (a.get(), b.get()) {
        (None, None) => true,
        (Some(x), Some(y)) => x == y,
        _ => false,
    }
}

/// `string.IsNullOrEmpty(s)`.
pub fn str_is_null_or_empty(s: Str) -> bool {
    s.get().map(|t| t.is_empty()).unwrap_or(true)
}

/// `string.Concat`: null parts count as empty; the result is never null.
pub fn concat(parts: &[Str]) -> Str {
    let mut out = String::new();
    for p in parts {
        if let Some(s) = p.get() {
            out.push_str(s);
        }
    }
    Str::new(&out)
}

/// One UTF-16 unit as a string (`char.ToString()`). A lone surrogate becomes U+FFFD (cannot be represented).
pub fn char_str(c: u16) -> Str {
    let ch = char::from_u32(c as u32).unwrap_or('\u{fffd}');
    let mut buf = [0u8; 4];
    Str::new(ch.encode_utf8(&mut buf))
}

/// Index of `value` in `cases`, or -1: the selector of a `switch` on a string.
pub fn str_switch(value: Str, cases: &[Str]) -> i32 {
    match value.get() {
        None => -1,
        Some(v) => cases.iter().position(|c| c.get() == Some(v)).map(|i| i as i32).unwrap_or(-1),
    }
}

/// `x.ToString()` of the primitive types in the thread's current culture (the game makes the selected language's
/// culture current: a German player reads "1,5"). Integers and booleans print alike in every culture the game has.
pub trait ToStr {
    fn cs_to_string(self) -> String;
}
macro_rules! int_to_str {
    ($($t:ty),*) => {$( impl ToStr for $t { fn cs_to_string(self) -> String { self.to_string() } } )*};
}
int_to_str!(i8, u8, i16, u16, i32, u32, i64, u64);
impl ToStr for bool {
    fn cs_to_string(self) -> String {
        if self { "True".to_owned() } else { "False".to_owned() }
    }
}
impl ToStr for f32 {
    /// .NET Framework `float.ToString()` is format "G7".
    fn cs_to_string(self) -> String {
        super::fmt::float_to_string_current(self as f64, true)
    }
}
impl ToStr for f64 {
    /// .NET Framework `double.ToString()` is format "G15".
    fn cs_to_string(self) -> String {
        super::fmt::float_to_string_current(self, false)
    }
}

pub fn to_str<T: ToStr>(v: T) -> Str {
    Str::new(&v.cs_to_string())
}

/// "G<precision>" of a finite or NaN value in the INVARIANT culture (`rt::fmt` puts the culture's separators in).
pub(crate) fn general_invariant(v: f64, precision: usize) -> String {
    general(v, precision)
}

/// .NET "G<precision>": `precision` significant digits (round half away from zero on the exact decimal value),
/// trailing zeros dropped, scientific notation `d.dddE+XX` when the exponent is >= precision or < -4.
fn general(x: f64, precision: usize) -> String {
    if x.is_nan() {
        return "NaN".to_owned();
    }
    if x.is_infinite() {
        return if x > 0.0 { "Infinity".to_owned() } else { "-Infinity".to_owned() };
    }
    if x == 0.0 {
        // .NET Framework prints negative zero as "0"
        return "0".to_owned();
    }
    // exact-enough decimal expansion: 60 digits after the first
    let e = format!("{:.60e}", x.abs());
    let (mant, exp) = e.split_once('e').expect("exponent");
    let mut exp: i32 = exp.parse().expect("exponent value");
    let mut digits: Vec<u8> = mant.bytes().filter(|b| b.is_ascii_digit()).map(|b| b - b'0').collect();
    let round_up = digits[precision] >= 5;
    digits.truncate(precision);
    if round_up {
        let mut i = precision;
        loop {
            if i == 0 {
                digits.insert(0, 1);
                digits.truncate(precision);
                exp += 1;
                break;
            }
            i -= 1;
            if digits[i] == 9 {
                digits[i] = 0;
            } else {
                digits[i] += 1;
                break;
            }
        }
    }
    while digits.len() > 1 && *digits.last().expect("digit") == 0 {
        digits.pop();
    }
    let ds: String = digits.iter().map(|d| (b'0' + d) as char).collect();
    let mut out = String::new();
    if x < 0.0 {
        out.push('-');
    }
    if exp >= precision as i32 || exp < -4 {
        out.push_str(&ds[..1]);
        if ds.len() > 1 {
            out.push('.');
            out.push_str(&ds[1..]);
        }
        out.push_str(&format!("E{}{:02}", if exp < 0 { '-' } else { '+' }, exp.abs()));
    } else if exp < 0 {
        out.push_str("0.");
        for _ in 0..(-exp - 1) {
            out.push('0');
        }
        out.push_str(&ds);
    } else {
        let int_len = exp as usize + 1;
        if ds.len() <= int_len {
            out.push_str(&ds);
            for _ in ds.len()..int_len {
                out.push('0');
            }
        } else {
            out.push_str(&ds[..int_len]);
            out.push('.');
            out.push_str(&ds[int_len..]);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::s;

    #[test]
    fn null_empty_and_equality() {
        assert!(Str::NULL.is_null());
        assert!(!s!("").is_null());
        assert!(str_eq(Str::NULL, Str::NULL));
        assert!(!str_eq(Str::NULL, s!("")));
        assert!(str_eq(s!("ab"), concat(&[s!("a"), Str::NULL, s!("b")])));
        assert!(!concat(&[Str::NULL, Str::NULL]).is_null());
    }

    #[test]
    fn length_and_index_are_utf16() {
        assert_eq!(s!("abc").len(), 3);
        assert_eq!(s!("abc").char_at(1), b'b' as u16);
        let t = s!("a\u{e9}\u{1f600}"); // 'a', e-acute (1 unit), emoji (2 units)
        assert_eq!(t.len(), 4);
        assert_eq!(t.char_at(1), 0xe9);
        assert_eq!(t.char_at(2), 0xd83d);
        assert!(crate::rt::try_(|| s!("abc").char_at(3)).is_err());
        assert!(crate::rt::try_(|| Str::NULL.len()).is_err());
    }

    #[test]
    fn switch_selector() {
        let cases = [s!("a"), s!("b")];
        assert_eq!(str_switch(s!("b"), &cases), 1);
        assert_eq!(str_switch(s!("c"), &cases), -1);
        assert_eq!(str_switch(Str::NULL, &cases), -1);
    }

    #[test]
    fn number_formatting_like_net_framework() {
        let f = |x: f32| x.cs_to_string();
        assert_eq!(f(1.0), "1");
        assert_eq!(f(0.6), "0.6");
        assert_eq!(f(-2.5), "-2.5");
        assert_eq!(f(0.1 + 0.2), "0.3");
        assert_eq!(f(1.0 / 3.0), "0.3333333");
        assert_eq!(f(1234567.0), "1234567");
        assert_eq!(f(12345678.0), "1.234568E+07");
        assert_eq!(f(0.0001), "0.0001");
        assert_eq!(f(0.00001), "1E-05");
        assert_eq!(f(16777216.0), "1.677722E+07");
        assert_eq!(f(f32::MAX), "3.402823E+38");
        assert_eq!(f(f32::NAN), "NaN");
        assert_eq!(f(-0.0), "0");
        let d = |x: f64| x.cs_to_string();
        assert_eq!(d(0.1), "0.1");
        assert_eq!(d(0.1 + 0.2), "0.3");
        assert_eq!(d(1.0 / 3.0), "0.333333333333333");
        assert_eq!(d(1e15), "1E+15");
        assert_eq!(d(123456789012345.0), "123456789012345");
        assert_eq!(d(2.5), "2.5");
        assert_eq!(true.cs_to_string(), "True");
        assert_eq!((-5i32).cs_to_string(), "-5");
        assert_eq!(200u8.cs_to_string(), "200");
    }
}
