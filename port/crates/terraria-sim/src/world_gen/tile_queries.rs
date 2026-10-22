//! Small `WorldGen` tile queries (WorldGen.cs) read by the drawing code: `SolidTile`, `IsRope`, `GetCactusType`,
//! `InAPlaceWithWind`, `IsPalmOasisTree`, `GetBiomeInfluence`, `IsBelowANonHammeredPlatform`.

use terraria_core::Tile;
use terraria_data::sets::tile_id::sets as tile_sets;
use terraria_data::sets::wall_id::sets as wall_sets;

use crate::game::Game;

/// `WorldGen.SolidTile(int i, int j, bool noDoors = false)` (WorldGen.cs:70923). The `try { } catch { }` returns false for coordinates outside the world.
pub fn solid_tile(g: &Game, i: i32, j: i32, no_doors: bool) -> bool {
    let Some(tile) = g.main.tile.try_get(i, j) else {
        return false;
    };
    if tile.active() && g.main.tile_solid[tile.type_ as usize] && !g.main.tile_solid_top[tile.type_ as usize] && !tile.half_brick() && tile.slope() == 0 && !tile.in_active() {
        if no_doors && tile.type_ == 10 {
            return false;
        }
        return true;
    }
    false
}

/// `WorldGen.SolidTile(Tile testTile)` (WorldGen.cs:70768)..
pub fn solid_tile_tile(g: &Game, test_tile: &Tile) -> bool {
    if test_tile.nactive() && g.main.tile_solid[test_tile.type_ as usize] && !g.main.tile_solid_top[test_tile.type_ as usize] && !test_tile.half_brick() && test_tile.slope() == 0 {
        return true;
    }
    false
}

/// `WorldGen.beachDistance` (WorldGen.cs:4098)
pub const BEACH_DISTANCE: i32 = 380;

/// `WorldGen.IsPalmOasisTree(int x)` (WorldGen.cs:27399)..
pub fn is_palm_oasis_tree(g: &Game, x: i32) -> bool {
    if x >= BEACH_DISTANCE {
        return x <= g.main.max_tiles_x - BEACH_DISTANCE;
    }
    false
}

/// `WorldGen.DoesWindBlowAtThisHeight(int tileY)` (WorldGen.cs:87776)..
pub fn does_wind_blow_at_this_height(g: &Game, tile_y: i32) -> bool {
    let mut flag = (tile_y as f64) < g.main.world_surface;
    if g.main.remix_world {
        flag = !flag;
    }
    flag
}

/// `WorldGen.InAPlaceWithWind(int x, int y, int width, int height)` (WorldGen.cs:87786)..
pub fn in_a_place_with_wind(g: &Game, x: i32, y: i32, width: i32, height: i32) -> bool {
    if !does_wind_blow_at_this_height(g, y) {
        return false;
    }
    for i in 0..width {
        for j in 0..height {
            if super::tile_frame::in_world(g, x + i, y + j, 0) {
                let tile = g.main.tile.get(x + i, y + j);
                if tile.liquid > 0 || (tile.wall > 0 && !wall_sets::ALLOWS_WIND[tile.wall as usize]) {
                    return false;
                }
            }
        }
    }
    true
}

