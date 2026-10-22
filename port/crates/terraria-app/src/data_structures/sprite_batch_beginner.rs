//! `Terraria.DataStructures.SpriteBatchBeginner` (DataStructures/SpriteBatchBeginner.cs)

use terraria_core::xna::Matrix;

use crate::gfx::{BlendState, SamplerState, SpriteBatch};
use crate::graphics::tile_batch::TileBatch;

/// `SpriteBatchBeginner`: the arguments of one `SpriteBatch.Begin`, kept to begin a batch again later.
/// `sortMode` (Deferred), `depthStencilState` (None), `rasterizerState` and `effect` (null) are not modelled: the
/// render pass has one fixed state for each of them.
#[derive(Clone, Copy, Debug)]
pub struct SpriteBatchBeginner {
    blend_state: BlendState,
    sampler_state: SamplerState,
    pub transform_matrix: Matrix,
}

impl SpriteBatchBeginner {
    /// `new SpriteBatchBeginner(sortMode, blendState, samplerState, depthStencilState, rasterizerState, effect, transformMatrix)`
    pub fn new(blend_state: BlendState, sampler_state: SamplerState, transform_matrix: Matrix) -> Self {
        Self { blend_state, sampler_state, transform_matrix }
    }

    /// `Begin(SpriteBatch spriteBatch)`
    pub fn begin(&self, sprite_batch: &mut SpriteBatch) {
        sprite_batch.begin(self.blend_state, self.sampler_state, self.transform_matrix);
    }

    /// `Begin(TileBatch tileBatch)`
    pub fn begin_tile_batch(&self, tile_batch: &mut TileBatch) {
        tile_batch.begin(self.transform_matrix);
    }
}
