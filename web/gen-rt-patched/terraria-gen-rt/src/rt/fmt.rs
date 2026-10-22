//! Number formatting and composite formatting as .NET Framework 4 does them: `x.ToString("F2")`,
//! `string.Format("{0,5:0.0%}", x)`, interpolated strings with a format or an alignment.
//!
//! The algorithms are the Framework's (`Number.NumberToString`, `NumberToStringFormat`, `RoundNumber`,
//! `StringBuilder.AppendFormat`): a number becomes decimal digits + a scale first (15 significant digits for a
//! `double`, 7 for a `float`, as the Framework's `DoubleToNumber` does), and all rounding is "half away from zero"
//! on those DECIMAL digits. That is why `2.675.ToString("F2")` is "2.68" there and "2.67" in C.
//!
//! Verified against the real runtime: `tools/cs2rs/tests/framework/fmt_probe.out` was written by
//! `FmtProbe.cs` on .NET Framework 4.8 x86 (5,200 value / format pairs: every format string the game contains,
//! DESIGN.md "Format strings"); the unit test below replays all of it.
//!
//! Cultures: the game's twelve (GameCulture.cs) and the invariant culture, each with the `NumberFormatInfo` the
//! real .NET Framework has for it on Windows 11 ([`NumberFormat`], read from
//! `tools/cs2rs/tests/framework/culture_numbers.tsv`, which `culture_numbers.ps1` wrote): decimal and group
//! separator (de-DE "1.234,5", fr-FR "1 234,5" with U+202F, ru-RU / pl-PL with U+00A0), the percent pattern
//! ("12.5%" en-US, "12,5 %" de-DE / fr-FR / es-ES, "12.5 %" invariant), the words for NaN and infinity. The game
//! makes the selected language's culture the thread's culture (LanguageManager.cs:97-98), so every
//! `float.ToString()` and every format follows it. A culture outside that list is formatted as en-US and recorded
//! as an unimplemented host member.

use super::exc::{exc, throw};
use super::gc::{tid, type_id_of, Gc, Obj};

use super::{Arr, Str};

/// Decimal digits (values 0..=9, no leading zero, empty = zero) and the position of the decimal point:
/// the value is `0.DIGITS x 10^scale`. The Framework's `NumberBuffer`.
#[derive(Clone, Debug)]
struct Number {
    neg: bool,
    digits: Vec<u8>,
    scale: i32,
}

impl Number {
    fn from_int(v: i128) -> Number {
        let neg = v < 0;
        let s = v.unsigned_abs().to_string();
        let mut digits: Vec<u8> = s.bytes().map(|b| b - b'0').collect();
        let scale = digits.len() as i32;
        while digits.last() == Some(&0) {
            digits.pop();
        }
        if digits.is_empty() {
            return Number { neg: false, digits, scale: 0 };
        }
        Number { neg, digits, scale }
    }
    /// `DoubleToNumber(value, precision)`: the value correctly rounded to `precision` significant digits.
    fn from_f64(v: f64, precision: usize) -> Number {
        let neg = v.is_sign_negative();
        let s = format!("{:.*e}", precision - 1, v.abs());
        let (mant, exp) = s.split_once('e').expect("exponent");
        let mut digits: Vec<u8> = mant.bytes().filter(|b| b.is_ascii_digit()).map(|b| b - b'0').collect();
        while digits.last() == Some(&0) {
            digits.pop();
        }
        if digits.is_empty() {
            return Number { neg, digits, scale: 0 };
        }
        Number { neg, digits, scale: exp.parse::<i32>().expect("exponent") + 1 }
    }
    fn is_zero(&self) -> bool {
        self.digits.is_empty()
    }
    /// `RoundNumber(number, pos)`: keeps `pos` digits, rounding half up on the next DECIMAL digit. A number that
    /// becomes zero loses its sign (the Framework's rule; .NET Core 3.0 changed it).
    fn round(&mut self, pos: i32) {
        let mut i = 0usize;
        while (i as i32) < pos && i < self.digits.len() {
            i += 1;
        }
        if i as i32 == pos && i < self.digits.len() && self.digits[i] >= 5 {
            while i > 0 && self.digits[i - 1] == 9 {
                i -= 1;
            }
            if i > 0 {
                self.digits[i - 1] += 1;
            } else {
                self.scale += 1;
                self.digits.clear();
                self.digits.push(1);
                i = 1;
            }
        } else {
            while i > 0 && self.digits[i - 1] == 0 {
                i -= 1;
            }
        }
        self.digits.truncate(i);
        if i == 0 {
            self.scale = 0;
            self.neg = false;
        }
    }
    fn digit(&self, i: i32) -> u8 {
        if i >= 0 && (i as usize) < self.digits.len() {
            self.digits[i as usize]
        } else {
            0
        }
    }
}

fn format_exception() -> ! {
    throw(exc("System.FormatException", Str::lit("Format specifier was invalid.")))
}

