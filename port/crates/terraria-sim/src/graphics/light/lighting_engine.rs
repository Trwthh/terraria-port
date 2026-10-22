//! `Terraria.Graphics.Light.LightingEngine` (LightingEngine.cs): the `LightMode.Color` engine. One call of
//! `process_area` runs one step of a four-step cycle (minimap, scene metrics, scan, blur); the light map seen by
//! `get_color` is replaced when the blur step finishes.
//!
//! Not ported (called through `unported!` where the original calls them): `Main.UpdateSceneMetrics`,
//! `Main.Map.UpdateLighting`, `MapHelper.sceneArea` writers, the `Player` flags of
//! `Main.SceneMetrics.PerspectivePlayer`, `CaptureManager.Instance.IsCapturing`, `StateSnapshot`.
//! `TimeLogger` (profiling timers) is left out: it has no effect on the game.

use terraria_core::xna::{Point, Rectangle, Vector3};

use super::i_lighting_engine::ILightingEngine;
use super::light_map::LightMap;
use super::tile_light_scanner::TileLightScanner;
use super::tile_light_scanner_options::TileLightScannerOptions;
use crate::game::Game;
use crate::unported;

/// `LightingEngine.EngineState`
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum EngineState {
    MinimapUpdate = 0,
    ExportMetrics = 1,
    Scan = 2,
    Blur = 3,
}

/// `LightingEngine.PerFrameLight`
#[derive(Clone, Copy, Debug)]
struct PerFrameLight {
    position: Point,
    color: Vector3,
}

/// `LightingEngine`
pub struct LightingEngine {
    /// `_perFrameLightSwapUpdateCount`
    per_frame_light_swap_update_count: u32,
    /// `_perFrameLights`
    per_frame_lights: Vec<PerFrameLight>,
    /// `_oldPerFrameLights`
    old_per_frame_lights: Vec<PerFrameLight>,
    /// `_tileScanner`
    tile_scanner: TileLightScanner,
    /// `_activeLightMap`
    active_light_map: LightMap,
    /// `_activeProcessedArea`
    active_processed_area: Rectangle,
    /// `_workingLightMap`
    working_light_map: LightMap,
    /// `_workingProcessedArea`
    working_processed_area: Rectangle,
    /// `_state`
    state: EngineState,
}

impl LightingEngine {
    /// `AREA_PADDING`
    pub const AREA_PADDING: i32 = 28;

    /// Field initialisers of `LightingEngine`.
    pub fn new() -> LightingEngine {
        LightingEngine {
            per_frame_light_swap_update_count: 0,
            per_frame_lights: Vec::new(),
            old_per_frame_lights: Vec::new(),
            tile_scanner: TileLightScanner::new(),
            active_light_map: LightMap::new(),
            active_processed_area: Rectangle::EMPTY,
            working_light_map: LightMap::new(),
            working_processed_area: Rectangle::EMPTY,
            state: EngineState::MinimapUpdate,
        }
    }

    /// Makes runs repeatable. The original seeds `TileLightScanner._random` and both `LightMap._random` from a
    /// GUID (`FastRandom.CreateWithRandomSeed()`); this has no counterpart in the original.
    pub fn set_random_seeds(&mut self, scanner: u64, active_map: u64, working_map: u64) {
        self.tile_scanner.set_random_seed(scanner);
        self.active_light_map.set_random_seed(active_map);
        self.working_light_map.set_random_seed(working_map);
    }

    /// `_activeLightMap` (read-only view for tests and tools)
    pub fn active_light_map(&self) -> &LightMap {
        &self.active_light_map
    }

    /// `_activeProcessedArea`
    pub fn active_processed_area(&self) -> Rectangle {
        self.active_processed_area
    }

    /// `ProcessScan(Rectangle area)` (LightingEngine.cs:104)
    fn process_scan(&mut self, g: &mut Game, area: Rectangle) {
        let mut area = area;
        area.inflate(28, 28);
        self.working_processed_area = area;
        self.working_light_map.set_size(area.width, area.height);
        self.working_light_map.non_visible_padding = 18;
        self.tile_scanner.update();
        let options = TileLightScannerOptions { draw_invisible_walls: g.main.should_show_invisible_blocks_and_walls() };
        self.tile_scanner.export_to(g, area, &mut self.working_light_map, options);
    }

    /// `ProcessBlur()` (LightingEngine.cs:116)
    fn process_blur(&mut self, g: &Game) {
        self.update_light_decay(g);
        self.apply_per_frame_lights(g);
        self.working_light_map.blur();
    }

