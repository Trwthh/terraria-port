//! `Terraria.Lighting` (Lighting.cs): the static facade over the lighting engines. Only `LightMode.Color`
//! (`LightingEngine`) is ported; every path that would reach `LegacyLighting` (Retro, Trippy, White) calls
//! `unported!("LegacyLighting.X")` with the neutral value noted at the call.
//!
//! The statics of `Lighting` live in `LightingState` (`Game::lighting`). The state is the one `Main.Initialize`
//! leaves behind: `Lighting.Initialize()` has run, so the mode is `Color` and `OffScreenTiles` is 35.
//!
//! Overloads: `GetColor(int, int)` is `get_color`; `GetColor(int, int, Color)` is `get_color_tinted`;
//! `GetColor(Point)` is `get_color_point`; `GetColor(Point, Color)` is `get_color_point_tinted`;
//! `GetColor9Slice` is `get_color9_slice` (`Color[]`) and `get_color9_slice_vector3` (`Vector3[]`), likewise
//! `get_color4_slice` / `get_color4_slice_vector3`; `AddLight(int, int, float, float, float)` is `add_light`,
//! `AddLight(Vector2, Vector3)` is `add_light_vector`, `AddLight(Vector2, float, float, float)` is
//! `add_light_position`, `AddLight(int, int, int torchID, float)` is `add_light_torch` and
//! `AddLight(Vector2, int torchID)` is `add_light_position_torch`.

use terraria_core::xna::{Color, Point, Rectangle, Vector2, Vector3};

use crate::game::Game;
use crate::graphics::light::{ILightingEngine, LightMode, LightingEngine};
use crate::graphics::VertexColors;
use crate::light_support::torch_color;
use crate::unported;

/// Which `ILightingEngine` the static `_activeEngine` points at.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ActiveEngine {
    /// `NewEngine`
    New,
    /// `LegacyEngine` (not ported)
    Legacy,
}

/// Static fields of `Terraria.Lighting`.
pub struct LightingState {
    /// `Lighting.OffScreenTiles` (declared 45; the `Color` mode setter sets 35)
    pub off_screen_tiles: i32,
    /// `Lighting._mode`
    mode: LightMode,
    /// `Lighting.NewEngine`. Taken out while it runs because it needs `&mut Game`.
    new_engine: Option<Box<LightingEngine>>,
    /// `Lighting._activeEngine`; `None` is the original's `null` before `Initialize`.
    active_engine: Option<ActiveEngine>,
    /// `Lighting.GlobalBrightness`
    pub global_brightness: f32,
}

impl LightingState {
    /// The state after `Main.Initialize_AlmostEverything` -> `Lighting.Initialize()` (Main.cs:6774):
    /// `GlobalBrightness = 1.2f`, engines rebuilt, `_activeEngine == null` so `Mode = LightMode.Color`.
    /// (The setter's writes to `Main.renderCount` / `Main.renderNow` are in `Game::new`.)
    pub fn new() -> LightingState {
        LightingState {
            off_screen_tiles: 35,
            mode: LightMode::Color,
            new_engine: Some(Box::new(LightingEngine::new())),
            active_engine: Some(ActiveEngine::New),
            global_brightness: 1.2f32,
        }
    }

    /// The engine, for tests and tools (the original keeps it private).
    pub fn new_engine(&self) -> &LightingEngine {
        self.new_engine.as_ref().expect("LightingEngine is only taken out while it runs")
    }

    /// The engine, for tests and tools (the original keeps it private).
    pub fn new_engine_mut(&mut self) -> &mut LightingEngine {
        self.new_engine.as_mut().expect("LightingEngine is only taken out while it runs")
    }
}

impl Default for LightingState {
    fn default() -> Self {
        LightingState::new()
    }
}

/// `Lighting.DEFAULT_GLOBAL_BRIGHTNESS`
pub const DEFAULT_GLOBAL_BRIGHTNESS: f32 = 1.2f32;
/// `Lighting.BLIND_GLOBAL_BRIGHTNESS`
pub const BLIND_GLOBAL_BRIGHTNESS: f32 = 1f32;

