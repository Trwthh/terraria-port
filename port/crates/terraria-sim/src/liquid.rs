//! `Terraria.Liquid` (decompile/Terraria/Terraria/Liquid.cs): the liquid simulation.
//!
//! The original is a struct with static fields; `Main.liquid[]` is the active list, `Main.liquidBuffer[]` the overflow queue
//! (`liquid_buffer.rs`). All statics live in `Game::liquid` ([`LiquidState`]). `Tile` is a class in C#, so every `Tile tile = Main.tile[..]`
//! in the original reads and writes the world; here such a local is the coordinate pair and every access goes through `g.main.tile`.
//!
//! Calls into code that is not ported are `unported!` at the original call site (`WorldGen.KillTile`, `WorldGen.PlaceTile`,
//! `WorldGen.ReplaceTile`, `WorldGen.CheckAlch`, `WorldGen.CheckLilyPad`, `NetMessage`, `SoundEngine`, ...), each with the neutral
//! continuation written next to it.

use std::collections::HashSet;

use terraria_core::Tile;

use crate::game::Game;
use crate::liquid_buffer::{self, LiquidBuffer};
use crate::liquid_tile_data::{self, Liq};
use crate::unported;
use crate::world_gen::tile_frame::in_world;
use crate::world_gen::tile_queries::solid_tile;
use crate::world_gen::water_check::{self, set_boulder_solidity};

/// `Liquid.maxLiquidBuffer` (Liquid.cs:15)
pub const MAX_LIQUID_BUFFER: i32 = 50000;
/// `Liquid.maxLiquid` (Liquid.cs:17)
pub const MAX_LIQUID: i32 = 25000;

/// `Terraria.Liquid` (the struct `Main.liquid[]` holds)
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Liquid {
    pub x: i32,
    pub y: i32,
    pub kill: i32,
    pub delay: i32,
}

/// The static fields of `Liquid` and `LiquidBuffer`, `Main.liquid` / `Main.liquidBuffer`, and the few `WorldGen` / `GenVars` / `Main`
/// statics only the liquid code reads (each documented with its original).
pub struct LiquidState {
    /// `Main.liquid` (`new Liquid[Liquid.maxLiquid]`, Main.cs:1195)
    pub liquid: Vec<Liquid>,
    /// `Main.liquidBuffer` (`new LiquidBuffer[Liquid.maxLiquidBuffer]`, Main.cs:1197)
    pub liquid_buffer: Vec<LiquidBuffer>,
    /// `LiquidBuffer.numLiquidBuffer`
    pub num_liquid_buffer: i32,
    /// `Liquid.skipCount` (0)
    pub skip_count: i32,
    /// `Liquid.stuckCount` (0)
    pub stuck_count: i32,
    /// `Liquid.stuckAmount` (0)
    pub stuck_amount: i32,
    /// `Liquid.cycles` (`= 10`)
    pub cycles: i32,
    /// `Liquid.curMaxLiquid` (`= 0`; `Liquid.ReInit` and `Main.DoUpdate` (Main.cs:17290) set it)
    pub cur_max_liquid: i32,
    /// `Liquid.numLiquid` (0)
    pub num_liquid: i32,
    /// `Liquid.stuck` (false)
    pub stuck: bool,
    /// `Liquid.quickFall` (false)
    pub quick_fall: bool,
    /// `Liquid.quickSettle` (false)
    pub quick_settle: bool,
    /// `Liquid.wetCounter` (private, 0)
    wet_counter: i32,
    /// `Liquid.panicCounter` (0)
    pub panic_counter: i32,
    /// `Liquid.panicMode` (false)
    pub panic_mode: bool,
    /// `Liquid.panicY` (0)
    pub panic_y: i32,
    /// `Liquid._netChangeSet`
    net_change_set: HashSet<i32>,
    /// `Liquid._swapNetChangeSet`
    swap_net_change_set: HashSet<i32>,
    /// `Main.Setting_UseReducedMaxLiquids` (Main.cs:1741, false)
    pub setting_use_reduced_max_liquids: bool,
    /// `GenVars.waterLine` (`public static int`, 0). `WorldFile.LoadWorld` sets it to `maxTilesY` (WorldFile.cs:743), `Liquid.StartPanic` too.
    pub water_line: i32,
    /// `GenVars.lavaLine` (0). Written by world generation.
    pub lava_line: i32,
    /// `WorldGen.remixWorldGen` (false). Written by world generation.
    pub remix_world_gen: bool,
    /// `WorldGen.noTrapsWorldGen` (false). Written by world generation.
    pub no_traps_world_gen: bool,
    /// `Main.dualDungeonsSeed` (false). Written by world generation.
    pub dual_dungeons_seed: bool,
    /// `Main.skyblockWorld` (false). Written by world generation.
    pub skyblock_world: bool,
}

impl LiquidState {
    /// `Liquid.wetCounter` (private in the original); tests read it to compare against the game.
    pub fn wet_counter_for_tests(&self) -> i32 {
        self.wet_counter
    }

    /// The values of the static initializers of `Liquid`, `LiquidBuffer`, `Main.liquid` / `Main.liquidBuffer`.
    pub fn new() -> Self {
        Self {
            liquid: vec![Liquid::default(); MAX_LIQUID as usize],
            liquid_buffer: vec![LiquidBuffer::default(); MAX_LIQUID_BUFFER as usize],
            num_liquid_buffer: 0,
            skip_count: 0,
            stuck_count: 0,
            stuck_amount: 0,
            cycles: 10,
            cur_max_liquid: 0,
            num_liquid: 0,
            stuck: false,
            quick_fall: false,
            quick_settle: false,
            wet_counter: 0,
            panic_counter: 0,
            panic_mode: false,
            panic_y: 0,
            net_change_set: HashSet::new(),
            swap_net_change_set: HashSet::new(),
            setting_use_reduced_max_liquids: false,
            water_line: 0,
            lava_line: 0,
            remix_world_gen: false,
            no_traps_world_gen: false,
            dual_dungeons_seed: false,
            skyblock_world: false,
        }
    }
}

impl Default for LiquidState {
    fn default() -> Self {
        Self::new()
    }
}

/// A tile coordinate: the port's stand-in for the `Tile` reference the original holds (`Tile` is a class there).
type P = (i32, i32);

#[inline]
fn lq(g: &Game, p: P) -> u8 {
    g.main.tile.get(p.0, p.1).liquid
}

#[inline]
fn lt(g: &Game, p: P) -> u8 {
    g.main.tile.get(p.0, p.1).liquid_type()
}

#[inline]
fn set_lq(g: &mut Game, p: P, v: u8) {
    g.main.tile.get_mut(p.0, p.1).liquid = v;
}

/// `Tile.liquidType(int liquidType)` (Tile.cs:216): a value outside 0..3 changes nothing (`LiquidOverwriteStrip` passes -1).
fn set_liquid_type(g: &mut Game, p: P, liquid_type: i32) {
    if (0..=3).contains(&liquid_type) {
        g.main.tile.get_mut(p.0, p.1).set_liquid_type(liquid_type as u8);
    }
}

/// `tile.nactive() && Main.tileSolid[tile.type] && !Main.tileSolidTop[tile.type]`, which the original writes out at every use
/// (or its negation `!tile.nactive() || !Main.tileSolid[tile.type] || Main.tileSolidTop[tile.type]`).
#[inline]
fn blocks(g: &Game, p: P) -> bool {
    let t = g.main.tile.get(p.0, p.1);
    t.nactive() && g.main.tile_solid[t.type_ as usize] && !g.main.tile_solid_top[t.type_ as usize]
}

/// `Liquid.NetSendLiquid(int x, int y)` (Liquid.cs:57)
pub fn net_send_liquid(g: &mut Game, x: i32, y: i32) {
    if g.world_gen.is_generating_or_loading_world {
        return;
    }
    g.liquid.net_change_set.insert(((x & 0xFFFF) << 16) | (y & 0xFFFF));
}

/// `Liquid.tilesIgnoreWater(bool ignoreSolids)` (Liquid.cs:69)
pub fn tiles_ignore_water(g: &mut Game, ignore_solids: bool) {
    set_boulder_solidity(g, !ignore_solids);
    g.main.tile_solid[546] = !ignore_solids;
}

