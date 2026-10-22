//! All game state. The original keeps it in static fields of `Main`, `WorldGen`, ...; here each of
//! those classes has a struct and `Game` owns one of each (see `port/PORTING.md`).
//!
//! Fields are added as code that uses them is ported. Each has the original's name (snake_case) and
//! initial value, with the source line.

use std::ops::{Deref, DerefMut};

use terraria_core::{TileMap, UnifiedRandom};
use terraria_data::MainArrays;

/// Static fields of `Terraria.Main`.
pub struct MainState {
    /// `Main.tile`
    pub tile: TileMap,
    /// `Main.tileSolid`, `Main.wallHouse`, ... (reachable directly: `g.main.tile_solid`)
    pub arrays: MainArrays,
    /// `Main.maxTilesX` (Main.cs:1087: `maxTilesX = 8401` before a world is loaded)
    pub max_tiles_x: i32,
    /// `Main.maxTilesY`
    pub max_tiles_y: i32,
    /// `Main.leftWorld` .. `Main.bottomWorld`, in pixels
    pub left_world: f32,
    pub right_world: f32,
    pub top_world: f32,
    pub bottom_world: f32,
    /// `Main.worldSurface`
    pub world_surface: f64,
    /// `Main.rockLayer`
    pub rock_layer: f64,
    /// `Main.spawnTileX`, `Main.spawnTileY`
    pub spawn_tile_x: i32,
    pub spawn_tile_y: i32,
    /// `Main.dungeonX`, `Main.dungeonY`
    pub dungeon_x: i32,
    pub dungeon_y: i32,
    /// `Main.worldName`
    pub world_name: String,
    /// `Main.rand`. `WorldGen.genRand` is this same generator (`genRand => Main.rand`, WorldGen.cs:4389).
    pub rand: UnifiedRandom,
    /// `Main.netMode`: 0 single player, 1 client, 2 server
    pub net_mode: i32,
    /// `Main.dedServ`
    pub ded_serv: bool,
    /// `Main.gameMenu`
    pub game_menu: bool,
    /// `Main.time`
    pub time: f64,
    /// `Main.dayTime`
    pub day_time: bool,
    /// `Main.hardMode`
    pub hard_mode: bool,
    /// `Main.GameMode`
    pub game_mode: i32,
    /// `Main.screenPosition`, in world pixels (top-left corner of the view)
    pub screen_position: (f32, f32),
    /// `Main.screenWidth`, `Main.screenHeight`
    pub screen_width: i32,
    pub screen_height: i32,
    /// `SceneMetrics.EchoMonolith` of `Main.SceneMetrics` (false until a player is near an echo monolith)
    pub echo_monolith: bool,
    /// `Main.SceneMetrics.PerspectivePlayer.CanSeeInvisibleBlocks` (a `Player` field; false for a fresh player)
    pub perspective_player_can_see_invisible_blocks: bool,
    /// `Main.getGoodWorld` (Main.cs:331, `= false`). Read by `Collision.CanTileHurt`.
    pub get_good_world: bool,
    /// `Main.dontStarveWorld` (Main.cs:335, `= false`). Read by `Collision.CanTileHurt`.
    pub dont_starve_world: bool,
    // ---- statics read by the lighting engine (Lighting.cs, Graphics/Light/*.cs) ----
    /// `Main.render` (`public static bool render;`). Written by `Lighting.LightTiles`; read by the renderer.
    pub render: bool,
    /// `Main.renderCount` (`= 99`; `Lighting.Initialize()` -> `Lighting.Mode` setter leaves 0). Written by the lighting engine.
    pub render_count: i32,
    /// `Main.renderNow` (`public static bool renderNow;`). Written by the lighting mode setter and the camera code.
    pub render_now: bool,
    /// `Main.gamePaused`. Written by `Main.Update`.
    pub game_paused: bool,
    /// `Main.mapDelay` (`= 2`). Counted down by the lighting engine.
    pub map_delay: i32,
    /// `Main.mapEnabled` (`= true`). Written by the settings code.
    pub map_enabled: bool,
    /// `Main.updateMap` (`Rectangle?`). Written by the lighting engine, read by the map code.
    pub update_map: Option<terraria_core::xna::Rectangle>,
    /// `MapHelper.sceneArea` (static of `MapHelper`, `default(Rectangle)`). Written by `MapHelper.Update` (not ported).
    pub map_helper_scene_area: terraria_core::xna::Rectangle,
    /// `Main.waterStyle` (0). Written by the biome code (`Main.UpdateBackgroundsAndWater...`, not ported).
    pub water_style: i32,
    /// `Main._gameUpdateCount` (read through `Main.GameUpdateCount`). Written by `Main.DoUpdate`.
    pub game_update_count: u32,
    /// `Main.mouseTextColor` (byte, 0). Written by `Main.DoUpdate`'s mouse text pulse.
    pub mouse_text_color: u8,
    /// `Main.DiscoR` (= 255). Written by `Main.DoUpdate`.
    pub disco_r: i32,
    /// `Main.DiscoG` (0). Written by `Main.DoUpdate`.
    pub disco_g: i32,
    /// `Main.DiscoB` (0). Written by `Main.DoUpdate`.
    pub disco_b: i32,
    /// `Main.demonTorch` (= 1f). Written by `Main.DoUpdate`.
    pub demon_torch: f32,
    /// `Main.shimmerDarken` (= 0f). Written by the shimmer scene code (not ported).
    pub shimmer_darken: f32,
    /// `Main.sundialCooldown` (0). Written by `Main.DoUpdate`.
    pub sundial_cooldown: i32,
    /// `Main.moondialCooldown` (0). Written by `Main.DoUpdate`.
    pub moondial_cooldown: i32,
    /// `Main.cageFrames` (= 25)
    pub cage_frames: i32,
    /// `Main.jellyfishCageMode` (`new byte[3, cageFrames]`). Written by the jellyfish jar update code (not ported).
    pub jellyfish_cage_mode: [[u8; 25]; 3],
    /// `Main.liquidAlpha` (`new float[15]`). Written by the water drawing code (`Main.DrawWater`-side, not ported).
    pub liquid_alpha: [f32; 15],
    /// `Main.tileColor` (`default(Color)`). Written by `Main.ApplyColorOfTheSkiesToTiles` (from `Main.ColorOfTheSkies`, not ported).
    pub tile_color: terraria_core::xna::Color,
    /// `Main.raining`. Written by the weather code (not ported).
    pub raining: bool,
    /// `Main.timeForVisualEffects` (0.0). Written by `Main.DoUpdate`.
    pub time_for_visual_effects: f64,
    /// `Main.GlobalTimeWrappedHourly` (0f). Written by `Main.Update` / `Main.Draw` from the game clock.
    pub global_time_wrapped_hourly: f32,
    /// `Main.myPlayer` (0)
    pub my_player: i32,
    /// `Main.SceneState.airLightDecay` (`SceneState.Reset`: 1f). Written by `SceneState.Update` (not ported).
    pub scene_state_air_light_decay: f32,
    /// `Main.SceneState.solidLightDecay` (`SceneState.Reset`: 1f). Written by `SceneState.Update` (not ported).
    pub scene_state_solid_light_decay: f32,
    /// `Main.treeX` (`new int[3]`). Written by `WorldFile.LoadHeader`.
    pub tree_x: [i32; 3],
    /// `Main.treeStyle` (`new int[4]`). Written by `WorldFile.LoadHeader`.
    pub tree_style: [i32; 4],
    /// `Main.remixWorld` (false). Written by `WorldFile.LoadHeader`.
    pub remix_world: bool,
    /// `Main.windSpeedCurrent` (`public static float`, 0). Written by `WorldFile.LoadHeader` (= `windSpeedTarget`) and by the weather
    /// update (`Main.UpdateWeather`, not ported), which must keep it moving towards `windSpeedTarget`. Read through `WindForVisuals`.
    pub wind_speed_current: f32,
    /// `moonPhase` (Main.cs:1259), initially `0`. Written by the sky / background code.
    pub moon_phase: i32,
    /// `bloodMoon` (Main.cs:1267), initially `false`. Written by the sky / background code.
    pub blood_moon: bool,
    /// `eclipse` (Main.cs:1285), initially `false`. Written by the sky / background code.
    pub eclipse: bool,
    /// `moonType` (Main.cs:1407), initially `0`. Written by the sky / background code.
    pub moon_type: i32,
    /// `pumpkinMoon` (Main.cs:1269), initially `false`. Written by the sky / background code.
    pub pumpkin_moon: bool,
    /// `snowMoon` (Main.cs:1271), initially `false`. Written by the sky / background code.
    pub snow_moon: bool,
    /// `dayRate` (Main.cs:749), initially `1`. Written by the sky / background code.
    pub day_rate: i32,
    /// `desiredWorldTilesUpdateRate` (Main.cs:751), initially `1`. Written by the sky / background code.
    pub desired_world_tiles_update_rate: i32,
    /// `fastForwardTimeToDusk` (Main.cs:2318), initially `false`. Written by the sky / background code.
    pub fast_forward_time_to_dusk: bool,
    /// `fastForwardTimeToDawn` (Main.cs:2314), initially `false`. Written by the sky / background code.
    pub fast_forward_time_to_dawn: bool,
    /// `sunModY` (Main.cs:1261), initially `0`. Written by the sky / background code.
    pub sun_mod_y: i16,
    /// `moonModY` (Main.cs:1263), initially `0`. Written by the sky / background code.
    pub moon_mod_y: i16,
    /// `eclipseLight` (Main.cs:1287), initially `0f32`. Written by the sky / background code.
    pub eclipse_light: f32,
    /// `atmo` (Main.cs:2672), initially `0f32`. Written by the sky / background code.
    pub atmo: f32,
    /// `ColorOfTheSkies` (Main.cs:2678), initially `terraria_core::xna::Color::default()`. Written by the sky / background code.
    pub color_of_the_skies: terraria_core::xna::Color,
    /// `ColorOfSurfaceBackgroundsBase` (Main.cs:2680), initially `terraria_core::xna::Color::WHITE`. Written by the sky / background code.
    pub color_of_surface_backgrounds_base: terraria_core::xna::Color,
    /// `ColorOfSurfaceBackgroundsModified` (Main.cs:2682), initially `terraria_core::xna::Color::WHITE`. Written by the sky / background code.
    pub color_of_surface_backgrounds_modified: terraria_core::xna::Color,
    /// `cloudAlpha` (Main.cs:1273), initially `0f32`. Written by the sky / background code.
    pub cloud_alpha: f32,
    /// `maxRaining` (Main.cs:1275), initially `0f32`. Written by the sky / background code.
    pub max_raining: f32,
    /// `oldMaxRaining` (Main.cs:1277), initially `0f32`. Written by the sky / background code.
    pub old_max_raining: f32,
    /// `lightning` (Main.cs:2656), initially `0f32`. Written by the sky / background code.
    pub lightning: f32,
    /// `windSpeedTarget` (Main.cs:1341), initially `0f32`. Written by the sky / background code.
    pub wind_speed_target: f32,
    /// `numClouds` (Main.cs:1335), initially `200`. Written by the sky / background code.
    pub num_clouds: i32,
    /// `cloudBGActive` (Main.cs:627), initially `0f32`. Written by the sky / background code.
    pub cloud_bg_active: f32,
    /// `cloudBGAlpha` (Main.cs:625), initially `0f32`. Written by the sky / background code.
    pub cloud_bg_alpha: f32,
    /// `cloudBGX` (Main.cs:623), initially `[0f32; 2]`. Written by the sky / background code.
    pub cloud_bgx: [f32; 2],
    /// `cloudBG` (Main.cs:629), initially `[112, 113]`. Written by the sky / background code.
    pub cloud_bg: [i32; 2],
    /// `checkForSpawns` (Main.cs:1289), initially `0`. Written by the sky / background code.
    pub check_for_spawns: i32,
    /// `invasionDelay` (Main.cs:1996), initially `0`. Written by the sky / background code.
    pub invasion_delay: i32,
    /// `invasionType` (Main.cs:1990), initially `0`. Written by the sky / background code.
    pub invasion_type: i32,
    /// `tenthAnniversaryWorld` (Main.cs:333), initially `false`. Written by the sky / background code.
    pub tenth_anniversary_world: bool,
    /// `drunkWorld` (Main.cs:329), initially `false`. Written by the sky / background code.
    pub drunk_world: bool,
    /// `notTheBeesWorld` (Main.cs:337), initially `false`. Written by the sky / background code.
    pub not_the_bees_world: bool,
    /// `vampireSeed` (Main.cs:347), initially `false`. Written by the sky / background code.
    pub vampire_seed: bool,
    /// `onlyShimmerOceanWorldsGeneration` (Main.cs (static, false outside the world generator)), initially `false`. Written by the sky / background code.
    pub only_shimmer_ocean_worlds_generation: bool,
    /// `numStars` (Main.cs:1327), initially `0`. Written by the sky / background code.
    pub num_stars: i32,
    /// `resetClouds` (Main.cs:1357), initially `true`. Written by the sky / background code.
    pub reset_clouds: bool,
    /// `mapFullscreen` (Main.cs:917), initially `false`. Written by the sky / background code.
    pub map_fullscreen: bool,
    /// `BackgroundEnabled` (Main.cs:807), initially `true`. Written by the sky / background code.
    pub background_enabled: bool,
    /// `background` (Main.cs:1235), initially `0`. Written by the sky / background code.
    pub background: i32,
    /// `bgStyle` (Main.cs:813), initially `0`. Written by the sky / background code.
    pub bg_style: i32,
    /// `bgDelay` (Main.cs:811), initially `0`. Written by the sky / background code.
    pub bg_delay: i32,
    /// `bgAlphaFrontLayer` (Main.cs:815), initially `[0f32; 16]`. Written by the sky / background code.
    pub bg_alpha_front_layer: [f32; 16],
    /// `bgAlphaFarBackLayer` (Main.cs:817), initially `[0f32; 16]`. Written by the sky / background code.
    pub bg_alpha_far_back_layer: [f32; 16],
    /// `bgFrame` (Main.cs:819), initially `[0; 16]`. Written by the sky / background code.
    pub bg_frame: [i32; 16],
    /// `bgFrameCounter` (Main.cs:821), initially `[0; 16]`. Written by the sky / background code.
    pub bg_frame_counter: [i32; 16],
    /// `instantBGTransitionCounter` (Main.cs:809), initially `2`. Written by the sky / background code.
    pub instant_bg_transition_counter: i32,
    /// `treeMntBGSet1` (Main.cs:631), initially `[0; 2]`. Written by the sky / background code.
    pub tree_mnt_bg_set1: [i32; 2],
    /// `treeMntBGSet2` (Main.cs:633), initially `[0; 2]`. Written by the sky / background code.
    pub tree_mnt_bg_set2: [i32; 2],
    /// `treeMntBGSet3` (Main.cs:635), initially `[0; 2]`. Written by the sky / background code.
    pub tree_mnt_bg_set3: [i32; 2],
    /// `treeMntBGSet4` (Main.cs:637), initially `[0; 2]`. Written by the sky / background code.
    pub tree_mnt_bg_set4: [i32; 2],
    /// `treeBGSet1` (Main.cs:639), initially `[0; 3]`. Written by the sky / background code.
    pub tree_bg_set1: [i32; 3],
    /// `treeBGSet2` (Main.cs:641), initially `[0; 3]`. Written by the sky / background code.
    pub tree_bg_set2: [i32; 3],
    /// `treeBGSet3` (Main.cs:643), initially `[0; 3]`. Written by the sky / background code.
    pub tree_bg_set3: [i32; 3],
    /// `treeBGSet4` (Main.cs:645), initially `[0; 3]`. Written by the sky / background code.
    pub tree_bg_set4: [i32; 3],
    /// `corruptBG` (Main.cs:647), initially `[0; 3]`. Written by the sky / background code.
    pub corrupt_bg: [i32; 3],
    /// `jungleBG` (Main.cs:649), initially `[0; 3]`. Written by the sky / background code.
    pub jungle_bg: [i32; 3],
    /// `snowMntBG` (Main.cs:651), initially `[0; 2]`. Written by the sky / background code.
    pub snow_mnt_bg: [i32; 2],
    /// `snowBG` (Main.cs:653), initially `[0; 3]`. Written by the sky / background code.
    pub snow_bg: [i32; 3],
    /// `hallowBG` (Main.cs:655), initially `[0; 3]`. Written by the sky / background code.
    pub hallow_bg: [i32; 3],
    /// `crimsonBG` (Main.cs:657), initially `[0; 3]`. Written by the sky / background code.
    pub crimson_bg: [i32; 3],
    /// `desertBackgroundSet` (Main.cs:659), initially `...`. Written by the sky / background code.
    pub desert_background_set: crate::main_time::BackgroundVariantSet,
    /// `mushroomBG` (Main.cs:661), initially `[0; 3]`. Written by the sky / background code.
    pub mushroom_bg: [i32; 3],
    /// `oceanBG` (Main.cs:663), initially `0`. Written by the sky / background code.
    pub ocean_bg: i32,
    /// `underworldBG` (Main.cs:665), initially `[0; 5]`. Written by the sky / background code.
    pub underworld_bg: [i32; 5],
    /// `caveBackX` (Main.cs:671), initially `[0; 4]`. Written by the sky / background code.
    pub cave_back_x: [i32; 4],
    /// `caveBackStyle` (Main.cs:673), initially `[0; 4]`. Written by the sky / background code.
    pub cave_back_style: [i32; 4],
    /// `iceBackStyle` (Main.cs:675), initially `0`. Written by the sky / background code.
    pub ice_back_style: i32,
    /// `hellBackStyle` (Main.cs:677), initially `0`. Written by the sky / background code.
    pub hell_back_style: i32,
    /// `jungleBackStyle` (Main.cs:679), initially `0`. Written by the sky / background code.
    pub jungle_back_style: i32,
    /// `undergroundBackground` (Main.cs:565), initially `0`. Written by the sky / background code.
    pub underground_background: i32,
    /// `oldUndergroundBackground` (Main.cs:567), initially `0`. Written by the sky / background code.
    pub old_underground_background: i32,
    /// `ugBackTransition` (Main.cs:1239), initially `0f32`. Written by the sky / background code.
    pub ug_back_transition: f32,
    /// `magmaBGFrame` (Main.cs:1069), initially `0`. Written by the sky / background code.
    pub magma_bg_frame: i32,
    /// `magmaBGFrameCounter` (Main.cs:1071), initially `0`. Written by the sky / background code.
    pub magma_bg_frame_counter: i32,
    /// `caveParallax` (Main.cs:1189), initially `0.88f32`. Written by the sky / background code.
    pub cave_parallax: f32,
    /// `hellBlackBoxBottom` (Main.cs:2632), initially `0f32`. Written by the sky / background code.
    pub hell_black_box_bottom: f32,
    /// `_drawBackground_backTexture` (Main.cs:2636), initially `[0; 7]`. Written by the sky / background code.
    pub draw_background_back_texture: [i32; 7],
    /// `_drawBackground_oldBackTexture` (Main.cs:2638), initially `[0; 7]`. Written by the sky / background code.
    pub draw_background_old_back_texture: [i32; 7],
    /// `_drawBackground_tempBack` (Main.cs:2640), initially `[0; 7]`. Written by the sky / background code.
    pub draw_background_temp_back: [i32; 7],
    /// `_drawbackground_colorSlices` (Main.cs:2634), initially `...`. Written by the sky / background code.
    pub drawbackground_color_slices: [terraria_core::xna::Color; 9],
    /// `bgParallax` (Main.cs:939), initially `0.0f64`. Written by the sky / background code.
    pub bg_parallax: f64,
    /// `bgScale` (Main.cs:2674), initially `1f32`. Written by the sky / background code.
    pub bg_scale: f32,
    /// `bgTopY` (Main.cs:949), initially `0f32`. Written by the sky / background code.
    pub bg_top_y: f32,
    /// `bgStartX` (Main.cs:941), initially `0`. Written by the sky / background code.
    pub bg_start_x: i32,
    /// `bgLoops` (Main.cs:943), initially `0`. Written by the sky / background code.
    pub bg_loops: i32,
    /// `bgStartY` (Main.cs:945), initially `0`. Written by the sky / background code.
    pub bg_start_y: i32,
    /// `bgLoopsY` (Main.cs:947), initially `0`. Written by the sky / background code.
    pub bg_loops_y: i32,
    /// `bgWidthScaled` (Main.cs:2676, (int)(1024f * bgScale)), initially `1024`. Written by the sky / background code.
    pub bg_width_scaled: i32,
    /// `scAdj` (Main.cs:2684), initially `0f32`. Written by the sky / background code.
    pub sc_adj: f32,
    /// `backgroundLayerTransitionSpeed` (Main.cs:2670), initially `0.05f32`. Written by the sky / background code.
    pub background_layer_transition_speed: f32,
    /// `SmoothedMushroomLightInfluence` (Main.cs:1359), initially `0f32`. Written by the sky / background code.
    pub smoothed_mushroom_light_influence: f32,
    /// `GraveyardVisualIntensity` (Main.cs:1403), initially `0f32`. Written by the sky / background code.
    pub graveyard_visual_intensity: f32,
    /// `LastCelestialBodyPosition` (Main.cs:2708), initially `terraria_core::xna::Vector2::ZERO`. Written by the sky / background code.
    pub last_celestial_body_position: terraria_core::xna::Vector2,
    /// `Main.ladyBugRainBoost` (Main.cs:2714, 0). Counted down by `Main.UpdateTime`; written by the critter code (not ported).
    pub lady_bug_rain_boost: i32,
    /// `Main.star` (`new Star[400]`, Main.cs:1693). Filled by `Star.SpawnStars`, moved by `Star.UpdateStars`.
    pub star: Vec<crate::star::Star>,
    /// `Star.dayCheck` (Star.cs, false)
    pub star_day_check: bool,
    /// `Star.starfallBoost` (Star.cs, 1f)
    pub starfall_boost: f32,
    /// `Star.starFallCount` (Star.cs, 0)
    pub star_fall_count: i32,
    /// `Main.cloud` (`new Cloud[200]`, Main.cs:1355)
    pub cloud: Vec<crate::cloud::Cloud>,
    /// `Cloud.rand` (`new UnifiedRandom()`, Cloud.cs: random seed; a fixed seed here)
    pub cloud_rand: terraria_core::UnifiedRandom,
    /// `Cloud.lastCameraCenter` (`Vector2?`, null)
    pub last_camera_center: Option<terraria_core::xna::Vector2>,
    /// `TextureAssets.Cloud[n].Width()/Height()` for the 42 cloud textures (`Images/Cloud_<n>`); filled at start-up from the files
    pub cloud_texture_size: Vec<(i32, i32)>,
    /// `Main.backgroundWidth` (`new int[maxBackgrounds]`, maxBackgrounds = 344). Set by `Main.LoadBackground`.
    pub background_width: [i32; 344],
    /// `Main.backgroundHeight` (`new int[maxBackgrounds]`). Set by `Main.LoadBackground`.
    pub background_height: [i32; 344],
    /// `TextureAssets.Background[i].State != AssetState.NotLoaded`, per background (what `Main.LoadBackground` tests)
    pub background_loaded: [bool; 344],
    /// `Main.SceneMetrics` (`Main._playerSceneMetrics`, Main.cs:1319): the tile census around the view
    pub scene_metrics: crate::scene_metrics::SceneMetrics,
    /// the `Zone*` fields of the local player, set by `Player.UpdateBiomes` (see `player_biomes.rs`)
    pub player_zones: crate::player_biomes::PlayerZones,
}

