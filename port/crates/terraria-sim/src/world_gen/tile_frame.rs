//! Tile framing of `Terraria.WorldGen`: `TileFrame`, the `TileMergeAttempt*` helpers it relies on, the section
//! framing loops and the small pure helpers they call. The big `TileFrameCosmetic` is in `tile_frame_cosmetic.rs`.
//!
//! Not ported (called through `unported!` at the exact place the original calls them): `TileFrameImportant` and
//! everything under it, `CheckOnTable1x1`, `CheckCactus`, `CheckVines`, `SpawnFallingBlockProjectile`,
//! `Liquid.AddWater`, `MapUpdateQueue.Add`, `Main.sectionManager`.

use terraria_core::framing::{self, FramingData};
use terraria_core::Tile;
use terraria_data::main_arrays::WALL_COUNT;
use terraria_data::sets::tile_id;
use terraria_data::MainArrays;

use super::tile_frame_cosmetic::tile_frame_cosmetic;
use crate::game::Game;
use crate::unported;

/// `WorldGen.TileMergeCullCache` (WorldGen.cs:4009): which neighbours to forget because they differ in
/// `invisibleBlock()` from the centre tile while invisible blocks are not shown.
#[derive(Clone, Copy, Default)]
pub struct TileMergeCullCache {
    pub cull_top: bool,
    pub cull_bottom: bool,
    pub cull_left: bool,
    pub cull_right: bool,
    pub cull_top_left: bool,
    pub cull_top_right: bool,
    pub cull_bottom_left: bool,
    pub cull_bottom_right: bool,
}

impl TileMergeCullCache {
    /// `TileMergeCullCache.Cull(ref up, ref down, ref left, ref right, ref upLeft, ref upRight, ref downLeft, ref downRight)` (WorldGen.cs:4027)
    pub fn cull(&self, up: &mut i32, down: &mut i32, left: &mut i32, right: &mut i32, up_left: &mut i32, up_right: &mut i32, down_left: &mut i32, down_right: &mut i32) {
        if self.cull_top {
            *up = -1;
        }
        if self.cull_bottom {
            *down = -1;
        }
        if self.cull_left {
            *left = -1;
        }
        if self.cull_right {
            *right = -1;
        }
        if self.cull_top_left {
            *up_left = -1;
        }
        if self.cull_top_right {
            *up_right = -1;
        }
        if self.cull_bottom_left {
            *down_left = -1;
        }
        if self.cull_bottom_right {
            *down_right = -1;
        }
    }
}

/// The tables `Framing` reads, borrowed from the game state (`Framing.cs` reads the same statics).
pub fn framing_data(arrays: &MainArrays) -> FramingData<'_> {
    FramingData {
        forced_dirt_merging: &tile_id::sets::FORCED_DIRT_MERGING,
        tile_brick: &arrays.tile_brick,
        gemspark_framing_types: &tile_id::sets::GEMSPARK_FRAMING_TYPES,
        truncates_walls: &tile_id::sets::TRUNCATES_WALLS,
        wall_large_frames: &arrays.wall_large_frames,
        wall_count: WALL_COUNT as u16,
    }
}

/// `WorldGen.InWorld(int x, int y, int fluff = 0)` (WorldGen.cs:9687)
pub fn in_world(g: &Game, x: i32, y: i32, fluff: i32) -> bool {
    if x < fluff || x >= g.main.max_tiles_x - fluff || y < fluff || y >= g.main.max_tiles_y - fluff {
        return false;
    }
    true
}