/// `Liquid.worldGenTilesIgnoreWater(bool ignoreSolids)` (Liquid.cs:75)
pub fn world_gen_tiles_ignore_water(g: &mut Game, ignore_solids: bool) {
    g.main.tile_solid[10] = !ignore_solids;
    g.main.tile_solid[192] = !ignore_solids;
    g.main.tile_solid[191] = !ignore_solids;
    g.main.tile_solid[190] = !ignore_solids;
}

/// `Liquid.ReInit()` (Liquid.cs:83)
pub fn re_init(g: &mut Game) {
    let l = &mut g.liquid;
    l.skip_count = 0;
    l.stuck_count = 0;
    l.stuck_amount = 0;
    l.cycles = 10;
    l.cur_max_liquid = MAX_LIQUID;
    l.num_liquid = 0;
    l.stuck = false;
    l.quick_fall = false;
    l.quick_settle = false;
    l.wet_counter = 0;
    l.panic_counter = 0;
    l.panic_mode = false;
    l.panic_y = 0;
    if l.setting_use_reduced_max_liquids {
        l.cur_max_liquid = 5000;
    }
}

/// `Liquid.QuickWater(int verbose = 0, int minY = -1, int maxY = -1)` (Liquid.cs:104)
pub fn quick_water(g: &mut Game, verbose: i32, min_y: i32, max_y: i32) {
    if g.world_gen.is_generating_or_loading_world {
        water_check::shimmer_remove_water(g);
        if g.liquid.no_traps_world_gen {
            set_boulder_solidity(g, false);
        }
    }
    g.main.tile_solid[379] = true;
    tiles_ignore_water(g, true);
    let mut min_y = min_y;
    let mut max_y = max_y;
    if min_y == -1 {
        min_y = 3;
    }
    if max_y == -1 {
        max_y = g.main.max_tiles_y - 3;
    }
    let mut num = max_y;
    while num >= min_y {
        update_progress_display(verbose, min_y, max_y, num);
        let mut i = 4;
        while i < g.main.max_tiles_x - 4 {
            if g.main.tile.get(i, num).liquid != 0 {
                settle_water_at(g, i, num);
            }
            i += 1;
        }
        num -= 1;
    }
    tiles_ignore_water(g, false);
    if g.world_gen.is_generating_or_loading_world {
        water_check::shimmer_remove_water(g);
        if g.liquid.no_traps_world_gen {
            set_boulder_solidity(g, true);
        }
    }
    if g.world_gen.generating_world && !g.liquid.skyblock_world {
        unported!("WorldGen.LiquidInteractionsCleanup");
    }
}

/// `Liquid.SettleWaterAt(int originX, int originY)` (Liquid.cs:150)
fn settle_water_at(g: &mut Game, origin_x: i32, origin_y: i32) {
    let tile: P = (origin_x, origin_y);
    tiles_ignore_water(g, true);
    if lq(g, tile) == 0 || (g.main.tile.get(tile.0, tile.1).active() && g.main.tile.get(tile.0, tile.1).type_ == 379) {
        return;
    }
    let mut num = origin_x;
    let mut num2 = origin_y;
    let tile_at_xy_has_lava = g.main.tile.get(tile.0, tile.1).lava();
    let flag = g.main.tile.get(tile.0, tile.1).honey();
    let flag2 = g.main.tile.get(tile.0, tile.1).shimmer();
    let mut num3: i32 = lq(g, tile) as i32;
    let mut b: u8 = lt(g, tile);
    set_lq(g, tile, 0);
    let mut flag3 = true;
    loop {
        let mut tile2: P = (num, num2 + 1);
        let mut flag4 = false;
        while num2 < g.main.max_tiles_y - 5 && lq(g, tile2) == 0 && !blocks(g, tile2) {
            num2 += 1;
            flag4 = true;
            flag3 = false;
            tile2 = (num, num2 + 1);
        }
        if g.world_gen.is_generating_or_loading_world && flag4 && !flag && !flag2 {
            if g.liquid.remix_world_gen {
                // `(byte)(cond ? 1 : 0)`
                b = if num2 > g.liquid.lava_line
                    && (((num2 as f64) < g.main.rock_layer - 80.0) || num2 > g.main.max_tiles_y - 350)
                    && !crate::scene_metrics::ocean_depths(g, num, num2)
                {
                    1
                } else {
                    0
                };
            } else if num2 > g.liquid.water_line {
                b = 1;
            }
            // `WorldGen.generatingWorld && DungeonUtils.InAnyPotentialDungeonBounds(num, num2) && DungeonUtils.IsConsideredDungeonWall(tile.wall, allDungeons: true)`
            if g.world_gen.generating_world && unported!("DungeonUtils.InAnyPotentialDungeonBounds", false) && unported!("DungeonUtils.IsConsideredDungeonWall", false) {
                b = 0;
            }
        }
        let mut num4: i32 = -1;
        let mut num5: i32 = 0;
        let mut num6: i32 = -1;
        let mut num7: i32 = 0;
        let mut flag5 = false;
        let mut flag6 = false;
        let mut flag7 = false;
        loop {
            if lq(g, (num + num5 * num4, num2)) == 0 {
                num6 = num4;
                num7 = num5;
            }
            if num4 == -1 && num + num5 * num4 < 5 {
                flag6 = true;
            } else if num4 == 1 && num + num5 * num4 > g.main.max_tiles_x - 5 {
                flag5 = true;
            }
            tile2 = (num + num5 * num4, num2 + 1);
            if lq(g, tile2) != 0 && lq(g, tile2) != u8::MAX && lt(g, tile2) == b {
                let mut num8: i32 = 255 - lq(g, tile2) as i32;
                if num8 > num3 {
                    num8 = num3;
                }
                let v = lq(g, tile2).wrapping_add(num8 as u8);
                set_lq(g, tile2, v);
                num3 -= num8;
                if num3 == 0 {
                    break;
                }
            }
            if num2 < g.main.max_tiles_y - 5 && lq(g, tile2) == 0 && !blocks(g, tile2) {
                flag7 = true;
                break;
            }
            let tile3: P = (num + (num5 + 1) * num4, num2);
            if (lq(g, tile3) != 0 && (!flag3 || num4 != 1)) || blocks(g, tile3) {
                if num4 == 1 {
                    flag5 = true;
                } else {
                    flag6 = true;
                }
            }
            if flag6 && flag5 {
                break;
            }
            if flag5 {
                num4 = -1;
                num5 += 1;
            } else if flag6 {
                if num4 == 1 {
                    num5 += 1;
                }
                num4 = 1;
            } else {
                if num4 == 1 {
                    num5 += 1;
                }
                num4 = -num4;
            }
        }
        num += num7 * num6;
        if num3 == 0 || !flag7 {
            break;
        }
        num2 += 1;
    }
    set_lq(g, (num, num2), num3 as u8);
    set_liquid_type(g, (num, num2), b as i32);
    if lq(g, (num, num2)) > 0 {
        attempt_to_move_lava(g, num, num2, tile_at_xy_has_lava);
        attempt_to_move_honey(g, num, num2, flag);
        attempt_to_move_shimmer(g, num, num2, flag2);
    }
    tiles_ignore_water(g, false);
}

/// `Liquid.AttemptToMoveHoney(int X, int Y, bool tileAtXYHasHoney)` (Liquid.cs:291)
fn attempt_to_move_honey(g: &mut Game, x: i32, y: i32, tile_at_xy_has_honey: bool) {
    if lq(g, (x - 1, y)) > 0 && g.main.tile.get(x - 1, y).honey() != tile_at_xy_has_honey {
        if tile_at_xy_has_honey {
            honey_check(g, x, y);
        } else {
            honey_check(g, x - 1, y);
        }
    } else if lq(g, (x + 1, y)) > 0 && g.main.tile.get(x + 1, y).honey() != tile_at_xy_has_honey {
        if tile_at_xy_has_honey {
            honey_check(g, x, y);
        } else {
            honey_check(g, x + 1, y);
        }
    } else if lq(g, (x, y - 1)) > 0 && g.main.tile.get(x, y - 1).honey() != tile_at_xy_has_honey {
        if tile_at_xy_has_honey {
            honey_check(g, x, y);
        } else {
            honey_check(g, x, y - 1);
        }
    } else if lq(g, (x, y + 1)) > 0 && g.main.tile.get(x, y + 1).honey() != tile_at_xy_has_honey {
        if tile_at_xy_has_honey {
            honey_check(g, x, y);
        } else {
            honey_check(g, x, y + 1);
        }
    }
}