/// `System.Globalization.NumberFormatInfo` of one culture: the members number formatting reads.
#[derive(Debug)]
pub struct NumberFormat {
    pub name: String,
    pub decimal: String,
    pub group: String,
    pub negative: String,
    pub positive: String,
    pub percent_decimal: String,
    pub percent_group: String,
    pub percent_symbol: String,
    /// 0 `n %`, 1 `n%`, 2 `%n`, 3 `% n`
    pub percent_positive_pattern: u8,
    /// 0 `-n %`, 1 `-n%`, 2 `-%n`, ...
    pub percent_negative_pattern: u8,
    pub nan: String,
    pub positive_infinity: String,
    pub negative_infinity: String,
    pub per_mille: String,
}
/// A culture's number format (they live for the whole process).
pub type Nf = &'static NumberFormat;

fn number_formats() -> &'static std::collections::HashMap<String, NumberFormat> {
    static TABLE: std::sync::OnceLock<std::collections::HashMap<String, NumberFormat>> = std::sync::OnceLock::new();
    TABLE.get_or_init(|| {
        // written by tools/cs2rs/tests/framework/culture_numbers.ps1 from the real .NET Framework; strings are
        // UTF-16 code units in hex
        let text = include_str!("../../../../../tools/cs2rs/tests/framework/culture_numbers.tsv");
        let hex = |h: &str| -> String {
            if h == "-" {
                return String::new();
            }
            let units: Vec<u16> = (0..h.len() / 4).map(|i| u16::from_str_radix(&h[i * 4..i * 4 + 4], 16).expect("culture_numbers.tsv: hex")).collect();
            String::from_utf16(&units).expect("culture_numbers.tsv: UTF-16")
        };
        let mut m = std::collections::HashMap::new();
        for line in text.lines() {
            let line = line.trim_start_matches('\u{feff}');
            if line.starts_with('#') || line.trim().is_empty() {
                continue;
            }
            let c: Vec<&str> = line.split('\t').collect();
            assert!(c.len() >= 19, "culture_numbers.tsv: {} columns in {line:?}", c.len());
            assert!(c[3] == "3" && c[10] == "3" && c[6] == "1", "culture_numbers.tsv: {}: group sizes other than 3 or a number negative pattern other than -n are not implemented", c[0]);
            let name = if c[0] == "invariant" { String::new() } else { c[0].to_owned() };
            m.insert(
                name.clone(),
                NumberFormat {
                    name,
                    decimal: hex(c[1]),
                    group: hex(c[2]),
                    negative: hex(c[4]),
                    positive: hex(c[5]),
                    percent_decimal: hex(c[8]),
                    percent_group: hex(c[9]),
                    percent_symbol: hex(c[11]),
                    percent_positive_pattern: c[12].parse().expect("pattern"),
                    percent_negative_pattern: c[13].parse().expect("pattern"),
                    nan: hex(c[15]),
                    positive_infinity: hex(c[16]),
                    negative_infinity: hex(c[17]),
                    per_mille: hex(c[18]),
                },
            );
        }
        m
    })
}

/// The number format of the culture called `name` ("" is the invariant culture). A culture the table does not have
/// is formatted as en-US and recorded as an unimplemented host member (once).
pub fn nf_of(name: &str) -> Nf {
    let t = number_formats();
    match t.get(name) {
        Some(f) => f,
        None => {
            crate::host::unimplemented("number formatting for a culture other than the game's twelve and the invariant culture (formatted as en-US)");
            &t["en-US"]
        }
    }
}

/// Which culture's rules: `false` en-US, `true` invariant (the two the first probe compares), or a format.
pub trait CultureSel: Copy {
    fn nf(self) -> Nf;
}
impl CultureSel for bool {
    fn nf(self) -> Nf {
        nf_of(if self { "" } else { "en-US" })
    }
}
impl CultureSel for Nf {
    fn nf(self) -> Nf {
        self
    }
}

/// Integer part with optional group separators, then `decimals` fraction digits.
fn fixed(n: &Number, decimals: i32, group: Option<&str>, decimal: &str, out: &mut String) {
    let int_len = n.scale.max(0);
    if int_len == 0 {
        out.push('0');
    } else {
        for i in 0..int_len {
            out.push((b'0' + n.digit(i)) as char);
            let left = int_len - 1 - i;
            if let Some(g) = group {
                if left > 0 && left % 3 == 0 {
                    out.push_str(g);
                }
            }
        }
    }
    if decimals > 0 {
        out.push_str(decimal);
        for i in 0..decimals {
            out.push((b'0' + n.digit(n.scale + i)) as char);
        }
    }
}

/// What kind of number is formatted: decides which standard formats apply and how `X` and the default print it.
#[derive(Clone, Copy)]
enum Kind {
    Int { bits: u32, value: i128 },
    F32(f32),
    F64(f64),
}

/// NaN and the infinities: the culture's symbols (en-US prints the infinity SIGN: Windows 10 culture data, measured
/// with the probe; the invariant culture the word; ru-RU and zh-Hant have their own word for NaN).
pub(crate) fn non_finite(v: f64, c: impl CultureSel) -> Option<&'static str> {
    let nf = c.nf();
    if v.is_nan() {
        Some(nf.nan.as_str())
    } else if v == f64::INFINITY {
        Some(nf.positive_infinity.as_str())
    } else if v == f64::NEG_INFINITY {
        Some(nf.negative_infinity.as_str())
    } else {
        None
    }
}

