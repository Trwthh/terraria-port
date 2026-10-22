//! Number text of Newtonsoft.Json 10 on .NET Framework 4: `double.ToString("R")`, `float.ToString("R")`, the
//! `".0"` suffix of `JsonConvert.ToString(double)` and the grammar `JsonTextReader` accepts.
//!
//! Framework quirks reproduced (checked against the game's own Newtonsoft.Json.dll, tests/dotnet/json_probe.ps1):
//! - `"R"` formats with 15 significant digits and uses 17 only when 15 do not parse back to the same value;
//!   scientific notation from exponent 15 (`1E+15`) and below exponent -4 (`1E-05`), exponent with at least 2 digits;
//! - the sign of negative zero is lost (`-0.0` is written `0.0`);
//! - `EnsureDecimalPlace` appends `.0` when the text has no `.`, `E`, `N` (NaN) or `I` (Infinity).

/// `value.ToString("G{precision}")` of a finite, non-zero magnitude: `digits` (no dot), decimal exponent of the
/// first digit.
fn sig_digits(v: f64, precision: usize) -> (String, i32) {
    let s = format!("{:.*e}", precision - 1, v.abs());
    let (m, e) = s.split_once('e').expect("exponent form");
    (m.replace('.', ""), e.parse().expect("exponent"))
}

/// .NET "G" formatting of `v` with `precision` significant digits (trailing zeros removed).
pub fn format_g(v: f64, precision: usize) -> String {
    if v == 0.0 {
        return "0".into();
    }
    let (mut digits, x) = sig_digits(v, precision);
    while digits.len() > 1 && digits.ends_with('0') {
        digits.pop();
    }
    let sign = if v < 0.0 { "-" } else { "" };
    if x >= -4 && (x as i64) < precision as i64 {
        if x < 0 {
            format!("{sign}0.{}{digits}", "0".repeat((-x - 1) as usize))
        } else if digits.len() as i32 <= x + 1 {
            format!("{sign}{digits}{}", "0".repeat((x + 1) as usize - digits.len()))
        } else {
            format!("{sign}{}.{}", &digits[..(x + 1) as usize], &digits[(x + 1) as usize..])
        }
    } else {
        let mant = if digits.len() > 1 { format!("{}.{}", &digits[..1], &digits[1..]) } else { digits };
        format!("{sign}{mant}E{}{:02}", if x < 0 { '-' } else { '+' }, x.abs())
    }
}

/// `double.ToString("R", CultureInfo.InvariantCulture)` (.NET Framework).
pub fn format_double_r(v: f64) -> String {
    if v.is_nan() {
        return "NaN".into();
    }
    if v.is_infinite() {
        return if v > 0.0 { "Infinity" } else { "-Infinity" }.into();
    }
    let g15 = format_g(v, 15);
    let back: f64 = g15.replace('E', "e").parse().unwrap_or(f64::NAN);
    let s = if back == v { g15 } else { format_g(v, 17) };
    // the Framework prints negative zero without its sign
    if s == "-0" {
        "0".into()
    } else {
        s
    }
}

/// `float.ToString("R", CultureInfo.InvariantCulture)` (.NET Framework): 7 digits, 9 when 7 do not round-trip.
pub fn format_float_r(v: f32) -> String {
    if v.is_nan() {
        return "NaN".into();
    }
    if v.is_infinite() {
        return if v > 0.0 { "Infinity" } else { "-Infinity" }.into();
    }
    let d = v as f64;
    let g7 = format_g(d, 7);
    let back: f32 = g7.replace('E', "e").parse().unwrap_or(f32::NAN);
    let s = if back == v { g7 } else { format_g(d, 9) };
    if s == "-0" {
        "0".into()
    } else {
        s
    }
}

/// `JsonConvert.ToString(double)`: "R" plus `EnsureDecimalPlace`.
pub fn json_double(v: f64) -> String {
    ensure_decimal(format_double_r(v))
}
/// `JsonConvert.ToString(float)`.
pub fn json_float(v: f32) -> String {
    ensure_decimal(format_float_r(v))
}
fn ensure_decimal(s: String) -> String {
    if s.contains(['.', 'E', 'e', 'N', 'I']) {
        s
    } else {
        s + ".0"
    }
}