/// `WorldGen.GetRopeEnds(x, y, out topRopeY, out bottomRopeY, treatEmptyAsTheRopeEnd, rangeToCheck)` (WorldGen.cs:70837).
/// Returns `(topRopeY, bottomRopeY)`.
pub fn get_rope_ends(g: &Game, x: i32, y: i32, treat_empty_as_the_rope_end: bool, range_to_check: i32) -> (i32, i32) {
    let mut top_rope_y = -1;
    let mut bottom_rope_y = -1;
    let mut i = 1;
    while i <= range_to_check && in_world(g, x, y - i, 0) {
        let tile = *g.main.tile.get(x, y - i);
        if !tile.active() {
            if treat_empty_as_the_rope_end {
                top_rope_y = y - i;
            }
            break;
        }
        if g.main.tile_rope[tile.type_ as usize] {
            top_rope_y = y - i;
            break;
        }
        i += 1;
    }
    let num = y - top_rope_y;
    let mut j = 1 + num;
    while j <= range_to_check + 1 && in_world(g, x, top_rope_y + j, 0) {
        let tile2 = *g.main.tile.get(x, top_rope_y + j);
        if !tile2.active() {
            if treat_empty_as_the_rope_end {
                bottom_rope_y = top_rope_y + j;
            }
            break;
        }
        if g.main.tile_rope[tile2.type_ as usize] {
            bottom_rope_y = top_rope_y + j;
            break;
        }
        j += 1;
    }
    (top_rope_y, bottom_rope_y)
}

/// `WorldGen.IsRope(int x, int y, int rangeToCheck = 5)` (WorldGen.cs:70886) and its `out` overload
/// `IsRope(x, y, out topRopeY, out bottomRopeY, rangeToCheck)` (WorldGen.cs:70893), whose `out` values the first one discards.
pub fn is_rope(g: &Game, x: i32, y: i32, range_to_check: i32) -> bool {
    let tile = *g.main.tile.get(x, y);
    if !tile.active() {
        return false;
    }
    if g.main.tile_rope[tile.type_ as usize] {
        return true;
    }
    if tile.type_ != 314 && tile.type_ != 380 && !tile_id::sets::PLATFORMS[tile.type_ as usize] {
        return false;
    }
    let (top_rope_y, bottom_rope_y) = get_rope_ends(g, x, y, false, range_to_check);
    if top_rope_y != -1 {
        return bottom_rope_y != -1;
    }
    false
}

/// `WorldGen.TileMergeAttemptFrametest(i, j, myType, int lookfor, ref up .. ref downRight)` (WorldGen.cs:81802)
pub fn tile_merge_attempt_frametest(g: &mut Game, i: i32, j: i32, my_type: i32, lookfor: i32, up: &mut i32, down: &mut i32, left: &mut i32, right: &mut i32, up_left: &mut i32, up_right: &mut i32, down_left: &mut i32, down_right: &mut i32) {
    if *up == lookfor {
        tile_frame(g, i, j - 1, false, false);
        if g.world_gen.merge_down {
            *up = my_type;
        }
    }
    if *down == lookfor {
        tile_frame(g, i, j + 1, false, false);
        if g.world_gen.merge_up {
            *down = my_type;
        }
    }
    if *left == lookfor {
        tile_frame(g, i - 1, j, false, false);
        if g.world_gen.merge_right {
            *left = my_type;
        }
    }
    if *right == lookfor {
        tile_frame(g, i + 1, j, false, false);
        if g.world_gen.merge_left {
            *right = my_type;
        }
    }
    if *up_left == lookfor {
        *up_left = my_type;
    }
    if *up_right == lookfor {
        *up_right = my_type;
    }
    if *down_left == lookfor {
        *down_left = my_type;
    }
    if *down_right == lookfor {
        *down_right = my_type;
    }
}