/// The invariant text of a default `ToString()` under a culture: its decimal separator and signs.
fn localise_general(s: String, nf: Nf) -> String {
    if nf.decimal == "." && nf.negative == "-" && nf.positive == "+" {
        return s;
    }
    let mut out = String::with_capacity(s.len() + 2);
    for ch in s.chars() {
        match ch {
            '.' => out.push_str(&nf.decimal),
            '-' => out.push_str(&nf.negative),
            '+' => out.push_str(&nf.positive),
            c => out.push(c),
        }
    }
    out
}

/// `float.ToString()` / `double.ToString()` in the CURRENT culture (the translated `x.ToString()` and string
/// concatenation of a floating-point value).
pub fn float_to_string_current(v: f64, single: bool) -> String {
    default_text(if single { Kind::F32(v as f32) } else { Kind::F64(v) }, current_nf())
}

fn default_text(k: Kind, nf: Nf) -> String {
    match k {
        Kind::Int { value, .. } => localise_general(value.to_string(), nf),
        Kind::F32(f) => non_finite(f as f64, nf).map(str::to_owned).unwrap_or_else(|| localise_general(super::string::general_invariant(f as f64, 7), nf)),
        Kind::F64(f) => non_finite(f, nf).map(str::to_owned).unwrap_or_else(|| localise_general(super::string::general_invariant(f, 15), nf)),
    }
}

fn number_of(k: Kind) -> Number {
    match k {
        Kind::Int { value, .. } => Number::from_int(value),
        Kind::F32(f) => Number::from_f64(f as f64, 7),
        Kind::F64(f) => Number::from_f64(f, 15),
    }
}

fn format_kind(k: Kind, fmt: &str, nf: Nf) -> String {
    if fmt.is_empty() {
        return default_text(k, nf);
    }
    let non_finite_text = match k {
        Kind::F32(f) => non_finite(f as f64, nf),
        Kind::F64(f) => non_finite(f, nf),
        _ => None,
    };
    // a standard format: one letter and up to two digits
    let b = fmt.as_bytes();
    if b[0].is_ascii_alphabetic() && b.len() <= 3 && b[1..].iter().all(|c| c.is_ascii_digit()) {
        let prec: i32 = if b.len() > 1 { fmt[1..].parse().unwrap() } else { -1 };
        let letter = b[0].to_ascii_uppercase();
        if let Some(t) = non_finite_text {
            return t.to_owned();
        }
        match letter {
            b'G' if prec < 0 => return default_text(k, nf),
            b'D' => {
                let Kind::Int { value, .. } = k else { format_exception() };
                let digits = value.unsigned_abs().to_string();
                let pad = (prec.max(0) as usize).saturating_sub(digits.len());
                return format!("{}{}{}", if value < 0 { nf.negative.as_str() } else { "" }, "0".repeat(pad), digits);
            }
            b'X' => {
                let Kind::Int { bits, value } = k else { format_exception() };
                let mask: u128 = if bits >= 128 { u128::MAX } else { (1u128 << bits) - 1 };
                let v = (value as u128) & mask;
                let s = if b[0] == b'X' { format!("{v:X}") } else { format!("{v:x}") };
                let pad = (prec.max(0) as usize).saturating_sub(s.len());
                return format!("{}{}", "0".repeat(pad), s);
            }
            b'F' | b'N' | b'P' => {
                let mut n = number_of(k);
                let decimals = if prec < 0 { 2 } else { prec };
                if letter == b'P' {
                    if !n.is_zero() {
                        n.scale += 2;
                    }
                }
                n.round(n.scale + decimals);
                let mut out = String::new();
                if letter == b'P' {
                    // PercentPositivePattern / PercentNegativePattern: where the symbol and the sign go
                    let mut digits = String::new();
                    fixed(&n, decimals, Some(&nf.percent_group), &nf.percent_decimal, &mut digits);
                    let (neg, sym) = (nf.negative.as_str(), nf.percent_symbol.as_str());
                    let pattern = if n.neg { nf.percent_negative_pattern } else { nf.percent_positive_pattern };
                    match (n.neg, pattern) {
                        (false, 0) => out = format!("{digits} {sym}"),
                        (false, 1) => out = format!("{digits}{sym}"),
                        (false, 2) => out = format!("{sym}{digits}"),
                        (false, 3) => out = format!("{sym} {digits}"),
                        (true, 0) => out = format!("{neg}{digits} {sym}"),
                        (true, 1) => out = format!("{neg}{digits}{sym}"),
                        (true, 2) => out = format!("{neg}{sym}{digits}"),
                        _ => return crate::host::unimplemented_value("percent pattern of a culture (rt::fmt implements the positive patterns 0..3 and the negative patterns 0..2)"),
                    }
                    return out;
                }
                if n.neg {
                    out.push_str(&nf.negative);
                }
                fixed(&n, decimals, if letter == b'N' { Some(&nf.group) } else { None }, &nf.decimal, &mut out);
                return out;
            }
            _ => {
                return crate::host::unimplemented_value(Box::leak(format!("number format \"{fmt}\" (rt::fmt implements D, F, N, P, X, G without a precision, and custom patterns)").into_boxed_str()));
            }
        }
    }
    if let Some(t) = non_finite_text {
        return t.to_owned();
    }
    custom(number_of(k), fmt, nf)
}

