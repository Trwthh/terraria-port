//! `Terraria.GameContent.Drawing.WallDrawing` (GameContent/Drawing/WallDrawing.cs): draws the world's walls through
//! `Main.tileBatch`, with the neighbour outlines through `Main.spriteBatch`.

use terraria_core::xna::{Color, Rectangle, Vector2};
use terraria_core::Tile;
use terraria_sim::unported;

use crate::draw_context::DrawContext;
use crate::game_content::drawing::draw_black_helper::DrawBlackHelper;
use crate::game_content::drawing::tile_drawing::get_screen_draw_area;
use crate::game_content::drawing::tile_drawing_base::{TileDrawingBase, DRAW_OWN_BLACKS};
use crate::gfx::{SpriteEffects, Texture2D};
use crate::pending::{lighting_get_color, lighting_get_corner_colors, lighting_not_retro, world_gen_solid_tile_tile, VertexColors};
use crate::testing::debug_options;
use terraria_data::sets::tile_id::sets as tile_sets;

/// `WallDrawing._glowPaintColors` (WallDrawing.cs:14)
const GLOW_PAINT_COLORS: VertexColors = VertexColors { top_left_color: Color::WHITE, top_right_color: Color::WHITE, bottom_left_color: Color::WHITE, bottom_right_color: Color::WHITE };

/// `TilePaintSystemV2.WallVariationKey`
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct WallVariationKey {
    wall_type: i32,
    paint_color: i32,
}

/// `Main.WallsRenderer`
pub struct WallDrawing {
    pub base: TileDrawingBase,
    /// `_shouldShowInvisibleWalls`
    should_show_invisible_walls: bool,
    draw_black_helper: DrawBlackHelper,
    last_paint_lookup_key: WallVariationKey,
    last_paint_lookup_texture: Option<Texture2D>,
}

impl WallDrawing {
    /// `new WallDrawing(TilePaintSystemV2 paintSystem)`
    pub fn new() -> Self {
        Self {
            base: TileDrawingBase::default(),
            should_show_invisible_walls: false,
            draw_black_helper: DrawBlackHelper::new(0, Vector2::ZERO),
            last_paint_lookup_key: WallVariationKey::default(),
            last_paint_lookup_texture: None,
        }
    }

    /// `LerpVertexColorsWithColor(ref VertexColors colors, Color lerpColor, float percent)` (WallDrawing.cs:28)
    pub fn lerp_vertex_colors_with_color(colors: &mut VertexColors, lerp_color: Color, percent: f32) {
        colors.top_left_color = Color::lerp(colors.top_left_color, lerp_color, percent);
        colors.top_right_color = Color::lerp(colors.top_right_color, lerp_color, percent);
        colors.bottom_left_color = Color::lerp(colors.bottom_left_color, lerp_color, percent);
        colors.bottom_right_color = Color::lerp(colors.bottom_right_color, lerp_color, percent);
    }

    /// `Update()` (WallDrawing.cs:41)
    pub fn update(&mut self, ctx: &mut DrawContext) {
        if !ctx.g.main.ded_serv {
            self.should_show_invisible_walls = ctx.g.main.should_show_invisible_blocks_and_walls();
        }
    }

    /// `DrawOutline(Texture2D texture, Vector2 position, Rectangle sourceRectangle, Color color)` (WallDrawing.cs:49)
    pub fn draw_outline(ctx: &mut DrawContext, texture: Option<&Texture2D>, position: Vector2, source_rectangle: Rectangle, color: Color) {
        if let Some(texture) = texture {
            ctx.sprite_batch.draw(texture, position, Some(source_rectangle), color, 0.0, Vector2::ZERO, Vector2::ONE, SpriteEffects::NONE);
        }
    }

