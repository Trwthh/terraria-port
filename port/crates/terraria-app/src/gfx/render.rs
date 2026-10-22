//! The render-world half: replays the host crate's [`Frame`]s with wgpu render passes.
//!
//! Commands are executed strictly in order. Draws on one target between two state breaks share a render pass;
//! a texture upload between draws ends the pass and is submitted in sequence, so `Texture2D.SetData` in the
//! middle of a frame is seen by the draws after it and not by the ones before.
//!
//! # Effects
//! A draw that carries an `EffectSnapshot` of a compiled effect runs that pass's Direct3D 9 shaders, translated to
//! WGSL by `terraria-shaders` the first time the pass is drawn:
//! - `Command::CreateEffect` brings the D3DX effect file; `fx_binary::parse_full` gives its passes and shader blobs;
//! - pipelines are cached by (effect, technique, pass, blend state, topology, cull mode); every target is
//!   `Rgba8Unorm`, so the target format is not part of the key;
//! - the float registers come from the snapshot's parameter values through the shader's constant table and its
//!   preshader (`uniforms::fill`), per draw, into a per-frame uniform buffer bound with dynamic offsets;
//! - sampler register 0 is the draw's texture and `SamplerStates[0]`, registers 1..3 are the snapshot's
//!   `Textures[1..3]` with their sampler states; a register with no texture samples opaque black, as Direct3D 9
//!   does for a null texture;
//! - a pass with a vertex shader takes the frame's raw vertices and runs the translated vertex shader, which adds
//!   Direct3D 9's pixel-centre offset for the current viewport; a pass without one takes the sprite vertices
//!   (SpriteBatch's vertex stage, done on the CPU by the host);
//! - the blend / cull / scissor state of the draw applies unchanged: the game's passes assign no render state
//!   (`terraria-host/src/gfx/effect.rs`); a pass that assigns anything but shaders is refused.
//! A pass that cannot be translated, or whose parameters do not fit its constant table, is reported once with the
//! reason and its draws are NOT drawn (nothing is substituted).
//!
//! Not executed here (logged once):
//! - an effect built without a file (an `EffectSnapshot` with no `effect_id`): drawn with the plain sprite shader;
//! - `Command::Readback` (the asynchronous request / response variant) is still not served: logged once.
//!
//! # Synchronous readback (`Texture2D.GetData` on a render target)
//! The GPU state ([`GfxGpu`]) lives behind `Arc<Mutex<..>>` ([`SharedGpu`]) and is held by both worlds, and the frames
//! waiting to be replayed are one shared queue ([`PublishedFrames`]). Whoever holds the GPU lock replays everything in
//! the queue, in order: the render world once per Bevy frame, [`WgpuReadback`] (the host's `readback_hook`, called on
//! the game thread in the middle of `Main.Draw`) when the game reads a target. The hook replays the frames still queued,
//! then its own partial frame, copies the target into a mapped buffer and waits for the GPU. The state a partial frame
//! leaves (bound target, viewport, scissor, pending discard) is carried into the next frame ([`Carry`]); the back buffer
//! view of the last render is cached so a partial frame can still draw into the back buffer.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use bevy::asset::RenderAssetUsages;
use bevy::image::Image;
use bevy::prelude::*;
use bevy::render::render_asset::RenderAssets;
use bevy::render::renderer::{RenderDevice, RenderQueue};
use bevy::render::texture::GpuImage;
use bevy::render::{Extract, ExtractSchedule, Render, RenderApp, RenderSystems};
use terraria_host::gfx::fx_binary::{self, FxEffect, StateClass};
use terraria_host::gfx::{Blend, BlendDesc, BlendFunction, Command, CullMode, DrawList, EffectId, EffectSnapshot, Frame, IRect, ParamValue, RawVertex, SamplerDesc, TextureAddressMode, TextureId, Topology, Vertex};

use terraria_shaders as shaders;
use shaders::bytecode::Shader;
use shaders::wgsl::Translated;

/// Format every target is rendered in. No sRGB conversion anywhere, like XNA.
const TARGET_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

/// The back-buffer image. Stored as sRGB so Bevy's own pipeline (sRGB -> linear on sample, linear ->
/// sRGB on the swap chain) shows the stored bytes unchanged; rendered through an `Rgba8Unorm` view.
pub fn new_target_image(width: u32, height: u32) -> Image {
    let mut image = Image::new_target_texture(width.max(1), height.max(1), wgpu::TextureFormat::Rgba8UnormSrgb, None);
    // Bevy samples the default (sRGB) view; this module renders through the Rgba8Unorm view
    image.texture_descriptor.view_formats = &[TARGET_FORMAT];
    image.asset_usage = RenderAssetUsages::RENDER_WORLD | RenderAssetUsages::MAIN_WORLD;
    image
}

/// The recorder of the hand-ported prototype (the translated game records through `terraria_host`'s `GraphicsDevice`,
/// whose frames are handed to [`PublishedFrames`] the same way).
#[derive(Resource, Default)]
pub struct DrawListRes(pub DrawList);

/// The Bevy image that is `TextureId::BACK_BUFFER`.
#[derive(Resource)]
pub struct BackBufferImage(pub Handle<Image>);

/// Frames waiting for the render world, oldest first. Anything that records a `Frame` pushes it here.
#[derive(Resource, Default, Clone)]
pub struct PublishedFrames(pub Arc<Mutex<Vec<Arc<Frame>>>>);

impl PublishedFrames {
    pub fn push(&self, frame: Frame) {
        if !frame.commands.is_empty() {
            self.0.lock().unwrap_or_else(|p| p.into_inner()).push(Arc::new(frame));
        }
    }
}

pub struct GfxPlugin;

impl Plugin for GfxPlugin {
    fn build(&self, app: &mut App) {
        let shared = SharedGpu::default();
        app.init_resource::<DrawListRes>().init_resource::<PublishedFrames>().insert_resource(shared.clone()).add_systems(Last, publish_draw_list);
        let published = app.world().resource::<PublishedFrames>().clone();
        let Some(render_app) = app.get_sub_app_mut(RenderApp) else { return };
        render_app
            .init_resource::<ExtractedFrames>()
            .insert_resource(shared)
            .insert_resource(published)
            .add_systems(ExtractSchedule, extract_back_buffer)
            // after assets are on the GPU, before the main graph samples the back-buffer image
            .add_systems(Render, replay_frames.in_set(RenderSystems::Queue));
    }
}

