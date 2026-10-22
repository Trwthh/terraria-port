//! The draw-call trace: every call the translated game makes at the XNA graphics API boundary that changes what is
//! drawn, as one text line, in the format of the ORIGINAL's recorder (`tools/refdump_draw/xna/Graphics/Rec.cs`, which
//! runs the real `Terraria.exe` on an XNA stand-in). `crates/terraria-gen-game/tests/draw.rs` records the same scenes
//! with this module and compares the two line by line (`tools/cs2rs/VERIFICATION.md`, "Draw output").
//!
//! Off unless a driver calls [`set_active`]: then each recorded call is hashed into a per-frame digest
//! ([`frame_start`], [`frame_digest`]), and with [`set_full`] also written to a buffer ([`take_output`]).
//!
//! Only calls that come FROM translated code are recorded: host code that uses the same functions internally
//! (`SpriteBatch` applying its render state, `SetRenderTargets` binding through `SetRenderTarget`, a disposed render
//! target unbinding itself) runs inside [`internal`], which mutes the recorder.
//!
//! Format (one line per call, fields separated by one space; Rec.cs has the same table):
//! - values: int decimal; float = its 32 bits as 8 lower-case hex digits; Color = `PackedValue` (R in the low byte) as
//!   8 hex digits; Vector2 `x,y`; Rectangle `x,y,w,h`; null `-`
//! - resources: content textures and effects by their name in the game's asset repository (`/` separators; the pages
//!   of a `DynamicSpriteFont` `<font>#<k>`, k in order of first use in its character table; the driver names them,
//!   [`set_texture_label`], [`set_missing_hook`]); textures the game creates `tex<k>`, render targets `rt<k>`, sprite
//!   batches `sb<k>`, vertex / index buffers `vb<k>` / `ib<k>` (k = creation order from 0)
//! - states: blend `b:cs,cd,cf,as,ad,af,factor,mask`; sampler `s:filter,u,v,w,aniso,maxmip,bias`; rasterizer
//!   `r:cull,fill,scissor,msaa,depthbias,slope`; depth `z:enable,write,func,stencil`

use std::cell::{Cell, RefCell};
use std::collections::HashMap;

use terraria_core::xna::{Color, Matrix, Rectangle, Vector2, Vector3, Vector4};
use terraria_gen_rt::rt::{Gc, Nullable};

use super::device::Texture2D;
use super::draw_list::TextureId;
use super::effect::Effect;
use super::states::{BlendState, DepthStencilState, RasterizerState, SamplerState};

pub const FNV_BASIS: u64 = 14695981039346656037;
pub const FNV_PRIME: u64 = 1099511628211;

#[derive(Default)]
struct Tracer {
    active: bool,
    full: bool,
    out: Vec<u8>,
    frame_hash: u64,
    frame_lines: u64,
    lines: u64,
    new_tex: u32,
    new_rt: u32,
    new_sb: u32,
    new_vb: u32,
    new_ib: u32,
    textures: HashMap<u32, String>,
    effects: HashMap<usize, String>,
}

thread_local! {
    static TR: RefCell<Tracer> = RefCell::new(Tracer { frame_hash: FNV_BASIS, ..Default::default() });
    static DEPTH: Cell<u32> = const { Cell::new(0) };
    static MISSING: RefCell<Option<std::rc::Rc<dyn Fn()>>> = const { RefCell::new(None) };
}

/// Driver: record (digest) from now on, or stop.
pub fn set_active(on: bool) {
    TR.with(|t| t.borrow_mut().active = on);
}
/// Driver: also keep the lines ([`take_output`]).
pub fn set_full(on: bool) {
    TR.with(|t| t.borrow_mut().full = on);
}
/// Driver: the lines kept since the last call.
pub fn take_output() -> Vec<u8> {
    TR.with(|t| std::mem::take(&mut t.borrow_mut().out))
}
/// Lines written in full mode so far.
pub fn lines_written() -> u64 {
    TR.with(|t| t.borrow().lines)
}
/// Driver: a frame starts (its digest restarts).
pub fn frame_start() {
    TR.with(|t| {
        let mut t = t.borrow_mut();
        t.frame_hash = FNV_BASIS;
        t.frame_lines = 0;
    });
}
/// The frame so far: (lines, FNV-1a 64 of the lines with their newlines).
pub fn frame_digest() -> (u64, u64) {
    TR.with(|t| {
        let t = t.borrow();
        (t.frame_lines, t.frame_hash)
    })
}

/// Is a call recorded now? (Active, and not inside host-internal use of the API.)
pub fn on() -> bool {
    DEPTH.get() == 0 && TR.with(|t| t.borrow().active)
}

