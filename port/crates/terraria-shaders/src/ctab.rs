//! The constant table of a compiled shader (the `CTAB` comment, `D3DXSHADER_CONSTANTTABLE`): which HLSL uniform
//! lives in which registers.
//!
//! Layout (MSDN `D3DXSHADER_CONSTANTTABLE`, `D3DXSHADER_CONSTANTINFO`, `D3DXSHADER_TYPEINFO`; offsets are relative
//! to the start of the table, the byte after the `CTAB` FOURCC):
//! ```text
//! table     u32 size (28), u32 creator, u32 version, u32 constants, u32 constantInfo, u32 flags, u32 target
//! constant  u32 name, u16 registerSet, u16 registerIndex, u16 registerCount, u16 reserved, u32 typeInfo,
//!           u32 defaultValue                                                          (20 bytes each)
//! type      u16 class, u16 type, u16 rows, u16 columns, u16 elements, u16 structMembers, u32 structMemberInfo
//! ```

/// `D3DXREGISTER_SET`
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum RegisterSet {
    Bool,
    Int4,
    Float4,
    Sampler,
}

impl RegisterSet {
    /// register letter
    pub fn letter(self) -> char {
        match self {
            RegisterSet::Bool => 'b',
            RegisterSet::Int4 => 'i',
            RegisterSet::Float4 => 'c',
            RegisterSet::Sampler => 's',
        }
    }
}

/// `D3DXPARAMETER_CLASS`
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Class {
    Scalar,
    Vector,
    MatrixRows,
    MatrixColumns,
    Object,
    Struct,
}

/// `D3DXPARAMETER_TYPE`, the values a constant table can hold.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Type {
    Void,
    Bool,
    Int,
    Float,
    String,
    Texture,
    Texture1D,
    Texture2D,
    Texture3D,
    TextureCube,
    Sampler,
    Sampler1D,
    Sampler2D,
    Sampler3D,
    SamplerCube,
    PixelShader,
    VertexShader,
    PixelFragment,
    VertexFragment,
    Unsupported,
}

impl Type {
    pub fn hlsl(self) -> &'static str {
        use Type::*;
        match self {
            Void => "void",
            Bool => "bool",
            Int => "int",
            Float => "float",
            String => "string",
            Texture => "texture",
            Texture1D => "texture1D",
            Texture2D => "texture2D",
            Texture3D => "texture3D",
            TextureCube => "textureCUBE",
            Sampler => "sampler",
            Sampler1D => "sampler1D",
            Sampler2D => "sampler2D",
            Sampler3D => "sampler3D",
            SamplerCube => "samplerCUBE",
            PixelShader => "pixelshader",
            VertexShader => "vertexshader",
            PixelFragment => "pixelfragment",
            VertexFragment => "vertexfragment",
            Unsupported => "unsupported",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Constant {
    pub name: String,
    pub register_set: RegisterSet,
    pub register_index: u32,
    pub register_count: u32,
    pub class: Class,
    pub ty: Type,
    pub rows: u32,
    pub columns: u32,
    /// array length (1 for a non-array)
    pub elements: u32,
    pub struct_members: u32,
    /// the default value stored in the table (4 floats per register), when there is one
    pub default_value: Option<Vec<f32>>,
}

impl Constant {
    /// HLSL-like spelling: `float4`, `float4x4`, `sampler2D`, `float3[4]`.
    pub fn hlsl(&self) -> String {
        let mut s = match self.class {
            Class::Scalar | Class::Object => self.ty.hlsl().to_owned(),
            Class::Vector => format!("{}{}", self.ty.hlsl(), self.columns),
            Class::MatrixRows => format!("row_major {}{}x{}", self.ty.hlsl(), self.rows, self.columns),
            Class::MatrixColumns => format!("{}{}x{}", self.ty.hlsl(), self.rows, self.columns),
            Class::Struct => "struct".to_owned(),
        };
        if self.elements > 1 {
            s.push_str(&format!("[{}]", self.elements));
        }
        s
    }
    /// `c4`, `c0..c3`, `s1`
    pub fn registers(&self) -> String {
        let l = self.register_set.letter();
        if self.register_count > 1 {
            format!("{l}{}..{l}{}", self.register_index, self.register_index + self.register_count - 1)
        } else {
            format!("{l}{}", self.register_index)
        }
    }
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct ConstantTable {
    /// the compiler's name ("Microsoft (R) HLSL Shader Compiler 9.x")
    pub creator: String,
    /// the shader version token the table was made for (a preshader's is `0x4658....`)
    pub version: u32,
    /// "ps_2_0", "tx_1_0" (preshader)
    pub target: String,
    pub constants: Vec<Constant>,
}

impl ConstantTable {
    pub fn floats(&self) -> impl Iterator<Item = &Constant> {
        self.constants.iter().filter(|c| c.register_set == RegisterSet::Float4)
    }
    pub fn samplers(&self) -> impl Iterator<Item = &Constant> {
        self.constants.iter().filter(|c| c.register_set == RegisterSet::Sampler)
    }
    /// The float constant that owns float register `register`.
    pub fn float_at(&self, register: u32) -> Option<&Constant> {
        self.floats().find(|c| (c.register_index..c.register_index + c.register_count).contains(&register))
    }
}

fn u16_at(d: &[u8], at: usize) -> Result<u32, String> {
    d.get(at..at + 2).map(|b| u16::from_le_bytes([b[0], b[1]]) as u32).ok_or_else(|| format!("CTAB: truncated at {at}"))
}
fn u32_at(d: &[u8], at: usize) -> Result<u32, String> {
    d.get(at..at + 4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]])).ok_or_else(|| format!("CTAB: truncated at {at}"))
}
fn str_at(d: &[u8], at: usize) -> Result<String, String> {
    let tail = d.get(at..).ok_or_else(|| format!("CTAB: string offset {at} outside the table"))?;
    let end = tail.iter().position(|b| *b == 0).ok_or("CTAB: unterminated string")?;
    Ok(String::from_utf8_lossy(&tail[..end]).into_owned())
}

