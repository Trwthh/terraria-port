//! The simulation: game rules ported from the decompile, no rendering. See `port/PORTING.md`.
#![allow(clippy::too_many_arguments, clippy::collapsible_if, clippy::collapsible_else_if, clippy::needless_range_loop)]

pub mod cloud;
pub mod collision;
pub mod game;
pub mod graphics;
pub mod io;
pub mod light_support;
pub mod lighting;
pub mod liquid;
pub mod liquid_buffer;
pub mod liquid_tile_data;
pub mod main_time;
pub mod player_biomes;
pub mod scene_metrics;
pub mod sky_gen;
pub mod star;
pub mod world_gen_bg;
pub mod unported;
pub mod world_gen;

pub use game::Game;
