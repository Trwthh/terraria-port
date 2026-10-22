//! `fsin`, `fcos`, `fptan`, `fpatan` as the processor's microcode defines them, up to its last-bit error (see the crate documentation):
//!
//! * argument reduction with the 66-bit approximation of pi built into the instructions, `p66 = 0xC90FDAA22168C234C * 2^-66`:
//!   `r = x - k * p66/2` computed EXACTLY (k = nearest integer), then the true sine / cosine of `r` in the quadrant `k mod 4`;
//! * `|x| >= 2^63`: the operand is out of range and is returned unchanged (`fptan`: the caller's `fstp st(0)` then leaves an empty stack,
//!   and reading it gives the indefinite NaN);
//! * the result is the correctly rounded 64-bit significand (this is where the model and the microcode can differ in the last bit of the
//!   register, in about 1% of the calls; the difference survives the rounding to double in about 1 call in 100,000).

use crate::big::{self, Big, Fx};
use crate::consts::{COS_C, SIN_C};
use crate::f80::{Kind, F80};

/// p66 / 2 scaled by 2^65 (66 bits, odd)
const PH: u128 = 0x3_243F_6A88_85A3_08D3;

/// floor(a * b / 2^126) for Q2.126 operands below 2^127
#[inline(always)]
fn mulq(a: u128, b: u128) -> u128 {
    let (h, l) = mul_full(a, b);
    (h << 2) | (l >> 126)
}

/// full 256-bit product as (high, low)
#[inline(always)]
fn mul_full(a: u128, b: u128) -> (u128, u128) {
    const M: u128 = 0xffff_ffff_ffff_ffff;
    let (a1, a0) = (a >> 64, a & M);
    let (b1, b0) = (b >> 64, b & M);
    let p00 = a0 * b0;
    let p01 = a0 * b1;
    let p10 = a1 * b0;
    let p11 = a1 * b1;
    let (mid, c1) = p01.overflowing_add(p10);
    let (mid, c2) = mid.overflowing_add(p00 >> 64);
    let h = p11 + (mid >> 64) + ((c1 as u128 + c2 as u128) << 64);
    let l = (mid << 64) | (p00 & M);
    (h, l)
}

/// Rounds the positive 256-bit integer (h, l) * 2^lsb_exp to a 64-bit significand. Returns None when the value is within `2^-54` of a
/// unit in the last place of a rounding boundary (the caller's value carries an error below 2^-58 ulp, so the decision would not be safe).
#[inline(always)]
fn round256(neg: bool, h: u128, l: u128, lsb_exp: i32) -> Option<F80> {
    let lz = if h != 0 { h.leading_zeros() } else { 128 + l.leading_zeros() };
    // normalise: top bit at bit 255
    let (nh, nl) = if lz == 0 {
        (h, l)
    } else if lz < 128 {
        ((h << lz) | (l >> (128 - lz)), l << lz)
    } else {
        (l << (lz - 128), 0)
    };
    let man = (nh >> 64) as u64;
    let rem = ((nh & 0xffff_ffff_ffff_ffff) << 64) | (nl >> 64); // top 128 bits of the 192 discarded bits
    const HALF: u128 = 1 << 127;
    let dist = if rem >= HALF { rem - HALF } else { HALF - rem };
    if dist < (1u128 << 74) {
        return None;
    }
    let exp = lsb_exp + 255 - lz as i32;
    if rem > HALF {
        let (m, carry) = man.overflowing_add(1);
        if carry {
            return Some(F80 { kind: Kind::Finite, neg, exp: exp + 1, man: 1 << 63 });
        }
        return Some(F80 { kind: Kind::Finite, neg, exp, man: m });
    }
    Some(F80 { kind: Kind::Finite, neg, exp, man })
}

/// The reduced argument: `|x| = k * p66/2 + r`, `|r| = m * 2^e` exactly, `|r| <= p66/4`.
#[derive(Clone, Copy)]
struct Red {
    quad: u8,
    rneg: bool,
    m: u128,
    e: i32,
}

#[inline(always)]
fn reduce(ax: f64) -> Red {
    let (mx, ex) = big::split(ax);
    if ax < 0.75 {
        return Red { quad: 0, rneg: false, m: mx as u128, e: ex };
    }
    // X = |x| * 2^65 < 2^128, exact (ex >= -53)
    let x65 = (mx as u128) << (ex + 65);
    let (k, rem) = if ax < 1.0e15 {
        // k from a double estimate (off by at most one), remainder exactly in 128-bit integers
        let mut k = (ax * std::f64::consts::FRAC_2_PI) as u128;
        let mut prod = k * PH;
        if prod > x65 {
            k -= 1;
            prod -= PH;
        }
        let mut rem = x65 - prod;
        if rem >= PH {
            k += 1;
            rem -= PH;
        }
        (k, rem)
    } else {
        (x65 / PH, x65 % PH)
    };
    if rem * 2 > PH {
        Red { quad: ((k + 1) & 3) as u8, rneg: true, m: PH - rem, e: -65 }
    } else {
        Red { quad: (k & 3) as u8, rneg: false, m: rem, e: -65 }
    }
}

