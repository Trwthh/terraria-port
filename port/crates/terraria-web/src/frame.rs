//! Serializes the host's [`Frame`] into the byte format of `web/www/FRAME_FORMAT.md` for the WebGPU replayer in the
//! Worker (`web/www/gfx.js`). Everything Direct3D-specific is resolved here, as `terraria-app/src/gfx/render.rs`
//! resolves it natively: effect files are parsed (`fx_binary::parse_full`), each pass is translated to WGSL once
//! (`terraria_shaders::wgsl::translate`), parameters become register blocks (`uniforms::fill`), and the texture of
//! every sampler unit is chosen. The replayer only creates WebGPU objects and draws.
//!
//! Rules copied from render.rs (the reference): a draw whose texture is the bound target is not drawn; an effect
//! pass that cannot be translated, or whose parameters do not fit its constants, is reported once and its draws are
//! skipped; an effect without a file is drawn with the plain sprite shader; a draw under an effect needs a non-empty
//! viewport (the vertex shader's first register row is (1/w, -1/h, 0, 0) of it); a sampler unit with no texture, or
//! with a texture that does not exist, samples opaque black.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use terraria_host::gfx::fx_binary::{self, FxEffect, StateClass};
use terraria_host::gfx::{BlendDesc, Command, EffectId, EffectSnapshot, Frame, IRect, ParamValue, SamplerDesc, TextureId, Topology};
use terraria_shaders as shaders;

/// A translated pass: what every draw under it needs.
struct Pass {
    id: u32,
    pixel: shaders::bytecode::Shader,
    vertex: Option<shaders::bytecode::Shader>,
    translated: shaders::wgsl::Translated,
    /// emitted to the replayer yet (DefineShader)
    defined: bool,
}

/// Serializer state that outlives one frame (textures, effects and passes live across frames).
#[derive(Default)]
pub struct Encoder {
    effects: HashMap<EffectId, (String, Arc<Result<FxEffect, String>>)>,
    passes: HashMap<(EffectId, u32, u32), Option<Pass>>,
    next_shader: u32,
    /// existing textures and their sizes (the back buffer is texture 0, sized by the frame)
    textures: HashMap<TextureId, (u32, u32)>,
    warned: HashSet<String>,
    /// the target / viewport carried from the previous frame (XNA state survives Present)
    target: Option<TextureId>,
    viewport: Option<IRect>,
}

const NO_TEXTURE: u32 = 0xFFFF_FFFF;

impl Encoder {
    fn warn(&mut self, text: String) {
        if self.warned.insert(text.clone()) {
            crate::log(2, &format!("[gfx] {text}"));
        }
    }

