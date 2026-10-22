//! `System` members the game's simulation code uses: `string` methods, `Array` helpers, `Tuple`, sorting,
//! integer parsing, the sequential form of `FastParallel.For`.
//!
//! String indices and lengths are UTF-16 code units, as in C#. Comparisons are ordinal: the game runs with the
//! invariant behaviour for ASCII text; culture-sensitive collation of other text is not modelled (stated per function).

use super::collections::{CsKey, Seq};
use super::exc::{exc, throw};
use super::gc::{alloc_init, null_reference, Delegate, Gc, Marker, Obj, Trace};
use super::string::Str;
use super::{Arr, Arr2, List, Val};

#[cold]
fn arg_range(what: &'static str) -> ! {
    throw(exc("System.ArgumentOutOfRangeException", Str::lit(what)))
}
#[cold]
fn arg(what: &'static str) -> ! {
    throw(exc("System.ArgumentException", Str::lit(what)))
}
#[cold]
fn arg_null(what: &'static str) -> ! {
    throw(exc("System.ArgumentNullException", Str::lit(what)))
}

fn units(s: Str) -> Vec<u16> {
    s.as_str().encode_utf16().collect()
}
fn from_units(u: &[u16]) -> Str {
    Str::new(&String::from_utf16_lossy(u))
}

// ---------------------------------------------------------------------------------------------- string

/// `s.Substring(start)`.
pub fn substring(s: Str, start: i32) -> Str {
    let n = s.len();
    if start < 0 || start > n {
        arg_range("startIndex");
    }
    substring2(s, start, n - start)
}

/// `s.Substring(start, length)`.
pub fn substring2(s: Str, start: i32, length: i32) -> Str {
    let t = s.as_str();
    let n = s.len();
    if start < 0 || start > n {
        arg_range("startIndex");
    }
    if length < 0 || start > n - length {
        arg_range("length");
    }
    if t.is_ascii() {
        return Str::new(&t[start as usize..(start + length) as usize]);
    }
    let u = units(s);
    from_units(&u[start as usize..(start + length) as usize])
}

/// `s.ToLower()` (deviation: Unicode default case mapping, not the current culture's).
pub fn to_lower(s: Str) -> Str {
    Str::new(&s.as_str().to_lowercase())
}
/// `s.ToUpper()` (same deviation).
pub fn to_upper(s: Str) -> Str {
    Str::new(&s.as_str().to_uppercase())
}

/// .NET `char.IsWhiteSpace` for the BMP.
pub fn is_white_space(c: u16) -> bool {
    matches!(c, 0x09..=0x0D | 0x20 | 0x85 | 0xA0 | 0x1680 | 0x2000..=0x200A | 0x2028 | 0x2029 | 0x202F | 0x205F | 0x3000)
}

/// `string.IsNullOrWhiteSpace(s)`.
pub fn is_null_or_white_space(s: Str) -> bool {
    match s.get() {
        None => true,
        Some(t) => t.encode_utf16().all(is_white_space),
    }
}

fn trim_by(s: Str, start: bool, end: bool, is_trim: impl Fn(u16) -> bool) -> Str {
    let u = units(s);
    let (mut a, mut b) = (0usize, u.len());
    if start {
        while a < b && is_trim(u[a]) {
            a += 1;
        }
    }
    if end {
        while b > a && is_trim(u[b - 1]) {
            b -= 1;
        }
    }
    if a == 0 && b == u.len() {
        s
    } else {
        from_units(&u[a..b])
    }
}

/// `s.Trim()`.
pub fn trim(s: Str) -> Str {
    trim_by(s, true, true, is_white_space)
}
fn trim_set(chars: Arr<u16>) -> Vec<u16> {
    if chars.is_null() {
        Vec::new()
    } else {
        chars.as_slice().to_vec()
    }
}
/// `s.Trim(params char[])`, `TrimStart`, `TrimEnd`: null / empty set means white space.
pub fn trim_chars(s: Str, chars: Arr<u16>) -> Str {
    let set = trim_set(chars);
    trim_by(s, true, true, |c| if set.is_empty() { is_white_space(c) } else { set.contains(&c) })
}
pub fn trim_start_chars(s: Str, chars: Arr<u16>) -> Str {
    let set = trim_set(chars);
    trim_by(s, true, false, |c| if set.is_empty() { is_white_space(c) } else { set.contains(&c) })
}
pub fn trim_end_chars(s: Str, chars: Arr<u16>) -> Str {
    let set = trim_set(chars);
    trim_by(s, false, true, |c| if set.is_empty() { is_white_space(c) } else { set.contains(&c) })
}

/// `System.StringComparison.OrdinalIgnoreCase` / `InvariantCultureIgnoreCase` / `CurrentCultureIgnoreCase`.
fn ignores_case(comparison: i32) -> bool {
    matches!(comparison, 1 | 3 | 5)
}

/// `s.StartsWith(value)` (deviation: ordinal; .NET compares with the current culture here).
pub fn starts_with(s: Str, value: Str) -> bool {
    if value.is_null() {
        arg_null("value");
    }
    s.as_str().starts_with(value.as_str())
}
/// `s.StartsWith(value, comparison)`: ordinal, optionally ignoring case.
pub fn starts_with_cmp(s: Str, value: Str, comparison: i32) -> bool {
    if value.is_null() {
        arg_null("value");
    }
    if ignores_case(comparison) {
        s.as_str().to_lowercase().starts_with(&value.as_str().to_lowercase())
    } else {
        s.as_str().starts_with(value.as_str())
    }
}
pub fn ends_with(s: Str, value: Str) -> bool {
    if value.is_null() {
        arg_null("value");
    }
    s.as_str().ends_with(value.as_str())
}
pub fn ends_with_cmp(s: Str, value: Str, comparison: i32) -> bool {
    if value.is_null() {
        arg_null("value");
    }
    if ignores_case(comparison) {
        s.as_str().to_lowercase().ends_with(&value.as_str().to_lowercase())
    } else {
        s.as_str().ends_with(value.as_str())
    }
}
/// `s.Contains(value)` (ordinal in .NET too).
pub fn contains(s: Str, value: Str) -> bool {
    if value.is_null() {
        arg_null("value");
    }
    s.as_str().contains(value.as_str())
}
/// `s.IndexOf(char)`.
pub fn index_of_char(s: Str, c: u16) -> i32 {
    units(s).iter().position(|x| *x == c).map(|p| p as i32).unwrap_or(-1)
}
fn find_units(hay: &[u16], needle: &[u16], from: usize, ignore_case: bool) -> i32 {
    if needle.is_empty() {
        return from as i32;
    }
    if needle.len() > hay.len() {
        return -1;
    }
    let eq = |a: u16, b: u16| a == b || ignore_case && lower_unit(a) == lower_unit(b);
    let mut i = from;
    while i + needle.len() <= hay.len() {
        if (0..needle.len()).all(|k| eq(hay[i + k], needle[k])) {
            return i as i32;
        }
        i += 1;
    }
    -1
}
/// Simple (one unit to one unit) lower-case mapping, as the ordinal ignore-case comparison uses.
fn lower_unit(c: u16) -> u16 {
    match char::from_u32(c as u32) {
        Some(ch) => {
            let mut l = ch.to_lowercase();
            match (l.next(), l.next()) {
                (Some(x), None) if (x as u32) < 0x10000 => x as u16,
                _ => c,
            }
        }
        None => c,
    }
}
/// `s.IndexOf(value)`. Deviation: ordinal; .NET Framework searches with the current culture (the same for ASCII).
pub fn index_of_str(s: Str, value: Str) -> i32 {
    index_of_str_from(s, value, 0)
}
/// `s.IndexOf(value, startIndex)` (ordinal, see [`index_of_str`]).
pub fn index_of_str_from(s: Str, value: Str, start: i32) -> i32 {
    if value.is_null() {
        arg_null("value");
    }
    let u = units(s);
    if start < 0 || start as usize > u.len() {
        arg_range("startIndex");
    }
    find_units(&u, &units(value), start as usize, false)
}
/// `s.IndexOf(value, comparison)`: ordinal, optionally ignoring case.
pub fn index_of_str_cmp(s: Str, value: Str, comparison: i32) -> i32 {
    if value.is_null() {
        arg_null("value");
    }
    find_units(&units(s), &units(value), 0, ignores_case(comparison))
}
/// `s.IndexOf(c, startIndex)`.
pub fn index_of_char_from(s: Str, c: u16, start: i32) -> i32 {
    let u = units(s);
    if start < 0 || start as usize > u.len() {
        arg_range("startIndex");
    }
    u[start as usize..].iter().position(|x| *x == c).map(|p| p as i32 + start).unwrap_or(-1)
}
/// `s.LastIndexOf(c, startIndex)`: searches backwards from `startIndex`.
pub fn last_index_of_char_from(s: Str, c: u16, start: i32) -> i32 {
    let u = units(s);
    if u.is_empty() {
        return -1;
    }
    if start < 0 || start as usize >= u.len() {
        arg_range("startIndex");
    }
    (0..=start as usize).rev().find(|&i| u[i] == c).map(|p| p as i32).unwrap_or(-1)
}
/// `s.LastIndexOf(value)` (ordinal, see [`index_of_str`]).
pub fn last_index_of_str(s: Str, value: Str) -> i32 {
    if value.is_null() {
        arg_null("value");
    }
    let (u, v) = (units(s), units(value));
    if v.is_empty() {
        return if u.is_empty() { 0 } else { u.len() as i32 - 1 };
    }
    if v.len() > u.len() {
        return -1;
    }
    (0..=u.len() - v.len()).rev().find(|&i| u[i..i + v.len()] == v[..]).map(|p| p as i32).unwrap_or(-1)
}
/// `s.LastIndexOfAny(char[])`.
pub fn last_index_of_any(s: Str, any_of: Arr<u16>) -> i32 {
    if any_of.is_null() {
        arg_null("anyOf");
    }
    let set = any_of.as_slice();
    units(s).iter().rposition(|c| set.contains(c)).map(|p| p as i32).unwrap_or(-1)
}
/// `s.Insert(startIndex, value)`.
pub fn insert(s: Str, start: i32, value: Str) -> Str {
    if value.is_null() {
        arg_null("value");
    }
    let mut u = units(s);
    if start < 0 || start as usize > u.len() {
        arg_range("startIndex");
    }
    let v = units(value);
    u.splice(start as usize..start as usize, v);
    from_units(&u)
}
/// `s.Remove(startIndex, count)`.
pub fn remove2(s: Str, start: i32, count: i32) -> Str {
    let mut u = units(s);
    if start < 0 {
        arg_range("startIndex");
    }
    if count < 0 || start as usize > u.len() || count as usize > u.len() - start as usize {
        arg_range("count");
    }
    u.drain(start as usize..(start + count) as usize);
    from_units(&u)
}
/// `s.Split(char[], StringSplitOptions)`: options 1 = `RemoveEmptyEntries`.
pub fn split_opts(s: Str, separators: Arr<u16>, options: i32) -> Arr<Str> {
    let all = split(s, separators);
    if options & 1 == 0 {
        return all;
    }
    let kept: Vec<Str> = all.as_slice().iter().copied().filter(|p| !p.as_str().is_empty()).collect();
    Arr::from_slice(&kept)
}
/// `s.Split(string[], StringSplitOptions)`: the first separator that matches at a position wins; null or empty
/// separators are ignored; a null / empty separator array means white space.
pub fn split_strs_opts(s: Str, separators: Arr<Str>, options: i32) -> Arr<Str> {
    let seps: Vec<Vec<u16>> = if separators.is_null() { Vec::new() } else { separators.as_slice().iter().filter(|x| !x.is_null() && !x.as_str().is_empty()).map(|x| units(*x)).collect() };
    if seps.is_empty() {
        return split_opts(s, Arr::NULL, options);
    }
    let u = units(s);
    let mut parts: Vec<Str> = Vec::new();
    let (mut start, mut i) = (0usize, 0usize);
    while i <= u.len() {
        let hit = if i == u.len() { None } else { seps.iter().find(|p| u[i..].starts_with(p)) };
        if i == u.len() || hit.is_some() {
            if options & 1 == 0 || i > start {
                parts.push(from_units(&u[start..i]));
            }
            if i == u.len() {
                break;
            }
            i += hit.unwrap().len();
            start = i;
        } else {
            i += 1;
        }
    }
    Arr::from_slice(&parts)
}
/// `a.Equals(b, comparison)`: ordinal, optionally ignoring case (the culture comparisons are treated as ordinal:
/// the same for ASCII).
pub fn equals_cmp(a: Str, b: Str, comparison: i32) -> bool {
    if b.is_null() {
        return false;
    }
    let (x, y) = (units(a), units(b));
    x.len() == y.len() && find_units(&x, &y, 0, ignores_case(comparison)) == 0
}

#[cfg(test)]
mod styled_parse_tests {
    use super::*;

    /// Expected: `int.TryParse(s, NumberStyles.HexNumber, CultureInfo.InvariantCulture, out v)` on the .NET Framework.
    #[test]
    fn hex_and_integer_styles() {
        let p = |t: &str, style, bits, signed| try_parse_integer_styled(Str::new(t), style, bits, signed);
        assert_eq!(p("FF0000", 515, 32, true), Some(0xFF0000));
        assert_eq!(p(" ff ", 515, 32, true), Some(255));
        assert_eq!(p("FFFFFFFF", 515, 32, true), Some(-1));
        assert_eq!(p("FFFFFFFF", 515, 32, false), Some(0xFFFF_FFFF));
        assert_eq!(p("100000000", 515, 32, false), None);
        assert_eq!(p("0x10", 515, 32, true), None);
        assert_eq!(p("-1", 515, 32, true), None);
        assert_eq!(p("", 515, 32, true), None);
        assert_eq!(p(" -12 ", 7, 32, true), Some(-12));
        assert_eq!(p("4294967296", 7, 32, false), None);
        let a = Arr::<i32>::from_slice(&[9, 5, 3, 8, 1, 7]);
        array_sort_range(a, 1, 4);
        assert_eq!(a.as_slice(), &[9, 1, 3, 5, 8, 7]);
    }
}

