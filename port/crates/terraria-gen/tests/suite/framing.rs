//! The translated `WorldGen.SectionTileFrame` over the whole reference world against the real game
//! (`tools/refdump/RefDumpFramed.cs`; mirrored from `terraria-sim/tests/tile_frame_reference_world.rs`).
//!
//! The harness loaded the world, set `Main.netMode = 1`, `WorldGen.generatingWorld = false`,
//! `Main.rand = new UnifiedRandom(424242)`, called `WorldGen.SectionTileFrame(0, 0, 20, 7)` once and dumped every
//! tile. Two cases: the world as loaded, and with a rectangle overwritten by a pseudo-random tile pattern
//! (`ScrambleRegion`). Compared: the tiles before the run (per-column hashes), every field of all 5,040,000 tiles
//! after it, and the next `Main.rand.Next()`.

use crate::game::{self, Report, RECORD};
use std::collections::BTreeMap;
use terraria_gen::gen::terraria;
use terraria_gen::rt;

const SEED: i32 = 424242; // RefDumpFramed.cs FramedSeed

fn json_number(json: &str, key: &str) -> i64 {
    let k = format!("\"{key}\":");
    let at = json.find(&k).unwrap_or_else(|| panic!("{key} missing from the dump json")) + k.len();
    let end = json[at..].find(|c: char| c != '-' && !c.is_ascii_digit()).map_or(json.len(), |e| at + e);
    json[at..end].parse().unwrap()
}

/// `ScrambleRegion` of tools/refdump/RefDumpFramed.cs, line for line; the tables come from the translated statics.
unsafe fn scramble(w: i32, h: i32) {
    let main = terraria::main::s();
    let important = (*main).tile_frame_important;
    let vine = (*terraria::id::tile_id::sets::s()).is_vine;
    let cand: Vec<u16> = (0..important.len()).filter(|&t| !*important.at(t) && !*vine.at(t) && t != 80 && t != 49).map(|t| t as u16).collect();
    let l = cand.len() as u32;
    let mut state: u32 = 0x2545F491;
    let mut draw = || {
        state = state.wrapping_mul(1664525).wrapping_add(1013904223);
        state >> 8
    };
    for x in 810..1410.min(w) {
        for y in 860..1160.min(h) {
            let r = draw();
            let idx = (r >> 4) % (l + l / 3);
            let s = draw() % 12;
            let active = idx < l;
            let slope: u16 = if s < 4 { s as u16 + 1 } else { 0 };
            let half = s == 4;
            let t = game::tile(x, y).ptr();
            (*t).type_ = if active { cand[idx as usize] } else { 0 };
            (*t).s_tile_header = ((*t).s_tile_header & !(0x20 | 0x400 | 0x7000)) | ((active as u16) << 5) | ((half as u16) << 10) | (slope << 12);
            (*t).frame_x = -1;
            (*t).frame_y = -1;
        }
    }
}

const FIELDS: [&str; 9] = ["type", "wall", "liquid", "sTileHeader", "bTileHeader", "bTileHeader2", "bTileHeader3", "frameX", "frameY"];
const OFFSETS: [(usize, usize); 9] = [(0, 2), (2, 4), (4, 5), (5, 7), (7, 8), (8, 9), (9, 10), (10, 12), (12, 14)];

