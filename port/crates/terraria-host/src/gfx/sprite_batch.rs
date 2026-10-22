//! `Microsoft.Xna.Framework.Graphics.SpriteBatch` (reference/xna/SpriteBatch.cs), all sort modes.
//!
//! Follows the reference method by method: `Begin` stores the states (and applies them at once in `Immediate`
//! mode), `Draw` queues a `SpriteInfo` (or renders it at once in `Immediate` mode), `End` applies the states and
//! flushes: sort, then one device draw per run of equal texture, once per pass of the custom effect.

use terraria_core::xna::{Color, Matrix, Rectangle, Vector2};
use terraria_gen_rt::rt::{self, Gc, Marker, Nullable, Trace};

use super::batcher::{xna_sprite, SpriteBatcher, SpriteSortMode};
use super::device::{GraphicsDevice, Texture2D};
use super::draw_list::TextureId;
use super::effect::Effect;
use super::states::{presets, throw, BlendState, DepthStencilState, RasterizerState, SamplerState};
use super::trace;

pub struct SpriteBatch {
    pub device: Gc<GraphicsDevice>,
    sort_mode: SpriteSortMode,
    blend_state: Gc<BlendState>,
    sampler_state: Gc<SamplerState>,
    depth_stencil_state: Gc<DepthStencilState>,
    rasterizer_state: Gc<RasterizerState>,
    custom_effect: Gc<Effect>,
    transform_matrix: Matrix,
    in_begin_end_pair: bool,
    batcher: SpriteBatcher,
    /// the texture objects of the queued sprites (kept alive until the flush; `Textures[0]` is set from here)
    queued_textures: Vec<Gc<Texture2D>>,
    pub disposed: bool,
    /// the draw trace's name of this batch: `sb<trace_id>`
    pub trace_id: u32,
}
// SAFETY: every handle is reported.
unsafe impl Trace for SpriteBatch {
    unsafe fn trace(&self, m: &mut Marker) {
        m.mark(self.device);
        m.mark(self.blend_state);
        m.mark(self.sampler_state);
        m.mark(self.depth_stencil_state);
        m.mark(self.rasterizer_state);
        m.mark(self.custom_effect);
        for t in &self.queued_textures {
            m.mark(*t);
        }
    }
}

impl SpriteBatch {
    /// `new SpriteBatch(graphicsDevice)`
    pub fn new_gc(device: Gc<GraphicsDevice>) -> Gc<SpriteBatch> {
        if device.is_null() {
            throw("System.ArgumentNullException", "graphicsDevice: The GraphicsDevice must not be null when creating new resources.");
        }
        let trace_id = trace::new_sprite_batch_id();
        trace::line(|| format!("sb.new sb{trace_id}"));
        rt::alloc_init(SpriteBatch {
            device,
            sort_mode: SpriteSortMode::Deferred,
            blend_state: Gc::NULL,
            sampler_state: Gc::NULL,
            depth_stencil_state: Gc::NULL,
            rasterizer_state: Gc::NULL,
            custom_effect: Gc::NULL,
            transform_matrix: Matrix::IDENTITY,
            in_begin_end_pair: false,
            batcher: SpriteBatcher::new(),
            queued_textures: Vec::new(),
            disposed: false,
            trace_id,
        })
    }

    pub fn graphics_device(&self) -> Gc<GraphicsDevice> {
        self.device
    }
    pub fn is_disposed(&self) -> bool {
        self.disposed
    }
    pub fn dispose(&mut self) {
        self.disposed = true;
    }

    /// XNA's private `SpriteBatch.spriteQueueCount`: sprites queued since the last flush. The game reads it by
    /// reflection (`Terraria.Graphics.GraphicsUtils.PendingDrawCallCount`).
    pub fn sprite_queue_count(&self) -> i32 {
        self.batcher.len() as i32
    }
    /// XNA's private `SpriteBatch.spriteTextures`: the texture of every queued sprite, in queue order (the game
    /// reads it by reflection together with [`SpriteBatch::sprite_queue_count`]). A new array per call; XNA's own
    /// array is longer than the count and reused, which the game's use (`textures[i]` for `i < count`) cannot see.
    pub fn sprite_textures(&self) -> rt::Arr<Gc<Texture2D>> {
        let v: Vec<Gc<Texture2D>> = self
            .batcher
            .queue()
            .iter()
            // SAFETY: queued textures are alive (traced by this batch) until the flush.
            .map(|s| self.queued_textures.iter().copied().find(|t| unsafe { (*t.ptr()).id } == s.texture).unwrap_or(Gc::NULL))
            .collect();
        rt::Arr::from_slice(&v)
    }

