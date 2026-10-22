//! Direct3D 9 shader model 2 bytecode -> WGSL: one module per effect pass.
//!
//! # The module that is generated
//! ```text
//! group 0   binding 2*n     texture_2d<f32>  `tex{n}`        sampler register s{n} (only registers the shaders declare)
//!           binding 2*n+1   sampler          `smp{n}`
//! group 1   binding 0       array<vec4<f32>, N>  `ps_c`      the pixel shader's float registers c0..c{N-1} (absent when N = 0)
//!           binding 1       VsUniforms { half_pixel, c: array<vec4<f32>, M> }   only when the pass has a vertex shader
//! @vertex fn vs      @fragment fn fs
//! ```
//! A pass WITHOUT a vertex shader runs on SpriteBatch's fixed vertex stage (in Direct3D the vertex shader of
//! XNA's `SpriteEffect` stays bound when a pass only sets a pixel shader). That stage is emitted as a fixed `vs`
//! taking the port's sprite vertex (clip-space position, uv, colour) and passing uv and colour on; it gives the
//! pixel shader `t0.xy` and `v0` and nothing else, and a pixel shader that reads more is refused.
//!
//! A pass WITH a vertex shader gets `vs` translated from its bytecode. It takes the raw vertex (position, texture
//! coordinate 0, colour 0: vec4 each, missing components (0, 0, 0, 1) as Direct3D expands them) and feeds the
//! pixel shader what it wrote (`oT#` at location #, `oD#` at location 8 + #, clamped to 0..1 as shader model 2
//! does). After the program the Direct3D 9 pixel centre convention is applied: `half_pixel` = (1 / width,
//! -1 / height) of the viewport is added to the position times w, which moves the geometry by (+0.5, +0.5) pixels:
//! Direct3D 9 samples pixels at integer coordinates, wgpu at half-integers.
//!
//! # Instruction semantics (Direct3D 9 reference for shader model 2)
//! Every instruction is one line of WGSL with the assembly as a comment. A destination write mask selects which
//! components are computed and written; a source swizzle is applied before the operation.
//! - `rcp` 1 / x, `rsq` 1 / sqrt(|x|), `log` log2(|x|), `exp` 2^x, `pow` |x|^y, `frc` x - floor(x);
//! - `lrp` s0 * (s1 - s2) + s2, `cmp` s0 >= 0 ? s1 : s2, `slt` / `sge` 1.0 or 0.0;
//! - `dp2add` s0.x * s1.x + s0.y * s1.y + s2, `nrm` s0 / sqrt(s0.xyz . s0.xyz);
//! - `sincos` x = cos, y = sin (Direct3D expands it to a polynomial valid on -pi..pi; real `cos` / `sin` here);
//! - `texld` samples with the 2D coordinate; flag 1 divides by w first, flag 2 adds w as a level bias;
//! - `texkill` discards when any of the four components is negative (the shader model 2 rule);
//! - `_sat` clamps the result to 0..1; `_pp` (partial precision) and `_centroid` are hints and ignored.

use super::bytecode::{Dst, Extra, Instruction, Opcode, RegType, Shader, ShaderKind, Src, SrcMod, Usage, MASK_ALL};
use super::check;
use super::ctab::RegisterSet;
use super::disasm::instruction_text;
use super::uniforms::float_register_count;

/// The generated module and what a backend has to bind to it.
#[derive(Clone, Debug, PartialEq)]
pub struct Translated {
    pub source: String,
    /// length of `ps_c`; 0: the module has no `ps_c` binding
    pub ps_registers: u32,
    /// `Some(M)`: the pass has a vertex shader with M float registers (`VsUniforms`); `None`: the sprite stage
    pub vs_registers: Option<u32>,
    /// sampler registers that are bound (texture at binding 2n, sampler at 2n + 1 of group 0), ascending
    pub samplers: Vec<u32>,
}

impl Translated {
    pub fn has_vertex_shader(&self) -> bool {
        self.vs_registers.is_some()
    }
}

const XYZW: [char; 4] = ['x', 'y', 'z', 'w'];

fn vec_type(n: usize) -> &'static str {
    match n {
        1 => "f32",
        2 => "vec2<f32>",
        3 => "vec3<f32>",
        _ => "vec4<f32>",
    }
}