    /// `Present()` (LightingEngine.cs:123): `Utils.Swap` of the two maps and of the two areas.
    fn present(&mut self) {
        std::mem::swap(&mut self.active_light_map, &mut self.working_light_map);
        std::mem::swap(&mut self.active_processed_area, &mut self.working_processed_area);
    }

    /// `IncrementState()`: `(EngineState)((int)(_state + 1) % 4)`
    fn increment_state(&mut self) {
        self.state = match (self.state as i32 + 1) % 4 {
            0 => EngineState::MinimapUpdate,
            1 => EngineState::ExportMetrics,
            2 => EngineState::Scan,
            _ => EngineState::Blur,
        };
    }

    /// `UpdateLightDecay()` (LightingEngine.cs:129)
    fn update_light_decay(&mut self, g: &Game) {
        let working_light_map = &mut self.working_light_map;
        working_light_map.light_decay_through_air = 0.91f32;
        working_light_map.light_decay_through_solid = 0.56f32;
        working_light_map.light_decay_through_honey = Vector3::new(0.75f32, 0.7f32, 0.6f32) * 0.91f32;
        match g.main.water_style {
            0 | 1 | 7 | 8 => {
                working_light_map.light_decay_through_water = Vector3::new(0.88f32, 0.96f32, 1.015f32) * 0.91f32;
            }
            2 => {
                working_light_map.light_decay_through_water = Vector3::new(0.94f32, 0.85f32, 1.01f32) * 0.91f32;
            }
            3 => {
                working_light_map.light_decay_through_water = Vector3::new(0.84f32, 0.95f32, 1.015f32) * 0.91f32;
            }
            4 => {
                working_light_map.light_decay_through_water = Vector3::new(0.9f32, 0.86f32, 1.01f32) * 0.91f32;
            }
            5 => {
                working_light_map.light_decay_through_water = Vector3::new(0.84f32, 0.99f32, 1.01f32) * 0.91f32;
            }
            6 => {
                working_light_map.light_decay_through_water = Vector3::new(0.83f32, 0.93f32, 0.98f32) * 0.91f32;
            }
            9 => {
                working_light_map.light_decay_through_water = Vector3::new(1f32, 0.88f32, 0.84f32) * 0.91f32;
            }
            10 => {
                working_light_map.light_decay_through_water = Vector3::new(0.83f32, 1f32, 1f32) * 0.91f32;
            }
            12 => {
                working_light_map.light_decay_through_water = Vector3::new(0.95f32, 0.98f32, 0.85f32) * 0.91f32;
            }
            13 => {
                working_light_map.light_decay_through_water = Vector3::new(0.9f32, 1f32, 1.02f32) * 0.91f32;
            }
            _ => {}
        }
        // `Player perspectivePlayer = Main.SceneMetrics.PerspectivePlayer;` -- players and SceneMetrics are not
        // ported; every flag read below is `false` (a fresh player) until they are.
        if unported!("Player.nightVision", false) {
            working_light_map.light_decay_through_air *= 1.03f32;
            working_light_map.light_decay_through_solid *= 1.03f32;
        }
        if unported!("Player.blind", false) {
            working_light_map.light_decay_through_air *= 0.95f32;
            working_light_map.light_decay_through_solid *= 0.95f32;
        }
        if unported!("Player.blackout", false) {
            working_light_map.light_decay_through_air *= 0.85f32;
            working_light_map.light_decay_through_solid *= 0.85f32;
        }
        if unported!("Player.headcovered", false) {
            working_light_map.light_decay_through_air *= 0.85f32;
            working_light_map.light_decay_through_solid *= 0.85f32;
        }
        working_light_map.light_decay_through_air *= g.main.scene_state_air_light_decay;
        working_light_map.light_decay_through_solid *= g.main.scene_state_solid_light_decay;
    }

    /// `ApplyPerFrameLights()` (LightingEngine.cs:196)
    fn apply_per_frame_lights(&mut self, g: &Game) {
        let use_old = g.main.game_update_count == self.per_frame_light_swap_update_count;
        let list = if use_old { &self.old_per_frame_lights } else { &self.per_frame_lights };
        for i in 0..list.len() {
            let position = list[i].position;
            if self.working_processed_area.contains_point(position) {
                let value = list[i].color;
                let value2 = self.working_light_map.get(
                    position.x - self.working_processed_area.x,
                    position.y - self.working_processed_area.y,
                );
                // Vector3.Max(ref value2, ref value, out value)
                let value = Vector3::max(value2, value);
                self.working_light_map.set(
                    position.x - self.working_processed_area.x,
                    position.y - self.working_processed_area.y,
                    value,
                );
            }
        }
        if !unported!("CaptureManager.Instance.IsCapturing", false) {
            if g.main.game_update_count != self.per_frame_light_swap_update_count {
                std::mem::swap(&mut self.per_frame_lights, &mut self.old_per_frame_lights);
            }
            self.per_frame_lights.clear();
            self.per_frame_light_swap_update_count = g.main.game_update_count;
        }
    }