/// `Liquid.AttemptToMoveLava(int X, int Y, bool tileAtXYHasLava)` (Liquid.cs:339)
fn attempt_to_move_lava(g: &mut Game, x: i32, y: i32, tile_at_xy_has_lava: bool) {
    if lq(g, (x - 1, y)) > 0 && g.main.tile.get(x - 1, y).lava() != tile_at_xy_has_lava {
        if tile_at_xy_has_lava {
            lava_check(g, x, y);
        } else {
            lava_check(g, x - 1, y);
        }
    } else if lq(g, (x + 1, y)) > 0 && g.main.tile.get(x + 1, y).lava() != tile_at_xy_has_lava {
        if tile_at_xy_has_lava {
            lava_check(g, x, y);
        } else {
            lava_check(g, x + 1, y);
        }
    } else if lq(g, (x, y - 1)) > 0 && g.main.tile.get(x, y - 1).lava() != tile_at_xy_has_lava {
        if tile_at_xy_has_lava {
            lava_check(g, x, y);
        } else {
            lava_check(g, x, y - 1);
        }
    } else if lq(g, (x, y + 1)) > 0 && g.main.tile.get(x, y + 1).lava() != tile_at_xy_has_lava {
        if tile_at_xy_has_lava {
            lava_check(g, x, y);
        } else {
            lava_check(g, x, y + 1);
        }
    }
}

/// `Liquid.AttemptToMoveShimmer(int X, int Y, bool tileAtXYHasShimmer)` (Liquid.cs:387)
fn attempt_to_move_shimmer(g: &mut Game, x: i32, y: i32, tile_at_xy_has_shimmer: bool) {
    if lq(g, (x - 1, y)) > 0 && g.main.tile.get(x - 1, y).shimmer() != tile_at_xy_has_shimmer {
        if tile_at_xy_has_shimmer {
            shimmer_check(g, x, y);
        } else {
            shimmer_check(g, x - 1, y);
        }
    } else if lq(g, (x + 1, y)) > 0 && g.main.tile.get(x + 1, y).shimmer() != tile_at_xy_has_shimmer {
        if tile_at_xy_has_shimmer {
            shimmer_check(g, x, y);
        } else {
            shimmer_check(g, x + 1, y);
        }
    } else if lq(g, (x, y - 1)) > 0 && g.main.tile.get(x, y - 1).shimmer() != tile_at_xy_has_shimmer {
        if tile_at_xy_has_shimmer {
            shimmer_check(g, x, y);
        } else {
            shimmer_check(g, x, y - 1);
        }
    } else if lq(g, (x, y + 1)) > 0 && g.main.tile.get(x, y + 1).shimmer() != tile_at_xy_has_shimmer {
        if tile_at_xy_has_shimmer {
            shimmer_check(g, x, y);
        } else {
            shimmer_check(g, x, y + 1);
        }
    }
}

/// `Liquid.UpdateProgressDisplay(int verbose, int minY, int maxY, int y)` (Liquid.cs:435): only sets `Main.statusText`, which is
/// the world-loading progress text and has no counterpart yet.
fn update_progress_display(verbose: i32, _min_y: i32, _max_y: i32, _y: i32) {
    if verbose > 0 || verbose < 0 {
        unported!("Main.statusText (Liquid.UpdateProgressDisplay)");
    }
}