fn publish_draw_list(mut list: ResMut<DrawListRes>, out: Res<PublishedFrames>) {
    out.push(list.0.take_frame());
}

#[derive(Resource, Default)]
struct ExtractedFrames {
    back_buffer: Option<Handle<Image>>,
}

fn extract_back_buffer(back: Extract<Option<Res<BackBufferImage>>>, mut out: ResMut<ExtractedFrames>) {
    out.back_buffer = back.as_ref().map(|b| b.0.clone());
}

/// The GPU state of the replay, shared by the render world and the readback hook (see the module docs).
#[derive(Resource, Clone, Default)]
pub struct SharedGpu(Arc<Mutex<GfxGpu>>);

impl SharedGpu {
    /// True once the render world has seen the back-buffer image on the GPU (the game can start drawing into it).
    pub fn has_back_buffer(&self) -> bool {
        self.0.lock().unwrap_or_else(|p| p.into_inner()).back.is_some()
    }
}

/// The back-buffer image as the render world last saw it.
struct BackView {
    view: wgpu::TextureView,
    width: u32,
    height: u32,
}

/// What a partial frame (a readback in the middle of a frame) leaves for the frame that continues it.
#[derive(Clone, Copy)]
struct Carry {
    target: TextureId,
    discard: bool,
    viewport: Option<IRect>,
    scissor: Option<IRect>,
}

struct GpuTexture {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    width: u32,
    height: u32,
}

struct Shared {
    shader: wgpu::ShaderModule,
    layout: wgpu::BindGroupLayout,
    pipeline_layout: wgpu::PipelineLayout,
}

/// (effect, technique index, pass index)
type PassKey = (EffectId, u32, u32);

/// One translated effect pass.
struct PassGpu {
    pixel: Shader,
    vertex: Option<Shader>,
    translated: Translated,
    module: wgpu::ShaderModule,
    textures_layout: wgpu::BindGroupLayout,
    uniforms_layout: wgpu::BindGroupLayout,
    pipeline_layout: wgpu::PipelineLayout,
}

struct EffectGpu {
    name: String,
    /// `Err`: the file could not be parsed (reported when it arrived)
    fx: Result<FxEffect, String>,
}

/// Uniform data of the effect draws of a frame: one buffer, a region per draw, bound with dynamic offsets.
#[derive(Default)]
struct UniformArena {
    buffer: Option<wgpu::Buffer>,
    capacity: u64,
    used: u64,
    /// changes when the buffer is replaced (bind groups of the old buffer are stale)
    generation: u64,
}

impl UniformArena {
    /// Copies `data` into the arena; returns the offset.
    fn push(&mut self, device: &wgpu::Device, queue: &RenderQueue, data: &[u8]) -> u32 {
        let align = device.limits().min_uniform_buffer_offset_alignment as u64;
        let start = self.used.next_multiple_of(align);
        // a binding's range must lie inside the buffer for the largest uniform block of any pass (keep slack)
        if self.buffer.is_none() || start + data.len() as u64 + 4096 > self.capacity {
            self.capacity = (self.capacity * 2).max(1 << 18);
            self.buffer = Some(device.create_buffer(&wgpu::BufferDescriptor { label: Some("effect uniforms"), size: self.capacity, usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false }));
            self.generation += 1;
            self.used = 0;
            return self.push(device, queue, data);
        }
        queue.write_buffer(self.buffer.as_ref().expect("created above"), start, data);
        self.used = start + data.len() as u64;
        start as u32
    }
}

#[derive(Default)]
pub struct GfxGpu {
    shared: Option<Shared>,
    pipelines: HashMap<(BlendDesc, Topology, CullMode), wgpu::RenderPipeline>,
    samplers: HashMap<SamplerDesc, wgpu::Sampler>,
    vertex_buffer: Option<wgpu::Buffer>,
    vertex_capacity: u64,
    raw_vertex_buffer: Option<wgpu::Buffer>,
    raw_vertex_capacity: u64,
    textures: HashMap<TextureId, GpuTexture>,
    bind_groups: HashMap<(TextureId, SamplerDesc), wgpu::BindGroup>,
    effects: HashMap<EffectId, EffectGpu>,
    /// `None`: the pass could not be translated (reported once)
    effect_passes: HashMap<PassKey, Option<Arc<PassGpu>>>,
    effect_pipelines: HashMap<(PassKey, BlendDesc, Topology, CullMode), wgpu::RenderPipeline>,
    effect_texture_groups: HashMap<(PassKey, Vec<(TextureId, SamplerDesc)>), wgpu::BindGroup>,
    effect_uniform_groups: HashMap<(PassKey, u64), wgpu::BindGroup>,
    uniforms: UniformArena,
    /// what Direct3D 9 samples from a sampler with no texture: opaque black
    null_texture: Option<wgpu::TextureView>,
    /// passes whose parameters did not fit the constant table (reported once each)
    reported_passes: Vec<PassKey>,
    warned_effects: bool,
    warned_readback: bool,
    back: Option<BackView>,
    carry: Option<Carry>,
}

/// The texture id standing for "no texture bound" in bind group cache keys.
const NO_TEXTURE: TextureId = TextureId(u32::MAX);

const SHADER: &str = r#"
struct VOut { @builtin(position) pos: vec4<f32>, @location(0) uv: vec2<f32>, @location(1) color: vec4<f32> };
@vertex fn vs(@location(0) pos: vec2<f32>, @location(1) uv: vec2<f32>, @location(2) color: vec4<f32>) -> VOut {
    var o: VOut;
    o.pos = vec4<f32>(pos, 0.0, 1.0);
    o.uv = uv;
    o.color = color;
    return o;
}
@group(0) @binding(0) var tex: texture_2d<f32>;
@group(0) @binding(1) var samp: sampler;
// SpriteBatch's effect: texture * vertex colour
@fragment fn fs(i: VOut) -> @location(0) vec4<f32> {
    return textureSample(tex, samp, i.uv) * i.color;
}
"#;