/// `WorldGen.GetCactusType(tileX, tileY, frameX, frameY, out evil, out good, out crimson)` (WorldGen.cs:48154).
/// Literal port; returns `(evil, good, crimson)`.
pub fn get_cactus_type(g: &Game, tile_x: i32, tile_y: i32, frame_x: i32, frame_y: i32) -> (bool, bool, bool) {
    let (mut evil, mut good, mut crimson) = (false, false, false);
    let mut num = tile_x;
    if frame_x == 36 {
        num -= 1;
    }
    if frame_x == 54 {
        num += 1;
    }
    if frame_x == 108 {
        num = if frame_y != 18 { num + 1 } else { num - 1 };
    }
    let mut num2 = tile_y;
    let mut flag = false;
    if !super::tile_frame::in_world(g, num, num2, 2) {
        return (evil, good, crimson);
    }
    let mut tile = *g.main.tile.get(num, num2);
    if tile.type_ == 80 && tile.active() {
        flag = true;
    }
    while !tile.active() || !g.main.tile_solid[tile.type_ as usize] || !flag {
        if tile.type_ == 80 && tile.active() {
            flag = true;
        }
        num2 += 1;
        if num2 > tile_y + 20 || !super::tile_frame::in_world(g, num, num2, 2) {
            break;
        }
        tile = *g.main.tile.get(num, num2);
    }
    if tile.active() {
        if tile.type_ == 112 {
            evil = true;
        }
        if tile.type_ == 116 {
            good = true;
        }
        if tile.type_ == 234 {
            crimson = true;
        }
    }
    (evil, good, crimson)
}

/// `WorldGen.GetBiomeInfluence(startX, endX, startY, endY, out corruptCount, out crimsonCount, out hallowedCount)`
/// (WorldGen.cs:48217). Literal port; returns `(corruptCount, crimsonCount, hallowedCount)`.
pub fn get_biome_influence(g: &Game, start_x: i32, end_x: i32, start_y: i32, end_y: i32) -> (i32, i32, i32) {
    let (mut corrupt_count, mut crimson_count, mut hallowed_count) = (0, 0, 0);
    for i in start_x..=end_x {
        for j in start_y..=end_y {
            // Main.tile[i, j] is null outside the world in the original; the port skips those tiles
            if let Some(tile) = g.main.tile.try_get(i, j) {
                if tile_sets::CORRUPT[tile.type_ as usize] {
                    corrupt_count += 1;
                }
                if tile_sets::CRIMSON[tile.type_ as usize] {
                    crimson_count += 1;
                }
                if tile_sets::HALLOW[tile.type_ as usize] {
                    hallowed_count += 1;
                }
            }
        }
    }
    (corrupt_count, crimson_count, hallowed_count)
}

/// `WorldGen.IsRope(int x, int y, out int topRopeY, out int bottomRopeY, int rangeToCheck = 5)` (WorldGen.cs:70893).
/// The sim has the overload without the `out` values; this one returns them. Returns `(isRope, topRopeY, bottomRopeY)`.
pub fn is_rope_out(g: &Game, x: i32, y: i32, range_to_check: i32) -> (bool, i32, i32) {
    let mut top_rope_y = -1;
    let mut bottom_rope_y = -1;
    let tile = *g.main.tile.get(x, y);
    if !tile.active() {
        return (false, top_rope_y, bottom_rope_y);
    }
    if g.main.tile_rope[tile.type_ as usize] {
        return (true, top_rope_y, bottom_rope_y);
    }
    if tile.type_ != 314 && tile.type_ != 380 && !tile_sets::PLATFORMS[tile.type_ as usize] {
        return (false, top_rope_y, bottom_rope_y);
    }
    let (top, bottom) = super::tile_frame::get_rope_ends(g, x, y, false, range_to_check);
    top_rope_y = top;
    bottom_rope_y = bottom;
    if top_rope_y != -1 {
        return (bottom_rope_y != -1, top_rope_y, bottom_rope_y);
    }
    (false, top_rope_y, bottom_rope_y)
}

/// `WorldGen.IsBelowANonHammeredPlatform(int x, int y)` (WorldGen.cs:38597)
pub fn is_below_a_non_hammered_platform(g: &Game, x: i32, y: i32) -> bool {
    if y < 1 {
        return false;
    }
    // `Main.tile[x, y - 1] == null` (outside the world)
    let Some(tile) = g.main.tile.try_get(x, y - 1) else {
        return false;
    };
    if !tile.active() || !tile_sets::PLATFORMS[tile.type_ as usize] || tile.half_brick() || tile.slope() != 0 {
        return false;
    }
    true
}
