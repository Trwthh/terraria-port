//! `Terraria.Graphics.Light.ILightingEngine` (ILightingEngine.cs).

use terraria_core::xna::{Rectangle, Vector3};

use crate::game::Game;

/// The interface `Lighting` talks to. `LightingEngine` implements it; `LegacyLighting` (not ported) is the other
/// implementation. `ProcessArea` reads and writes `Main` statics, so it takes the game state.
pub trait ILightingEngine {
    fn rebuild(&mut self);
    fn add_light(&mut self, x: i32, y: i32, color: Vector3);
    fn process_area(&mut self, g: &mut Game, area: Rectangle);
    fn get_color(&self, x: i32, y: i32) -> Vector3;
    fn clear(&mut self);
}
