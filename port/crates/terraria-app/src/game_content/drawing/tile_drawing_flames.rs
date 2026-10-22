//! `TileDrawing.GetTileFlameData` (TileDrawing.cs:1870) and `TileDrawing.DrawSingleTile_Flames` (TileDrawing.cs:2707): the animated
//! glow and flame overlays drawn on top of lit furniture (torches, candles, lamps, chandeliers, ...).
//!
//! The two big `switch` bodies were produced from the C# by `tools/port_scripts/cs_tile_flames.py` and then fixed by hand where the
//! output did not compile; the statement order and every constant are the original's. Local variables keep the decompiler's names
//! in snake case. `TileFlameData` is a struct of `TileDrawing` in the original.

#![allow(unused_mut, unused_assignments, unused_variables, unused_parens, clippy::needless_late_init, clippy::unnecessary_cast, clippy::identity_op)]

use terraria_core::xna::{Color, Rectangle, Vector2};
use terraria_sim::unported;

use crate::data_structures::tile_draw_info::TileDrawInfo;
use crate::draw_context::DrawContext;
use crate::gfx::Texture2D;
use crate::pending::{main_moon_phase, utils_wrapped_lerp};

use super::tile_drawing::{TileDrawing, ZERO};
use super::tile_drawing_specials::utils_random_int;

/// `TileDrawing.TileFlameData` (TileDrawing.cs:35). `default(TileFlameData)`: no texture, zero count, `Color.Transparent`.
#[derive(Clone, Debug, Default)]
pub struct TileFlameData {
    pub flame_texture: Option<Texture2D>,
    pub flame_seed: u64,
    pub flame_count: i32,
    pub flame_color: Color,
    pub flame_range_x_min: i32,
    pub flame_range_x_max: i32,
    pub flame_range_y_min: i32,
    pub flame_range_y_max: i32,
    pub flame_range_mult_x: f32,
    pub flame_range_mult_y: f32,
}

/// `Utils.RotatedBy(this Vector2 spinningpoint, double radians, Vector2 center = default(Vector2))` (Utils.cs:1694), centre at the origin
fn vector2_rotated_by(spinningpoint: Vector2, radians: f32) -> Vector2 {
    let radians = radians as f64;
    let num = radians.cos() as f32;
    let num2 = radians.sin() as f32;
    let vector = spinningpoint;
    let mut result = Vector2::ZERO;
    result.x += vector.x * num - vector.y * num2;
    result.y += vector.x * num2 + vector.y * num;
    result
}

/// `Texture2D.Frame()` (Utils.cs:1361 with every argument defaulted): the whole texture
fn texture_frame_full(tex: Option<&Texture2D>) -> Rectangle {
    match tex {
        Some(t) => Rectangle::new(0, 0, t.width, t.height),
        None => Rectangle::EMPTY, // nothing is drawn with a missing texture
    }
}

impl TileDrawing {
    /// `GetFallenStarFurnitureFlameColor()` (TileDrawing.cs:1830)
    pub(super) fn get_fallen_star_furniture_flame_color(&self, ctx: &DrawContext) -> Color {
        let num = utils_wrapped_lerp(0.5, 1.0, ctx.g.main.global_time_wrapped_hourly % 2.0 / 2.0);
        let num2 = (150.0_f32 * num) as i32;
        Color::from_ints(150, num2, num2, 50)
    }

    /// `GetHallowedFurnitureFlameColor()` (TileDrawing.cs:1837)
    pub(super) fn get_hallowed_furniture_flame_color(&self, ctx: &DrawContext) -> Color {
        let num = utils_wrapped_lerp(0.5, 1.0, ctx.g.main.global_time_wrapped_hourly % 2.0 / 2.0);
        let num2 = (170.0_f32 * num) as i32;
        Color::from_ints(170, num2, num2, 75)
    }

    /// `GetCloudFurnitureFlameColor()` (TileDrawing.cs:1844)
    pub(super) fn get_cloud_furniture_flame_color(&self, ctx: &DrawContext) -> Color {
        self.get_wrapped_furniture_flame_color(ctx, Color::from_ints(255, 255, 255, 0), 0.75, 1.0)
    }

    /// `GetLibrarianFurnitureFlameColor()` (TileDrawing.cs:1849)
    pub(super) fn get_librarian_furniture_flame_color(&self, ctx: &DrawContext) -> Color {
        self.get_wrapped_furniture_flame_color(ctx, Color::from_ints(255, 255, 255, 0), 0.25, 1.0)
    }

    /// `GetForbiddenFurnitureFlameColor()` (TileDrawing.cs:1854)
    pub(super) fn get_forbidden_furniture_flame_color(&self, ctx: &DrawContext) -> Color {
        self.get_wrapped_furniture_flame_color(ctx, Color::from_ints(255, 255, 255, 0), 0.25, 1.0)
    }

    /// `GetBoulderFurnitureFlameColor()` (TileDrawing.cs:1859)
    pub(super) fn get_boulder_furniture_flame_color(&self, ctx: &DrawContext) -> Color {
        self.get_wrapped_furniture_flame_color(ctx, Color::from_ints(255, 255, 255, 0), 0.25, 1.0)
    }

    /// `GetWrappedFurnitureFlameColor(Color baseColor, float min = 0.75f, float max = 1f)` (TileDrawing.cs:1864)
    fn get_wrapped_furniture_flame_color(&self, ctx: &DrawContext, base_color: Color, min: f32, max: f32) -> Color {
        let num = utils_wrapped_lerp(min, max, ctx.g.main.global_time_wrapped_hourly % 2.0 / 2.0);
        base_color * num
    }

