//! `System.DateTime`, `System.Diagnostics.Stopwatch`, `Environment.TickCount`.
//!
//! `TimeSpan` is the runtime's (`rt::time::TimeSpan`). `DateTime` has .NET's layout: one 64-bit word, ticks in
//! the low 62 bits and the `DateTimeKind` in the top two. Expected values: tests/dotnet/system_probe.ps1.

use std::sync::OnceLock;
use std::time::Instant;

use terraria_gen_rt::host;

use super::cultures::{Culture, CULTURES};
use terraria_gen_rt::rt::{self, time::TimeSpan, Gc, Str};

use crate::gfx::states::{plain_trace, throw};

pub const TICKS_PER_DAY: i64 = 864_000_000_000;
pub const UNIX_EPOCH_TICKS: i64 = 621_355_968_000_000_000;
pub const FILE_TIME_EPOCH_TICKS: i64 = 504_911_232_000_000_000;
pub const MAX_TICKS: i64 = 3_155_378_975_999_999_999;

pub const KIND_UNSPECIFIED: i32 = 0;
pub const KIND_UTC: i32 = 1;
pub const KIND_LOCAL: i32 = 2;

const TICKS_MASK: u64 = 0x3FFF_FFFF_FFFF_FFFF;

/// `System.DateTime`. All-zero is `DateTime.MinValue` / `default(DateTime)`.
#[repr(C)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct DateTime {
    pub data: u64,
}
// SAFETY: no handles inside. Boxing (`string.Format("{0}", DateTime.Now)`, concatenation): value equality by ticks,
// `ToString()` is the general format of the current culture.
unsafe impl rt::Trace for DateTime {
    const NEEDS_TRACE: bool = false;
    const BOX: rt::BoxKind = rt::BoxKind::Value(crate::tid::DATE_TIME);
    unsafe fn trace(&self, _m: &mut rt::Marker) {}
    unsafe fn obj_equals(this: *const Self, other: rt::Obj) -> bool {
        !other.is_null() && rt::type_id_of(other.0 as *const u8) == crate::tid::DATE_TIME && (*this).ticks() == (*(other.0 as *const DateTime)).ticks()
    }
    unsafe fn obj_hash(this: *const Self) -> i32 {
        let t = (*this).ticks();
        (t as i32) ^ ((t >> 32) as i32)
    }
    unsafe fn obj_to_string(this: *const Self) -> Str {
        (*this).to_string_general()
    }
}
impl rt::BoxedText for DateTime {
    fn text(self) -> Str {
        self.to_string_general()
    }
}

/// `DateTimeFormatInfo.LongTimePattern` of the game's cultures, read from the .NET Framework 4.8 on Windows 11
/// (`[CultureInfo]::GetCultureInfo(name).DateTimeFormat.LongTimePattern`, 2026-10-11). Not part of `cultures.rs` because
/// only `DateTime.ToString()` needs it.
fn long_time_pattern(culture: &str) -> &'static str {
    match culture {
        "en-US" => "h:mm:ss tt",
        "ko-KR" => "tt h:mm:ss",
        "es-ES" | "ru-RU" | "zh-Hans" | "ja-JP" | "zh-Hant" => "H:mm:ss",
        // invariant, de-DE, it-IT, fr-FR, pt-BR, pl-PL
        _ => "HH:mm:ss",
    }
}

const DAYS_TO_MONTH_365: [i32; 13] = [0, 31, 59, 90, 120, 151, 181, 212, 243, 273, 304, 334, 365];
const DAYS_TO_MONTH_366: [i32; 13] = [0, 31, 60, 91, 121, 152, 182, 213, 244, 274, 305, 335, 366];

fn is_leap(y: i32) -> bool {
    y % 4 == 0 && (y % 100 != 0 || y % 400 == 0)
}

#[cold]
fn out_of_range() -> ! {
    throw("System.ArgumentOutOfRangeException", "The added or subtracted value results in an un-representable DateTime.")
}

