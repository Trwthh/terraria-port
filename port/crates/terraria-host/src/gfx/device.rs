//! `GraphicsDevice`, `Texture2D` / `RenderTarget2D`, `Viewport`, vertex and index buffers, user primitives.
//!
//! Source of the rules: reference/xna/GraphicsDevice.cs, Texture2D.cs, RenderTarget2D.cs, Viewport.cs.
//!
//! One Rust type per C# hierarchy (like `rt::io::Stream`): `Texture2D` is also `RenderTarget2D`
//! (`is_render_target`), `VertexBuffer` is also `DynamicVertexBuffer`, `IndexBuffer` also `DynamicIndexBuffer`.
//!
//! # Render target rules (XNA 4, `SetRenderTargets` / `ClearDirtyBuffers` / `Present`)
//! - Binding the target that is already bound does nothing.
//! - Binding a `DiscardContents` target marks it dirty; a dirty target is cleared to [`DISCARD_COLOR`] (68, 34, 136,
//!   255) before the first draw into it, when another target is bound, and at `Present`, unless `Clear` was called.
//! - The back buffer is dirty after every `Present`, whatever its `RenderTargetUsage`; with `PreserveContents` it
//!   keeps its pixels across a render-target switch inside a frame, with `DiscardContents` it is dirty again.
//! - Binding a target resets the viewport to the full target.
//!
//! # What a draw records
//! Texture `Textures[0]`, `BlendState`, `SamplerStates[0]`, `RasterizerState.CullMode`, the scissor rectangle when
//! `RasterizerState.ScissorTestEnable`, and the effect pass applied last (`EffectSnapshot`), if any.
//! Vertex positions:
//! - no effect pass applied, or one without a vertex shader: Direct3D keeps the vertex shader that is bound, which
//!   is `SpriteEffect`'s from the last `SpriteBatch` render state. The vertices are transformed here by that
//!   batch's matrix (its `transformMatrix`), then pixels -> clip space;
//! - an effect pass with its own vertex shader (six passes of `PixelShader`: `MagicMissile`, `FinalFractalVertex`,
//!   `HorizonClouds`, `LitNature`, `Aurora`, `StormLightning`): the vertices are recorded as the game supplied them
//!   (`RawVertex`: position, texture coordinate 0 with up to four components, colour) and the backend runs the
//!   translated vertex shader, which reads `MatrixTransform` from the snapshot.

use std::cell::RefCell;
use std::sync::Arc;

use terraria_core::xna::{Color, Matrix, Rectangle, Vector2, Vector3};
use terraria_gen_rt::host;
use terraria_gen_rt::rt::{self, Arr, Delegate, Gc, Marker, Nullable, Obj, Str, Trace};

use super::batcher::transform_xyz;
use super::draw_list::{BlendDesc, DrawList, EffectSnapshot, Frame, IRect, RawVertex, SamplerDesc, TextureId, Topology, Vertex, DISCARD_COLOR};
use super::states::{plain_trace, presets, throw, BlendState, DepthStencilState, RasterizerState, SamplerState};
use super::trace;

/// `SurfaceFormat.Color`: the only format with pixel access here.
pub const SURFACE_FORMAT_COLOR: i32 = 0;
/// `RenderTargetUsage`
pub const USAGE_DISCARD_CONTENTS: i32 = 0;
pub const USAGE_PRESERVE_CONTENTS: i32 = 1;

thread_local! {
    /// Texture ids of collected `Texture2D` objects, destroyed by the device at the next frame start.
    static DEAD_TEXTURES: RefCell<Vec<TextureId>> = const { RefCell::new(Vec::new()) };
}

// ------------------------------------------------------------------------------------------------ Viewport

/// `Microsoft.Xna.Framework.Graphics.Viewport` (a struct).
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Viewport {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub min_depth: f32,
    pub max_depth: f32,
}
plain_trace!(Viewport, VertexPositionColorTexture, VertexElement);

impl Viewport {
    /// `new Viewport(x, y, width, height)`: depth range 0..1.
    pub fn new(x: i32, y: i32, width: i32, height: i32) -> Viewport {
        Viewport { x, y, width, height, min_depth: 0.0, max_depth: 1.0 }
    }
    /// `new Viewport(bounds)`
    pub fn from_bounds(r: Rectangle) -> Viewport {
        Viewport::new(r.x, r.y, r.width, r.height)
    }
    pub fn bounds(&self) -> Rectangle {
        Rectangle::new(self.x, self.y, self.width, self.height)
    }
    /// `AspectRatio`: 0 when either size is 0.
    pub fn aspect_ratio(&self) -> f32 {
        if self.height == 0 || self.width == 0 {
            0.0
        } else {
            self.width as f32 / self.height as f32
        }
    }
    /// `TitleSafeArea` on Windows is the whole viewport.
    pub fn title_safe_area(&self) -> Rectangle {
        self.bounds()
    }
}

// ------------------------------------------------------------------------------------------------ Texture2D

/// `Texture2D` and `RenderTarget2D`.
pub struct Texture2D {
    pub device: Gc<GraphicsDevice>,
    pub id: TextureId,
    pub width: i32,
    pub height: i32,
    /// `SurfaceFormat` as given to the constructor
    pub format: i32,
    pub is_render_target: bool,
    /// `RenderTargetUsage` (targets)
    pub usage: i32,
    pub depth_format: i32,
    pub disposed: bool,
    /// `GraphicsResource.Name`
    pub name: Str,
    /// Re-reads the pixels of a content texture for `GetData` (no CPU copy is kept for content).
    pub reload: Option<Box<dyn Fn() -> Option<Vec<u8>>>>,
    /// `GraphicsResource.Disposing`: raised by `Dispose()` (sender = the texture, `EventArgs.Empty` = null here:
    /// the game's handlers do not read it).
    pub disposing: EventHandler,
    /// `RenderTarget2D.ContentLost`: kept, never raised (the device is never lost or reset, and a target's content
    /// is never discarded behind the game's back).
    pub content_lost: EventHandler,
}
/// `RenderTargetBinding` (a struct): the 2D target of one slot.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct RenderTargetBinding {
    pub render_target: Gc<Texture2D>,
}
// SAFETY: the one handle is reported.
unsafe impl Trace for RenderTargetBinding {
    unsafe fn trace(&self, m: &mut Marker) {
        m.mark(self.render_target)
    }
}

/// `o is GraphicsResource`: the host classes XNA derives from `GraphicsResource` (textures and render targets,
/// effects, vertex and index buffers, vertex declarations, the four state objects, `SpriteBatch`).
pub fn is_graphics_resource(o: rt::Obj) -> bool {
    use crate::gfx::states::{BlendState, DepthStencilState, RasterizerState, SamplerState};
    rt::obj_is::<Texture2D>(o)
        || rt::obj_is::<crate::gfx::effect::Effect>(o)
        || rt::obj_is::<VertexBuffer>(o)
        || rt::obj_is::<IndexBuffer>(o)
        || rt::obj_is::<VertexDeclaration>(o)
        || rt::obj_is::<BlendState>(o)
        || rt::obj_is::<SamplerState>(o)
        || rt::obj_is::<RasterizerState>(o)
        || rt::obj_is::<DepthStencilState>(o)
        || rt::obj_is::<crate::gfx::SpriteBatch>(o)
}

/// `GraphicsResource.Name` read on a resource held through the base class (null when never set, as in XNA).
pub fn graphics_resource_get_name(resource: rt::Obj) -> Str {
    if rt::obj_is::<Texture2D>(resource) {
        // SAFETY: type checked; the reference ends with this statement.
        unsafe { (*(resource.0 as *mut Texture2D)).name }
    } else if rt::obj_is::<crate::gfx::effect::Effect>(resource) {
        // SAFETY: as above.
        Str::new(unsafe { &(*(resource.0 as *mut crate::gfx::effect::Effect)).name })
    } else {
        let _ = resource.ptr();
        terraria_gen_rt::host::unimplemented_value("Microsoft.Xna.Framework.Graphics.GraphicsResource.Name.get on a resource that is neither a texture nor an effect")
    }
}

/// `resource.Disposing += handler` / `-= handler` on a resource held through the base class: textures and render
/// targets raise it from `Dispose()`.
pub fn graphics_resource_add_disposing(resource: rt::Obj, h: EventHandler) {
    if rt::obj_is::<Texture2D>(resource) {
        // SAFETY: type checked; the reference ends with this statement.
        unsafe { (*(resource.0 as *mut Texture2D)).add_disposing(h) }
    } else {
        let _ = resource.ptr();
        terraria_gen_rt::host::unimplemented("Microsoft.Xna.Framework.Graphics.GraphicsResource.Disposing.add on a resource that is not a texture");
    }
}
pub fn graphics_resource_remove_disposing(resource: rt::Obj, h: EventHandler) {
    if rt::obj_is::<Texture2D>(resource) {
        // SAFETY: as above.
        unsafe { (*(resource.0 as *mut Texture2D)).remove_disposing(h) }
    } else {
        let _ = resource.ptr();
        terraria_gen_rt::host::unimplemented("Microsoft.Xna.Framework.Graphics.GraphicsResource.Disposing.remove on a resource that is not a texture");
    }
}

