//! 320-bit unsigned fixed point (64 integer bits, 256 fraction bits) and the elementary functions on it. This is the crate's
//! "exact" arithmetic: every function is accurate to about 2^-245, far below the 2^-64 the x87 register can hold, so the correctly
//! rounded 64-bit significand can be read off. It is slow (microseconds); `trig.rs` has the fast path for sin / cos.

use crate::consts::{ATAN_J8, LN2};
use crate::f80::F80;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) struct Fx(pub [u64; 5]);

impl Fx {
    pub const ZERO: Fx = Fx([0; 5]);
    pub const ONE: Fx = Fx([0, 0, 0, 0, 1]);

    pub fn from_u64(v: u64) -> Fx {
        Fx([0, 0, 0, 0, v])
    }
    pub fn add(self, o: Fx) -> Fx {
        let mut r = [0u64; 5];
        let mut c = 0u128;
        for i in 0..5 {
            let t = self.0[i] as u128 + o.0[i] as u128 + c;
            r[i] = t as u64;
            c = t >> 64;
        }
        Fx(r)
    }
    /// `self - o`, requires `self >= o`
    pub fn sub(self, o: Fx) -> Fx {
        let mut r = [0u64; 5];
        let mut b = 0i128;
        for i in 0..5 {
            let t = self.0[i] as i128 - o.0[i] as i128 - b;
            r[i] = t as u64;
            b = (t < 0) as i128;
        }
        Fx(r)
    }
    pub fn ge(&self, o: &Fx) -> bool {
        for i in (0..5).rev() {
            if self.0[i] != o.0[i] {
                return self.0[i] > o.0[i];
            }
        }
        true
    }
    /// truncated product
    pub fn mul(self, o: Fx) -> Fx {
        let mut p = [0u64; 10];
        for i in 0..5 {
            let mut c = 0u128;
            for j in 0..5 {
                let t = self.0[i] as u128 * o.0[j] as u128 + p[i + j] as u128 + c;
                p[i + j] = t as u64;
                c = t >> 64;
            }
            p[i + 5] = c as u64;
        }
        Fx([p[4], p[5], p[6], p[7], p[8]])
    }
    pub fn mul_u64(self, m: u64) -> Fx {
        let mut r = [0u64; 5];
        let mut c = 0u128;
        for i in 0..5 {
            let t = self.0[i] as u128 * m as u128 + c;
            r[i] = t as u64;
            c = t >> 64;
        }
        Fx(r)
    }
    pub fn div_u64(self, d: u64) -> Fx {
        let mut r = [0u64; 5];
        let mut rem = 0u128;
        for i in (0..5).rev() {
            let t = (rem << 64) | self.0[i] as u128;
            r[i] = (t / d as u128) as u64;
            rem = t % d as u128;
        }
        Fx(r)
    }
    pub fn shr(self, n: u32) -> Fx {
        if n >= 320 {
            return Fx::ZERO;
        }
        let (w, b) = ((n / 64) as usize, n % 64);
        let mut r = [0u64; 5];
        for i in 0..5 - w {
            let lo = self.0[i + w] >> b;
            let hi = if b > 0 && i + w + 1 < 5 { self.0[i + w + 1] << (64 - b) } else { 0 };
            r[i] = lo | hi;
        }
        Fx(r)
    }
    pub fn shl(self, n: u32) -> Fx {
        if n >= 320 {
            return Fx::ZERO;
        }
        let (w, b) = ((n / 64) as usize, n % 64);
        let mut r = [0u64; 5];
        for i in w..5 {
            let lo = self.0[i - w] << b;
            let hi = if b > 0 && i - w >= 1 { self.0[i - w - 1] >> (64 - b) } else { 0 };
            r[i] = lo | hi;
        }
        Fx(r)
    }
    pub fn bit_len(&self) -> u32 {
        for i in (0..5).rev() {
            if self.0[i] != 0 {
                return 64 * i as u32 + 64 - self.0[i].leading_zeros();
            }
        }
        0
    }
    pub fn to_f64(&self) -> f64 {
        let mut v = 0.0f64;
        for i in (0..5).rev() {
            v = v * 18446744073709551616.0 + self.0[i] as f64;
        }
        v / 18446744073709551616.0 / 18446744073709551616.0 / 18446744073709551616.0 / 18446744073709551616.0
    }
    /// `v * 2^e` for a finite positive double with the result inside the fixed-point range (truncated below 2^-256)
    pub fn from_f64(v: f64) -> Fx {
        let b = v.to_bits();
        let e = ((b >> 52) & 0x7ff) as i32;
        let m = if e == 0 { b & 0x000f_ffff_ffff_ffff } else { (b & 0x000f_ffff_ffff_ffff) | 1 << 52 };
        let e = if e == 0 { -1074 } else { e - 1075 };
        let sh = e + 256;
        let x = Fx([m, 0, 0, 0, 0]);
        if sh >= 0 { x.shl(sh as u32) } else { x.shr((-sh) as u32) }
    }
    /// 1 / self for self in [2^-2, 2^8): Newton from a double seed
    pub fn recip(self) -> Fx {
        let mut y = Fx::from_f64(1.0 / self.to_f64());
        let two = Fx::from_u64(2);
        for _ in 0..4 {
            y = y.mul(two.sub(self.mul(y)));
        }
        y
    }
}