    /// `DrawWalls()` (WallDrawing.cs:54)
    pub fn draw_walls(&mut self, ctx: &mut DrawContext) {
        if debug_options::HIDE_WALLS {
            return;
        }
        let gfx_quality = ctx.state.gfx_quality;
        let screen_position = Vector2::new(ctx.g.main.screen_position.0, ctx.g.main.screen_position.1);
        let mut num = (120.0_f32 * (1.0 - gfx_quality) + 40.0_f32 * gfx_quality) as i32;
        if debug_options::DEV_LIGHT_TILES_CHEAT {
            num = 1000;
        }
        let num2 = (num as f32 * 0.4) as i32;
        let num3 = (num as f32 * 0.35) as i32;
        let num4 = (num as f32 * 0.3) as i32;
        let (draw_off_set, first_tile_x, last_tile_x, first_tile_y, last_tile_y) = get_screen_draw_area(ctx, !ctx.state.draw_to_screen);
        let mut vertices = VertexColors::default();
        let mut source_rectangle = Rectangle::new(0, 0, 32, 32);
        let underworld_layer = ctx.g.main.max_tiles_y - 200; // Main.UnderworldLayer
        self.draw_black_helper = DrawBlackHelper::new(0, draw_off_set);
        self.last_paint_lookup_key = WallVariationKey::default();
        self.last_paint_lookup_texture = None;
        for i in first_tile_y..last_tile_y {
            for j in first_tile_x..last_tile_x {
                let tile = *ctx.g.main.tile.get(j, i);
                let wall = tile.wall;
                if wall == 0 || self.full_tile(ctx, j, i) || (wall == 318 && !self.should_show_invisible_walls) || (tile.invisible_wall() && !self.should_show_invisible_walls) {
                    continue;
                }
                let mut color = lighting_get_color(ctx.g, j, i);
                if tile.fullbright_wall() {
                    color = Color::WHITE;
                }
                if wall == 318 {
                    color = Color::WHITE;
                }
                if DRAW_OWN_BLACKS {
                    if color.r() == 0 && color.g() == 0 && color.b() == 0 {
                        self.draw_black_helper.draw_black(ctx, j, i);
                        continue;
                    }
                } else if color.r() == 0 && color.g() == 0 && color.b() == 0 && i < underworld_layer {
                    continue;
                }
                let wall_draw_texture = self.get_wall_draw_texture(ctx, &tile);
                ctx.tile_batch.set_layer(((if tile.active() { 1u32 } else { 0u32 }) << 21) | ((tile.wall_color() as u32) << 16) | wall as u32, 0);
                source_rectangle.x = tile.wall_frame_x();
                source_rectangle.y = tile.wall_frame_y() + ctx.state.wall_frame[wall as usize] as i32 * 180;
                let mut wall2 = tile.wall;
                if (wall2 as u32).wrapping_sub(242) <= 1 {
                    let num5 = 20;
                    let num6 = (ctx.state.wall_frame_counter[wall as usize] as i32 + j * 11 + i * 27) % (num5 * 8);
                    source_rectangle.y = tile.wall_frame_y() + 180 * (num6 / num5);
                }
                let position = Vector2::new((j * 16 - screen_position.x as i32 - 8) as f32, (i * 16 - screen_position.y as i32 - 8) as f32) + draw_off_set;
                if lighting_not_retro(ctx.g) && !ctx.g.main.wall_light[wall as usize] && tile.wall != 241 && (tile.wall < 88 || tile.wall > 93) && !world_gen_solid_tile_tile(ctx.g, &tile) {
                    if tile.wall == 346 {
                        let c = Color::new(ctx.g.main.disco_r as u8, ctx.g.main.disco_g as u8, ctx.g.main.disco_b as u8, 255);
                        vertices.top_right_color = c;
                        vertices.top_left_color = c;
                        vertices.bottom_right_color = c;
                        vertices.bottom_left_color = c;
                    } else if tile.wall == 44 {
                        let c = Color::new(ctx.g.main.disco_r as u8, ctx.g.main.disco_g as u8, ctx.g.main.disco_b as u8, 255);
                        vertices.top_right_color = c;
                        vertices.top_left_color = c;
                        vertices.bottom_right_color = c;
                        vertices.bottom_left_color = c;
                    } else {
                        vertices = lighting_get_corner_colors(ctx.g, j, i);
                        wall2 = tile.wall;
                        if (wall2 as u32).wrapping_sub(341) <= 4 {
                            Self::lerp_vertex_colors_with_color(&mut vertices, Color::WHITE, 0.5);
                        }
                        if tile.fullbright_wall() {
                            vertices = GLOW_PAINT_COLORS;
                        }
                    }
                    ctx.tile_batch.draw(wall_draw_texture.as_ref(), position, source_rectangle, vertices, Vector2::ZERO, 1.0, SpriteEffects::NONE);
                    if tile.wall == 347 {
                        let value = ctx.glow_mask(361);
                        // LiquidRenderer.SetShimmerVertexColors_Sparkle(ref vertices, 0.7f, j, i, top: true): not ported; the glow layer stays invisible
                        vertices = unported!("LiquidRenderer.SetShimmerVertexColors_Sparkle", VertexColors::new(Color::TRANSPARENT));
                        ctx.tile_batch.draw(value.as_ref(), position, source_rectangle, vertices, Vector2::ZERO, 1.0, SpriteEffects::NONE);
                    }
                } else {
                    let mut color2 = color;
                    if wall == 44 || wall == 346 {
                        color2 = Color::from_ints_rgb(ctx.g.main.disco_r, ctx.g.main.disco_g, ctx.g.main.disco_b);
                    }
                    if (wall as u32).wrapping_sub(341) <= 4 {
                        color2 = Color::lerp(color2, Color::WHITE, 0.5);
                    }
                    ctx.tile_batch.draw(wall_draw_texture.as_ref(), position, source_rectangle, color2.into(), Vector2::ZERO, 1.0, SpriteEffects::NONE);
                    if tile.wall == 347 {
                        let value2 = ctx.glow_mask(361);
                        let color3 = unported!("LiquidRenderer.GetShimmerGlitterColor", Color::TRANSPARENT) * 0.7_f32;
                        ctx.tile_batch.draw(value2.as_ref(), position, source_rectangle, color3.into(), Vector2::ZERO, 1.0, SpriteEffects::NONE);
                    }
                }
                if color.r() as i32 > num2 || color.g() as i32 > num3 || color.b() as i32 > num4 {
                    let wall_blend = &ctx.g.main.wall_blend;
                    let left = *ctx.g.main.tile.get(j - 1, i);
                    let right = *ctx.g.main.tile.get(j + 1, i);
                    let up = *ctx.g.main.tile.get(j, i - 1);
                    let down = *ctx.g.main.tile.get(j, i + 1);
                    let num7 = left.wall > 0 && wall_blend[left.wall as usize] != wall_blend[tile.wall as usize];
                    let flag = right.wall > 0 && wall_blend[right.wall as usize] != wall_blend[tile.wall as usize];
                    let flag2 = up.wall > 0 && wall_blend[up.wall as usize] != wall_blend[tile.wall as usize];
                    let flag3 = down.wall > 0 && wall_blend[down.wall as usize] != wall_blend[tile.wall as usize];
                    let sx = screen_position.x as i32;
                    let sy = screen_position.y as i32;
                    if num7 {
                        let t = ctx.wall_outline();
                        Self::draw_outline(ctx, t.as_ref(), Vector2::new((j * 16 - sx) as f32, (i * 16 - sy) as f32) + draw_off_set, Rectangle::new(0, 0, 2, 16), color);
                    }
                    if flag {
                        let t = ctx.wall_outline();
                        Self::draw_outline(ctx, t.as_ref(), Vector2::new((j * 16 - sx + 14) as f32, (i * 16 - sy) as f32) + draw_off_set, Rectangle::new(14, 0, 2, 16), color);
                    }
                    if flag2 {
                        let t = ctx.wall_outline();
                        Self::draw_outline(ctx, t.as_ref(), Vector2::new((j * 16 - sx) as f32, (i * 16 - sy) as f32) + draw_off_set, Rectangle::new(0, 0, 16, 2), color);
                    }
                    if flag3 {
                        let t = ctx.wall_outline();
                        Self::draw_outline(ctx, t.as_ref(), Vector2::new((j * 16 - sx) as f32, (i * 16 - sy + 14) as f32) + draw_off_set, Rectangle::new(0, 14, 16, 2), color);
                    }
                }
            }
        }
        self.draw_black_helper.end_strip(ctx);
        self.base.restart_layered_batch(ctx);
        unported!("Main.DrawTileCracks(2, hitReplace)");
        unported!("Main.DrawTileCracks(2, hitTile)");
    }