    fn dev<'a>(&self) -> &'a mut GraphicsDevice {
        // SAFETY: the batch keeps its device alive; the reference does not outlive one host call.
        unsafe { &mut *self.device.ptr() }
    }

    /// `Begin()`: Deferred, AlphaBlend, LinearClamp, DepthStencilState.None, CullCounterClockwise, no effect, identity.
    pub fn begin(&mut self) {
        self.begin_impl(0, 0, Gc::NULL, Gc::NULL, Gc::NULL, Gc::NULL, Gc::NULL, Matrix::IDENTITY);
    }
    /// `Begin(sortMode, blendState)`
    pub fn begin_sort_blend(&mut self, sort_mode: i32, blend_state: Gc<BlendState>) {
        self.begin_impl(1, sort_mode, blend_state, Gc::NULL, Gc::NULL, Gc::NULL, Gc::NULL, Matrix::IDENTITY);
    }
    /// `Begin(sortMode, blendState, samplerState, depthStencilState, rasterizerState)`
    pub fn begin_states(&mut self, sort_mode: i32, blend_state: Gc<BlendState>, sampler_state: Gc<SamplerState>, depth_stencil_state: Gc<DepthStencilState>, rasterizer_state: Gc<RasterizerState>) {
        self.begin_impl(2, sort_mode, blend_state, sampler_state, depth_stencil_state, rasterizer_state, Gc::NULL, Matrix::IDENTITY);
    }
    /// `Begin(sortMode, blendState, samplerState, depthStencilState, rasterizerState, effect)`
    pub fn begin_effect(&mut self, sort_mode: i32, blend_state: Gc<BlendState>, sampler_state: Gc<SamplerState>, depth_stencil_state: Gc<DepthStencilState>, rasterizer_state: Gc<RasterizerState>, effect: Gc<Effect>) {
        self.begin_impl(3, sort_mode, blend_state, sampler_state, depth_stencil_state, rasterizer_state, effect, Matrix::IDENTITY);
    }
    /// `Begin(sortMode, blendState, samplerState, depthStencilState, rasterizerState, effect, transformMatrix)`.
    /// A null state means the default listed at [`Self::begin`].
    #[allow(clippy::too_many_arguments)]
    pub fn begin_full(
        &mut self,
        sort_mode: i32,
        blend_state: Gc<BlendState>,
        sampler_state: Gc<SamplerState>,
        depth_stencil_state: Gc<DepthStencilState>,
        rasterizer_state: Gc<RasterizerState>,
        effect: Gc<Effect>,
        transform_matrix: Matrix,
    ) {
        self.begin_impl(4, sort_mode, blend_state, sampler_state, depth_stencil_state, rasterizer_state, effect, transform_matrix);
    }

