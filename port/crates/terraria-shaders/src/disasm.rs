//! Assembly listing of a decoded shader, in the notation of Microsoft's shader assembler (`ps_2_0` ... `mad r0.xyz,
//! r1, c0.x, c1`), with the constant table and the preshader as comments in front.

use super::bytecode::{Dst, Extra, Instruction, Opcode, RegType, SamplerType, Shader, Src, SrcMod, MASK_ALL, SWIZZLE_IDENTITY};
use super::ctab::ConstantTable;

const XYZW: [char; 4] = ['x', 'y', 'z', 'w'];

pub fn mask_text(mask: u8) -> String {
    (0..4).filter(|k| mask & (1 << k) != 0).map(|k| XYZW[k]).collect()
}

/// Shortest decimal that reads back as the same `f32`; whole numbers without a fraction.
pub fn fmt_f32(v: f32) -> String {
    format!("{v}")
}

pub fn dst_text(d: &Dst) -> String {
    let mut s = d.reg.name();
    if d.mask != MASK_ALL && d.reg.ty != RegType::Sampler {
        s.push('.');
        s.push_str(&mask_text(d.mask));
    }
    s
}

pub fn src_text(src: &Src) -> String {
    let mut s = src.reg.name();
    if let Some((reg, comp)) = src.relative {
        s = format!("{}[{}.{}]", s, reg.name(), XYZW[comp as usize]);
    }
    let (prefix, suffix) = match src.modifier {
        SrcMod::None => ("", ""),
        SrcMod::Negate => ("-", ""),
        SrcMod::Bias => ("", "_bias"),
        SrcMod::BiasNegate => ("-", "_bias"),
        SrcMod::Sign => ("", "_bx2"),
        SrcMod::SignNegate => ("-", "_bx2"),
        SrcMod::Complement => ("1-", ""),
        SrcMod::X2 => ("", "_x2"),
        SrcMod::X2Negate => ("-", "_x2"),
        SrcMod::Dz => ("", "_dz"),
        SrcMod::Dw => ("", "_dw"),
        SrcMod::Abs => ("", "_abs"),
        SrcMod::AbsNegate => ("-", "_abs"),
        SrcMod::Not => ("!", ""),
    };
    s = format!("{prefix}{s}{suffix}");
    if src.reg.ty != RegType::Sampler && src.swizzle != SWIZZLE_IDENTITY {
        s.push('.');
        if src.swizzle.iter().all(|c| *c == src.swizzle[0]) {
            s.push(XYZW[src.swizzle[0] as usize]);
        } else {
            s.extend(src.swizzle.iter().map(|c| XYZW[*c as usize]));
        }
    }
    s
}

const COMPARISONS: [&str; 7] = ["", "_gt", "_eq", "_ge", "_lt", "_ne", "_le"];

/// One instruction as assembly text.
pub fn instruction_text(i: &Instruction) -> String {
    let mut name = i.opcode.name().to_owned();
    match i.opcode {
        Opcode::TexLd => match i.control {
            0 => {}
            1 => name.push('p'),
            2 => name.push('b'),
            other => name.push_str(&format!("?{other}")),
        },
        Opcode::Ifc | Opcode::BreakC | Opcode::SetP => name.push_str(COMPARISONS.get(i.control as usize).copied().unwrap_or("_?")),
        Opcode::Dcl => match i.extra {
            Extra::DclSampler(SamplerType::D2) => name.push_str("_2d"),
            Extra::DclSampler(SamplerType::Cube) => name.push_str("_cube"),
            Extra::DclSampler(SamplerType::Volume) => name.push_str("_volume"),
            Extra::DclUsage { usage, index } => {
                name.push('_');
                name.push_str(usage.name());
                if index != 0 {
                    name.push_str(&index.to_string());
                }
            }
            _ => {}
        },
        _ => {}
    }
    if let Some(d) = &i.dst {
        if d.saturate {
            name.push_str("_sat");
        }
        if d.partial_precision {
            name.push_str("_pp");
        }
        if d.centroid {
            name.push_str("_centroid");
        }
    }
    let mut operands: Vec<String> = Vec::new();
    if let Some(d) = &i.dst {
        operands.push(dst_text(d));
    }
    operands.extend(i.srcs.iter().map(src_text));
    match i.extra {
        Extra::DefFloat(v) => operands.extend(v.iter().map(|f| fmt_f32(*f))),
        Extra::DefInt(v) => operands.extend(v.iter().map(|f| f.to_string())),
        Extra::DefBool(v) => operands.push(v.to_string()),
        _ => {}
    }
    if operands.is_empty() {
        name
    } else {
        format!("{name} {}", operands.join(", "))
    }
}

fn table_text(out: &mut String, table: &ConstantTable, indent: &str) {
    let width = table.constants.iter().map(|c| c.hlsl().len()).max().unwrap_or(0);
    let name_width = table.constants.iter().map(|c| c.name.len()).max().unwrap_or(0);
    for c in &table.constants {
        out.push_str(&format!("//{indent}{:width$} {:name_width$}  {}", c.hlsl(), c.name, c.registers()));
        if let Some(d) = &c.default_value {
            out.push_str(&format!("  default ({})", d.iter().map(|f| fmt_f32(*f)).collect::<Vec<_>>().join(", ")));
        }
        out.push('\n');
    }
}

/// The whole listing: constant table, preshader, then the program.
pub fn listing(shader: &Shader) -> String {
    let mut out = String::new();
    if let Some(t) = &shader.ctab {
        out.push_str(&format!("// Constant table ({}; {}):\n", t.target, t.creator));
        table_text(&mut out, t, "   ");
        out.push_str("//\n");
    } else {
        out.push_str("// No constant table.\n//\n");
    }
    if let Some(p) = &shader.preshader {
        out.push_str("// Preshader (run on the CPU when the pass is applied; c# = its inputs, oc# = constants of the shader below,\n");
        out.push_str("// r# = temporaries, (..) = literals):\n");
        table_text(&mut out, &p.ctab, "   ");
        out.push_str("//\n");
        for i in &p.instructions {
            out.push_str(&format!("//     {}\n", i.text(&p.literals)));
        }
        out.push_str("//\n");
    }
    for fourcc in &shader.other_comments {
        out.push_str(&format!("// (comment block {:?} not decoded)\n", String::from_utf8_lossy(fourcc)));
    }
    out.push_str(&format!("    {}\n", shader.model()));
    for i in &shader.instructions {
        out.push_str(&format!("    {}\n", instruction_text(i)));
    }
    out.push_str(&format!("\n// {} instructions\n", shader.executable().count()));
    out
}

#[cfg(test)]
mod tests {
    use super::super::bytecode;

    #[test]
    fn lists_the_tiny_shader() {
        let s = bytecode::parse(&bytecode::tests::tiny_ps()).unwrap();
        let text = super::listing(&s);
        assert!(text.contains("    ps_2_0\n    dcl t0.xy\n    dcl_2d s0\n    texld r0, t0, s0\n    mov oC0, r0\n"), "{text}");
        assert!(text.ends_with("// 2 instructions\n"));
    }
}
