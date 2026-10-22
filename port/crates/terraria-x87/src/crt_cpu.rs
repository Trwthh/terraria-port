//! The small register machine the transliterated CRT routines (`crt_sse2_gen.rs`) run on: eight XMM registers (two 64-bit lanes each),
//! eight 32-bit registers, the arithmetic flags, a 256-byte stack, a tiny x87 stack that only ever holds doubles, and read-only access to
//! the constant tables copied from the DLL. Every operation is the IEEE-754 double operation SSE2 performs (round to nearest, exceptions
//! masked), including x86's NaN rules (first NaN operand wins, quieted; an invalid operation gives the "real indefinite" 0xFFF8...).

#![allow(dead_code)] // the instruction set is complete on purpose; not every routine uses every instruction
use crate::crt_sse2_gen::{DATA, DATA_BASE};

const STK_BASE: u32 = 0x7ff0_0000;
const STK_SIZE: u32 = 256;
pub(crate) const SENTINEL: u32 = 0xffff_ffff;
const QNAN_BIT: u64 = 0x0008_0000_0000_0000;
const INDEFINITE: u64 = 0xfff8_0000_0000_0000;

pub(crate) struct Cpu {
    pub x: [[u64; 2]; 8],
    pub r: [u32; 8],
    pub zf: bool,
    pub cf: bool,
    pub sf: bool,
    pub of: bool,
    pub pf: bool,
    stk: [u8; STK_SIZE as usize],
    st: [u64; 8],
    st_len: usize,
    /// set when the routine left the modelled subset (bad address, unsupported instruction); the result is then not trustworthy
    pub fault: u32,
    /// number of calls to the CRT's error-support routine (domain / overflow / underflow reporting; it does not change the result)
    pub errors: u32,
}

#[inline(always)]
fn is_nan(a: u64) -> bool {
    (a & 0x7fff_ffff_ffff_ffff) > 0x7ff0_0000_0000_0000
}

macro_rules! binop {
    ($name:ident, $op:tt) => {
        #[inline(always)]
        pub fn $name(a: u64, b: u64) -> u64 {
            if is_nan(a) {
                return a | QNAN_BIT;
            }
            if is_nan(b) {
                return b | QNAN_BIT;
            }
            let r = f64::from_bits(a) $op f64::from_bits(b);
            if r.is_nan() { INDEFINITE } else { r.to_bits() }
        }
    };
}

impl Cpu {
    pub fn new() -> Cpu {
        let mut c = Cpu {
            x: [[0; 2]; 8],
            r: [0; 8],
            zf: false,
            cf: false,
            sf: false,
            of: false,
            pf: false,
            stk: [0; STK_SIZE as usize],
            st: [0; 8],
            st_len: 0,
            fault: 0,
            errors: 0,
        };
        c.r[4] = STK_BASE + STK_SIZE - 16;
        c
    }

    binop!(add, +);
    binop!(sub, -);
    binop!(mul, *);
    binop!(div, /);

    #[inline(always)]
    pub fn sqrt(a: u64) -> u64 {
        if is_nan(a) {
            return a | QNAN_BIT;
        }
        let r = f64::from_bits(a).sqrt();
        if r.is_nan() { INDEFINITE } else { r.to_bits() }
    }

    #[inline(always)]
    pub fn cmpsd(a: u64, b: u64, pred: u32) -> u64 {
        let (x, y) = (f64::from_bits(a), f64::from_bits(b));
        let un = x.is_nan() || y.is_nan();
        let t = match pred {
            0 => x == y,
            1 => x < y,
            2 => x <= y,
            3 => un,
            4 => !(x == y),
            5 => !(x < y),
            6 => !(x <= y),
            _ => !un,
        };
        if t { u64::MAX } else { 0 }
    }

    pub fn comisd(&mut self, a: u64, b: u64) {
        let (x, y) = (f64::from_bits(a), f64::from_bits(b));
        self.of = false;
        self.sf = false;
        if x.is_nan() || y.is_nan() {
            self.zf = true;
            self.pf = true;
            self.cf = true;
        } else {
            self.zf = x == y;
            self.pf = false;
            self.cf = x < y;
        }
    }