/// `WorldGen.TileMergeAttemptFrametest(i, j, myType, bool[] lookfor, ref up .. ref downRight)` (WorldGen.cs:81854)
pub fn tile_merge_attempt_frametest_tbl(g: &mut Game, i: i32, j: i32, my_type: i32, lookfor: &[bool], up: &mut i32, down: &mut i32, left: &mut i32, right: &mut i32, up_left: &mut i32, up_right: &mut i32, down_left: &mut i32, down_right: &mut i32) {
    if *up > -1 && lookfor[*up as usize] {
        tile_frame(g, i, j - 1, false, false);
        if g.world_gen.merge_down {
            *up = my_type;
        }
    }
    if *down > -1 && lookfor[*down as usize] {
        tile_frame(g, i, j + 1, false, false);
        if g.world_gen.merge_up {
            *down = my_type;
        }
    }
    if *left > -1 && lookfor[*left as usize] {
        tile_frame(g, i - 1, j, false, false);
        if g.world_gen.merge_right {
            *left = my_type;
        }
    }
    if *right > -1 && lookfor[*right as usize] {
        tile_frame(g, i + 1, j, false, false);
        if g.world_gen.merge_left {
            *right = my_type;
        }
    }
    if *up_left > -1 && lookfor[*up_left as usize] {
        *up_left = my_type;
    }
    if *up_right > -1 && lookfor[*up_right as usize] {
        *up_right = my_type;
    }
    if *down_left > -1 && lookfor[*down_left as usize] {
        *down_left = my_type;
    }
    if *down_right > -1 && lookfor[*down_right as usize] {
        *down_right = my_type;
    }
}

/// `WorldGen.TileMergeAttempt(int myType, int lookfor, ref up, ref down, ref left, ref right)` (WorldGen.cs:81906)
pub fn tile_merge_attempt_4(my_type: i32, lookfor: i32, up: &mut i32, down: &mut i32, left: &mut i32, right: &mut i32) {
    if lookfor == *up {
        *up = my_type;
    }
    if lookfor == *down {
        *down = my_type;
    }
    if lookfor == *left {
        *left = my_type;
    }
    if lookfor == *right {
        *right = my_type;
    }
}

/// `WorldGen.TileMergeAttempt(int myType, bool[] lookfor, ref up, ref down, ref left, ref right)` (WorldGen.cs:81926)
pub fn tile_merge_attempt_tbl_4(my_type: i32, lookfor: &[bool], up: &mut i32, down: &mut i32, left: &mut i32, right: &mut i32) {
    if *up > -1 && lookfor[*up as usize] {
        *up = my_type;
    }
    if *down > -1 && lookfor[*down as usize] {
        *down = my_type;
    }
    if *left > -1 && lookfor[*left as usize] {
        *left = my_type;
    }
    if *right > -1 && lookfor[*right as usize] {
        *right = my_type;
    }
}

/// `WorldGen.TileMergeAttempt(int myType, int lookfor, ref up .. ref downRight)` (WorldGen.cs:81946)
pub fn tile_merge_attempt(my_type: i32, lookfor: i32, up: &mut i32, down: &mut i32, left: &mut i32, right: &mut i32, up_left: &mut i32, up_right: &mut i32, down_left: &mut i32, down_right: &mut i32) {
    if lookfor == *up {
        *up = my_type;
    }
    if lookfor == *down {
        *down = my_type;
    }
    if lookfor == *left {
        *left = my_type;
    }
    if lookfor == *right {
        *right = my_type;
    }
    if lookfor == *up_left {
        *up_left = my_type;
    }
    if lookfor == *up_right {
        *up_right = my_type;
    }
    if lookfor == *down_left {
        *down_left = my_type;
    }
    if lookfor == *down_right {
        *down_right = my_type;
    }
}

