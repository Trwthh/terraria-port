//! `Terraria.Graphics.TileBatch` (Graphics/TileBatch.cs): the per-corner-colour quad batcher the tile and wall
//! code draws through, with its layered sorting.
//!
//! The data structures (`_spriteDataQueue`, slices that double in size, batches keyed by layer stack and texture,
//! the recent-layer cache) are ported as they are, because the draw order the game gets depends on them. What is
//! replaced is the GPU upload: the original fills a `DynamicVertexBuffer` and calls `DrawIndexedPrimitives` per
//! batch; here every batch becomes one `DrawList::draw_colored_quads` call, in the same order.
//!
//! Not modelled: the `RasterizerState` argument of `Begin` (the render pass never culls) and the vertex buffer
//! chunking (2048 sprites per upload), which changes the number of draw calls but not the picture.

use std::collections::HashMap;

use terraria_core::xna::{Matrix, Rectangle, Vector2, Vector4};

use crate::gfx::{BlendState, ColoredQuad, Color, DrawList, SamplerState, SpriteEffects, Texture2D, TextureId};
use crate::pending::VertexColors;

/// `TileBatch.SpriteData`
#[derive(Clone, Copy)]
struct SpriteData {
    source: Vector4,
    destination: Vector4,
    origin: Vector2,
    colors: VertexColors,
    rotation: f32,
    effects: SpriteEffects,
}

impl Default for SpriteData {
    fn default() -> Self {
        Self { source: Vector4::ZERO, destination: Vector4::ZERO, origin: Vector2::ZERO, colors: VertexColors::default(), rotation: 0.0, effects: SpriteEffects::NONE }
    }
}

/// `TileBatch.DataSlice`
#[derive(Clone, Copy, Default)]
struct DataSlice {
    start: i32,
    length: i32,
    next: i32,
}

/// `TileBatch.LayerBatch`
#[derive(Clone, Copy, Default)]
struct LayerBatch {
    layer_stack: u32,
    texture: u16,
    head: i32,
    tail: i32,
    length: i32,
    next_sprite: i32,
}

impl LayerBatch {
    /// `SortKey`
    fn sort_key(&self) -> u64 {
        ((self.layer_stack as u64) << 16) | self.texture as u64
    }

    /// `CurrentSliceIsFull`
    fn current_slice_is_full(&self) -> bool {
        if self.length >= 2 {
            return (self.length & (self.length - 1)) == 0;
        }
        false
    }
}

/// `TileBatch.LayerBatchKey`. The original's `Equals` compares only the layer stack and relies on the hash code
/// (`layerStack ^ texture.GetHashCode()`) to tell textures apart; the key here is the pair itself, which is the same
/// unless two textures collide on that hash.
#[derive(Clone, Copy, Default, PartialEq, Eq, Hash)]
struct LayerBatchKey {
    layer_stack: u32,
    texture: Option<TextureId>,
}

/// `TileBatch.RecentLayerCacheEntry`
#[derive(Clone, Copy, Default)]
struct RecentLayerCacheEntry {
    texture: Option<TextureId>,
    batch_index: i32,
}

const CORNER_OFFSET_X: [f32; 4] = [0.0, 1.0, 1.0, 0.0];
const CORNER_OFFSET_Y: [f32; 4] = [0.0, 0.0, 1.0, 1.0];

/// `Terraria.Graphics.TileBatch`
pub struct TileBatch {
    sprite_data_queue: Vec<SpriteData>,
    /// `_spriteTextures` (unlayered mode only)
    sprite_textures: Vec<Option<Texture2D>>,
    queued_sprite_count: i32,
    layered_sorting_enabled: bool,
    batch_data: Vec<DataSlice>,
    batch_data_count: i32,
    batches: Vec<LayerBatch>,
    batch_count: i32,
    next_layer_stack: Option<u32>,
    current_batch_index: i32,
    current_batch_key: LayerBatchKey,
    batch_lookup: HashMap<LayerBatchKey, i32>,
    batch_lookup_cache: Vec<RecentLayerCacheEntry>,
    pass_textures: Vec<Texture2D>,
    pass_texture_count: i32,
    texture_id_lookup: HashMap<TextureId, u16>,
    /// The matrix the underlying `SpriteBatch.Begin` left in the sprite effect (see `Begin`).
    transformation: Matrix,
    /// `_drawCalls`
    draw_calls: i32,
}

