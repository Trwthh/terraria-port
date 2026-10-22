//! `System.Decimal`: a 96-bit integer, a sign and a power of ten (0..=28) as its divisor.
//!
//! The game uses it in ONE place (`UIAchievementListItem`: the progress numbers of an achievement with a tracker:
//! `(decimal)int`, `(decimal)float`, `== 0m`, `/`, `(int)`, `(float)`), so this is the part of the type those need,
//! with .NET's results: the unit test compares with values printed by the real .NET Framework
//! (`tools/cs2rs/tests/framework/decimal_probe.out`). Addition, multiplication, parsing and formatting with a
//! format string are NOT here; the translator has no template for them, so a use would not compile silently.

use super::{Marker, Str, Trace};

/// `System.Decimal`. All-zero is `0m`.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct Decimal {
    /// low, mid, high 32 bits of the 96-bit magnitude
    lo: u32,
    mid: u32,
    hi: u32,
    /// as `decimal.GetBits()[3]`: bit 31 the sign, bits 16..24 the scale
    flags: u32,
}

// SAFETY: no handles.
unsafe impl Trace for Decimal {
    const NEEDS_TRACE: bool = false;
    const RESCAN: bool = false;
    unsafe fn trace(&self, _m: &mut Marker) {}
}

const MAX96: u128 = (1u128 << 96) - 1;

fn pow10(n: u32) -> u128 {
    10u128.pow(n)
}

impl Decimal {
    pub const ZERO: Decimal = Decimal { lo: 0, mid: 0, hi: 0, flags: 0 };

    fn new(mag: u128, scale: u32, negative: bool) -> Decimal {
        debug_assert!(mag <= MAX96 && scale <= 28);
        Decimal { lo: mag as u32, mid: (mag >> 32) as u32, hi: (mag >> 64) as u32, flags: (scale << 16) | if negative { 0x8000_0000 } else { 0 } }
    }
    fn mag(&self) -> u128 {
        self.lo as u128 | (self.mid as u128) << 32 | (self.hi as u128) << 64
    }
    fn scale(&self) -> u32 {
        (self.flags >> 16) & 0xFF
    }
    fn negative(&self) -> bool {
        self.flags & 0x8000_0000 != 0
    }

    /// A constant, from the four words of `decimal.GetBits`.
    pub const fn from_bits(bits: [i32; 4]) -> Decimal {
        Decimal { lo: bits[0] as u32, mid: bits[1] as u32, hi: bits[2] as u32, flags: bits[3] as u32 }
    }
    /// `decimal.GetBits(d)`.
    pub fn bits(&self) -> [i32; 4] {
        [self.lo as i32, self.mid as i32, self.hi as i32, self.flags as i32]
    }

    pub fn from_i64(v: i64) -> Decimal {
        Decimal::new(v.unsigned_abs() as u128, 0, v < 0)
    }
    pub fn from_u64(v: u64) -> Decimal {
        Decimal::new(v as u128, 0, false)
    }
    pub fn from_i32(v: i32) -> Decimal {
        Decimal::from_i64(v as i64)
    }
    pub fn from_u32(v: u32) -> Decimal {
        Decimal::from_u64(v as u64)
    }
    pub fn from_i16(v: i16) -> Decimal {
        Decimal::from_i64(v as i64)
    }
    pub fn from_u16(v: u16) -> Decimal {
        Decimal::from_u64(v as u64)
    }
    pub fn from_i8(v: i8) -> Decimal {
        Decimal::from_i64(v as i64)
    }
    pub fn from_u8(v: u8) -> Decimal {
        Decimal::from_u64(v as u64)
    }

    /// `(decimal)float`: 7 significant digits, half to even (`VarDecFromR4`). `OverflowException` outside the range.
    pub fn from_f32(v: f32) -> Decimal {
        Decimal::from_real(v as f64, 7)
    }
    /// `(decimal)double`: 15 significant digits, half to even (`VarDecFromR8`).
    pub fn from_f64(v: f64) -> Decimal {
        Decimal::from_real(v, 15)
    }
    fn from_real(v: f64, digits: i32) -> Decimal {
        if v.is_nan() || v.is_infinite() || v.abs() >= 7.9228162514264337593543950335e28 {
            super::throw(super::exc("System.OverflowException", Str::lit("Value was either too large or too small for a Decimal.")));
        }
        let negative = v < 0.0;
        let a = v.abs();
        if a == 0.0 {
            return Decimal::ZERO;
        }
        // the power of ten that leaves `digits` digits before the point
        let mut exp = digits - 1 - a.log10().floor() as i32;
        if exp > 28 + digits {
            return Decimal::ZERO;
        }
        let scaled = |e: i32| if e >= 0 { a * 10f64.powi(e) } else { a / 10f64.powi(-e) };
        let limit = 10f64.powi(digits);
        let mut x = scaled(exp);
        // log10 can be one off at a power of ten
        if x >= limit {
            exp -= 1;
            x = scaled(exp);
        } else if x < limit / 10.0 {
            exp += 1;
            x = scaled(exp);
        }
        if exp > 28 {
            // fewer digits fit below 1e-28: the scale is capped
            x = scaled(28);
            exp = 28;
        }
        // half to even
        let fl = x.floor();
        let frac = x - fl;
        let mut mant = fl as u128;
        if frac > 0.5 || (frac == 0.5 && mant & 1 == 1) {
            mant += 1;
        }
        if mant == 0 {
            return Decimal::ZERO;
        }
        if exp < 0 {
            // an integer larger than the digits kept: zeros follow
            return Decimal::new(mant * pow10((-exp) as u32), 0, negative);
        }
        let mut scale = exp as u32;
        while scale > 0 && mant % 10 == 0 {
            mant /= 10;
            scale -= 1;
        }
        Decimal::new(mant, scale, negative)
    }