/// `Liquid.Update()` (Liquid.cs:451): the instance method on `Main.liquid[idx]`.
pub fn update(g: &mut Game, idx: usize) {
    let (x, y) = (g.liquid.liquid[idx].x, g.liquid.liquid[idx].y);
    g.main.tile_solid[379] = true;
    let tile: P = (x - 1, y);
    let tile2: P = (x + 1, y);
    let tile3: P = (x, y - 1);
    let tile4: P = (x, y + 1);
    let tile5: P = (x, y);
    if blocks(g, tile5) {
        // `_ = tile5.type; _ = 10;` has no effect
        g.liquid.liquid[idx].kill = 999;
        return;
    }
    let liquid = lq(g, tile5);
    let mut num: f32; // `float num = 0f;` in the original; every read below follows an assignment
    if y > g.main.underworld_layer() && lt(g, tile5) == 0 && lq(g, tile5) > 0 {
        let mut b: u8 = 2;
        if lq(g, tile5) < b {
            b = lq(g, tile5);
        }
        let v = lq(g, tile5).wrapping_sub(b);
        set_lq(g, tile5, v);
    }
    if lq(g, tile5) == 0 {
        g.liquid.liquid[idx].kill = 999;
        return;
    }
    if g.main.tile.get(tile5.0, tile5.1).lava() {
        lava_check(g, x, y);
        if !g.liquid.quick_fall {
            if g.liquid.liquid[idx].delay < 5 {
                g.liquid.liquid[idx].delay += 1;
                return;
            }
            g.liquid.liquid[idx].delay = 0;
        }
    } else {
        if g.main.tile.get(tile.0, tile.1).lava() {
            add_water(g, x - 1, y);
        }
        if g.main.tile.get(tile2.0, tile2.1).lava() {
            add_water(g, x + 1, y);
        }
        if g.main.tile.get(tile3.0, tile3.1).lava() {
            add_water(g, x, y - 1);
        }
        if g.main.tile.get(tile4.0, tile4.1).lava() {
            add_water(g, x, y + 1);
        }
        if g.main.tile.get(tile5.0, tile5.1).honey() {
            honey_check(g, x, y);
            if !g.liquid.quick_fall {
                if g.liquid.liquid[idx].delay < 10 {
                    g.liquid.liquid[idx].delay += 1;
                    return;
                }
                g.liquid.liquid[idx].delay = 0;
            }
        } else {
            if g.main.tile.get(tile.0, tile.1).honey() {
                add_water(g, x - 1, y);
            }
            if g.main.tile.get(tile2.0, tile2.1).honey() {
                add_water(g, x + 1, y);
            }
            if g.main.tile.get(tile3.0, tile3.1).honey() {
                add_water(g, x, y - 1);
            }
            if g.main.tile.get(tile4.0, tile4.1).honey() {
                add_water(g, x, y + 1);
            }
            if g.main.tile.get(tile5.0, tile5.1).shimmer() {
                shimmer_check(g, x, y);
            } else {
                if g.main.tile.get(tile.0, tile.1).shimmer() {
                    add_water(g, x - 1, y);
                }
                if g.main.tile.get(tile2.0, tile2.1).shimmer() {
                    add_water(g, x + 1, y);
                }
                if g.main.tile.get(tile3.0, tile3.1).shimmer() {
                    add_water(g, x, y - 1);
                }
                if g.main.tile.get(tile4.0, tile4.1).shimmer() {
                    add_water(g, x, y + 1);
                }
            }
        }
    }
    if !blocks(g, tile4) && (lq(g, tile4) == 0 || lt(g, tile4) == lt(g, tile5)) && lq(g, tile4) < u8::MAX {
        let mut flag = false;
        num = (255 - lq(g, tile4) as i32) as f32;
        if num > lq(g, tile5) as i32 as f32 {
            num = lq(g, tile5) as i32 as f32;
        }
        if num == 1.0 && lq(g, tile5) == u8::MAX {
            flag = true;
        }
        if !flag {
            let v = lq(g, tile5).wrapping_sub(num as u8);
            set_lq(g, tile5, v);
        }
        let v = lq(g, tile4).wrapping_add(num as u8);
        set_lq(g, tile4, v);
        let t5 = lt(g, tile5) as i32;
        set_liquid_type(g, tile4, t5);
        add_water(g, x, y + 1);
        g.main.tile.get_mut(tile4.0, tile4.1).set_skip_liquid(true);
        g.main.tile.get_mut(tile5.0, tile5.1).set_skip_liquid(true);
        if g.liquid.quick_settle && lq(g, tile5) > 250 {
            set_lq(g, tile5, u8::MAX);
        } else if !flag {
            add_water(g, x - 1, y);
            add_water(g, x + 1, y);
        }
    }
    if lq(g, tile5) > 0 {
        let mut flag2 = true;
        let mut flag3 = true;
        let mut flag4 = true;
        let mut flag5 = true;
        if blocks(g, tile) {
            flag2 = false;
        } else if lq(g, tile) > 0 && lt(g, tile) != lt(g, tile5) {
            flag2 = false;
        } else if blocks(g, (x - 2, y)) {
            flag4 = false;
        } else if lq(g, (x - 2, y)) == 0 {
            flag4 = false;
        } else if lq(g, (x - 2, y)) > 0 && lt(g, (x - 2, y)) != lt(g, tile5) {
            flag4 = false;
        }
        if blocks(g, tile2) {
            flag3 = false;
        } else if lq(g, tile2) > 0 && lt(g, tile2) != lt(g, tile5) {
            flag3 = false;
        } else if blocks(g, (x + 2, y)) {
            flag5 = false;
        } else if lq(g, (x + 2, y)) == 0 {
            flag5 = false;
        } else if lq(g, (x + 2, y)) > 0 && lt(g, (x + 2, y)) != lt(g, tile5) {
            flag5 = false;
        }
        let mut num2: i32 = 0;
        if lq(g, tile5) < 3 {
            num2 = -1;
        }
        if lq(g, tile5) > 250 {
            flag4 = false;
            flag5 = false;
        }
        if flag2 && flag3 {
            if flag4 && flag5 {
                let mut flag6 = true;
                let mut flag7 = true;
                if blocks(g, (x - 3, y)) {
                    flag6 = false;
                } else if lq(g, (x - 3, y)) == 0 {
                    flag6 = false;
                } else if lt(g, (x - 3, y)) != lt(g, tile5) {
                    flag6 = false;
                }
                if blocks(g, (x + 3, y)) {
                    flag7 = false;
                } else if lq(g, (x + 3, y)) == 0 {
                    flag7 = false;
                } else if lt(g, (x + 3, y)) != lt(g, tile5) {
                    flag7 = false;
                }
                if flag6 && flag7 {
                    num = (lq(g, tile) as i32 + lq(g, tile2) as i32 + lq(g, (x - 2, y)) as i32 + lq(g, (x + 2, y)) as i32 + lq(g, (x - 3, y)) as i32 + lq(g, (x + 3, y)) as i32 + lq(g, tile5) as i32 + num2) as f32;
                    num = ((num / 7.0) as f64).round_ties_even() as f32; // (float)Math.Round(num / 7f)
                    let mut num3 = 0;
                    let t5 = lt(g, tile5) as i32;
                    set_liquid_type(g, tile, t5);
                    if lq(g, tile) != num as u8 {
                        set_lq(g, tile, num as u8);
                        add_water(g, x - 1, y);
                    } else {
                        num3 += 1;
                    }
                    let t5 = lt(g, tile5) as i32;
                    set_liquid_type(g, tile2, t5);
                    if lq(g, tile2) != num as u8 {
                        set_lq(g, tile2, num as u8);
                        add_water(g, x + 1, y);
                    } else {
                        num3 += 1;
                    }
                    let t5 = lt(g, tile5) as i32;
                    set_liquid_type(g, (x - 2, y), t5);
                    if lq(g, (x - 2, y)) != num as u8 {
                        set_lq(g, (x - 2, y), num as u8);
                        add_water(g, x - 2, y);
                    } else {
                        num3 += 1;
                    }
                    let t5 = lt(g, tile5) as i32;
                    set_liquid_type(g, (x + 2, y), t5);
                    if lq(g, (x + 2, y)) != num as u8 {
                        set_lq(g, (x + 2, y), num as u8);
                        add_water(g, x + 2, y);
                    } else {
                        num3 += 1;
                    }
                    let t5 = lt(g, tile5) as i32;
                    set_liquid_type(g, (x - 3, y), t5);
                    if lq(g, (x - 3, y)) != num as u8 {
                        set_lq(g, (x - 3, y), num as u8);
                        add_water(g, x - 3, y);
                    } else {
                        num3 += 1;
                    }
                    let t5 = lt(g, tile5) as i32;
                    set_liquid_type(g, (x + 3, y), t5);
                    if lq(g, (x + 3, y)) != num as u8 {
                        set_lq(g, (x + 3, y), num as u8);
                        add_water(g, x + 3, y);
                    } else {
                        num3 += 1;
                    }
                    if lq(g, tile) != num as u8 || lq(g, tile5) != num as u8 {
                        add_water(g, x - 1, y);
                    }
                    if lq(g, tile2) != num as u8 || lq(g, tile5) != num as u8 {
                        add_water(g, x + 1, y);
                    }
                    if lq(g, (x - 2, y)) != num as u8 || lq(g, tile5) != num as u8 {
                        add_water(g, x - 2, y);
                    }
                    if lq(g, (x + 2, y)) != num as u8 || lq(g, tile5) != num as u8 {
                        add_water(g, x + 2, y);
                    }
                    if lq(g, (x - 3, y)) != num as u8 || lq(g, tile5) != num as u8 {
                        add_water(g, x - 3, y);
                    }
                    if lq(g, (x + 3, y)) != num as u8 || lq(g, tile5) != num as u8 {
                        add_water(g, x + 3, y);
                    }
                    if num3 != 6 || lq(g, tile3) <= 0 {
                        set_lq(g, tile5, num as u8);
                    }
                } else {
                    let mut num4 = 0;
                    num = (lq(g, tile) as i32 + lq(g, tile2) as i32 + lq(g, (x - 2, y)) as i32 + lq(g, (x + 2, y)) as i32 + lq(g, tile5) as i32 + num2) as f32;
                    num = ((num / 5.0) as f64).round_ties_even() as f32; // (float)Math.Round(num / 5f)
                    let t5 = lt(g, tile5) as i32;
                    set_liquid_type(g, tile, t5);
                    if lq(g, tile) != num as u8 {
                        set_lq(g, tile, num as u8);
                        add_water(g, x - 1, y);
                    } else {
                        num4 += 1;
                    }
                    let t5 = lt(g, tile5) as i32;
                    set_liquid_type(g, tile2, t5);
                    if lq(g, tile2) != num as u8 {
                        set_lq(g, tile2, num as u8);
                        add_water(g, x + 1, y);
                    } else {
                        num4 += 1;
                    }
                    let t5 = lt(g, tile5) as i32;
                    set_liquid_type(g, (x - 2, y), t5);
                    if lq(g, (x - 2, y)) != num as u8 {
                        set_lq(g, (x - 2, y), num as u8);
                        add_water(g, x - 2, y);
                    } else {
                        num4 += 1;
                    }
                    let t5 = lt(g, tile5) as i32;
                    set_liquid_type(g, (x + 2, y), t5);
                    if lq(g, (x + 2, y)) != num as u8 {
                        set_lq(g, (x + 2, y), num as u8);
                        add_water(g, x + 2, y);
                    } else {
                        num4 += 1;
                    }
                    if lq(g, tile) != num as u8 || lq(g, tile5) != num as u8 {
                        add_water(g, x - 1, y);
                    }
                    if lq(g, tile2) != num as u8 || lq(g, tile5) != num as u8 {
                        add_water(g, x + 1, y);
                    }
                    if lq(g, (x - 2, y)) != num as u8 || lq(g, tile5) != num as u8 {
                        add_water(g, x - 2, y);
                    }
                    if lq(g, (x + 2, y)) != num as u8 || lq(g, tile5) != num as u8 {
                        add_water(g, x + 2, y);
                    }
                    if num4 != 4 || lq(g, tile3) <= 0 {
                        set_lq(g, tile5, num as u8);
                    }
                }
            } else if flag4 {
                num = (lq(g, tile) as i32 + lq(g, tile2) as i32 + lq(g, (x - 2, y)) as i32 + lq(g, tile5) as i32 + num2) as f32;
                num = ((num / 4.0) as f64).round_ties_even() as f32; // (float)Math.Round(num / 4f)
                let t5 = lt(g, tile5) as i32;
                set_liquid_type(g, tile, t5);
                if lq(g, tile) != num as u8 || lq(g, tile5) != num as u8 {
                    set_lq(g, tile, num as u8);
                    add_water(g, x - 1, y);
                }
                let t5 = lt(g, tile5) as i32;
                set_liquid_type(g, tile2, t5);
                if lq(g, tile2) != num as u8 || lq(g, tile5) != num as u8 {
                    set_lq(g, tile2, num as u8);
                    add_water(g, x + 1, y);
                }
                let t5 = lt(g, tile5) as i32;
                set_liquid_type(g, (x - 2, y), t5);
                if lq(g, (x - 2, y)) != num as u8 || lq(g, tile5) != num as u8 {
                    set_lq(g, (x - 2, y), num as u8);
                    add_water(g, x - 2, y);
                }
                set_lq(g, tile5, num as u8);
            } else if flag5 {
                num = (lq(g, tile) as i32 + lq(g, tile2) as i32 + lq(g, (x + 2, y)) as i32 + lq(g, tile5) as i32 + num2) as f32;
                num = ((num / 4.0) as f64).round_ties_even() as f32; // (float)Math.Round(num / 4f)
                let t5 = lt(g, tile5) as i32;
                set_liquid_type(g, tile, t5);
                if lq(g, tile) != num as u8 || lq(g, tile5) != num as u8 {
                    set_lq(g, tile, num as u8);
                    add_water(g, x - 1, y);
                }
                let t5 = lt(g, tile5) as i32;
                set_liquid_type(g, tile2, t5);
                if lq(g, tile2) != num as u8 || lq(g, tile5) != num as u8 {
                    set_lq(g, tile2, num as u8);
                    add_water(g, x + 1, y);
                }
                let t5 = lt(g, tile5) as i32;
                set_liquid_type(g, (x + 2, y), t5);
                if lq(g, (x + 2, y)) != num as u8 || lq(g, tile5) != num as u8 {
                    set_lq(g, (x + 2, y), num as u8);
                    add_water(g, x + 2, y);
                }
                set_lq(g, tile5, num as u8);
            } else {
                num = (lq(g, tile) as i32 + lq(g, tile2) as i32 + lq(g, tile5) as i32 + num2) as f32;
                num = ((num / 3.0) as f64).round_ties_even() as f32; // (float)Math.Round(num / 3f)
                if num == 254.0 && g.main.rand.next(30) == 0 {
                    num = 255.0;
                }
                let t5 = lt(g, tile5) as i32;
                set_liquid_type(g, tile, t5);
                if lq(g, tile) != num as u8 {
                    set_lq(g, tile, num as u8);
                    add_water(g, x - 1, y);
                }
                let t5 = lt(g, tile5) as i32;
                set_liquid_type(g, tile2, t5);
                if lq(g, tile2) != num as u8 {
                    set_lq(g, tile2, num as u8);
                    add_water(g, x + 1, y);
                }
                set_lq(g, tile5, num as u8);
            }
        } else if flag2 {
            num = (lq(g, tile) as i32 + lq(g, tile5) as i32 + num2) as f32;
            num = ((num / 2.0) as f64).round_ties_even() as f32; // (float)Math.Round(num / 2f)
            if lq(g, tile) != num as u8 {
                set_lq(g, tile, num as u8);
            }
            let t5 = lt(g, tile5) as i32;
            set_liquid_type(g, tile, t5);
            if lq(g, tile5) != num as u8 || lq(g, tile) != num as u8 {
                add_water(g, x - 1, y);
            }
            set_lq(g, tile5, num as u8);
        } else if flag3 {
            num = (lq(g, tile2) as i32 + lq(g, tile5) as i32 + num2) as f32;
            num = ((num / 2.0) as f64).round_ties_even() as f32; // (float)Math.Round(num / 2f)
            if lq(g, tile2) != num as u8 {
                set_lq(g, tile2, num as u8);
            }
            let t5 = lt(g, tile5) as i32;
            set_liquid_type(g, tile2, t5);
            if lq(g, tile5) != num as u8 || lq(g, tile2) != num as u8 {
                add_water(g, x + 1, y);
            }
            set_lq(g, tile5, num as u8);
        }
    }
    if lq(g, tile5) != liquid {
        if lq(g, tile5) == 254 && liquid == u8::MAX {
            if g.liquid.quick_settle {
                set_lq(g, tile5, u8::MAX);
                g.liquid.liquid[idx].kill += 1;
            } else {
                g.liquid.liquid[idx].kill += 1;
            }
        } else {
            add_water(g, x, y - 1);
            g.liquid.liquid[idx].kill = 0;
        }
    } else {
        g.liquid.liquid[idx].kill += 1;
    }
}

