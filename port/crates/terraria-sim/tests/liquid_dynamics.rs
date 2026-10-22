//! `Liquid.UpdateLiquid` dynamics against the real game.
//!
//! `tools/refdump_liquid` (RefDumpLiquid.exe <world> <outdir> <scenario>) loads `ref_small.wld` with the settle of `liquid_load_settle.rs`
//! (`Main.rand = new UnifiedRandom(12345)` before it), then: `Main.netMode = 0`, `WorldGen.generatingWorld = false`,
//! `Main.rand = new UnifiedRandom(90210)`; the scenario's operations change tiles (deactivate a block, or fill air with liquid 255); then what
//! `WorldGen.TileFrame` does after such a change is imitated (`Liquid.AddWater(x, y)` for every tile with liquid in the operations' rectangle grown
//! by 1, x outer, y inner); then the game's own `Liquid.UpdateLiquid()` is called 600 times. Before the first call and after every 60 calls the
//! region around the operations is dumped (type, liquid, liquid type, checkingLiquid / skipLiquid bits, sTileHeader of every tile), together with
//! `numLiquid`, `numLiquidBuffer`, `wetCounter` and a hash of the whole `Main.liquid` list. This test replays the same and compares exactly.
//!
//! `Main.DoUpdate` -> `WorldGen.UpdateWorld` calls `UpdateLiquid` every second tick under `Main.SwapRandom("UpdateLiquid")`; here every call is made
//! with the one seeded generator on both sides, which exercises the same method. Not exercised: tile changes by liquids meeting (obsidian,
//! honey / shimmer blocks: `WorldGen.PlaceTile` / `KillTile` are not ported; the scenarios keep different liquids apart) and plants killed by
//! water or lava (`WorldGen.KillTile`); a mismatch of a tile's type would show it.

mod liquid_common;

use liquid_common::{list_hash, raw_game, root, SETTLE_SEED};
use terraria_core::UnifiedRandom;
use terraria_sim::liquid;

const SEED_DYNAMICS: i32 = 90210;
const SNAPSHOT_EVERY: i32 = 60;
const MARGIN: i32 = 20;

/// `Op { kind, x0, x1, y0, y1, liquid_type }`, mirrored from `Scenarios` in RefDumpLiquid.cs
struct Op(char, i32, i32, i32, i32, u8);

fn scenario_ops(name: &str) -> Vec<Op> {
    match name {
        "lake_drain" => vec![Op('D', 2472, 2475, 262, 270, 0)],
        "surface_fill" => vec![Op('F', 2091, 2094, 222, 226, 0)],
        "shimmer_drain" => vec![Op('D', 215, 218, 463, 468, 0)],
        "honey_drain" => vec![Op('D', 309, 312, 650, 655, 0)],
        "lava_open" => vec![Op('D', 2813, 2815, 1185, 1189, 0)],
        _ => panic!("unknown scenario {name}"),
    }
}

struct Reader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl Reader<'_> {
    fn i32(&mut self) -> i32 {
        let v = i32::from_le_bytes(self.data[self.pos..self.pos + 4].try_into().unwrap());
        self.pos += 4;
        v
    }
    fn u64(&mut self) -> u64 {
        let v = u64::from_le_bytes(self.data[self.pos..self.pos + 8].try_into().unwrap());
        self.pos += 8;
        v
    }
    fn bytes(&mut self, n: usize) -> &[u8] {
        let v = &self.data[self.pos..self.pos + n];
        self.pos += n;
        v
    }
}

