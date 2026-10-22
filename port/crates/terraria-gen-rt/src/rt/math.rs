//! `System.Math` members with the behaviour C# code relies on (see `port/PORTING.md`).

use super::exc::{exc, throw};

/// `Math.Round(double)`: round half to even.
pub fn round(x: f64) -> f64 {
    x.round_ties_even()
}

/// `Math.Abs(int)`: throws `OverflowException` for `int.MinValue`.
pub fn abs_i32(x: i32) -> i32 {
    if x == i32::MIN {
        throw(exc("System.OverflowException", crate::s!("Negating the minimum value of a twos complement number is invalid.")))
    }
    x.abs()
}

/// `Math.Sign(int)`.
pub fn sign_i32(x: i32) -> i32 {
    x.signum()
}

/// `Math.Max(float, float)` as .NET Framework 4 writes it (`if (val1 > val2) return val1; if (float.IsNaN(val1))
/// return val1; return val2;`): a NaN operand comes back with its own bits (the x87 / SSE "indefinite" 0xFFC00000 of
/// 0/0 stays negative; a canonical NaN would not).
pub fn max_f32(a: f32, b: f32) -> f32 {
    if a > b || a.is_nan() {
        a
    } else {
        b
    }
}

/// `Math.Min(float, float)` as .NET Framework 4 writes it (`if (val1 < val2) return val1; if (float.IsNaN(val1))
/// return val1; return val2;`): a NaN operand comes back with its own bits.
pub fn min_f32(a: f32, b: f32) -> f32 {
    if a < b || a.is_nan() {
        a
    } else {
        b
    }
}

/// `Math.Max(double, double)`, the same rule as [`max_f32`].
pub fn max_f64(a: f64, b: f64) -> f64 {
    if a > b || a.is_nan() {
        a
    } else {
        b
    }
}

/// `Math.Min(double, double)`, the same rule as [`min_f32`].
pub fn min_f64(a: f64, b: f64) -> f64 {
    if a < b || a.is_nan() {
        a
    } else {
        b
    }
}

/// `Math.Sign(double)` / `Math.Sign(float)`: -1, 0 or 1; `ArithmeticException` for NaN.
pub fn sign_f64(x: f64) -> i32 {
    if x.is_nan() {
        throw(exc("System.ArithmeticException", crate::s!("Function does not accept floating point Not-a-Number values.")))
    }
    if x > 0.0 {
        1
    } else if x < 0.0 {
        -1
    } else {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rounding_and_abs() {
        assert_eq!(round(0.5), 0.0);
        assert_eq!(round(1.5), 2.0);
        assert_eq!(round(2.5), 2.0);
        assert_eq!(round(-2.5), -2.0);
        assert_eq!(abs_i32(-3), 3);
        assert!(crate::rt::try_(|| abs_i32(i32::MIN)).is_err());
        assert!(max_f32(1.0, f32::NAN).is_nan());
        assert_eq!(min_f32(1.0, 2.0), 1.0);
        assert_eq!(sign_i32(-9), -1);
        assert_eq!((sign_f64(-0.5), sign_f64(0.0), sign_f64(3.0)), (-1, 0, 1));
        assert!(crate::rt::try_(|| sign_f64(f64::NAN)).is_err());
        assert_eq!(max_f64(1.0, 2.0), 2.0);
        assert!(min_f64(f64::NAN, 2.0).is_nan());
    }
}
