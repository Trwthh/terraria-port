//! Shared by the liquid tests: reference file locations, the raw (un-settled) world, and the generator-state sync with the game.
#![allow(dead_code)]

use std::path::PathBuf;

use terraria_sim::Game;

pub fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..")
}

/// A `Game` holding the tiles of `ref_small.wld` exactly as the file has them (no liquid settle yet) and the header values the settle reads,
/// with `Liquid.cycles = 7` and `Liquid.curMaxLiquid = 25000`: what `Main.DoUpdate` leaves in the statics during the menu frames before a world
/// is loaded (Main.cs:17289-17290, `gfxQuality` 1), which is also what `tools/refdump_liquid` sets before the game's `LoadWorld`.
pub fn raw_game() -> Option<Game> {
    let wld = root().join("work/refsave/Worlds/ref_small.wld");
    let Ok(bytes) = std::fs::read(&wld) else {
        eprintln!("SKIP: need {}", wld.display());
        return None;
    };
    let world = terraria_formats::wld::read_world(&bytes).expect("read ref_small.wld");
    let mut g = Game::new();
    g.liquid.cycles = 7;
    g.liquid.cur_max_liquid = 25000;
    g.main.tile = world.tiles;
    g.main.max_tiles_x = world.header.max_tiles_x;
    g.main.max_tiles_y = world.header.max_tiles_y;
    g.main.world_surface = world.header.flags.world_surface;
    g.main.rock_layer = world.header.flags.rock_layer;
    Some(g)
}

/// One line of `settle_trace.txt` (RunSettleTrace in tools/refdump_liquid/RefDumpLiquid.cs): `tag numLiquid numLiquidBuffer wetCounter stuckCount
/// listHash regionHash`, the list hash being FNV-1a 64 over (x, y, kill, delay) as little-endian i32 of the first `numLiquid` entries of
/// `Main.liquid`, the region hash FNV-1a 64 over (liquid, bTileHeader & 0x60, bTileHeader3 & 0x18) of the tiles x 2460..=2530, y 225..=270.
pub fn trace_line(g: &Game, tag: &str) -> String {
    let mut rh: u64 = 14695981039346656037;
    for x in 2460..=2530 {
        for y in 225..=270 {
            let t = g.main.tile.get(x, y);
            for b in [t.liquid, t.b_tile_header & 0x60, t.b_tile_header3 & 0x18] {
                rh ^= b as u64;
                rh = rh.wrapping_mul(1099511628211);
            }
        }
    }
    format!("{tag} {} {} {} {} {} {rh}", g.liquid.num_liquid, g.liquid.num_liquid_buffer, g.liquid.wet_counter_for_tests(), g.liquid.stuck_count, list_hash(g))
}

/// FNV-1a 64 over (x, y, kill, delay) of the first `numLiquid` entries of `Main.liquid`
pub fn list_hash(g: &Game) -> u64 {
    let mut lh: u64 = 14695981039346656037;
    for e in &g.liquid.liquid[..g.liquid.num_liquid as usize] {
        for v in [e.x, e.y, e.kill, e.delay] {
            for b in v.to_le_bytes() {
                lh ^= b as u64;
                lh = lh.wrapping_mul(1099511628211);
            }
        }
    }
    lh
}

/// `Main.rand = new UnifiedRandom(12345)`: the generator RefDumpLiquid.exe settle_trace sets right before the settle statements.
pub const SETTLE_SEED: i32 = 12345;