/// `GraphicsResource.Name = name` on a resource held through the base class (`AssetInitializer.TagAsset`).
pub fn graphics_resource_set_name(resource: rt::Obj, name: Str) {
    if rt::obj_is::<Texture2D>(resource) {
        // SAFETY: type checked; the reference ends with this statement.
        unsafe { (*(resource.0 as *mut Texture2D)).name = name };
    } else if rt::obj_is::<crate::gfx::effect::Effect>(resource) {
        // SAFETY: as above.
        unsafe { (*(resource.0 as *mut crate::gfx::effect::Effect)).name = name.get().unwrap_or("").to_owned() };
    } else {
        let _ = resource.ptr();
        terraria_gen_rt::host::unimplemented("Microsoft.Xna.Framework.Graphics.GraphicsResource.Name.set on a resource that is neither a texture nor an effect");
    }
}

// SAFETY: both handles are reported; `reload` closures hold paths only.
unsafe impl Trace for Texture2D {
    unsafe fn trace(&self, m: &mut Marker) {
        m.mark(self.device);
        self.name.trace(m);
        self.disposing.trace(m);
        self.content_lost.trace(m);
    }
}
impl Drop for Texture2D {
    fn drop(&mut self) {
        if !self.disposed {
            let id = self.id;
            let _ = DEAD_TEXTURES.try_with(|d| d.borrow_mut().push(id));
        }
    }
}

fn dev<'a>(device: Gc<GraphicsDevice>) -> &'a mut GraphicsDevice {
    if device.is_null() {
        throw("System.ArgumentNullException", "graphicsDevice: The GraphicsDevice must not be null when creating new resources.");
    }
    // SAFETY: a live device object; host methods do not call back into generated code while the reference lives.
    unsafe { &mut *device.ptr() }
}

fn size_arg(width: i32, height: i32) -> (u32, u32) {
    if width <= 0 || height <= 0 {
        throw("System.ArgumentOutOfRangeException", "Texture width and height must be greater than zero.");
    }
    (width as u32, height as u32)
}

impl Texture2D {
    /// `new Texture2D(device, width, height)` / `(device, width, height, mipMap, format)`. Contents: transparent black.
    /// A CPU copy is kept (textures the game creates are the ones it fills and may read back).
    pub fn new_gc(device: Gc<GraphicsDevice>, width: i32, height: i32, mip_map: bool, format: i32) -> Gc<Texture2D> {
        let (w, h) = size_arg(width, height);
        if mip_map {
            host::unimplemented("Microsoft.Xna.Framework.Graphics.Texture2D..ctor(GraphicsDevice, int, int, bool, SurfaceFormat) with mipMap = true (one level is created)");
        }
        let id = dev(device).list.create_texture(w, h, None, format == SURFACE_FORMAT_COLOR);
        let label = trace::new_texture_label(id);
        trace::line(|| format!("tex.new {label} {width} {height} {} {format}", trace::b(mip_map)));
        rt::alloc_init(Texture2D { device, id, width, height, format, is_render_target: false, usage: 0, depth_format: 0, disposed: false, name: Str::NULL, reload: None, disposing: Delegate::NULL, content_lost: Delegate::NULL })
    }

    /// A texture from decoded RGBA8 pixels (content loading, `FromStream`).
    pub fn from_rgba(device: Gc<GraphicsDevice>, width: i32, height: i32, rgba: Vec<u8>, keep_cpu_copy: bool) -> Gc<Texture2D> {
        let (w, h) = size_arg(width, height);
        let id = dev(device).list.create_texture(w, h, Some(rgba), keep_cpu_copy);
        rt::alloc_init(Texture2D { device, id, width, height, format: SURFACE_FORMAT_COLOR, is_render_target: false, usage: 0, depth_format: 0, disposed: false, name: Str::NULL, reload: None, disposing: Delegate::NULL, content_lost: Delegate::NULL })
    }

    /// `new RenderTarget2D(device, width, height, mipMap, format, depthFormat, multiSampleCount, usage)`; the shorter
    /// overloads pass `usage = DiscardContents`.
    pub fn new_render_target(device: Gc<GraphicsDevice>, width: i32, height: i32, mip_map: bool, format: i32, depth_format: i32, usage: i32) -> Gc<Texture2D> {
        let (w, h) = size_arg(width, height);
        let _ = mip_map;
        if !(0..=2).contains(&usage) {
            throw("System.ArgumentOutOfRangeException", "usage");
        }
        let id = dev(device).list.create_target(w, h);
        let label = trace::new_target_label(id);
        trace::line(|| format!("rt.new {label} {width} {height} {} {format} {depth_format} {usage}", trace::b(mip_map)));
        rt::alloc_init(Texture2D { device, id, width, height, format, is_render_target: true, usage, depth_format, disposed: false, name: Str::NULL, reload: None, disposing: Delegate::NULL, content_lost: Delegate::NULL })
    }

    pub fn width(&self) -> i32 {
        self.width
    }
    pub fn height(&self) -> i32 {
        self.height
    }
    pub fn bounds(&self) -> Rectangle {
        Rectangle::new(0, 0, self.width, self.height)
    }
    pub fn format(&self) -> i32 {
        self.format
    }
    pub fn graphics_device(&self) -> Gc<GraphicsDevice> {
        self.device
    }
    pub fn is_disposed(&self) -> bool {
        self.disposed
    }
    /// `RenderTarget2D.IsContentLost`: contents are lost by a device reset; this device is never reset.
    pub fn is_content_lost(&self) -> bool {
        false
    }
    pub fn render_target_usage(&self) -> i32 {
        self.usage
    }
    pub fn add_disposing(&mut self, h: EventHandler) {
        self.disposing = Delegate::combine(self.disposing, h);
    }
    pub fn remove_disposing(&mut self, h: EventHandler) {
        self.disposing = Delegate::remove(self.disposing, h);
    }
    pub fn add_content_lost(&mut self, h: EventHandler) {
        self.content_lost = Delegate::combine(self.content_lost, h);
    }
    pub fn remove_content_lost(&mut self, h: EventHandler) {
        self.content_lost = Delegate::remove(self.content_lost, h);
    }
    pub fn dispose(&mut self) {
        if !self.disposed {
            trace::line(|| format!("tex.dispose {}", trace::tex_id(self.id)));
            // GraphicsResource.Dispose(true) raises Disposing first (reference/xna: `raise_Disposing(this, EventArgs.Empty)`)
            let handlers = self.disposing;
            if !handlers.is_null() {
                let me: Obj = Gc(self as *mut Texture2D).up();
                handlers.invoke(|f| f(me, Gc::NULL));
            }
            self.disposed = true;
            let d = dev(self.device);
            if d.render_target.0 as *const Texture2D == self as *const Texture2D {
                // XNA leaves a disposed target bound and fails later; binding the back buffer is the safe reading
                trace::internal(|| d.set_render_target(Gc::NULL));
            }
            d.list.destroy_texture(self.id);
        }
    }
    fn check(&self) {
        if self.disposed {
            throw("System.ObjectDisposedException", "Cannot access a disposed object. Object name: 'Texture2D'.");
        }
    }

    fn bytes_of<T: Copy + Trace + 'static>(data: Arr<T>, start: i32, count: i32) -> &'static [u8] {
        if data.is_null() {
            throw("System.ArgumentNullException", "data");
        }
        if start < 0 || count < 0 || start as i64 + count as i64 > data.len() as i64 {
            throw("System.ArgumentOutOfRangeException", "startIndex / elementCount");
        }
        let s = &data.as_slice()[start as usize..(start + count) as usize];
        // SAFETY: the element types are plain values (Color, byte, uint, ...); viewing them as bytes is always valid.
        unsafe { core::slice::from_raw_parts(s.as_ptr() as *const u8, core::mem::size_of_val(s)) }
    }