/// A float as a WGSL literal that reads back as exactly the same `f32`.
fn literal(v: f32) -> Result<String, String> {
    if !v.is_finite() {
        return Err(format!("a non-finite constant ({v}) cannot be written in WGSL"));
    }
    let mut s = format!("{v}");
    if !s.contains('.') && !s.contains('e') {
        s.push_str(".0");
    }
    // WGSL reads the decimal as an abstract (64-bit) float and converts: make sure that path gives the same bits
    if s.parse::<f64>().map(|d| d as f32).ok().map(f32::to_bits) != Some(v.to_bits()) {
        return Err(format!("constant {v} does not survive decimal formatting"));
    }
    Ok(s)
}

/// The WGSL name of a register.
fn reg_name(src: &Src) -> String {
    src.reg.name()
}

/// Source operand narrowed to result components `comps` (indices into the swizzled value), as f32 / vecN.
fn src(s: &Src, comps: &[usize]) -> String {
    let letters: String = comps.iter().map(|k| XYZW[s.swizzle[*k] as usize]).collect();
    let mut e = reg_name(s);
    if letters != "xyzw" {
        e.push('.');
        e.push_str(&letters);
    }
    match s.modifier {
        SrcMod::None => e,
        SrcMod::Negate => format!("-{e}"),
        SrcMod::Abs => format!("abs({e})"),
        SrcMod::AbsNegate => format!("-abs({e})"),
        _ => unreachable!("refused by check"),
    }
}

/// Parenthesise an operand that starts with a minus sign when it is the right-hand side of a binary operator.
fn rhs(e: String) -> String {
    if e.starts_with('-') {
        format!("({e})")
    } else {
        e
    }
}

struct Stage<'a> {
    shader: &'a Shader,
    /// the lines of the function body
    body: Vec<(String, String)>,
}

