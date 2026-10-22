//! `Terraria.SceneMetrics` (SceneMetrics.cs): the tile census around the view that decides the biome zones and the
//! backgrounds. `Scan` is ported for the part the zone flags and the backgrounds read; what needs the player, the NPCs or
//! the on-screen music boxes is `unported!` at its place.

use terraria_core::xna::{Rectangle, Vector2};
use terraria_data::sets::tile_id::sets as tile_sets;
use terraria_data::sets::wall_id::sets::conversion as wall_conversion;

use crate::game::Game;
use crate::unported;

pub const SHIMMER_TILE_THRESHOLD: i32 = 300;
pub const CORRUPTION_TILE_THRESHOLD: i32 = 300;
pub const CORRUPTION_TILE_MAX: i32 = 1000;
pub const CRIMSON_TILE_THRESHOLD: i32 = 300;
pub const CRIMSON_TILE_MAX: i32 = 1000;
pub const HALLOW_TILE_THRESHOLD: i32 = 125;
pub const HALLOW_TILE_MAX: i32 = 600;
pub const JUNGLE_TILE_THRESHOLD: i32 = 140;
pub const JUNGLE_TILE_MAX: i32 = 700;
pub const SNOW_TILE_NORMAL_THRESHOLD: i32 = 1500;
pub const SNOW_TILE_SKYBLOCK_THRESHOLD: i32 = 300;
pub const SNOW_TILE_MAX: i32 = 6000;
pub const DESERT_TILE_NORMAL_THRESHOLD: i32 = 1500;
pub const DESERT_TILE_SKYBLOCK_THRESHOLD: i32 = 300;
pub const MUSHROOM_TILE_THRESHOLD: i32 = 100;
pub const MUSHROOM_TILE_MAX: i32 = 160;
pub const METEOR_TILE_THRESHOLD: i32 = 75;
pub const DUNGEON_TILE_THRESHOLD: i32 = 250;
pub const GRAVEYARD_TILE_MAX: i32 = 36;
pub const GRAVEYARD_TILE_MIN: i32 = 16;
pub const GRAVEYARD_TILE_THRESHOLD: i32 = 28;

/// `SceneMetrics.ZoneScanSize` = `AssumedConstantScreenSize / 16 + ZoneScanPadding * 2 - 1` = (1920 / 16 + 49, 1200 / 16 + 49)
pub const ZONE_SCAN_SIZE: (i32, i32) = (1920 / 16 + 25 * 2 - 1, 1200 / 16 + 25 * 2 - 1);

/// `SceneMetrics.SnowTileThreshold` (SceneMetrics.cs:156): `WorldGen.Skyblock.lowTiles` is not ported (false)
pub fn snow_tile_threshold(_g: &Game) -> i32 {
    if unported!("WorldGen.Skyblock.lowTiles", false) {
        return SNOW_TILE_SKYBLOCK_THRESHOLD;
    }
    SNOW_TILE_NORMAL_THRESHOLD
}

/// `SceneMetrics.DesertTileThreshold` (SceneMetrics.cs:168)
pub fn desert_tile_threshold(_g: &Game) -> i32 {
    if unported!("WorldGen.Skyblock.lowTiles", false) {
        return DESERT_TILE_SKYBLOCK_THRESHOLD;
    }
    DESERT_TILE_NORMAL_THRESHOLD
}