    /// `GetTileFlameData(int tileX, int tileY, int type, int tileFrameY)` (TileDrawing.cs:1870)
    pub(super) fn get_tile_flame_data(&self, ctx: &mut DrawContext, tile_x: i32, tile_y: i32, type_: i32, tile_frame_y: i32) -> TileFlameData {
        match type_ {
            270 => {
                return TileFlameData { flame_texture: ctx.firefly_jar(), flame_color: Color::from_ints(200, 200, 200, 0), flame_count: 1, ..Default::default() };
            }
            271 => {
                return TileFlameData { flame_texture: ctx.lightningbug_jar(), flame_color: Color::from_ints(200, 200, 200, 0), flame_count: 1, ..Default::default() };
            }
            581 => {
                return TileFlameData { flame_texture: ctx.glow_mask(291), flame_color: Color::from_ints(200, 100, 100, 0), flame_count: 1, ..Default::default() };
            }
            _ => {}
        }
        if !ctx.g.main.tile_flame[type_ as usize] {
            return TileFlameData::default();
        }
        let flame_seed: u64 = ctx.state.tile_frame_seed ^ ((((tile_x as i64) << 32) | (tile_y as u32 as i64)) as u64);
        let mut num = 0;
        match type_ {
            4 => num = 0,
            33 | 174 => num = 1,
            100 | 173 => num = 2,
            34 => num = 3,
            93 => num = 4,
            49 => num = 5,
            372 => num = 16,
            646 => num = 17,
            98 => num = 6,
            35 => num = 7,
            42 => num = 13,
            _ => {}
        }
        let mut result = TileFlameData { flame_texture: ctx.flame(num), flame_seed, ..Default::default() };
        match num {
            7 => {
                result.flame_count = 4;
                result.flame_color = Color::from_ints(50, 50, 50, 0);
                result.flame_range_x_min = -10;
                result.flame_range_x_max = 11;
                result.flame_range_y_min = -10;
                result.flame_range_y_max = 10;
                result.flame_range_mult_x = 0.0_f32;
                result.flame_range_mult_y = 0.0_f32;
            }
            1 => {
                match ctx.g.main.tile.get(tile_x, tile_y).frame_y as i32 / 22 {
                    5 | 6 | 7 | 10 => {
                        result.flame_count = 7;
                        result.flame_color = Color::from_ints(50, 50, 50, 0);
                        result.flame_range_x_min = -10;
                        result.flame_range_x_max = 11;
                        result.flame_range_y_min = -10;
                        result.flame_range_y_max = 11;
                        result.flame_range_mult_x = 0.075_f32;
                        result.flame_range_mult_y = 0.075_f32;
                    }
                    8 => {
                        result.flame_count = 7;
                        result.flame_color = Color::from_ints(50, 50, 50, 0);
                        result.flame_range_x_min = -10;
                        result.flame_range_x_max = 11;
                        result.flame_range_y_min = -10;
                        result.flame_range_y_max = 11;
                        result.flame_range_mult_x = 0.3_f32;
                        result.flame_range_mult_y = 0.3_f32;
                    }
                    12 => {
                        result.flame_count = 7;
                        result.flame_color = Color::from_ints(50, 50, 50, 0);
                        result.flame_range_x_min = -10;
                        result.flame_range_x_max = 11;
                        result.flame_range_y_min = -10;
                        result.flame_range_y_max = 1;
                        result.flame_range_mult_x = 0.1_f32;
                        result.flame_range_mult_y = 0.15_f32;
                    }
                    14 => {
                        result.flame_count = 8;
                        result.flame_color = Color::from_ints(75, 75, 75, 0);
                        result.flame_range_x_min = -10;
                        result.flame_range_x_max = 11;
                        result.flame_range_y_min = -10;
                        result.flame_range_y_max = 11;
                        result.flame_range_mult_x = 0.1_f32;
                        result.flame_range_mult_y = 0.1_f32;
                    }
                    16 => {
                        result.flame_count = 4;
                        result.flame_color = Color::from_ints(75, 75, 75, 0);
                        result.flame_range_x_min = -10;
                        result.flame_range_x_max = 11;
                        result.flame_range_y_min = -10;
                        result.flame_range_y_max = 11;
                        result.flame_range_mult_x = 0.15_f32;
                        result.flame_range_mult_y = 0.15_f32;
                    }
                    27 | 28 => {
                        result.flame_count = 1;
                        result.flame_color = Color::from_ints(75, 75, 75, 0);
                        result.flame_range_x_min = -10;
                        result.flame_range_x_max = 11;
                        result.flame_range_y_min = -10;
                        result.flame_range_y_max = 11;
                        result.flame_range_mult_x = 0.0_f32;
                        result.flame_range_mult_y = 0.0_f32;
                    }
                    43 => {
                        result.flame_count = 1;
                        result.flame_color = self.get_fallen_star_furniture_flame_color(ctx);
                        result.flame_range_mult_x = 0.0_f32;
                        result.flame_range_mult_y = 0.0_f32;
                    }
                    44 => {
                        result.flame_count = 3;
                        result.flame_color = Color::from_ints(200, 200, 200, 150);
                        result.flame_range_x_min = -10;
                        result.flame_range_x_max = 11;
                        result.flame_range_y_min = -10;
                        result.flame_range_y_max = 1;
                        result.flame_range_mult_x = 0.15_f32;
                        result.flame_range_mult_y = 0.35_f32;
                    }
                    45 => {
                        result.flame_count = 1;
                        result.flame_color = self.get_hallowed_furniture_flame_color(ctx);
                        result.flame_range_mult_x = 0.0_f32;
                        result.flame_range_mult_y = 0.0_f32;
                    }
                    56 => {
                        result.flame_count = 1;
                        result.flame_color = self.get_cloud_furniture_flame_color(ctx);
                        result.flame_range_mult_x = 0.0_f32;
                        result.flame_range_mult_y = 0.0_f32;
                    }
                    57 | 60 => {
                        result.flame_count = 1;
                        result.flame_color = Color::from_ints(200, 200, 200, 150);
                        result.flame_range_mult_x = 0.0_f32;
                        result.flame_range_mult_y = 0.0_f32;
                    }
                    58 => {
                        result.flame_count = 1;
                        result.flame_color = self.get_librarian_furniture_flame_color(ctx);
                        result.flame_range_mult_x = 0.0_f32;
                        result.flame_range_mult_y = 0.0_f32;
                    }
                    61 => {
                        result.flame_count = 1;
                        result.flame_color = self.get_forbidden_furniture_flame_color(ctx);
                        result.flame_range_mult_x = 0.0_f32;
                        result.flame_range_mult_y = 0.0_f32;
                    }
                    63 => {
                        result.flame_count = 1;
                        result.flame_color = self.get_boulder_furniture_flame_color(ctx);
                        result.flame_range_mult_x = 0.0_f32;
                        result.flame_range_mult_y = 0.0_f32;
                    }
                    47 | 48 | 49 | 51 | 52 | 54 => {
                        result.flame_count = 0;
                    }
                    _ => {
                        result.flame_count = 7;
                        result.flame_color = Color::from_ints(100, 100, 100, 0);
                        result.flame_range_x_min = -10;
                        result.flame_range_x_max = 11;
                        result.flame_range_y_min = -10;
                        result.flame_range_y_max = 1;
                        result.flame_range_mult_x = 0.15_f32;
                        result.flame_range_mult_y = 0.35_f32;
                    }
                }
            }
            2 => {
                match ctx.g.main.tile.get(tile_x, tile_y).frame_y as i32 / 36 {
                    3 => {
                        result.flame_count = 3;
                        result.flame_color = Color::from_ints(50, 50, 50, 0);
                        result.flame_range_x_min = -10;
                        result.flame_range_x_max = 11;
                        result.flame_range_y_min = -10;
                        result.flame_range_y_max = 11;
                        result.flame_range_mult_x = 0.05_f32;
                        result.flame_range_mult_y = 0.15_f32;
                    }
                    6 => {
                        result.flame_count = 5;
                        result.flame_color = Color::from_ints(75, 75, 75, 0);
                        result.flame_range_x_min = -10;
                        result.flame_range_x_max = 11;
                        result.flame_range_y_min = -10;
                        result.flame_range_y_max = 11;
                        result.flame_range_mult_x = 0.15_f32;
                        result.flame_range_mult_y = 0.15_f32;
                    }
                    9 => {
                        result.flame_count = 7;
                        result.flame_color = Color::from_ints(100, 100, 100, 0);
                        result.flame_range_x_min = -10;
                        result.flame_range_x_max = 11;
                        result.flame_range_y_min = -10;
                        result.flame_range_y_max = 11;
                        result.flame_range_mult_x = 0.3_f32;
                        result.flame_range_mult_y = 0.3_f32;
                    }
                    11 => {
                        result.flame_count = 7;
                        result.flame_color = Color::from_ints(50, 50, 50, 0);
                        result.flame_range_x_min = -10;
                        result.flame_range_x_max = 11;
                        result.flame_range_y_min = -10;
                        result.flame_range_y_max = 1;
                        result.flame_range_mult_x = 0.1_f32;
                        result.flame_range_mult_y = 0.15_f32;
                    }
                    13 => {
                        result.flame_count = 8;
                        result.flame_color = Color::from_ints(75, 75, 75, 0);
                        result.flame_range_x_min = -10;
                        result.flame_range_x_max = 11;
                        result.flame_range_y_min = -10;
                        result.flame_range_y_max = 11;
                        result.flame_range_mult_x = 0.1_f32;
                        result.flame_range_mult_y = 0.1_f32;
                    }
                    28 | 29 => {
                        result.flame_count = 1;
                        result.flame_color = Color::from_ints(75, 75, 75, 0);
                        result.flame_range_x_min = -10;
                        result.flame_range_x_max = 11;
                        result.flame_range_y_min = -10;
                        result.flame_range_y_max = 1;
                        result.flame_range_mult_x = 0.0_f32;
                        result.flame_range_mult_y = 0.0_f32;
                    }
                    44 => {
                        result.flame_count = 1;
                        result.flame_color = self.get_fallen_star_furniture_flame_color(ctx);
                        result.flame_range_mult_x = 0.0_f32;
                        result.flame_range_mult_y = 0.0_f32;
                    }
                    45 => {
                        result.flame_count = 3;
                        result.flame_color = Color::from_ints(200, 200, 200, 150);
                        result.flame_range_x_min = -10;
                        result.flame_range_x_max = 11;
                        result.flame_range_y_min = -10;
                        result.flame_range_y_max = 1;
                        result.flame_range_mult_x = 0.15_f32;
                        result.flame_range_mult_y = 0.35_f32;
                    }
                    46 => {
                        result.flame_count = 1;
                        result.flame_color = self.get_hallowed_furniture_flame_color(ctx);
                        result.flame_range_mult_x = 0.0_f32;
                        result.flame_range_mult_y = 0.0_f32;
                    }
                    57 => {
                        result.flame_count = 1;
                        result.flame_color = self.get_cloud_furniture_flame_color(ctx);
                        result.flame_range_mult_x = 0.0_f32;
                        result.flame_range_mult_y = 0.0_f32;
                    }
                    58 | 61 => {
                        result.flame_count = 1;
                        result.flame_color = Color::from_ints(200, 200, 200, 150);
                        result.flame_range_mult_x = 0.0_f32;
                        result.flame_range_mult_y = 0.0_f32;
                    }
                    59 => {
                        result.flame_count = 1;
                        result.flame_color = self.get_librarian_furniture_flame_color(ctx);
                        result.flame_range_mult_x = 0.0_f32;
                        result.flame_range_mult_y = 0.0_f32;
                    }
                    62 => {
                        result.flame_count = 1;
                        result.flame_color = self.get_forbidden_furniture_flame_color(ctx);
                        result.flame_range_mult_x = 0.0_f32;
                        result.flame_range_mult_y = 0.0_f32;
                    }
                    64 => {
                        result.flame_count = 1;
                        result.flame_color = self.get_boulder_furniture_flame_color(ctx);
                        result.flame_range_mult_x = 0.0_f32;
                        result.flame_range_mult_y = 0.0_f32;
                    }
                    48 | 49 | 50 | 52 | 53 | 55 => {
                        result.flame_count = 0;
                    }
                    _ => {
                        result.flame_count = 7;
                        result.flame_color = Color::from_ints(100, 100, 100, 0);
                        result.flame_range_x_min = -10;
                        result.flame_range_x_max = 11;
                        result.flame_range_y_min = -10;
                        result.flame_range_y_max = 1;
                        result.flame_range_mult_x = 0.15_f32;
                        result.flame_range_mult_y = 0.35_f32;
                    }
                }
            }
            3 => {
                let mut num2: i32 = ctx.g.main.tile.get(tile_x, tile_y).frame_y as i32 / 54;
                if ctx.g.main.tile.get(tile_x, tile_y).frame_x as i32 >= 108 {
                    num2 += 37 * (ctx.g.main.tile.get(tile_x, tile_y).frame_x as i32 / 108);
                }
                match num2 {
                    8 => {
                        result.flame_count = 7;
                        result.flame_color = Color::from_ints(50, 50, 50, 0);
                        result.flame_range_x_min = -10;
                        result.flame_range_x_max = 11;
                        result.flame_range_y_min = -10;
                        result.flame_range_y_max = 11;
                        result.flame_range_mult_x = 0.075_f32;
                        result.flame_range_mult_y = 0.075_f32;
                    }
                    9 => {
                        result.flame_count = 3;
                        result.flame_color = Color::from_ints(50, 50, 50, 0);
                        result.flame_range_x_min = -1;
                        result.flame_range_x_max = 1;
                        result.flame_range_y_min = -1;
                        result.flame_range_y_max = 1;
                        result.flame_range_mult_x = 2.0_f32;
                        result.flame_range_mult_y = 2.0_f32;
                    }
                    11 => {
                        result.flame_count = 7;
                        result.flame_color = Color::from_ints(50, 50, 50, 0);
                        result.flame_range_x_min = -10;
                        result.flame_range_x_max = 11;
                        result.flame_range_y_min = -10;
                        result.flame_range_y_max = 11;
                        result.flame_range_mult_x = 0.3_f32;
                        result.flame_range_mult_y = 0.3_f32;
                    }
                    15 => {
                        result.flame_count = 7;
                        result.flame_color = Color::from_ints(50, 50, 50, 0);
                        result.flame_range_x_min = -10;
                        result.flame_range_x_max = 11;
                        result.flame_range_y_min = -10;
                        result.flame_range_y_max = 1;
                        result.flame_range_mult_x = 0.1_f32;
                        result.flame_range_mult_y = 0.15_f32;
                    }
                    17 | 20 => {
                        result.flame_count = 7;
                        result.flame_color = Color::from_ints(50, 50, 50, 0);
                        result.flame_range_x_min = -10;
                        result.flame_range_x_max = 11;
                        result.flame_range_y_min = -10;
                        result.flame_range_y_max = 11;
                        result.flame_range_mult_x = 0.075_f32;
                        result.flame_range_mult_y = 0.075_f32;
                    }
                    18 => {
                        result.flame_count = 8;
                        result.flame_color = Color::from_ints(75, 75, 75, 0);
                        result.flame_range_x_min = -10;
                        result.flame_range_x_max = 11;
                        result.flame_range_y_min = -10;
                        result.flame_range_y_max = 11;
                        result.flame_range_mult_x = 0.1_f32;
                        result.flame_range_mult_y = 0.1_f32;
                    }
                    34 | 35 => {
                        result.flame_count = 1;
                        result.flame_color = Color::from_ints(75, 75, 75, 0);
                        result.flame_range_x_min = -10;
                        result.flame_range_x_max = 11;
                        result.flame_range_y_min = -10;
                        result.flame_range_y_max = 11;
                        result.flame_range_mult_x = 0.0_f32;
                        result.flame_range_mult_y = 0.0_f32;
                    }
                    50 => {
                        result.flame_count = 1;
                        result.flame_color = self.get_fallen_star_furniture_flame_color(ctx);
                        result.flame_range_mult_x = 0.0_f32;
                        result.flame_range_mult_y = 0.0_f32;
                    }
                    51 => {
                        result.flame_count = 3;
                        result.flame_color = Color::from_ints(200, 200, 200, 150);
                        result.flame_range_x_min = -10;
                        result.flame_range_x_max = 11;
                        result.flame_range_y_min = -10;
                        result.flame_range_y_max = 1;
                        result.flame_range_mult_x = 0.15_f32;
                        result.flame_range_mult_y = 0.35_f32;
                    }
                    52 => {
                        result.flame_count = 1;
                        result.flame_color = self.get_hallowed_furniture_flame_color(ctx);
                        result.flame_range_mult_x = 0.0_f32;
                        result.flame_range_mult_y = 0.0_f32;
                    }
                    63 => {
                        result.flame_count = 1;
                        result.flame_color = self.get_cloud_furniture_flame_color(ctx);
                        result.flame_range_mult_x = 0.0_f32;
                        result.flame_range_mult_y = 0.0_f32;
                    }
                    64 | 67 => {
                        result.flame_count = 1;
                        result.flame_color = Color::from_ints(200, 200, 200, 150);
                        result.flame_range_mult_x = 0.0_f32;
                        result.flame_range_mult_y = 0.0_f32;
                    }
                    65 => {
                        result.flame_count = 1;
                        result.flame_color = self.get_librarian_furniture_flame_color(ctx);
                        result.flame_range_mult_x = 0.0_f32;
                        result.flame_range_mult_y = 0.0_f32;
                    }
                    68 => {
                        result.flame_count = 1;
                        result.flame_color = self.get_forbidden_furniture_flame_color(ctx);
                        result.flame_range_mult_x = 0.0_f32;
                        result.flame_range_mult_y = 0.0_f32;
                    }
                    70 => {
                        result.flame_count = 1;
                        result.flame_color = self.get_boulder_furniture_flame_color(ctx);
                        result.flame_range_mult_x = 0.0_f32;
                        result.flame_range_mult_y = 0.0_f32;
                    }
                    54 | 55 | 56 | 58 | 59 | 61 => {
                        result.flame_count = 0;
                    }
                    _ => {
                        result.flame_count = 7;
                        result.flame_color = Color::from_ints(100, 100, 100, 0);
                        result.flame_range_x_min = -10;
                        result.flame_range_x_max = 11;
                        result.flame_range_y_min = -10;
                        result.flame_range_y_max = 1;
                        result.flame_range_mult_x = 0.15_f32;
                        result.flame_range_mult_y = 0.35_f32;
                    }
                }
            }
            4 => {
                match ctx.g.main.tile.get(tile_x, tile_y).frame_y as i32 / 54 {
                    1 => {
                        result.flame_count = 3;
                        result.flame_color = Color::from_ints(50, 50, 50, 0);
                        result.flame_range_x_min = -10;
                        result.flame_range_x_max = 11;
                        result.flame_range_y_min = -10;
                        result.flame_range_y_max = 11;
                        result.flame_range_mult_x = 0.15_f32;
                        result.flame_range_mult_y = 0.15_f32;
                    }
                    2 | 4 => {
                        result.flame_count = 7;
                        result.flame_color = Color::from_ints(50, 50, 50, 0);
                        result.flame_range_x_min = -10;
                        result.flame_range_x_max = 11;
                        result.flame_range_y_min = -10;
                        result.flame_range_y_max = 11;
                        result.flame_range_mult_x = 0.075_f32;
                        result.flame_range_mult_y = 0.075_f32;
                    }
                    3 => {
                        result.flame_count = 7;
                        result.flame_color = Color::from_ints(100, 100, 100, 0);
                        result.flame_range_x_min = -10;
                        result.flame_range_x_max = 11;
                        result.flame_range_y_min = -20;
                        result.flame_range_y_max = 1;
                        result.flame_range_mult_x = 0.2_f32;
                        result.flame_range_mult_y = 0.35_f32;
                    }
                    5 => {
                        result.flame_count = 7;
                        result.flame_color = Color::from_ints(50, 50, 50, 0);
                        result.flame_range_x_min = -10;
                        result.flame_range_x_max = 11;
                        result.flame_range_y_min = -10;
                        result.flame_range_y_max = 11;
                        result.flame_range_mult_x = 0.3_f32;
                        result.flame_range_mult_y = 0.3_f32;
                    }
                    9 => {
                        result.flame_count = 7;
                        result.flame_color = Color::from_ints(50, 50, 50, 0);
                        result.flame_range_x_min = -10;
                        result.flame_range_x_max = 11;
                        result.flame_range_y_min = -10;
                        result.flame_range_y_max = 1;
                        result.flame_range_mult_x = 0.1_f32;
                        result.flame_range_mult_y = 0.15_f32;
                    }
                    13 => {
                        result.flame_count = 8;
                        result.flame_color = Color::from_ints(50, 50, 50, 0);
                        result.flame_range_x_min = -10;
                        result.flame_range_x_max = 11;
                        result.flame_range_y_min = -10;
                        result.flame_range_y_max = 11;
                        result.flame_range_mult_x = 0.1_f32;
                        result.flame_range_mult_y = 0.1_f32;
                    }
                    12 => {
                        result.flame_count = 1;
                        result.flame_color = Color::from_ints(100, 100, 100, 0);
                        result.flame_range_x_min = -10;
                        result.flame_range_x_max = 11;
                        result.flame_range_y_min = -10;
                        result.flame_range_y_max = 11;
                        result.flame_range_mult_x = 0.01_f32;
                        result.flame_range_mult_y = 0.01_f32;
                    }
                    28 | 29 => {
                        result.flame_count = 1;
                        result.flame_color = Color::from_ints(75, 75, 75, 0);
                        result.flame_range_x_min = -10;
                        result.flame_range_x_max = 11;
                        result.flame_range_y_min = -10;
                        result.flame_range_y_max = 11;
                        result.flame_range_mult_x = 0.0_f32;
                        result.flame_range_mult_y = 0.0_f32;
                    }
                    44 => {
                        result.flame_count = 1;
                        result.flame_color = self.get_fallen_star_furniture_flame_color(ctx);
                        result.flame_range_mult_x = 0.0_f32;
                        result.flame_range_mult_y = 0.0_f32;
                    }
                    45 => {
                        result.flame_count = 3;
                        result.flame_color = Color::from_ints(200, 200, 200, 150);
                        result.flame_range_x_min = -10;
                        result.flame_range_x_max = 11;
                        result.flame_range_y_min = -10;
                        result.flame_range_y_max = 1;
                        result.flame_range_mult_x = 0.15_f32;
                        result.flame_range_mult_y = 0.35_f32;
                    }
                    46 => {
                        result.flame_count = 1;
                        result.flame_color = self.get_hallowed_furniture_flame_color(ctx);
                        result.flame_range_mult_x = 0.0_f32;
                        result.flame_range_mult_y = 0.0_f32;
                    }
                    57 => {
                        result.flame_count = 1;
                        result.flame_color = self.get_cloud_furniture_flame_color(ctx);
                        result.flame_range_mult_x = 0.0_f32;
                        result.flame_range_mult_y = 0.0_f32;
                    }
                    58 | 61 => {
                        result.flame_count = 1;
                        result.flame_color = Color::from_ints(200, 200, 200, 150);
                        result.flame_range_mult_x = 0.0_f32;
                        result.flame_range_mult_y = 0.0_f32;
                    }
                    59 => {
                        result.flame_count = 1;
                        result.flame_color = self.get_librarian_furniture_flame_color(ctx);
                        result.flame_range_mult_x = 0.0_f32;
                        result.flame_range_mult_y = 0.0_f32;
                    }
                    62 => {
                        result.flame_count = 1;
                        result.flame_color = self.get_forbidden_furniture_flame_color(ctx);
                        result.flame_range_mult_x = 0.0_f32;
                        result.flame_range_mult_y = 0.0_f32;
                    }
                    64 => {
                        result.flame_count = 1;
                        result.flame_color = self.get_boulder_furniture_flame_color(ctx);
                        result.flame_range_mult_x = 0.0_f32;
                        result.flame_range_mult_y = 0.0_f32;
                    }
                    48 | 49 | 50 | 52 | 53 | 55 => {
                        result.flame_count = 0;
                    }
                    _ => {
                        result.flame_count = 7;
                        result.flame_color = Color::from_ints(100, 100, 100, 0);
                        result.flame_range_x_min = -10;
                        result.flame_range_x_max = 11;
                        result.flame_range_y_min = -10;
                        result.flame_range_y_max = 1;
                        result.flame_range_mult_x = 0.15_f32;
                        result.flame_range_mult_y = 0.35_f32;
                    }
                }
            }
            13 => {
                match tile_frame_y / 36 {
                    1 | 3 | 6 | 8 | 19 | 27 | 29 | 30 | 31 | 32 | 36 | 39 | 53 | 57 | 60 | 62 | 66 | 69 => {
                        result.flame_count = 7;
                        result.flame_color = Color::from_ints(100, 100, 100, 0);
                        result.flame_range_x_min = -10;
                        result.flame_range_x_max = 11;
                        result.flame_range_y_min = -10;
                        result.flame_range_y_max = 1;
                        result.flame_range_mult_x = 0.15_f32;
                        result.flame_range_mult_y = 0.35_f32;
                    }
                    2 | 16 | 25 => {
                        result.flame_count = 7;
                        result.flame_color = Color::from_ints(50, 50, 50, 0);
                        result.flame_range_x_min = -10;
                        result.flame_range_x_max = 11;
                        result.flame_range_y_min = -10;
                        result.flame_range_y_max = 1;
                        result.flame_range_mult_x = 0.15_f32;
                        result.flame_range_mult_y = 0.1_f32;
                    }
                    11 => {
                        result.flame_count = 7;
                        result.flame_color = Color::from_ints(50, 50, 50, 0);
                        result.flame_range_x_min = -10;
                        result.flame_range_x_max = 11;
                        result.flame_range_y_min = -10;
                        result.flame_range_y_max = 11;
                        result.flame_range_mult_x = 0.075_f32;
                        result.flame_range_mult_y = 0.075_f32;
                    }
                    34 | 35 => {
                        result.flame_count = 1;
                        result.flame_color = Color::from_ints(75, 75, 75, 0);
                        result.flame_range_x_min = -10;
                        result.flame_range_x_max = 11;
                        result.flame_range_y_min = -10;
                        result.flame_range_y_max = 1;
                        result.flame_range_mult_x = 0.0_f32;
                        result.flame_range_mult_y = 0.0_f32;
                    }
                    44 => {
                        result.flame_count = 7;
                        result.flame_color = Color::from_ints(100, 100, 100, 0);
                        result.flame_range_x_min = -10;
                        result.flame_range_x_max = 11;
                        result.flame_range_y_min = -10;
                        result.flame_range_y_max = 1;
                        result.flame_range_mult_x = 0.15_f32;
                        result.flame_range_mult_y = 0.35_f32;
                    }
                    50 => {
                        result.flame_count = 1;
                        result.flame_color = self.get_fallen_star_furniture_flame_color(ctx);
                        result.flame_range_mult_x = 0.0_f32;
                        result.flame_range_mult_y = 0.0_f32;
                    }
                    51 => {
                        result.flame_count = 3;
                        result.flame_color = Color::from_ints(200, 200, 200, 150);
                        result.flame_range_x_min = -10;
                        result.flame_range_x_max = 11;
                        result.flame_range_y_min = -10;
                        result.flame_range_y_max = 1;
                        result.flame_range_mult_x = 0.15_f32;
                        result.flame_range_mult_y = 0.35_f32;
                    }
                    52 => {
                        result.flame_count = 1;
                        result.flame_color = self.get_hallowed_furniture_flame_color(ctx);
                        result.flame_range_mult_x = 0.0_f32;
                        result.flame_range_mult_y = 0.0_f32;
                    }
                    63 => {
                        result.flame_count = 1;
                        result.flame_color = self.get_cloud_furniture_flame_color(ctx);
                        result.flame_range_mult_x = 0.0_f32;
                        result.flame_range_mult_y = 0.0_f32;
                    }
                    64 | 67 => {
                        result.flame_count = 1;
                        result.flame_color = Color::from_ints(200, 200, 200, 150);
                        result.flame_range_mult_x = 0.0_f32;
                        result.flame_range_mult_y = 0.0_f32;
                    }
                    65 => {
                        result.flame_count = 1;
                        result.flame_color = self.get_librarian_furniture_flame_color(ctx);
                        result.flame_range_mult_x = 0.0_f32;
                        result.flame_range_mult_y = 0.0_f32;
                    }
                    68 => {
                        result.flame_count = 1;
                        result.flame_color = self.get_forbidden_furniture_flame_color(ctx);
                        result.flame_range_mult_x = 0.0_f32;
                        result.flame_range_mult_y = 0.0_f32;
                    }
                    70 => {
                        result.flame_count = 1;
                        result.flame_color = self.get_boulder_furniture_flame_color(ctx);
                        result.flame_range_mult_x = 0.0_f32;
                        result.flame_range_mult_y = 0.0_f32;
                    }
                    54 | 55 | 56 | 58 | 59 | 61 => {
                        result.flame_count = 0;
                    }
                    _ => {
                        result.flame_count = 0;
                    }
                }
            }
            _ => {
                result.flame_count = 7;
                result.flame_color = Color::from_ints(100, 100, 100, 0);
                if tile_frame_y / 22 == 14 {
                    result.flame_color = Color::from_floats((ctx.g.main.disco_r as f32) / 255.0_f32, (ctx.g.main.disco_g as f32) / 255.0_f32, (ctx.g.main.disco_b as f32) / 255.0_f32, 0.0_f32);
                }
                result.flame_range_x_min = -10;
                result.flame_range_x_max = 11;
                result.flame_range_y_min = -10;
                result.flame_range_y_max = 1;
                result.flame_range_mult_x = 0.15_f32;
                result.flame_range_mult_y = 0.35_f32;
            }
        }
        result
    }