    /// Every `Begin` overload (`overload` 0..=4 as the draw trace names them: the argument count order above).
    #[allow(clippy::too_many_arguments)]
    fn begin_impl(
        &mut self,
        overload: u32,
        sort_mode: i32,
        blend_state: Gc<BlendState>,
        sampler_state: Gc<SamplerState>,
        depth_stencil_state: Gc<DepthStencilState>,
        rasterizer_state: Gc<RasterizerState>,
        effect: Gc<Effect>,
        transform_matrix: Matrix,
    ) {
        trace::line(|| {
            format!(
                "sb.begin sb{} {overload} {sort_mode} {} {} {} {} {} {}",
                self.trace_id,
                trace::blend(blend_state),
                trace::sampler(sampler_state),
                trace::depth(depth_stencil_state),
                trace::raster(rasterizer_state),
                trace::effect(effect),
                if overload == 4 { trace::matrix(&transform_matrix) } else { "-".to_owned() }
            )
        });
        if self.in_begin_end_pair {
            throw("System.InvalidOperationException", "Begin cannot be called again until End has been successfully called.");
        }
        let Some(mode) = SpriteSortMode::from_i32(sort_mode) else { throw("System.ArgumentOutOfRangeException", "sortMode") };
        self.sort_mode = mode;
        self.batcher.sort_mode = mode;
        self.blend_state = blend_state;
        self.sampler_state = sampler_state;
        self.depth_stencil_state = depth_stencil_state;
        self.rasterizer_state = rasterizer_state;
        self.custom_effect = effect;
        self.transform_matrix = transform_matrix;
        if mode == SpriteSortMode::Immediate {
            if self.dev().sprite_begin_count > 0 {
                throw("System.InvalidOperationException", "Cannot nest Begin calls on a single SpriteBatch when using SpriteSortMode.Immediate.");
            }
            trace::internal(|| self.set_render_state());
            self.dev().sprite_immediate_begin_count += 1;
        } else if self.dev().sprite_immediate_begin_count > 0 {
            throw("System.InvalidOperationException", "Cannot nest Begin calls on a single SpriteBatch when using SpriteSortMode.Immediate.");
        }
        self.dev().sprite_begin_count += 1;
        self.in_begin_end_pair = true;
    }

    /// `End()`
    pub fn end(&mut self) {
        trace::line(|| format!("sb.end sb{}", self.trace_id));
        if !self.in_begin_end_pair {
            throw("System.InvalidOperationException", "Begin must be called before calling End.");
        }
        if self.sort_mode != SpriteSortMode::Immediate {
            trace::internal(|| self.set_render_state());
        } else {
            self.dev().sprite_immediate_begin_count -= 1;
        }
        if !self.batcher.is_empty() {
            trace::internal(|| self.flush());
        }
        self.in_begin_end_pair = false;
        self.dev().sprite_begin_count -= 1;
    }

    /// `SetRenderState`: the four states onto the device and the sprite effect's pass (which replaces any custom
    /// effect pass applied before; the batch's own custom effect is applied per texture run in `flush`).
    fn set_render_state(&mut self) {
        let p = presets();
        let d = self.dev();
        d.set_blend_state(if self.blend_state.is_null() { p.alpha_blend } else { self.blend_state });
        d.set_depth_stencil_state(if self.depth_stencil_state.is_null() { p.depth_none } else { self.depth_stencil_state });
        d.set_rasterizer_state(if self.rasterizer_state.is_null() { p.cull_counter_clockwise } else { self.rasterizer_state });
        d.set_sampler_state(0, if self.sampler_state.is_null() { p.linear_clamp } else { self.sampler_state });
        d.sprite_transform = self.transform_matrix;
        d.applied_effect = None;
    }

    /// `Flush` + `RenderBatch`.
    ///
    /// The vertices are written once and drawn once per pass. Under passes with their own vertex shader they are
    /// raw (pixel position, layer depth, uv, colour): the shader positions them, not SpriteBatch's matrix. A custom
    /// effect whose passes disagree about having a vertex shader would need both forms of the same sprites; no
    /// effect of the game does that and it is reported instead of drawn wrong.
    fn flush(&mut self) {
        let d = self.dev();
        d.pre_sprite_draw();
        // SAFETY: the batch keeps its effect alive; `ptr()` checks null; passes are kept alive by their technique.
        let passes = if self.custom_effect.is_null() { Vec::new() } else { unsafe { (*(*self.custom_effect.ptr()).current_technique().ptr()).passes.clone() } };
        let raw = match passes.first() {
            // SAFETY: as above.
            Some(first) => unsafe {
                let raw = (*first.ptr()).vertex_shader;
                if passes.iter().any(|p| (*p.ptr()).vertex_shader != raw) {
                    terraria_gen_rt::host::unimplemented("Microsoft.Xna.Framework.Graphics.SpriteBatch with a custom effect whose passes mix vertex-shader and fixed-pipeline passes");
                }
                raw
            },
            None => d.applied_effect.as_ref().is_some_and(|e| e.vertex_shader),
        };
        let mut runs: Vec<(TextureId, u32, u32)> = Vec::new();
        if raw {
            self.batcher.flush_raw(&mut d.list, |_, texture, first, count| runs.push((texture, first, count)));
        } else {
            self.batcher.flush(&mut d.list, &self.transform_matrix, |_, texture, first, count| runs.push((texture, first, count)));
        }
        for (texture, first, count) in runs {
            // SAFETY: queued textures are alive (traced by this batch) until cleared below.
            let object = self.queued_textures.iter().copied().find(|t| unsafe { (*t.ptr()).id } == texture).unwrap_or(Gc::NULL);
            if passes.is_empty() {
                d.textures[0] = object;
                let e = d.effect_for_draw();
                d.draw_sprite_range(texture, first, count, e, raw);
            } else {
                for pass in &passes {
                    // SAFETY: passes are kept alive by their technique.
                    unsafe { (*pass.ptr()).apply() };
                    d.textures[0] = object;
                    let e = d.effect_for_draw();
                    d.draw_sprite_range(texture, first, count, e, raw);
                }
            }
        }
        self.queued_textures.clear();
    }