    pub fn cvtsd2si(a: u64) -> u32 {
        let r = f64::from_bits(a).round_ties_even();
        if r >= -2147483648.0 && r <= 2147483647.0 { r as i32 as u32 } else { 0x8000_0000 }
    }

    pub fn cvttsd2si(a: u64) -> u32 {
        let r = f64::from_bits(a).trunc();
        if r >= -2147483648.0 && r <= 2147483647.0 { r as i32 as u32 } else { 0x8000_0000 }
    }

    fn dwords(a: [u64; 2]) -> [u32; 4] {
        [a[0] as u32, (a[0] >> 32) as u32, a[1] as u32, (a[1] >> 32) as u32]
    }
    fn from_dwords(d: [u32; 4]) -> [u64; 2] {
        [d[0] as u64 | (d[1] as u64) << 32, d[2] as u64 | (d[3] as u64) << 32]
    }
    pub fn pshufd(b: [u64; 2], imm: u32) -> [u64; 2] {
        let d = Self::dwords(b);
        Self::from_dwords([d[(imm & 3) as usize], d[(imm >> 2 & 3) as usize], d[(imm >> 4 & 3) as usize], d[(imm >> 6 & 3) as usize]])
    }
    pub fn psubd(a: [u64; 2], b: [u64; 2]) -> [u64; 2] {
        let (x, y) = (Self::dwords(a), Self::dwords(b));
        Self::from_dwords([x[0].wrapping_sub(y[0]), x[1].wrapping_sub(y[1]), x[2].wrapping_sub(y[2]), x[3].wrapping_sub(y[3])])
    }
    pub fn paddd(a: [u64; 2], b: [u64; 2]) -> [u64; 2] {
        let (x, y) = (Self::dwords(a), Self::dwords(b));
        Self::from_dwords([x[0].wrapping_add(y[0]), x[1].wrapping_add(y[1]), x[2].wrapping_add(y[2]), x[3].wrapping_add(y[3])])
    }
    pub fn pcmpeqd(a: [u64; 2], b: [u64; 2]) -> [u64; 2] {
        let (x, y) = (Self::dwords(a), Self::dwords(b));
        let m = |i: usize| if x[i] == y[i] { u32::MAX } else { 0 };
        Self::from_dwords([m(0), m(1), m(2), m(3)])
    }
    pub fn pmaxsw(a: [u64; 2], b: [u64; 2]) -> [u64; 2] {
        let mut o = [0u64; 2];
        for l in 0..2 {
            for w in 0..4 {
                let x = (a[l] >> (16 * w)) as u16 as i16;
                let y = (b[l] >> (16 * w)) as u16 as i16;
                o[l] |= (x.max(y) as u16 as u64) << (16 * w);
            }
        }
        o
    }
    pub fn pmovmskb(a: [u64; 2]) -> u32 {
        let mut m = 0u32;
        for i in 0..16 {
            m |= (((a[i / 8] >> (8 * (i % 8) + 7)) & 1) as u32) << i;
        }
        m
    }