/// The fields of `SceneMetrics` the ported code reads (counts, zone flags). Tile counts are kept in `tile_counts` as `_tileCounts`.
#[derive(Clone, Debug)]
pub struct SceneMetrics {
    /// `_tileCounts = new int[TileID.Count]`
    pub tile_counts: Vec<i32>,
    /// `_liquidCounts = new int[LiquidID.Count]`
    pub liquid_counts: [i32; 4],
    pub last_scan_time: u32,
    /// `Center` and `TileCenter`
    pub center: Vector2,
    pub tile_center: (i32, i32),
    pub shimmer_tile_count: i32,
    pub evil_tile_count: i32,
    pub holy_tile_count: i32,
    pub honey_block_count: i32,
    pub sand_tile_count: i32,
    pub ocean_sand_tile_count: i32,
    pub desert_sand_tile_count: i32,
    pub mushroom_tile_count: i32,
    pub snow_tile_count: i32,
    pub water_candle_count: i32,
    pub peace_candle_count: i32,
    pub shadow_candle_count: i32,
    pub meteor_tile_count: i32,
    pub blood_tile_count: i32,
    pub jungle_tile_count: i32,
    pub dungeon_tile_count: i32,
    pub graveyard_tile_count: i32,
    pub has_sunflower: bool,
    pub has_garden_gnome: bool,
    pub behind_backwall: bool,
    pub below_surface: bool,
    pub zone_sky_height: bool,
    pub zone_overworld_height: bool,
    pub zone_dirt_layer_height: bool,
    pub zone_rock_layer_height: bool,
    pub zone_underworld_height: bool,
    pub zone_corrupt: bool,
    pub zone_crimson: bool,
    pub zone_hallow: bool,
    pub zone_jungle: bool,
    pub zone_snow: bool,
    pub zone_desert: bool,
    pub zone_glowshroom: bool,
    pub zone_meteor: bool,
    pub zone_graveyard: bool,
    pub zone_dungeon: bool,
    pub zone_lihzhard_temple: bool,
    pub zone_granite: bool,
    pub zone_marble: bool,
    pub zone_hive: bool,
    pub zone_gem_cave: bool,
    pub zone_beach: bool,
    pub zone_underground_desert: bool,
    pub zone_rain: bool,
    pub zone_sandstorm: bool,
    pub surface_atmospherics: bool,
    pub underground_for_shimmering: bool,
    pub zone_shimmer: bool,
    pub zone_water_candle: bool,
    pub zone_peace_candle: bool,
    pub zone_shadow_candle: bool,
    /// `SceneMetrics.BloodMoonMonolith` (a monolith near the view; the on-screen scan is not ported)
    pub blood_moon_monolith: bool,
}

impl SceneMetrics {
    pub fn new() -> Self {
        Self {
            tile_counts: vec![0; terraria_data::main_arrays::TILE_COUNT],
            liquid_counts: [0; 4],
            last_scan_time: 0,
            center: Vector2::ZERO,
            tile_center: (0, 0),
            shimmer_tile_count: 0,
            evil_tile_count: 0,
            holy_tile_count: 0,
            honey_block_count: 0,
            sand_tile_count: 0,
            ocean_sand_tile_count: 0,
            desert_sand_tile_count: 0,
            mushroom_tile_count: 0,
            snow_tile_count: 0,
            water_candle_count: 0,
            peace_candle_count: 0,
            shadow_candle_count: 0,
            meteor_tile_count: 0,
            blood_tile_count: 0,
            jungle_tile_count: 0,
            dungeon_tile_count: 0,
            graveyard_tile_count: 0,
            has_sunflower: false,
            has_garden_gnome: false,
            behind_backwall: false,
            below_surface: false,
            zone_sky_height: false,
            zone_overworld_height: false,
            zone_dirt_layer_height: false,
            zone_rock_layer_height: false,
            zone_underworld_height: false,
            zone_corrupt: false,
            zone_crimson: false,
            zone_hallow: false,
            zone_jungle: false,
            zone_snow: false,
            zone_desert: false,
            zone_glowshroom: false,
            zone_meteor: false,
            zone_graveyard: false,
            zone_dungeon: false,
            zone_lihzhard_temple: false,
            zone_granite: false,
            zone_marble: false,
            zone_hive: false,
            zone_gem_cave: false,
            zone_beach: false,
            zone_underground_desert: false,
            zone_rain: false,
            zone_sandstorm: false,
            surface_atmospherics: false,
            underground_for_shimmering: false,
            zone_shimmer: false,
            zone_water_candle: false,
            zone_peace_candle: false,
            zone_shadow_candle: false,
            blood_moon_monolith: false,
        }
    }

