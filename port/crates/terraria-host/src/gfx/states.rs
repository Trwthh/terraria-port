//! `BlendState`, `SamplerState`, `RasterizerState`, `DepthStencilState`: objects with XNA's rules.
//!
//! - Default constructor values and the presets are XNA's (reference/xna/{BlendState,SamplerState,RasterizerState,
//!   DepthStencilState}.cs).
//! - A state object that has been bound to the device is immutable: a setter throws `InvalidOperationException`
//!   (XNA's `ThrowIfBound`). The presets are bound from the start.
//! - Replayed: every `BlendState` field except `MultiSampleMask`; `SamplerState.Filter/AddressU/AddressV`;
//!   `RasterizerState.CullMode/ScissorTestEnable`. Stored but NOT replayed (the backend has no depth buffer and no
//!   MSAA): all of `DepthStencilState`, `RasterizerState.FillMode/DepthBias/SlopeScaleDepthBias/MultiSampleAntiAlias`,
//!   `SamplerState.AddressW/MaxAnisotropy/MaxMipLevel/MipMapLevelOfDetailBias`.

use std::cell::RefCell;

use terraria_core::xna::Color;
use terraria_gen_rt::rt::{self, Gc, Marker, Rooted, Str, Trace};

use super::draw_list::{Blend, BlendDesc, BlendFunction, CullMode, SamplerDesc, TextureAddressMode, TextureFilter};

#[cold]
pub(crate) fn throw(kind: &'static str, message: &'static str) -> ! {
    rt::throw(rt::exc(kind, Str::lit(message)))
}

/// `null` of a `T?`.
pub(crate) fn none<T: Copy>() -> rt::Nullable<T> {
    // SAFETY: all-zero is `HasValue == false` with a zeroed value, valid for every value type of the runtime.
    unsafe { rt::zero() }
}

#[cold]
fn bound() -> ! {
    throw("System.InvalidOperationException", "Cannot change read-only state object: it has been bound to a GraphicsDevice or is a default state.")
}

fn enum_arg<T>(v: Option<T>) -> T {
    match v {
        Some(v) => v,
        None => throw("System.ArgumentOutOfRangeException", "value is not a member of the enumeration"),
    }
}

macro_rules! plain_trace {
    ($($t:ty),*) => { $(
        // SAFETY: no handles inside.
        unsafe impl terraria_gen_rt::rt::Trace for $t {
            const NEEDS_TRACE: bool = false;
            unsafe fn trace(&self, _m: &mut terraria_gen_rt::rt::Marker) {}
        }
    )* };
}
pub(crate) use plain_trace;

// ------------------------------------------------------------------------------------------------ BlendState

pub struct BlendState {
    pub desc: BlendDesc,
    pub multi_sample_mask: i32,
    pub is_bound: bool,
}
plain_trace!(BlendState, SamplerState, RasterizerState, DepthStencilState);

impl BlendState {
    /// `new BlendState()`: One / Zero, Add, factor white, all channels (the same values as `Opaque`).
    pub fn new_gc() -> Gc<BlendState> {
        rt::alloc_init(BlendState { desc: BlendDesc::Opaque, multi_sample_mask: -1, is_bound: false })
    }
    fn preset(desc: BlendDesc) -> Gc<BlendState> {
        rt::alloc_init(BlendState { desc, multi_sample_mask: -1, is_bound: true })
    }
    fn edit(&mut self) -> &mut BlendDesc {
        if self.is_bound {
            bound()
        }
        &mut self.desc
    }
    pub fn color_source_blend(&self) -> i32 {
        self.desc.color_src as i32
    }
    pub fn set_color_source_blend(&mut self, v: i32) {
        self.edit().color_src = enum_arg(Blend::from_i32(v));
    }
    pub fn color_destination_blend(&self) -> i32 {
        self.desc.color_dst as i32
    }
    pub fn set_color_destination_blend(&mut self, v: i32) {
        self.edit().color_dst = enum_arg(Blend::from_i32(v));
    }
    pub fn color_blend_function(&self) -> i32 {
        self.desc.color_fn as i32
    }
    pub fn set_color_blend_function(&mut self, v: i32) {
        self.edit().color_fn = enum_arg(BlendFunction::from_i32(v));
    }
    pub fn alpha_source_blend(&self) -> i32 {
        self.desc.alpha_src as i32
    }
    pub fn set_alpha_source_blend(&mut self, v: i32) {
        self.edit().alpha_src = enum_arg(Blend::from_i32(v));
    }
    pub fn alpha_destination_blend(&self) -> i32 {
        self.desc.alpha_dst as i32
    }
    pub fn set_alpha_destination_blend(&mut self, v: i32) {
        self.edit().alpha_dst = enum_arg(Blend::from_i32(v));
    }
    pub fn alpha_blend_function(&self) -> i32 {
        self.desc.alpha_fn as i32
    }
    pub fn set_alpha_blend_function(&mut self, v: i32) {
        self.edit().alpha_fn = enum_arg(BlendFunction::from_i32(v));
    }
    pub fn blend_factor(&self) -> Color {
        let f = self.desc.blend_factor;
        Color::new(f[0], f[1], f[2], f[3])
    }
    pub fn set_blend_factor(&mut self, c: Color) {
        self.edit().blend_factor = c.to_rgba_bytes();
    }
    /// `ColorWriteChannels` (flags: Red 1, Green 2, Blue 4, Alpha 8).
    pub fn color_write_channels(&self) -> i32 {
        self.desc.write_mask as i32
    }
    pub fn set_color_write_channels(&mut self, v: i32) {
        self.edit().write_mask = (v & 15) as u8;
    }
}

