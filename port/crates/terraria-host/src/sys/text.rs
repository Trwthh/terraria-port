//! `System.Text.Encoding`, `StringBuilder`, `Convert.ToBase64String`, `Uri.EscapeDataString`, the numeric format
//! strings the game uses (`X2`, `F2`, `P0`), `Math.Round(double, int)`, `Math.Log(double, double)`.
//! Expected values: tests/dotnet/system_probe.ps1 (real .NET Framework).

use terraria_gen_rt::host;
use terraria_gen_rt::rt::{self, Arr, Gc, Str};

use crate::gfx::states::{plain_trace, throw};

// ------------------------------------------------------------------------------------------------ Encoding

/// `System.Text.Encoding`: the three the game asks for.
pub struct Encoding {
    pub kind: EncodingKind,
}
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum EncodingKind {
    Ascii,
    Utf8,
    /// `Encoding.Unicode` / `new UnicodeEncoding()`: UTF-16 little endian
    Utf16Le,
}
plain_trace!(Encoding);
// SAFETY: no handles inside. `ToString()` through `object` (a builder passed to `string.Concat(object, ..)`,
// `string.Format`, `Append(object)`) is the text, not the type name.
unsafe impl rt::Trace for StringBuilder {
    const NEEDS_TRACE: bool = false;
    unsafe fn trace(&self, _m: &mut rt::Marker) {}
    unsafe fn obj_to_string(this: *const Self) -> Str {
        (*this).to_str()
    }
}

impl Encoding {
    /// `Encoding.ASCII` / `Encoding.UTF8` / `Encoding.Unicode` / `new UnicodeEncoding()`. A fresh object per call
    /// (the game never compares encodings by reference).
    pub fn new_gc(kind: EncodingKind) -> Gc<Encoding> {
        rt::alloc_init(Encoding { kind })
    }
    pub fn encode(&self, s: &str) -> Vec<u8> {
        match self.kind {
            EncodingKind::Ascii => s.encode_utf16().map(|u| if u < 0x80 { u as u8 } else { b'?' }).collect(),
            EncodingKind::Utf8 => s.as_bytes().to_vec(),
            EncodingKind::Utf16Le => s.encode_utf16().flat_map(|u| u.to_le_bytes()).collect(),
        }
    }
    pub fn decode(&self, b: &[u8]) -> String {
        match self.kind {
            EncodingKind::Ascii => b.iter().map(|c| if *c < 0x80 { *c as char } else { '?' }).collect(),
            EncodingKind::Utf8 => String::from_utf8_lossy(b).into_owned(),
            EncodingKind::Utf16Le => String::from_utf16_lossy(&b.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect::<Vec<_>>()),
        }
    }
    /// `GetBytes(string)`
    pub fn get_bytes(&self, s: Str) -> Arr<u8> {
        match s.get() {
            Some(t) => Arr::from_slice(&self.encode(t)),
            None => throw("System.ArgumentNullException", "s"),
        }
    }
    /// `GetByteCount(string)`
    pub fn get_byte_count(&self, s: Str) -> i32 {
        match s.get() {
            Some(t) => self.encode(t).len() as i32,
            None => throw("System.ArgumentNullException", "s"),
        }
    }
    /// `GetString(byte[])`
    pub fn get_string(&self, bytes: Arr<u8>) -> Str {
        if bytes.is_null() {
            throw("System.ArgumentNullException", "bytes");
        }
        Str::new(&self.decode(bytes.as_slice()))
    }
    /// `GetString(byte[], index, count)`
    pub fn get_string_range(&self, bytes: Arr<u8>, index: i32, count: i32) -> Str {
        if bytes.is_null() {
            throw("System.ArgumentNullException", "bytes");
        }
        if index < 0 || count < 0 || index as i64 + count as i64 > bytes.len() as i64 {
            throw("System.ArgumentOutOfRangeException", "index / count");
        }
        Str::new(&self.decode(&bytes.as_slice()[index as usize..(index + count) as usize]))
    }
}

// ------------------------------------------------------------------------------------------------ StringBuilder

