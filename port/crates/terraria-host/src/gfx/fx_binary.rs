//! The compiled Direct3D 9 effect (`.fx` compiled by the XNA content pipeline, a D3DX "fx_2_0" binary), read
//! completely: parameters (type, class, rows / columns, semantic, default value, annotations), techniques, passes,
//! every state assignment of every pass and sampler, and the shader blobs. The Direct3D 9 shader bytecode inside
//! the blobs is NOT decoded here; that is `terraria-shaders`.
//!
//! Layout (public descriptions: Wine `dlls/d3dx9_36/effect.c` `d3dx_parse_effect`, MojoShader
//! `mojoshader_effects.c`; all offsets are relative to `base`, the byte after the 8-byte D3DX header):
//! ```text
//! [XNA wrapper, optional]  u32 0xBCF00BCF, u32 offset of the D3DX effect from the start of the blob
//! D3DX effect              u32 0xFEFF0901, u32 start        -> base = here; the tables are at base + start
//! tables                   u32 numParams, u32 numTechniques, u32 shaderCount, u32 numObjects
//!   param                  u32 typeOffset, u32 valueOffset, u32 flags, u32 numAnnotations, annotations (2 x u32 each)
//!   technique              u32 nameOffset, u32 numAnnotations, u32 numPasses, annotations, passes
//!   pass                   u32 nameOffset, u32 numAnnotations, u32 numStates, annotations, states
//!   state                  u32 operation, u32 index, u32 typeOffset, u32 valueOffset
//!   u32 numSmallObjects, u32 numLargeObjects
//!   small object           u32 objectId, u32 size, bytes (padded to 4)              strings, texture names
//!   large object           u32 technique (-1: a parameter), u32 pass / parameter index, u32 element, u32 state index,
//!                          u32 usage, u32 size, bytes (padded to 4)
//!                          usage 0: shader bytecode (shader states) or an FXLC expression (numeric states),
//!                          usage 1: the name of the parameter the state takes its value from,
//!                          usage 2: an array selector (name + FXLC index expression)
//! type (at base + typeOffset)   u32 type, u32 class, u32 nameOffset, u32 semanticOffset, u32 elements, then
//!                          vector: u32 columns, u32 rows; scalar / matrix: u32 rows, u32 columns;
//!                          struct: u32 members, member types inline; object: nothing
//! value (at base + valueOffset) numeric: 4 bytes per component; string / texture / shader: u32 object id per
//!                          element; sampler: u32 numStates, states
//! string (at base + offset)     u32 length (with the terminating zero), bytes
//! ```
//! `operation` indexes the fixed state table of D3DX effects ([`STATE_TABLE`]): render states, texture stage
//! states, fixed-function states, `VertexShader` (146), `PixelShader` (147), shader constants, `Texture` (164),
//! sampler states (165..=177) and `Sampler` (178).

use std::sync::Arc;

use super::effect::EffectDesc;

/// `D3DXPARAMETER_CLASS`
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ParamClass {
    Scalar,
    Vector,
    MatrixRows,
    MatrixColumns,
    Object,
    Struct,
}