// ------------------------------------------------------------------------------------------------ SamplerState

pub struct SamplerState {
    pub desc: SamplerDesc,
    pub address_w: i32,
    pub max_anisotropy: i32,
    pub max_mip_level: i32,
    pub mip_map_level_of_detail_bias: f32,
    pub is_bound: bool,
}

impl SamplerState {
    /// `new SamplerState()`: Linear, Wrap, MaxAnisotropy 4 (the same values as `LinearWrap`).
    pub fn new_gc() -> Gc<SamplerState> {
        Self::make(SamplerDesc::LinearWrap, false)
    }
    fn make(desc: SamplerDesc, is_bound: bool) -> Gc<SamplerState> {
        rt::alloc_init(SamplerState { desc, address_w: desc.address_u as i32, max_anisotropy: 4, max_mip_level: 0, mip_map_level_of_detail_bias: 0.0, is_bound })
    }
    fn check(&self) {
        if self.is_bound {
            bound()
        }
    }
    pub fn filter(&self) -> i32 {
        self.desc.filter as i32
    }
    pub fn set_filter(&mut self, v: i32) {
        self.check();
        self.desc.filter = enum_arg(TextureFilter::from_i32(v));
    }
    pub fn address_u(&self) -> i32 {
        self.desc.address_u as i32
    }
    pub fn set_address_u(&mut self, v: i32) {
        self.check();
        self.desc.address_u = enum_arg(TextureAddressMode::from_i32(v));
    }
    pub fn address_v(&self) -> i32 {
        self.desc.address_v as i32
    }
    pub fn set_address_v(&mut self, v: i32) {
        self.check();
        self.desc.address_v = enum_arg(TextureAddressMode::from_i32(v));
    }
    pub fn set_address_w(&mut self, v: i32) {
        self.check();
        self.address_w = enum_arg(TextureAddressMode::from_i32(v)) as i32;
    }
}

// ------------------------------------------------------------------------------------------------ RasterizerState

pub struct RasterizerState {
    pub cull_mode: CullMode,
    /// `FillMode`: Solid 0, WireFrame 1
    pub fill_mode: i32,
    pub scissor_test_enable: bool,
    pub multi_sample_anti_alias: bool,
    pub depth_bias: f32,
    pub slope_scale_depth_bias: f32,
    pub is_bound: bool,
}

impl RasterizerState {
    /// `new RasterizerState()`: CullCounterClockwiseFace, Solid, no scissor test, MultiSampleAntiAlias on.
    pub fn new_gc() -> Gc<RasterizerState> {
        Self::make(CullMode::CullCounterClockwiseFace, false)
    }
    fn make(cull_mode: CullMode, is_bound: bool) -> Gc<RasterizerState> {
        rt::alloc_init(RasterizerState { cull_mode, fill_mode: 0, scissor_test_enable: false, multi_sample_anti_alias: true, depth_bias: 0.0, slope_scale_depth_bias: 0.0, is_bound })
    }
    fn check(&self) {
        if self.is_bound {
            bound()
        }
    }
    pub fn cull_mode(&self) -> i32 {
        self.cull_mode as i32
    }
    pub fn set_cull_mode(&mut self, v: i32) {
        self.check();
        self.cull_mode = enum_arg(CullMode::from_i32(v));
    }
    pub fn scissor_test_enable(&self) -> bool {
        self.scissor_test_enable
    }
    pub fn set_scissor_test_enable(&mut self, v: bool) {
        self.check();
        self.scissor_test_enable = v;
    }
    pub fn set_fill_mode(&mut self, v: i32) {
        self.check();
        if !(0..=1).contains(&v) {
            enum_arg::<()>(None);
        }
        self.fill_mode = v;
    }
    pub fn set_multi_sample_anti_alias(&mut self, v: bool) {
        self.check();
        self.multi_sample_anti_alias = v;
    }
}

