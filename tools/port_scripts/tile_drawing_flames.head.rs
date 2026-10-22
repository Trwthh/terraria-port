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
//@FLAME_DATA_SWITCH@
        result
    }

    /// `DrawSingleTile_Flames(Vector2 screenPosition, Vector2 screenOffset, int tileX, int tileY, TileDrawInfo drawData)` (TileDrawing.cs:2707)
    pub(super) fn draw_single_tile_flames(&mut self, ctx: &mut DrawContext, screen_position: Vector2, screen_offset: Vector2, tile_x: i32, tile_y: i32, draw_data: &TileDrawInfo) {
//@DRAW_FLAMES_BODY@
    }
}
