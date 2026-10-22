//! The D3DX preshader of an effect's shader (the `PRES` comment inside the shader bytecode).
//!
//! The HLSL compiler moves expressions that depend only on uniforms out of the shader into a small program that
//! D3DX runs on the CPU when the pass is applied; its results are written into the shader's float constant
//! registers. Without it those registers stay empty and the shader computes nonsense, so it is not optional.
//!
//! The format is not documented by Microsoft. This follows Wine's `dlls/d3dx9_36/preshader.c` (whose operation
//! semantics are checked against native D3DX by Wine's tests) and MojoShader's `parse_preshader`:
//! ```text
//! PRES comment body   u32 version (0x4658 0200 / 0201), then comments: CTAB, PRSI, CLIT, FXLC
//! CTAB                a constant table: the effect parameters the program reads, in ITS OWN float registers
//! CLIT                u32 count, count x f64            literals
//! FXLC                u32 instructions, then per instruction
//!                       u32 opcode (bits 20..30) | scalar flag (bit 31) | component count (bits 0..15)
//!                       u32 number of inputs
//!                       inputs, then the output; each operand:
//!                         u32 0, or u32 1 + (u32 table, u32 offset) of an index register (arrays)
//!                         u32 table (1 literals, 2 inputs, 4 float constants of the shader, 7 temporaries)
//!                         u32 offset in components (register * 4 + component for tables 2 and 4)
//! PRSI                register counts and the list of output register ranges (informative)
//! ```
//! Where the two references disagree, Wine's behaviour is used because it is tested against the native library:
//! `exp` is 2^x and `log` is log2(|x|) (MojoShader: natural), `rsq` takes the absolute value, `dot` writes one
//! component, `div` yields 0 (native never emits it). A scalar instruction (bit 31) reads component 0 of its
//! first input for every component.

use super::ctab::{self, ConstantTable, RegisterSet};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum PresOp {
    Mov,
    Neg,
    Rcp,
    Frc,
    Exp,
    Log,
    Rsq,
    Sin,
    Cos,
    Asin,
    Acos,
    Atan,
    Min,
    Max,
    Lt,
    Ge,
    Add,
    Mul,
    Atan2,
    Div,
    Cmp,
    Dot,
}