/// Parse the table; `data` starts after the `CTAB` FOURCC.
pub fn parse(data: &[u8]) -> Result<ConstantTable, String> {
    let size = u32_at(data, 0)?;
    if size != 28 {
        return Err(format!("CTAB: header size {size}, expected 28"));
    }
    let (creator, version, count, info, target) = (u32_at(data, 4)?, u32_at(data, 8)?, u32_at(data, 12)?, u32_at(data, 16)? as usize, u32_at(data, 24)?);
    if count > 4096 {
        return Err(format!("CTAB: implausible constant count {count}"));
    }
    let mut table = ConstantTable { creator: str_at(data, creator as usize)?, version, target: str_at(data, target as usize)?, constants: Vec::new() };
    for i in 0..count as usize {
        let at = info + i * 20;
        let register_set = match u16_at(data, at + 4)? {
            0 => RegisterSet::Bool,
            1 => RegisterSet::Int4,
            2 => RegisterSet::Float4,
            3 => RegisterSet::Sampler,
            other => return Err(format!("CTAB: unknown register set {other}")),
        };
        let (register_index, register_count) = (u16_at(data, at + 6)?, u16_at(data, at + 8)?);
        let (type_info, default_value) = (u32_at(data, at + 12)? as usize, u32_at(data, at + 16)? as usize);
        let class = match u16_at(data, type_info)? {
            0 => Class::Scalar,
            1 => Class::Vector,
            2 => Class::MatrixRows,
            3 => Class::MatrixColumns,
            4 => Class::Object,
            5 => Class::Struct,
            other => return Err(format!("CTAB: unknown parameter class {other}")),
        };
        use Type::*;
        const TYPES: [Type; 20] = [Void, Bool, Int, Float, String, Texture, Texture1D, Texture2D, Texture3D, TextureCube, Sampler, Sampler1D, Sampler2D, Sampler3D, SamplerCube, PixelShader, VertexShader, PixelFragment, VertexFragment, Unsupported];
        let ty = *TYPES.get(u16_at(data, type_info + 2)? as usize).ok_or("CTAB: unknown parameter type")?;
        let default_value = if default_value != 0 && register_set == RegisterSet::Float4 {
            let mut v = Vec::new();
            for k in 0..register_count as usize * 4 {
                v.push(f32::from_bits(u32_at(data, default_value + k * 4)?));
            }
            Some(v)
        } else {
            None
        };
        table.constants.push(Constant {
            name: str_at(data, u32_at(data, at)? as usize)?,
            register_set,
            register_index,
            register_count,
            class,
            ty,
            rows: u16_at(data, type_info + 4)?,
            columns: u16_at(data, type_info + 6)?,
            elements: u16_at(data, type_info + 8)?.max(1),
            struct_members: u16_at(data, type_info + 10)?,
            default_value,
        });
    }
    Ok(table)
}
