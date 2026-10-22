//! Shared helpers of the `Terraria.Collision` tests: the reference world exactly as the game had it when
//! `tools/refdump_collision` ran, the reader for its case files and the comparison with the port.
//!
//! File formats are documented at the top of `tools/refdump_collision/Cases.cs` (cases) and `Stamps.cs` (stamps).
#![allow(dead_code)]

use std::collections::BTreeMap;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::PathBuf;

use terraria_core::xna::{Point, Rectangle, Vector2, Vector4};
use terraria_core::{Tile, UnifiedRandom};
use terraria_sim::io::world_file::load_world;
use terraria_sim::Game;

pub const RECORD: usize = 14;

pub fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..")
}

pub fn dump_dir() -> PathBuf {
    root().join("refdata/collision")
}

fn read_tile(rec: &[u8]) -> Tile {
    let u16_at = |o: usize| u16::from_le_bytes([rec[o], rec[o + 1]]);
    Tile {
        type_: u16_at(0),
        wall: u16_at(2),
        liquid: rec[4],
        s_tile_header: u16_at(5),
        b_tile_header: rec[7],
        b_tile_header2: rec[8],
        b_tile_header3: rec[9],
        frame_x: u16_at(10) as i16,
        frame_y: u16_at(12) as i16,
    }
}

fn write_tile(t: &Tile, out: &mut [u8]) {
    out[0..2].copy_from_slice(&t.type_.to_le_bytes());
    out[2..4].copy_from_slice(&t.wall.to_le_bytes());
    out[4] = t.liquid;
    out[5..7].copy_from_slice(&t.s_tile_header.to_le_bytes());
    out[7] = t.b_tile_header;
    out[8] = t.b_tile_header2;
    out[9] = t.b_tile_header3;
    out[10..12].copy_from_slice(&t.frame_x.to_le_bytes());
    out[12..14].copy_from_slice(&t.frame_y.to_le_bytes());
}

/// FNV-1a 64 over the 14-byte records of each column (`ColumnHashes` of RefDumpCollision.cs).
fn column_hashes(g: &Game) -> Vec<u64> {
    let (w, h) = (g.main.tile.width, g.main.tile.height);
    let mut rec = [0u8; RECORD];
    (0..w)
        .map(|x| {
            let mut hash: u64 = 14695981039346656037;
            for y in 0..h {
                write_tile(g.main.tile.get(x, y), &mut rec);
                for &b in &rec {
                    hash ^= b as u64;
                    hash = hash.wrapping_mul(1099511628211);
                }
            }
            hash
        })
        .collect()
}

/// The reference world in the state the real game had it when the cases were recorded:
/// `ref_small.wld` through the port's loader, the liquid state of `WorldFile.LoadWorld`'s settle pass taken from
/// `refdata/framed/ref_small.liquid.bin` (the settle is not ported; see tile_frame_reference_world.rs), then the
/// harness' stamps; verified against the per-column hashes the harness wrote. `None` (with a message) when a file is missing.
pub fn reference_game() -> Option<Game> {
    let root = root();
    let wld = root.join("work/refsave/Worlds/ref_small.wld");
    let liquid_path = root.join("refdata/framed/ref_small.liquid.bin");
    let stamps_path = dump_dir().join("stamps.bin");
    let cols_path = dump_dir().join("world_columns.bin");
    let (Ok(wld_bytes), Ok(liquid), Ok(stamps), Ok(cols)) =
        (std::fs::read(&wld), std::fs::read(&liquid_path), std::fs::read(&stamps_path), std::fs::read(&cols_path))
    else {
        eprintln!(
            "SKIP: need {}, {}, {} and {} (run tools\\refdump_collision\\run.cmd and tools\\refdump)",
            wld.display(),
            liquid_path.display(),
            stamps_path.display(),
            cols_path.display()
        );
        return None;
    };
    let world = terraria_formats::wld::read_world(&wld_bytes).expect("read ref_small.wld");
    let mut g = Game::new();
    let _world = load_world(&mut g, world);
    let (w, h) = (g.main.tile.width, g.main.tile.height);
    for x in 0..w {
        for y in 0..h {
            let t = g.main.tile.get_mut(x, y);
            t.liquid = 0;
            t.b_tile_header &= !0x60;
            t.b_tile_header3 &= !0x18;
        }
    }
    for rec in liquid.chunks_exact(8) {
        let idx = i32::from_le_bytes(rec[0..4].try_into().unwrap());
        let (x, y) = (idx / h, idx % h);
        let t = g.main.tile.get_mut(x, y);
        t.liquid = rec[4];
        t.b_tile_header |= rec[5];
        t.b_tile_header3 |= rec[6];
    }
    assert_eq!(&stamps[0..4], b"STMP");
    let n = i32::from_le_bytes(stamps[4..8].try_into().unwrap()) as usize;
    assert_eq!(stamps.len(), 8 + n * (8 + RECORD));
    for k in 0..n {
        let o = 8 + k * (8 + RECORD);
        let x = i32::from_le_bytes(stamps[o..o + 4].try_into().unwrap());
        let y = i32::from_le_bytes(stamps[o + 4..o + 8].try_into().unwrap());
        *g.main.tile.get_mut(x, y) = read_tile(&stamps[o + 8..o + 8 + RECORD]);
    }
    let mine = column_hashes(&g);
    let theirs: Vec<u64> = cols.chunks_exact(8).map(|c| u64::from_le_bytes(c.try_into().unwrap())).collect();
    let bad: Vec<usize> = (0..w as usize).filter(|&x| mine[x] != theirs[x]).collect();
    assert!(bad.is_empty(), "{} columns differ from the tiles the game had, first: {:?}", bad.len(), &bad[..bad.len().min(10)]);
    Some(g)
}