/// `Lighting.GlobalBrightness` getter
pub fn global_brightness(g: &Game) -> f32 {
    g.lighting.global_brightness
}

/// `Lighting.GlobalBrightness` setter
pub fn set_global_brightness(g: &mut Game, value: f32) {
    g.lighting.global_brightness = value;
}

/// `Lighting.Mode` getter
pub fn mode(g: &Game) -> LightMode {
    g.lighting.mode
}

/// `Lighting.Mode` setter (Lighting.cs:29)
pub fn set_mode(g: &mut Game, value: LightMode) {
    g.lighting.mode = value;
    match g.lighting.mode {
        LightMode::Color => {
            g.lighting.active_engine = Some(ActiveEngine::New);
            unported!("LegacyLighting.Mode"); // LegacyEngine.Mode = 0
            g.lighting.off_screen_tiles = 35;
        }
        LightMode::White => {
            g.lighting.active_engine = Some(ActiveEngine::Legacy);
            unported!("LegacyLighting.Mode"); // LegacyEngine.Mode = 1
        }
        LightMode::Retro => {
            g.lighting.active_engine = Some(ActiveEngine::Legacy);
            unported!("LegacyLighting.Mode"); // LegacyEngine.Mode = 2
        }
        LightMode::Trippy => {
            g.lighting.active_engine = Some(ActiveEngine::Legacy);
            unported!("LegacyLighting.Mode"); // LegacyEngine.Mode = 3
        }
    }
    g.main.render_count = 0;
    g.main.render_now = false;
}

/// `Lighting.NotRetro`
pub fn not_retro(g: &Game) -> bool {
    if mode(g) != LightMode::Retro {
        return mode(g) != LightMode::Trippy;
    }
    false
}

/// `Lighting.UsingNewLighting`
pub fn using_new_lighting(g: &Game) -> bool {
    mode(g) == LightMode::Color
}

/// `Lighting.UpdateEveryFrame`
pub fn update_every_frame(g: &Game) -> bool {
    // Main.RenderTargetsRequired depends on GameZoomTarget / GameViewMatrix (not ported); at the default zoom of 1 it is false.
    if !unported!("Main.RenderTargetsRequired", false) {
        return !not_retro(g);
    }
    false
}

/// `Lighting.Initialize()` (Lighting.cs:89)
pub fn initialize(g: &mut Game) {
    g.lighting.global_brightness = 1.2f32;
    g.lighting.new_engine_mut().rebuild();
    unported!("LegacyLighting.Rebuild");
    if g.lighting.active_engine.is_none() {
        set_mode(g, LightMode::Color);
    }
}

/// `_activeEngine.GetColor(x, y)`. A missing engine is the original's `NullReferenceException`.
fn engine_get_color(g: &Game, x: i32, y: i32) -> Vector3 {
    match g.lighting.active_engine.expect("Lighting.Initialize() has not run (_activeEngine is null)") {
        ActiveEngine::New => g.lighting.new_engine().get_color(x, y),
        // LegacyLighting.GetColor: neutral value is black.
        ActiveEngine::Legacy => unported!("LegacyLighting.GetColor", Vector3::ZERO),
    }
}

/// `_activeEngine.AddLight(x, y, color)`
fn engine_add_light(g: &mut Game, x: i32, y: i32, color: Vector3) {
    match g.lighting.active_engine.expect("Lighting.Initialize() has not run (_activeEngine is null)") {
        ActiveEngine::New => g.lighting.new_engine_mut().add_light(x, y, color),
        ActiveEngine::Legacy => unported!("LegacyLighting.AddLight"),
    }
}

