//! CPU reference interpreter for the same bytecode, with a software texture sampler.
//!
//! It exists to test the WGSL path: both execute the instruction list with the semantics written down in
//! [`super::wgsl`], this one directly on `f32` registers. Registers start at zero (Direct3D leaves them undefined
//! and its validator rejects reads before writes). The sampler does point and bilinear filtering of one mip
//! level with wrap / clamp / mirror addressing, texel centres at half-integers (as wgpu, Vulkan and Direct3D 10+
//! define it; with Direct3D 9's half-texel shifted rasterisation this gives the same texels for a 1:1 sprite).

use super::bytecode::{Dst, Opcode, Reg, RegType, Shader, ShaderKind, Src, SrcMod};
use super::check;

pub type Vec4 = [f32; 4];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Address {
    Wrap,
    Clamp,
    Mirror,
}

/// An RGBA8 texture (rows top to bottom).
#[derive(Clone, Copy, Debug)]
pub struct Texture<'a> {
    pub width: u32,
    pub height: u32,
    pub rgba: &'a [u8],
}

/// What is bound to one sampler register.
#[derive(Clone, Copy, Debug)]
pub struct Sampler<'a> {
    /// `None`: nothing bound; Direct3D 9 samples opaque black (0, 0, 0, 1)
    pub texture: Option<Texture<'a>>,
    pub linear: bool,
    pub address_u: Address,
    pub address_v: Address,
}

impl Sampler<'_> {
    pub const UNBOUND: Sampler<'static> = Sampler { texture: None, linear: false, address_u: Address::Clamp, address_v: Address::Clamp };
}

fn address(i: i64, size: i64, mode: Address) -> usize {
    (match mode {
        Address::Wrap => i.rem_euclid(size),
        Address::Clamp => i.clamp(0, size - 1),
        Address::Mirror => {
            // Vulkan: (size - 1) - mirror((i mod 2 size) - size), mirror(n) = n >= 0 ? n : -(1 + n)
            let n = i.rem_euclid(2 * size) - size;
            (size - 1) - if n >= 0 { n } else { -(1 + n) }
        }
    }) as usize
}

/// Sample at normalised coordinates (`texld`).
pub fn sample(s: &Sampler, u: f32, v: f32) -> Vec4 {
    let Some(t) = &s.texture else { return [0.0, 0.0, 0.0, 1.0] };
    let (w, h) = (t.width as i64, t.height as i64);
    let texel = |x: i64, y: i64| -> Vec4 {
        let at = (address(y, h, s.address_v) * t.width as usize + address(x, w, s.address_u)) * 4;
        [t.rgba[at] as f32 / 255.0, t.rgba[at + 1] as f32 / 255.0, t.rgba[at + 2] as f32 / 255.0, t.rgba[at + 3] as f32 / 255.0]
    };
    if !(u.is_finite() && v.is_finite()) {
        return texel(0, 0);
    }
    if !s.linear {
        return texel((u * t.width as f32).floor() as i64, (v * t.height as f32).floor() as i64);
    }
    let (x, y) = (u * t.width as f32 - 0.5, v * t.height as f32 - 0.5);
    let (x0, y0) = (x.floor(), y.floor());
    let (fx, fy) = (x - x0, y - y0);
    let (x0, y0) = (x0 as i64, y0 as i64);
    let (a, b, c, d) = (texel(x0, y0), texel(x0 + 1, y0), texel(x0, y0 + 1), texel(x0 + 1, y0 + 1));
    let mut out = [0f32; 4];
    for k in 0..4 {
        let top = a[k] + (b[k] - a[k]) * fx;
        let bottom = c[k] + (d[k] - c[k]) * fx;
        out[k] = top + (bottom - top) * fy;
    }
    out
}

/// Inputs of a pixel shader.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PixelInputs {
    /// `t0..t7`
    pub t: [Vec4; 8],
    /// `v0`, `v1`
    pub v: [Vec4; 2],
}

/// Outputs of a vertex shader.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct VertexOutputs {
    /// `oPos` (before the half-pixel adjustment the WGSL adds)
    pub position: Vec4,
    /// `oT0..oT7`
    pub t: [Vec4; 8],
    /// `oD0`, `oD1`, clamped to 0..1
    pub d: [Vec4; 2],
}