/// `WorldGen.TileMergeAttempt(int myType, bool[] lookfor, ref up .. ref downRight)` (WorldGen.cs:81982)
pub fn tile_merge_attempt_tbl(my_type: i32, lookfor: &[bool], up: &mut i32, down: &mut i32, left: &mut i32, right: &mut i32, up_left: &mut i32, up_right: &mut i32, down_left: &mut i32, down_right: &mut i32) {
    if *up > -1 && lookfor[*up as usize] {
        *up = my_type;
    }
    if *down > -1 && lookfor[*down as usize] {
        *down = my_type;
    }
    if *left > -1 && lookfor[*left as usize] {
        *left = my_type;
    }
    if *right > -1 && lookfor[*right as usize] {
        *right = my_type;
    }
    if *up_left > -1 && lookfor[*up_left as usize] {
        *up_left = my_type;
    }
    if *up_right > -1 && lookfor[*up_right as usize] {
        *up_right = my_type;
    }
    if *down_left > -1 && lookfor[*down_left as usize] {
        *down_left = my_type;
    }
    if *down_right > -1 && lookfor[*down_right as usize] {
        *down_right = my_type;
    }
}

/// `WorldGen.TileMergeAttempt(int myType, bool[] lookfor, int exclude, ref up .. ref downRight)` (WorldGen.cs:82018)
pub fn tile_merge_attempt_tbl_exclude_type(my_type: i32, lookfor: &[bool], exclude: i32, up: &mut i32, down: &mut i32, left: &mut i32, right: &mut i32, up_left: &mut i32, up_right: &mut i32, down_left: &mut i32, down_right: &mut i32) {
    if *up > -1 && *up != exclude && lookfor[*up as usize] {
        *up = my_type;
    }
    if *down > -1 && *down != exclude && lookfor[*down as usize] {
        *down = my_type;
    }
    if *left > -1 && *left != exclude && lookfor[*left as usize] {
        *left = my_type;
    }
    if *right > -1 && *right != exclude && lookfor[*right as usize] {
        *right = my_type;
    }
    if *up_left > -1 && *up_left != exclude && lookfor[*up_left as usize] {
        *up_left = my_type;
    }
    if *up_right > -1 && *up_right != exclude && lookfor[*up_right as usize] {
        *up_right = my_type;
    }
    if *down_left > -1 && *down_left != exclude && lookfor[*down_left as usize] {
        *down_left = my_type;
    }
    if *down_right > -1 && *down_right != exclude && lookfor[*down_right as usize] {
        *down_right = my_type;
    }
}

/// `WorldGen.TileMergeAttempt(int myType, bool[] lookfor, bool[] exclude, ref up .. ref downRight)` (WorldGen.cs:82054)
pub fn tile_merge_attempt_tbl_exclude_tbl(my_type: i32, lookfor: &[bool], exclude: &[bool], up: &mut i32, down: &mut i32, left: &mut i32, right: &mut i32, up_left: &mut i32, up_right: &mut i32, down_left: &mut i32, down_right: &mut i32) {
    if *up > -1 && !exclude[*up as usize] && lookfor[*up as usize] {
        *up = my_type;
    }
    if *down > -1 && !exclude[*down as usize] && lookfor[*down as usize] {
        *down = my_type;
    }
    if *left > -1 && !exclude[*left as usize] && lookfor[*left as usize] {
        *left = my_type;
    }
    if *right > -1 && !exclude[*right as usize] && lookfor[*right as usize] {
        *right = my_type;
    }
    if *up_left > -1 && !exclude[*up_left as usize] && lookfor[*up_left as usize] {
        *up_left = my_type;
    }
    if *up_right > -1 && !exclude[*up_right as usize] && lookfor[*up_right as usize] {
        *up_right = my_type;
    }
    if *down_left > -1 && !exclude[*down_left as usize] && lookfor[*down_left as usize] {
        *down_left = my_type;
    }
    if *down_right > -1 && !exclude[*down_right as usize] && lookfor[*down_right as usize] {
        *down_right = my_type;
    }
}

