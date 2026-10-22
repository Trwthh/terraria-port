//! `Terraria.Graphics.Light.TileLightScanner` (TileLightScanner.cs): turns the tiles of an area into the
//! starting light colours and light masks of a `LightMap`.
//!
//! `get_tile_mask`, `light_is_blocked`, `get_tile_light` and the `apply_*` methods were translated from the C#
//! text by a script (statement by statement: same order, same constants, same branches) and then read against
//! the original; local variables keep the decompiler's names. `R`, `G`, `B` became `r`, `g_`, `b` (`g` is the
//! game state), `out`/`ref` parameters became `&mut` parameters, `switch` became `match` (a `break` that is not
//! the last statement of a case became `break 'swN`).
//!
//! Callees that live in other classes of the original are in `light_support.rs`.

#![allow(clippy::needless_return)]

use terraria_core::xna::{math_helper, Color, Rectangle, Vector3, Vector4};
use terraria_core::Tile;
use terraria_data::sets::tile_id;

use super::light_map::LightMap;
use super::light_mask_mode::LightMaskMode;
use super::tile_light_scanner_options::TileLightScannerOptions;
use crate::game::Game;
use crate::light_support::{
    get_portal_color, get_shimmer_base_color, hsl_to_rgb, multiply_rgba, paint_color, solid_tile3, torch_color, FastRandom,
};
use crate::world_gen::tile_frame::in_world;

/// `TileLightScanner`
pub struct TileLightScanner {
    /// `_random` (`FastRandom.CreateWithRandomSeed()`)
    random: FastRandom,
    /// `_drawInvisibleWalls`
    draw_invisible_walls: bool,
}

impl Default for TileLightScanner {
    fn default() -> Self {
        TileLightScanner::new()
    }
}

impl TileLightScanner {
    /// Field initialisers of `TileLightScanner`.
    pub fn new() -> TileLightScanner {
        TileLightScanner { random: FastRandom::create_with_random_seed(), draw_invisible_walls: false }
    }

    /// Seeds `_random` (the original seeds it from a GUID; used to make runs repeatable).
    pub fn set_random_seed(&mut self, seed: u64) {
        self.random = FastRandom::new(seed);
    }

    /// `_random` as it is now.
    pub fn random(&self) -> FastRandom {
        self.random
    }

    /// `ExportTo(Rectangle area, LightMap outputMap, TileLightScannerOptions options)` (TileLightScanner.cs:17).
    ///
    /// The original runs the body of the `for i` loop inside `FastParallel.For(area.Left, area.Right, ...)`.
    /// It is run here as a plain loop in index order. Iteration `i` writes only the cells
    /// `outputMap[i - area.X, *]` (its own column), and reads `Main.tile`, `_random` (never advanced here:
    /// `WithModifier` returns a copy) and `_drawInvisibleWalls`, none of which the iterations change, so the
    /// order does not matter. The one shared mutable thing is `Main.rand`, drawn from in `ApplySurfaceLight`
    /// when `Main.liquidAlpha[13] > 0` (a data race in the original); the sequential order is one of the
    /// outcomes the original can produce.
    pub fn export_to(&mut self, g: &mut Game, area: Rectangle, output_map: &mut LightMap, options: TileLightScannerOptions) {
        self.draw_invisible_walls = options.draw_invisible_walls;
        for i in area.left()..area.right() {
            for j in area.top()..area.bottom() {
                if self.is_tile_null_or_touching_null(g, i, j) {
                    output_map.set_mask_at(i - area.x, j - area.y, LightMaskMode::None);
                    output_map.set(i - area.x, j - area.y, Vector3::ZERO);
                } else {
                    let tile = *g.main.tile.get(i, j);
                    let tile_mask = self.get_tile_mask(g, &tile);
                    output_map.set_mask_at(i - area.x, j - area.y, tile_mask);
                    let mut output_color = Vector3::ZERO;
                    self.get_tile_light(g, i, j, &mut output_color);
                    output_map.set(i - area.x, j - area.y, output_color);
                }
            }
        }
    }

    /// `IsTileNullOrTouchingNull(int x, int y)` (TileLightScanner.cs:43). A tile is "null" when it lies outside
    /// the tile map (the original's `Main.tile[x, y]` is null there).
    fn is_tile_null_or_touching_null(&self, g: &Game, x: i32, y: i32) -> bool {
        if in_world(g, x, y, 1) {
            if g.main.tile.try_get(x, y).is_some()
                && g.main.tile.try_get(x + 1, y).is_some()
                && g.main.tile.try_get(x - 1, y).is_some()
                && g.main.tile.try_get(x, y - 1).is_some()
            {
                return g.main.tile.try_get(x, y + 1).is_none();
            }
            return true;
        }
        true
    }

    /// `Update()` (TileLightScanner.cs:56)
    pub fn update(&mut self) {
        self.random.next_seed();
    }

    /// `GetMaskMode(int x, int y)` (TileLightScanner.cs:61)
    pub fn get_mask_mode(&self, g: &Game, x: i32, y: i32) -> LightMaskMode {
        let tile = *g.main.tile.get(x, y);
        self.get_tile_mask(g, &tile)
    }

    /// `TileLightScanner.GetTileMask` (TileLightScanner.cs:66)
    fn get_tile_mask(&self, g: &Game, tile: &Tile) -> LightMaskMode {
        if self.light_is_blocked(g, tile) && (tile.type_ as i32) != 131 && !tile.in_active() && (tile.slope() as i32) == 0 {
            if tile_id::sets::CRACKED_BRICKS[tile.type_ as usize] {
                return LightMaskMode::CrackedBricks;
            }
            return LightMaskMode::Solid;
        }
        if !tile.lava() && (tile.liquid as i32) > 128 {
            if !tile.honey() {
                return LightMaskMode::Water;
            }
            return LightMaskMode::Honey;
        }
        return LightMaskMode::None;
    }

    /// `TileLightScanner.GetTileLight` (TileLightScanner.cs:87)
    pub fn get_tile_light(&self, g: &mut Game, x: i32, y: i32, output_color: &mut Vector3) {
        *output_color = Vector3::ZERO;
        let tile: Tile = *g.main.tile.get(x, y);
        let mut local_random: FastRandom = self.random.with_modifier_xy(x, y);
        if y <= g.main.world_surface as i32 {
            self.apply_surface_light(g, &tile, x, y, output_color);
        } else if y > g.main.underworld_layer() {
            self.apply_hell_light(g, &tile, x, y, output_color);
        }
        self.apply_wall_light(g, &tile, x, y, &mut local_random, output_color);
        if tile.active() {
            self.apply_tile_light(g, &tile, x, y, &mut local_random, output_color);
        }
        self.apply_liquid_light(g, &tile, output_color);
    }

    /// `TileLightScanner.ApplyLiquidLight` (TileLightScanner.cs:108)
    #[allow(unused_variables, unused_assignments, unused_mut)]
    fn apply_liquid_light(&self, g: &Game, tile: &Tile, light_color: &mut Vector3) {
        if (tile.liquid as i32) <= 0 {
            return;
        }
        if tile.lava() {
            let mut num: f32 = 0.55f32;
            num += (270 - (g.main.mouse_text_color as i32)) as f32 / 900f32;
            if light_color.x < num {
                light_color.x = num;
            }
            if light_color.y < num {
                light_color.y = num * 0.6f32;
            }
            if light_color.z < num {
                light_color.z = num * 0.2f32;
            }
        } else if tile.shimmer() {
            let mut num2: f32 = 0.7f32;
            let mut num3: f32 = 0.7f32;
            num2 += (270 - (g.main.mouse_text_color as i32)) as f32 / 900f32;
            num3 += (270 - (g.main.mouse_text_color as i32)) as f32 / 125f32;
            if light_color.x < num2 {
                light_color.x = num2 * 0.6f32;
            }
            if light_color.y < num3 {
                light_color.y = num3 * 0.25f32;
            }
            if light_color.z < num2 {
                light_color.z = num2 * 0.9f32;
            }
        }
    }

    /// `TileLightScanner.LightIsBlocked` (TileLightScanner.cs:152)
    fn light_is_blocked(&self, g: &Game, tile: &Tile) -> bool {
        if tile.active() && g.main.tile_block_light[tile.type_ as usize] {
            if tile.invisible_block() {
                return self.draw_invisible_walls;
            }
            return true;
        }
        return false;
    }

    /// `TileLightScanner.ApplyWallLight` (TileLightScanner.cs:165)
    #[allow(unused_variables, unused_assignments, unused_mut)]
    fn apply_wall_light(&self, g: &Game, tile: &Tile, x: i32, y: i32, local_random: &mut FastRandom, light_color: &mut Vector3) {
        let mut num: f32 = 0f32;
        let mut num2: f32 = 0f32;
        let mut num3: f32 = 0f32;
        let mut flag: bool = false;
        match tile.wall {
            357 => {
                if !self.light_is_blocked(g, tile) {
                    num = 0.15f32;
                    num2 = 0.27f32;
                    num3 = 0.3f32;
                    flag = true;
                }
            }
            182 => {
                if !self.light_is_blocked(g, tile) {
                    num = 0.24f32;
                    num2 = 0.12f32;
                    num3 = 0.089999996f32;
                }
            }
            33 => {
                if !self.light_is_blocked(g, tile) {
                    num = 0.089999996f32;
                    num2 = 0.052500002f32;
                    num3 = 0.24f32;
                }
            }
            174 => {
                if !self.light_is_blocked(g, tile) {
                    num = 0.2975f32;
                }
            }
            175 => {
                if !self.light_is_blocked(g, tile) {
                    if (tile.wall_color() as i32) == 0 {
                        num = 0.075f32;
                        num2 = 0.15f32;
                        num3 = 0.4f32;
                    } else {
                        flag = true;
                    }
                }
            }
            176 => {
                if !self.light_is_blocked(g, tile) {
                    num = 0.1f32;
                    num2 = 0.1f32;
                    num3 = 0.1f32;
                }
            }
            137 => {
                if !self.light_is_blocked(g, tile) {
                    let mut num4: f32 = 0.4f32;
                    num4 += (270 - (g.main.mouse_text_color as i32)) as f32 / 1500f32;
                    num4 += local_random.next_range(0, 50) as f32 * 0.0005f32;
                    num = 1f32 * num4;
                    num2 = 0.5f32 * num4;
                    num3 = 0.1f32 * num4;
                }
            }
            44 => {
                if !self.light_is_blocked(g, tile) {
                    num = g.main.disco_r as f32 / 255f32 * 0.15f32;
                    num2 = g.main.disco_g as f32 / 255f32 * 0.15f32;
                    num3 = g.main.disco_b as f32 / 255f32 * 0.15f32;
                }
            }
            154 => {
                num = 0.6f32;
                num3 = 0.6f32;
            }
            166 => {
                num = 0.6f32;
                num2 = 0.6f32;
            }
            165 => {
                num3 = 0.6f32;
            }
            156 => {
                num2 = 0.6f32;
            }
            164 => {
                num = 0.6f32;
            }
            155 => {
                num = 0.6f32;
                num2 = 0.6f32;
                num3 = 0.6f32;
            }
            153 => {
                num = 0.6f32;
                num2 = 0.3f32;
            }
            341 => {
                if !self.light_is_blocked(g, tile) {
                    num = 0.25f32;
                    num2 = 0.1f32;
                    num3 = 0f32;
                }
            }
            343 => {
                if !self.light_is_blocked(g, tile) {
                    num = 0f32;
                    num2 = 0.25f32;
                    num3 = 0f32;
                }
            }
            344 => {
                if !self.light_is_blocked(g, tile) {
                    num = 0f32;
                    num2 = 0.16f32;
                    num3 = 0.34f32;
                }
            }
            342 => {
                if !self.light_is_blocked(g, tile) {
                    num = 0.3f32;
                    num2 = 0f32;
                    num3 = 0.17f32;
                }
            }
            345 => {
                if !self.light_is_blocked(g, tile) {
                    num = 0.3f32;
                    num2 = 0f32;
                    num3 = 0.35f32;
                }
            }
            346 => {
                if !self.light_is_blocked(g, tile) {
                    num = g.main.disco_r as f32 / 255f32 * 0.25f32;
                    num2 = g.main.disco_g as f32 / 255f32 * 0.25f32;
                    num3 = g.main.disco_b as f32 / 255f32 * 0.25f32;
                }
            }
            _ => {}
        }
        if flag && (tile.wall_color() as i32) != 0 {
            let color: Color = paint_color(tile.wall_color() as i32);
            num = color.r() as i32 as f32 / 765f32;
            num2 = color.g() as i32 as f32 / 765f32;
            num3 = color.b() as i32 as f32 / 765f32;
        }
        if light_color.x < num {
            light_color.x = num;
        }
        if light_color.y < num2 {
            light_color.y = num2;
        }
        if light_color.z < num3 {
            light_color.z = num3;
        }
    }

