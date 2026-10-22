//! `Microsoft.Xna.Framework.Graphics.PackedVector.HalfVector2`: two 16-bit floats in a `uint`, the form the network
//! protocol uses for vectors (`Utils.WritePackedVector2` / `ReadPackedVector2`, `NetMessage` 106 "poof of smoke").
//!
//! XNA's `HalfTypeHelper.Convert(float)` (reproduced from the decompiled XNA 4.0 algorithm; no XNA binary is
//! available on this machine to run it against): round to nearest even, denormals handled, magnitudes above the
//! largest finite half (65504, rounding included) give 0x7FFF with the sign (a NaN pattern, not infinity), which is
//! why it is written out here instead of using a generic float-to-half routine. The inverse is exact.
//! `tests` compare with an independent IEEE binary16 conversion for every value that does not overflow, and pin the
//! overflow rule.

use crate::gfx::states::plain_trace;
use terraria_core::xna::Vector2;

/// `HalfVector2` (a struct): `PackedValue = half(X) | half(Y) << 16`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct HalfVector2 {
    pub packed_value: u32,
}
plain_trace!(HalfVector2);

/// `HalfTypeHelper.Convert(float)`
pub fn float_to_half(value: f32) -> u16 {
    let num = value.to_bits();
    let num2 = (num & 0x8000_0000) >> 16;
    let mut num3 = num & 0x7FFF_FFFF;
    if num3 > 0x477F_EFFF {
        return (num2 | 0x7FFF) as u16;
    }
    if num3 < 0x3880_0000 {
        let num4 = (num3 & 0x007F_FFFF) | 0x0080_0000;
        let num5 = 113i32 - (num3 >> 23) as i32;
        num3 = if num5 >= 32 { 0 } else { num4 >> num5 };
        return (num2 | ((num3 + 4095 + ((num3 >> 13) & 1)) >> 13)) as u16;
    }
    (num2 | ((num3.wrapping_add(0xC800_0000) + 4095 + ((num3 >> 13) & 1)) >> 13)) as u16
}

/// `HalfTypeHelper.Convert(ushort)`
pub fn half_to_float(value: u16) -> f32 {
    if value == 0 {
        return 0.0;
    }
    if value == 0x8000 {
        return -0.0;
    }
    let mut exp = ((value >> 10) & 0x1F) as i32;
    let mut mant = (value & 0x3FF) as u32;
    let sign = ((value & 0x8000) as u32) << 16;
    if exp == 0x1F {
        // infinity or NaN
        return f32::from_bits(sign | 0x7F80_0000 | (mant << 13));
    }
    if exp == 0 {
        // denormal: normalise
        exp = 1;
        while mant & 0x400 == 0 {
            mant <<= 1;
            exp -= 1;
        }
        mant &= 0x3FF;
    }
    f32::from_bits(sign | (((exp + 112) as u32) << 23) | (mant << 13))
}

impl HalfVector2 {
    /// `new HalfVector2(x, y)`
    pub fn new(x: f32, y: f32) -> HalfVector2 {
        HalfVector2 { packed_value: float_to_half(x) as u32 | (float_to_half(y) as u32) << 16 }
    }
    pub fn packed_value(&self) -> u32 {
        self.packed_value
    }
    pub fn set_packed_value(&mut self, v: u32) {
        self.packed_value = v;
    }
    /// `ToVector2()`
    pub fn to_vector2(&self) -> Vector2 {
        Vector2::new(half_to_float(self.packed_value as u16), half_to_float((self.packed_value >> 16) as u16))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// IEEE 754 binary16, round to nearest even, written independently (by comparing neighbours).
    fn reference_half(f: f32) -> Option<u16> {
        if !f.is_finite() || f.abs() > 65504.0 {
            return None;
        }
        // brute force: the nearest of all 63,488 finite halves, ties to even mantissa
        let mut best = (f64::MAX, 0u16);
        for h in 0..=0xFFFFu16 {
            if (h >> 10) & 0x1F == 0x1F {
                continue;
            }
            let d = (half_to_float(h) as f64 - f as f64).abs();
            if d < best.0 || (d == best.0 && h & 1 == 0 && best.1 & 1 == 1) {
                best = (d, h);
            }
        }
        // +0 and -0 both exist; the sign of the input decides
        Some(if f == 0.0 && f.is_sign_negative() { 0x8000 } else { best.1 })
    }

    #[test]
    fn conversion_matches_ieee_binary16_in_range() {
        let samples = [0.0f32, -0.0, 1.0, -1.0, 0.5, 0.1, 3.14159, 100.25, 1000.1, 65504.0, 65519.9, 6.1e-5, 6.0e-5, 1e-7, 5.96e-8, 2.98e-8, 2.99e-8, -2.5e-6, 123.456, 0.333333];
        for f in samples {
            if let Some(r) = reference_half(f) {
                assert_eq!(float_to_half(f), r, "{f}");
            }
        }
        // a sweep with simple LCG
        let mut x = 12345u32;
        for _ in 0..400 {
            x = x.wrapping_mul(1664525).wrapping_add(1013904223);
            let f = (x as f32 / u32::MAX as f32 - 0.5) * 2000.0;
            assert_eq!(float_to_half(f), reference_half(f).unwrap(), "{f}");
        }
        // half -> float -> half is the identity for all finite halves
        for h in 0..=0xFFFFu16 {
            if (h >> 10) & 0x1F != 0x1F {
                assert_eq!(float_to_half(half_to_float(h)), h, "{h:#x}");
            }
        }
    }

    #[test]
    fn overflow_is_a_nan_pattern_like_xna() {
        assert_eq!(float_to_half(65520.0), 0x7FFF);
        assert_eq!(float_to_half(-1e9), 0xFFFF);
        assert!(half_to_float(float_to_half(f32::INFINITY)).is_nan());
        assert_eq!(float_to_half(65504.0), 0x7BFF);
    }

    #[test]
    fn half_vector2_as_the_game_uses_it() {
        let h = HalfVector2::new(1.5, -2.25);
        assert_eq!(h.packed_value(), 0xC080_3E00);
        let v = h.to_vector2();
        assert_eq!((v.x, v.y), (1.5, -2.25));
        let mut h2 = HalfVector2::default();
        h2.set_packed_value(0xC080_3E00);
        assert_eq!(h2, h);
        // a position like the poof of smoke: precision is that of a half
        let p = HalfVector2::new(1234.567, 89.01).to_vector2();
        assert!((p.x - 1234.567).abs() < 1.0 && (p.y - 89.01).abs() < 0.05);
    }
}
