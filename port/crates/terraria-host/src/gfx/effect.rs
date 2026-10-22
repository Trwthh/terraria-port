//! `Effect`, `EffectTechnique`, `EffectPass`, `EffectParameter`: the object model and parameter storage.
//!
//! Effects are executed by the backend, not here. Loading a compiled effect hands the file to the backend
//! (`Command::CreateEffect`); `EffectPass.Apply()` records a snapshot (effect, technique, pass, every parameter
//! value, the textures in slots 1..3) that travels with each draw in the draw list. The backend translates the
//! pass's Direct3D 9 shaders (`terraria-shaders`) and feeds them from the snapshot. `docs/effects/README.md`.
//!
//! Names: an effect built from a description ([`EffectDesc`]) knows its techniques, passes and parameters and
//! answers an unknown name with null, as XNA does. The description comes out of the compiled effect
//! (`terraria_formats::xnb::Effect::bytecode`, a D3DX effect binary) through [`super::fx_binary::parse`]; a
//! parameter starts with the default value stored in the file (all zero in the game's three effects except
//! `TileShader.Epsilon`).
//!
//! # What `Apply` does to device state (XNA / D3DX)
//! D3DX applies every state assignment of the pass (shaders, render states, sampler states) and does not restore
//! them afterwards (XNA creates its effects with `D3DXFX_DONOTSAVESTATE`). The passes of the game's three effects
//! assign nothing but `PixelShader` and, in six passes, `VertexShader`: no render state, no sampler state, no
//! texture (the listing in `docs/effects/<Effect>.md` shows every assignment). So `Apply` changes exactly one
//! thing here: the device's active effect pass. The blend / rasterizer / sampler state objects and `Textures[n]`
//! stay as the game set them. A pass with any other state assignment is refused by the backend by name.
//! A pass WITHOUT a vertex shader leaves the vertex shader that is bound, which is SpriteBatch's (the game always
//! begins a SpriteBatch first): such draws keep SpriteBatch's transform.

use std::sync::Arc;

use terraria_core::xna::{Matrix, Vector2, Vector3, Vector4};
use terraria_gen_rt::rt::{self, Arr, Gc, Marker, Str, Trace};

use super::device::{GraphicsDevice, Texture2D};
use super::draw_list::{EffectId, EffectSnapshot, ParamValue};
use super::fx_binary::{FxValue, ParamClass, ParamType};
use super::states::throw;

/// Techniques (with pass names) and parameter names of an effect.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct EffectDesc {
    pub techniques: Vec<(String, Vec<String>)>,
    pub parameters: Vec<String>,
    /// the whole parsed effect (types, default values, pass states, shader blobs), when the description came
    /// from a compiled effect
    pub fx: Option<Arc<super::fx_binary::FxEffect>>,
    /// the compiled effect file itself, for the backend that executes its shaders
    pub bytecode: Option<Arc<[u8]>>,
}

pub struct EffectParameter {
    /// the effect this parameter belongs to (its address: the draw trace's key for the effect's name)
    pub owner: usize,
    pub name: String,
    pub value: ParamValue,
    /// the texture object behind `ParamValue::Texture` (kept alive while it is set)
    pub texture: Gc<Texture2D>,
}
// SAFETY: the one handle is reported.
unsafe impl Trace for EffectParameter {
    unsafe fn trace(&self, m: &mut Marker) {
        m.mark(self.texture)
    }
}

