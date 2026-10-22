//! Decoder for Direct3D 9 shader bytecode (the token stream of `ps_2_0` / `vs_2_0` / `*_3_0` programs).
//!
//! Format (MSDN "Direct3D Shader Codes"; the same layout is read by Wine, MojoShader and vkd3d-shader's d3dbc):
//! ```text
//! version token      0xFFFF0000 | major << 8 | minor   pixel shader;   0xFFFE0000 | ...   vertex shader
//! comment token      bits 0..15 = 0xFFFE, bits 16..30 = length in tokens; the first data token is a FOURCC
//!                    ("CTAB" constant table, "PRES" preshader of a D3DX effect)
//! instruction token  bits 0..15 opcode, 16..23 opcode specific (texld: 1 projected, 2 bias; ifc / breakc / setp:
//!                    comparison), 24..27 number of tokens that follow (shader model 2+), 28 predicated,
//!                    30 co-issue, 31 zero
//! destination token  bits 0..10 register number, 11..12 and 28..30 register type, 13 relative addressing,
//!                    16..19 write mask, 20..23 result modifier (1 saturate, 2 partial precision, 4 centroid),
//!                    24..27 result shift (ps_1_x), 31 one
//! source token       bits 0..10 register number, 11..12 and 28..30 register type, 13 relative addressing (one more
//!                    token follows in shader model 2+), 16..23 swizzle (2 bits per component), 24..27 source
//!                    modifier, 31 one
//! dcl                usage token (bits 0..4 usage, 16..19 usage index; samplers: bits 27..30 texture type), then a
//!                    destination token
//! def / defi / defb  destination token, then 4 floats / 4 integers / 1 boolean
//! end token          0x0000FFFF
//! ```
//! Only shader models 2 and 3 are accepted (shader model 1 packs its tokens differently and none of the game's
//! shaders use it).

use super::ctab::{self, ConstantTable};
use super::preshader::{self, Preshader};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ShaderKind {
    Pixel,
    Vertex,
}

macro_rules! opcodes {
    ($( $value:literal $variant:ident $name:literal $dst:literal $src:literal; )*) => {
        /// `D3DSHADER_INSTRUCTION_OPCODE_TYPE`
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
        pub enum Opcode { $( $variant, )* }
        impl Opcode {
            pub fn from_u16(v: u16) -> Option<Opcode> {
                match v { $( $value => Some(Opcode::$variant), )* _ => None }
            }
            /// assembler mnemonic
            pub fn name(self) -> &'static str {
                match self { $( Opcode::$variant => $name, )* }
            }
            /// (has a destination operand, number of source operands) in shader model 2 / 3
            fn shape(self) -> (bool, usize) {
                match self { $( Opcode::$variant => ($dst, $src), )* }
            }
            pub const ALL: &'static [Opcode] = &[ $( Opcode::$variant, )* ];
        }
    };
}

// value, variant, mnemonic, has destination, source count (shader model 2; `sincos` has one source in model 3)
opcodes! {
    0 Nop "nop" false 0;
    1 Mov "mov" true 1;
    2 Add "add" true 2;
    3 Sub "sub" true 2;
    4 Mad "mad" true 3;
    5 Mul "mul" true 2;
    6 Rcp "rcp" true 1;
    7 Rsq "rsq" true 1;
    8 Dp3 "dp3" true 2;
    9 Dp4 "dp4" true 2;
    10 Min "min" true 2;
    11 Max "max" true 2;
    12 Slt "slt" true 2;
    13 Sge "sge" true 2;
    14 Exp "exp" true 1;
    15 Log "log" true 1;
    16 Lit "lit" true 1;
    17 Dst "dst" true 2;
    18 Lrp "lrp" true 3;
    19 Frc "frc" true 1;
    20 M4x4 "m4x4" true 2;
    21 M4x3 "m4x3" true 2;
    22 M3x4 "m3x4" true 2;
    23 M3x3 "m3x3" true 2;
    24 M3x2 "m3x2" true 2;
    25 Call "call" false 1;
    26 CallNz "callnz" false 2;
    27 Loop "loop" false 2;
    28 Ret "ret" false 0;
    29 EndLoop "endloop" false 0;
    30 Label "label" false 1;
    31 Dcl "dcl" true 0;
    32 Pow "pow" true 2;
    33 Crs "crs" true 2;
    34 Sgn "sgn" true 3;
    35 Abs "abs" true 1;
    36 Nrm "nrm" true 1;
    37 SinCos "sincos" true 3;
    38 Rep "rep" false 1;
    39 EndRep "endrep" false 0;
    40 If "if" false 1;
    41 Ifc "if" false 2;
    42 Else "else" false 0;
    43 EndIf "endif" false 0;
    44 Break "break" false 0;
    45 BreakC "break" false 2;
    46 Mova "mova" true 1;
    47 DefB "defb" true 0;
    48 DefI "defi" true 0;
    65 TexKill "texkill" true 0;
    66 TexLd "texld" true 2;
    80 Cnd "cnd" true 3;
    81 Def "def" true 0;
    88 Cmp "cmp" true 3;
    89 Bem "bem" true 2;
    90 Dp2Add "dp2add" true 3;
    91 Dsx "dsx" true 1;
    92 Dsy "dsy" true 1;
    93 TexLdd "texldd" true 4;
    94 SetP "setp" true 2;
    95 TexLdl "texldl" true 2;
    96 BreakP "breakp" false 1;
}