    /// `SetData<T>(int level, Rectangle? rect, T[] data, int startIndex, int elementCount)`. `T` is any 1/2/4-byte
    /// value type; the bytes are RGBA rows (`SurfaceFormat.Color`: a `Color[]`, or 4 bytes per pixel).
    pub fn set_data_rect<T: Copy + Trace + 'static>(&mut self, level: i32, rect: Nullable<Rectangle>, data: Arr<T>, start: i32, count: i32) {
        trace::line(|| format!("tex.data {} c {level} {} {start} {count} {}", trace::tex_id(self.id), trace::nrect(rect), trace::hash_arr(data, start, count)));
        self.set_data_rect_inner(level, rect, data, start, count)
    }
    fn set_data_rect_inner<T: Copy + Trace + 'static>(&mut self, level: i32, rect: Nullable<Rectangle>, data: Arr<T>, start: i32, count: i32) {
        self.check();
        if self.format != SURFACE_FORMAT_COLOR {
            host::unimplemented("Microsoft.Xna.Framework.Graphics.Texture2D.SetData<T>: SurfaceFormat other than Color");
            return;
        }
        if level != 0 {
            throw("System.ArgumentOutOfRangeException", "level");
        }
        let r = if rect.has_value { rect.value } else { self.bounds() };
        if r.x < 0 || r.y < 0 || r.width <= 0 || r.height <= 0 || r.x + r.width > self.width || r.y + r.height > self.height {
            throw("System.ArgumentException", "The rectangle is outside the texture.");
        }
        let bytes = Self::bytes_of(data, start, count);
        if bytes.len() != r.width as usize * r.height as usize * 4 {
            throw("System.ArgumentException", "The size of the data passed in is too large or too small for this resource.");
        }
        dev(self.device).list.set_data(self.id, IRect { x: r.x, y: r.y, w: r.width, h: r.height }, bytes);
    }
    /// `SetData<T>(T[] data)`
    pub fn set_data<T: Copy + Trace + 'static>(&mut self, data: Arr<T>) {
        let n = if data.is_null() { 0 } else { data.len() };
        trace::line(|| format!("tex.data {} a 0 - 0 {n} {}", trace::tex_id(self.id), trace::hash_arr(data, 0, n)));
        self.set_data_rect_inner(0, super::states::none(), data, 0, n);
    }
    /// `SetData<T>(T[] data, int startIndex, int elementCount)`
    pub fn set_data_range<T: Copy + Trace + 'static>(&mut self, data: Arr<T>, start: i32, count: i32) {
        trace::line(|| format!("tex.data {} b 0 - {start} {count} {}", trace::tex_id(self.id), trace::hash_arr(data, start, count)));
        self.set_data_rect_inner(0, super::states::none(), data, start, count);
    }

    /// The texture's RGBA8 pixels if the host can produce them now.
    pub fn pixels(&self) -> Option<Vec<u8>> {
        let d = dev(self.device);
        if let Some(p) = d.list.cpu_pixels(self.id) {
            return Some(p.to_vec());
        }
        if let Some(f) = &self.reload {
            return f();
        }
        if self.is_render_target {
            if let Some(hook) = d.readback_hook.as_mut() {
                let frame = d.list.take_frame();
                return Some(hook(frame, self.id));
            }
        }
        None
    }

    /// `GetData<T>(T[] data, int startIndex, int elementCount)` (whole texture, level 0).
    ///
    /// Served from: the CPU copy (textures created with `new Texture2D` / `FromStream`), a re-read of the content
    /// file (content textures), or the backend's synchronous readback hook (render targets). A render target
    /// without a hook is `unimplemented_value`.
    pub fn get_data_range<T: Copy + Trace + 'static>(&mut self, data: Arr<T>, start: i32, count: i32) {
        self.check();
        if data.is_null() {
            throw("System.ArgumentNullException", "data");
        }
        if start < 0 || count < 0 || start as i64 + count as i64 > data.len() as i64 {
            throw("System.ArgumentOutOfRangeException", "startIndex / elementCount");
        }
        let Some(p) = self.pixels() else {
            return host::unimplemented_value("Microsoft.Xna.Framework.Graphics.Texture2D.GetData<T>(T[], int, int) on a render target (no synchronous readback in the backend)");
        };
        let want = count as usize * core::mem::size_of::<T>();
        if want != p.len() {
            throw("System.ArgumentException", "The size of the data passed in is too large or too small for this resource.");
        }
        // SAFETY: `data[start..start+count]` is in bounds (checked) and `T` is a plain value type.
        unsafe { core::ptr::copy_nonoverlapping(p.as_ptr(), data.at(start) as *mut u8, want) };
    }
    /// `GetData<T>(T[] data)`
    pub fn get_data<T: Copy + Trace + 'static>(&mut self, data: Arr<T>) {
        let n = if data.is_null() { 0 } else { data.len() };
        self.get_data_range(data, 0, n);
    }
}

// ------------------------------------------------------------------------------------------------ vertices

/// `VertexPositionColorTexture`: Position (Vector3), Color, TextureCoordinate (Vector2); 24 bytes like XNA's.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VertexPositionColorTexture {
    pub position: Vector3,
    pub color: Color,
    pub texture_coordinate: Vector2,
}

/// `VertexElement` (a struct): byte offset, `VertexElementFormat`, `VertexElementUsage`, usage index.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VertexElement {
    pub offset: i32,
    pub format: i32,
    pub usage: i32,
    pub usage_index: i32,
}
impl VertexPositionColorTexture {
    /// The vertex as Direct3D hands it to a vertex shader.
    fn raw(&self) -> RawVertex {
        RawVertex { pos: [self.position.x, self.position.y, self.position.z, 1.0], tex: [self.texture_coordinate.x, self.texture_coordinate.y, 0.0, 1.0], color: self.color.to_rgba_bytes() }
    }
}

impl VertexElement {
    pub fn new(offset: i32, format: i32, usage: i32, usage_index: i32) -> VertexElement {
        VertexElement { offset, format, usage, usage_index }
    }
}

/// `VertexDeclaration`.
pub struct VertexDeclaration {
    pub elements: Vec<VertexElement>,
    pub stride: i32,
}
plain_trace!(VertexDeclaration);

fn format_size(format: i32) -> i32 {
    // VertexElementFormat: Single, Vector2, Vector3, Vector4, Color, Byte4, Short2, Short4, NormalizedShort2, NormalizedShort4, HalfVector2, HalfVector4
    match format {
        0 | 4 | 5 | 6 | 8 | 10 => 4,
        1 | 7 | 9 | 11 => 8,
        2 => 12,
        3 => 16,
        _ => throw("System.ArgumentOutOfRangeException", "VertexElementFormat"),
    }
}

impl VertexDeclaration {
    /// `new VertexDeclaration(params VertexElement[])`: the stride is the end of the last element by offset.
    pub fn new_gc(elements: Arr<VertexElement>) -> Gc<VertexDeclaration> {
        if elements.is_null() || elements.len() == 0 {
            throw("System.ArgumentNullException", "elements");
        }
        let e = elements.as_slice().to_vec();
        let stride = e.iter().map(|x| x.offset + format_size(x.format)).max().unwrap_or(0);
        rt::alloc_init(VertexDeclaration { elements: e, stride })
    }
    pub fn vertex_stride(&self) -> i32 {
        self.stride
    }
    /// The declaration of `VertexPositionColorTexture`.
    pub fn position_color_texture() -> VertexDeclaration {
        VertexDeclaration { elements: vec![VertexElement::new(0, 2, 0, 0), VertexElement::new(12, 4, 1, 0), VertexElement::new(16, 1, 2, 0)], stride: 24 }
    }

    /// Reads vertex `i` of a raw buffer: Position, Color (white when absent), TextureCoordinate 0, each expanded
    /// to four components the way Direct3D expands a shorter element: missing y, z are 0 and w is 1.
    fn read(&self, bytes: &[u8], i: usize) -> RawVertex {
        let base = i * self.stride as usize;
        let f = |o: usize| f32::from_le_bytes(bytes[base + o..base + o + 4].try_into().expect("4 bytes"));
        // VertexElementFormat Single 0, Vector2 1, Vector3 2, Vector4 3
        let floats = |o: usize, format: i32| -> Option<[f32; 4]> {
            let mut v = [0.0, 0.0, 0.0, 1.0];
            let n = match format {
                0..=3 => format as usize + 1,
                _ => return None,
            };
            for (k, slot) in v.iter_mut().enumerate().take(n) {
                *slot = f(o + k * 4);
            }
            Some(v)
        };
        let mut v = RawVertex { pos: [0.0, 0.0, 0.0, 1.0], tex: [0.0, 0.0, 0.0, 1.0], color: [255; 4] };
        for e in &self.elements {
            let o = e.offset as usize;
            // VertexElementUsage Position 0, Color 1, TextureCoordinate 2
            match (e.usage, e.usage_index) {
                (0, 0) => match floats(o, e.format) {
                    Some(p) => v.pos = p,
                    None => host::unimplemented("Microsoft.Xna.Framework.Graphics.VertexDeclaration: a Position element that is not Single / Vector2 / Vector3 / Vector4"),
                },
                (2, 0) => match floats(o, e.format) {
                    Some(t) => v.tex = t,
                    None => host::unimplemented("Microsoft.Xna.Framework.Graphics.VertexDeclaration: a TextureCoordinate element that is not Single / Vector2 / Vector3 / Vector4"),
                },
                (1, 0) if e.format == 4 => v.color = [bytes[base + o], bytes[base + o + 1], bytes[base + o + 2], bytes[base + o + 3]],
                // other elements (normals, second texture coordinates, ...): no shader of the game declares them
                _ => {}
            }
        }
        v
    }
}