    /// `ExportToMiniMap()` (LightingEngine.cs:228)
    fn export_to_mini_map(&mut self, g: &mut Game) {
        if !g.main.map_enabled || self.active_processed_area.width <= 0 || self.active_processed_area.height <= 0 {
            return;
        }
        let mut area = Rectangle::new(
            self.active_processed_area.x + 28,
            self.active_processed_area.y + 28,
            self.active_processed_area.width - 56,
            self.active_processed_area.height - 56,
        );
        let mut value = Rectangle::new(0, 0, g.main.max_tiles_x, g.main.max_tiles_y);
        value.inflate(-40, -40);
        area = Rectangle::intersect(area, value);
        // MapHelper.sceneArea is a static of MapHelper; its writer (MapHelper.Update) is not ported.
        area = Rectangle::intersect(area, g.main.map_helper_scene_area);
        // FastParallel.For(area.Left, area.Right, ...): iteration i writes only column i of the map (through
        // `Main.Map.UpdateLighting(i, j, light)`) and reads only `_activeLightMap`, so a sequential loop is the same.
        for i in area.left()..area.right() {
            for j in area.top()..area.bottom() {
                let vector = self.active_light_map.get(i - self.active_processed_area.x, j - self.active_processed_area.y);
                let num = f32::max(f32::max(vector.x, vector.y), vector.z);
                // Math.Max(float, float) differs from f32::max only for NaN, which light values never are.
                let light = std::cmp::min(255, (num * 255f32) as i32) as u8;
                unported!("WorldMap.UpdateLighting"); // Main.Map.UpdateLighting(i, j, light)
                let _ = light;
            }
        }
        g.main.update_map = Some(area);
    }

    /// `AddGameplaySnapshotComponents()` (LightingEngine.cs:261): registers fields with the test-only `StateSnapshot`.
    pub fn add_gameplay_snapshot_components(&self) {
        unported!("StateSnapshot.Gameplay.AddCollection");
    }
}

impl Default for LightingEngine {
    fn default() -> Self {
        LightingEngine::new()
    }
}

impl ILightingEngine for LightingEngine {
    /// `Rebuild()` (LightingEngine.cs:218)
    fn rebuild(&mut self) {
        self.active_processed_area = Rectangle::EMPTY;
        self.working_processed_area = Rectangle::EMPTY;
        self.state = EngineState::MinimapUpdate;
        self.active_light_map = LightMap::new();
        self.working_light_map = LightMap::new();
    }

    /// `AddLight(int x, int y, Vector3 color)` (LightingEngine.cs:60)
    fn add_light(&mut self, x: i32, y: i32, color: Vector3) {
        self.per_frame_lights.push(PerFrameLight { position: Point::new(x, y), color });
    }

    /// `ProcessArea(Rectangle area)` (LightingEngine.cs:82)
    fn process_area(&mut self, g: &mut Game, area: Rectangle) {
        match self.state {
            EngineState::MinimapUpdate => {
                if g.main.map_delay > 0 {
                    g.main.map_delay -= 1;
                } else {
                    self.export_to_mini_map(g);
                }
                g.main.render_count = 3;
            }
            EngineState::ExportMetrics => {
                unported!("Main.UpdateSceneMetrics");
                g.main.render_count = 0;
            }
            EngineState::Scan => {
                self.process_scan(g, area);
                g.main.render_count = 1;
            }
            EngineState::Blur => {
                self.process_blur(g);
                self.present();
                g.main.render_count = 2;
            }
        }
        self.increment_state();
    }

    /// `GetColor(int x, int y)` (LightingEngine.cs:75)
    fn get_color(&self, x: i32, y: i32) -> Vector3 {
        if !self.active_processed_area.contains_xy(x, y) {
            return Vector3::ZERO;
        }
        let x = x - self.active_processed_area.x;
        let y = y - self.active_processed_area.y;
        self.active_light_map.get(x, y)
    }

    /// `Clear()` (LightingEngine.cs:66)
    fn clear(&mut self) {
        self.active_light_map.clear();
        self.working_light_map.clear();
        self.per_frame_lights.clear();
        self.old_per_frame_lights.clear();
        self.per_frame_light_swap_update_count = 0;
    }
}