    // ---- integer flags
    #[inline(always)]
    fn szp(&mut self, r: u32) {
        self.zf = r == 0;
        self.sf = (r as i32) < 0;
        self.pf = (r as u8).count_ones() % 2 == 0;
    }
    #[inline(always)]
    pub fn f_add(&mut self, a: u32, b: u32) -> u32 {
        let r = a.wrapping_add(b);
        self.cf = r < a;
        self.of = ((a ^ r) & (b ^ r)) >> 31 != 0;
        self.szp(r);
        r
    }
    #[inline(always)]
    pub fn f_adc(&mut self, a: u32, b: u32) -> u32 {
        let w = a as u64 + b as u64 + self.cf as u64;
        let r = w as u32;
        self.cf = w >> 32 != 0;
        self.of = ((a ^ r) & (b ^ r)) >> 31 != 0;
        self.szp(r);
        r
    }
    #[inline(always)]
    pub fn f_sub(&mut self, a: u32, b: u32) -> u32 {
        let r = a.wrapping_sub(b);
        self.cf = a < b;
        self.of = ((a ^ b) & (a ^ r)) >> 31 != 0;
        self.szp(r);
        r
    }
    #[inline(always)]
    pub fn f_sbb(&mut self, a: u32, b: u32) -> u32 {
        let c = self.cf as u64;
        let r = (a as u64).wrapping_sub(b as u64).wrapping_sub(c) as u32;
        self.cf = (a as u64) < (b as u64 + c);
        self.of = ((a ^ b) & (a ^ r)) >> 31 != 0;
        self.szp(r);
        r
    }
    #[inline(always)]
    fn logic(&mut self, r: u32) -> u32 {
        self.cf = false;
        self.of = false;
        self.szp(r);
        r
    }
    #[inline(always)]
    pub fn f_and(&mut self, a: u32, b: u32) -> u32 {
        self.logic(a & b)
    }
    #[inline(always)]
    pub fn f_or(&mut self, a: u32, b: u32) -> u32 {
        self.logic(a | b)
    }
    #[inline(always)]
    pub fn f_xor(&mut self, a: u32, b: u32) -> u32 {
        self.logic(a ^ b)
    }
    pub fn f_shl(&mut self, a: u32, n: u32) -> u32 {
        let n = n & 31;
        if n == 0 {
            return a;
        }
        let r = a << n;
        self.cf = (a >> (32 - n)) & 1 != 0;
        self.of = ((r >> 31) != 0) != self.cf;
        self.szp(r);
        r
    }
    pub fn f_shr(&mut self, a: u32, n: u32) -> u32 {
        let n = n & 31;
        if n == 0 {
            return a;
        }
        let r = a >> n;
        self.cf = (a >> (n - 1)) & 1 != 0;
        self.of = a >> 31 != 0;
        self.szp(r);
        r
    }
    pub fn f_sar(&mut self, a: u32, n: u32) -> u32 {
        let n = n & 31;
        if n == 0 {
            return a;
        }
        let r = ((a as i32) >> n) as u32;
        self.cf = ((a as i32) >> (n - 1)) & 1 != 0;
        self.of = false;
        self.szp(r);
        r
    }
    pub fn f_neg(&mut self, a: u32) -> u32 {
        let r = 0u32.wrapping_sub(a);
        self.cf = a != 0;
        self.of = a == 0x8000_0000;
        self.szp(r);
        r
    }
    pub fn f_inc(&mut self, a: u32) -> u32 {
        let r = a.wrapping_add(1);
        self.of = r == 0x8000_0000;
        self.szp(r);
        r
    }
    pub fn f_dec(&mut self, a: u32) -> u32 {
        let r = a.wrapping_sub(1);
        self.of = a == 0x8000_0000;
        self.szp(r);
        r
    }
    pub fn f_not(&mut self, a: u32) -> u32 {
        !a
    }

    // ---- memory
    #[inline(always)]
    fn rd8(&mut self, addr: u32) -> u8 {
        let s = addr.wrapping_sub(STK_BASE);
        if s < STK_SIZE {
            return self.stk[s as usize];
        }
        let o = addr.wrapping_sub(DATA_BASE) as usize;
        if o < DATA.len() * 8 {
            return (DATA[o / 8] >> (8 * (o % 8))) as u8;
        }
        self.fault = addr | 1;
        0
    }
    #[inline(always)]
    fn wr8(&mut self, addr: u32, v: u8) {
        let s = addr.wrapping_sub(STK_BASE);
        if s < STK_SIZE {
            self.stk[s as usize] = v;
        } else {
            self.fault = addr | 1;
        }
    }
    #[inline(always)]
    pub fn rd64(&mut self, addr: u32) -> u64 {
        let o = addr.wrapping_sub(DATA_BASE) as usize;
        if o % 8 == 0 && o / 8 < DATA.len() {
            return DATA[o / 8];
        }
        let mut v = 0u64;
        for i in 0..8 {
            v |= (self.rd8(addr.wrapping_add(i)) as u64) << (8 * i);
        }
        v
    }
    #[inline(always)]
    pub fn rd128(&mut self, addr: u32) -> [u64; 2] {
        [self.rd64(addr), self.rd64(addr.wrapping_add(8))]
    }
    pub fn rd32(&mut self, addr: u32) -> u32 {
        let mut v = 0u32;
        for i in 0..4 {
            v |= (self.rd8(addr.wrapping_add(i)) as u32) << (8 * i);
        }
        v
    }
    pub fn wr32(&mut self, addr: u32, v: u32) {
        for i in 0..4 {
            self.wr8(addr.wrapping_add(i), (v >> (8 * i)) as u8);
        }
    }
    pub fn wr64(&mut self, addr: u32, v: u64) {
        for i in 0..8 {
            self.wr8(addr.wrapping_add(i), (v >> (8 * i)) as u8);
        }
    }
    pub fn push32(&mut self, v: u32) {
        self.r[4] = self.r[4].wrapping_sub(4);
        self.wr32(self.r[4], v);
    }
    pub fn pop32(&mut self) -> u32 {
        let v = self.rd32(self.r[4]);
        self.r[4] = self.r[4].wrapping_add(4);
        v
    }