/// `System.Text.StringBuilder`
pub struct StringBuilder {
    pub text: String,
    /// `Capacity` as last reported / requested (see [`StringBuilder::capacity`]).
    cap: i32,
}
impl StringBuilder {
    /// `new StringBuilder()` / `(value)`: capacity 16, or the value's length when that is larger.
    pub fn new_gc(initial: Str) -> Gc<StringBuilder> {
        let text = initial.get().unwrap_or("").to_owned();
        let cap = 16.max(text.encode_utf16().count() as i32);
        rt::alloc_init(StringBuilder { text, cap })
    }
    /// `new StringBuilder(capacity)`: 0 means the default 16.
    pub fn with_capacity_gc(capacity: i32) -> Gc<StringBuilder> {
        if capacity < 0 {
            throw("System.ArgumentOutOfRangeException", "capacity");
        }
        rt::alloc_init(StringBuilder { text: String::new(), cap: if capacity == 0 { 16 } else { capacity } })
    }
    /// `Capacity`. A MODEL of the .NET Framework 4 value, which is the summed size of the builder's chunks and
    /// depends on how the text was appended: here the capacity grows, when the text has outgrown it, by one block of
    /// `max(missing, min(capacity, 8000))` characters (`ExpandByABlock` for one append of everything missing). The
    /// game reads `Capacity` in one place, to pass `Capacity + n` to `EnsureCapacity`, whose result it ignores.
    pub fn capacity(&mut self) -> i32 {
        let len = self.length();
        if len > self.cap {
            self.cap += (len - self.cap).max(self.cap.min(8000));
        }
        self.cap
    }
    /// `EnsureCapacity(capacity)`: the capacity afterwards.
    pub fn ensure_capacity(&mut self, capacity: i32) -> i32 {
        if capacity < 0 {
            throw("System.ArgumentOutOfRangeException", "capacity");
        }
        if self.capacity() < capacity {
            self.cap = capacity;
        }
        self.cap
    }
    /// `Remove(startIndex, length)` (UTF-16 units)
    pub fn remove(&mut self, start: i32, length: i32) {
        let mut u: Vec<u16> = self.text.encode_utf16().collect();
        if start < 0 || length < 0 || start as usize > u.len() || length as usize > u.len() - start as usize {
            throw("System.ArgumentOutOfRangeException", if start < 0 || start as usize > u.len() { "startIndex" } else { "length" });
        }
        u.drain(start as usize..(start + length) as usize);
        self.text = String::from_utf16_lossy(&u);
    }
    /// `this[index]` (get)
    pub fn char_at(&self, index: i32) -> u16 {
        match self.text.encode_utf16().nth(index.max(0) as usize) {
            Some(c) if index >= 0 => c,
            _ => throw("System.IndexOutOfRangeException", "Index was outside the bounds of the array."),
        }
    }
    /// `Append(char)`
    pub fn append_char(&mut self, c: u16) {
        // a lone surrogate cannot be stored (the runtime's strings are UTF-8): U+FFFD, as `Str` does elsewhere
        self.text.push(char::from_u32(c as u32).unwrap_or('\u{fffd}'));
    }
    /// `Append(string)`: null appends nothing.
    pub fn append(&mut self, s: Str) {
        if let Some(t) = s.get() {
            self.text.push_str(t);
        }
    }
    /// `AppendLine(string)`
    pub fn append_line(&mut self, s: Str) {
        self.append(s);
        self.text.push_str("\r\n");
    }
    /// `Length` in UTF-16 units.
    pub fn length(&self) -> i32 {
        self.text.encode_utf16().count() as i32
    }
    pub fn clear(&mut self) {
        self.text.clear();
    }
    pub fn to_str(&self) -> Str {
        Str::new(&self.text)
    }
}

// ------------------------------------------------------------------------------------------------ Convert / Uri

/// `Convert.ToBase64String(byte[])`
pub fn to_base64_string(bytes: Arr<u8>) -> Str {
    if bytes.is_null() {
        throw("System.ArgumentNullException", "inArray");
    }
    Str::new(&base64(bytes.as_slice()))
}

pub fn base64(b: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(b.len().div_ceil(3) * 4);
    for c in b.chunks(3) {
        let n = (c[0] as u32) << 16 | (*c.get(1).unwrap_or(&0) as u32) << 8 | *c.get(2).unwrap_or(&0) as u32;
        out.push(T[(n >> 18) as usize & 63] as char);
        out.push(T[(n >> 12) as usize & 63] as char);
        out.push(if c.len() > 1 { T[(n >> 6) as usize & 63] as char } else { '=' });
        out.push(if c.len() > 2 { T[n as usize & 63] as char } else { '=' });
    }
    out
}