/// `VertexBuffer` / `DynamicVertexBuffer`: raw bytes plus the declaration that explains them.
pub struct VertexBuffer {
    /// the draw trace's name: `vb<trace_id>`
    pub trace_id: u32,
    pub device: Gc<GraphicsDevice>,
    pub declaration: VertexDeclaration,
    pub data: Vec<u8>,
    pub vertex_count: i32,
    pub disposed: bool,
}
// SAFETY: the one handle is reported.
unsafe impl Trace for VertexBuffer {
    unsafe fn trace(&self, m: &mut Marker) {
        m.mark(self.device)
    }
}

impl VertexBuffer {
    /// `new [Dynamic]VertexBuffer(device, typeof(VertexPositionColorTexture), vertexCount, usage)`.
    /// `vertex_type` is the `Type` object; only `VertexPositionColorTexture` is known by type here (a custom vertex
    /// struct needs the `VertexDeclaration` overload: [`VertexBuffer::new_with_declaration`]).
    pub fn new_gc(device: Gc<GraphicsDevice>, vertex_type: Obj, vertex_count: i32, usage: i32) -> Gc<VertexBuffer> {
        dev(device);
        let name = rt::type_full_name(vertex_type);
        if name.as_str() != "Microsoft.Xna.Framework.Graphics.VertexPositionColorTexture" {
            return host::unimplemented_value("Microsoft.Xna.Framework.Graphics.VertexBuffer..ctor(GraphicsDevice, Type, int, BufferUsage) for a vertex type other than VertexPositionColorTexture");
        }
        let b = Self::alloc(device, VertexDeclaration::position_color_texture(), vertex_count);
        // SAFETY: fresh object.
        let id = unsafe { (*b.ptr()).trace_id };
        trace::line(|| format!("vb.new vb{id} {} {vertex_count} {usage}", name.as_str()));
        b
    }
    /// `new [Dynamic]VertexBuffer(device, vertexDeclaration, vertexCount, usage)`
    pub fn new_with_declaration(device: Gc<GraphicsDevice>, declaration: Gc<VertexDeclaration>, vertex_count: i32, _usage: i32) -> Gc<VertexBuffer> {
        dev(device);
        // SAFETY: `ptr()` checks for null.
        let d = unsafe { &*declaration.ptr() };
        Self::alloc(device, VertexDeclaration { elements: d.elements.clone(), stride: d.stride }, vertex_count)
    }
    fn alloc(device: Gc<GraphicsDevice>, declaration: VertexDeclaration, vertex_count: i32) -> Gc<VertexBuffer> {
        if vertex_count <= 0 {
            throw("System.ArgumentOutOfRangeException", "vertexCount");
        }
        let size = vertex_count as usize * declaration.stride as usize;
        rt::alloc_init(VertexBuffer { trace_id: trace::new_vertex_buffer_id(), device, declaration, data: vec![0; size], vertex_count, disposed: false })
    }
    pub fn vertex_count(&self) -> i32 {
        self.vertex_count
    }
    pub fn is_disposed(&self) -> bool {
        self.disposed
    }
    pub fn graphics_device(&self) -> Gc<GraphicsDevice> {
        self.device
    }
    pub fn dispose(&mut self) {
        self.disposed = true;
        self.data = Vec::new();
    }
    /// `DynamicVertexBuffer.IsContentLost`: never (no device reset).
    pub fn is_content_lost(&self) -> bool {
        false
    }
    /// `SetData<T>(int offsetInBytes, T[] data, int startIndex, int elementCount, int vertexStride[, SetDataOptions])`.
    /// `Discard` / `NoOverwrite` only matter to a GPU driver; the bytes are stored either way.
    pub fn set_data_at<T: Copy + Trace + 'static>(&mut self, offset_in_bytes: i32, data: Arr<T>, start: i32, count: i32, vertex_stride: i32) {
        trace::line(|| format!("vb.data vb{} {offset_in_bytes} {start} {count} {vertex_stride} {}", self.trace_id, trace::hash_arr(data, start, count)));
        self.set_data_at_inner(offset_in_bytes, data, start, count, vertex_stride)
    }
    fn set_data_at_inner<T: Copy + Trace + 'static>(&mut self, offset_in_bytes: i32, data: Arr<T>, start: i32, count: i32, vertex_stride: i32) {
        if self.disposed {
            throw("System.ObjectDisposedException", "VertexBuffer");
        }
        let src = Texture2D::bytes_of(data, start, count);
        let elem = core::mem::size_of::<T>();
        if vertex_stride != 0 && vertex_stride as usize != elem {
            host::unimplemented("Microsoft.Xna.Framework.Graphics.VertexBuffer.SetData<T> with vertexStride != sizeof(T)");
            return;
        }
        let o = offset_in_bytes as usize;
        if offset_in_bytes < 0 || o + src.len() > self.data.len() {
            throw("System.ArgumentOutOfRangeException", "The vertex data does not fit the buffer.");
        }
        self.data[o..o + src.len()].copy_from_slice(src);
    }
    /// `SetData<T>(T[] data, int startIndex, int elementCount[, SetDataOptions])`
    pub fn set_data_range<T: Copy + Trace + 'static>(&mut self, data: Arr<T>, start: i32, count: i32) {
        self.set_data_at(0, data, start, count, 0);
    }
    /// `SetData<T>(T[] data)`
    pub fn set_data<T: Copy + Trace + 'static>(&mut self, data: Arr<T>) {
        let n = if data.is_null() { 0 } else { data.len() };
        self.set_data_at(0, data, 0, n, 0);
    }
}

/// `IndexBuffer` / `DynamicIndexBuffer`.
pub struct IndexBuffer {
    /// the draw trace's name: `ib<trace_id>`
    pub trace_id: u32,
    pub device: Gc<GraphicsDevice>,
    pub indices: Vec<u32>,
    pub sixteen_bit: bool,
    pub disposed: bool,
}
// SAFETY: the one handle is reported.
unsafe impl Trace for IndexBuffer {
    unsafe fn trace(&self, m: &mut Marker) {
        m.mark(self.device)
    }
}

impl IndexBuffer {
    /// `new [Dynamic]IndexBuffer(device, typeof(short | ushort | int | uint), indexCount, usage)`.
    pub fn new_gc(device: Gc<GraphicsDevice>, index_type: Obj, index_count: i32, usage: i32) -> Gc<IndexBuffer> {
        dev(device);
        let sixteen_bit = match rt::type_full_name(index_type).as_str() {
            "System.Int16" | "System.UInt16" => true,
            "System.Int32" | "System.UInt32" => false,
            _ => throw("System.ArgumentOutOfRangeException", "indexType must be a 16 or 32 bit integer type"),
        };
        let b = Self::alloc(device, sixteen_bit, index_count);
        // SAFETY: fresh object.
        let id = unsafe { (*b.ptr()).trace_id };
        trace::line(|| format!("ib.new ib{id} {} {index_count} {usage}", if sixteen_bit { 0 } else { 1 }));
        b
    }
    /// `new [Dynamic]IndexBuffer(device, IndexElementSize, indexCount, usage)` (SixteenBits 0, ThirtyTwoBits 1).
    pub fn new_with_size(device: Gc<GraphicsDevice>, index_element_size: i32, index_count: i32, _usage: i32) -> Gc<IndexBuffer> {
        dev(device);
        Self::alloc(device, index_element_size == 0, index_count)
    }
    fn alloc(device: Gc<GraphicsDevice>, sixteen_bit: bool, index_count: i32) -> Gc<IndexBuffer> {
        if index_count <= 0 {
            throw("System.ArgumentOutOfRangeException", "indexCount");
        }
        rt::alloc_init(IndexBuffer { trace_id: trace::new_index_buffer_id(), device, indices: vec![0; index_count as usize], sixteen_bit, disposed: false })
    }
    pub fn is_disposed(&self) -> bool {
        self.disposed
    }
    pub fn graphics_device(&self) -> Gc<GraphicsDevice> {
        self.device
    }
    pub fn dispose(&mut self) {
        self.disposed = true;
        self.indices = Vec::new();
    }
    /// `SetData<T>(int offsetInBytes, T[] data, int startIndex, int elementCount)`; `T` is a 2- or 4-byte integer.
    pub fn set_data_at<T: Copy + Trace + 'static>(&mut self, offset_in_bytes: i32, data: Arr<T>, start: i32, count: i32) {
        trace::line(|| format!("ib.data ib{} {offset_in_bytes} {start} {count} {}", self.trace_id, trace::hash_arr(data, start, count)));
        if self.disposed {
            throw("System.ObjectDisposedException", "IndexBuffer");
        }
        let src = Texture2D::bytes_of(data, start, count);
        let elem = if self.sixteen_bit { 2 } else { 4 };
        if core::mem::size_of::<T>() != elem {
            throw("System.ArgumentException", "The index element type does not match the buffer's IndexElementSize.");
        }
        let first = offset_in_bytes as usize / elem;
        if offset_in_bytes < 0 || first + count as usize > self.indices.len() {
            throw("System.ArgumentOutOfRangeException", "The index data does not fit the buffer.");
        }
        for (k, c) in src.chunks_exact(elem).enumerate() {
            self.indices[first + k] = if self.sixteen_bit { u16::from_le_bytes([c[0], c[1]]) as u32 } else { u32::from_le_bytes([c[0], c[1], c[2], c[3]]) };
        }
    }
    /// `SetData<T>(T[] data)`
    pub fn set_data<T: Copy + Trace + 'static>(&mut self, data: Arr<T>) {
        let n = if data.is_null() { 0 } else { data.len() };
        self.set_data_at(0, data, 0, n);
    }
    /// `SetData<T>(T[] data, int startIndex, int elementCount)`
    pub fn set_data_range<T: Copy + Trace + 'static>(&mut self, data: Arr<T>, start: i32, count: i32) {
        self.set_data_at(0, data, start, count);
    }
}