impl Stage<'_> {
    fn line(&mut self, code: String, comment: String) {
        self.body.push((code, comment));
    }

    /// Assign `expr` (of `comps.len()` components, in mask order) to the masked components of `d`.
    fn assign(&mut self, d: &Dst, expr: String, comment: String) {
        let comps: Vec<usize> = (0..4).filter(|k| d.mask & (1 << k) != 0).collect();
        let expr = if d.saturate { format!("saturate({expr})") } else { expr };
        let r = d.reg.name();
        let code = match d.mask {
            MASK_ALL => format!("{r} = {expr};"),
            0b0111 => format!("{r} = vec4<f32>({expr}, {r}.w);"),
            0b0011 => format!("{r} = vec4<f32>({expr}, {r}.zw);"),
            _ if comps.len() == 1 => format!("{r}.{} = {expr};", XYZW[comps[0]]),
            _ => {
                let writes: Vec<String> = comps.iter().enumerate().map(|(n, k)| format!("{r}.{} = v.{};", XYZW[*k], XYZW[n])).collect();
                format!("{{ let v = {expr}; {} }}", writes.join(" "))
            }
        };
        self.line(code, comment);
    }

    fn instruction(&mut self, i: &Instruction) -> Result<(), String> {
        check::instruction(self.shader, i)?;
        let text = instruction_text(i);
        if matches!(i.opcode, Opcode::Nop | Opcode::Dcl | Opcode::Def) {
            return Ok(());
        }
        let d = i.dst.expect("every executable instruction implemented here has a destination");
        let comps: Vec<usize> = (0..4).filter(|k| d.mask & (1 << k) != 0).collect();
        let n = comps.len();
        let ty = vec_type(n);
        let s = |k: usize| src(&i.srcs[k], &comps);
        let xyz = [0usize, 1, 2];
        // a scalar result replicated to every written component
        let splat = |e: String| if n == 1 { e } else { format!("{ty}({e})") };
        let zero = if n == 1 { "0.0".to_owned() } else { format!("{ty}(0.0)") };
        let one = if n == 1 { "1.0".to_owned() } else { format!("{ty}(1.0)") };
        let expr = match i.opcode {
            Opcode::Mov => s(0),
            Opcode::Add => format!("{} + {}", s(0), rhs(s(1))),
            Opcode::Sub => format!("{} - {}", s(0), rhs(s(1))),
            Opcode::Mul => format!("{} * {}", s(0), rhs(s(1))),
            Opcode::Mad => format!("{} * {} + {}", s(0), rhs(s(1)), rhs(s(2))),
            Opcode::Rcp => format!("1.0 / {}", rhs(s(0))),
            Opcode::Rsq => format!("inverseSqrt(abs({}))", s(0)),
            Opcode::Dp3 => splat(format!("dot({}, {})", src(&i.srcs[0], &xyz), src(&i.srcs[1], &xyz))),
            Opcode::Dp4 => splat(format!("dot({}, {})", src(&i.srcs[0], &[0, 1, 2, 3]), src(&i.srcs[1], &[0, 1, 2, 3]))),
            Opcode::Min => format!("min({}, {})", s(0), s(1)),
            Opcode::Max => format!("max({}, {})", s(0), s(1)),
            Opcode::Slt => format!("select({zero}, {one}, {} < {})", s(0), rhs(s(1))),
            Opcode::Sge => format!("select({zero}, {one}, {} >= {})", s(0), rhs(s(1))),
            Opcode::Exp => format!("exp2({})", s(0)),
            Opcode::Log => format!("log2(abs({}))", s(0)),
            Opcode::Pow => format!("pow(abs({}), {})", s(0), s(1)),
            Opcode::Lrp => format!("{} * ({} - {}) + {}", s(0), s(1), rhs(s(2)), rhs(s(2))),
            Opcode::Frc => format!("fract({})", s(0)),
            Opcode::Abs => format!("abs({})", s(0)),
            Opcode::Nrm => format!("{} * inverseSqrt(dot({}, {}))", s(0), src(&i.srcs[0], &xyz), src(&i.srcs[0], &xyz)),
            Opcode::Cmp => format!("select({}, {}, {} >= {zero})", s(2), s(1), s(0)),
            Opcode::Dp2Add => splat(format!("dot({}, {}) + {}", src(&i.srcs[0], &[0, 1]), src(&i.srcs[1], &[0, 1]), rhs(src(&i.srcs[2], &[0])))),
            Opcode::SinCos => {
                let a = src(&i.srcs[0], &[0]);
                match d.mask {
                    1 => format!("cos({a})"),
                    2 => format!("sin({a})"),
                    _ => format!("vec2<f32>(cos({a}), sin({a}))"),
                }
            }
            Opcode::TexLd => {
                let unit = i.srcs[1].reg.num;
                let call = match i.control {
                    0 => format!("textureSample(tex{unit}, smp{unit}, {})", src(&i.srcs[0], &[0, 1])),
                    1 => format!("textureSample(tex{unit}, smp{unit}, {} / {})", src(&i.srcs[0], &[0, 1]), rhs(src(&i.srcs[0], &[3]))),
                    _ => format!("textureSampleBias(tex{unit}, smp{unit}, {}, {})", src(&i.srcs[0], &[0, 1]), src(&i.srcs[0], &[3])),
                };
                if d.mask == MASK_ALL {
                    call
                } else {
                    format!("{call}.{}", comps.iter().map(|k| XYZW[*k]).collect::<String>())
                }
            }
            Opcode::TexKill => {
                self.line(format!("if (any({} < vec4<f32>(0.0))) {{ discard; }}", d.reg.name()), text);
                return Ok(());
            }
            _ => unreachable!("refused by check"),
        };
        self.assign(&d, expr, text);
        Ok(())
    }
}

fn emit_body(out: &mut String, body: &[(String, String)]) {
    let width = body.iter().map(|(c, _)| c.len()).filter(|l| *l <= 72).max().unwrap_or(0);
    for (code, comment) in body {
        if comment.is_empty() {
            out.push_str(&format!("    {code}\n"));
        } else {
            out.push_str(&format!("    {code:width$}  // {comment}\n"));
        }
    }
}

/// Declarations shared by both stages: temporaries, constants.
fn prologue(stage: &mut Stage, uniform: &str) -> Result<(), String> {
    let shader = stage.shader;
    let defs: Vec<(u32, [f32; 4])> = shader.float_defs().collect();
    let mut constants: Vec<u32> = Vec::new();
    let mut temps: Vec<u32> = Vec::new();
    for i in shader.executable() {
        for s in &i.srcs {
            match s.reg.ty {
                RegType::Const if !constants.contains(&s.reg.num) => constants.push(s.reg.num),
                RegType::Temp if !temps.contains(&s.reg.num) => temps.push(s.reg.num),
                _ => {}
            }
        }
        if let Some(d) = &i.dst {
            if d.reg.ty == RegType::Temp && !temps.contains(&d.reg.num) {
                temps.push(d.reg.num);
            }
        }
    }
    constants.sort_unstable();
    temps.sort_unstable();
    let preshader_outputs = shader.preshader.as_ref().map(|p| p.outputs()).unwrap_or_default();
    for c in constants {
        if let Some((_, v)) = defs.iter().find(|(r, _)| *r == c) {
            let lits: Result<Vec<String>, String> = v.iter().map(|f| literal(*f)).collect();
            stage.line(format!("let c{c} = vec4<f32>({});", lits?.join(", ")), format!("def c{c}"));
            continue;
        }
        let mut what: Vec<String> = Vec::new();
        if let Some(k) = shader.ctab.as_ref().and_then(|t| t.float_at(c)) {
            what.push(format!("{} {}", k.hlsl(), k.name));
        }
        if let Some((_, mask)) = preshader_outputs.iter().find(|(r, _)| *r == c) {
            what.push(format!("preshader output .{}", super::disasm::mask_text(*mask)));
        }
        if what.is_empty() {
            return Err(format!("{}: register c{c} is read but is neither a def, a constant table entry nor a preshader output", shader.model()));
        }
        stage.line(format!("let c{c} = {uniform}[{c}];"), what.join("; "));
    }
    for r in temps {
        stage.line(format!("var r{r}: vec4<f32>;"), String::new());
    }
    Ok(())
}

