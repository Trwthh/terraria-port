//! Adapters for parts of the game that live in `terraria-sim` / `terraria-core` but are not there yet.
//!
//! Every function here has the name of the original it stands for and is called from the drawing code at the
//! place the original calls it. Swapping to the real one is a one-line change per function (the body becomes a
//! call into `terraria_sim`). Functions that are pure and small are ported literally (marked "literal port"):
//! they belong in `terraria-sim` / `terraria-core` and are only here because those crates were not ours to edit.

use terraria_core::xna::{Color, Vector3};
use terraria_sim::unported;
use terraria_sim::Game;

// ---------------------------------------------------------------------------------------------------------
// Terraria.Graphics.VertexColors and Terraria.Lighting: the real ones in terraria-sim (swapped in).
// ---------------------------------------------------------------------------------------------------------

pub use terraria_sim::graphics::light::LightMode;
pub use terraria_sim::graphics::VertexColors;

/// Debug switch (`TERRARIA_UNLIT=1` in main.rs, not in the original): every light lookup answers full white, which shows
/// the tiles in places the game would leave black (caves without a light source) while there is no player to carry a torch.
pub static UNLIT: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

fn unlit() -> bool {
    UNLIT.load(std::sync::atomic::Ordering::Relaxed)
}

/// `Lighting.GetColor(x, y)` (Lighting.cs:222)
pub fn lighting_get_color(g: &Game, x: i32, y: i32) -> Color {
    if unlit() {
        return Color::WHITE;
    }
    terraria_sim::lighting::get_color(g, x, y)
}

/// `Lighting.GetCornerColors(x, y, out VertexColors vertices, float scale = 1f)` (Lighting.cs:297)
pub fn lighting_get_corner_colors(g: &Game, x: i32, y: i32) -> VertexColors {
    if unlit() {
        return VertexColors::new(Color::WHITE);
    }
    terraria_sim::lighting::get_corner_colors(g, x, y, 1.0)
}

/// `Lighting.GetColor9Slice(x, y, ref Vector3[] slices)` (Lighting.cs:284)
pub fn lighting_get_color_9_slice(g: &Game, x: i32, y: i32, slices: &mut [Vector3; 9]) {
    if unlit() {
        *slices = [Vector3::ONE; 9];
        return;
    }
    terraria_sim::lighting::get_color9_slice_vector3(g, x, y, slices);
}

/// `Lighting.GetColor4Slice(x, y, ref Vector3[] slices)` (Lighting.cs:548)
pub fn lighting_get_color_4_slice(g: &Game, x: i32, y: i32, slices: &mut [Vector3; 9]) {
    if unlit() {
        *slices = [Vector3::ONE; 9];
        return;
    }
    terraria_sim::lighting::get_color4_slice_vector3(g, x, y, slices);
}

/// `Lighting.GetColor9Slice(x, y, ref Color[] slices)` (Lighting.cs): the `Color` overload `Main.DrawBackground*` uses.
pub fn lighting_get_color_9_slice_colors(g: &Game, x: i32, y: i32, slices: &mut [Color; 9]) {
    if unlit() {
        *slices = [Color::WHITE; 9];
        return;
    }
    terraria_sim::lighting::get_color9_slice(g, x, y, slices);
}

/// `Lighting.GetColor4Slice(x, y, ref Color[] slices)` (Lighting.cs): the `Color` overload `Main.DrawBackground*` uses.
pub fn lighting_get_color_4_slice_colors(g: &Game, x: i32, y: i32, slices: &mut [Color; 9]) {
    if unlit() {
        *slices = [Color::WHITE; 9];
        return;
    }
    terraria_sim::lighting::get_color4_slice(g, x, y, slices);
}

/// `Lighting.GetCornerColors(x, y, out VertexColors vertices, float scale)` (Lighting.cs:297) with the scale argument
pub fn lighting_get_corner_colors_scaled(g: &Game, x: i32, y: i32, scale: f32) -> VertexColors {
    if unlit() {
        return VertexColors::new(Color::WHITE);
    }
    terraria_sim::lighting::get_corner_colors(g, x, y, scale)
}