#[cfg(test)]
mod string_search_tests {
    use super::*;

    /// Expected values: the same calls on the .NET Framework 4.8 (Windows PowerShell 5.1), 2026-10-11.
    #[test]
    fn string_members_match_dotnet() {
        let c = |t: &str| -> Arr<u16> { Arr::from_slice(&t.encode_utf16().collect::<Vec<u16>>()) };
        let j = |a: Arr<Str>| a.as_slice().iter().map(|x| x.as_str().to_owned()).collect::<Vec<_>>().join("|");
        let l = Str::new;
        assert_eq!(index_of_str(l("the quick  brown"), l("quick")), 4);
        assert_eq!(index_of_str_from(l("abcabc"), l("c"), 3), 5);
        assert_eq!(index_of_str_from(l("abcabc"), l("zz"), 2), -1);
        assert_eq!(index_of_str_from(l("abc"), l(""), 2), 2);
        assert_eq!(index_of_str_cmp(l("ABC"), l("bc"), 5), 1);
        assert_eq!(index_of_str_cmp(l("ABC"), l("bc"), 4), -1);
        assert_eq!(index_of_char_from(l("a.b.c"), b'.' as u16, 2), 3);
        assert_eq!(last_index_of_char_from(l("a.b.c"), b'.' as u16, 2), 1);
        assert_eq!(last_index_of_char_from(l("a.b.c"), b'.' as u16, 4), 3);
        assert_eq!(last_index_of_str(l("abcabc"), l("bc")), 4);
        assert_eq!(last_index_of_str(l("abc"), l("x")), -1);
        assert_eq!(last_index_of_any(l("a/b\\c"), c("/\\")), 3);
        assert_eq!(last_index_of_any(l("abc"), c("x")), -1);
        assert_eq!(insert(l("abcd"), 2, l("XY")).as_str(), "abXYcd");
        assert_eq!(remove2(l("abcdef"), 1, 3).as_str(), "aef");
        assert_eq!(j(split_opts(l("a,,b;c"), c(",;"), 1)), "a|b|c");
        assert_eq!(j(split_opts(l("a,,b;c"), c(",;"), 0)), "a||b|c");
        assert_eq!(j(split_strs_opts(l("a--b----c"), Arr::from_slice(&[l("--")]), 0)), "a|b||c");
        assert_eq!(j(split_strs_opts(l("a--b----c"), Arr::from_slice(&[l("--")]), 1)), "a|b|c");
        assert!(equals_cmp(l("Abc"), l("aBC"), 5));
        assert!(!equals_cmp(l("Abc"), l("aBC"), 4));
        assert_eq!(index_of_str(l("h\u{e9}llo"), l("llo")), 2);
        assert_eq!(index_of_str_from(l(""), l(""), 0), 0);
        assert_eq!(index_of_str_from(l("abc"), l(""), 3), 3);
    }
}

/// `s.Replace(old, new)`: `old` must not be empty; a null `new` removes.
pub fn replace(s: Str, old: Str, new: Str) -> Str {
    if old.is_null() {
        arg_null("oldValue");
    }
    if old.as_str().is_empty() {
        arg("String cannot be of zero length.");
    }
    Str::new(&s.as_str().replace(old.as_str(), new.get().unwrap_or("")))
}
/// `s.Replace(oldChar, newChar)`.
pub fn replace_char(s: Str, old: u16, new: u16) -> Str {
    let u: Vec<u16> = units(s).into_iter().map(|c| if c == old { new } else { c }).collect();
    from_units(&u)
}
/// `s.Split(params char[])`: empty entries are kept; a null / empty separator set means white space.
pub fn split(s: Str, separators: Arr<u16>) -> Arr<Str> {
    let set = trim_set(separators);
    let is_sep = |c: u16| if set.is_empty() { is_white_space(c) } else { set.contains(&c) };
    let u = units(s);
    let mut parts: Vec<Str> = Vec::new();
    let mut start = 0;
    for i in 0..=u.len() {
        if i == u.len() || is_sep(u[i]) {
            parts.push(from_units(&u[start..i]));
            start = i + 1;
        }
    }
    Arr::from_slice(&parts)
}
/// `string.Join(separator, string[])`: null elements count as empty.
pub fn join(separator: Str, values: Arr<Str>) -> Str {
    if values.is_null() {
        arg_null("value");
    }
    let sep = separator.get().unwrap_or("");
    let parts: Vec<&str> = values.as_slice().iter().map(|s| s.get().unwrap_or("")).collect();
    Str::new(&parts.join(sep))
}
/// `string.Join(separator, IEnumerable<string>)`.
pub fn join_seq(separator: Str, values: Seq<Str>) -> Str {
    let sep = separator.get().unwrap_or("");
    let v = values.to_vec();
    let parts: Vec<&str> = v.iter().map(|s| s.get().unwrap_or("")).collect();
    Str::new(&parts.join(sep))
}
/// `string.Join<T>(separator, IEnumerable<T>)`: the `ToString()` of every element (the virtual one, through the
/// object; a null element is the empty string).
pub fn join_seq_any<T: Val>(separator: Str, values: Seq<T>) -> Str {
    let sep = separator.get().unwrap_or("");
    let parts: Vec<String> = values
        .to_vec()
        .iter()
        .map(|x| {
            // SAFETY: an element of a runtime sequence is a valid value of its type.
            let o = unsafe { x.box_ref() };
            if o.is_null() {
                String::new()
            } else {
                super::obj_to_string(o).get().unwrap_or("").to_owned()
            }
        })
        .collect();
    Str::new(&parts.join(sep))
}
/// `s.ToCharArray()`.
pub fn to_char_array(s: Str) -> Arr<u16> {
    Arr::from_slice(&units(s))
}
/// `new string(c, count)`.
pub fn repeat_char(c: u16, count: i32) -> Str {
    if count < 0 {
        arg_range("count");
    }
    from_units(&vec![c; count as usize])
}
/// `new string(char[])`.
pub fn from_chars(chars: Arr<u16>) -> Str {
    if chars.is_null() {
        Str::lit("")
    } else {
        from_units(chars.as_slice())
    }
}
/// `string.CompareOrdinal(a, b)` and the ordering behind `a.CompareTo(b)` for ASCII text (deviation: .NET's
/// `CompareTo` / `Compare` use the current culture's collation; this is ordinal by UTF-16 unit).
pub fn compare_ordinal(a: Str, b: Str) -> i32 {
    match (a.get(), b.get()) {
        (None, None) => 0,
        (None, Some(_)) => -1,
        (Some(_), None) => 1,
        (Some(x), Some(y)) => {
            let (ux, uy): (Vec<u16>, Vec<u16>) = (x.encode_utf16().collect(), y.encode_utf16().collect());
            match ux.cmp(&uy) {
                core::cmp::Ordering::Less => -1,
                core::cmp::Ordering::Equal => 0,
                core::cmp::Ordering::Greater => 1,
            }
        }
    }
}

/// `int.Parse(s)` / `Convert.ToInt32(string)` as .NET parses with `NumberStyles.Integer`: optional white space,
/// optional sign, decimal digits. `Convert.ToInt32(null)` is 0; `int.Parse(null)` throws (`null_is_zero` = false).
pub fn parse_i32(s: Str, null_is_zero: bool) -> i32 {
    let Some(t) = s.get() else {
        if null_is_zero {
            return 0;
        }
        arg_null("s")
    };
    match parse_int(t) {
        Ok(v) if v >= i32::MIN as i64 && v <= i32::MAX as i64 => v as i32,
        Ok(_) => throw(exc("System.OverflowException", Str::lit("Value was either too large or too small for an Int32."))),
        Err(overflow) => {
            if overflow {
                throw(exc("System.OverflowException", Str::lit("Value was either too large or too small for an Int32.")))
            }
            throw(exc("System.FormatException", Str::lit("Input string was not in a correct format.")))
        }
    }
}
/// `int.TryParse(s, out result)`.
///
/// # Safety
/// `out` is the address of the `out` argument.
pub unsafe fn try_parse_i32(s: Str, out: *mut i32) -> bool {
    match s.get().map(parse_int) {
        Some(Ok(v)) if v >= i32::MIN as i64 && v <= i32::MAX as i64 => {
            *out = v as i32;
            true
        }
        _ => {
            *out = 0;
            false
        }
    }
}
/// Err(true) = digits but out of the i64 range, Err(false) = not a number.
fn parse_int(t: &str) -> Result<i64, bool> {
    let u: Vec<u16> = t.encode_utf16().collect();
    let (mut a, mut b) = (0, u.len());
    while a < b && is_white_space(u[a]) {
        a += 1;
    }
    while b > a && is_white_space(u[b - 1]) {
        b -= 1;
    }
    let mut neg = false;
    if a < b && (u[a] == b'-' as u16 || u[a] == b'+' as u16) {
        neg = u[a] == b'-' as u16;
        a += 1;
    }
    if a == b {
        return Err(false);
    }
    let mut v: i64 = 0;
    let mut overflow = false;
    for &c in &u[a..b] {
        if !(b'0' as u16..=b'9' as u16).contains(&c) {
            return Err(false);
        }
        match v.checked_mul(10).and_then(|x| x.checked_add((c - b'0' as u16) as i64)) {
            Some(x) => v = x,
            None => overflow = true,
        }
    }
    if overflow {
        Err(true)
    } else {
        Ok(if neg { -v } else { v })
    }
}

/// `Path.Combine(path1, path2)` (.NET Framework on Windows): a rooted `path2` wins; otherwise the two are
/// joined with `\\` unless `path1` already ends with a separator. (Invalid-character checks are not modelled.)
pub fn path_combine(path1: Str, path2: Str) -> Str {
    if path1.is_null() || path2.is_null() {
        arg_null("path1");
    }
    let (a, b) = (path1.as_str(), path2.as_str());
    if b.is_empty() {
        return path1;
    }
    if a.is_empty() {
        return path2;
    }
    let bb = b.as_bytes();
    let rooted = bb[0] == b'\\' || bb[0] == b'/' || (bb.len() >= 2 && bb[1] == b':');
    if rooted {
        return path2;
    }
    let last = a.as_bytes()[a.len() - 1];
    if last == b'\\' || last == b'/' || last == b':' {
        Str::new(&format!("{a}{b}"))
    } else {
        Str::new(&format!("{a}\\{b}"))
    }
}

// ---------------------------------------------------------------------------------------------- Array

/// What `Array.Clear` / `Array.Copy` need from an array handle of a statically known type.
pub trait ArrayLike: Copy {
    type Elem: Val;
    /// Address of the first element and the total element count (null array: `ArgumentNullException`).
    fn flat(self) -> (*mut Self::Elem, i32);
}
impl<T: Val> ArrayLike for Arr<T> {
    type Elem = T;
    fn flat(self) -> (*mut T, i32) {
        if self.is_null() {
            arg_null("array");
        }
        let n = self.len();
        (if n == 0 { core::ptr::NonNull::dangling().as_ptr() } else { self.at(0) }, n)
    }
}
impl<T: Val> ArrayLike for Arr2<T> {
    type Elem = T;
    fn flat(self) -> (*mut T, i32) {
        if self.is_null() {
            arg_null("array");
        }
        let n = self.len();
        (if n == 0 { core::ptr::NonNull::dangling().as_ptr() } else { self.at(0, 0) }, n)
    }
}

/// `Array.Clear(array, index, length)`: the range is set to zero / null (row-major for `T[,]`).
pub fn array_clear<A: ArrayLike>(a: A, index: i32, length: i32) {
    let (p, n) = a.flat();
    if index < 0 || length < 0 || index > n - length {
        throw(exc("System.IndexOutOfRangeException", Str::lit("Index was outside the bounds of the array.")));
    }
    // SAFETY: the range was checked against the array's length; zero is a valid value of every element type.
    unsafe { core::ptr::write_bytes(p.add(index as usize), 0, length as usize) }
}

/// `Array.Copy(source, sourceIndex, destination, destinationIndex, length)` between arrays of the same element
/// type (overlap is handled as by `memmove`, as in .NET).
pub fn array_copy<A: ArrayLike, B: ArrayLike<Elem = A::Elem>>(src: A, src_index: i32, dst: B, dst_index: i32, length: i32) {
    let (sp, sn) = src.flat();
    let (dp, dn) = dst.flat();
    if length < 0 || src_index < 0 || dst_index < 0 {
        arg_range("length");
    }
    if src_index > sn - length || dst_index > dn - length {
        arg("Source array was not long enough. Check srcIndex and length, and the array's lower bounds.");
    }
    // SAFETY: both ranges were checked.
    unsafe {
        // store barrier: the copied handles land in an array the collector may have traced already
        super::gc::wb_slice(core::slice::from_raw_parts(sp.add(src_index as usize), length as usize));
        core::ptr::copy(sp.add(src_index as usize), dp.add(dst_index as usize), length as usize)
    }
}

/// `Array.Resize(ref array, newSize)`: a new array with the first `min(old, new)` elements (null: a new array).
///
/// # Safety
/// `array` is the address of the `ref` argument.
pub unsafe fn array_resize<T: Val>(array: *mut Arr<T>, new_size: i32) {
    if new_size < 0 {
        arg_range("newSize");
    }
    let old = *array;
    if !old.is_null() && old.len() == new_size {
        return;
    }
    let fresh = Arr::<T>::new(new_size);
    if !old.is_null() {
        let n = old.len().min(new_size);
        if n > 0 {
            core::ptr::copy_nonoverlapping(old.at(0), fresh.at(0), n as usize);
        }
    }
    *array = fresh;
}

/// `Array.IndexOf(array, value)`.
pub fn array_index_of<T: CsKey>(a: Arr<T>, v: T) -> i32 {
    if a.is_null() {
        arg_null("array");
    }
    a.as_slice().iter().position(|x| x.cs_eq(&v)).map(|p| p as i32).unwrap_or(-1)
}