/// Local time minus UTC, in ticks, right now (whole minutes).
pub fn local_offset_ticks() -> i64 {
    #[cfg(windows)]
    {
        #[repr(C)]
        #[derive(Default)]
        struct SystemTime {
            year: u16,
            month: u16,
            day_of_week: u16,
            day: u16,
            hour: u16,
            minute: u16,
            second: u16,
            milliseconds: u16,
        }
        #[link(name = "kernel32")]
        extern "system" {
            fn GetLocalTime(t: *mut SystemTime);
            fn GetSystemTime(t: *mut SystemTime);
        }
        let (mut l, mut u) = (SystemTime::default(), SystemTime::default());
        // SAFETY: both calls fill the struct they are given.
        unsafe {
            GetSystemTime(&mut u);
            GetLocalTime(&mut l);
        }
        let t = |s: &SystemTime| DateTime::from_parts(s.year as i32, s.month as i32, s.day as i32, s.hour as i32, s.minute as i32, s.second as i32, 0, KIND_UNSPECIFIED).ticks();
        let minutes = ((t(&l) - t(&u)) as f64 / 600_000_000.0).round() as i64;
        minutes * 600_000_000
    }
    #[cfg(not(windows))]
    {
        0
    }
}

impl DateTime {
    pub const MIN_VALUE: DateTime = DateTime { data: 0 };

    pub fn from_ticks_kind(ticks: i64, kind: i32) -> DateTime {
        if !(0..=MAX_TICKS).contains(&ticks) {
            throw("System.ArgumentOutOfRangeException", "Ticks must be between DateTime.MinValue.Ticks and DateTime.MaxValue.Ticks.");
        }
        DateTime { data: ticks as u64 | ((kind as u64 & 3) << 62) }
    }
    /// `new DateTime(ticks)`
    pub fn from_ticks(ticks: i64) -> DateTime {
        Self::from_ticks_kind(ticks, KIND_UNSPECIFIED)
    }
    /// `new DateTime(year, month, day, hour, minute, second, millisecond, kind)`
    #[allow(clippy::too_many_arguments)]
    pub fn from_parts(year: i32, month: i32, day: i32, hour: i32, minute: i32, second: i32, millisecond: i32, kind: i32) -> DateTime {
        if !(1..=9999).contains(&year) || !(1..=12).contains(&month) {
            throw("System.ArgumentOutOfRangeException", "Year, Month, and Day parameters describe an un-representable DateTime.");
        }
        let days = if is_leap(year) { &DAYS_TO_MONTH_366 } else { &DAYS_TO_MONTH_365 };
        if day < 1 || day > days[month as usize] - days[month as usize - 1] {
            throw("System.ArgumentOutOfRangeException", "Year, Month, and Day parameters describe an un-representable DateTime.");
        }
        if !(0..24).contains(&hour) || !(0..60).contains(&minute) || !(0..60).contains(&second) || !(0..1000).contains(&millisecond) {
            throw("System.ArgumentOutOfRangeException", "Hour, Minute, and Second parameters describe an un-representable DateTime.");
        }
        let y = (year - 1) as i64;
        let n = y * 365 + y / 4 - y / 100 + y / 400 + days[month as usize - 1] as i64 + day as i64 - 1;
        let ticks = n * TICKS_PER_DAY + (hour as i64 * 3600 + minute as i64 * 60 + second as i64) * 10_000_000 + millisecond as i64 * 10_000;
        Self::from_ticks_kind(ticks, kind)
    }
    /// `new DateTime(year, month, day)`
    pub fn from_ymd(year: i32, month: i32, day: i32) -> DateTime {
        Self::from_parts(year, month, day, 0, 0, 0, 0, KIND_UNSPECIFIED)
    }

    /// `DateTime.UtcNow` ([`set_fixed_now`]: the pinned instant)
    pub fn utc_now() -> DateTime {
        if let Some(t) = FIXED_NOW.get() {
            return Self::from_ticks_kind(t, KIND_UTC);
        }
        let d = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default();
        Self::from_ticks_kind(UNIX_EPOCH_TICKS + (d.as_nanos() / 100) as i64, KIND_UTC)
    }
    /// `DateTime.Now` ([`set_fixed_now`]: the pinned instant)
    pub fn now() -> DateTime {
        if let Some(t) = FIXED_NOW.get() {
            return Self::from_ticks_kind(t, KIND_LOCAL);
        }
        Self::from_ticks_kind(Self::utc_now().ticks() + local_offset_ticks(), KIND_LOCAL)
    }

    pub fn ticks(&self) -> i64 {
        (self.data & TICKS_MASK) as i64
    }
    /// `Kind` as the `DateTimeKind` value.
    pub fn kind(&self) -> i32 {
        (self.data >> 62) as i32 & 3
    }