    /// `EnoughTilesForShimmer` .. `EnoughTilesForGraveyard` (SceneMetrics.cs:252-272)
    pub fn enough_tiles_for_shimmer(&self) -> bool {
        self.shimmer_tile_count >= SHIMMER_TILE_THRESHOLD
    }
    pub fn enough_tiles_for_jungle(&self) -> bool {
        self.jungle_tile_count >= JUNGLE_TILE_THRESHOLD
    }
    pub fn enough_tiles_for_hallow(&self) -> bool {
        self.holy_tile_count >= HALLOW_TILE_THRESHOLD
    }
    pub fn enough_tiles_for_snow(&self, g: &Game) -> bool {
        self.snow_tile_count >= snow_tile_threshold(g)
    }
    pub fn enough_tiles_for_glowing_mushroom(&self) -> bool {
        self.mushroom_tile_count >= MUSHROOM_TILE_THRESHOLD
    }
    pub fn enough_tiles_for_desert(&self, g: &Game) -> bool {
        self.desert_sand_tile_count >= desert_tile_threshold(g)
    }
    pub fn enough_tiles_for_corruption(&self) -> bool {
        self.evil_tile_count >= CORRUPTION_TILE_THRESHOLD
    }
    pub fn enough_tiles_for_crimson(&self) -> bool {
        self.blood_tile_count >= CRIMSON_TILE_THRESHOLD
    }
    pub fn enough_tiles_for_meteor(&self) -> bool {
        self.meteor_tile_count >= METEOR_TILE_THRESHOLD
    }
    pub fn enough_tiles_for_dungeon(&self) -> bool {
        self.dungeon_tile_count >= DUNGEON_TILE_THRESHOLD
    }
    pub fn enough_tiles_for_graveyard(&self) -> bool {
        self.graveyard_tile_count >= GRAVEYARD_TILE_THRESHOLD
    }
}

impl Default for SceneMetrics {
    fn default() -> Self {
        Self::new()
    }
}

/// `WorldGen.oceanLevel` (WorldGen.cs:4391)
fn ocean_level(g: &Game) -> f64 {
    (g.main.world_surface + g.main.rock_layer) / 2.0 + 40.0
}

/// `WorldGen.oceanDepths(int x, int y)` (WorldGen.cs:10972)
pub fn ocean_depths(g: &Game, x: i32, y: i32) -> bool {
    if (y as f64) > ocean_level(g) {
        return false;
    }
    if x < crate::world_gen::tile_queries::BEACH_DISTANCE || x > g.main.max_tiles_x - crate::world_gen::tile_queries::BEACH_DISTANCE {
        return true;
    }
    false
}

/// `WorldGen.IsSurfaceForAtmospherics(Point tileCoords)` (WorldGen.cs:10985)
pub fn is_surface_for_atmospherics(g: &Game, tile_y: i32) -> bool {
    if g.main.remix_world {
        if (tile_y as f64) > g.main.rock_layer {
            return tile_y < g.main.max_tiles_y - 350;
        }
        return false;
    }
    (tile_y as f64) <= g.main.world_surface
}

/// `SceneMetrics.Reset()` (SceneMetrics.cs:788): the counts the ported code keeps go back to zero, the zone flags too
fn reset(m: &mut SceneMetrics) {
    for c in m.tile_counts.iter_mut() {
        *c = 0;
    }
    m.liquid_counts = [0; 4];
    m.shimmer_tile_count = 0;
    m.evil_tile_count = 0;
    m.holy_tile_count = 0;
    m.honey_block_count = 0;
    m.sand_tile_count = 0;
    m.ocean_sand_tile_count = 0;
    m.desert_sand_tile_count = 0;
    m.mushroom_tile_count = 0;
    m.snow_tile_count = 0;
    m.water_candle_count = 0;
    m.peace_candle_count = 0;
    m.shadow_candle_count = 0;
    m.meteor_tile_count = 0;
    m.blood_tile_count = 0;
    m.jungle_tile_count = 0;
    m.dungeon_tile_count = 0;
    m.graveyard_tile_count = 0;
    m.has_sunflower = false;
    m.has_garden_gnome = false;
    unported!("SceneMetrics.Reset (music box, campfire, banner, ore finder, NPC position fields)");
}