/// `Convert.FromBase64String`: `FormatException` on anything that is not base64 (white space is skipped).
pub fn from_base64_string(s: Str) -> Arr<u8> {
    let Some(t) = s.get() else { throw("System.ArgumentNullException", "s") };
    let mut vals = Vec::new();
    let mut pad = 0;
    for c in t.bytes().filter(|c| !c.is_ascii_whitespace()) {
        let v = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            b'=' => {
                pad += 1;
                0
            }
            _ => throw("System.FormatException", "The input is not a valid Base-64 string."),
        };
        if pad > 0 && c != b'=' {
            throw("System.FormatException", "The input is not a valid Base-64 string.");
        }
        vals.push(v);
    }
    if vals.len() % 4 != 0 || pad > 2 {
        throw("System.FormatException", "Invalid length for a Base-64 char array or string.");
    }
    let mut out = Vec::new();
    for q in vals.chunks(4) {
        let n = (q[0] as u32) << 18 | (q[1] as u32) << 12 | (q[2] as u32) << 6 | q[3] as u32;
        out.extend_from_slice(&[(n >> 16) as u8, (n >> 8) as u8, n as u8]);
    }
    out.truncate(out.len() - pad);
    Arr::from_slice(&out)
}

/// `Uri.EscapeDataString` as .NET Framework 4.0 escapes (RFC 2396 unreserved: letters, digits, `-_.!~*'()`).
pub fn uri_escape_data_string(s: Str) -> Str {
    let Some(t) = s.get() else { throw("System.ArgumentNullException", "stringToEscape") };
    let mut out = String::new();
    for b in t.bytes() {
        if b.is_ascii_alphanumeric() || b"-_.!~*'()".contains(&b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    Str::new(&out)
}

/// `Uri.UnescapeDataString`: `%XX` runs are decoded as UTF-8; anything that is not a valid escape stays.
pub fn uri_unescape_data_string(s: Str) -> Str {
    let Some(t) = s.get() else { throw("System.ArgumentNullException", "stringToUnescape") };
    let b = t.as_bytes();
    let hex = |c: u8| (c as char).to_digit(16).map(|d| d as u8);
    let mut out: Vec<u8> = Vec::new();
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            if let (Some(h), Some(l)) = (hex(b[i + 1]), hex(b[i + 2])) {
                // collect the whole escaped run, keep only what decodes as UTF-8
                let mut run = vec![h << 4 | l];
                let mut j = i + 3;
                while j + 2 < b.len() && b[j] == b'%' {
                    match (b.get(j + 1).copied().and_then(hex), b.get(j + 2).copied().and_then(hex)) {
                        (Some(h), Some(l)) => {
                            run.push(h << 4 | l);
                            j += 3;
                        }
                        _ => break,
                    }
                }
                match String::from_utf8(run.clone()) {
                    Ok(s) => out.extend_from_slice(s.as_bytes()),
                    // bytes that are not UTF-8 stay escaped (what .NET does for an invalid sequence)
                    Err(_) => out.extend_from_slice(&b[i..j]),
                }
                i = j;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    Str::new(&String::from_utf8_lossy(&out))
}

// ------------------------------------------------------------------------------------------------ number formatting

/// `byte.ToString("X2")` and the other integer hex formats: `X` + minimum digits (upper case), `x` lower case.
pub fn int_to_string_format(value: u64, format: Str) -> Str {
    let f = format.get().unwrap_or("");
    let (spec, digits) = f.split_at(f.len().min(1));
    let width: usize = if digits.is_empty() { 0 } else { digits.parse().unwrap_or_else(|_| throw("System.FormatException", "Format specifier was invalid.")) };
    match spec {
        "X" => Str::new(&format!("{value:0width$X}")),
        "x" => Str::new(&format!("{value:0width$x}")),
        "D" | "d" => Str::new(&format!("{value:0width$}")),
        _ => host::unimplemented_value("System.Byte.ToString(string) with a format other than X / x / D"),
    }
}

/// The decimal digits .NET Framework formats a `float` from: 7 significant digits, then (digits, exponent) with
/// value = 0.d1d2... * 10^exponent.
fn float_digits(v: f32) -> (Vec<u8>, i32) {
    if v == 0.0 {
        return (vec![0], 0);
    }
    let s = format!("{:.6e}", v.abs());
    let (m, e) = s.split_once('e').expect("exponent");
    (m.bytes().filter(|c| c.is_ascii_digit()).map(|c| c - b'0').collect(), e.parse::<i32>().expect("exponent") + 1)
}

/// Rounds the digit string half away from zero to `decimals` fraction digits; returns the integer part digits and
/// the fraction digits.
fn round_fixed(digits: &[u8], exp: i32, decimals: usize) -> (String, String) {
    // digit k (0-based) has weight 10^(exp - 1 - k); keep digits with weight >= 10^-decimals
    let keep = exp + decimals as i32;
    let mut d: Vec<u8> = if keep <= 0 { Vec::new() } else { digits.iter().copied().chain(std::iter::repeat(0)).take(keep as usize).collect() };
    let next = if keep < 0 { 0 } else { digits.get(keep as usize).copied().unwrap_or(0) };
    let mut exp = exp;
    if next >= 5 {
        let mut i = d.len();
        loop {
            if i == 0 {
                d.insert(0, 1);
                exp += 1;
                break;
            }
            i -= 1;
            if d[i] == 9 {
                d[i] = 0;
            } else {
                d[i] += 1;
                break;
            }
        }
    }
    let int_len = exp.max(0) as usize;
    let mut all: Vec<u8> = Vec::new();
    // leading zeros when the number is below 1
    for _ in 0..(-exp).max(0) {
        all.push(0);
    }
    all.extend_from_slice(&d);
    while all.len() < int_len + decimals {
        all.push(0);
    }
    let to_s = |v: &[u8]| v.iter().map(|c| (c + b'0') as char).collect::<String>();
    let (i, f) = all.split_at(int_len.min(all.len()));
    (if i.is_empty() { "0".to_owned() } else { to_s(i) }, to_s(&f[..decimals.min(f.len())]))
}

fn group(int: &str) -> String {
    let mut out = String::new();
    for (k, c) in int.chars().enumerate() {
        if k > 0 && (int.len() - k) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// `float.ToString(format)` for `F<n>`, `N<n>` and `P<n>` (invariant / en-US: `.` decimal point, `,` groups,
/// percent pattern `n %`). .NET Framework rounds the 7-significant-digit decimal form half away from zero.
pub fn single_to_string_format(v: f32, format: Str) -> Str {
    let f = format.get().unwrap_or("");
    let (spec, digits) = f.split_at(f.len().min(1));
    let precision = |default: usize| if digits.is_empty() { default } else { digits.parse::<usize>().unwrap_or_else(|_| throw("System.FormatException", "Format specifier was invalid.")) };
    if !v.is_finite() {
        return Str::lit(if v.is_nan() { "NaN" } else if v > 0.0 { "Infinity" } else { "-Infinity" });
    }
    let (d, e) = float_digits(v);
    let join = |int: String, frac: String| if frac.is_empty() { int } else { format!("{int}.{frac}") };
    let nonzero = |int: &str, frac: &str| int.bytes().chain(frac.bytes()).any(|c| c != b'0');
    let text = match spec {
        "F" | "f" => {
            let (int, frac) = round_fixed(&d, e, precision(2));
            let sign = if v < 0.0 && nonzero(&int, &frac) { "-" } else { "" };
            format!("{sign}{}", join(int, frac))
        }
        "N" | "n" => {
            let (int, frac) = round_fixed(&d, e, precision(2));
            let sign = if v < 0.0 && nonzero(&int, &frac) { "-" } else { "" };
            format!("{sign}{}", join(group(&int), frac))
        }
        "P" | "p" => {
            let (int, frac) = round_fixed(&d, e + 2, precision(2));
            let sign = if v < 0.0 && nonzero(&int, &frac) { "-" } else { "" };
            format!("{sign}{} %", join(group(&int), frac))
        }
        _ => return host::unimplemented_value("System.Single.ToString(string) with a format other than F / N / P"),
    };
    Str::new(&text)
}

/// `Math.Round(double value, int digits)`: .NET Framework's `InternalRound` (scale, round half to even, unscale).
pub fn math_round_digits(value: f64, digits: i32) -> f64 {
    if !(0..=15).contains(&digits) {
        throw("System.ArgumentOutOfRangeException", "Rounding digits must be between 0 and 15, inclusive.");
    }
    if value.abs() < 1e16 {
        let p = 10f64.powi(digits);
        return rt::math::round(value * p) / p;
    }
    value
}

/// `Math.Log(double a, double newBase)`: the special cases of the reference source, then `Log(a) / Log(newBase)`.
pub fn math_log_base(a: f64, new_base: f64) -> f64 {
    if a.is_nan() {
        return a;
    }
    if new_base.is_nan() {
        return new_base;
    }
    if new_base == 1.0 {
        return f64::NAN;
    }
    if a != 1.0 && (new_base == 0.0 || new_base.is_infinite()) {
        return f64::NAN;
    }
    a.ln() / new_base.ln()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fmt(v: f32, f: &'static str) -> &'static str {
        single_to_string_format(v, Str::lit(f)).as_str()
    }

    /// tests/dotnet/system_probe.out, sections "Convert / Uri / Encoding" and "Number formatting".
    #[test]
    fn matches_dotnet() {
        assert_eq!(base64(&[0, 1, 2, 250, 251, 252, 253, 254, 255, 7]), "AAEC+vv8/f7/Bw==");
        assert_eq!(base64(&[]), "");
        assert_eq!(from_base64_string(Str::lit("AAEC+vv8/f7/Bw==")).as_slice(), [0, 1, 2, 250, 251, 252, 253, 254, 255, 7]);
        assert_eq!(uri_escape_data_string(Str::lit("a b&c=d/e?f#g+h~i_j-k.l!m*n'o(p)\u{e9}\u{4e2d}")).as_str(), "a%20b%26c%3Dd%2Fe%3Ff%23g%2Bh~i_j-k.l!m*n'o(p)%C3%A9%E4%B8%AD");
        assert_eq!(uri_unescape_data_string(Str::lit("a%20b%26c%C3%A9%4e%zz+%")).as_str(), "a b&c\u{e9}N%zz+%");
        let hex = |b: &[u8]| b.iter().map(|c| format!("{c:02x}")).collect::<String>();
        unsafe {
            let unicode = &*Encoding::new_gc(EncodingKind::Utf16Le).ptr();
            assert_eq!(hex(unicode.get_bytes(Str::lit("h3y_gUyZ")).as_slice()), "6800330079005f006700550079005a00");
            assert_eq!((*Encoding::new_gc(EncodingKind::Utf8).ptr()).get_byte_count(Str::lit("a\u{e9}\u{4e2d}")), 6);
            let ascii = &*Encoding::new_gc(EncodingKind::Ascii).ptr();
            assert_eq!(ascii.get_string(Arr::from_slice(&[72, 105, 200, 33])).as_str(), "Hi?!");
            assert_eq!(hex(ascii.get_bytes(Str::lit("Hi\u{e9}!")).as_slice()), "48693f21");
        }
        let x2: Vec<&str> = [0u64, 9, 10, 171, 255].iter().map(|v| int_to_string_format(*v, Str::lit("X2")).as_str()).collect();
        assert_eq!(x2, ["00", "09", "0A", "AB", "FF"]);
        let f2: Vec<&str> = [0.005f32, 1.005, 2.675, -0.004, 1234.5, 0.125, 0.135].iter().map(|v| fmt(*v, "F2")).collect();
        assert_eq!(f2, ["0.01", "1.01", "2.68", "0.00", "1234.50", "0.13", "0.14"]);
        let p0: Vec<&str> = [0.5f32, 1.25, 0.004, 0.995, -0.2].iter().map(|v| fmt(*v, "P0")).collect();
        assert_eq!(p0, ["50 %", "125 %", "0 %", "100 %", "-20 %"]);
        assert_eq!(fmt(12.5, "P0"), "1,250 %");
        assert_eq!([2.675, 2.665, 1.005, -2.5, 0.125].map(|v| math_round_digits(v, 2)), [2.68, 2.66, 1.0, -2.5, 0.12]);
        assert_eq!([0.5, 1.5, 2.5, -0.5].map(|v| math_round_digits(v, 0)), [0.0, 2.0, 2.0, -0.0]);
        assert_eq!((math_log_base(8.0, 2.0), math_log_base(100.0, 10.0)), (3.0, 2.0));
    }

    #[test]
    fn string_builder() {
        let b = StringBuilder::new_gc(Str::NULL);
        let b = unsafe { &mut *b.ptr() };
        b.append_char(b'a' as u16);
        b.append(Str::lit("bc"));
        b.append(Str::NULL);
        b.append_line(Str::lit("\u{4e2d}"));
        assert_eq!((b.to_str().as_str(), b.length()), ("abc\u{4e2d}\r\n", 6));
    }
}

#[cfg(test)]
mod builder_object_tests {
    use super::*;

    #[test]
    fn a_string_builder_held_as_object_prints_its_text() {
        let b = StringBuilder::new_gc(Str::new("abc"));
        // SAFETY: fresh object.
        unsafe { (*b.ptr()).append(Str::new("def")) };
        assert_eq!(rt::obj_to_string(b.up()).as_str(), "abcdef");
    }
}