    /// `GetWallDrawTexture(Tile tile)` (WallDrawing.cs:209)
    pub fn get_wall_draw_texture(&mut self, ctx: &mut DrawContext, tile: &Tile) -> Option<Texture2D> {
        self.get_wall_draw_texture_type_paint(ctx, tile.wall as i32, tile.wall_color() as i32)
    }

    /// `GetWallDrawTexture(int wallType, int paintColor)` (WallDrawing.cs:214)
    pub fn get_wall_draw_texture_type_paint(&mut self, ctx: &mut DrawContext, wall_type: i32, paint_color: i32) -> Option<Texture2D> {
        let wall_variation_key = WallVariationKey { wall_type, paint_color };
        if self.last_paint_lookup_key == wall_variation_key {
            return self.last_paint_lookup_texture.clone();
        }
        self.last_paint_lookup_key = wall_variation_key;
        self.last_paint_lookup_texture = Self::lookup_wall_draw_texture(ctx, wall_variation_key);
        self.last_paint_lookup_texture.clone()
    }

    /// `LookupWallDrawTexture(TilePaintSystemV2.WallVariationKey key)` (WallDrawing.cs:230)
    fn lookup_wall_draw_texture(ctx: &mut DrawContext, key: WallVariationKey) -> Option<Texture2D> {
        if key.paint_color != 0 {
            // the paint system renders the variant into a target and answers null until it is ready
            let texture2_d: Option<Texture2D> = unported!("TilePaintSystemV2.TryGetWallAndRequestIfNotReady", None);
            if texture2_d.is_some() {
                return texture2_d;
            }
        }
        ctx.wall_texture(key.wall_type)
    }

