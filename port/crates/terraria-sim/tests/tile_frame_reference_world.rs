//! Whole-world check of the tile framing port against the real game.
//!
//! `tools/refdump` (`RefDump.exe framed work\refsave\Worlds\ref_small.wld refdata\framed`) loads the world with the game's
//! own `WorldFile.LoadWorld`, sets `Main.netMode = 1`, `WorldGen.generatingWorld = false`, `Main.rand = new UnifiedRandom(424242)`,
//! calls `WorldGen.SectionTileFrame(0, 0, 20, 7)` once and dumps every tile (see the header of
//! `tools/refdump/RefDumpFramed.cs` for the file layout). This test does the same through the Rust port and compares every
//! field of every tile, the tiles before the run (per-column hashes) and the next random number.
//!
//! Two cases: the world as the game made it, and the same world with a rectangle overwritten by a deterministic
//! pseudo-random tile pattern (`scramble`, mirrored in `ScrambleRegion` of RefDumpFramed.cs) so that every tile type,
//! slope and half brick goes through the framing, not only the types a small normal world contains.
//!
//! Skipped with a message when the world or the reference dump is missing.

use std::collections::BTreeMap;
use std::path::PathBuf;

use terraria_core::{Tile, UnifiedRandom};
use terraria_data::sets::tile_id;
use terraria_sim::io::world_file::load_world;
use terraria_sim::world_gen::section_tile_frame;
use terraria_sim::{unported, Game};

const SEED: i32 = 424242; // RefDumpFramed.cs FramedSeed
const RECORD: usize = 14;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..")
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

/// FNV-1a 64 over the 14-byte records of each column, the same function as `SnapshotTiles` in RefDumpFramed.cs.
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

fn json_number(json: &str, key: &str) -> i64 {
    let k = format!("\"{key}\":");
    let at = json.find(&k).unwrap_or_else(|| panic!("{key} missing from the dump json")) + k.len();
    let end = json[at..].find(|c: char| c != '-' && !c.is_ascii_digit()).map_or(json.len(), |e| at + e);
    json[at..end].parse().unwrap()
}

#[derive(Default)]
struct Bucket {
    count: u64,
    first: Vec<(i32, i32)>,
}

fn add(map: &mut BTreeMap<String, Bucket>, key: String, x: i32, y: i32) {
    let b = map.entry(key).or_default();
    b.count += 1;
    if b.first.len() < 5 {
        b.first.push((x, y));
    }
}

/// `ScrambleRegion` of tools/refdump/RefDumpFramed.cs, line for line.
fn scramble(g: &mut Game) {
    let (w, h) = (g.main.tile.width, g.main.tile.height);
    let cand: Vec<u16> = (0..g.main.tile_frame_important.len())
        .filter(|&t| !g.main.tile_frame_important[t] && !tile_id::sets::IS_VINE[t] && t != 80 && t != 49)
        .map(|t| t as u16)
        .collect();
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
            let t = g.main.tile.get_mut(x, y);
            t.type_ = if active { cand[idx as usize] } else { 0 };
            t.s_tile_header = (t.s_tile_header & !(0x20 | 0x400 | 0x7000)) | ((active as u16) << 5) | ((half as u16) << 10) | (slope << 12);
            t.frame_x = -1;
            t.frame_y = -1;
        }
    }
}

#[test]
fn whole_reference_world_matches_the_real_game() {
    run_case("ref_small", false);
}

#[test]
fn scrambled_reference_world_matches_the_real_game() {
    run_case("ref_small_scrambled", true);
}

