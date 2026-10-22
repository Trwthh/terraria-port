//! Direct3D 9 shader bytecode -> WGSL, for the three compiled XNA effects of Terraria 1.4.5.8.
//!
//! - [`bytecode`]: the token stream decoder (`ps_2_0` / `vs_2_0`; shader model 3 tokens decode too);
//! - [`ctab`]: the constant table (uniform name -> register);
//! - [`preshader`]: the D3DX preshader (CPU program that fills constant registers from effect parameters);
//! - [`disasm`]: assembly listing;
//! - [`uniforms`]: effect parameter values -> constant registers (constant table packing rules + preshader);
//! - [`wgsl`]: the translator;
//! - [`interp`]: a CPU reference interpreter with a software texture sampler, used to test the WGSL path.
//!
//! The crate has no dependencies and uses no `crate::` paths, so the app can also compile it as a module
//! (`#[path]`) until it is added to the app's manifest. `docs/effects/README.md` in `terraria-host` has the
//! coverage tables and the list of deviations.

pub mod bytecode;
pub mod check;
pub mod ctab;
pub mod disasm;
pub mod interp;
pub mod preshader;
pub mod uniforms;
pub mod wgsl;