/// `D3DXPARAMETER_TYPE`
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ParamType {
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

impl ParamType {
    fn from_u32(v: u32) -> Result<ParamType, String> {
        use ParamType::*;
        const ALL: [ParamType; 20] =
            [Void, Bool, Int, Float, String, Texture, Texture1D, Texture2D, Texture3D, TextureCube, Sampler, Sampler1D, Sampler2D, Sampler3D, SamplerCube, PixelShader, VertexShader, PixelFragment, VertexFragment, Unsupported];
        ALL.get(v as usize).copied().ok_or_else(|| format!("unknown D3DXPARAMETER_TYPE {v}"))
    }
    pub fn is_sampler(self) -> bool {
        matches!(self, ParamType::Sampler | ParamType::Sampler1D | ParamType::Sampler2D | ParamType::Sampler3D | ParamType::SamplerCube)
    }
    pub fn is_texture(self) -> bool {
        matches!(self, ParamType::Texture | ParamType::Texture1D | ParamType::Texture2D | ParamType::Texture3D | ParamType::TextureCube)
    }
    pub fn is_shader(self) -> bool {
        matches!(self, ParamType::PixelShader | ParamType::VertexShader)
    }
    /// HLSL spelling
    pub fn hlsl(self) -> &'static str {
        use ParamType::*;
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

/// The type of a parameter, annotation or state value.
#[derive(Clone, Debug, PartialEq)]
pub struct FxType {
    pub ty: ParamType,
    pub class: ParamClass,
    pub name: String,
    pub semantic: String,
    /// array length; 0 = not an array
    pub elements: u32,
    pub rows: u32,
    pub columns: u32,
    /// struct members
    pub members: Vec<FxType>,
}

impl FxType {
    /// Number of 4-byte components of ONE element (numeric types and structs of them).
    pub fn components(&self) -> u32 {
        match self.class {
            ParamClass::Scalar | ParamClass::Vector | ParamClass::MatrixRows | ParamClass::MatrixColumns => self.rows * self.columns,
            ParamClass::Struct => self.members.iter().map(|m| m.components() * m.elements.max(1)).sum(),
            ParamClass::Object => 0,
        }
    }
    /// HLSL-like spelling: `float4`, `float4x4`, `sampler2D`, `float3[4]`.
    pub fn hlsl(&self) -> String {
        let mut s = match self.class {
            ParamClass::Scalar => self.ty.hlsl().to_owned(),
            ParamClass::Vector => format!("{}{}", self.ty.hlsl(), self.columns),
            ParamClass::MatrixRows => format!("row_major {}{}x{}", self.ty.hlsl(), self.rows, self.columns),
            ParamClass::MatrixColumns => format!("{}{}x{}", self.ty.hlsl(), self.rows, self.columns),
            ParamClass::Object => self.ty.hlsl().to_owned(),
            ParamClass::Struct => "struct".to_owned(),
        };
        if self.elements > 0 {
            s.push_str(&format!("[{}]", self.elements));
        }
        s
    }
}

/// A value stored in the effect file.
#[derive(Clone, Debug, PartialEq)]
pub enum FxValue {
    /// numeric: the raw 4-byte components (reinterpret by `FxType::ty`), all elements in order
    Numeric(Vec<u32>),
    /// string / texture / shader: one object id per element
    Objects(Vec<u32>),
    /// sampler: the sampler's state assignments, per element
    Samplers(Vec<Vec<FxState>>),
}

impl FxValue {
    /// Numeric components as floats, converting from the stored type (`bool` / `int` -> float as D3DX does).
    pub fn floats(&self, ty: ParamType) -> Option<Vec<f32>> {
        let FxValue::Numeric(raw) = self else { return None };
        Some(raw.iter().map(|w| raw_to_f32(*w, ty)).collect())
    }
}

/// One stored component as a float (`float`: the bits; `int`: the integer; `bool`: 0 or 1).
pub fn raw_to_f32(raw: u32, ty: ParamType) -> f32 {
    match ty {
        ParamType::Float => f32::from_bits(raw),
        ParamType::Int => raw as i32 as f32,
        ParamType::Bool => (raw != 0) as u32 as f32,
        _ => f32::from_bits(raw),
    }
}

/// What a state table entry sets (Wine's `STATE_CLASS`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum StateClass {
    LightEnable,
    Fvf,
    Light,
    Material,
    NPatchMode,
    PixelShader,
    RenderState,
    SetSampler,
    SamplerState,
    Texture,
    TextureStage,
    Transform,
    VertexShader,
    ShaderConst,
}

/// The fixed table `operation` indexes: (class, name). The order is the file format's (it is not Direct3D's
/// numbering of render states). Source: Wine `dlls/d3dx9_36/effect.c` `state_table`, the same order as MojoShader's
/// `MOJOSHADER_renderStateType` for the render states.
pub const STATE_TABLE: [(StateClass, &str); 179] = {
    use StateClass::*;
    [
        (RenderState, "ZEnable"),
        (RenderState, "FillMode"),
        (RenderState, "ShadeMode"),
        (RenderState, "ZWriteEnable"),
        (RenderState, "AlphaTestEnable"),
        (RenderState, "LastPixel"),
        (RenderState, "SrcBlend"),
        (RenderState, "DestBlend"),
        (RenderState, "CullMode"),
        (RenderState, "ZFunc"),
        (RenderState, "AlphaRef"),
        (RenderState, "AlphaFunc"),
        (RenderState, "DitherEnable"),
        (RenderState, "AlphaBlendEnable"),
        (RenderState, "FogEnable"),
        (RenderState, "SpecularEnable"),
        (RenderState, "FogColor"),
        (RenderState, "FogTableMode"),
        (RenderState, "FogStart"),
        (RenderState, "FogEnd"),
        (RenderState, "FogDensity"),
        (RenderState, "RangeFogEnable"),
        (RenderState, "StencilEnable"),
        (RenderState, "StencilFail"),
        (RenderState, "StencilZFail"),
        (RenderState, "StencilPass"),
        (RenderState, "StencilFunc"),
        (RenderState, "StencilRef"),
        (RenderState, "StencilMask"),
        (RenderState, "StencilWriteMask"),
        (RenderState, "TextureFactor"),
        (RenderState, "Wrap0"),
        (RenderState, "Wrap1"),
        (RenderState, "Wrap2"),
        (RenderState, "Wrap3"),
        (RenderState, "Wrap4"),
        (RenderState, "Wrap5"),
        (RenderState, "Wrap6"),
        (RenderState, "Wrap7"),
        (RenderState, "Wrap8"),
        (RenderState, "Wrap9"),
        (RenderState, "Wrap10"),
        (RenderState, "Wrap11"),
        (RenderState, "Wrap12"),
        (RenderState, "Wrap13"),
        (RenderState, "Wrap14"),
        (RenderState, "Wrap15"),
        (RenderState, "Clipping"),
        (RenderState, "Lighting"),
        (RenderState, "Ambient"),
        (RenderState, "FogVertexMode"),
        (RenderState, "ColorVertex"),
        (RenderState, "LocalViewer"),
        (RenderState, "NormalizeNormals"),
        (RenderState, "DiffuseMaterialSource"),
        (RenderState, "SpecularMaterialSource"),
        (RenderState, "AmbientMaterialSource"),
        (RenderState, "EmissiveMaterialSource"),
        (RenderState, "VertexBlend"),
        (RenderState, "ClipPlaneEnable"),
        (RenderState, "PointSize"),
        (RenderState, "PointSize_Min"),
        (RenderState, "PointSize_Max"),
        (RenderState, "PointSpriteEnable"),
        (RenderState, "PointScaleEnable"),
        (RenderState, "PointScale_A"),
        (RenderState, "PointScale_B"),
        (RenderState, "PointScale_C"),
        (RenderState, "MultiSampleAntialias"),
        (RenderState, "MultiSampleMask"),
        (RenderState, "PatchEdgeStyle"),
        (RenderState, "DebugMonitorToken"),
        (RenderState, "IndexedVertexBlendEnable"),
        (RenderState, "ColorWriteEnable"),
        (RenderState, "TweenFactor"),
        (RenderState, "BlendOp"),
        (RenderState, "PositionDegree"),
        (RenderState, "NormalDegree"),
        (RenderState, "ScissorTestEnable"),
        (RenderState, "SlopeScaleDepthBias"),
        (RenderState, "AntialiasedLineEnable"),
        (RenderState, "MinTessellationLevel"),
        (RenderState, "MaxTessellationLevel"),
        (RenderState, "AdaptiveTess_X"),
        (RenderState, "AdaptiveTess_Y"),
        (RenderState, "AdaptiveTess_Z"),
        (RenderState, "AdaptiveTess_W"),
        (RenderState, "EnableAdaptiveTessellation"),
        (RenderState, "TwoSidedStencilMode"),
        (RenderState, "CCW_StencilFail"),
        (RenderState, "CCW_StencilZFail"),
        (RenderState, "CCW_StencilPass"),
        (RenderState, "CCW_StencilFunc"),
        (RenderState, "ColorWriteEnable1"),
        (RenderState, "ColorWriteEnable2"),
        (RenderState, "ColorWriteEnable3"),
        (RenderState, "BlendFactor"),
        (RenderState, "SRGBWriteEnable"),
        (RenderState, "DepthBias"),
        (RenderState, "SeparateAlphaBlendEnable"),
        (RenderState, "SrcBlendAlpha"),
        (RenderState, "DestBlendAlpha"),
        (RenderState, "BlendOpAlpha"),
        (TextureStage, "ColorOp"),
        (TextureStage, "ColorArg0"),
        (TextureStage, "ColorArg1"),
        (TextureStage, "ColorArg2"),
        (TextureStage, "AlphaOp"),
        (TextureStage, "AlphaArg0"),
        (TextureStage, "AlphaArg1"),
        (TextureStage, "AlphaArg2"),
        (TextureStage, "ResultArg"),
        (TextureStage, "BumpEnvMat00"),
        (TextureStage, "BumpEnvMat01"),
        (TextureStage, "BumpEnvMat10"),
        (TextureStage, "BumpEnvMat11"),
        (TextureStage, "TexCoordIndex"),
        (TextureStage, "BumpEnvLScale"),
        (TextureStage, "BumpEnvLOffset"),
        (TextureStage, "TextureTransformFlags"),
        (TextureStage, "Constant"),
        (NPatchMode, "NPatchMode"),
        (Fvf, "FVF"),
        (Transform, "ProjectionTransform"),
        (Transform, "ViewTransform"),
        (Transform, "WorldTransform"),
        (Transform, "TextureTransform"),
        (Material, "MaterialDiffuse"),
        (Material, "MaterialAmbient"),
        (Material, "MaterialSpecular"),
        (Material, "MaterialEmissive"),
        (Material, "MaterialPower"),
        (Light, "LightType"),
        (Light, "LightDiffuse"),
        (Light, "LightSpecular"),
        (Light, "LightAmbient"),
        (Light, "LightPosition"),
        (Light, "LightDirection"),
        (Light, "LightRange"),
        (Light, "LightFallOff"),
        (Light, "LightAttenuation0"),
        (Light, "LightAttenuation1"),
        (Light, "LightAttenuation2"),
        (Light, "LightTheta"),
        (Light, "LightPhi"),
        (LightEnable, "LightEnable"),
        (VertexShader, "VertexShader"),
        (PixelShader, "PixelShader"),
        (ShaderConst, "VertexShaderConstantF"),
        (ShaderConst, "VertexShaderConstantB"),
        (ShaderConst, "VertexShaderConstantI"),
        (ShaderConst, "VertexShaderConstant"),
        (ShaderConst, "VertexShaderConstant1"),
        (ShaderConst, "VertexShaderConstant2"),
        (ShaderConst, "VertexShaderConstant3"),
        (ShaderConst, "VertexShaderConstant4"),
        (ShaderConst, "PixelShaderConstantF"),
        (ShaderConst, "PixelShaderConstantB"),
        (ShaderConst, "PixelShaderConstantI"),
        (ShaderConst, "PixelShaderConstant"),
        (ShaderConst, "PixelShaderConstant1"),
        (ShaderConst, "PixelShaderConstant2"),
        (ShaderConst, "PixelShaderConstant3"),
        (ShaderConst, "PixelShaderConstant4"),
        (Texture, "Texture"),
        (SamplerState, "AddressU"),
        (SamplerState, "AddressV"),
        (SamplerState, "AddressW"),
        (SamplerState, "BorderColor"),
        (SamplerState, "MagFilter"),
        (SamplerState, "MinFilter"),
        (SamplerState, "MipFilter"),
        (SamplerState, "MipMapLodBias"),
        (SamplerState, "MaxMipLevel"),
        (SamplerState, "MaxAnisotropy"),
        (SamplerState, "SRGBTexture"),
        (SamplerState, "ElementIndex"),
        (SamplerState, "DMAPOffset"),
        (SetSampler, "Sampler"),
    ]
};

/// Where a state's value comes from when it is not the constant stored with it.
#[derive(Clone, Debug, PartialEq)]
pub enum FxStateSource {
    /// the value stored in the file (`FxState::value`)
    Constant,
    /// compiled shader bytecode (`VertexShader` / `PixelShader` states compiled inline in the pass)
    Shader(Arc<[u8]>),
    /// an FXLC expression program evaluated from parameters at run time (not executed by this crate)
    Expression(Arc<[u8]>),
    /// the value of the named effect parameter
    Parameter(String),
    /// an element of the named array parameter chosen by an FXLC expression (not executed by this crate)
    ArraySelector { parameter: String, program: Arc<[u8]> },
}

/// One state assignment of a pass or sampler.
#[derive(Clone, Debug, PartialEq)]
pub struct FxState {
    /// index into [`STATE_TABLE`]
    pub operation: u32,
    /// the state's array index (`Sampler[1]`, `Wrap3`...): for `Sampler` the sampler register
    pub index: u32,
    pub ty: FxType,
    pub value: FxValue,
    pub source: FxStateSource,
}

impl FxState {
    pub fn class(&self) -> StateClass {
        STATE_TABLE[self.operation as usize].0
    }
    pub fn name(&self) -> &'static str {
        STATE_TABLE[self.operation as usize].1
    }
    /// The constant value as one integer (render and sampler states are `int` / `bool` / `float` scalars).
    pub fn constant_u32(&self) -> Option<u32> {
        match (&self.source, &self.value) {
            (FxStateSource::Constant, FxValue::Numeric(v)) if v.len() == 1 => Some(match self.ty.ty {
                ParamType::Float => f32::from_bits(v[0]) as u32,
                _ => v[0],
            }),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct FxAnnotation {
    pub ty: FxType,
    pub value: FxValue,
    /// string annotations: the text
    pub text: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FxParameter {
    pub ty: FxType,
    /// the default value compiled into the effect
    pub value: FxValue,
    pub flags: u32,
    pub annotations: Vec<FxAnnotation>,
}

impl FxParameter {
    pub fn name(&self) -> &str {
        &self.ty.name
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct FxPass {
    pub name: String,
    pub annotations: Vec<FxAnnotation>,
    pub states: Vec<FxState>,
}

impl FxPass {
    fn shader(&self, class: StateClass) -> Option<&Arc<[u8]>> {
        self.states.iter().find(|s| s.class() == class).and_then(|s| match &s.source {
            FxStateSource::Shader(b) => Some(b),
            _ => None,
        })
    }
    /// The pass's compiled pixel shader, when the pass assigns one inline.
    pub fn pixel_shader(&self) -> Option<&Arc<[u8]>> {
        self.shader(StateClass::PixelShader)
    }
    /// The pass's compiled vertex shader, when the pass assigns one inline.
    pub fn vertex_shader(&self) -> Option<&Arc<[u8]>> {
        self.shader(StateClass::VertexShader)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct FxTechnique {
    pub name: String,
    pub annotations: Vec<FxAnnotation>,
    pub passes: Vec<FxPass>,
}

/// A whole compiled effect.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct FxEffect {
    pub parameters: Vec<FxParameter>,
    pub techniques: Vec<FxTechnique>,
    /// the header's third count (Wine: "shader count"); informative
    pub shader_count: u32,
    /// the small objects by id (strings, texture names); empty for ids that are shaders or unused
    pub objects: Vec<Vec<u8>>,
}

impl FxEffect {
    pub fn parameter(&self, name: &str) -> Option<&FxParameter> {
        self.parameters.iter().find(|p| p.ty.name == name)
    }
}

struct Parser<'a> {
    data: &'a [u8],
    base: usize,
    object_count: u32,
}

/// A read position inside the effect.
struct Cursor<'a> {
    data: &'a [u8],
    pos: usize,
}
impl Cursor<'_> {
    fn u32(&mut self) -> Result<u32, String> {
        let b = self.pos.checked_add(4).and_then(|end| self.data.get(self.pos..end)).ok_or_else(|| format!("truncated at {}", self.pos))?;
        self.pos += 4;
        Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }
    fn bytes(&mut self, n: usize) -> Result<&[u8], String> {
        let b = self.pos.checked_add(n).and_then(|end| self.data.get(self.pos..end)).ok_or_else(|| format!("truncated at {} (+{n})", self.pos))?;
        // data is padded to a multiple of four bytes
        self.pos += (n + 3) & !3;
        Ok(b)
    }
}

const MAX_COUNT: u32 = 4096;

fn count(n: u32, what: &str) -> Result<u32, String> {
    if n > MAX_COUNT {
        return Err(format!("implausible {what} count {n}"));
    }
    Ok(n)
}

impl<'a> Parser<'a> {
    fn at(&self, offset: u32) -> Result<Cursor<'a>, String> {
        let pos = self.base.checked_add(offset as usize).filter(|p| *p <= self.data.len()).ok_or_else(|| format!("offset {offset} outside the effect"))?;
        Ok(Cursor { data: self.data, pos })
    }

    fn string(&self, offset: u32) -> Result<String, String> {
        let mut c = self.at(offset)?;
        let len = c.u32()? as usize;
        if len == 0 {
            return Ok(String::new());
        }
        let bytes = c.bytes(len).map_err(|_| "string length outside the effect".to_owned())?;
        let text = bytes.strip_suffix(&[0]).unwrap_or(bytes);
        if !text.iter().all(|c| c.is_ascii_graphic() || *c == b' ') {
            return Err(format!("not a name at offset {offset}: {text:?}"));
        }
        Ok(String::from_utf8_lossy(text).into_owned())
    }

    fn typedef(&self, c: &mut Cursor<'a>, depth: u32) -> Result<FxType, String> {
        if depth > 8 {
            return Err("struct nesting too deep".into());
        }
        let ty = ParamType::from_u32(c.u32()?)?;
        let class = match c.u32()? {
            0 => ParamClass::Scalar,
            1 => ParamClass::Vector,
            2 => ParamClass::MatrixRows,
            3 => ParamClass::MatrixColumns,
            4 => ParamClass::Object,
            5 => ParamClass::Struct,
            other => return Err(format!("unknown D3DXPARAMETER_CLASS {other}")),
        };
        let name = self.string(c.u32()?)?;
        let semantic = self.string(c.u32()?)?;
        let elements = count(c.u32()?, "element")?;
        let (mut rows, mut columns, mut members) = (0, 0, Vec::new());
        match class {
            ParamClass::Vector => {
                columns = c.u32()?;
                rows = c.u32()?;
            }
            ParamClass::Scalar | ParamClass::MatrixRows | ParamClass::MatrixColumns => {
                rows = c.u32()?;
                columns = c.u32()?;
            }
            ParamClass::Struct => {
                let n = count(c.u32()?, "struct member")?;
                for _ in 0..n {
                    members.push(self.typedef(c, depth + 1)?);
                }
            }
            ParamClass::Object => {}
        }
        if rows > 4 || columns > 4 {
            return Err(format!("implausible type {rows}x{columns} for {name}"));
        }
        Ok(FxType { ty, class, name, semantic, elements, rows, columns, members })
    }

    fn value(&self, ty: &FxType, c: &mut Cursor<'a>) -> Result<FxValue, String> {
        let n = ty.elements.max(1);
        match ty.class {
            ParamClass::Object => {
                if ty.ty.is_sampler() {
                    let mut samplers = Vec::new();
                    for _ in 0..n {
                        let states = count(c.u32()?, "sampler state")?;
                        let mut list = Vec::new();
                        for _ in 0..states {
                            list.push(self.state(c)?);
                        }
                        samplers.push(list);
                    }
                    Ok(FxValue::Samplers(samplers))
                } else if ty.ty.is_texture() || ty.ty.is_shader() || ty.ty == ParamType::String {
                    let mut ids = Vec::new();
                    for _ in 0..n {
                        let id = c.u32()?;
                        if id >= self.object_count {
                            return Err(format!("object id {id} of {} outside the {} objects", ty.name, self.object_count));
                        }
                        ids.push(id);
                    }
                    Ok(FxValue::Objects(ids))
                } else {
                    Err(format!("object parameter {} of unsupported type {:?}", ty.name, ty.ty))
                }
            }
            _ => {
                let words = ty.components() * n;
                let mut raw = Vec::with_capacity(words as usize);
                for _ in 0..words {
                    raw.push(c.u32()?);
                }
                Ok(FxValue::Numeric(raw))
            }
        }
    }

    fn typed_value(&self, c: &mut Cursor<'a>) -> Result<(FxType, FxValue), String> {
        let (type_offset, value_offset) = (c.u32()?, c.u32()?);
        let ty = self.typedef(&mut self.at(type_offset)?, 0)?;
        let value = self.value(&ty, &mut self.at(value_offset)?)?;
        Ok((ty, value))
    }

    fn state(&self, c: &mut Cursor<'a>) -> Result<FxState, String> {
        let operation = c.u32()?;
        if operation as usize >= STATE_TABLE.len() {
            return Err(format!("unknown state operation {operation}"));
        }
        let index = c.u32()?;
        let (ty, value) = self.typed_value(c)?;
        Ok(FxState { operation, index, ty, value, source: FxStateSource::Constant })
    }

    fn annotations(&self, c: &mut Cursor<'a>, n: u32) -> Result<Vec<FxAnnotation>, String> {
        let mut out = Vec::new();
        for _ in 0..count(n, "annotation")? {
            let (ty, value) = self.typed_value(c)?;
            out.push(FxAnnotation { ty, value, text: None });
        }
        Ok(out)
    }
}

fn c_string(bytes: &[u8]) -> String {
    let end = bytes.iter().position(|b| *b == 0).unwrap_or(bytes.len());
    String::from_utf8_lossy(&bytes[..end]).into_owned()
}

/// Everything in a compiled effect.
pub fn parse_full(bytecode: &[u8]) -> Result<FxEffect, String> {
    let word = |at: usize| bytecode.get(at..at.checked_add(4)?).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]));
    let mut start = 0usize;
    if word(0) == Some(0xBCF0_0BCF) {
        start = word(4).ok_or("truncated XNA effect header")? as usize;
    }
    if word(start) != Some(0xFEFF_0901) {
        return Err(format!("no D3DX effect signature at {start} (found {:08X?})", word(start)));
    }
    let tables = word(start + 4).ok_or("truncated effect header")?;
    let mut p = Parser { data: bytecode, base: start + 8, object_count: 0 };
    let mut c = p.at(tables)?;
    let (num_params, num_techniques, shader_count, num_objects) = (count(c.u32()?, "parameter")?, count(c.u32()?, "technique")?, c.u32()?, c.u32()?);
    if num_techniques > 256 || num_objects > 65536 {
        return Err(format!("implausible counts: {num_techniques} techniques, {num_objects} objects"));
    }
    p.object_count = num_objects;
    let mut fx = FxEffect { shader_count, objects: vec![Vec::new(); num_objects as usize], ..Default::default() };
    for _ in 0..num_params {
        let (type_offset, value_offset, flags, annotations) = (c.u32()?, c.u32()?, c.u32()?, c.u32()?);
        let annotations = p.annotations(&mut c, annotations)?;
        let ty = p.typedef(&mut p.at(type_offset)?, 0)?;
        if ty.name.is_empty() {
            return Err("a parameter without a name".into());
        }
        let value = p.value(&ty, &mut p.at(value_offset)?)?;
        fx.parameters.push(FxParameter { ty, value, flags, annotations });
    }
    for _ in 0..num_techniques {
        let (name_offset, annotations, num_passes) = (c.u32()?, c.u32()?, count(c.u32()?, "pass")?);
        let annotations = p.annotations(&mut c, annotations)?;
        let mut passes = Vec::new();
        for _ in 0..num_passes {
            let (pass_name, pass_annotations, states) = (c.u32()?, c.u32()?, count(c.u32()?, "state")?);
            let pass_annotations = p.annotations(&mut c, pass_annotations)?;
            let mut list = Vec::new();
            for _ in 0..states {
                list.push(p.state(&mut c)?);
            }
            passes.push(FxPass { name: p.string(pass_name)?, annotations: pass_annotations, states: list });
        }
        fx.techniques.push(FxTechnique { name: p.string(name_offset)?, annotations, passes });
    }
    if fx.techniques.is_empty() {
        return Err("the effect has no technique".into());
    }

    let (small, large) = (c.u32()?, c.u32()?);
    if small > 65536 || large > 65536 {
        return Err(format!("implausible object counts: {small} small, {large} large"));
    }
    for _ in 0..small {
        let id = c.u32()?;
        let size = c.u32()? as usize;
        let data = c.bytes(size)?.to_vec();
        *fx.objects.get_mut(id as usize).ok_or_else(|| format!("small object id {id} outside the {num_objects} objects"))? = data;
    }
    for k in 0..large {
        let (technique, index, element, state_index, usage) = (c.u32()?, c.u32()?, c.u32()?, c.u32()?, c.u32()?);
        let size = c.u32()? as usize;
        let data: Arc<[u8]> = c.bytes(size)?.into();
        let state: &mut FxState = if technique == u32::MAX {
            let param = fx.parameters.get_mut(index as usize).ok_or_else(|| format!("large object {k}: parameter {index} does not exist"))?;
            let FxValue::Samplers(samplers) = &mut param.value else { return Err(format!("large object {k}: parameter {} is not a sampler", param.ty.name)) };
            let e = if element == u32::MAX || param.ty.elements == 0 { 0 } else { element as usize };
            samplers.get_mut(e).and_then(|s| s.get_mut(state_index as usize)).ok_or_else(|| format!("large object {k}: sampler state {state_index} of {} does not exist", param.ty.name))?
        } else {
            fx.techniques
                .get_mut(technique as usize)
                .and_then(|t| t.passes.get_mut(index as usize))
                .and_then(|p| p.states.get_mut(state_index as usize))
                .ok_or_else(|| format!("large object {k}: technique {technique} pass {index} state {state_index} does not exist"))?
        };
        state.source = match usage {
            0 if state.ty.ty.is_shader() => FxStateSource::Shader(data),
            0 if matches!(state.ty.ty, ParamType::Bool | ParamType::Int | ParamType::Float | ParamType::String) => FxStateSource::Expression(data),
            1 => FxStateSource::Parameter(c_string(&data)),
            2 => {
                // u32 size of the name (padded), the name, then the FXLC program
                let name_size = data.get(0..4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]) as usize).ok_or_else(|| format!("large object {k}: truncated array selector"))?;
                let name = data.get(4..4 + name_size).ok_or_else(|| format!("large object {k}: truncated array selector name"))?;
                FxStateSource::ArraySelector { parameter: c_string(name), program: data[4 + name_size..].into() }
            }
            other => return Err(format!("large object {k}: unknown usage {other} for a {:?} state", state.ty.ty)),
        };
    }

    // string annotations: resolve the text
    let objects = fx.objects.clone();
    let resolve = |list: &mut Vec<FxAnnotation>| {
        for a in list {
            if let (ParamType::String, FxValue::Objects(ids)) = (a.ty.ty, &a.value) {
                a.text = ids.first().and_then(|id| objects.get(*id as usize)).map(|b| c_string(b));
            }
        }
    };
    for param in &mut fx.parameters {
        resolve(&mut param.annotations);
    }
    for t in &mut fx.techniques {
        resolve(&mut t.annotations);
        for pass in &mut t.passes {
            resolve(&mut pass.annotations);
        }
    }
    Ok(fx)
}

