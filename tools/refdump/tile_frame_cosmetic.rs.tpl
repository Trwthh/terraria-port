//! `WorldGen.TileFrameCosmetic` (WorldGen.cs:82301-85749): chooses the sprite-sheet cell (`frameX`, `frameY`) of every
//! tile that is not frame-important from its eight neighbours, and records which sides merge into a neighbour
//! (`WorldGen.mergeUp` .. `mergeRight`).
//!
//! The big middle part is generated mechanically from the decompiled source by
//! `tools/refdump/gen_tile_frame_cosmetic.py` (template: `tools/refdump/tile_frame_cosmetic.rs.tpl`); the outer
//! `switch` statements around it are the hand-written lines of this template. This file is the source of truth
//! from now on.
//!
//! Differences forced by Rust, all behaviour-neutral:
//! * the C# `Tile` locals (`tile`, `tile2` .. `tile8`, `tileCache`) are references into `Main.tile`, so later reads see
//!   writes made in between (`TileFrame(i, j - 1)` can change a neighbour). Each is a local macro that re-reads
//!   `Main.tile` at the same coordinates every time it is used. The `Tile tileCache` parameter of the original is
//!   always `Main.tile[i, j]` (every caller passes that), so it is not a parameter here.
//! * tiles are never null, so the `tile != null &&` guards are dropped.
//! * `TileMergeCullCache` / `Rectangle` locals are plain structs.

use terraria_core::framing;
use terraria_data::sets::tile_id;

use super::tile_frame::{
    cactus_frame, framing_data, get_tile_merge_culling, is_rope, tile_frame, tile_merge_attempt, tile_merge_attempt_4,
    tile_merge_attempt_frametest, tile_merge_attempt_frametest_tbl, tile_merge_attempt_tbl, tile_merge_attempt_tbl_4,
    tile_merge_attempt_tbl_exclude_tbl, tile_merge_attempt_tbl_exclude_type, tile_merge_attempt_weird,
};
use crate::game::Game;

/// The part of `Microsoft.Xna.Framework.Rectangle` this method uses (`new Rectangle(-1, -1, 0, 0)`; only `X` and `Y` change).
#[allow(dead_code)]
struct Rectangle {
    x: i32,
    y: i32,
    width: i32,
    height: i32,
}

/// `WorldGen.TileFrameCosmetic(int i, int j, Tile tileCache, bool resetFrame)` (WorldGen.cs:82301). `tileCache` is
/// `Main.tile[i, j]` at every call site and is not a parameter here.
#[allow(unused_assignments, unused_mut, unused_variables, unused_parens, unused_macros, non_snake_case)]
pub fn tile_frame_cosmetic(g: &mut Game, i: i32, j: i32, reset_frame: bool) {
    // The original tests `tileCache == null || !tileCache.active()` first; its caller has already indexed
    // `Main.tile` (which throws outside the world), so the bounds test comes first here.
    if i <= 0 || j <= 0 || i >= g.main.max_tiles_x - 1 || j >= g.main.max_tiles_y - 1 {
        return;
    }
    macro_rules! tile_cache {
        () => {
            *g.main.tile.get(i, j)
        };
    }
    if !tile_cache!().active() {
        return;
    }
    let mut num: i32 = tile_cache!().type_ as i32;
    if g.main.tile_frame_important[num as usize] {
        return;
    }
    match num {
        80 => {
            cactus_frame(g, i, j);
        }
        255..=268 => {
            let d = framing_data(&g.main.arrays);
            framing::self_frame_8way(&mut g.main.tile, &d, &mut g.main.rand, i, j, reset_frame);
        }
        _ => {
            match num {
                385 | 446 | 447 | 448 => {}
                _ => {
                    //@@BODY@@
                }
            }
            // goto case 255;
            let d = framing_data(&g.main.arrays);
            framing::self_frame_8way(&mut g.main.tile, &d, &mut g.main.rand, i, j, reset_frame);
        }
    }
}
