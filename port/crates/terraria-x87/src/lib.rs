//! `System.Math` as the original game computes it: 32-bit .NET Framework 4 (CLR 4.0.30319, legacy x86 JIT), x87 FPU with the control
//! word `0x027F` (53-bit precision, round to nearest, exceptions masked).
//!
//! Every function takes and returns the values the C# code sees and is tested bit for bit against tables dumped from that runtime
//! (`refdata/worldgen/x87*`, produced by `tools/refdump_worldgen`). Three families, by how the runtime implements them
//! (established from `clr.dll`'s import table and from raw instruction dumps, see README.md of this crate):
//!
//! | family | functions | implementation here | exactness |
//! |---|---|---|---|
//! | x87 instruction, inline | `sin` `cos` `tan` (`fsin` `fcos` `fptan`), `atan` `atan2` (`fpatan`) | on x86 / x86_64 the processor's own instruction (`hw.rs`; `TERRARIA_X87_TRIG=model` for the model); elsewhere exact reduction with the instruction's 66-bit pi, high-precision evaluation, rounding to the 64-bit register, then to double | the instruction: exact on that CPU by definition; the model: limited by the last-bit error of the CPU's microcode, about 1 call in 100,000 off by one ulp |
//! | exact by IEEE-754 | `sqrt` `abs` `floor` `ceiling` `round*` `truncate` `sign` `min` `max` `fmod` `ieee_remainder` | the same operations in Rust | exact |
//! | C runtime, SSE2 code | `pow` `log` `log10` `acos` `asin` `exp` | transliteration of the routines in `ucrtbase_clr0400.dll` (`crt_sse2_gen.rs`) | exact (same operations, same tables) |
//! | C runtime, x87 code | `sinh` `cosh` `tanh` | correctly rounded value | approximate: see README.md |
//!
//! The reference tables come from one machine (AMD Ryzen 5 7600X). The first family depends on the processor: Intel and AMD microcode
//! differ in the last bit of the 80-bit result for some arguments. "Exact" for that family always means "equal to that CPU".

mod big;
mod consts;
mod crt_cpu;
mod crt_sse2_gen;
pub mod f80;
pub mod hw;
mod trig;

pub use f80::F80;
use crt_cpu::{Cpu, SENTINEL};

const INDEFINITE: u64 = 0xfff8_0000_0000_0000;

// the instruction: this processor's (hw.rs) on x86, else the model (trig.rs)
#[inline]
fn x87_fsin(x: f64) -> F80 {
    if hw::enabled() { hw::fsin(x) } else { trig::fsin(x) }
}
#[inline]
fn x87_fcos(x: f64) -> F80 {
    if hw::enabled() { hw::fcos(x) } else { trig::fcos(x) }
}
#[inline]
fn x87_fptan(x: f64) -> F80 {
    if hw::enabled() { hw::fptan(x) } else { trig::fptan(x) }
}
#[inline]
fn x87_fpatan(y: f64, x: f64) -> F80 {
    if hw::enabled() { hw::fpatan(y, x) } else { trig::fpatan(y, x) }
}

/// The model of the instructions (trig.rs), whatever the processor (tests of the model).
pub mod model {
    use crate::{trig, F80};
    pub fn fsin(x: f64) -> F80 {
        trig::fsin(x)
    }
    pub fn fcos(x: f64) -> F80 {
        trig::fcos(x)
    }
    pub fn fptan(x: f64) -> F80 {
        trig::fptan(x)
    }
    pub fn fpatan(y: f64, x: f64) -> F80 {
        trig::fpatan(y, x)
    }
}


// ------------------------------------------------------------------------------------------------ x87 instructions