/// `WorldGen.TileMergeAttemptWeird(int myType, int changeTo, bool[] exclude, ref up .. ref downRight)` (WorldGen.cs:82090)
pub fn tile_merge_attempt_weird(my_type: i32, change_to: i32, exclude: &[bool], up: &mut i32, down: &mut i32, left: &mut i32, right: &mut i32, up_left: &mut i32, up_right: &mut i32, down_left: &mut i32, down_right: &mut i32) {
    if *up > -1 && !exclude[*up as usize] && *up != my_type {
        *up = change_to;
    }
    if *down > -1 && !exclude[*down as usize] && *down != my_type {
        *down = change_to;
    }
    if *left > -1 && !exclude[*left as usize] && *left != my_type {
        *left = change_to;
    }
    if *right > -1 && !exclude[*right as usize] && *right != my_type {
        *right = change_to;
    }
    if *up_left > -1 && !exclude[*up_left as usize] && *up_left != my_type {
        *up_left = change_to;
    }
    if *up_right > -1 && !exclude[*up_right as usize] && *up_right != my_type {
        *up_right = change_to;
    }
    if *down_left > -1 && !exclude[*down_left as usize] && *down_left != my_type {
        *down_left = change_to;
    }
    if *down_right > -1 && !exclude[*down_right as usize] && *down_right != my_type {
        *down_right = change_to;
    }
}

/// `WorldGen.GetTileMergeCulling(tileCache, tileTopCache, tileBottomCache, tileLeftCache, tileRightCache,
/// tileBottomLeftCache, tileBottomRightCache, tileTopLeftCache, tileTopRightCache)` (WorldGen.cs:85751).
/// The neighbours are never null in the port, so the `!= null` tests are true.
pub fn get_tile_merge_culling(g: &Game, tile_cache: Tile, tile_top_cache: Tile, tile_bottom_cache: Tile, tile_left_cache: Tile, tile_right_cache: Tile, tile_bottom_left_cache: Tile, tile_bottom_right_cache: Tile, tile_top_left_cache: Tile, tile_top_right_cache: Tile) -> TileMergeCullCache {
    if g.main.should_show_invisible_blocks_and_walls() {
        return TileMergeCullCache::default();
    }
    let flag = tile_cache.invisible_block();
    TileMergeCullCache {
        cull_top: tile_top_cache.invisible_block() != flag,
        cull_bottom: tile_bottom_cache.invisible_block() != flag,
        cull_left: tile_left_cache.invisible_block() != flag,
        cull_right: tile_right_cache.invisible_block() != flag,
        cull_top_left: tile_top_left_cache.invisible_block() != flag,
        cull_top_right: tile_top_right_cache.invisible_block() != flag,
        cull_bottom_left: tile_bottom_left_cache.invisible_block() != flag,
        cull_bottom_right: tile_bottom_right_cache.invisible_block() != flag,
    }
}

/// `WorldGen.CactusFrame(int i, int j)` (WorldGen.cs:56865). Out-of-world reads throw `IndexOutOfRangeException` in the
/// original and land in its `catch`, which zeroes the tile's frame; `try_get` + `Err` models that.
pub fn cactus_frame(g: &mut Game, i: i32, j: i32) {
    if cactus_frame_try(g, i, j).is_err() {
        // catch { Main.tile[i, j].frameX = 0; Main.tile[i, j].frameY = 0; }
        let t = g.main.tile.get_mut(i, j);
        t.frame_x = 0;
        t.frame_y = 0;
    }
}

