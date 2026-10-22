//! `System.TimeSpan` as a natively mapped struct (`tools/cs2rs/native.d/rt_time.json`): the worked example of
//! `tools/cs2rs/NATIVE_SCHEMA.md` ("a library struct as a native Rust type").
//!
//! .NET Framework rules, not .NET Core's: `FromSeconds` / `FromMilliseconds` round to whole milliseconds
//! (`TimeSpan.Interval`), which .NET Core 3.0 changed.

use super::collections::CsKey;
use super::exc::{exc, throw};
use super::gc::{tid, BoxKind, Marker, Trace};
use super::string::Str;

pub const TICKS_PER_MILLISECOND: i64 = 10_000;
pub const TICKS_PER_SECOND: i64 = 10_000_000;
pub const TICKS_PER_MINUTE: i64 = 600_000_000;
pub const TICKS_PER_HOUR: i64 = 36_000_000_000;
pub const TICKS_PER_DAY: i64 = 864_000_000_000;

/// `System.TimeSpan`: one field, as in .NET. All-zero is `TimeSpan.Zero` (= `default(TimeSpan)`).
#[repr(C)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Hash)]
pub struct TimeSpan {
    pub ticks: i64,
}

// SAFETY: no handles inside.
unsafe impl Trace for TimeSpan {
    const NEEDS_TRACE: bool = false;
    const BOX: BoxKind = BoxKind::Value(tid::TIME_SPAN);
    #[inline(always)]
    unsafe fn trace(&self, _m: &mut Marker) {}
    fn cs_compare(&self, other: &Self) -> i32 {
        self.compare_to(*other)
    }
}

/// `Dictionary<TimeSpan, V>` / `List<TimeSpan>.Contains`: `TimeSpan.Equals` + `GetHashCode`.
impl CsKey for TimeSpan {
    fn cs_eq(&self, o: &Self) -> bool {
        self.ticks == o.ticks
    }
    fn cs_hash<H: core::hash::Hasher>(&self, h: &mut H) {
        // TimeSpan.GetHashCode: (int)ticks ^ (int)(ticks >> 32)
        h.write_i32((self.ticks as i32) ^ ((self.ticks >> 32) as i32))
    }
}

#[cold]
fn overflow() -> ! {
    throw(exc("System.OverflowException", Str::lit("TimeSpan overflowed because the duration is too long.")))
}

impl TimeSpan {
    pub const ZERO: TimeSpan = TimeSpan { ticks: 0 };

    /// `new TimeSpan(ticks)`.
    pub const fn new(ticks: i64) -> TimeSpan {
        TimeSpan { ticks }
    }
    /// `new TimeSpan(hours, minutes, seconds)`.
    pub fn from_hms(hours: i32, minutes: i32, seconds: i32) -> TimeSpan {
        let total = hours as i64 * 3600 + minutes as i64 * 60 + seconds as i64;
        if total > i64::MAX / TICKS_PER_SECOND || total < i64::MIN / TICKS_PER_SECOND {
            overflow();
        }
        TimeSpan { ticks: total * TICKS_PER_SECOND }
    }
    /// `TimeSpan.Interval(value, scale)` of .NET Framework: whole milliseconds, rounded half away from zero.
    fn interval(value: f64, scale: f64) -> TimeSpan {
        if value.is_nan() {
            throw(exc("System.ArgumentException", Str::lit("TimeSpan does not accept floating point Not-a-Number values.")));
        }
        let millis = value * scale + if value >= 0.0 { 0.5 } else { -0.5 };
        if millis > (i64::MAX / TICKS_PER_MILLISECOND) as f64 || millis < (i64::MIN / TICKS_PER_MILLISECOND) as f64 {
            overflow();
        }
        TimeSpan { ticks: millis as i64 * TICKS_PER_MILLISECOND }
    }
    pub fn from_seconds(v: f64) -> TimeSpan {
        Self::interval(v, 1000.0)
    }
    pub fn from_milliseconds(v: f64) -> TimeSpan {
        Self::interval(v, 1.0)
    }
    pub fn from_minutes(v: f64) -> TimeSpan {
        Self::interval(v, 60_000.0)
    }
    pub fn from_hours(v: f64) -> TimeSpan {
        Self::interval(v, 3_600_000.0)
    }
    pub fn from_days(v: f64) -> TimeSpan {
        Self::interval(v, 86_400_000.0)
    }
    pub fn total_seconds(&self) -> f64 {
        self.ticks as f64 * 1e-7
    }
    /// `TotalMilliseconds`, clamped to +-MaxMilliSeconds as .NET does.
    pub fn total_milliseconds(&self) -> f64 {
        let t = self.ticks as f64 * 1e-4;
        const MAX: f64 = (i64::MAX / TICKS_PER_MILLISECOND) as f64;
        t.clamp(-MAX, MAX)
    }
    pub fn total_minutes(&self) -> f64 {
        self.ticks as f64 * (1.0 / TICKS_PER_MINUTE as f64)
    }
    pub fn total_hours(&self) -> f64 {
        self.ticks as f64 * (1.0 / TICKS_PER_HOUR as f64)
    }
    pub fn total_days(&self) -> f64 {
        self.ticks as f64 * (1.0 / TICKS_PER_DAY as f64)
    }
    pub fn days(&self) -> i32 {
        (self.ticks / TICKS_PER_DAY) as i32
    }
    pub fn hours(&self) -> i32 {
        ((self.ticks / TICKS_PER_HOUR) % 24) as i32
    }
    pub fn minutes(&self) -> i32 {
        ((self.ticks / TICKS_PER_MINUTE) % 60) as i32
    }
    pub fn seconds(&self) -> i32 {
        ((self.ticks / TICKS_PER_SECOND) % 60) as i32
    }
    pub fn milliseconds(&self) -> i32 {
        ((self.ticks / TICKS_PER_MILLISECOND) % 1000) as i32
    }
    /// `a + b` / `a.Add(b)`: `OverflowException` like .NET.
    pub fn add(self, o: TimeSpan) -> TimeSpan {
        match self.ticks.checked_add(o.ticks) {
            Some(ticks) => TimeSpan { ticks },
            None => overflow(),
        }
    }
    pub fn subtract(self, o: TimeSpan) -> TimeSpan {
        match self.ticks.checked_sub(o.ticks) {
            Some(ticks) => TimeSpan { ticks },
            None => overflow(),
        }
    }
    pub fn compare_to(&self, o: TimeSpan) -> i32 {
        (self.ticks > o.ticks) as i32 - (self.ticks < o.ticks) as i32
    }