/// `FindSection(format, section)`: start of the requested `;` section, 0 when it is missing or empty.
fn find_section(f: &[char], mut section: i32) -> usize {
    if section == 0 {
        return 0;
    }
    let mut i = 0;
    while i < f.len() {
        let c = f[i];
        i += 1;
        match c {
            '\'' | '"' => {
                while i < f.len() && f[i] != c {
                    i += 1;
                }
                i += 1;
            }
            '\\' => i += 1,
            ';' => {
                section -= 1;
                if section == 0 {
                    return if i < f.len() && f[i] != ';' { i } else { 0 };
                }
            }
            _ => {}
        }
    }
    0
}

/// `NumberToStringFormat`: a custom numeric pattern (`0`, `#`, `.`, `,`, `%`, sections, literals).
fn custom(mut n: Number, fmt: &str, nf: Nf) -> String {
    let f: Vec<char> = fmt.chars().collect();
    let mut section = find_section(&f, if n.is_zero() { 2 } else if n.neg { 1 } else { 0 });
    let (mut digit_count, mut decimal_pos, mut first_digit, mut last_digit, mut thousand_seps);
    loop {
        digit_count = 0i32;
        decimal_pos = -1i32;
        first_digit = i32::MAX;
        last_digit = 0i32;
        thousand_seps = false;
        let mut thousand_pos = -1i32;
        let mut thousand_count = 0i32;
        let mut scale_adjust = 0i32;
        let mut i = section;
        while i < f.len() && f[i] != ';' {
            let c = f[i];
            i += 1;
            match c {
                '#' => digit_count += 1,
                '0' => {
                    if first_digit == i32::MAX {
                        first_digit = digit_count;
                    }
                    digit_count += 1;
                    last_digit = digit_count;
                }
                '.' => {
                    if decimal_pos < 0 {
                        decimal_pos = digit_count;
                    }
                }
                ',' => {
                    if digit_count > 0 && decimal_pos < 0 {
                        if thousand_pos >= 0 {
                            if thousand_pos == digit_count {
                                thousand_count += 1;
                                continue;
                            }
                            thousand_seps = true;
                        }
                        thousand_pos = digit_count;
                        thousand_count = 1;
                    }
                }
                '%' => scale_adjust += 2,
                '\u{2030}' => scale_adjust += 3,
                '\'' | '"' => {
                    while i < f.len() && f[i] != c {
                        i += 1;
                    }
                    i += 1;
                }
                '\\' => i += 1,
                'E' | 'e' => {
                    // scientific notation (`E0`, `E+0`, `E-0`) is not implemented; a lone E is a literal
                    let mut j = i;
                    if j < f.len() && (f[j] == '+' || f[j] == '-') {
                        j += 1;
                    }
                    if j < f.len() && f[j] == '0' {
                        return crate::host::unimplemented_value(Box::leak(format!("number format \"{fmt}\" (scientific notation in a custom pattern)").into_boxed_str()));
                    }
                }
                _ => {}
            }
        }
        if decimal_pos < 0 {
            decimal_pos = digit_count;
        }
        if thousand_pos >= 0 {
            if thousand_pos == decimal_pos {
                scale_adjust -= thousand_count * 3;
            } else {
                thousand_seps = true;
            }
        }
        if !n.is_zero() {
            n.scale += scale_adjust;
            let pos = n.scale + digit_count - decimal_pos;
            n.round(pos);
            if n.is_zero() {
                let s2 = find_section(&f, 2);
                if s2 != section {
                    section = s2;
                    continue;
                }
            }
        } else {
            n.neg = false;
            n.scale = 0;
        }
        break;
    }
    let first_digit = if first_digit < decimal_pos { decimal_pos - first_digit } else { 0 };
    let last_digit = if last_digit > decimal_pos { decimal_pos - last_digit } else { 0 };
    let mut dig_pos = if n.scale > decimal_pos { n.scale } else { decimal_pos };
    let mut adjust = n.scale - decimal_pos;
    // positions (counted from the decimal point) after which a group separator is written
    let mut sep_pos: Vec<i32> = Vec::new();
    if thousand_seps {
        let total = dig_pos + if adjust < 0 { adjust } else { 0 };
        let num_digits = if first_digit > total { first_digit } else { total };
        let mut g = 3;
        while num_digits > g {
            sep_pos.push(g);
            g += 3;
        }
    }
    let mut out = String::new();
    if n.neg && section == 0 {
        out.push_str(&nf.negative);
    }
    let mut dig = 0usize;
    let mut decimal_written = false;
    let next = |n: &Number, dig: &mut usize| -> Option<u8> {
        if *dig < n.digits.len() {
            *dig += 1;
            Some(n.digits[*dig - 1])
        } else {
            None
        }
    };
    let mut i = section;
    while i < f.len() && f[i] != ';' {
        let c = f[i];
        i += 1;
        if adjust > 0 && matches!(c, '#' | '0' | '.') {
            while adjust > 0 {
                out.push((b'0' + next(&n, &mut dig).unwrap_or(0)) as char);
                if thousand_seps && dig_pos > 1 && sep_pos.last().map(|p| dig_pos == p + 1).unwrap_or(false) {
                    out.push_str(&nf.group);
                    sep_pos.pop();
                }
                dig_pos -= 1;
                adjust -= 1;
            }
        }
        match c {
            '#' | '0' => {
                let ch = if adjust < 0 {
                    adjust += 1;
                    if dig_pos <= first_digit {
                        Some(0u8)
                    } else {
                        None
                    }
                } else {
                    match next(&n, &mut dig) {
                        Some(d) => Some(d),
                        None => {
                            if dig_pos > last_digit {
                                Some(0)
                            } else {
                                None
                            }
                        }
                    }
                };
                if let Some(d) = ch {
                    out.push((b'0' + d) as char);
                    if thousand_seps && dig_pos > 1 && sep_pos.last().map(|p| dig_pos == p + 1).unwrap_or(false) {
                        out.push_str(&nf.group);
                        sep_pos.pop();
                    }
                }
                dig_pos -= 1;
            }
            '.' => {
                if dig_pos != 0 || decimal_written {
                    continue;
                }
                if last_digit < 0 || (decimal_pos < digit_count && dig < n.digits.len()) {
                    out.push_str(&nf.decimal);
                    decimal_written = true;
                }
            }
            ',' => {}
            '\'' | '"' => {
                while i < f.len() && f[i] != c {
                    out.push(f[i]);
                    i += 1;
                }
                i += 1;
            }
            '\\' => {
                if i < f.len() {
                    out.push(f[i]);
                    i += 1;
                }
            }
            // the culture's symbols (NumberToStringFormat appends PercentSymbol / PerMilleSymbol)
            '%' => out.push_str(&nf.percent_symbol),
            '\u{2030}' => out.push_str(&nf.per_mille),
            _ => out.push(c),
        }
    }
    // (the group separators are consumed from the largest position down)
    let _ = &mut sep_pos;
    out
}

