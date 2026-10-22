//! The three probes of the first float-probe set that need the x87 maths functions (`refdata/worldgen/float_probes`, P25 / P26 / P30);
//! the other probes are scored by `tools/refdump_worldgen/float_model/score.py`.
use std::path::PathBuf;
use terraria_x87 as m;

#[test]
fn trig_probes_of_the_first_set() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../refdata/worldgen/float_probes");
    let (Ok(b), Ok(js)) = (std::fs::read(dir.join("probes.bin")), std::fs::read_to_string(dir.join("probes.json"))) else {
        println!("SKIPPED: {} not found", dir.display());
        return;
    };
    let n = u32::from_le_bytes(b[12..16].try_into().unwrap()) as usize;
    let names: Vec<&str> = js.split("\"name\":\"").skip(1).map(|s| s.split('"').next().unwrap()).collect();
    let f = |o: usize| f32::from_le_bytes(b[o..o + 4].try_into().unwrap()) as f64;
    let block = |name: &str| 20 + n * 16 + names.iter().position(|x| *x == name).unwrap() * n * 8;
    let probes: [(&str, &dyn Fn(f64, f64, f64) -> f32); 3] = [
        ("P25_trig_cast", &|x, y, z| (m::cos_f32(x) as f64 * y + m::sin_f32(x) as f64 * z) as f32),
        ("P26_atan2_cast", &|x, y, z| (m::atan2_f32(y, x) as f64 * z) as f32),
        ("P30_pow_cast", &|x, y, _| ((m::pow(x.abs(), 1.5) as f32) as f64 * y) as f32),
    ];
    for (name, model) in probes {
        let base = block(name);
        let mut bad = 0;
        for i in 0..n {
            let (x, y, z) = (f(20 + i * 16), f(24 + i * 16), f(28 + i * 16));
            let want = u64::from_le_bytes(b[base + i * 8..base + i * 8 + 8].try_into().unwrap()) as u32;
            let got = model(x, y, z);
            bad += (got.to_bits() != want && !(got.is_nan() && f32::from_bits(want).is_nan())) as usize;
        }
        println!("{name}: {n} inputs, {} exact", n - bad);
        assert_eq!(bad, 0, "{name}");
    }
}