/// `Lighting.LightTiles(Rectangle area)` (Lighting.cs:100). `TimeLogger` timing is left out.
pub fn light_tiles(g: &mut Game, area: Rectangle) {
    g.main.render = true;
    update_global_brightness(g);
    match g.lighting.active_engine.expect("Lighting.Initialize() has not run (_activeEngine is null)") {
        ActiveEngine::New => {
            let mut engine = g.lighting.new_engine.take().expect("LightingEngine is only taken out while it runs");
            engine.process_area(g, area);
            g.lighting.new_engine = Some(engine);
        }
        ActiveEngine::Legacy => unported!("LegacyLighting.ProcessArea"),
    }
}

/// `Lighting.UpdateGlobalBrightness()` (Lighting.cs:109)
fn update_global_brightness(g: &mut Game) {
    g.lighting.global_brightness = 1.2f32;
    // Main.player[Main.myPlayer].blind: players are not ported; a fresh player is not blind.
    if unported!("Player.blind", false) {
        g.lighting.global_brightness = 1f32;
    }
}

/// `Lighting.Brightness(int x, int y)` (Lighting.cs:118)
pub fn brightness(g: &Game, x: i32, y: i32) -> f32 {
    let color = engine_get_color(g, x, y);
    g.lighting.global_brightness * (color.x + color.y + color.z) / 3f32
}

/// `Lighting.GetSubLight(Vector2 position)` (Lighting.cs:124)
pub fn get_sub_light(g: &Game, position: Vector2) -> Vector3 {
    let vector = position / 16f32 - Vector2::new(0.5f32, 0.5f32);
    let vector2 = Vector2::new(vector.x % 1f32, vector.y % 1f32);
    let num = vector.x as i32;
    let num2 = vector.y as i32;
    let color = engine_get_color(g, num, num2);
    let color2 = engine_get_color(g, num + 1, num2);
    let color3 = engine_get_color(g, num, num2 + 1);
    let color4 = engine_get_color(g, num + 1, num2 + 1);
    let value = Vector3::lerp(color, color2, vector2.x);
    let value2 = Vector3::lerp(color3, color4, vector2.x);
    Vector3::lerp(value, value2, vector2.y)
}

/// `Lighting.AddLight(Vector2 position, Vector3 rgb)` (Lighting.cs:139)
pub fn add_light_vector(g: &mut Game, position: Vector2, rgb: Vector3) {
    add_light(g, (position.x / 16f32) as i32, (position.y / 16f32) as i32, rgb.x, rgb.y, rgb.z);
}

/// `Lighting.AddLight(Vector2 position, float r, float g, float b)` (Lighting.cs:144)
pub fn add_light_position(g: &mut Game, position: Vector2, r: f32, g_: f32, b: f32) {
    add_light(g, (position.x / 16f32) as i32, (position.y / 16f32) as i32, r, g_, b);
}

/// `Lighting.AddLight(int i, int j, int torchID, float lightAmount)` (Lighting.cs:149). Unlike the other
/// overloads it does not check `Main.gamePaused` / `Main.netMode`.
pub fn add_light_torch(g: &mut Game, i: i32, j: i32, torch_id: i32, light_amount: f32) {
    let (mut r, mut g_, mut b) = (0f32, 0f32, 0f32);
    torch_color(g, torch_id, &mut r, &mut g_, &mut b);
    engine_add_light(g, i, j, Vector3::new(r * light_amount, g_ * light_amount, b * light_amount));
}

/// `Lighting.AddLight(Vector2 position, int torchID)` (Lighting.cs:155). The tile coordinates are
/// `(int)position.X / 16` (cast first, then integer division), unlike the float overloads.
pub fn add_light_position_torch(g: &mut Game, position: Vector2, torch_id: i32) {
    let (mut r, mut g_, mut b) = (0f32, 0f32, 0f32);
    torch_color(g, torch_id, &mut r, &mut g_, &mut b);
    add_light(g, position.x as i32 / 16, position.y as i32 / 16, r, g_, b);
}

/// `Lighting.AddLight(int i, int j, float r, float g, float b)` (Lighting.cs:161)
pub fn add_light(g: &mut Game, i: i32, j: i32, r: f32, g_: f32, b: f32) {
    if !g.main.game_paused && g.main.net_mode != 2 {
        engine_add_light(g, i, j, Vector3::new(r, g_, b));
    }
}