#[allow(unused_assignments)] // the original assigns num and num2 again before reading them
fn cactus_frame_try(g: &mut Game, i: i32, j: i32) -> Result<(), ()> {
    macro_rules! t {
        ($x:expr, $y:expr) => {
            g.main.tile.try_get($x, $y).ok_or(())?
        };
    }
    let mut num = j;
    let mut num2 = i;
    if unported!("WorldGen.CheckCactus", false) {
        return Ok(());
    }
    while t!(num2, num).active() && t!(num2, num).type_ == 80 {
        num += 1;
        // Main.tile[num2, num] == null: never null
        if !t!(num2, num).active() || t!(num2, num).type_ != 80 {
            if t!(num2 - 1, num).active() && t!(num2 - 1, num).type_ == 80 && t!(num2 - 1, num - 1).active() && t!(num2 - 1, num - 1).type_ == 80 && num2 >= i {
                num2 -= 1;
            }
            if t!(num2 + 1, num).active() && t!(num2 + 1, num).type_ == 80 && t!(num2 + 1, num - 1).active() && t!(num2 + 1, num - 1).type_ == 80 && num2 <= i {
                num2 += 1;
            }
        }
    }
    num -= 1;
    let num3 = i - num2;
    num2 = i;
    num = j;
    let mut num4 = t!(i - 2, j).type_ as i32;
    let mut num5 = t!(i - 1, j).type_ as i32;
    let mut num6 = t!(i + 1, j).type_ as i32;
    let mut num7 = t!(i, j - 1).type_ as i32;
    if num7 == 227 {
        num7 = 80;
    }
    let mut num8 = t!(i, j + 1).type_ as i32;
    let mut num9 = t!(i - 1, j + 1).type_ as i32;
    let mut num10 = t!(i + 1, j + 1).type_ as i32;
    if !t!(i - 2, j).active() {
        num4 = -1;
    }
    if !t!(i - 1, j).active() {
        num5 = -1;
    }
    if !t!(i + 1, j).active() {
        num6 = -1;
    }
    if !t!(i, j - 1).active() {
        num7 = -1;
    }
    if !t!(i, j + 1).active() {
        num8 = -1;
    }
    if !t!(i - 1, j + 1).active() {
        num9 = -1;
    }
    if !t!(i + 1, j + 1).active() {
        num10 = -1;
    }
    let mut num11: i16 = t!(i, j).frame_x;
    let mut num12: i16 = t!(i, j).frame_y;
    match num3 {
        0 => {
            if num7 != 80 {
                if num5 == 80 && num6 == 80 && num9 != 80 && num10 != 80 && num4 != 80 {
                    num11 = 90;
                    num12 = 0;
                } else if num5 == 80 && num9 != 80 && num4 != 80 {
                    num11 = 72;
                    num12 = 0;
                } else if num6 == 80 && num10 != 80 {
                    num11 = 18;
                    num12 = 0;
                } else {
                    num11 = 0;
                    num12 = 0;
                }
            } else if num5 == 80 && num6 == 80 && num9 != 80 && num10 != 80 && num4 != 80 {
                num11 = 90;
                num12 = 36;
            } else if num5 == 80 && num9 != 80 && num4 != 80 {
                num11 = 72;
                num12 = 36;
            } else if num6 == 80 && num10 != 80 {
                num11 = 18;
                num12 = 36;
            } else if num8 >= 0 && g.main.tile_solid[num8 as usize] {
                num11 = 0;
                num12 = 36;
            } else {
                num11 = 0;
                num12 = 18;
            }
        }
        -1 => {
            if num6 == 80 {
                if num7 != 80 && num8 != 80 {
                    num11 = 108;
                    num12 = 36;
                } else if num8 != 80 {
                    num11 = 54;
                    num12 = 36;
                } else if num7 != 80 {
                    num11 = 54;
                    num12 = 0;
                } else {
                    num11 = 54;
                    num12 = 18;
                }
            } else if num7 != 80 {
                num11 = 54;
                num12 = 0;
            } else {
                num11 = 54;
                num12 = 18;
            }
        }
        1 => {
            if num5 == 80 {
                if num7 != 80 && num8 != 80 {
                    num11 = 108;
                    num12 = 18;
                } else if num8 != 80 {
                    num11 = 36;
                    num12 = 36;
                } else if num7 != 80 {
                    num11 = 36;
                    num12 = 0;
                } else {
                    num11 = 36;
                    num12 = 18;
                }
            } else if num7 != 80 {
                num11 = 36;
                num12 = 0;
            } else {
                num11 = 36;
                num12 = 18;
            }
        }
        _ => {}
    }
    if num11 != t!(i, j).frame_x || num12 != t!(i, j).frame_y {
        let t = g.main.tile.get_mut(i, j);
        t.frame_x = num11;
        t.frame_y = num12;
        tile_frame_cosmetic(g, i - 1, j, false);
        tile_frame_cosmetic(g, i, j - 1, false);
        tile_frame_cosmetic(g, i, j + 1, false);
        tile_frame_cosmetic(g, i + 1, j, false);
    }
    Ok(())
}

