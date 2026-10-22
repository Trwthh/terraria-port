//! The trigonometric x87 instructions executed by THIS processor (`fsin`, `fcos`, `fptan`, `fpatan`), for the x86 and
//! x86_64 builds.
//!
//! The original runs these instructions inline (`Math.Sin` is `fsin`, crate README section 1); their last bit is the
//! processor's microcode: Intel and AMD differ, and the model of `trig.rs` (correct rounding of the true value) equals
//! the 64-bit register on the reference machine (AMD Ryzen 5 7600X) for about 99% of the arguments, the double for all but
//! about 1 in 100,000 (crate README section 3). The original game itself gives a CPU's own results on that CPU, so on a
//! processor with these instructions the port executes them too: then it is the original's arithmetic on any x86
//! machine, the reference machine included, and the reference runs' last-bit cases (the gallery and interaction runs of
//! tests/draw.rs: dust velocities through `Utils.RotatedBy`) agree. `trig.rs` stays the definition elsewhere (the browser
//! build: no x87) and when `TERRARIA_X87_TRIG=model` (read once).
//!
//! The precision control does not apply to these instructions (Intel SDM vol. 1, 8.1.5.2: only FADD, FSUB, FMUL, FDIV,
//! FSQRT and their forms), so the control word this process runs with does not matter; rounding control is the
//! default, round to nearest, in both runtimes.

use crate::f80::{Kind, F80};

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
use std::sync::atomic::{AtomicU8, Ordering};

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
static MODE: AtomicU8 = AtomicU8::new(0);

/// Use the processor's instructions? (x86 / x86_64, unless `TERRARIA_X87_TRIG=model`)
#[inline]
pub fn enabled() -> bool {
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        match MODE.load(Ordering::Relaxed) {
            1 => true,
            2 => false,
            _ => {
                let on = !std::env::var("TERRARIA_X87_TRIG").is_ok_and(|v| v == "model");
                MODE.store(if on { 1 } else { 2 }, Ordering::Relaxed);
                on
            }
        }
    }
    #[cfg(not(any(target_arch = "x86", target_arch = "x86_64")))]
    false
}

/// For tests: force the model (`false`) or the instructions (`true`).
pub fn set_enabled(on: bool) {
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    MODE.store(if on { 1 } else { 2 }, Ordering::Relaxed);
    #[cfg(not(any(target_arch = "x86", target_arch = "x86_64")))]
    let _ = on;
}

/// An 80-bit register image (little endian: 64-bit significand, then sign and 15-bit exponent) as an [`F80`].
fn from_ext(b: [u8; 10]) -> F80 {
    let man = u64::from_le_bytes(b[0..8].try_into().unwrap());
    let se = u16::from_le_bytes([b[8], b[9]]);
    let neg = se & 0x8000 != 0;
    let e = (se & 0x7fff) as i32;
    if e == 0x7fff {
        return if man << 1 == 0 { F80 { kind: Kind::Inf, neg, exp: 0, man } } else { F80 { kind: Kind::Nan, neg, exp: 0, man } };
    }
    if man == 0 {
        return F80::zero(neg);
    }
    // a denormal register value (e == 0) has the exponent of e == 1 and no integer bit; normalise
    let (mut exp, mut m) = (if e == 0 { 1 - 16383 } else { e - 16383 }, man);
    while m >> 63 == 0 {
        m <<= 1;
        exp -= 1;
    }
    F80 { kind: Kind::Finite, neg, exp, man: m }
}

macro_rules! unary {
    ($name:ident, $ins:literal) => {
        #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
        pub fn $name(x: f64) -> F80 {
            let mut out = [0u8; 10];
            // SAFETY: loads `x`, runs the instruction, stores the 80-bit result into `out`; the x87 stack is left as found.
            unsafe {
                core::arch::asm!(
                    "fld qword ptr [{x}]",
                    $ins,
                    "fstp tbyte ptr [{out}]",
                    x = in(reg) &x as *const f64,
                    out = in(reg) out.as_mut_ptr(),
                    out("st(0)") _, out("st(1)") _, out("st(2)") _, out("st(3)") _,
                    out("st(4)") _, out("st(5)") _, out("st(6)") _, out("st(7)") _,
                    options(nostack),
                );
            }
            from_ext(out)
        }
    };
}
unary!(fsin, "fsin");
unary!(fcos, "fcos");
// `fptan` pushes 1.0 when it succeeds; `Math.Tan` pops it (`fstp st(0)`) and stores what is below (out of range: nothing
// was pushed, the pop removes the operand and the store reads an empty register: the masked underflow gives the
// indefinite value)
unary!(fptan, "fptan\n fstp st(0)");

/// `fpatan` with `y` in st(1) and `x` in st(0).
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
pub fn fpatan(y: f64, x: f64) -> F80 {
    let mut out = [0u8; 10];
    // SAFETY: as `unary!`.
    unsafe {
        core::arch::asm!(
            "fld qword ptr [{y}]",
            "fld qword ptr [{x}]",
            "fpatan",
            "fstp tbyte ptr [{out}]",
            y = in(reg) &y as *const f64,
            x = in(reg) &x as *const f64,
            out = in(reg) out.as_mut_ptr(),
            out("st(0)") _, out("st(1)") _, out("st(2)") _, out("st(3)") _,
            out("st(4)") _, out("st(5)") _, out("st(6)") _, out("st(7)") _,
            options(nostack),
        );
    }
    from_ext(out)
}

#[cfg(not(any(target_arch = "x86", target_arch = "x86_64")))]
pub fn fsin(x: f64) -> F80 {
    crate::trig::fsin(x)
}
#[cfg(not(any(target_arch = "x86", target_arch = "x86_64")))]
pub fn fcos(x: f64) -> F80 {
    crate::trig::fcos(x)
}
#[cfg(not(any(target_arch = "x86", target_arch = "x86_64")))]
pub fn fptan(x: f64) -> F80 {
    crate::trig::fptan(x)
}
#[cfg(not(any(target_arch = "x86", target_arch = "x86_64")))]
pub fn fpatan(y: f64, x: f64) -> F80 {
    crate::trig::fpatan(y, x)
}