// ------------------------------------------------------------------------------------------------ DepthStencilState

/// Stored, never replayed: the backend renders without a depth / stencil buffer. With `SpriteBatch` this is only
/// observable when sprites with different `layerDepth` overlap while a depth-testing state is set.
pub struct DepthStencilState {
    pub depth_buffer_enable: bool,
    pub depth_buffer_write_enable: bool,
    /// `CompareFunction` (default LessEqual = 3)
    pub depth_buffer_function: i32,
    pub stencil_enable: bool,
    pub is_bound: bool,
}

impl DepthStencilState {
    /// `new DepthStencilState()`: depth test and write on, LessEqual, stencil off (the same values as `Default`).
    pub fn new_gc() -> Gc<DepthStencilState> {
        Self::make(true, true, false)
    }
    fn make(enable: bool, write: bool, is_bound: bool) -> Gc<DepthStencilState> {
        rt::alloc_init(DepthStencilState { depth_buffer_enable: enable, depth_buffer_write_enable: write, depth_buffer_function: 3, stencil_enable: false, is_bound })
    }
    fn check(&self) {
        if self.is_bound {
            bound()
        }
    }
    pub fn depth_buffer_enable(&self) -> bool {
        self.depth_buffer_enable
    }
    pub fn set_depth_buffer_enable(&mut self, v: bool) {
        self.check();
        self.depth_buffer_enable = v;
    }
    pub fn set_depth_buffer_write_enable(&mut self, v: bool) {
        self.check();
        self.depth_buffer_write_enable = v;
    }
    pub fn set_depth_buffer_function(&mut self, v: i32) {
        self.check();
        self.depth_buffer_function = v;
    }
    pub fn set_stencil_enable(&mut self, v: bool) {
        self.check();
        self.stencil_enable = v;
    }
}

// ------------------------------------------------------------------------------------------------ presets

/// The static preset objects (`BlendState.AlphaBlend`, ...): one object each per thread, kept alive as roots.
#[derive(Clone, Copy)]
pub struct Presets {
    pub additive: Gc<BlendState>,
    pub alpha_blend: Gc<BlendState>,
    pub non_premultiplied: Gc<BlendState>,
    pub opaque: Gc<BlendState>,
    pub anisotropic_clamp: Gc<SamplerState>,
    pub anisotropic_wrap: Gc<SamplerState>,
    pub linear_clamp: Gc<SamplerState>,
    pub linear_wrap: Gc<SamplerState>,
    pub point_clamp: Gc<SamplerState>,
    pub point_wrap: Gc<SamplerState>,
    pub cull_clockwise: Gc<RasterizerState>,
    pub cull_counter_clockwise: Gc<RasterizerState>,
    pub cull_none: Gc<RasterizerState>,
    pub depth_default: Gc<DepthStencilState>,
    pub depth_read: Gc<DepthStencilState>,
    pub depth_none: Gc<DepthStencilState>,
}
// SAFETY: every handle is reported.
unsafe impl Trace for Presets {
    unsafe fn trace(&self, m: &mut Marker) {
        for b in [self.additive, self.alpha_blend, self.non_premultiplied, self.opaque] {
            m.mark(b);
        }
        for s in [self.anisotropic_clamp, self.anisotropic_wrap, self.linear_clamp, self.linear_wrap, self.point_clamp, self.point_wrap] {
            m.mark(s);
        }
        for r in [self.cull_clockwise, self.cull_counter_clockwise, self.cull_none] {
            m.mark(r);
        }
        for d in [self.depth_default, self.depth_read, self.depth_none] {
            m.mark(d);
        }
    }
}

thread_local! {
    static PRESETS: RefCell<Option<Rooted<Presets>>> = const { RefCell::new(None) };
}

