//! Bit-for-bit comparison with the reference tables dumped from the real 32-bit runtime (`refdata/worldgen/x87`, `x87_more`, `x87_dense`,
//! `x87_raw`; formats in `refdata/worldgen/README.md`). Run with `--nocapture` to see the report. A missing table is skipped with a message.
//!
//! Two kinds of assertion:
//! * functions that are exact by construction (IEEE operations, transliterated C runtime routines): every record must match;
//! * functions computed by x87 microcode (`fsin`, `fcos`, `fptan`, `fpatan`): the model is "correctly rounded 64-bit register value, then
//!   rounded to double". The microcode is not correctly rounded in about 1% of the calls, which reaches the double in about 1 call in
//!   100,000. The tests require that every mismatch is exactly that (one unit of the 64-bit significand apart) and bound their number.

use std::path::PathBuf;
use terraria_x87 as m;
use terraria_x87::f80::{Kind, F80};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../refdata/worldgen")
}

struct Table {
    nargs: usize,
    words: Vec<u64>,
}
fn table(dir: &str, name: &str) -> Option<Table> {
    let p = root().join(dir).join(format!("{name}.bin"));
    let b = match std::fs::read(&p) {
        Ok(b) => b,
        Err(_) => {
            println!("SKIPPED {dir}/{name}: {} not found", p.display());
            return None;
        }
    };
    assert_eq!(&b[0..4], b"X87T");
    let nargs = u32::from_le_bytes(b[8..12].try_into().unwrap()) as usize;
    let words = b[20..].chunks_exact(8).map(|c| u64::from_le_bytes(c.try_into().unwrap())).collect();
    Some(Table { nargs, words })
}

fn ulps(a: u64, b: u64) -> f64 {
    let (x, y) = (f64::from_bits(a), f64::from_bits(b));
    if a == b {
        return 0.0;
    }
    if x.is_nan() || y.is_nan() || x.is_infinite() || y.is_infinite() || (a >> 63) != (b >> 63) {
        return f64::INFINITY;
    }
    (a as i64 - b as i64).unsigned_abs() as f64
}

#[derive(Default)]
struct Stat {
    n: usize,
    bad: usize,
    max_ulp: f64,
    shown: Vec<String>,
    buckets: [usize; 5],
}
impl Stat {
    fn add(&mut self, args: &[f64], got: u64, want: u64) {
        self.n += 1;
        if got == want || (f64::from_bits(got).is_nan() && f64::from_bits(want).is_nan() && got == want) {
            return;
        }
        self.bad += 1;
        let u = ulps(got, want);
        if u > self.max_ulp {
            self.max_ulp = u;
        }
        let a = args[0].abs();
        let k = if a <= 1.0 { 0 } else if a <= 10.0 { 1 } else if a <= 100.0 { 2 } else if a <= 1000.0 { 3 } else { 4 };
        self.buckets[k] += 1;
        if self.shown.len() < 6 {
            self.shown.push(format!("    f({:?}) = {:016x} ({:e}), reference {:016x} ({:e})", args, got, f64::from_bits(got), want, f64::from_bits(want)));
        }
    }
    fn report(&self, name: &str) {
        println!(
            "{:<26} records {:>8}  exact {:>8}  mismatches {:>5}  max ulp {:<4}  by |arg0| (<=1,<=10,<=100,<=1000,>1000): {:?}",
            name, self.n, self.n - self.bad, self.bad, self.max_ulp, self.buckets
        );
        for s in &self.shown {
            println!("{s}");
        }
    }
}

fn run(dir: &str, name: &str, f: &dyn Fn(&[f64]) -> f64) -> Option<Stat> {
    let t = table(dir, name)?;
    let mut st = Stat::default();
    let mut args = vec![0.0; t.nargs];
    for rec in t.words.chunks_exact(t.nargs + 1) {
        for i in 0..t.nargs {
            args[i] = f64::from_bits(rec[i]);
        }
        st.add(&args, f(&args).to_bits(), rec[t.nargs]);
    }
    st.report(&format!("{dir}/{name}"));
    Some(st)
}
fn exact(dir: &str, name: &str, f: &dyn Fn(&[f64]) -> f64) {
    if let Some(st) = run(dir, name, f) {
        assert_eq!(st.bad, 0, "{dir}/{name}: {} mismatches", st.bad);
    }
}

