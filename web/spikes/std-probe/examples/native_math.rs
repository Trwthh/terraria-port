//! The same three libm calls as src/lib.rs, natively (x86_64-pc-windows-gnu), for comparison.
use std::hint::black_box;
fn main() {
    let s = black_box(1.0e6f64).sin().to_bits();
    let p = black_box(1.7f64).powf(black_box(33.3)).to_bits();
    let e = black_box(0.3f64).exp().to_bits();
    println!("sin_bits_hi 0x{:x} sin_bits_lo 0x{:x} pow_bits_lo 0x{:x} exp_bits_lo 0x{:x}", s >> 32, s & 0xffff_ffff, p & 0xffff_ffff, e & 0xffff_ffff);
}