/// The preset objects of this thread (created on first use).
pub fn presets() -> Presets {
    PRESETS.with(|p| {
        if let Some(r) = p.borrow().as_ref() {
            return r.get();
        }
        let v = Presets {
            additive: BlendState::preset(BlendDesc::Additive),
            alpha_blend: BlendState::preset(BlendDesc::AlphaBlend),
            non_premultiplied: BlendState::preset(BlendDesc::NonPremultiplied),
            opaque: BlendState::preset(BlendDesc::Opaque),
            anisotropic_clamp: SamplerState::make(SamplerDesc::AnisotropicClamp, true),
            anisotropic_wrap: SamplerState::make(SamplerDesc::AnisotropicWrap, true),
            linear_clamp: SamplerState::make(SamplerDesc::LinearClamp, true),
            linear_wrap: SamplerState::make(SamplerDesc::LinearWrap, true),
            point_clamp: SamplerState::make(SamplerDesc::PointClamp, true),
            point_wrap: SamplerState::make(SamplerDesc::PointWrap, true),
            cull_clockwise: RasterizerState::make(CullMode::CullClockwiseFace, true),
            cull_counter_clockwise: RasterizerState::make(CullMode::CullCounterClockwiseFace, true),
            cull_none: RasterizerState::make(CullMode::None, true),
            depth_default: DepthStencilState::make(true, true, true),
            depth_read: DepthStencilState::make(true, false, true),
            depth_none: DepthStencilState::make(false, false, true),
        };
        *p.borrow_mut() = Some(Rooted::new(v));
        v
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn presets_have_xna_values_and_identity() {
        let p = presets();
        assert_eq!(p.alpha_blend, presets().alpha_blend, "BlendState.AlphaBlend is one object");
        unsafe {
            let a = &*p.alpha_blend.ptr();
            assert_eq!((a.color_source_blend(), a.color_destination_blend(), a.alpha_source_blend(), a.alpha_destination_blend()), (0, 5, 0, 5));
            let add = &*p.additive.ptr();
            assert_eq!((add.color_source_blend(), add.color_destination_blend(), add.alpha_source_blend(), add.alpha_destination_blend()), (4, 0, 4, 0));
            let np = &*p.non_premultiplied.ptr();
            assert_eq!((np.color_source_blend(), np.color_destination_blend()), (4, 5));
            let op = &*p.opaque.ptr();
            assert_eq!((op.color_source_blend(), op.color_destination_blend()), (0, 1));
            assert_eq!(((*p.point_clamp.ptr()).filter(), (*p.point_clamp.ptr()).address_u()), (1, 1));
            assert_eq!(((*p.linear_wrap.ptr()).filter(), (*p.linear_wrap.ptr()).address_v()), (0, 0));
            assert_eq!((*p.cull_none.ptr()).cull_mode(), 0);
            assert_eq!((*p.cull_clockwise.ptr()).cull_mode(), 1);
            assert_eq!((*p.cull_counter_clockwise.ptr()).cull_mode(), 2);
            assert!(!(*p.depth_none.ptr()).depth_buffer_enable && (*p.depth_default.ptr()).depth_buffer_enable);
            assert!((*p.depth_read.ptr()).depth_buffer_enable && !(*p.depth_read.ptr()).depth_buffer_write_enable);
        }
    }

    /// Main.cs:27899 `_multiplyBlendState` and Main.cs:2484 `ScissorState`, built the way the game builds them.
    #[test]
    fn the_games_custom_states() {
        unsafe {
            let b = BlendState::new_gc();
            let s = &mut *b.ptr();
            s.set_color_blend_function(2);
            s.set_color_destination_blend(0);
            s.set_color_source_blend(4);
            s.set_alpha_blend_function(2);
            s.set_alpha_destination_blend(0);
            s.set_alpha_source_blend(4);
            assert_eq!(
                s.desc,
                BlendDesc { color_src: Blend::SourceAlpha, color_dst: Blend::One, color_fn: BlendFunction::ReverseSubtract, alpha_src: Blend::SourceAlpha, alpha_dst: Blend::One, alpha_fn: BlendFunction::ReverseSubtract, blend_factor: [255; 4], write_mask: 15 }
            );
            let r = RasterizerState::new_gc();
            (*r.ptr()).set_cull_mode(0);
            (*r.ptr()).set_scissor_test_enable(true);
            assert!((*r.ptr()).scissor_test_enable && (*r.ptr()).cull_mode == CullMode::None);
        }
    }

    #[test]
    fn a_bound_state_is_read_only() {
        let p = presets();
        let e = rt::try_(|| unsafe { (*p.alpha_blend.ptr()).set_color_source_blend(1) }).unwrap_err();
        assert!(rt::exc_is(e, "System.InvalidOperationException"));
        let e = rt::try_(|| unsafe { (*BlendState::new_gc().ptr()).set_color_source_blend(99) }).unwrap_err();
        assert!(rt::exc_is(e, "System.ArgumentOutOfRangeException"));
    }
}
