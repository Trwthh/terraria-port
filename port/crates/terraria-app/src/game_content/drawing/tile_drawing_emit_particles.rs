//! `TileDrawing.DrawTiles_EmitParticles` (TileDrawing.cs:6803-7681): the dust, gore and light each tile type sheds while it is drawn.

use terraria_core::xna::Color;
use terraria_core::Tile;
use terraria_sim::unported;

use crate::draw_context::DrawContext;

use super::tile_drawing::TileDrawing;

impl TileDrawing {
    /// `DrawTiles_EmitParticles(int j, int i, Tile tileCache, ushort typeCache, short tileFrameX, short tileFrameY, Color tileLight)`
    /// (TileDrawing.cs:6803). Every branch spawns a `Dust` or `Gore` (and a few add light for the tile); neither system is ported, so the
    /// whole method is `unported!`: no particle appears, nothing else changes (the method writes only `_rand` and the particle arrays).
    #[allow(clippy::too_many_arguments)]
    pub(super) fn draw_tiles_emit_particles(&mut self, ctx: &mut DrawContext, j: i32, i: i32, tile_cache: &Tile, type_cache: u16, tile_frame_x: i16, tile_frame_y: i16, tile_light: Color) {
        let _ = (ctx, j, i, tile_cache, type_cache, tile_frame_x, tile_frame_y, tile_light);
        unported!("TileDrawing.DrawTiles_EmitParticles");
    }
}