/// `WorldGen.TileFrame(int i, int j, bool resetFrame = false, bool noBreak = false)` (WorldGen.cs:82239).
pub fn tile_frame(g: &mut Game, i: i32, j: i32, reset_frame: bool, no_break: bool) {
    // `Main.tile[i, j] == null` is never true in the port
    if i <= 5 || j <= 5 || i >= g.main.max_tiles_x - 5 || j >= g.main.max_tiles_y - 5 {
        return;
    }
    unported!("MapUpdateQueue.Add");
    // try { ... } catch { }: the only exceptions the original can swallow here are out-of-world reads, which the
    // bounds test above rules out.
    {
        if !g.main.tile.get(i, j).active() {
            let tile = g.main.tile.get_mut(i, j);
            tile.set_half_brick(false);
            tile.clear_block_paint_and_coating();
            tile.set_slope(0);
        }
        let tile = *g.main.tile.get(i, j);
        if tile.liquid > 0 && g.main.net_mode != 1 && !g.world_gen.generating_world {
            crate::liquid::add_water(g, i, j);
        }
        if !g.main.tile.get(i, j).active() {
            return;
        }
        let type_ = g.main.tile.get(i, j).type_ as i32;
        if g.main.tile_frame_important[type_ as usize] {
            if !no_break || tile_id::sets::TORCHES[type_ as usize] || tile_id::sets::FRAME_IMPORTANT_ECHO_CULLING[type_ as usize] {
                unported!("WorldGen.TileFrameImportant");
            }
            return;
        }
        if !g.main.tile_solid[type_ as usize] {
            match type_ {
                49 => {
                    unported!("WorldGen.CheckOnTable1x1");
                    return;
                }
                80 => {
                    // neutral value: "the cactus needed no fix", so framing goes on
                    if unported!("WorldGen.CheckCactus", false) {
                        return;
                    }
                }
                _ => {}
            }
            // neutral value: true = "the vine is fine", so framing goes on
            if tile_id::sets::IS_VINE[type_ as usize] && !unported!("WorldGen.CheckVines", true) {
                return;
            }
        }
        // neutral value of SpawnFallingBlockProjectile: false = "no projectile was spawned", so the tile is framed
        if !g.world_gen.generating_world && (!tile_id::sets::FALLING[type_ as usize] || !unported!("WorldGen.SpawnFallingBlockProjectile", false)) {
            tile_frame_cosmetic(g, i, j, reset_frame);
        }
    }
}

/// `WorldGen.Reframe(int x, int y, bool resetFrame = false)` (WorldGen.cs:81241): tile framing then wall framing of one cell.
pub fn reframe(g: &mut Game, x: i32, y: i32, reset_frame: bool) {
    if g.world_gen.generating_world {
        tile_frame_cosmetic(g, x, y, reset_frame);
    } else {
        tile_frame(g, x, y, reset_frame, true);
    }
    let show_invisible = g.main.should_show_invisible_blocks_and_walls();
    let d = framing_data(&g.main.arrays);
    framing::wall_frame(&mut g.main.tile, &d, &mut g.main.rand, x, y, reset_frame, show_invisible);
}