fn blend_factor(b: Blend) -> wgpu::BlendFactor {
    use wgpu::BlendFactor as F;
    match b {
        Blend::One => F::One,
        Blend::Zero => F::Zero,
        Blend::SourceColor => F::Src,
        Blend::InverseSourceColor => F::OneMinusSrc,
        Blend::SourceAlpha => F::SrcAlpha,
        Blend::InverseSourceAlpha => F::OneMinusSrcAlpha,
        Blend::DestinationColor => F::Dst,
        Blend::InverseDestinationColor => F::OneMinusDst,
        Blend::DestinationAlpha => F::DstAlpha,
        Blend::InverseDestinationAlpha => F::OneMinusDstAlpha,
        Blend::BlendFactor => F::Constant,
        Blend::InverseBlendFactor => F::OneMinusConstant,
        Blend::SourceAlphaSaturation => F::SrcAlphaSaturated,
    }
}

fn blend_component(src: Blend, dst: Blend, f: BlendFunction) -> wgpu::BlendComponent {
    use wgpu::BlendOperation as O;
    let operation = match f {
        BlendFunction::Add => O::Add,
        BlendFunction::Subtract => O::Subtract,
        BlendFunction::ReverseSubtract => O::ReverseSubtract,
        BlendFunction::Min => O::Min,
        BlendFunction::Max => O::Max,
    };
    // Direct3D ignores the factors for Min / Max; wgpu requires them to be One
    if matches!(f, BlendFunction::Min | BlendFunction::Max) {
        return wgpu::BlendComponent { src_factor: wgpu::BlendFactor::One, dst_factor: wgpu::BlendFactor::One, operation };
    }
    wgpu::BlendComponent { src_factor: blend_factor(src), dst_factor: blend_factor(dst), operation }
}

fn build_shared(device: &wgpu::Device) -> Shared {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("spritebatch"), source: wgpu::ShaderSource::Wgsl(SHADER.into()) });
    let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("spritebatch texture"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture { sample_type: wgpu::TextureSampleType::Float { filterable: true }, view_dimension: wgpu::TextureViewDimension::D2, multisampled: false },
                count: None,
            },
            wgpu::BindGroupLayoutEntry { binding: 1, visibility: wgpu::ShaderStages::FRAGMENT, ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering), count: None },
        ],
    });
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: Some("spritebatch"), bind_group_layouts: &[Some(&layout)], immediate_size: 0 });
    Shared { shader, layout, pipeline_layout }
}

fn build_pipeline(device: &wgpu::Device, shared: &Shared, blend: BlendDesc, topology: Topology, cull: CullMode) -> wgpu::RenderPipeline {
    let attributes = wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x2, 2 => Unorm8x4];
    let blend_state = wgpu::BlendState { color: blend_component(blend.color_src, blend.color_dst, blend.color_fn), alpha: blend_component(blend.alpha_src, blend.alpha_dst, blend.alpha_fn) };
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("spritebatch"),
        layout: Some(&shared.pipeline_layout),
        vertex: wgpu::VertexState {
            module: &shared.shader,
            entry_point: Some("vs"),
            compilation_options: Default::default(),
            buffers: &[wgpu::VertexBufferLayout { array_stride: size_of::<Vertex>() as u64, step_mode: wgpu::VertexStepMode::Vertex, attributes: &attributes }],
        },
        fragment: Some(wgpu::FragmentState {
            module: &shared.shader,
            entry_point: Some("fs"),
            compilation_options: Default::default(),
            targets: &[Some(wgpu::ColorTargetState { format: TARGET_FORMAT, blend: Some(blend_state), write_mask: wgpu::ColorWrites::from_bits_truncate(blend.write_mask as u32) })],
        }),
        // XNA judges winding on screen (y down): clockwise triangles are front faces. wgpu judges it the same way
        // (framebuffer coordinates), so front_face is Cw and "cull clockwise faces" removes the front.
        // Checked with host_demo section 5: under CullCounterClockwise a normal sprite shows and a mirrored one
        // (negative X scale) does not.
        primitive: wgpu::PrimitiveState {
            topology: match topology {
                Topology::Triangles => wgpu::PrimitiveTopology::TriangleList,
                Topology::Lines => wgpu::PrimitiveTopology::LineList,
            },
            front_face: wgpu::FrontFace::Cw,
            cull_mode: match cull {
                CullMode::None => None,
                CullMode::CullClockwiseFace => Some(wgpu::Face::Front),
                CullMode::CullCounterClockwiseFace => Some(wgpu::Face::Back),
            },
            ..Default::default()
        },
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        multiview_mask: None,
        cache: None,
    })
}

/// The current value of an effect parameter as flat floats (what `shaders::uniforms::fill` asks for).
fn parameter_floats(e: &EffectSnapshot, name: &str) -> Option<Vec<f32>> {
    match &e.params.iter().find(|(n, _)| n == name)?.1 {
        ParamValue::Floats(f) => Some(f.clone()),
        // D3DX converts: `SetValue(bool)` / `SetValue(int)` on a parameter the shader keeps in a float register
        ParamValue::Bool(b) => Some(vec![*b as u8 as f32]),
        ParamValue::Int(i) => Some(vec![*i as f32]),
        ParamValue::Unset | ParamValue::Texture(_) => None,
    }
}

