//! `TileDrawing.DrawSpecialTilesLegacy` (TileDrawing.cs:8034): the tiles `CacheSpecialDraws_Part1` put on the legacy list
//! (sun altar orb, item frames, food platters, weapon racks, the crystal-butterfly cage). Split out of `tile_drawing.rs` for size.

use terraria_core::xna::{Color, Rectangle, Vector2};
use terraria_sim::light_support::multiply_rgba;
use terraria_sim::unported;

use crate::draw_context::DrawContext;
use crate::gfx::SpriteEffects;
use crate::main_draw::hsl_to_rgb;
use crate::pending::lighting_get_color;

use super::tile_drawing::TileDrawing;

/// `Utils.ToRotationVector2(this float f)`
fn to_rotation_vector2(f: f32) -> Vector2 {
    Vector2::new((f as f64).cos() as f32, (f as f64).sin() as f32)
}

impl TileDrawing {
    /// `DrawSpecialTilesLegacy(Vector2 screenPosition, Vector2 offSet)` (TileDrawing.cs:8034). Everything that reads a `TileEntity`,
    /// an `Item` or `ItemSlot` is `unported!` at its place (those systems are not ported); the tile entities were never found
    /// (`CacheSpecialDraws_Part1` gets -1 from `TileEntityType<T>.Find`), so only the sun altar orb and the cage are reached today.
    pub(super) fn draw_special_tiles_legacy(&mut self, ctx: &mut DrawContext, screen_position: Vector2, off_set: Vector2) {
        if self.special_tiles_count == 0 {
            return;
        }
        self.base.restart_layered_batch(ctx);
        for i in 0..self.special_tiles_count {
            let num = self.special_tile_x[i as usize];
            let num2 = self.special_tile_y[i as usize];
            let tile = ctx.g.main.tile.try_get(num, num2).unwrap_or_default();
            let type_ = tile.type_;
            let frame_x = tile.frame_x;
            let frame_y = tile.frame_y;
            ctx.tile_batch.set_layer(0, 0);
            if type_ == 237 {
                if let Some(sun_orb) = ctx.texture("Images/SunOrb") {
                    let mouse_text_color = ctx.g.main.mouse_text_color as i32;
                    let sun_circle = ctx.state.sun_circle;
                    ctx.sprite_batch.draw(
                        &sun_orb,
                        Vector2::new((num * 16 - screen_position.x as i32) as f32 + 8.0, (num2 * 16 - screen_position.y as i32 - 36) as f32) + off_set,
                        Some(Rectangle::new(0, 0, sun_orb.width, sun_orb.height)),
                        Color::from_ints(mouse_text_color, mouse_text_color, mouse_text_color, 0),
                        sun_circle,
                        Vector2::new((sun_orb.width / 2) as f32, (sun_orb.height / 2) as f32),
                        Vector2::ONE,
                        SpriteEffects::NONE,
                    );
                }
            }
            if type_ == 334 && frame_x >= 5000 {
                unported!("Item.netDefaults / Item.Prefix / TextureAssets.Item (weapon rack item)");
            }
            if type_ == 395 && unported!("TileEntity.TryGetAt<TEItemFrame>", false) {
                unported!("ItemSlot.DrawItemIcon");
            }
            if type_ == 520 && unported!("TileEntity.TryGetAt<TEFoodPlatter>", false) {
                unported!("ItemSlot.GetItemLight / tileBatch.Draw(item)");
            }
            if type_ == 471 && unported!("TileEntity.TryGetAt<TEWeaponsRack>", false) {
                unported!("ItemSlot.DrawItemIcon");
            }
            if type_ == 620 {
                let value3 = ctx.extra(202);
                let num8 = 2;
                ctx.state.critter_cage = true;
                let water_animal_cage_frame = self.get_water_animal_cage_frame(ctx, num, num2, frame_x as i32, frame_y as i32);
                let num9 = 8;
                let num10 = ctx.state.butterfly_cage_frame[num9][water_animal_cage_frame as usize];
                let num11 = 6;
                let num12 = 1.0_f32;
                let source_rectangle3 = Rectangle::new(0, 34 * num10, 32, 32);
                let vector = Vector2::new((num * 16 - screen_position.x as i32) as f32, (num2 * 16 - screen_position.y as i32 + num8) as f32) + off_set;
                ctx.tile_batch.draw(value3.as_ref(), vector, source_rectangle3, Color::from_ints(255, 255, 255, 255).into(), Vector2::ZERO, 1.0, SpriteEffects::NONE);
                for j in 0..num11 {
                    let mut color5 = multiply_rgba(Color::from_ints(127, 127, 127, 0), hsl_to_rgb((ctx.g.main.global_time_wrapped_hourly + j as f32 / num11 as f32) % 1.0, 1.0, 0.5, 255));
                    color5 = color5 * (1.0 - num12 * 0.5);
                    color5.set_a(0);
                    let num13 = 2;
                    let position2 = vector + to_rotation_vector2(j as f32 / num11 as f32 * (std::f32::consts::PI * 2.0)) * (num13 as f32 * num12 + 2.0);
                    ctx.tile_batch.draw(value3.as_ref(), position2, source_rectangle3, color5.into(), Vector2::ZERO, 1.0, SpriteEffects::NONE);
                }
                ctx.tile_batch.draw(value3.as_ref(), vector, source_rectangle3, (Color::from_ints(255, 255, 255, 0) * 0.1_f32).into(), Vector2::ZERO, 1.0, SpriteEffects::NONE);
            }
            let _ = lighting_get_color; // read by the item overlays above (unported)
        }
    }
}