    /// (year, month, day, day of year) of the date part: `DateTime.GetDatePart`.
    fn date(&self) -> (i32, i32, i32, i32) {
        let mut n = (self.ticks() / TICKS_PER_DAY) as i32;
        let y400 = n / 146097;
        n -= y400 * 146097;
        let mut y100 = n / 36524;
        if y100 == 4 {
            y100 = 3;
        }
        n -= y100 * 36524;
        let y4 = n / 1461;
        n -= y4 * 1461;
        let mut y1 = n / 365;
        if y1 == 4 {
            y1 = 3;
        }
        let year = y400 * 400 + y100 * 100 + y4 * 4 + y1 + 1;
        n -= y1 * 365;
        let leap = y1 == 3 && (y4 != 24 || y100 == 3);
        let days = if leap { &DAYS_TO_MONTH_366 } else { &DAYS_TO_MONTH_365 };
        let mut m = (n >> 5) + 1;
        while n >= days[m as usize] {
            m += 1;
        }
        (year, m, n - days[m as usize - 1] + 1, n + 1)
    }
    pub fn year(&self) -> i32 {
        self.date().0
    }
    pub fn month(&self) -> i32 {
        self.date().1
    }
    pub fn day(&self) -> i32 {
        self.date().2
    }
    pub fn day_of_year(&self) -> i32 {
        self.date().3
    }
    /// `DayOfWeek`: Sunday = 0.
    pub fn day_of_week(&self) -> i32 {
        ((self.ticks() / TICKS_PER_DAY + 1) % 7) as i32
    }
    pub fn hour(&self) -> i32 {
        ((self.ticks() / 36_000_000_000) % 24) as i32
    }
    pub fn minute(&self) -> i32 {
        ((self.ticks() / 600_000_000) % 60) as i32
    }
    pub fn second(&self) -> i32 {
        ((self.ticks() / 10_000_000) % 60) as i32
    }
    pub fn millisecond(&self) -> i32 {
        ((self.ticks() / 10_000) % 1000) as i32
    }

    fn add_ticks(&self, value: i64) -> DateTime {
        let t = self.ticks();
        if value > MAX_TICKS - t || value < -t {
            out_of_range();
        }
        DateTime { data: (t + value) as u64 | (self.data & !TICKS_MASK) }
    }
    /// `DateTime.Add(double value, int scale)`: the value is rounded to whole milliseconds (half away from zero).
    fn add_scaled(&self, value: f64, scale: f64) -> DateTime {
        let millis = (value * scale + if value >= 0.0 { 0.5 } else { -0.5 }) as i64;
        if !(-315_537_897_600_000..=315_537_897_600_000).contains(&millis) {
            out_of_range();
        }
        self.add_ticks(millis * 10_000)
    }
    pub fn add_seconds(&self, v: f64) -> DateTime {
        self.add_scaled(v, 1000.0)
    }
    pub fn add_minutes(&self, v: f64) -> DateTime {
        self.add_scaled(v, 60_000.0)
    }
    pub fn add_hours(&self, v: f64) -> DateTime {
        self.add_scaled(v, 3_600_000.0)
    }
    pub fn add_days(&self, v: f64) -> DateTime {
        self.add_scaled(v, 86_400_000.0)
    }
    pub fn add_milliseconds(&self, v: f64) -> DateTime {
        self.add_scaled(v, 1.0)
    }
    /// `Add(TimeSpan)` / `operator +`
    pub fn add(&self, t: TimeSpan) -> DateTime {
        self.add_ticks(t.ticks)
    }
    /// `operator -(DateTime, DateTime)`: the kinds are ignored, as in .NET.
    pub fn subtract(&self, o: DateTime) -> TimeSpan {
        TimeSpan::new(self.ticks() - o.ticks())
    }
    /// `operator -(DateTime, TimeSpan)`
    pub fn subtract_span(&self, t: TimeSpan) -> DateTime {
        self.add_ticks(-t.ticks)
    }
    /// `CompareTo` / the comparison operators: by ticks only.
    pub fn compare_to(&self, o: DateTime) -> i32 {
        self.ticks().cmp(&o.ticks()) as i32
    }
    pub fn ticks_eq(&self, o: DateTime) -> bool {
        self.ticks() == o.ticks()
    }