    /// `ToString(format)` with a CUSTOM format (.NET Framework 4 `TimeSpanFormat.FormatCustomized`): `d` `h` `m`
    /// `s` (repeat for a minimum width; `hh` / `mm` / `ss` at most two), `f` .. `fffffff` fraction digits, `\x`
    /// and quoted text literal, `%x` a single specifier. Components are the absolute values; anything else is a
    /// `FormatException`. The standard formats (`c`, `g`, `G`) are not implemented.
    pub fn to_string_format(&self, format: Str) -> Str {
        let bad = || -> ! { throw(exc("System.FormatException", Str::lit("Input string was not in a correct format."))) };
        let f: Vec<char> = match format.get() {
            Some(s) if s.chars().count() > 1 || s.starts_with('%') => s.chars().collect(),
            _ => return crate::host::unimplemented_value("System.TimeSpan.ToString(string) with a standard format (c, g, G)"),
        };
        let t = self.ticks.unsigned_abs();
        let day = t / TICKS_PER_DAY as u64;
        let time = t % TICKS_PER_DAY as u64;
        let (hours, minutes, seconds, fraction) = (time / TICKS_PER_HOUR as u64 % 24, time / TICKS_PER_MINUTE as u64 % 60, time / TICKS_PER_SECOND as u64 % 60, time % TICKS_PER_SECOND as u64);
        let mut out = String::new();
        let mut i = 0;
        while i < f.len() {
            let c = f[i];
            let mut run = 1;
            while i + run < f.len() && f[i + run] == c {
                run += 1;
            }
            match c {
                'h' | 'm' | 's' => {
                    if run > 2 {
                        bad()
                    }
                    let v = if c == 'h' { hours } else if c == 'm' { minutes } else { seconds };
                    out.push_str(&format!("{v:0run$}"));
                    i += run;
                }
                'd' => {
                    if run > 8 {
                        bad()
                    }
                    out.push_str(&format!("{day:0run$}"));
                    i += run;
                }
                'f' => {
                    if run > 7 {
                        bad()
                    }
                    let v = fraction / 10u64.pow(7 - run as u32);
                    out.push_str(&format!("{v:0run$}"));
                    i += run;
                }
                '\'' | '"' => {
                    let mut j = i + 1;
                    while j < f.len() && f[j] != c {
                        if f[j] == '\\' && j + 1 < f.len() {
                            j += 1;
                        }
                        out.push(f[j]);
                        j += 1;
                    }
                    if j >= f.len() {
                        bad()
                    }
                    i = j + 1;
                }
                '%' => {
                    if i + 1 < f.len() && f[i + 1] != '%' {
                        i += 1;
                    } else {
                        bad()
                    }
                }
                '\\' => {
                    if i + 1 < f.len() {
                        out.push(f[i + 1]);
                        i += 2;
                    } else {
                        bad()
                    }
                }
                _ => bad(),
            }
        }
        Str::new(&out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rt::{box_id, box_value, try_, unbox};

    #[test]
    fn framework_rounding_and_parts() {
        // .NET Framework: FromSeconds(1.2345) is 1.235 s (whole milliseconds); .NET Core keeps 1.2345
        assert_eq!(TimeSpan::from_seconds(1.2345).ticks, 12_350_000);
        assert_eq!(TimeSpan::from_seconds(-1.2345).ticks, -12_350_000);
        assert_eq!(TimeSpan::from_milliseconds(0.4).ticks, 0);
        let t = TimeSpan::from_hms(26, 3, 4).add(TimeSpan::from_milliseconds(5.0));
        assert_eq!((t.days(), t.hours(), t.minutes(), t.seconds(), t.milliseconds()), (1, 2, 3, 4, 5));
        // TotalSeconds multiplies by 1e-7 (TimeSpan.SecondsPerTick), it does not divide: the last bit shows
        assert_eq!(t.total_seconds(), 93784.00499999999);
        assert!(try_(|| TimeSpan::new(i64::MAX).add(TimeSpan::new(1))).is_err());
        assert!(try_(|| TimeSpan::from_seconds(f64::NAN)).is_err());
    }

    #[test]
    fn boxes_with_its_own_id() {
        let o = box_value(TimeSpan::new(7), box_id::<TimeSpan>());
        assert!(crate::rt::is_boxed(o, tid::TIME_SPAN));
        assert_eq!(unsafe { unbox::<TimeSpan>(o, tid::TIME_SPAN) }.ticks, 7);
        assert_eq!(unsafe { core::mem::zeroed::<TimeSpan>() }, TimeSpan::ZERO);
    }
}