impl EffectParameter {
    pub fn name(&self) -> Str {
        Str::new(&self.name)
    }
    fn trace(&self, kind: &str, value: impl FnOnce() -> String) {
        super::trace::line(|| format!("fx.param {} {} {kind} {}", super::trace::effect_key(self.owner), self.name, value()));
    }
    pub fn set_bool(&mut self, v: bool) {
        self.trace("b", || super::trace::b(v).to_owned());
        self.value = ParamValue::Bool(v);
    }
    pub fn set_int(&mut self, v: i32) {
        self.trace("i", || v.to_string());
        self.value = ParamValue::Int(v);
    }
    pub fn set_float(&mut self, v: f32) {
        self.trace("f", || super::trace::f(v));
        self.value = ParamValue::Floats(vec![v]);
    }
    pub fn set_vector2(&mut self, v: Vector2) {
        self.trace("v2", || super::trace::v2(v));
        self.value = ParamValue::Floats(vec![v.x, v.y]);
    }
    pub fn set_vector3(&mut self, v: Vector3) {
        self.trace("v3", || super::trace::v3(v));
        self.value = ParamValue::Floats(vec![v.x, v.y, v.z]);
    }
    pub fn set_vector4(&mut self, v: Vector4) {
        self.trace("v4", || super::trace::v4(v));
        self.value = ParamValue::Floats(vec![v.x, v.y, v.z, v.w]);
    }
    /// `SetValue(Matrix)`: stored row major (M11, M12, ... M44).
    pub fn set_matrix(&mut self, m: Matrix) {
        self.trace("m", || super::trace::matrix(&m));
        self.value = ParamValue::Floats(vec![m.m11, m.m12, m.m13, m.m14, m.m21, m.m22, m.m23, m.m24, m.m31, m.m32, m.m33, m.m34, m.m41, m.m42, m.m43, m.m44]);
    }
    pub fn set_float_array(&mut self, v: Arr<f32>) {
        self.trace("fa", || v.as_slice().iter().map(|x| super::trace::f(*x)).collect::<Vec<_>>().join(","));
        self.value = ParamValue::Floats(v.as_slice().to_vec());
    }
    pub fn set_vector2_array(&mut self, v: Arr<Vector2>) {
        self.trace("v2a", || v.as_slice().iter().map(|x| super::trace::v2(*x)).collect::<Vec<_>>().join(";"));
        self.value = ParamValue::Floats(v.as_slice().iter().flat_map(|p| [p.x, p.y]).collect());
    }
    pub fn set_vector3_array(&mut self, v: Arr<Vector3>) {
        self.value = ParamValue::Floats(v.as_slice().iter().flat_map(|p| [p.x, p.y, p.z]).collect());
    }
    pub fn set_vector4_array(&mut self, v: Arr<Vector4>) {
        self.trace("v4a", || v.as_slice().iter().map(|x| super::trace::v4(*x)).collect::<Vec<_>>().join(";"));
        self.value = ParamValue::Floats(v.as_slice().iter().flat_map(|p| [p.x, p.y, p.z, p.w]).collect());
    }
    /// `SetValue(Texture)`; null clears it.
    pub fn set_texture(&mut self, t: Gc<Texture2D>) {
        self.trace("t", || super::trace::tex(t));
        self.texture = t;
        // SAFETY: checked non-null.
        self.value = ParamValue::Texture(if t.is_null() { None } else { Some(unsafe { (*t.ptr()).id }) });
    }
    fn floats(&self, n: usize, member: &'static str) -> &[f32] {
        match &self.value {
            ParamValue::Floats(f) if f.len() >= n => &f[..n],
            // only a parameter of an effect built without a file has no value
            ParamValue::Unset => terraria_gen_rt::host::unimplemented_value(member),
            _ => throw("System.InvalidCastException", "The parameter does not hold a value of the requested type."),
        }
    }
    pub fn get_value_single(&self) -> f32 {
        self.floats(1, "Microsoft.Xna.Framework.Graphics.EffectParameter.GetValueSingle() of a parameter the game never set")[0]
    }
    pub fn get_value_vector2(&self) -> Vector2 {
        let f = self.floats(2, "Microsoft.Xna.Framework.Graphics.EffectParameter.GetValueVector2() of a parameter the game never set");
        Vector2::new(f[0], f[1])
    }
    pub fn get_value_vector3(&self) -> Vector3 {
        let f = self.floats(3, "Microsoft.Xna.Framework.Graphics.EffectParameter.GetValueVector3() of a parameter the game never set");
        Vector3 { x: f[0], y: f[1], z: f[2] }
    }
    pub fn get_value_texture2d(&self) -> Gc<Texture2D> {
        self.texture
    }
}