/// `Lighting.NextLightMode()` (Lighting.cs:169). `Mode++` from `Color` passes through the undefined value 4,
/// whose setter only resets `renderCount` / `renderNow`; `Enum.IsDefined` fails and `Mode = White` repeats
/// those writes, so the net effect is `set_mode(White)`.
pub fn next_light_mode(g: &mut Game) {
    let next = match mode(g) {
        LightMode::White => LightMode::Retro,
        LightMode::Retro => LightMode::Trippy,
        LightMode::Trippy => LightMode::Color,
        LightMode::Color => LightMode::White,
    };
    set_mode(g, next);
    clear(g);
}

/// `Lighting.Clear()` (Lighting.cs:179)
pub fn clear(g: &mut Game) {
    match g.lighting.active_engine.expect("Lighting.Initialize() has not run (_activeEngine is null)") {
        ActiveEngine::New => g.lighting.new_engine_mut().clear(),
        ActiveEngine::Legacy => unported!("LegacyLighting.Clear"),
    }
}

/// `Lighting.GetColor(Point tileCoords)` (Lighting.cs:184)
pub fn get_color_point(g: &Game, tile_coords: Point) -> Color {
    if g.main.game_menu {
        return Color::WHITE;
    }
    Color::from_vector3(engine_get_color(g, tile_coords.x, tile_coords.y) * g.lighting.global_brightness)
}

/// `Lighting.GetColor(Point tileCoords, Color originalColor)` (Lighting.cs:193)
pub fn get_color_point_tinted(g: &Game, tile_coords: Point, original_color: Color) -> Color {
    if g.main.game_menu {
        return original_color;
    }
    Color::from_vector3(engine_get_color(g, tile_coords.x, tile_coords.y) * original_color.to_vector3())
}

/// `Lighting.GetColor(int x, int y, Color oldColor)` (Lighting.cs:202)
pub fn get_color_tinted(g: &Game, x: i32, y: i32, old_color: Color) -> Color {
    if g.main.game_menu {
        return old_color;
    }
    Color::from_vector3(engine_get_color(g, x, y) * old_color.to_vector3())
}

/// `Lighting.GetColorClamped(int x, int y, Color oldColor)` (Lighting.cs:211)
pub fn get_color_clamped(g: &Game, x: i32, y: i32, old_color: Color) -> Color {
    if g.main.game_menu {
        return old_color;
    }
    let mut color = engine_get_color(g, x, y);
    color = Vector3::min(Vector3::ONE, color);
    Color::from_vector3(color * old_color.to_vector3())
}

/// `Lighting.GetColor(int x, int y)` (Lighting.cs:222)
pub fn get_color(g: &Game, x: i32, y: i32) -> Color {
    if g.main.game_menu {
        return Color::WHITE;
    }
    let mut result = Color::default();
    let color = engine_get_color(g, x, y);
    let num = g.lighting.global_brightness * 255f32;
    let mut num2 = (color.x * num) as i32;
    let mut num3 = (color.y * num) as i32;
    let mut num4 = (color.z * num) as i32;
    if num2 > 255 {
        num2 = 255;
    }
    if num3 > 255 {
        num3 = 255;
    }
    if num4 > 255 {
        num4 = 255;
    }
    num4 <<= 16;
    num3 <<= 8;
    result.set_packed_value((num2 | num3 | num4 | -16777216) as u32);
    result
}

/// `Lighting.GetColor9Slice(int centerX, int centerY, ref Color[] slices)` (Lighting.cs:252)
pub fn get_color9_slice(g: &Game, center_x: i32, center_y: i32, slices: &mut [Color]) {
    let mut num = 0usize;
    let mut i = center_x - 1;
    while i <= center_x + 1 {
        let mut j = center_y - 1;
        while j <= center_y + 1 {
            let color = engine_get_color(g, i, j);
            let mut num2 = (255f32 * color.x * g.lighting.global_brightness) as i32;
            let mut num3 = (255f32 * color.y * g.lighting.global_brightness) as i32;
            let mut num4 = (255f32 * color.z * g.lighting.global_brightness) as i32;
            if num2 > 255 {
                num2 = 255;
            }
            if num3 > 255 {
                num3 = 255;
            }
            if num4 > 255 {
                num4 = 255;
            }
            num4 <<= 16;
            num3 <<= 8;
            slices[num].set_packed_value((num2 | num3 | num4 | -16777216) as u32);
            num += 3;
            j += 1;
        }
        num -= 8;
        i += 1;
    }
}