#[test]
fn ieee_exact_functions() {
    exact("x87", "Sqrt", &|a| m::sqrt(a[0]));
    exact("x87", "Abs", &|a| m::abs(a[0]));
    exact("x87", "Floor", &|a| m::floor(a[0]));
    exact("x87", "Ceiling", &|a| m::ceiling(a[0]));
    exact("x87", "Round", &|a| m::round(a[0]));
    exact("x87", "RoundAway", &|a| m::round_away(a[0]));
    exact("x87_more", "Truncate", &|a| m::truncate(a[0]));
    exact("x87_more", "Sign", &|a| m::sign(a[0]).unwrap_or(-99) as f64);
    for d in [0, 1, 2, 3, 4, 15] {
        exact("x87_more", &format!("Round{d}"), &|a| m::round_digits(a[0], d));
    }
    exact("x87_more", "Round2Away", &|a| m::round_digits_away(a[0], 2));
    exact("x87_more", "IEEERemainder", &|a| m::ieee_remainder(a[0], a[1]));
    exact("x87_more", "Min", &|a| m::min(a[0], a[1]));
    exact("x87_more", "Max", &|a| m::max(a[0], a[1]));
    exact("x87_more", "Fmod", &|a| m::fmod(a[0], a[1]));
    // doubles kept in locals or stored to a field give the same bits as IEEE arithmetic (no extra precision at a 53-bit control word)
    exact("x87_more", "DblLocal", &|a| { let x = a[0] * a[1] + 0.1; let b = x / 3.0 - a[0]; x * b + a[1] });
    exact("x87_more", "DblField", &|a| { let x = a[0] * a[1] + 0.1; let b = x / 3.0 - a[0]; x * b + a[1] });
    exact("x87_more", "SqrtMul", &|a| m::sqrt(a[0]) * a[1]);
}

#[test]
fn crt_sse2_functions() {
    exact("x87", "Pow", &|a| m::pow(a[0], a[1]));
    exact("x87_more", "PowSpecial", &|a| m::pow(a[0], a[1]));
    exact("x87", "Log", &|a| m::log(a[0]));
    exact("x87", "Log10", &|a| m::log10(a[0]));
    exact("x87", "Acos", &|a| m::acos(a[0]));
    exact("x87", "Asin", &|a| m::asin(a[0]));
    exact("x87", "Exp", &|a| m::exp(a[0]));
    // the routine returns a double: nothing wider than 53 bits reaches the following multiplication
    exact("x87_more", "PowMul", &|a| m::pow(a[0], 1.5) * a[1]);
}

/// Functions computed by microcode: report, and require every mismatch to be "one unit of the 64-bit significand".
fn microcode(dir: &str, name: &str, ext: &dyn Fn(&[f64]) -> F80, budget_per_million: usize) {
    let Some(t) = table(dir, name) else { return };
    let mut st = Stat::default();
    let mut unexplained = 0;
    let mut args = vec![0.0; t.nargs];
    for rec in t.words.chunks_exact(t.nargs + 1) {
        for i in 0..t.nargs {
            args[i] = f64::from_bits(rec[i]);
        }
        let v = ext(&args);
        let want = rec[t.nargs];
        st.add(&args, v.to_f64().to_bits(), want);
        if v.to_f64().to_bits() != want && !neighbours(v).iter().any(|w| w.to_f64().to_bits() == want) {
            unexplained += 1;
        }
    }
    st.report(&format!("{dir}/{name}"));
    assert_eq!(unexplained, 0, "{dir}/{name}: mismatches that are not a last-bit difference of the 64-bit result");
    assert!(st.bad * 1_000_000 <= budget_per_million * st.n.max(1), "{dir}/{name}: {} mismatches in {}", st.bad, st.n);
}
fn neighbours(v: F80) -> [F80; 2] {
    if v.kind != Kind::Finite {
        return [v, v];
    }
    let up = if v.man == u64::MAX { F80 { man: 1 << 63, exp: v.exp + 1, ..v } } else { F80 { man: v.man + 1, ..v } };
    let dn = if v.man == 1 << 63 { F80 { man: u64::MAX, exp: v.exp - 1, ..v } } else { F80 { man: v.man - 1, ..v } };
    [up, dn]
}

