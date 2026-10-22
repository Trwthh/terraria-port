//! `Terraria.Graphics.Light.LightMode` (LightMode.cs).

/// The lighting modes of `Lighting.Mode`. Only `Color` is ported; the other three are `LegacyLighting`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i32)]
pub enum LightMode {
    White = 0,
    Retro = 1,
    Trippy = 2,
    Color = 3,
}