pub struct EffectPass {
    pub name: String,
    pub index: u32,
    /// the pass assigns a vertex shader (draws under it record raw vertices)
    pub vertex_shader: bool,
    pub effect: Gc<Effect>,
    pub technique: Gc<EffectTechnique>,
}
// SAFETY: both handles are reported.
unsafe impl Trace for EffectPass {
    unsafe fn trace(&self, m: &mut Marker) {
        m.mark(self.effect);
        m.mark(self.technique);
    }
}

impl EffectPass {
    pub fn name(&self) -> Str {
        Str::new(&self.name)
    }
    /// `Apply()`: makes this pass the device's active effect pass. Recorded, not executed (module docs).
    pub fn apply(&mut self) {
        // SAFETY: a pass keeps its effect and technique alive; `ptr()` checks null.
        let (e, t) = unsafe { (&*self.effect.ptr(), &*self.technique.ptr()) };
        super::trace::line(|| format!("fx.apply {} {} {}", super::trace::effect(self.effect), t.name, self.name));
        e.check();
        let snapshot = EffectSnapshot {
            effect_id: e.id,
            technique_index: e.techniques.iter().position(|x| *x == self.technique).unwrap_or(0) as u32,
            vertex_shader: self.vertex_shader,
            effect: e.name.clone(),
            technique: t.name.clone(),
            pass: self.name.clone(),
            pass_index: self.index,
            // SAFETY: parameters are kept alive by the effect.
            params: e.parameters.iter().map(|p| unsafe { ((*p.ptr()).name.clone(), (*p.ptr()).value.clone()) }).collect(),
            extra_textures: Vec::new(),
        };
        // SAFETY: an effect keeps its device alive.
        unsafe { (*e.device.ptr()).applied_effect = Some(Arc::new(snapshot)) };
    }
}

/// `EffectTechnique`, and its `EffectPassCollection` (`Passes` is the technique itself).
pub struct EffectTechnique {
    pub name: String,
    pub passes: Vec<Gc<EffectPass>>,
}
// SAFETY: every handle is reported.
unsafe impl Trace for EffectTechnique {
    unsafe fn trace(&self, m: &mut Marker) {
        for p in &self.passes {
            m.mark(*p);
        }
    }
}

impl EffectTechnique {
    pub fn name(&self) -> Str {
        Str::new(&self.name)
    }
    /// `Passes.Count`
    pub fn pass_count(&self) -> i32 {
        self.passes.len() as i32
    }
    /// `Passes[int]`: `IndexOutOfRangeException` outside the collection... XNA returns null for an index out of
    /// range; so does this.
    pub fn pass_at(&self, i: i32) -> Gc<EffectPass> {
        usize::try_from(i).ok().and_then(|i| self.passes.get(i)).copied().unwrap_or(Gc::NULL)
    }
    /// `Passes[string]`: null when there is no pass of that name.
    pub fn pass_named(&self, name: Str) -> Gc<EffectPass> {
        let n = name.as_str();
        // SAFETY: passes are kept alive by the technique.
        self.passes.iter().copied().find(|p| unsafe { (*p.ptr()).name == n }).unwrap_or(Gc::NULL)
    }
}

/// `Effect`, and its `EffectParameterCollection` / `EffectTechniqueCollection` (the effect itself).
pub struct Effect {
    pub device: Gc<GraphicsDevice>,
    /// asset name (`PixelShader`), for the snapshot
    pub name: String,
    /// the compiled effect in the draw list; `None` when the description carried no file
    pub id: Option<EffectId>,
    pub parameters: Vec<Gc<EffectParameter>>,
    pub techniques: Vec<Gc<EffectTechnique>>,
    pub current_technique: Gc<EffectTechnique>,
    pub disposed: bool,
}
// SAFETY: every handle is reported.
unsafe impl Trace for Effect {
    unsafe fn trace(&self, m: &mut Marker) {
        m.mark(self.device);
        for p in &self.parameters {
            m.mark(*p);
        }
        for t in &self.techniques {
            m.mark(*t);
        }
        m.mark(self.current_technique);
    }
}