    /// Encodes one frame. `flush(bytes, last)` receives it in parts cut at command boundaries once more than
    /// `CHUNK` bytes are waiting (the title's content load uploads hundreds of MB in one frame: one buffer of that size
    /// in wasm memory did not fit); `last == true` for the final part. FRAME_FORMAT.md v2: the command count is
    /// 0xFFFFFFFF and the commands end with op 0.
    pub fn encode(&mut self, frame: &Frame, flush: &mut dyn FnMut(&[u8], bool)) {
        const CHUNK: usize = 8 << 20;
        let mut out = Vec::with_capacity(64 + frame.vertices.len() * 20 + frame.raw_vertices.len() * 36);
        out.extend_from_slice(b"TWF1");
        out.extend_from_slice(&(frame.serial as f64).to_le_bytes());
        put_u32(&mut out, frame.back_buffer.0);
        put_u32(&mut out, frame.back_buffer.1);
        self.textures.insert(TextureId::BACK_BUFFER, frame.back_buffer);
        put_u32(&mut out, frame.vertices.len() as u32);
        put_u32(&mut out, frame.raw_vertices.len() as u32);
        put_u32(&mut out, 0xFFFF_FFFF);
        for v in &frame.vertices {
            out.extend_from_slice(bytemuck::bytes_of(v));
        }
        for v in &frame.raw_vertices {
            out.extend_from_slice(bytemuck::bytes_of(v));
        }
        let mut target = self.target.unwrap_or(TextureId::BACK_BUFFER);
        let mut viewport = self.viewport;
        for c in &frame.commands {
            if out.len() > CHUNK {
                flush(&out, false);
                out.clear();
            }
            match c {
                Command::CreateTexture { id, width, height, target: is_target } => {
                    self.textures.insert(*id, (*width, *height));
                    out.push(1);
                    put_u32(&mut out, id.0);
                    put_u32(&mut out, *width);
                    put_u32(&mut out, *height);
                    out.push(*is_target as u8);
                }
                Command::Upload { id, rect, rgba } => {
                    out.push(2);
                    put_u32(&mut out, id.0);
                    put_rect(&mut out, *rect);
                    out.extend_from_slice(rgba);
                }
                Command::DestroyTexture { id } => {
                    self.textures.remove(id);
                    out.push(3);
                    put_u32(&mut out, id.0);
                }
                Command::SetTarget { target: t, preserve } => {
                    target = *t;
                    viewport = None;
                    out.push(4);
                    put_u32(&mut out, t.0);
                    out.push(*preserve as u8);
                }
                Command::Clear(color) => {
                    out.push(5);
                    out.extend_from_slice(&[color.r(), color.g(), color.b(), color.a()]);
                }
                Command::Viewport(r) => {
                    viewport = Some(*r);
                    out.push(6);
                    put_rect(&mut out, *r);
                }
                Command::Scissor(r) => {
                    out.push(7);
                    out.push(r.is_some() as u8);
                    put_rect(&mut out, r.unwrap_or_default());
                }
                Command::CreateEffect { id, name, bytecode } => {
                    self.effects.insert(*id, (name.clone(), Arc::new(fx_binary::parse_full(bytecode))));
                }
                Command::Readback { id, request } => {
                    out.push(10);
                    put_u32(&mut out, id.0);
                    out.extend_from_slice(&(*request as f64).to_le_bytes());
                }
                Command::Draw { texture, blend, sampler, cull, topology, effect, first_vertex, vertex_count, raw } => {
                    // a target cannot be sampled while it is bound (XNA throws; nothing is drawn)
                    if *texture != target {
                        let size = self.textures.get(&target).copied().unwrap_or((0, 0));
                        let (_, _, vw, vh) = clamp_rect(viewport.unwrap_or(IRect { x: 0, y: 0, w: size.0 as i32, h: size.1 as i32 }), size.0, size.1);
                        let resolved = match effect.as_ref().and_then(|e| e.effect_id.map(|id| (e, id))) {
                            // raw vertices exist only for passes with a vertex shader (render.rs skips them otherwise)
                            None if *raw => None,
                            None => {
                                if effect.is_some() {
                                    self.warn("an effect without a compiled file was applied: its draws use the plain sprite shader".into());
                                }
                                Some(None)
                            }
                            Some(_) if vw == 0 || vh == 0 => None,
                            Some((e, id)) => self.effect_draw(&mut out, e, id, *texture, *sampler, target, (vw as f32, vh as f32), *raw).map(Some),
                        };
                        match resolved {
                            None => {}
                            Some(fx) => {
                                out.push(8);
                                put_u32(&mut out, texture.0);
                                put_blend(&mut out, blend);
                                put_sampler(&mut out, sampler);
                                out.push(*cull as u8);
                                out.push(match topology {
                                    Topology::Triangles => 0,
                                    Topology::Lines => 1,
                                });
                                put_u32(&mut out, *first_vertex);
                                put_u32(&mut out, *vertex_count);
                                match fx {
                                    None => put_u32(&mut out, 0),
                                    Some(d) => {
                                        put_u32(&mut out, d.shader);
                                        put_u32(&mut out, d.ps.len() as u32);
                                        out.extend_from_slice(&d.ps);
                                        put_u32(&mut out, d.vs.len() as u32);
                                        out.extend_from_slice(&d.vs);
                                        out.push(d.units.len() as u8);
                                        for (unit, tex, s) in d.units {
                                            out.push(unit as u8);
                                            put_u32(&mut out, tex);
                                            put_sampler(&mut out, &s);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        out.push(0); // End
        flush(&out, true);
        self.target = Some(target);
        self.viewport = viewport;
    }

    /// Resolves one draw under an effect pass; emits DefineShader into `out` the first time the pass is used. `None`:
    /// the draw is not made (the reason has been reported).
    #[allow(clippy::too_many_arguments)]
    fn effect_draw(&mut self, out: &mut Vec<u8>, e: &EffectSnapshot, id: EffectId, texture: TextureId, sampler: SamplerDesc, target: TextureId, viewport: (f32, f32), raw: bool) -> Option<EffectDraw> {
        let key = (id, e.technique_index, e.pass_index);
        if !self.passes.contains_key(&key) {
            let built = match self.effects.get(&id) {
                Some((name, fx)) => build_pass(name, fx, e.technique_index, e.pass_index),
                None => Err("the effect was never created in the draw list".into()),
            };
            let pass = match built {
                Ok((pixel, vertex, translated)) => {
                    self.next_shader += 1;
                    Some(Pass { id: self.next_shader, pixel, vertex, translated, defined: false })
                }
                Err(err) => {
                    self.warn(format!("ERROR: effect {} technique {} pass {} ({}) cannot be executed; its draws are skipped: {err}", e.effect, e.technique, e.pass, e.pass_index));
                    None
                }
            };
            self.passes.insert(key, pass);
        }
        let textures = &self.textures;
        let pass = self.passes.get_mut(&key)?.as_mut()?;
        if pass.vertex.is_some() != raw {
            let text = format!("ERROR: effect {} pass {}: vertex format of the draw does not match the pass; skipped", e.effect, e.pass);
            self.warn(text);
            return None;
        }
        if !pass.defined {
            pass.defined = true;
            out.push(9);
            put_u32(out, pass.id);
            put_str(out, &format!("{}.{}[{}] {}", e.effect, e.technique, e.pass_index, e.pass));
            put_str(out, &pass.translated.source);
            out.push(pass.translated.samplers.len() as u8);
            for u in &pass.translated.samplers {
                out.push(*u as u8);
            }
            put_u32(out, pass.translated.ps_registers);
            out.extend_from_slice(&pass.translated.vs_registers.map_or(-1i32, |n| n as i32).to_le_bytes());
        }
        // ---- uniforms: parameters -> registers (constant table + preshader)
        let lookup = |name: &str| parameter_floats(e, name);
        let filled = shaders::uniforms::fill(&pass.pixel, &lookup).and_then(|ps| match &pass.vertex {
            Some(vs) => shaders::uniforms::fill(vs, &lookup).map(|v| (ps, Some(v))),
            None => Ok((ps, None)),
        });
        let (ps_regs, vs_regs) = match filled {
            Ok(r) => r,
            Err(err) => {
                let text = format!("ERROR: effect {} pass {}: the parameter values do not fit the shader's constants; its draws are skipped: {err}", e.effect, e.pass);
                self.warn(text);
                return None;
            }
        };
        let mut ps = Vec::new();
        if pass.translated.ps_registers > 0 {
            let mut regs = ps_regs;
            regs.resize(pass.translated.ps_registers as usize, [0.0; 4]);
            ps.extend_from_slice(bytemuck::cast_slice(&regs));
        }
        let mut vs = Vec::new();
        if let (Some(count), Some(mut regs)) = (pass.translated.vs_registers, vs_regs) {
            regs.resize(count as usize, [0.0; 4]);
            let mut data = vec![[1.0 / viewport.0, -1.0 / viewport.1, 0.0, 0.0]];
            data.extend(regs);
            vs.extend_from_slice(bytemuck::cast_slice(&data));
        }
        // ---- textures: sampler register 0 is the draw's texture, 1..3 come from Textures[1..3]
        let mut units = Vec::new();
        for unit in &pass.translated.samplers {
            let entry = if *unit == 0 { Some((texture, sampler)) } else { e.extra_textures.iter().find(|(slot, _, _)| *slot as u32 == *unit).map(|(_, t, s)| (*t, *s)) };
            match entry {
                Some((t, _)) if t == target => return None,
                Some((t, s)) if textures.contains_key(&t) => units.push((*unit, t.0, s)),
                _ => units.push((*unit, NO_TEXTURE, SamplerDesc::PointClamp)),
            }
        }
        Some(EffectDraw { shader: pass.id, ps, vs, units })
    }
}

struct EffectDraw {
    shader: u32,
    ps: Vec<u8>,
    vs: Vec<u8>,
    units: Vec<(u32, u32, SamplerDesc)>,
}

/// Translate one pass of an effect (render.rs `build_pass` without the GPU objects).
fn build_pass(name: &str, fx: &Result<FxEffect, String>, technique: u32, pass: u32) -> Result<(shaders::bytecode::Shader, Option<shaders::bytecode::Shader>, shaders::wgsl::Translated), String> {
    let fx = fx.as_ref().map_err(|e| format!("the effect file could not be parsed: {e}"))?;
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
    let label = format!("{name}.{}[{pass}] {}", t.name, p.name);
    let translated = shaders::wgsl::translate(&label, &pixel, vertex.as_ref())?;
    Ok((pixel, vertex, translated))
}

/// The current value of an effect parameter as flat floats (render.rs `parameter_floats`).
fn parameter_floats(e: &EffectSnapshot, name: &str) -> Option<Vec<f32>> {
    match &e.params.iter().find(|(n, _)| n == name)?.1 {
        ParamValue::Floats(f) => Some(f.clone()),
        ParamValue::Bool(b) => Some(vec![*b as u8 as f32]),
        ParamValue::Int(i) => Some(vec![*i as f32]),
        ParamValue::Unset | ParamValue::Texture(_) => None,
    }
}

/// render.rs `clamp_rect`.
fn clamp_rect(r: IRect, w: u32, h: u32) -> (u32, u32, u32, u32) {
    let x0 = r.x.clamp(0, w as i32);
    let y0 = r.y.clamp(0, h as i32);
    let x1 = (r.x.saturating_add(r.w)).clamp(x0, w as i32);
    let y1 = (r.y.saturating_add(r.h)).clamp(y0, h as i32);
    (x0 as u32, y0 as u32, (x1 - x0) as u32, (y1 - y0) as u32)
}

fn put_u32(out: &mut Vec<u8>, v: u32) {
    out.extend_from_slice(&v.to_le_bytes());
}
fn put_rect(out: &mut Vec<u8>, r: IRect) {
    for v in [r.x, r.y, r.w, r.h] {
        out.extend_from_slice(&v.to_le_bytes());
    }
}
fn put_str(out: &mut Vec<u8>, s: &str) {
    put_u32(out, s.len() as u32);
    out.extend_from_slice(s.as_bytes());
}
fn put_blend(out: &mut Vec<u8>, b: &BlendDesc) {
    out.extend_from_slice(&[b.color_src as u8, b.color_dst as u8, b.color_fn as u8, b.alpha_src as u8, b.alpha_dst as u8, b.alpha_fn as u8]);
    out.extend_from_slice(&b.blend_factor);
    out.push(b.write_mask);
}
fn put_sampler(out: &mut Vec<u8>, s: &SamplerDesc) {
    out.extend_from_slice(&[s.filter as u8, s.address_u as u8, s.address_v as u8]);
}