impl MainState {
    /// `Main.UnderworldLayer` (Main.cs:2971): `maxTilesY - 200`
    pub fn underworld_layer(&self) -> i32 {
        self.max_tiles_y - 200
    }

    /// `Main.WindForVisuals` (Main.cs:3004): `=> windSpeedCurrent`
    pub fn wind_for_visuals(&self) -> f32 {
        self.wind_speed_current
    }

    /// `Main.ShouldShowInvisibleBlocksAndWalls` (Main.cs:58992)
    pub fn should_show_invisible_blocks_and_walls(&self) -> bool {
        if !self.echo_monolith {
            return self.perspective_player_can_see_invisible_blocks;
        }
        true
    }
}

impl Deref for MainState {
    type Target = MainArrays;
    fn deref(&self) -> &MainArrays {
        &self.arrays
    }
}
impl DerefMut for MainState {
    fn deref_mut(&mut self) -> &mut MainArrays {
        &mut self.arrays
    }
}

/// Static fields of `Terraria.WorldGen`.
pub struct WorldGenState {
    /// `WorldGen.generatingWorld`
    pub generating_world: bool,
    /// `WorldGen.noTileActions`
    pub no_tile_actions: bool,
    /// `WorldGen.noLiquidCheck`
    pub no_liquid_check: bool,
    /// `WorldGen.destroyObject`
    pub destroy_object: bool,
    /// `WorldGen.mergeUp`, `mergeDown`, `mergeLeft`, `mergeRight` (scratch flags written by TileFrame)
    pub merge_up: bool,
    pub merge_down: bool,
    pub merge_left: bool,
    pub merge_right: bool,
    /// `WorldGen.tileReframeCount` (WorldGen.cs:4068; depth counter of the recursive neighbour re-framing, starts at 0)
    pub tile_reframe_count: i32,
    /// `WorldGen.tileCounts`
    pub tile_counts: Vec<i32>,
    /// `WorldGen.TreeTops` (`TreeTopsInfo._variations`, `new int[AreaId.Count]`, AreaId.Count = 13). Written by `WorldFile.LoadHeader`.
    pub tree_tops: [i32; 13],
    /// `WorldGen.hallowBG` (int, 0). Written by `WorldFile.LoadHeader` through `WorldGen.setBG`.
    pub hallow_bg: i32,
    /// `WorldGen.isGeneratingOrLoadingWorld` (`public static volatile bool`, false). Set around the world load's liquid settle
    /// (`liquid::load_world_settle`) and by world generation.
    pub is_generating_or_loading_world: bool,
    /// `treeBG1` (WorldGen.cs), initially `0`. Written by the sky / background code.
    pub tree_bg1: i32,
    /// `treeBG2` (WorldGen.cs), initially `0`. Written by the sky / background code.
    pub tree_bg2: i32,
    /// `treeBG3` (WorldGen.cs), initially `0`. Written by the sky / background code.
    pub tree_bg3: i32,
    /// `treeBG4` (WorldGen.cs), initially `0`. Written by the sky / background code.
    pub tree_bg4: i32,
    /// `corruptBG` (WorldGen.cs:4078), initially `0`. Written by the sky / background code.
    pub corrupt_bg: i32,
    /// `jungleBG` (WorldGen.cs:4080), initially `0`. Written by the sky / background code.
    pub jungle_bg: i32,
    /// `snowBG` (WorldGen.cs:4082), initially `0`. Written by the sky / background code.
    pub snow_bg: i32,
    /// `crimsonBG` (WorldGen.cs:4086), initially `0`. Written by the sky / background code.
    pub crimson_bg: i32,
    /// `desertBG` (WorldGen.cs), initially `0`. Written by the sky / background code.
    pub desert_bg: i32,
    /// `oceanBG` (WorldGen.cs:4090), initially `0`. Written by the sky / background code.
    pub ocean_bg: i32,
    /// `mushroomBG` (WorldGen.cs:4092), initially `0`. Written by the sky / background code.
    pub mushroom_bg: i32,
    /// `underworldBG` (WorldGen.cs:4094), initially `0`. Written by the sky / background code.
    pub underworld_bg: i32,
    /// `drunkWorldGen` (WorldGen.cs), initially `false`. Written by the sky / background code.
    pub drunk_world_gen: bool,
    /// `remixWorldGen` (WorldGen.cs), initially `false`. Written by the sky / background code.
    pub remix_world_gen: bool,
    /// `notTheBees` (WorldGen.cs), initially `false`. Written by the sky / background code.
    pub not_the_bees: bool,
    /// `BackgroundsCache` (WorldGen.cs), initially `...`. Written by the sky / background code.
    pub backgrounds_cache: crate::main_time::BackgroundChangeFlashInfo,
}