impl Effect {
    /// An effect with the techniques, passes and parameters of `desc`; `CurrentTechnique` is the first technique.
    pub fn new_gc(device: Gc<GraphicsDevice>, name: &str, desc: &EffectDesc) -> Gc<Effect> {
        if desc.techniques.is_empty() {
            throw("System.InvalidOperationException", "An effect needs at least one technique.");
        }
        let id = match &desc.bytecode {
            // SAFETY: `ptr()` checks null; the list is the device's own.
            Some(b) => Some(unsafe { (*device.ptr()).list.create_effect(name, b.clone()) }),
            None => None,
        };
        let effect = rt::alloc_init(Effect { device, name: name.to_owned(), id, parameters: Vec::new(), techniques: Vec::new(), current_technique: Gc::NULL, disposed: false });
        // SAFETY: fresh object; nothing else refers to it yet.
        let e = unsafe { &mut *effect.ptr() };
        for p in &desc.parameters {
            // the value compiled into the effect, as XNA's parameter holds it before the first SetValue
            let value = match desc.fx.as_ref().and_then(|fx| fx.parameter(p)) {
                Some(fp) if !matches!(fp.ty.class, ParamClass::Object | ParamClass::Struct) => match (&fp.value, fp.ty.ty, fp.ty.class, fp.ty.elements) {
                    (FxValue::Numeric(raw), ParamType::Bool, ParamClass::Scalar, 0) => ParamValue::Bool(raw[0] != 0),
                    (FxValue::Numeric(raw), ParamType::Int, ParamClass::Scalar, 0) => ParamValue::Int(raw[0] as i32),
                    (v, ty, _, _) => v.floats(ty).map_or(ParamValue::Unset, ParamValue::Floats),
                },
                _ => ParamValue::Unset,
            };
            e.parameters.push(rt::alloc_init(EffectParameter { owner: effect.0 as usize, name: p.clone(), value, texture: Gc::NULL }));
        }
        for (t, (tname, passes)) in desc.techniques.iter().enumerate() {
            let technique = rt::alloc_init(EffectTechnique { name: tname.clone(), passes: Vec::new() });
            for (i, pname) in passes.iter().enumerate() {
                let vertex_shader = desc.fx.as_ref().is_some_and(|fx| fx.techniques[t].passes[i].vertex_shader().is_some());
                let pass = rt::alloc_init(EffectPass { name: pname.clone(), index: i as u32, vertex_shader, effect, technique });
                // SAFETY: fresh object.
                unsafe { (*technique.ptr()).passes.push(pass) };
            }
            e.techniques.push(technique);
        }
        e.current_technique = e.techniques[0];
        effect
    }