/// `Liquid.StartPanic()` (Liquid.cs:975)
pub fn start_panic(g: &mut Game) {
    if !g.liquid.panic_mode {
        g.liquid.water_line = g.main.max_tiles_y; // GenVars.waterLine
        g.liquid.num_liquid = 0;
        g.liquid.num_liquid_buffer = 0;
        g.liquid.panic_counter = 0;
        g.liquid.panic_mode = true;
        g.liquid.panic_y = g.main.max_tiles_y - 3;
        if g.main.ded_serv {
            unported!("Console.WriteLine(Misc.ForceWaterSettling)");
        }
    }
}

/// `Liquid.UpdateLiquid()` (Liquid.cs:992)
pub fn update_liquid(g: &mut Game) {
    let mut num = 8;
    tiles_ignore_water(g, true);
    if g.main.net_mode == 2 && !g.world_gen.is_generating_or_loading_world {
        let num2 = unported!("Main.player[i].active (count of active players)", 0);
        g.liquid.cycles = 10 + num2 / 3;
        g.liquid.cur_max_liquid = MAX_LIQUID - num2 * 250;
        num = 10 + num2 / 3;
        if g.liquid.setting_use_reduced_max_liquids {
            g.liquid.cur_max_liquid = 5000;
        }
    }
    if !g.world_gen.is_generating_or_loading_world {
        if !g.liquid.panic_mode {
            if g.liquid.num_liquid_buffer as f64 >= MAX_LIQUID_BUFFER as f64 * 0.9 {
                g.liquid.panic_counter += 1;
                if g.liquid.panic_counter > 3600 {
                    start_panic(g);
                }
            } else {
                g.liquid.panic_counter = 0;
            }
        }
        if g.liquid.panic_mode {
            let mut num3 = 0;
            while g.liquid.panic_y >= 3 && num3 < 5 {
                num3 += 1;
                let py = g.liquid.panic_y;
                quick_water(g, 0, py, py);
                g.liquid.panic_y -= 1;
                if g.liquid.panic_y >= 3 {
                    continue;
                }
                unported!("Console.WriteLine(Misc.WaterSettled)");
                g.liquid.panic_counter = 0;
                g.liquid.panic_mode = false;
                water_check::water_check(g);
                if g.main.net_mode != 2 {
                    continue;
                }
                unported!("Netplay.Clients[j].TileSections");
            }
            return;
        }
    }
    let mut flag = g.liquid.quick_settle;
    if g.liquid.setting_use_reduced_max_liquids {
        flag |= g.liquid.num_liquid > 2000;
    }
    if flag {
        g.liquid.quick_fall = true;
    } else {
        g.liquid.quick_fall = false;
    }
    g.liquid.wet_counter += 1;
    let num4 = g.liquid.cur_max_liquid / g.liquid.cycles;
    let num5 = num4 * (g.liquid.wet_counter - 1);
    let mut num6 = num4 * g.liquid.wet_counter;
    if g.liquid.wet_counter == g.liquid.cycles {
        num6 = g.liquid.num_liquid;
    }
    if num6 > g.liquid.num_liquid {
        num6 = g.liquid.num_liquid;
        // `_ = Main.netMode;` has no effect
        g.liquid.wet_counter = g.liquid.cycles;
    }
    if g.liquid.quick_fall {
        let mut m = num5;
        while m < num6 {
            g.liquid.liquid[m as usize].delay = 10;
            update(g, m as usize);
            let (lx, ly) = (g.liquid.liquid[m as usize].x, g.liquid.liquid[m as usize].y);
            g.main.tile.get_mut(lx, ly).set_skip_liquid(false);
            m += 1;
        }
    } else {
        let mut n = num5;
        while n < num6 {
            let (lx, ly) = (g.liquid.liquid[n as usize].x, g.liquid.liquid[n as usize].y);
            if !g.main.tile.get(lx, ly).skip_liquid() {
                update(g, n as usize);
            } else {
                g.main.tile.get_mut(lx, ly).set_skip_liquid(false);
            }
            n += 1;
        }
    }
    if g.liquid.wet_counter >= g.liquid.cycles {
        g.liquid.wet_counter = 0;
        let mut num7 = g.liquid.num_liquid - 1;
        while num7 >= 0 {
            if g.liquid.liquid[num7 as usize].kill >= num {
                let (lx, ly) = (g.liquid.liquid[num7 as usize].x, g.liquid.liquid[num7 as usize].y);
                if g.main.tile.get(lx, ly).liquid == 254 {
                    g.main.tile.get_mut(lx, ly).liquid = u8::MAX;
                }
                del_water(g, num7);
            }
            num7 -= 1;
        }
        let mut num8 = g.liquid.cur_max_liquid - (g.liquid.cur_max_liquid - g.liquid.num_liquid);
        if num8 > g.liquid.num_liquid_buffer {
            num8 = g.liquid.num_liquid_buffer;
        }
        for _num9 in 0..num8 {
            let (bx, by) = (g.liquid.liquid_buffer[0].x, g.liquid.liquid_buffer[0].y);
            g.main.tile.get_mut(bx, by).set_checking_liquid(false);
            add_water(g, bx, by);
            liquid_buffer::del_buffer(g, 0);
        }
        if g.liquid.num_liquid > 0 && g.liquid.num_liquid > g.liquid.stuck_amount - 50 && g.liquid.num_liquid < g.liquid.stuck_amount + 50 {
            g.liquid.stuck_count += 1;
            if g.liquid.stuck_count >= 10000 {
                g.liquid.stuck = true;
                let mut num10 = g.liquid.num_liquid - 1;
                while num10 >= 0 {
                    del_water(g, num10);
                    num10 -= 1;
                }
                g.liquid.stuck = false;
                g.liquid.stuck_count = 0;
            }
        } else {
            g.liquid.stuck_count = 0;
            g.liquid.stuck_amount = g.liquid.num_liquid;
        }
    }
    if !g.world_gen.is_generating_or_loading_world && g.main.net_mode == 2 && !g.liquid.net_change_set.is_empty() {
        // Utils.Swap(ref _netChangeSet, ref _swapNetChangeSet);
        let l = &mut g.liquid;
        std::mem::swap(&mut l.net_change_set, &mut l.swap_net_change_set);
        unported!("NetLiquidModule.CreateAndBroadcastByChunk");
        l.swap_net_change_set.clear();
    }
    tiles_ignore_water(g, false);
}