/// Records a line (built only when recording).
pub fn line(f: impl FnOnce() -> String) {
    if !on() {
        return;
    }
    let s = f();
    TR.with(|t| {
        let mut t = t.borrow_mut();
        let mut h = t.frame_hash;
        for &b in s.as_bytes() {
            h ^= b as u64;
            h = h.wrapping_mul(FNV_PRIME);
        }
        h ^= 10;
        h = h.wrapping_mul(FNV_PRIME);
        t.frame_hash = h;
        t.frame_lines += 1;
        if t.full {
            t.out.extend_from_slice(s.as_bytes());
            t.out.push(b'\n');
            t.lines += 1;
        }
    });
}

/// Host code using API functions on its own behalf: not recorded.
pub fn internal<R>(f: impl FnOnce() -> R) -> R {
    struct Guard;
    impl Drop for Guard {
        fn drop(&mut self) {
            DEPTH.set(DEPTH.get() - 1);
        }
    }
    DEPTH.set(DEPTH.get() + 1);
    let _g = Guard;
    f()
}

// ------------------------------------------------------------------------------------------------ labels

fn next(f: impl FnOnce(&mut Tracer) -> &mut u32) -> u32 {
    TR.with(|t| {
        let mut t = t.borrow_mut();
        let c = f(&mut t);
        let k = *c;
        *c += 1;
        k
    })
}
/// A texture the game created (`new Texture2D`): `tex<k>`.
pub fn new_texture_label(id: TextureId) -> String {
    let s = format!("tex{}", next(|t| &mut t.new_tex));
    TR.with(|t| t.borrow_mut().textures.insert(id.0, s.clone()));
    s
}
/// A render target the game created: `rt<k>`.
pub fn new_target_label(id: TextureId) -> String {
    let s = format!("rt{}", next(|t| &mut t.new_rt));
    TR.with(|t| t.borrow_mut().textures.insert(id.0, s.clone()));
    s
}
pub fn new_sprite_batch_id() -> u32 {
    next(|t| &mut t.new_sb)
}
pub fn new_vertex_buffer_id() -> u32 {
    next(|t| &mut t.new_vb)
}
pub fn new_index_buffer_id() -> u32 {
    next(|t| &mut t.new_ib)
}
/// Driver: the name of a content texture (kept if it has one already).
pub fn set_texture_label(id: TextureId, name: &str) {
    TR.with(|t| {
        t.borrow_mut().textures.entry(id.0).or_insert_with(|| name.to_owned());
    });
}
pub fn has_texture_label(id: TextureId) -> bool {
    TR.with(|t| t.borrow().textures.contains_key(&id.0))
}
/// Driver: the name of a content effect.
pub fn set_effect_label(effect: Gc<Effect>, name: &str) {
    TR.with(|t| {
        t.borrow_mut().effects.entry(effect.0 as usize).or_insert_with(|| name.to_owned());
    });
}
pub fn has_effect_label(effect: Gc<Effect>) -> bool {
    TR.with(|t| t.borrow().effects.contains_key(&(effect.0 as usize)))
}
/// Driver: called when a texture or effect without a name is recorded (it names the newly loaded content).
pub fn set_missing_hook(f: Option<std::rc::Rc<dyn Fn()>>) {
    MISSING.with(|m| *m.borrow_mut() = f);
}
fn call_missing() {
    let f = MISSING.with(|m| m.borrow().clone());
    if let Some(f) = f {
        internal(|| f());
    }
}

// ------------------------------------------------------------------------------------------------ value formats

