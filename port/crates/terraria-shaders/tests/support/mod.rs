//! Shared by the tests and the documentation generator: the game's real effects, and the documented test inputs
//! of the interpreter-versus-GPU comparison.
#![allow(dead_code)]

use std::path::PathBuf;

use terraria_formats::xnb::{read_xnb, Asset};
use terraria_host::gfx::fx_binary::{self, FxEffect, FxParameter, FxValue, ParamClass, ParamType};
use terraria_shaders::bytecode::{self, Shader};

pub const EFFECTS: [&str; 3] = ["PixelShader", "TileShader", "ScreenShader"];

pub struct Pass {
    pub effect: &'static str,
    pub technique: String,
    /// index inside the technique
    pub index: usize,
    pub name: String,
    pub pixel_bytes: Vec<u8>,
    pub pixel: Shader,
    pub vertex_bytes: Option<Vec<u8>>,
    pub vertex: Option<Shader>,
}

impl Pass {
    /// `PixelShader/003_ArmorColored`: unique even where the effect repeats a pass name.
    pub fn stem(&self) -> String {
        format!("{}/{:03}_{}", self.effect, self.index, self.name)
    }
    pub fn label(&self) -> String {
        format!("{}.{}[{}] {}", self.effect, self.technique, self.index, self.name)
    }
}

pub struct Loaded {
    pub name: &'static str,
    pub file_size: usize,
    pub fx: FxEffect,
    pub passes: Vec<Pass>,
}

/// The three effects of the player's install, or `None` (with a note) when it is absent.
pub fn load_effects() -> Option<Vec<Loaded>> {
    let content = terraria_host::content::install_dir().join("Content");
    let mut out = Vec::new();
    for name in EFFECTS {
        let path = content.join(format!("{name}.xnb"));
        if !path.is_file() {
            eprintln!("SKIPPED: no Terraria install ({} is missing; set TERRARIA_DIR)", path.display());
            return None;
        }
        let x = read_xnb(&std::fs::read(&path).unwrap()).unwrap();
        let Asset::Effect(e) = x.asset else { panic!("{name} is not an effect") };
        let fx = fx_binary::parse_full(&e.bytecode).unwrap_or_else(|err| panic!("{name}: {err}"));
        let mut passes = Vec::new();
        for t in &fx.techniques {
            for (index, p) in t.passes.iter().enumerate() {
                let pixel_bytes = p.pixel_shader().unwrap_or_else(|| panic!("{name}.{}: no pixel shader", p.name)).to_vec();
                let pixel = bytecode::parse(&pixel_bytes).unwrap_or_else(|err| panic!("{name}.{} ps: {err}", p.name));
                let vertex_bytes = p.vertex_shader().map(|b| b.to_vec());
                let vertex = vertex_bytes.as_ref().map(|b| bytecode::parse(b).unwrap_or_else(|err| panic!("{name}.{} vs: {err}", p.name)));
                passes.push(Pass { effect: name, technique: t.name.clone(), index, name: p.name.clone(), pixel_bytes, pixel, vertex_bytes, vertex });
            }
        }
        out.push(Loaded { name, file_size: e.bytecode.len(), fx, passes });
    }
    Some(out)
}

/// `port/crates/terraria-host/docs/effects`
pub fn docs_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..").join("terraria-host").join("docs").join("effects")
}

/// Size of the test target and of the texture on sampler 0.
pub const SIZE: u32 = 64;
/// Sizes of the textures on samplers 0..3.
pub const TEXTURE_SIZES: [u32; 4] = [64, 32, 16, 32];
/// Vertex colour of the test quad (RGBA bytes).
pub const VERTEX_COLOR: [u8; 4] = [230, 204, 179, 217];

/// The documented uniform values of the comparison, by parameter name. A parameter that is not listed gets the
/// default stored in the effect file when that is non-zero (`Epsilon`), else [`fallback_value`].
pub const TEST_VALUES: &[(&str, &[f32])] = &[
    ("uColor", &[0.9, 0.5, 0.2]),
    ("uSecondaryColor", &[0.2, 0.6, 0.9]),
    ("uOpacity", &[0.8]),
    ("uSaturation", &[0.7]),
    ("uRotation", &[0.3]),
    ("uTime", &[1.37]),
    ("uSourceRect", &[0.0, 0.0, 64.0, 64.0]),
    ("uDrawPosition", &[100.0, 80.0]),
    ("uTargetPosition", &[300.0, 200.0]),
    ("uLightSource", &[0.3, 0.4, 0.5]),
    ("uImageSize0", &[64.0, 64.0]),
    ("uImageSize1", &[32.0, 32.0]),
    ("uImageSize2", &[16.0, 16.0]),
    ("uImageSize3", &[32.0, 32.0]),
    ("uLegacyArmorSourceRect", &[0.0, 0.0, 64.0, 64.0]),
    ("uLegacyArmorSheetSize", &[64.0, 64.0]),
    ("uShaderSpecificData", &[0.3, 0.6, 0.8, 0.2]),
    ("uScreenResolution", &[64.0, 64.0]),
    ("uScreenPosition", &[1000.0, 500.0]),
    ("uIntensity", &[0.75]),
    ("uProgress", &[0.4]),
    ("uImageOffset", &[0.1, 0.2]),
    ("uSceneSize", &[64.0, 64.0]),
    ("uSceneOffset", &[3.0, 5.0]),
    ("uZoom", &[1.0, 1.0]),
    ("uMultiChunkScene", &[0.0]),
    ("leafMinHue", &[0.15]),
    ("leafMaxHue", &[0.55]),
    ("leafMinSat", &[0.2]),
    ("leafMaxSat", &[0.9]),
    ("leafHueTestOffset", &[0.1]),
    ("invertSpecialGroupResult", &[0.0]),
    // pixels (0..64, 0..64) -> clip space, as XNA's Matrix (row vector convention): x' = 2x/64 - 1, y' = 1 - 2y/64
    ("MatrixTransform", &[2.0 / 64.0, 0.0, 0.0, 0.0, 0.0, -2.0 / 64.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, -1.0, 1.0, 0.0, 1.0]),
];