/// `Liquid.AddWater(int x, int y)` (Liquid.cs:1169)
pub fn add_water(g: &mut Game, x: i32, y: i32) {
    // `Main.tile[x, y] == null`: outside the loaded world
    if !g.main.tile.in_bounds(x, y) {
        return;
    }
    let tile = *g.main.tile.get(x, y);
    if tile.checking_liquid()
        || x >= g.main.max_tiles_x - 5
        || y >= g.main.max_tiles_y - 5
        || x < 5
        || y < 5
        || tile.liquid == 0
        || (tile.nactive() && g.main.tile_solid[tile.type_ as usize] && tile.type_ != 546 && !g.main.tile_solid_top[tile.type_ as usize])
    {
        return;
    }
    if g.liquid.num_liquid >= g.liquid.cur_max_liquid - 1 {
        liquid_buffer::add_buffer(g, x, y);
        return;
    }
    {
        let t = g.main.tile.get_mut(x, y);
        t.set_checking_liquid(true);
        t.set_skip_liquid(false);
    }
    let n = g.liquid.num_liquid as usize;
    g.liquid.liquid[n].kill = 0;
    g.liquid.liquid[n].x = x;
    g.liquid.liquid[n].y = y;
    g.liquid.liquid[n].delay = 0;
    g.liquid.num_liquid += 1;
    if g.main.net_mode == 2 {
        net_send_liquid(g, x, y);
    }
    if !tile.active() || g.world_gen.is_generating_or_loading_world {
        return;
    }
    let mut flag = false;
    if tile.lava() {
        if tile_object_data_check_lava_death(g, &tile) {
            flag = true;
        }
    } else if tile_object_data_check_water_death(g, &tile) {
        flag = true;
    }
    if flag {
        unported!("WorldGen.KillTile"); // neutral: the tile stays
        if g.main.net_mode == 2 {
            unported!("NetMessage.SendData");
        }
    }
}

/// `Liquid.UndergroundDesertCheck(int x, int y)` (Liquid.cs:1218)
fn underground_desert_check(g: &Game, x: i32, y: i32) -> bool {
    let num = 3;
    let mut i = x - num;
    while i <= x + num {
        let mut j = y - num;
        while j <= y + num {
            if in_world(g, i, j, 0) && (g.main.tile.get(i, j).wall == 187 || g.main.tile.get(i, j).wall == 216) {
                return true;
            }
            j += 1;
        }
        i += 1;
    }
    false
}

/// `Liquid.LiquidCheck(int x, int y, int thisLiquidType, bool createMergeTilesDuringGen = false)` (Liquid.cs:1234)
pub fn liquid_check(g: &mut Game, x: i32, y: i32, this_liquid_type: i32, create_merge_tiles_during_gen: bool) {
    if !in_world(g, x, y, 1) || solid_tile(g, x, y, false) {
        return;
    }
    let tile: P = (x - 1, y);
    let tile2: P = (x + 1, y);
    let tile3: P = (x, y - 1);
    let tile4: P = (x, y + 1);
    let tile5: P = (x, y);
    if (lq(g, tile) > 0 && lt(g, tile) as i32 != this_liquid_type) || (lq(g, tile2) > 0 && lt(g, tile2) as i32 != this_liquid_type) || (lq(g, tile3) > 0 && lt(g, tile3) as i32 != this_liquid_type) {
        let any = |g: &Game, f: fn(&Tile) -> bool| -> bool { [tile, tile2, tile3].iter().any(|p| lq(g, *p) > 0 && f(g.main.tile.get(p.0, p.1))) };
        let water_nearby = any(g, |t| t.liquid_type() == 0);
        let lava_nearby = any(g, |t| t.lava());
        let honey_nearby = any(g, |t| t.honey());
        let shimmer_nearby = any(g, |t| t.shimmer());
        let mut num: i32 = 0;
        if lt(g, tile) as i32 != this_liquid_type {
            num += lq(g, tile) as i32;
            set_lq(g, tile, 0);
        }
        if lt(g, tile2) as i32 != this_liquid_type {
            num += lq(g, tile2) as i32;
            set_lq(g, tile2, 0);
        }
        if lt(g, tile3) as i32 != this_liquid_type {
            num += lq(g, tile3) as i32;
            set_lq(g, tile3, 0);
        }
        let mut liquid_merge_tile_type = 56;
        let mut liquid_merge_type = 0;
        get_liquid_merge_types(this_liquid_type, &mut liquid_merge_tile_type, &mut liquid_merge_type, water_nearby, lava_nearby, honey_nearby, shimmer_nearby);
        let t5 = *g.main.tile.get(tile5.0, tile5.1);
        if num >= 24 && liquid_merge_type != this_liquid_type && (!t5.active() || g.main.tile_obsidian_kill[t5.type_ as usize]) {
            set_lq(g, tile5, 0);
            create_liquid_merge_tile(g, x, y, this_liquid_type, liquid_merge_type, liquid_merge_tile_type, create_merge_tiles_during_gen);
        }
    } else {
        if lq(g, tile4) == 0 || lt(g, tile4) as i32 == this_liquid_type {
            return;
        }
        let mut flag = false;
        let t5 = *g.main.tile.get(tile5.0, tile5.1);
        let t4 = *g.main.tile.get(tile4.0, tile4.1);
        if t5.active() && terraria_data::sets::tile_id::sets::IS_A_CONTAINER[t5.type_ as usize] && !terraria_data::sets::tile_id::sets::IS_A_CONTAINER[t4.type_ as usize] {
            flag = true;
        }
        if this_liquid_type != 0 && g.main.tile_cut[t4.type_ as usize] {
            unported!("WorldGen.KillTile"); // neutral: the tile stays
            if g.main.net_mode == 2 {
                unported!("NetMessage.SendData");
            }
        }
        // `Main.tile[x, y + 1]` is read again: KillTile may have changed it
        let t4 = *g.main.tile.get(tile4.0, tile4.1);
        if !(!t4.active() || g.main.tile_obsidian_kill[t4.type_ as usize] || flag) {
            return;
        }
        if lq(g, tile5) < 24 {
            set_lq(g, tile5, 0);
            set_liquid_type(g, tile5, 0);
            if g.main.net_mode == 2 {
                unported!("NetMessage.SendTileSquare");
            }
            return;
        }
        let mut liquid_merge_tile_type2 = 56;
        let mut liquid_merge_type2 = 0;
        let water_nearby2 = t4.liquid_type() == 0; // Tile.water()
        let lava_nearby2 = t4.lava();
        let honey_nearby2 = t4.honey();
        let shimmer_nearby2 = t4.shimmer();
        get_liquid_merge_types(this_liquid_type, &mut liquid_merge_tile_type2, &mut liquid_merge_type2, water_nearby2, lava_nearby2, honey_nearby2, shimmer_nearby2);
        set_lq(g, tile5, 0);
        set_lq(g, tile4, 0);
        create_liquid_merge_tile(g, x, y + 1, this_liquid_type, liquid_merge_type2, liquid_merge_tile_type2, create_merge_tiles_during_gen);
    }
}