/// `fsin` result as it sits in the x87 register (64-bit significand). `Math.Sin(x) * y` multiplies THIS value, not the double.
pub fn sin_ext(x: f64) -> F80 {
    x87_fsin(x)
}
/// `fcos` result in the register.
pub fn cos_ext(x: f64) -> F80 {
    x87_fcos(x)
}
/// `fptan` result in the register (after the pop of the pushed 1.0).
pub fn tan_ext(x: f64) -> F80 {
    x87_fptan(x)
}
/// `fpatan` result in the register, with the CLR's extra rule that two infinite operands give NaN.
pub fn atan2_ext(y: f64, x: f64) -> F80 {
    if y.is_infinite() && x.is_infinite() {
        return F80::indefinite();
    }
    x87_fpatan(y, x)
}
/// `fld1; fpatan` result in the register.
pub fn atan_ext(x: f64) -> F80 {
    x87_fpatan(x, 1.0)
}

/// `Math.Sin(x)` stored to a `double`.
pub fn sin(x: f64) -> f64 {
    x87_fsin(x).to_f64()
}
/// `Math.Cos(x)` stored to a `double`.
pub fn cos(x: f64) -> f64 {
    x87_fcos(x).to_f64()
}
/// `Math.Tan(x)` stored to a `double`.
pub fn tan(x: f64) -> f64 {
    x87_fptan(x).to_f64()
}
/// `Math.Atan(x)` stored to a `double`.
pub fn atan(x: f64) -> f64 {
    atan_ext(x).to_f64()
}
/// `Math.Atan2(y, x)` stored to a `double`.
pub fn atan2(y: f64, x: f64) -> f64 {
    atan2_ext(y, x).to_f64()
}
/// `(float)Math.Sin(x)`: one rounding from the register to `float`.
pub fn sin_f32(x: f64) -> f32 {
    x87_fsin(x).to_f32()
}
/// `(float)Math.Cos(x)`
pub fn cos_f32(x: f64) -> f32 {
    x87_fcos(x).to_f32()
}
/// `(float)Math.Tan(x)`
pub fn tan_f32(x: f64) -> f32 {
    x87_fptan(x).to_f32()
}
/// `(float)Math.Atan2(y, x)`
pub fn atan2_f32(y: f64, x: f64) -> f32 {
    atan2_ext(y, x).to_f32()
}
/// `(float)Math.Atan(x)`
pub fn atan_f32(x: f64) -> f32 {
    atan_ext(x).to_f32()
}

// ------------------------------------------------------------------------------------------------ exact functions