/// `Lighting.GetColor9Slice(int x, int y, ref Vector3[] slices)` (Lighting.cs:284)
pub fn get_color9_slice_vector3(g: &Game, x: i32, y: i32, slices: &mut [Vector3]) {
    slices[0] = engine_get_color(g, x - 1, y - 1) * g.lighting.global_brightness;
    slices[3] = engine_get_color(g, x - 1, y) * g.lighting.global_brightness;
    slices[6] = engine_get_color(g, x - 1, y + 1) * g.lighting.global_brightness;
    slices[1] = engine_get_color(g, x, y - 1) * g.lighting.global_brightness;
    slices[4] = engine_get_color(g, x, y) * g.lighting.global_brightness;
    slices[7] = engine_get_color(g, x, y + 1) * g.lighting.global_brightness;
    slices[2] = engine_get_color(g, x + 1, y - 1) * g.lighting.global_brightness;
    slices[5] = engine_get_color(g, x + 1, y) * g.lighting.global_brightness;
    slices[8] = engine_get_color(g, x + 1, y + 1) * g.lighting.global_brightness;
}

/// `(int)(sum * scale)` clamped above to 255, the packing used by `GetCornerColors`.
fn corner_pack(sum_r: f32, sum_g: f32, sum_b: f32, num: f32) -> u32 {
    let mut num2 = (sum_r * num) as i32;
    let mut num3 = (sum_g * num) as i32;
    let mut num4 = (sum_b * num) as i32;
    if num2 > 255 {
        num2 = 255;
    }
    if num3 > 255 {
        num3 = 255;
    }
    if num4 > 255 {
        num4 = 255;
    }
    num3 <<= 8;
    num4 <<= 16;
    (num2 | num3 | num4 | -16777216) as u32
}

/// `Lighting.GetCornerColors(int centerX, int centerY, out VertexColors vertices, float scale = 1f)` (Lighting.cs:297).
/// The four blocks of the original differ only in which neighbours are summed; each is written out with the
/// same additions in the same order.
pub fn get_corner_colors(g: &Game, center_x: i32, center_y: i32, scale: f32) -> VertexColors {
    let mut vertices = VertexColors::default();
    let color = engine_get_color(g, center_x, center_y);
    let color2 = engine_get_color(g, center_x, center_y - 1);
    let color3 = engine_get_color(g, center_x, center_y + 1);
    let color4 = engine_get_color(g, center_x - 1, center_y);
    let color5 = engine_get_color(g, center_x + 1, center_y);
    let color6 = engine_get_color(g, center_x - 1, center_y - 1);
    let color7 = engine_get_color(g, center_x + 1, center_y - 1);
    let color8 = engine_get_color(g, center_x - 1, center_y + 1);
    let color9 = engine_get_color(g, center_x + 1, center_y + 1);
    let num = g.lighting.global_brightness * scale * 63.75f32;
    vertices.top_left_color.set_packed_value(corner_pack(
        color2.x + color6.x + color4.x + color.x,
        color2.y + color6.y + color4.y + color.y,
        color2.z + color6.z + color4.z + color.z,
        num,
    ));
    vertices.top_right_color.set_packed_value(corner_pack(
        color2.x + color7.x + color5.x + color.x,
        color2.y + color7.y + color5.y + color.y,
        color2.z + color7.z + color5.z + color.z,
        num,
    ));
    vertices.bottom_left_color.set_packed_value(corner_pack(
        color3.x + color8.x + color4.x + color.x,
        color3.y + color8.y + color4.y + color.y,
        color3.z + color8.z + color4.z + color.z,
        num,
    ));
    vertices.bottom_right_color.set_packed_value(corner_pack(
        color3.x + color9.x + color5.x + color.x,
        color3.y + color9.y + color5.y + color.y,
        color3.z + color9.z + color5.z + color.z,
        num,
    ));
    vertices
}