/// `D3DSHADER_PARAM_REGISTER_TYPE`
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum RegType {
    /// `r#`
    Temp,
    /// `v#`: vertex shader input; pixel shader colour input
    Input,
    /// `c#`
    Const,
    /// vertex shader `a0`
    Addr,
    /// pixel shader `t#` (texture coordinate input)
    Texture,
    /// vertex shader `oPos` (0), `oFog` (1), `oPts` (2)
    RastOut,
    /// vertex shader `oD#`
    AttrOut,
    /// vertex shader `oT#` (shader model 2), `o#` (shader model 3)
    TexCrdOut,
    /// `i#`
    ConstInt,
    /// pixel shader `oC#`
    ColorOut,
    /// pixel shader `oDepth`
    DepthOut,
    /// `s#`
    Sampler,
    /// `b#`
    ConstBool,
    /// `aL`
    Loop,
    /// pixel shader `vPos` (0), `vFace` (1)
    MiscType,
    Label,
    /// `p0`
    Predicate,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Reg {
    pub ty: RegType,
    pub num: u32,
}

impl Reg {
    /// assembler spelling
    pub fn name(self) -> String {
        match self.ty {
            RegType::Temp => format!("r{}", self.num),
            RegType::Input => format!("v{}", self.num),
            RegType::Const => format!("c{}", self.num),
            RegType::Addr => format!("a{}", self.num),
            RegType::Texture => format!("t{}", self.num),
            RegType::RastOut => ["oPos", "oFog", "oPts"].get(self.num as usize).map(|s| (*s).to_owned()).unwrap_or_else(|| format!("oRast{}", self.num)),
            RegType::AttrOut => format!("oD{}", self.num),
            RegType::TexCrdOut => format!("oT{}", self.num),
            RegType::ConstInt => format!("i{}", self.num),
            RegType::ColorOut => format!("oC{}", self.num),
            RegType::DepthOut => "oDepth".to_owned(),
            RegType::Sampler => format!("s{}", self.num),
            RegType::ConstBool => format!("b{}", self.num),
            RegType::Loop => "aL".to_owned(),
            RegType::MiscType => ["vPos", "vFace"].get(self.num as usize).map(|s| (*s).to_owned()).unwrap_or_else(|| format!("vMisc{}", self.num)),
            RegType::Label => format!("l{}", self.num),
            RegType::Predicate => format!("p{}", self.num),
        }
    }
}

fn reg_of(token: u32, kind: ShaderKind) -> Result<Reg, String> {
    let ty = ((token >> 28) & 7) | ((token >> 8) & 0x18);
    let num = token & 0x7FF;
    let ty = match (ty, kind) {
        (0, _) => RegType::Temp,
        (1, _) => RegType::Input,
        (2, _) => RegType::Const,
        (3, ShaderKind::Vertex) => RegType::Addr,
        (3, ShaderKind::Pixel) => RegType::Texture,
        (4, _) => RegType::RastOut,
        (5, _) => RegType::AttrOut,
        (6, _) => RegType::TexCrdOut,
        (7, _) => RegType::ConstInt,
        (8, _) => RegType::ColorOut,
        (9, _) => RegType::DepthOut,
        (10, _) => RegType::Sampler,
        // CONST2 / CONST3 / CONST4: float constants 2048.., 4096.., 6144..
        (11, _) => return Ok(Reg { ty: RegType::Const, num: num + 2048 }),
        (12, _) => return Ok(Reg { ty: RegType::Const, num: num + 4096 }),
        (13, _) => return Ok(Reg { ty: RegType::Const, num: num + 6144 }),
        (14, _) => RegType::ConstBool,
        (15, _) => RegType::Loop,
        (17, _) => RegType::MiscType,
        (18, _) => RegType::Label,
        (19, _) => RegType::Predicate,
        (other, _) => return Err(format!("unknown register type {other}")),
    };
    Ok(Reg { ty, num })
}

/// Write mask bits: x 1, y 2, z 4, w 8.
pub const MASK_ALL: u8 = 15;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Dst {
    pub reg: Reg,
    pub mask: u8,
    pub saturate: bool,
    pub partial_precision: bool,
    pub centroid: bool,
}

/// `D3DSHADER_PARAM_SRCMOD_TYPE`
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SrcMod {
    None,
    Negate,
    Bias,
    BiasNegate,
    Sign,
    SignNegate,
    Complement,
    X2,
    X2Negate,
    Dz,
    Dw,
    Abs,
    AbsNegate,
    Not,
}

impl SrcMod {
    pub fn name(self) -> &'static str {
        match self {
            SrcMod::None => "none",
            SrcMod::Negate => "negate",
            SrcMod::Bias => "bias",
            SrcMod::BiasNegate => "bias negate",
            SrcMod::Sign => "sign (bx2)",
            SrcMod::SignNegate => "sign negate",
            SrcMod::Complement => "complement",
            SrcMod::X2 => "x2",
            SrcMod::X2Negate => "x2 negate",
            SrcMod::Dz => "dz",
            SrcMod::Dw => "dw",
            SrcMod::Abs => "abs",
            SrcMod::AbsNegate => "abs negate",
            SrcMod::Not => "not",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Src {
    pub reg: Reg,
    /// source component (0 x .. 3 w) for each of the four result components
    pub swizzle: [u8; 4],
    pub modifier: SrcMod,
    /// relative addressing (`c[a0.x + n]`): the address register and its component
    pub relative: Option<(Reg, u8)>,
}

pub const SWIZZLE_IDENTITY: [u8; 4] = [0, 1, 2, 3];

/// `D3DDECLUSAGE`
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Usage {
    Position,
    BlendWeight,
    BlendIndices,
    Normal,
    PSize,
    TexCoord,
    Tangent,
    Binormal,
    TessFactor,
    PositionT,
    Color,
    Fog,
    Depth,
    Sample,
}

impl Usage {
    pub fn name(self) -> &'static str {
        match self {
            Usage::Position => "position",
            Usage::BlendWeight => "blendweight",
            Usage::BlendIndices => "blendindices",
            Usage::Normal => "normal",
            Usage::PSize => "psize",
            Usage::TexCoord => "texcoord",
            Usage::Tangent => "tangent",
            Usage::Binormal => "binormal",
            Usage::TessFactor => "tessfactor",
            Usage::PositionT => "positiont",
            Usage::Color => "color",
            Usage::Fog => "fog",
            Usage::Depth => "depth",
            Usage::Sample => "sample",
        }
    }
}

/// `D3DSAMPLER_TEXTURE_TYPE`
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SamplerType {
    D2,
    Cube,
    Volume,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Extra {
    None,
    /// `dcl_usage# v#` (vertex shader inputs; shader model 3 inputs and outputs)
    DclUsage { usage: Usage, index: u32 },
    /// `dcl_2d s#`
    DclSampler(SamplerType),
    /// `dcl t#` / `dcl v#` in a shader model 2 pixel shader (no usage)
    DclInput,
    DefFloat([f32; 4]),
    DefInt([i32; 4]),
    DefBool(bool),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Instruction {
    pub opcode: Opcode,
    /// bits 16..23 of the instruction token: `texld` 1 projected / 2 bias; comparison for `ifc`, `breakc`, `setp`
    pub control: u8,
    pub dst: Option<Dst>,
    pub srcs: Vec<Src>,
    pub extra: Extra,
    pub predicated: bool,
    pub coissue: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Shader {
    pub kind: ShaderKind,
    pub major: u8,
    pub minor: u8,
    pub instructions: Vec<Instruction>,
    /// the shader's own constant table (`CTAB` comment)
    pub ctab: Option<ConstantTable>,
    /// the D3DX preshader (`PRES` comment): computes some of the float constant registers from effect parameters
    pub preshader: Option<Preshader>,
    /// FOURCCs of comments that are neither `CTAB` nor `PRES`
    pub other_comments: Vec<[u8; 4]>,
}

impl Shader {
    /// `ps_2_0`, `vs_3_0`
    pub fn model(&self) -> String {
        format!("{}_{}_{}", if self.kind == ShaderKind::Pixel { "ps" } else { "vs" }, self.major, self.minor)
    }
    /// Instructions that execute (no `dcl`, `def*`, `nop`): what fxc's listing counts as "instruction slots" up to
    /// multi-slot instructions.
    pub fn executable(&self) -> impl Iterator<Item = &Instruction> {
        self.instructions.iter().filter(|i| !matches!(i.opcode, Opcode::Dcl | Opcode::Def | Opcode::DefI | Opcode::DefB | Opcode::Nop))
    }
    /// `def c#` constants: (register, value)
    pub fn float_defs(&self) -> impl Iterator<Item = (u32, [f32; 4])> + '_ {
        self.instructions.iter().filter_map(|i| match (i.opcode, i.dst, i.extra) {
            (Opcode::Def, Some(d), Extra::DefFloat(v)) => Some((d.reg.num, v)),
            _ => None,
        })
    }
    /// `dcl` instructions: (destination, what is declared)
    pub fn declarations(&self) -> impl Iterator<Item = (Dst, Extra)> + '_ {
        self.instructions.iter().filter(|i| i.opcode == Opcode::Dcl).filter_map(|i| i.dst.map(|d| (d, i.extra)))
    }
}

fn dst_of(token: u32, kind: ShaderKind) -> Result<Dst, String> {
    if token & 0x8000_0000 == 0 {
        return Err(format!("destination token {token:08X} without the parameter bit"));
    }
    if token & 0x2000 != 0 {
        return Err("relative addressing in a destination register is not supported".into());
    }
    if (token >> 24) & 0xF != 0 {
        return Err(format!("result shift {} (a ps_1_x feature) is not supported", (token >> 24) & 0xF));
    }
    let modifier = (token >> 20) & 0xF;
    if modifier & !7 != 0 {
        return Err(format!("unknown result modifier bits {modifier:#x}"));
    }
    Ok(Dst { reg: reg_of(token, kind)?, mask: ((token >> 16) & 0xF) as u8, saturate: modifier & 1 != 0, partial_precision: modifier & 2 != 0, centroid: modifier & 4 != 0 })
}

fn src_of(token: u32, kind: ShaderKind) -> Result<Src, String> {
    if token & 0x8000_0000 == 0 {
        return Err(format!("source token {token:08X} without the parameter bit"));
    }
    let modifier = match (token >> 24) & 0xF {
        0 => SrcMod::None,
        1 => SrcMod::Negate,
        2 => SrcMod::Bias,
        3 => SrcMod::BiasNegate,
        4 => SrcMod::Sign,
        5 => SrcMod::SignNegate,
        6 => SrcMod::Complement,
        7 => SrcMod::X2,
        8 => SrcMod::X2Negate,
        9 => SrcMod::Dz,
        10 => SrcMod::Dw,
        11 => SrcMod::Abs,
        12 => SrcMod::AbsNegate,
        13 => SrcMod::Not,
        other => return Err(format!("unknown source modifier {other}")),
    };
    let s = (token >> 16) & 0xFF;
    Ok(Src { reg: reg_of(token, kind)?, swizzle: [(s & 3) as u8, ((s >> 2) & 3) as u8, ((s >> 4) & 3) as u8, ((s >> 6) & 3) as u8], modifier, relative: None })
}

/// Decode a compiled shader. Errors name what is not understood; nothing is skipped silently.
pub fn parse(bytes: &[u8]) -> Result<Shader, String> {
    if bytes.len() % 4 != 0 || bytes.len() < 8 {
        return Err(format!("shader bytecode of {} bytes is not a token stream", bytes.len()));
    }
    let tokens: Vec<u32> = bytes.chunks_exact(4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]])).collect();
    let version = tokens[0];
    let kind = match version >> 16 {
        0xFFFF => ShaderKind::Pixel,
        0xFFFE => ShaderKind::Vertex,
        _ => return Err(format!("not a shader version token: {version:08X}")),
    };
    let (major, minor) = (((version >> 8) & 0xFF) as u8, (version & 0xFF) as u8);
    if !(2..=3).contains(&major) {
        return Err(format!("shader model {major}.{minor} is not supported (only 2 and 3)"));
    }
    let mut shader = Shader { kind, major, minor, instructions: Vec::new(), ctab: None, preshader: None, other_comments: Vec::new() };
    let mut i = 1;
    loop {
        let Some(&token) = tokens.get(i) else { return Err("no end token".into()) };
        if token == 0x0000_FFFF {
            if i + 1 != tokens.len() {
                return Err(format!("{} tokens after the end token", tokens.len() - i - 1));
            }
            break;
        }
        if token & 0xFFFF == 0xFFFE {
            let len = ((token >> 16) & 0x7FFF) as usize;
            let body = tokens.get(i + 1..i + 1 + len).ok_or("comment runs past the end of the shader")?;
            if let Some(&fourcc) = body.first() {
                let data: Vec<u8> = body[1..].iter().flat_map(|t| t.to_le_bytes()).collect();
                match &fourcc.to_le_bytes() {
                    b"CTAB" => {
                        if shader.ctab.is_some() {
                            return Err("more than one CTAB comment".into());
                        }
                        shader.ctab = Some(ctab::parse(&data)?);
                    }
                    b"PRES" => {
                        if shader.preshader.is_some() {
                            return Err("more than one PRES comment".into());
                        }
                        shader.preshader = Some(preshader::parse(&body[1..])?);
                    }
                    other => shader.other_comments.push(*other),
                }
            }
            i += 1 + len;
            continue;
        }
        if token & 0x8000_0000 != 0 {
            return Err(format!("token {i}: {token:08X} is not an instruction token"));
        }
        let code = (token & 0xFFFF) as u16;
        let opcode = Opcode::from_u16(code).ok_or_else(|| format!("token {i}: unknown opcode {code}"))?;
        let len = ((token >> 24) & 0xF) as usize;
        let args = tokens.get(i + 1..i + 1 + len).ok_or_else(|| format!("{}: instruction runs past the end of the shader", opcode.name()))?;
        let mut ins = Instruction { opcode, control: ((token >> 16) & 0xFF) as u8, dst: None, srcs: Vec::new(), extra: Extra::None, predicated: token & 0x1000_0000 != 0, coissue: token & 0x4000_0000 != 0 };
        if ins.predicated {
            return Err(format!("{}: predicated instructions are not supported", opcode.name()));
        }
        let wrong_length = || format!("{}: {len} argument tokens do not match the instruction", opcode.name());
        match opcode {
            Opcode::Dcl => {
                let [usage, dst] = args else { return Err(wrong_length()) };
                let d = dst_of(*dst, kind)?;
                ins.extra = if d.reg.ty == RegType::Sampler {
                    Extra::DclSampler(match (usage >> 27) & 0xF {
                        2 => SamplerType::D2,
                        3 => SamplerType::Cube,
                        4 => SamplerType::Volume,
                        other => return Err(format!("dcl: unknown sampler texture type {other}")),
                    })
                } else if kind == ShaderKind::Pixel && major < 3 {
                    Extra::DclInput
                } else {
                    let u = match usage & 0x1F {
                        0 => Usage::Position,
                        1 => Usage::BlendWeight,
                        2 => Usage::BlendIndices,
                        3 => Usage::Normal,
                        4 => Usage::PSize,
                        5 => Usage::TexCoord,
                        6 => Usage::Tangent,
                        7 => Usage::Binormal,
                        8 => Usage::TessFactor,
                        9 => Usage::PositionT,
                        10 => Usage::Color,
                        11 => Usage::Fog,
                        12 => Usage::Depth,
                        13 => Usage::Sample,
                        other => return Err(format!("dcl: unknown usage {other}")),
                    };
                    Extra::DclUsage { usage: u, index: (usage >> 16) & 0xF }
                };
                ins.dst = Some(d);
            }
            Opcode::Def => {
                let [dst, a, b, c, d] = args else { return Err(wrong_length()) };
                ins.dst = Some(dst_of(*dst, kind)?);
                ins.extra = Extra::DefFloat([f32::from_bits(*a), f32::from_bits(*b), f32::from_bits(*c), f32::from_bits(*d)]);
            }
            Opcode::DefI => {
                let [dst, a, b, c, d] = args else { return Err(wrong_length()) };
                ins.dst = Some(dst_of(*dst, kind)?);
                ins.extra = Extra::DefInt([*a as i32, *b as i32, *c as i32, *d as i32]);
            }
            Opcode::DefB => {
                let [dst, v] = args else { return Err(wrong_length()) };
                ins.dst = Some(dst_of(*dst, kind)?);
                ins.extra = Extra::DefBool(*v != 0);
            }
            _ => {
                let (has_dst, mut sources) = opcode.shape();
                if opcode == Opcode::SinCos && major >= 3 {
                    sources = 1;
                }
                let mut k = 0;
                if has_dst {
                    ins.dst = Some(dst_of(*args.get(k).ok_or_else(wrong_length)?, kind)?);
                    k += 1;
                }
                for _ in 0..sources {
                    let t = *args.get(k).ok_or_else(wrong_length)?;
                    k += 1;
                    let mut s = src_of(t, kind)?;
                    if t & 0x2000 != 0 {
                        // shader model 2+: the address register is an explicit source token
                        let a = src_of(*args.get(k).ok_or_else(wrong_length)?, kind)?;
                        k += 1;
                        s.relative = Some((a.reg, a.swizzle[0]));
                    }
                    ins.srcs.push(s);
                }
                if k != len {
                    return Err(wrong_length());
                }
            }
        }
        shader.instructions.push(ins);
        i += 1 + len;
    }
    Ok(shader)
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;

    /// `ps_2_0; dcl t0.xy; dcl_2d s0; texld r0, t0, s0; mov oC0, r0` written out by hand from the token layout.
    pub(crate) fn tiny_ps() -> Vec<u8> {
        let t: [u32; 14] = [
            0xFFFF_0200,
            0x0200_001F, 0x8000_0000, 0xB003_0000, // dcl t0.xy
            0x0200_001F, 0x9000_0000, 0xA00F_0800, // dcl_2d s0
            0x0300_0042, 0x800F_0000, 0xB0E4_0000, 0xA0E4_0800, // texld r0, t0, s0
            0x0200_0001, 0x800F_0800, // mov oC0, ... (completed below)
            0x80E4_0000,
        ];
        let mut bytes: Vec<u8> = t.iter().flat_map(|w| w.to_le_bytes()).collect();
        bytes.extend_from_slice(&0x0000_FFFFu32.to_le_bytes());
        bytes
    }

    #[test]
    fn decodes_registers_masks_and_swizzles() {
        let s = parse(&tiny_ps()).unwrap();
        assert_eq!((s.kind, s.major, s.minor), (ShaderKind::Pixel, 2, 0));
        assert_eq!(s.model(), "ps_2_0");
        assert_eq!(s.instructions.len(), 4);
        let dcl = &s.instructions[0];
        assert_eq!((dcl.opcode, dcl.dst.unwrap().reg, dcl.dst.unwrap().mask, dcl.extra), (Opcode::Dcl, Reg { ty: RegType::Texture, num: 0 }, 3, Extra::DclInput));
        assert_eq!(s.instructions[1].extra, Extra::DclSampler(SamplerType::D2));
        let tex = &s.instructions[2];
        assert_eq!(tex.opcode, Opcode::TexLd);
        assert_eq!(tex.srcs[1].reg, Reg { ty: RegType::Sampler, num: 0 });
        assert_eq!(tex.srcs[0].swizzle, SWIZZLE_IDENTITY);
        let mov = &s.instructions[3];
        assert_eq!(mov.dst.unwrap().reg, Reg { ty: RegType::ColorOut, num: 0 });
        assert_eq!(s.executable().count(), 2);
    }

    #[test]
    fn refuses_what_it_does_not_understand() {
        assert!(parse(&[]).is_err());
        // ps_1_1
        assert!(parse(&[0x01, 0x01, 0xFF, 0xFF, 0xFF, 0xFF, 0, 0]).unwrap_err().contains("shader model 1.1"));
        // unknown opcode 200 in a ps_2_0
        let mut b = 0xFFFF_0200u32.to_le_bytes().to_vec();
        b.extend_from_slice(&200u32.to_le_bytes());
        b.extend_from_slice(&0x0000_FFFFu32.to_le_bytes());
        assert!(parse(&b).unwrap_err().contains("unknown opcode 200"));
        // truncated: no end token
        assert!(parse(&0xFFFF_0200u32.to_le_bytes().repeat(2)).is_err());
    }
}