/// Translate one pass of an effect and build what does not depend on the draw.
fn build_pass(device: &wgpu::Device, effect: &EffectGpu, technique: u32, pass: u32) -> Result<PassGpu, String> {
    let fx = effect.fx.as_ref().map_err(|e| format!("the effect file could not be parsed: {e}"))?;
    let t = fx.techniques.get(technique as usize).ok_or("no such technique")?;
    let p = t.passes.get(pass as usize).ok_or("no such pass")?;
    if let Some(s) = p.states.iter().find(|s| !matches!(s.class(), StateClass::PixelShader | StateClass::VertexShader)) {
        return Err(format!("the pass assigns the state {}[{}], which this backend does not apply (only shaders are supported)", s.name(), s.index));
    }
    let pixel = shaders::bytecode::parse(p.pixel_shader().ok_or("the pass has no compiled pixel shader")?)?;
    let vertex = match p.vertex_shader() {
        Some(b) => Some(shaders::bytecode::parse(b)?),
        None => None,
    };
    let label = format!("{}.{}[{pass}] {}", effect.name, t.name, p.name);
    let translated = shaders::wgsl::translate(&label, &pixel, vertex.as_ref())?;
    let module = device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some(&label), source: wgpu::ShaderSource::Wgsl(translated.source.as_str().into()) });
    let mut entries = Vec::new();
    for unit in &translated.samplers {
        entries.push(wgpu::BindGroupLayoutEntry {
            binding: unit * 2,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture { sample_type: wgpu::TextureSampleType::Float { filterable: true }, view_dimension: wgpu::TextureViewDimension::D2, multisampled: false },
            count: None,
        });
        entries.push(wgpu::BindGroupLayoutEntry { binding: unit * 2 + 1, visibility: wgpu::ShaderStages::FRAGMENT, ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering), count: None });
    }
    let textures_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor { label: Some("effect textures"), entries: &entries });
    let uniform = |binding, visibility| wgpu::BindGroupLayoutEntry { binding, visibility, ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: true, min_binding_size: None }, count: None };
    let mut entries = Vec::new();
    if translated.ps_registers > 0 {
        entries.push(uniform(0, wgpu::ShaderStages::FRAGMENT));
    }
    if translated.vs_registers.is_some() {
        entries.push(uniform(1, wgpu::ShaderStages::VERTEX));
    }
    let uniforms_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor { label: Some("effect uniforms"), entries: &entries });
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: Some(&label), bind_group_layouts: &[Some(&textures_layout), Some(&uniforms_layout)], immediate_size: 0 });
    Ok(PassGpu { pixel, vertex, translated, module, textures_layout, uniforms_layout, pipeline_layout })
}

fn build_effect_pipeline(device: &wgpu::Device, pass: &PassGpu, blend: BlendDesc, topology: Topology, cull: CullMode) -> wgpu::RenderPipeline {
    let sprite_attributes = wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x2, 2 => Unorm8x4];
    let raw_attributes = wgpu::vertex_attr_array![0 => Float32x4, 1 => Float32x4, 2 => Unorm8x4];
    let (stride, attributes): (usize, &[wgpu::VertexAttribute]) = if pass.vertex.is_some() { (size_of::<RawVertex>(), &raw_attributes) } else { (size_of::<Vertex>(), &sprite_attributes) };
    let blend_state = wgpu::BlendState { color: blend_component(blend.color_src, blend.color_dst, blend.color_fn), alpha: blend_component(blend.alpha_src, blend.alpha_dst, blend.alpha_fn) };
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("effect pass"),
        layout: Some(&pass.pipeline_layout),
        vertex: wgpu::VertexState {
            module: &pass.module,
            entry_point: Some("vs"),
            compilation_options: Default::default(),
            buffers: &[wgpu::VertexBufferLayout { array_stride: stride as u64, step_mode: wgpu::VertexStepMode::Vertex, attributes }],
        },
        fragment: Some(wgpu::FragmentState {
            module: &pass.module,
            entry_point: Some("fs"),
            compilation_options: Default::default(),
            targets: &[Some(wgpu::ColorTargetState { format: TARGET_FORMAT, blend: Some(blend_state), write_mask: wgpu::ColorWrites::from_bits_truncate(blend.write_mask as u32) })],
        }),
        // the same winding rule as the sprite pipeline (see `build_pipeline`)
        primitive: wgpu::PrimitiveState {
            topology: match topology {
                Topology::Triangles => wgpu::PrimitiveTopology::TriangleList,
                Topology::Lines => wgpu::PrimitiveTopology::LineList,
            },
            front_face: wgpu::FrontFace::Cw,
            cull_mode: match cull {
                CullMode::None => None,
                CullMode::CullClockwiseFace => Some(wgpu::Face::Front),
                CullMode::CullCounterClockwiseFace => Some(wgpu::Face::Back),
            },
            ..Default::default()
        },
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        multiview_mask: None,
        cache: None,
    })
}

/// Everything one effect draw binds.
struct EffectDraw {
    key: PassKey,
    pipeline: wgpu::RenderPipeline,
    textures: wgpu::BindGroup,
    uniforms: wgpu::BindGroup,
    offsets: Vec<u32>,
    raw: bool,
}