/// `SceneMetrics.ScanTiles()` (SceneMetrics.cs:354)
fn scan_tiles(g: &Game, m: &mut SceneMetrics) {
    // Utils.CenteredRectangle(TileCenter, ZoneScanSize), then WorldUtils.ClampToWorld
    let rect = Rectangle::new(m.tile_center.0 - ZONE_SCAN_SIZE.0 / 2, m.tile_center.1 - ZONE_SCAN_SIZE.1 / 2, ZONE_SCAN_SIZE.0, ZONE_SCAN_SIZE.1);
    let rect = clamp_to_world(g, rect);
    for i in rect.x..rect.x + rect.width {
        for j in rect.y..rect.y + rect.height {
            let Some(tile) = g.main.tile.try_get(i, j) else { continue };
            if !tile.active() {
                if tile.liquid > 0 {
                    m.liquid_counts[tile.liquid_type() as usize] += 1;
                }
                continue;
            }
            m.tile_counts[tile.type_ as usize] += 1;
            if tile_sets::IS_DESERT_BIOME_SAND[tile.type_ as usize] && ocean_depths(g, i, j) {
                m.ocean_sand_tile_count += 1;
            }
            if tile.type_ == 49 && tile.frame_x < 18 {
                m.water_candle_count += 1;
            }
            if tile.type_ == 372 && tile.frame_x < 18 {
                m.peace_candle_count += 1;
            }
            if tile.type_ == 646 && tile.frame_x < 18 {
                m.shadow_candle_count += 1;
            }
            // campfire / cat bast / heart lantern / star in a bottle / banner / ore finder: not read by the zones or the backgrounds
        }
    }
    unported!("SceneMetrics.ScanTiles: HasCampfire / HasCatBast / HasHeartLantern / HasStarInBottle / NPCBannerBuff / UpdateOreFinder");
}

/// `WorldUtils.ClampToWorld(Rectangle)` (WorldUtils.cs): clamps the rectangle into `[0, maxTilesX) x [0, maxTilesY)`
fn clamp_to_world(g: &Game, r: Rectangle) -> Rectangle {
    let left = r.x.max(0);
    let top = r.y.max(0);
    let right = (r.x + r.width).min(g.main.max_tiles_x);
    let bottom = (r.y + r.height).min(g.main.max_tiles_y);
    Rectangle::new(left, top, (right - left).max(0), (bottom - top).max(0))
}