#[test]
fn x87_instruction_functions() {
    // budget: 100 per million on the 20000-record tables means at most 2 records
    microcode("x87", "Sin", &|a| m::sin_ext(a[0]), 100);
    microcode("x87", "Cos", &|a| m::cos_ext(a[0]), 100);
    microcode("x87", "Tan", &|a| m::tan_ext(a[0]), 100);
    microcode("x87", "Atan", &|a| m::atan_ext(a[0]), 100);
    microcode("x87", "Atan2", &|a| m::atan2_ext(a[0], a[1]), 100);
    // the register value (64-bit significand) is what the next operation sees: multiply / add at 53-bit precision, then store
    let mul53 = |v: F80, y: f64| v.mul(F80::from_f64(y), 53).to_f64();
    let add53 = |v: F80, y: f64| v.add(F80::from_f64(y), 53).to_f64();
    microcode2("x87_more", "SinMul", &|a| mul53(m::sin_ext(a[0]), a[1]), &|a| m::sin(a[0]) * a[1]);
    microcode2("x87_more", "CosMul", &|a| mul53(m::cos_ext(a[0]), a[1]), &|a| m::cos(a[0]) * a[1]);
    microcode2("x87_more", "SinAdd", &|a| add53(m::sin_ext(a[0]), a[1]), &|a| m::sin(a[0]) + a[1]);
    microcode2("x87_more", "TanMul", &|a| mul53(m::tan_ext(a[0]), a[1]), &|a| m::tan(a[0]) * a[1]);
    microcode2("x87_more", "AtanMul", &|a| mul53(m::atan_ext(a[0]), a[1]), &|a| m::atan(a[0]) * a[1]);
    microcode2("x87_more", "Atan2Mul3", &|a| mul53(m::atan2_ext(a[0], a[1]), 57.29577951308232), &|a| m::atan2(a[0], a[1]) * 57.29577951308232);
    microcode2(
        "x87_more",
        "SinLocalMul",
        &|a| m::sin_ext(a[0]).mul(F80::from_f64(a[1]), 53).add(m::cos_ext(a[0]), 53).to_f64(),
        &|a| m::sin(a[0]) * a[1] + m::cos(a[0]),
    );
}

/// Two candidate models for "the result of the instruction is used without a store": `ext` = the 64-bit register value feeds the next
/// operation, `dbl` = the value is first rounded to double. Prints how many records each reproduces.
fn microcode2(dir: &str, name: &str, ext: &dyn Fn(&[f64]) -> f64, dbl: &dyn Fn(&[f64]) -> f64) {
    let Some(t) = table(dir, name) else { return };
    let (mut n, mut be, mut bd) = (0, 0, 0);
    let mut args = vec![0.0; t.nargs];
    for rec in t.words.chunks_exact(t.nargs + 1) {
        for i in 0..t.nargs {
            args[i] = f64::from_bits(rec[i]);
        }
        n += 1;
        let want = rec[t.nargs];
        let same = |g: f64| g.to_bits() == want || (g.is_nan() && f64::from_bits(want).is_nan());
        be += !same(ext(&args)) as usize;
        bd += !same(dbl(&args)) as usize;
    }
    println!("{:<26} records {:>8}  mismatches: register value used {:>5} | rounded to double first {:>5}", format!("{dir}/{name}"), n, be, bd);
    assert!(be * 1_000_000 <= 200 * n, "{dir}/{name}: the register-value model misses {be} of {n}");
}

#[test]
fn x87_crt_functions_approximate() {
    // x87 routines of the C runtime (not used by the game): correctly rounded values are returned; reported, not required to be exact
    for (name, f) in [("Sinh", m::sinh as fn(f64) -> f64), ("Cosh", m::cosh), ("Tanh", m::tanh)] {
        if let Some(st) = run("x87_more", name, &|a| f(a[0])) {
            assert!(st.max_ulp <= 1.0 || st.max_ulp.is_infinite(), "{name}: more than one ulp apart");
        }
    }
}