impl GfxGpu {
    /// Prepare a draw under an effect pass. `None`: the draw cannot be made (the reason has been reported).
    #[allow(clippy::too_many_arguments)]
    fn effect_draw(&mut self, device: &wgpu::Device, queue: &RenderQueue, e: &EffectSnapshot, id: EffectId, texture: TextureId, sampler: SamplerDesc, blend: BlendDesc, topology: Topology, cull: CullMode, target: TextureId, viewport: (f32, f32)) -> Option<EffectDraw> {
        let key: PassKey = (id, e.technique_index, e.pass_index);
        if !self.effect_passes.contains_key(&key) {
            let built = match self.effects.get(&id) {
                Some(effect) => build_pass(device, effect, e.technique_index, e.pass_index),
                None => Err("the effect was never created in the draw list".into()),
            };
            let built = match built {
                Ok(p) => Some(Arc::new(p)),
                Err(err) => {
                    eprintln!("[gfx] ERROR: effect {} technique {} pass {} ({}) cannot be executed; its draws are skipped: {err}", e.effect, e.technique, e.pass, e.pass_index);
                    None
                }
            };
            self.effect_passes.insert(key, built);
        }
        let pass = self.effect_passes.get(&key)?.clone()?;

        // ---- uniforms: parameters -> registers (constant table + preshader)
        let lookup = |name: &str| parameter_floats(e, name);
        let filled = shaders::uniforms::fill(&pass.pixel, &lookup).and_then(|ps| match &pass.vertex {
            Some(vs) => shaders::uniforms::fill(vs, &lookup).map(|v| (ps, Some(v))),
            None => Ok((ps, None)),
        });
        let (ps_regs, vs_regs) = match filled {
            Ok(r) => r,
            Err(err) => {
                if !self.reported_passes.contains(&key) {
                    self.reported_passes.push(key);
                    eprintln!("[gfx] ERROR: effect {} pass {}: the parameter values do not fit the shader's constants; its draws are skipped: {err}", e.effect, e.pass);
                }
                return None;
            }
        };
        let mut offsets = Vec::new();
        if pass.translated.ps_registers > 0 {
            offsets.push(self.uniforms.push(device, queue, bytemuck::cast_slice(&ps_regs)));
        }
        if let (Some(count), Some(mut regs)) = (pass.translated.vs_registers, vs_regs) {
            regs.resize(count as usize, [0.0; 4]);
            let mut data = vec![[1.0 / viewport.0, -1.0 / viewport.1, 0.0, 0.0]];
            data.extend(regs);
            offsets.push(self.uniforms.push(device, queue, bytemuck::cast_slice(&data)));
        }
        let generation = self.uniforms.generation;
        if !self.effect_uniform_groups.contains_key(&(key, generation)) {
            self.effect_uniform_groups.retain(|(k, g), _| *k != key || *g == generation);
            let buffer = self.uniforms.buffer.get_or_insert_with(|| device.create_buffer(&wgpu::BufferDescriptor { label: Some("effect uniforms"), size: 4096, usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false }));
            let region = |size: u64| wgpu::BindingResource::Buffer(wgpu::BufferBinding { buffer, offset: 0, size: std::num::NonZeroU64::new(size) });
            let mut entries = Vec::new();
            if pass.translated.ps_registers > 0 {
                entries.push(wgpu::BindGroupEntry { binding: 0, resource: region(pass.translated.ps_registers as u64 * 16) });
            }
            if let Some(count) = pass.translated.vs_registers {
                entries.push(wgpu::BindGroupEntry { binding: 1, resource: region((count as u64 + 1) * 16) });
            }
            let group = device.create_bind_group(&wgpu::BindGroupDescriptor { label: Some("effect uniforms"), layout: &pass.uniforms_layout, entries: &entries });
            self.effect_uniform_groups.insert((key, generation), group);
        }
        let uniforms = self.effect_uniform_groups.get(&(key, generation))?.clone();

        // ---- textures: sampler register 0 is the draw's texture, 1..3 come from Textures[1..3]
        let mut bound: Vec<(TextureId, SamplerDesc)> = Vec::new();
        for unit in &pass.translated.samplers {
            let entry = if *unit == 0 { Some((texture, sampler)) } else { e.extra_textures.iter().find(|(slot, _, _)| *slot as u32 == *unit).map(|(_, t, s)| (*t, *s)) };
            match entry {
                // a target cannot be sampled while it is bound (XNA throws; nothing is drawn here)
                Some((t, _)) if t == target => return None,
                Some((t, s)) if self.textures.contains_key(&t) => bound.push((t, s)),
                _ => bound.push((NO_TEXTURE, SamplerDesc::PointClamp)),
            }
        }
        let group_key = (key, bound);
        if !self.effect_texture_groups.contains_key(&group_key) {
            let null = self
                .null_texture
                .get_or_insert_with(|| {
                    let t = device.create_texture(&wgpu::TextureDescriptor {
                        label: Some("null texture"),
                        size: wgpu::Extent3d { width: 1, height: 1, depth_or_array_layers: 1 },
                        mip_level_count: 1,
                        sample_count: 1,
                        dimension: wgpu::TextureDimension::D2,
                        format: TARGET_FORMAT,
                        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                        view_formats: &[],
                    });
                    queue.write_texture(
                        wgpu::TexelCopyTextureInfo { texture: &t, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
                        &[0, 0, 0, 255],
                        wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(4), rows_per_image: Some(1) },
                        wgpu::Extent3d { width: 1, height: 1, depth_or_array_layers: 1 },
                    );
                    t.create_view(&wgpu::TextureViewDescriptor::default())
                })
                .clone();
            for (_, s) in &group_key.1 {
                self.samplers.entry(*s).or_insert_with(|| build_sampler(device, *s));
            }
            let mut entries = Vec::new();
            for (unit, (t, s)) in pass.translated.samplers.iter().zip(&group_key.1) {
                let view = self.textures.get(t).map_or(&null, |g| &g.view);
                entries.push(wgpu::BindGroupEntry { binding: unit * 2, resource: wgpu::BindingResource::TextureView(view) });
                entries.push(wgpu::BindGroupEntry { binding: unit * 2 + 1, resource: wgpu::BindingResource::Sampler(&self.samplers[s]) });
            }
            let group = device.create_bind_group(&wgpu::BindGroupDescriptor { label: Some("effect textures"), layout: &pass.textures_layout, entries: &entries });
            self.effect_texture_groups.insert(group_key.clone(), group);
        }
        let textures = self.effect_texture_groups.get(&group_key)?.clone();

        let pipeline = self.effect_pipelines.entry((key, blend, topology, cull)).or_insert_with(|| build_effect_pipeline(device, &pass, blend, topology, cull)).clone();
        Some(EffectDraw { key, pipeline, textures, uniforms, offsets, raw: pass.vertex.is_some() })
    }
}

fn build_sampler(device: &wgpu::Device, s: SamplerDesc) -> wgpu::Sampler {
    let address = |a| match a {
        TextureAddressMode::Wrap => wgpu::AddressMode::Repeat,
        TextureAddressMode::Clamp => wgpu::AddressMode::ClampToEdge,
        TextureAddressMode::Mirror => wgpu::AddressMode::MirrorRepeat,
    };
    let (min_linear, mag_linear) = s.filter.min_mag_linear();
    let f = |linear| if linear { wgpu::FilterMode::Linear } else { wgpu::FilterMode::Nearest };
    device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("spritebatch"),
        address_mode_u: address(s.address_u),
        address_mode_v: address(s.address_v),
        address_mode_w: wgpu::AddressMode::ClampToEdge,
        mag_filter: f(mag_linear),
        min_filter: f(min_linear),
        ..Default::default()
    })
}