// ---------------------------------------------------------------- case files

pub struct Case {
    pub threw: bool,
    pub inputs: Vec<u32>,
    pub outputs: Vec<u32>,
}

pub fn read_cases(family: &str) -> Vec<Case> {
    let path = dump_dir().join(format!("{family}.bin"));
    let d = std::fs::read(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    assert_eq!(&d[0..4], b"CLSN");
    assert_eq!(u32::from_le_bytes(d[4..8].try_into().unwrap()), 1);
    let mut p = 8;
    let mut cases = Vec::new();
    let word = |p: usize| u32::from_le_bytes(d[p..p + 4].try_into().unwrap());
    while p < d.len() {
        let threw = d[p] != 0;
        let n_in = word(p + 1) as usize;
        let n_out = word(p + 5) as usize;
        p += 9;
        let inputs = (0..n_in).map(|k| word(p + k * 4)).collect();
        p += n_in * 4;
        let outputs = (0..n_out).map(|k| word(p + k * 4)).collect();
        p += n_out * 4;
        cases.push(Case { threw, inputs, outputs });
    }
    cases
}

/// Reads the input words of a case.
pub struct R<'a> {
    w: &'a [u32],
    p: usize,
}

impl<'a> R<'a> {
    pub fn new(w: &'a [u32]) -> Self {
        Self { w, p: 0 }
    }
    pub fn u32(&mut self) -> u32 {
        let v = self.w[self.p];
        self.p += 1;
        v
    }
    pub fn i32(&mut self) -> i32 {
        self.u32() as i32
    }
    pub fn bool(&mut self) -> bool {
        self.u32() != 0
    }
    pub fn f32(&mut self) -> f32 {
        f32::from_bits(self.u32())
    }
    pub fn v2(&mut self) -> Vector2 {
        let x = self.f32();
        let y = self.f32();
        Vector2::new(x, y)
    }
    pub fn pt(&mut self) -> Point {
        let x = self.i32();
        let y = self.i32();
        Point::new(x, y)
    }
    pub fn rect(&mut self) -> Rectangle {
        let x = self.i32();
        let y = self.i32();
        let w = self.i32();
        let h = self.i32();
        Rectangle::new(x, y, w, h)
    }
    pub fn pts(&mut self) -> Vec<Point> {
        let n = self.u32();
        (0..n).map(|_| self.pt()).collect()
    }
    pub fn done(&self) -> bool {
        self.p == self.w.len()
    }
}

/// Output words with a mark for the float ones (for the ulp analysis of a mismatch).
#[derive(Default)]
pub struct Out {
    pub w: Vec<u32>,
    pub is_f: Vec<bool>,
}

impl Out {
    pub fn i(&mut self, v: i32) -> &mut Self {
        self.w.push(v as u32);
        self.is_f.push(false);
        self
    }
    pub fn b(&mut self, v: bool) -> &mut Self {
        self.w.push(v as u32);
        self.is_f.push(false);
        self
    }
    pub fn f(&mut self, v: f32) -> &mut Self {
        self.w.push(v.to_bits());
        self.is_f.push(true);
        self
    }
    pub fn v2(&mut self, v: Vector2) -> &mut Self {
        self.f(v.x).f(v.y)
    }
    pub fn v4(&mut self, v: Vector4) -> &mut Self {
        self.f(v.x).f(v.y).f(v.z).f(v.w)
    }
    pub fn pt(&mut self, p: Point) -> &mut Self {
        self.i(p.x).i(p.y)
    }
    pub fn rect(&mut self, r: Rectangle) -> &mut Self {
        self.i(r.x).i(r.y).i(r.width).i(r.height)
    }
    pub fn pts(&mut self, l: &[Point]) -> &mut Self {
        self.i(l.len() as i32);
        for p in l {
            self.pt(*p);
        }
        self
    }
    pub fn floats(&mut self, l: &[f32]) -> &mut Self {
        self.i(l.len() as i32);
        for v in l {
            self.f(*v);
        }
        self
    }
    /// The eight static flags of `Collision` packed into one word, bit 0..7 = up, down, stair, stairFall, honey, shimmer,
    /// sloping, landMine.
    pub fn flags(&mut self, g: &Game) -> &mut Self {
        let c = &g.collision;
        let bits = [c.up, c.down, c.stair, c.stair_fall, c.honey, c.shimmer, c.sloping, c.land_mine];
        let mut w = 0i32;
        for (k, b) in bits.iter().enumerate() {
            if *b {
                w |= 1 << k;
            }
        }
        self.i(w)
    }
}