// ---------------------------------------------------------------------------------------------------- float-cast variants (X87F)
fn ftable(name: &str, f: &dyn Fn(&[f32]) -> f32, exact: bool) {
    let p = root().join("x87_more").join(format!("{name}.bin"));
    let Ok(b) = std::fs::read(&p) else {
        println!("SKIPPED x87_more/{name}: {} not found", p.display());
        return;
    };
    assert_eq!(&b[0..4], b"X87F");
    let nargs = u32::from_le_bytes(b[8..12].try_into().unwrap()) as usize;
    let w: Vec<u32> = b[20..].chunks_exact(4).map(|c| u32::from_le_bytes(c.try_into().unwrap())).collect();
    let (mut n, mut bad) = (0, 0);
    let mut args = vec![0f32; nargs];
    for rec in w.chunks_exact(nargs + 1) {
        for i in 0..nargs {
            args[i] = f32::from_bits(rec[i]);
        }
        n += 1;
        let g = f(&args);
        if g.to_bits() != rec[nargs] && !(g.is_nan() && f32::from_bits(rec[nargs]).is_nan()) {
            bad += 1;
            if bad <= 4 {
                println!("    {name}{args:?} = {:08x}, reference {:08x}", g.to_bits(), rec[nargs]);
            }
        }
    }
    println!("{:<26} records {:>8}  exact {:>8}  mismatches {:>5}", format!("x87_more/{name}"), n, n - bad, bad);
    if exact {
        assert_eq!(bad, 0, "{name}");
    } else {
        assert!(bad * 1_000_000 <= 100 * n, "{name}");
    }
}

#[test]
fn float_cast_variants() {
    ftable("SinF", &|a| m::sin_f32(a[0] as f64), false);
    ftable("CosF", &|a| m::cos_f32(a[0] as f64), false);
    ftable("Atan2F", &|a| m::atan2_f32(a[0] as f64, a[1] as f64), false);
    ftable("SqrtF", &|a| m::sqrt(a[0] as f64) as f32, true);
    ftable("PowF", &|a| m::pow(a[0] as f64, a[1] as f64) as f32, true);
    // Math.Abs(float) of a float product: the product is NOT rounded to float before fabs; the caller's store rounds once
    ftable("AbsMulF", &|a| (a[0] as f64 * a[1] as f64).abs() as f32, true);
}

// ---------------------------------------------------------------------------------------------------- dense tables (X87D)
struct Rng(u64);
impl Rng {
    fn u01(&mut self) -> f64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        (self.0.wrapping_mul(0x2545F4914F6CDD1D) >> 11) as f64 * (1.0 / 9007199254740992.0)
    }
}
fn dense(name: &str, f: &dyn Fn(f64, f64) -> F80, exact: bool) {
    let p = root().join("x87_dense").join(format!("{name}.bin"));
    let Ok(b) = std::fs::read(&p) else {
        println!("SKIPPED x87_dense/{name}: {} not found", p.display());
        return;
    };
    assert_eq!(&b[0..4], b"X87D");
    let u32at = |o: usize| u32::from_le_bytes(b[o..o + 4].try_into().unwrap());
    let u64at = |o: usize| u64::from_le_bytes(b[o..o + 8].try_into().unwrap());
    let (nargs, count, mode) = (u32at(8) as usize, u32at(12) as usize, u32at(16));
    let mut rng = Rng(u64at(20));
    let (lo0, hi0, lo1, hi1) = (f64::from_bits(u64at(28)), f64::from_bits(u64at(36)), f64::from_bits(u64at(44)), f64::from_bits(u64at(52)));
    let want_fnv = u64at(60);
    let mut h = 0xcbf29ce484222325u64;
    let mut fnv = |v: u64| {
        for i in 0..8 {
            h ^= (v >> (8 * i)) & 0xff;
            h = h.wrapping_mul(0x100000001b3);
        }
    };
    let mut st = Stat::default();
    let mut unexplained = 0;
    for i in 0..count {
        let mut a = lo0 + (hi0 - lo0) * rng.u01();
        let mut c = 0.0;
        if nargs == 2 {
            c = lo1 + (hi1 - lo1) * rng.u01();
        }
        if mode == 1 {
            a = a as f32 as f64;
            c = c as f32 as f64;
        }
        fnv(a.to_bits());
        if nargs == 2 {
            fnv(c.to_bits());
        }
        let want = u64at(68 + 8 * i);
        let v = f(a, c);
        st.add(&[a, c][..nargs], v.to_f64().to_bits(), want);
        if v.to_f64().to_bits() != want && !neighbours(v).iter().any(|w| w.to_f64().to_bits() == want) {
            unexplained += 1;
        }
    }
    let _ = &mut fnv;
    assert_eq!(h, want_fnv, "x87_dense/{name}: regenerated inputs differ from the harness inputs");
    st.report(&format!("x87_dense/{name}"));
    if exact {
        assert_eq!(st.bad, 0, "x87_dense/{name}");
    } else {
        assert_eq!(unexplained, 0, "x87_dense/{name}: mismatches that are not a last-bit difference of the 64-bit result");
        assert!(st.bad * 1_000_000 <= 100 * st.n, "x87_dense/{name}: {} mismatches", st.bad);
    }
}