    /// `a / b` (`VarDecDiv`): up to 28 decimals, as many as fit into 96 bits, the last one rounded half to even;
    /// trailing zeros of an exact result are dropped down to the scale the operands call for.
    /// `DivideByZeroException` for `b == 0`.
    pub fn div(a: Decimal, b: Decimal) -> Decimal {
        let (am, bm) = (a.mag(), b.mag());
        if bm == 0 {
            super::throw(super::exc("System.DivideByZeroException", Str::lit("Attempted to divide by zero.")));
        }
        let negative = a.negative() != b.negative();
        if am == 0 {
            return Decimal::ZERO;
        }
        // quotient digits one at a time: q * 10^-scale, remainder r < b
        let mut q = am / bm;
        let mut r = am % bm;
        let mut scale: i32 = a.scale() as i32 - b.scale() as i32;
        while r != 0 && scale < 28 {
            // one more digit only if it still fits into 96 bits
            let d = (r * 10) / bm;
            let Some(nq) = q.checked_mul(10).and_then(|v| v.checked_add(d)).filter(|v| *v <= MAX96) else { break };
            q = nq;
            r = (r * 10) % bm;
            scale += 1;
        }
        if r != 0 {
            // the digit that did not fit decides the rounding (half to even; "half" needs the rest to be zero)
            let twice = r * 2;
            if twice > bm || (twice == bm && q & 1 == 1) {
                q += 1;
                if q > MAX96 {
                    // carry out of 96 bits: drop a digit
                    q = q / 10 + u128::from(q % 10 >= 5);
                    scale -= 1;
                }
            }
        }
        // a negative scale: the divisor had more decimals than the dividend; scale the quotient up
        while scale < 0 {
            q = match q.checked_mul(10).filter(|v| *v <= MAX96) {
                Some(v) => v,
                None => super::throw(super::exc("System.OverflowException", Str::lit("Value was either too large or too small for a Decimal."))),
            };
            scale += 1;
        }
        // an exact quotient keeps no more decimals than it needs (but not fewer than the operands' preferred scale)
        let preferred = (a.scale() as i32 - b.scale() as i32).max(0);
        if r == 0 {
            while scale > preferred && q % 10 == 0 {
                q /= 10;
                scale -= 1;
            }
        }
        Decimal::new(q, scale as u32, negative)
    }

    /// `decimal.Truncate(d)` as a whole number.
    fn truncated(&self) -> u128 {
        self.mag() / pow10(self.scale())
    }
    /// `(int)d`: towards zero; `OverflowException` outside `int`.
    pub fn to_i32(&self) -> i32 {
        let t = self.truncated();
        let limit = if self.negative() { 1u128 << 31 } else { (1u128 << 31) - 1 };
        if t > limit {
            super::throw(super::exc("System.OverflowException", Str::lit("Value was either too large or too small for an Int32.")));
        }
        if self.negative() {
            (t as i64).wrapping_neg() as i32
        } else {
            t as i32
        }
    }
    /// `(long)d`.
    pub fn to_i64(&self) -> i64 {
        let t = self.truncated();
        let limit = if self.negative() { 1u128 << 63 } else { (1u128 << 63) - 1 };
        if t > limit {
            super::throw(super::exc("System.OverflowException", Str::lit("Value was either too large or too small for an Int64.")));
        }
        if self.negative() {
            (t as i128).wrapping_neg() as i64
        } else {
            t as i64
        }
    }
    /// `(double)d` (`VarR8FromDec`): the magnitude as a double, divided by the power of ten.
    pub fn to_f64(&self) -> f64 {
        let m = self.mag() as f64;
        let v = m / 10f64.powi(self.scale() as i32);
        if self.negative() {
            -v
        } else {
            v
        }
    }
    /// `(float)d`: through double, as .NET does.
    pub fn to_f32(&self) -> f32 {
        self.to_f64() as f32
    }