// ---------------------------------------------------------------------------------------------- sorting

/// `ArraySortHelper<T>.DepthLimitedQuickSort`: the algorithm `Array.Sort` / `List<T>.Sort` run with a comparer
/// when the program targets .NET Framework 4.0 (Terraria does: `TargetFramework` net40, so
/// `BinaryCompatibility.TargetsAtLeast_Desktop_V4_5` is false and the introsort of 4.5 is not used).
/// It is not stable; which of two equal elements comes first is part of the game's behaviour, hence the port
/// of the exact algorithm, comparison for comparison. `cmp(a, b)` is `comparer.Compare(a, b)`.
pub fn depth_limited_quick_sort<T: Copy>(keys: &mut [T], cmp: &mut dyn FnMut(T, T) -> i32) {
    if keys.len() < 2 {
        return;
    }
    dlqs(keys, 0, keys.len() as i32 - 1, cmp, 32);
}

fn swap_if_greater<T: Copy>(keys: &mut [T], cmp: &mut dyn FnMut(T, T) -> i32, a: i32, b: i32) {
    if a != b && cmp(keys[a as usize], keys[b as usize]) > 0 {
        keys.swap(a as usize, b as usize);
    }
}

fn dlqs<T: Copy>(keys: &mut [T], mut left: i32, mut right: i32, cmp: &mut dyn FnMut(T, T) -> i32, mut depth_limit: i32) {
    loop {
        if depth_limit == 0 {
            heapsort(keys, left, right, cmp);
            return;
        }
        let mut i = left;
        let mut j = right;
        let middle = i + ((j - i) >> 1);
        swap_if_greater(keys, cmp, i, middle);
        swap_if_greater(keys, cmp, i, j);
        swap_if_greater(keys, cmp, middle, j);
        let x = keys[middle as usize];
        loop {
            while cmp(keys[i as usize], x) < 0 {
                i += 1;
            }
            while cmp(x, keys[j as usize]) < 0 {
                j -= 1;
            }
            if i > j {
                break;
            }
            if i < j {
                keys.swap(i as usize, j as usize);
            }
            i += 1;
            j -= 1;
            if i > j {
                break;
            }
        }
        depth_limit -= 1;
        if j - left <= right - i {
            if left < j {
                dlqs(keys, left, j, cmp, depth_limit);
            }
            left = i;
        } else {
            if i < right {
                dlqs(keys, i, right, cmp, depth_limit);
            }
            right = j;
        }
        if left >= right {
            return;
        }
    }
}

fn heapsort<T: Copy>(keys: &mut [T], lo: i32, hi: i32, cmp: &mut dyn FnMut(T, T) -> i32) {
    let n = hi - lo + 1;
    let mut i = n / 2;
    while i >= 1 {
        down_heap(keys, i, n, lo, cmp);
        i -= 1;
    }
    let mut i = n;
    while i > 1 {
        keys.swap(lo as usize, (lo + i - 1) as usize);
        down_heap(keys, 1, i - 1, lo, cmp);
        i -= 1;
    }
}

fn down_heap<T: Copy>(keys: &mut [T], mut i: i32, n: i32, lo: i32, cmp: &mut dyn FnMut(T, T) -> i32) {
    let d = keys[(lo + i - 1) as usize];
    while i <= n / 2 {
        let mut child = 2 * i;
        if child < n && cmp(keys[(lo + child - 1) as usize], keys[(lo + child) as usize]) < 0 {
            child += 1;
        }
        if !(cmp(d, keys[(lo + child - 1) as usize]) < 0) {
            break;
        }
        keys[(lo + i - 1) as usize] = keys[(lo + child - 1) as usize];
        i = child;
    }
    keys[(lo + i - 1) as usize] = d;
}

/// `list.Sort(comparison)`. The comparison runs generated code, so the list is sorted in a copy and written
/// back (a comparison that modifies the list throws in .NET; here its changes are overwritten).
pub fn list_sort_by<T: Val>(list: Gc<List<T>>, comparison: Delegate<dyn Fn(T, T) -> i32>) {
    if comparison.is_null() {
        arg_null("comparison");
    }
    // SAFETY: `ptr` null-checks; the references end with each statement.
    unsafe {
        let mut v = (*list.ptr()).snapshot();
        depth_limited_quick_sort(&mut v, &mut |a, b| comparison.invoke(|f| f(a, b)));
        (*list.0).replace(v);
    }
}

/// `list.Sort(IComparer<T>)`: `compare` is the comparer's `Compare` (the generated dispatcher of the interface).
/// A null comparer is `Comparer<T>.Default`, which the caller must map to [`list_sort`] instead.
pub fn list_sort_with<T: Val>(list: Gc<List<T>>, mut compare: impl FnMut(T, T) -> i32) {
    // SAFETY: `ptr` null-checks; the references end with each statement.
    unsafe {
        let mut v = (*list.ptr()).snapshot();
        depth_limited_quick_sort(&mut v, &mut |a, b| compare(a, b));
        (*list.0).replace(v);
    }
}
/// `ConditionalWeakTable<K, V>.GetValue(key, createValueCallback)`: the value stored for `key`, created by the
/// callback on first use. The table is the runtime's `Dictionary` (native.d/rt_json.json): its entries live as long
/// as the table, where the CLR drops an entry when its key is collected. The game keys its tables by effect
/// parameters and `Type` objects, which live as long as the process.
pub fn cwt_get_value<K: crate::rt::CsKey, V: Val>(table: Gc<crate::rt::Dictionary<K, V>>, key: K, create: Delegate<dyn Fn(K) -> V>) -> V {
    // SAFETY: `ptr()` checks null; no reference is alive while the callback (translated code) runs.
    unsafe {
        let mut v: V = crate::rt::zero();
        if (*table.ptr()).try_get_value(key, &mut v) {
            return v;
        }
        if create.is_null() {
            arg_null("createValueCallback");
        }
        let v = create.invoke(|f| f(key));
        (*table.ptr()).add(key, v);
        v
    }
}

/// `ushort.Parse(s)` (`NumberStyles.Integer`): the port a player types (`Main.OnSubmitServerPort`).
pub fn parse_u16(s: Str) -> u16 {
    let Some(t) = s.get() else { arg_null("s") };
    match parse_int(t) {
        Ok(v) if (0..=u16::MAX as i64).contains(&v) => v as u16,
        Ok(_) | Err(true) => throw(exc("System.OverflowException", Str::lit("Value was either too large or too small for a UInt16."))),
        Err(false) => throw(exc("System.FormatException", Str::lit("Input string was not in a correct format."))),
    }
}
/// `ushort.TryParse(s, out result)` (`Main.IsGoodPortAddress`).
///
/// # Safety
/// `out` is the address of the `out` argument.
pub unsafe fn try_parse_u16(s: Str, out: *mut u16) -> bool {
    match s.get().map(parse_int) {
        Some(Ok(v)) if (0..=u16::MAX as i64).contains(&v) => {
            *out = v as u16;
            true
        }
        _ => {
            *out = 0;
            false
        }
    }
}

/// `int.TryParse(s, NumberStyles, provider, out result)` / `uint.TryParse(..)` for the styles the game passes:
/// `NumberStyles.HexNumber` (515: optional white space around hexadecimal digits, no sign, no prefix) and
/// `NumberStyles.Integer` (7: white space, an optional leading sign, decimal digits). `bits` is the width of the
/// target type and `signed` its signedness: a hexadecimal value is the bit pattern (`"FFFFFFFF"` is -1 for `int`),
/// longer input fails. Any other style is a host member that is not implemented.
pub fn try_parse_integer_styled(s: Str, style: i32, bits: u32, signed: bool) -> Option<i64> {
    let text = s.get()?.trim_matches(|c: char| c == ' ' || ('\u{9}'..='\u{d}').contains(&c));
    match style {
        515 => {
            if text.is_empty() || text.len() > (bits / 4) as usize || !text.bytes().all(|b| b.is_ascii_hexdigit()) {
                return None;
            }
            let v = u64::from_str_radix(text, 16).ok()?;
            Some(if signed && bits < 64 && v >> (bits - 1) & 1 == 1 { v as i64 - (1i64 << bits) } else { v as i64 })
        }
        7 => {
            let v: i128 = text.parse().ok()?;
            let (lo, hi) = if signed { (-(1i128 << (bits - 1)), (1i128 << (bits - 1)) - 1) } else { (0, (1i128 << bits) - 1) };
            if v < lo || v > hi {
                None
            } else {
                Some(v as i64)
            }
        }
        _ => crate::host::unimplemented_value("System.Int32.TryParse(string, NumberStyles, IFormatProvider, out int) with a style other than HexNumber / Integer"),
    }
}

/// `Array.Sort(array, index, length)` for primitives: the range in ascending order (the introspective sort of
/// .NET 4.5+ and the quicksort of 4.0 give the same result for a total order without equal-but-distinguishable keys).
pub fn array_sort_range<T: CsOrd>(a: Arr<T>, index: i32, length: i32) {
    if a.is_null() {
        arg_null("array");
    }
    if index < 0 || length < 0 {
        arg_range(if index < 0 { "index" } else { "length" });
    }
    if a.len() - index < length {
        arg("Offset and length were out of bounds for the array or count is greater than the number of elements from index to the end of the source collection.");
    }
    let mut v = a.as_slice()[index as usize..(index + length) as usize].to_vec();
    depth_limited_quick_sort(&mut v, &mut |x, y| x.cs_cmp(&y));
    for (i, x) in v.into_iter().enumerate() {
        // SAFETY: inside the checked range.
        unsafe { *a.at(index + i as i32) = super::gc::wbv(x) };
    }
}

/// `list.Find(match)`: the first element the predicate accepts, else `default(T)`.
pub fn list_find<T: Val>(list: Gc<List<T>>, pred: Delegate<dyn Fn(T) -> bool>) -> T {
    let i = list_find_index(list, pred);
    // SAFETY: the zero pattern is `default(T)`; the index was just found.
    unsafe {
        if i < 0 {
            crate::rt::zero()
        } else {
            (*list.ptr()).get_item(i)
        }
    }
}
/// `list.FindIndex(match)`
pub fn list_find_index<T: Val>(list: Gc<List<T>>, pred: Delegate<dyn Fn(T) -> bool>) -> i32 {
    if pred.is_null() {
        arg_null("match");
    }
    // SAFETY: the predicate runs on a snapshot (it is translated code and may touch the list).
    let v = unsafe { (*list.ptr()).snapshot() };
    v.iter().position(|x| pred.invoke(|f| f(*x))).map(|p| p as i32).unwrap_or(-1)
}
/// `list.FindLastIndex(match)`
pub fn list_find_last_index<T: Val>(list: Gc<List<T>>, pred: Delegate<dyn Fn(T) -> bool>) -> i32 {
    if pred.is_null() {
        arg_null("match");
    }
    // SAFETY: as above.
    let v = unsafe { (*list.ptr()).snapshot() };
    v.iter().rposition(|x| pred.invoke(|f| f(*x))).map(|p| p as i32).unwrap_or(-1)
}
/// `list.FindAll(match)`: a new list of the accepted elements, in order.
pub fn list_find_all<T: Val>(list: Gc<List<T>>, pred: Delegate<dyn Fn(T) -> bool>) -> Gc<List<T>> {
    if pred.is_null() {
        arg_null("match");
    }
    // SAFETY: as above.
    let v = unsafe { (*list.ptr()).snapshot() };
    let out = List::<T>::new_gc();
    for x in v {
        if pred.invoke(|f| f(x)) {
            // SAFETY: fresh list.
            unsafe { (*out.ptr()).add(x) };
        }
    }
    out
}
/// `list.Reverse(index, count)`
pub fn list_reverse_range<T: Val>(list: Gc<List<T>>, index: i32, count: i32) {
    // SAFETY: `ptr` null-checks; the references end with each statement.
    unsafe {
        let mut v = (*list.ptr()).snapshot();
        if index < 0 || count < 0 {
            arg_range(if index < 0 { "index" } else { "count" });
        }
        if v.len() < index as usize || v.len() - (index as usize) < count as usize {
            arg("Offset and length were out of bounds for the array or count is greater than the number of elements from index to the end of the source collection.");
        }
        v[index as usize..(index + count) as usize].reverse();
        (*list.0).replace(v);
    }
}

/// `array.Sort(comparison)`.
pub fn array_sort_by<T: Val>(a: Arr<T>, comparison: Delegate<dyn Fn(T, T) -> i32>) {
    if a.is_null() {
        arg_null("array");
    }
    let mut v = a.as_slice().to_vec();
    depth_limited_quick_sort(&mut v, &mut |x, y| comparison.invoke(|f| f(x, y)));
    for (i, x) in v.into_iter().enumerate() {
        // SAFETY: same length as the array.
        unsafe { *a.at(i as i32) = super::gc::wbv(x) };
    }
}

/// Default ordering of a primitive (`Comparer<T>.Default`): what `list.Sort()` / `Array.Sort(array)` use.
pub trait CsOrd: Val {
    fn cs_cmp(&self, o: &Self) -> i32;
}
macro_rules! ord_int {
    ($($t:ty),*) => {$( impl CsOrd for $t {
        fn cs_cmp(&self, o: &Self) -> i32 { if self < o { -1 } else if self > o { 1 } else { 0 } }
    } )*};
}
ord_int!(bool, u8, i8, u16, i16, u32, i32, u64, i64);
macro_rules! ord_float {
    ($($t:ty),*) => {$( impl CsOrd for $t {
        /// `float.CompareTo`: NaN is smaller than everything and equal to NaN.
        fn cs_cmp(&self, o: &Self) -> i32 {
            if self < o { -1 } else if self > o { 1 } else if self == o { 0 } else if self.is_nan() { if o.is_nan() { 0 } else { -1 } } else { 1 }
        }
    } )*};
}
ord_float!(f32, f64);
impl CsOrd for Str {
    /// `Comparer<string>.Default` is the culture-sensitive comparison (`string_compare`: the en-US model).
    fn cs_cmp(&self, o: &Self) -> i32 {
        string_compare(*self, *o)
    }
}