/// The written component mask of every output register of a vertex shader.
fn vertex_outputs(vs: &Shader) -> Vec<(RegType, u32, u8)> {
    let mut out: Vec<(RegType, u32, u8)> = Vec::new();
    for i in vs.executable() {
        if let Some(d) = &i.dst {
            if matches!(d.reg.ty, RegType::RastOut | RegType::AttrOut | RegType::TexCrdOut) {
                match out.iter_mut().find(|(t, n, _)| *t == d.reg.ty && *n == d.reg.num) {
                    Some(e) => e.2 |= d.mask,
                    None => out.push((d.reg.ty, d.reg.num, d.mask)),
                }
            }
        }
    }
    out.sort_unstable();
    out
}

fn sampler_comment(shader: &Shader, unit: u32) -> String {
    shader.ctab.as_ref().and_then(|t| t.samplers().find(|c| (c.register_index..c.register_index + c.register_count).contains(&unit))).map(|c| format!("  // s{unit}: {}", c.name)).unwrap_or_default()
}

/// Translate one effect pass. `label` goes into the header comment.
pub fn translate(label: &str, pixel: &Shader, vertex: Option<&Shader>) -> Result<Translated, String> {
    if pixel.kind != ShaderKind::Pixel {
        return Err("the pixel shader of the pass is not a pixel shader".into());
    }
    check::shader(pixel)?;
    if let Some(vs) = vertex {
        if vs.kind != ShaderKind::Vertex {
            return Err("the vertex shader of the pass is not a vertex shader".into());
        }
        check::shader(vs)?;
        if let Some(c) = vs.ctab.as_ref().and_then(|t| t.constants.iter().find(|c| c.register_set != RegisterSet::Float4)) {
            return Err(format!("vs: constant {} is in the {:?} register set, which is not implemented", c.name, c.register_set));
        }
    }
    if let Some(t) = &pixel.ctab {
        if let Some(c) = t.constants.iter().find(|c| !matches!(c.register_set, RegisterSet::Float4 | RegisterSet::Sampler)) {
            return Err(format!("ps: constant {} is in the {:?} register set, which is not implemented", c.name, c.register_set));
        }
    }

    let mut out = String::new();
    out.push_str(&format!("// {label}\n// Generated by terraria-shaders from the Direct3D 9 bytecode of the pass. Do not edit: regenerate.\n"));
    out.push_str(&format!("// pixel shader: {}, {} instructions", pixel.model(), pixel.executable().count()));
    if let Some(p) = &pixel.preshader {
        out.push_str(&format!(" (+ a preshader of {} instructions run on the CPU)", p.instructions.len()));
    }
    out.push('\n');
    match vertex {
        Some(vs) => out.push_str(&format!("// vertex shader: {}, {} instructions\n\n", vs.model(), vs.executable().count())),
        None => out.push_str("// vertex stage: none in the pass: SpriteBatch's fixed vertex stage\n\n"),
    }

    // ---- samplers
    let mut samplers: Vec<u32> = pixel.declarations().filter(|(_, e)| matches!(e, Extra::DclSampler(_))).map(|(d, _)| d.reg.num).collect();
    samplers.sort_unstable();
    samplers.dedup();
    for i in pixel.executable().filter(|i| i.opcode == Opcode::TexLd) {
        let unit = i.srcs[1].reg.num;
        if !samplers.contains(&unit) {
            return Err(format!("ps: `{}` samples s{unit}, which is not declared", instruction_text(i)));
        }
    }
    for unit in &samplers {
        out.push_str(&format!("@group(0) @binding({}) var tex{unit}: texture_2d<f32>;{}\n", unit * 2, sampler_comment(pixel, *unit)));
        out.push_str(&format!("@group(0) @binding({}) var smp{unit}: sampler;\n", unit * 2 + 1));
    }

    // ---- uniforms
    let ps_registers = float_register_count(pixel);
    if ps_registers > 0 {
        out.push_str(&format!("@group(1) @binding(0) var<uniform> ps_c: array<vec4<f32>, {ps_registers}>;  // pixel shader float registers\n"));
    }
    let vs_registers = vertex.map(|vs| float_register_count(vs).max(1));
    if let Some(m) = vs_registers {
        out.push_str("struct VsUniforms {\n    half_pixel: vec4<f32>,  // (1 / viewport width, -1 / viewport height, 0, 0)\n");
        out.push_str(&format!("    c: array<vec4<f32>, {m}>,  // vertex shader float registers\n}};\n@group(1) @binding(1) var<uniform> vs_u: VsUniforms;\n"));
    }
    out.push('\n');

    // ---- pixel shader inputs
    let ps_inputs: Vec<(Dst, Extra)> = pixel.declarations().filter(|(d, _)| matches!(d.reg.ty, RegType::Texture | RegType::Input)).collect();
    for i in pixel.executable() {
        for s in &i.srcs {
            if matches!(s.reg.ty, RegType::Texture | RegType::Input) && !ps_inputs.iter().any(|(d, _)| d.reg == s.reg) {
                return Err(format!("ps: `{}` reads {}, which is not declared", instruction_text(i), s.reg.name()));
            }
        }
    }

    let mut fs = Stage { shader: pixel, body: Vec::new() };
    match vertex {
        None => {
            out.push_str("struct VsOut {\n    @builtin(position) position: vec4<f32>,\n    @location(0) uv: vec2<f32>,\n    @location(1) color: vec4<f32>,\n};\n\n");
            out.push_str("// SpriteBatch's vertex stage (not from bytecode): positions arrive in clip space.\n@vertex\nfn vs(@location(0) pos: vec2<f32>, @location(1) uv: vec2<f32>, @location(2) color: vec4<f32>) -> VsOut {\n");
            out.push_str("    var o: VsOut;\n    o.position = vec4<f32>(pos, 0.0, 1.0);\n    o.uv = uv;\n    o.color = color;\n    return o;\n}\n\n");
            for (d, _) in &ps_inputs {
                match (d.reg.ty, d.reg.num) {
                    (RegType::Texture, 0) if d.mask & !3 == 0 => fs.line("let t0 = vec4<f32>(i.uv, 0.0, 1.0);".into(), format!("dcl {}", super::disasm::dst_text(d))),
                    (RegType::Input, 0) => fs.line("let v0 = i.color;".into(), "dcl v0".into()),
                    _ => {
                        return Err(format!(
                            "ps: the pass has no vertex shader, so SpriteBatch's vertex stage feeds it; that stage provides t0.xy and v0, but the pixel shader declares {}",
                            super::disasm::dst_text(d)
                        ))
                    }
                }
            }
        }
        Some(vs) => {
            let outputs = vertex_outputs(vs);
            if !outputs.iter().any(|(t, n, m)| *t == RegType::RastOut && *n == 0 && *m == MASK_ALL) {
                return Err("vs: the program does not write all of oPos".into());
            }
            out.push_str("struct VsOut {\n    @builtin(position) position: vec4<f32>,\n");
            for (t, n, _) in &outputs {
                match t {
                    RegType::TexCrdOut => out.push_str(&format!("    @location({n}) oT{n}: vec4<f32>,\n")),
                    RegType::AttrOut => out.push_str(&format!("    @location({}) oD{n}: vec4<f32>,\n", 8 + n)),
                    _ => {}
                }
            }
            out.push_str("};\n\n");
            // vertex inputs by usage
            let mut vsb = Stage { shader: vs, body: Vec::new() };
            let mut declared: Vec<u32> = Vec::new();
            for (d, e) in vs.declarations() {
                let Extra::DclUsage { usage, index } = e else { return Err("vs: a declaration without a usage".into()) };
                let attribute = match (usage, index) {
                    (Usage::Position, 0) => "a_position",
                    (Usage::TexCoord, 0) => "a_texcoord",
                    (Usage::Color, 0) => "a_color",
                    _ => return Err(format!("vs: vertex input {}{index} is not provided by the port's vertex format (position, texcoord0, color0)", usage.name())),
                };
                vsb.line(format!("let v{} = {attribute};", d.reg.num), format!("dcl_{} v{}", usage.name(), d.reg.num));
                declared.push(d.reg.num);
            }
            for i in vs.executable() {
                for s in &i.srcs {
                    if s.reg.ty == RegType::Input && !declared.contains(&s.reg.num) {
                        return Err(format!("vs: `{}` reads {}, which is not declared", instruction_text(i), s.reg.name()));
                    }
                }
            }
            prologue(&mut vsb, "vs_u.c")?;
            for (t, n, _) in &outputs {
                let name = super::bytecode::Reg { ty: *t, num: *n }.name();
                vsb.line(format!("var {name}: vec4<f32>;"), String::new());
            }
            for i in &vs.instructions {
                vsb.instruction(i).map_err(|e| format!("vs: {e}"))?;
            }
            vsb.line("var o: VsOut;".into(), String::new());
            vsb.line("o.position = vec4<f32>(oPos.xy + vs_u.half_pixel.xy * oPos.w, oPos.zw);".into(), "Direct3D 9 pixel centres".into());
            for (t, n, _) in &outputs {
                match t {
                    RegType::TexCrdOut => vsb.line(format!("o.oT{n} = oT{n};"), String::new()),
                    RegType::AttrOut => vsb.line(format!("o.oD{n} = saturate(oD{n});"), "shader model 2 clamps colour outputs".into()),
                    _ => {}
                }
            }
            vsb.line("return o;".into(), String::new());
            out.push_str("@vertex\nfn vs(@location(0) a_position: vec4<f32>, @location(1) a_texcoord: vec4<f32>, @location(2) a_color: vec4<f32>) -> VsOut {\n");
            emit_body(&mut out, &vsb.body);
            out.push_str("}\n\n");

            for (d, _) in &ps_inputs {
                let (ty, name) = match d.reg.ty {
                    RegType::Texture => (RegType::TexCrdOut, format!("oT{}", d.reg.num)),
                    _ => (RegType::AttrOut, format!("oD{}", d.reg.num)),
                };
                let written = outputs.iter().find(|(t, n, _)| *t == ty && *n == d.reg.num).map_or(0, |o| o.2);
                if d.mask & !written != 0 {
                    return Err(format!("ps: declares {} but the vertex shader writes only {name}.{}", super::disasm::dst_text(d), super::disasm::mask_text(written)));
                }
                fs.line(format!("let {} = i.{name};", d.reg.name()), format!("dcl {}", super::disasm::dst_text(d)));
            }
        }
    }

    prologue(&mut fs, "ps_c")?;
    if !pixel.executable().any(|i| i.dst.is_some_and(|d| d.reg.ty == RegType::ColorOut && d.mask == MASK_ALL)) {
        return Err("ps: the program never writes all of oC0".into());
    }
    fs.line("var oC0: vec4<f32>;".into(), String::new());
    for i in &pixel.instructions {
        fs.instruction(i).map_err(|e| format!("ps: {e}"))?;
    }
    fs.line("return oC0;".into(), String::new());
    out.push_str("@fragment\nfn fs(i: VsOut) -> @location(0) vec4<f32> {\n");
    emit_body(&mut out, &fs.body);
    out.push_str("}\n");

    Ok(Translated { source: out, ps_registers, vs_registers, samplers })
}

#[cfg(test)]
mod tests {
    use super::super::bytecode;
    use super::*;

    #[test]
    fn the_tiny_shader_translates() {
        let s = bytecode::parse(&bytecode::tests::tiny_ps()).unwrap();
        let t = translate("test", &s, None).unwrap();
        assert_eq!((t.ps_registers, t.vs_registers, t.samplers.clone()), (0, None, vec![0]));
        assert!(t.source.contains("r0 = textureSample(tex0, smp0, t0.xy);"), "{}", t.source);
        assert!(t.source.contains("oC0 = r0;"), "{}", t.source);
    }

    #[test]
    fn literals_round_trip() {
        for v in [0.1f32, 1.0, -0.33333334, 0.00055555557, 1e-10, 16777216.0, f32::MIN_POSITIVE] {
            let s = literal(v).unwrap();
            assert_eq!((s.parse::<f64>().unwrap() as f32).to_bits(), v.to_bits(), "{s}");
        }
        assert!(literal(f32::INFINITY).is_err());
    }
}