    // ---- x87 stack (doubles only)
    pub fn fpush(&mut self, v: u64) {
        if self.st_len < 8 {
            self.st[self.st_len] = v;
            self.st_len += 1;
        } else {
            self.fault = 3;
        }
    }
    pub fn fpop(&mut self) -> u64 {
        if self.st_len == 0 {
            self.fault = 5;
            return INDEFINITE;
        }
        self.st_len -= 1;
        self.st[self.st_len]
    }
    pub fn fst(&mut self, i: usize) -> u64 {
        if i >= self.st_len {
            self.fault = 7;
            return INDEFINITE;
        }
        self.st[self.st_len - 1 - i]
    }
    pub fn fset(&mut self, i: usize, v: u64) {
        if i >= self.st_len {
            self.fault = 9;
            return;
        }
        let k = self.st_len - 1 - i;
        self.st[k] = v;
    }
    pub fn fxch(&mut self, i: usize) {
        let a = self.fst(0);
        let b = self.fst(i);
        self.fset(0, b);
        self.fset(i, a);
    }

    /// The CRT's error-support routine (0x100986f5 in the DLL): `f(arg1*, arg2*, retval*, code)`. It builds the `_exception` record,
    /// calls the (default, do-nothing) matherr handler, sets errno and stores the return value back. Two groups of codes replace the value:
    /// 26 stores 1.0, 1000/1001 store `*arg1` (a NaN argument is returned as it came). The value passes through an x87 load/store, which
    /// quiets a signalling NaN.
    pub fn error_support(&mut self) {
        self.errors += 1;
        let esp = self.r[4];
        let p1 = self.rd32(esp);
        let pret = self.rd32(esp.wrapping_add(8));
        let code = self.rd32(esp.wrapping_add(12));
        let quiet = |v: u64| if is_nan(v) { v | QNAN_BIT } else { v };
        match code {
            0x1a => self.wr64(pret, 0x3ff0_0000_0000_0000),
            0x3e8 | 0x3e9 => {
                let v = self.rd64(p1);
                self.wr64(pret, quiet(v));
            }
            2 | 3 | 8 | 9 | 0xe | 0xf | 0x18 | 0x19 | 0x1b | 0x1c | 0x31 | 0x3a | 0x3d => {
                let v = self.rd64(pret);
                self.wr64(pret, quiet(v));
            }
            _ => {}
        }
    }
    pub fn unsupported(&mut self, addr: u32) {
        self.fault = addr;
    }
}

/// `(a * b + b) * c` with every intermediate rounded to a 64-bit significand and the result rounded to double: the x87 sequence
/// `fmul; faddp; fmulp; fstp qword` that `_CIexp` runs with the precision control temporarily set to 64 bits.
pub(crate) fn ext_mul_add_mul(a: u64, b: u64, c: u64) -> u64 {
    use crate::f80::F80;
    let (a, b, c) = (F80::from_f64(f64::from_bits(a)), F80::from_f64(f64::from_bits(b)), F80::from_f64(f64::from_bits(c)));
    a.mul(b, 64).add(b, 64).mul(c, 64).to_f64().to_bits()
}