/// `Liquid.LiquidOverwriteStrip(int x, int y, int liquidType)` (Liquid.cs:1322)
fn liquid_overwrite_strip(g: &mut Game, x: i32, y: i32, liquid_type: i32) {
    let mut num = x;
    while num >= 0 && g.main.tile.get(num, y).liquid > 0 {
        set_liquid_type(g, (num, y), liquid_type);
        num -= 1;
    }
    let mut i = x;
    while i < g.main.max_tiles_x && g.main.tile.get(i, y).liquid > 0 {
        set_liquid_type(g, (i, y), liquid_type);
        i += 1;
    }
}

/// `Liquid.CreateLiquidMergeTile(int x, int y, int thisLiquidType, int liquidMergeType, int liquidMergeTileType, bool createMergeTilesDuringGen)` (Liquid.cs:1336)
fn create_liquid_merge_tile(g: &mut Game, x: i32, y: i32, _this_liquid_type: i32, _liquid_merge_type: i32, liquid_merge_tile_type: i32, create_merge_tiles_during_gen: bool) {
    // `TileChangeType liquidChangeType = WorldGen.GetLiquidChangeType(thisLiquidType, liquidMergeType)` only feeds
    // PlayLiquidChangeSound and NetMessage.SendTileSquare below, both not ported.
    if g.world_gen.is_generating_or_loading_world && !create_merge_tiles_during_gen {
        let mut liquid_type = -1;
        match liquid_merge_tile_type {
            229 => liquid_type = 0,
            230 => liquid_type = 1,
            56 => liquid_type = 1,
            659 => liquid_type = 3,
            _ => {}
        }
        if y >= g.main.underworld_layer() {
            liquid_type = 1;
        }
        liquid_overwrite_strip(g, x, y, liquid_type);
    } else {
        if !g.main.game_menu && !g.world_gen.is_generating_or_loading_world {
            unported!("WorldGen.PlayLiquidChangeSound");
        }
        let mut t = true;
        std::mem::swap(&mut t, &mut g.main.tile_solid[546]); // Utils.Swap(ref t, ref Main.tileSolid[546])
        // `!tile.active() || !WorldGen.ReplaceTile(x, y, liquidMergeTileType, 0)`; neutral for ReplaceTile: false (nothing replaced)
        if !g.main.tile.get(x, y).active() || !unported!("WorldGen.ReplaceTile", false) {
            unported!("WorldGen.KillTile");
            unported!("WorldGen.PlaceTile");
        }
        water_check::square_tile_frame(g, x, y, true);
        g.main.tile_solid[546] = t;
        if g.main.net_mode == 2 {
            unported!("NetMessage.SendTileSquare");
        }
    }
}

/// `Liquid.GetLiquidMergeTypes(...)` (Liquid.cs:1386)
pub fn get_liquid_merge_types(this_liquid_type: i32, liquid_merge_tile_type: &mut i32, liquid_merge_type: &mut i32, water_nearby: bool, lava_nearby: bool, honey_nearby: bool, shimmer_nearby: bool) {
    *liquid_merge_tile_type = 56;
    *liquid_merge_type = this_liquid_type;
    if this_liquid_type != 0 && water_nearby {
        match this_liquid_type {
            1 => *liquid_merge_tile_type = 56,
            2 => *liquid_merge_tile_type = 229,
            3 => *liquid_merge_tile_type = 659,
            _ => {}
        }
        *liquid_merge_type = 0;
    }
    if this_liquid_type != 1 && lava_nearby {
        match this_liquid_type {
            0 => *liquid_merge_tile_type = 56,
            2 => *liquid_merge_tile_type = 230,
            3 => *liquid_merge_tile_type = 659,
            _ => {}
        }
        *liquid_merge_type = 1;
    }
    if this_liquid_type != 2 && honey_nearby {
        match this_liquid_type {
            0 => *liquid_merge_tile_type = 229,
            1 => *liquid_merge_tile_type = 230,
            3 => *liquid_merge_tile_type = 659,
            _ => {}
        }
        *liquid_merge_type = 2;
    }
    if this_liquid_type != 3 && shimmer_nearby {
        match this_liquid_type {
            0 => *liquid_merge_tile_type = 659,
            1 => *liquid_merge_tile_type = 659,
            2 => *liquid_merge_tile_type = 659,
            _ => {}
        }
        *liquid_merge_type = 3;
    }
}

/// `Liquid.LavaCheck(int x, int y)` (Liquid.cs:1456)
pub fn lava_check(g: &mut Game, x: i32, y: i32) {
    if !g.liquid.remix_world_gen && !g.liquid.dual_dungeons_seed && g.world_gen.generating_world && underground_desert_check(g, x, y) {
        let mut i = x - 3;
        while i <= x + 3 {
            let mut j = y - 3;
            while j <= y + 3 {
                g.main.tile.get_mut(i, j).set_liquid_type(1); // Tile.lava(lava: true)
                j += 1;
            }
            i += 1;
        }
    }
    liquid_check(g, x, y, 1, false);
}

/// `Liquid.HoneyCheck(int x, int y)` (Liquid.cs:1471)
pub fn honey_check(g: &mut Game, x: i32, y: i32) {
    liquid_check(g, x, y, 2, false);
}

/// `Liquid.ShimmerCheck(int x, int y)` (Liquid.cs:1476)
pub fn shimmer_check(g: &mut Game, x: i32, y: i32) {
    liquid_check(g, x, y, 3, false);
}