/// Distance in representable floats between two bit patterns (sign-magnitude mapped to a line).
pub fn ulp_distance(a: u32, b: u32) -> u64 {
    fn key(x: u32) -> i64 {
        if x & 0x8000_0000 != 0 {
            -((x & 0x7FFF_FFFF) as i64)
        } else {
            x as i64
        }
    }
    (key(a) - key(b)).unsigned_abs()
}

#[derive(Default, Clone)]
pub struct Stats {
    pub cases: u64,
    pub exact: u64,
    pub game_threw: u64,
    pub both_threw: u64,
    pub mismatch: u64,
    /// mismatching cases in which every differing word is a float
    pub float_only: u64,
    pub max_ulp: u64,
    pub examples: Vec<String>,
}

pub type Runner<'a> = &'a dyn Fn(&mut Game, u32, &mut R) -> Out;

/// Replays every case of `family` through `run` and compares bit for bit. The state before each case is the one
/// the harness sets: all `Collision` flags false, `Main.rand = new UnifiedRandom(seed)`, `Main.netMode = 0`,
/// `Main.getGoodWorld = Main.dontStarveWorld = false`. A call of the port that unwinds (the original throws) counts as a throw.
pub fn replay(g: &mut Game, family: &str, run: Runner) -> Stats {
    replay_opts(g, family, false, run)
}

/// [`replay`]; with `ignore_flags` the last output word (the packed `Collision` flags) is not compared. Needed for `SwitchTiles`,
/// whose unported callees (`Wiring.HitSwitch` -> projectiles, ...) call `Collision.WetCollision` & co. in the game and so set flags
/// that the port, which does not run them, cannot set.
pub fn replay_opts(g: &mut Game, family: &str, ignore_flags: bool, run: Runner) -> Stats {
    let mut st = Stats::default();
    for (idx, case) in read_cases(family).iter().enumerate() {
        st.cases += 1;
        g.collision = terraria_sim::collision::CollisionState::new();
        g.main.net_mode = 0;
        g.main.get_good_world = false;
        g.main.dont_starve_world = false;
        let mut r = R::new(&case.inputs);
        let seed = r.u32();
        g.main.rand = UnifiedRandom::new(seed as i32);
        let result = catch_unwind(AssertUnwindSafe(|| run(g, seed, &mut r)));
        if case.threw {
            st.game_threw += 1;
        }
        match (case.threw, result) {
            (true, Err(_)) => {
                st.both_threw += 1;
                st.exact += 1;
            }
            (false, Ok(out)) => {
                assert!(r.done(), "{family}: case {idx} did not consume all its input words");
                let same = if ignore_flags { out.w[..out.w.len() - 1] == case.outputs[..case.outputs.len() - 1] } else { out.w == case.outputs };
                if same {
                    st.exact += 1;
                } else {
                    st.mismatch += 1;
                    let mut float_only = out.w.len() == case.outputs.len();
                    let mut worst = 0u64;
                    if float_only {
                        for k in 0..out.w.len() {
                            if out.w[k] != case.outputs[k] {
                                if out.is_f[k] {
                                    worst = worst.max(ulp_distance(out.w[k], case.outputs[k]));
                                } else {
                                    float_only = false;
                                }
                            }
                        }
                    }
                    if float_only {
                        st.float_only += 1;
                        st.max_ulp = st.max_ulp.max(worst);
                    }
                    if st.examples.len() < 3 {
                        st.examples.push(format!(
                            "case {idx}: inputs {:?}\n      game {:?}\n      port {:?}",
                            case.inputs, case.outputs, out.w
                        ));
                    }
                }
            }
            (true, Ok(out)) => {
                st.mismatch += 1;
                if st.examples.len() < 3 {
                    st.examples.push(format!("case {idx}: the game threw, the port returned {:?}; inputs {:?}", out.w, case.inputs));
                }
            }
            (false, Err(_)) => {
                st.mismatch += 1;
                if st.examples.len() < 3 {
                    st.examples.push(format!("case {idx}: the port threw, the game returned {:?}; inputs {:?}", case.outputs, case.inputs));
                }
            }
        }
    }
    st
}

/// Prints the table of a set of families and returns the names that have any mismatch.
pub fn report(all: &BTreeMap<String, Stats>) -> Vec<String> {
    eprintln!("{:<40} {:>7} {:>7} {:>8} {:>9} {:>10} {:>8}", "family", "cases", "exact", "mismatch", "game-threw", "float-only", "max ulp");
    let mut bad = Vec::new();
    for (name, s) in all {
        eprintln!("{:<40} {:>7} {:>7} {:>8} {:>9} {:>10} {:>8}", name, s.cases, s.exact, s.mismatch, s.game_threw, s.float_only, s.max_ulp);
        if s.mismatch != 0 {
            bad.push(name.clone());
            for e in &s.examples {
                eprintln!("    {e}");
            }
        }
    }
    bad
}