/// `WorldGen.GetSectionBounds(sectionStartX, sectionStartY, sectionEndXInclusive, sectionEndYInclusive, out tileStartX,
/// out tileEndX, out tileStartY, out tileEndY)` (WorldGen.cs:81279). Returns `(tileStartX, tileEndX, tileStartY, tileEndY)`.
pub fn get_section_bounds(g: &Game, section_start_x: i32, section_start_y: i32, section_end_x_inclusive: i32, section_end_y_inclusive: i32) -> (i32, i32, i32, i32) {
    let mut tile_start_x = section_start_x * 200;
    let mut tile_end_x = (section_end_x_inclusive + 1) * 200;
    let mut tile_start_y = section_start_y * 150;
    let mut tile_end_y = (section_end_y_inclusive + 1) * 150;
    if tile_start_x < 1 {
        tile_start_x = 1;
    }
    if tile_start_y < 1 {
        tile_start_y = 1;
    }
    if tile_start_x > g.main.max_tiles_x - 2 {
        tile_start_x = g.main.max_tiles_x - 2;
    }
    if tile_start_y > g.main.max_tiles_y - 2 {
        tile_start_y = g.main.max_tiles_y - 2;
    }
    if tile_end_x > g.main.max_tiles_x - 2 {
        tile_end_x = g.main.max_tiles_x - 2;
    }
    if tile_end_y > g.main.max_tiles_y - 2 {
        tile_end_y = g.main.max_tiles_y - 2;
    }
    (tile_start_x, tile_end_x, tile_start_y, tile_end_y)
}

/// `WorldGen.SectionTileFrame(sectionStartX, sectionStartY, sectionEndXInclusive, sectionEndYInclusive)` (WorldGen.cs:81254):
/// re-frames (with `resetFrame: true`) every tile of the given sections plus a one tile border, column by column.
pub fn section_tile_frame(g: &mut Game, section_start_x: i32, section_start_y: i32, section_end_x_inclusive: i32, section_end_y_inclusive: i32) {
    unported!("Main.mapTime = Main.mapTimeMax + 10");
    let (tile_start_x, tile_end_x, tile_start_y, tile_end_y) = get_section_bounds(g, section_start_x, section_start_y, section_end_x_inclusive, section_end_y_inclusive);
    unported!("MapUpdateQueue.Add(Rectangle)");
    let mut i = tile_start_x - 1;
    while i < tile_end_x + 1 {
        let mut j = tile_start_y - 1;
        while j < tile_end_y + 1 {
            // if (Main.tile[i, j] == null) Main.tile[i, j] = new Tile();  -- never null in the port
            reframe(g, i, j, true);
            j += 1;
        }
        i += 1;
    }
    let mut k = section_start_x;
    while k <= section_end_x_inclusive {
        let mut l = section_start_y;
        while l <= section_end_y_inclusive {
            unported!("Main.sectionManager.SetSectionFramed");
            l += 1;
        }
        k += 1;
    }
}

/// `WorldGen.SectionTileFrameWithCheck(startX, startY, endX, endY)` (WorldGen.cs:81122): frames the sections in a tile
/// rectangle that are loaded and not framed yet. The section manager (`Main.sectionManager`) is not ported, so
/// `SectionLoaded` is reported as false (neutral: nothing to frame) and the loop does nothing.
pub fn section_tile_frame_with_check(g: &mut Game, start_x: i32, start_y: i32, end_x: i32, end_y: i32) {
    let section_x = start_x / 200; // Netplay.GetSectionX (Netplay.cs:776)
    let section_y = start_y / 150; // Netplay.GetSectionY (Netplay.cs:781)
    let section_x2 = end_x / 200;
    let section_y2 = end_y / 150;
    let mut i = section_x;
    while i <= section_x2 {
        let mut j = section_y;
        while j <= section_y2 {
            if unported!("Main.sectionManager.SectionLoaded", false) && !unported!("Main.sectionManager.SectionFramed", false) {
                section_tile_frame(g, i, j, i, j);
                if !unported!("Main.sectionManager.AnyUnfinishedSections", false) {
                    return;
                }
            }
            j += 1;
        }
        i += 1;
    }
}