fn run_case(report: &mut Report, stem: &str, scrambled: bool) {
    let dump_dir = game::root().join("refdata/framed");
    let (Ok(post), Ok(pre_cols), Ok(json)) = (
        std::fs::read(dump_dir.join(format!("{stem}.post.bin"))),
        std::fs::read(dump_dir.join(format!("{stem}.pre_columns.bin"))),
        std::fs::read_to_string(dump_dir.join(format!("{stem}.json"))),
    ) else {
        report.fail(format!("framing {stem}: dump missing in {}", dump_dir.display()));
        return;
    };
    let Some((w, h)) = (unsafe { game::load_reference_world(report) }) else { return };
    unsafe {
        if scrambled {
            scramble(w, h);
        }
        let n = game::apply_liquid(stem);
        assert_eq!(&post[0..4], b"TFRM");
        assert_eq!((i32::from_le_bytes(post[8..12].try_into().unwrap()), i32::from_le_bytes(post[12..16].try_into().unwrap())), (w, h));
        assert_eq!(u32::from_le_bytes(post[16..20].try_into().unwrap()) as usize, RECORD);
        assert_eq!(post.len(), 20 + w as usize * h as usize * RECORD);
        // ---- the starting point must be the tiles the game had after LoadWorld
        let mine = game::column_hashes(w, h);
        let theirs: Vec<u64> = pre_cols.chunks_exact(8).map(|c| u64::from_le_bytes(c.try_into().unwrap())).collect();
        let bad: Vec<usize> = (0..w as usize).filter(|&x| mine[x] != theirs[x]).collect();
        if !bad.is_empty() {
            report.fail(format!("framing {stem}: {} tile columns differ before the run (first {:?})", bad.len(), &bad[..bad.len().min(8)]));
            return;
        }
        report.line(format!("[{stem}] world {w} x {h}: all column hashes before the run equal the game's ({n} tiles with liquid state from the dump)"));

        // ---- the same run as the harness
        let main = terraria::main::s();
        (*main).net_mode = 1;
        (*terraria::world_gen::s()).generating_world = false;
        (*main).rand = rt::alloc_init(terraria_core::UnifiedRandom::new(SEED));
        let t0 = std::time::Instant::now();
        if let Err(e) = game::run(|| terraria::world_gen::section_tile_frame(0, 0, w / 200 - 1, h / 150 - 1)) {
            report.fail(format!("framing {stem}: WorldGen.SectionTileFrame did not finish: {e}"));
            return;
        }
        let elapsed = t0.elapsed();

        // ---- compare every field of every tile
        let mut rec = [0u8; RECORD];
        let (mut exact, mut mismatch) = (0u64, 0u64);
        let mut by_field: BTreeMap<String, (u64, Vec<(i32, i32)>)> = BTreeMap::new();
        let mut by_type: BTreeMap<u16, u64> = BTreeMap::new();
        for x in 0..w {
            for y in 0..h {
                let o = 20 + (x as usize * h as usize + y as usize) * RECORD;
                let want = &post[o..o + RECORD];
                game::read_record(game::tile(x, y).ptr(), &mut rec);
                if rec == *want {
                    exact += 1;
                    continue;
                }
                mismatch += 1;
                let fields: Vec<&str> = (0..9).filter(|&k| rec[OFFSETS[k].0..OFFSETS[k].1] != want[OFFSETS[k].0..OFFSETS[k].1]).map(|k| FIELDS[k]).collect();
                let e = by_field.entry(fields.join("+")).or_default();
                e.0 += 1;
                if e.1.len() < 5 {
                    e.1.push((x, y));
                }
                *by_type.entry(u16::from_le_bytes([want[0], want[1]])).or_insert(0) += 1;
            }
        }
        let next = (*(*main).rand.ptr()).next_raw();
        let want_next = json_number(&json, "nextRandAfterRun") as i32;
        report.line(format!(
            "[{stem}] SectionTileFrame in {:.1} s: {} tiles compared (all 9 fields): {exact} exact, {mismatch} mismatches; next Main.rand.Next(): here {next}, game {want_next}",
            elapsed.as_secs_f64(),
            exact + mismatch
        ));
        for (k, (n, first)) in &by_field {
            report.line(format!("    fields {k}: {n} tiles, first {first:?}"));
        }
        if !by_type.is_empty() {
            let mut v: Vec<_> = by_type.iter().collect();
            v.sort_by_key(|(_, n)| std::cmp::Reverse(**n));
            report.line(format!("    by tile type (type: tiles): {}", v.iter().take(24).map(|(t, n)| format!("{t}: {n}")).collect::<Vec<_>>().join(", ")));
        }
        if mismatch != 0 {
            report.fail(format!("framing {stem}: {mismatch} tiles differ from the real game"));
        }
        if next != want_next {
            report.fail(format!("framing {stem}: the random generator state differs after the run ({next} vs {want_next})"));
        }
    }
}

pub fn run(report: &mut Report) {
    run_case(report, "ref_small", false);
    run_case(report, "ref_small_scrambled", true);
}