/// The effect as the object model needs it: names for `Effect` / `EffectTechnique` / `EffectPass` /
/// `EffectParameter`, plus the whole parsed effect and the file's bytes for the backend.
pub fn parse(bytecode: &[u8]) -> Result<EffectDesc, String> {
    let fx = parse_full(bytecode)?;
    Ok(EffectDesc {
        techniques: fx.techniques.iter().map(|t| (t.name.clone(), t.passes.iter().map(|p| p.name.clone()).collect())).collect(),
        parameters: fx.parameters.iter().map(|p| p.ty.name.clone()).collect(),
        fx: Some(Arc::new(fx)),
        bytecode: Some(bytecode.into()),
    })
}

/// The game's three effects from the player's install, or `None` (with a note on stderr) when it is absent.
#[cfg(test)]
pub(crate) fn load_game_effect(name: &str) -> Option<(Vec<u8>, FxEffect)> {
    use terraria_formats::xnb::{read_xnb, Asset};
    let path = crate::content::install_dir().join("Content").join(format!("{name}.xnb"));
    if !path.is_file() {
        eprintln!("skipped: no Terraria install ({} is missing)", path.display());
        return None;
    }
    let x = read_xnb(&std::fs::read(&path).unwrap()).unwrap();
    let Asset::Effect(e) = x.asset else { panic!("{name} is not an effect") };
    let fx = parse_full(&e.bytecode).unwrap_or_else(|err| panic!("{name}: {err}"));
    Some((e.bytecode, fx))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The game's three effects (needs the player's install; skipped with a note when it is absent). The names
    /// checked are ones the game's code asks for by name (`ArmorShaderData`, `MiscShaderData`, `Filters`).
    #[test]
    fn parses_the_games_effects() {
        let Some((bytes, pixel)) = load_game_effect("PixelShader") else { return };
        let desc = parse(&bytes).unwrap();
        let passes: usize = pixel.techniques.iter().map(|t| t.passes.len()).sum();
        eprintln!("PixelShader: {} techniques, {passes} passes, {} parameters: {:?}", pixel.techniques.len(), pixel.parameters.len(), desc.parameters);
        assert!(desc.techniques[0].1.iter().any(|p| p == "ArmorColored"), "{:?}", desc.techniques);
        assert!(desc.parameters.iter().any(|p| p == "uColor") && desc.parameters.iter().any(|p| p == "uOpacity"));
        let (_, tile) = load_game_effect("TileShader").unwrap();
        eprintln!("TileShader: {:?}", tile.techniques.iter().map(|t| (&t.name, t.passes.len())).collect::<Vec<_>>());
        assert!(!tile.techniques[0].passes.is_empty());
        let (_, screen) = load_game_effect("ScreenShader").unwrap();
        let passes: usize = screen.techniques.iter().map(|t| t.passes.len()).sum();
        eprintln!("ScreenShader: {} techniques, {passes} passes, {} parameters", screen.techniques.len(), screen.parameters.len());
        assert!(screen.parameter("uIntensity").is_some());
    }

    /// Internal consistency of the three real files: every pass assigns a compiled pixel shader that starts with a
    /// pixel shader version token and ends with the end token; vertex shaders likewise; the passes assign NOTHING
    /// else (no render state, no sampler state, no texture: `EffectPass.Apply` relies on it, see `effect.rs`), and
    /// no sampler parameter carries sampler states; parameter values have the size their type says; parameter
    /// names are unique; a repeated pass name (PixelShader has `ArmorPlaid` twice) repeats the same bytecode.
    /// (That every constant of every shader maps to a parameter is checked in `terraria-shaders`, which decodes
    /// the constant tables: test `every_constant_maps_to_an_effect_parameter`.)
    #[test]
    fn the_games_effects_are_internally_consistent() {
        for name in ["PixelShader", "TileShader", "ScreenShader"] {
            let Some((_, fx)) = load_game_effect(name) else { return };
            let mut names: Vec<&str> = fx.parameters.iter().map(|p| p.name()).collect();
            names.sort_unstable();
            let before = names.len();
            names.dedup();
            assert_eq!(before, names.len(), "{name}: duplicate parameter names");
            for p in &fx.parameters {
                match &p.value {
                    FxValue::Numeric(raw) => assert_eq!(raw.len() as u32, p.ty.components() * p.ty.elements.max(1), "{name}.{}", p.name()),
                    FxValue::Objects(ids) => assert_eq!(ids.len() as u32, p.ty.elements.max(1), "{name}.{}", p.name()),
                    FxValue::Samplers(s) => {
                        assert_eq!(s.len() as u32, p.ty.elements.max(1), "{name}.{}", p.name());
                        assert!(s.iter().all(|states| states.is_empty()), "{name}.{}: the sampler has state assignments", p.name());
                    }
                }
            }
            let (mut ps, mut vs) = (0, 0);
            for t in &fx.techniques {
                for (i, pass) in t.passes.iter().enumerate() {
                    if let Some(first) = t.passes[..i].iter().find(|p| p.name == pass.name) {
                        eprintln!("{name}.{}: pass {i} repeats the name {}", t.name, pass.name);
                        assert_eq!(first.pixel_shader(), pass.pixel_shader(), "{name}.{}: two different passes named {}", t.name, pass.name);
                    }
                }
                for pass in &t.passes {
                    let blob = pass.pixel_shader().unwrap_or_else(|| panic!("{name}.{}.{}: no compiled pixel shader", t.name, pass.name));
                    let version = u32::from_le_bytes(blob[0..4].try_into().unwrap());
                    assert_eq!(version >> 16, 0xFFFF, "{name}.{}: pixel shader version token {version:08X}", pass.name);
                    assert_eq!(&blob[blob.len() - 4..], &0x0000_FFFFu32.to_le_bytes(), "{name}.{}: no end token", pass.name);
                    ps += 1;
                    if let Some(blob) = pass.vertex_shader() {
                        let version = u32::from_le_bytes(blob[0..4].try_into().unwrap());
                        assert_eq!(version >> 16, 0xFFFE, "{name}.{}: vertex shader version token {version:08X}", pass.name);
                        vs += 1;
                    }
                    for s in &pass.states {
                        match (&s.source, s.class()) {
                            (FxStateSource::Shader(_), StateClass::PixelShader | StateClass::VertexShader) => {}
                            other => panic!("{name}.{}.{}: state {} with {other:?}: the passes of the game's effects assign shaders only", t.name, pass.name, s.name()),
                        }
                    }
                }
            }
            eprintln!("{name}: {ps} pixel shaders, {vs} vertex shaders, {} parameters, {} objects, header shader count {}", fx.parameters.len(), fx.objects.len(), fx.shader_count);
        }
    }

    #[test]
    fn rejects_garbage() {
        assert!(parse(&[]).is_err());
        assert!(parse(&[1, 2, 3, 4, 5, 6, 7, 8, 9]).is_err());
        let mut b = 0xFEFF_0901u32.to_le_bytes().to_vec();
        b.extend_from_slice(&0u32.to_le_bytes());
        b.extend_from_slice(&[0xFF; 16]);
        assert!(parse(&b).is_err());
    }
}
