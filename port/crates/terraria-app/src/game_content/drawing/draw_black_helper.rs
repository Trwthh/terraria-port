//! `Terraria.GameContent.Drawing.DrawBlackHelper` (GameContent/Drawing/DrawBlackHelper.cs)

use terraria_core::xna::{Color, Vector2, Vector4};

use crate::draw_context::DrawContext;
use crate::pending::VertexColors;

/// `DrawBlackHelper`: merges horizontal runs of black tiles into one quad.
#[derive(Clone, Copy, Debug)]
pub struct DrawBlackHelper {
    layer: u32,
    draw_offset: Vector2,
    y: i32,
    start_x: i32,
    end_x: i32,
}

impl DrawBlackHelper {
    /// `new DrawBlackHelper(uint layer, Vector2 drawOffset)`
    pub fn new(layer: u32, draw_offset: Vector2) -> Self {
        Self { layer, draw_offset, y: 0, start_x: 0, end_x: 0 }
    }

    /// `DrawBlack(int x, int y)`
    pub fn draw_black(&mut self, ctx: &mut DrawContext, x: i32, y: i32) {
        if y == self.y && x == self.end_x {
            self.end_x += 1;
            return;
        }
        self.end_strip(ctx);
        self.y = y;
        self.start_x = x;
        self.end_x = x + 1;
    }

    /// `EndStrip()`
    pub fn end_strip(&self, ctx: &mut DrawContext) {
        if self.start_x != self.end_x {
            let screen_position = Vector2::new(ctx.g.main.screen_position.0, ctx.g.main.screen_position.1);
            let vector = Vector2::new((self.start_x << 4) as f32, (self.y << 4) as f32) - screen_position + self.draw_offset;
            ctx.tile_batch.set_layer(self.layer, 0);
            let black_tile = ctx.black_tile();
            ctx.tile_batch.draw_vec4(black_tile.as_ref(), Vector4::new(vector.x, vector.y, ((self.end_x - self.start_x) << 4) as f32, 16.0), VertexColors::new(Color::BLACK));
        }
    }
}