/// A primitive number that can be formatted (`IFormattable` of the numeric types).
pub trait Formattable: Copy {
    #[doc(hidden)]
    fn fmt_kind(self) -> KindOf;
}
#[doc(hidden)]
pub struct KindOf(Kind);
macro_rules! formattable_int {
    ($($t:ty),*) => {$( impl Formattable for $t {
        fn fmt_kind(self) -> KindOf { KindOf(Kind::Int { bits: <$t>::BITS, value: self as i128 }) }
    } )*};
}
formattable_int!(u8, i8, u16, i16, u32, i32, u64, i64);
impl Formattable for f32 {
    fn fmt_kind(self) -> KindOf {
        KindOf(Kind::F32(self))
    }
}
impl Formattable for f64 {
    fn fmt_kind(self) -> KindOf {
        KindOf(Kind::F64(self))
    }
}

/// The number format of the thread's current culture (`Thread.CurrentThread.CurrentCulture`).
pub fn current_nf() -> Nf {
    let c = super::sys::CultureInfo::current();
    // SAFETY: the current culture is a live, non-null object.
    nf_of(unsafe { (*c.ptr()).name.as_str() })
}

/// The number format of a provider. A null provider means the current culture, as in .NET.
fn provider_nf(provider: Obj) -> Nf {
    if provider.is_null() {
        return current_nf();
    }
    let c: Gc<super::sys::CultureInfo> = super::as_rt(provider);
    if c.is_null() {
        return crate::host::unimplemented_value("IFormatProvider that is not a CultureInfo (rt::fmt)");
    }
    // SAFETY: checked downcast.
    nf_of(unsafe { (*c.ptr()).name.as_str() })
}

/// The text .NET Framework prints for `value.ToString(format, culture)`; `culture` selects the rules.
pub fn number_to_string<T: Formattable>(value: T, format: &str, culture: impl CultureSel) -> String {
    format_kind(value.fmt_kind().0, format, culture.nf())
}

/// `value.ToString(format)`: the current culture. A null format is the default format.
pub fn to_string_fmt<T: Formattable>(value: T, format: Str) -> Str {
    Str::new(&number_to_string(value, format.get().unwrap_or(""), current_nf()))
}

/// `value.ToString(format, provider)` / `value.ToString(provider)` (pass a null format).
pub fn to_string_provider<T: Formattable, P>(value: T, format: Str, provider: Gc<P>) -> Str {
    Str::new(&number_to_string(value, format.get().unwrap_or(""), provider_nf(provider.up())))
}

