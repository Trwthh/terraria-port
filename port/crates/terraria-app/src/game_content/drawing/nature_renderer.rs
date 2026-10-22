//! `Terraria.GameContent.Drawing.NextNatureRenderer` (GameContent/Drawing/NextNatureRenderer.cs) behind `INatureRenderer`:
//! trees, grass and vines are queued while `TileDrawing.PostDrawTiles` runs and drawn once afterwards.

use terraria_core::xna::{Color, Rectangle, Vector2};
use terraria_sim::unported;

use crate::data_structures::sprite_batch_beginner::SpriteBatchBeginner;
use crate::draw_context::DrawContext;
use crate::gfx::{SpriteEffects, Texture2D};
use crate::pending::utils_remap;

/// `SideFlags` (GameContent/Drawing/SideFlags.cs)
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct SideFlags(pub i32);

impl SideFlags {
    pub const NONE: SideFlags = SideFlags(0);
    pub const LEFT: SideFlags = SideFlags(1);
    pub const RIGHT: SideFlags = SideFlags(2);
    pub const TOP: SideFlags = SideFlags(4);
    pub const BOTTOM: SideFlags = SideFlags(8);
}

impl std::ops::BitOr for SideFlags {
    type Output = SideFlags;
    fn bitor(self, rhs: SideFlags) -> SideFlags {
        SideFlags(self.0 | rhs.0)
    }
}

impl std::ops::BitOrAssign for SideFlags {
    fn bitor_assign(&mut self, rhs: SideFlags) {
        self.0 |= rhs.0;
    }
}

/// `DrawData` with a position (not a destination rectangle), as `NextNatureRenderer` builds it
#[derive(Clone, Debug)]
struct DrawData {
    texture: Option<Texture2D>,
    position: Vector2,
    source_rect: Option<Rectangle>,
    color: Color,
    rotation: f32,
    origin: Vector2,
    scale: Vector2,
    effect: SpriteEffects,
}

/// `NextNatureRenderer.Entry`
#[derive(Clone, Debug)]
struct Entry {
    data: DrawData,
    #[allow(dead_code)] // read by the `LitNature` shader path only
    seams: SideFlags,
    #[allow(dead_code)]
    is_glow_mask: bool,
}

/// `NextNatureRenderer`
#[derive(Default)]
pub struct NatureRenderer {
    entries: Vec<Entry>,
}

impl NatureRenderer {
    pub fn new() -> Self {
        Self { entries: Vec::new() }
    }

    /// `DrawNature(Texture2D texture, Vector2 position, Rectangle sourceRectangle, Color color, float rotation, Vector2 origin,
    /// float scale, SpriteEffects effects, float layerDepth, SideFlags seams = SideFlags.None)` (NextNatureRenderer.cs:22)
    #[allow(clippy::too_many_arguments)]
    pub fn draw_nature(&mut self, texture: Option<&Texture2D>, position: Vector2, source_rectangle: Rectangle, color: Color, rotation: f32, origin: Vector2, scale: f32, effects: SpriteEffects, _layer_depth: f32, seams: SideFlags) {
        let seams = seams | get_origin_sides(source_rectangle, origin);
        self.entries.push(Entry {
            data: DrawData { texture: texture.cloned(), position, source_rect: Some(source_rectangle), color, rotation, origin, scale: Vector2::new(scale, scale), effect: effects },
            is_glow_mask: false,
            seams,
        });
    }

    /// `DrawGlowmask(Texture2D texture, Vector2 position, Rectangle? sourceRectangle, Color color, float rotation, Vector2 origin,
    /// float scale, SpriteEffects effects, float layerDepth)` (NextNatureRenderer.cs:64)
    #[allow(clippy::too_many_arguments)]
    pub fn draw_glowmask(&mut self, texture: Option<&Texture2D>, position: Vector2, source_rectangle: Option<Rectangle>, color: Color, rotation: f32, origin: Vector2, scale: f32, effects: SpriteEffects, _layer_depth: f32) {
        self.entries.push(Entry {
            data: DrawData { texture: texture.cloned(), position, source_rect: source_rectangle, color, rotation, origin, scale: Vector2::new(scale, scale), effect: effects },
            is_glow_mask: true,
            seams: SideFlags::NONE,
        });
    }