// ------------------------------------------------------------------------------------------------ PresentationParameters

/// `PresentationParameters` (the device's copy; `GraphicsDeviceManager.ApplyChanges` writes it).
pub struct PresentationParameters {
    pub back_buffer_width: i32,
    pub back_buffer_height: i32,
    /// `SurfaceFormat.Color`
    pub back_buffer_format: i32,
    /// `DepthFormat.Depth24`
    pub depth_stencil_format: i32,
    pub render_target_usage: i32,
    pub is_full_screen: bool,
    pub multi_sample_count: i32,
    /// `PresentInterval`: Default 0, One 1, Two 2, Immediate 3
    pub presentation_interval: i32,
    /// `DeviceWindowHandle` (an `IntPtr` value)
    pub device_window_handle: isize,
}
plain_trace!(PresentationParameters);

impl PresentationParameters {
    pub fn back_buffer_width(&self) -> i32 {
        self.back_buffer_width
    }
    pub fn back_buffer_height(&self) -> i32 {
        self.back_buffer_height
    }
    pub fn back_buffer_format(&self) -> i32 {
        self.back_buffer_format
    }
    pub fn bounds(&self) -> Rectangle {
        Rectangle::new(0, 0, self.back_buffer_width, self.back_buffer_height)
    }
}

// ------------------------------------------------------------------------------------------------ GraphicsDevice

/// `EventHandler<EventArgs>`
pub type EventHandler = Delegate<dyn Fn(Obj, Obj)>;

pub struct GraphicsDevice {
    pub list: DrawList,
    pub presentation_parameters: Gc<PresentationParameters>,
    pub blend_state: Gc<BlendState>,
    pub depth_stencil_state: Gc<DepthStencilState>,
    pub rasterizer_state: Gc<RasterizerState>,
    pub sampler_states: [Gc<SamplerState>; 16],
    pub textures: [Gc<Texture2D>; 16],
    pub scissor_rectangle: Rectangle,
    pub viewport: Viewport,
    /// null: the back buffer
    pub render_target: Gc<Texture2D>,
    lazy_clear: bool,
    saved_back_buffer_lazy_clear: bool,
    pub vertex_buffer: Gc<VertexBuffer>,
    pub vertex_offset: i32,
    pub indices: Gc<IndexBuffer>,
    /// The matrix of the last `SpriteBatch` render state (see the module docs).
    pub sprite_transform: Matrix,
    /// The custom effect pass applied last; cleared by `SpriteBatch`'s own effect.
    pub applied_effect: Option<Arc<EffectSnapshot>>,
    pub sprite_begin_count: i32,
    pub sprite_immediate_begin_count: i32,
    pub device_lost: EventHandler,
    pub device_reset: EventHandler,
    pub device_resetting: EventHandler,
    pub disposing: EventHandler,
    /// `GraphicsProfile`: Reach 0, HiDef 1
    pub graphics_profile: i32,
    pub disposed: bool,
    /// Backend service for a synchronous `GetData` on a render target: replay `frame` now and return the
    /// target's RGBA8 pixels.
    pub readback_hook: Option<Box<dyn FnMut(Frame, TextureId) -> Vec<u8>>>,
    /// draws counted since the last `take_frame` (diagnostics)
    pub draw_calls: u32,
}
// SAFETY: every handle is reported.
unsafe impl Trace for GraphicsDevice {
    unsafe fn trace(&self, m: &mut Marker) {
        m.mark(self.presentation_parameters);
        m.mark(self.blend_state);
        m.mark(self.depth_stencil_state);
        m.mark(self.rasterizer_state);
        for s in self.sampler_states {
            m.mark(s);
        }
        for t in self.textures {
            m.mark(t);
        }
        m.mark(self.render_target);
        m.mark(self.vertex_buffer);
        m.mark(self.indices);
        self.device_lost.trace(m);
        self.device_reset.trace(m);
        self.device_resetting.trace(m);
        self.disposing.trace(m);
    }
}

impl GraphicsDevice {
    /// The device for a window with a `width` x `height` client area. Initial state as XNA's after creation:
    /// `BlendState.Opaque`, `DepthStencilState.Default`, `RasterizerState.CullCounterClockwise`,
    /// `SamplerState.LinearWrap` in every slot, viewport and scissor rectangle = the back buffer.
    pub fn new_gc(width: i32, height: i32) -> Gc<GraphicsDevice> {
        let p = presets();
        let mut list = DrawList::new();
        list.set_back_buffer_size(width.max(1) as u32, height.max(1) as u32);
        let pp = rt::alloc_init(PresentationParameters {
            back_buffer_width: width,
            back_buffer_height: height,
            back_buffer_format: SURFACE_FORMAT_COLOR,
            depth_stencil_format: 2,
            render_target_usage: USAGE_DISCARD_CONTENTS,
            is_full_screen: false,
            multi_sample_count: 0,
            presentation_interval: 1,
            device_window_handle: 0,
        });
        rt::alloc_init(GraphicsDevice {
            list,
            presentation_parameters: pp,
            blend_state: p.opaque,
            depth_stencil_state: p.depth_default,
            rasterizer_state: p.cull_counter_clockwise,
            sampler_states: [p.linear_wrap; 16],
            textures: [Gc::NULL; 16],
            scissor_rectangle: Rectangle::new(0, 0, width, height),
            viewport: Viewport::new(0, 0, width, height),
            render_target: Gc::NULL,
            lazy_clear: true,
            saved_back_buffer_lazy_clear: false,
            vertex_buffer: Gc::NULL,
            vertex_offset: 0,
            indices: Gc::NULL,
            sprite_transform: Matrix::IDENTITY,
            applied_effect: None,
            sprite_begin_count: 0,
            sprite_immediate_begin_count: 0,
            device_lost: Delegate::NULL,
            device_reset: Delegate::NULL,
            device_resetting: Delegate::NULL,
            disposing: Delegate::NULL,
            graphics_profile: 1,
            disposed: false,
            readback_hook: None,
            draw_calls: 0,
        })
    }

    // ---- driver side

    /// The window's client size changed: what `GraphicsDeviceManager` does through a device reset
    /// (`PresentationParameters`, viewport, scissor rectangle; the back buffer becomes the target).
    pub fn reset_back_buffer(&mut self, width: i32, height: i32) {
        // SAFETY: the device's own live object.
        let pp = unsafe { &mut *self.presentation_parameters.ptr() };
        pp.back_buffer_width = width;
        pp.back_buffer_height = height;
        self.list.set_back_buffer_size(width.max(1) as u32, height.max(1) as u32);
        self.render_target = Gc::NULL;
        self.list.set_render_target(TextureId::BACK_BUFFER, true);
        self.viewport = Viewport::new(0, 0, width, height);
        self.scissor_rectangle = Rectangle::new(0, 0, width, height);
        self.lazy_clear = true;
    }

    /// Start of `Game.Draw` (`Game.BeginDraw`): destroys the textures of collected objects.
    pub fn begin_frame(&mut self) {
        let dead = DEAD_TEXTURES.with(|d| std::mem::take(&mut *d.borrow_mut()));
        for id in dead {
            if self.list.is_alive(id) {
                self.list.destroy_texture(id);
            }
        }
    }