struct Machine<'a> {
    shader: &'a Shader,
    r: [Vec4; 32],
    c: Vec<Vec4>,
    inputs: [Vec4; 16],
    t: [Vec4; 8],
    out_color: Vec4,
    out_pos: Vec4,
    out_t: [Vec4; 8],
    out_d: [Vec4; 2],
    samplers: &'a [Sampler<'a>],
    killed: bool,
    perturb: Perturb,
}

/// Deliberate small errors, for testing how sensitive a pixel is (the comparison with the GPU uses this to tell
/// a discontinuity of the shader from a wrong translation). All zero: the reference result.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Perturb {
    /// added to every component of every `texld` result
    pub sample: f32,
    /// added to every component of the result of the instructions in `check::INEXACT`
    pub transcendental: f32,
}

impl Machine<'_> {
    fn read_reg(&self, reg: Reg) -> Result<Vec4, String> {
        let n = reg.num as usize;
        let missing = || format!("register {} is outside the register file", reg.name());
        Ok(match reg.ty {
            RegType::Temp => *self.r.get(n).ok_or_else(missing)?,
            RegType::Const => *self.c.get(n).ok_or_else(missing)?,
            RegType::Input => *self.inputs.get(n).ok_or_else(missing)?,
            RegType::Texture => *self.t.get(n).ok_or_else(missing)?,
            _ => return Err(format!("reading register {} is not implemented", reg.name())),
        })
    }

    fn src(&self, s: &Src) -> Result<Vec4, String> {
        let v = self.read_reg(s.reg)?;
        let mut o = [v[s.swizzle[0] as usize], v[s.swizzle[1] as usize], v[s.swizzle[2] as usize], v[s.swizzle[3] as usize]];
        for x in &mut o {
            *x = match s.modifier {
                SrcMod::None => *x,
                SrcMod::Negate => -*x,
                SrcMod::Abs => x.abs(),
                SrcMod::AbsNegate => -x.abs(),
                other => return Err(format!("the source modifier `{}` is not implemented", other.name())),
            };
        }
        Ok(o)
    }

    fn write(&mut self, d: &Dst, mut v: Vec4) -> Result<(), String> {
        if d.saturate {
            for x in &mut v {
                // WGSL saturate = clamp(x, 0, 1) = min(max(x, 0), 1)
                *x = x.max(0.0).min(1.0);
            }
        }
        let n = d.reg.num as usize;
        let target: &mut Vec4 = match d.reg.ty {
            RegType::Temp => self.r.get_mut(n),
            RegType::ColorOut if n == 0 => Some(&mut self.out_color),
            RegType::RastOut if n == 0 => Some(&mut self.out_pos),
            RegType::AttrOut => self.out_d.get_mut(n),
            RegType::TexCrdOut => self.out_t.get_mut(n),
            _ => None,
        }
        .ok_or_else(|| format!("writing register {} is not implemented", d.reg.name()))?;
        for k in 0..4 {
            if d.mask & (1 << k) != 0 {
                target[k] = v[k];
            }
        }
        Ok(())
    }

    fn run(&mut self) -> Result<(), String> {
        for i in &self.shader.instructions {
            if matches!(i.opcode, Opcode::Nop | Opcode::Dcl | Opcode::Def) {
                continue;
            }
            let d = i.dst.ok_or("instruction without a destination")?;
            let mut s = [[0f32; 4]; 3];
            for (k, src) in i.srcs.iter().enumerate().take(3) {
                if src.reg.ty != RegType::Sampler {
                    s[k] = self.src(src)?;
                }
            }
            let [a, b, c] = s;
            let each = |f: &dyn Fn(usize) -> f32| -> Vec4 { [f(0), f(1), f(2), f(3)] };
            let dot3 = |x: Vec4, y: Vec4| x[0] * y[0] + x[1] * y[1] + x[2] * y[2];
            let v: Vec4 = match i.opcode {
                Opcode::Mov => a,
                Opcode::Add => each(&|k| a[k] + b[k]),
                Opcode::Sub => each(&|k| a[k] - b[k]),
                Opcode::Mul => each(&|k| a[k] * b[k]),
                Opcode::Mad => each(&|k| a[k] * b[k] + c[k]),
                Opcode::Rcp => each(&|k| 1.0 / a[k]),
                Opcode::Rsq => each(&|k| 1.0 / a[k].abs().sqrt()),
                Opcode::Dp3 => [dot3(a, b); 4],
                Opcode::Dp4 => [dot3(a, b) + a[3] * b[3]; 4],
                Opcode::Min => each(&|k| a[k].min(b[k])),
                Opcode::Max => each(&|k| a[k].max(b[k])),
                Opcode::Slt => each(&|k| (a[k] < b[k]) as u8 as f32),
                Opcode::Sge => each(&|k| (a[k] >= b[k]) as u8 as f32),
                Opcode::Exp => each(&|k| a[k].exp2()),
                Opcode::Log => each(&|k| a[k].abs().log2()),
                Opcode::Pow => each(&|k| a[k].abs().powf(b[k])),
                Opcode::Lrp => each(&|k| a[k] * (b[k] - c[k]) + c[k]),
                Opcode::Frc => each(&|k| a[k] - a[k].floor()),
                Opcode::Abs => each(&|k| a[k].abs()),
                Opcode::Nrm => {
                    let f = 1.0 / dot3(a, a).sqrt();
                    each(&|k| a[k] * f)
                }
                Opcode::Cmp => each(&|k| if a[k] >= 0.0 { b[k] } else { c[k] }),
                Opcode::Dp2Add => [a[0] * b[0] + a[1] * b[1] + c[0]; 4],
                Opcode::SinCos => [a[0].cos(), a[0].sin(), 0.0, 0.0],
                Opcode::TexLd => {
                    let unit = i.srcs[1].reg.num as usize;
                    let smp = self.samplers.get(unit).copied().unwrap_or(Sampler::UNBOUND);
                    // flag 2 (bias) changes the mip level only: one level here
                    let (u, v) = if i.control == 1 { (a[0] / a[3], a[1] / a[3]) } else { (a[0], a[1]) };
                    sample(&smp, u, v).map(|x| x + self.perturb.sample)
                }
                Opcode::TexKill => {
                    let value = self.read_reg(d.reg)?;
                    if value.iter().any(|x| *x < 0.0) {
                        self.killed = true;
                    }
                    continue;
                }
                other => return Err(format!("the {} instruction is not implemented", other.name())),
            };
            let v = if self.perturb.transcendental != 0.0 && check::INEXACT.contains(&i.opcode) { v.map(|x| x + self.perturb.transcendental) } else { v };
            self.write(&d, v)?;
        }
        Ok(())
    }
}