/// Wall-clock milliseconds of every [`replay_frames`] run that had frames to replay: the CPU side of the GPU replay
/// (uploads, pipeline look-ups, pass encoding, submit), measured on the render thread. Printed by the shell at exit
/// with `TERRARIA_FRAME_TIMES=1`. What the GPU itself needs afterwards is not in it.
pub static REPLAY_MS: std::sync::Mutex<Vec<f32>> = std::sync::Mutex::new(Vec::new());

fn replay_frames(extracted: Res<ExtractedFrames>, published: Res<PublishedFrames>, shared: Res<SharedGpu>, images: Res<RenderAssets<GpuImage>>, render_device: Res<RenderDevice>, render_queue: Res<RenderQueue>) {
    let t0 = std::time::Instant::now();
    let had_frames = !published.0.lock().unwrap_or_else(|p| p.into_inner()).is_empty();
    let device = render_device.wgpu_device();
    // lock order: the GPU first, the queue only for the moment of taking it (the readback hook does the same)
    let mut gpu = shared.0.lock().unwrap_or_else(|p| p.into_inner());
    if let Some(g) = extracted.back_buffer.as_ref().and_then(|h| images.get(h)) {
        let view = (*g.texture.create_view(&wgpu::TextureViewDescriptor { format: Some(TARGET_FORMAT), ..Default::default() })).clone();
        gpu.back = Some(BackView { view, width: g.texture.width(), height: g.texture.height() });
    }
    replay_published(&mut gpu, &published, device, &render_queue);
    if had_frames {
        if let Ok(mut v) = REPLAY_MS.lock() {
            v.push(t0.elapsed().as_secs_f32() * 1000.0);
        }
    }
}

/// Replays every queued frame, oldest first.
fn replay_published(gpu: &mut GfxGpu, published: &PublishedFrames, device: &wgpu::Device, queue: &RenderQueue) {
    let frames = std::mem::take(&mut *published.0.lock().unwrap_or_else(|p| p.into_inner()));
    for frame in frames {
        replay_frame(&frame, gpu, device, queue, false);
    }
}

fn clamp_rect(r: IRect, w: u32, h: u32) -> (u32, u32, u32, u32) {
    let x0 = r.x.clamp(0, w as i32);
    let y0 = r.y.clamp(0, h as i32);
    let x1 = (r.x.saturating_add(r.w)).clamp(x0, w as i32);
    let y1 = (r.y.saturating_add(r.h)).clamp(y0, h as i32);
    (x0 as u32, y0 as u32, (x1 - x0) as u32, (y1 - y0) as u32)
}