    /// `FullTile(int x, int y)` (WallDrawing.cs:243)
    fn full_tile(&self, ctx: &DrawContext, x: i32, y: i32) -> bool {
        if ctx.g.main.tile.get(x - 1, y).block_type() != 0 || ctx.g.main.tile.get(x + 1, y).block_type() != 0 {
            return false;
        }
        let tile = *ctx.g.main.tile.get(x, y);
        if tile.active() {
            if ctx.g.main.tile_frame_important[tile.type_ as usize] || tile_sets::DRAWS_WALLS[tile.type_ as usize] {
                return false;
            }
            if tile.invisible_block() && !self.should_show_invisible_walls {
                return false;
            }
            if debug_options::SHOW_UNBREAKABLE_WALL && tile.wall == 350 {
                return false;
            }
            if tile.type_ == 740 {
                let frame_x = tile.frame_x;
                let frame_y = tile.frame_y;
                if (frame_x == 180 || frame_x == 198) && frame_y >= 0 && frame_y <= 36 {
                    return false;
                }
                if frame_x >= 108 && frame_x <= 144 && (frame_y == 18 || frame_y == 36) {
                    return false;
                }
            }
            if ctx.g.main.tile_solid[tile.type_ as usize] && !ctx.g.main.tile_solid_top[tile.type_ as usize] {
                let frame_x2 = tile.frame_x as i32;
                let frame_y2 = tile.frame_y as i32;
                if ctx.g.main.tile_large_frames[tile.type_ as usize] > 0 {
                    if frame_y2 == 18 || frame_y2 == 108 {
                        if frame_x2 >= 18 && frame_x2 <= 54 {
                            return true;
                        }
                        if frame_x2 >= 108 && frame_x2 <= 144 {
                            return true;
                        }
                    }
                } else {
                    match frame_y2 {
                        0 => {
                            if frame_x2 >= 180 && frame_x2 <= 198 {
                                return true;
                            }
                        }
                        18 => {
                            if frame_x2 >= 18 && frame_x2 <= 54 {
                                return true;
                            }
                            if frame_x2 >= 108 && frame_x2 <= 144 {
                                return true;
                            }
                            if frame_x2 >= 180 && frame_x2 <= 198 {
                                return true;
                            }
                        }
                        36 => {
                            if frame_x2 >= 108 && frame_x2 <= 144 {
                                return true;
                            }
                            if frame_x2 >= 180 && frame_x2 <= 198 {
                                return true;
                            }
                        }
                        90..=180 => {
                            if frame_x2 <= 54 {
                                return true;
                            }
                            if frame_x2 >= 144 && frame_x2 <= 216 {
                                return true;
                            }
                        }
                        _ => {
                            if frame_y2 == 198 && frame_x2 >= 108 && frame_x2 <= 144 {
                                return true;
                            }
                        }
                    }
                }
            }
        }
        false
    }
}

impl Default for WallDrawing {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::draw_context::test_support::with_ctx;

    fn solid(type_: u16, frame_x: i16, frame_y: i16) -> Tile {
        let mut t = Tile::default();
        t.type_ = type_;
        t.set_active(true);
        t.frame_x = frame_x;
        t.frame_y = frame_y;
        t
    }

    #[test]
    fn full_tile_hides_walls_behind_inner_frames_of_solid_blocks() {
        with_ctx(|ctx| {
            let walls = WallDrawing::new();
            // WallDrawing.cs:301-308: not a large-frames tile, frameY 18 with frameX in 18..=54 is an interior frame
            *ctx.g.main.tile.get_mut(10, 10) = solid(1, 18, 18);
            assert!(walls.full_tile(ctx, 10, 10));
            // frameX 0 on the same row is an edge frame
            *ctx.g.main.tile.get_mut(10, 10) = solid(1, 0, 18);
            assert!(!walls.full_tile(ctx, 10, 10));
            // WallDrawing.cs:245: a half brick next to the tile means the wall shows
            *ctx.g.main.tile.get_mut(10, 10) = solid(1, 18, 18);
            ctx.g.main.tile.get_mut(9, 10).set_half_brick(true);
            assert!(!walls.full_tile(ctx, 10, 10));
        });
    }

    #[test]
    fn full_tile_is_false_for_frame_important_tiles() {
        with_ctx(|ctx| {
            let walls = WallDrawing::new();
            // WallDrawing.cs:256: Main.tileFrameImportant (tile 4, a torch) never hides the wall
            *ctx.g.main.tile.get_mut(10, 10) = solid(4, 18, 18);
            assert!(!walls.full_tile(ctx, 10, 10));
        });
    }
}