fn run_case(stem: &str, scrambled: bool) {
    let root = root();
    let wld = root.join("work/refsave/Worlds/ref_small.wld");
    let dump_dir = root.join("refdata/framed");
    let post_path = dump_dir.join(format!("{stem}.post.bin"));
    let pre_path = dump_dir.join(format!("{stem}.pre_columns.bin"));
    let json_path = dump_dir.join(format!("{stem}.json"));
    let (Ok(wld_bytes), Ok(post), Ok(pre_cols), Ok(json)) =
        (std::fs::read(&wld), std::fs::read(&post_path), std::fs::read(&pre_path), std::fs::read_to_string(&json_path))
    else {
        eprintln!(
            "SKIP: need {} and the dump in {} (run tools\\refdump\\bin\\RefDump.exe framed <world> refdata\\framed)",
            wld.display(),
            dump_dir.display()
        );
        return;
    };

    // ---- the starting point must be the tiles the game had after LoadWorld
    let world = terraria_formats::wld::read_world(&wld_bytes).expect("read ref_small.wld");
    let mut g = Game::new();
    let _world = load_world(&mut g, world);
    if scrambled {
        scramble(&mut g);
    }
    let (w, h) = (g.main.tile.width, g.main.tile.height);
    assert_eq!(&post[0..4], b"TFRM");
    assert_eq!(i32::from_le_bytes(post[8..12].try_into().unwrap()), w);
    assert_eq!(i32::from_le_bytes(post[12..16].try_into().unwrap()), h);
    assert_eq!(u32::from_le_bytes(post[16..20].try_into().unwrap()) as usize, RECORD);
    assert_eq!(post.len(), 20 + w as usize * h as usize * RECORD);
    // `WorldFile.LoadWorld` ends with the liquid settle (Liquid.QuickWater / UpdateLiquid / WaterCheck, WorldFile.cs:754-776),
    // which is not ported and changes only the liquid state of some tiles: `liquid`, the liquid type bits of bTileHeader (0x60)
    // and the checkingLiquid / skipLiquid bits of bTileHeader3 (0x18). Framing never reads them (netMode 1 skips
    // Liquid.AddWater), so those fields are taken from the dump (`<name>.liquid.bin`); every other field of every tile must
    // already be equal, which the per-column hashes of the whole pre-run state below prove.
    let liquid = std::fs::read(dump_dir.join(format!("{stem}.liquid.bin"))).expect("liquid.bin of the dump");
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
    eprintln!("tiles with a liquid state taken from the dump: {}", liquid.len() / 8);
    let mine = column_hashes(&g);
    let theirs: Vec<u64> = pre_cols.chunks_exact(8).map(|c| u64::from_le_bytes(c.try_into().unwrap())).collect();
    let bad_cols: Vec<usize> = (0..w as usize).filter(|&x| mine[x] != theirs[x]).collect();
    assert!(bad_cols.is_empty(), "{} columns differ before framing, first: {:?}", bad_cols.len(), &bad_cols[..bad_cols.len().min(10)]);

    // ---- the same run as the harness: netMode 1, generatingWorld false, seeded generator, one SectionTileFrame call
    g.main.net_mode = 1;
    g.world_gen.generating_world = false;
    g.main.rand = UnifiedRandom::new(SEED);
    g.main.rand = UnifiedRandom::new(SEED); // WorldGen.genRand is `=> Main.rand` in the original
    let t0 = std::time::Instant::now();
    section_tile_frame(&mut g, 0, 0, w / 200 - 1, h / 150 - 1);
    eprintln!("[{stem}] SectionTileFrame over {w} x {h}: {:?}", t0.elapsed());

    // ---- compare
    let mut compared: u64 = 0;
    let mut non_important_compared: u64 = 0;
    let mut non_important_mismatch: u64 = 0;
    let mut other_mismatch: u64 = 0;
    let mut by_type: BTreeMap<String, Bucket> = BTreeMap::new();
    let mut by_field: BTreeMap<String, Bucket> = BTreeMap::new();
    for x in 0..w {
        for y in 0..h {
            let o = 20 + (x as usize * h as usize + y as usize) * RECORD;
            let want = read_tile(&post[o..o + RECORD]);
            let have = *g.main.tile.get(x, y);
            compared += 1;
            let important = g.main.tile_frame_important[want.type_ as usize];
            let non_important = want.active() && !important;
            if non_important {
                non_important_compared += 1;
            }
            if want == have {
                continue;
            }
            if non_important {
                non_important_mismatch += 1;
                add(&mut by_type, format!("type {}", want.type_), x, y);
            } else {
                other_mismatch += 1;
            }
            let mut fields = Vec::new();
            if want.type_ != have.type_ {
                fields.push("type");
            }
            if want.wall != have.wall {
                fields.push("wall");
            }
            if want.liquid != have.liquid {
                fields.push("liquid");
            }
            if want.s_tile_header != have.s_tile_header {
                fields.push("sTileHeader");
            }
            if want.b_tile_header != have.b_tile_header {
                fields.push("bTileHeader");
            }
            if want.b_tile_header2 != have.b_tile_header2 {
                fields.push("bTileHeader2");
            }
            if want.b_tile_header3 != have.b_tile_header3 {
                fields.push("bTileHeader3");
            }
            if want.frame_x != have.frame_x {
                fields.push("frameX");
            }
            if want.frame_y != have.frame_y {
                fields.push("frameY");
            }
            add(&mut by_field, format!("{} [{}]", fields.join("+"), if non_important { "non-important active tile" } else { "other" }), x, y);
        }
    }
    let mut kinds = std::collections::BTreeSet::new();
    for x in 0..w {
        for y in 0..h {
            let t = g.main.tile.get(x, y);
            if t.active() && !g.main.tile_frame_important[t.type_ as usize] {
                kinds.insert(t.type_);
            }
        }
    }
    eprintln!("distinct non-frame-important tile types in the world: {} {:?}", kinds.len(), kinds);
    let next = g.main.rand.next_raw();
    let want_next = json_number(&json, "nextRandAfterRun") as i32;

    eprintln!("tiles compared (all fields): {compared}; non-frame-important active tiles: {non_important_compared}");
    eprintln!("mismatching non-frame-important tiles: {non_important_mismatch}; mismatching other tiles: {other_mismatch}");
    for (k, b) in &by_type {
        eprintln!("  {k}: {} mismatches, first {:?}", b.count, b.first);
    }
    for (k, b) in &by_field {
        eprintln!("  fields {k}: {} tiles, first {:?}", b.count, b.first);
    }
    eprintln!("next random number after the run: port {next}, game {want_next}");
    eprintln!("unported calls reached (name, hits):");
    for (name, n) in unported::report() {
        eprintln!("  {name}: {n}");
    }

    assert_eq!(non_important_mismatch, 0, "non-frame-important tiles differ from the real game");
    assert_eq!(other_mismatch, 0, "other tile fields differ from the real game");
    assert_eq!(next, want_next, "random generator state differs after the run");
}