/// `NumberStyles.Float` grammar (no white space: the reader never passes any): `[+-]? digits [. digits] [e [+-] digits]`
/// with at least one digit in the mantissa. The text is parsed by Rust only after this check (Rust also accepts
/// `inf` and `nan`, .NET does not).
pub fn parse_float(text: &str) -> Option<f64> {
    let b = text.as_bytes();
    let mut i = 0;
    if i < b.len() && (b[i] == b'+' || b[i] == b'-') {
        i += 1;
    }
    let (mut int_digits, mut frac_digits) = (0, 0);
    while i < b.len() && b[i].is_ascii_digit() {
        i += 1;
        int_digits += 1;
    }
    if i < b.len() && b[i] == b'.' {
        i += 1;
        while i < b.len() && b[i].is_ascii_digit() {
            i += 1;
            frac_digits += 1;
        }
    }
    if int_digits + frac_digits == 0 {
        return None;
    }
    if i < b.len() && (b[i] == b'e' || b[i] == b'E') {
        i += 1;
        if i < b.len() && (b[i] == b'+' || b[i] == b'-') {
            i += 1;
        }
        let start = i;
        while i < b.len() && b[i].is_ascii_digit() {
            i += 1;
        }
        if i == start {
            return None;
        }
    }
    if i != b.len() {
        return None;
    }
    let v: f64 = text.parse().ok()?;
    // .NET Framework: a value that overflows a double is an OverflowException
    if v.is_infinite() {
        None
    } else {
        Some(v)
    }
}

/// Result of reading an integer token.
#[derive(Debug, PartialEq)]
pub enum Int {
    Long(i64),
    /// does not fit in a `long`: kept as `System.Numerics.BigInteger` text
    Big(String),
}

/// `ConvertUtils.Int64TryParse` + the `BigInteger` fallback of `JsonTextReader.ParseReadNumber`. `text` is
/// `-? digits+`; `None` when it is not.
pub fn parse_integer(text: &str) -> Option<Int> {
    let (neg, digits) = match text.strip_prefix('-') {
        Some(d) => (true, d),
        None => (false, text),
    };
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    match text.parse::<i64>() {
        Ok(v) => Some(Int::Long(v)),
        Err(_) => {
            let t = digits.trim_start_matches('0');
            let t = if t.is_empty() { "0" } else { t };
            Some(Int::Big(if neg && t != "0" { format!("-{t}") } else { t.to_owned() }))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Values checked against `double.ToString("R")` of the .NET Framework (json_cases.expected has the full list).
    #[test]
    fn double_r() {
        let cases: &[(f64, &str)] = &[
            (0.1, "0.1"),
            (0.30000000000000004, "0.30000000000000004"),
            (5e-324, "4.94065645841247E-324"),
            (1.7976931348623157e308, "1.7976931348623157E+308"),
            (123456789012345.6, "123456789012345.59"),
            (1e15, "1E+15"),
            (1e14, "100000000000000"),
            (1e-5, "1E-05"),
            (1e-4, "0.0001"),
            (1234567.125, "1234567.125"),
            (-1e21, "-1E+21"),
            (-0.0, "0"),
            (100.0, "100"),
            (12345678901234567.0, "12345678901234568"),
        ];
        for (v, s) in cases {
            assert_eq!(format_double_r(*v), *s, "{v:e}");
        }
        assert_eq!(json_double(100.0), "100.0");
        assert_eq!(json_double(f64::NAN), "NaN");
        assert_eq!(json_double(1e15), "1E+15");
        assert_eq!(json_float(0.1), "0.1");
        assert_eq!(json_float(1.0 / 3.0), "0.333333343");
        assert_eq!(json_float(16777216.0), "16777216.0");
        assert_eq!(json_float(1e10), "1E+10");
    }

    #[test]
    fn float_grammar() {
        for t in ["1.", ".5", "-.5", "1e5", "1E-5", "0e0", "+1"] {
            assert!(parse_float(t).is_some(), "{t}");
        }
        for t in ["", "-", "1e", "1e+", "--1", "1.5.5", "inf", "-inf", "nan", "1E400", "e5", "."] {
            assert!(parse_float(t).is_none(), "{t}");
        }
        assert_eq!(parse_integer("-9223372036854775808"), Some(Int::Long(i64::MIN)));
        assert_eq!(parse_integer("9223372036854775808"), Some(Int::Big("9223372036854775808".into())));
        assert_eq!(parse_integer("1x"), None);
    }
}
