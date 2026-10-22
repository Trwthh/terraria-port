//! The liquid settle `WorldFile.LoadWorld` runs after reading a world file (WorldFile.cs:742-773: `Liquid.QuickWater(2)`,
//! `WorldGen.WaterCheck()`, `Liquid.UpdateLiquid()` until `numLiquid == 0` with `quickSettle`, `WorldGen.WaterCheck()`), against the real game.
//!
//! Three comparisons:
//!
//! 1. `refdata/framed/ref_small.liquid.bin`: the liquid state of every tile after the game's own `LoadWorld` of `work/refsave/Worlds/ref_small.wld`
//!    as `tools/refdump` made it (layout in `tools/refdump/RefDumpFramed.cs`: 8-byte records `i32 x*height+y | u8 liquid | u8 bTileHeader & 0x60 |
//!    u8 bTileHeader3 & 0x18 | u8 0` for every tile that has any of them non-zero). That harness never ran `Main.DoUpdate`, so `Liquid.curMaxLiquid`
//!    was still 0 (its static initialiser): every liquid went to the buffer and `UpdateLiquid` had nothing to do. The Rust loader
//!    (`io::world_file::load_world`, which includes the ported settle) must reproduce `liquid`, the liquid type bits and the `checkingLiquid` /
//!    `skipLiquid` bits of every tile in that state.
//! 2. `refdata/liquid/settle_trace.liquid.bin`, `settle_trace.json`, `settle_trace.txt`: the same load in the state the real game has it in
//!    (`Liquid.cycles = 7`, `Liquid.curMaxLiquid = 25000`, see `liquid_common::raw_game`), from `tools/refdump_liquid` (RefDumpLiquid.exe
//!    settle_trace), where the settle is written out step by step and its numbers recorded after QuickWater, WaterCheck and every `UpdateLiquid` call.
//!    The ported settle must reproduce every recorded line (`numLiquid`, `numLiquidBuffer`, `wetCounter`, `stuckCount`, a hash of the whole
//!    `Main.liquid` list and a hash of a region of the tiles) and the final liquid state of every tile. `Main.rand` is `new UnifiedRandom(12345)` on
//!    both sides right before the settle (it draws from it; in the game it is clock-seeded and not reproducible).
//!
//! Skipped with a message when the world or the dumps are missing.

mod liquid_common;

use liquid_common::{raw_game, root};
use terraria_sim::io::world_file::load_world;
use terraria_sim::Game;

/// (x, y, expected, actual) of the first mismatching tiles, and the number of mismatches.
type Mismatches = (u64, Vec<(i32, i32, (u8, u8, u8), (u8, u8, u8))>);

fn compare(g: &Game, dump: &[u8]) -> Mismatches {
    let (w, h) = (g.main.tile.width, g.main.tile.height);
    let mut want = vec![(0u8, 0u8, 0u8); w as usize * h as usize];
    for rec in dump.chunks_exact(8) {
        let idx = i32::from_le_bytes(rec[0..4].try_into().unwrap()) as usize;
        want[idx] = (rec[4], rec[5], rec[6]);
    }
    let mut count = 0u64;
    let mut first = Vec::new();
    for x in 0..w {
        for y in 0..h {
            let t = g.main.tile.get(x, y);
            let have = (t.liquid, t.b_tile_header & 0x60, t.b_tile_header3 & 0x18);
            let exp = want[x as usize * h as usize + y as usize];
            if have != exp {
                count += 1;
                if first.len() < 12 {
                    first.push((x, y, exp, have));
                }
            }
        }
    }
    (count, first)
}

fn json_number(json: &str, key: &str) -> i64 {
    let k = format!("\"{key}\":");
    let at = json.find(&k).unwrap_or_else(|| panic!("{key} missing")) + k.len();
    let end = json[at..].find(|c: char| c != '-' && !c.is_ascii_digit()).map_or(json.len(), |e| at + e);
    json[at..end].parse().unwrap()
}

#[test]
fn load_world_liquid_settle_matches_the_reference_dump_of_the_framing_harness() {
    let root = root();
    let wld = root.join("work/refsave/Worlds/ref_small.wld");
    let dump = root.join("refdata/framed/ref_small.liquid.bin");
    let (Ok(bytes), Ok(dump)) = (std::fs::read(&wld), std::fs::read(&dump)) else {
        eprintln!("SKIP: need {} and {}", wld.display(), dump.display());
        return;
    };
    let world = terraria_formats::wld::read_world(&bytes).expect("read ref_small.wld");
    let mut g = Game::new(); // Liquid.curMaxLiquid = 0, cycles = 10: the static initialisers, as in that harness
    let t0 = std::time::Instant::now();
    load_world(&mut g, world);
    eprintln!("load_world (with the settle) took {:?}; numLiquid {}, numLiquidBuffer {}", t0.elapsed(), g.liquid.num_liquid, g.liquid.num_liquid_buffer);
    let (count, first) = compare(&g, &dump);
    eprintln!("tiles whose liquid state differs from the game's: {count}; first: {first:?}");
    assert_eq!(count, 0, "liquid state after load differs from the real game, first (x, y, want, have): {first:?}");
}