/// The `Draw` overloads and `Begin()` are all ported; the tile and wall code uses a few of them
#[allow(dead_code)]
impl TileBatch {
    pub fn new() -> Self {
        Self {
            sprite_data_queue: vec![SpriteData::default(); 2048],
            sprite_textures: vec![None; 2048],
            queued_sprite_count: 0,
            layered_sorting_enabled: false,
            batch_data: vec![DataSlice::default(); 2048],
            batch_data_count: 0,
            batches: vec![LayerBatch::default(); 2048],
            batch_count: 0,
            next_layer_stack: None,
            current_batch_index: 0,
            current_batch_key: LayerBatchKey::default(),
            batch_lookup: HashMap::new(),
            batch_lookup_cache: vec![RecentLayerCacheEntry::default(); 2048],
            pass_textures: Vec::with_capacity(512),
            pass_texture_count: 0,
            texture_id_lookup: HashMap::new(),
            transformation: Matrix::IDENTITY,
            draw_calls: 0,
        }
    }

    /// `Begin(RasterizerState rasterizer, Matrix transformation)`. The original begins and ends a `SpriteBatch` so
    /// the device holds that batch's transformation matrix and blend/sampler state; the tile batch then draws with it.
    pub fn begin(&mut self, transformation: Matrix) {
        self.transformation = transformation;
        assert!(self.queued_sprite_count == 0, "Sprites have already been added before calling Begin");
    }

    /// `Begin()`: `Begin(RasterizerState.CullCounterClockwise, Matrix.Identity)`
    pub fn begin_default(&mut self) {
        self.begin(Matrix::IDENTITY);
    }

    /// `Restart()`
    pub fn restart(&mut self, list: &mut DrawList) -> i32 {
        self.end(list)
    }

    /// `SetLayer(uint layer, byte stack = 0)`
    pub fn set_layer(&mut self, layer: u32, stack: u8) {
        assert!(layer < 16777216, "Max Layer Exceeded");
        self.set_layer_stack((layer << 8) | stack as u32);
    }

    /// `SetLayerStack(uint layerStack)`
    pub fn set_layer_stack(&mut self, layer_stack: u32) {
        if !self.layered_sorting_enabled {
            assert!(self.queued_sprite_count <= 0, "Sprites have already been added before setting the first layer");
            self.layered_sorting_enabled = true;
        }
        self.next_layer_stack = Some(layer_stack);
    }

    /// Reserves the next sprite slot (`GetNextSpriteIndex`) and writes `data` there. A missing texture (logged once
    /// by the caller that looked it up) draws nothing.
    fn queue(&mut self, texture: Option<&Texture2D>, data: SpriteData) {
        let Some(texture) = texture else { return };
        let i = self.get_next_sprite_index(texture) as usize;
        self.sprite_data_queue[i] = data;
    }

    /// `Draw(Texture2D texture, Vector2 position, VertexColors colors)`
    pub fn draw_position(&mut self, texture: Option<&Texture2D>, position: Vector2, colors: VertexColors) {
        let Some(t) = texture else { return };
        let vector = Vector2::new(t.width as f32, t.height as f32); // texture.Size()
        self.queue(texture, SpriteData {
            source: Vector4::new(0.0, 0.0, vector.x, vector.y),
            destination: Vector4::new(position.x, position.y, vector.x, vector.y),
            origin: Vector2::ZERO,
            colors,
            rotation: 0.0,
            effects: SpriteEffects::NONE,
        });
    }

    /// `Draw(Texture2D texture, Vector2 position, Rectangle sourceRectangle, VertexColors colors,
    /// Vector2 origin = default, float scale = 1f, SpriteEffects effects = SpriteEffects.None)`
    #[allow(clippy::too_many_arguments)]
    pub fn draw(&mut self, texture: Option<&Texture2D>, position: Vector2, source_rectangle: Rectangle, colors: VertexColors, origin: Vector2, scale: f32, effects: SpriteEffects) {
        self.queue(texture, SpriteData {
            source: Vector4::new(source_rectangle.x as f32, source_rectangle.y as f32, source_rectangle.width as f32, source_rectangle.height as f32),
            destination: Vector4::new(position.x, position.y, scale * source_rectangle.width as f32, scale * source_rectangle.height as f32),
            origin,
            colors,
            rotation: 0.0,
            effects,
        });
    }