/// `Math.Sqrt`: `fsqrt` honours the precision control, so this is the IEEE square root. A negative operand gives the indefinite NaN.
pub fn sqrt(x: f64) -> f64 {
    if x.is_nan() {
        return f64::from_bits(x.to_bits() | 0x0008_0000_0000_0000);
    }
    if x < 0.0 {
        return f64::from_bits(INDEFINITE);
    }
    x.sqrt()
}
/// `Math.Abs(double)`: `fabs` (clears the sign, also of a NaN).
pub fn abs(x: f64) -> f64 {
    x.abs()
}
/// `Math.Floor(double)`
pub fn floor(x: f64) -> f64 {
    x.floor()
}
/// `Math.Ceiling(double)`
pub fn ceiling(x: f64) -> f64 {
    x.ceil()
}
/// `Math.Round(double)`: `frndint`, round half to even.
pub fn round(x: f64) -> f64 {
    x.round_ties_even()
}
/// `Math.Truncate(double)`
pub fn truncate(x: f64) -> f64 {
    x.trunc()
}
const POW10: [f64; 16] = [1e0, 1e1, 1e2, 1e3, 1e4, 1e5, 1e6, 1e7, 1e8, 1e9, 1e10, 1e11, 1e12, 1e13, 1e14, 1e15];
/// `Math.Round(double, int)` (mscorlib `InternalRound`, banker's rounding): `value * 10^digits`, `frndint`, `/ 10^digits`, only when
/// `|value| < 1e16`. Panics for digits outside 0..=15 (the original throws `ArgumentOutOfRangeException`).
pub fn round_digits(x: f64, digits: i32) -> f64 {
    assert!((0..=15).contains(&digits), "Math.Round: digits out of range");
    if x.abs() < 1e16 {
        let p = POW10[digits as usize];
        return (x * p).round_ties_even() / p;
    }
    x
}
/// `Math.Round(double, MidpointRounding.AwayFromZero)`
pub fn round_away(x: f64) -> f64 {
    round_digits_away(x, 0)
}
/// `Math.Round(double, int, MidpointRounding.AwayFromZero)`: `value * 10^digits`, split with `modf`, add the sign of the fraction when
/// `|fraction| >= 0.5`, `/ 10^digits`.
pub fn round_digits_away(x: f64, digits: i32) -> f64 {
    assert!((0..=15).contains(&digits), "Math.Round: digits out of range");
    if x.abs() < 1e16 {
        let p = POW10[digits as usize];
        let v = x * p;
        let mut i = v.trunc();
        let frac = v - i;
        if frac.abs() >= 0.5 {
            i += if frac < 0.0 { -1.0 } else { 1.0 };
        }
        return i / p;
    }
    x
}
/// `Math.Sign(double)`. The original throws `ArithmeticException` for NaN; this returns `None`.
pub fn sign(x: f64) -> Option<i32> {
    if x < 0.0 {
        Some(-1)
    } else if x > 0.0 {
        Some(1)
    } else if x == 0.0 {
        Some(0)
    } else {
        None
    }
}
/// `Math.Min(double, double)` of .NET Framework 4: `val1 < val2 ? val1 : IsNaN(val1) ? val1 : val2`.
pub fn min(a: f64, b: f64) -> f64 {
    if a < b || a.is_nan() { a } else { b }
}
/// `Math.Max(double, double)` of .NET Framework 4: `val1 > val2 ? val1 : IsNaN(val1) ? val1 : val2`.
pub fn max(a: f64, b: f64) -> f64 {
    if a > b || a.is_nan() { a } else { b }
}
/// The `%` operator on doubles (and on floats: the exact result fits the operand type). `fprem` loop: exact, sign of the dividend.
pub fn fmod(x: f64, y: f64) -> f64 {
    if x.is_nan() {
        return f64::from_bits(x.to_bits() | 0x0008_0000_0000_0000);
    }
    if y.is_nan() {
        return f64::from_bits(y.to_bits() | 0x0008_0000_0000_0000);
    }
    if x.is_infinite() || y == 0.0 {
        return f64::from_bits(INDEFINITE);
    }
    x % y
}
/// `Math.IEEERemainder` (the managed code of .NET Framework 4, statement for statement).
pub fn ieee_remainder(x: f64, y: f64) -> f64 {
    if x.is_nan() {
        return x;
    }
    if y.is_nan() {
        return y;
    }
    let regular = fmod(x, y);
    if regular.is_nan() {
        return f64::from_bits(INDEFINITE);
    }
    if regular == 0.0 && x.is_sign_negative() {
        return -0.0;
    }
    let alt = regular - y.abs() * sign(x).unwrap_or(0) as f64;
    if alt.abs() == regular.abs() {
        let div = x / y;
        let rounded = div.round_ties_even();
        return if rounded.abs() > div.abs() { alt } else { regular };
    }
    if alt.abs() < regular.abs() { alt } else { regular }
}

// ------------------------------------------------------------------------------------------------ C runtime (SSE2 routines)