    fn check(&self) {
        if self.disposed {
            throw("System.ObjectDisposedException", "Cannot access a disposed object. Object name: 'Effect'.");
        }
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
    pub fn current_technique(&self) -> Gc<EffectTechnique> {
        self.current_technique
    }
    pub fn set_current_technique(&mut self, t: Gc<EffectTechnique>) {
        // SAFETY: checked non-null.
        super::trace::line(|| format!("fx.technique {} {}", super::trace::effect_key(self as *const Effect as usize), if t.is_null() { "-".to_owned() } else { unsafe { (*t.ptr()).name.clone() } }));
        if t.is_null() || !self.techniques.contains(&t) {
            throw("System.InvalidOperationException", "CurrentTechnique must be set to a valid technique of this effect.");
        }
        self.current_technique = t;
    }
    /// `Parameters[string]`: null when the effect has no such parameter.
    pub fn parameter(&self, name: Str) -> Gc<EffectParameter> {
        let n = name.as_str();
        // SAFETY: parameters are kept alive by the effect.
        self.parameters.iter().copied().find(|p| unsafe { (*p.ptr()).name == n }).unwrap_or(Gc::NULL)
    }
    /// `Parameters[int]`
    pub fn parameter_at(&self, i: i32) -> Gc<EffectParameter> {
        usize::try_from(i).ok().and_then(|i| self.parameters.get(i)).copied().unwrap_or(Gc::NULL)
    }
    pub fn parameter_count(&self) -> i32 {
        self.parameters.len() as i32
    }
    /// `Techniques[string]`: null when there is none of that name.
    pub fn technique(&self, name: Str) -> Gc<EffectTechnique> {
        let n = name.as_str();
        // SAFETY: techniques are kept alive by the effect.
        self.techniques.iter().copied().find(|t| unsafe { (*t.ptr()).name == n }).unwrap_or(Gc::NULL)
    }
    pub fn technique_at(&self, i: i32) -> Gc<EffectTechnique> {
        usize::try_from(i).ok().and_then(|i| self.techniques.get(i)).copied().unwrap_or(Gc::NULL)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gfx::device::{VertexDeclaration, VertexElement};
    use crate::gfx::draw_list::Command;
    use crate::gfx::fx_binary;
    use crate::gfx::states::presets;

    /// The real `PixelShader` effect: the file reaches the draw list, parameters start with the file's defaults,
    /// and a draw under a pass with a vertex shader (`MagicMissile`, drawn by the game's `VertexStrip` with a
    /// Vector2 position, a colour and a Vector3 texture coordinate) records the vertices untransformed.
    #[test]
    fn the_real_effect_records_raw_vertices_under_a_vertex_shader_pass() {
        let Some((bytes, _)) = fx_binary::load_game_effect("PixelShader") else { return };
        unsafe {
            let d = GraphicsDevice::new_gc(100, 100);
            let dv = &mut *d.ptr();
            let e = Effect::new_gc(d, "PixelShader", &fx_binary::parse(&bytes).unwrap());
            let fx = &*e.ptr();
            assert!(matches!(dv.list.commands().last(), Some(Command::CreateEffect { name, bytecode, .. }) if name == "PixelShader" && bytecode.len() == bytes.len()));
            assert_eq!((*fx.parameter(Str::lit("uOpacity")).ptr()).get_value_single(), 0.0, "the default stored in the file");
            assert_eq!((*fx.parameter(Str::lit("uImage0")).ptr()).value, ParamValue::Unset);
            let t = &*fx.current_technique().ptr();
            assert!(!(*t.pass_named(Str::lit("ArmorColored")).ptr()).vertex_shader);
            let pass = t.pass_named(Str::lit("MagicMissile"));
            assert!((*pass.ptr()).vertex_shader);

            // VertexStrip.CustomVertexInfo (Graphics/VertexStrip.cs:23): Vector2 position, Color, Vector3 texcoord
            #[repr(C)]
            #[derive(Clone, Copy)]
            struct CustomVertexInfo {
                position: Vector2,
                color: [u8; 4],
                tex: Vector3,
            }
            crate::gfx::states::plain_trace!(CustomVertexInfo);
            let decl = VertexDeclaration::new_gc(Arr::from_slice(&[VertexElement::new(0, 1, 0, 0), VertexElement::new(8, 4, 1, 0), VertexElement::new(12, 2, 2, 0)]));
            let v = |x: f32, y: f32| CustomVertexInfo { position: Vector2::new(x, y), color: [1, 2, 3, 4], tex: Vector3 { x: 0.25, y: 0.5, z: 2.0 } };
            let white = crate::gfx::device::Texture2D::from_rgba(d, 1, 1, vec![255; 4], false);
            dv.set_texture(0, white);
            dv.set_rasterizer_state(presets().cull_none);
            (*pass.ptr()).apply();
            dv.draw_user_indexed_primitives_decl(0, Arr::from_slice(&[v(10.0, 20.0), v(30.0, 20.0), v(30.0, 40.0)]), 0, 3, Arr::from_slice(&[0i16, 1, 2]), 0, 1, decl);
            let Some(Command::Draw { raw: true, first_vertex: 0, vertex_count: 3, effect: Some(s), .. }) = dv.list.commands().last() else { panic!("{:?}", dv.list.commands().last()) };
            assert!(s.vertex_shader && s.effect_id.is_some() && s.pass == "MagicMissile");
            assert!(dv.list.vertices().is_empty());
            let raw = dv.list.raw_vertices();
            assert_eq!((raw[1].pos, raw[1].tex, raw[1].color), ([30.0, 20.0, 0.0, 1.0], [0.25, 0.5, 2.0, 1.0], [1, 2, 3, 4]));

            // the same draw under a pass without a vertex shader keeps SpriteBatch's vertex stage
            (*t.pass_named(Str::lit("ArmorColored")).ptr()).apply();
            dv.draw_user_indexed_primitives_decl(0, Arr::from_slice(&[v(0.0, 0.0), v(100.0, 0.0), v(100.0, 100.0)]), 0, 3, Arr::from_slice(&[0i16, 1, 2]), 0, 1, decl);
            assert!(matches!(dv.list.commands().last(), Some(Command::Draw { raw: false, .. })));
            assert_eq!((dv.list.vertices()[2].pos, dv.list.vertices()[2].uv), ([1.0, -1.0], [0.25, 0.5]));
        }
    }

    pub(crate) fn desc() -> EffectDesc {
        EffectDesc { techniques: vec![("Technique1".into(), vec!["ArmorColored".into(), "ArmorSolar".into()])], parameters: vec!["uColor".into(), "uOpacity".into(), "MatrixTransform".into(), "uImage1".into()], ..Default::default() }
    }

    #[test]
    fn names_resolve_like_xna_and_apply_snapshots_the_parameters() {
        unsafe {
            let d = GraphicsDevice::new_gc(100, 100);
            let e = Effect::new_gc(d, "PixelShader", &desc());
            let fx = &*e.ptr();
            assert!(fx.parameter(Str::lit("nope")).is_null());
            assert!(fx.technique(Str::lit("nope")).is_null());
            let t = &*fx.current_technique().ptr();
            assert_eq!((t.name.as_str(), t.pass_count()), ("Technique1", 2));
            assert!(t.pass_at(2).is_null() && t.pass_named(Str::lit("x")).is_null());
            (*fx.parameter(Str::lit("uColor")).ptr()).set_vector3(Vector3 { x: 1.0, y: 0.5, z: 0.25 });
            (*fx.parameter(Str::lit("uOpacity")).ptr()).set_float(0.75);
            let pass = t.pass_named(Str::lit("ArmorSolar"));
            assert_eq!((*pass.ptr()).index, 1);
            (*pass.ptr()).apply();
            let s = (*d.ptr()).applied_effect.clone().unwrap();
            assert_eq!((s.effect.as_str(), s.technique.as_str(), s.pass.as_str(), s.pass_index), ("PixelShader", "Technique1", "ArmorSolar", 1));
            assert_eq!(s.params[0], ("uColor".to_owned(), ParamValue::Floats(vec![1.0, 0.5, 0.25])));
            assert_eq!(s.params[1].1, ParamValue::Floats(vec![0.75]));
            assert_eq!(s.params[2].1, ParamValue::Unset);
            assert_eq!((s.effect_id, s.technique_index, s.vertex_shader), (None, 0, false));
            // a later SetValue does not change what was applied
            (*fx.parameter(Str::lit("uOpacity")).ptr()).set_float(0.1);
            assert_eq!((*d.ptr()).applied_effect.as_ref().unwrap().params[1].1, ParamValue::Floats(vec![0.75]));
            assert_eq!((*fx.parameter(Str::lit("uColor")).ptr()).get_value_vector3().y, 0.5);
        }
    }
}