/// `a.CompareTo(b)` for primitives and strings.
pub fn compare_to<T: CsOrd>(a: T, b: T) -> i32 {
    a.cs_cmp(&b)
}

/// `x.CompareTo(object value)` of a primitive (`Double.CompareTo(object)`, `Single`, `Byte`: the generic
/// `Utils.Max<T>` / `Clamp<T>` call them through `IComparable`). .NET: a null value is less than every value (1); a
/// boxed value of the same type compares as `CompareTo(T)`; anything else throws `ArgumentException`
/// ("Object must be of type Double.").
///
/// # Safety
/// `type_id` must be the boxing id of `T`.
pub unsafe fn compare_to_obj<T: CsOrd + Copy>(this: T, value: Obj, type_id: u32, type_name: &str) -> i32 {
    if value.is_null() {
        return 1;
    }
    if !super::gc::is_boxed(value, type_id) {
        throw(exc("System.ArgumentException", Str::new(&format!("Object must be of type {type_name}."))));
    }
    this.cs_cmp(&*(value.0 as *const T))
}

/// `e.HasFlag(flag)` on a boxed enum `this` (TeleporterGenerator's placement flags, secret-seed worlds with
/// teleporters): `ArgumentNullException` for a null flag, `ArgumentException` for a flag of another enum type,
/// otherwise `(this & flag) == flag` on the underlying numbers.
pub unsafe fn enum_has_flag(this: Obj, flag: Obj) -> bool {
    if flag.is_null() {
        arg_null("flag");
    }
    let t = super::gc::type_id_of(this.0 as *const u8);
    if super::gc::type_id_of(flag.0 as *const u8) != t {
        throw(exc("System.ArgumentException", Str::new("The argument type is not the same as the enum type.")));
    }
    let (v, f) = (super::object::convert_to_i64(this), super::object::convert_to_i64(flag));
    v & f == f
}

/// `list.Sort()` for primitives (equal primitives are indistinguishable, so only the order matters).
pub fn list_sort<T: CsOrd>(list: Gc<List<T>>) {
    // SAFETY: `ptr` null-checks; the references end with each statement.
    unsafe {
        let mut v = (*list.ptr()).snapshot();
        depth_limited_quick_sort(&mut v, &mut |a, b| a.cs_cmp(&b));
        (*list.0).replace(v);
    }
}
/// `Array.Sort(array)` for primitives.
pub fn array_sort<T: CsOrd>(a: Arr<T>) {
    if a.is_null() {
        arg_null("array");
    }
    let mut v = a.as_slice().to_vec();
    depth_limited_quick_sort(&mut v, &mut |x, y| x.cs_cmp(&y));
    for (i, x) in v.into_iter().enumerate() {
        // SAFETY: same length as the array.
        unsafe { *a.at(i as i32) = super::gc::wbv(x) };
    }
}

// ---------------------------------------------------------------------------------------------- Tuple

/// `System.Tuple<T1, T2>` (a class: reference identity, immutable).
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Tuple2<A, B> {
    pub item1: A,
    pub item2: B,
}
unsafe impl<A: Trace, B: Trace> Trace for Tuple2<A, B> {
    unsafe fn trace(&self, m: &mut Marker) {
        self.item1.trace(m);
        self.item2.trace(m);
    }
}
impl<A: Val, B: Val> Tuple2<A, B> {
    pub fn new_gc(item1: A, item2: B) -> Gc<Self> {
        alloc_init(Tuple2 { item1, item2 })
    }
}
/// `System.Tuple<T1, T2, T3>`.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Tuple3<A, B, C> {
    pub item1: A,
    pub item2: B,
    pub item3: C,
}
unsafe impl<A: Trace, B: Trace, C: Trace> Trace for Tuple3<A, B, C> {
    unsafe fn trace(&self, m: &mut Marker) {
        self.item1.trace(m);
        self.item2.trace(m);
        self.item3.trace(m);
    }
}
impl<A: Val, B: Val, C: Val> Tuple3<A, B, C> {
    pub fn new_gc(item1: A, item2: B, item3: C) -> Gc<Self> {
        alloc_init(Tuple3 { item1, item2, item3 })
    }
}

// ---------------------------------------------------------------------------------------------- DoubleStack<T>

/// `Terraria.DataStructures.DoubleStack<T1>` (DoubleStack.cs), ported line by line because the generator does not
/// emit generic game classes yet. The original's quirks are kept: `PopBack` / `PeekBack` read the slot AT `_end`
/// (one past the last pushed element), and `PopBack` does not shrink back past a segment.
pub struct DoubleStack<T> {
    segment_list: Vec<Vec<T>>,
    segment_size: i32,
    segment_count: i32,
    segment_shift_position: i32,
    start: i32,
    end: i32,
    size: i32,
    last: i32,
}
unsafe impl<T: Val> Trace for DoubleStack<T> {
    const NEEDS_TRACE: bool = T::NEEDS_TRACE;
    unsafe fn trace(&self, m: &mut Marker) {
        if T::NEEDS_TRACE {
            for s in &self.segment_list {
                s.trace(m)
            }
        }
    }
}
impl<T: Val> DoubleStack<T> {
    fn zero() -> T {
        // SAFETY: zero is `default(T)` (runtime invariant).
        unsafe { core::mem::zeroed() }
    }
    fn empty() -> ! {
        throw(exc("System.InvalidOperationException", Str::lit("The DoubleStack is empty.")))
    }
    fn slot(&mut self, pos: i32) -> &mut T {
        let (seg, at) = (pos / self.segment_size, pos % self.segment_size);
        match self.segment_list.get_mut(seg as usize).filter(|_| pos >= 0).and_then(|s| s.get_mut(at as usize)) {
            Some(x) => x,
            None => throw(exc("System.IndexOutOfRangeException", Str::lit("Index was outside the bounds of the array."))),
        }
    }
    /// `new DoubleStack<T>(segmentSize = 1024, initialSize = 0)`.
    pub fn new_gc(mut segment_size: i32, mut initial_size: i32) -> Gc<Self> {
        if segment_size < 16 {
            segment_size = 16;
        }
        let start = segment_size / 2;
        initial_size = initial_size.wrapping_add(start);
        let num = initial_size / segment_size + 1;
        alloc_init(DoubleStack {
            segment_list: (0..num).map(|_| vec![Self::zero(); segment_size as usize]).collect(),
            segment_size,
            segment_count: num,
            segment_shift_position: segment_size + start,
            start,
            end: start,
            size: 0,
            last: segment_size * num - 1,
        })
    }
    pub fn count(&self) -> i32 {
        self.size
    }
    pub fn push_front(&mut self, front: T) {
        if self.start == 0 {
            self.segment_list.insert(0, vec![Self::zero(); self.segment_size as usize]);
            self.segment_count += 1;
            self.start += self.segment_size;
            self.end += self.segment_size;
            self.last += self.segment_size;
        }
        self.start -= 1;
        let p = self.start;
        *self.slot(p) = front;
        self.size += 1;
    }
    pub fn pop_front(&mut self) -> T {
        if self.size == 0 {
            Self::empty();
        }
        let p = self.start;
        let result = *self.slot(p);
        *self.slot(p) = Self::zero();
        self.start += 1;
        self.size -= 1;
        if self.start >= self.segment_shift_position {
            let first = self.segment_list.remove(0);
            self.segment_list.push(first);
            self.start -= self.segment_size;
            self.end -= self.segment_size;
        }
        if self.size == 0 {
            self.start = self.segment_size / 2;
            self.end = self.start;
        }
        result
    }
    pub fn peek_front(&mut self) -> T {
        if self.size == 0 {
            Self::empty();
        }
        let p = self.start;
        *self.slot(p)
    }
    pub fn push_back(&mut self, back: T) {
        if self.end == self.last {
            self.segment_list.push(vec![Self::zero(); self.segment_size as usize]);
            self.segment_count += 1;
            self.last += self.segment_size;
        }
        let p = self.end;
        *self.slot(p) = back;
        self.end += 1;
        self.size += 1;
    }
    /// As in the original: reads the slot at `_end`, not the last pushed element.
    pub fn pop_back(&mut self) -> T {
        if self.size == 0 {
            Self::empty();
        }
        let p = self.end;
        let result = *self.slot(p);
        *self.slot(p) = Self::zero();
        self.end -= 1;
        self.size -= 1;
        if self.size == 0 {
            self.start = self.segment_size / 2;
            self.end = self.start;
        }
        result
    }
    /// As in the original: the slot at `_end`.
    pub fn peek_back(&mut self) -> T {
        if self.size == 0 {
            Self::empty();
        }
        let p = self.end;
        *self.slot(p)
    }
    pub fn clear(&mut self, quick_clear: bool) {
        if !quick_clear {
            for s in &mut self.segment_list {
                for x in s.iter_mut() {
                    *x = Self::zero();
                }
            }
        }
        self.start = self.segment_size / 2;
        self.end = self.start;
        self.size = 0;
    }
}

// ---------------------------------------------------------------------------------------------- threading

