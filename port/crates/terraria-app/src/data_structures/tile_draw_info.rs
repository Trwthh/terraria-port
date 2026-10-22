//! `Terraria.DataStructures.TileDrawInfo` (DataStructures/TileDrawInfo.cs)

use terraria_core::xna::{Color, Rectangle, Vector3};
use terraria_core::Tile;

use crate::gfx::{SpriteEffects, Texture2D};

/// `Terraria.DataStructures.TileDrawInfo`. `tileCache` is a reference to the live tile in the original; here it is a
/// copy taken when the info is created (nothing in the drawing path writes the tile).
#[derive(Clone, Debug)]
pub struct TileDrawInfo {
    pub tile_cache: Tile,
    pub type_cache: u16,
    pub tile_frame_x: i16,
    pub tile_frame_y: i16,
    /// `None` when the texture file is missing (logged once by `DrawContext`); nothing is drawn with it.
    pub draw_texture: Option<Texture2D>,
    pub tile_light: Color,
    pub tile_top: i32,
    pub tile_width: i32,
    pub tile_height: i32,
    pub half_brick_height: i32,
    pub add_fr_y: i32,
    pub add_fr_x: i32,
    pub tile_sprite_effect: SpriteEffects,
    pub glow_texture: Option<Texture2D>,
    pub glow_source_rect: Rectangle,
    pub glow_color: Color,
    pub color_slices: [Vector3; 9],
    pub final_color: Color,
    pub color_tint: Color,
}

impl TileDrawInfo {
    /// `new TileDrawInfo()`: every field default (`null`, 0, `Color.Transparent`), `colorSlices = new Vector3[9]`.
    pub fn new() -> Self {
        Self {
            tile_cache: Tile::default(),
            type_cache: 0,
            tile_frame_x: 0,
            tile_frame_y: 0,
            draw_texture: None,
            tile_light: Color::TRANSPARENT,
            tile_top: 0,
            tile_width: 0,
            tile_height: 0,
            half_brick_height: 0,
            add_fr_y: 0,
            add_fr_x: 0,
            tile_sprite_effect: SpriteEffects::NONE,
            glow_texture: None,
            glow_source_rect: Rectangle::EMPTY,
            glow_color: Color::TRANSPARENT,
            color_slices: [Vector3::ZERO; 9],
            final_color: Color::TRANSPARENT,
            color_tint: Color::TRANSPARENT,
        }
    }
}

impl Default for TileDrawInfo {
    fn default() -> Self {
        Self::new()
    }
}
