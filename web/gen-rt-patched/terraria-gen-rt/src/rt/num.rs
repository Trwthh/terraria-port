//! Integer division and float-to-integer conversion with C# / x86 .NET Framework behaviour.

use super::exc::{exc, throw};
use crate::rt::Str;

/// Integer types of C#.
pub trait Int: Copy {
    fn cs_div(self, b: Self) -> Self;
    fn cs_rem(self, b: Self) -> Self;
}

#[cold]
#[inline(never)]
fn div_zero() -> ! {
    throw(exc("System.DivideByZeroException", crate::s!("Attempted to divide by zero.")))
}
#[cold]
#[inline(never)]
fn overflow() -> ! {
    throw(exc("System.OverflowException", Str::NULL))
}

macro_rules! int_signed {
    ($($t:ty),*) => {$(
        impl Int for $t {
            #[inline(always)]
            fn cs_div(self, b: Self) -> Self {
                if b == 0 { div_zero() }
                // x86 `idiv` faults on MinValue / -1; the CLR reports it as OverflowException
                if b == -1 && self == <$t>::MIN { overflow() }
                self / b
            }
            #[inline(always)]
            fn cs_rem(self, b: Self) -> Self {
                if b == 0 { div_zero() }
                if b == -1 && self == <$t>::MIN { overflow() }
                self % b
            }
        }
    )*};
}
macro_rules! int_unsigned {
    ($($t:ty),*) => {$(
        impl Int for $t {
            #[inline(always)]
            fn cs_div(self, b: Self) -> Self { if b == 0 { div_zero() } self / b }
            #[inline(always)]
            fn cs_rem(self, b: Self) -> Self { if b == 0 { div_zero() } self % b }
        }
    )*};
}
// C# promotes smaller integers to int before dividing, so only int / long (and uint / ulong) reach here.
int_signed!(i32, i64);
int_unsigned!(u32, u64);

/// `a / b` on integers: truncates toward zero, throws on zero.
#[inline(always)]
pub fn div<T: Int>(a: T, b: T) -> T {
    a.cs_div(b)
}

/// `a % b` on integers: sign of the dividend, throws on zero.
#[inline(always)]
pub fn rem<T: Int>(a: T, b: T) -> T {
    a.cs_rem(b)
}

/// `(int)x` etc. for floating point `x` (a `float` is widened to `f64` first, which is exact).
///
/// C# leaves the out-of-range result unspecified; these reproduce what 32-bit .NET Framework does, because that
/// is what the game ran on: truncation toward zero, and the "integer indefinite" value `0x8000...` for NaN and
/// out-of-range input. Conversions to types narrower than `int` convert to `int` first and then drop bits.
/// `to_u32` / `to_u64` follow the CLR helpers (`(uint)(long)x`, and the 2^63 split for `ulong`); these two are
/// from documentation of the helpers, not verified against the game.
pub mod conv {
    #[inline(always)]
    pub fn to_i32(x: f64) -> i32 {
        if x > -2147483649.0 && x < 2147483648.0 {
            x as i32
        } else {
            i32::MIN
        }
    }
    #[inline(always)]
    pub fn to_i64(x: f64) -> i64 {
        if x >= -9223372036854775808.0 && x < 9223372036854775808.0 {
            x as i64
        } else {
            i64::MIN
        }
    }
    #[inline(always)]
    pub fn to_u8(x: f64) -> u8 {
        to_i32(x) as u8
    }
    #[inline(always)]
    pub fn to_i8(x: f64) -> i8 {
        to_i32(x) as i8
    }
    #[inline(always)]
    pub fn to_i16(x: f64) -> i16 {
        to_i32(x) as i16
    }
    #[inline(always)]
    pub fn to_u16(x: f64) -> u16 {
        to_i32(x) as u16
    }
    #[inline(always)]
    pub fn to_u32(x: f64) -> u32 {
        to_i64(x) as u32
    }
    #[inline(always)]
    pub fn to_u64(x: f64) -> u64 {
        if x < 9223372036854775808.0 {
            to_i64(x) as u64
        } else {
            (to_i64(x - 9223372036854775808.0) as u64).wrapping_add(0x8000_0000_0000_0000)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::conv::*;
    use super::*;
    use crate::rt::try_;

    #[test]
    fn division_truncates_and_keeps_dividend_sign() {
        assert_eq!(div(-7, 2), -3);
        assert_eq!(rem(-7, 2), -1);
        assert_eq!(div(7, -2), -3);
        assert_eq!(rem(7, -2), 1);
        assert_eq!(div(7u32, 2u32), 3);
    }

    #[test]
    fn division_exceptions() {
        let kind = |r: Result<i32, crate::rt::Gc<crate::rt::Exception>>| unsafe { (*r.unwrap_err().ptr()).kind };
        assert_eq!(kind(try_(|| div(1, 0))), "System.DivideByZeroException");
        assert_eq!(kind(try_(|| rem(1, 0))), "System.DivideByZeroException");
        assert_eq!(kind(try_(|| div(i32::MIN, -1))), "System.OverflowException");
    }

    #[test]
    fn float_to_int_like_x86() {
        assert_eq!(to_i32(2.9), 2);
        assert_eq!(to_i32(-2.9), -2);
        assert_eq!(to_i32(f64::NAN), i32::MIN);
        assert_eq!(to_i32(1e10), i32::MIN);
        assert_eq!(to_i32(-1e10), i32::MIN);
        assert_eq!(to_i32(2147483647.5), 2147483647);
        assert_eq!(to_i32(-2147483648.9), i32::MIN);
        assert_eq!(to_u8(300.7), 44); // (byte)(int)300.7
        assert_eq!(to_u8(-1.0), 255);
        assert_eq!(to_i16(70000.0), 4464);
        assert_eq!(to_i64(1e19), i64::MIN);
        assert_eq!(to_u32(-1.0), u32::MAX);
        assert_eq!(to_u64(1.8446744073709552e19 / 2.0), 0x8000_0000_0000_0000);
    }
}