    /// `Draw(Texture2D texture, Vector4 destination, VertexColors colors)`
    pub fn draw_vec4(&mut self, texture: Option<&Texture2D>, destination: Vector4, colors: VertexColors) {
        let Some(t) = texture else { return };
        self.queue(texture, SpriteData {
            source: Vector4::new(0.0, 0.0, t.width as f32, t.height as f32),
            destination,
            origin: Vector2::ZERO,
            colors,
            rotation: 0.0,
            effects: SpriteEffects::NONE,
        });
    }

    /// `Draw(Texture2D texture, Vector4 destination, Rectangle sourceRectangle, VertexColors colors)`
    pub fn draw_vec4_source(&mut self, texture: Option<&Texture2D>, destination: Vector4, source_rectangle: Rectangle, colors: VertexColors) {
        self.queue(texture, SpriteData {
            source: Vector4::new(source_rectangle.x as f32, source_rectangle.y as f32, source_rectangle.width as f32, source_rectangle.height as f32),
            destination,
            origin: Vector2::ZERO,
            colors,
            rotation: 0.0,
            effects: SpriteEffects::NONE,
        });
    }

    /// `Draw(Texture2D texture, Vector4 destination, Rectangle sourceRectangle, VertexColors colors,
    /// Vector2 origin = default, SpriteEffects effects = SpriteEffects.None, float rotation = 0f)`
    #[allow(clippy::too_many_arguments)]
    pub fn draw_vec4_source_origin(&mut self, texture: Option<&Texture2D>, destination: Vector4, source_rectangle: Rectangle, colors: VertexColors, origin: Vector2, effects: SpriteEffects, rotation: f32) {
        self.queue(texture, SpriteData {
            source: Vector4::new(source_rectangle.x as f32, source_rectangle.y as f32, source_rectangle.width as f32, source_rectangle.height as f32),
            destination,
            origin,
            colors,
            rotation,
            effects,
        });
    }

    /// `Draw(Texture2D texture, Vector4 destination, VertexColors colors, Vector2 origin = default,
    /// SpriteEffects effects = SpriteEffects.None, float rotation = 0f)`
    pub fn draw_vec4_origin(&mut self, texture: Option<&Texture2D>, destination: Vector4, colors: VertexColors, origin: Vector2, effects: SpriteEffects, rotation: f32) {
        let Some(t) = texture else { return };
        self.queue(texture, SpriteData {
            source: Vector4::new(0.0, 0.0, t.width as f32, t.height as f32),
            destination,
            origin,
            colors,
            rotation,
            effects,
        });
    }

    /// `Draw(Texture2D texture, Rectangle destinationRectangle, VertexColors colors)`
    pub fn draw_rect(&mut self, texture: Option<&Texture2D>, destination_rectangle: Rectangle, colors: VertexColors) {
        let Some(t) = texture else { return };
        self.queue(texture, SpriteData {
            source: Vector4::new(0.0, 0.0, t.width as f32, t.height as f32),
            destination: Vector4::new(destination_rectangle.x as f32, destination_rectangle.y as f32, destination_rectangle.width as f32, destination_rectangle.height as f32),
            origin: Vector2::ZERO,
            colors,
            rotation: 0.0,
            effects: SpriteEffects::NONE,
        });
    }

    /// `Draw(Texture2D texture, Rectangle destinationRectangle, Rectangle sourceRectangle, VertexColors colors)`
    pub fn draw_rect_source(&mut self, texture: Option<&Texture2D>, destination_rectangle: Rectangle, source_rectangle: Rectangle, colors: VertexColors) {
        self.queue(texture, SpriteData {
            source: Vector4::new(source_rectangle.x as f32, source_rectangle.y as f32, source_rectangle.width as f32, source_rectangle.height as f32),
            destination: Vector4::new(destination_rectangle.x as f32, destination_rectangle.y as f32, destination_rectangle.width as f32, destination_rectangle.height as f32),
            origin: Vector2::ZERO,
            colors,
            rotation: 0.0,
            effects: SpriteEffects::NONE,
        });
    }

