//! `Terraria.Graphics.Light` (decompile/Terraria/Terraria/Graphics/Light/), the `LightMode.Color` engine.
//! `LegacyLighting.cs` (Retro / Trippy / White) is not ported.

pub mod i_lighting_engine;
pub mod light_map;
pub mod light_mask_mode;
pub mod light_mode;
pub mod lighting_engine;
pub mod tile_light_scanner;
pub mod tile_light_scanner_options;

pub use i_lighting_engine::ILightingEngine;
pub use light_map::LightMap;
pub use light_mask_mode::LightMaskMode;
pub use light_mode::LightMode;
pub use lighting_engine::LightingEngine;
pub use tile_light_scanner::TileLightScanner;
pub use tile_light_scanner_options::TileLightScannerOptions;
