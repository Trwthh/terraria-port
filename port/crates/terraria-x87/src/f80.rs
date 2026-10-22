//! A value in an x87 register: sign, 15-bit exponent, 64-bit significand. Only what the crate needs: exact conversion from `f64`, the
//! round-to-nearest-even store to `f64` / `f32` (what `fstp qword` / `fstp dword` do), and multiply / add with a chosen significand
//! precision (64 bits = the register width the transcendental instructions deliver; 53 bits = what `fmul`/`fadd` produce under the CLR's
//! precision-control setting).

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Zero,
    Finite,
    Inf,
    Nan,
}

/// `value = (-1)^neg * man * 2^(exp - 63)` with `man` normalised (bit 63 set) when `kind == Finite`. The exponent is not range-limited:
/// the x87 exponent range (15 bits) is never exceeded by anything derived from doubles here.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct F80 {
    pub kind: Kind,
    pub neg: bool,
    pub exp: i32,
    pub man: u64,
}

impl F80 {
    pub const fn zero(neg: bool) -> F80 {
        F80 { kind: Kind::Zero, neg, exp: 0, man: 0 }
    }
    pub const fn inf(neg: bool) -> F80 {
        F80 { kind: Kind::Inf, neg, exp: 0, man: 0 }
    }
    /// The NaN the x87 produces for an invalid operation ("real indefinite"): negative quiet NaN, zero payload.
    pub const fn indefinite() -> F80 {
        F80 { kind: Kind::Nan, neg: true, exp: 0, man: 0xC000_0000_0000_0000 }
    }

    /// Exact (`fld qword`). A signalling NaN is quieted, as the instruction does with the invalid exception masked.
    pub fn from_f64(x: f64) -> F80 {
        let b = x.to_bits();
        let neg = b >> 63 != 0;
        let e = ((b >> 52) & 0x7ff) as i32;
        let f = b & 0x000f_ffff_ffff_ffff;
        if e == 0x7ff {
            if f == 0 {
                return F80::inf(neg);
            }
            return F80 { kind: Kind::Nan, neg, exp: 0, man: 0xC000_0000_0000_0000 | (f << 11) };
        }
        if e == 0 {
            if f == 0 {
                return F80::zero(neg);
            }
            let lz = f.leading_zeros();
            return F80 { kind: Kind::Finite, neg, exp: -1022 - 52 + 63 - lz as i32, man: f << lz };
        }
        F80 { kind: Kind::Finite, neg, exp: e - 1023, man: (f | 1 << 52) << 11 }
    }

    /// Builds a finite value from an unsigned 128-bit integer `v` scaled by `2^lsb_exp`, rounded to `bits` significand bits
    /// (round to nearest, ties to even; `sticky` says that nonzero bits were dropped below `v`).
    pub fn from_u128(neg: bool, v: u128, lsb_exp: i32, sticky: bool, bits: u32) -> F80 {
        if v == 0 {
            return F80::zero(neg);
        }
        let n = 128 - v.leading_zeros(); // bit length
        let mut exp = lsb_exp + n as i32 - 1;
        let man: u64;
        if n <= bits {
            man = (v as u64) << (64 - n);
            // (sticky bits below an exactly representable value cannot round it up in nearest mode)
        } else {
            let s = n - bits;
            let q = (v >> s) as u64;
            let rem = v & ((1u128 << s) - 1);
            let half = 1u128 << (s - 1);
            let up = rem > half || (rem == half && (sticky || q & 1 == 1));
            let q = q as u128 + up as u128;
            if q >> bits != 0 {
                exp += 1;
                man = 1 << 63;
            } else {
                man = (q as u64) << (64 - bits);
            }
        }
        F80 { kind: Kind::Finite, neg, exp, man }
    }

    /// `fmul` with the result rounded to `bits` significand bits (64 or 53).
    pub fn mul(self, o: F80, bits: u32) -> F80 {
        let neg = self.neg ^ o.neg;
        match (self.kind, o.kind) {
            (Kind::Nan, _) => self,
            (_, Kind::Nan) => o,
            (Kind::Inf, Kind::Zero) | (Kind::Zero, Kind::Inf) => F80::indefinite(),
            (Kind::Inf, _) | (_, Kind::Inf) => F80::inf(neg),
            (Kind::Zero, _) | (_, Kind::Zero) => F80::zero(neg),
            _ => F80::from_u128(neg, self.man as u128 * o.man as u128, self.exp + o.exp - 126, false, bits),
        }
    }

    /// `fadd` with the result rounded to `bits` significand bits.
    pub fn add(self, o: F80, bits: u32) -> F80 {
        match (self.kind, o.kind) {
            (Kind::Nan, _) => return self,
            (_, Kind::Nan) => return o,
            (Kind::Inf, Kind::Inf) => return if self.neg == o.neg { self } else { F80::indefinite() },
            (Kind::Inf, _) => return self,
            (_, Kind::Inf) => return o,
            (Kind::Zero, Kind::Zero) => return F80::zero(self.neg && o.neg),
            (Kind::Zero, _) => return F80::from_u128(o.neg, o.man as u128, o.exp - 63, false, bits),
            (_, Kind::Zero) => return F80::from_u128(self.neg, self.man as u128, self.exp - 63, false, bits),
            _ => {}
        }
        let (a, b) = if self.exp >= o.exp { (self, o) } else { (o, self) };
        let d = (a.exp - b.exp) as u32;
        // a.man at bits 126..63 of a u128, b shifted right by d with a sticky bit
        let am = (a.man as u128) << 62;
        let (bm, sticky) = if d >= 126 {
            (0u128, true)
        } else {
            let full = (b.man as u128) << 62;
            (full >> d, full & ((1u128 << d) - 1) != 0)
        };
        let lsb = a.exp - 63 - 62;
        if a.neg == b.neg {
            F80::from_u128(a.neg, am + bm, lsb, sticky, bits)
        } else if am > bm || (am == bm && !sticky) {
            if am == bm {
                return F80::zero(false);
            }
            // am - (bm + fraction): borrow one unit when sticky, the fraction that remains is still "sticky"
            let v = am - bm - sticky as u128;
            F80::from_u128(a.neg, v, lsb, sticky, bits)
        } else {
            let v = bm - am;
            F80::from_u128(b.neg, v, lsb, sticky, bits)
        }
    }

