//! `Terraria.Graphics.Light.LightMaskMode` (LightMaskMode.cs).

/// How a tile attenuates light that passes through it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
#[repr(u8)]
pub enum LightMaskMode {
    #[default]
    None = 0,
    Solid = 1,
    Water = 2,
    Honey = 3,
    CrackedBricks = 4,
}
