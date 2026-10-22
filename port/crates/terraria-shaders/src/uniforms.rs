//! Effect parameter values -> float constant registers of one shader.
//!
//! What D3DX does when an effect pass is applied (Wine `d3dx9_36/preshader.c` `set_constants`, MojoShader
//! `MOJOSHADER_effectCommitChanges`):
//! 1. every constant of the shader's constant table takes the value of the effect parameter with the same name,
//!    packed into its registers (below);
//! 2. the preshader, if the shader has one, gets ITS constant table filled the same way into its own input
//!    registers, runs, and writes more of the shader's registers.
//!
//! Packing of a value into float registers (4 floats each), by the class the CONSTANT TABLE gives the constant:
//! - scalar / vector: one register per array element, components from x; unused components are 0;
//! - `MatrixColumns` (HLSL's default `float4x4`): one register per COLUMN, so register k holds (M1k, M2k, M3k,
//!   M4k) of the matrix as XNA's `Matrix` names its fields; `dp4 oPos.x, v0, c0` then is the row vector times M;
//! - `MatrixRows` (`row_major`): one register per row;
//! - `register_count` limits how many registers are written (the compiler drops unused trailing ones);
//! - `bool` / `int` parameters that the compiler placed in float registers become 1.0 / 0.0 or the number.
//!
//! Values are given flat: all elements in order, a matrix row major (M11, M12, .. M44) as XNA stores it.

use super::bytecode::{Opcode, RegType, Shader};
use super::ctab::{Class, Constant, RegisterSet};

/// Number of float registers the uniform block of `shader` needs: one past the highest register that the
/// constant table, the preshader or an instruction refers to and that is not a `def` constant.
pub fn float_register_count(shader: &Shader) -> u32 {
    let defs: Vec<u32> = shader.float_defs().map(|(r, _)| r).collect();
    let mut end = 0u32;
    if let Some(t) = &shader.ctab {
        end = t.floats().map(|c| c.register_index + c.register_count).max().unwrap_or(0);
    }
    if let Some(p) = &shader.preshader {
        end = end.max(p.output_register_end());
    }
    for i in shader.instructions.iter().filter(|i| !matches!(i.opcode, Opcode::Def | Opcode::Dcl)) {
        for s in &i.srcs {
            if s.reg.ty == RegType::Const && !defs.contains(&s.reg.num) {
                end = end.max(s.reg.num + 1);
            }
        }
    }
    end
}

/// Pack one value into the registers of `c` (see the module docs). `regs` is the register file.
pub fn pack(c: &Constant, values: &[f32], regs: &mut [[f32; 4]]) -> Result<(), String> {
    if c.register_set != RegisterSet::Float4 {
        return Err(format!("constant {}: the {:?} register set is not supported (only float registers)", c.name, c.register_set));
    }
    let (rows, columns) = (c.rows as usize, c.columns as usize);
    let per_element = match c.class {
        Class::Scalar | Class::Vector | Class::MatrixRows | Class::MatrixColumns => rows * columns,
        Class::Struct | Class::Object => return Err(format!("constant {}: {:?} constants are not supported", c.name, c.class)),
    };
    if rows == 0 || columns == 0 || rows > 4 || columns > 4 {
        return Err(format!("constant {}: unsupported shape {rows}x{columns}", c.name));
    }
    if values.len() != per_element * c.elements as usize {
        return Err(format!("constant {} is a {}: it takes {} values, {} were given", c.name, c.hlsl(), per_element * c.elements as usize, values.len()));
    }
    let registers_per_element = match c.class {
        Class::MatrixColumns => columns,
        Class::MatrixRows => rows,
        _ => 1,
    };
    let (first, count) = (c.register_index as usize, c.register_count as usize);
    if first + count > regs.len() {
        return Err(format!("constant {}: registers {} are outside the register file of {}", c.name, c.registers(), regs.len()));
    }
    for k in 0..count {
        let (element, part) = (k / registers_per_element, k % registers_per_element);
        let mut r = [0f32; 4];
        if element < c.elements as usize {
            let v = &values[element * per_element..(element + 1) * per_element];
            match c.class {
                Class::MatrixColumns => {
                    for (row, slot) in r.iter_mut().enumerate().take(rows) {
                        *slot = v[row * columns + part];
                    }
                }
                Class::MatrixRows => r[..columns].copy_from_slice(&v[part * columns..(part + 1) * columns]),
                _ => r[..columns].copy_from_slice(v),
            }
        }
        regs[first + k] = r;
    }
    Ok(())
}