impl PresOp {
    /// (opcode value, input count) -> operation
    fn decode(code: u32, inputs: u32) -> Option<PresOp> {
        use PresOp::*;
        Some(match (code, inputs) {
            (0x100, 1) => Mov,
            (0x101, 1) => Neg,
            (0x103, 1) => Rcp,
            (0x104, 1) => Frc,
            (0x105, 1) => Exp,
            (0x106, 1) => Log,
            (0x107, 1) => Rsq,
            (0x108, 1) => Sin,
            (0x109, 1) => Cos,
            (0x10A, 1) => Asin,
            (0x10B, 1) => Acos,
            (0x10C, 1) => Atan,
            (0x200, 2) => Min,
            (0x201, 2) => Max,
            (0x202, 2) => Lt,
            (0x203, 2) => Ge,
            (0x204, 2) => Add,
            (0x205, 2) => Mul,
            (0x206, 2) => Atan2,
            (0x208, 2) => Div,
            (0x300, 3) => Cmp,
            (0x500, 2) => Dot,
            _ => return None,
        })
    }
    pub fn name(self) -> &'static str {
        use PresOp::*;
        match self {
            Mov => "mov",
            Neg => "neg",
            Rcp => "rcp",
            Frc => "frc",
            Exp => "exp",
            Log => "log",
            Rsq => "rsq",
            Sin => "sin",
            Cos => "cos",
            Asin => "asin",
            Acos => "acos",
            Atan => "atan",
            Min => "min",
            Max => "max",
            Lt => "lt",
            Ge => "ge",
            Add => "add",
            Mul => "mul",
            Atan2 => "atan2",
            Div => "div",
            Cmp => "cmp",
            Dot => "dot",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Table {
    /// the `CLIT` literals
    Literal,
    /// the preshader's own float registers, filled from effect parameters through its constant table
    Input,
    /// the shader's float constant registers
    Output,
    Temp,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Operand {
    pub table: Table,
    /// in components (`register * 4 + component` for inputs and outputs)
    pub offset: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PresInstruction {
    pub op: PresOp,
    /// the first input is one scalar used for every component
    pub scalar: bool,
    pub components: u32,
    pub inputs: Vec<Operand>,
    pub output: Operand,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Preshader {
    pub version: u32,
    /// the effect parameters the program reads (float registers of the preshader itself)
    pub ctab: ConstantTable,
    pub literals: Vec<f64>,
    pub instructions: Vec<PresInstruction>,
    /// number of temporary components
    pub temp_count: u32,
    /// number of input components (4 per register of `ctab`)
    pub input_count: u32,
    /// `PRSI`: (first register, register count) of the shader constants the program writes
    pub output_ranges: Vec<(u32, u32)>,
}

impl Preshader {
    /// One past the highest shader float register the program writes.
    pub fn output_register_end(&self) -> u32 {
        self.instructions.iter().filter(|i| i.output.table == Table::Output).map(|i| (i.output.offset + if i.op == PresOp::Dot { 0 } else { i.components - 1 }) / 4 + 1).max().unwrap_or(0)
    }

    /// The shader float registers (register, component mask) the program writes.
    pub fn outputs(&self) -> Vec<(u32, u8)> {
        let mut out: Vec<(u32, u8)> = Vec::new();
        for i in &self.instructions {
            if i.output.table != Table::Output {
                continue;
            }
            let n = if i.op == PresOp::Dot { 1 } else { i.components };
            for k in 0..n {
                let (reg, comp) = ((i.output.offset + k) / 4, (i.output.offset + k) % 4);
                match out.iter_mut().find(|(r, _)| *r == reg) {
                    Some((_, m)) => *m |= 1 << comp,
                    None => out.push((reg, 1 << comp)),
                }
            }
        }
        out.sort_unstable();
        out
    }

    /// Run the program. `inputs`: the preshader's input registers (4 floats each, filled through `self.ctab`);
    /// `outputs`: the shader's float constant registers.
    pub fn run(&self, inputs: &[f32], outputs: &mut [[f32; 4]]) -> Result<(), String> {
        let mut temps = vec![0f64; self.temp_count as usize];
        let register_count = outputs.len();
        for ins in &self.instructions {
            let read = |temps: &[f64], outputs: &[[f32; 4]], o: &Operand, comp: u32| -> Result<f64, String> {
                let at = (o.offset + comp) as usize;
                let missing = || format!("preshader: operand {at} outside its table");
                Ok(match o.table {
                    Table::Literal => *self.literals.get(at).ok_or_else(missing)?,
                    Table::Input => *inputs.get(at).ok_or_else(missing)? as f64,
                    Table::Output => outputs.get(at / 4).ok_or_else(missing)?[at % 4] as f64,
                    Table::Temp => *temps.get(at).ok_or_else(missing)?,
                })
            };
            let n = ins.components;
            let mut results = [0f64; 4];
            let count = if ins.op == PresOp::Dot {
                let mut sum = 0.0;
                for j in 0..n {
                    sum += read(&temps, outputs, &ins.inputs[0], if ins.scalar { 0 } else { j })? * read(&temps, outputs, &ins.inputs[1], j)?;
                }
                results[0] = sum;
                1
            } else {
                for j in 0..n {
                    let mut a = [0f64; 3];
                    for (k, input) in ins.inputs.iter().enumerate() {
                        a[k] = read(&temps, outputs, input, if ins.scalar && k == 0 { 0 } else { j })?;
                    }
                    results[j as usize] = match ins.op {
                        PresOp::Mov => a[0],
                        PresOp::Neg => -a[0],
                        PresOp::Rcp => 1.0 / a[0],
                        PresOp::Frc => a[0] - a[0].floor(),
                        PresOp::Exp => 2f64.powf(a[0]),
                        PresOp::Log => {
                            let v = a[0].abs();
                            if v == 0.0 {
                                0.0
                            } else {
                                v.log2()
                            }
                        }
                        PresOp::Rsq => {
                            let v = a[0].abs();
                            if v == 0.0 {
                                f64::INFINITY
                            } else {
                                1.0 / v.sqrt()
                            }
                        }
                        PresOp::Sin => a[0].sin(),
                        PresOp::Cos => a[0].cos(),
                        PresOp::Asin => a[0].asin(),
                        PresOp::Acos => a[0].acos(),
                        PresOp::Atan => a[0].atan(),
                        // C fmin / fmax: a NaN operand yields the other one, as f64::min / max do
                        PresOp::Min => a[0].min(a[1]),
                        PresOp::Max => a[0].max(a[1]),
                        PresOp::Lt => (a[0] < a[1]) as u8 as f64,
                        PresOp::Ge => (a[0] >= a[1]) as u8 as f64,
                        PresOp::Add => a[0] + a[1],
                        PresOp::Mul => a[0] * a[1],
                        PresOp::Atan2 => a[0].atan2(a[1]),
                        PresOp::Div => 0.0,
                        PresOp::Cmp => {
                            if a[0] >= 0.0 {
                                a[1]
                            } else {
                                a[2]
                            }
                        }
                        PresOp::Dot => unreachable!(),
                    };
                }
                n
            };
            for j in 0..count {
                let at = (ins.output.offset + j) as usize;
                match ins.output.table {
                    Table::Temp => *temps.get_mut(at).ok_or("preshader: temporary outside its table")? = results[j as usize],
                    Table::Output => outputs.get_mut(at / 4).ok_or_else(|| format!("preshader: output register c{} outside the {register_count} registers given", at / 4))?[at % 4] = results[j as usize] as f32,
                    _ => return Err("preshader: an instruction writes to a read-only table".into()),
                }
            }
        }
        Ok(())
    }
}

/// Parse the body of a `PRES` comment (the tokens after the FOURCC).
pub fn parse(tokens: &[u32]) -> Result<Preshader, String> {
    let version = *tokens.first().ok_or("preshader: empty")?;
    if !(0x4658_0200..=0x4658_0201).contains(&version) {
        return Err(format!("preshader: unsupported version {version:08X}"));
    }
    let (mut ctab_data, mut clit, mut fxlc, mut prsi): (Option<Vec<u8>>, Option<&[u32]>, Option<&[u32]>, Option<&[u32]>) = (None, None, None, None);
    let mut i = 1;
    while i < tokens.len() {
        let t = tokens[i];
        if t == 0x0000_FFFF {
            break;
        }
        if t & 0xFFFF != 0xFFFE {
            return Err(format!("preshader: token {t:08X} is not a comment"));
        }
        let len = ((t >> 16) & 0x7FFF) as usize;
        let body = tokens.get(i + 1..i + 1 + len).ok_or("preshader: section runs past the end")?;
        if let Some(fourcc) = body.first() {
            match &fourcc.to_le_bytes() {
                b"CTAB" => ctab_data = Some(body[1..].iter().flat_map(|t| t.to_le_bytes()).collect()),
                b"CLIT" => clit = Some(&body[1..]),
                b"FXLC" => fxlc = Some(&body[1..]),
                b"PRSI" => prsi = Some(&body[1..]),
                other => return Err(format!("preshader: unknown section {:?}", String::from_utf8_lossy(other))),
            }
        }
        i += 1 + len;
    }
    let ctab = match ctab_data {
        Some(d) => ctab::parse(&d)?,
        None => ConstantTable::default(),
    };
    for c in &ctab.constants {
        if c.register_set != RegisterSet::Float4 {
            return Err(format!("preshader: input {} is in the {:?} register set (only float inputs are supported)", c.name, c.register_set));
        }
    }
    let input_count = ctab.constants.iter().map(|c| (c.register_index + c.register_count) * 4).max().unwrap_or(0);

    let mut literals = Vec::new();
    if let Some(c) = clit {
        let n = *c.first().ok_or("preshader: empty CLIT")? as usize;
        let words = c.get(1..1 + n * 2).ok_or("preshader: CLIT shorter than its count")?;
        for w in words.chunks_exact(2) {
            literals.push(f64::from_bits(w[0] as u64 | (w[1] as u64) << 32));
        }
    }

    let mut output_ranges = Vec::new();
    if let Some(p) = prsi {
        // MojoShader: word 7 is the number of (first register, count) pairs that follow word 8
        if let Some(&n) = p.get(6) {
            for k in 0..n as usize {
                if let (Some(a), Some(b)) = (p.get(7 + k * 2), p.get(8 + k * 2)) {
                    output_ranges.push((*a, *b));
                }
            }
        }
    }

    let fxlc = fxlc.ok_or("preshader: no FXLC section")?;
    let count = *fxlc.first().ok_or("preshader: empty FXLC")?;
    let mut p = 1usize;
    let mut next = |what: &str| -> Result<u32, String> {
        let v = *fxlc.get(p).ok_or_else(|| format!("preshader: FXLC ends inside {what}"))?;
        p += 1;
        Ok(v)
    };
    let mut instructions = Vec::new();
    let mut temp_count = 0u32;
    for n in 0..count {
        let raw = next("an instruction")?;
        let inputs = next("an instruction")?;
        let code = (raw & 0x7FF0_0000) >> 20;
        let components = raw & 0xFFFF;
        let scalar = raw & 0x8000_0000 != 0;
        let op = PresOp::decode(code, inputs).ok_or_else(|| format!("preshader: instruction {n}: unknown opcode {code:#x} with {inputs} inputs"))?;
        if !(1..=4).contains(&components) {
            return Err(format!("preshader: instruction {n} ({}): {components} components", op.name()));
        }
        let mut operands = Vec::new();
        for k in 0..=inputs {
            if next("an operand")? != 0 {
                return Err(format!("preshader: instruction {n} ({}): array-indexed operands are not supported", op.name()));
            }
            let table = match next("an operand")? {
                1 => Table::Literal,
                2 => Table::Input,
                4 => Table::Output,
                7 => Table::Temp,
                other => return Err(format!("preshader: instruction {n} ({}): unsupported register table {other}", op.name())),
            };
            let offset = next("an operand")?;
            let is_output = k == inputs;
            let width = if is_output {
                if op == PresOp::Dot {
                    1
                } else {
                    components
                }
            } else if scalar && k == 0 {
                1
            } else {
                components
            };
            let limit = match table {
                Table::Literal => literals.len() as u32,
                Table::Input => input_count,
                Table::Output => u32::MAX,
                Table::Temp => {
                    if is_output {
                        temp_count = temp_count.max(offset + width);
                    }
                    u32::MAX
                }
            };
            if offset.checked_add(width).is_none_or(|end| end > limit) {
                return Err(format!("preshader: instruction {n} ({}): operand {offset}+{width} outside its table of {limit}", op.name()));
            }
            if is_output && !matches!(table, Table::Output | Table::Temp) {
                return Err(format!("preshader: instruction {n} ({}) writes to a read-only table", op.name()));
            }
            operands.push(Operand { table, offset });
        }
        let output = operands.pop().expect("at least the output");
        instructions.push(PresInstruction { op, scalar, components, inputs: operands, output });
    }
    for (n, ins) in instructions.iter().enumerate() {
        for (k, o) in ins.inputs.iter().enumerate() {
            let width = if ins.scalar && k == 0 { 1 } else { ins.components };
            if o.table == Table::Temp && o.offset + width > temp_count {
                return Err(format!("preshader: instruction {n} reads temporary {} which is never written", o.offset));
            }
        }
    }
    Ok(Preshader { version, ctab, literals, instructions, temp_count, input_count, output_ranges })
}

fn components(offset: u32, n: u32) -> String {
    (0..n).map(|k| ['x', 'y', 'z', 'w'][((offset + k) % 4) as usize]).collect()
}

/// Number formatting shared by the listings: integers without a fraction, everything else shortest round trip.
pub fn fmt_f64(v: f64) -> String {
    if v == v.trunc() && v.abs() < 1e15 {
        format!("{}", v as i64)
    } else {
        format!("{v}")
    }
}

impl PresInstruction {
    /// One line of assembly: `mul oc3.xy, c0.xy, (0.5, 2)`. `c#` are the preshader's inputs, `oc#` the shader's
    /// float constants it writes, `r#` temporaries (4 components each), `(..)` literals.
    pub fn text(&self, literals: &[f64]) -> String {
        let operand = |o: &Operand, n: u32| match o.table {
            Table::Literal => format!("({})", (0..n).map(|k| literals.get((o.offset + k) as usize).map(|v| fmt_f64(*v)).unwrap_or_else(|| "?".into())).collect::<Vec<_>>().join(", ")),
            Table::Input => format!("c{}.{}", o.offset / 4, components(o.offset, n)),
            Table::Output => format!("oc{}.{}", o.offset / 4, components(o.offset, n)),
            Table::Temp => format!("r{}.{}", o.offset / 4, components(o.offset, n)),
        };
        let mut parts = vec![operand(&self.output, if self.op == PresOp::Dot { 1 } else { self.components })];
        for (k, i) in self.inputs.iter().enumerate() {
            parts.push(operand(i, if self.scalar && k == 0 { 1 } else { self.components }));
        }
        format!("{} {}", self.op.name(), parts.join(", "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn section(fourcc: &[u8; 4], body: &[u32]) -> Vec<u32> {
        let mut v = vec![0xFFFE | ((body.len() as u32 + 1) << 16), u32::from_le_bytes(*fourcc)];
        v.extend_from_slice(body);
        v
    }

    /// A hand-made program: `r0.xy = (2, 3) * (2, 3)`; `oc1.zw = r0.xy + literal[2] (scalar)`; `oc2.x = dot(r0.xy,
    /// (2, 3))`.
    #[test]
    fn runs_a_hand_made_program() {
        let lit = |v: f64| [v.to_bits() as u32, (v.to_bits() >> 32) as u32];
        let mut clit = vec![4];
        for v in [2.0, 3.0, 10.0, 0.0] {
            clit.extend_from_slice(&lit(v));
        }
        let fxlc = [
            3, // instructions
            0x2050_0002, 2, 0, 1, 0, 0, 1, 0, 0, 7, 0, // mul r0.xy, (2, 3), (2, 3)
            0xA040_0002, 2, 0, 1, 2, 0, 7, 0, 0, 4, 6, // add (scalar) oc1.zw, (10), r0.xy
            0x5000_0002, 2, 0, 7, 0, 0, 1, 0, 0, 4, 8, // dot oc2.x, r0.xy, (2, 3)
        ];
        let mut tokens = vec![0x4658_0200];
        tokens.extend(section(b"CLIT", &clit));
        tokens.extend(section(b"FXLC", &fxlc));
        let p = parse(&tokens).unwrap();
        assert_eq!(p.instructions.len(), 3);
        assert_eq!(p.instructions[1].text(&p.literals), "add oc1.zw, (10), r0.xy");
        assert_eq!(p.outputs(), vec![(1, 0b1100), (2, 0b0001)]);
        assert_eq!(p.output_register_end(), 3);
        let mut out = [[0f32; 4]; 3];
        p.run(&[], &mut out).unwrap();
        assert_eq!(out[1], [0.0, 0.0, 14.0, 19.0]);
        assert_eq!(out[2], [8.0 + 27.0, 0.0, 0.0, 0.0]);
    }

    #[test]
    fn refuses_unknown_opcodes_and_tables() {
        let mut tokens = vec![0x4658_0200];
        tokens.extend(section(b"FXLC", &[1, 0x7770_0001, 1, 0, 7, 0, 0, 7, 0]));
        assert!(parse(&tokens).unwrap_err().contains("unknown opcode"));
        let mut tokens = vec![0x4658_0200];
        tokens.extend(section(b"FXLC", &[1, 0x1000_0001, 1, 0, 3, 0, 0, 7, 0]));
        assert!(parse(&tokens).unwrap_err().contains("register table 3"));
    }
}