/// `SceneMetrics.AggregateTileCounts()` (SceneMetrics.cs:586)
fn aggregate_tile_counts(g: &Game, m: &mut SceneMetrics) {
    let mut num = -10;
    if unported!("Main.infectedSeed", false) {
        num *= 3;
    }
    let tc = &m.tile_counts;
    let mut has_sunflower = m.has_sunflower;
    let mut has_garden_gnome = m.has_garden_gnome;
    if tc[27] > 0 {
        has_sunflower = true;
    }
    if tc[567] > 0 {
        has_garden_gnome = true;
    }
    m.shimmer_tile_count = m.liquid_counts[3];
    m.honey_block_count = tc[229];
    m.holy_tile_count = tc[109] + tc[492] + tc[110] + tc[113] + tc[117] + tc[116] + tc[164] + tc[403] + tc[402];
    m.snow_tile_count = tc[147] + tc[148] + tc[161] + tc[162] + tc[164] + tc[163] + tc[200];
    if g.main.remix_world {
        m.jungle_tile_count = tc[60] + tc[61] + tc[62] + tc[74] + tc[225];
        m.evil_tile_count = tc[23] + tc[661] + tc[24] + tc[25] + tc[32] + tc[112] + tc[163] + tc[400] + tc[398] + tc[27] * num + tc[474];
        m.blood_tile_count = tc[199] + tc[662] + tc[201] + tc[203] + tc[200] + tc[401] + tc[399] + tc[234] + tc[352] + tc[27] * num + tc[195];
    } else {
        m.jungle_tile_count = tc[60] + tc[61] + tc[62] + tc[74] + tc[226] + tc[225];
        m.evil_tile_count = tc[23] + tc[661] + tc[24] + tc[25] + tc[32] + tc[112] + tc[163] + tc[400] + tc[398] + tc[27] * num;
        m.blood_tile_count = tc[199] + tc[662] + tc[201] + tc[203] + tc[200] + tc[401] + tc[399] + tc[234] + tc[352] + tc[27] * num;
    }
    m.mushroom_tile_count = tc[70] + tc[71] + tc[72] + tc[528];
    m.meteor_tile_count = tc[37];
    m.dungeon_tile_count = tc[41] + tc[43] + tc[44] + tc[481] + tc[482] + tc[483];
    m.sand_tile_count = tc[53] + tc[112] + tc[116] + tc[234] + tc[397] + tc[398] + tc[402] + tc[399] + tc[396] + tc[400] + tc[403] + tc[401];
    // PartyMonolithCount = _tileCounts[455] is not read
    m.graveyard_tile_count = tc[85];
    m.graveyard_tile_count -= tc[27] / 2;
    if tc[27] > 0 {
        has_sunflower = true;
    }
    if m.graveyard_tile_count > GRAVEYARD_TILE_MIN {
        has_sunflower = false;
    }
    m.has_sunflower = has_sunflower;
    m.has_garden_gnome = has_garden_gnome;
    if m.graveyard_tile_count < 0 {
        m.graveyard_tile_count = 0;
    }
    if m.holy_tile_count < 0 {
        m.holy_tile_count = 0;
    }
    if m.evil_tile_count < 0 {
        m.evil_tile_count = 0;
    }
    if m.blood_tile_count < 0 {
        m.blood_tile_count = 0;
    }
    let holy_tile_count = m.holy_tile_count;
    m.holy_tile_count -= m.evil_tile_count;
    m.holy_tile_count -= m.blood_tile_count;
    m.evil_tile_count -= holy_tile_count;
    m.blood_tile_count -= holy_tile_count;
    if m.holy_tile_count < 0 {
        m.holy_tile_count = 0;
    }
    if m.evil_tile_count < 0 {
        m.evil_tile_count = 0;
    }
    if m.blood_tile_count < 0 {
        m.blood_tile_count = 0;
    }
    m.desert_sand_tile_count = 0.max(m.sand_tile_count - m.ocean_sand_tile_count);
}