/// Values for a parameter the table does not name: 0.25, 0.5, 0.75, 0.375, ... per component.
pub fn fallback_value(n: usize) -> Vec<f32> {
    (0..n).map(|k| [0.25, 0.5, 0.75, 0.375][k % 4]).collect()
}

/// The test value of an effect parameter (flat floats), or `None` for samplers / textures.
pub fn test_value(p: &FxParameter) -> Option<Vec<f32>> {
    if p.ty.class == ParamClass::Object || p.ty.class == ParamClass::Struct {
        return None;
    }
    let n = (p.ty.components() * p.ty.elements.max(1)) as usize;
    if let Some((_, v)) = TEST_VALUES.iter().find(|(name, _)| *name == p.name()) {
        // uDirection is a float in PixelShader and a float2 in ScreenShader: values are cut or repeated to the shape
        return Some((0..n).map(|k| v[k % v.len()]).collect());
    }
    if p.name() == "uDirection" {
        return Some((0..n).map(|k| [1.0, 0.5][k % 2]).collect());
    }
    let default = p.value.floats(p.ty.ty).unwrap_or_default();
    if default.iter().any(|v| *v != 0.0) {
        return Some(default);
    }
    Some(fallback_value(n))
}

/// The parameter lookup `terraria_shaders::uniforms::fill` wants, over the test values.
pub fn test_parameters(fx: &FxEffect) -> impl Fn(&str) -> Option<Vec<f32>> + '_ {
    move |name| fx.parameter(name).and_then(test_value)
}

/// The stored default of a parameter as text.
pub fn default_text(p: &FxParameter) -> String {
    match &p.value {
        FxValue::Numeric(raw) => {
            let v: Vec<String> = raw
                .iter()
                .map(|w| match p.ty.ty {
                    ParamType::Bool => (*w != 0).to_string(),
                    ParamType::Int => (*w as i32).to_string(),
                    _ => format!("{}", f32::from_bits(*w)),
                })
                .collect();
            if v.iter().all(|s| s == "0" || s == "false") && v.len() > 4 {
                format!("all {}", v[0])
            } else {
                v.join(", ")
            }
        }
        FxValue::Objects(ids) => format!("object {}", ids.iter().map(|i| i.to_string()).collect::<Vec<_>>().join(", ")),
        FxValue::Samplers(s) => {
            if s.iter().all(|x| x.is_empty()) {
                "no sampler states".to_owned()
            } else {
                s.iter().flatten().map(|st| format!("{}[{}]", st.name(), st.index)).collect::<Vec<_>>().join(", ")
            }
        }
    }
}

/// Deterministic test texture for sampler `unit`, `size` x `size` RGBA8.
/// `smooth`: low-frequency sines (for the bilinear run: neighbouring texels differ by a few steps);
/// otherwise: per-texel hash noise (for the point-sampled run: any wrong texel shows).
/// Sampler 0 looks like a sprite: its alpha falls to zero towards two corners.
pub fn test_texture(unit: usize, size: u32, smooth: bool) -> Vec<u8> {
    let mut out = Vec::with_capacity((size * size * 4) as usize);
    for y in 0..size {
        for x in 0..size {
            for ch in 0..4u32 {
                let v = if smooth {
                    let (fx, fy) = (x as f32 / size as f32, y as f32 / size as f32);
                    let phase = unit as f32 * 1.3 + ch as f32 * 0.9;
                    let s = 0.5 + 0.25 * (fx * 5.1 + phase).sin() + 0.25 * (fy * 4.3 - phase * 0.7).cos();
                    if ch == 3 && unit == 0 {
                        (1.6 - 1.4 * ((fx - 0.5).abs() + (fy - 0.5).abs())).clamp(0.0, 1.0) * 255.0
                    } else {
                        s.clamp(0.0, 1.0) * 255.0
                    }
                } else {
                    // xorshift-style integer hash of (unit, x, y, channel)
                    let mut h = (x.wrapping_mul(0x9E37_79B1)) ^ (y.wrapping_mul(0x85EB_CA6B)) ^ ((unit as u32 * 4 + ch).wrapping_mul(0xC2B2_AE35));
                    h ^= h >> 15;
                    h = h.wrapping_mul(0x2C1B_3C6D);
                    h ^= h >> 12;
                    h = h.wrapping_mul(0x297A_2D39);
                    h ^= h >> 15;
                    (h & 0xFF) as f32
                };
                out.push(v as u8);
            }
        }
    }
    out
}