/// floor(log2 |r|)
#[inline(always)]
fn top_exp(r: &Red) -> i32 {
    r.e + 127 - r.m.leading_zeros() as i32
}

#[inline(always)]
fn series(c: &[u128; 20], z: u128) -> u128 {
    // number of terms from the size of z = r^2 (Q2.126): the first omitted term must be far below 2^-126
    let zb = 128 - z.leading_zeros() as i32 - 126; // z < 2^zb
    let n = if zb <= -40 {
        3
    } else if zb <= -20 {
        5
    } else if zb <= -8 {
        9
    } else if zb <= -3 {
        14
    } else {
        19
    };
    let mut t = c[n];
    let mut i = n;
    while i > 0 {
        i -= 1;
        t = c[i] - mulq(z, t);
    }
    t
}

/// |sin r| as a correctly rounded 64-bit significand
fn sin_r(r: &Red, neg: bool) -> F80 {
    let te = top_exp(r);
    if te < -33 {
        // sin r = r (1 - r^2/6 + ...): the correction is below half a unit of the 64-bit significand, and r itself is representable
        return F80::from_u128(neg, r.m, r.e, false, 64);
    }
    let rfx = r.m << (r.e + 126); // Q2.126, exact: e >= -97 here
    let z = mulq(rfx, rfx);
    let a = series(&SIN_C, z);
    let (h, l) = mul_full(a, r.m);
    if let Some(v) = round256(neg, h, l, r.e - 126) {
        return v;
    }
    sin_r_slow(r, neg)
}

/// cos r (always positive, r is at most pi/4)
fn cos_r(r: &Red, neg: bool) -> F80 {
    let te = top_exp(r);
    if te < -33 {
        return F80 { kind: Kind::Finite, neg, exp: 0, man: 1 << 63 };
    }
    let rfx = r.m << (r.e + 126);
    let z = mulq(rfx, rfx);
    let b = series(&COS_C, z);
    if let Some(v) = round256(neg, 0, b, -126) {
        return v;
    }
    cos_r_slow(r, neg)
}

fn fx_of(r: &Red) -> (Fx, Fx, i32) {
    // r as a plain fixed-point number and normalised to [1, 2) with its exponent
    let n = 128 - r.m.leading_zeros();
    let base = Fx([r.m as u64, (r.m >> 64) as u64, 0, 0, 0]);
    let norm = base.shl(257 - n);
    let exp = r.e + n as i32 - 1;
    let plain = norm.shr((-exp).clamp(0, 319) as u32);
    (plain, norm, exp)
}

#[cold]
fn sin_r_slow(r: &Red, neg: bool) -> F80 {
    let (plain, norm, exp) = fx_of(r);
    let (s_over_r, _) = big::sin_cos_series(plain);
    Big { fx: norm.mul(s_over_r), exp }.to_f80(neg)
}

#[cold]
fn cos_r_slow(r: &Red, neg: bool) -> F80 {
    let (plain, _, _) = fx_of(r);
    let (_, c) = big::sin_cos_series(plain);
    Big { fx: c, exp: 0 }.to_f80(neg)
}

/// Common handling of the operands that never reach the series. `Err(x)` carries the finite in-range magnitude.
#[inline(always)]
fn classify(x: f64) -> Result<F80, f64> {
    let ax = x.abs();
    if ax < 9223372036854775808.0 {
        return Err(ax);
    }
    if x.is_nan() {
        return Ok(F80::from_f64(x)); // quieted
    }
    if ax == f64::INFINITY {
        return Ok(F80::indefinite());
    }
    Ok(F80::from_f64(x)) // out of range: C2 set, operand unchanged
}

/// `fsin`
pub fn fsin(x: f64) -> F80 {
    let ax = match classify(x) {
        Ok(v) => return v,
        Err(ax) => ax,
    };
    if ax == 0.0 {
        return F80::zero(x.is_sign_negative());
    }
    let r = reduce(ax);
    let xneg = x < 0.0;
    match r.quad {
        0 => sin_r(&r, xneg ^ r.rneg),
        1 => cos_r(&r, xneg),
        2 => sin_r(&r, !(xneg ^ r.rneg)),
        _ => cos_r(&r, !xneg),
    }
}