pub struct Game {
    pub main: MainState,
    /// Static fields of `Terraria.Lighting` and the engine behind them (see `lighting.rs`).
    pub lighting: crate::lighting::LightingState,
    pub world_gen: WorldGenState,
    /// Static fields of `Terraria.Collision` (see `collision/mod.rs`).
    pub collision: crate::collision::CollisionState,
    /// Static fields of `Terraria.Liquid` / `LiquidBuffer`, `Main.liquid`, `Main.liquidBuffer` (see `liquid.rs`).
    pub liquid: crate::liquid::LiquidState,
}

impl Game {
    /// State as it is after `Main.Initialize()` with no world loaded.
    pub fn new() -> Self {
        let arrays = MainArrays::default();
        let tile_count = arrays.tile_solid.len();
        Self {
            lighting: crate::lighting::LightingState::new(),
            main: MainState {
                tile: TileMap::new(1, 1),
                arrays,
                max_tiles_x: 8401,
                max_tiles_y: 2401,
                left_world: 0.0,
                right_world: 134400.0,
                top_world: 0.0,
                bottom_world: 38400.0,
                world_surface: 0.0,
                rock_layer: 0.0,
                spawn_tile_x: 0,
                spawn_tile_y: 0,
                dungeon_x: 0,
                dungeon_y: 0,
                world_name: String::new(),
                // Main.rand is created with the tick count in the original; a fixed seed keeps runs repeatable
                // until the port needs wall-clock seeding.
                rand: UnifiedRandom::new(0),
                net_mode: 0,
                ded_serv: false,
                game_menu: true,
                time: 13500.0,
                day_time: true,
                hard_mode: false,
                game_mode: 0,
                screen_position: (0.0, 0.0),
                screen_width: 1152,
                screen_height: 864,
                echo_monolith: false,
                perspective_player_can_see_invisible_blocks: false,
                get_good_world: false,
                dont_starve_world: false,
                render: false,
                render_count: 0,
                render_now: false,
                game_paused: false,
                map_delay: 2,
                map_enabled: true,
                update_map: None,
                map_helper_scene_area: terraria_core::xna::Rectangle::EMPTY,
                water_style: 0,
                game_update_count: 0,
                mouse_text_color: 0,
                disco_r: 255,
                disco_g: 0,
                disco_b: 0,
                demon_torch: 1f32,
                shimmer_darken: 0f32,
                sundial_cooldown: 0,
                moondial_cooldown: 0,
                cage_frames: 25,
                jellyfish_cage_mode: [[0; 25]; 3],
                liquid_alpha: [0f32; 15],
                tile_color: terraria_core::xna::Color::default(),
                raining: false,
                time_for_visual_effects: 0.0,
                global_time_wrapped_hourly: 0f32,
                my_player: 0,
                scene_state_air_light_decay: 1f32,
                scene_state_solid_light_decay: 1f32,
                tree_x: [0; 3],
                tree_style: [0; 4],
                remix_world: false,
                wind_speed_current: 0f32,
                moon_phase: 0,
                blood_moon: false,
                eclipse: false,
                moon_type: 0,
                pumpkin_moon: false,
                snow_moon: false,
                day_rate: 1,
                desired_world_tiles_update_rate: 1,
                fast_forward_time_to_dusk: false,
                fast_forward_time_to_dawn: false,
                sun_mod_y: 0,
                moon_mod_y: 0,
                eclipse_light: 0f32,
                atmo: 0f32,
                color_of_the_skies: terraria_core::xna::Color::default(),
                color_of_surface_backgrounds_base: terraria_core::xna::Color::WHITE,
                color_of_surface_backgrounds_modified: terraria_core::xna::Color::WHITE,
                cloud_alpha: 0f32,
                max_raining: 0f32,
                old_max_raining: 0f32,
                lightning: 0f32,
                wind_speed_target: 0f32,
                num_clouds: 200,
                cloud_bg_active: 0f32,
                cloud_bg_alpha: 0f32,
                cloud_bgx: [0f32; 2],
                cloud_bg: [112, 113],
                check_for_spawns: 0,
                invasion_delay: 0,
                invasion_type: 0,
                tenth_anniversary_world: false,
                drunk_world: false,
                not_the_bees_world: false,
                vampire_seed: false,
                only_shimmer_ocean_worlds_generation: false,
                num_stars: 0,
                reset_clouds: true,
                map_fullscreen: false,
                background_enabled: true,
                background: 0,
                bg_style: 0,
                bg_delay: 0,
                bg_alpha_front_layer: [0f32; 16],
                bg_alpha_far_back_layer: [0f32; 16],
                bg_frame: [0; 16],
                bg_frame_counter: [0; 16],
                instant_bg_transition_counter: 2,
                tree_mnt_bg_set1: [0; 2],
                tree_mnt_bg_set2: [0; 2],
                tree_mnt_bg_set3: [0; 2],
                tree_mnt_bg_set4: [0; 2],
                tree_bg_set1: [0; 3],
                tree_bg_set2: [0; 3],
                tree_bg_set3: [0; 3],
                tree_bg_set4: [0; 3],
                corrupt_bg: [0; 3],
                jungle_bg: [0; 3],
                snow_mnt_bg: [0; 2],
                snow_bg: [0; 3],
                hallow_bg: [0; 3],
                crimson_bg: [0; 3],
                desert_background_set: crate::main_time::BackgroundVariantSet::new(),
                mushroom_bg: [0; 3],
                ocean_bg: 0,
                underworld_bg: [0; 5],
                cave_back_x: [0; 4],
                cave_back_style: [0; 4],
                ice_back_style: 0,
                hell_back_style: 0,
                jungle_back_style: 0,
                underground_background: 0,
                old_underground_background: 0,
                ug_back_transition: 0f32,
                magma_bg_frame: 0,
                magma_bg_frame_counter: 0,
                cave_parallax: 0.88f32,
                hell_black_box_bottom: 0f32,
                draw_background_back_texture: [0; 7],
                draw_background_old_back_texture: [0; 7],
                draw_background_temp_back: [0; 7],
                drawbackground_color_slices: [terraria_core::xna::Color::default(); 9],
                bg_parallax: 0.0f64,
                bg_scale: 1f32,
                bg_top_y: 0f32,
                bg_start_x: 0,
                bg_loops: 0,
                bg_start_y: 0,
                bg_loops_y: 0,
                bg_width_scaled: 1024,
                sc_adj: 0f32,
                background_layer_transition_speed: 0.05f32,
                smoothed_mushroom_light_influence: 0f32,
                graveyard_visual_intensity: 0f32,
                last_celestial_body_position: terraria_core::xna::Vector2::ZERO,
                lady_bug_rain_boost: 0,
                star: vec![crate::star::Star::default(); 400],
                star_day_check: false,
                starfall_boost: 1f32,
                star_fall_count: 0,
                cloud: vec![crate::cloud::Cloud::default(); 200],
                cloud_rand: terraria_core::UnifiedRandom::new(0),
                last_camera_center: None,
                cloud_texture_size: vec![(0, 0); 42],
                background_width: [0; 344],
                background_height: [0; 344],
                background_loaded: [false; 344],
                scene_metrics: crate::scene_metrics::SceneMetrics::new(),
                player_zones: crate::player_biomes::PlayerZones::default(),
            },
            world_gen: WorldGenState {
                generating_world: false,
                no_tile_actions: false,
                no_liquid_check: false,
                destroy_object: false,
                merge_up: false,
                merge_down: false,
                merge_left: false,
                merge_right: false,
                tile_reframe_count: 0,
                tile_counts: vec![0; tile_count],
                tree_tops: [0; 13],
                hallow_bg: 0,
                is_generating_or_loading_world: false,
                tree_bg1: 0,
                tree_bg2: 0,
                tree_bg3: 0,
                tree_bg4: 0,
                corrupt_bg: 0,
                jungle_bg: 0,
                snow_bg: 0,
                crimson_bg: 0,
                desert_bg: 0,
                ocean_bg: 0,
                mushroom_bg: 0,
                underworld_bg: 0,
                drunk_world_gen: false,
                remix_world_gen: false,
                not_the_bees: false,
                backgrounds_cache: crate::main_time::BackgroundChangeFlashInfo::new(),
            },
            collision: crate::collision::CollisionState::new(),
            liquid: crate::liquid::LiquidState::new(),
        }
    }
}

impl Default for Game {
    fn default() -> Self {
        Self::new()
    }
}