/// `SceneMetrics.CalculateZones()` (SceneMetrics.cs:668)
fn calculate_zones(g: &Game, m: &mut SceneMetrics) {
    let tile_safely = *g.main.tile.get(m.tile_center.0, m.tile_center.1); // Framing.GetTileSafely(TileCenter): TileCenter is clamped into the world
    let (cx, cy) = m.tile_center;
    m.behind_backwall = tile_safely.wall > 0;
    m.zone_sky_height = (cy as f64) <= g.main.world_surface * 0.3499999940395355;
    m.zone_overworld_height = (cy as f64) <= g.main.world_surface && (cy as f64) > g.main.world_surface * 0.3499999940395355;
    m.below_surface = (cy as f64) > g.main.world_surface;
    m.zone_dirt_layer_height = (cy as f64) <= g.main.rock_layer && (cy as f64) > g.main.world_surface;
    m.zone_rock_layer_height = cy <= g.main.underworld_layer() && (cy as f64) > g.main.rock_layer;
    m.zone_underworld_height = cy > g.main.underworld_layer();
    m.zone_corrupt = m.enough_tiles_for_corruption();
    m.zone_crimson = m.enough_tiles_for_crimson();
    m.zone_hallow = m.enough_tiles_for_hallow();
    m.zone_jungle = m.enough_tiles_for_jungle() && !m.zone_underworld_height;
    m.zone_snow = m.enough_tiles_for_snow(g);
    m.zone_desert = m.enough_tiles_for_desert(g);
    m.zone_glowshroom = m.enough_tiles_for_glowing_mushroom();
    m.zone_meteor = m.enough_tiles_for_meteor();
    m.zone_graveyard = m.enough_tiles_for_graveyard();
    let mut num = g.main.world_surface;
    if unported!("Main.drunkWorld", g.main.drunk_world) || !(g.main.world_surface > 50.0) {
        num = num.max((g.main.dungeon_y + 40) as f64);
    }
    m.zone_dungeon = m.enough_tiles_for_dungeon() && (cy as f64) > num && g.main.wall_dungeon[tile_safely.wall as usize];
    m.zone_lihzhard_temple = tile_safely.wall == 87;
    m.zone_granite = tile_safely.wall == 184 || tile_safely.wall == 180;
    m.zone_marble = tile_safely.wall == 183 || tile_safely.wall == 178;
    m.zone_hive = tile_safely.wall == 108 || tile_safely.wall == 86;
    m.zone_gem_cave = tile_safely.wall >= 48 && tile_safely.wall <= 53;
    m.zone_beach = ocean_depths(g, cx, cy);
    m.zone_underground_desert = m.zone_desert
        && m.below_surface
        && (wall_conversion::SANDSTONE[tile_safely.wall as usize] || wall_conversion::HARDENED_SAND[tile_safely.wall as usize] || tile_safely.wall == 223)
        && !g.main.wall_house[tile_safely.wall as usize];
    m.surface_atmospherics = is_surface_for_atmospherics(g, cy);
    if g.main.remix_world && m.zone_dungeon {
        m.surface_atmospherics = false;
    }
    m.zone_rain = g.main.raining && m.surface_atmospherics;
    m.zone_sandstorm = m.zone_desert && m.surface_atmospherics && unported!("Sandstorm.Happening", false);
    if m.zone_sandstorm {
        m.zone_rain = false;
    }
    m.underground_for_shimmering = (cy as f64) > g.main.world_surface + 84.0 && cy < g.main.max_tiles_y - 396;
    m.zone_shimmer = m.enough_tiles_for_shimmer() && m.underground_for_shimmering && !m.zone_dungeon;
    m.zone_water_candle = m.water_candle_count > 0;
    m.zone_peace_candle = m.peace_candle_count > 0;
    m.zone_shadow_candle = m.shadow_candle_count > 0;
    if unported!("Main.dualDungeonsSeed", false) && m.below_surface && !m.zone_underworld_height {
        unported!("SceneMetrics.CalculateZones: NPCSpawningFlagsForDualDungeons scan");
    }
}

/// `SceneMetrics.Scan(SceneMetricsScanSettings settings)` (SceneMetrics.cs:327) on `Main.SceneMetrics`.
/// `settings.BiomeScanCenterPositionInWorld` is `center`; `settings.VisualScanArea` is `visual_scan_area`.
pub fn scan(g: &mut Game, center: Vector2, visual_scan_area: Option<Rectangle>) {
    let mut m = std::mem::replace(&mut g.main.scene_metrics, SceneMetrics::new());
    if m.last_scan_time != g.main.game_update_count || !(m.center == center) {
        reset(&mut m);
        m.last_scan_time = g.main.game_update_count;
        m.center = center;
        // Center.ToTileCoordinates().ClampedInWorld()
        let tx = ((center.x / 16.0) as i32).clamp(0, g.main.max_tiles_x - 1);
        let ty = ((center.y / 16.0) as i32).clamp(0, g.main.max_tiles_y - 1);
        m.tile_center = (tx, ty);
        scan_tiles(g, &mut m);
        if visual_scan_area.is_some() {
            unported!("SceneMetrics.ScanOnScreenTiles (clock, music boxes, fountains, monoliths)");
        }
        unported!("SceneMetrics.ScanNPCPositions");
        aggregate_tile_counts(g, &mut m);
        calculate_zones(g, &mut m);
        unported!("SceneMetrics.AddPlayerEffects");
    }
    g.main.scene_metrics = m;
}