fn machine<'a>(shader: &'a Shader, constants: &[Vec4], samplers: &'a [Sampler<'a>]) -> Result<Machine<'a>, String> {
    check::shader(shader)?;
    let mut c = constants.to_vec();
    for (reg, value) in shader.float_defs() {
        if c.len() <= reg as usize {
            c.resize(reg as usize + 1, [0.0; 4]);
        }
        c[reg as usize] = value;
    }
    Ok(Machine { shader, r: [[0.0; 4]; 32], c, inputs: [[0.0; 4]; 16], t: [[0.0; 4]; 8], out_color: [0.0; 4], out_pos: [0.0; 4], out_t: [[0.0; 4]; 8], out_d: [[0.0; 4]; 2], samplers, killed: false, perturb: Perturb::default() })
}

/// A prepared pixel shader: validated once, run per pixel.
pub struct PixelProgram<'a> {
    template: Machine<'a>,
}

impl<'a> PixelProgram<'a> {
    /// `constants`: the float register file from [`super::uniforms::fill`]; `samplers[n]` is register `s{n}`.
    pub fn new(shader: &'a Shader, constants: &[Vec4], samplers: &'a [Sampler<'a>]) -> Result<Self, String> {
        if shader.kind != ShaderKind::Pixel {
            return Err("not a pixel shader".into());
        }
        Ok(PixelProgram { template: machine(shader, constants, samplers)? })
    }

    /// The colour written to `oC0`, or `None` when `texkill` discarded the pixel.
    pub fn run(&self, inputs: &PixelInputs) -> Result<Option<Vec4>, String> {
        self.run_perturbed(inputs, Perturb::default())
    }

    /// [`Self::run`] with deliberate small errors (see [`Perturb`]).
    pub fn run_perturbed(&self, inputs: &PixelInputs, perturb: Perturb) -> Result<Option<Vec4>, String> {
        let t = &self.template;
        let mut m = Machine { shader: t.shader, r: t.r, c: t.c.clone(), inputs: t.inputs, t: inputs.t, out_color: t.out_color, out_pos: t.out_pos, out_t: t.out_t, out_d: t.out_d, samplers: t.samplers, killed: false, perturb };
        m.inputs[0] = inputs.v[0];
        m.inputs[1] = inputs.v[1];
        m.run()?;
        Ok(if m.killed { None } else { Some(m.out_color) })
    }
}

/// Run a vertex shader on one vertex. `inputs[n]` is register `v{n}`.
pub fn run_vertex(shader: &Shader, constants: &[Vec4], inputs: &[Vec4; 16]) -> Result<VertexOutputs, String> {
    if shader.kind != ShaderKind::Vertex {
        return Err("not a vertex shader".into());
    }
    let mut m = machine(shader, constants, &[])?;
    m.inputs = *inputs;
    m.run()?;
    let mut d = m.out_d;
    for v in &mut d {
        for x in v.iter_mut() {
            *x = x.max(0.0).min(1.0);
        }
    }
    Ok(VertexOutputs { position: m.out_pos, t: m.out_t, d })
}

/// Float colour -> the byte an `Rgba8Unorm` target stores (round to nearest).
pub fn to_unorm8(v: f32) -> u8 {
    if v.is_nan() {
        return 0;
    }
    (v.clamp(0.0, 1.0) * 255.0 + 0.5).floor() as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEX: [u8; 16] = [0, 0, 0, 255, 255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255]; // 2x2: black red / green blue

    fn sampler(linear: bool, mode: Address) -> Sampler<'static> {
        Sampler { texture: Some(Texture { width: 2, height: 2, rgba: &TEX }), linear, address_u: mode, address_v: mode }
    }