/// One argument of a composite format: `arg.ToString(format, provider)` when the argument is `IFormattable`
/// (numbers, enums, `TimeSpan`), else `arg.ToString()`; null is the empty string.
fn format_arg(arg: Obj, spec: &str, nf: Nf) -> String {
    if arg.is_null() {
        return String::new();
    }
    // SAFETY: non-null objects have a header; the id says what the box holds.
    unsafe {
        let id = type_id_of(arg.0 as *const u8);
        let p = arg.0 as *const u8;
        let kind = |id: u32| -> Option<Kind> {
            Some(match id {
                tid::U8 => Kind::Int { bits: 8, value: *p as i128 },
                tid::I8 => Kind::Int { bits: 8, value: *(p as *const i8) as i128 },
                tid::I16 => Kind::Int { bits: 16, value: *(p as *const i16) as i128 },
                tid::U16 => Kind::Int { bits: 16, value: *(p as *const u16) as i128 },
                tid::I32 => Kind::Int { bits: 32, value: *(p as *const i32) as i128 },
                tid::U32 => Kind::Int { bits: 32, value: *(p as *const u32) as i128 },
                tid::I64 => Kind::Int { bits: 64, value: *(p as *const i64) as i128 },
                tid::U64 => Kind::Int { bits: 64, value: *(p as *const u64) as i128 },
                tid::F32 => Kind::F32(*(p as *const f32)),
                tid::F64 => Kind::F64(*(p as *const f64)),
                _ => return None,
            })
        };
        if let Some(k) = kind(id) {
            return format_kind(k, spec, nf);
        }
        if let Some(info) = super::enum_info(id) {
            // Enum.ToString(format): G (the name), D (the number), X (hexadecimal, the width of the underlying type)
            return match spec {
                "" | "G" | "g" | "F" | "f" => super::obj_to_string(arg).as_str().to_owned(),
                "D" | "d" => format_kind(kind(info.underlying).expect("enum underlying type"), "", nf),
                "X" | "x" => match kind(info.underlying) {
                    Some(Kind::Int { bits, value }) => format_kind(Kind::Int { bits, value }, &format!("X{}", bits / 4), nf),
                    _ => format_exception(),
                },
                _ => format_exception(),
            };
        }
        if id == tid::TIME_SPAN && !spec.is_empty() {
            return (*(p as *const super::time::TimeSpan)).to_string_format(Str::new(spec)).as_str().to_owned();
        }
        // not IFormattable (string, bool, char, a class): ToString(), the format is ignored
        super::obj_to_string(arg).as_str().to_owned()
    }
}

fn bad_format() -> ! {
    throw(exc("System.FormatException", Str::lit("Input string was not in a correct format.")))
}

/// `StringBuilder.AppendFormat(provider, format, args)`: the composite format of .NET Framework 4.
pub fn composite(format: &str, args: &[Obj], culture: impl CultureSel) -> String {
    let nf = culture.nf();
    let f: Vec<char> = format.chars().collect();
    let mut out = String::new();
    let mut pos = 0usize;
    let len = f.len();
    loop {
        // literal text up to the next item
        let mut ch = '\0';
        while pos < len {
            ch = f[pos];
            pos += 1;
            if ch == '}' {
                if pos < len && f[pos] == '}' {
                    pos += 1;
                } else {
                    bad_format();
                }
            }
            if ch == '{' {
                if pos < len && f[pos] == '{' {
                    pos += 1;
                } else {
                    pos -= 1;
                    break;
                }
            }
            out.push(ch);
        }
        if pos == len {
            break;
        }
        pos += 1;
        if pos == len || !f[pos].is_ascii_digit() {
            bad_format();
        }
        let mut index = 0usize;
        loop {
            index = index * 10 + f[pos] as usize - '0' as usize;
            pos += 1;
            if pos == len {
                bad_format();
            }
            ch = f[pos];
            if !(ch.is_ascii_digit() && index < 1_000_000) {
                break;
            }
        }
        if index >= args.len() {
            throw(exc("System.FormatException", Str::lit("Index (zero based) must be greater than or equal to zero and less than the size of the argument list.")));
        }
        while pos < len && f[pos] == ' ' {
            pos += 1;
        }
        ch = if pos < len { f[pos] } else { '\0' };
        let mut left = false;
        let mut width = 0usize;
        if ch == ',' {
            pos += 1;
            while pos < len && f[pos] == ' ' {
                pos += 1;
            }
            if pos == len {
                bad_format();
            }
            ch = f[pos];
            if ch == '-' {
                left = true;
                pos += 1;
                if pos == len {
                    bad_format();
                }
                ch = f[pos];
            }
            if !ch.is_ascii_digit() {
                bad_format();
            }
            loop {
                width = width * 10 + ch as usize - '0' as usize;
                pos += 1;
                if pos == len {
                    bad_format();
                }
                ch = f[pos];
                if !(ch.is_ascii_digit() && width < 1_000_000) {
                    break;
                }
            }
        }
        while pos < len && f[pos] == ' ' {
            pos += 1;
        }
        ch = if pos < len { f[pos] } else { '\0' };
        let mut spec = String::new();
        if ch == ':' {
            pos += 1;
            loop {
                if pos == len {
                    bad_format();
                }
                ch = f[pos];
                pos += 1;
                if ch == '{' {
                    if pos < len && f[pos] == '{' {
                        pos += 1;
                    } else {
                        bad_format();
                    }
                } else if ch == '}' {
                    if pos < len && f[pos] == '}' {
                        pos += 1;
                    } else {
                        pos -= 1;
                        break;
                    }
                }
                spec.push(ch);
            }
        }
        if pos >= len || f[pos] != '}' {
            bad_format();
        }
        pos += 1;
        let s = format_arg(args[index], &spec, nf);
        let n = s.encode_utf16().count();
        let pad = width.saturating_sub(n);
        if !left {
            out.extend(std::iter::repeat(' ').take(pad));
        }
        out.push_str(&s);
        if left {
            out.extend(std::iter::repeat(' ').take(pad));
        }
    }
    out
}