/// `Main.LocalPlayer.Center` as `Main.UpdateSceneMetrics` reads it (Main.cs:63538 `trackedPosition = LocalPlayer.Center`).
/// SWAP: `Player` is not ported, so the perspective position is the centre of the screen (`screenPosition + (screenWidth / 2, screenHeight / 2)`).
pub fn main_local_player_center(g: &Game) -> terraria_core::xna::Vector2 {
    terraria_core::xna::Vector2::new(g.main.screen_position.0 + g.main.screen_width as f32 / 2.0, g.main.screen_position.1 + g.main.screen_height as f32 / 2.0)
}

/// `Lighting.Mode`
pub fn lighting_mode(g: &Game) -> LightMode {
    terraria_sim::lighting::mode(g)
}

/// `Lighting.NotRetro` (Lighting.cs:65)
pub fn lighting_not_retro(g: &Game) -> bool {
    terraria_sim::lighting::not_retro(g)
}

/// `Lighting.UpdateEveryFrame` (Lighting.cs:79)
pub fn lighting_update_every_frame(g: &Game) -> bool {
    terraria_sim::lighting::update_every_frame(g)
}

/// `Lighting.Brightness(x, y)` (Lighting.cs:118)
pub fn lighting_brightness(g: &Game, x: i32, y: i32) -> f32 {
    terraria_sim::lighting::brightness(g, x, y)
}

/// `Lighting.LightTiles(Rectangle)` (Lighting.cs:100)
pub fn lighting_light_tiles(g: &mut Game, area: terraria_core::xna::Rectangle) {
    terraria_sim::lighting::light_tiles(g, area);
}

// ---------------------------------------------------------------------------------------------------------
// Main.ColorOfTheSkies (set by Main.SetBackColor from the time of day and the biome influences, not ported)
// SWAP: read `g.main.color_of_the_skies` once the sky colour code is ported.
// ---------------------------------------------------------------------------------------------------------

/// `Main.ColorOfTheSkies` (Main.cs:2678), written by `SetBackColor` / `UpdateAtmosphereTransparencyToSkyColor` (terraria-sim `sky_gen`).
pub fn main_color_of_the_skies(g: &Game) -> Color {
    g.main.color_of_the_skies
}

/// `SkyManager.Instance.ProcessTileColor(Color)`. Neutral: the colour unchanged.
pub fn sky_manager_process_tile_color(_g: &Game, color: Color) -> Color {
    unported!("SkyManager.Instance.ProcessTileColor", color)
}

// ---------------------------------------------------------------------------------------------------------
// Main.sectionManager + WorldGen.SectionTileFrameWithCheck
// SWAP: `terraria_sim::world_gen::section_tile_frame_with_check(g, ...)` once `Main.sectionManager` is ported there
// (today that function frames nothing because it reports every section as not loaded).
// ---------------------------------------------------------------------------------------------------------

/// Stand-in for `Main.sectionManager` in single player after a world load: every section is loaded and not yet framed.
/// `SectionFramed(x, y)` is the `framed` flag, `AnyUnfinishedSections` is "some section is loaded but not framed".
#[derive(Default)]
pub struct SectionManager {
    framed: std::collections::HashSet<(i32, i32)>,
}

impl SectionManager {
    /// `Main.sectionManager.AnyUnfinishedSections`
    pub fn any_unfinished_sections(&self, g: &Game) -> bool {
        let sections_x = (g.main.max_tiles_x + 199) / 200;
        let sections_y = (g.main.max_tiles_y + 149) / 150;
        (self.framed.len() as i32) < sections_x * sections_y
    }

    /// `WorldGen.SectionTileFrameWithCheck(startX, startY, endX, endY)` (WorldGen.cs:81122), with `SectionLoaded` true and
    /// `SectionFramed` read from this stand-in. The framing itself is the real `WorldGen.SectionTileFrame`.
    pub fn section_tile_frame_with_check(&mut self, g: &mut Game, start_x: i32, start_y: i32, end_x: i32, end_y: i32) {
        let section_x = start_x / 200;
        let section_y = start_y / 150;
        let section_x2 = end_x / 200;
        let section_y2 = end_y / 150;
        for i in section_x..=section_x2 {
            for j in section_y..=section_y2 {
                if !self.framed.contains(&(i, j)) {
                    terraria_sim::world_gen::section_tile_frame(g, i, j, i, j);
                    self.framed.insert((i, j));
                    if !self.any_unfinished_sections(g) {
                        return;
                    }
                }
            }
        }
    }
}