    /// `DrawAfterAllObjects(SpriteBatchBeginner beginner)` (NextNatureRenderer.cs:70)
    pub fn draw_after_all_objects(&mut self, ctx: &mut DrawContext, beginner: &SpriteBatchBeginner) {
        if self.entries.is_empty() {
            return;
        }
        let mut num = 0.0_f32;
        if ctx.g.main.day_time {
            let from_value = ctx.g.main.time as f32;
            let num2 = 54000.0_f32;
            let val = utils_remap(from_value, 1200.0, 5400.0, 0.0, 1.0) * utils_remap(from_value, 1200.0, 7200.0, 1.0, 0.0) * 0.3;
            let mut num3 = utils_remap(from_value, num2 - 10800.0, num2 - 4200.0, 0.0, 1.0) * utils_remap(from_value, num2 - 1800.0, num2 - 600.0, 1.0, 0.0) * 0.4;
            num3 *= num3;
            let val2 = utils_remap(from_value, 0.0, 7200.0, 0.0, 1.0) * utils_remap(from_value, num2 - 7200.0, num2, 1.0, 0.0) * 0.0;
            num = val2.max(val.max(num3)); // Math.Max(val2: ..., val1: Math.Max(val, num3))
            if unported!("Main.eclipse", false) {
                num = 0.0;
            }
        }
        num *= 0.4;
        let last_celestial_body_position = unported!("Main.LastCelestialBodyPosition", Vector2::ZERO);
        let num4 = utils_remap(last_celestial_body_position.x.min(1.0 - last_celestial_body_position.x), 0.0, 1.0 / 96.0, 0.0, 1.0);
        num *= num4;
        // neutral: no surface background is drawn yet, so the sun-lit shader has nothing to light against
        if !unported!("Main.ShouldDrawSurfaceBackground() && Main.HorizonHelper.SunVisibilityEnabled", false) {
            num = 0.0;
        }
        if num == 0.0 {
            self.draw_without_shader(ctx, beginner);
        } else {
            unported!("NextNatureRenderer.DrawWithLitNatureShader"); // neutral: the plain pass
            self.draw_without_shader(ctx, beginner);
        }
        self.entries.clear();
    }

    /// `DrawWithoutShader(SpriteBatchBeginner beginner, SpriteBatch spriteBatch)` (NextNatureRenderer.cs:106)
    fn draw_without_shader(&mut self, ctx: &mut DrawContext, beginner: &SpriteBatchBeginner) {
        beginner.begin(ctx.sprite_batch);
        for entry in &self.entries {
            // `DrawData.Draw(SpriteBatch sb)` (DrawData.cs): the position overload
            if let Some(t) = entry.data.texture.as_ref() {
                ctx.sprite_batch.draw(t, entry.data.position, entry.data.source_rect, entry.data.color, entry.data.rotation, entry.data.origin, entry.data.scale, entry.data.effect);
            }
        }
        ctx.sprite_batch.end(ctx.list);
    }
}

/// `NextNatureRenderer.GetOriginSides(Rectangle sourceRectangle, Vector2 origin)` (NextNatureRenderer.cs:35)
fn get_origin_sides(source_rectangle: Rectangle, origin: Vector2) -> SideFlags {
    let num = origin.x / source_rectangle.width as f32;
    let num2 = 1.0 - num;
    let num3 = origin.y / source_rectangle.height as f32;
    let num4 = 1.0 - num3;
    let mut side_flags = SideFlags::NONE;
    if (num as f64) < 0.25 {
        side_flags |= SideFlags::LEFT;
    }
    if (num2 as f64) < 0.25 {
        side_flags |= SideFlags::RIGHT;
    }
    if (num3 as f64) < 0.25 {
        side_flags |= SideFlags::TOP;
    }
    if (num4 as f64) < 0.25 {
        side_flags |= SideFlags::BOTTOM;
    }
    side_flags
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn origin_sides_flag_the_edges_the_origin_sits_near() {
        // origin at the bottom centre of an 80 x 80 frame (a tree top): x = 0.5 (no side), y = 1 -> 1 - y = 0 < 0.25 is Bottom
        assert_eq!(get_origin_sides(Rectangle::new(0, 0, 80, 80), Vector2::new(40.0, 80.0)), SideFlags::BOTTOM);
        // origin (0, 30) of a 40 x 40 branch frame: x = 0 -> Left; y = 0.75, 1 - 0.75 = 0.25 is not < 0.25
        assert_eq!(get_origin_sides(Rectangle::new(0, 0, 40, 40), Vector2::new(0.0, 30.0)), SideFlags::LEFT);
        // origin (40, 24) of the right-hand branch: x = 1 -> Right; y = 0.6
        assert_eq!(get_origin_sides(Rectangle::new(0, 0, 40, 40), Vector2::new(40.0, 24.0)), SideFlags::RIGHT);
    }
}