#[test]
fn dense_tables() {
    let d = |x: f64| F80::from_f64(x);
    dense("Sin_m10_10", &|a, _| m::sin_ext(a), false);
    dense("Sin_0_1000", &|a, _| m::sin_ext(a), false);
    dense("Sin_f32_m7_7", &|a, _| m::sin_ext(a), false);
    dense("Cos_m10_10", &|a, _| m::cos_ext(a), false);
    dense("Cos_0_1000", &|a, _| m::cos_ext(a), false);
    dense("Cos_f32_m7_7", &|a, _| m::cos_ext(a), false);
    dense("Tan_m10_10", &|a, _| m::tan_ext(a), false);
    dense("Atan_m100_100", &|a, _| m::atan_ext(a), false);
    dense("Atan2_m1000_1000", &|a, b| m::atan2_ext(a, b), false);
    dense("Atan2_f32_m1000_1000", &|a, b| m::atan2_ext(a, b), false);
    dense("Pow_0_100_m5_5", &|a, b| d(m::pow(a, b)), true);
    dense("Pow_f32_0_10_0_4", &|a, b| d(m::pow(a, b)), true);
    dense("Log_0_1000", &|a, _| d(m::log(a)), true);
    dense("Log10_0_1000", &|a, _| d(m::log10(a)), true);
    dense("Exp_m20_20", &|a, _| d(m::exp(a)), true);
    dense("Acos_m1_1", &|a, _| d(m::acos(a)), true);
    dense("Asin_m1_1", &|a, _| d(m::asin(a)), true);
}

#[test]
fn dense_tables_approximate() {
    // reported only (x87 routines of the C runtime, not used by the game)
    for (name, f) in [("Sinh_m10_10", m::sinh as fn(f64) -> f64), ("Cosh_m10_10", m::cosh), ("Tanh_m10_10", m::tanh)] {
        let p = root().join("x87_dense").join(format!("{name}.bin"));
        let Ok(b) = std::fs::read(&p) else { continue };
        let count = u32::from_le_bytes(b[12..16].try_into().unwrap()) as usize;
        let mut rng = Rng(u64::from_le_bytes(b[20..28].try_into().unwrap()));
        let mut st = Stat::default();
        for i in 0..count.min(60000) {
            let a = -10.0 + 20.0 * rng.u01();
            st.add(&[a], f(a).to_bits(), u64::from_le_bytes(b[68 + 8 * i..76 + 8 * i].try_into().unwrap()));
        }
        st.report(&format!("x87_dense/{name} (first 60000)"));
    }
}