/// `ReLogic.Threading.FastParallel.For(from, to, callback, context)` with `ForceTasksOnCallingThread = true`:
/// one range, on the calling thread (FastParallel.cs: `num2 = 1`). The generated code is single-threaded
/// (DESIGN.md section 1); the callbacks of the game partition their work by index, so the result is the same.
pub fn fast_parallel_for(from: i32, to: i32, callback: Delegate<dyn Fn(i32, i32, Obj)>, context: Obj) {
    if to.wrapping_sub(from) == 0 {
        return;
    }
    if callback.is_null() {
        null_reference();
    }
    callback.invoke(|f| f(from, to, context))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rt::try_;
    use crate::s;

    fn kind<R>(r: Result<R, Gc<crate::rt::Exception>>) -> &'static str {
        match r {
            Ok(_) => "ok",
            Err(e) => unsafe { (*e.ptr()).kind },
        }
    }

    #[test]
    fn substring_counts_utf16_units_and_checks_ranges() {
        assert_eq!(substring(s!("hello"), 2).as_str(), "llo");
        assert_eq!(substring(s!("hello"), 5).as_str(), "");
        assert_eq!(substring2(s!("hello"), 1, 3).as_str(), "ell");
        // U+1F600 is two UTF-16 units: index 2 is the 'b'
        assert_eq!(substring2(s!("\u{1F600}b\u{e9}"), 2, 2).as_str(), "b\u{e9}");
        assert_eq!(kind(try_(|| substring(s!("abc"), 4))), "System.ArgumentOutOfRangeException");
        assert_eq!(kind(try_(|| substring2(s!("abc"), 1, 3))), "System.ArgumentOutOfRangeException");
        assert_eq!(kind(try_(|| substring2(s!("abc"), -1, 1))), "System.ArgumentOutOfRangeException");
        assert_eq!(kind(try_(|| substring(Str::NULL, 0))), "System.NullReferenceException");
    }

    #[test]
    fn trim_split_join_replace() {
        assert_eq!(trim(s!("  a b\t\n")).as_str(), "a b");
        assert_eq!(trim_end_chars(s!("xxaxx"), Arr::from_slice(&[b'x' as u16])).as_str(), "xxa");
        assert_eq!(trim_start_chars(s!("xxaxx"), Arr::from_slice(&[b'x' as u16])).as_str(), "axx");
        assert_eq!(trim_chars(s!(" a "), Arr::NULL).as_str(), "a");
        let parts = split(s!("a,,b,"), Arr::from_slice(&[b',' as u16]));
        let got: Vec<&str> = parts.as_slice().iter().map(|p| p.as_str()).collect();
        assert_eq!(got, ["a", "", "b", ""]); // .NET keeps empty entries
        assert_eq!(split(s!("a b"), Arr::NULL).len(), 2); // no separators: white space
        assert_eq!(join(s!("-"), Arr::from_slice(&[s!("a"), Str::NULL, s!("c")])).as_str(), "a--c");
        assert_eq!(replace(s!("aXbX"), s!("X"), Str::NULL).as_str(), "ab");
        assert_eq!(replace_char(s!("a.b"), b'.' as u16, b'_' as u16).as_str(), "a_b");
        assert_eq!(kind(try_(|| replace(s!("a"), s!(""), s!("b")))), "System.ArgumentException");
        assert!(is_null_or_white_space(Str::NULL) && is_null_or_white_space(s!(" \t")) && !is_null_or_white_space(s!(" a")));
        assert!(starts_with_cmp(s!("Hello"), s!("heL"), 5) && !starts_with_cmp(s!("Hello"), s!("heL"), 4));
        assert_eq!(index_of_char(s!("abc"), b'c' as u16), 2);
        assert_eq!(repeat_char(b'=' as u16, 3).as_str(), "===");
    }

    #[test]
    fn path_combine_like_net_on_windows() {
        assert_eq!(path_combine(s!("a"), s!("b")).as_str(), "a\\b");
        assert_eq!(path_combine(s!("a\\"), s!("b")).as_str(), "a\\b");
        assert_eq!(path_combine(s!("a/"), s!("b")).as_str(), "a/b");
        assert_eq!(path_combine(s!("a"), s!("C:\\x")).as_str(), "C:\\x");
        assert_eq!(path_combine(s!("a"), s!("\\x")).as_str(), "\\x");
        assert_eq!(path_combine(s!(""), s!("b")).as_str(), "b");
        assert_eq!(path_combine(s!("a"), s!("")).as_str(), "a");
    }

    #[test]
    fn integer_parsing_like_net() {
        assert_eq!(parse_i32(s!(" -42 "), false), -42);
        assert_eq!(parse_i32(s!("+7"), false), 7);
        assert_eq!(parse_i32(Str::NULL, true), 0);
        assert_eq!(kind(try_(|| parse_i32(Str::NULL, false))), "System.ArgumentNullException");
        assert_eq!(kind(try_(|| parse_i32(s!("12a"), false))), "System.FormatException");
        assert_eq!(kind(try_(|| parse_i32(s!(""), false))), "System.FormatException");
        assert_eq!(kind(try_(|| parse_i32(s!("2147483648"), false))), "System.OverflowException");
        assert_eq!(parse_i32(s!("-2147483648"), false), i32::MIN);
        let mut v = 5;
        unsafe {
            assert!(try_parse_i32(s!("31"), &mut v) && v == 31);
            assert!(!try_parse_i32(s!("x"), &mut v) && v == 0);
            let mut p = 1u16;
            assert!(try_parse_u16(s!(" 47800 "), &mut p) && p == 47800 && parse_u16(s!("7777")) == 7777);
            assert!(!try_parse_u16(s!("65536"), &mut p) && p == 0 && !try_parse_u16(s!("-1"), &mut p) && !try_parse_u16(s!("77a"), &mut p));
            assert!(!try_parse_i32(Str::NULL, &mut v));
        }
    }

    #[test]
    fn array_helpers() {
        let a = Arr::<i32>::from_slice(&[1, 2, 3, 4, 5]);
        array_clear(a, 1, 3);
        assert_eq!(a.as_slice(), [1, 0, 0, 0, 5]);
        assert_eq!(kind(try_(|| array_clear(a, 3, 3))), "System.IndexOutOfRangeException");
        let b = Arr::<i32>::from_slice(&[9, 8, 7, 6, 5]);
        array_copy(b, 1, a, 0, 3);
        assert_eq!(a.as_slice(), [8, 7, 6, 0, 5]);
        array_copy(a, 0, a, 1, 4); // overlapping, like memmove
        assert_eq!(a.as_slice(), [8, 8, 7, 6, 0]);
        assert_eq!(kind(try_(|| array_copy(b, 3, a, 0, 3))), "System.ArgumentException");
        let mut c = a;
        unsafe {
            array_resize(&mut c, 7);
            assert_eq!(c.as_slice(), [8, 8, 7, 6, 0, 0, 0]);
            assert!(c != a);
            array_resize(&mut c, 2);
            assert_eq!(c.as_slice(), [8, 8]);
            let mut n: Arr<i32> = Arr::NULL;
            array_resize(&mut n, 2);
            assert_eq!(n.as_slice(), [0, 0]);
        }
        let g = Arr2::<u8>::from_rows(&[&[1, 2], &[3, 4]]);
        array_clear(g, 1, 2);
        unsafe { assert_eq!((*g.at(0, 0), *g.at(0, 1), *g.at(1, 0), *g.at(1, 1)), (1, 0, 0, 4)) };
        assert_eq!(array_index_of(b, 7), 2);
        assert_eq!(array_index_of(b, 70), -1);
    }

    /// The quick sort of .NET Framework 4.0 is not stable. Expected orders below were derived by hand from the
    /// algorithm (ArraySortHelper<T>.DepthLimitedQuickSort): pairs are (key, tag), compared by key only.
    #[test]
    fn sort_is_the_framework_4_0_quick_sort() {
        let mut cmp = |a: (i32, char), b: (i32, char)| if a.0 < b.0 { -1 } else if a.0 > b.0 { 1 } else { 0 };
        // [1a 1b 1c]: i=0, j=2, middle=1; no swaps (none greater); x=1b. Scan: keys[0]<x? no. x<keys[2]? no. i<j: swap 0,2
        // -> [1c 1b 1a], i=1, j=1. Scan again: no moves; i == j: no swap, i=2, j=0. Left part (0..0) and right part (2..2)
        // are single elements. Result: 1c 1b 1a.
        let mut v = vec![(1, 'a'), (1, 'b'), (1, 'c')];
        depth_limited_quick_sort(&mut v, &mut cmp);
        assert_eq!(v, [(1, 'c'), (1, 'b'), (1, 'a')]);
        // [2a 1b 2c 1d]: middle = 0 + (3 >> 1) = 1. SwapIfGreater(0,1): 2a > 1b -> [1b 2a 2c 1d]. (0,3): 1b > 1d? no.
        // (1,3): 2a > 1d -> [1b 1d 2c 2a]. x = 1d. i: keys[0]=1b < 1d? no. j: 1d < keys[3]=2a yes j=2; 1d < 2c yes j=1;
        // 1d < 1d no. i=0 < j=1: swap -> [1d 1b 2c 2a], i=1, j=0: loop ends. j-left = 0 <= right-i = 2: left part empty,
        // continue with (1..3): middle = 2. (1,2): 1b > 2c? no. (1,3): no. (2,3): 2c > 2a? no. x = 2c. i: 1b < 2c -> i=2;
        // 2c < 2c no. j: 2c < 2a? no (j=3). i=2 < j=3: swap -> [1d 1b 2a 2c], i=3, j=2. Done.
        let mut v = vec![(2, 'a'), (1, 'b'), (2, 'c'), (1, 'd')];
        depth_limited_quick_sort(&mut v, &mut cmp);
        assert_eq!(v, [(1, 'd'), (1, 'b'), (2, 'a'), (2, 'c')]);
        // plain keys: any correct sort gives the same result
        let mut k: Vec<i32> = (0..500).map(|i| (i * 7919 + 13) % 257 - 100).collect();
        let mut want = k.clone();
        want.sort();
        depth_limited_quick_sort(&mut k, &mut |a, b| a.cs_cmp(&b));
        assert_eq!(k, want);
        // the depth limit of 32 only matters for adversarial input; the heap sort it falls back to must sort too
        let mut h: Vec<i32> = (0..200).rev().collect();
        heapsort(&mut h, 0, 199, &mut |a, b| a.cs_cmp(&b));
        assert_eq!(h, (0..200).collect::<Vec<_>>());
    }

    #[test]
    fn double_stack_like_the_original() {
        let d = DoubleStack::<i32>::new_gc(16, 0);
        let s = unsafe { &mut *d.ptr() };
        // front pushes come back last-in first-out; back pushes are read from the front in order
        s.push_back(1);
        s.push_back(2);
        s.push_front(0);
        assert_eq!((s.count(), s.peek_front()), (3, 0));
        assert_eq!((s.pop_front(), s.pop_front(), s.pop_front(), s.count()), (0, 1, 2, 0));
        assert!(try_(|| s.pop_front()).is_err());
        // the original's PopBack reads the slot AT _end (after the last element): default(T), not the element
        s.push_back(7);
        assert_eq!(s.peek_back(), 0);
        assert_eq!((s.pop_back(), s.count()), (0, 0));
        // growth at both ends, then the wire-list usage of Wiring: push back, pop front
        for i in 0..100 {
            s.push_back(i);
        }
        for i in 0..40 {
            s.push_front(-i);
        }
        assert_eq!(s.count(), 140);
        assert_eq!(s.pop_front(), -39);
        let mut last = 0;
        while s.count() > 0 {
            last = s.pop_front();
        }
        assert_eq!(last, 99);
        s.push_front(5);
        s.clear(false);
        assert_eq!(s.count(), 0);
    }

    #[test]
    fn float_compare_to_orders_nan_first() {
        assert_eq!(compare_to(f32::NAN, 1.0), -1);
        assert_eq!(compare_to(1.0f32, f32::NAN), 1);
        assert_eq!(compare_to(f32::NAN, f32::NAN), 0);
        assert_eq!(compare_to(2, 3), -1);
        assert_eq!(compare_to(s!("b"), s!("a")), 1);
    }

    #[test]
    fn fast_parallel_runs_one_range_on_this_thread() {
        use std::cell::Cell;
        use std::rc::Rc;
        let seen = Rc::new(Cell::new((0, 0, 0)));
        let s2 = seen.clone();
        let d: Delegate<dyn Fn(i32, i32, Obj)> = Delegate::new(Box::new(move |a, b, _| s2.set((a, b, s2.get().2 + 1))));
        fast_parallel_for(3, 10, d, Gc::NULL);
        assert_eq!(seen.get(), (3, 10, 1));
        fast_parallel_for(5, 5, d, Gc::NULL); // empty range: no call
        assert_eq!(seen.get().2, 1);
    }
}

/// `EqualityComparer<T>.Default`: the comparer has no state; its members are `rt::CsKey` of `T`.
#[derive(Clone, Copy)]
pub struct DefaultComparer;
unsafe impl Trace for DefaultComparer {
    const NEEDS_TRACE: bool = false;
    unsafe fn trace(&self, _m: &mut super::Marker) {}
}

/// `Array.FindAll(array, match)`: the matching elements in order, in a new array.
pub fn array_find_all<T: Val>(a: Arr<T>, pred: Delegate<dyn Fn(T) -> bool>) -> Arr<T> {
    if a.is_null() {
        throw(exc("System.ArgumentNullException", Str::lit("array")))
    }
    // snapshot first: the predicate is translated code
    let items: Vec<T> = a.as_slice().to_vec();
    let kept: Vec<T> = items.into_iter().filter(|x| pred.invoke(|f| f(*x))).collect();
    Arr::from_slice(&kept)
}

/// `Type.GetType(name)` for the two probes the game makes of its runtime: it is not Mono (`Mono.Runtime` is
/// absent) and it is .NET Framework 4.5 or newer (`System.Reflection.ReflectionContext` exists; the reference
/// machine runs 4.8). Any other name is a host member that is not implemented.
pub fn type_get_type(name: Str) -> Obj {
    match name.as_str() {
        "Mono.Runtime" => Gc::NULL,
        "System.Reflection.ReflectionContext" => super::type_of("System.Reflection.ReflectionContext", 0, 0),
        other => crate::host::unimplemented_value::<Obj>(Box::leak(format!("System.Type.GetType(\"{other}\")").into_boxed_str())),
    }
}

// ---------------------------------------------------------------------------------------------- culture

/// `System.Globalization.CultureInfo`: the name only. The game creates one per language (`GameCulture`) and makes
/// the selected one the thread's culture (`LanguageManager.SetLanguage`, LanguageManager.cs:97); at start-up that is
/// en-US (Program.cs:197). The runtime formats numbers and compares strings by the en-US rules whatever the
/// current culture is (DESIGN.md section 8b): for en-US and the invariant culture the formats the game uses agree.
#[derive(Clone, Copy)]
pub struct CultureInfo {
    pub name: Str,
}
unsafe impl Trace for CultureInfo {
    unsafe fn trace(&self, m: &mut Marker) {
        self.name.trace(m)
    }
}
impl CultureInfo {
    /// `LCID` of the cultures the game names (its twelve `GameCulture`s and the invariant culture), read from the .NET
    /// Framework on Windows 11 (`CultureInfo.GetCultureInfo(name).LCID`, 2026-10-11). ReLogic's text wrapping compares
    /// the LCIDs of two cultures (`StringReaderWrapExtension.BreaksBetweenMostGlyphs`). Any other culture is a host
    /// member that is not implemented.
    pub fn lcid(&self) -> i32 {
        match self.name.get().unwrap_or("") {
            "" => 127,
            "en-US" => 1033,
            "de-DE" => 1031,
            "it-IT" => 1040,
            "fr-FR" => 1036,
            "es-ES" => 3082,
            "ru-RU" => 1049,
            "zh-Hans" => 4,
            "pt-BR" => 1046,
            "pl-PL" => 1045,
            "ja-JP" => 1041,
            "ko-KR" => 1042,
            "zh-Hant" => 31748,
            _ => crate::host::unimplemented_value("System.Globalization.CultureInfo.LCID of a culture other than the game's twelve and the invariant culture"),
        }
    }
}
thread_local! {
    static CULTURE: std::cell::Cell<Option<super::Rooted<Gc<CultureInfo>>>> = const { std::cell::Cell::new(None) };
    static UI_CULTURE: std::cell::Cell<Option<super::Rooted<Gc<CultureInfo>>>> = const { std::cell::Cell::new(None) };
    static INVARIANT: super::Rooted<Gc<CultureInfo>> = super::Rooted::new(alloc_init(CultureInfo { name: Str::lit("") }));
}
fn culture_get(slot: &'static std::thread::LocalKey<std::cell::Cell<Option<super::Rooted<Gc<CultureInfo>>>>>) -> Gc<CultureInfo> {
    slot.with(|c| {
        let cur = c.take();
        // a process starts in the user's culture; the reference machine is en-US
        let cur = cur.unwrap_or_else(|| super::Rooted::new(alloc_init(CultureInfo { name: Str::lit("en-US") })));
        let g = cur.get();
        c.set(Some(cur));
        g
    })
}
impl CultureInfo {
    /// `new CultureInfo(name)`.
    pub fn new_gc(name: Str) -> Gc<CultureInfo> {
        if name.is_null() {
            throw(exc("System.ArgumentNullException", Str::lit("name")))
        }
        alloc_init(CultureInfo { name })
    }
    pub fn invariant() -> Gc<CultureInfo> {
        INVARIANT.with(|c| c.get())
    }
    /// `Thread.CurrentThread.CurrentCulture` / `CultureInfo.CurrentCulture`.
    pub fn current() -> Gc<CultureInfo> {
        culture_get(&CULTURE)
    }
    pub fn set_current(c: Gc<CultureInfo>) {
        if c.is_null() {
            throw(exc("System.ArgumentNullException", Str::lit("value")))
        }
        CULTURE.with(|s| s.set(Some(super::Rooted::new(c))));
    }
    pub fn current_ui() -> Gc<CultureInfo> {
        culture_get(&UI_CULTURE)
    }
    pub fn set_current_ui(c: Gc<CultureInfo>) {
        if c.is_null() {
            throw(exc("System.ArgumentNullException", Str::lit("value")))
        }
        UI_CULTURE.with(|s| s.set(Some(super::Rooted::new(c))));
    }
}

