//! What the translator and the interpreter implement, checked up front so that anything else is a hard error
//! naming the instruction instead of a wrong picture.

use super::bytecode::{Extra, Instruction, Opcode, RegType, SamplerType, Shader, ShaderKind, SrcMod, MASK_ALL};
use super::disasm::instruction_text;

/// Instructions with defined semantics here. Everything else the decoder knows is refused by name.
pub const SUPPORTED: &[Opcode] = &[
    Opcode::Nop,
    Opcode::Mov,
    Opcode::Add,
    Opcode::Sub,
    Opcode::Mad,
    Opcode::Mul,
    Opcode::Rcp,
    Opcode::Rsq,
    Opcode::Dp3,
    Opcode::Dp4,
    Opcode::Min,
    Opcode::Max,
    Opcode::Slt,
    Opcode::Sge,
    Opcode::Exp,
    Opcode::Log,
    Opcode::Lrp,
    Opcode::Frc,
    Opcode::Pow,
    Opcode::Abs,
    Opcode::Nrm,
    Opcode::SinCos,
    Opcode::TexKill,
    Opcode::TexLd,
    Opcode::Cmp,
    Opcode::Dp2Add,
    Opcode::Dcl,
    Opcode::Def,
];

/// Instructions whose result depends on how the GPU evaluates a transcendental function or a reciprocal square
/// root (compared with a tolerance of one 8-bit step against the interpreter; everything else is plain IEEE
/// arithmetic).
pub const INEXACT: &[Opcode] = &[Opcode::Rsq, Opcode::Exp, Opcode::Log, Opcode::Pow, Opcode::Nrm, Opcode::SinCos];

fn replicate(swizzle: [u8; 4], mask: u8) -> bool {
    let mut used = (0..4).filter(|k| mask & (1 << k) != 0).map(|k| swizzle[k]);
    let first = used.next();
    used.all(|c| Some(c) == first)
}

/// Check one instruction; `Err` names it.
pub fn instruction(shader: &Shader, i: &Instruction) -> Result<(), String> {
    let fail = |why: &str| Err(format!("`{}`: {why}", instruction_text(i)));
    if !SUPPORTED.contains(&i.opcode) {
        return fail(&format!("the {} instruction is not implemented", i.opcode.name()));
    }
    if i.coissue {
        return fail("co-issued instructions are not implemented");
    }
    let pixel = shader.kind == ShaderKind::Pixel;
    for s in &i.srcs {
        if !matches!(s.modifier, SrcMod::None | SrcMod::Negate | SrcMod::Abs | SrcMod::AbsNegate) {
            return fail(&format!("the source modifier `{}` is not implemented", s.modifier.name()));
        }
        if s.relative.is_some() {
            return fail("relative addressing is not implemented");
        }
        let ok = match s.reg.ty {
            RegType::Temp | RegType::Const | RegType::Input | RegType::Sampler => true,
            RegType::Texture => pixel,
            _ => false,
        };
        if !ok {
            return fail(&format!("reading register {} is not implemented", s.reg.name()));
        }
    }
    if let Some(d) = &i.dst {
        let ok = match (i.opcode, d.reg.ty) {
            (Opcode::Def, RegType::Const) => true,
            (Opcode::Dcl, RegType::Input | RegType::Sampler) => true,
            (Opcode::Dcl, RegType::Texture) => pixel,
            (Opcode::Def | Opcode::Dcl, _) => false,
            (_, RegType::Temp) => true,
            (_, RegType::ColorOut) => pixel && d.reg.num == 0,
            (_, RegType::RastOut) => !pixel && d.reg.num == 0,
            (_, RegType::AttrOut) => !pixel && d.reg.num < 2,
            (_, RegType::TexCrdOut) => !pixel && d.reg.num < 8,
            _ => false,
        };
        if !ok {
            return fail(&format!("writing register {} is not implemented", d.reg.name()));
        }
        if d.mask == 0 {
            return fail("empty write mask");
        }
    }
    let mask = i.dst.map_or(MASK_ALL, |d| d.mask);
    match i.opcode {
        Opcode::Dcl => {
            if let Extra::DclSampler(t) = i.extra {
                if t != SamplerType::D2 {
                    return fail("only 2D samplers are implemented");
                }
            }
        }
        // Direct3D defines these on one scalar (the source must repeat one component)
        Opcode::Rcp | Opcode::Rsq | Opcode::Log | Opcode::Pow => {
            if i.srcs.iter().any(|s| !replicate(s.swizzle, mask)) {
                return fail("a scalar instruction whose source selects different components");
            }
        }
        Opcode::SinCos => {
            if shader.major != 2 {
                return fail("only the shader model 2 form of sincos is implemented");
            }
            if mask & !3 != 0 {
                return fail("sincos writes x (cos) and y (sin) only");
            }
            if !replicate(i.srcs[0].swizzle, MASK_ALL) {
                return fail("sincos takes one scalar");
            }
        }
        Opcode::Dp2Add => {
            if !replicate(i.srcs[2].swizzle, MASK_ALL) {
                return fail("the third source of dp2add must select one component");
            }
        }
        Opcode::TexLd => {
            if !pixel {
                return fail("texture sampling in a vertex shader is not implemented");
            }
            if i.control > 2 {
                return fail("unknown texld flag");
            }
            if i.srcs[1].reg.ty != RegType::Sampler {
                return fail("the second source of texld must be a sampler");
            }
        }
        Opcode::TexKill => {
            if !pixel {
                return fail("texkill in a vertex shader");
            }
            if mask != MASK_ALL {
                return fail("texkill with a write mask is not implemented");
            }
        }
        _ => {}
    }
    if i.opcode != Opcode::TexLd && i.srcs.iter().any(|s| s.reg.ty == RegType::Sampler) {
        return fail("a sampler register outside texld");
    }
    Ok(())
}

/// Check a whole shader.
pub fn shader(s: &Shader) -> Result<(), String> {
    if s.major != 2 {
        return Err(format!("{}: only shader model 2 programs are implemented", s.model()));
    }
    for i in &s.instructions {
        instruction(s, i).map_err(|e| format!("{}: {e}", s.model()))?;
    }
    Ok(())
}

/// `true` when the shader uses an instruction from [`INEXACT`].
pub fn is_inexact(s: &Shader) -> bool {
    s.instructions.iter().any(|i| INEXACT.contains(&i.opcode))
}