    /// `ToBinary()`: Utc and Unspecified are the raw word; Local is stored as UTC ticks with the local flag.
    pub fn to_binary(&self) -> i64 {
        if self.kind() == KIND_LOCAL {
            let mut t = self.ticks() - local_offset_ticks();
            if t < 0 {
                t += 0x4000_0000_0000_0000;
            }
            (t as u64 | 0x8000_0000_0000_0000) as i64
        } else {
            self.data as i64
        }
    }
    /// `DateTime.FromBinary`
    pub fn from_binary(data: i64) -> DateTime {
        if data as u64 & 0x8000_0000_0000_0000 != 0 {
            let mut t = (data as u64 & TICKS_MASK) as i64;
            if t > 0x3FFF_FFFF_FFFF_FFFF - TICKS_PER_DAY {
                t -= 0x4000_0000_0000_0000;
            }
            let local = (t + local_offset_ticks()).clamp(0, MAX_TICKS);
            DateTime::from_ticks_kind(local, KIND_LOCAL)
        } else {
            let t = (data as u64 & TICKS_MASK) as i64;
            if t > MAX_TICKS {
                throw("System.ArgumentException", "Valid values are between DateTime.MinValue.Ticks and DateTime.MaxValue.Ticks.");
            }
            DateTime { data: data as u64 }
        }
    }
    /// `ToUniversalTime()`
    pub fn to_universal_time(&self) -> DateTime {
        if self.kind() == KIND_UTC {
            *self
        } else {
            DateTime::from_ticks_kind((self.ticks() - local_offset_ticks()).clamp(0, MAX_TICKS), KIND_UTC)
        }
    }
    /// `ToLocalTime()`
    pub fn to_local_time(&self) -> DateTime {
        if self.kind() == KIND_LOCAL {
            *self
        } else {
            DateTime::from_ticks_kind((self.ticks() + local_offset_ticks()).clamp(0, MAX_TICKS), KIND_LOCAL)
        }
    }

    /// `ToString(format)` with the thread's current culture (`Thread.CurrentThread.CurrentCulture`, set from the
    /// active language by `LanguageManager`; en-US until then). The data of the game's twelve cultures and the invariant
    /// one comes from the real .NET Framework (`cultures.rs`, `tools/gen_cultures.ps1`); tests/dotnet/culture_probe.ps1
    /// checks the formatted values against it.
    /// Standard formats: `D` (the culture's long date pattern), `d` (short date). Custom specifiers: `y yy yyy yyyy`,
    /// `M MM MMM MMMM` (genitive month names where .NET uses them: next to a `d` / `dd`), `d dd ddd dddd`, `H HH`, `h hh`,
    /// `m mm`, `s ss`, `f`.. `fffffff`, `t tt`, `/` and `:` (the culture's separators), quotes and a backslash escape.
    pub fn to_string_format(&self, format: Str) -> Str {
        let f = format.get().unwrap_or("G");
        let c = current_culture();
        let pattern = match f {
            "D" => c.long_date,
            "d" => c.short_date,
            _ if f.chars().count() == 1 => return host::unimplemented_value("System.DateTime.ToString(string) with a standard format other than D / d"),
            _ => f,
        };
        Str::new(&self.format_custom(pattern, c))
    }
    /// `ToString()`: the general format "G" of the current culture, short date + long time (en-US: `10/11/2026 3:04:05 PM`).
    pub fn to_string_general(&self) -> Str {
        let c = current_culture();
        Str::new(&self.format_custom(&format!("{} {}", c.short_date, long_time_pattern(c.name)), c))
    }
    /// `ToString(format, CultureInfo.InvariantCulture)` (the game's file-name formats).
    pub fn to_string_invariant(&self, format: Str) -> Str {
        let f = format.get().unwrap_or("G");
        let c = &CULTURES[0];
        let pattern = match f {
            "D" => c.long_date,
            "d" => c.short_date,
            _ if f.chars().count() == 1 => return host::unimplemented_value("System.DateTime.ToString(string, IFormatProvider) with a standard format other than D / d"),
            _ => f,
        };
        Str::new(&self.format_custom(pattern, c))
    }
    /// `ToShortDateString()` (= `ToString("d")`)
    pub fn to_short_date_string(&self) -> Str {
        Str::new(&self.format_custom(current_culture().short_date, current_culture()))
    }
    /// `ToLongDateString()` (= `ToString("D")`)
    pub fn to_long_date_string(&self) -> Str {
        Str::new(&self.format_custom(current_culture().long_date, current_culture()))
    }