/// `fcos`
pub fn fcos(x: f64) -> F80 {
    let ax = match classify(x) {
        Ok(v) => return v,
        Err(ax) => ax,
    };
    if ax == 0.0 {
        return F80 { kind: Kind::Finite, neg: false, exp: 0, man: 1 << 63 };
    }
    let r = reduce(ax);
    match r.quad {
        0 => cos_r(&r, false),
        1 => sin_r(&r, !r.rneg),
        2 => cos_r(&r, true),
        _ => sin_r(&r, r.rneg),
    }
}

/// `fptan` followed by the `fstp st(0)` that removes the 1.0 the instruction pushes (what `Math.Tan` executes)
pub fn fptan(x: f64) -> F80 {
    let ax = match classify(x) {
        Ok(v) => {
            // out of range: nothing was pushed, the pop removes the operand itself and the caller reads an empty register
            return if v.kind == Kind::Nan { v } else { F80::indefinite() };
        }
        Err(ax) => ax,
    };
    if ax == 0.0 {
        return F80::zero(x.is_sign_negative());
    }
    let r = reduce(ax);
    let xneg = x < 0.0;
    if r.quad & 1 == 0 && top_exp(&r) < -33 {
        // tan r = r (1 + r^2/3 + ...), correction below half a unit of the 64-bit significand
        return F80::from_u128(xneg ^ r.rneg, r.m, r.e, false, 64);
    }
    let (plain, norm, exp) = fx_of(&r);
    let (s_over_r, c) = big::sin_cos_series(plain);
    let s = norm.mul(s_over_r); // sin r = s * 2^exp, s in [0.84, 2)
    if r.quad & 1 == 0 {
        Big { fx: s.mul(c.recip()), exp }.to_f80(xneg ^ r.rneg)
    } else {
        // tan(r + pi/2) = -cos r / sin r
        Big { fx: c.mul(s.recip()), exp: -exp }.to_f80(!(xneg ^ r.rneg))
    }
}

/// `fpatan`: atan2(y, x) with the true pi, every special case as the instruction defines it
pub fn fpatan(y: f64, x: f64) -> F80 {
    if y.is_nan() {
        return F80::from_f64(y);
    }
    if x.is_nan() {
        return F80::from_f64(x);
    }
    let yneg = y.is_sign_negative();
    let xneg = x.is_sign_negative();
    let pi_f = |num: u64, den: u64, neg: bool| Big { fx: big::pi().mul_u64(num).div_u64(den), exp: 0 }.to_f80(neg);
    let (ay, ax) = (y.abs(), x.abs());
    if ay == f64::INFINITY {
        return if ax == f64::INFINITY { pi_f(if xneg { 3 } else { 1 }, 4, yneg) } else { pi_f(1, 2, yneg) };
    }
    if ax == f64::INFINITY || ay == 0.0 {
        // y finite with x infinite, or y zero (x zero included): 0 for x positive, pi for x negative
        return if xneg { pi_f(1, 1, yneg) } else { F80::zero(yneg) };
    }
    if ax == 0.0 {
        return pi_f(1, 2, yneg);
    }
    let a = big::atan_ratio(ay, ax); // atan(|y| / |x|) in (0, pi/2)
    if !xneg {
        return a.to_f80(yneg);
    }
    let plain = a.fx.shr((-a.exp).clamp(0, 319) as u32);
    Big { fx: big::pi().sub(plain), exp: 0 }.to_f80(yneg)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signatures() {
        // the three values recorded in refdata/worldgen/x87/x87_env.json
        assert_eq!(fsin(std::f64::consts::PI).to_f64(), 1.2246063538223773e-16);
        assert_eq!(fsin(1e22).to_f64(), 1e22);
        assert_eq!(fcos(std::f64::consts::FRAC_PI_2).to_f64(), 6.1230317691118863e-17);
    }

    #[test]
    fn fast_path_agrees_with_big_arithmetic() {
        let mut s = 0x1234_5678_9abc_def1u64;
        for i in 0..20000 {
            s ^= s >> 12;
            s ^= s << 25;
            s ^= s >> 27;
            let u = (s.wrapping_mul(0x2545F4914F6CDD1D) >> 11) as f64 / 9007199254740992.0;
            let x = match i % 4 {
                0 => u * 20.0 - 10.0,
                1 => u * 2000.0,
                2 => (u * 120.0 - 60.0).exp2(),
                _ => (u * 1e15).floor() * 1.5707963267948966,
            };
            let ax = x.abs();
            if ax == 0.0 || ax >= 9.2e18 {
                continue;
            }
            let r = reduce(ax);
            assert_eq!(sin_r(&r, false), sin_r_slow(&r, false), "sin {x}");
            assert_eq!(cos_r(&r, false), cos_r_slow(&r, false), "cos {x}");
        }
    }
}