    /// `GraphicsDevice.Present()` (`Game.EndDraw`): throws while a render target is bound; a back buffer nobody
    /// drew into or cleared gets the discard colour; afterwards the back buffer is dirty again.
    pub fn present(&mut self) -> Frame {
        trace::line(|| "gd.present".to_owned());
        if !self.render_target.is_null() {
            throw("System.InvalidOperationException", "Cannot call Present when a render target is active.");
        }
        self.applied_effect = None;
        if self.lazy_clear {
            self.list.clear(DISCARD_COLOR);
        }
        self.lazy_clear = true;
        self.draw_calls = 0;
        self.list.take_frame()
    }

    // ---- properties

    pub fn presentation_parameters(&self) -> Gc<PresentationParameters> {
        self.presentation_parameters
    }
    pub fn is_disposed(&self) -> bool {
        self.disposed
    }
    /// `GraphicsDeviceStatus`: always Normal (0).
    pub fn graphics_device_status(&self) -> i32 {
        0
    }
    pub fn graphics_profile(&self) -> i32 {
        self.graphics_profile
    }
    pub fn viewport(&self) -> Viewport {
        self.viewport
    }
    /// `Viewport.set`: must lie inside the current target.
    pub fn set_viewport(&mut self, v: Viewport) {
        trace::line(|| format!("gd.viewport {} {} {} {} {} {}", v.x, v.y, v.width, v.height, trace::f(v.min_depth), trace::f(v.max_depth)));
        let (tw, th) = self.target_size();
        if v.x < 0 || v.y < 0 || v.width < 0 || v.height < 0 || v.x + v.width > tw || v.y + v.height > th {
            throw("System.ArgumentException", "The viewport is invalid. The viewport cannot be larger than or outside of the current render target bounds.");
        }
        self.viewport = v;
        self.list.set_viewport(IRect { x: v.x, y: v.y, w: v.width, h: v.height });
    }
    pub fn scissor_rectangle(&self) -> Rectangle {
        self.scissor_rectangle
    }
    /// `ScissorRectangle.set`: must lie inside the current target.
    pub fn set_scissor_rectangle(&mut self, r: Rectangle) {
        trace::line(|| format!("gd.scissor {}", trace::rect(r)));
        let (tw, th) = self.target_size();
        if r.x < 0 || r.y < 0 || r.width < 0 || r.height < 0 || r.x + r.width > tw || r.y + r.height > th {
            throw("System.ArgumentException", "The scissor rectangle is invalid. It cannot be larger than or outside of the current render target bounds.");
        }
        self.scissor_rectangle = r;
    }
    pub fn blend_state(&self) -> Gc<BlendState> {
        self.blend_state
    }
    pub fn set_blend_state(&mut self, s: Gc<BlendState>) {
        trace::line(|| format!("gd.blend {}", trace::blend(s)));
        // SAFETY: `ptr()` throws NullReferenceException on null... XNA throws ArgumentNullException; same failure class.
        unsafe { (*s.ptr()).is_bound = true };
        self.blend_state = s;
    }
    pub fn depth_stencil_state(&self) -> Gc<DepthStencilState> {
        self.depth_stencil_state
    }
    pub fn set_depth_stencil_state(&mut self, s: Gc<DepthStencilState>) {
        trace::line(|| format!("gd.depth {}", trace::depth(s)));
        // SAFETY: as above.
        unsafe { (*s.ptr()).is_bound = true };
        self.depth_stencil_state = s;
    }
    pub fn rasterizer_state(&self) -> Gc<RasterizerState> {
        self.rasterizer_state
    }
    pub fn set_rasterizer_state(&mut self, s: Gc<RasterizerState>) {
        trace::line(|| format!("gd.raster {}", trace::raster(s)));
        // SAFETY: as above.
        unsafe { (*s.ptr()).is_bound = true };
        self.rasterizer_state = s;
    }
    /// `SamplerStates[i]`
    pub fn sampler_state(&self, i: i32) -> Gc<SamplerState> {
        self.sampler_states[slot(i)]
    }
    pub fn set_sampler_state(&mut self, i: i32, s: Gc<SamplerState>) {
        trace::line(|| format!("gd.sampler {i} {}", trace::sampler(s)));
        // SAFETY: as above.
        unsafe { (*s.ptr()).is_bound = true };
        self.sampler_states[slot(i)] = s;
    }
    /// `Textures[i]`
    pub fn texture(&self, i: i32) -> Gc<Texture2D> {
        self.textures[slot(i)]
    }
    pub fn set_texture(&mut self, i: i32, t: Gc<Texture2D>) {
        trace::line(|| format!("gd.texture {i} {}", trace::tex(t)));
        self.textures[slot(i)] = t;
    }
    pub fn indices(&self) -> Gc<IndexBuffer> {
        self.indices
    }
    pub fn set_indices(&mut self, b: Gc<IndexBuffer>) {
        // SAFETY: checked non-null.
        trace::line(|| format!("gd.ib {}", if b.is_null() { "-".to_owned() } else { format!("ib{}", unsafe { (*b.ptr()).trace_id }) }));
        self.indices = b;
    }
    /// `SetVertexBuffer(vertexBuffer)` / `(vertexBuffer, vertexOffset)`; null unbinds.
    pub fn set_vertex_buffer(&mut self, b: Gc<VertexBuffer>, vertex_offset: i32) {
        // SAFETY: checked non-null.
        trace::line(|| format!("gd.vb {} {vertex_offset}", if b.is_null() { "-".to_owned() } else { format!("vb{}", unsafe { (*b.ptr()).trace_id }) }));
        self.vertex_buffer = b;
        self.vertex_offset = vertex_offset;
    }

    fn target_size(&self) -> (i32, i32) {
        if self.render_target.is_null() {
            // SAFETY: the device's own live object.
            let pp = unsafe { &*self.presentation_parameters.ptr() };
            (pp.back_buffer_width, pp.back_buffer_height)
        } else {
            // SAFETY: a bound target is alive (the device traces it).
            let t = unsafe { &*self.render_target.ptr() };
            (t.width, t.height)
        }
    }

    // ---- targets and clearing

    /// `SetRenderTarget(renderTarget)`; null is the back buffer. See the module docs for the rules.
    pub fn set_render_target(&mut self, target: Gc<Texture2D>) {
        trace::line(|| format!("gd.target {}", trace::tex(target)));
        if target == self.render_target {
            return;
        }
        let mut usage = USAGE_DISCARD_CONTENTS;
        let mut id = TextureId::BACK_BUFFER;
        if !target.is_null() {
            // SAFETY: checked non-null.
            let t = unsafe { &*target.ptr() };
            t.check();
            if !t.is_render_target {
                throw("System.ArgumentException", "The texture is not a render target.");
            }
            usage = t.usage;
            id = t.id;
        }
        if self.render_target.is_null() {
            self.saved_back_buffer_lazy_clear = self.lazy_clear;
        } else if self.lazy_clear {
            self.list.clear(DISCARD_COLOR);
        }
        self.render_target = target;
        // contents are never discarded by the backend: the discard colour is an explicit Clear (lazy, as in XNA)
        self.list.set_render_target(id, true);
        if target.is_null() {
            // SAFETY: the device's own live object.
            let pp_usage = unsafe { (*self.presentation_parameters.ptr()).render_target_usage };
            self.lazy_clear = if pp_usage == USAGE_DISCARD_CONTENTS { true } else { self.saved_back_buffer_lazy_clear };
        } else {
            self.lazy_clear = usage == USAGE_DISCARD_CONTENTS;
        }
        let (w, h) = self.target_size();
        self.viewport = Viewport::new(0, 0, w, h);
    }

    /// `GetRenderTargets()`: the bound target as a one-element array, or an empty array for the back buffer (XNA's
    /// answer when no render target is set).
    pub fn get_render_targets(&self) -> rt::Arr<RenderTargetBinding> {
        if self.render_target.is_null() {
            rt::Arr::from_slice(&[])
        } else {
            rt::Arr::from_slice(&[RenderTargetBinding { render_target: self.render_target }])
        }
    }
    /// `SetRenderTargets(params RenderTargetBinding[])`: null or empty binds the back buffer. The game restores what
    /// [`GraphicsDevice::get_render_targets`] gave it (`WorldSceneLayerTarget`); more than one target (MRT) is not
    /// something this host renders and stops.
    pub fn set_render_targets(&mut self, bindings: rt::Arr<RenderTargetBinding>) {
        let n = if bindings.is_null() { 0 } else { bindings.len() };
        trace::line(|| {
            let mut s = format!("gd.targets {n}");
            for b in if bindings.is_null() { &[][..] } else { bindings.as_slice() } {
                s.push(' ');
                s.push_str(&trace::tex(b.render_target));
            }
            s
        });
        match n {
            0 => trace::internal(|| self.set_render_target(Gc::NULL)),
            // SAFETY: index checked by the length.
            1 => trace::internal(|| self.set_render_target(unsafe { (*bindings.at(0)).render_target })),
            _ => {
                let _: () = host::unimplemented_value("Microsoft.Xna.Framework.Graphics.GraphicsDevice.SetRenderTargets with more than one target");
            }
        }
    }