#[cfg(test)]
mod tests {
    use super::*;
    use terraria_core::TileMap;

    fn world(w: i32, h: i32) -> Game {
        let mut g = Game::new();
        g.main.tile = TileMap::new(w, h);
        g.main.max_tiles_x = w;
        g.main.max_tiles_y = h;
        g.main.world_surface = 100.0;
        g.main.rock_layer = 150.0;
        g.main.game_update_count = 1;
        g
    }

    fn fill(g: &mut Game, type_: u16, n: i32, x0: i32, y0: i32) {
        for k in 0..n {
            let t = g.main.tile.get_mut(x0 + k % 40, y0 + k / 40);
            t.set_active(true);
            t.type_ = type_;
        }
    }

    #[test]
    fn corruption_zone_needs_300_net_tiles_and_the_hallow_count_is_subtracted() {
        let mut g = world(800, 400);
        fill(&mut g, 23, 299, 400, 90); // ebonstone grass: 299 evil tiles, no zone
        scan(&mut g, Vector2::new(400.0 * 16.0, 95.0 * 16.0), None);
        assert_eq!(g.main.scene_metrics.evil_tile_count, 299);
        assert!(!g.main.scene_metrics.zone_corrupt);
        g.main.game_update_count = 2;
        fill(&mut g, 23, 301, 400, 90);
        scan(&mut g, Vector2::new(400.0 * 16.0, 95.0 * 16.0), None);
        assert_eq!(g.main.scene_metrics.evil_tile_count, 301);
        assert!(g.main.scene_metrics.zone_corrupt);
        // 200 hallowed grass tiles: holy 200, evil 301 - 200 = 101 (< 300), holy 200 - 301 -> 0
        g.main.game_update_count = 3;
        fill(&mut g, 109, 200, 440, 90);
        scan(&mut g, Vector2::new(400.0 * 16.0, 95.0 * 16.0), None);
        assert_eq!(g.main.scene_metrics.evil_tile_count, 101);
        assert_eq!(g.main.scene_metrics.holy_tile_count, 0);
        assert!(!g.main.scene_metrics.zone_corrupt && !g.main.scene_metrics.zone_hallow);
    }

    #[test]
    fn height_zones_follow_the_tile_row_of_the_scan_centre() {
        let mut g = world(800, 400);
        let rows = [(30, "sky"), (80, "overworld"), (120, "dirt"), (200, "rock"), (380, "underworld")];
        for (row, name) in rows {
            g.main.game_update_count += 1;
            scan(&mut g, Vector2::new(400.0 * 16.0, row as f32 * 16.0), None);
            let m = &g.main.scene_metrics;
            let got = (m.zone_sky_height, m.zone_overworld_height, m.zone_dirt_layer_height, m.zone_rock_layer_height, m.zone_underworld_height);
            let want = match name {
                "sky" => (true, false, false, false, false),     // 30 <= 100 * 0.35 = 35
                "overworld" => (false, true, false, false, false), // 35 < 80 <= 100
                "dirt" => (false, false, true, false, false),     // 100 < 120 <= 150
                "rock" => (false, false, false, true, false),     // 150 < 200 <= 400 - 200
                _ => (false, false, false, false, true),          // 380 > 200
            };
            assert_eq!(got, want, "row {row}");
        }
    }

    #[test]
    fn beach_zone_is_the_outer_380_tiles_above_the_ocean_level() {
        let mut g = world(1000, 400);
        // oceanLevel = (100 + 150) / 2 + 40 = 165
        assert!(ocean_depths(&g, 100, 164));
        assert!(!ocean_depths(&g, 100, 166));
        assert!(!ocean_depths(&g, 500, 100));
        assert!(ocean_depths(&g, 700, 100)); // > 1000 - 380
        g.main.game_update_count += 1;
        scan(&mut g, Vector2::new(100.0 * 16.0, 100.0 * 16.0), None);
        assert!(g.main.scene_metrics.zone_beach);
    }
}
