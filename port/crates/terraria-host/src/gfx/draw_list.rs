//! The backend-neutral output of all drawing: an ordered command list plus one shared vertex buffer.
//!
//! Everything the XNA object model (`GraphicsDevice`, `SpriteBatch`, `Texture2D`, ...) does ends up here.
//! A backend (the wgpu replay in `terraria-app`) takes a [`Frame`] once per presented frame and executes the
//! commands in order. Resource commands (create / upload / destroy) are in the same stream as the draws, so a
//! `Texture2D.SetData` between two draws is seen between them.
//!
//! Conventions (XNA 4 HiDef, no sRGB anywhere): textures and targets are RGBA8, blending works on the stored
//! bytes, content textures are premultiplied alpha as the content pipeline wrote them. Vertex positions are in
//! clip space of the viewport that was current when they were recorded (x right, y up, -1..1).
//!
//! Direct3D 9 half-pixel offset: XNA's sprite projection subtracts half a pixel (`SpriteBatch.SetRenderState`,
//! reference/xna/SpriteBatch.cs:784) because D3D9 pixel centres are at integers. wgpu's are at .5, so the same
//! texel-to-pixel mapping is obtained WITHOUT the offset; it is left out here on purpose.

use std::sync::Arc;

use terraria_core::xna::{Color, Matrix, Vector2};

/// A texture or render target known to the draw list. `0` is the back buffer (a target, never sampled).
thread_local! {
    static UPLOAD_SINK: std::cell::RefCell<Option<Box<dyn FnMut(TextureId, u32, u32, &[u8])>>> = const { std::cell::RefCell::new(None) };
}

/// Installs (or removes) a receiver for the initial pixels of textures created with data (content loads): it gets
/// (id, width, height, RGBA rows) at once and the frame carries only the `CreateTexture` (the receiver keeps the
/// contents when that command arrives). Native backends leave it unset: the pixels travel in the frame.
pub fn set_upload_sink(sink: Option<Box<dyn FnMut(TextureId, u32, u32, &[u8])>>) {
    UPLOAD_SINK.with(|s| *s.borrow_mut() = sink);
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TextureId(pub u32);
impl TextureId {
    pub const BACK_BUFFER: TextureId = TextureId(0);
}
/// Render targets are textures (`RenderTarget2D : Texture2D`).
pub type TargetId = TextureId;

/// `Microsoft.Xna.Framework.Graphics.Blend` (reference/xna/enums/Blend.cs; values are the enum's).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Blend {
    One = 0,
    Zero = 1,
    SourceColor = 2,
    InverseSourceColor = 3,
    SourceAlpha = 4,
    InverseSourceAlpha = 5,
    DestinationColor = 6,
    InverseDestinationColor = 7,
    DestinationAlpha = 8,
    InverseDestinationAlpha = 9,
    BlendFactor = 10,
    InverseBlendFactor = 11,
    SourceAlphaSaturation = 12,
}
impl Blend {
    pub fn from_i32(v: i32) -> Option<Blend> {
        use Blend::*;
        const ALL: [Blend; 13] = [
            One,
            Zero,
            SourceColor,
            InverseSourceColor,
            SourceAlpha,
            InverseSourceAlpha,
            DestinationColor,
            InverseDestinationColor,
            DestinationAlpha,
            InverseDestinationAlpha,
            BlendFactor,
            InverseBlendFactor,
            SourceAlphaSaturation,
        ];
        ALL.get(usize::try_from(v).ok()?).copied()
    }
}

/// `BlendFunction` (reference/xna/enums/BlendFunction.cs).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum BlendFunction {
    Add = 0,
    Subtract = 1,
    ReverseSubtract = 2,
    Min = 3,
    Max = 4,
}
impl BlendFunction {
    pub fn from_i32(v: i32) -> Option<BlendFunction> {
        use BlendFunction::*;
        [Add, Subtract, ReverseSubtract, Min, Max].get(usize::try_from(v).ok()?).copied()
    }
}