    fn format_custom(&self, pattern: &str, cul: &Culture) -> String {
        let (year, month, day, _) = self.date();
        let c: Vec<char> = pattern.chars().collect();
        let mut out = String::new();
        let mut i = 0;
        while i < c.len() {
            let ch = c[i];
            let mut n = 1;
            while i + n < c.len() && c[i + n] == ch {
                n += 1;
            }
            match ch {
                'y' => match n {
                    1 => out.push_str(&(year % 100).to_string()),
                    2 => out.push_str(&format!("{:02}", year % 100)),
                    _ => out.push_str(&format!("{:0width$}", year, width = n)),
                },
                'M' => match n {
                    1 => out.push_str(&month.to_string()),
                    2 => out.push_str(&format!("{month:02}")),
                    3 => out.push_str(cul.month_abbr[month as usize - 1]),
                    _ => out.push_str(if use_genitive(&c, i, n) { cul.month_genitive[month as usize - 1] } else { cul.month_names[month as usize - 1] }),
                },
                'd' => match n {
                    1 => out.push_str(&day.to_string()),
                    2 => out.push_str(&format!("{day:02}")),
                    3 => out.push_str(cul.day_abbr[self.day_of_week() as usize]),
                    _ => out.push_str(cul.day_names[self.day_of_week() as usize]),
                },
                'H' => out.push_str(&if n == 1 { self.hour().to_string() } else { format!("{:02}", self.hour()) }),
                'h' => {
                    let h = if self.hour() % 12 == 0 { 12 } else { self.hour() % 12 };
                    out.push_str(&if n == 1 { h.to_string() } else { format!("{h:02}") });
                }
                'm' => out.push_str(&if n == 1 { self.minute().to_string() } else { format!("{:02}", self.minute()) }),
                's' => out.push_str(&if n == 1 { self.second().to_string() } else { format!("{:02}", self.second()) }),
                'f' if n <= 7 => {
                    let frac = format!("{:07}", self.ticks() % 10_000_000);
                    out.push_str(&frac[..n]);
                }
                't' => {
                    let d = if self.hour() < 12 { cul.am } else { cul.pm };
                    if n == 1 {
                        out.extend(d.chars().next());
                    } else {
                        out.push_str(d);
                    }
                }
                '/' => {
                    out.push_str(cul.date_separator);
                    i += 1;
                    continue;
                }
                ':' => {
                    out.push_str(cul.time_separator);
                    i += 1;
                    continue;
                }
                '\'' | '"' => {
                    let mut j = i + 1;
                    while j < c.len() && c[j] != ch {
                        out.push(c[j]);
                        j += 1;
                    }
                    i = j + 1;
                    continue;
                }
                '\\' => {
                    if i + 1 < c.len() {
                        out.push(c[i + 1]);
                    }
                    i += 2;
                    continue;
                }
                'g' | 'K' | 'z' | 'F' | '%' => {
                    return host::unimplemented_value("System.DateTime.ToString(string): custom format specifier g / K / z / F / %");
                }
                _ => {
                    for _ in 0..n {
                        out.push(ch);
                    }
                }
            }
            i += n;
        }
        out
    }
}

/// .NET's `IsUseGenitiveForm`: the genitive month name is used when a `d` or `dd` (not `ddd` / `dddd`) appears before
/// or after the month pattern, anywhere in the format.
fn use_genitive(f: &[char], index: usize, token_len: usize) -> bool {
    let mut i = index as isize - 1;
    while i >= 0 && f[i as usize] != 'd' {
        i -= 1;
    }
    if i >= 0 {
        let mut repeat = 0;
        i -= 1;
        while i >= 0 && f[i as usize] == 'd' {
            repeat += 1;
            i -= 1;
        }
        if repeat <= 1 {
            return true;
        }
    }
    let mut i = index + token_len;
    while i < f.len() && f[i] != 'd' {
        i += 1;
    }
    if i < f.len() {
        let mut repeat = 0;
        i += 1;
        while i < f.len() && f[i] == 'd' {
            repeat += 1;
            i += 1;
        }
        if repeat <= 1 {
            return true;
        }
    }
    false
}