/// Primary weight of an ASCII character in the en-US collation of Windows (word sort): control characters and
/// the hyphen / apostrophe are ignorable (`None`), then white space, punctuation and symbols, digits, letters
/// (case does not matter at this level). Non-ASCII characters sort after ASCII by code point (deviation: their real
/// weights are not modelled).
fn primary_weight(c: char) -> Option<u32> {
    const SYMBOLS: &str = " _,;:!?.\"()[]{}@*/\\&#%`^+<=>|~$";
    match c {
        '-' | '\'' => None,
        c if (c as u32) < 0x20 => None,
        c if c.is_ascii_digit() => Some(1000 + c as u32),
        c if c.is_ascii_alphabetic() => Some(2000 + c.to_ascii_lowercase() as u32),
        c if c.is_ascii() => Some(SYMBOLS.find(c).map(|i| i as u32 + 1).unwrap_or(900)),
        c => Some(10_000 + c as u32),
    }
}

/// `string.Compare(a, b)` / `a.CompareTo(b)`: the culture-sensitive comparison, modelled for en-US and ASCII text
/// (the identifiers and file names the game compares this way). Null sorts before everything. Order: primary
/// weights (letters without case, digits before letters, symbols before digits, hyphen and apostrophe ignored);
/// on a tie lower case before upper case at the first difference; on a tie the string with the ignorable
/// character later is smaller; then equal.
pub fn string_compare(a: Str, b: Str) -> i32 {
    let (x, y) = match (a.get(), b.get()) {
        (None, None) => return 0,
        (None, _) => return -1,
        (_, None) => return 1,
        (Some(x), Some(y)) => (x, y),
    };
    let wx: Vec<(u32, char)> = x.chars().filter_map(|c| primary_weight(c).map(|w| (w, c))).collect();
    let wy: Vec<(u32, char)> = y.chars().filter_map(|c| primary_weight(c).map(|w| (w, c))).collect();
    for (p, q) in wx.iter().zip(wy.iter()) {
        if p.0 != q.0 {
            return if p.0 < q.0 { -1 } else { 1 };
        }
    }
    if wx.len() != wy.len() {
        return if wx.len() < wy.len() { -1 } else { 1 };
    }
    for (p, q) in wx.iter().zip(wy.iter()) {
        if p.1 != q.1 {
            // same letter, different case: lower case first
            return if p.1.is_lowercase() { -1 } else { 1 };
        }
    }
    // only ignorable characters (hyphen, apostrophe) differ: the string without them comes first ("coop" before
    // "co-op"); between two strings that both have some, ordinal decides (deviation from the exact special weights)
    let (ix, iy) = (x.chars().count() - wx.len(), y.chars().count() - wy.len());
    if ix != iy {
        return if ix < iy { -1 } else { 1 };
    }
    compare_ordinal(a, b).signum()
}

fn bit_converter_check(value: Arr<u8>, start: i32, size: i32) -> usize {
    if value.is_null() {
        throw(exc("System.ArgumentNullException", Str::lit("value")));
    }
    if start < 0 || start >= value.len() {
        throw(exc("System.ArgumentOutOfRangeException", Str::lit("startIndex")));
    }
    if start > value.len() - size {
        throw(exc("System.ArgumentException", Str::lit("Destination array is not long enough to copy all the items in the collection. Check array index and length.")));
    }
    start as usize
}
/// `BitConverter.ToUInt16(value, startIndex)`: little endian, with .NET's argument checks.
pub fn bit_converter_to_u16(value: Arr<u8>, start: i32) -> u16 {
    let i = bit_converter_check(value, start, 2);
    let b = value.as_slice();
    u16::from_le_bytes([b[i], b[i + 1]])
}
/// `BitConverter.ToInt32(value, startIndex)`
pub fn bit_converter_to_i32(value: Arr<u8>, start: i32) -> i32 {
    let i = bit_converter_check(value, start, 4);
    let b = value.as_slice();
    i32::from_le_bytes([b[i], b[i + 1], b[i + 2], b[i + 3]])
}
/// `BitConverter.GetBytes(int)` / `GetBytes(ulong)`: the value's bytes, little endian.
pub fn bit_converter_bytes_i32(v: i32) -> Arr<u8> {
    Arr::from_slice(&v.to_le_bytes())
}
pub fn bit_converter_bytes_u64(v: u64) -> Arr<u8> {
    Arr::from_slice(&v.to_le_bytes())
}

/// `Buffer.BlockCopy(src, srcOffset, dst, dstOffset, count)`: BYTES of two arrays of primitives; offsets and count
/// are in bytes whatever the element types (`byte[]` to `byte[]` in the network buffers, `char[]` to `byte[]` in
/// `Utils`). The ranges may overlap (memmove). .NET's argument checks.
pub fn buffer_block_copy<A: Val, B: Val>(src: Arr<A>, src_offset: i32, dst: Arr<B>, dst_offset: i32, count: i32) {
    if src.is_null() {
        throw(exc("System.ArgumentNullException", Str::lit("src")));
    }
    if dst.is_null() {
        throw(exc("System.ArgumentNullException", Str::lit("dst")));
    }
    if A::NEEDS_TRACE || B::NEEDS_TRACE {
        throw(exc("System.ArgumentException", Str::lit("Object must be an array of primitives.")));
    }
    if src_offset < 0 {
        throw(exc("System.ArgumentOutOfRangeException", Str::lit("srcOffset")));
    }
    if dst_offset < 0 {
        throw(exc("System.ArgumentOutOfRangeException", Str::lit("dstOffset")));
    }
    if count < 0 {
        throw(exc("System.ArgumentOutOfRangeException", Str::lit("count")));
    }
    let (sb, db) = (src.len() as i64 * core::mem::size_of::<A>() as i64, dst.len() as i64 * core::mem::size_of::<B>() as i64);
    if src_offset as i64 + count as i64 > sb || dst_offset as i64 + count as i64 > db {
        throw(exc("System.ArgumentException", Str::lit("Offset and length were out of bounds for the array or count is greater than the number of elements from index to the end of the source collection.")));
    }
    if count == 0 {
        return;
    }
    // SAFETY: both ranges were checked against their arrays; the elements are plain data.
    unsafe { core::ptr::copy((src.at(0) as *const u8).add(src_offset as usize), (dst.at(0) as *mut u8).add(dst_offset as usize), count as usize) };
}

/// A library collection interface that has no Rust type of its own (`ICollection<T>`, `IList<T>`,
/// `IReadOnlyCollection<T>` ...: the value is held as `object`) seen as `IEnumerable<T>`: the runtime collection
/// behind it, by its Rust type. Null stays null. A list, an array, a queue or a stack of `T`; anything else is an
/// `InvalidCastException` naming the type (sets and dictionaries need their key type and are not covered).
pub fn obj_seq<T: Val>(o: Obj) -> Seq<T> {
    if o.is_null() {
        return Seq::NULL;
    }
    if super::obj_is::<List<T>>(o) {
        return Seq::from_list(Gc(o.0 as *mut List<T>));
    }
    if super::obj_is::<super::arr::ArrData<T>>(o) {
        // SAFETY: the type was checked; `Arr<T>` is a transparent wrapper around the handle.
        return Seq::from_arr(unsafe { core::mem::transmute_copy::<Obj, Arr<T>>(&o) });
    }
    if super::obj_is::<super::Queue<T>>(o) {
        return Seq::from_queue(Gc(o.0 as *mut super::Queue<T>));
    }
    if super::obj_is::<super::Stack<T>>(o) {
        return Seq::from_stack(Gc(o.0 as *mut super::Stack<T>));
    }
    throw(exc("System.InvalidCastException", Str::new(&format!("a collection interface over {} seen as IEnumerable<{}>: no rule in the runtime", super::obj_to_string(o).as_str(), core::any::type_name::<T>()))))
}

/// A string as `IEnumerable<char>` (`text.Contains('$')`, LINQ over a string).
pub fn str_seq(s: Str) -> Seq<u16> {
    if s.is_null() {
        return unsafe { super::zero() };
    }
    Seq::from_vec(s.as_str().encode_utf16().collect())
}

/// `list.ForEach(action)`: by index, as `List<T>.ForEach` does; the action may not change the list
/// (`InvalidOperationException`, as .NET Framework 4.5+ throws).
pub fn list_for_each<T: Val>(list: Gc<List<T>>, action: Delegate<dyn Fn(T)>) {
    if action.is_null() {
        throw(exc("System.ArgumentNullException", Str::lit("match")))
    }
    // SAFETY: the reference to the list lives only for each short call.
    unsafe {
        let n = (*list.ptr()).count();
        let mut i = 0;
        while i < n {
            if (*list.ptr()).count() != n {
                throw(exc("System.InvalidOperationException", Str::lit("Collection was modified; enumeration operation may not execute.")))
            }
            let item = (*list.ptr()).get_item(i);
            action.invoke(|f| f(item));
            i += 1;
        }
    }
}

/// `source.ToDictionary(keySelector, elementSelector)`: `Add` in source order (a duplicate key throws).
pub fn to_dictionary<T: Val, K: CsKey, V: Val>(source: Seq<T>, key: Delegate<dyn Fn(T) -> K>, value: Delegate<dyn Fn(T) -> V>) -> Gc<super::Dictionary<K, V>> {
    let d = super::Dictionary::<K, V>::new_gc();
    for x in source.to_vec() {
        let k = key.invoke(|f| f(x));
        let v = value.invoke(|f| f(x));
        // SAFETY: the reference lives for the call.
        unsafe { (*d.ptr()).add(k, v) };
    }
    d
}

#[cfg(test)]
mod culture_tests {
    use super::*;

    #[test]
    fn en_us_word_sort_on_ascii() {
        let c = |a: &'static str, b: &'static str| string_compare(Str::lit(a), Str::lit(b));
        assert_eq!(c("a", "A"), -1);
        assert_eq!(c("A", "b"), -1);
        assert_eq!(c("Item_1", "Item_10"), -1);
        assert_eq!(c("Item_2", "Item_10"), 1);
        assert_eq!(c("_a", "1"), -1);
        assert_eq!(c("1", "a"), -1);
        assert_eq!(c("co-op", "coop"), 1);
        assert_eq!(c("coop", "co-op"), -1);
        assert_eq!(c("abc", "abc"), 0);
        assert_eq!(c("Zombie", "apple"), 1);
        assert_eq!(string_compare(Str::NULL, Str::lit("")), -1);
    }
}

/// An array of references seen as a sequence of a base type / interface (array covariance in an
/// `IEnumerable<Base>` position): a snapshot of the handles, retyped.
pub fn arr_up_seq<A: 'static, B: 'static>(a: Arr<Gc<A>>) -> Seq<Gc<B>> {
    if a.is_null() {
        return unsafe { super::zero() };
    }
    Seq::from_vec(a.as_slice().iter().map(|g| g.up::<B>()).collect())
}

/// Array covariance, `Derived[]` used as `Base[]` / `object[]`: the same array under the other element type
/// (both are arrays of object handles).
pub fn arr_up<A: 'static, B: 'static>(a: Arr<Gc<A>>) -> Arr<Gc<B>> {
    // SAFETY: `Gc<A>` and `Gc<B>` are one pointer each; the array object is the same.
    unsafe { Arr::from_obj(a.up::<super::Object>()) }
}

/// `string[]` used as `object[]`: a NEW array with the strings as objects (a string is not an object handle in this
/// runtime). A snapshot: see the emitter's note at the call sites.
pub fn arr_str_obj(a: Arr<Str>) -> Arr<Obj> {
    if a.is_null() {
        return Arr::NULL;
    }
    let v: Vec<Obj> = a.as_slice().iter().map(|s| super::str_obj(*s)).collect();
    Arr::from_slice(&v)
}

/// `System.ArraySegment<T>`: a struct of the array, the offset and the count.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct ArraySegment<T> {
    pub array: Arr<T>,
    pub offset: i32,
    pub count: i32,
}
// SAFETY: the one handle is reported.
unsafe impl<T: 'static> Trace for ArraySegment<T> {
    unsafe fn trace(&self, m: &mut Marker) {
        self.array.trace(m)
    }
}
impl<T: Val> ArraySegment<T> {
    /// `new ArraySegment<T>(array, offset, count)` with .NET's argument checks.
    pub fn new(array: Arr<T>, offset: i32, count: i32) -> Self {
        if array.is_null() {
            throw(exc("System.ArgumentNullException", Str::lit("array")));
        }
        if offset < 0 || count < 0 {
            throw(exc("System.ArgumentOutOfRangeException", Str::lit(if offset < 0 { "offset" } else { "count" })));
        }
        if array.len() - offset < count {
            throw(exc("System.ArgumentException", Str::lit("Offset and length were out of bounds for the array or count is greater than the number of elements from index to the end of the source collection.")));
        }
        ArraySegment { array, offset, count }
    }
    /// `new ArraySegment<T>(array)`.
    pub fn whole(array: Arr<T>) -> Self {
        if array.is_null() {
            throw(exc("System.ArgumentNullException", Str::lit("array")));
        }
        ArraySegment { array, offset: 0, count: array.len() }
    }
}

// ---------------------------------------------------------------------------------------------- LinkedList<T>

/// `System.Collections.Generic.LinkedListNode<T>`.
pub struct LinkedListNode<T> {
    pub value: T,
    next: Gc<LinkedListNode<T>>,
    prev: Gc<LinkedListNode<T>>,
    list: Gc<LinkedList<T>>,
}
unsafe impl<T: Val> Trace for LinkedListNode<T> {
    unsafe fn trace(&self, m: &mut Marker) {
        self.value.trace(m);
        m.mark(self.next);
        m.mark(self.prev);
        m.mark(self.list);
    }
}
impl<T: Val> LinkedListNode<T> {
    /// `node.Next`: null at the end (the list is not circular from outside, as in .NET).
    pub fn next(&self) -> Gc<LinkedListNode<T>> {
        self.next
    }
    pub fn previous(&self) -> Gc<LinkedListNode<T>> {
        self.prev
    }
    pub fn list(&self) -> Gc<LinkedList<T>> {
        self.list
    }
}