pub fn f(v: f32) -> String {
    format!("{:08x}", v.to_bits())
}
pub fn b(v: bool) -> &'static str {
    if v {
        "1"
    } else {
        "0"
    }
}
pub fn color(c: Color) -> String {
    format!("{:08x}", c.packed_value())
}
pub fn v2(v: Vector2) -> String {
    format!("{},{}", f(v.x), f(v.y))
}
pub fn v3(v: Vector3) -> String {
    format!("{},{},{}", f(v.x), f(v.y), f(v.z))
}
pub fn v4(v: Vector4) -> String {
    format!("{},{},{},{}", f(v.x), f(v.y), f(v.z), f(v.w))
}
pub fn rect(r: Rectangle) -> String {
    format!("{},{},{},{}", r.x, r.y, r.width, r.height)
}
pub fn nrect(r: Nullable<Rectangle>) -> String {
    if r.has_value {
        rect(r.value)
    } else {
        "-".to_owned()
    }
}
pub fn matrix(m: &Matrix) -> String {
    [m.m11, m.m12, m.m13, m.m14, m.m21, m.m22, m.m23, m.m24, m.m31, m.m32, m.m33, m.m34, m.m41, m.m42, m.m43, m.m44].iter().map(|v| f(*v)).collect::<Vec<_>>().join(",")
}
pub fn tex_id(id: TextureId) -> String {
    if let Some(s) = TR.with(|t| t.borrow().textures.get(&id.0).cloned()) {
        return s;
    }
    call_missing();
    TR.with(|t| t.borrow().textures.get(&id.0).cloned()).unwrap_or_else(|| "?".to_owned())
}
pub fn tex(t: Gc<Texture2D>) -> String {
    if t.is_null() {
        return "-".to_owned();
    }
    // SAFETY: a live texture object (the caller holds it).
    tex_id(unsafe { (*t.ptr()).id })
}
pub fn effect(e: Gc<Effect>) -> String {
    if e.is_null() {
        return "-".to_owned();
    }
    effect_key(e.0 as usize)
}
pub fn effect_key(key: usize) -> String {
    if let Some(s) = TR.with(|t| t.borrow().effects.get(&key).cloned()) {
        return s;
    }
    call_missing();
    TR.with(|t| t.borrow().effects.get(&key).cloned()).unwrap_or_else(|| "?".to_owned())
}
pub fn blend(s: Gc<BlendState>) -> String {
    if s.is_null() {
        return "-".to_owned();
    }
    // SAFETY: a live state object.
    let d = unsafe { (*s.ptr()).desc };
    let factor = u32::from_le_bytes(d.blend_factor);
    format!("b:{},{},{},{},{},{},{:08x},{}", d.color_src as u8, d.color_dst as u8, d.color_fn as u8, d.alpha_src as u8, d.alpha_dst as u8, d.alpha_fn as u8, factor, d.write_mask)
}
pub fn sampler(s: Gc<SamplerState>) -> String {
    if s.is_null() {
        return "-".to_owned();
    }
    // SAFETY: a live state object.
    let s = unsafe { &*s.ptr() };
    format!("s:{},{},{},{},{},{},{}", s.desc.filter as u8, s.desc.address_u as u8, s.desc.address_v as u8, s.address_w, s.max_anisotropy, s.max_mip_level, f(s.mip_map_level_of_detail_bias))
}
pub fn raster(s: Gc<RasterizerState>) -> String {
    if s.is_null() {
        return "-".to_owned();
    }
    // SAFETY: a live state object.
    let s = unsafe { &*s.ptr() };
    format!("r:{},{},{},{},{},{}", s.cull_mode as u8, s.fill_mode, b(s.scissor_test_enable), b(s.multi_sample_anti_alias), f(s.depth_bias), f(s.slope_scale_depth_bias))
}
pub fn depth(s: Gc<DepthStencilState>) -> String {
    if s.is_null() {
        return "-".to_owned();
    }
    // SAFETY: a live state object.
    let s = unsafe { &*s.ptr() };
    format!("z:{},{},{},{}", b(s.depth_buffer_enable), b(s.depth_buffer_write_enable), s.depth_buffer_function, b(s.stencil_enable))
}

/// FNV-1a 64 over raw element bytes, as `size:hash` (Rec.Hash): `size` is one element's size.
pub fn hash_bytes(elem_size: usize, bytes: &[u8]) -> String {
    thread_local! {
        /// `DRAW_RAW=1` (diagnosis, as Rec.cs): the bytes themselves instead of their hash
        static RAW: bool = std::env::var("DRAW_RAW").is_ok_and(|v| v == "1");
    }
    if RAW.with(|r| *r) {
        let mut s = format!("{elem_size}:");
        for x in bytes {
            s.push_str(&format!("{x:02x}"));
        }
        return s;
    }
    let mut h = FNV_BASIS;
    for &x in bytes {
        h ^= x as u64;
        h = h.wrapping_mul(FNV_PRIME);
    }
    format!("{elem_size}:{h:016x}")
}
/// `Rec.Hash(data, start, count)`: `-` for a null array, `range` when the range is outside it.
pub fn hash_arr<T: Copy + terraria_gen_rt::rt::Trace + 'static>(data: terraria_gen_rt::rt::Arr<T>, start: i32, count: i32) -> String {
    if data.is_null() {
        return "-".to_owned();
    }
    if start < 0 || count < 0 || start as i64 + count as i64 > data.len() as i64 {
        return "range".to_owned();
    }
    let s = &data.as_slice()[start as usize..(start + count) as usize];
    // SAFETY: plain value types viewed as bytes.
    let bytes = unsafe { core::slice::from_raw_parts(s.as_ptr() as *const u8, core::mem::size_of_val(s)) };
    hash_bytes(core::mem::size_of::<T>(), bytes)
}