/// `Thread.CurrentThread.CurrentCulture` as far as date formatting goes: the runtime holds the thread's culture
/// (`rt::sys::CultureInfo::set_current`, set by `LanguageManager.SetLanguage`); its name selects the data here. A
/// culture this crate has no data for formats like en-US (the runtime's own default).
pub fn current_culture() -> &'static Culture {
    let name = terraria_gen_rt::rt::sys::CultureInfo::current();
    // SAFETY: a live runtime object (rooted by the runtime)
    let n = unsafe { (*name.ptr()).name };
    culture_by_name(n.get().unwrap_or("en-US"))
}
/// The data for a culture name; en-US when unknown.
pub fn culture_by_name(name: &str) -> &'static Culture {
    CULTURES.iter().find(|c| c.name.eq_ignore_ascii_case(name)).unwrap_or(&CULTURES[1])
}

// ------------------------------------------------------------------------------------------------ Stopwatch

fn process_start() -> Instant {
    static START: OnceLock<Instant> = OnceLock::new();
    *START.get_or_init(Instant::now)
}

/// `Stopwatch.Frequency`: timestamps are in 100 ns units (`IsHighResolution` true), so 10,000,000 per second.
pub const STOPWATCH_FREQUENCY: i64 = 10_000_000;

thread_local! {
    static FIXED_TIMESTAMP: std::cell::Cell<Option<i64>> = const { std::cell::Cell::new(None) };
}

/// Driver (repeatable scenarios: `tests/draw.rs`, whose reference pins the original the same way): the stopwatch clock
/// of this thread stands still at `t` (`Stopwatch.GetTimestamp`, every `Stopwatch`): budgets the game measures in real
/// time (`Main.DoDraw` draws map sections for 5 ms per frame) then never run out. `None`: the clock.
pub fn set_fixed_timestamp(t: Option<i64>) {
    FIXED_TIMESTAMP.set(t);
}

/// `Stopwatch.GetTimestamp()`: 100 ns units since the process started (the origin is arbitrary in .NET too).
pub fn stopwatch_get_timestamp() -> i64 {
    if let Some(t) = FIXED_TIMESTAMP.get() {
        return t;
    }
    (process_start().elapsed().as_nanos() / 100) as i64
}

thread_local! {
    static FIXED_NOW: std::cell::Cell<Option<i64>> = const { std::cell::Cell::new(None) };
}

/// Driver (repeatable scenarios: `tests/draw.rs`, whose reference pins the original the same way): `DateTime.Now` and
/// `DateTime.UtcNow` on this thread answer this many ticks (local and UTC alike). `None`: the clock.
pub fn set_fixed_now(ticks: Option<i64>) {
    FIXED_NOW.set(ticks);
}

thread_local! {
    static FIXED_TICK_COUNT: std::cell::Cell<Option<i32>> = const { std::cell::Cell::new(None) };
}

/// Driver (repeatable scenarios: `tests/draw.rs`, whose reference pins the original the same way): every
/// `Environment.TickCount` on this thread answers `v` (the seed of every `new UnifiedRandom()`). `None`: the clock.
pub fn set_fixed_tick_count(v: Option<i32>) {
    FIXED_TICK_COUNT.set(v);
}

/// `Environment.TickCount`: milliseconds since the process started (the original counts from system start; the
/// game uses it as a seed and for differences), wrapping like the `int` it is.
pub fn environment_tick_count() -> i32 {
    if let Some(v) = FIXED_TICK_COUNT.get() {
        return v;
    }
    process_start().elapsed().as_millis() as u32 as i32
}

/// `System.Diagnostics.Stopwatch`
pub struct Stopwatch {
    elapsed: i64,
    started_at: i64,
    running: bool,
}
plain_trace!(Stopwatch);