    /// `GetRenderTargets()` reduced to the one target the game binds: null for the back buffer.
    pub fn render_target(&self) -> Gc<Texture2D> {
        self.render_target
    }

    /// `Clear(color)`: the whole target (XNA clears the viewport; the game clears with a full-target viewport.
    /// A `Clear` under a partial viewport is reported through the registry instead of being drawn wrong).
    pub fn clear(&mut self, color: Color) {
        trace::line(|| format!("gd.clear {}", trace::color(color)));
        let (w, h) = self.target_size();
        if self.viewport.x != 0 || self.viewport.y != 0 || self.viewport.width != w || self.viewport.height != h {
            host::unimplemented("Microsoft.Xna.Framework.Graphics.GraphicsDevice.Clear(Color) with a viewport smaller than the target (the whole target is cleared)");
        }
        self.lazy_clear = false;
        self.list.clear(color);
    }

    // ---- drawing

    /// SpriteBatch's vertex stage (`SpriteEffect`'s vertex shader, bound since the last batch's render state).
    fn clip(&self, p: [f32; 4]) -> [f32; 2] {
        self.list.to_clip(transform_xyz(Vector2::new(p[0], p[1]), p[2], &self.sprite_transform))
    }

    /// State every draw needs: dirty-target clear, scissor, cull. Returns (texture, blend, sampler), or `None` when
    /// nothing can be drawn (no texture in slot 0: Direct3D would sample black; no caller in the game does this).
    fn pre_draw(&mut self) -> Option<(TextureId, BlendDesc, SamplerDesc)> {
        if self.lazy_clear {
            self.lazy_clear = false;
            self.list.clear(DISCARD_COLOR);
        }
        // SAFETY: state objects are never null (setters dereference them) and are traced by the device.
        let (r, blend, sampler) = unsafe { (&*self.rasterizer_state.ptr(), (*self.blend_state.ptr()).desc, (*self.sampler_states[0].ptr()).desc) };
        let s = self.scissor_rectangle;
        self.list.set_scissor(if r.scissor_test_enable { Some(IRect { x: s.x, y: s.y, w: s.width, h: s.height }) } else { None });
        self.list.set_cull(r.cull_mode);
        if self.textures[0].is_null() {
            host::unimplemented("Microsoft.Xna.Framework.Graphics.GraphicsDevice draw with Textures[0] == null (nothing is drawn)");
            return None;
        }
        // SAFETY: checked non-null.
        let t = unsafe { &*self.textures[0].ptr() };
        t.check();
        Some((t.id, blend, sampler))
    }

    /// The primitives of one XNA draw call, expanded to a list. `vertex(i)` is vertex `i` of the stream after
    /// index lookup.
    fn emit(&mut self, primitive_type: i32, primitive_count: i32, vertex: &dyn Fn(usize) -> RawVertex) {
        if primitive_count <= 0 {
            throw("System.ArgumentOutOfRangeException", "primitiveCount");
        }
        let Some((texture, blend, sampler)) = self.pre_draw() else { return };
        let n = primitive_count as usize;
        // under a pass with its own vertex shader the vertices go to the backend as they are
        let raw = self.applied_effect.as_ref().is_some_and(|e| e.vertex_shader);
        let first = if raw { self.list.raw_vertex_count() } else { self.list.vertex_count() };
        let push = |d: &mut GraphicsDevice, i: usize| {
            let v = vertex(i);
            if raw {
                d.list.push_raw_vertices(&[v]);
            } else {
                let pos = d.clip(v.pos);
                d.list.push_vertices(&[Vertex { pos, uv: [v.tex[0], v.tex[1]], color: v.color }]);
            }
        };
        // PrimitiveType: TriangleList 0, TriangleStrip 1, LineList 2, LineStrip 3
        let topology = match primitive_type {
            0 => {
                for i in 0..n * 3 {
                    push(self, i);
                }
                Topology::Triangles
            }
            1 => {
                // odd triangles swap two vertices so every triangle keeps the strip's winding (Direct3D's rule)
                for t in 0..n {
                    let (a, b, c) = if t % 2 == 0 { (t, t + 1, t + 2) } else { (t + 1, t, t + 2) };
                    push(self, a);
                    push(self, b);
                    push(self, c);
                }
                Topology::Triangles
            }
            2 => {
                for i in 0..n * 2 {
                    push(self, i);
                }
                Topology::Lines
            }
            3 => {
                for l in 0..n {
                    push(self, l);
                    push(self, l + 1);
                }
                Topology::Lines
            }
            _ => throw("System.ArgumentOutOfRangeException", "primitiveType"),
        };
        let effect = self.effect_for_draw();
        match (raw, effect) {
            (true, Some(e)) => {
                let count = self.list.raw_vertex_count() - first;
                self.list.draw_raw_range(texture, blend, sampler, topology, e, first, count);
            }
            (_, effect) => self.list.draw(texture, blend, sampler, topology, effect, first),
        }
        self.draw_calls += 1;
    }

    /// The applied effect pass as a draw records it: with the textures bound to slots 1..=3 right now.
    pub(crate) fn effect_for_draw(&self) -> Option<Arc<EffectSnapshot>> {
        let e = self.applied_effect.as_ref()?;
        let mut extra = Vec::new();
        for slot in 1..=3usize {
            if !self.textures[slot].is_null() {
                // SAFETY: checked non-null; state objects are never null.
                unsafe { extra.push((slot as u8, (*self.textures[slot].ptr()).id, (*self.sampler_states[slot].ptr()).desc)) };
            }
        }
        if extra == e.extra_textures {
            return Some(e.clone());
        }
        let mut s = (**e).clone();
        s.extra_textures = extra;
        Some(Arc::new(s))
    }

    /// Sprite vertices already in clip space (from `SpriteBatch`): one draw per call.
    pub(crate) fn draw_sprite_range(&mut self, texture: TextureId, first_vertex: u32, vertex_count: u32, effect: Option<Arc<EffectSnapshot>>, raw: bool) {
        // SAFETY: as in `pre_draw`.
        let (blend, sampler) = unsafe { ((*self.blend_state.ptr()).desc, (*self.sampler_states[0].ptr()).desc) };
        match (raw, effect) {
            (true, Some(e)) => self.list.draw_raw_range(texture, blend, sampler, Topology::Triangles, e, first_vertex, vertex_count),
            (_, effect) => self.list.draw_range(texture, blend, sampler, Topology::Triangles, effect, first_vertex, vertex_count),
        }
        self.draw_calls += 1;
    }

    /// The part of `pre_draw` a sprite flush needs before it writes vertices.
    pub(crate) fn pre_sprite_draw(&mut self) {
        if self.lazy_clear {
            self.lazy_clear = false;
            self.list.clear(DISCARD_COLOR);
        }
        // SAFETY: as in `pre_draw`.
        let r = unsafe { &*self.rasterizer_state.ptr() };
        let s = self.scissor_rectangle;
        self.list.set_scissor(if r.scissor_test_enable { Some(IRect { x: s.x, y: s.y, w: s.width, h: s.height }) } else { None });
        self.list.set_cull(r.cull_mode);
    }

    fn vertex_stream(&self) -> &VertexBuffer {
        if self.vertex_buffer.is_null() {
            throw("System.InvalidOperationException", "A valid vertex buffer (and a valid index buffer if you are using indexed primitives) must be set on the device before any draw operations may be performed.");
        }
        // SAFETY: checked non-null; traced by the device.
        unsafe { &*self.vertex_buffer.ptr() }
    }

    /// `DrawIndexedPrimitives(primitiveType, baseVertex, minVertexIndex, numVertices, startIndex, primitiveCount)`
    /// from the bound vertex and index buffers.
    pub fn draw_indexed_primitives(&mut self, primitive_type: i32, base_vertex: i32, _min_vertex_index: i32, _num_vertices: i32, start_index: i32, primitive_count: i32) {
        trace::line(|| format!("gd.dip {primitive_type} {base_vertex} {_min_vertex_index} {_num_vertices} {start_index} {primitive_count}"));
        let this = self as *mut GraphicsDevice;
        let vb = self.vertex_stream();
        if self.indices.is_null() {
            throw("System.InvalidOperationException", "A valid index buffer must be set on the device before indexed primitives may be drawn.");
        }
        // SAFETY: checked non-null; traced by the device.
        let ib = unsafe { &*self.indices.ptr() };
        let base = (self.vertex_offset + base_vertex) as usize;
        let start = start_index as usize;
        let vertex = |i: usize| {
            let Some(index) = ib.indices.get(start + i) else { throw("System.InvalidOperationException", "The index buffer is too small for the draw call.") };
            let v = base + *index as usize;
            if (v + 1) * vb.declaration.stride as usize > vb.data.len() {
                throw("System.InvalidOperationException", "The vertex buffer is too small for the draw call.");
            }
            vb.declaration.read(&vb.data, v)
        };
        // SAFETY: `emit` does not touch the vertex / index buffer objects, which live in their own allocations.
        unsafe { (*this).emit(primitive_type, primitive_count, &vertex) };
    }