fn run(name: &str) {
    let path = root().join(format!("refdata/liquid/{name}.bin"));
    let Ok(dump) = std::fs::read(&path) else {
        eprintln!("SKIP: need {} (run tools\\refdump_liquid\\run.cmd)", path.display());
        return;
    };
    let Some(mut g) = raw_game() else { return };

    // the start: the settle with the fixed generator
    g.main.rand = UnifiedRandom::new(SETTLE_SEED);
    liquid::load_world_settle(&mut g);
    liquid::load_world_settle_finish(&mut g);

    // the scenario
    g.main.net_mode = 0;
    g.world_gen.generating_world = false;
    g.main.rand = UnifiedRandom::new(SEED_DYNAMICS);
    let ops = scenario_ops(name);
    let (mut minx, mut miny, mut maxx, mut maxy) = (i32::MAX, i32::MAX, i32::MIN, i32::MIN);
    for Op(kind, x0, x1, y0, y1, t) in &ops {
        minx = minx.min(*x0);
        maxx = maxx.max(*x1);
        miny = miny.min(*y0);
        maxy = maxy.max(*y1);
        for x in *x0..=*x1 {
            for y in *y0..=*y1 {
                let tile = g.main.tile.get_mut(x, y);
                if *kind == 'D' {
                    tile.set_active(false);
                } else {
                    tile.liquid = 255;
                    tile.set_liquid_type(*t);
                }
            }
        }
    }
    for x in minx - 1..=maxx + 1 {
        for y in miny - 1..=maxy + 1 {
            if g.main.tile.get(x, y).liquid > 0 {
                liquid::add_water(&mut g, x, y);
            }
        }
    }

    // the dump
    let mut r = Reader { data: &dump, pos: 0 };
    assert_eq!(r.bytes(4), b"LIQD");
    assert_eq!(r.i32(), 1);
    let (rx0, ry0, rw, rh) = (r.i32(), r.i32(), r.i32(), r.i32());
    let snapshots = r.i32();
    assert_eq!(r.i32(), SEED_DYNAMICS);
    assert_eq!((rx0, ry0), ((minx - MARGIN).max(0), (miny - MARGIN).max(0)), "region of the dump");

    let mut bad_snapshots = Vec::new();
    let mut first_report = None;
    for s in 0..snapshots {
        let calls = r.i32();
        assert_eq!(calls, s * SNAPSHOT_EVERY);
        let want_num = r.i32();
        let want_buf = r.i32();
        let want_wet = r.i32();
        let want_hash = r.u64();
        let mut tile_diffs = 0u32;
        let mut first_tile = None;
        for x in rx0..rx0 + rw {
            for y in ry0..ry0 + rh {
                let rec = r.bytes(8).to_vec();
                let t = g.main.tile.get(x, y);
                let have = {
                    let mut b = [0u8; 8];
                    b[0..2].copy_from_slice(&t.type_.to_le_bytes());
                    b[2] = t.liquid;
                    b[3] = t.b_tile_header & 0x60;
                    b[4] = t.b_tile_header3 & 0x18;
                    b[5..7].copy_from_slice(&t.s_tile_header.to_le_bytes());
                    b
                };
                if rec != have {
                    tile_diffs += 1;
                    if first_tile.is_none() {
                        first_tile = Some((x, y, rec.clone(), have));
                    }
                }
            }
        }
        let have_hash = list_hash(&g);
        let list_ok = want_num == g.liquid.num_liquid && want_buf == g.liquid.num_liquid_buffer && want_wet == g.liquid.wet_counter_for_tests() && want_hash == have_hash;
        if tile_diffs > 0 || !list_ok {
            bad_snapshots.push(calls);
            if first_report.is_none() {
                first_report = Some(format!(
                    "after {calls} UpdateLiquid calls: {tile_diffs} tiles differ (first {first_tile:?}); numLiquid game {want_num} port {}, numLiquidBuffer game {want_buf} port {}, wetCounter game {want_wet} port {}, list hash equal: {}",
                    g.liquid.num_liquid,
                    g.liquid.num_liquid_buffer,
                    g.liquid.wet_counter_for_tests(),
                    want_hash == have_hash
                ));
            }
        }
        if s < snapshots - 1 {
            for _ in 0..SNAPSHOT_EVERY {
                liquid::update_liquid(&mut g);
            }
        }
    }
    let want_next = r.i32();
    let have_next = g.main.rand.next_raw();
    eprintln!("[{name}] snapshots compared: {snapshots}; snapshots with a difference: {bad_snapshots:?}; next Main.rand value: game {want_next}, port {have_next}");
    if let Some(msg) = &first_report {
        eprintln!("[{name}] first difference {msg}");
    }
    assert!(bad_snapshots.is_empty(), "[{name}] {}", first_report.unwrap_or_default());
    assert_eq!(want_next, have_next, "[{name}] Main.rand state after the run");
}

#[test]
fn lake_drain_matches_the_real_game() {
    run("lake_drain");
}

#[test]
fn surface_fill_matches_the_real_game() {
    run("surface_fill");
}

#[test]
fn shimmer_drain_matches_the_real_game() {
    run("shimmer_drain");
}

#[test]
fn honey_drain_matches_the_real_game() {
    run("honey_drain");
}

#[test]
fn lava_open_matches_the_real_game() {
    run("lava_open");
}