/// The value of a `BlendState` object at the moment of a draw.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct BlendDesc {
    pub color_src: Blend,
    pub color_dst: Blend,
    pub color_fn: BlendFunction,
    pub alpha_src: Blend,
    pub alpha_dst: Blend,
    pub alpha_fn: BlendFunction,
    /// `BlendState.BlendFactor` (RGBA)
    pub blend_factor: [u8; 4],
    /// `ColorWriteChannels`: Red 1, Green 2, Blue 4, Alpha 8
    pub write_mask: u8,
}
#[allow(non_upper_case_globals)] // named like the XNA presets so ported code reads `BlendState::AlphaBlend`
impl BlendDesc {
    const fn preset(src: Blend, dst: Blend) -> BlendDesc {
        BlendDesc { color_src: src, color_dst: dst, color_fn: BlendFunction::Add, alpha_src: src, alpha_dst: dst, alpha_fn: BlendFunction::Add, blend_factor: [255; 4], write_mask: 15 }
    }
    /// `BlendState.AlphaBlend`: One, InverseSourceAlpha (premultiplied). reference/xna/BlendState.cs static ctor.
    pub const AlphaBlend: BlendDesc = BlendDesc::preset(Blend::One, Blend::InverseSourceAlpha);
    /// `BlendState.Additive`: SourceAlpha, One.
    pub const Additive: BlendDesc = BlendDesc::preset(Blend::SourceAlpha, Blend::One);
    /// `BlendState.NonPremultiplied`: SourceAlpha, InverseSourceAlpha.
    pub const NonPremultiplied: BlendDesc = BlendDesc::preset(Blend::SourceAlpha, Blend::InverseSourceAlpha);
    /// `BlendState.Opaque`: One, Zero.
    pub const Opaque: BlendDesc = BlendDesc::preset(Blend::One, Blend::Zero);
}
impl Default for BlendDesc {
    fn default() -> Self {
        BlendDesc::AlphaBlend
    }
}

/// `TextureFilter` (reference/xna/enums/TextureFilter.cs).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum TextureFilter {
    Linear = 0,
    Point = 1,
    Anisotropic = 2,
    LinearMipPoint = 3,
    PointMipLinear = 4,
    MinLinearMagPointMipLinear = 5,
    MinLinearMagPointMipPoint = 6,
    MinPointMagLinearMipLinear = 7,
    MinPointMagLinearMipPoint = 8,
}
impl TextureFilter {
    pub fn from_i32(v: i32) -> Option<TextureFilter> {
        use TextureFilter::*;
        [Linear, Point, Anisotropic, LinearMipPoint, PointMipLinear, MinLinearMagPointMipLinear, MinLinearMagPointMipPoint, MinPointMagLinearMipLinear, MinPointMagLinearMipPoint]
            .get(usize::try_from(v).ok()?)
            .copied()
    }
    /// (minification is linear, magnification is linear). Textures here have one mip level, so the mip part of the filter has no effect.
    pub fn min_mag_linear(self) -> (bool, bool) {
        use TextureFilter::*;
        match self {
            Linear | Anisotropic | LinearMipPoint => (true, true),
            Point | PointMipLinear => (false, false),
            MinLinearMagPointMipLinear | MinLinearMagPointMipPoint => (true, false),
            MinPointMagLinearMipLinear | MinPointMagLinearMipPoint => (false, true),
        }
    }
}

/// `TextureAddressMode` (reference/xna/enums/TextureAddressMode.cs).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum TextureAddressMode {
    Wrap = 0,
    Clamp = 1,
    Mirror = 2,
}
impl TextureAddressMode {
    pub fn from_i32(v: i32) -> Option<TextureAddressMode> {
        use TextureAddressMode::*;
        [Wrap, Clamp, Mirror].get(usize::try_from(v).ok()?).copied()
    }
}