/// A positive high-precision number `fx * 2^exp` (keeps relative precision for small results).
#[derive(Clone, Copy, Debug)]
pub(crate) struct Big {
    pub fx: Fx,
    pub exp: i32,
}

impl Big {
    /// Correctly rounded 64-bit significand (round to nearest; the working error of 2^-240 is ignored: a result that close to a rounding
    /// boundary of the 64-bit register does not occur for the transcendental functions here).
    pub fn to_f80(&self, neg: bool) -> F80 {
        let n = self.fx.bit_len();
        if n == 0 {
            return F80::zero(neg);
        }
        // take the top 128 bits into a u128 with a sticky bit
        let (v, sticky, lsb) = if n <= 128 {
            (self.fx.0[0] as u128 | (self.fx.0[1] as u128) << 64, false, -256)
        } else {
            let s = n - 128;
            let t = self.fx.shr(s);
            let back = t.shl(s);
            (t.0[0] as u128 | (t.0[1] as u128) << 64, back != self.fx, -256 + s as i32)
        };
        F80::from_u128(neg, v, lsb + self.exp, sticky, 64)
    }
}

/// sin and cos of `r` (0 <= r < 1), Taylor series
pub(crate) fn sin_cos_series(r: Fx) -> (Fx, Fx) {
    let z = r.mul(r);
    // sin(r)/r = 1 - z/(2*3) (1 - z/(4*5) (1 - ...)),  cos(r) = 1 - z/(1*2) (1 - z/(3*4) (1 - ...))
    let mut s = Fx::ONE;
    let mut c = Fx::ONE;
    let mut n = 36u64;
    while n >= 1 {
        s = Fx::ONE.sub(z.mul(s).div_u64(2 * n * (2 * n + 1)));
        c = Fx::ONE.sub(z.mul(c).div_u64((2 * n - 1) * 2 * n));
        n -= 1;
    }
    (s, c) // note: `s` is sin(r)/r
}

/// atan(t)/t for 0 <= t <= 2^-4 as a series; atan(t) for 0 <= t <= 1 via atan(t) = atan(j/8) + atan((t - j/8)/(1 + t j/8))
fn atan_over_t_small(u: Fx) -> Fx {
    static ODD_RECIP: std::sync::OnceLock<[Fx; 37]> = std::sync::OnceLock::new();
    let recip = ODD_RECIP.get_or_init(|| std::array::from_fn(|n| Fx::ONE.div_u64(2 * n as u64 + 1)));
    let z = u.mul(u);
    // 1 - z/3 + z^2/5 - ...  evaluated from the far end; |z| <= 2^-8
    let mut acc = Fx::ZERO;
    let mut n = 36usize;
    loop {
        // acc = 1/(2n+1) - z*acc
        acc = recip[n].sub(z.mul(acc));
        if n == 0 {
            break;
        }
        n -= 1;
    }
    acc
}

/// atan(t) for 0 <= t <= 1 as a plain fixed-point number (absolute accuracy)
pub(crate) fn atan_unit(t: Fx) -> Fx {
    let j = (t.to_f64() * 8.0 + 0.5).floor().clamp(0.0, 8.0) as u64;
    let c = Fx::from_u64(j).div_u64(8);
    let den = Fx::ONE.add(t.mul(c));
    let (num, negu) = if t.ge(&c) { (t.sub(c), false) } else { (c.sub(t), true) };
    let u = num.mul(den.recip());
    let a = u.mul(atan_over_t_small(u));
    let base = Fx(ATAN_J8[j as usize]);
    if negu { base.sub(a) } else { base.add(a) }
}

pub(crate) fn pi() -> Fx {
    Fx(ATAN_J8[8]).mul_u64(4)
}

