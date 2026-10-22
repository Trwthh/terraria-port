//! `Terraria.LiquidBuffer` (decompile/Terraria/Terraria/LiquidBuffer.cs): the overflow queue of `Liquid.AddWater`.
//! The static `numLiquidBuffer` is `g.liquid.num_liquid_buffer`, `Main.liquidBuffer` is `g.liquid.liquid_buffer`.

use crate::game::Game;
use crate::liquid::MAX_LIQUID_BUFFER;

/// `Terraria.LiquidBuffer` (a struct: `x`, `y`)
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LiquidBuffer {
    pub x: i32,
    pub y: i32,
}

/// `LiquidBuffer.AddBuffer(int x, int y)` (LiquidBuffer.cs:11)
pub fn add_buffer(g: &mut Game, x: i32, y: i32) {
    if g.liquid.num_liquid_buffer < MAX_LIQUID_BUFFER - 2 && !g.main.tile.get(x, y).checking_liquid() {
        g.main.tile.get_mut(x, y).set_checking_liquid(true);
        let n = g.liquid.num_liquid_buffer as usize;
        g.liquid.liquid_buffer[n].x = x;
        g.liquid.liquid_buffer[n].y = y;
        g.liquid.num_liquid_buffer += 1;
    }
}

/// `LiquidBuffer.DelBuffer(int l)` (LiquidBuffer.cs:22)
pub fn del_buffer(g: &mut Game, l: i32) {
    g.liquid.num_liquid_buffer -= 1;
    let n = g.liquid.num_liquid_buffer as usize;
    g.liquid.liquid_buffer[l as usize].x = g.liquid.liquid_buffer[n].x;
    g.liquid.liquid_buffer[l as usize].y = g.liquid.liquid_buffer[n].y;
}