/// `System.Collections.Generic.LinkedList<T>`: nodes are objects, so a node handle stays valid while the list
/// changes around it (the game walks `First` / `Next` while removing).
pub struct LinkedList<T> {
    first: Gc<LinkedListNode<T>>,
    last: Gc<LinkedListNode<T>>,
    count: i32,
}
unsafe impl<T: Val> Trace for LinkedList<T> {
    unsafe fn trace(&self, m: &mut Marker) {
        // every node is reachable from the first through `next`
        m.mark(self.first);
        m.mark(self.last);
    }
}
impl<T: Val> LinkedList<T> {
    pub fn new_gc() -> Gc<LinkedList<T>> {
        alloc_init(LinkedList { first: Gc::NULL, last: Gc::NULL, count: 0 })
    }
    pub fn count(&self) -> i32 {
        self.count
    }
    pub fn first(&self) -> Gc<LinkedListNode<T>> {
        self.first
    }
    pub fn last(&self) -> Gc<LinkedListNode<T>> {
        self.last
    }
    /// `list.AddLast(value)`.
    pub fn add_last(this: Gc<Self>, value: T) -> Gc<LinkedListNode<T>> {
        // SAFETY: short-lived references to runtime objects.
        unsafe {
            let l = &mut *this.ptr();
            let n = alloc_init(LinkedListNode { value, next: Gc::NULL, prev: l.last, list: this });
            if l.last.is_null() {
                l.first = n;
            } else {
                (*l.last.ptr()).next = n;
            }
            l.last = n;
            l.count += 1;
            n
        }
    }
    /// `list.AddFirst(value)`.
    pub fn add_first(this: Gc<Self>, value: T) -> Gc<LinkedListNode<T>> {
        // SAFETY: as `add_last`.
        unsafe {
            let l = &mut *this.ptr();
            let n = alloc_init(LinkedListNode { value, next: l.first, prev: Gc::NULL, list: this });
            if l.first.is_null() {
                l.last = n;
            } else {
                (*l.first.ptr()).prev = n;
            }
            l.first = n;
            l.count += 1;
            n
        }
    }
    /// `list.AddAfter(node, value)`: `ArgumentNullException` for a null node, `InvalidOperationException` when the
    /// node belongs to another list.
    pub fn add_after(this: Gc<Self>, node: Gc<LinkedListNode<T>>, value: T) -> Gc<LinkedListNode<T>> {
        // SAFETY: as `add_last`.
        unsafe {
            if node.is_null() {
                throw(exc("System.ArgumentNullException", Str::lit("node")))
            }
            if (*node.ptr()).list != this {
                throw(exc("System.InvalidOperationException", Str::lit("The LinkedList node does not belong to current LinkedList.")))
            }
            let after = (*node.ptr()).next;
            let n = alloc_init(LinkedListNode { value, next: after, prev: node, list: this });
            let l = &mut *this.ptr();
            (*node.ptr()).next = n;
            if after.is_null() {
                l.last = n;
            } else {
                (*after.ptr()).prev = n;
            }
            l.count += 1;
            n
        }
    }
    /// `list.AddBefore(node, value)`: the same checks as `AddAfter`.
    pub fn add_before(this: Gc<Self>, node: Gc<LinkedListNode<T>>, value: T) -> Gc<LinkedListNode<T>> {
        // SAFETY: as `add_last`.
        unsafe {
            if node.is_null() {
                throw(exc("System.ArgumentNullException", Str::lit("node")))
            }
            if (*node.ptr()).list != this {
                throw(exc("System.InvalidOperationException", Str::lit("The LinkedList node does not belong to current LinkedList.")))
            }
            let before = (*node.ptr()).prev;
            let n = alloc_init(LinkedListNode { value, next: node, prev: before, list: this });
            let l = &mut *this.ptr();
            (*node.ptr()).prev = n;
            if before.is_null() {
                l.first = n;
            } else {
                (*before.ptr()).next = n;
            }
            l.count += 1;
            n
        }
    }
    /// `list.Remove(node)`: `InvalidOperationException` when the node belongs to another list.
    pub fn remove_node(this: Gc<Self>, node: Gc<LinkedListNode<T>>) {
        // SAFETY: as `add_last`.
        unsafe {
            if node.is_null() {
                throw(exc("System.ArgumentNullException", Str::lit("node")))
            }
            let n = &mut *node.ptr();
            if n.list != this {
                throw(exc("System.InvalidOperationException", Str::lit("The LinkedList node does not belong to current LinkedList.")))
            }
            let l = &mut *this.ptr();
            if n.prev.is_null() {
                l.first = n.next;
            } else {
                (*n.prev.ptr()).next = n.next;
            }
            if n.next.is_null() {
                l.last = n.prev;
            } else {
                (*n.next.ptr()).prev = n.prev;
            }
            n.next = Gc::NULL;
            n.prev = Gc::NULL;
            n.list = Gc::NULL;
            l.count -= 1;
        }
    }
    pub fn clear(&mut self) {
        // SAFETY: nodes are runtime objects.
        unsafe {
            let mut n = self.first;
            while !n.is_null() {
                let next = (*n.ptr()).next;
                (*n.ptr()).next = Gc::NULL;
                (*n.ptr()).prev = Gc::NULL;
                (*n.ptr()).list = Gc::NULL;
                n = next;
            }
        }
        self.first = Gc::NULL;
        self.last = Gc::NULL;
        self.count = 0;
    }
    pub fn to_vec(&self) -> Vec<T> {
        let mut v = Vec::with_capacity(self.count as usize);
        let mut n = self.first;
        while !n.is_null() {
            // SAFETY: nodes are runtime objects.
            unsafe {
                v.push((*n.ptr()).value);
                n = (*n.ptr()).next;
            }
        }
        v
    }
    /// The list as `IEnumerable<T>` / in `foreach`. Deviation: a snapshot taken when the conversion runs (.NET
    /// enumerates live and throws when the list changes under the enumerator).
    pub fn as_seq(this: Gc<Self>) -> Seq<T> {
        if this.is_null() {
            return unsafe { super::zero() };
        }
        // SAFETY: non-null list.
        Seq::from_vec(unsafe { (*this.ptr()).to_vec() })
    }
}
impl<T: CsKey> LinkedList<T> {
    pub fn contains(&self, v: T) -> bool {
        self.to_vec().iter().any(|x| x.cs_eq(&v))
    }
    /// `list.Remove(value)`: the first node holding an equal value.
    pub fn remove_value(this: Gc<Self>, v: T) -> bool {
        // SAFETY: as `add_last`.
        unsafe {
            let mut n = (*this.ptr()).first;
            while !n.is_null() {
                if (*n.ptr()).value.cs_eq(&v) {
                    Self::remove_node(this, n);
                    return true;
                }
                n = (*n.ptr()).next;
            }
        }
        false
    }
}

// ---------------------------------------------------------------------------------------------- System.Enum

fn enum_box(info: &'static super::EnumInfo, v: u64) -> Obj {
    use super::tid;
    match info.underlying {
        tid::U8 => super::box_enum(v as u8, info),
        tid::I8 => super::box_enum(v as i8, info),
        tid::I16 => super::box_enum(v as i16, info),
        tid::U16 => super::box_enum(v as u16, info),
        tid::U32 => super::box_enum(v as u32, info),
        tid::I64 => super::box_enum(v as i64, info),
        tid::U64 => super::box_enum(v, info),
        _ => super::box_enum(v as i32, info),
    }
}

/// `Enum.GetValues(typeof(E))`: the values sorted by their unsigned magnitude (one entry per NAME, as .NET), as an
/// array of the enum's underlying integer type (an `E[]` is that array type here).
pub fn enum_get_values(t: Obj) -> Obj {
    use super::tid;
    let info = super::type_enum_info(t);
    macro_rules! arr {
        ($t:ty) => {{
            let v: Vec<$t> = info.names.iter().map(|(k, _)| *k as $t).collect();
            Arr::<$t>::from_slice(&v).up()
        }};
    }
    match info.underlying {
        tid::U8 => arr!(u8),
        tid::I8 => arr!(i8),
        tid::I16 => arr!(i16),
        tid::U16 => arr!(u16),
        tid::U32 => arr!(u32),
        tid::I64 => arr!(i64),
        tid::U64 => arr!(u64),
        _ => arr!(i32),
    }
}

/// `Enum.GetNames(typeof(E))`: the names in the order of `GetValues`.
pub fn enum_get_names(t: Obj) -> Arr<Str> {
    let info = super::type_enum_info(t);
    let v: Vec<Str> = info.names.iter().map(|(_, n)| Str::lit(n)).collect();
    Arr::from_slice(&v)
}

fn enum_value_of(info: &'static super::EnumInfo, value: Obj) -> u64 {
    let n = super::convert_to_i64(value);
    // the pattern in the width of the underlying type, as the names table stores it
    match info.underlying {
        super::tid::U8 | super::tid::I8 => n as u8 as u64,
        super::tid::I16 | super::tid::U16 => n as u16 as u64,
        super::tid::I32 | super::tid::U32 => n as u32 as u64,
        _ => n as u64,
    }
}
fn enum_key(info: &'static super::EnumInfo, k: u64) -> u64 {
    match info.underlying {
        super::tid::U8 | super::tid::I8 => k as u8 as u64,
        super::tid::I16 | super::tid::U16 => k as u16 as u64,
        super::tid::I32 | super::tid::U32 => k as u32 as u64,
        _ => k,
    }
}

/// `Enum.GetName(typeof(E), value)`: the name of the value, or null.
pub fn enum_get_name(t: Obj, value: Obj) -> Str {
    let info = super::type_enum_info(t);
    let v = enum_value_of(info, value);
    match info.names.iter().find(|(k, _)| enum_key(info, *k) == v) {
        Some((_, n)) => Str::lit(n),
        None => Str::NULL,
    }
}

/// `Enum.IsDefined(typeof(E), value)` for a number or a name.
pub fn enum_is_defined(t: Obj, value: Obj) -> bool {
    let info = super::type_enum_info(t);
    let name = super::obj_as_str(value);
    if !name.is_null() {
        return info.names.iter().any(|(_, n)| *n == name.as_str());
    }
    let v = enum_value_of(info, value);
    info.names.iter().any(|(k, _)| enum_key(info, *k) == v)
}

pub fn enum_parse_text(info: &'static super::EnumInfo, text: &str, ignore_case: bool) -> Option<u64> {
    let text = text.trim();
    if text.is_empty() {
        return None;
    }
    let first = text.as_bytes()[0];
    if first.is_ascii_digit() || first == b'-' || first == b'+' {
        if let Ok(n) = text.parse::<i64>() {
            return Some(n as u64);
        }
    }
    // a comma-separated list of names is or-ed together
    let mut result = 0u64;
    for part in text.split(',') {
        let part = part.trim();
        let hit = info.names.iter().find(|(_, n)| if ignore_case { n.eq_ignore_ascii_case(part) } else { *n == part })?;
        result |= hit.0;
    }
    Some(result)
}

/// `Enum.Parse(typeof(E), text, ignoreCase)`: a boxed `E` (`ArgumentException` for an unknown name).
pub fn enum_parse(t: Obj, text: Str, ignore_case: bool) -> Obj {
    let info = super::type_enum_info(t);
    if text.is_null() {
        throw(exc("System.ArgumentNullException", Str::lit("value")))
    }
    match enum_parse_text(info, text.as_str(), ignore_case) {
        Some(v) => enum_box(info, v),
        None => throw(exc("System.ArgumentException", Str::new(&format!("Requested value '{}' was not found.", text.as_str())))),
    }
}

/// `Enum.TryParse<E>(text, ignoreCase, out result)`: `result` is the enum's underlying integer.
///
/// # Safety
/// `out` must point at a variable of the enum's underlying type `T`.
pub unsafe fn enum_try_parse<T: Copy>(info: &'static super::EnumInfo, text: Str, ignore_case: bool, out: *mut T) -> bool {
    let parsed = text.get().and_then(|s| enum_parse_text(info, s, ignore_case));
    let v = parsed.unwrap_or(0);
    assert!(core::mem::size_of::<T>() <= 8);
    core::ptr::copy_nonoverlapping(&v as *const u64 as *const u8, out as *mut u8, core::mem::size_of::<T>());
    parsed.is_some()
}

#[cfg(test)]
mod linked_list_and_enum_tests {
    use super::*;

    #[test]
    fn linked_list_nodes_survive_removal_of_neighbours() {
        let l = LinkedList::<i32>::new_gc();
        let a = LinkedList::add_last(l, 1);
        let b = LinkedList::add_last(l, 2);
        let c = LinkedList::add_last(l, 3);
        LinkedList::add_first(l, 0);
        unsafe {
            assert_eq!((*l.ptr()).to_vec(), [0, 1, 2, 3]);
            LinkedList::remove_node(l, b);
            assert!((*a.ptr()).next() == c && (*c.ptr()).previous() == a);
            assert_eq!((*l.ptr()).count(), 3);
            assert!(LinkedList::remove_value(l, 3) && !LinkedList::remove_value(l, 9));
            assert!((*l.ptr()).last() == a && (*a.ptr()).next().is_null());
            assert!(crate::rt::try_(|| LinkedList::remove_node(l, b)).is_err());
            assert_eq!(LinkedList::as_seq(l).to_vec(), [0, 1]);
            // AddAfter / AddBefore: in the middle, at both ends, and with a node that is not in the list
            let first = (*l.ptr()).first();
            let x = LinkedList::add_after(l, first, 10);
            assert_eq!((*l.ptr()).to_vec(), [0, 10, 1]);
            let y = LinkedList::add_after(l, a, 11);
            assert!((*l.ptr()).last() == y && (*y.ptr()).previous() == a && (*y.ptr()).next().is_null());
            let z = LinkedList::add_before(l, first, 12);
            assert!((*l.ptr()).first() == z && (*z.ptr()).next() == first && (*first.ptr()).previous() == z);
            LinkedList::add_before(l, x, 13);
            assert_eq!((*l.ptr()).to_vec(), [12, 0, 13, 10, 1, 11]);
            assert_eq!((*l.ptr()).count(), 6);
            assert!(crate::rt::try_(|| LinkedList::add_after(l, b, 5)).is_err());
            assert!(crate::rt::try_(|| LinkedList::add_before(l, Gc::NULL, 5)).is_err());
        }
    }