    pub fn neg(mut self) -> F80 {
        self.neg = !self.neg;
        self
    }

    /// `fstp qword`: round to nearest even to binary64 (one rounding from the 64-bit significand, including into the subnormal range).
    pub fn to_f64(self) -> f64 {
        let s = (self.neg as u64) << 63;
        match self.kind {
            Kind::Zero => f64::from_bits(s),
            Kind::Inf => f64::from_bits(s | 0x7ff0_0000_0000_0000),
            Kind::Nan => f64::from_bits(s | 0x7ff8_0000_0000_0000 | ((self.man >> 11) & 0x0007_ffff_ffff_ffff)),
            Kind::Finite => {
                let e = self.exp;
                if e > 1023 {
                    return f64::from_bits(s | 0x7ff0_0000_0000_0000);
                }
                let sh: u32 = if e >= -1022 { 11 } else { (11 + (-1022 - e)).min(65) as u32 };
                let (q, rem, half) = if sh >= 65 {
                    (0u64, 1u128, 2u128)
                } else {
                    let m = self.man as u128;
                    ((m >> sh) as u64, m & ((1u128 << sh) - 1), 1u128 << (sh - 1))
                };
                let q = q + (rem > half || (rem == half && q & 1 == 1)) as u64;
                if e >= -1022 {
                    // q in [2^52, 2^53]; q == 2^53 carries into the exponent, which the addition below does by itself
                    f64::from_bits(s | (((e + 1022) as u64) << 52).wrapping_add(q))
                } else {
                    f64::from_bits(s | q)
                }
            }
        }
    }

    /// `fstp dword`: round to nearest even to binary32, directly from the 64-bit significand (NOT via binary64).
    pub fn to_f32(self) -> f32 {
        let s = (self.neg as u32) << 31;
        match self.kind {
            Kind::Zero => f32::from_bits(s),
            Kind::Inf => f32::from_bits(s | 0x7f80_0000),
            Kind::Nan => f32::from_bits(s | 0x7fc0_0000 | ((self.man >> 40) & 0x003f_ffff) as u32),
            Kind::Finite => {
                let e = self.exp;
                if e > 127 {
                    return f32::from_bits(s | 0x7f80_0000);
                }
                let sh: u32 = if e >= -126 { 40 } else { (40 + (-126 - e)).min(65) as u32 };
                let (q, rem, half) = if sh >= 65 {
                    (0u64, 1u128, 2u128)
                } else {
                    let m = self.man as u128;
                    ((m >> sh) as u64, m & ((1u128 << sh) - 1), 1u128 << (sh - 1))
                };
                let q = (q + (rem > half || (rem == half && q & 1 == 1)) as u64) as u32;
                if e >= -126 {
                    f32::from_bits(s | (((e + 126) as u32) << 23).wrapping_add(q))
                } else {
                    f32::from_bits(s | q)
                }
            }
        }
    }

    /// The ten bytes `fstp tbyte` writes (little endian): for comparing against raw instruction dumps.
    pub fn to_bytes(self) -> [u8; 10] {
        let (se, man): (u16, u64) = match self.kind {
            Kind::Zero => (0, 0),
            Kind::Inf => (0x7fff, 1 << 63),
            Kind::Nan => (0x7fff, self.man),
            Kind::Finite => {
                let e = self.exp + 16383;
                if e <= 0 {
                    // denormal extended: not reachable from double inputs in this crate
                    (0, if 1 - e < 64 { self.man >> (1 - e) } else { 0 })
                } else {
                    (e as u16, self.man)
                }
            }
        };
        let se = se | (self.neg as u16) << 15;
        let mut b = [0u8; 10];
        b[..8].copy_from_slice(&man.to_le_bytes());
        b[8..].copy_from_slice(&se.to_le_bytes());
        b
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_and_ops_match_ieee_when_exact() {
        let vals = [0.0, -0.0, 1.0, -1.5, 0.1, 1e300, -1e-300, 5e-324, 2.2250738585072014e-308, f64::MAX, f64::INFINITY, 3.0e-310];
        for &a in &vals {
            assert_eq!(F80::from_f64(a).to_f64().to_bits(), a.to_bits());
            assert_eq!(F80::from_f64(a).to_f32().to_bits(), (a as f32).to_bits(), "{a}");
            for &b in &vals {
                // rounding to 53 bits and then storing equals the IEEE operation whenever the result is a normal double
                let m = F80::from_f64(a).mul(F80::from_f64(b), 53).to_f64();
                let s = F80::from_f64(a).add(F80::from_f64(b), 53).to_f64();
                let (im, is) = (a * b, a + b);
                if im.is_nan() {
                    assert!(m.is_nan());
                } else if im == 0.0 || im.abs() >= f64::MIN_POSITIVE {
                    assert_eq!(m.to_bits(), im.to_bits(), "{a} * {b}");
                }
                if is.is_nan() {
                    assert!(s.is_nan());
                } else {
                    assert_eq!(s.to_bits(), is.to_bits(), "{a} + {b}");
                }
            }
        }
    }
}