    /// `GetNextSpriteIndex(Texture2D texture)`
    fn get_next_sprite_index(&mut self, texture: &Texture2D) -> i32 {
        if self.layered_sorting_enabled {
            let id = texture.id;
            if let Some(value) = self.next_layer_stack {
                if Some(id) != self.current_batch_key.texture || value != self.current_batch_key.layer_stack {
                    self.switch_batch(texture, value);
                }
            } else if Some(id) != self.current_batch_key.texture {
                self.switch_batch(texture, self.current_batch_key.layer_stack.wrapping_add(1));
            }
            self.next_layer_stack = None;
            let mut batch = self.batches[self.current_batch_index as usize];
            let index = self.get_next_sprite_index_in_batch(&mut batch);
            self.batches[self.current_batch_index as usize] = batch;
            return index;
        }
        if self.queued_sprite_count as usize >= self.sprite_data_queue.len() {
            let n = self.sprite_data_queue.len() << 1;
            self.sprite_data_queue.resize(n, SpriteData::default());
        }
        if self.queued_sprite_count as usize >= self.sprite_textures.len() {
            let n = self.sprite_textures.len() << 1;
            self.sprite_textures.resize(n, None);
        }
        self.sprite_textures[self.queued_sprite_count as usize] = Some(texture.clone());
        let i = self.queued_sprite_count;
        self.queued_sprite_count += 1;
        i
    }

    /// `GetNextSpriteIndex(ref LayerBatch layerBatchState)`
    fn get_next_sprite_index_in_batch(&mut self, layer_batch_state: &mut LayerBatch) -> i32 {
        if layer_batch_state.current_slice_is_full() {
            let new_sprite_buffer_slice = self.get_new_sprite_buffer_slice(layer_batch_state.length);
            self.batch_data[layer_batch_state.tail as usize].next = new_sprite_buffer_slice;
            layer_batch_state.tail = new_sprite_buffer_slice;
            layer_batch_state.next_sprite = self.batch_data[new_sprite_buffer_slice as usize].start;
        }
        layer_batch_state.length += 1;
        let i = layer_batch_state.next_sprite;
        layer_batch_state.next_sprite += 1;
        i
    }

    /// `GetNewSpriteBufferSlice(int length)`
    fn get_new_sprite_buffer_slice(&mut self, length: i32) -> i32 {
        if self.batch_data_count as usize == self.batch_data.len() {
            let n = self.batch_data.len() * 2;
            self.batch_data.resize(n, DataSlice::default());
        }
        let num = self.batch_data_count;
        self.batch_data_count += 1;
        self.batch_data[num as usize] = DataSlice { start: self.queued_sprite_count, length, next: 0 };
        self.queued_sprite_count += length;
        while self.queued_sprite_count as usize > self.sprite_data_queue.len() {
            let n = self.sprite_data_queue.len() * 2;
            self.sprite_data_queue.resize(n, SpriteData::default());
        }
        num
    }

    /// `SwitchBatch(Texture2D texture, uint layerStack)`
    fn switch_batch(&mut self, texture: &Texture2D, layer_stack: u32) {
        let id = texture.id;
        let current_batch_key = self.current_batch_key;
        let current_batch_index = self.current_batch_index;
        self.current_batch_key = LayerBatchKey { layer_stack, texture: Some(id) };
        let num = ((layer_stack >> 14) | (layer_stack & 0xFFFF)) as usize;
        if num < self.batch_lookup_cache.len() && self.batch_lookup_cache[num].texture == Some(id) {
            self.current_batch_index = self.batch_lookup_cache[num].batch_index;
        } else if let Some(&index) = self.batch_lookup.get(&self.current_batch_key) {
            self.current_batch_index = index;
        } else {
            self.current_batch_index = 0; // TryGetValue leaves 0 in the out parameter
            self.create_batch(texture);
        }
        let num2 = ((current_batch_key.layer_stack >> 14) | (current_batch_key.layer_stack & 0xFFFF)) as usize;
        if num2 < self.batch_lookup_cache.len() {
            self.batch_lookup_cache[num2] = RecentLayerCacheEntry { texture: current_batch_key.texture, batch_index: current_batch_index };
        }
    }

    /// `CreateBatch()`
    fn create_batch(&mut self, texture: &Texture2D) {
        let id = texture.id;
        let value = match self.texture_id_lookup.get(&id) {
            Some(&v) => v,
            None => {
                let v = self.pass_texture_count as u16;
                self.texture_id_lookup.insert(id, v);
                self.pass_textures.push(texture.clone());
                self.pass_texture_count += 1;
                v
            }
        };
        if self.batch_count as usize == self.batches.len() {
            let n = self.batches.len() * 2;
            self.batches.resize(n, LayerBatch::default());
        }
        let new_sprite_buffer_slice = self.get_new_sprite_buffer_slice(2);
        self.current_batch_index = self.batch_count;
        self.batch_count += 1;
        self.batches[self.current_batch_index as usize] = LayerBatch {
            layer_stack: self.current_batch_key.layer_stack,
            texture: value,
            head: new_sprite_buffer_slice,
            tail: new_sprite_buffer_slice,
            length: 0,
            next_sprite: self.batch_data[new_sprite_buffer_slice as usize].start,
        };
        self.batch_lookup.insert(self.current_batch_key, self.current_batch_index);
    }

