//! `Terraria.WorldGen` (decompile/Terraria/Terraria/WorldGen.cs), split by topic.

pub mod tile_frame;
pub mod tile_frame_cosmetic;
pub mod tile_queries;
pub mod tree_foliage;
pub mod water_check;

pub use tile_frame::{reframe, section_tile_frame, section_tile_frame_with_check, tile_frame};
pub use tile_frame_cosmetic::tile_frame_cosmetic;