#[test]
fn settle_with_the_games_own_statics_matches_the_real_game_step_by_step() {
    let root = root();
    let dir = root.join("refdata/liquid");
    let (Ok(dump), Ok(json), Ok(trace)) = (
        std::fs::read(dir.join("settle_trace.liquid.bin")),
        std::fs::read_to_string(dir.join("settle_trace.json")),
        std::fs::read_to_string(dir.join("settle_trace.txt")),
    ) else {
        eprintln!("SKIP: need refdata/liquid/settle_trace.* (run tools\\refdump_liquid\\run.cmd)");
        return;
    };
    let Some(mut g) = raw_game() else { return };
    g.main.rand = terraria_core::UnifiedRandom::new(liquid_common::SETTLE_SEED);
    // the steps of `load_world_settle`, with the game's numbers recorded after each (RunSettleTrace in RefDumpLiquid.cs)
    let mut lines: Vec<String> = Vec::new();
    g.world_gen.is_generating_or_loading_world = true;
    g.liquid.water_line = g.main.max_tiles_y;
    lines.push(liquid_common::trace_line(&g, "start"));
    terraria_sim::liquid::quick_water(&mut g, 2, -1, -1);
    lines.push(liquid_common::trace_line(&g, "quickwater"));
    terraria_sim::world_gen::water_check::water_check(&mut g);
    lines.push(liquid_common::trace_line(&g, "watercheck"));
    g.liquid.quick_settle = true;
    let mut n = 0;
    while g.liquid.num_liquid > 0 && n < 100000 {
        n += 1;
        terraria_sim::liquid::update_liquid(&mut g);
        lines.push(liquid_common::trace_line(&g, &n.to_string()));
    }
    g.liquid.quick_settle = false;
    terraria_sim::world_gen::water_check::water_check(&mut g);
    g.world_gen.is_generating_or_loading_world = false;
    lines.push(liquid_common::trace_line(&g, "end"));

    let want: Vec<&str> = trace.lines().collect();
    let first_bad = (0..want.len().max(lines.len())).find(|&i| want.get(i).copied() != lines.get(i).map(|s| s.as_str()));
    eprintln!("UpdateLiquid iterations: game {}, port {n}", want.len() - 4);
    if let Some(i) = first_bad {
        eprintln!("first differing line {i}:\n  game {:?}\n  port {:?}", want.get(i), lines.get(i));
    }
    let (count, first) = compare(&g, &dump);
    eprintln!("tiles whose final liquid state differs from the game's: {count}; first: {first:?}");
    eprintln!("numLiquid: game {}, port {}", json_number(&json, "numLiquid"), g.liquid.num_liquid);
    assert_eq!(first_bad, None, "the recorded steps differ from the game's");
    assert_eq!(count, 0, "final liquid state differs from the game, first (x, y, want, have): {first:?}");
    assert_eq!(g.liquid.num_liquid as i64, json_number(&json, "numLiquid"));
}

/// `load_world` as the loader runs it, in the game's state (`cycles` 7, `curMaxLiquid` 25000): the settle is part of it and leaves
/// `quickSettle` off and `isGeneratingOrLoadingWorld` false.
#[test]
fn load_world_leaves_the_liquid_statics_as_the_game_does() {
    let root = root();
    let wld = root.join("work/refsave/Worlds/ref_small.wld");
    let Ok(bytes) = std::fs::read(&wld) else {
        eprintln!("SKIP: need {}", wld.display());
        return;
    };
    let world = terraria_formats::wld::read_world(&bytes).expect("read ref_small.wld");
    let mut g = Game::new();
    g.liquid.cycles = 7;
    g.liquid.cur_max_liquid = 25000;
    load_world(&mut g, world);
    assert!(!g.liquid.quick_settle);
    assert!(!g.world_gen.is_generating_or_loading_world);
    assert!(g.liquid.num_liquid < 100, "the settle leaves only the few entries of the closing WaterCheck, got {}", g.liquid.num_liquid);
}