    /// `End()`: submits everything queued since `Begin`/`Restart`; returns the number of draw calls.
    pub fn end(&mut self, list: &mut DrawList) -> i32 {
        self.layered_sorting_enabled = false;
        if self.queued_sprite_count == 0 {
            return 0;
        }
        self.draw_calls = 0; // FlushRenderState
        if self.pass_texture_count > 0 {
            self.flush_layered(list);
        } else {
            self.flush(list);
        }
        self.draw_calls
    }

    /// `Flush()`: runs of equal texture in submission order.
    fn flush(&mut self, list: &mut DrawList) {
        let mut texture: Option<Texture2D> = None;
        let mut num = 0;
        for i in 0..self.queued_sprite_count {
            let t = self.sprite_textures[i as usize].clone();
            if t.as_ref().map(|t| t.id) != texture.as_ref().map(|t| t.id) {
                if i > num {
                    self.render_batch(list, texture.as_ref().unwrap(), num, i - num);
                }
                num = i;
                texture = t;
            }
        }
        if let Some(t) = texture.as_ref() {
            self.render_batch(list, t, num, self.queued_sprite_count - num);
        }
        for t in &mut self.sprite_textures[..self.queued_sprite_count as usize] {
            *t = None;
        }
        self.queued_sprite_count = 0;
    }

    /// `RenderBatch(Texture2D texture, SpriteData[] sprites, int offset, int count)`
    fn render_batch(&mut self, list: &mut DrawList, texture: &Texture2D, offset: i32, count: i32) {
        let quads: Vec<ColoredQuad> = (0..count).map(|k| fill_quad(texture, &self.sprite_data_queue[(offset + k) as usize])).collect();
        list.draw_colored_quads(texture.id, BlendState::AlphaBlend, SamplerState::PointClamp, &self.transformation, &quads);
        self.draw_calls += 1;
    }

    /// `FlushLayered()`
    fn flush_layered(&mut self, list: &mut DrawList) {
        self.batches[..self.batch_count as usize].sort_by_key(|b| b.sort_key());
        for i in 0..self.batch_count as usize {
            let layer_batch = self.batches[i];
            let texture = self.pass_textures[layer_batch.texture as usize].clone();
            let mut quads: Vec<ColoredQuad> = Vec::with_capacity(layer_batch.length as usize);
            // FillVertexBuffer(batch, ref currentSlice, ref batchOffset, ref vbCount) walked over the slice chain
            let mut current_slice = self.batch_data[layer_batch.head as usize];
            let mut batch_offset = 0;
            while batch_offset < layer_batch.length {
                if current_slice.length == 0 {
                    current_slice = self.batch_data[current_slice.next as usize];
                }
                let num = (layer_batch.length - batch_offset).min(current_slice.length);
                for k in 0..num {
                    quads.push(fill_quad(&texture, &self.sprite_data_queue[(current_slice.start + k) as usize]));
                }
                batch_offset += num;
                current_slice.start += num;
                current_slice.length -= num;
            }
            list.draw_colored_quads(texture.id, BlendState::AlphaBlend, SamplerState::PointClamp, &self.transformation, &quads);
            self.draw_calls += 1;
        }
        self.queued_sprite_count = 0;
        self.batch_data_count = 0;
        self.batch_count = 0;
        self.batch_lookup.clear();
        for e in &mut self.batch_lookup_cache {
            *e = RecentLayerCacheEntry::default();
        }
        self.pass_textures.clear();
        self.pass_texture_count = 0;
        self.texture_id_lookup.clear();
        self.current_batch_key = LayerBatchKey::default();
    }
}

impl Default for TileBatch {
    fn default() -> Self {
        Self::new()
    }
}