/// `Liquid.DelWater(int l)` (Liquid.cs:1481)
pub fn del_water(g: &mut Game, l: i32) {
    let num = g.liquid.liquid[l as usize].x;
    let num2 = g.liquid.liquid[l as usize].y;
    let tile: P = (num - 1, num2);
    let tile2: P = (num + 1, num2);
    let tile3: P = (num, num2 + 1);
    let tile4: P = (num, num2);
    let b: u8 = 2;
    if lq(g, tile4) < b {
        set_lq(g, tile4, 0);
        if lq(g, tile) < b {
            set_lq(g, tile, 0);
        } else {
            add_water(g, num - 1, num2);
        }
        if lq(g, tile2) < b {
            set_lq(g, tile2, 0);
        } else {
            add_water(g, num + 1, num2);
        }
    } else if lq(g, tile4) < 20 {
        if (lq(g, tile) < lq(g, tile4) && !blocks(g, tile)) || (lq(g, tile2) < lq(g, tile4) && !blocks(g, tile2)) || (lq(g, tile3) < u8::MAX && !blocks(g, tile3)) {
            set_lq(g, tile4, 0);
        }
    } else if lq(g, tile3) < u8::MAX && !blocks(g, tile3) && !g.liquid.stuck && !blocks(g, (num, num2)) {
        g.liquid.liquid[l as usize].kill = 0;
        return;
    }
    if lq(g, tile4) < 250 && lq(g, (num, num2 - 1)) > 0 {
        add_water(g, num, num2 - 1);
    }
    if lq(g, tile4) == 0 {
        set_liquid_type(g, tile4, 0);
    } else {
        if lq(g, tile2) > 0 && lq(g, tile2) < 250 && !blocks(g, tile2) && lq(g, tile4) != lq(g, tile2) {
            add_water(g, num + 1, num2);
        }
        if lq(g, tile) > 0 && lq(g, tile) < 250 && !blocks(g, tile) && lq(g, tile4) != lq(g, tile) {
            add_water(g, num - 1, num2);
        }
        if g.main.tile.get(tile4.0, tile4.1).lava() {
            lava_check(g, num, num2);
            let mut i = num - 1;
            while i <= num + 1 {
                let mut j = num2 - 1;
                while j <= num2 + 1 {
                    let tile5 = *g.main.tile.get(i, j);
                    if !tile5.active() {
                        j += 1;
                        continue;
                    }
                    if tile5.type_ == 2 || tile5.type_ == 23 || tile5.type_ == 109 || tile5.type_ == 199 || tile5.type_ == 477 || tile5.type_ == 492 {
                        g.main.tile.get_mut(i, j).type_ = 0;
                        water_check::square_tile_frame(g, i, j, true);
                        if g.main.net_mode == 2 {
                            unported!("NetMessage.SendTileSquare");
                        }
                    } else if tile5.type_ == 60 || tile5.type_ == 70 || tile5.type_ == 661 || tile5.type_ == 662 {
                        g.main.tile.get_mut(i, j).type_ = 59;
                        water_check::square_tile_frame(g, i, j, true);
                        if g.main.net_mode == 2 {
                            unported!("NetMessage.SendTileSquare");
                        }
                    }
                    j += 1;
                }
                i += 1;
            }
        } else if g.main.tile.get(tile4.0, tile4.1).honey() {
            honey_check(g, num, num2);
        } else if g.main.tile.get(tile4.0, tile4.1).shimmer() {
            shimmer_check(g, num, num2);
        }
    }
    if g.main.net_mode == 2 {
        net_send_liquid(g, num, num2);
    }
    g.liquid.num_liquid -= 1;
    let (lx, ly) = (g.liquid.liquid[l as usize].x, g.liquid.liquid[l as usize].y);
    g.main.tile.get_mut(lx, ly).set_checking_liquid(false);
    let n = g.liquid.num_liquid as usize;
    g.liquid.liquid[l as usize].x = g.liquid.liquid[n].x;
    g.liquid.liquid[l as usize].y = g.liquid.liquid[n].y;
    g.liquid.liquid[l as usize].kill = g.liquid.liquid[n].kill;
    let t4type = g.main.tile.get(tile4.0, tile4.1).type_;
    if g.main.tile_alch[t4type as usize] {
        unported!("WorldGen.CheckAlch");
    } else if t4type == 518 {
        if g.liquid.quick_fall {
            unported!("WorldGen.CheckLilyPad");
        } else if lq(g, (num, num2 + 1)) < u8::MAX || lq(g, (num, num2 - 1)) > 0 {
            water_check::square_tile_frame(g, num, num2, true);
        } else {
            unported!("WorldGen.CheckLilyPad");
        }
    }
}

/// `TileObjectData.GetTileData(Tile getTile)` (TileObjectData.cs:5197), reduced to what the liquid checks read: the
/// `WaterDeath` / `LavaDeath` / `UsesGlobalLiquidChecks` flags of the data that applies (see `liquid_tile_data.rs`).
/// `None` is the original's `null` (not active, or the tile type has no object data).
fn tile_object_data_get_tile_data(tile: &Tile) -> Option<Liq> {
    if !tile.active() {
        return None;
    }
    let data = liquid_tile_data::OBJECT_DATA.binary_search_by_key(&tile.type_, |d| d.type_).ok().map(|i| &liquid_tile_data::OBJECT_DATA[i])?;
    let num = tile.frame_x as i32 / data.coordinate_full_width;
    let num2 = tile.frame_y as i32 / data.coordinate_full_height;
    let mut num3 = data.style_wrap_limit;
    if num3 == 0 {
        num3 = 1;
    }
    let num4 = if !data.style_horizontal { num * num3 + num2 } else { num2 * num3 + num };
    let mut num5 = num4 / data.style_multiplier;
    let mut num6 = num4 % data.style_multiplier;
    if data.has_style_override {
        // `return getStyleOverride(getTile, tileObjectData)` (GetStyle_Stalactite / GetStyle_SmallPiles / GetStyle_Detritus, tile types 165 / 185 /
        // 233) picks one of the type's styles. Every style and alternate of those three types has `UsesGlobalLiquidChecks` and the same
        // `WaterDeath` / `LavaDeath` (refdata/tile_object_data.json), so the checks give the same answer whichever one it picks.
        return Some(data.base);
    }
    let style_line_skip = data.style_line_skip;
    if style_line_skip > 1 {
        if data.style_horizontal {
            num5 = num2 / style_line_skip * num3 + num;
            num6 = num2 % style_line_skip;
        } else {
            num5 = num / style_line_skip * num3 + num2;
            num6 = num % style_line_skip;
        }
    }
    let mut liq = data.base;
    let mut alternates = data.base_alternates;
    if !data.styles.is_empty() && num5 >= 0 && (num5 as usize) < data.styles.len() {
        liq = data.styles[num5 as usize].liq;
        alternates = data.styles[num5 as usize].alternates;
    }
    for alt in alternates {
        if num6 >= alt.style && num6 <= alt.style + alt.random_style_range {
            return Some(alt.liq);
        }
    }
    Some(liq)
}

/// `TileObjectData.CheckWaterDeath(Tile checkTile)` (TileObjectData.cs:5084)
pub fn tile_object_data_check_water_death(g: &Game, check_tile: &Tile) -> bool {
    if !check_tile.active() {
        return false;
    }
    match tile_object_data_get_tile_data(check_tile) {
        None => g.main.tile_water_death[check_tile.type_ as usize],
        Some(d) if d & 4 != 0 => g.main.tile_water_death[check_tile.type_ as usize],
        Some(d) => d & 1 != 0,
    }
}

/// `TileObjectData.CheckLavaDeath(Tile checkTile)` (TileObjectData.cs:5108)
pub fn tile_object_data_check_lava_death(g: &Game, check_tile: &Tile) -> bool {
    if !check_tile.active() {
        return false;
    }
    match tile_object_data_get_tile_data(check_tile) {
        None => g.main.tile_lava_death[check_tile.type_ as usize],
        Some(d) if d & 4 != 0 => g.main.tile_lava_death[check_tile.type_ as usize],
        Some(d) => d & 2 != 0,
    }
}

/// `WorldFile.LoadWorld` (WorldFile.cs:742-769): the liquid settle that follows the file read, from `isGeneratingOrLoadingWorld = true`
/// to `Liquid.quickSettle = false`. `Main.statusText` (the progress text) is not modelled.
pub fn load_world_settle(g: &mut Game) {
    g.world_gen.is_generating_or_loading_world = true;
    g.liquid.water_line = g.main.max_tiles_y; // GenVars.waterLine = Main.maxTilesY
    quick_water(g, 2, -1, -1);
    water_check::water_check(g);
    let mut num4 = 0;
    g.liquid.quick_settle = true;
    let mut num5 = g.liquid.num_liquid + g.liquid.num_liquid_buffer;
    let mut num6 = 0f32;
    while g.liquid.num_liquid > 0 && num4 < 100000 {
        num4 += 1;
        let mut num7 = (num5 - (g.liquid.num_liquid + g.liquid.num_liquid_buffer)) as f32 / num5 as f32;
        if g.liquid.num_liquid + g.liquid.num_liquid_buffer > num5 {
            num5 = g.liquid.num_liquid + g.liquid.num_liquid_buffer;
        }
        if num7 > num6 {
            num6 = num7;
        } else {
            num7 = num6;
        }
        let _ = num7; // only feeds Main.statusText
        update_liquid(g);
    }
    g.liquid.quick_settle = false;
}

/// `WorldFile.LoadWorld` (WorldFile.cs:772-773): `WorldGen.WaterCheck(); WorldGen.isGeneratingOrLoadingWorld = false;`
pub fn load_world_settle_finish(g: &mut Game) {
    water_check::water_check(g);
    g.world_gen.is_generating_or_loading_world = false;
}