/// `partial`: the frame ends where a readback was asked, not at a `Present`; the target / viewport / scissor state goes to the next frame.
fn replay_frame(frame: &Frame, gpu: &mut GfxGpu, device: &wgpu::Device, queue: &RenderQueue, partial: bool) {
    if gpu.shared.is_none() {
        gpu.shared = Some(build_shared(device));
    }

    // vertices for the whole frame in one buffer
    let bytes: &[u8] = bytemuck::cast_slice(&frame.vertices);
    if gpu.vertex_buffer.is_none() || gpu.vertex_capacity < bytes.len() as u64 {
        gpu.vertex_capacity = (bytes.len() as u64).next_power_of_two().max(1 << 16);
        gpu.vertex_buffer = Some(device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("spritebatch vertices"),
            size: gpu.vertex_capacity,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        }));
    }
    let Some(vertex_buffer) = gpu.vertex_buffer.clone() else { return };
    if !bytes.is_empty() {
        queue.write_buffer(&vertex_buffer, 0, bytes);
    }
    // raw vertices (draws under effect passes with a vertex shader)
    let raw_bytes: &[u8] = bytemuck::cast_slice(&frame.raw_vertices);
    if !raw_bytes.is_empty() {
        if gpu.raw_vertex_buffer.is_none() || gpu.raw_vertex_capacity < raw_bytes.len() as u64 {
            gpu.raw_vertex_capacity = (raw_bytes.len() as u64).next_power_of_two().max(1 << 14);
            gpu.raw_vertex_buffer = Some(device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("effect raw vertices"),
                size: gpu.raw_vertex_capacity,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }));
        }
        if let Some(b) = &gpu.raw_vertex_buffer {
            queue.write_buffer(b, 0, raw_bytes);
        }
    }
    let raw_vertex_buffer = gpu.raw_vertex_buffer.clone();
    gpu.uniforms.used = 0;

    let mut encoder: Option<wgpu::CommandEncoder> = None;
    let carry = gpu.carry.take();
    let mut target = carry.map_or(TextureId::BACK_BUFFER, |c| c.target);
    // the target was bound with DiscardContents and nothing has been drawn into it yet
    let mut discard = carry.is_some_and(|c| c.discard);
    let mut viewport: Option<IRect> = carry.and_then(|c| c.viewport);
    let mut scissor: Option<IRect> = carry.and_then(|c| c.scissor);
    let cmds = &frame.commands;
    let mut i = 0;
    while i < cmds.len() {
        match &cmds[i] {
            Command::CreateTexture { id, width, height, target: is_target } => {
                let mut usage = wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST;
                if *is_target {
                    usage |= wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC;
                }
                // wgpu textures start zeroed: transparent black, like a new XNA texture
                let texture = device.create_texture(&wgpu::TextureDescriptor {
                    label: Some("host texture"),
                    size: wgpu::Extent3d { width: (*width).max(1), height: (*height).max(1), depth_or_array_layers: 1 },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: TARGET_FORMAT,
                    usage,
                    view_formats: &[],
                });
                let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
                gpu.textures.insert(*id, GpuTexture { texture, view, width: *width, height: *height });
                i += 1;
            }
            Command::Upload { id, rect, rgba } => {
                // draws recorded before the upload must see the old contents
                if let Some(e) = encoder.take() {
                    queue.submit([e.finish()]);
                }
                if let Some(t) = gpu.textures.get(id) {
                    queue.write_texture(
                        wgpu::TexelCopyTextureInfo { texture: &t.texture, mip_level: 0, origin: wgpu::Origin3d { x: rect.x as u32, y: rect.y as u32, z: 0 }, aspect: wgpu::TextureAspect::All },
                        rgba,
                        wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(rect.w as u32 * 4), rows_per_image: Some(rect.h as u32) },
                        wgpu::Extent3d { width: rect.w as u32, height: rect.h as u32, depth_or_array_layers: 1 },
                    );
                }
                i += 1;
            }
            Command::DestroyTexture { id } => {
                gpu.textures.remove(id);
                gpu.bind_groups.retain(|(t, _), _| t != id);
                gpu.effect_texture_groups.retain(|(_, bound), _| !bound.iter().any(|(t, _)| t == id));
                i += 1;
            }
            Command::CreateEffect { id, name, bytecode } => {
                let fx = fx_binary::parse_full(bytecode);
                if let Err(e) = &fx {
                    eprintln!("[gfx] ERROR: effect {name}: {e}");
                }
                gpu.effects.insert(*id, EffectGpu { name: name.clone(), fx });
                i += 1;
            }
            Command::SetTarget { target: t, preserve } => {
                target = *t;
                discard = !*preserve;
                viewport = None;
                scissor = None;
                i += 1;
            }
            Command::Viewport(v) => {
                viewport = Some(*v);
                i += 1;
            }
            Command::Scissor(s) => {
                scissor = *s;
                i += 1;
            }
            Command::Readback { .. } => {
                if !gpu.warned_readback {
                    gpu.warned_readback = true;
                    eprintln!("[gfx] render target readback (Texture2D.GetData on a target) is not implemented in the wgpu replay");
                }
                i += 1;
            }
            Command::Clear(_) | Command::Draw { .. } => {
                let mut load = if discard { wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT) } else { wgpu::LoadOp::Load };
                discard = false;
                if let Command::Clear(c) = &cmds[i] {
                    load = wgpu::LoadOp::Clear(wgpu::Color { r: c.r() as f64 / 255.0, g: c.g() as f64 / 255.0, b: c.b() as f64 / 255.0, a: c.a() as f64 / 255.0 });
                    i += 1;
                }
                // the pass runs until a command that is not a draw or a viewport / scissor change
                let end = cmds[i..].iter().position(|c| !matches!(c, Command::Draw { .. } | Command::Viewport(_) | Command::Scissor(_))).map_or(cmds.len(), |n| i + n);
                let (view, tw, th) = if target == TextureId::BACK_BUFFER {
                    match gpu.back.as_ref().filter(|b| (b.width, b.height) == frame.back_buffer) {
                        Some(b) => (b.view.clone(), frame.back_buffer.0, frame.back_buffer.1),
                        None => {
                            i = end;
                            continue;
                        }
                    }
                } else {
                    match gpu.textures.get(&target) {
                        Some(t) => (t.view.clone(), t.width, t.height),
                        None => {
                            i = end;
                            continue;
                        }
                    }
                };
                let enc = encoder.get_or_insert_with(|| device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("spritebatch") }));
                let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("spritebatch"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment { view: &view, depth_slice: None, resolve_target: None, ops: wgpu::Operations { load, store: wgpu::StoreOp::Store } })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                    multiview_mask: None,
                });
                pass.set_vertex_buffer(0, vertex_buffer.slice(..));
                // returns the size of the viewport in pixels (the vertex shaders of effect passes need it); `None`: empty
                let apply_viewport = |pass: &mut wgpu::RenderPass, v: Option<IRect>| {
                    let (x, y, w, h) = clamp_rect(v.unwrap_or(IRect { x: 0, y: 0, w: tw as i32, h: th as i32 }), tw, th);
                    if w > 0 && h > 0 {
                        pass.set_viewport(x as f32, y as f32, w as f32, h as f32, 0.0, 1.0);
                        Some((w as f32, h as f32))
                    } else {
                        None
                    }
                };
                let apply_scissor = |pass: &mut wgpu::RenderPass, s: Option<IRect>| {
                    let (x, y, w, h) = clamp_rect(s.unwrap_or(IRect { x: 0, y: 0, w: tw as i32, h: th as i32 }), tw, th);
                    pass.set_scissor_rect(x, y, w, h);
                    w > 0 && h > 0
                };
                let mut viewport_size = apply_viewport(&mut pass, viewport);
                let mut viewport_ok = viewport_size.is_some();
                let mut scissor_ok = if scissor.is_some() { apply_scissor(&mut pass, scissor) } else { true };
                // the pipeline that is set: `None` for the sprite pipeline, the pass for an effect pipeline
                let mut bound: Option<(Option<PassKey>, BlendDesc, Topology, CullMode)> = None;
                let mut raw_bound = false;
                for cmd in &cmds[i..end] {
                    match cmd {
                        Command::Viewport(v) => {
                            viewport = Some(*v);
                            viewport_size = apply_viewport(&mut pass, viewport);
                            viewport_ok = viewport_size.is_some();
                        }
                        Command::Scissor(s) => {
                            scissor = *s;
                            scissor_ok = apply_scissor(&mut pass, scissor);
                        }
                        Command::Draw { texture, blend, sampler, cull, topology, effect, first_vertex, vertex_count, raw } => {
                            if !(viewport_ok && scissor_ok) {
                                continue;
                            }
                            // a target cannot be sampled while it is bound (XNA throws; nothing is drawn here)
                            if *texture == target {
                                continue;
                            }
                            if let Some((e, id)) = effect.as_ref().and_then(|e| e.effect_id.map(|id| (e, id))) {
                                let Some(size) = viewport_size else { continue };
                                let Some(draw) = gpu.effect_draw(device, queue, e, id, *texture, *sampler, *blend, *topology, *cull, target, size) else { continue };
                                if draw.raw != *raw {
                                    // the host records raw vertices exactly for passes with a vertex shader
                                    eprintln!("[gfx] ERROR: effect {} pass {}: vertex format of the draw does not match the pass; skipped", e.effect, e.pass);
                                    continue;
                                }
                                if bound != Some((Some(draw.key), *blend, *topology, *cull)) {
                                    pass.set_pipeline(&draw.pipeline);
                                    let f = blend.blend_factor;
                                    pass.set_blend_constant(wgpu::Color { r: f[0] as f64 / 255.0, g: f[1] as f64 / 255.0, b: f[2] as f64 / 255.0, a: f[3] as f64 / 255.0 });
                                    bound = Some((Some(draw.key), *blend, *topology, *cull));
                                }
                                if draw.raw != raw_bound {
                                    match (&raw_vertex_buffer, draw.raw) {
                                        (Some(b), true) => pass.set_vertex_buffer(0, b.slice(..)),
                                        (None, true) => continue,
                                        (_, false) => pass.set_vertex_buffer(0, vertex_buffer.slice(..)),
                                    }
                                    raw_bound = draw.raw;
                                }
                                pass.set_bind_group(0, &draw.textures, &[]);
                                pass.set_bind_group(1, &draw.uniforms, &draw.offsets);
                                pass.draw(*first_vertex..*first_vertex + *vertex_count, 0..1);
                                continue;
                            }
                            if effect.is_some() && !gpu.warned_effects {
                                gpu.warned_effects = true;
                                eprintln!("[gfx] an effect without a compiled file was applied: its draws use the plain sprite shader");
                            }
                            if *raw {
                                continue;
                            }
                            if raw_bound {
                                pass.set_vertex_buffer(0, vertex_buffer.slice(..));
                                raw_bound = false;
                            }
                            let Some(tex) = gpu.textures.get(texture) else { continue };
                            let shared = gpu.shared.as_ref().expect("built above");
                            let smp = gpu.samplers.entry(*sampler).or_insert_with(|| build_sampler(device, *sampler));
                            let group = gpu.bind_groups.entry((*texture, *sampler)).or_insert_with(|| {
                                device.create_bind_group(&wgpu::BindGroupDescriptor {
                                    label: Some("spritebatch texture"),
                                    layout: &shared.layout,
                                    entries: &[
                                        wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&tex.view) },
                                        wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(smp) },
                                    ],
                                })
                            });
                            if bound != Some((None, *blend, *topology, *cull)) {
                                let pipeline = gpu.pipelines.entry((*blend, *topology, *cull)).or_insert_with(|| build_pipeline(device, shared, *blend, *topology, *cull));
                                pass.set_pipeline(pipeline);
                                let f = blend.blend_factor;
                                pass.set_blend_constant(wgpu::Color { r: f[0] as f64 / 255.0, g: f[1] as f64 / 255.0, b: f[2] as f64 / 255.0, a: f[3] as f64 / 255.0 });
                                bound = Some((None, *blend, *topology, *cull));
                            }
                            pass.set_bind_group(0, &*group, &[]);
                            pass.draw(*first_vertex..*first_vertex + *vertex_count, 0..1);
                        }
                        _ => unreachable!("the pass range holds only draws and viewport / scissor changes"),
                    }
                }
                drop(pass);
                i = end;
            }
        }
    }
    if let Some(e) = encoder.take() {
        queue.submit([e.finish()]);
    }
    if partial {
        gpu.carry = Some(Carry { target, discard, viewport, scissor });
    }
}