/// Fill the float register file of `shader` from effect parameters. `parameter(name)` gives the current value of
/// an effect parameter as flat floats (module docs), or `None` when the effect has no such parameter (an error:
/// every constant of a shader inside an effect is an effect parameter).
pub fn fill(shader: &Shader, parameter: &dyn Fn(&str) -> Option<Vec<f32>>) -> Result<Vec<[f32; 4]>, String> {
    let mut regs = vec![[0f32; 4]; float_register_count(shader) as usize];
    let value = |c: &Constant| parameter(&c.name).ok_or_else(|| format!("constant {}: the effect has no parameter of that name", c.name));
    if let Some(t) = &shader.ctab {
        for c in &t.constants {
            match c.register_set {
                RegisterSet::Sampler => {}
                RegisterSet::Float4 => pack(c, &value(c)?, &mut regs)?,
                other => return Err(format!("constant {}: the {other:?} register set is not supported (only float registers and samplers)", c.name)),
            }
        }
    }
    if let Some(p) = &shader.preshader {
        let mut inputs = vec![[0f32; 4]; (p.input_count / 4) as usize];
        for c in &p.ctab.constants {
            pack(c, &value(c)?, &mut inputs)?;
        }
        let flat: Vec<f32> = inputs.iter().flatten().copied().collect();
        p.run(&flat, &mut regs)?;
    }
    Ok(regs)
}

#[cfg(test)]
mod tests {
    use super::super::ctab::{Class, Constant, RegisterSet, Type};
    use super::*;

    fn constant(class: Class, rows: u32, columns: u32, elements: u32, index: u32, count: u32) -> Constant {
        Constant { name: "x".into(), register_set: RegisterSet::Float4, register_index: index, register_count: count, class, ty: Type::Float, rows, columns, elements, struct_members: 0, default_value: None }
    }

    #[test]
    fn vectors_take_one_register_per_element() {
        let mut regs = [[9f32; 4]; 4];
        pack(&constant(Class::Vector, 1, 3, 1, 1, 1), &[1.0, 2.0, 3.0], &mut regs).unwrap();
        assert_eq!(regs[1], [1.0, 2.0, 3.0, 0.0]);
        assert_eq!(regs[0], [9.0; 4]);
        // float2[2] in c2..c3
        pack(&constant(Class::Vector, 1, 2, 2, 2, 2), &[1.0, 2.0, 3.0, 4.0], &mut regs).unwrap();
        assert_eq!((regs[2], regs[3]), ([1.0, 2.0, 0.0, 0.0], [3.0, 4.0, 0.0, 0.0]));
        assert!(pack(&constant(Class::Vector, 1, 3, 1, 1, 1), &[1.0], &mut regs).unwrap_err().contains("takes 3 values"));
    }

    #[test]
    fn column_major_matrices_put_a_column_in_each_register() {
        let m: Vec<f32> = (1..=16).map(|v| v as f32).collect(); // M11 = 1, M12 = 2, ... M44 = 16
        let mut regs = [[0f32; 4]; 4];
        pack(&constant(Class::MatrixColumns, 4, 4, 1, 0, 4), &m, &mut regs).unwrap();
        assert_eq!(regs[0], [1.0, 5.0, 9.0, 13.0], "c0 = (M11, M21, M31, M41)");
        assert_eq!(regs[3], [4.0, 8.0, 12.0, 16.0]);
        pack(&constant(Class::MatrixRows, 4, 4, 1, 0, 4), &m, &mut regs).unwrap();
        assert_eq!(regs[1], [5.0, 6.0, 7.0, 8.0]);
        // only two registers used by the shader
        let mut regs = [[0f32; 4]; 2];
        pack(&constant(Class::MatrixColumns, 4, 4, 1, 0, 2), &m, &mut regs).unwrap();
        assert_eq!(regs[1], [2.0, 6.0, 10.0, 14.0]);
    }
}