impl Stopwatch {
    /// `new Stopwatch()`
    pub fn new_gc() -> Gc<Stopwatch> {
        rt::alloc_init(Stopwatch { elapsed: 0, started_at: 0, running: false })
    }
    /// `Stopwatch.StartNew()`
    pub fn start_new() -> Gc<Stopwatch> {
        let s = Self::new_gc();
        // SAFETY: fresh object.
        unsafe { (*s.ptr()).start() };
        s
    }
    pub fn start(&mut self) {
        if !self.running {
            self.started_at = stopwatch_get_timestamp();
            self.running = true;
        }
    }
    pub fn stop(&mut self) {
        if self.running {
            self.elapsed += (stopwatch_get_timestamp() - self.started_at).max(0);
            self.running = false;
        }
    }
    pub fn reset(&mut self) {
        self.elapsed = 0;
        self.running = false;
        self.started_at = 0;
    }
    pub fn restart(&mut self) {
        self.elapsed = 0;
        self.started_at = stopwatch_get_timestamp();
        self.running = true;
    }
    pub fn is_running(&self) -> bool {
        self.running
    }
    pub fn elapsed_ticks(&self) -> i64 {
        self.elapsed + if self.running { stopwatch_get_timestamp() - self.started_at } else { 0 }
    }
    pub fn elapsed(&self) -> TimeSpan {
        TimeSpan::new(self.elapsed_ticks())
    }
    pub fn elapsed_milliseconds(&self) -> i64 {
        self.elapsed_ticks() / 10_000
    }
}

/// `EqualityComparer<DateTime>.Default`: `DateTime.Equals(DateTime)` compares the ticks and ignores the kind;
/// `GetHashCode()` is `(int)ticks ^ (int)(ticks >> 32)` (reference source, DateTime.cs). Needed for a `DateTime`
/// as a dictionary key or inside a struct with default (field-wise) equality.
impl rt::CsKey for DateTime {
    fn cs_eq(&self, o: &Self) -> bool {
        self.ticks() == o.ticks()
    }
    fn cs_hash<H: std::hash::Hasher>(&self, h: &mut H) {
        let t = self.ticks();
        h.write_i32((t as i32) ^ ((t >> 32) as i32));
    }
}
/// `Comparer<DateTime>.Default` (`OrderBy(x => x.Date)`, `List<DateTime>.Sort()`): `CompareTo`, by ticks.
impl rt::sys::CsOrd for DateTime {
    fn cs_cmp(&self, o: &Self) -> i32 {
        self.compare_to(*o)
    }
}

#[cfg(test)]
mod general_format_tests {
    use super::*;

    /// Expected text: `(New-Object DateTime 2026,10,11,15,4,5).ToString('G', en-US)` on the .NET Framework.
    #[test]
    fn to_string_is_the_general_format_and_boxes() {
        let d = DateTime::from_parts(2026, 10, 11, 15, 4, 5, 0, KIND_UNSPECIFIED);
        assert_eq!(d.to_string_general().get().unwrap(), "10/11/2026 3:04:05 PM");
        let o = rt::Trace::to_obj(&d);
        assert_eq!(rt::obj_to_string(o).get().unwrap(), "10/11/2026 3:04:05 PM");
        let back: DateTime = unsafe { rt::Trace::from_obj(o) };
        assert_eq!(back, d);
    }
}

#[cfg(test)]
mod key_tests {
    use super::*;
    use rt::sys::CsOrd;
    use rt::CsKey;