/// The RGBA8 rows of render target `id`, top row first, once everything submitted so far has run (blocks on the GPU).
fn read_target(gpu: &GfxGpu, device: &wgpu::Device, queue: &RenderQueue, id: TextureId) -> Vec<u8> {
    let Some(t) = gpu.textures.get(&id) else {
        eprintln!("[gfx] readback of texture {} which the GPU does not hold; returning no pixels", id.0);
        return Vec::new();
    };
    let (w, h) = (t.width.max(1), t.height.max(1));
    let bpr = (w * 4).next_multiple_of(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT);
    let buf = device.create_buffer(&wgpu::BufferDescriptor { label: Some("readback"), size: bpr as u64 * h as u64, usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
    let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("readback") });
    enc.copy_texture_to_buffer(
        t.texture.as_image_copy(),
        wgpu::TexelCopyBufferInfo { buffer: &buf, layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(bpr), rows_per_image: Some(h) } },
        wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
    );
    queue.submit([enc.finish()]);
    let (tx, rx) = std::sync::mpsc::channel();
    buf.slice(..).map_async(wgpu::MapMode::Read, move |r| {
        let _ = tx.send(r);
    });
    device.poll(wgpu::PollType::wait_indefinitely()).expect("wait for the readback copy");
    rx.recv().expect("map callback").expect("map the readback buffer");
    let data = buf.slice(..).get_mapped_range();
    let mut out = Vec::with_capacity((t.width * t.height * 4) as usize);
    for row in data.chunks(bpr as usize).take(t.height as usize) {
        out.extend_from_slice(&row[..(t.width * 4) as usize]);
    }
    drop(data);
    buf.unmap();
    out
}

/// The host's `readback_hook` for the wgpu replay (install with `terraria_host::readback::install_sync`).
pub struct WgpuReadback {
    shared: SharedGpu,
    published: PublishedFrames,
    device: wgpu::Device,
    queue: RenderQueue,
}

impl WgpuReadback {
    pub fn new(shared: &SharedGpu, published: &PublishedFrames, device: &RenderDevice, queue: &RenderQueue) -> WgpuReadback {
        WgpuReadback { shared: shared.clone(), published: published.clone(), device: device.wgpu_device().clone(), queue: queue.clone() }
    }
}

impl terraria_host::readback::ReadbackBackend for WgpuReadback {
    fn read_target(&mut self, frame: Frame, id: TextureId) -> Vec<u8> {
        let mut gpu = self.shared.0.lock().unwrap_or_else(|p| p.into_inner());
        replay_published(&mut gpu, &self.published, &self.device, &self.queue);
        replay_frame(&frame, &mut gpu, &self.device, &self.queue, true);
        read_target(&gpu, &self.device, &self.queue, id)
    }
}