/// `WorldGen.RefreshSections(startX, startY, endX, endY)` (WorldGen.cs:81144), called when `Main.sectionManager.AnyNeedRefresh`.
pub fn world_gen_refresh_sections(_g: &mut Game, _start_x: i32, _start_y: i32, _end_x: i32, _end_y: i32) {
    unported!("WorldGen.RefreshSections");
}

// ---------------------------------------------------------------------------------------------------------
// Main statics that are simulation state but not in `MainState` yet.
// SWAP: read `g.main.<field>` once they exist (`remixWorld` is there already).
// ---------------------------------------------------------------------------------------------------------

/// `Main.bloodMoon`
pub fn main_blood_moon(g: &Game) -> bool {
    g.main.blood_moon
}
/// `Main.raining`
pub fn main_raining(g: &Game) -> bool {
    g.main.raining
}
/// `Main.cloudAlpha`
pub fn main_cloud_alpha(g: &Game) -> f32 {
    g.main.cloud_alpha
}
/// `Main.moonPhase`
pub fn main_moon_phase(g: &Game) -> i32 {
    g.main.moon_phase
}
/// `Main.remixWorld`
pub fn main_remix_world(g: &Game) -> bool {
    g.main.remix_world
}

// ---------------------------------------------------------------------------------------------------------
// Literal ports that moved to terraria-sim (`world_gen::tile_queries`, `Tile::act_color` in terraria-core), under the
// names the drawing code already calls them by.
// ---------------------------------------------------------------------------------------------------------

pub use terraria_sim::world_gen::tile_queries::{
    get_biome_influence as world_gen_get_biome_influence, get_cactus_type as world_gen_get_cactus_type, in_a_place_with_wind as world_gen_in_a_place_with_wind,
    is_palm_oasis_tree as world_gen_is_palm_oasis_tree, is_rope_out as world_gen_is_rope_out, solid_tile as world_gen_solid_tile, solid_tile_tile as world_gen_solid_tile_tile,
    BEACH_DISTANCE as WORLD_GEN_BEACH_DISTANCE,
};

/// `WorldGen.IsAlchemyPlantHarvestable(int style, int y)` (WorldGen.cs:66758). Literal port; the weather fields are adapters above.
pub fn world_gen_is_alchemy_plant_harvestable(g: &Game, style: i32, y: i32) -> bool {
    if style == 0 && g.main.day_time {
        return true;
    }
    if style == 1 && !g.main.day_time {
        return true;
    }
    if style == 3 && !g.main.day_time && (main_blood_moon(g) || main_moon_phase(g) == 0) {
        return true;
    }
    if style == 4 && (main_raining(g) || main_cloud_alpha(g) > 0.0) {
        return true;
    }
    if style == 5 {
        let mut flag = (y as f64) > g.main.world_surface;
        if main_remix_world(g) {
            let num = g.main.max_tiles_y - 350;
            flag = y < num;
        }
        if (!main_raining(g) || flag) && g.main.time > 40500.0 {
            return true;
        }
    }
    false
}

// ---------------------------------------------------------------------------------------------------------
// Terraria.Utils (Utils.cs): literal ports of the small float helpers the tile drawing uses. They belong in terraria-core.
// ---------------------------------------------------------------------------------------------------------

/// `Utils.GetLerpValue(float from, float to, float t, bool clamped = false)` (Utils.cs:315)
pub fn utils_get_lerp_value(from: f32, to: f32, t: f32, clamped: bool) -> f32 {
    if clamped {
        if from < to {
            if t < from {
                return 0.0;
            }
            if t > to {
                return 1.0;
            }
        } else {
            if t < to {
                return 1.0;
            }
            if t > from {
                return 0.0;
            }
        }
    }
    (t - from) / (to - from)
}

/// `Utils.Remap(float fromValue, float fromMin, float fromMax, float toMin, float toMax, bool clamped = true)` (Utils.cs:345)
pub fn utils_remap(from_value: f32, from_min: f32, from_max: f32, to_min: f32, to_max: f32) -> f32 {
    terraria_core::xna::math_helper::lerp(to_min, to_max, utils_get_lerp_value(from_min, from_max, from_value, true))
}

/// `Utils.WrappedLerp(float value1, float value2, float percent)` (Utils.cs:305)
pub fn utils_wrapped_lerp(value1: f32, value2: f32, percent: f32) -> f32 {
    let mut num = percent * 2.0;
    if num > 1.0 {
        num = 2.0 - num;
    }
    terraria_core::xna::math_helper::lerp(value1, value2, num)
}