    #[test]
    fn enum_reflection_follows_the_names_table() {
        static INFO: crate::rt::EnumInfo =
            crate::rt::EnumInfo { tid: 9000, name: "Test.Fruit", names: &[(0, "Apple"), (5, "Pear"), (6, "Plum")], flags: false, signed: true, underlying: crate::rt::tid::I32 };
        let t = crate::rt::type_of_enum(&INFO);
        assert_eq!(enum_get_names(t).as_slice().iter().map(|s| s.as_str()).collect::<Vec<_>>(), ["Apple", "Pear", "Plum"]);
        let values: Arr<i32> = crate::rt::cast_rt(enum_get_values(t));
        assert_eq!(values.as_slice(), [0, 5, 6]);
        assert_eq!(enum_get_name(t, crate::rt::box_value(5i32, crate::rt::tid::I32)).as_str(), "Pear");
        assert!(enum_get_name(t, crate::rt::box_value(4i32, crate::rt::tid::I32)).is_null());
        let p = enum_parse(t, Str::lit("plum"), true);
        assert!(crate::rt::is_boxed(p, 9000));
        assert_eq!(crate::rt::obj_to_string(p).as_str(), "Plum");
        assert!(crate::rt::try_(|| enum_parse(t, Str::lit("plum"), false)).is_err());
        assert!(enum_is_defined(t, crate::rt::str_obj(Str::lit("Pear"))) && !enum_is_defined(t, crate::rt::box_value(7i32, crate::rt::tid::I32)));
        let mut out = 0i32;
        assert!(unsafe { enum_try_parse(&INFO, Str::lit("6"), false, &mut out) } && out == 6);
        assert!(!unsafe { enum_try_parse(&INFO, Str::lit("Kiwi"), false, &mut out) } && out == 0);
    }
}

/// `string.Concat(params object[])`: every element's `ToString()`, null as the empty string.
pub fn concat_objects(parts: Arr<Obj>) -> Str {
    if parts.is_null() {
        throw(exc("System.ArgumentNullException", Str::lit("args")))
    }
    let texts: Vec<Str> = parts.as_slice().iter().map(|o| super::obj_to_string_or_empty(*o)).collect();
    super::concat(&texts)
}

// ---------------------------------------------------------------------------------------------- OrderBy / ThenBy

/// `IOrderedEnumerable<T>`: the elements in source order and, per `OrderBy` / `ThenBy` level, the rank of each
/// element's key. Enumerating sorts by the ranks level by level; equal keys keep source order (LINQ's sort is
/// stable). Deviation: the key selectors run when `OrderBy` / `ThenBy` is called, .NET runs them when the result is
/// first enumerated.
pub struct OrderedData<T> {
    items: Vec<T>,
    ranks: Vec<Vec<u32>>,
}
unsafe impl<T: Val> Trace for OrderedData<T> {
    const NEEDS_TRACE: bool = T::NEEDS_TRACE;
    unsafe fn trace(&self, m: &mut Marker) {
        self.items.trace(m)
    }
}
fn rank_of<T: Val, K: CsOrd>(items: &[T], key: Delegate<dyn Fn(T) -> K>, descending: bool) -> Vec<u32> {
    let keys: Vec<K> = items.iter().map(|x| key.invoke(|f| f(*x))).collect();
    let mut idx: Vec<usize> = (0..items.len()).collect();
    idx.sort_by(|a, b| keys[*a].cs_cmp(&keys[*b]).cmp(&0));
    let mut rank = vec![0u32; items.len()];
    let mut r = 0u32;
    for (n, i) in idx.iter().enumerate() {
        if n > 0 && keys[idx[n - 1]].cs_cmp(&keys[*i]) != 0 {
            r += 1;
        }
        rank[*i] = r;
    }
    if descending {
        for x in rank.iter_mut() {
            *x = r - *x;
        }
    }
    rank
}
impl<T: Val> OrderedData<T> {
    /// `source.OrderBy(key)` / `OrderByDescending`.
    pub fn order_by<K: CsOrd>(source: Seq<T>, key: Delegate<dyn Fn(T) -> K>, descending: bool) -> Gc<OrderedData<T>> {
        if source.is_null() || key.is_null() {
            throw(exc("System.ArgumentNullException", Str::lit("source")))
        }
        let items = source.to_vec();
        let ranks = vec![rank_of(&items, key, descending)];
        alloc_init(OrderedData { items, ranks })
    }
    /// `ordered.ThenBy(key)` / `ThenByDescending`: a new sequence with one more level.
    pub fn then_by<K: CsOrd>(this: Gc<Self>, key: Delegate<dyn Fn(T) -> K>, descending: bool) -> Gc<OrderedData<T>> {
        if key.is_null() {
            throw(exc("System.ArgumentNullException", Str::lit("keySelector")))
        }
        // SAFETY: the reference ends before the selector (translated code) runs.
        let (items, mut ranks) = unsafe { ((*this.ptr()).items.clone(), (*this.ptr()).ranks.clone()) };
        ranks.push(rank_of(&items, key, descending));
        alloc_init(OrderedData { items, ranks })
    }
    pub fn to_vec(&self) -> Vec<T> {
        let mut idx: Vec<usize> = (0..self.items.len()).collect();
        idx.sort_by(|a, b| {
            for level in &self.ranks {
                match level[*a].cmp(&level[*b]) {
                    core::cmp::Ordering::Equal => {}
                    o => return o,
                }
            }
            core::cmp::Ordering::Equal
        });
        idx.into_iter().map(|i| self.items[i]).collect()
    }
    /// The ordered sequence as `IEnumerable<T>`.
    pub fn to_seq(this: Gc<Self>) -> Seq<T> {
        if this.is_null() {
            return unsafe { super::zero() };
        }
        // SAFETY: non-null.
        Seq::from_vec(unsafe { (*this.ptr()).to_vec() })
    }
}

/// `list.RemoveAll(match)`: the elements for which the predicate is false stay, in order; returns how many went.
pub fn list_remove_all<T: Val>(list: Gc<List<T>>, pred: Delegate<dyn Fn(T) -> bool>) -> i32 {
    if pred.is_null() {
        throw(exc("System.ArgumentNullException", Str::lit("match")))
    }
    // SAFETY: references to the list live only for each short call; the predicate runs on a snapshot.
    unsafe {
        let items: Vec<T> = (*list.ptr()).to_array().as_slice().to_vec();
        let kept: Vec<T> = items.iter().copied().filter(|x| !pred.invoke(|f| f(*x))).collect();
        let removed = (items.len() - kept.len()) as i32;
        if removed > 0 {
            (*list.ptr()).clear();
            for x in kept {
                (*list.ptr()).add(x);
            }
        }
        removed
    }
}

#[cfg(test)]
mod order_tests {
    use super::*;

    #[test]
    fn order_by_then_by_is_stable_per_level() {
        let src = Seq::from_vec(vec![(2, 1), (1, 9), (2, 0), (1, 3), (1, 9)].into_iter().map(|(a, b)| a * 100 + b).collect::<Vec<i32>>());
        let o = OrderedData::order_by(src, Delegate::new(Box::new(|x: i32| x / 100)), false);
        assert_eq!(OrderedData::to_seq(o).to_vec(), [109, 103, 109, 201, 200]);
        let t = OrderedData::then_by(o, Delegate::new(Box::new(|x: i32| x % 100)), false);
        assert_eq!(OrderedData::to_seq(t).to_vec(), [103, 109, 109, 200, 201]);
        let d = OrderedData::then_by(OrderedData::order_by(src, Delegate::new(Box::new(|x: i32| x / 100)), true), Delegate::new(Box::new(|x: i32| x % 100)), true);
        assert_eq!(OrderedData::to_seq(d).to_vec(), [201, 200, 109, 109, 103]);
        let l = List::<i32>::new_gc();
        unsafe {
            for v in [1, 2, 3, 4, 5, 6] {
                (*l.ptr()).add(v);
            }
            assert_eq!(list_remove_all(l, Delegate::new(Box::new(|x: i32| x % 2 == 0))), 3);
            assert_eq!((*l.ptr()).to_array().as_slice(), [1, 3, 5]);
        }
    }
}

/// `IEnumerable<Derived>` where `IEnumerable<Base>` is expected (covariance): the same elements, retyped lazily
/// (the source is enumerated when the result is).
pub fn seq_up<A: 'static, B: 'static>(source: Seq<Gc<A>>) -> Seq<Gc<B>> {
    if source.is_null() {
        return unsafe { super::zero() };
    }
    Seq::lazy(move || Box::new(super::VecEnum::new(source.to_vec().into_iter().map(|g| g.up::<B>()).collect())), move |m| unsafe { source.trace(m) })
}

/// `source.OfType<R>()` (`cast` false: elements failing the test are skipped) / `source.Cast<R>()` (`cast` true: a
/// non-null element failing the test throws `InvalidCastException`; null passes). Lazy: the source is enumerated
/// when the result is.
pub fn of_type<T: Val, R: Val>(source: Seq<T>, cast: bool, is: impl Fn(T) -> bool + 'static, convert: impl Fn(T) -> R + 'static) -> Seq<R> {
    if source.is_null() {
        throw(exc("System.ArgumentNullException", Str::lit("source")))
    }
    Seq::lazy(
        move || {
            let mut out: Vec<R> = Vec::new();
            for x in source.to_vec() {
                if is(x) {
                    out.push(convert(x));
                } else if cast {
                    // SAFETY: a zeroed value is null for every handle type; `Cast` on values that are not references
                    // never fails the test (same type).
                    let null: R = unsafe { super::zero() };
                    let is_null = unsafe { core::mem::size_of::<T>() == core::mem::size_of::<usize>() && core::mem::transmute_copy::<T, usize>(&x) == 0 };
                    if is_null {
                        out.push(null);
                    } else {
                        throw(exc("System.InvalidCastException", Str::lit("Specified cast is not valid.")))
                    }
                }
            }
            Box::new(super::VecEnum::new(out))
        },
        move |m| unsafe { source.trace(m) },
    )
}

/// `IGrouping<K, T>`: one key and its elements in source order.
pub struct Grouping<K, T> {
    pub key: K,
    items: Vec<T>,
}
unsafe impl<K: Val, T: Val> Trace for Grouping<K, T> {
    unsafe fn trace(&self, m: &mut Marker) {
        self.key.trace(m);
        self.items.trace(m)
    }
}
impl<K: Val, T: Val> Grouping<K, T> {
    /// The group as `IEnumerable<T>`.
    pub fn to_seq(this: Gc<Self>) -> Seq<T> {
        if this.is_null() {
            return unsafe { super::zero() };
        }
        // SAFETY: non-null.
        Seq::from_vec(unsafe { (*this.ptr()).items.clone() })
    }
}

/// `source.GroupBy(keySelector)`: groups in the order their keys first appear, elements in source order (keys
/// compared by the default comparer). Deviation: computed when called (.NET defers it to the enumeration).
pub fn group_by<T: Val, K: CsKey>(source: Seq<T>, key: Delegate<dyn Fn(T) -> K>) -> Seq<Gc<Grouping<K, T>>> {
    if source.is_null() || key.is_null() {
        throw(exc("System.ArgumentNullException", Str::lit("source")))
    }
    let mut groups: Vec<(K, Vec<T>)> = Vec::new();
    for x in source.to_vec() {
        let k = key.invoke(|f| f(x));
        match groups.iter_mut().find(|(g, _)| g.cs_eq(&k)) {
            Some((_, items)) => items.push(x),
            None => groups.push((k, vec![x])),
        }
    }
    Seq::from_vec(groups.into_iter().map(|(key, items)| alloc_init(Grouping { key, items })).collect())
}

/// `string.Compare(a, b, comparisonType)`: `Ordinal` (4) and `OrdinalIgnoreCase` (5) by UTF-16 units (upper-cased
/// for 5, as .NET does); the culture kinds (0..3) by the en-US model of [`string_compare`], ignoring case for the
/// odd ones.
pub fn string_compare_kind(a: Str, b: Str, kind: i32) -> i32 {
    match kind {
        4 => compare_ordinal(a, b).signum(),
        5 => match (a.get(), b.get()) {
            (None, None) => 0,
            (None, _) => -1,
            (_, None) => 1,
            (Some(x), Some(y)) => {
                let (x, y): (Vec<u16>, Vec<u16>) = (x.to_uppercase().encode_utf16().collect(), y.to_uppercase().encode_utf16().collect());
                match x.cmp(&y) {
                    core::cmp::Ordering::Less => -1,
                    core::cmp::Ordering::Equal => 0,
                    core::cmp::Ordering::Greater => 1,
                }
            }
        },
        1 | 3 => match (a.get(), b.get()) {
            (Some(x), Some(y)) => string_compare(Str::new(&x.to_lowercase()), Str::new(&y.to_lowercase())),
            _ => string_compare(a, b),
        },
        0 | 2 => string_compare(a, b),
        _ => throw(exc("System.ArgumentException", Str::lit("The string comparison type passed in is currently not supported."))),
    }
}

/// `source.SelectMany(selector)`: the inner sequences one after the other. Lazy: evaluated when enumerated.
pub fn select_many<T: Val, R: Val>(source: Seq<T>, selector: Delegate<dyn Fn(T) -> Seq<R>>) -> Seq<R> {
    if source.is_null() || selector.is_null() {
        throw(exc("System.ArgumentNullException", Str::lit("source")))
    }
    Seq::lazy(
        move || {
            let mut out: Vec<R> = Vec::new();
            for x in source.to_vec() {
                let inner = selector.invoke(|f| f(x));
                out.extend(inner.to_vec());
            }
            Box::new(super::VecEnum::new(out))
        },
        move |m| unsafe {
            source.trace(m);
            selector.trace(m)
        },
    )
}
