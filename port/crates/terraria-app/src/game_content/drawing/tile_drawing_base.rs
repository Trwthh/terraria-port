//! `Terraria.GameContent.Drawing.TileDrawingBase` (GameContent/Drawing/TileDrawingBase.cs)

use terraria_core::xna::Matrix;

use crate::data_structures::sprite_batch_beginner::SpriteBatchBeginner;
use crate::draw_context::DrawContext;
use crate::gfx::{BlendState, SamplerState};

/// `TileDrawingBase.DrawOwnBlacks` (TileDrawingBase.cs:10). Read by `Main.DrawBlack`, `WallDrawing` and `TileDrawing`.
pub const DRAW_OWN_BLACKS: bool = true;

/// The part of `TileDrawingBase` that is state: `batchBeginner`. (`FlushLogData` / `DrawCallLogData` only feed
/// `TimeLogger`, which is not ported.)
#[derive(Clone, Copy, Debug, Default)]
pub struct TileDrawingBase {
    batch_beginner: Option<SpriteBatchBeginner>,
}

impl TileDrawingBase {
    /// `Begin(RasterizerState rasterizer, Matrix transformation)`. `Main.DefaultSamplerState` is `SamplerState.PointClamp`
    /// in the port (main.rs uses the same).
    pub fn begin(&mut self, ctx: &mut DrawContext, transformation: Matrix) {
        let beginner = SpriteBatchBeginner::new(BlendState::AlphaBlend, SamplerState::PointClamp, transformation);
        self.batch_beginner = Some(beginner);
        beginner.begin_tile_batch(ctx.tile_batch);
        beginner.begin(ctx.sprite_batch);
    }

    /// `End()`: the tile batch is flushed first, then the sprite batch.
    pub fn end(&self, ctx: &mut DrawContext) {
        let _num = ctx.tile_batch.end(ctx.list);
        ctx.sprite_batch.end(ctx.list);
    }

    /// `RestartLayeredBatch()`
    pub fn restart_layered_batch(&self, ctx: &mut DrawContext) {
        let _value = ctx.tile_batch.restart(ctx.list);
    }

    /// `RestartSpriteBatch()`
    pub fn restart_sprite_batch(&self, ctx: &mut DrawContext) {
        ctx.sprite_batch.end(ctx.list);
        self.batch_beginner.expect("RestartSpriteBatch before Begin").begin(ctx.sprite_batch);
    }
}