    /// Order of the VALUES: -1, 0, 1 (`1.0m` equals `1.00m`).
    pub fn cmp_value(&self, other: &Decimal) -> i32 {
        let (am, bm) = (self.mag(), other.mag());
        let (an, bn) = (self.negative() && am != 0, other.negative() && bm != 0);
        if an != bn {
            return if an { -1 } else { 1 };
        }
        // bring both to the larger scale; 96 bits times 10^28 does not fit u128, so compare in two steps
        let (sa, sb) = (self.scale(), other.scale());
        let ord = if sa == sb {
            am.cmp(&bm)
        } else if sa < sb {
            match am.checked_mul(pow10(sb - sa)) {
                Some(v) => v.cmp(&bm),
                None => core::cmp::Ordering::Greater,
            }
        } else {
            match bm.checked_mul(pow10(sa - sb)) {
                Some(v) => am.cmp(&v),
                None => core::cmp::Ordering::Less,
            }
        };
        let r = match ord {
            core::cmp::Ordering::Less => -1,
            core::cmp::Ordering::Equal => 0,
            core::cmp::Ordering::Greater => 1,
        };
        if an {
            -r
        } else {
            r
        }
    }

    /// `d.ToString()` in the invariant culture: every stored digit, trailing zeros included.
    pub fn to_str(&self) -> Str {
        let digits = self.mag().to_string();
        let scale = self.scale() as usize;
        let mut s = String::new();
        if self.negative() && self.mag() != 0 {
            s.push('-');
        }
        if scale == 0 {
            s.push_str(&digits);
        } else if digits.len() > scale {
            s.push_str(&digits[..digits.len() - scale]);
            s.push('.');
            s.push_str(&digits[digits.len() - scale..]);
        } else {
            s.push_str("0.");
            for _ in 0..scale - digits.len() {
                s.push('0');
            }
            s.push_str(&digits);
        }
        Str::new(&s)
    }
}

impl PartialEq for Decimal {
    fn eq(&self, other: &Decimal) -> bool {
        self.cmp_value(other) == 0
    }
}

#[cfg(test)]
mod tests {
    use super::Decimal;

    /// Against the real .NET Framework: `tools/cs2rs/tests/framework/decimal_probe.out`
    /// (`div a b lo mid hi flags float-bits text`; `f32 float-bits lo mid hi flags truncated-int back-float-bits text`).
    #[test]
    fn decimal_matches_dotnet() {
        let text = include_str!("../../../../../tools/cs2rs/tests/framework/decimal_probe.out");
        let (mut divs, mut floats) = (0, 0);
        for line in text.lines() {
            let w: Vec<&str> = line.split_whitespace().collect();
            let n = |i: usize| w[i].parse::<i64>().unwrap();
            match w[0] {
                "div" => {
                    let q = Decimal::div(Decimal::from_i32(n(1) as i32), Decimal::from_i32(n(2) as i32));
                    assert_eq!(q.bits(), [n(3) as i32, n(4) as i32, n(5) as i32, n(6) as i32], "{line}");
                    assert_eq!(q.to_f32().to_bits(), n(7) as u32, "{line}");
                    assert_eq!(q.to_str().as_str(), w[8], "{line}");
                    divs += 1;
                }
                "f32" => {
                    let f = f32::from_bits(n(1) as u32);
                    let d = Decimal::from_f32(f);
                    assert_eq!(d.bits(), [n(2) as i32, n(3) as i32, n(4) as i32, n(5) as i32], "{line}");
                    if f.abs() < 2.0e9 {
                        assert_eq!(d.to_i32() as i64, n(6), "{line}");
                    }
                    assert_eq!(d.to_f32().to_bits(), n(7) as u32, "{line}");
                    assert_eq!(d.to_str().as_str(), w[8], "{line}");
                    floats += 1;
                }
                other => panic!("decimal_probe.out: {other}"),
            }
        }
        assert!(divs >= 13 && floats >= 18, "{divs} divisions, {floats} conversions");
        // what the achievement list does with them
        let (a, b) = (Decimal::from_i32(3), Decimal::from_i32(7));
        assert!(a != Decimal::ZERO && Decimal::from_i32(0) == Decimal::from_bits([0, 0, 0, 0]));
        assert_eq!((a.to_i32(), b.to_i32()), (3, 7));
        assert_eq!(Decimal::from_bits([10, 0, 0, 0x10000]), Decimal::from_i32(1));
        assert_eq!(Decimal::from_i32(-5).cmp_value(&Decimal::from_i32(2)), -1);
    }
}