/// `new Color((byte)num5, (byte)num6, (byte)num7, 255)` after `(int)(255f * c.X * GlobalBrightness)` clamped above to 255
fn slice_color(g: &Game, c: Vector3) -> Color {
    let mut num5 = (255f32 * c.x * g.lighting.global_brightness) as i32;
    let mut num6 = (255f32 * c.y * g.lighting.global_brightness) as i32;
    let mut num7 = (255f32 * c.z * g.lighting.global_brightness) as i32;
    if num5 > 255 {
        num5 = 255;
    }
    if num6 > 255 {
        num6 = 255;
    }
    if num7 > 255 {
        num7 = 255;
    }
    Color::new(num5 as u8, num6 as u8, num7 as u8, 255)
}

/// `Lighting.GetColor4Slice(int centerX, int centerY, ref Color[] slices)` (Lighting.cs:384)
pub fn get_color4_slice(g: &Game, center_x: i32, center_y: i32, slices: &mut [Color]) {
    let color = engine_get_color(g, center_x, center_y - 1);
    let color2 = engine_get_color(g, center_x, center_y + 1);
    let color3 = engine_get_color(g, center_x - 1, center_y);
    let color4 = engine_get_color(g, center_x + 1, center_y);
    let num = color.x + color.y + color.z;
    let num2 = color2.x + color2.y + color2.z;
    let num3 = color4.x + color4.y + color4.z;
    let num4 = color3.x + color3.y + color3.z;
    if num >= num4 {
        slices[0] = slice_color(g, color3);
    } else {
        slices[0] = slice_color(g, color);
    }
    if num >= num3 {
        slices[1] = slice_color(g, color4);
    } else {
        slices[1] = slice_color(g, color);
    }
    if num2 >= num4 {
        slices[2] = slice_color(g, color3);
    } else {
        slices[2] = slice_color(g, color2);
    }
    if num2 >= num3 {
        slices[3] = slice_color(g, color4);
    } else {
        slices[3] = slice_color(g, color2);
    }
}

/// `Lighting.GetColor4Slice(int x, int y, ref Vector3[] slices)` (Lighting.cs:548)
pub fn get_color4_slice_vector3(g: &Game, x: i32, y: i32, slices: &mut [Vector3]) {
    let color = engine_get_color(g, x, y - 1);
    let color2 = engine_get_color(g, x, y + 1);
    let color3 = engine_get_color(g, x - 1, y);
    let color4 = engine_get_color(g, x + 1, y);
    let num = color.x + color.y + color.z;
    let num2 = color2.x + color2.y + color2.z;
    let num3 = color4.x + color4.y + color4.z;
    let num4 = color3.x + color3.y + color3.z;
    if num >= num4 {
        slices[0] = color3 * g.lighting.global_brightness;
    } else {
        slices[0] = color * g.lighting.global_brightness;
    }
    if num >= num3 {
        slices[1] = color4 * g.lighting.global_brightness;
    } else {
        slices[1] = color * g.lighting.global_brightness;
    }
    if num2 >= num4 {
        slices[2] = color3 * g.lighting.global_brightness;
    } else {
        slices[2] = color2 * g.lighting.global_brightness;
    }
    if num2 >= num3 {
        slices[3] = color4 * g.lighting.global_brightness;
    } else {
        slices[3] = color2 * g.lighting.global_brightness;
    }
}

/// `Lighting.AddGameplaySnapshotComponents()` (Lighting.cs:592): forwards to `NewEngine`.
pub fn add_gameplay_snapshot_components(g: &Game) {
    g.lighting.new_engine().add_gameplay_snapshot_components();
}
