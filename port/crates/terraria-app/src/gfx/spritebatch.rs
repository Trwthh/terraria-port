//! The hand-ported prototype's `SpriteBatch` (deferred mode only) on top of the host crate's sprite queue.
//! Its vertex arithmetic is the prototype's own (kept bit for bit so its screenshots do not change); the
//! XNA-exact batch with every sort mode is `terraria_host::gfx::SpriteBatch`.

use terraria_core::xna::{Color, Matrix, Rectangle, Vector2};
use terraria_host::gfx::{BlendDesc as BlendState, DrawList, QuadLayout, QueuedSprite, SamplerDesc as SamplerState, SpriteBatcher, TextureId, Topology};

// internal shorthand
type Vec2 = Vector2;

/// `SpriteEffects` flags.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct SpriteEffects(pub u8);
impl SpriteEffects {
    pub const NONE: Self = Self(0);
    pub const FLIP_HORIZONTALLY: Self = Self(1);
    pub const FLIP_VERTICALLY: Self = Self(2);
}

/// A loaded texture: its id in the draw list plus the size the game code reads (`Texture2D.Width/Height`).
#[derive(Clone, Debug)]
pub struct Texture2D {
    pub id: TextureId,
    pub width: i32,
    pub height: i32,
}

/// `Microsoft.Xna.Framework.Graphics.SpriteBatch` in deferred mode.
pub struct SpriteBatch {
    begun: bool,
    blend: BlendState,
    sampler: SamplerState,
    transform: Matrix,
    sprites: SpriteBatcher,
}

impl SpriteBatch {
    pub fn new() -> Self {
        Self { begun: false, blend: BlendState::default(), sampler: SamplerState::default(), transform: Matrix::IDENTITY, sprites: SpriteBatcher::new() }
    }

    /// `Begin(SpriteSortMode.Deferred, blendState, samplerState, ..., transformMatrix)`
    pub fn begin(&mut self, blend: BlendState, sampler: SamplerState, transform: Matrix) {
        assert!(!self.begun, "Begin cannot be called again until End has been successfully called.");
        self.begun = true;
        self.blend = blend;
        self.sampler = sampler;
        self.transform = transform;
    }

    /// `Draw(texture, position, sourceRectangle, color, rotation, origin, scale, effects, layerDepth)`
    #[allow(clippy::too_many_arguments)]
    pub fn draw(&mut self, texture: &Texture2D, position: Vec2, source: Option<Rectangle>, color: Color, rotation: f32, origin: Vec2, scale: Vec2, effects: SpriteEffects) {
        let src = source.unwrap_or(Rectangle::new(0, 0, texture.width, texture.height));
        let size = Vec2::new(src.width as f32 * scale.x, src.height as f32 * scale.y);
        self.push(texture, position, size, src, color, rotation, Vec2::new(origin.x * scale.x, origin.y * scale.y), effects);
    }

    /// `Draw(texture, destinationRectangle, sourceRectangle, color, rotation, origin, effects, layerDepth)`.
    /// With a destination rectangle XNA scales the origin by destination size / source size.
    #[allow(clippy::too_many_arguments)]
    pub fn draw_rect(&mut self, texture: &Texture2D, dest: Rectangle, source: Option<Rectangle>, color: Color, rotation: f32, origin: Vec2, effects: SpriteEffects) {
        let src = source.unwrap_or(Rectangle::new(0, 0, texture.width, texture.height));
        let size = Vec2::new(dest.width as f32, dest.height as f32);
        let o = Vec2::new(
            if src.width != 0 { origin.x * size.x / src.width as f32 } else { 0.0 },
            if src.height != 0 { origin.y * size.y / src.height as f32 } else { 0.0 },
        );
        self.push(texture, Vec2::new(dest.x as f32, dest.y as f32), size, src, color, rotation, o, effects);
    }

    /// `Draw(texture, position, color)`
    pub fn draw_simple(&mut self, texture: &Texture2D, position: Vec2, color: Color) {
        self.draw(texture, position, None, color, 0.0, Vec2::ZERO, Vec2::ONE, SpriteEffects::NONE);
    }

    #[allow(clippy::too_many_arguments)]
    fn push(&mut self, texture: &Texture2D, position: Vec2, size: Vec2, src: Rectangle, color: Color, rotation: f32, origin_px: Vec2, effects: SpriteEffects) {
        assert!(self.begun, "Begin must be called successfully before a Draw can be called.");
        let (sin, cos) = if rotation == 0.0 { (0.0, 1.0) } else { rotation.sin_cos() };
        // corner order: top-left, top-right, bottom-left, bottom-right
        let local = [Vec2::ZERO, Vec2::new(size.x, 0.0), Vec2::new(0.0, size.y), size];
        let mut corners = [Vec2::ZERO; 4];
        for (c, l) in corners.iter_mut().zip(local) {
            let p = Vec2::new(l.x - origin_px.x, l.y - origin_px.y);
            *c = Vec2::new(position.x + p.x * cos - p.y * sin, position.y + p.x * sin + p.y * cos);
        }
        let (tw, th) = (texture.width as f32, texture.height as f32);
        let (mut u0, mut u1) = (src.x as f32 / tw, (src.x + src.width) as f32 / tw);
        let (mut v0, mut v1) = (src.y as f32 / th, (src.y + src.height) as f32 / th);
        if effects.0 & SpriteEffects::FLIP_HORIZONTALLY.0 != 0 {
            std::mem::swap(&mut u0, &mut u1);
        }
        if effects.0 & SpriteEffects::FLIP_VERTICALLY.0 != 0 {
            std::mem::swap(&mut v0, &mut v1);
        }
        self.sprites.push(QueuedSprite {
            texture: texture.id,
            pos: corners,
            uv: [Vec2::new(u0, v0), Vec2::new(u1, v0), Vec2::new(u0, v1), Vec2::new(u1, v1)],
            color,
            depth: 0.0,
            layout: QuadLayout::Strip,
        });
    }

    /// `End()`: emit the queued sprites as draws, one per run of equal texture.
    pub fn end(&mut self, list: &mut DrawList) {
        assert!(self.begun, "Begin must be called before calling End.");
        self.begun = false;
        let (blend, sampler) = (self.blend, self.sampler);
        self.sprites.flush(list, &self.transform, |list, texture, first, count| list.draw_range(texture, blend, sampler, Topology::Triangles, None, first, count));
    }
}

impl Default for SpriteBatch {
    fn default() -> Self {
        Self::new()
    }
}