// ---------------------------------------------------------------------------------------------------- raw register values (X87R)
#[test]
fn raw_register_values() {
    for (name, nargs) in [("Fsin", 1usize), ("Fcos", 1), ("Fptan", 1), ("Fpatan1", 1), ("Fpatan", 2)] {
        let p = root().join("x87_raw").join(format!("{name}.bin"));
        let Ok(b) = std::fs::read(&p) else {
            println!("SKIPPED x87_raw/{name}");
            continue;
        };
        let count = u32::from_le_bytes(b[12..16].try_into().unwrap()) as usize;
        let rs = 8 * nargs + 18;
        let (mut same80, mut off1, mut other, mut bad53) = (0, 0, 0, 0);
        let limit = if nargs == 1 && name != "Fptan" && name != "Fpatan1" { count } else { count.min(60000) };
        for i in 0..limit {
            let o = 20 + i * rs;
            let a0 = f64::from_bits(u64::from_le_bytes(b[o..o + 8].try_into().unwrap()));
            let a1 = if nargs == 2 { f64::from_bits(u64::from_le_bytes(b[o + 8..o + 16].try_into().unwrap())) } else { 0.0 };
            let raw: [u8; 10] = b[o + 8 * nargs..o + 8 * nargs + 10].try_into().unwrap();
            let clr = u64::from_le_bytes(b[o + 8 * nargs + 10..o + 8 * nargs + 18].try_into().unwrap());
            let v = match name {
                "Fsin" => m::sin_ext(a0),
                "Fcos" => m::cos_ext(a0),
                "Fptan" => m::tan_ext(a0),
                "Fpatan1" => m::atan_ext(a0),
                _ => m::atan2_ext(a0, a1),
            };
            if v.to_bytes() == raw {
                same80 += 1;
            } else if neighbours(v).iter().any(|w| w.to_bytes() == raw) {
                off1 += 1;
            } else if !(v.kind == Kind::Nan && raw[9] & 0x7f == 0x7f) && !(name == "Fpatan" && a0.is_infinite() && a1.is_infinite()) {
                other += 1;
                if other <= 5 {
                    println!("    {name}({a0:?}, {a1:?}): model {:02x?} raw {:02x?}", v.to_bytes(), raw);
                }
            }
            bad53 += (v.to_f64().to_bits() != clr) as usize;
        }
        println!(
            "x87_raw/{name:<8} records {limit:>7}: 80-bit result identical {same80:>7} ({:.3}%), one unit of the last place apart {off1:>5}, other {other}; System.Math double differs {bad53}",
            100.0 * same80 as f64 / limit as f64
        );
        assert_eq!(other, 0, "{name}: register values further than one unit apart");
    }
}

// ---------------------------------------------------------------------------------------------------- speed
#[test]
fn speed_report() {
    let mut rng = Rng(0x9E3779B97F4A7C15);
    let xs: Vec<f64> = (0..200_000).map(|_| -10.0 + 20.0 * rng.u01()).collect();
    let ys: Vec<f64> = (0..200_000).map(|_| 1000.0 * rng.u01()).collect();
    let time = |label: &str, f: &dyn Fn(f64) -> f64, v: &[f64]| {
        let t = std::time::Instant::now();
        let mut acc = 0.0;
        for _ in 0..5 {
            for &x in v {
                acc += f(x);
            }
        }
        let ns = t.elapsed().as_nanos() as f64 / (5 * v.len()) as f64;
        println!("{label:<34} {ns:>8.1} ns/call   (checksum {acc:e})");
    };
    time("sin, |x| <= 10", &m::sin, &xs);
    time("cos, |x| <= 10", &m::cos, &xs);
    time("sin, 0 <= x <= 1000", &m::sin, &ys);
    time("cos, 0 <= x <= 1000", &m::cos, &ys);
    time("std f64::sin (for scale)", &f64::sin, &xs);
    let small = &xs[..20000];
    time("tan, |x| <= 10", &m::tan, small);
    time("atan, |x| <= 10", &m::atan, small);
    time("atan2(x, 3.7)", &|x| m::atan2(x, 3.7), small);
    time("pow(|x|, 1.5)", &|x| m::pow(x.abs(), 1.5), &xs);
    time("log(|x|)", &|x| m::log(x.abs()), &xs);
    time("acos(x/10)", &|x| m::acos(x / 10.0), &xs);
    time("sqrt(|x|)", &|x| m::sqrt(x.abs()), &xs);
}