    /// `DrawSingleTile_Flames(Vector2 screenPosition, Vector2 screenOffset, int tileX, int tileY, TileDrawInfo drawData)` (TileDrawing.cs:2707)
    pub(super) fn draw_single_tile_flames(&mut self, ctx: &mut DrawContext, screen_position: Vector2, screen_offset: Vector2, tile_x: i32, tile_y: i32, draw_data: &TileDrawInfo) {
        if (draw_data.type_cache as i32) == 548 && (draw_data.tile_frame_x as i32) / 54 > 6 {
            let tex1 = ctx.glow_mask(297);
            ctx.tile_batch.draw(tex1.as_ref(), Vector2::new(((tile_x * 16 - (screen_position.x as i32)) as f32) - ((draw_data.tile_width as f32) - 16.0_f32) / 2.0_f32, (tile_y * 16 - (screen_position.y as i32) + draw_data.tile_top) as f32) + screen_offset, Rectangle::new(((draw_data.tile_frame_x as i32) + draw_data.add_fr_x) as i32, ((draw_data.tile_frame_y as i32) + draw_data.add_fr_y) as i32, (draw_data.tile_width) as i32, (draw_data.tile_height) as i32), (Color::WHITE).into(), ZERO, 1.0_f32, draw_data.tile_sprite_effect);
        }
        if (draw_data.type_cache as i32) == 613 {
            let tex2 = ctx.glow_mask(298);
            ctx.tile_batch.draw(tex2.as_ref(), Vector2::new(((tile_x * 16 - (screen_position.x as i32)) as f32) - ((draw_data.tile_width as f32) - 16.0_f32) / 2.0_f32, (tile_y * 16 - (screen_position.y as i32) + draw_data.tile_top) as f32) + screen_offset, Rectangle::new(((draw_data.tile_frame_x as i32) + draw_data.add_fr_x) as i32, ((draw_data.tile_frame_y as i32) + draw_data.add_fr_y) as i32, (draw_data.tile_width) as i32, (draw_data.tile_height) as i32), (Color::WHITE).into(), ZERO, 1.0_f32, draw_data.tile_sprite_effect);
        }
        if (draw_data.type_cache as i32) == 614 {
            let tex3 = ctx.glow_mask(299);
            ctx.tile_batch.draw(tex3.as_ref(), Vector2::new(((tile_x * 16 - (screen_position.x as i32)) as f32) - ((draw_data.tile_width as f32) - 16.0_f32) / 2.0_f32, (tile_y * 16 - (screen_position.y as i32) + draw_data.tile_top) as f32) + screen_offset, Rectangle::new(((draw_data.tile_frame_x as i32) + draw_data.add_fr_x) as i32, ((draw_data.tile_frame_y as i32) + draw_data.add_fr_y) as i32, (draw_data.tile_width) as i32, (draw_data.tile_height) as i32), (Color::WHITE).into(), ZERO, 1.0_f32, draw_data.tile_sprite_effect);
        }
        if (draw_data.type_cache as i32) == 593 {
            let tex4 = ctx.glow_mask(295);
            ctx.tile_batch.draw(tex4.as_ref(), Vector2::new(((tile_x * 16 - (screen_position.x as i32)) as f32) - ((draw_data.tile_width as f32) - 16.0_f32) / 2.0_f32, (tile_y * 16 - (screen_position.y as i32) + draw_data.tile_top) as f32) + screen_offset, Rectangle::new(((draw_data.tile_frame_x as i32) + draw_data.add_fr_x) as i32, ((draw_data.tile_frame_y as i32) + draw_data.add_fr_y) as i32, (draw_data.tile_width) as i32, (draw_data.tile_height) as i32), (Color::WHITE).into(), ZERO, 1.0_f32, draw_data.tile_sprite_effect);
        }
        if (draw_data.type_cache as i32) == 594 {
            let tex5 = ctx.glow_mask(296);
            ctx.tile_batch.draw(tex5.as_ref(), Vector2::new(((tile_x * 16 - (screen_position.x as i32)) as f32) - ((draw_data.tile_width as f32) - 16.0_f32) / 2.0_f32, (tile_y * 16 - (screen_position.y as i32) + draw_data.tile_top) as f32) + screen_offset, Rectangle::new(((draw_data.tile_frame_x as i32) + draw_data.add_fr_x) as i32, ((draw_data.tile_frame_y as i32) + draw_data.add_fr_y) as i32, (draw_data.tile_width) as i32, (draw_data.tile_height) as i32), (Color::WHITE).into(), ZERO, 1.0_f32, draw_data.tile_sprite_effect);
        }
        if (draw_data.type_cache as i32) == 215 && (draw_data.tile_frame_y as i32) < 36 {
            let mut num: i32 = 15;
            let mut color = Color::from_ints(255, 255, 255, 0);
            match (draw_data.tile_frame_x as i32) / 54 {
                5 => {
                    color = Color::from_floats((ctx.g.main.disco_r as f32) / 255.0_f32, (ctx.g.main.disco_g as f32) / 255.0_f32, (ctx.g.main.disco_b as f32) / 255.0_f32, 0.0_f32);
                }
                14 => {
                    color = Color::from_ints(50, 50, 100, 20);
                }
                15 => {
                    color = Color::from_ints(255, 255, 255, 200);
                }
                _ => {}
            }
            let tex6 = ctx.flame(num);
            ctx.tile_batch.draw(tex6.as_ref(), Vector2::new(((tile_x * 16 - (screen_position.x as i32)) as f32) - ((draw_data.tile_width as f32) - 16.0_f32) / 2.0_f32, (tile_y * 16 - (screen_position.y as i32) + draw_data.tile_top) as f32) + screen_offset, Rectangle::new(((draw_data.tile_frame_x as i32)) as i32, ((draw_data.tile_frame_y as i32) + draw_data.add_fr_y) as i32, (draw_data.tile_width) as i32, (draw_data.tile_height) as i32), (color).into(), ZERO, 1.0_f32, draw_data.tile_sprite_effect);
        }
        if (draw_data.type_cache as i32) == 85 {
            let mut graveyard_visual_intensity: f32 = unported!("Main.GraveyardVisualIntensity", 0.0_f32);
            if graveyard_visual_intensity > 0.0_f32 {
                let mut num2: u64 = ctx.state.tile_frame_seed ^ (((tile_x as i64) << 32) | (tile_y as u32 as i64)) as u64;
                let mut tile_flame_data = self.get_tile_flame_data(ctx, tile_x, tile_y, (draw_data.type_cache as i32), (draw_data.tile_frame_y as i32));
                if num2 == 0 {
                    num2 = tile_flame_data.flame_seed;
                }
                tile_flame_data.flame_seed = num2;
                let mut vector = Vector2::new(((tile_x * 16 - (screen_position.x as i32)) as f32) - ((draw_data.tile_width as f32) - 16.0_f32) / 2.0_f32, (tile_y * 16 - (screen_position.y as i32) + draw_data.tile_top) as f32) + screen_offset;
                let mut source_rectangle = Rectangle::new(((draw_data.tile_frame_x as i32) + draw_data.add_fr_x) as i32, ((draw_data.tile_frame_y as i32)) as i32, (draw_data.tile_width) as i32, (draw_data.tile_height) as i32);
                for i in 0..tile_flame_data.flame_count {
                    let mut color2 = tile_flame_data.flame_color * graveyard_visual_intensity;
                    let mut x: f32 = (utils_random_int(&mut tile_flame_data.flame_seed, tile_flame_data.flame_range_x_min, tile_flame_data.flame_range_x_max) as f32) * tile_flame_data.flame_range_mult_x;
                    let mut y: f32 = (utils_random_int(&mut tile_flame_data.flame_seed, tile_flame_data.flame_range_y_min, tile_flame_data.flame_range_y_max) as f32) * tile_flame_data.flame_range_mult_y;
                    let mut num3: f32 = 0.0_f32;
                    while num3 < 1.0_f32 {
                        ctx.tile_batch.draw(tile_flame_data.flame_texture.as_ref(), vector + Vector2::new(x, y) + vector2_rotated_by(Vector2::UNIT_X, num3 * (std::f32::consts::PI * 2.0_f32)) * 2.0_f32, source_rectangle, (color2).into(), ZERO, 1.0_f32, draw_data.tile_sprite_effect);
                        num3 += 0.25_f32;
                    }
                    ctx.tile_batch.draw(tile_flame_data.flame_texture.as_ref(), vector, source_rectangle, (Color::WHITE * graveyard_visual_intensity).into(), ZERO, 1.0_f32, draw_data.tile_sprite_effect);
                }
            }
        }
        if (draw_data.type_cache as i32) == 356 && ctx.g.main.sundial_cooldown == 0 {
            let mut value = ctx.glow_mask(325);
            let mut source_rectangle2 = Rectangle::new(((draw_data.tile_frame_x as i32)) as i32, ((draw_data.tile_frame_y as i32) + draw_data.add_fr_y) as i32, (draw_data.tile_width) as i32, (draw_data.tile_height) as i32);
            let mut color3 = Color::from_ints(100, 100, 100, 0);
            let mut num4: i32 = tile_x - (draw_data.tile_frame_x as i32) / 18;
            let mut num5: i32 = tile_y - (draw_data.tile_frame_y as i32) / 18;
            let mut seed: u64 = ctx.state.tile_frame_seed ^ (((num4 as i64) << 32) | (num5 as u32 as i64)) as u64;
            for j in 0..7 {
                let mut num6: f32 = (utils_random_int(&mut seed, -10, 11) as f32) * 0.15_f32;
                let mut num7: f32 = (utils_random_int(&mut seed, -10, 1) as f32) * 0.35_f32;
                ctx.tile_batch.draw(value.as_ref(), Vector2::new(((tile_x * 16 - (screen_position.x as i32)) as f32) - ((draw_data.tile_width as f32) - 16.0_f32) / 2.0_f32 + num6, ((tile_y * 16 - (screen_position.y as i32) + draw_data.tile_top) as f32) + num7) + screen_offset, source_rectangle2, (color3).into(), ZERO, 1.0_f32, draw_data.tile_sprite_effect);
            }
        }
        if (draw_data.type_cache as i32) == 663 && ctx.g.main.moondial_cooldown == 0 {
            let mut value2 = ctx.glow_mask(335);
            let mut source_rectangle3 = Rectangle::new(((draw_data.tile_frame_x as i32)) as i32, ((draw_data.tile_frame_y as i32) + draw_data.add_fr_y) as i32, (draw_data.tile_width) as i32, (draw_data.tile_height) as i32);
            source_rectangle3.y += 54 * main_moon_phase(ctx.g);
            ctx.tile_batch.draw(value2.as_ref(), Vector2::new(((tile_x * 16 - (screen_position.x as i32)) as f32) - ((draw_data.tile_width as f32) - 16.0_f32) / 2.0_f32, (tile_y * 16 - (screen_position.y as i32) + draw_data.tile_top) as f32) + screen_offset, source_rectangle3, (Color::WHITE * (((ctx.g.main.mouse_text_color as i32 as i32) as f32) / 255.0_f32)).into(), ZERO, 1.0_f32, draw_data.tile_sprite_effect);
        }
        if (draw_data.type_cache as i32) == 286 {
            let tex7 = ctx.glow_snail();
            ctx.tile_batch.draw(tex7.as_ref(), Vector2::new(((tile_x * 16 - (screen_position.x as i32)) as f32) - ((draw_data.tile_width as f32) - 16.0_f32) / 2.0_f32, (tile_y * 16 - (screen_position.y as i32) + draw_data.tile_top) as f32) + screen_offset, Rectangle::new(((draw_data.tile_frame_x as i32) + draw_data.add_fr_x) as i32, ((draw_data.tile_frame_y as i32) + draw_data.add_fr_y) as i32, (draw_data.tile_width) as i32, (draw_data.tile_height) as i32), (Color::from_ints(75, 100, 255, 0)).into(), ZERO, 1.0_f32, draw_data.tile_sprite_effect);
        }
        if (draw_data.type_cache as i32) == 582 {
            let tex8 = ctx.glow_mask(293);
            ctx.tile_batch.draw(tex8.as_ref(), Vector2::new(((tile_x * 16 - (screen_position.x as i32)) as f32) - ((draw_data.tile_width as f32) - 16.0_f32) / 2.0_f32, (tile_y * 16 - (screen_position.y as i32) + draw_data.tile_top) as f32) + screen_offset, Rectangle::new(((draw_data.tile_frame_x as i32) + draw_data.add_fr_x) as i32, ((draw_data.tile_frame_y as i32) + draw_data.add_fr_y) as i32, (draw_data.tile_width) as i32, (draw_data.tile_height) as i32), (Color::from_ints(200, 100, 100, 0)).into(), ZERO, 1.0_f32, draw_data.tile_sprite_effect);
        }
        if (draw_data.type_cache as i32) == 391 {
            let tex9 = ctx.glow_mask(131);
            ctx.tile_batch.draw(tex9.as_ref(), Vector2::new(((tile_x * 16 - (screen_position.x as i32)) as f32) - ((draw_data.tile_width as f32) - 16.0_f32) / 2.0_f32, (tile_y * 16 - (screen_position.y as i32) + draw_data.tile_top) as f32) + screen_offset, Rectangle::new(((draw_data.tile_frame_x as i32) + draw_data.add_fr_x) as i32, ((draw_data.tile_frame_y as i32) + draw_data.add_fr_y) as i32, (draw_data.tile_width) as i32, (draw_data.tile_height) as i32), (Color::from_ints(250, 250, 250, 200)).into(), ZERO, 1.0_f32, draw_data.tile_sprite_effect);
        }
        if (draw_data.type_cache as i32) == 619 {
            let tex10 = ctx.glow_mask(300);
            ctx.tile_batch.draw(tex10.as_ref(), Vector2::new(((tile_x * 16 - (screen_position.x as i32)) as f32) - ((draw_data.tile_width as f32) - 16.0_f32) / 2.0_f32, (tile_y * 16 - (screen_position.y as i32) + draw_data.tile_top) as f32) + screen_offset, Rectangle::new(((draw_data.tile_frame_x as i32) + draw_data.add_fr_x) as i32, ((draw_data.tile_frame_y as i32) + draw_data.add_fr_y) as i32, (draw_data.tile_width) as i32, (draw_data.tile_height) as i32), (Color::from_ints(75, 100, 255, 0)).into(), ZERO, 1.0_f32, draw_data.tile_sprite_effect);
        }
        if (draw_data.type_cache as i32) == 270 {
            let tex11 = ctx.firefly_jar();
            ctx.tile_batch.draw(tex11.as_ref(), Vector2::new(((tile_x * 16 - (screen_position.x as i32)) as f32) - ((draw_data.tile_width as f32) - 16.0_f32) / 2.0_f32, (tile_y * 16 - (screen_position.y as i32) + draw_data.tile_top) as f32) + screen_offset, Rectangle::new(((draw_data.tile_frame_x as i32) + draw_data.add_fr_x) as i32, ((draw_data.tile_frame_y as i32)) as i32, (draw_data.tile_width) as i32, (draw_data.tile_height) as i32), (Color::from_ints(200, 200, 200, 0)).into(), ZERO, 1.0_f32, draw_data.tile_sprite_effect);
        }
        if (draw_data.type_cache as i32) == 271 {
            let tex12 = ctx.lightningbug_jar();
            ctx.tile_batch.draw(tex12.as_ref(), Vector2::new(((tile_x * 16 - (screen_position.x as i32)) as f32) - ((draw_data.tile_width as f32) - 16.0_f32) / 2.0_f32, (tile_y * 16 - (screen_position.y as i32) + draw_data.tile_top) as f32) + screen_offset, Rectangle::new(((draw_data.tile_frame_x as i32) + draw_data.add_fr_x) as i32, ((draw_data.tile_frame_y as i32)) as i32, (draw_data.tile_width) as i32, (draw_data.tile_height) as i32), (Color::from_ints(200, 200, 200, 0)).into(), ZERO, 1.0_f32, draw_data.tile_sprite_effect);
        }
        if (draw_data.type_cache as i32) == 581 {
            let tex13 = ctx.glow_mask(291);
            ctx.tile_batch.draw(tex13.as_ref(), Vector2::new(((tile_x * 16 - (screen_position.x as i32)) as f32) - ((draw_data.tile_width as f32) - 16.0_f32) / 2.0_f32, (tile_y * 16 - (screen_position.y as i32) + draw_data.tile_top) as f32) + screen_offset, Rectangle::new(((draw_data.tile_frame_x as i32) + draw_data.add_fr_x) as i32, ((draw_data.tile_frame_y as i32)) as i32, (draw_data.tile_width) as i32, (draw_data.tile_height) as i32), (Color::from_ints(200, 200, 200, 0)).into(), ZERO, 1.0_f32, draw_data.tile_sprite_effect);
        }
        if (draw_data.type_cache as i32) == 316 || (draw_data.type_cache as i32) == 317 || (draw_data.type_cache as i32) == 318 {
            let tex14 = ctx.jellyfish_bowl((draw_data.type_cache as i32) - 316);
            ctx.tile_batch.draw(tex14.as_ref(), Vector2::new(((tile_x * 16 - (screen_position.x as i32)) as f32) - ((draw_data.tile_width as f32) - 16.0_f32) / 2.0_f32, (tile_y * 16 - (screen_position.y as i32) + draw_data.tile_top) as f32) + screen_offset, Rectangle::new(((draw_data.tile_frame_x as i32) + draw_data.add_fr_x) as i32, ((draw_data.tile_frame_y as i32) + draw_data.add_fr_y) as i32, (draw_data.tile_width) as i32, (draw_data.tile_height) as i32), (Color::from_ints(200, 200, 200, 0)).into(), ZERO, 1.0_f32, draw_data.tile_sprite_effect);
        }
        if (draw_data.type_cache as i32) == 149 && (draw_data.tile_frame_x as i32) < 54 {
            let tex15 = ctx.xmas_light();
            ctx.tile_batch.draw(tex15.as_ref(), Vector2::new(((tile_x * 16 - (screen_position.x as i32)) as f32) - ((draw_data.tile_width as f32) - 16.0_f32) / 2.0_f32, (tile_y * 16 - (screen_position.y as i32) + draw_data.tile_top) as f32) + screen_offset, Rectangle::new(((draw_data.tile_frame_x as i32)) as i32, ((draw_data.tile_frame_y as i32)) as i32, (draw_data.tile_width) as i32, (draw_data.tile_height) as i32), (Color::from_ints(200, 200, 200, 0)).into(), ZERO, 1.0_f32, draw_data.tile_sprite_effect);
        }
        if (draw_data.type_cache as i32) == 300 || (draw_data.type_cache as i32) == 302 || (draw_data.type_cache as i32) == 303 || (draw_data.type_cache as i32) == 306 {
            let mut num8: i32 = 9;
            if (draw_data.type_cache as i32) == 302 {
                num8 = 10;
            }
            if (draw_data.type_cache as i32) == 303 {
                num8 = 11;
            }
            if (draw_data.type_cache as i32) == 306 {
                num8 = 12;
            }
            let tex16 = ctx.flame(num8);
            ctx.tile_batch.draw(tex16.as_ref(), Vector2::new(((tile_x * 16 - (screen_position.x as i32)) as f32) - ((draw_data.tile_width as f32) - 16.0_f32) / 2.0_f32, (tile_y * 16 - (screen_position.y as i32) + draw_data.tile_top) as f32) + screen_offset, Rectangle::new(((draw_data.tile_frame_x as i32)) as i32, ((draw_data.tile_frame_y as i32) + draw_data.add_fr_y) as i32, (draw_data.tile_width) as i32, (draw_data.tile_height) as i32), (Color::from_ints(200, 200, 200, 0)).into(), ZERO, 1.0_f32, draw_data.tile_sprite_effect);
        } else if ctx.g.main.tile_flame[((draw_data.type_cache as i32)) as usize] {
            let mut seed2: u64 = ctx.state.tile_frame_seed ^ (((tile_x as i64) << 32) | (tile_y as u32 as i64)) as u64;
            let mut type_cache: i32 = (draw_data.type_cache as i32);
            let mut num9: i32 = 0;
            match type_cache {
                4 => {
                    num9 = 0;
                }
                33 | 174 => {
                    num9 = 1;
                }
                100 | 173 => {
                    num9 = 2;
                }
                34 => {
                    num9 = 3;
                }
                93 => {
                    num9 = 4;
                }
                49 => {
                    num9 = 5;
                }
                372 => {
                    num9 = 16;
                }
                646 => {
                    num9 = 17;
                }
                98 => {
                    num9 = 6;
                }
                35 => {
                    num9 = 7;
                }
                42 => {
                    num9 = 13;
                }
                _ => {}
            }
            match num9 {
                7 => {
                    for num92 in 0..4 {
                        let mut num93: f32 = (utils_random_int(&mut seed2, -10, 11) as f32) * 0.15_f32;
                        let mut num94: f32 = (utils_random_int(&mut seed2, -10, 10) as f32) * 0.15_f32;
                        num93 = 0.0_f32;
                        num94 = 0.0_f32;
                        let tex17 = ctx.flame(num9);
                        ctx.tile_batch.draw(tex17.as_ref(), Vector2::new(((tile_x * 16 - (screen_position.x as i32)) as f32) - ((draw_data.tile_width as f32) - 16.0_f32) / 2.0_f32 + num93, ((tile_y * 16 - (screen_position.y as i32) + draw_data.tile_top) as f32) + num94) + screen_offset, Rectangle::new(((draw_data.tile_frame_x as i32)) as i32, ((draw_data.tile_frame_y as i32)) as i32, (draw_data.tile_width) as i32, (draw_data.tile_height) as i32), (Color::from_ints(50, 50, 50, 0)).into(), ZERO, 1.0_f32, draw_data.tile_sprite_effect);
                    }
                }
                1 => {
                    let mut num67: i32 = ctx.g.main.tile.get(tile_x, tile_y).frame_y as i32 / 22;
                    let mut flag3: bool = num67 >= 44;
                    match num67 {
                        5 | 6 | 7 | 10 => {
                            for num86 in 0..7 {
                                let mut num87: f32 = (utils_random_int(&mut seed2, -10, 11) as f32) * 0.075_f32;
                                let mut num88: f32 = (utils_random_int(&mut seed2, -10, 11) as f32) * 0.075_f32;
                                let tex18 = ctx.flame(num9);
                                ctx.tile_batch.draw(tex18.as_ref(), Vector2::new(((tile_x * 16 - (screen_position.x as i32)) as f32) - ((draw_data.tile_width as f32) - 16.0_f32) / 2.0_f32 + num87, ((tile_y * 16 - (screen_position.y as i32) + draw_data.tile_top) as f32) + num88) + screen_offset, Rectangle::new(((draw_data.tile_frame_x as i32)) as i32, ((draw_data.tile_frame_y as i32)) as i32, (draw_data.tile_width) as i32, (draw_data.tile_height) as i32), (Color::from_ints(50, 50, 50, 0)).into(), ZERO, 1.0_f32, draw_data.tile_sprite_effect);
                            }
                        }
                        8 => {
                            for num74 in 0..7 {
                                let mut num75: f32 = (utils_random_int(&mut seed2, -10, 11) as f32) * 0.3_f32;
                                let mut num76: f32 = (utils_random_int(&mut seed2, -10, 11) as f32) * 0.3_f32;
                                let tex19 = ctx.flame(num9);
                                ctx.tile_batch.draw(tex19.as_ref(), Vector2::new(((tile_x * 16 - (screen_position.x as i32)) as f32) - ((draw_data.tile_width as f32) - 16.0_f32) / 2.0_f32 + num75, ((tile_y * 16 - (screen_position.y as i32) + draw_data.tile_top) as f32) + num76) + screen_offset, Rectangle::new(((draw_data.tile_frame_x as i32)) as i32, ((draw_data.tile_frame_y as i32)) as i32, (draw_data.tile_width) as i32, (draw_data.tile_height) as i32), (Color::from_ints(50, 50, 50, 0)).into(), ZERO, 1.0_f32, draw_data.tile_sprite_effect);
                            }
                        }
                        12 => {
                            for num77 in 0..7 {
                                let mut num78: f32 = (utils_random_int(&mut seed2, -10, 11) as f32) * 0.1_f32;
                                let mut num79: f32 = (utils_random_int(&mut seed2, -10, 1) as f32) * 0.15_f32;
                                let tex20 = ctx.flame(num9);
                                ctx.tile_batch.draw(tex20.as_ref(), Vector2::new(((tile_x * 16 - (screen_position.x as i32)) as f32) - ((draw_data.tile_width as f32) - 16.0_f32) / 2.0_f32 + num78, ((tile_y * 16 - (screen_position.y as i32) + draw_data.tile_top) as f32) + num79) + screen_offset, Rectangle::new(((draw_data.tile_frame_x as i32)) as i32, ((draw_data.tile_frame_y as i32)) as i32, (draw_data.tile_width) as i32, (draw_data.tile_height) as i32), (Color::from_ints(50, 50, 50, 0)).into(), ZERO, 1.0_f32, draw_data.tile_sprite_effect);
                            }
                        }
                        14 => {
                            for num83 in 0..8 {
                                let mut num84: f32 = (utils_random_int(&mut seed2, -10, 11) as f32) * 0.1_f32;
                                let mut num85: f32 = (utils_random_int(&mut seed2, -10, 11) as f32) * 0.1_f32;
                                let tex21 = ctx.flame(num9);
                                ctx.tile_batch.draw(tex21.as_ref(), Vector2::new(((tile_x * 16 - (screen_position.x as i32)) as f32) - ((draw_data.tile_width as f32) - 16.0_f32) / 2.0_f32 + num84, ((tile_y * 16 - (screen_position.y as i32) + draw_data.tile_top) as f32) + num85) + screen_offset, Rectangle::new(((draw_data.tile_frame_x as i32)) as i32, ((draw_data.tile_frame_y as i32)) as i32, (draw_data.tile_width) as i32, (draw_data.tile_height) as i32), (Color::from_ints(75, 75, 75, 0)).into(), ZERO, 1.0_f32, draw_data.tile_sprite_effect);
                            }
                        }
                        16 => {
                            for num80 in 0..4 {
                                let mut num81: f32 = (utils_random_int(&mut seed2, -10, 11) as f32) * 0.15_f32;
                                let mut num82: f32 = (utils_random_int(&mut seed2, -10, 11) as f32) * 0.15_f32;
                                let tex22 = ctx.flame(num9);
                                ctx.tile_batch.draw(tex22.as_ref(), Vector2::new(((tile_x * 16 - (screen_position.x as i32)) as f32) - ((draw_data.tile_width as f32) - 16.0_f32) / 2.0_f32 + num81, ((tile_y * 16 - (screen_position.y as i32) + draw_data.tile_top) as f32) + num82) + screen_offset, Rectangle::new(((draw_data.tile_frame_x as i32)) as i32, ((draw_data.tile_frame_y as i32)) as i32, (draw_data.tile_width) as i32, (draw_data.tile_height) as i32), (Color::from_ints(75, 75, 75, 0)).into(), ZERO, 1.0_f32, draw_data.tile_sprite_effect);
                            }
                        }
                        27 | 28 => {
                            let tex23 = ctx.flame(num9);
                            ctx.tile_batch.draw(tex23.as_ref(), Vector2::new(((tile_x * 16 - (screen_position.x as i32)) as f32) - ((draw_data.tile_width as f32) - 16.0_f32) / 2.0_f32, (tile_y * 16 - (screen_position.y as i32) + draw_data.tile_top) as f32) + screen_offset, Rectangle::new(((draw_data.tile_frame_x as i32)) as i32, ((draw_data.tile_frame_y as i32)) as i32, (draw_data.tile_width) as i32, (draw_data.tile_height) as i32), (Color::from_ints(75, 75, 75, 0)).into(), ZERO, 1.0_f32, draw_data.tile_sprite_effect);
                        }
                        43 => {
                            let mut tile_flame_data7 = self.get_tile_flame_data(ctx, tile_x, tile_y, (draw_data.type_cache as i32), (draw_data.tile_frame_y as i32));
                            if seed2 == 0 {
                                seed2 = tile_flame_data7.flame_seed;
                            }
                            tile_flame_data7.flame_seed = seed2;
                            for num89 in 0..tile_flame_data7.flame_count {
                                let mut num90: f32 = (utils_random_int(&mut tile_flame_data7.flame_seed, tile_flame_data7.flame_range_x_min, tile_flame_data7.flame_range_x_max) as f32) * tile_flame_data7.flame_range_mult_x;
                                let mut num91: f32 = (utils_random_int(&mut tile_flame_data7.flame_seed, tile_flame_data7.flame_range_y_min, tile_flame_data7.flame_range_y_max) as f32) * tile_flame_data7.flame_range_mult_y;
                                ctx.tile_batch.draw(tile_flame_data7.flame_texture.as_ref(), Vector2::new(((tile_x * 16 - (screen_position.x as i32)) as f32) - ((draw_data.tile_width as f32) - 16.0_f32) / 2.0_f32 + num90, ((tile_y * 16 - (screen_position.y as i32) + draw_data.tile_top) as f32) + num91) + screen_offset, Rectangle::new(((draw_data.tile_frame_x as i32)) as i32, ((draw_data.tile_frame_y as i32)) as i32, (draw_data.tile_width) as i32, (draw_data.tile_height) as i32), (tile_flame_data7.flame_color).into(), ZERO, 1.0_f32, draw_data.tile_sprite_effect);
                            }
                        }
                        _ => {
                            if flag3 {
                                let mut tile_flame_data6 = self.get_tile_flame_data(ctx, tile_x, tile_y, (draw_data.type_cache as i32), (draw_data.tile_frame_y as i32));
                                if seed2 == 0 {
                                    seed2 = tile_flame_data6.flame_seed;
                                }
                                tile_flame_data6.flame_seed = seed2;
                                for num68 in 0..tile_flame_data6.flame_count {
                                    let mut num69: f32 = (utils_random_int(&mut tile_flame_data6.flame_seed, tile_flame_data6.flame_range_x_min, tile_flame_data6.flame_range_x_max) as f32) * tile_flame_data6.flame_range_mult_x;
                                    let mut num70: f32 = (utils_random_int(&mut tile_flame_data6.flame_seed, tile_flame_data6.flame_range_y_min, tile_flame_data6.flame_range_y_max) as f32) * tile_flame_data6.flame_range_mult_y;
                                    ctx.tile_batch.draw(tile_flame_data6.flame_texture.as_ref(), Vector2::new(((tile_x * 16 - (screen_position.x as i32)) as f32) - ((draw_data.tile_width as f32) - 16.0_f32) / 2.0_f32 + num69, ((tile_y * 16 - (screen_position.y as i32) + draw_data.tile_top) as f32) + num70) + screen_offset, Rectangle::new(((draw_data.tile_frame_x as i32)) as i32, ((draw_data.tile_frame_y as i32)) as i32, (draw_data.tile_width) as i32, (draw_data.tile_height) as i32), (tile_flame_data6.flame_color).into(), ZERO, 1.0_f32, draw_data.tile_sprite_effect);
                                }
                            } else {
                                for num71 in 0..7 {
                                    let mut num72: f32 = (utils_random_int(&mut seed2, -10, 11) as f32) * 0.15_f32;
                                    let mut num73: f32 = (utils_random_int(&mut seed2, -10, 1) as f32) * 0.35_f32;
                                    let tex24 = ctx.flame(num9);
                                    ctx.tile_batch.draw(tex24.as_ref(), Vector2::new(((tile_x * 16 - (screen_position.x as i32)) as f32) - ((draw_data.tile_width as f32) - 16.0_f32) / 2.0_f32 + num72, ((tile_y * 16 - (screen_position.y as i32) + draw_data.tile_top) as f32) + num73) + screen_offset, Rectangle::new(((draw_data.tile_frame_x as i32)) as i32, ((draw_data.tile_frame_y as i32)) as i32, (draw_data.tile_width) as i32, (draw_data.tile_height) as i32), (Color::from_ints(100, 100, 100, 0)).into(), ZERO, 1.0_f32, draw_data.tile_sprite_effect);
                                }
                            }
                        }
                    }
                }
                2 => {
                    let mut num95: i32 = ctx.g.main.tile.get(tile_x, tile_y).frame_y as i32 / 36;
                    let mut flag4: bool = num95 >= 45;
                    match num95 {
                        3 => {
                            for num114 in 0..3 {
                                let mut num115: f32 = (utils_random_int(&mut seed2, -10, 11) as f32) * 0.05_f32;
                                let mut num116: f32 = (utils_random_int(&mut seed2, -10, 11) as f32) * 0.15_f32;
                                let tex25 = ctx.flame(num9);
                                ctx.tile_batch.draw(tex25.as_ref(), Vector2::new(((tile_x * 16 - (screen_position.x as i32)) as f32) - ((draw_data.tile_width as f32) - 16.0_f32) / 2.0_f32 + num115, ((tile_y * 16 - (screen_position.y as i32) + draw_data.tile_top) as f32) + num116) + screen_offset, Rectangle::new(((draw_data.tile_frame_x as i32)) as i32, ((draw_data.tile_frame_y as i32)) as i32, (draw_data.tile_width) as i32, (draw_data.tile_height) as i32), (Color::from_ints(50, 50, 50, 0)).into(), ZERO, 1.0_f32, draw_data.tile_sprite_effect);
                            }
                        }
                        6 => {
                            for num102 in 0..5 {
                                let mut num103: f32 = (utils_random_int(&mut seed2, -10, 11) as f32) * 0.15_f32;
                                let mut num104: f32 = (utils_random_int(&mut seed2, -10, 11) as f32) * 0.15_f32;
                                let tex26 = ctx.flame(num9);
                                ctx.tile_batch.draw(tex26.as_ref(), Vector2::new(((tile_x * 16 - (screen_position.x as i32)) as f32) - ((draw_data.tile_width as f32) - 16.0_f32) / 2.0_f32 + num103, ((tile_y * 16 - (screen_position.y as i32) + draw_data.tile_top) as f32) + num104) + screen_offset, Rectangle::new(((draw_data.tile_frame_x as i32)) as i32, ((draw_data.tile_frame_y as i32)) as i32, (draw_data.tile_width) as i32, (draw_data.tile_height) as i32), (Color::from_ints(75, 75, 75, 0)).into(), ZERO, 1.0_f32, draw_data.tile_sprite_effect);
                            }
                        }
                        9 => {
                            for num105 in 0..7 {
                                let mut num106: f32 = (utils_random_int(&mut seed2, -10, 11) as f32) * 0.3_f32;
                                let mut num107: f32 = (utils_random_int(&mut seed2, -10, 11) as f32) * 0.3_f32;
                                let tex27 = ctx.flame(num9);
                                ctx.tile_batch.draw(tex27.as_ref(), Vector2::new(((tile_x * 16 - (screen_position.x as i32)) as f32) - ((draw_data.tile_width as f32) - 16.0_f32) / 2.0_f32 + num106, ((tile_y * 16 - (screen_position.y as i32) + draw_data.tile_top) as f32) + num107) + screen_offset, Rectangle::new(((draw_data.tile_frame_x as i32)) as i32, ((draw_data.tile_frame_y as i32)) as i32, (draw_data.tile_width) as i32, (draw_data.tile_height) as i32), (Color::from_ints(100, 100, 100, 0)).into(), ZERO, 1.0_f32, draw_data.tile_sprite_effect);
                            }
                        }
                        11 => {
                            for num111 in 0..7 {
                                let mut num112: f32 = (utils_random_int(&mut seed2, -10, 11) as f32) * 0.1_f32;
                                let mut num113: f32 = (utils_random_int(&mut seed2, -10, 1) as f32) * 0.15_f32;
                                let tex28 = ctx.flame(num9);
                                ctx.tile_batch.draw(tex28.as_ref(), Vector2::new(((tile_x * 16 - (screen_position.x as i32)) as f32) - ((draw_data.tile_width as f32) - 16.0_f32) / 2.0_f32 + num112, ((tile_y * 16 - (screen_position.y as i32) + draw_data.tile_top) as f32) + num113) + screen_offset, Rectangle::new(((draw_data.tile_frame_x as i32)) as i32, ((draw_data.tile_frame_y as i32)) as i32, (draw_data.tile_width) as i32, (draw_data.tile_height) as i32), (Color::from_ints(50, 50, 50, 0)).into(), ZERO, 1.0_f32, draw_data.tile_sprite_effect);
                            }
                        }
                        13 => {
                            for num108 in 0..8 {
                                let mut num109: f32 = (utils_random_int(&mut seed2, -10, 11) as f32) * 0.1_f32;
                                let mut num110: f32 = (utils_random_int(&mut seed2, -10, 11) as f32) * 0.1_f32;
                                let tex29 = ctx.flame(num9);
                                ctx.tile_batch.draw(tex29.as_ref(), Vector2::new(((tile_x * 16 - (screen_position.x as i32)) as f32) - ((draw_data.tile_width as f32) - 16.0_f32) / 2.0_f32 + num109, ((tile_y * 16 - (screen_position.y as i32) + draw_data.tile_top) as f32) + num110) + screen_offset, Rectangle::new(((draw_data.tile_frame_x as i32)) as i32, ((draw_data.tile_frame_y as i32)) as i32, (draw_data.tile_width) as i32, (draw_data.tile_height) as i32), (Color::from_ints(75, 75, 75, 0)).into(), ZERO, 1.0_f32, draw_data.tile_sprite_effect);
                            }
                        }
                        28 | 29 => {
                            let tex30 = ctx.flame(num9);
                            ctx.tile_batch.draw(tex30.as_ref(), Vector2::new(((tile_x * 16 - (screen_position.x as i32)) as f32) - ((draw_data.tile_width as f32) - 16.0_f32) / 2.0_f32, (tile_y * 16 - (screen_position.y as i32) + draw_data.tile_top) as f32) + screen_offset, Rectangle::new(((draw_data.tile_frame_x as i32)) as i32, ((draw_data.tile_frame_y as i32)) as i32, (draw_data.tile_width) as i32, (draw_data.tile_height) as i32), (Color::from_ints(75, 75, 75, 0)).into(), ZERO, 1.0_f32, draw_data.tile_sprite_effect);
                        }
                        44 => {
                            let mut tile_flame_data9 = self.get_tile_flame_data(ctx, tile_x, tile_y, (draw_data.type_cache as i32), (draw_data.tile_frame_y as i32));
                            if seed2 == 0 {
                                seed2 = tile_flame_data9.flame_seed;
                            }
                            tile_flame_data9.flame_seed = seed2;
                            for num117 in 0..tile_flame_data9.flame_count {
                                let mut num118: f32 = (utils_random_int(&mut tile_flame_data9.flame_seed, tile_flame_data9.flame_range_x_min, tile_flame_data9.flame_range_x_max) as f32) * tile_flame_data9.flame_range_mult_x;
                                let mut num119: f32 = (utils_random_int(&mut tile_flame_data9.flame_seed, tile_flame_data9.flame_range_y_min, tile_flame_data9.flame_range_y_max) as f32) * tile_flame_data9.flame_range_mult_y;
                                ctx.tile_batch.draw(tile_flame_data9.flame_texture.as_ref(), Vector2::new(((tile_x * 16 - (screen_position.x as i32)) as f32) - ((draw_data.tile_width as f32) - 16.0_f32) / 2.0_f32 + num118, ((tile_y * 16 - (screen_position.y as i32) + draw_data.tile_top) as f32) + num119) + screen_offset, Rectangle::new(((draw_data.tile_frame_x as i32) + draw_data.add_fr_x) as i32, ((draw_data.tile_frame_y as i32) + draw_data.add_fr_y) as i32, (draw_data.tile_width) as i32, (draw_data.tile_height) as i32), (tile_flame_data9.flame_color).into(), ZERO, 1.0_f32, draw_data.tile_sprite_effect);
                            }
                        }
                        _ => {
                            if flag4 {
                                let mut tile_flame_data8 = self.get_tile_flame_data(ctx, tile_x, tile_y, (draw_data.type_cache as i32), (draw_data.tile_frame_y as i32));
                                if seed2 == 0 {
                                    seed2 = tile_flame_data8.flame_seed;
                                }
                                tile_flame_data8.flame_seed = seed2;
                                for num96 in 0..tile_flame_data8.flame_count {
                                    let mut num97: f32 = (utils_random_int(&mut tile_flame_data8.flame_seed, tile_flame_data8.flame_range_x_min, tile_flame_data8.flame_range_x_max) as f32) * tile_flame_data8.flame_range_mult_x;
                                    let mut num98: f32 = (utils_random_int(&mut tile_flame_data8.flame_seed, tile_flame_data8.flame_range_y_min, tile_flame_data8.flame_range_y_max) as f32) * tile_flame_data8.flame_range_mult_y;
                                    ctx.tile_batch.draw(tile_flame_data8.flame_texture.as_ref(), Vector2::new(((tile_x * 16 - (screen_position.x as i32)) as f32) - ((draw_data.tile_width as f32) - 16.0_f32) / 2.0_f32 + num97, ((tile_y * 16 - (screen_position.y as i32) + draw_data.tile_top) as f32) + num98) + screen_offset, Rectangle::new(((draw_data.tile_frame_x as i32) + draw_data.add_fr_x) as i32, ((draw_data.tile_frame_y as i32) + draw_data.add_fr_y) as i32, (draw_data.tile_width) as i32, (draw_data.tile_height) as i32), (tile_flame_data8.flame_color).into(), ZERO, 1.0_f32, draw_data.tile_sprite_effect);
                                }
                            } else {
                                for num99 in 0..7 {
                                    let mut num100: f32 = (utils_random_int(&mut seed2, -10, 11) as f32) * 0.15_f32;
                                    let mut num101: f32 = (utils_random_int(&mut seed2, -10, 1) as f32) * 0.35_f32;
                                    let tex31 = ctx.flame(num9);
                                    ctx.tile_batch.draw(tex31.as_ref(), Vector2::new(((tile_x * 16 - (screen_position.x as i32)) as f32) - ((draw_data.tile_width as f32) - 16.0_f32) / 2.0_f32 + num100, ((tile_y * 16 - (screen_position.y as i32) + draw_data.tile_top) as f32) + num101) + screen_offset, Rectangle::new(((draw_data.tile_frame_x as i32) + draw_data.add_fr_x) as i32, ((draw_data.tile_frame_y as i32) + draw_data.add_fr_y) as i32, (draw_data.tile_width) as i32, (draw_data.tile_height) as i32), (Color::from_ints(100, 100, 100, 0)).into(), ZERO, 1.0_f32, draw_data.tile_sprite_effect);
                                }
                            }
                        }
                    }
                }
                3 => {
                    let mut num12: i32 = ctx.g.main.tile.get(tile_x, tile_y).frame_y as i32 / 54;
                    if ctx.g.main.tile.get(tile_x, tile_y).frame_x as i32 >= 108 {
                        num12 += 37 * (ctx.g.main.tile.get(tile_x, tile_y).frame_x as i32 / 108);
                    }
                    let mut flag: bool = num12 >= 51;
                    match num12 {
                        8 => {
                            for n in 0..7 {
                                let mut num17: f32 = (utils_random_int(&mut seed2, -10, 11) as f32) * 0.075_f32;
                                let mut num18: f32 = (utils_random_int(&mut seed2, -10, 11) as f32) * 0.075_f32;
                                let tex32 = ctx.flame(num9);
                                ctx.tile_batch.draw(tex32.as_ref(), Vector2::new(((tile_x * 16 - (screen_position.x as i32)) as f32) - ((draw_data.tile_width as f32) - 16.0_f32) / 2.0_f32 + num17, ((tile_y * 16 - (screen_position.y as i32) + draw_data.tile_top) as f32) + num18) + screen_offset, Rectangle::new(((draw_data.tile_frame_x as i32)) as i32, ((draw_data.tile_frame_y as i32)) as i32, (draw_data.tile_width) as i32, (draw_data.tile_height) as i32), (Color::from_ints(50, 50, 50, 0)).into(), ZERO, 1.0_f32, draw_data.tile_sprite_effect);
                            }
                        }
                        9 => {
                            for num25 in 0..3 {
                                let mut num26: f32 = (utils_random_int(&mut seed2, -10, 11) as f32) * 0.05_f32;
                                let mut num27: f32 = (utils_random_int(&mut seed2, -10, 11) as f32) * 0.15_f32;
                                let tex33 = ctx.flame(num9);
                                ctx.tile_batch.draw(tex33.as_ref(), Vector2::new(((tile_x * 16 - (screen_position.x as i32)) as f32) - ((draw_data.tile_width as f32) - 16.0_f32) / 2.0_f32 + num26, ((tile_y * 16 - (screen_position.y as i32) + draw_data.tile_top) as f32) + num27) + screen_offset, Rectangle::new(((draw_data.tile_frame_x as i32)) as i32, ((draw_data.tile_frame_y as i32)) as i32, (draw_data.tile_width) as i32, (draw_data.tile_height) as i32), (Color::from_ints(50, 50, 50, 0)).into(), ZERO, 1.0_f32, draw_data.tile_sprite_effect);
                            }
                        }
                        11 => {
                            for num22 in 0..7 {
                                let mut num23: f32 = (utils_random_int(&mut seed2, -10, 11) as f32) * 0.3_f32;
                                let mut num24: f32 = (utils_random_int(&mut seed2, -10, 11) as f32) * 0.3_f32;
                                let tex34 = ctx.flame(num9);
                                ctx.tile_batch.draw(tex34.as_ref(), Vector2::new(((tile_x * 16 - (screen_position.x as i32)) as f32) - ((draw_data.tile_width as f32) - 16.0_f32) / 2.0_f32 + num23, ((tile_y * 16 - (screen_position.y as i32) + draw_data.tile_top) as f32) + num24) + screen_offset, Rectangle::new(((draw_data.tile_frame_x as i32)) as i32, ((draw_data.tile_frame_y as i32)) as i32, (draw_data.tile_width) as i32, (draw_data.tile_height) as i32), (Color::from_ints(50, 50, 50, 0)).into(), ZERO, 1.0_f32, draw_data.tile_sprite_effect);
                            }
                        }
                        15 => {
                            for num34 in 0..7 {
                                let mut num35: f32 = (utils_random_int(&mut seed2, -10, 11) as f32) * 0.1_f32;
                                let mut num36: f32 = (utils_random_int(&mut seed2, -10, 1) as f32) * 0.15_f32;
                                let tex35 = ctx.flame(num9);
                                ctx.tile_batch.draw(tex35.as_ref(), Vector2::new(((tile_x * 16 - (screen_position.x as i32)) as f32) - ((draw_data.tile_width as f32) - 16.0_f32) / 2.0_f32 + num35, ((tile_y * 16 - (screen_position.y as i32) + draw_data.tile_top) as f32) + num36) + screen_offset, Rectangle::new(((draw_data.tile_frame_x as i32)) as i32, ((draw_data.tile_frame_y as i32)) as i32, (draw_data.tile_width) as i32, (draw_data.tile_height) as i32), (Color::from_ints(50, 50, 50, 0)).into(), ZERO, 1.0_f32, draw_data.tile_sprite_effect);
                            }
                        }
                        17 | 20 => {
                            for num28 in 0..7 {
                                let mut num29: f32 = (utils_random_int(&mut seed2, -10, 11) as f32) * 0.075_f32;
                                let mut num30: f32 = (utils_random_int(&mut seed2, -10, 11) as f32) * 0.075_f32;
                                let tex36 = ctx.flame(num9);
                                ctx.tile_batch.draw(tex36.as_ref(), Vector2::new(((tile_x * 16 - (screen_position.x as i32)) as f32) - ((draw_data.tile_width as f32) - 16.0_f32) / 2.0_f32 + num29, ((tile_y * 16 - (screen_position.y as i32) + draw_data.tile_top) as f32) + num30) + screen_offset, Rectangle::new(((draw_data.tile_frame_x as i32)) as i32, ((draw_data.tile_frame_y as i32)) as i32, (draw_data.tile_width) as i32, (draw_data.tile_height) as i32), (Color::from_ints(50, 50, 50, 0)).into(), ZERO, 1.0_f32, draw_data.tile_sprite_effect);
                            }
                        }
                        18 => {
                            for num19 in 0..8 {
                                let mut num20: f32 = (utils_random_int(&mut seed2, -10, 11) as f32) * 0.1_f32;
                                let mut num21: f32 = (utils_random_int(&mut seed2, -10, 11) as f32) * 0.1_f32;
                                let tex37 = ctx.flame(num9);
                                ctx.tile_batch.draw(tex37.as_ref(), Vector2::new(((tile_x * 16 - (screen_position.x as i32)) as f32) - ((draw_data.tile_width as f32) - 16.0_f32) / 2.0_f32 + num20, ((tile_y * 16 - (screen_position.y as i32) + draw_data.tile_top) as f32) + num21) + screen_offset, Rectangle::new(((draw_data.tile_frame_x as i32)) as i32, ((draw_data.tile_frame_y as i32)) as i32, (draw_data.tile_width) as i32, (draw_data.tile_height) as i32), (Color::from_ints(75, 75, 75, 0)).into(), ZERO, 1.0_f32, draw_data.tile_sprite_effect);
                            }
                        }
                        34 | 35 => {
                            let tex38 = ctx.flame(num9);
                            ctx.tile_batch.draw(tex38.as_ref(), Vector2::new(((tile_x * 16 - (screen_position.x as i32)) as f32) - ((draw_data.tile_width as f32) - 16.0_f32) / 2.0_f32, (tile_y * 16 - (screen_position.y as i32) + draw_data.tile_top) as f32) + screen_offset, Rectangle::new(((draw_data.tile_frame_x as i32)) as i32, ((draw_data.tile_frame_y as i32)) as i32, (draw_data.tile_width) as i32, (draw_data.tile_height) as i32), (Color::from_ints(75, 75, 75, 0)).into(), ZERO, 1.0_f32, draw_data.tile_sprite_effect);
                        }
                        50 => {
                            let mut tile_flame_data3 = self.get_tile_flame_data(ctx, tile_x, tile_y, (draw_data.type_cache as i32), (draw_data.tile_frame_y as i32));
                            if seed2 == 0 {
                                seed2 = tile_flame_data3.flame_seed;
                            }
                            tile_flame_data3.flame_seed = seed2;
                            for num31 in 0..tile_flame_data3.flame_count {
                                let mut num32: f32 = (utils_random_int(&mut tile_flame_data3.flame_seed, tile_flame_data3.flame_range_x_min, tile_flame_data3.flame_range_x_max) as f32) * tile_flame_data3.flame_range_mult_x;
                                let mut num33: f32 = (utils_random_int(&mut tile_flame_data3.flame_seed, tile_flame_data3.flame_range_y_min, tile_flame_data3.flame_range_y_max) as f32) * tile_flame_data3.flame_range_mult_y;
                                ctx.tile_batch.draw(tile_flame_data3.flame_texture.as_ref(), Vector2::new(((tile_x * 16 - (screen_position.x as i32)) as f32) - ((draw_data.tile_width as f32) - 16.0_f32) / 2.0_f32 + num32, ((tile_y * 16 - (screen_position.y as i32) + draw_data.tile_top) as f32) + num33) + screen_offset, Rectangle::new(((draw_data.tile_frame_x as i32) + draw_data.add_fr_x) as i32, ((draw_data.tile_frame_y as i32) + draw_data.add_fr_y) as i32, (draw_data.tile_width) as i32, (draw_data.tile_height) as i32), (tile_flame_data3.flame_color).into(), ZERO, 1.0_f32, draw_data.tile_sprite_effect);
                            }
                        }
                        _ => {
                            if flag {
                                let mut tile_flame_data2 = self.get_tile_flame_data(ctx, tile_x, tile_y, (draw_data.type_cache as i32), (draw_data.tile_frame_y as i32));
                                if seed2 == 0 {
                                    seed2 = tile_flame_data2.flame_seed;
                                }
                                tile_flame_data2.flame_seed = seed2;
                                for l in 0..tile_flame_data2.flame_count {
                                    let mut num13: f32 = (utils_random_int(&mut tile_flame_data2.flame_seed, tile_flame_data2.flame_range_x_min, tile_flame_data2.flame_range_x_max) as f32) * tile_flame_data2.flame_range_mult_x;
                                    let mut num14: f32 = (utils_random_int(&mut tile_flame_data2.flame_seed, tile_flame_data2.flame_range_y_min, tile_flame_data2.flame_range_y_max) as f32) * tile_flame_data2.flame_range_mult_y;
                                    ctx.tile_batch.draw(tile_flame_data2.flame_texture.as_ref(), Vector2::new(((tile_x * 16 - (screen_position.x as i32)) as f32) - ((draw_data.tile_width as f32) - 16.0_f32) / 2.0_f32 + num13, ((tile_y * 16 - (screen_position.y as i32) + draw_data.tile_top) as f32) + num14) + screen_offset, Rectangle::new(((draw_data.tile_frame_x as i32)) as i32, ((draw_data.tile_frame_y as i32)) as i32, (draw_data.tile_width) as i32, (draw_data.tile_height) as i32), (tile_flame_data2.flame_color).into(), ZERO, 1.0_f32, draw_data.tile_sprite_effect);
                                }
                            } else {
                                for m in 0..7 {
                                    let mut num15: f32 = (utils_random_int(&mut seed2, -10, 11) as f32) * 0.15_f32;
                                    let mut num16: f32 = (utils_random_int(&mut seed2, -10, 1) as f32) * 0.35_f32;
                                    let tex39 = ctx.flame(num9);
                                    ctx.tile_batch.draw(tex39.as_ref(), Vector2::new(((tile_x * 16 - (screen_position.x as i32)) as f32) - ((draw_data.tile_width as f32) - 16.0_f32) / 2.0_f32 + num15, ((tile_y * 16 - (screen_position.y as i32) + draw_data.tile_top) as f32) + num16) + screen_offset, Rectangle::new(((draw_data.tile_frame_x as i32) + draw_data.add_fr_x) as i32, ((draw_data.tile_frame_y as i32) + draw_data.add_fr_y) as i32, (draw_data.tile_width) as i32, (draw_data.tile_height) as i32), (Color::from_ints(100, 100, 100, 0)).into(), ZERO, 1.0_f32, draw_data.tile_sprite_effect);
                                }
                            }
                        }
                    }
                }
                4 => {
                    let mut num37: i32 = ctx.g.main.tile.get(tile_x, tile_y).frame_y as i32 / 54;
                    let mut flag2: bool = num37 >= 45;
                    match num37 {
                        1 => {
                            for num64 in 0..3 {
                                let mut num65: f32 = (utils_random_int(&mut seed2, -10, 11) as f32) * 0.15_f32;
                                let mut num66: f32 = (utils_random_int(&mut seed2, -10, 11) as f32) * 0.15_f32;
                                let tex40 = ctx.flame(num9);
                                ctx.tile_batch.draw(tex40.as_ref(), Vector2::new(((tile_x * 16 - (screen_position.x as i32)) as f32) - ((draw_data.tile_width as f32) - 16.0_f32) / 2.0_f32 + num65, ((tile_y * 16 - (screen_position.y as i32) + draw_data.tile_top) as f32) + num66) + screen_offset, Rectangle::new(((draw_data.tile_frame_x as i32)) as i32, ((draw_data.tile_frame_y as i32)) as i32, (draw_data.tile_width) as i32, (draw_data.tile_height) as i32), (Color::from_ints(50, 50, 50, 0)).into(), ZERO, 1.0_f32, draw_data.tile_sprite_effect);
                            }
                        }
                        2 | 4 => {
                            for num44 in 0..7 {
                                let mut num45: f32 = (utils_random_int(&mut seed2, -10, 11) as f32) * 0.075_f32;
                                let mut num46: f32 = (utils_random_int(&mut seed2, -10, 11) as f32) * 0.075_f32;
                                let tex41 = ctx.flame(num9);
                                ctx.tile_batch.draw(tex41.as_ref(), Vector2::new(((tile_x * 16 - (screen_position.x as i32)) as f32) - ((draw_data.tile_width as f32) - 16.0_f32) / 2.0_f32 + num45, ((tile_y * 16 - (screen_position.y as i32) + draw_data.tile_top) as f32) + num46) + screen_offset, Rectangle::new(((draw_data.tile_frame_x as i32)) as i32, ((draw_data.tile_frame_y as i32)) as i32, (draw_data.tile_width) as i32, (draw_data.tile_height) as i32), (Color::from_ints(50, 50, 50, 0)).into(), ZERO, 1.0_f32, draw_data.tile_sprite_effect);
                            }
                        }
                        3 => {
                            for num52 in 0..7 {
                                let mut num53: f32 = (utils_random_int(&mut seed2, -10, 11) as f32) * 0.2_f32;
                                let mut num54: f32 = (utils_random_int(&mut seed2, -20, 1) as f32) * 0.35_f32;
                                let tex42 = ctx.flame(num9);
                                ctx.tile_batch.draw(tex42.as_ref(), Vector2::new(((tile_x * 16 - (screen_position.x as i32)) as f32) - ((draw_data.tile_width as f32) - 16.0_f32) / 2.0_f32 + num53, ((tile_y * 16 - (screen_position.y as i32) + draw_data.tile_top) as f32) + num54) + screen_offset, Rectangle::new(((draw_data.tile_frame_x as i32)) as i32, ((draw_data.tile_frame_y as i32)) as i32, (draw_data.tile_width) as i32, (draw_data.tile_height) as i32), (Color::from_ints(100, 100, 100, 0)).into(), ZERO, 1.0_f32, draw_data.tile_sprite_effect);
                            }
                        }
                        5 => {
                            for num61 in 0..7 {
                                let mut num62: f32 = (utils_random_int(&mut seed2, -10, 11) as f32) * 0.3_f32;
                                let mut num63: f32 = (utils_random_int(&mut seed2, -10, 11) as f32) * 0.3_f32;
                                let tex43 = ctx.flame(num9);
                                ctx.tile_batch.draw(tex43.as_ref(), Vector2::new(((tile_x * 16 - (screen_position.x as i32)) as f32) - ((draw_data.tile_width as f32) - 16.0_f32) / 2.0_f32 + num62, ((tile_y * 16 - (screen_position.y as i32) + draw_data.tile_top) as f32) + num63) + screen_offset, Rectangle::new(((draw_data.tile_frame_x as i32)) as i32, ((draw_data.tile_frame_y as i32)) as i32, (draw_data.tile_width) as i32, (draw_data.tile_height) as i32), (Color::from_ints(50, 50, 50, 0)).into(), ZERO, 1.0_f32, draw_data.tile_sprite_effect);
                            }
                        }
                        9 => {
                            for num55 in 0..7 {
                                let mut num56: f32 = (utils_random_int(&mut seed2, -10, 11) as f32) * 0.1_f32;
                                let mut num57: f32 = (utils_random_int(&mut seed2, -10, 1) as f32) * 0.15_f32;
                                let tex44 = ctx.flame(num9);
                                ctx.tile_batch.draw(tex44.as_ref(), Vector2::new(((tile_x * 16 - (screen_position.x as i32)) as f32) - ((draw_data.tile_width as f32) - 16.0_f32) / 2.0_f32 + num56, ((tile_y * 16 - (screen_position.y as i32) + draw_data.tile_top) as f32) + num57) + screen_offset, Rectangle::new(((draw_data.tile_frame_x as i32)) as i32, ((draw_data.tile_frame_y as i32)) as i32, (draw_data.tile_width) as i32, (draw_data.tile_height) as i32), (Color::from_ints(50, 50, 50, 0)).into(), ZERO, 1.0_f32, draw_data.tile_sprite_effect);
                            }
                        }
                        13 => {
                            for num47 in 0..8 {
                                let mut num48: f32 = (utils_random_int(&mut seed2, -10, 11) as f32) * 0.1_f32;
                                let mut num49: f32 = (utils_random_int(&mut seed2, -10, 11) as f32) * 0.1_f32;
                                let tex45 = ctx.flame(num9);
                                ctx.tile_batch.draw(tex45.as_ref(), Vector2::new(((tile_x * 16 - (screen_position.x as i32)) as f32) - ((draw_data.tile_width as f32) - 16.0_f32) / 2.0_f32 + num48, ((tile_y * 16 - (screen_position.y as i32) + draw_data.tile_top) as f32) + num49) + screen_offset, Rectangle::new(((draw_data.tile_frame_x as i32)) as i32, ((draw_data.tile_frame_y as i32)) as i32, (draw_data.tile_width) as i32, (draw_data.tile_height) as i32), (Color::from_ints(75, 75, 75, 0)).into(), ZERO, 1.0_f32, draw_data.tile_sprite_effect);
                            }
                        }
                        12 => {
                            let mut num50: f32 = (utils_random_int(&mut seed2, -10, 11) as f32) * 0.01_f32;
                            let mut num51: f32 = (utils_random_int(&mut seed2, -10, 11) as f32) * 0.01_f32;
                            let tex46 = ctx.flame(num9);
                            ctx.tile_batch.draw(tex46.as_ref(), Vector2::new(((tile_x * 16 - (screen_position.x as i32)) as f32) - ((draw_data.tile_width as f32) - 16.0_f32) / 2.0_f32 + num50, ((tile_y * 16 - (screen_position.y as i32) + draw_data.tile_top) as f32) + num51) + screen_offset, Rectangle::new(((draw_data.tile_frame_x as i32)) as i32, ((draw_data.tile_frame_y as i32)) as i32, (draw_data.tile_width) as i32, (draw_data.tile_height) as i32), (Color::from_ints((utils_random_int(&mut seed2, 90, 111)) as i32, (utils_random_int(&mut seed2, 90, 111)) as i32, (utils_random_int(&mut seed2, 90, 111)) as i32, 0)).into(), ZERO, 1.0_f32, draw_data.tile_sprite_effect);
                        }
                        28 | 29 => {
                            let tex47 = ctx.flame(num9);
                            ctx.tile_batch.draw(tex47.as_ref(), Vector2::new(((tile_x * 16 - (screen_position.x as i32)) as f32) - ((draw_data.tile_width as f32) - 16.0_f32) / 2.0_f32, (tile_y * 16 - (screen_position.y as i32) + draw_data.tile_top) as f32) + screen_offset, Rectangle::new(((draw_data.tile_frame_x as i32)) as i32, ((draw_data.tile_frame_y as i32)) as i32, (draw_data.tile_width) as i32, (draw_data.tile_height) as i32), (Color::from_ints(75, 75, 75, 0)).into(), ZERO, 1.0_f32, draw_data.tile_sprite_effect);
                        }
                        44 => {
                            let mut tile_flame_data5 = self.get_tile_flame_data(ctx, tile_x, tile_y, (draw_data.type_cache as i32), (draw_data.tile_frame_y as i32));
                            if seed2 == 0 {
                                seed2 = tile_flame_data5.flame_seed;
                            }
                            tile_flame_data5.flame_seed = seed2;
                            for num58 in 0..tile_flame_data5.flame_count {
                                let mut num59: f32 = (utils_random_int(&mut tile_flame_data5.flame_seed, tile_flame_data5.flame_range_x_min, tile_flame_data5.flame_range_x_max) as f32) * tile_flame_data5.flame_range_mult_x;
                                let mut num60: f32 = (utils_random_int(&mut tile_flame_data5.flame_seed, tile_flame_data5.flame_range_y_min, tile_flame_data5.flame_range_y_max) as f32) * tile_flame_data5.flame_range_mult_y;
                                ctx.tile_batch.draw(tile_flame_data5.flame_texture.as_ref(), Vector2::new(((tile_x * 16 - (screen_position.x as i32)) as f32) - ((draw_data.tile_width as f32) - 16.0_f32) / 2.0_f32 + num59, ((tile_y * 16 - (screen_position.y as i32) + draw_data.tile_top) as f32) + num60) + screen_offset, Rectangle::new(((draw_data.tile_frame_x as i32) + draw_data.add_fr_x) as i32, ((draw_data.tile_frame_y as i32) + draw_data.add_fr_y) as i32, (draw_data.tile_width) as i32, (draw_data.tile_height) as i32), (tile_flame_data5.flame_color).into(), ZERO, 1.0_f32, draw_data.tile_sprite_effect);
                            }
                        }
                        _ => {
                            if flag2 {
                                let mut tile_flame_data4 = self.get_tile_flame_data(ctx, tile_x, tile_y, (draw_data.type_cache as i32), (draw_data.tile_frame_y as i32));
                                if seed2 == 0 {
                                    seed2 = tile_flame_data4.flame_seed;
                                }
                                tile_flame_data4.flame_seed = seed2;
                                for num38 in 0..tile_flame_data4.flame_count {
                                    let mut num39: f32 = (utils_random_int(&mut tile_flame_data4.flame_seed, tile_flame_data4.flame_range_x_min, tile_flame_data4.flame_range_x_max) as f32) * tile_flame_data4.flame_range_mult_x;
                                    let mut num40: f32 = (utils_random_int(&mut tile_flame_data4.flame_seed, tile_flame_data4.flame_range_y_min, tile_flame_data4.flame_range_y_max) as f32) * tile_flame_data4.flame_range_mult_y;
                                    ctx.tile_batch.draw(tile_flame_data4.flame_texture.as_ref(), Vector2::new(((tile_x * 16 - (screen_position.x as i32)) as f32) - ((draw_data.tile_width as f32) - 16.0_f32) / 2.0_f32 + num39, ((tile_y * 16 - (screen_position.y as i32) + draw_data.tile_top) as f32) + num40) + screen_offset, Rectangle::new(((draw_data.tile_frame_x as i32) + draw_data.add_fr_x) as i32, ((draw_data.tile_frame_y as i32) + draw_data.add_fr_y) as i32, (draw_data.tile_width) as i32, (draw_data.tile_height) as i32), (tile_flame_data4.flame_color).into(), ZERO, 1.0_f32, draw_data.tile_sprite_effect);
                                }
                            } else {
                                for num41 in 0..7 {
                                    let mut num42: f32 = (utils_random_int(&mut seed2, -10, 11) as f32) * 0.15_f32;
                                    let mut num43: f32 = (utils_random_int(&mut seed2, -10, 1) as f32) * 0.35_f32;
                                    let tex48 = ctx.flame(num9);
                                    ctx.tile_batch.draw(tex48.as_ref(), Vector2::new(((tile_x * 16 - (screen_position.x as i32)) as f32) - ((draw_data.tile_width as f32) - 16.0_f32) / 2.0_f32 + num42, ((tile_y * 16 - (screen_position.y as i32) + draw_data.tile_top) as f32) + num43) + screen_offset, Rectangle::new(((draw_data.tile_frame_x as i32) + draw_data.add_fr_x) as i32, ((draw_data.tile_frame_y as i32) + draw_data.add_fr_y) as i32, (draw_data.tile_width) as i32, (draw_data.tile_height) as i32), (Color::from_ints(100, 100, 100, 0)).into(), ZERO, 1.0_f32, draw_data.tile_sprite_effect);
                                }
                            }
                        }
                    }
                }
                13 => {
                    let mut num120: i32 = (draw_data.tile_frame_y as i32) / 36;
                    let mut flag5: bool = num120 >= 51;
                    match num120 {
                        1 | 3 | 6 | 8 | 19 | 27 | 29 | 30 | 31 | 32 | 36 | 39 => {
                            for num133 in 0..7 {
                                let mut num134: f32 = (utils_random_int(&mut seed2, -10, 11) as f32) * 0.15_f32;
                                let mut num135: f32 = (utils_random_int(&mut seed2, -10, 1) as f32) * 0.35_f32;
                                let tex49 = ctx.flame(num9);
                                ctx.tile_batch.draw(tex49.as_ref(), Vector2::new(((tile_x * 16 - (screen_position.x as i32)) as f32) - ((draw_data.tile_width as f32) - 16.0_f32) / 2.0_f32 + num134, ((tile_y * 16 - (screen_position.y as i32) + draw_data.tile_top) as f32) + num135) + screen_offset, Rectangle::new(((draw_data.tile_frame_x as i32)) as i32, ((draw_data.tile_frame_y as i32)) as i32, (draw_data.tile_width) as i32, (draw_data.tile_height) as i32), (Color::from_ints(100, 100, 100, 0)).into(), ZERO, 1.0_f32, draw_data.tile_sprite_effect);
                            }
                        }
                        2 | 16 | 25 => {
                            for num130 in 0..7 {
                                let mut num131: f32 = (utils_random_int(&mut seed2, -10, 11) as f32) * 0.15_f32;
                                let mut num132: f32 = (utils_random_int(&mut seed2, -10, 1) as f32) * 0.1_f32;
                                let tex50 = ctx.flame(num9);
                                ctx.tile_batch.draw(tex50.as_ref(), Vector2::new(((tile_x * 16 - (screen_position.x as i32)) as f32) - ((draw_data.tile_width as f32) - 16.0_f32) / 2.0_f32 + num131, ((tile_y * 16 - (screen_position.y as i32) + draw_data.tile_top) as f32) + num132) + screen_offset, Rectangle::new(((draw_data.tile_frame_x as i32)) as i32, ((draw_data.tile_frame_y as i32)) as i32, (draw_data.tile_width) as i32, (draw_data.tile_height) as i32), (Color::from_ints(50, 50, 50, 0)).into(), ZERO, 1.0_f32, draw_data.tile_sprite_effect);
                            }
                        }
                        _ => {
                            match num120 {
                                29 => {
                                    for num124 in 0..7 {
                                        let mut num125: f32 = (utils_random_int(&mut seed2, -10, 11) as f32) * 0.15_f32;
                                        let mut num126: f32 = (utils_random_int(&mut seed2, -10, 1) as f32) * 0.15_f32;
                                        let tex51 = ctx.flame(num9);
                                        ctx.tile_batch.draw(tex51.as_ref(), Vector2::new(((tile_x * 16 - (screen_position.x as i32)) as f32) - ((draw_data.tile_width as f32) - 16.0_f32) / 2.0_f32 + num125, ((tile_y * 16 - (screen_position.y as i32) + draw_data.tile_top) as f32) + num126) + screen_offset, Rectangle::new(((draw_data.tile_frame_x as i32)) as i32, ((draw_data.tile_frame_y as i32)) as i32, (draw_data.tile_width) as i32, (draw_data.tile_height) as i32), (Color::from_ints(25, 25, 25, 0)).into(), ZERO, 1.0_f32, draw_data.tile_sprite_effect);
                                    }
                                }
                                34 | 35 => {
                                    let tex52 = ctx.flame(num9);
                                    ctx.tile_batch.draw(tex52.as_ref(), Vector2::new(((tile_x * 16 - (screen_position.x as i32)) as f32) - ((draw_data.tile_width as f32) - 16.0_f32) / 2.0_f32, (tile_y * 16 - (screen_position.y as i32) + draw_data.tile_top) as f32) + screen_offset, Rectangle::new(((draw_data.tile_frame_x as i32)) as i32, ((draw_data.tile_frame_y as i32)) as i32, (draw_data.tile_width) as i32, (draw_data.tile_height) as i32), (Color::from_ints(75, 75, 75, 0)).into(), ZERO, 1.0_f32, draw_data.tile_sprite_effect);
                                }
                                50 => {
                                    let mut tile_flame_data11 = self.get_tile_flame_data(ctx, tile_x, tile_y, (draw_data.type_cache as i32), (draw_data.tile_frame_y as i32));
                                    if seed2 == 0 {
                                        seed2 = tile_flame_data11.flame_seed;
                                    }
                                    tile_flame_data11.flame_seed = seed2;
                                    for num127 in 0..tile_flame_data11.flame_count {
                                        let mut num128: f32 = (utils_random_int(&mut tile_flame_data11.flame_seed, tile_flame_data11.flame_range_x_min, tile_flame_data11.flame_range_x_max) as f32) * tile_flame_data11.flame_range_mult_x;
                                        let mut num129: f32 = (utils_random_int(&mut tile_flame_data11.flame_seed, tile_flame_data11.flame_range_y_min, tile_flame_data11.flame_range_y_max) as f32) * tile_flame_data11.flame_range_mult_y;
                                        ctx.tile_batch.draw(tile_flame_data11.flame_texture.as_ref(), Vector2::new(((tile_x * 16 - (screen_position.x as i32)) as f32) - ((draw_data.tile_width as f32) - 16.0_f32) / 2.0_f32 + num128, ((tile_y * 16 - (screen_position.y as i32) + draw_data.tile_top) as f32) + num129) + screen_offset, Rectangle::new(((draw_data.tile_frame_x as i32) + draw_data.add_fr_x) as i32, ((draw_data.tile_frame_y as i32) + draw_data.add_fr_y) as i32, (draw_data.tile_width) as i32, (draw_data.tile_height) as i32), (tile_flame_data11.flame_color).into(), ZERO, 1.0_f32, draw_data.tile_sprite_effect);
                                    }
                                }
                                _ => {
                                    if flag5 {
                                        let mut tile_flame_data10 = self.get_tile_flame_data(ctx, tile_x, tile_y, (draw_data.type_cache as i32), (draw_data.tile_frame_y as i32));
                                        if seed2 == 0 {
                                            seed2 = tile_flame_data10.flame_seed;
                                        }
                                        tile_flame_data10.flame_seed = seed2;
                                        for num121 in 0..tile_flame_data10.flame_count {
                                            let mut num122: f32 = (utils_random_int(&mut tile_flame_data10.flame_seed, tile_flame_data10.flame_range_x_min, tile_flame_data10.flame_range_x_max) as f32) * tile_flame_data10.flame_range_mult_x;
                                            let mut num123: f32 = (utils_random_int(&mut tile_flame_data10.flame_seed, tile_flame_data10.flame_range_y_min, tile_flame_data10.flame_range_y_max) as f32) * tile_flame_data10.flame_range_mult_y;
                                            ctx.tile_batch.draw(tile_flame_data10.flame_texture.as_ref(), Vector2::new(((tile_x * 16 - (screen_position.x as i32)) as f32) - ((draw_data.tile_width as f32) - 16.0_f32) / 2.0_f32 + num122, ((tile_y * 16 - (screen_position.y as i32) + draw_data.tile_top) as f32) + num123) + screen_offset, Rectangle::new(((draw_data.tile_frame_x as i32)) as i32, ((draw_data.tile_frame_y as i32)) as i32, (draw_data.tile_width) as i32, (draw_data.tile_height) as i32), (tile_flame_data10.flame_color).into(), ZERO, 1.0_f32, draw_data.tile_sprite_effect);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                _ => {
                    let mut color4 = Color::from_ints(100, 100, 100, 0);
                    if (draw_data.tile_cache.type_ as i32) == 4 {
                        match (draw_data.tile_cache.frame_y as i32) / 22 {
                            14 => {
                                color4 = Color::from_floats((ctx.g.main.disco_r as f32) / 255.0_f32, (ctx.g.main.disco_g as f32) / 255.0_f32, (ctx.g.main.disco_b as f32) / 255.0_f32, 0.0_f32);
                            }
                            22 => {
                                color4 = Color::from_ints(50, 50, 100, 20);
                            }
                            23 => {
                                color4 = Color::from_ints(255, 255, 255, 200);
                            }
                            _ => {}
                        }
                    }
                    if (draw_data.tile_cache.type_ as i32) == 646 {
                        color4 = Color::from_ints(100, 100, 100, 150);
                    }
                    for k in 0..7 {
                        let mut num10: f32 = (utils_random_int(&mut seed2, -10, 11) as f32) * 0.15_f32;
                        let mut num11: f32 = (utils_random_int(&mut seed2, -10, 1) as f32) * 0.35_f32;
                        let tex53 = ctx.flame(num9);
                        ctx.tile_batch.draw(tex53.as_ref(), Vector2::new(((tile_x * 16 - (screen_position.x as i32)) as f32) - ((draw_data.tile_width as f32) - 16.0_f32) / 2.0_f32 + num10, ((tile_y * 16 - (screen_position.y as i32) + draw_data.tile_top) as f32) + num11) + screen_offset, Rectangle::new(((draw_data.tile_frame_x as i32)) as i32, ((draw_data.tile_frame_y as i32)) as i32, (draw_data.tile_width) as i32, (draw_data.tile_height) as i32), (color4).into(), ZERO, 1.0_f32, draw_data.tile_sprite_effect);
                    }
                }
            }
        }
        if (draw_data.type_cache as i32) == 144 {
            let tex54 = ctx.timer();
            ctx.tile_batch.draw(tex54.as_ref(), Vector2::new(((tile_x * 16 - (screen_position.x as i32)) as f32) - ((draw_data.tile_width as f32) - 16.0_f32) / 2.0_f32, (tile_y * 16 - (screen_position.y as i32) + draw_data.tile_top) as f32) + screen_offset, Rectangle::new(((draw_data.tile_frame_x as i32)) as i32, ((draw_data.tile_frame_y as i32)) as i32, (draw_data.tile_width) as i32, (draw_data.tile_height) as i32), (Color::from_ints(200, 200, 200, 0)).into(), ZERO, 1.0_f32, draw_data.tile_sprite_effect);
        }
        if (draw_data.type_cache as i32) == 237 {
            let tex55 = ctx.sun_altar();
            ctx.tile_batch.draw(tex55.as_ref(), Vector2::new(((tile_x * 16 - (screen_position.x as i32)) as f32) - ((draw_data.tile_width as f32) - 16.0_f32) / 2.0_f32, (tile_y * 16 - (screen_position.y as i32) + draw_data.tile_top) as f32) + screen_offset, Rectangle::new(((draw_data.tile_frame_x as i32)) as i32, ((draw_data.tile_frame_y as i32)) as i32, (draw_data.tile_width) as i32, (draw_data.tile_height) as i32), (Color::from_ints((ctx.g.main.mouse_text_color as i32 as i32 / 2) as i32, (ctx.g.main.mouse_text_color as i32 as i32 / 2) as i32, (ctx.g.main.mouse_text_color as i32 as i32 / 2) as i32, 0)).into(), ZERO, 1.0_f32, draw_data.tile_sprite_effect);
        }
        if (draw_data.type_cache as i32) != 658 || (draw_data.tile_frame_x as i32) % 36 != 0 || (draw_data.tile_frame_y as i32) % 54 != 0 {
            return;
        }
        let mut num136: i32 = (draw_data.tile_frame_y as i32) / 54;
        if num136 != 2 {
            let mut value3 = ctx.glow_mask(334);
            let mut vector2 = Vector2::new(0.0_f32, -10.0_f32);
            let mut position = Vector2::new(((tile_x * 16 - (screen_position.x as i32)) as f32) - (draw_data.tile_width as f32) / 2.0_f32, (tile_y * 16 - (screen_position.y as i32) + draw_data.tile_top) as f32) + screen_offset + vector2;
            let mut source_rectangle4 = texture_frame_full(value3.as_ref());
            let mut color5 = Color::from_ints((ctx.g.main.mouse_text_color as i32 as i32) as i32, (ctx.g.main.mouse_text_color as i32 as i32) as i32, (ctx.g.main.mouse_text_color as i32 as i32) as i32, 0);
            if num136 == 0 {
                color5 = color5 * 0.75_f32;
            }
            ctx.tile_batch.draw(value3.as_ref(), position, source_rectangle4, (color5).into(), ZERO, 1.0_f32, draw_data.tile_sprite_effect);
        }
    }
}