/// The value of a `SamplerState` object at the moment of a draw.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SamplerDesc {
    pub filter: TextureFilter,
    pub address_u: TextureAddressMode,
    pub address_v: TextureAddressMode,
}
#[allow(non_upper_case_globals)]
impl SamplerDesc {
    const fn preset(filter: TextureFilter, address: TextureAddressMode) -> SamplerDesc {
        SamplerDesc { filter, address_u: address, address_v: address }
    }
    pub const PointClamp: SamplerDesc = SamplerDesc::preset(TextureFilter::Point, TextureAddressMode::Clamp);
    pub const PointWrap: SamplerDesc = SamplerDesc::preset(TextureFilter::Point, TextureAddressMode::Wrap);
    pub const LinearClamp: SamplerDesc = SamplerDesc::preset(TextureFilter::Linear, TextureAddressMode::Clamp);
    pub const LinearWrap: SamplerDesc = SamplerDesc::preset(TextureFilter::Linear, TextureAddressMode::Wrap);
    pub const AnisotropicClamp: SamplerDesc = SamplerDesc::preset(TextureFilter::Anisotropic, TextureAddressMode::Clamp);
    pub const AnisotropicWrap: SamplerDesc = SamplerDesc::preset(TextureFilter::Anisotropic, TextureAddressMode::Wrap);
}
impl Default for SamplerDesc {
    fn default() -> Self {
        SamplerDesc::PointClamp
    }
}

/// An integer rectangle in target pixels (viewport, scissor, upload region).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub struct IRect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
pub struct Vertex {
    /// clip-space position inside the current viewport
    pub pos: [f32; 2],
    pub uv: [f32; 2],
    pub color: [u8; 4],
}

/// A vertex for a draw under an effect pass that has its own vertex shader: the vertex as the game supplied it,
/// untransformed (the vertex shader runs in the backend). Missing components are (0, 0, 0, 1), as Direct3D
/// expands a shorter vertex element.
#[derive(Clone, Copy, Debug, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
pub struct RawVertex {
    /// `POSITION0`
    pub pos: [f32; 4],
    /// `TEXCOORD0`
    pub tex: [f32; 4],
    /// `COLOR0`
    pub color: [u8; 4],
}

/// A compiled effect known to the draw list (see [`Command::CreateEffect`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct EffectId(pub u32);

/// A value of an effect parameter at the moment a pass was applied.
#[derive(Clone, Debug, PartialEq)]
pub enum ParamValue {
    /// no value: a sampler / texture parameter nobody set, or a parameter of an effect built without a file
    Unset,
    Bool(bool),
    Int(i32),
    /// `float`, `Vector2/3/4`, `Matrix` (16 values, row major as XNA stores it), float arrays
    Floats(Vec<f32>),
    Texture(Option<TextureId>),
}

/// What `EffectPass.Apply()` recorded: enough for a backend that executes the shader to reproduce the draw.
#[derive(Clone, Debug, PartialEq)]
pub struct EffectSnapshot {
    /// the compiled effect ([`Command::CreateEffect`]); `None` for an effect built without a file (nothing to run)
    pub effect_id: Option<EffectId>,
    /// index of the technique inside the effect
    pub technique_index: u32,
    /// the pass assigns a vertex shader: the draw's vertices are [`RawVertex`] (see [`Command::Draw`])
    pub vertex_shader: bool,
    /// asset name the effect was loaded from (`PixelShader`, `TileShader`, `ScreenShader`), or a host label
    pub effect: String,
    pub technique: String,
    pub pass: String,
    pub pass_index: u32,
    pub params: Vec<(String, ParamValue)>,
    /// `GraphicsDevice.Textures[1..=3]` and their samplers at the draw (slot, texture, sampler): the second and
    /// third images of the game's shaders. Slot 0 is the draw's own texture.
    pub extra_textures: Vec<(u8, TextureId, SamplerDesc)>,
}