/// One sprite of `FillVertexBuffer(Texture2D texture, SpriteData[] sprites, int offset, int count, int vbSpriteOffset)`
/// (TileBatch.cs:620): corner positions and texture coordinates exactly as computed there.
fn fill_quad(texture: &Texture2D, s: &SpriteData) -> ColoredQuad {
    let num = 1.0_f32 / texture.width as f32;
    let num2 = 1.0_f32 / texture.height as f32;
    let (num3, num4) = if s.rotation != 0.0 { ((s.rotation as f64).cos() as f32, (s.rotation as f64).sin() as f32) } else { (1.0, 0.0) };
    let num5 = s.origin.x / s.source.z;
    let num6 = s.origin.y / s.source.w;
    let mut pos = [Vector2::ZERO; 4];
    let mut uv = [Vector2::ZERO; 4];
    for j in 0..4 {
        let mut num7 = CORNER_OFFSET_X[j];
        let mut num8 = CORNER_OFFSET_Y[j];
        let num9 = (num7 - num5) * s.destination.z;
        let num10 = (num8 - num6) * s.destination.w;
        let x = s.destination.x + num9 * num3 - num10 * num4;
        let y = s.destination.y + num9 * num4 + num10 * num3;
        if s.effects.0 & SpriteEffects::FLIP_VERTICALLY.0 != 0 {
            num8 = 1.0 - num8;
        }
        if s.effects.0 & SpriteEffects::FLIP_HORIZONTALLY.0 != 0 {
            num7 = 1.0 - num7;
        }
        pos[j] = Vector2::new(x, y);
        uv[j] = Vector2::new((s.source.x + num7 * s.source.z) * num, (s.source.y + num8 * s.source.w) * num2);
    }
    let c: [Color; 4] = [s.colors.top_left_color, s.colors.top_right_color, s.colors.bottom_right_color, s.colors.bottom_left_color];
    ColoredQuad { pos, uv, color: c }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tex(list: &mut DrawList, w: i32, h: i32) -> Texture2D {
        Texture2D { id: list.create_texture(w as u32, h as u32, None, false), width: w, height: h }
    }

    fn white() -> VertexColors {
        VertexColors::new(Color::WHITE)
    }

    fn draw_at(b: &mut TileBatch, t: &Texture2D, x: f32) {
        b.draw(Some(t), Vector2::new(x, 0.0), Rectangle::new(0, 0, 16, 16), white(), Vector2::ZERO, 1.0, SpriteEffects::NONE);
    }

    /// x positions of the quads in the order they reached the draw list
    fn order(list: &DrawList) -> Vec<f32> {
        list.vertices().chunks(6).map(|q| q[0].pos[0]).collect()
    }

    #[test]
    fn unlayered_batch_keeps_submission_order() {
        let mut list = DrawList::default();
        list.set_back_buffer_size(100, 100);
        let (a, b) = (tex(&mut list, 64, 64), tex(&mut list, 64, 64));
        let mut tb = TileBatch::new();
        tb.begin_default();
        draw_at(&mut tb, &a, 1.0);
        draw_at(&mut tb, &b, 2.0);
        draw_at(&mut tb, &a, 3.0);
        assert_eq!(tb.end(&mut list), 3);
        assert_eq!(order(&list).len(), 3);
    }

    #[test]
    fn layers_sort_by_layer_stack_then_texture_first_use() {
        let mut list = DrawList::default();
        list.set_back_buffer_size(100, 100);
        let (a, b) = (tex(&mut list, 64, 64), tex(&mut list, 64, 64));
        let mut tb = TileBatch::new();
        tb.begin_default();
        // layer 2 first, then layer 1 (two textures), then layer 2 again with the first texture
        tb.set_layer(2, 0);
        draw_at(&mut tb, &a, 10.0);
        tb.set_layer(1, 0);
        draw_at(&mut tb, &b, 20.0);
        draw_at(&mut tb, &a, 30.0); // same layer, texture changed: layer stack + 1 (sorts after the first batch of layer 1)
        tb.set_layer(2, 0);
        draw_at(&mut tb, &a, 40.0);
        let calls = tb.end(&mut list);
        assert_eq!(calls, 3);
        let xs: Vec<f32> = order(&list).iter().map(|x| ((x + 1.0) / 2.0 * 100.0).round()).collect();
        // layer 1 first (b at 20, then the stack+1 batch with a at 30), then layer 2 (a at 10 and 40 share a batch)
        assert_eq!(xs, vec![20.0, 30.0, 10.0, 40.0]);
    }
}