/// atan(a/b) for doubles a, b > 0, as a Big, in (0, pi/2)
pub(crate) fn atan_ratio(a: f64, b: f64) -> Big {
    let (ma, ea) = split(a);
    let (mb, eb) = split(b);
    // q = ma / mb in [2^-53, 2^53] ... normalise ma to the top: (ma << 203) / mb has >= 150 significant bits
    let q = Fx([0, 0, 0, ma << 11, 0]).div_u64(mb); // (ma * 2^11 / mb) / 2^64: value in (2^-54, 2^-10) with 256 fraction bits
    let n = q.bit_len();
    let qn = q.shl(257 - n); // normalised to [1, 2): bit 256 set
    let e = ea - eb + 53 + (n as i32 - 257); // a/b = qn * 2^e    (q = ma*2^11/mb/2^64 = (ma/mb)*2^-53)
    if e < -70 {
        // atan(t) = t (1 - t^2/3 + ...): the correction is below 2^-140 relative; keep the relative form
        let t2 = qn.mul(qn).shr((-2 * e).min(319) as u32);
        let corr = Fx::ONE.sub(t2.div_u64(3));
        return Big { fx: qn.mul(corr), exp: e };
    }
    if e <= 0 {
        // t = a/b <= 1 (or just above 1 when e == 0: qn in [1,2) then t in [1,2): use the reflection below)
        let t = qn.shr((-e) as u32);
        if !t.ge(&Fx::ONE) || t == Fx::ONE {
            if e > -8 {
                return Big { fx: atan_unit(t), exp: 0 };
            }
            // small t: relative form through the series directly (t <= 2^-7)
            return Big { fx: qn.mul(atan_over_t_small(t)), exp: e };
        }
    }
    // t > 1: atan(t) = pi/2 - atan(1/t), 1/t = b/a
    let inv = atan_ratio(b, a);
    let x = inv.fx.shr((-inv.exp).clamp(0, 319) as u32);
    Big { fx: pi().div_u64(2).sub(x), exp: 0 }
}

/// integer significand and exponent of a positive finite double: `v = m * 2^e`, m < 2^53
pub(crate) fn split(v: f64) -> (u64, i32) {
    let b = v.to_bits();
    let e = ((b >> 52) & 0x7ff) as i32;
    let f = b & 0x000f_ffff_ffff_ffff;
    if e == 0 { (f, -1074) } else { (f | 1 << 52, e - 1075) }
}

/// exp(r) for 0 <= r < 1
pub(crate) fn exp_series(r: Fx) -> Fx {
    let mut acc = Fx::ONE;
    let mut n = 60u64;
    while n >= 1 {
        acc = Fx::ONE.add(r.mul(acc).div_u64(n));
        n -= 1;
    }
    acc
}

/// exp(x) for a finite double |x| < 800 as (mantissa in [1, e), binary exponent)
pub(crate) fn exp_big(x: f64) -> Big {
    let ln2 = Fx(LN2);
    let k = (x / std::f64::consts::LN_2).floor() as i64;
    // r = x - k ln2 in [0, ln2) up to the double estimate of k; computed with signs handled explicitly
    let ax = Fx::from_f64(x.abs());
    let kl = ln2.mul_u64(k.unsigned_abs());
    let (mut r, mut k) = if x >= 0.0 {
        if ax.ge(&kl) { (ax.sub(kl), k) } else { (ax.add(ln2).sub(kl), k - 1) }
    } else {
        // x < 0, k < 0: r = k|ln2| - |x|
        if kl.ge(&ax) { (kl.sub(ax), k) } else { (kl.add(ln2).sub(ax), k - 1) }
    };
    while r.ge(&ln2) {
        r = r.sub(ln2);
        k += 1;
    }
    Big { fx: exp_series(r), exp: k as i32 }
}

/// sinh(x)/x for 0 <= x < 1 (keeps relative precision for small x)
pub(crate) fn sinh_over_x_series(x: Fx) -> Fx {
    let z = x.mul(x);
    let mut acc = Fx::ONE;
    let mut n = 36u64;
    while n >= 1 {
        acc = Fx::ONE.add(z.mul(acc).div_u64(2 * n * (2 * n + 1)));
        n -= 1;
    }
    acc
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identities() {
        // sin^2 + cos^2 = 1, tan(atan(j/8)) = j/8, exp(ln2) = 2
        let r = Fx::from_f64(0.7);
        let (s_over_r, c) = sin_cos_series(r);
        let s = s_over_r.mul(r);
        let one = s.mul(s).add(c.mul(c));
        let d = if one.ge(&Fx::ONE) { one.sub(Fx::ONE) } else { Fx::ONE.sub(one) };
        assert!(d.bit_len() < 16, "{d:?}");
        for j in 1..=6u64 {
            let a = Fx(ATAN_J8[j as usize]);
            let (so, c) = sin_cos_series(a);
            let t = so.mul(a).mul(c.recip());
            let want = Fx::from_u64(j).div_u64(8);
            let d = if t.ge(&want) { t.sub(want) } else { want.sub(t) };
            assert!(d.bit_len() < 16, "{j} {d:?}");
            let back = atan_unit(Fx::from_f64(j as f64 / 8.0 + 0.03));
            assert!((back.to_f64() - (j as f64 / 8.0 + 0.03).atan()).abs() < 1e-15);
        }
        let e = exp_series(Fx(LN2));
        let d = if e.ge(&Fx::from_u64(2)) { e.sub(Fx::from_u64(2)) } else { Fx::from_u64(2).sub(e) };
        assert!(d.bit_len() < 16, "{d:?}");
        assert!((pi().to_f64() - std::f64::consts::PI).abs() < 1e-15);
    }
}
