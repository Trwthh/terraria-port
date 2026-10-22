//! The `WorldGen` methods the liquid code is built on (decompile/Terraria/Terraria/WorldGen.cs): `WaterCheck`, `ClearPendingLiquid`,
//! `SetBoulderSolidity`, `ShimmerRemoveWater` and `SquareTileFrame`.

use crate::game::Game;
use crate::liquid::{self, tile_object_data_check_lava_death, tile_object_data_check_water_death};
use crate::unported;
use crate::world_gen::tile_frame::tile_frame;

/// `WorldGen.SetBoulderSolidity(bool solid)` (WorldGen.cs:11018)
pub fn set_boulder_solidity(g: &mut Game, solid: bool) {
    g.main.tile_solid[138] = solid;
    g.main.tile_solid[484] = solid;
    g.main.tile_solid[664] = solid;
    g.main.tile_solid[711] = solid;
    g.main.tile_solid[712] = solid;
    g.main.tile_solid[713] = solid;
    g.main.tile_solid[714] = solid;
    g.main.tile_solid[715] = solid;
    g.main.tile_solid[716] = solid;
}

/// `WorldGen.ShimmerRemoveWater()` (WorldGen.cs:35091). Only does anything while generating a world; its body reads
/// `GenVars.shimmerPosition`, which belongs to the world generator.
pub fn shimmer_remove_water(g: &mut Game) {
    if !g.world_gen.generating_world {
        return;
    }
    unported!("WorldGen.ShimmerRemoveWater (GenVars.shimmerPosition)");
}

/// `WorldGen.SquareTileFrame(int i, int j, bool resetFrame = true)` (WorldGen.cs:81096)
pub fn square_tile_frame(g: &mut Game, i: i32, j: i32, reset_frame: bool) {
    tile_frame(g, i - 1, j - 1, false, false);
    tile_frame(g, i - 1, j, false, false);
    tile_frame(g, i - 1, j + 1, false, false);
    tile_frame(g, i, j - 1, false, false);
    tile_frame(g, i, j, reset_frame, false);
    tile_frame(g, i, j + 1, false, false);
    tile_frame(g, i + 1, j - 1, false, false);
    tile_frame(g, i + 1, j, false, false);
    tile_frame(g, i + 1, j + 1, false, false);
}

/// `WorldGen.WaterCheck()` (WorldGen.cs:81326)
pub fn water_check(g: &mut Game) {
    liquid::tiles_ignore_water(g, true);
    g.liquid.num_liquid = 0;
    g.liquid.num_liquid_buffer = 0;
    let mut i = 1;
    while i < g.main.max_tiles_x - 1 {
        let mut num = g.main.max_tiles_y - 2;
        while num > 0 {
            g.main.tile.get_mut(i, num).set_checking_liquid(false);
            let tile = *g.main.tile.get(i, num);
            if tile.liquid > 0 && tile.nactive() && g.main.tile_solid[tile.type_ as usize] && !g.main.tile_solid_top[tile.type_ as usize] {
                if tile.type_ != 379 {
                    g.main.tile.get_mut(i, num).liquid = 0;
                }
            } else if tile.liquid > 0 {
                if tile.active() {
                    if tile.lava() {
                        if tile_object_data_check_lava_death(g, &tile) {
                            unported!("WorldGen.KillTile"); // neutral: the tile stays
                        }
                    } else if tile_object_data_check_water_death(g, &tile) {
                        unported!("WorldGen.KillTile"); // neutral: the tile stays
                    }
                }
                // `tile` is a reference in the original: the lines below read the tile as KillTile left it (unchanged here)
                let tile2 = *g.main.tile.get(i, num + 1);
                if (!tile2.nactive() || !g.main.tile_solid[tile2.type_ as usize] || g.main.tile_solid_top[tile2.type_ as usize]) && tile2.liquid < u8::MAX {
                    if tile2.liquid > 250 {
                        g.main.tile.get_mut(i, num + 1).liquid = u8::MAX;
                    } else {
                        liquid::add_water(g, i, num);
                    }
                }
                let tile3 = *g.main.tile.get(i - 1, num);
                let tile4 = *g.main.tile.get(i + 1, num);
                let tile = *g.main.tile.get(i, num);
                if (!tile3.nactive() || !g.main.tile_solid[tile3.type_ as usize] || g.main.tile_solid_top[tile3.type_ as usize]) && tile3.liquid != tile.liquid {
                    liquid::add_water(g, i, num);
                } else if (!tile4.nactive() || !g.main.tile_solid[tile4.type_ as usize] || g.main.tile_solid_top[tile4.type_ as usize]) && tile4.liquid != tile.liquid {
                    liquid::add_water(g, i, num);
                }
                // AddWater changes only the checkingLiquid / skipLiquid bits of these tiles, which are not read below;
                // `tile2.liquid` may have been set to 255 above, so it is read again (the original reads the live tile)
                let tile2 = *g.main.tile.get(i, num + 1);
                if tile.lava() {
                    if tile3.liquid > 0 && !tile3.lava() {
                        liquid::add_water(g, i, num);
                    } else if tile4.liquid > 0 && !tile4.lava() {
                        liquid::add_water(g, i, num);
                    } else if g.main.tile.get(i, num - 1).liquid > 0 && !g.main.tile.get(i, num - 1).lava() {
                        liquid::add_water(g, i, num);
                    } else if tile2.liquid > 0 && !tile2.lava() {
                        liquid::add_water(g, i, num);
                    }
                }
            }
            num -= 1;
        }
        i += 1;
    }
    liquid::tiles_ignore_water(g, false);
}

/// `WorldGen.ClearPendingLiquid()` (WorldGen.cs:81407)
pub fn clear_pending_liquid(g: &mut Game) {
    if g.liquid.num_liquid != 0 || g.liquid.num_liquid_buffer != 0 {
        for i in 0..g.liquid.num_liquid as usize {
            let l = g.liquid.liquid[i];
            g.main.tile.get_mut(l.x, l.y).set_checking_liquid(false);
        }
        for j in 0..g.liquid.num_liquid_buffer as usize {
            let b = g.liquid.liquid_buffer[j];
            g.main.tile.get_mut(b.x, b.y).set_checking_liquid(false);
        }
        g.liquid.num_liquid = 0;
        g.liquid.num_liquid_buffer = 0;
    }
}