/// `CullMode` (reference/xna/enums/CullMode.cs). Winding is judged on screen (y down), as Direct3D does:
/// a sprite's triangles are clockwise, so `CullCounterClockwiseFace` (SpriteBatch's default) keeps them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum CullMode {
    #[default]
    None = 0,
    CullClockwiseFace = 1,
    CullCounterClockwiseFace = 2,
}
impl CullMode {
    pub fn from_i32(v: i32) -> Option<CullMode> {
        use CullMode::*;
        [None, CullClockwiseFace, CullCounterClockwiseFace].get(usize::try_from(v).ok()?).copied()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Topology {
    /// triangle list (strips and indexed draws are expanded when recorded)
    Triangles,
    /// line list
    Lines,
}

#[derive(Clone, Debug)]
pub enum Command {
    /// A new texture (`target` = it can be rendered to). Contents are transparent black until uploaded / drawn.
    CreateTexture { id: TextureId, width: u32, height: u32, target: bool },
    /// RGBA8 rows for `rect` of the texture (`Texture2D.SetData`, content loads).
    Upload { id: TextureId, rect: IRect, rgba: Arc<[u8]> },
    DestroyTexture { id: TextureId },
    /// `GraphicsDevice.SetRenderTarget`. `preserve == false`: the target's contents are discarded when it is bound
    /// (`RenderTargetUsage.DiscardContents`); the replay clears it to transparent black unless a `Clear` follows.
    /// `preserve == true`: `RenderTargetUsage.PreserveContents`. README, "Render target rules".
    SetTarget { target: TargetId, preserve: bool },
    /// `GraphicsDevice.Clear(color)`: the whole current target (the game only clears with a full-target viewport).
    Clear(Color),
    Viewport(IRect),
    /// `None`: scissor test off.
    Scissor(Option<IRect>),
    /// `raw == false`: the range is in [`Frame::vertices`] (clip space). `raw == true`: the range is in
    /// [`Frame::raw_vertices`]; only recorded under an effect pass with a vertex shader, which positions them.
    Draw { texture: TextureId, blend: BlendDesc, sampler: SamplerDesc, cull: CullMode, topology: Topology, effect: Option<Arc<EffectSnapshot>>, first_vertex: u32, vertex_count: u32, raw: bool },
    /// A compiled effect was loaded: the D3DX effect file (what `Effect`'s constructor is given). Draws refer to it
    /// through [`EffectSnapshot::effect_id`]. Effects are never destroyed (the game loads three and keeps them).
    CreateEffect { id: EffectId, name: String, bytecode: Arc<[u8]> },
    /// Copy the target's pixels back to the CPU; the backend answers through [`DrawList::deliver_readback`].
    Readback { id: TextureId, request: u64 },
}

/// One presented frame: everything recorded since the previous [`DrawList::take_frame`].
#[derive(Clone, Debug, Default)]
pub struct Frame {
    /// increases by one per `take_frame`; a backend that sees the same serial twice must not replay it
    pub serial: u64,
    pub commands: Vec<Command>,
    pub vertices: Vec<Vertex>,
    /// vertices of the draws with `raw == true`
    pub raw_vertices: Vec<RawVertex>,
    pub back_buffer: (u32, u32),
}

#[derive(Clone, Debug)]
struct TextureInfo {
    width: u32,
    height: u32,
    target: bool,
    alive: bool,
    /// CPU copy (RGBA8), kept for textures the game may read back with `GetData`
    cpu: Option<Vec<u8>>,
}

/// The colour XNA clears a `DiscardContents` target to before the first draw after it is bound
/// (reference/xna/GraphicsDevice.cs `ClearDirtyBuffers`).
pub const DISCARD_COLOR: Color = Color::new(68, 34, 136, 255);

/// The recorder. One per `GraphicsDevice`.
#[derive(Debug)]
pub struct DrawList {
    commands: Vec<Command>,
    vertices: Vec<Vertex>,
    raw_vertices: Vec<RawVertex>,
    effects: u32,
    textures: Vec<TextureInfo>,
    current_target: TextureId,
    viewport: IRect,
    scissor: Option<IRect>,
    cull: CullMode,
    serial: u64,
    next_readback: u64,
    readbacks: Vec<(u64, TextureId, Vec<u8>)>,
}

impl Default for DrawList {
    fn default() -> Self {
        Self::new()
    }
}

impl DrawList {
    pub fn new() -> Self {
        DrawList {
            commands: Vec::new(),
            vertices: Vec::new(),
            raw_vertices: Vec::new(),
            effects: 0,
            // the back buffer; sized by `set_back_buffer_size`
            textures: vec![TextureInfo { width: 1, height: 1, target: true, alive: true, cpu: None }],
            current_target: TextureId::BACK_BUFFER,
            viewport: IRect { x: 0, y: 0, w: 1, h: 1 },
            scissor: None,
            cull: CullMode::None,
            serial: 0,
            next_readback: 1,
            readbacks: Vec::new(),
        }
    }

    // ------------------------------------------------------------------------------------------ resources

    /// The window's client size changed (or first known). The backend owns the actual surface.
    pub fn set_back_buffer_size(&mut self, width: u32, height: u32) {
        let t = &mut self.textures[0];
        t.width = width.max(1);
        t.height = height.max(1);
        if self.current_target == TextureId::BACK_BUFFER {
            self.viewport = IRect { x: 0, y: 0, w: t.width as i32, h: t.height as i32 };
        }
    }

    fn add(&mut self, width: u32, height: u32, target: bool, cpu: Option<Vec<u8>>) -> TextureId {
        let id = TextureId(self.textures.len() as u32);
        // larger than WebGPU's default limit (8192) or than any texture the game is known to make: say so (the browser
        // build cannot hold a 16384 x 16384 RGBA texture next to the game's heap)
        if width > 4096 || height > 4096 {
            terraria_gen_rt::host::log(&format!("[gfx] large texture {}: {width} x {height}, target {target}", id.0));
        }
        self.textures.push(TextureInfo { width, height, target, alive: true, cpu });
        self.commands.push(Command::CreateTexture { id, width, height, target });
        id
    }

    /// `new Texture2D(device, width, height)` followed by `SetData` when `rgba` is given.
    /// `keep_cpu_copy`: keep the pixels so `GetData` can be answered synchronously.
    pub fn create_texture(&mut self, width: u32, height: u32, rgba: Option<Vec<u8>>, keep_cpu_copy: bool) -> TextureId {
        if let Some(p) = &rgba {
            assert_eq!(p.len(), width as usize * height as usize * 4, "create_texture: pixel data size");
        }
        let cpu = match (&rgba, keep_cpu_copy) {
            (Some(p), true) => Some(p.clone()),
            (None, true) => Some(vec![0; width as usize * height as usize * 4]),
            _ => None,
        };
        let id = self.add(width, height, false, cpu);
        if let Some(p) = rgba {
            // A backend that takes the pixels of a NEW texture at once (the browser build: a title load would otherwise
            // hold hundreds of MB in one frame). Safe for a texture created now: no earlier draw can have sampled it.
            let taken = UPLOAD_SINK.with(|s| s.borrow_mut().as_mut().map(|f| f(id, width, height, &p)).is_some());
            if !taken {
                self.commands.push(Command::Upload { id, rect: IRect { x: 0, y: 0, w: width as i32, h: height as i32 }, rgba: p.into() });
            }
        }
        id
    }

    /// A compiled effect was loaded (`new Effect(device, bytes)`): hands the file to the backend.
    pub fn create_effect(&mut self, name: &str, bytecode: Arc<[u8]>) -> EffectId {
        self.effects += 1;
        let id = EffectId(self.effects);
        self.commands.push(Command::CreateEffect { id, name: name.to_owned(), bytecode });
        id
    }

    /// `new RenderTarget2D(device, width, height, ...)`.
    pub fn create_target(&mut self, width: u32, height: u32) -> TargetId {
        self.add(width.max(1), height.max(1), true, None)
    }

    /// `GraphicsResource.Dispose()` / the object was collected.
    pub fn destroy_texture(&mut self, id: TextureId) {
        assert!(id != TextureId::BACK_BUFFER, "the back buffer cannot be destroyed");
        let t = &mut self.textures[id.0 as usize];
        if t.alive {
            t.alive = false;
            t.cpu = None;
            self.commands.push(Command::DestroyTexture { id });
        }
    }

    pub fn is_alive(&self, id: TextureId) -> bool {
        self.textures.get(id.0 as usize).is_some_and(|t| t.alive)
    }

    pub fn texture_size(&self, id: TextureId) -> (u32, u32) {
        let t = &self.textures[id.0 as usize];
        (t.width, t.height)
    }

    pub fn is_target(&self, id: TextureId) -> bool {
        self.textures[id.0 as usize].target
    }

    /// `Texture2D.SetData` for a region; `rgba` is `rect.w * rect.h * 4` bytes.
    pub fn set_data(&mut self, id: TextureId, rect: IRect, rgba: &[u8]) {
        let t = &mut self.textures[id.0 as usize];
        assert!(t.alive, "SetData on a disposed texture");
        assert!(rect.x >= 0 && rect.y >= 0 && rect.w >= 0 && rect.h >= 0 && (rect.x + rect.w) as u32 <= t.width && (rect.y + rect.h) as u32 <= t.height, "SetData: rectangle outside the texture");
        assert_eq!(rgba.len(), rect.w as usize * rect.h as usize * 4, "SetData: data size");
        if let Some(cpu) = &mut t.cpu {
            let stride = t.width as usize * 4;
            for row in 0..rect.h as usize {
                let dst = (rect.y as usize + row) * stride + rect.x as usize * 4;
                let src = row * rect.w as usize * 4;
                cpu[dst..dst + rect.w as usize * 4].copy_from_slice(&rgba[src..src + rect.w as usize * 4]);
            }
        }
        if rect.w > 0 && rect.h > 0 {
            self.commands.push(Command::Upload { id, rect, rgba: rgba.into() });
        }
    }

    /// The CPU copy of a texture (`Texture2D.GetData`), if one is kept.
    pub fn cpu_pixels(&self, id: TextureId) -> Option<&[u8]> {
        self.textures[id.0 as usize].cpu.as_deref()
    }

    /// Ask the backend for the pixels of a render target. The answer arrives through [`Self::deliver_readback`]
    /// after the frame containing the request has been replayed (not in this frame).
    pub fn request_readback(&mut self, id: TextureId) -> u64 {
        let request = self.next_readback;
        self.next_readback += 1;
        self.commands.push(Command::Readback { id, request });
        request
    }

    /// Backend -> host: pixels for a [`Command::Readback`].
    pub fn deliver_readback(&mut self, request: u64, id: TextureId, rgba: Vec<u8>) {
        self.readbacks.retain(|(_, t, _)| *t != id);
        self.readbacks.push((request, id, rgba));
    }

    /// The newest delivered readback of `id`: (request number, pixels).
    pub fn readback(&self, id: TextureId) -> Option<(u64, &[u8])> {
        self.readbacks.iter().find(|(_, t, _)| *t == id).map(|(r, _, p)| (*r, p.as_slice()))
    }

    // ------------------------------------------------------------------------------------------ state

    pub fn current_target(&self) -> TargetId {
        self.current_target
    }

    /// `GraphicsDevice.SetRenderTarget`: binds the target and resets the viewport to its full size (XNA does both).
    pub fn set_render_target(&mut self, target: TargetId, preserve: bool) {
        let t = &self.textures[target.0 as usize];
        assert!(t.alive && t.target, "SetRenderTarget: not a live render target");
        self.current_target = target;
        self.viewport = IRect { x: 0, y: 0, w: t.width as i32, h: t.height as i32 };
        self.commands.push(Command::SetTarget { target, preserve });
        if self.scissor.is_some() {
            // the replay starts every target with the scissor test off; state it again
            self.commands.push(Command::Scissor(self.scissor));
        }
    }

    pub fn clear(&mut self, color: Color) {
        self.commands.push(Command::Clear(color));
    }

    pub fn viewport(&self) -> IRect {
        self.viewport
    }

    pub fn set_viewport(&mut self, v: IRect) {
        if v != self.viewport {
            self.viewport = v;
            self.commands.push(Command::Viewport(v));
        }
    }

    pub fn scissor(&self) -> Option<IRect> {
        self.scissor
    }

    /// `RasterizerState.ScissorTestEnable` + `GraphicsDevice.ScissorRectangle` as one effective state.
    pub fn set_scissor(&mut self, s: Option<IRect>) {
        if s != self.scissor {
            self.scissor = s;
            self.commands.push(Command::Scissor(s));
        }
    }

    /// `RasterizerState.CullMode` for the draws that follow. The hand-ported prototype never sets it (no culling).
    pub fn set_cull(&mut self, c: CullMode) {
        self.cull = c;
    }

    // ------------------------------------------------------------------------------------------ drawing

    /// Pixel position (origin top-left of the viewport, y down) -> clip space.
    #[inline]
    pub fn to_clip(&self, p: Vector2) -> [f32; 2] {
        let (vw, vh) = (self.viewport.w as f32, self.viewport.h as f32);
        [p.x / vw * 2.0 - 1.0, 1.0 - p.y / vh * 2.0]
    }

    pub fn vertex_count(&self) -> u32 {
        self.vertices.len() as u32
    }

    pub fn push_vertices(&mut self, v: &[Vertex]) {
        self.vertices.extend_from_slice(v);
    }

    pub fn raw_vertex_count(&self) -> u32 {
        self.raw_vertices.len() as u32
    }

    pub fn push_raw_vertices(&mut self, v: &[RawVertex]) {
        self.raw_vertices.extend_from_slice(v);
    }

    /// Record a draw of a range of raw vertices under an effect pass with a vertex shader.
    #[allow(clippy::too_many_arguments)]
    pub fn draw_raw_range(&mut self, texture: TextureId, blend: BlendDesc, sampler: SamplerDesc, topology: Topology, effect: Arc<EffectSnapshot>, first_vertex: u32, vertex_count: u32) {
        debug_assert!(effect.vertex_shader, "raw vertices need a vertex shader");
        if vertex_count > 0 {
            self.commands.push(Command::Draw { texture, blend, sampler, cull: self.cull, topology, effect: Some(effect), first_vertex, vertex_count, raw: true });
        }
    }

    /// Record a draw of the vertices pushed since `first_vertex`. Nothing is recorded for an empty range.
    pub fn draw(&mut self, texture: TextureId, blend: BlendDesc, sampler: SamplerDesc, topology: Topology, effect: Option<Arc<EffectSnapshot>>, first_vertex: u32) {
        let vertex_count = self.vertices.len() as u32 - first_vertex;
        self.draw_range(texture, blend, sampler, topology, effect, first_vertex, vertex_count);
    }

    /// Record a draw of an explicit vertex range (a multi-pass effect draws the same range once per pass).
    #[allow(clippy::too_many_arguments)]
    pub fn draw_range(&mut self, texture: TextureId, blend: BlendDesc, sampler: SamplerDesc, topology: Topology, effect: Option<Arc<EffectSnapshot>>, first_vertex: u32, vertex_count: u32) {
        if vertex_count > 0 {
            self.commands.push(Command::Draw { texture, blend, sampler, cull: self.cull, topology, effect, first_vertex, vertex_count, raw: false });
        }
    }

    /// The draw call `TileBatch` makes with `GraphicsDevice.DrawIndexedPrimitives` (TileBatch.cs:617): `quads` as one
    /// indexed triangle list (0,1,2 / 0,2,3 per quad), vertex colours per corner, positions in pixels transformed by
    /// `transform` (the matrix the batch was begun with) and mapped to the current viewport.
    pub fn draw_colored_quads(&mut self, texture: TextureId, blend: BlendDesc, sampler: SamplerDesc, transform: &Matrix, quads: &[ColoredQuad]) {
        if quads.is_empty() {
            return;
        }
        let first_vertex = self.vertices.len() as u32;
        for q in quads {
            let mut v = [Vertex { pos: [0.0; 2], uv: [0.0; 2], color: [0; 4] }; 4];
            for k in 0..4 {
                let p = Vector2::transform(q.pos[k], transform);
                v[k].pos = self.to_clip(p);
                v[k].uv = [q.uv[k].x, q.uv[k].y];
                v[k].color = q.color[k].to_rgba_bytes();
            }
            self.vertices.extend_from_slice(&[v[0], v[1], v[2], v[0], v[2], v[3]]);
        }
        self.draw(texture, blend, sampler, Topology::Triangles, None, first_vertex);
    }

    // ------------------------------------------------------------------------------------------ frames

    /// Start of `Game.Draw`: the back buffer is the target again. Recorded commands are kept (resources created
    /// during `Update` must reach the backend); they leave with [`Self::take_frame`].
    pub fn begin_frame(&mut self) {
        self.current_target = TextureId::BACK_BUFFER;
        let t = &self.textures[0];
        self.viewport = IRect { x: 0, y: 0, w: t.width as i32, h: t.height as i32 };
        if self.scissor.is_some() {
            self.scissor = None;
            self.commands.push(Command::Scissor(None));
        }
    }

    /// Everything recorded since the last call. Call exactly once per presented frame.
    pub fn take_frame(&mut self) -> Frame {
        self.serial += 1;
        let t = &self.textures[0];
        Frame { serial: self.serial, commands: std::mem::take(&mut self.commands), vertices: std::mem::take(&mut self.vertices), raw_vertices: std::mem::take(&mut self.raw_vertices), back_buffer: (t.width, t.height) }
    }

    /// The commands recorded so far (tests, debugging).
    pub fn commands(&self) -> &[Command] {
        &self.commands
    }

    /// The vertices recorded so far (tests, debugging).
    pub fn vertices(&self) -> &[Vertex] {
        &self.vertices
    }

    /// The raw vertices recorded so far (tests, debugging).
    pub fn raw_vertices(&self) -> &[RawVertex] {
        &self.raw_vertices
    }
}

/// One quad with a colour per corner (what `TileBatch` queues). Corner order is the vertex order of
/// `VertexPositionColorTexture` in TileBatch.cs:646: top-left, top-right, bottom-right, bottom-left.
#[derive(Clone, Copy, Debug)]
pub struct ColoredQuad {
    pub pos: [Vector2; 4],
    pub uv: [Vector2; 4],
    pub color: [Color; 4],
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resources_and_draws_stay_in_order_and_leave_once() {
        let mut l = DrawList::new();
        l.set_back_buffer_size(100, 50);
        let t = l.create_texture(2, 1, Some(vec![1, 2, 3, 4, 5, 6, 7, 8]), true);
        assert_eq!(t, TextureId(1));
        l.begin_frame();
        l.set_render_target(TextureId::BACK_BUFFER, false);
        let first = l.vertex_count();
        l.push_vertices(&[Vertex { pos: l.to_clip(Vector2::new(50.0, 25.0)), uv: [0.0; 2], color: [255; 4] }; 3]);
        l.draw(t, BlendDesc::AlphaBlend, SamplerDesc::PointClamp, Topology::Triangles, None, first);
        l.set_data(t, IRect { x: 1, y: 0, w: 1, h: 1 }, &[9, 9, 9, 9]);
        assert_eq!(l.cpu_pixels(t).unwrap(), &[1, 2, 3, 4, 9, 9, 9, 9]);
        let f = l.take_frame();
        assert_eq!(f.serial, 1);
        assert_eq!(f.back_buffer, (100, 50));
        let kinds: Vec<&str> = f
            .commands
            .iter()
            .map(|c| match c {
                Command::CreateTexture { .. } => "create",
                Command::Upload { .. } => "upload",
                Command::SetTarget { .. } => "target",
                Command::Draw { .. } => "draw",
                _ => "other",
            })
            .collect();
        assert_eq!(kinds, ["create", "upload", "target", "draw", "upload"]);
        assert_eq!(f.vertices[0].pos, [0.0, 0.0]);
        let f2 = l.take_frame();
        assert_eq!((f2.serial, f2.commands.len(), f2.vertices.len()), (2, 0, 0));
    }

    #[test]
    fn set_render_target_resets_the_viewport() {
        let mut l = DrawList::new();
        l.set_back_buffer_size(800, 600);
        let rt = l.create_target(64, 32);
        l.set_viewport(IRect { x: 10, y: 10, w: 20, h: 20 });
        l.set_render_target(rt, false);
        assert_eq!(l.viewport(), IRect { x: 0, y: 0, w: 64, h: 32 });
        l.set_render_target(TextureId::BACK_BUFFER, true);
        assert_eq!(l.viewport(), IRect { x: 0, y: 0, w: 800, h: 600 });
    }
}