    #[test]
    fn equality_and_order_are_by_ticks() {
        let utc = DateTime::from_parts(2024, 2, 29, 13, 5, 9, 123, KIND_UTC);
        let unspecified = DateTime::from_parts(2024, 2, 29, 13, 5, 9, 123, KIND_UNSPECIFIED);
        let later = utc.add_seconds(1.0);
        assert!(utc.cs_eq(&unspecified) && !utc.cs_eq(&later));
        assert_eq!(utc.cs_hash_code(), unspecified.cs_hash_code());
        // ticks 638448087091230000 = 0x08DC3924_E70B5D30: low ^ high
        assert_eq!(utc.cs_hash_code(), (638448087091230000i64 as i32) ^ ((638448087091230000i64 >> 32) as i32));
        assert_eq!((utc.cs_cmp(&later), later.cs_cmp(&utc), utc.cs_cmp(&unspecified)), (-1, 1, 0));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// tests/dotnet/system_probe.out, section "DateTime" (real .NET Framework).
    #[test]
    fn date_time_matches_dotnet() {
        let d = DateTime::from_parts(2024, 2, 29, 13, 5, 9, 123, KIND_UTC);
        assert_eq!(d.ticks(), 638448087091230000);
        assert_eq!(d.to_binary(), 5250134105518617904);
        let u = DateTime::from_parts(2024, 2, 29, 13, 5, 9, 123, KIND_UNSPECIFIED);
        assert_eq!(u.to_binary(), 638448087091230000);
        let back = DateTime::from_binary(5250134105518617904);
        assert_eq!((back.ticks(), back.kind()), (638448087091230000, KIND_UTC));
        assert_eq!((d.year(), d.month(), d.day(), d.hour(), d.minute(), d.second(), d.millisecond(), d.day_of_week(), d.day_of_year()), (2024, 2, 29, 13, 5, 9, 123, 4, 60));
        assert_eq!(d.add_seconds(86400.5).ticks(), 638448951096230000);
        assert_eq!(d.add_seconds(-0.0004).ticks(), 638448087091230000);
        assert_eq!(d.add_days(366.0).ticks(), 638764311091230000);
        assert_eq!(d.subtract(DateTime::from_parts(1999, 12, 31, 23, 59, 59, 0, 0)).ticks, 7625271101230000);
        assert_eq!(d.to_string_format(Str::lit("D")).as_str(), "Thursday, February 29, 2024");
        assert_eq!(d.to_string_format(Str::lit("MMMM d, yyy")).as_str(), "February 29, 2024");
        assert_eq!(d.to_string_format(Str::lit("yyyyMMddHHmmssfff")).as_str(), "20240229130509123");
        assert_eq!(DateTime::from_ymd(1970, 1, 1).ticks(), UNIX_EPOCH_TICKS);
        assert_eq!(DateTime::from_ymd(1601, 1, 1).ticks(), FILE_TIME_EPOCH_TICKS);
        for (ticks, y, m, day, dow) in [(0i64, 1, 1, 1, 1), (599317056000000000, 1900, 3, 1, 4), (631138176000000000, 2000, 12, 31, 0), (662431392000000000, 2100, 3, 1, 1), (3155378975990000000, 9999, 12, 31, 5)] {
            let x = DateTime::from_ticks(ticks);
            assert_eq!((x.year(), x.month(), x.day(), x.day_of_week()), (y, m, day, dow), "{ticks}");
            assert_eq!(DateTime::from_parts(y, m, day, x.hour(), x.minute(), x.second(), 0, 0).ticks(), ticks);
        }
        let e = rt::try_(|| DateTime::from_ymd(2023, 2, 29)).unwrap_err();
        assert!(rt::exc_is(e, "System.ArgumentOutOfRangeException"));
    }

    #[test]
    fn now_is_local_and_round_trips_through_binary() {
        let (utc, now) = (DateTime::utc_now(), DateTime::now());
        assert_eq!((utc.kind(), now.kind()), (KIND_UTC, KIND_LOCAL));
        let off = local_offset_ticks();
        assert!(off % 600_000_000 == 0 && off.abs() <= 14 * 36_000_000_000);
        assert!((now.ticks() - utc.ticks() - off).abs() < 10_000_000);
        let b = DateTime::from_binary(now.to_binary());
        assert_eq!((b.ticks(), b.kind()), (now.ticks(), KIND_LOCAL));
        assert_eq!(now.to_universal_time().ticks(), now.ticks() - off);
        assert!(utc.year() >= 2025);
    }

    #[test]
    fn stopwatch_state_machine() {
        let s = Stopwatch::new_gc();
        let s = unsafe { &mut *s.ptr() };
        assert!(!s.is_running() && s.elapsed_ticks() == 0);
        s.start();
        std::thread::sleep(std::time::Duration::from_millis(20));
        s.stop();
        let a = s.elapsed_ticks();
        assert!(a >= 150_000 && !s.is_running(), "{a}");
        std::thread::sleep(std::time::Duration::from_millis(5));
        assert_eq!(s.elapsed_ticks(), a, "stopped: time does not advance");
        s.start();
        std::thread::sleep(std::time::Duration::from_millis(5));
        assert!(s.elapsed_ticks() > a && s.elapsed().ticks >= a && s.elapsed_milliseconds() >= 20);
        s.restart();
        assert!(s.is_running() && s.elapsed_ticks() < a);
        s.reset();
        assert!(!s.is_running() && s.elapsed_ticks() == 0);
        assert!(stopwatch_get_timestamp() > 0 && environment_tick_count() >= 0);
    }
}

#[cfg(test)]
mod culture_tests;