fn need_format(format: Str) -> &'static str {
    match format.get() {
        Some(s) => s,
        None => throw(exc("System.ArgumentNullException", Str::lit("format"))),
    }
}

/// `string.Format(format, a)` / `(format, a, b)` / `(format, a, b, c)`: the current culture.
pub fn format(format: Str, args: &[Obj]) -> Str {
    Str::new(&composite(need_format(format), args, current_nf()))
}

/// `string.Format(format, params object[] args)`.
pub fn format_arr(format: Str, args: Arr<Obj>) -> Str {
    if args.is_null() {
        throw(exc("System.ArgumentNullException", Str::lit("args")))
    }
    Str::new(&composite(need_format(format), args.as_slice(), current_nf()))
}

/// `string.Format(provider, format, params object[] args)`.
pub fn format_provider<P>(provider: Gc<P>, format: Str, args: Arr<Obj>) -> Str {
    if args.is_null() {
        throw(exc("System.ArgumentNullException", Str::lit("args")))
    }
    Str::new(&composite(need_format(format), args.as_slice(), provider_nf(provider.up())))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rt::{box_value, char_str, str_obj, try_};

    fn unesc(s: &str) -> String {
        let mut out = String::new();
        let mut it = s.chars();
        while let Some(c) = it.next() {
            if c == '\\' {
                match it.next() {
                    Some('t') => out.push('\t'),
                    Some('n') => out.push('\n'),
                    Some(o) => out.push(o),
                    None => {}
                }
            } else {
                out.push(c);
            }
        }
        out
    }

    /// Every line of the probe that the real .NET Framework 4.8 (x86) wrote.
    #[test]
    fn matches_dotnet_framework() {
        let data = include_str!("../../../../../tools/cs2rs/tests/framework/fmt_probe.out");
        let (mut n, mut bad) = (0, Vec::new());
        for line in data.lines() {
            let p: Vec<&str> = line.split('\t').collect();
            assert_eq!(p.len(), 5, "probe line: {line}");
            // the format column is escaped only on the composite lines (FmtProbe.cs)
            let (kind, value, fmt, en, inv) = (p[0], p[1], if p[0] == "fmt" { unesc(p[2]) } else { p[2].to_owned() }, unesc(p[3]), unesc(p[4]));
            // R (round-trip) is not a format of the game: the probe has it for reference only
            if fmt == "R" {
                continue;
            }
            let both = |f: &dyn Fn(bool) -> String| (f(false), f(true));
            let got = try_(|| match kind {
                "i32" => both(&|i| number_to_string(value.parse::<i32>().unwrap(), &fmt, i)),
                "i64" => both(&|i| number_to_string(value.parse::<i64>().unwrap(), &fmt, i)),
                "u8" => both(&|i| number_to_string(value.parse::<u8>().unwrap(), &fmt, i)),
                "i16" => both(&|i| number_to_string(value.parse::<i16>().unwrap(), &fmt, i)),
                "u32" => both(&|i| number_to_string(value.parse::<u32>().unwrap(), &fmt, i)),
                "f64" => both(&|i| number_to_string(f64::from_bits(u64::from_str_radix(value, 16).unwrap()), &fmt, i)),
                "f32" => both(&|i| number_to_string(f32::from_bits(u32::from_str_radix(value, 16).unwrap()), &fmt, i)),
                "fmt" => {
                    let args: Vec<Obj> = value
                        .split(' ')
                        .map(|a| {
                            let (k, v) = a.split_once(':').unwrap_or((a, ""));
                            match k {
                                "null" => Gc::NULL,
                                "i32" => box_value(v.parse::<i32>().unwrap(), tid::I32),
                                "u8" => box_value(v.parse::<u8>().unwrap(), tid::U8),
                                "str" => str_obj(Str::new(&unesc(v))),
                                "bool" => box_value(v == "1", tid::BOOL),
                                "char" => box_value(v.parse::<u16>().unwrap(), tid::CHAR),
                                "f32" => box_value(f32::from_bits(u32::from_str_radix(v, 16).unwrap()), tid::F32),
                                "f64" => box_value(f64::from_bits(u64::from_str_radix(v, 16).unwrap()), tid::F64),
                                other => panic!("probe argument kind {other}"),
                            }
                        })
                        .collect();
                    both(&|i| composite(&fmt, &args, i))
                }
                "span" => {
                    let t = crate::rt::time::TimeSpan { ticks: value.parse().unwrap() };
                    // column 4: the custom format; column 5: TimeSpan.ToString()
                    (t.to_string_format(Str::new(&fmt)).as_str().to_owned(), inv.clone())
                }
                other => panic!("probe kind {other}"),
            });
            let got = match got {
                Ok(g) => g,
                Err(e) => {
                    bad.push(format!("{kind} {value} \"{fmt}\": threw {}", crate::rt::exc_text(e)));
                    continue;
                }
            };
            n += 1;
            if got.0 != en || got.1 != inv {
                bad.push(format!("{kind} {value} \"{fmt}\": got en-US {:?} invariant {:?}, .NET Framework {:?} / {:?}", got.0, got.1, en, inv));
            }
        }
        assert!(n > 5000, "probe lines read: {n}");
        assert!(bad.is_empty(), "{} of {n} differ from .NET Framework:\n{}", bad.len(), bad.iter().take(40).cloned().collect::<Vec<_>>().join("\n"));
    }

    /// Every line of `culture_fmt_probe.out` (written by `culture_numbers.ps1` from the real .NET Framework): the
    /// game's twelve cultures and the invariant culture x 23 floating-point values (as `float` and as `double`) x
    /// 17 formats and 7 integers x 11 formats.
    #[test]
    fn culture_formats_match_dotnet() {
        let text = include_str!("../../../../../tools/cs2rs/tests/framework/culture_fmt_probe.out");
        let hex = |h: &str| -> String {
            if h == "-" {
                return String::new();
            }
            let units: Vec<u16> = (0..h.len() / 4).map(|i| u16::from_str_radix(&h[i * 4..i * 4 + 4], 16).unwrap()).collect();
            String::from_utf16(&units).unwrap()
        };
        let (mut n, mut bad) = (0usize, Vec::new());
        let mut cultures = std::collections::BTreeSet::new();
        for line in text.lines() {
            let line = line.trim_start_matches('\u{feff}');
            let c: Vec<&str> = line.split('\t').collect();
            if c.len() < 5 {
                continue;
            }
            let nf = nf_of(if c[0] == "invariant" { "" } else { c[0] });
            cultures.insert(c[0].to_owned());
            let fmt = if c[3] == "-" { "" } else { c[3] };
            let want = hex(c[4]);
            let got = try_(|| match c[1] {
                "f64" => number_to_string(f64::from_bits(c[2].parse::<u64>().unwrap()), fmt, nf),
                "f32" => number_to_string(f32::from_bits(c[2].parse::<u32>().unwrap()), fmt, nf),
                "i32" => number_to_string(c[2].parse::<i32>().unwrap(), fmt, nf),
                other => panic!("probe kind {other}"),
            });
            n += 1;
            match got {
                Ok(g) if g == want => {}
                Ok(g) => bad.push(format!("{} {} {} {:?}: got {:?}, .NET Framework {:?}", c[0], c[1], c[2], c[3], g, want)),
                Err(e) => bad.push(format!("{} {} {} {:?}: threw {}", c[0], c[1], c[2], c[3], crate::rt::exc_text(e))),
            }
        }
        assert_eq!(cultures.len(), 13, "{cultures:?}");
        assert!(n > 11000, "probe lines read: {n}");
        assert!(bad.is_empty(), "{} of {n} differ from .NET Framework:\n{}", bad.len(), bad.iter().take(40).cloned().collect::<Vec<_>>().join("\n"));
        // the current culture decides `x.ToString()` and string.Format
        let before = crate::rt::sys::CultureInfo::current();
        crate::rt::sys::CultureInfo::set_current(crate::rt::sys::CultureInfo::new_gc(Str::lit("de-DE")));
        assert_eq!(crate::rt::to_str(1.5f32).as_str(), "1,5");
        assert_eq!(to_string_fmt(1234.5f64, Str::lit("N1")).as_str(), "1.234,5");
        assert_eq!(format(Str::lit("{0:0.0%} {1}"), &[box_value(0.125f64, tid::F64), box_value(2.5f32, tid::F32)]).as_str(), "12,5% 2,5");
        assert_eq!(to_string_provider(1.5f64, Str::NULL, crate::rt::sys::CultureInfo::invariant()).as_str(), "1.5");
        crate::rt::sys::CultureInfo::set_current(before);
        assert_eq!(crate::rt::to_str(1.5f32).as_str(), "1.5");
    }

    #[test]
    fn composite_errors_and_culture() {
        let one = [box_value(1i32, tid::I32)];
        assert!(try_(|| composite("{1}", &one, false)).is_err());
        assert!(try_(|| composite("{", &one, false)).is_err());
        assert!(try_(|| composite("}", &one, false)).is_err());
        assert!(try_(|| composite("{0", &one, false)).is_err());
        assert!(try_(|| composite("{a}", &one, false)).is_err());
        assert_eq!(composite("{0}{0}", &[str_obj(char_str(b'x' as u16))], false), "xx");
        // the thread starts as en-US
        assert_eq!(to_string_fmt(0.5f64, Str::lit("P0")).as_str(), "50%");
        assert_eq!(to_string_provider(0.5f64, Str::lit("P0"), crate::rt::sys::CultureInfo::invariant()).as_str(), "50 %");
        assert_eq!(format(Str::lit("{0:F1}/{1}"), &[box_value(2.25f32, tid::F32), Gc::NULL]).as_str(), "2.3/");
    }
}