    /// `TileLightScanner.ApplyTileLight` (TileLightScanner.cs:342)
    #[allow(unused_variables, unused_assignments, unused_mut)]
    fn apply_tile_light(&self, g: &Game, tile: &Tile, mut x: i32, mut y: i32, local_random: &mut FastRandom, light_color: &mut Vector3) {
        let mut r: f32 = 0f32;
        let mut g_: f32 = 0f32;
        let mut b: f32 = 0f32;
        let mut flag: bool = false;
        if g.main.tile_lighted[tile.type_ as usize] {
            'sw1: {
                match tile.type_ {
                    658 => {
                        if !tile.invisible_block() {
                            torch_color(g, 23, &mut r, &mut g_, &mut b);
                            match (tile.frame_y as i32) / 54 {
                                1 => {
                                    r *= 0.3f32;
                                    g_ *= 0.3f32;
                                    b *= 0.3f32;
                                }
                                2 => {
                                    r *= 0.1f32;
                                    g_ *= 0.1f32;
                                    b *= 0.1f32;
                                }
                                _ => {
                                    r *= 0.2f32;
                                    g_ *= 0.2f32;
                                    b *= 0.2f32;
                                }
                            }
                        }
                    }
                    356 => {
                        if g.main.sundial_cooldown == 0 {
                            r = 0.45f32;
                            g_ = 0.25f32;
                            b = 0f32;
                        }
                    }
                    663 => {
                        if g.main.moondial_cooldown == 0 {
                            r = 0f32;
                            g_ = 0.25f32;
                            b = 0.45f32;
                        }
                    }
                    656 => {
                        r = 0.2f32;
                        g_ = 0.55f32;
                        b = 0.5f32;
                    }
                    20 => {
                        let num28: i32 = (tile.frame_x as i32) / 18;
                        if num28 >= 30 && num28 <= 32 {
                            r = 0.325f32;
                            g_ = 0.15f32;
                            b = 0.05f32;
                        }
                    }
                    634 => {
                        r = 0.65f32;
                        g_ = 0.3f32;
                        b = 0.1f32;
                    }
                    633 | 637 | 638 => {
                        r = 0.325f32;
                        g_ = 0.15f32;
                        b = 0.05f32;
                    }
                    463 => {
                        r = 0.2f32;
                        g_ = 0.4f32;
                        b = 0.8f32;
                    }
                    491 => {
                        r = 0.5f32;
                        g_ = 0.4f32;
                        b = 0.7f32;
                    }
                    209 => {
                        if (tile.frame_x as i32) == 234 || (tile.frame_x as i32) == 252 {
                            let vector11: Vector3 = get_portal_color(g, g.main.my_player, 0).to_vector3() * 0.65f32;
                            r = vector11.x;
                            g_ = vector11.y;
                            b = vector11.z;
                        } else if (tile.frame_x as i32) == 306 || (tile.frame_x as i32) == 324 {
                            let vector12: Vector3 = get_portal_color(g, g.main.my_player, 1).to_vector3() * 0.65f32;
                            r = vector12.x;
                            g_ = vector12.y;
                            b = vector12.z;
                        }
                    }
                    415 => {
                        r = 0.7f32;
                        g_ = 0.5f32;
                        b = 0.1f32;
                    }
                    500 => {
                        r = 0.525f32;
                        g_ = 0.375f32;
                        b = 0.075f32;
                    }
                    416 => {
                        r = 0f32;
                        g_ = 0.6f32;
                        b = 0.7f32;
                    }
                    501 => {
                        r = 0f32;
                        g_ = 0.45f32;
                        b = 0.525f32;
                    }
                    417 => {
                        r = 0.6f32;
                        g_ = 0.2f32;
                        b = 0.6f32;
                    }
                    502 => {
                        r = 0.45f32;
                        g_ = 0.15f32;
                        b = 0.45f32;
                    }
                    418 => {
                        r = 0.6f32;
                        g_ = 0.6f32;
                        b = 0.9f32;
                    }
                    503 => {
                        r = 0.45f32;
                        g_ = 0.45f32;
                        b = 0.675f32;
                    }
                    390 => {
                        r = 0.4f32;
                        g_ = 0.2f32;
                        b = 0.1f32;
                    }
                    597 => {
                        match (tile.frame_x as i32) / 54 {
                            0 => {
                                r = 0.05f32;
                                g_ = 0.8f32;
                                b = 0.3f32;
                            }
                            1 => {
                                r = 0.7f32;
                                g_ = 0.8f32;
                                b = 0.05f32;
                            }
                            2 => {
                                r = 0.7f32;
                                g_ = 0.5f32;
                                b = 0.9f32;
                            }
                            3 => {
                                r = 0.6f32;
                                g_ = 0.6f32;
                                b = 0.8f32;
                            }
                            4 => {
                                r = 0.4f32;
                                g_ = 0.4f32;
                                b = 1.15f32;
                            }
                            5 => {
                                r = 0.85f32;
                                g_ = 0.45f32;
                                b = 0.1f32;
                            }
                            6 => {
                                r = 0.8f32;
                                g_ = 0.8f32;
                                b = 1f32;
                            }
                            7 => {
                                r = 0.5f32;
                                g_ = 0.8f32;
                                b = 1.2f32;
                            }
                            _ => {}
                        }
                        r *= 0.75f32;
                        g_ *= 0.75f32;
                        b *= 0.75f32;
                    }
                    564 => {
                        if (tile.frame_x as i32) < 36 {
                            r = 0.05f32;
                            g_ = 0.3f32;
                            b = 0.55f32;
                        }
                    }
                    568 => {
                        r = 1f32;
                        g_ = 0.61f32;
                        b = 0.65f32;
                    }
                    569 => {
                        r = 0.12f32;
                        g_ = 1f32;
                        b = 0.66f32;
                    }
                    570 => {
                        r = 0.57f32;
                        g_ = 0.57f32;
                        b = 1f32;
                    }
                    580 => {
                        r = 0.7f32;
                        g_ = 0.3f32;
                        b = 0.2f32;
                    }
                    391 => {
                        r = 0.3f32;
                        g_ = 0.1f32;
                        b = 0.25f32;
                    }
                    381 | 517 | 687 => {
                        r = 0.25f32;
                        g_ = 0.1f32;
                        b = 0f32;
                    }
                    534 | 535 | 689 => {
                        r = 0f32;
                        g_ = 0.25f32;
                        b = 0f32;
                    }
                    536 | 537 | 690 => {
                        r = 0f32;
                        g_ = 0.16f32;
                        b = 0.34f32;
                    }
                    539 | 540 | 688 => {
                        r = 0.3f32;
                        g_ = 0f32;
                        b = 0.17f32;
                    }
                    625 | 626 | 691 => {
                        r = 0.3f32;
                        g_ = 0f32;
                        b = 0.35f32;
                    }
                    627 | 628 | 692 => {
                        r = g.main.disco_r as f32 / 255f32 * 0.25f32;
                        g_ = g.main.disco_g as f32 / 255f32 * 0.25f32;
                        b = g.main.disco_b as f32 / 255f32 * 0.25f32;
                    }
                    184 => {
                        if (tile.frame_x as i32) == 110 {
                            r = 0.25f32;
                            g_ = 0.1f32;
                            b = 0f32;
                        }
                        if (tile.frame_x as i32) == 132 {
                            r = 0f32;
                            g_ = 0.25f32;
                            b = 0f32;
                        }
                        if (tile.frame_x as i32) == 154 {
                            r = 0f32;
                            g_ = 0.16f32;
                            b = 0.34f32;
                        }
                        if (tile.frame_x as i32) == 176 {
                            r = 0.3f32;
                            g_ = 0f32;
                            b = 0.17f32;
                        }
                        if (tile.frame_x as i32) == 198 {
                            r = 0.3f32;
                            g_ = 0f32;
                            b = 0.35f32;
                        }
                        if (tile.frame_x as i32) == 220 {
                            r = g.main.disco_r as f32 / 255f32 * 0.25f32;
                            g_ = g.main.disco_g as f32 / 255f32 * 0.25f32;
                            b = g.main.disco_b as f32 / 255f32 * 0.25f32;
                        }
                    }
                    370 => {
                        r = 0.32f32;
                        g_ = 0.16f32;
                        b = 0.12f32;
                    }
                    659 | 667 | 708 => {
                        let shimmer_base_color: Vector4 = get_shimmer_base_color(g, x as f32, y as f32);
                        r = shimmer_base_color.x;
                        g_ = shimmer_base_color.y;
                        b = shimmer_base_color.z;
                    }
                    711 => {
                        r = 0.01f32;
                        g_ = 0.01f32;
                        b = 0.01f32;
                    }
                    27 => {
                        if (tile.frame_y as i32) < 36 {
                            r = 0.3f32;
                            g_ = 0.27f32;
                        }
                    }
                    336 => {
                        r = 0.85f32;
                        g_ = 0.5f32;
                        b = 0.3f32;
                    }
                    340 => {
                        r = 0.45f32;
                        g_ = 1f32;
                        b = 0.45f32;
                    }
                    341 => {
                        r = 0.4f32 * g.main.demon_torch + 0.6f32 * (1f32 - g.main.demon_torch);
                        g_ = 0.35f32;
                        b = 1f32 * g.main.demon_torch + 0.6f32 * (1f32 - g.main.demon_torch);
                    }
                    342 => {
                        r = 0.5f32;
                        g_ = 0.5f32;
                        b = 1.1f32;
                    }
                    343 => {
                        r = 0.85f32;
                        g_ = 0.85f32;
                        b = 0.3f32;
                    }
                    344 => {
                        r = 0.6f32;
                        g_ = 1.026f32;
                        b = 0.96000004f32;
                    }
                    327 => {
                        let mut num13: f32 = 0.5f32;
                        num13 += (270 - (g.main.mouse_text_color as i32)) as f32 / 1500f32;
                        num13 += local_random.next_range(0, 50) as f32 * 0.0005f32;
                        r = 1f32 * num13;
                        g_ = 0.5f32 * num13;
                        b = 0.1f32 * num13;
                    }
                    316 | 317 | 318 => {
                        let num19: i32 = x - (tile.frame_x as i32) / 18;
                        let num20: i32 = y - (tile.frame_y as i32) / 18;
                        let mut num21: i32 = num19 / 3 * (num20 / 3);
                        num21 %= g.main.cage_frames;
                        let num22: i32 = (tile.type_ as i32) - 316;
                        let flag5: bool = g.main.jellyfish_cage_mode[num22 as usize][num21 as usize] == 2;
                        if (tile.type_ as i32) == 316 {
                            if flag5 {
                                r = 0.2f32;
                                g_ = 0.3f32;
                                b = 0.8f32;
                            } else {
                                r = 0.1f32;
                                g_ = 0.2f32;
                                b = 0.5f32;
                            }
                        }
                        if (tile.type_ as i32) == 317 {
                            if flag5 {
                                r = 0.2f32;
                                g_ = 0.7f32;
                                b = 0.3f32;
                            } else {
                                r = 0.05f32;
                                g_ = 0.45f32;
                                b = 0.1f32;
                            }
                        }
                        if (tile.type_ as i32) == 318 {
                            if flag5 {
                                r = 0.7f32;
                                g_ = 0.2f32;
                                b = 0.5f32;
                            } else {
                                r = 0.4f32;
                                g_ = 0.1f32;
                                b = 0.25f32;
                            }
                        }
                    }
                    719 => {
                        let num15: i32 = (x + y + (g.main.global_time_wrapped_hourly * 15f32) as i32) % 14;
                        let mut num16: f32 = 0f32;
                        let mut num17: f32 = 0f32;
                        let mut num18: f32 = 0f32;
                        match num15 {
                            0 => {
                                num16 = 255f32;
                                num17 = 171f32;
                                num18 = 183f32;
                            }
                            1 => {
                                num16 = 255f32;
                                num17 = 170f32;
                                num18 = 220f32;
                            }
                            2 => {
                                num16 = 252f32;
                                num17 = 171f32;
                                num18 = 255f32;
                            }
                            3 => {
                                num16 = 224f32;
                                num17 = 171f32;
                                num18 = 255f32;
                            }
                            4 => {
                                num16 = 192f32;
                                num17 = 171f32;
                                num18 = 255f32;
                            }
                            5 => {
                                num16 = 174f32;
                                num17 = 178f32;
                                num18 = 255f32;
                            }
                            6 => {
                                num16 = 168f32;
                                num17 = 195f32;
                                num18 = 255f32;
                            }
                            7 => {
                                num16 = 167f32;
                                num17 = 224f32;
                                num18 = 255f32;
                            }
                            8 => {
                                num16 = 168f32;
                                num17 = 255f32;
                                num18 = 252f32;
                            }
                            9 => {
                                num16 = 162f32;
                                num17 = 255f32;
                                num18 = 233f32;
                            }
                            10 => {
                                num16 = 158f32;
                                num17 = 255f32;
                                num18 = 198f32;
                            }
                            11 => {
                                num16 = 207f32;
                                num17 = 255f32;
                                num18 = 173f32;
                            }
                            12 => {
                                num16 = 255f32;
                                num17 = 213f32;
                                num18 = 186f32;
                            }
                            13 => {
                                num16 = 255f32;
                                num17 = 192f32;
                                num18 = 182f32;
                            }
                            _ => {}
                        }
                        r = num16 / 255f32;
                        g_ = num17 / 255f32;
                        b = num18 / 255f32;
                    }
                    718 => {
                        if !g.main.day_time && !solid_tile3(g, x, y - 1) {
                            r = local_random.next_float() * 0.04f32 + 0.1f32 + g.main.disco_r as f32 / 800f32;
                            g_ = local_random.next_float() * 0.04f32 + 0.1f32 + g.main.disco_g as f32 / 800f32;
                            b = local_random.next_float() * 0.04f32 + 0.1f32 + g.main.disco_b as f32 / 800f32;
                        }
                    }
                    717 => {
                        let mut num13: f32 = 0.55f32;
                        num13 += (270 - (g.main.mouse_text_color as i32)) as f32 / 800f32;
                        num13 += local_random.next_float() * 0.03f32;
                        num13 *= 0.5f32;
                        r = num13 * 1.1f32;
                        g_ = num13 * 0.4f32;
                        b = num13 * 0.1f32;
                    }
                    429 => {
                        let num8: i32 = (tile.frame_x as i32) / 18;
                        let flag2: bool = num8 % 2 >= 1;
                        let flag3: bool = num8 % 4 >= 2;
                        let flag4: bool = num8 % 8 >= 4;
                        let num9: bool = num8 % 16 >= 8;
                        if flag2 {
                            r += 0.5f32;
                        }
                        if flag3 {
                            g_ += 0.5f32;
                        }
                        if flag4 {
                            b += 0.5f32;
                        }
                        if num9 {
                            r += 0.2f32;
                            g_ += 0.2f32;
                        }
                    }
                    286 | 619 => {
                        r = 0.1f32;
                        g_ = 0.2f32;
                        b = 0.7f32;
                    }
                    620 => {
                        let mut color: Color = multiply_rgba(Color::from_ints(230, 230, 230, 0), hsl_to_rgb(g.main.global_time_wrapped_hourly * 0.5f32 % 1f32, 1f32, 0.5f32, 255));
                        color = color * 0.4f32;
                        r = color.r() as i32 as f32 / 255f32;
                        g_ = color.g() as i32 as f32 / 255f32;
                        b = color.b() as i32 as f32 / 255f32;
                    }
                    582 | 598 => {
                        r = 0.7f32;
                        g_ = 0.2f32;
                        b = 0.1f32;
                    }
                    270 => {
                        r = 0.73f32;
                        g_ = 1f32;
                        b = 0.41f32;
                    }
                    271 => {
                        r = 0.45f32;
                        g_ = 0.95f32;
                        b = 1f32;
                    }
                    581 => {
                        r = 1f32;
                        g_ = 0.75f32;
                        b = 0.5f32;
                    }
                    660 => {
                        torch_color(g, 23, &mut r, &mut g_, &mut b);
                    }
                    572 => {
                        match (tile.frame_y as i32) / 36 {
                            0 => {
                                r = 0.9f32;
                                g_ = 0.5f32;
                                b = 0.7f32;
                            }
                            1 => {
                                r = 0.7f32;
                                g_ = 0.55f32;
                                b = 0.96f32;
                            }
                            2 => {
                                r = 0.45f32;
                                g_ = 0.96f32;
                                b = 0.95f32;
                            }
                            3 => {
                                r = 0.5f32;
                                g_ = 0.96f32;
                                b = 0.62f32;
                            }
                            4 => {
                                r = 0.47f32;
                                g_ = 0.69f32;
                                b = 0.95f32;
                            }
                            5 => {
                                r = 0.92f32;
                                g_ = 0.57f32;
                                b = 0.51f32;
                            }
                            _ => {}
                        }
                    }
                    262 => {
                        r = 0.75f32;
                        b = 0.75f32;
                    }
                    263 => {
                        r = 0.75f32;
                        g_ = 0.75f32;
                    }
                    264 => {
                        b = 0.75f32;
                    }
                    265 => {
                        g_ = 0.75f32;
                    }
                    266 => {
                        r = 0.75f32;
                    }
                    267 => {
                        r = 0.75f32;
                        g_ = 0.75f32;
                        b = 0.75f32;
                    }
                    268 => {
                        r = 0.75f32;
                        g_ = 0.375f32;
                    }
                    237 => {
                        r = 0.1f32;
                        g_ = 0.1f32;
                    }
                    238 => {
                        if (light_color.x as f64) < 0.5f64 {
                            light_color.x = 0.5f32;
                        }
                        if (light_color.z as f64) < 0.5f64 {
                            light_color.z = 0.5f32;
                        }
                    }
                    235 => {
                        if (light_color.x as f64) < 0.6f64 {
                            light_color.x = 0.6f32;
                        }
                        if (light_color.y as f64) < 0.6f64 {
                            light_color.y = 0.6f32;
                        }
                    }
                    405 => {
                        if (tile.frame_x as i32) < 54 {
                            let mut num27: f32 = local_random.next_range(28, 42) as f32 * 0.005f32;
                            num27 += (270 - (g.main.mouse_text_color as i32)) as f32 / 700f32;
                            match (tile.frame_x as i32) / 54 {
                                1 => {
                                    r = 0.7f32;
                                    g_ = 1f32;
                                    b = 0.5f32;
                                }
                                2 => {
                                    r = 0.5f32 * g.main.demon_torch + 1f32 * (1f32 - g.main.demon_torch);
                                    g_ = 0.3f32;
                                    b = 1f32 * g.main.demon_torch + 0.5f32 * (1f32 - g.main.demon_torch);
                                }
                                3 => {
                                    r = 0.45f32;
                                    g_ = 0.75f32;
                                    b = 1f32;
                                }
                                4 => {
                                    r = 1.15f32;
                                    g_ = 1.15f32;
                                    b = 0.5f32;
                                }
                                5 => {
                                    r = g.main.disco_r as f32 / 255f32;
                                    g_ = g.main.disco_g as f32 / 255f32;
                                    b = g.main.disco_b as f32 / 255f32;
                                }
                                _ => {
                                    r = 0.9f32;
                                    g_ = 0.3f32;
                                    b = 0.1f32;
                                }
                            }
                            r += num27;
                            g_ += num27;
                            b += num27;
                        }
                    }
                    215 => {
                        if (tile.frame_y as i32) < 36 {
                            let mut num26: f32 = local_random.next_range(28, 42) as f32 * 0.005f32;
                            num26 += (270 - (g.main.mouse_text_color as i32)) as f32 / 700f32;
                            match (tile.frame_x as i32) / 54 {
                                1 => {
                                    r = 0.7f32;
                                    g_ = 1f32;
                                    b = 0.5f32;
                                }
                                2 => {
                                    r = 0.5f32 * g.main.demon_torch + 1f32 * (1f32 - g.main.demon_torch);
                                    g_ = 0.3f32;
                                    b = 1f32 * g.main.demon_torch + 0.5f32 * (1f32 - g.main.demon_torch);
                                }
                                3 => {
                                    r = 0.45f32;
                                    g_ = 0.75f32;
                                    b = 1f32;
                                }
                                4 => {
                                    r = 1.15f32;
                                    g_ = 1.15f32;
                                    b = 0.5f32;
                                }
                                5 => {
                                    r = g.main.disco_r as f32 / 255f32;
                                    g_ = g.main.disco_g as f32 / 255f32;
                                    b = g.main.disco_b as f32 / 255f32;
                                }
                                6 => {
                                    r = 0.75f32;
                                    g_ = 1.2824999f32;
                                    b = 1.2f32;
                                }
                                7 => {
                                    r = 0.95f32;
                                    g_ = 0.65f32;
                                    b = 1.3f32;
                                }
                                8 => {
                                    r = 1.4f32;
                                    g_ = 0.85f32;
                                    b = 0.55f32;
                                }
                                9 => {
                                    r = 0.25f32;
                                    g_ = 1.3f32;
                                    b = 0.8f32;
                                }
                                10 => {
                                    r = 0.95f32;
                                    g_ = 0.4f32;
                                    b = 1.4f32;
                                }
                                11 => {
                                    r = 1.4f32;
                                    g_ = 0.7f32;
                                    b = 0.5f32;
                                }
                                12 => {
                                    r = 1.25f32;
                                    g_ = 0.6f32;
                                    b = 1.2f32;
                                }
                                13 => {
                                    r = 0.75f32;
                                    g_ = 1.45f32;
                                    b = 0.9f32;
                                }
                                14 => {
                                    r = 0.25f32;
                                    g_ = 0.65f32;
                                    b = 1f32;
                                }
                                15 => {
                                    torch_color(g, 23, &mut r, &mut g_, &mut b);
                                }
                                _ => {
                                    r = 0.9f32;
                                    g_ = 0.3f32;
                                    b = 0.1f32;
                                }
                            }
                            r += num26;
                            g_ += num26;
                            b += num26;
                        }
                    }
                    92 => {
                        if (tile.frame_y as i32) <= 18 && (tile.frame_x as i32) == 0 {
                            r = 1f32;
                            g_ = 1f32;
                            b = 1f32;
                        }
                    }
                    592 => {
                        if (tile.frame_y as i32) > 0 {
                            let mut num25: f32 = local_random.next_range(28, 42) as f32 * 0.005f32;
                            num25 += (270 - (g.main.mouse_text_color as i32)) as f32 / 700f32;
                            r = 1.35f32;
                            g_ = 0.45f32;
                            b = 0.15f32;
                            r += num25;
                            g_ += num25;
                            b += num25;
                        }
                    }
                    593 => {
                        if (tile.frame_x as i32) < 18 {
                            r = 0.8f32;
                            g_ = 0.3f32;
                            b = 0.1f32;
                        }
                    }
                    594 => {
                        if (tile.frame_x as i32) < 36 {
                            r = 0.8f32;
                            g_ = 0.3f32;
                            b = 0.1f32;
                        }
                    }
                    548 => {
                        if (tile.frame_x as i32) / 54 >= 7 {
                            r = 0.7f32;
                            g_ = 0.3f32;
                            b = 0.2f32;
                        }
                    }
                    613 | 614 => {
                        r = 0.7f32;
                        g_ = 0.3f32;
                        b = 0.2f32;
                    }
                    93 => {
                        if (tile.frame_x as i32) != 0 {
                            break 'sw1;
                        }
                        match (tile.frame_y as i32) / 54 {
                            1 => {
                                r = 0.95f32;
                                g_ = 0.95f32;
                                b = 0.5f32;
                            }
                            2 => {
                                r = 0.85f32;
                                g_ = 0.6f32;
                                b = 1f32;
                            }
                            3 => {
                                r = 0.75f32;
                                g_ = 1f32;
                                b = 0.6f32;
                            }
                            4 | 5 => {
                                r = 0.75f32;
                                g_ = 0.85f32;
                                b = 1f32;
                            }
                            6 => {
                                r = 1f32;
                                g_ = 0.95f32;
                                b = 0.65f32;
                            }
                            7 => {
                                r = 1f32;
                                g_ = 0.97f32;
                                b = 0.85f32;
                            }
                            9 => {
                                r = 1f32;
                                g_ = 1f32;
                                b = 0.7f32;
                            }
                            10 => {
                                r = 1f32;
                                g_ = 0.95f32;
                                b = 0.65f32;
                            }
                            12 => {
                                r = 1f32;
                                g_ = 0.95f32;
                                b = 0.65f32;
                            }
                            13 => {
                                r = 1f32;
                                g_ = 1f32;
                                b = 0.6f32;
                            }
                            14 => {
                                r = 1f32;
                                g_ = 0.95f32;
                                b = 0.65f32;
                            }
                            18 => {
                                r = 1f32;
                                g_ = 0.95f32;
                                b = 0.65f32;
                            }
                            19 => {
                                if (tile.color() as i32) == 0 {
                                    r = 0.37f32;
                                    g_ = 0.8f32;
                                    b = 1f32;
                                } else {
                                    flag = true;
                                }
                            }
                            20 => {
                                r = 0f32;
                                g_ = 0.9f32;
                                b = 1f32;
                            }
                            21 => {
                                r = 0.25f32;
                                g_ = 0.7f32;
                                b = 1f32;
                            }
                            23 => {
                                r = 0.5f32 * g.main.demon_torch + 1f32 * (1f32 - g.main.demon_torch);
                                g_ = 0.3f32;
                                b = 1f32 * g.main.demon_torch + 0.5f32 * (1f32 - g.main.demon_torch);
                            }
                            24 => {
                                r = 0.35f32;
                                g_ = 0.5f32;
                                b = 0.3f32;
                            }
                            25 => {
                                r = 0.34f32;
                                g_ = 0.4f32;
                                b = 0.31f32;
                            }
                            26 => {
                                r = 0.25f32;
                                g_ = 0.32f32;
                                b = 0.5f32;
                            }
                            29 => {
                                r = 0.9f32;
                                g_ = 0.75f32;
                                b = 1f32;
                            }
                            30 => {
                                r = 1f32;
                                g_ = 0.95f32;
                                b = 0.65f32;
                            }
                            31 => {
                                let vector10: Vector3 = hsl_to_rgb(g.main.demon_torch * 0.12f32 + 0.69f32, 1f32, 0.75f32, 255).to_vector3() * 1.2f32;
                                r = vector10.x;
                                g_ = vector10.y;
                                b = vector10.z;
                            }
                            32 => {
                                r = 1f32;
                                g_ = 0.97f32;
                                b = 0.85f32;
                            }
                            33 => {
                                r = 0.55f32;
                                g_ = 0.45f32;
                                b = 0.95f32;
                            }
                            34 => {
                                r = 1f32;
                                g_ = 0.6f32;
                                b = 0.1f32;
                            }
                            35 => {
                                r = 0.3f32;
                                g_ = 0.75f32;
                                b = 0.55f32;
                            }
                            36 => {
                                r = 0.9f32;
                                g_ = 0.55f32;
                                b = 0.7f32;
                            }
                            37 => {
                                r = 0.55f32;
                                g_ = 0.85f32;
                                b = 1f32;
                            }
                            38 => {
                                r = 1f32;
                                g_ = 0.95f32;
                                b = 0.65f32;
                            }
                            39 => {
                                r = 1f32;
                                g_ = 0.95f32;
                                b = 0.65f32;
                            }
                            40 => {
                                r = 0.4f32;
                                g_ = 0.8f32;
                                b = 0.9f32;
                            }
                            41 => {
                                r = 1f32;
                                g_ = 1f32;
                                b = 1f32;
                            }
                            42 => {
                                r = 0.95f32;
                                g_ = 0.5f32;
                                b = 0.4f32;
                            }
                            43 => {
                                let vector9: Vector4 = get_shimmer_base_color(g, x as f32, y as f32) * 1.5f32;
                                r = math_helper::clamp(vector9.x, 0f32, 1f32);
                                g_ = math_helper::clamp(vector9.y, 0f32, 1f32);
                                b = math_helper::clamp(vector9.z, 0f32, 1f32);
                            }
                            44 => {
                                r = 1f32;
                                g_ = 0.95f32;
                                b = 0.65f32;
                            }
                            45 => {
                                r = 1f32;
                                g_ = 2f32 / 3f32;
                                b = 66f32 / 85f32;
                            }
                            46 => {
                                r = 1f32;
                                g_ = 0.95f32;
                                b = 0.65f32;
                            }
                            47 => {
                                r = 81f32 / 85f32;
                                g_ = 77f32 / 85f32;
                                b = 0.36078432f32;
                            }
                            48 => {
                                r = 54f32 / 85f32;
                                g_ = 0.5019608f32;
                                b = 1f32;
                            }
                            49 => {
                                r = 1f32;
                                g_ = 20f32 / 51f32;
                                b = 20f32 / 51f32;
                            }
                            50 => {
                                r = 38f32 / 51f32;
                                g_ = 38f32 / 51f32;
                                b = 1f32;
                            }
                            51 => {
                                r = 2f32 / 3f32;
                                g_ = 0.7058824f32;
                                b = 1f32;
                            }
                            52 => {
                                r = 1f32;
                                g_ = 0.95f32;
                                b = 0.65f32;
                            }
                            53 => {
                                r = 1f32;
                                g_ = 0.95f32;
                                b = 0.75f32;
                            }
                            54 => {
                                r = 1f32;
                                g_ = 0.85499996f32;
                                b = 0.585f32;
                            }
                            55 => {
                                r = 0.5f32;
                                g_ = 0.9f32;
                                b = 1f32;
                                flag = true;
                            }
                            56 => {
                                r = 1f32;
                                g_ = 0.9f32;
                                b = 0.9f32;
                            }
                            57 => {
                                r = 0.7058824f32;
                                g_ = 46f32 / 51f32;
                                b = 1f32;
                            }
                            58 => {
                                r = 0.5882353f32;
                                g_ = 47f32 / 51f32;
                                b = 49f32 / 51f32;
                            }
                            59 => {
                                r = 2f32 / 3f32;
                                g_ = 49f32 / 51f32;
                                b = 1f32;
                            }
                            60 => {
                                r = 1f32;
                                g_ = 0.95f32;
                                b = 0.65f32;
                            }
                            61 => {
                                r = 1f32;
                                g_ = 0.95f32;
                                b = 0.65f32;
                            }
                            62 => {
                                r = 47f32 / 51f32;
                                g_ = 0.4117647f32;
                                b = 1f32;
                            }
                            63 => {
                                r = 38f32 / 51f32;
                                g_ = 38f32 / 51f32;
                                b = 1f32;
                            }
                            64 => {
                                r = 43f32 / 51f32;
                                g_ = 35f32 / 51f32;
                                b = 49f32 / 51f32;
                            }
                            _ => {
                                r = 1f32;
                                g_ = 0.97f32;
                                b = 0.85f32;
                            }
                        }
                    }
                    96 => {
                        if (tile.frame_x as i32) >= 36 {
                            r = 0.5f32;
                            g_ = 0.35f32;
                            b = 0.1f32;
                        }
                    }
                    98 => {
                        if (tile.frame_y as i32) == 0 {
                            r = 1f32;
                            g_ = 0.97f32;
                            b = 0.85f32;
                        }
                    }
                    4 => {
                        if (tile.frame_x as i32) < 66 {
                            torch_color(g, (tile.frame_y as i32) / 22, &mut r, &mut g_, &mut b);
                        }
                    }
                    372 => {
                        if (tile.frame_x as i32) == 0 {
                            r = 0.9f32;
                            g_ = 0.1f32;
                            b = 0.75f32;
                        }
                    }
                    646 => {
                        if (tile.frame_x as i32) == 0 {
                            r = 0.2f32;
                            g_ = 0.3f32;
                            b = 0.32f32;
                        }
                    }
                    33 => {
                        if (tile.frame_x as i32) != 0 {
                            break 'sw1;
                        }
                        match (tile.frame_y as i32) / 22 {
                            0 => {
                                r = 1f32;
                                g_ = 0.95f32;
                                b = 0.65f32;
                            }
                            1 => {
                                r = 0.55f32;
                                g_ = 0.85f32;
                                b = 0.35f32;
                            }
                            2 => {
                                r = 0.65f32;
                                g_ = 0.95f32;
                                b = 0.5f32;
                            }
                            3 => {
                                r = 0.2f32;
                                g_ = 0.75f32;
                                b = 1f32;
                            }
                            5 => {
                                r = 0.85f32;
                                g_ = 0.6f32;
                                b = 1f32;
                            }
                            7 | 8 => {
                                r = 0.75f32;
                                g_ = 0.85f32;
                                b = 1f32;
                            }
                            9 => {
                                r = 1f32;
                                g_ = 0.95f32;
                                b = 0.65f32;
                            }
                            10 => {
                                r = 1f32;
                                g_ = 0.97f32;
                                b = 0.85f32;
                            }
                            14 => {
                                r = 1f32;
                                g_ = 1f32;
                                b = 0.6f32;
                            }
                            15 => {
                                r = 1f32;
                                g_ = 0.95f32;
                                b = 0.65f32;
                            }
                            18 => {
                                r = 1f32;
                                g_ = 0.95f32;
                                b = 0.65f32;
                            }
                            19 => {
                                if (tile.color() as i32) == 0 {
                                    r = 0.37f32;
                                    g_ = 0.8f32;
                                    b = 1f32;
                                } else {
                                    flag = true;
                                }
                            }
                            20 => {
                                r = 0f32;
                                g_ = 0.9f32;
                                b = 1f32;
                            }
                            21 => {
                                r = 0.25f32;
                                g_ = 0.7f32;
                                b = 1f32;
                            }
                            23 => {
                                r = 1f32;
                                g_ = 0.95f32;
                                b = 0.65f32;
                            }
                            24 => {
                                r = 1f32;
                                g_ = 0.95f32;
                                b = 0.65f32;
                            }
                            25 => {
                                r = 0.5f32 * g.main.demon_torch + 1f32 * (1f32 - g.main.demon_torch);
                                g_ = 0.3f32;
                                b = 1f32 * g.main.demon_torch + 0.5f32 * (1f32 - g.main.demon_torch);
                            }
                            28 => {
                                r = 0.9f32;
                                g_ = 0.75f32;
                                b = 1f32;
                            }
                            29 => {
                                r = 1f32;
                                g_ = 0.95f32;
                                b = 0.65f32;
                            }
                            30 => {
                                let vector8: Vector3 = hsl_to_rgb(g.main.demon_torch * 0.12f32 + 0.69f32, 1f32, 0.75f32, 255).to_vector3() * 1.2f32;
                                r = vector8.x;
                                g_ = vector8.y;
                                b = vector8.z;
                            }
                            31 => {
                                r = 1f32;
                                g_ = 0.97f32;
                                b = 0.85f32;
                            }
                            32 => {
                                r = 0.55f32;
                                g_ = 0.45f32;
                                b = 0.95f32;
                            }
                            33 => {
                                r = 1f32;
                                g_ = 0.6f32;
                                b = 0.1f32;
                            }
                            34 => {
                                r = 0.3f32;
                                g_ = 0.75f32;
                                b = 0.55f32;
                            }
                            35 => {
                                r = 0.9f32;
                                g_ = 0.55f32;
                                b = 0.7f32;
                            }
                            36 => {
                                r = 0.55f32;
                                g_ = 0.85f32;
                                b = 1f32;
                            }
                            37 => {
                                r = 1f32;
                                g_ = 0.95f32;
                                b = 0.65f32;
                            }
                            38 => {
                                r = 1f32;
                                g_ = 0.95f32;
                                b = 0.65f32;
                            }
                            39 => {
                                r = 0.4f32;
                                g_ = 0.8f32;
                                b = 0.9f32;
                            }
                            40 => {
                                r = 1f32;
                                g_ = 1f32;
                                b = 1f32;
                            }
                            41 => {
                                r = 0.95f32;
                                g_ = 0.5f32;
                                b = 0.4f32;
                            }
                            42 => {
                                let vector7: Vector4 = get_shimmer_base_color(g, x as f32, y as f32) * 1.5f32;
                                r = math_helper::clamp(vector7.x, 0f32, 1f32);
                                g_ = math_helper::clamp(vector7.y, 0f32, 1f32);
                                b = math_helper::clamp(vector7.z, 0f32, 1f32);
                            }
                            43 => {
                                r = 1f32;
                                g_ = 0.95f32;
                                b = 0.65f32;
                            }
                            44 => {
                                r = 1f32;
                                g_ = 2f32 / 3f32;
                                b = 66f32 / 85f32;
                            }
                            45 => {
                                r = 1f32;
                                g_ = 0.95f32;
                                b = 0.65f32;
                            }
                            46 => {
                                r = 81f32 / 85f32;
                                g_ = 77f32 / 85f32;
                                b = 0.36078432f32;
                            }
                            47 => {
                                r = 54f32 / 85f32;
                                g_ = 0.5019608f32;
                                b = 1f32;
                            }
                            48 => {
                                r = 1f32;
                                g_ = 20f32 / 51f32;
                                b = 20f32 / 51f32;
                            }
                            49 => {
                                r = 38f32 / 51f32;
                                g_ = 38f32 / 51f32;
                                b = 1f32;
                            }
                            50 => {
                                r = 2f32 / 3f32;
                                g_ = 0.7058824f32;
                                b = 1f32;
                            }
                            51 => {
                                r = 1f32;
                                g_ = 0.95f32;
                                b = 0.65f32;
                            }
                            52 => {
                                r = 1f32;
                                g_ = 0.95f32;
                                b = 0.75f32;
                            }
                            53 => {
                                r = 1f32;
                                g_ = 0.85499996f32;
                                b = 0.585f32;
                            }
                            54 => {
                                r = 0.5f32;
                                g_ = 0.9f32;
                                b = 1f32;
                                flag = true;
                            }
                            55 => {
                                r = 1f32;
                                g_ = 0.9f32;
                                b = 0.9f32;
                            }
                            56 => {
                                r = 0.7058824f32;
                                g_ = 46f32 / 51f32;
                                b = 1f32;
                            }
                            57 => {
                                r = 0.5882353f32;
                                g_ = 47f32 / 51f32;
                                b = 49f32 / 51f32;
                            }
                            58 => {
                                r = 2f32 / 3f32;
                                g_ = 49f32 / 51f32;
                                b = 1f32;
                            }
                            59 => {
                                r = 1f32;
                                g_ = 0.95f32;
                                b = 0.65f32;
                            }
                            60 => {
                                r = 1f32;
                                g_ = 0.95f32;
                                b = 0.65f32;
                            }
                            61 => {
                                r = 47f32 / 51f32;
                                g_ = 0.4117647f32;
                                b = 1f32;
                            }
                            62 => {
                                r = 38f32 / 51f32;
                                g_ = 38f32 / 51f32;
                                b = 1f32;
                            }
                            63 => {
                                r = 43f32 / 51f32;
                                g_ = 35f32 / 51f32;
                                b = 49f32 / 51f32;
                            }
                            _ => {
                                r = 1f32;
                                g_ = 0.95f32;
                                b = 0.65f32;
                            }
                        }
                    }
                    174 => {
                        if (tile.frame_x as i32) == 0 {
                            r = 1f32;
                            g_ = 0.95f32;
                            b = 0.65f32;
                        }
                    }
                    100 | 173 => {
                        if (tile.frame_x as i32) >= 36 {
                            break 'sw1;
                        }
                        match (tile.frame_y as i32) / 36 {
                            1 => {
                                r = 0.95f32;
                                g_ = 0.95f32;
                                b = 0.5f32;
                            }
                            2 => {
                                r = 0.85f32;
                                g_ = 0.6f32;
                                b = 1f32;
                            }
                            3 => {
                                r = 1f32;
                                g_ = 0.6f32;
                                b = 0.6f32;
                            }
                            5 => {
                                r = 1f32;
                                g_ = 0.95f32;
                                b = 0.65f32;
                            }
                            6 | 7 => {
                                r = 1f32;
                                g_ = 0.95f32;
                                b = 0.65f32;
                            }
                            8 => {
                                r = 1f32;
                                g_ = 0.97f32;
                                b = 0.85f32;
                            }
                            9 => {
                                r = 0.75f32;
                                g_ = 0.85f32;
                                b = 1f32;
                            }
                            11 => {
                                r = 1f32;
                                g_ = 1f32;
                                b = 0.7f32;
                            }
                            12 => {
                                r = 1f32;
                                g_ = 0.95f32;
                                b = 0.65f32;
                            }
                            13 => {
                                r = 1f32;
                                g_ = 1f32;
                                b = 0.6f32;
                            }
                            14 => {
                                r = 1f32;
                                g_ = 0.95f32;
                                b = 0.65f32;
                            }
                            18 => {
                                r = 1f32;
                                g_ = 0.95f32;
                                b = 0.65f32;
                            }
                            19 => {
                                if (tile.color() as i32) == 0 {
                                    r = 0.37f32;
                                    g_ = 0.8f32;
                                    b = 1f32;
                                } else {
                                    flag = true;
                                }
                            }
                            20 => {
                                r = 0f32;
                                g_ = 0.9f32;
                                b = 1f32;
                            }
                            21 => {
                                r = 0.25f32;
                                g_ = 0.7f32;
                                b = 1f32;
                            }
                            25 => {
                                r = 0.5f32 * g.main.demon_torch + 1f32 * (1f32 - g.main.demon_torch);
                                g_ = 0.3f32;
                                b = 1f32 * g.main.demon_torch + 0.5f32 * (1f32 - g.main.demon_torch);
                            }
                            22 => {
                                r = 0.35f32;
                                g_ = 0.5f32;
                                b = 0.3f32;
                            }
                            23 => {
                                r = 0.34f32;
                                g_ = 0.4f32;
                                b = 0.31f32;
                            }
                            24 => {
                                r = 0.25f32;
                                g_ = 0.32f32;
                                b = 0.5f32;
                            }
                            29 => {
                                r = 0.9f32;
                                g_ = 0.75f32;
                                b = 1f32;
                            }
                            30 => {
                                r = 1f32;
                                g_ = 0.95f32;
                                b = 0.65f32;
                            }
                            31 => {
                                let vector6: Vector3 = hsl_to_rgb(g.main.demon_torch * 0.12f32 + 0.69f32, 1f32, 0.75f32, 255).to_vector3() * 1.2f32;
                                r = vector6.x;
                                g_ = vector6.y;
                                b = vector6.z;
                            }
                            32 => {
                                r = 1f32;
                                g_ = 0.97f32;
                                b = 0.85f32;
                            }
                            33 => {
                                r = 0.55f32;
                                g_ = 0.45f32;
                                b = 0.95f32;
                            }
                            34 => {
                                r = 1f32;
                                g_ = 0.6f32;
                                b = 0.1f32;
                            }
                            35 => {
                                r = 0.3f32;
                                g_ = 0.75f32;
                                b = 0.55f32;
                            }
                            36 => {
                                r = 0.9f32;
                                g_ = 0.55f32;
                                b = 0.7f32;
                            }
                            37 => {
                                r = 0.55f32;
                                g_ = 0.85f32;
                                b = 1f32;
                            }
                            38 => {
                                r = 1f32;
                                g_ = 0.95f32;
                                b = 0.65f32;
                            }
                            39 => {
                                r = 1f32;
                                g_ = 0.95f32;
                                b = 0.65f32;
                            }
                            40 => {
                                r = 0.4f32;
                                g_ = 0.8f32;
                                b = 0.9f32;
                            }
                            41 => {
                                r = 1f32;
                                g_ = 1f32;
                                b = 1f32;
                            }
                            42 => {
                                r = 0.95f32;
                                g_ = 0.5f32;
                                b = 0.4f32;
                            }
                            43 => {
                                let vector5: Vector4 = get_shimmer_base_color(g, x as f32, y as f32) * 1.5f32;
                                r = math_helper::clamp(vector5.x, 0f32, 1f32);
                                g_ = math_helper::clamp(vector5.y, 0f32, 1f32);
                                b = math_helper::clamp(vector5.z, 0f32, 1f32);
                            }
                            44 => {
                                r = 1f32;
                                g_ = 0.95f32;
                                b = 0.65f32;
                            }
                            45 => {
                                r = 1f32;
                                g_ = 2f32 / 3f32;
                                b = 66f32 / 85f32;
                            }
                            46 => {
                                r = 1f32;
                                g_ = 0.95f32;
                                b = 0.65f32;
                            }
                            47 => {
                                r = 81f32 / 85f32;
                                g_ = 77f32 / 85f32;
                                b = 0.36078432f32;
                            }
                            48 => {
                                r = 54f32 / 85f32;
                                g_ = 0.5019608f32;
                                b = 1f32;
                            }
                            49 => {
                                r = 1f32;
                                g_ = 20f32 / 51f32;
                                b = 20f32 / 51f32;
                            }
                            50 => {
                                r = 38f32 / 51f32;
                                g_ = 38f32 / 51f32;
                                b = 1f32;
                            }
                            51 => {
                                r = 2f32 / 3f32;
                                g_ = 0.7058824f32;
                                b = 1f32;
                            }
                            52 => {
                                r = 1f32;
                                g_ = 0.95f32;
                                b = 0.65f32;
                            }
                            53 => {
                                r = 1f32;
                                g_ = 0.95f32;
                                b = 0.75f32;
                            }
                            54 => {
                                r = 1f32;
                                g_ = 0.85499996f32;
                                b = 0.585f32;
                            }
                            55 => {
                                r = 0.5f32;
                                g_ = 0.9f32;
                                b = 1f32;
                                flag = true;
                            }
                            56 => {
                                r = 1f32;
                                g_ = 0.9f32;
                                b = 0.9f32;
                            }
                            57 => {
                                r = 0.7058824f32;
                                g_ = 46f32 / 51f32;
                                b = 1f32;
                            }
                            58 => {
                                r = 0.5882353f32;
                                g_ = 47f32 / 51f32;
                                b = 49f32 / 51f32;
                            }
                            59 => {
                                r = 2f32 / 3f32;
                                g_ = 49f32 / 51f32;
                                b = 1f32;
                            }
                            60 => {
                                r = 1f32;
                                g_ = 0.95f32;
                                b = 0.65f32;
                            }
                            61 => {
                                r = 1f32;
                                g_ = 0.95f32;
                                b = 0.65f32;
                            }
                            62 => {
                                r = 47f32 / 51f32;
                                g_ = 0.4117647f32;
                                b = 1f32;
                            }
                            63 => {
                                r = 38f32 / 51f32;
                                g_ = 38f32 / 51f32;
                                b = 1f32;
                            }
                            64 => {
                                r = 43f32 / 51f32;
                                g_ = 35f32 / 51f32;
                                b = 49f32 / 51f32;
                            }
                            _ => {
                                r = 1f32;
                                g_ = 0.95f32;
                                b = 0.65f32;
                            }
                        }
                    }
                    34 => {
                        if (tile.frame_x as i32) % 108 >= 54 {
                            break 'sw1;
                        }
                        let num24: i32 = (tile.frame_y as i32) / 54;
                        match num24 + 37 * ((tile.frame_x as i32) / 108) {
                            7 => {
                                r = 0.95f32;
                                g_ = 0.95f32;
                                b = 0.5f32;
                            }
                            8 => {
                                r = 0.85f32;
                                g_ = 0.6f32;
                                b = 1f32;
                            }
                            9 => {
                                r = 1f32;
                                g_ = 0.6f32;
                                b = 0.6f32;
                            }
                            12 => {
                                r = 1f32;
                                g_ = 0.95f32;
                                b = 0.65f32;
                            }
                            13 => {
                                r = 1f32;
                                g_ = 0.97f32;
                                b = 0.85f32;
                            }
                            11 | 17 => {
                                r = 0.75f32;
                                g_ = 0.85f32;
                                b = 1f32;
                            }
                            15 => {
                                r = 1f32;
                                g_ = 1f32;
                                b = 0.7f32;
                            }
                            16 => {
                                r = 1f32;
                                g_ = 0.95f32;
                                b = 0.65f32;
                            }
                            18 => {
                                r = 1f32;
                                g_ = 1f32;
                                b = 0.6f32;
                            }
                            19 => {
                                r = 1f32;
                                g_ = 0.95f32;
                                b = 0.65f32;
                            }
                            23 => {
                                r = 1f32;
                                g_ = 0.95f32;
                                b = 0.65f32;
                            }
                            24 => {
                                if (tile.color() as i32) == 0 {
                                    r = 0.37f32;
                                    g_ = 0.8f32;
                                    b = 1f32;
                                } else {
                                    flag = true;
                                }
                            }
                            25 => {
                                r = 0f32;
                                g_ = 0.9f32;
                                b = 1f32;
                            }
                            26 => {
                                r = 0.25f32;
                                g_ = 0.7f32;
                                b = 1f32;
                            }
                            27 => {
                                r = 0.55f32;
                                g_ = 0.85f32;
                                b = 0.35f32;
                            }
                            28 => {
                                r = 0.65f32;
                                g_ = 0.95f32;
                                b = 0.5f32;
                            }
                            29 => {
                                r = 0.2f32;
                                g_ = 0.75f32;
                                b = 1f32;
                            }
                            30 => {
                                r = 1f32;
                                g_ = 0.95f32;
                                b = 0.65f32;
                            }
                            32 => {
                                r = 0.5f32 * g.main.demon_torch + 1f32 * (1f32 - g.main.demon_torch);
                                g_ = 0.3f32;
                                b = 1f32 * g.main.demon_torch + 0.5f32 * (1f32 - g.main.demon_torch);
                            }
                            35 => {
                                r = 0.9f32;
                                g_ = 0.75f32;
                                b = 1f32;
                            }
                            36 => {
                                r = 1f32;
                                g_ = 0.95f32;
                                b = 0.65f32;
                            }
                            37 => {
                                let vector4: Vector3 = hsl_to_rgb(g.main.demon_torch * 0.12f32 + 0.69f32, 1f32, 0.75f32, 255).to_vector3() * 1.2f32;
                                r = vector4.x;
                                g_ = vector4.y;
                                b = vector4.z;
                            }
                            38 => {
                                r = 1f32;
                                g_ = 0.97f32;
                                b = 0.85f32;
                            }
                            39 => {
                                r = 0.55f32;
                                g_ = 0.45f32;
                                b = 0.95f32;
                            }
                            40 => {
                                r = 1f32;
                                g_ = 0.6f32;
                                b = 0.1f32;
                            }
                            41 => {
                                r = 0.3f32;
                                g_ = 0.75f32;
                                b = 0.55f32;
                            }
                            42 => {
                                r = 0.9f32;
                                g_ = 0.55f32;
                                b = 0.7f32;
                            }
                            43 => {
                                r = 0.55f32;
                                g_ = 0.85f32;
                                b = 1f32;
                            }
                            44 => {
                                r = 1f32;
                                g_ = 0.95f32;
                                b = 0.65f32;
                            }
                            45 => {
                                r = 1f32;
                                g_ = 0.95f32;
                                b = 0.65f32;
                            }
                            46 => {
                                r = 0.4f32;
                                g_ = 0.8f32;
                                b = 0.9f32;
                            }
                            47 => {
                                r = 1f32;
                                g_ = 1f32;
                                b = 1f32;
                            }
                            48 => {
                                r = 0.95f32;
                                g_ = 0.5f32;
                                b = 0.4f32;
                            }
                            49 => {
                                let vector3: Vector4 = get_shimmer_base_color(g, x as f32, y as f32) * 1.5f32;
                                r = math_helper::clamp(vector3.x, 0f32, 1f32);
                                g_ = math_helper::clamp(vector3.y, 0f32, 1f32);
                                b = math_helper::clamp(vector3.z, 0f32, 1f32);
                            }
                            50 => {
                                r = 1f32;
                                g_ = 0.95f32;
                                b = 0.65f32;
                            }
                            51 => {
                                r = 1f32;
                                g_ = 2f32 / 3f32;
                                b = 66f32 / 85f32;
                            }
                            52 => {
                                r = 1f32;
                                g_ = 0.95f32;
                                b = 0.65f32;
                            }
                            53 => {
                                r = 81f32 / 85f32;
                                g_ = 77f32 / 85f32;
                                b = 0.36078432f32;
                            }
                            54 => {
                                r = 54f32 / 85f32;
                                g_ = 0.5019608f32;
                                b = 1f32;
                            }
                            55 => {
                                r = 1f32;
                                g_ = 20f32 / 51f32;
                                b = 20f32 / 51f32;
                            }
                            56 => {
                                r = 38f32 / 51f32;
                                g_ = 38f32 / 51f32;
                                b = 1f32;
                            }
                            57 => {
                                r = 2f32 / 3f32;
                                g_ = 0.7058824f32;
                                b = 1f32;
                            }
                            58 => {
                                r = 1f32;
                                g_ = 0.95f32;
                                b = 0.65f32;
                            }
                            59 => {
                                r = 1f32;
                                g_ = 0.95f32;
                                b = 0.75f32;
                            }
                            60 => {
                                r = 1f32;
                                g_ = 0.85499996f32;
                                b = 0.585f32;
                            }
                            61 => {
                                r = 0.5f32;
                                g_ = 0.9f32;
                                b = 1f32;
                                flag = true;
                            }
                            62 => {
                                r = 1f32;
                                g_ = 0.9f32;
                                b = 0.9f32;
                            }
                            63 => {
                                r = 0.7058824f32;
                                g_ = 46f32 / 51f32;
                                b = 1f32;
                            }
                            64 => {
                                r = 0.5882353f32;
                                g_ = 47f32 / 51f32;
                                b = 49f32 / 51f32;
                            }
                            65 => {
                                r = 2f32 / 3f32;
                                g_ = 49f32 / 51f32;
                                b = 1f32;
                            }
                            66 => {
                                r = 1f32;
                                g_ = 0.95f32;
                                b = 0.65f32;
                            }
                            67 => {
                                r = 1f32;
                                g_ = 0.95f32;
                                b = 0.65f32;
                            }
                            68 => {
                                r = 47f32 / 51f32;
                                g_ = 0.4117647f32;
                                b = 1f32;
                            }
                            69 => {
                                r = 38f32 / 51f32;
                                g_ = 38f32 / 51f32;
                                b = 1f32;
                            }
                            70 => {
                                r = 43f32 / 51f32;
                                g_ = 35f32 / 51f32;
                                b = 49f32 / 51f32;
                            }
                            _ => {
                                r = 1f32;
                                g_ = 0.95f32;
                                b = 0.8f32;
                            }
                        }
                    }
                    35 => {
                        if (tile.frame_x as i32) < 36 {
                            r = 0.75f32;
                            g_ = 0.6f32;
                            b = 0.3f32;
                        }
                    }
                    95 => {
                        if (tile.frame_x as i32) < 36 {
                            r = 1f32;
                            g_ = 0.95f32;
                            b = 0.8f32;
                        }
                    }
                    17 | 133 | 302 => {
                        r = 0.83f32;
                        g_ = 0.6f32;
                        b = 0.5f32;
                    }
                    77 => {
                        r = 0.75f32;
                        g_ = 0.45f32;
                        b = 0.25f32;
                    }
                    37 => {
                        let num23: f32 = local_random.next_range(95, 106) as f32 * 0.01f32;
                        r = 0.56f32 * num23;
                        g_ = 0.43f32 * num23;
                        b = 0.15f32 * num23;
                    }
                    22 | 140 => {
                        if (tile.color() as i32) != 27 && (tile.color() as i32) != 26 {
                            r = 0.12f32;
                        }
                        g_ = 0.07f32;
                        b = 0.32f32;
                    }
                    171 => {
                        if (tile.frame_x as i32) < 10 {
                            x -= tile.frame_x as i32;
                            y -= tile.frame_y as i32;
                        }
                        match ((g.main.tile.get(x, y).frame_y as i32) & 0x3C00) >> 10 {
                            1 => {
                                r = 0.1f32;
                                g_ = 0.1f32;
                                b = 0.1f32;
                            }
                            2 => {
                                r = 0.2f32;
                            }
                            3 => {
                                g_ = 0.2f32;
                            }
                            4 => {
                                b = 0.2f32;
                            }
                            5 => {
                                r = 0.125f32;
                                g_ = 0.125f32;
                            }
                            6 => {
                                r = 0.2f32;
                                g_ = 0.1f32;
                            }
                            7 => {
                                r = 0.125f32;
                                g_ = 0.125f32;
                            }
                            8 => {
                                r = 0.08f32;
                                g_ = 0.175f32;
                            }
                            9 => {
                                g_ = 0.125f32;
                                b = 0.125f32;
                            }
                            10 => {
                                r = 0.125f32;
                                b = 0.125f32;
                            }
                            11 => {
                                r = 0.1f32;
                                g_ = 0.1f32;
                                b = 0.2f32;
                            }
                            _ => {
                                b = 0f32;
                                g_ = b;
                                r = g_;
                            }
                        }
                        r *= 0.5f32;
                        g_ *= 0.5f32;
                        b *= 0.5f32;
                    }
                    204 | 347 => {
                        if (tile.color() as i32) != 27 && (tile.color() as i32) != 26 {
                            r = 0.35f32;
                        }
                    }
                    42 => {
                        if (tile.frame_x as i32) != 0 {
                            break 'sw1;
                        }
                        match (tile.frame_y as i32) / 36 {
                            0 => {
                                r = 0.7f32;
                                g_ = 0.65f32;
                                b = 0.55f32;
                            }
                            1 => {
                                r = 0.9f32;
                                g_ = 0.75f32;
                                b = 0.6f32;
                            }
                            2 => {
                                r = 0.8f32;
                                g_ = 0.6f32;
                                b = 0.6f32;
                            }
                            3 => {
                                r = 0.65f32;
                                g_ = 0.5f32;
                                b = 0.2f32;
                            }
                            4 => {
                                r = 0.5f32;
                                g_ = 0.7f32;
                                b = 0.4f32;
                            }
                            5 => {
                                r = 0.9f32;
                                g_ = 0.4f32;
                                b = 0.2f32;
                            }
                            6 => {
                                r = 0.7f32;
                                g_ = 0.75f32;
                                b = 0.3f32;
                            }
                            7 => {
                                let num14: f32 = g.main.demon_torch * 0.2f32;
                                r = 0.9f32 - num14;
                                g_ = 0.9f32 - num14;
                                b = 0.7f32 + num14;
                            }
                            8 => {
                                r = 0.75f32;
                                g_ = 0.6f32;
                                b = 0.3f32;
                            }
                            9 => {
                                r = 1f32;
                                g_ = 0.3f32;
                                b = 0.5f32;
                                b += g.main.demon_torch * 0.2f32;
                                r -= g.main.demon_torch * 0.1f32;
                                g_ -= g.main.demon_torch * 0.2f32;
                            }
                            11 => {
                                r = 0.85f32;
                                g_ = 0.6f32;
                                b = 1f32;
                            }
                            14 => {
                                r = 1f32;
                                g_ = 0.95f32;
                                b = 0.65f32;
                            }
                            15 | 16 => {
                                r = 1f32;
                                g_ = 0.95f32;
                                b = 0.65f32;
                            }
                            17 => {
                                r = 1f32;
                                g_ = 0.97f32;
                                b = 0.85f32;
                            }
                            18 => {
                                r = 0.75f32;
                                g_ = 0.85f32;
                                b = 1f32;
                            }
                            21 => {
                                r = 1f32;
                                g_ = 0.95f32;
                                b = 0.65f32;
                            }
                            22 => {
                                r = 1f32;
                                g_ = 1f32;
                                b = 0.6f32;
                            }
                            23 => {
                                r = 1f32;
                                g_ = 0.95f32;
                                b = 0.65f32;
                            }
                            27 => {
                                r = 1f32;
                                g_ = 0.95f32;
                                b = 0.65f32;
                            }
                            28 => {
                                if (tile.color() as i32) == 0 {
                                    r = 0.37f32;
                                    g_ = 0.8f32;
                                    b = 1f32;
                                } else {
                                    flag = true;
                                }
                            }
                            29 => {
                                r = 0f32;
                                g_ = 0.9f32;
                                b = 1f32;
                            }
                            30 => {
                                r = 0.25f32;
                                g_ = 0.7f32;
                                b = 1f32;
                            }
                            32 => {
                                r = 0.5f32 * g.main.demon_torch + 1f32 * (1f32 - g.main.demon_torch);
                                g_ = 0.3f32;
                                b = 1f32 * g.main.demon_torch + 0.5f32 * (1f32 - g.main.demon_torch);
                            }
                            35 => {
                                r = 0.7f32;
                                g_ = 0.6f32;
                                b = 0.9f32;
                            }
                            36 => {
                                r = 1f32;
                                g_ = 0.95f32;
                                b = 0.65f32;
                            }
                            37 => {
                                let vector2: Vector3 = hsl_to_rgb(g.main.demon_torch * 0.12f32 + 0.69f32, 1f32, 0.75f32, 255).to_vector3() * 1.2f32;
                                r = vector2.x;
                                g_ = vector2.y;
                                b = vector2.z;
                            }
                            38 => {
                                r = 1f32;
                                g_ = 0.97f32;
                                b = 0.85f32;
                            }
                            39 => {
                                r = 0.55f32;
                                g_ = 0.45f32;
                                b = 0.95f32;
                            }
                            40 => {
                                r = 1f32;
                                g_ = 0.6f32;
                                b = 0.1f32;
                            }
                            41 => {
                                r = 0.3f32;
                                g_ = 0.75f32;
                                b = 0.55f32;
                            }
                            42 => {
                                r = 0.9f32;
                                g_ = 0.55f32;
                                b = 0.7f32;
                            }
                            43 => {
                                r = 0.55f32;
                                g_ = 0.85f32;
                                b = 1f32;
                            }
                            44 => {
                                r = 1f32;
                                g_ = 0.95f32;
                                b = 0.65f32;
                            }
                            45 => {
                                r = 1f32;
                                g_ = 0.95f32;
                                b = 0.65f32;
                            }
                            46 => {
                                r = 0.4f32;
                                g_ = 0.8f32;
                                b = 0.9f32;
                            }
                            47 => {
                                r = 1f32;
                                g_ = 1f32;
                                b = 1f32;
                            }
                            48 => {
                                r = 0.95f32;
                                g_ = 0.5f32;
                                b = 0.4f32;
                            }
                            49 => {
                                let vector: Vector4 = get_shimmer_base_color(g, x as f32, y as f32) * 1.5f32;
                                r = math_helper::clamp(vector.x, 0f32, 1f32);
                                g_ = math_helper::clamp(vector.y, 0f32, 1f32);
                                b = math_helper::clamp(vector.z, 0f32, 1f32);
                            }
                            50 => {
                                r = 1f32;
                                g_ = 0.95f32;
                                b = 0.65f32;
                            }
                            51 => {
                                r = 1f32;
                                g_ = 2f32 / 3f32;
                                b = 66f32 / 85f32;
                            }
                            52 => {
                                r = 1f32;
                                g_ = 0.95f32;
                                b = 0.65f32;
                            }
                            53 => {
                                r = 81f32 / 85f32;
                                g_ = 77f32 / 85f32;
                                b = 0.36078432f32;
                            }
                            54 => {
                                r = 54f32 / 85f32;
                                g_ = 0.5019608f32;
                                b = 1f32;
                            }
                            55 => {
                                r = 1f32;
                                g_ = 20f32 / 51f32;
                                b = 20f32 / 51f32;
                            }
                            56 => {
                                r = 38f32 / 51f32;
                                g_ = 38f32 / 51f32;
                                b = 1f32;
                            }
                            57 => {
                                r = 2f32 / 3f32;
                                g_ = 0.7058824f32;
                                b = 1f32;
                            }
                            58 => {
                                r = 1f32;
                                g_ = 0.95f32;
                                b = 0.65f32;
                            }
                            59 => {
                                r = 1f32;
                                g_ = 0.95f32;
                                b = 0.75f32;
                            }
                            60 => {
                                r = 1f32;
                                g_ = 0.85499996f32;
                                b = 0.585f32;
                            }
                            61 => {
                                r = 0.5f32;
                                g_ = 0.9f32;
                                b = 1f32;
                                flag = true;
                            }
                            62 => {
                                r = 1f32;
                                g_ = 0.9f32;
                                b = 0.9f32;
                            }
                            63 => {
                                r = 0.7058824f32;
                                g_ = 46f32 / 51f32;
                                b = 1f32;
                            }
                            64 => {
                                r = 0.5882353f32;
                                g_ = 47f32 / 51f32;
                                b = 49f32 / 51f32;
                            }
                            65 => {
                                r = 2f32 / 3f32;
                                g_ = 49f32 / 51f32;
                                b = 1f32;
                            }
                            66 => {
                                r = 1f32;
                                g_ = 0.95f32;
                                b = 0.65f32;
                            }
                            67 => {
                                r = 1f32;
                                g_ = 0.95f32;
                                b = 0.65f32;
                            }
                            68 => {
                                r = 47f32 / 51f32;
                                g_ = 0.4117647f32;
                                b = 1f32;
                            }
                            69 => {
                                r = 38f32 / 51f32;
                                g_ = 38f32 / 51f32;
                                b = 1f32;
                            }
                            70 => {
                                r = 43f32 / 51f32;
                                g_ = 35f32 / 51f32;
                                b = 49f32 / 51f32;
                            }
                            _ => {
                                r = 1f32;
                                g_ = 1f32;
                                b = 1f32;
                            }
                        }
                    }
                    49 => {
                        if (tile.frame_x as i32) == 0 {
                            r = 0f32;
                            g_ = 0.35f32;
                            b = 0.8f32;
                        }
                    }
                    519 => {
                        if (tile.frame_y as i32) == 90 {
                            if (tile.color() as i32) == 0 {
                                let mut num12: f32 = local_random.next_range(28, 42) as f32 * 0.005f32;
                                num12 += (270 - (g.main.mouse_text_color as i32)) as f32 / 1000f32;
                                r = 0.1f32;
                                g_ = 0.2f32 + num12 / 2f32;
                                b = 0.7f32 + num12;
                            } else {
                                flag = true;
                            }
                        }
                    }
                    70 | 71 | 72 | 190 | 348 | 349 | 528 | 578 => {
                        if (tile.type_ as i32) != 349 || (tile.frame_x as i32) >= 36 {
                            let mut num11: f32 = local_random.next_range(28, 42) as f32 * 0.005f32;
                            num11 += (270 - (g.main.mouse_text_color as i32)) as f32 / 1000f32;
                            if (tile.color() as i32) == 0 {
                                r = 0f32;
                                g_ = 0.2f32 + num11 / 2f32;
                                b = 1f32;
                            } else {
                                flag = true;
                            }
                        }
                    }
                    739 => {
                        r = 0.35f32;
                        g_ = 0.63f32;
                        b = 0.7f32;
                        flag = true;
                    }
                    350 => {
                        let num10: f64 = g.main.time_for_visual_effects * 0.08f64;
                        r = ((0.0f64 - ((if (num10 / 6.283f64) as i32 % 3 == 1 { num10 } else { 0.0f64 }) as f64).cos()) * 0.1f64 + 0.1f64) as f32;
                        g_ = r;
                        b = g_;
                    }
                    61 | 703 => {
                        if (tile.frame_x as i32) == 144 {
                            let num6: f32 = 1f32 + (270 - (g.main.mouse_text_color as i32)) as f32 / 400f32;
                            let num7: f32 = 0.8f32 - (270 - (g.main.mouse_text_color as i32)) as f32 / 400f32;
                            r = 0.42f32 * num7;
                            g_ = 0.81f32 * num6;
                            b = 0.52f32 * num7;
                        }
                    }
                    26 | 31 | 695 | 696 => {
                        if (((tile.type_ as i32) == 31 || (tile.type_ as i32) == 696) && (tile.frame_x as i32) >= 36) || (((tile.type_ as i32) == 26 || (tile.type_ as i32) == 695) && (tile.frame_x as i32) >= 54) {
                            let num4: f32 = local_random.next_range(-5, 6) as f32 * 0.0025f32;
                            r = 0.5f32 + num4 * 2f32;
                            g_ = 0.2f32 + num4;
                            b = 0.1f32;
                        } else {
                            let num5: f32 = local_random.next_range(-5, 6) as f32 * 0.0025f32;
                            r = 0.31f32 + num5;
                            g_ = 0.1f32;
                            b = 0.44f32 + num5 * 2f32;
                        }
                    }
                    699 => {
                        r = 0.4f32;
                        g_ = 0.2f32;
                        b = 0.15f32;
                    }
                    84 => {
                        let num2: i32 = (tile.frame_x as i32) / 18;
                        let mut num3: f32 = 0f32;
                        match num2 {
                            2 => {
                                num3 = (270 - (g.main.mouse_text_color as i32)) as f32 / 400f32;
                                if num3 > 1f32 {
                                    num3 = 1f32;
                                } else if num3 < 0f32 {
                                    num3 = 0f32;
                                }
                                r = num3 * 1.4f32;
                                g_ = num3 * 1.2f32;
                                b = num3 / 2f32;
                            }
                            5 => {
                                num3 = 0.9f32;
                                r = num3;
                                g_ = num3 * 0.8f32;
                                b = num3 * 0.2f32;
                            }
                            6 => {
                                num3 = 0.08f32;
                                g_ = num3 * 0.8f32;
                                b = num3;
                            }
                            _ => {}
                        }
                    }
                    83 => {
                        if (tile.frame_x as i32) == 18 && !g.main.day_time {
                            r = 0.1f32;
                            g_ = 0.4f32;
                            b = 0.6f32;
                        }
                        if (tile.frame_x as i32) == 90 && !g.main.raining && g.main.time > 40500.0f64 {
                            r = 0.9f32;
                            g_ = 0.72f32;
                            b = 0.18f32;
                        }
                    }
                    126 => {
                        if (tile.frame_x as i32) < 36 {
                            r = g.main.disco_r as f32 / 255f32;
                            g_ = g.main.disco_g as f32 / 255f32;
                            b = g.main.disco_b as f32 / 255f32;
                        }
                    }
                    125 => {
                        let mut num: f32 = local_random.next_range(28, 42) as f32 * 0.01f32;
                        num += (270 - (g.main.mouse_text_color as i32)) as f32 / 800f32;
                        light_color.y = 0.3f32 * num;
                        g_ = light_color.y;
                        light_color.z = 0.6f32 * num;
                        b = light_color.z;
                    }
                    129 => {
                        match (tile.frame_x as i32) / 18 % 3 {
                            0 => {
                                r = 0f32;
                                g_ = 0.05f32;
                                b = 0.25f32;
                            }
                            1 => {
                                r = 0.2f32;
                                g_ = 0f32;
                                b = 0.15f32;
                            }
                            2 => {
                                r = 0.1f32;
                                g_ = 0f32;
                                b = 0.2f32;
                            }
                            _ => {}
                        }
                    }
                    149 => {
                        if (tile.frame_x as i32) <= 36 {
                            match (tile.frame_x as i32) / 18 {
                                0 => {
                                    r = 0.1f32;
                                    g_ = 0.2f32;
                                    b = 0.5f32;
                                }
                                1 => {
                                    r = 0.5f32;
                                    g_ = 0.1f32;
                                    b = 0.1f32;
                                }
                                2 => {
                                    r = 0.2f32;
                                    g_ = 0.5f32;
                                    b = 0.1f32;
                                }
                                _ => {}
                            }
                            r *= local_random.next_range(970, 1031) as f32 * 0.001f32;
                            g_ *= local_random.next_range(970, 1031) as f32 * 0.001f32;
                            b *= local_random.next_range(970, 1031) as f32 * 0.001f32;
                        }
                    }
                    160 => {
                        r = g.main.disco_r as f32 / 255f32 * 0.25f32;
                        g_ = g.main.disco_g as f32 / 255f32 * 0.25f32;
                        b = g.main.disco_b as f32 / 255f32 * 0.25f32;
                    }
                    354 => {
                        r = 0.65f32;
                        g_ = 0.35f32;
                        b = 0.15f32;
                    }
                    _ => {}
                }
            }
        }
        if flag && (tile.color() as i32) != 0 {
            let color2: Color = paint_color(tile.color() as i32);
            r = color2.r() as i32 as f32 / 255f32;
            g_ = color2.g() as i32 as f32 / 255f32;
            b = color2.b() as i32 as f32 / 255f32;
        }
        if light_color.x < r {
            light_color.x = r;
        }
        if light_color.y < g_ {
            light_color.y = g_;
        }
        if light_color.z < b {
            light_color.z = b;
        }
    }

    /// `TileLightScanner.ApplySurfaceLight` (TileLightScanner.cs:3150)
    #[allow(unused_variables, unused_assignments, unused_mut)]
    fn apply_surface_light(&self, g: &mut Game, tile: &Tile, x: i32, y: i32, light_color: &mut Vector3) {
        let mut num: f32 = 0f32;
        let mut num2: f32 = 0f32;
        let mut num3: f32 = 0f32;
        let num4: f32 = g.main.tile_color.r() as i32 as f32 / 255f32;
        let num5: f32 = g.main.tile_color.g() as i32 as f32 / 255f32;
        let num6: f32 = g.main.tile_color.b() as i32 as f32 / 255f32;
        let num7: f32 = (num4 + num5 + num6) / 3f32;
        if tile.active() && tile_id::sets::ALLOW_LIGHT_IN_WATER[tile.type_ as usize] {
            if light_color.x < num7 && (g.main.wall_light[tile.wall as usize] || (tile.wall as i32) == 73 || (tile.wall as i32) == 227 || (tile.invisible_wall() && !self.draw_invisible_walls)) {
                num = num4;
                num2 = num5;
                num3 = num6;
            }
        } else if (!tile.active() || !g.main.tile_no_sun_light[tile.type_ as usize] || (((tile.slope() as i32) != 0 || tile.half_brick() || (tile.invisible_block() && !self.draw_invisible_walls)) && (g.main.tile.get(x, y - 1).liquid as i32) == 0 && (g.main.tile.get(x, y + 1).liquid as i32) == 0 && (g.main.tile.get(x - 1, y).liquid as i32) == 0 && (g.main.tile.get(x + 1, y).liquid as i32) == 0)) && light_color.x < num7 && (g.main.wall_light[tile.wall as usize] || (tile.wall as i32) == 73 || (tile.wall as i32) == 227 || (tile.invisible_wall() && !self.draw_invisible_walls)) {
            if (tile.liquid as i32) < 200 {
                if !tile.half_brick() || (g.main.tile.get(x, y - 1).liquid as i32) < 200 {
                    num = num4;
                    num2 = num5;
                    num3 = num6;
                }
            } else if g.main.liquid_alpha[13] > 0f32 {
                // if (Main.rand == null) Main.rand = new UnifiedRandom();  (Main.rand is never null in the port)
                num3 = num6 * 0.175f32 * (1f32 + g.main.rand.next_float() * 0.13f32) * g.main.liquid_alpha[13];
            }
        }
        if (!tile.active() || tile.half_brick() || !g.main.tile_no_sun_light[tile.type_ as usize]) && (((tile.wall as i32) >= 88 && (tile.wall as i32) <= 93) || (tile.wall as i32) == 241) && (tile.liquid as i32) < 255 {
            num = num4;
            num2 = num5;
            num3 = num6;
            let mut num8: i32 = (tile.wall as i32) - 88;
            if (tile.wall as i32) == 241 {
                num8 = 6;
            }
            match num8 {
                0 => {
                    num *= 0.9f32;
                    num2 *= 0.15f32;
                    num3 *= 0.9f32;
                }
                1 => {
                    num *= 0.9f32;
                    num2 *= 0.9f32;
                    num3 *= 0.15f32;
                }
                2 => {
                    num *= 0.15f32;
                    num2 *= 0.15f32;
                    num3 *= 0.9f32;
                }
                3 => {
                    num *= 0.15f32;
                    num2 *= 0.9f32;
                    num3 *= 0.15f32;
                }
                4 => {
                    num *= 0.9f32;
                    num2 *= 0.15f32;
                    num3 *= 0.15f32;
                }
                5 => {
                    let num9: f32 = 0.2f32;
                    let num10: f32 = 0.7f32 - num9;
                    num *= num10 + g.main.disco_r as f32 / 255f32 * num9;
                    num2 *= num10 + g.main.disco_g as f32 / 255f32 * num9;
                    num3 *= num10 + g.main.disco_b as f32 / 255f32 * num9;
                }
                6 => {
                    num *= 0.9f32;
                    num2 *= 0.5f32;
                    num3 *= 0f32;
                }
                _ => {}
            }
        }
        let num11: f32 = 1f32 - g.main.shimmer_darken;
        num *= num11;
        num2 *= num11;
        num3 *= num11;
        if light_color.x < num {
            light_color.x = num;
        }
        if light_color.y < num2 {
            light_color.y = num2;
        }
        if light_color.z < num3 {
            light_color.z = num3;
        }
    }

    /// `TileLightScanner.ApplyHellLight` (TileLightScanner.cs:3259)
    #[allow(unused_variables, unused_assignments, unused_mut)]
    fn apply_hell_light(&self, g: &Game, tile: &Tile, x: i32, y: i32, light_color: &mut Vector3) {
        let mut num: f32 = 0f32;
        let mut num2: f32 = 0f32;
        let mut num3: f32 = 0f32;
        let num4: f32 = 0.55f32 + ((g.main.global_time_wrapped_hourly * 2f32) as f64).sin() as f32 * 0.08f32;
        if (!tile.active() || !g.main.tile_no_sun_light[tile.type_ as usize] || (((tile.slope() as i32) != 0 || tile.half_brick()) && (g.main.tile.get(x, y - 1).liquid as i32) == 0 && (g.main.tile.get(x, y + 1).liquid as i32) == 0 && (g.main.tile.get(x - 1, y).liquid as i32) == 0 && (g.main.tile.get(x + 1, y).liquid as i32) == 0)) && light_color.x < num4 && (g.main.wall_light[tile.wall as usize] || (tile.wall as i32) == 73 || (tile.wall as i32) == 227 || (tile.invisible_wall() && !self.draw_invisible_walls)) && (tile.liquid as i32) < 200 && (!tile.half_brick() || (g.main.tile.get(x, y - 1).liquid as i32) < 200) {
            num = num4;
            num2 = num4 * 0.6f32;
            num3 = num4 * 0.2f32;
        }
        if (!tile.active() || tile.half_brick() || !g.main.tile_no_sun_light[tile.type_ as usize]) && (((tile.wall as i32) >= 88 && (tile.wall as i32) <= 93) || (tile.wall as i32) == 241) && (tile.liquid as i32) < 255 {
            num = num4;
            num2 = num4 * 0.6f32;
            num3 = num4 * 0.2f32;
            let mut num5: i32 = (tile.wall as i32) - 88;
            if (tile.wall as i32) == 241 {
                num5 = 6;
            }
            match num5 {
                0 => {
                    num *= 0.9f32;
                    num2 *= 0.15f32;
                    num3 *= 0.9f32;
                }
                1 => {
                    num *= 0.9f32;
                    num2 *= 0.9f32;
                    num3 *= 0.15f32;
                }
                2 => {
                    num *= 0.15f32;
                    num2 *= 0.15f32;
                    num3 *= 0.9f32;
                }
                3 => {
                    num *= 0.15f32;
                    num2 *= 0.9f32;
                    num3 *= 0.15f32;
                }
                4 => {
                    num *= 0.9f32;
                    num2 *= 0.15f32;
                    num3 *= 0.15f32;
                }
                5 => {
                    let num6: f32 = 0.2f32;
                    let num7: f32 = 0.7f32 - num6;
                    num *= num7 + g.main.disco_r as f32 / 255f32 * num6;
                    num2 *= num7 + g.main.disco_g as f32 / 255f32 * num6;
                    num3 *= num7 + g.main.disco_b as f32 / 255f32 * num6;
                }
                6 => {
                    num *= 0.9f32;
                    num2 *= 0.5f32;
                    num3 *= 0f32;
                }
                _ => {}
            }
        }
        if light_color.x < num {
            light_color.x = num;
        }
        if light_color.y < num2 {
            light_color.y = num2;
        }
        if light_color.z < num3 {
            light_color.z = num3;
        }
    }
}