    /// `DrawPrimitives(primitiveType, startVertex, primitiveCount)` from the bound vertex buffer.
    pub fn draw_primitives(&mut self, primitive_type: i32, start_vertex: i32, primitive_count: i32) {
        trace::line(|| format!("gd.dp {primitive_type} {start_vertex} {primitive_count}"));
        let this = self as *mut GraphicsDevice;
        let vb = self.vertex_stream();
        let base = (self.vertex_offset + start_vertex) as usize;
        let vertex = |i: usize| {
            if (base + i + 1) * vb.declaration.stride as usize > vb.data.len() {
                throw("System.InvalidOperationException", "The vertex buffer is too small for the draw call.");
            }
            vb.declaration.read(&vb.data, base + i)
        };
        // SAFETY: as in `draw_indexed_primitives`.
        unsafe { (*this).emit(primitive_type, primitive_count, &vertex) };
    }

    /// `DrawUserPrimitives<VertexPositionColorTexture>(primitiveType, vertexData, vertexOffset, primitiveCount)`.
    pub fn draw_user_primitives(&mut self, primitive_type: i32, vertex_data: Arr<VertexPositionColorTexture>, vertex_offset: i32, primitive_count: i32) {
        let v = vertex_data.as_slice();
        let vertex = |i: usize| match v.get(vertex_offset as usize + i) {
            Some(x) => x.raw(),
            None => throw("System.ArgumentOutOfRangeException", "The vertex array is too small for the draw call."),
        };
        self.emit(primitive_type, primitive_count, &vertex);
    }

    /// `DrawUserIndexedPrimitives<VertexPositionColorTexture>(primitiveType, vertexData, vertexOffset, numVertices,
    /// short[] indexData, indexOffset, primitiveCount)`.
    #[allow(clippy::too_many_arguments)]
    pub fn draw_user_indexed_primitives(&mut self, primitive_type: i32, vertex_data: Arr<VertexPositionColorTexture>, vertex_offset: i32, _num_vertices: i32, index_data: Arr<i16>, index_offset: i32, primitive_count: i32) {
        let v = vertex_data.as_slice();
        let idx = index_data.as_slice();
        let vertex = |i: usize| match idx.get(index_offset as usize + i).and_then(|k| v.get(vertex_offset as usize + *k as u16 as usize)) {
            Some(x) => x.raw(),
            None => throw("System.ArgumentOutOfRangeException", "The vertex or index array is too small for the draw call."),
        };
        self.emit(primitive_type, primitive_count, &vertex);
    }

    /// `DrawUserIndexedPrimitives<T>(...)` for a vertex struct described by a `VertexDeclaration` (the game's
    /// `VertexStrip.CustomVertexInfo`). XNA takes the declaration from `T : IVertexType`; a host function cannot
    /// ask a translated struct for it, so the caller passes it (REQUESTS.md, "IVertexType").
    #[allow(clippy::too_many_arguments)]
    pub fn draw_user_indexed_primitives_decl<T: Copy + Trace + 'static>(
        &mut self,
        primitive_type: i32,
        vertex_data: Arr<T>,
        vertex_offset: i32,
        num_vertices: i32,
        index_data: Arr<i16>,
        index_offset: i32,
        primitive_count: i32,
        declaration: Gc<VertexDeclaration>,
    ) {
        trace::line(|| {
            // PrimitiveType: TriangleList 0, TriangleStrip 1, LineList 2, LineStrip 3 (Rec.cs IndexCount)
            let indices = match primitive_type {
                0 => primitive_count * 3,
                1 => primitive_count + 2,
                2 => primitive_count * 2,
                3 => primitive_count + 1,
                _ => 0,
            };
            format!(
                "gd.duip {primitive_type} {vertex_offset} {num_vertices} {index_offset} {primitive_count} {} {}",
                trace::hash_arr(vertex_data, vertex_offset, num_vertices),
                trace::hash_arr(index_data, index_offset, indices)
            )
        });
        // SAFETY: `ptr()` checks for null.
        let d = unsafe { &*declaration.ptr() };
        if d.stride as usize != core::mem::size_of::<T>() {
            throw("System.ArgumentException", "The vertex declaration's stride does not match the vertex type.");
        }
        let n = if vertex_data.is_null() { 0 } else { vertex_data.len() };
        let bytes = Texture2D::bytes_of(vertex_data, 0, n);
        let idx = index_data.as_slice();
        let _ = num_vertices;
        let vertex = |i: usize| {
            let Some(k) = idx.get(index_offset as usize + i) else { throw("System.ArgumentOutOfRangeException", "The index array is too small for the draw call.") };
            let v = vertex_offset as usize + *k as u16 as usize;
            if v >= n as usize {
                throw("System.ArgumentOutOfRangeException", "The vertex array is too small for the draw call.");
            }
            d.read(bytes, v)
        };
        self.emit(primitive_type, primitive_count, &vertex);
    }

    // ---- events (`DeviceLost` / `DeviceReset` / `DeviceResetting` never fire: the device is never lost)

    pub fn add_device_lost(&mut self, h: EventHandler) {
        self.device_lost = Delegate::combine(self.device_lost, h);
    }
    pub fn add_device_reset(&mut self, h: EventHandler) {
        self.device_reset = Delegate::combine(self.device_reset, h);
    }
    pub fn add_device_resetting(&mut self, h: EventHandler) {
        self.device_resetting = Delegate::combine(self.device_resetting, h);
    }
    pub fn remove_device_reset(&mut self, h: EventHandler) {
        self.device_reset = Delegate::remove(self.device_reset, h);
    }
}

/// `VertexBufferBinding` (a struct): buffer + vertex offset.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct VertexBufferBinding {
    pub vertex_buffer: Gc<VertexBuffer>,
    pub vertex_offset: i32,
    pub instance_frequency: i32,
}
// SAFETY: the one handle is reported.
unsafe impl Trace for VertexBufferBinding {
    unsafe fn trace(&self, m: &mut Marker) {
        m.mark(self.vertex_buffer)
    }
}

impl GraphicsDevice {
    /// `GetVertexBuffers()`: the one stream this device has; empty when none is bound.
    pub fn get_vertex_buffers(&self) -> Arr<VertexBufferBinding> {
        if self.vertex_buffer.is_null() {
            Arr::new(0)
        } else {
            Arr::from_slice(&[VertexBufferBinding { vertex_buffer: self.vertex_buffer, vertex_offset: self.vertex_offset, instance_frequency: 0 }])
        }
    }
    /// `SetVertexBuffers(params VertexBufferBinding[])`: stream 0 (the game binds one stream); none unbinds.
    pub fn set_vertex_buffers(&mut self, bindings: Arr<VertexBufferBinding>) {
        trace::line(|| {
            let mut s = format!("gd.vbs {}", if bindings.is_null() { 0 } else { bindings.len() });
            for b in if bindings.is_null() { &[][..] } else { bindings.as_slice() } {
                // SAFETY: checked non-null.
                let vb = if b.vertex_buffer.is_null() { "-".to_owned() } else { format!("vb{}", unsafe { (*b.vertex_buffer.ptr()).trace_id }) };
                s.push_str(&format!(" {vb},{},{}", b.vertex_offset, b.instance_frequency));
            }
            s
        });
        trace::internal(|| match if bindings.is_null() { None } else { bindings.as_slice().first() } {
            Some(b) => self.set_vertex_buffer(b.vertex_buffer, b.vertex_offset),
            None => self.set_vertex_buffer(Gc::NULL, 0),
        });
        if !bindings.is_null() && bindings.len() > 1 {
            host::unimplemented("Microsoft.Xna.Framework.Graphics.GraphicsDevice.SetVertexBuffers with more than one stream (only stream 0 is used)");
        }
    }
    /// `Adapter`
    pub fn adapter(&self) -> Gc<crate::game::GraphicsAdapter> {
        crate::game::GraphicsAdapter::default_adapter()
    }
}

fn slot(i: i32) -> usize {
    if !(0..16).contains(&i) {
        throw("System.ArgumentOutOfRangeException", "index");
    }
    i as usize
}