    #[allow(clippy::too_many_arguments)]
    fn internal_draw(&mut self, texture: Gc<Texture2D>, destination: [f32; 4], scale_destination: bool, source: Nullable<Rectangle>, color: Color, rotation: f32, origin: Vector2, effects: i32, depth: f32) {
        if texture.is_null() {
            throw("System.ArgumentNullException", "texture: This method does not accept null for this parameter.");
        }
        if !self.in_begin_end_pair {
            throw("System.InvalidOperationException", "Begin must be called successfully before a Draw can be called.");
        }
        // SAFETY: checked non-null.
        let t = unsafe { &*texture.ptr() };
        let src = if source.has_value { Some(source.value) } else { None };
        self.batcher.push(xna_sprite(t.id, (t.width, t.height), destination, scale_destination, src, color, rotation, origin, effects, depth));
        if !self.queued_textures.contains(&texture) {
            self.queued_textures.push(texture);
        }
        if self.sort_mode == SpriteSortMode::Immediate {
            trace::internal(|| self.flush());
        }
    }

    /// `Draw(texture, position, color)`
    pub fn draw_pos(&mut self, texture: Gc<Texture2D>, position: Vector2, color: Color) {
        trace::line(|| format!("sb.draw sb{} p {} {} {}", self.trace_id, trace::tex(texture), trace::v2(position), trace::color(color)));
        self.internal_draw(texture, [position.x, position.y, 1.0, 1.0], true, crate::gfx::states::none(), color, 0.0, Vector2::ZERO, 0, 0.0);
    }
    /// `Draw(texture, position, sourceRectangle, color)`
    pub fn draw_pos_src(&mut self, texture: Gc<Texture2D>, position: Vector2, source: Nullable<Rectangle>, color: Color) {
        trace::line(|| format!("sb.draw sb{} ps {} {} {} {}", self.trace_id, trace::tex(texture), trace::v2(position), trace::nrect(source), trace::color(color)));
        self.internal_draw(texture, [position.x, position.y, 1.0, 1.0], true, source, color, 0.0, Vector2::ZERO, 0, 0.0);
    }
    /// `Draw(texture, position, sourceRectangle, color, rotation, origin, float scale, effects, layerDepth)`
    #[allow(clippy::too_many_arguments)]
    pub fn draw_scale(&mut self, texture: Gc<Texture2D>, position: Vector2, source: Nullable<Rectangle>, color: Color, rotation: f32, origin: Vector2, scale: f32, effects: i32, layer_depth: f32) {
        trace::line(|| {
            format!(
                "sb.draw sb{} pf {} {} {} {} {} {} {} {effects} {}",
                self.trace_id,
                trace::tex(texture),
                trace::v2(position),
                trace::nrect(source),
                trace::color(color),
                trace::f(rotation),
                trace::v2(origin),
                trace::f(scale),
                trace::f(layer_depth)
            )
        });
        self.internal_draw(texture, [position.x, position.y, scale, scale], true, source, color, rotation, origin, effects, layer_depth);
    }
    /// `Draw(texture, position, sourceRectangle, color, rotation, origin, Vector2 scale, effects, layerDepth)`
    #[allow(clippy::too_many_arguments)]
    pub fn draw_scale2(&mut self, texture: Gc<Texture2D>, position: Vector2, source: Nullable<Rectangle>, color: Color, rotation: f32, origin: Vector2, scale: Vector2, effects: i32, layer_depth: f32) {
        trace::line(|| {
            format!(
                "sb.draw sb{} pv {} {} {} {} {} {} {} {effects} {}",
                self.trace_id,
                trace::tex(texture),
                trace::v2(position),
                trace::nrect(source),
                trace::color(color),
                trace::f(rotation),
                trace::v2(origin),
                trace::v2(scale),
                trace::f(layer_depth)
            )
        });
        self.internal_draw(texture, [position.x, position.y, scale.x, scale.y], true, source, color, rotation, origin, effects, layer_depth);
    }
    /// `Draw(texture, destinationRectangle, color)`
    pub fn draw_rect(&mut self, texture: Gc<Texture2D>, destination: Rectangle, color: Color) {
        trace::line(|| format!("sb.draw sb{} r {} {} {}", self.trace_id, trace::tex(texture), trace::rect(destination), trace::color(color)));
        self.internal_draw(texture, rect4(destination), false, crate::gfx::states::none(), color, 0.0, Vector2::ZERO, 0, 0.0);
    }
    /// `Draw(texture, destinationRectangle, sourceRectangle, color)`
    pub fn draw_rect_src(&mut self, texture: Gc<Texture2D>, destination: Rectangle, source: Nullable<Rectangle>, color: Color) {
        trace::line(|| format!("sb.draw sb{} rs {} {} {} {}", self.trace_id, trace::tex(texture), trace::rect(destination), trace::nrect(source), trace::color(color)));
        self.internal_draw(texture, rect4(destination), false, source, color, 0.0, Vector2::ZERO, 0, 0.0);
    }
    /// `Draw(texture, destinationRectangle, sourceRectangle, color, rotation, origin, effects, layerDepth)`
    #[allow(clippy::too_many_arguments)]
    pub fn draw_rect_full(&mut self, texture: Gc<Texture2D>, destination: Rectangle, source: Nullable<Rectangle>, color: Color, rotation: f32, origin: Vector2, effects: i32, layer_depth: f32) {
        trace::line(|| {
            format!(
                "sb.draw sb{} rf {} {} {} {} {} {} {effects} {}",
                self.trace_id,
                trace::tex(texture),
                trace::rect(destination),
                trace::nrect(source),
                trace::color(color),
                trace::f(rotation),
                trace::v2(origin),
                trace::f(layer_depth)
            )
        });
        self.internal_draw(texture, rect4(destination), false, source, color, rotation, origin, effects, layer_depth);
    }
}