fn crt1(f: fn(&mut Cpu), x: f64) -> f64 {
    let mut c = Cpu::new();
    c.fpush(x.to_bits());
    c.push32(SENTINEL);
    f(&mut c);
    debug_assert!(c.fault == 0, "CRT routine left the modelled subset at {:#x}", c.fault);
    f64::from_bits(c.fst(0))
}
/// `Math.Pow`: the CLR's argument checks (`COMDouble::Pow`: a NaN argument is returned, `pow(1, +-inf) = 1`, `pow(-1, +-inf) = NaN`)
/// and then `_CIpow` of the C runtime.
pub fn pow(x: f64, y: f64) -> f64 {
    if y.is_nan() {
        return y;
    }
    if x.is_nan() {
        return x;
    }
    if y.is_infinite() {
        if x == 1.0 {
            return x;
        }
        if x == -1.0 {
            return f64::from_bits(INDEFINITE);
        }
    }
    let mut c = Cpu::new();
    c.fpush(x.to_bits());
    c.fpush(y.to_bits());
    c.push32(SENTINEL);
    crt_sse2_gen::pow(&mut c);
    debug_assert!(c.fault == 0, "CRT routine left the modelled subset at {:#x}", c.fault);
    f64::from_bits(c.fst(0))
}
/// `Math.Log(double)`
pub fn log(x: f64) -> f64 {
    crt1(crt_sse2_gen::log, x)
}
/// `Math.Log10`
pub fn log10(x: f64) -> f64 {
    crt1(crt_sse2_gen::log10, x)
}
/// `Math.Acos`
pub fn acos(x: f64) -> f64 {
    crt1(crt_sse2_gen::acos, x)
}
/// `Math.Asin`
pub fn asin(x: f64) -> f64 {
    crt1(crt_sse2_gen::asin, x)
}
/// `Math.Exp`
pub fn exp(x: f64) -> f64 {
    crt1(crt_sse2_gen::exp, x)
}

// ------------------------------------------------------------------------------------------------ C runtime (x87 routines): approximate

fn hyper(x: f64, which: u8) -> f64 {
    use big::{Big, Fx};
    if x.is_nan() {
        return f64::from_bits(x.to_bits() | 0x0008_0000_0000_0000);
    }
    let ax = x.abs();
    let neg = x < 0.0;
    if ax == 0.0 {
        return if which == 1 { 1.0 } else { x };
    }
    if ax > 760.0 {
        return match which {
            0 => if neg { f64::NEG_INFINITY } else { f64::INFINITY },
            1 => f64::INFINITY,
            _ => if neg { -1.0 } else { 1.0 },
        };
    }
    // sinh and cosh as (fixed point, exponent)
    let (sh, ch): (Big, Big) = if ax < 1.0 {
        let fx = Fx::from_f64(ax);
        let (m, e) = big::split(ax);
        let n = 64 - m.leading_zeros();
        let norm = Fx([m, 0, 0, 0, 0]).shl(257 - n);
        let s = Big { fx: norm.mul(big::sinh_over_x_series(fx)), exp: e + n as i32 - 1 };
        let ex = big::exp_big(ax);
        let p = ex.fx.shl(ex.exp as u32); // e^x in [1, e)
        let c = p.add(p.recip()).shr(1);
        (s, Big { fx: c, exp: 0 })
    } else {
        let ex = big::exp_big(ax); // e^x = fx * 2^k, k >= 1
        let inv = ex.fx.recip().shr((2 * ex.exp).min(319) as u32); // e^-x / 2^k
        (Big { fx: ex.fx.sub(inv), exp: ex.exp - 1 }, Big { fx: ex.fx.add(inv), exp: ex.exp - 1 })
    };
    match which {
        0 => sh.to_f80(neg).to_f64(),
        1 => ch.to_f80(false).to_f64(),
        _ => {
            // tanh = sinh / cosh; both mantissas in [0.5, 4)
            let q = sh.fx.mul(ch.fx.recip());
            Big { fx: q, exp: sh.exp - ch.exp }.to_f80(neg).to_f64()
        }
    }
}
/// `Math.Sinh`: correctly rounded value. The runtime's x87 routine differs in the last bit for some arguments (README.md).
pub fn sinh(x: f64) -> f64 {
    hyper(x, 0)
}
/// `Math.Cosh`: correctly rounded value (see `sinh`).
pub fn cosh(x: f64) -> f64 {
    hyper(x, 1)
}
/// `Math.Tanh`: correctly rounded value (see `sinh`).
pub fn tanh(x: f64) -> f64 {
    hyper(x, 2)
}