    #[test]
    fn point_sampling_and_address_modes() {
        let s = sampler(false, Address::Clamp);
        assert_eq!(sample(&s, 0.25, 0.25), [0.0, 0.0, 0.0, 1.0]);
        assert_eq!(sample(&s, 0.75, 0.25), [1.0, 0.0, 0.0, 1.0]);
        assert_eq!(sample(&s, 0.25, 0.75), [0.0, 1.0, 0.0, 1.0]);
        assert_eq!(sample(&s, 1.75, 0.25), [1.0, 0.0, 0.0, 1.0], "clamp keeps the edge texel");
        assert_eq!(sample(&s, -0.75, 0.25), [0.0, 0.0, 0.0, 1.0]);
        let w = sampler(false, Address::Wrap);
        assert_eq!(sample(&w, 1.25, 0.25), [0.0, 0.0, 0.0, 1.0]);
        assert_eq!(sample(&w, -0.25, 0.25), [1.0, 0.0, 0.0, 1.0]);
        let m = sampler(false, Address::Mirror);
        assert_eq!(sample(&m, 1.25, 0.25), [1.0, 0.0, 0.0, 1.0], "mirrored: the first texel past the edge is the edge texel");
        assert_eq!(sample(&m, 1.75, 0.25), [0.0, 0.0, 0.0, 1.0]);
        assert_eq!(sample(&m, -0.25, 0.25), [0.0, 0.0, 0.0, 1.0]);
        assert_eq!(sample(&Sampler::UNBOUND, 0.5, 0.5), [0.0, 0.0, 0.0, 1.0]);
    }

    #[test]
    fn bilinear_sampling() {
        let s = sampler(true, Address::Clamp);
        // texel centres give the texel
        assert_eq!(sample(&s, 0.75, 0.25), [1.0, 0.0, 0.0, 1.0]);
        // the middle is the average of the four
        assert_eq!(sample(&s, 0.5, 0.5), [0.25, 0.25, 0.25, 1.0]);
        // clamped: left of the first centre is the first texel
        assert_eq!(sample(&s, 0.0, 0.25), [0.0, 0.0, 0.0, 1.0]);
        // wrapped: u = 0 is halfway between the last and the first texel of the row
        let w = sampler(true, Address::Wrap);
        assert_eq!(sample(&w, 0.0, 0.25), [0.5, 0.0, 0.0, 1.0]);
    }

    #[test]
    fn unorm_rounding() {
        assert_eq!((to_unorm8(0.0), to_unorm8(1.0), to_unorm8(2.0), to_unorm8(-1.0)), (0, 255, 255, 0));
        assert_eq!(to_unorm8(0.5), 128);
        assert_eq!(to_unorm8(127.4 / 255.0), 127);
    }
}