fn rect4(r: Rectangle) -> [f32; 4] {
    [r.x as f32, r.y as f32, r.width as f32, r.height as f32]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gfx::draw_list::{BlendDesc, Command, CullMode, IRect, SamplerDesc, DISCARD_COLOR};
    use crate::gfx::effect::{Effect, EffectDesc};
    use terraria_gen_rt::rt::Str;

    fn setup() -> (Gc<GraphicsDevice>, Gc<SpriteBatch>, Gc<Texture2D>, Gc<Texture2D>) {
        let d = GraphicsDevice::new_gc(200, 100);
        let sb = SpriteBatch::new_gc(d);
        let a = Texture2D::from_rgba(d, 2, 2, vec![255; 16], false);
        let b = Texture2D::from_rgba(d, 4, 4, vec![255; 64], false);
        (d, sb, a, b)
    }

    fn draws(cmds: &[Command]) -> Vec<(u32, u32, u32)> {
        cmds.iter().filter_map(|c| if let Command::Draw { texture, first_vertex, vertex_count, .. } = c { Some((texture.0, *first_vertex, *vertex_count)) } else { None }).collect()
    }

    #[test]
    fn deferred_draws_at_end_in_texture_runs_with_default_states() {
        let (d, sb, a, b) = setup();
        unsafe {
            let (dv, s) = (&mut *d.ptr(), &mut *sb.ptr());
            s.begin();
            s.draw_pos(a, Vector2::new(0.0, 0.0), Color::WHITE);
            s.draw_pos(a, Vector2::new(10.0, 0.0), Color::WHITE);
            s.draw_pos(b, Vector2::new(20.0, 0.0), Color::WHITE);
            assert!(draws(dv.list.commands()).is_empty(), "deferred: nothing is drawn before End");
            s.end();
            assert_eq!(draws(dv.list.commands()), [(1, 0, 12), (2, 12, 6)]);
            // the back buffer was dirty: the discard colour is cleared in before the first draw
            assert!(dv.list.commands().iter().any(|c| matches!(c, Command::Clear(c) if *c == DISCARD_COLOR)));
            let Some(Command::Draw { blend, sampler, cull, .. }) = dv.list.commands().iter().find(|c| matches!(c, Command::Draw { .. })) else { panic!() };
            assert_eq!((*blend, *sampler, *cull), (BlendDesc::AlphaBlend, SamplerDesc::LinearClamp, CullMode::CullCounterClockwiseFace));
            assert_eq!(dv.textures[0], b, "Textures[0] is the last batch's texture");
            // 2x2 sprite at (10, 0) in a 200x100 viewport
            assert_eq!(dv.list.vertices()[6].pos, [10.0 / 200.0 * 2.0 - 1.0, 1.0]);
            assert_eq!(dv.list.vertices()[8].pos, [12.0 / 200.0 * 2.0 - 1.0, 1.0 - 2.0 / 100.0 * 2.0]);
        }
    }

    #[test]
    fn immediate_draws_each_sprite_at_once_and_forbids_nesting() {
        let (d, sb, a, b) = setup();
        unsafe {
            let (dv, s) = (&mut *d.ptr(), &mut *sb.ptr());
            let p = presets();
            s.begin_full(1, p.additive, p.point_clamp, Gc::NULL, Gc::NULL, Gc::NULL, Matrix::IDENTITY);
            assert_eq!(dv.blend_state, p.additive, "Immediate applies the states in Begin");
            s.draw_pos(a, Vector2::ZERO, Color::WHITE);
            assert_eq!(draws(dv.list.commands()), [(1, 0, 6)]);
            s.draw_pos(b, Vector2::ZERO, Color::WHITE);
            assert_eq!(draws(dv.list.commands()).len(), 2);
            let other = SpriteBatch::new_gc(d);
            let e = rt::try_(|| (*other.ptr()).begin()).unwrap_err();
            assert!(rt::exc_is(e, "System.InvalidOperationException"));
            s.end();
            assert_eq!(draws(dv.list.commands()).len(), 2);
            let e = rt::try_(|| s.end()).unwrap_err();
            assert!(rt::exc_is(e, "System.InvalidOperationException"));
            let e = rt::try_(|| s.draw_pos(a, Vector2::ZERO, Color::WHITE)).unwrap_err();
            assert!(rt::exc_is(e, "System.InvalidOperationException"));
        }
    }

    #[test]
    fn sorted_modes_reorder_the_queue() {
        let (d, sb, a, b) = setup();
        unsafe {
            let (dv, s) = (&mut *d.ptr(), &mut *sb.ptr());
            // BackToFront: depth 0.9 (texture b) first, then 0.5 (a), then 0.1 (b)
            s.begin_sort_blend(3, Gc::NULL);
            s.draw_scale(b, Vector2::ZERO, crate::gfx::states::none(), Color::WHITE, 0.0, Vector2::ZERO, 1.0, 0, 0.1);
            s.draw_scale(a, Vector2::ZERO, crate::gfx::states::none(), Color::WHITE, 0.0, Vector2::ZERO, 1.0, 0, 0.5);
            s.draw_scale(b, Vector2::ZERO, crate::gfx::states::none(), Color::WHITE, 0.0, Vector2::ZERO, 1.0, 0, 0.9);
            s.end();
            assert_eq!(draws(dv.list.commands()).iter().map(|d| d.0).collect::<Vec<_>>(), [2, 1, 2]);
            let _ = dv.present();
            // Texture: one run per texture
            s.begin_sort_blend(2, Gc::NULL);
            for t in [a, b, a, b, a] {
                s.draw_pos(t, Vector2::ZERO, Color::WHITE);
            }
            s.end();
            assert_eq!(draws(dv.list.commands()), [(2, 0, 12), (1, 12, 18)]);
        }
    }

    #[test]
    fn scissor_viewport_render_target_and_effect_are_recorded() {
        let (d, sb, a, _) = setup();
        unsafe {
            let (dv, s) = (&mut *d.ptr(), &mut *sb.ptr());
            let rt2 = Texture2D::new_render_target(d, 64, 32, false, 0, 0, 0);
            dv.set_render_target(rt2);
            assert_eq!(dv.viewport(), crate::gfx::device::Viewport::new(0, 0, 64, 32));
            dv.clear(Color::TRANSPARENT);
            let scissor = RasterizerState::new_gc();
            (*scissor.ptr()).set_cull_mode(0);
            (*scissor.ptr()).set_scissor_test_enable(true);
            dv.set_scissor_rectangle(Rectangle::new(4, 4, 8, 8));
            let fx = Effect::new_gc(d, "PixelShader", &EffectDesc { techniques: vec![("T".into(), vec!["P0".into(), "P1".into()])], parameters: vec!["uOpacity".into()], ..Default::default() });
            (*(*fx.ptr()).parameter(Str::lit("uOpacity")).ptr()).set_float(0.5);
            s.begin_full(0, Gc::NULL, Gc::NULL, Gc::NULL, scissor, fx, Matrix::IDENTITY);
            s.draw_pos(a, Vector2::ZERO, Color::WHITE);
            s.end();
            dv.set_render_target(Gc::NULL);
            let cmds = dv.list.commands().to_vec();
            let kinds: Vec<String> = cmds
                .iter()
                .filter_map(|c| match c {
                    Command::SetTarget { target, preserve } => Some(format!("target {} {}", target.0, preserve)),
                    Command::Clear(c) => Some(format!("clear {}", c.a())),
                    Command::Scissor(s) => Some(format!("scissor {:?}", s.map(|r| (r.x, r.y, r.w, r.h)))),
                    Command::Draw { effect, cull, .. } => Some(format!("draw {} {:?}", effect.as_ref().map(|e| e.pass.clone()).unwrap_or_default(), cull)),
                    _ => None,
                })
                .collect();
            // the explicit Clear replaced the discard clear; the custom effect draws the run once per pass
            assert_eq!(kinds, ["target 3 true", "clear 0", "scissor Some((4, 4, 8, 8))", "draw P0 None", "draw P1 None", "target 0 true", "scissor Some((4, 4, 8, 8))"]);
            let Command::Draw { effect: Some(e), .. } = &cmds[cmds.len() - 4] else { panic!("{:?}", cmds) };
            assert_eq!(e.params[0].1, crate::gfx::draw_list::ParamValue::Floats(vec![0.5]));
            assert_eq!(dv.list.scissor(), Some(IRect { x: 4, y: 4, w: 8, h: 8 }));
            // binding the same target again is a no-op
            let n = dv.list.commands().len();
            dv.set_render_target(Gc::NULL);
            assert_eq!(dv.list.commands().len(), n);
        }
    }

    #[test]
    fn discard_and_preserve_follow_xna() {
        let (d, sb, a, _) = setup();
        unsafe {
            let (dv, s) = (&mut *d.ptr(), &mut *sb.ptr());
            let discard = Texture2D::new_render_target(d, 8, 8, false, 0, 0, 0);
            let preserve = Texture2D::new_render_target(d, 8, 8, false, 0, 0, 1);
            let clears = |dv: &GraphicsDevice| dv.list.commands().iter().filter(|c| matches!(c, Command::Clear(c) if *c == DISCARD_COLOR)).count();
            // a discard target that is bound and left without a draw is still cleared when another target is bound
            dv.set_render_target(discard);
            dv.set_render_target(preserve);
            assert_eq!(clears(dv), 1);
            // a preserve target is never cleared by binding
            s.begin();
            s.draw_pos(a, Vector2::ZERO, Color::WHITE);
            s.end();
            assert_eq!(clears(dv), 1);
            // back to the back buffer (DiscardContents by default): dirty again, cleared at Present
            dv.set_render_target(Gc::NULL);
            let f = dv.present();
            assert!(matches!(f.commands.last(), Some(Command::Clear(c)) if *c == DISCARD_COLOR));
            // Present with a target bound throws
            dv.set_render_target(preserve);
            let e = rt::try_(|| dv.present()).unwrap_err();
            assert!(rt::exc_is(e, "System.InvalidOperationException"));
        }
    }
}
