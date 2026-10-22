//! `Terraria.GameContent.Drawing.TileDrawing` (GameContent/Drawing/TileDrawing.cs): draws the world's tiles through
//! `Main.tileBatch`, one layer for non-solid tiles and one for solid tiles.
//!
//! Ported here: `Draw`, `DrawSingleTile`, `DrawBasicTile`, `DrawSingleTile_SlicedBlock`, `DrawXmasTree`, `DrawTile_BackRope`,
//! `IsVisible`, the no-paint paths of `GetTileDrawTexture`, `GetFinalLight`, `DrawTiles_GetLightOverride`,
//! `GetScreenDrawArea`, `CacheSpecialDraws_Part1/2` and the tree / cage frame helpers. `GetTileDrawData` is in
//! `tile_drawing_get_tile_draw_data.rs`. Everything else the original calls from these is `unported!` at its call site.

use std::collections::HashMap;

use terraria_core::xna::{Color, Point, Rectangle, Vector2, Vector3, Vector4};
use terraria_core::{Tile, UnifiedRandom};
use terraria_data::sets::tile_id::sets as tile_sets;
use terraria_sim::unported;

use crate::data_structures::tile_draw_info::TileDrawInfo;
use crate::draw_context::DrawContext;
use crate::game_content::drawing::draw_black_helper::DrawBlackHelper;
use crate::game_content::drawing::nature_renderer::NatureRenderer;
use crate::game_content::drawing::wind_grid::WindGrid;
use crate::game_content::drawing::tile_drawing_base::{TileDrawingBase, DRAW_OWN_BLACKS};
use crate::gfx::{SpriteEffects, Texture2D};
use crate::main_draw::shine;
use crate::pending::{
    lighting_get_color, lighting_get_color_4_slice, lighting_get_color_9_slice, lighting_not_retro, lighting_update_every_frame, utils_get_lerp_value, world_gen_is_alchemy_plant_harvestable,
    world_gen_is_palm_oasis_tree, world_gen_is_rope_out, VertexColors,
};
use crate::testing::debug_options;

/// `TileDrawing.Layer_LiquidBehindTiles` .. `Layer_OverTiles` (TileDrawing.cs:797-800)
pub const LAYER_LIQUID_BEHIND_TILES: u32 = 0;
pub const LAYER_BEHIND_TILES: u32 = 1;
pub const LAYER_TILES: u32 = 2;
pub const LAYER_OVER_TILES: u32 = 3;

/// `TileDrawing.TileCounterType` (TileDrawing.cs:21)
/// `BiomeGrass` and `Count` are members of the original enum that nothing in the ported part reads
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(usize)]
pub enum TileCounterType {
    Tree,
    WindyGrass,
    MultiTileGrass,
    MultiTileVine,
    Vine,
    BiomeGrass,
    VoidLens,
    ReverseVine,
    TeleportationPylon,
    MasterTrophy,
    AnyDirectionalGrass,
    Count,
}

/// `TilePaintSystemV2.TileVariationkey`
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TileVariationKey {
    pub tile_type: i32,
    pub tile_style: i32,
    pub paint_color: i32,
}

/// `TilePaintSystemV2.CageTopVariationkey`
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CageTopVariationKey {
    pub cage_style: i32,
    pub paint_color: i32,
}

/// `Main.TilesRenderer`
pub struct TileDrawing {
    pub base: TileDrawingBase,
    /// `_specialsCount` / `_specialPositions`: what `AddSpecialPoint` recorded, for the consumers that are not ported yet
    pub specials_count: [i32; 11],
    pub special_positions: Vec<Vec<Point>>,
    /// `_displayDollTileEntityPositions` .. `_weaponRackTileEntityPositions`, `_chestPositions`
    display_doll_tile_entity_positions: HashMap<Point, i32>,
    hat_rack_tile_entity_positions: HashMap<Point, i32>,
    training_dummy_tile_entity_positions: HashMap<Point, i32>,
    item_frame_tile_entity_positions: HashMap<Point, i32>,
    dead_cells_display_jar_tile_entity_positions: HashMap<Point, i32>,
    food_platter_tile_entity_positions: HashMap<Point, i32>,
    weapon_rack_tile_entity_positions: HashMap<Point, i32>,
    chest_positions: HashMap<Point, i32>,
    /// `_specialTilesCount`, `_specialTileX`, `_specialTileY` (the legacy special list)
    pub special_tiles_count: i32,
    pub special_tile_x: Vec<i32>,
    pub special_tile_y: Vec<i32>,
    /// `_rand`
    pub(super) rand: UnifiedRandom,
    /// `_leafFrequency` (TileDrawing.cs:57, `100000`)
    pub(super) leaf_frequency: i32,
    /// `_treeWindCounter`, `_grassWindCounter`, `_sunflowerWindCounter`, `_vineWindCounter` (`double`, 0)
    pub(super) tree_wind_counter: f64,
    pub(super) grass_wind_counter: f64,
    pub(super) sunflower_wind_counter: f64,
    pub(super) vine_wind_counter: f64,
    /// `_windGrid`
    pub(super) wind_grid: WindGrid,
    /// `_natureRenderer` (a `NextNatureRenderer`)
    pub(super) nature_renderer: NatureRenderer,
    /// `_shouldShowInvisibleBlocks`, `_shouldShowInvisibleBlocks_LastFrame`
    pub(super) should_show_invisible_blocks: bool,
    should_show_invisible_blocks_last_frame: bool,
    vine_roots_positions: Vec<Point>,
    reverse_vine_roots_positions: Vec<Point>,
    /// `_martianGlow` .. `_violetMossGlow` (TileDrawing.cs:128-140)
    pub(super) martian_glow: Color,
    pub(super) meteor_glow: Color,
    pub(super) lava_moss_glow: Color,
    pub(super) krypton_moss_glow: Color,
    pub(super) xenon_moss_glow: Color,
    pub(super) argon_moss_glow: Color,
    pub(super) violet_moss_glow: Color,
    /// `_isActiveAndNotPaused`
    pub(super) is_active_and_not_paused: bool,
    /// `_highQualityLightingRequirement`, `_mediumQualityLightingRequirement`
    high_quality_lighting_requirement: Color,
    medium_quality_lighting_requirement: Color,
    draw_black_helper: DrawBlackHelper,
    last_paint_lookup_key: TileVariationKey,
    last_paint_lookup_texture: Option<Texture2D>,
    /// `_glowPaintColorSlices`: nine `Vector3.One`
    glow_paint_color_slices: [Vector3; 9],
}

/// `TileDrawing._zero`
pub(super) const ZERO: Vector2 = Vector2::ZERO;

impl TileDrawing {
    /// `new TileDrawing(TilePaintSystemV2 paintSystem)` (TileDrawing.cs:188). `new UnifiedRandom()` seeds from the clock;
    /// `_rand` only decides when dust is emitted, which is not ported, so a fixed seed is used.
    pub fn new() -> Self {
        Self {
            base: TileDrawingBase::default(),
            specials_count: [0; 11],
            special_positions: vec![Vec::new(); 11],
            display_doll_tile_entity_positions: HashMap::new(),
            hat_rack_tile_entity_positions: HashMap::new(),
            training_dummy_tile_entity_positions: HashMap::new(),
            item_frame_tile_entity_positions: HashMap::new(),
            dead_cells_display_jar_tile_entity_positions: HashMap::new(),
            food_platter_tile_entity_positions: HashMap::new(),
            weapon_rack_tile_entity_positions: HashMap::new(),
            chest_positions: HashMap::new(),
            special_tiles_count: 0,
            special_tile_x: Vec::new(),
            special_tile_y: Vec::new(),
            rand: UnifiedRandom::new(0),
            leaf_frequency: 100000,
            tree_wind_counter: 0.0,
            grass_wind_counter: 0.0,
            sunflower_wind_counter: 0.0,
            vine_wind_counter: 0.0,
            wind_grid: WindGrid::new(),
            nature_renderer: NatureRenderer::new(),
            should_show_invisible_blocks: false,
            should_show_invisible_blocks_last_frame: false,
            vine_roots_positions: Vec::new(),
            reverse_vine_roots_positions: Vec::new(),
            martian_glow: Color::new(0, 0, 0, 0),
            meteor_glow: Color::new(100, 100, 100, 0),
            lava_moss_glow: Color::new(150, 100, 50, 0),
            krypton_moss_glow: Color::new(0, 200, 0, 0),
            xenon_moss_glow: Color::new(0, 180, 250, 0),
            argon_moss_glow: Color::new(225, 0, 125, 0),
            violet_moss_glow: Color::new(150, 0, 250, 0),
            is_active_and_not_paused: false,
            high_quality_lighting_requirement: Color::TRANSPARENT,
            medium_quality_lighting_requirement: Color::TRANSPARENT,
            draw_black_helper: DrawBlackHelper::new(0, ZERO),
            last_paint_lookup_key: TileVariationKey { tile_type: 0, tile_style: 0, paint_color: 0 },
            last_paint_lookup_texture: None,
            glow_paint_color_slices: [Vector3::ONE; 9],
        }
    }

    /// `AddSpecialPoint(int x, int y, TileCounterType type)` (TileDrawing.cs:183)
    pub(super) fn add_special_point(&mut self, x: i32, y: i32, type_: TileCounterType) {
        let t = type_ as usize;
        self.special_positions[t].push(Point { x, y });
        self.specials_count[t] += 1;
    }

    /// `Update()` (TileDrawing.cs:371). `Main.WindForVisuals` is `g.main.wind_speed_current` (written by the weather update,
    /// which is not ported; the world load sets it from the file).
    pub fn update(&mut self, ctx: &mut DrawContext) {
        if !ctx.g.main.ded_serv {
            let mut num = ctx.g.main.wind_for_visuals().abs() as f64;
            num = utils_get_lerp_value(0.08, 1.2, num as f32, true) as f64;
            self.tree_wind_counter += 1.0 / 240.0 + 1.0 / 240.0 * num * 2.0;
            self.grass_wind_counter += 1.0 / 180.0 + 1.0 / 180.0 * num * 4.0;
            self.sunflower_wind_counter += 1.0 / 420.0 + 1.0 / 420.0 * num * 5.0;
            self.vine_wind_counter += 1.0 / 120.0 + 1.0 / 120.0 * num * 0.4000000059604645;
            self.update_leaf_frequency(ctx);
            self.ensure_wind_grid_size(ctx);
            self.wind_grid.update(ctx.g, ctx.state.settings_enabled_tiles_sway_in_wind);
            self.should_show_invisible_blocks = ctx.g.main.should_show_invisible_blocks_and_walls();
            if self.should_show_invisible_blocks_last_frame != self.should_show_invisible_blocks {
                self.should_show_invisible_blocks_last_frame = self.should_show_invisible_blocks;
                unported!("Main.sectionManager.SetAllFramedSectionsAsNeedingRefresh");
            }
        }
    }

    /// `EnsureWindGridSize()` (TileDrawing.cs:7913)
    fn ensure_wind_grid_size(&mut self, ctx: &mut DrawContext) {
        let (_draw_off_set, first_tile_x, last_tile_x, first_tile_y, last_tile_y) = get_screen_draw_area(ctx, !ctx.state.draw_to_screen);
        self.wind_grid.set_size(last_tile_x - first_tile_x, last_tile_y - first_tile_y);
    }

    /// `ClearSpecialBlockCounts()` (TileDrawing.cs:393)
    fn clear_special_block_counts(&mut self) {
        self.vine_roots_positions.clear();
        self.reverse_vine_roots_positions.clear();
        for t in [3, 2, 6, 4, 1, 10, 0, 7, 8, 9] {
            self.specials_count[t] = 0;
            self.special_positions[t].clear();
        }
    }

    /// `Draw(bool solidLayer, bool intoRenderTargets, int waterStyleOverride = -1)` (TileDrawing.cs:470)
    pub fn draw(&mut self, ctx: &mut DrawContext, solid_layer: bool, _into_render_targets: bool, water_style_override: i32) {
        self.is_active_and_not_paused = unported!("FocusHelper.AllowTileDrawingToEmitEffects", true); // neutral for the rest of the flow: effects are attempted and reported where they are emitted
        unported!("Main.SceneMetrics.PerspectivePlayer"); // _perspectivePlayer
        let unscaled_position = Vector2::new(ctx.g.main.screen_position.0, ctx.g.main.screen_position.1);
        if !solid_layer {
            ctx.state.critter_cage = false;
        }
        self.ensure_wind_grid_size(ctx);
        self.clear_legacy_cached_draws();
        self.clear_cached_tile_draws(solid_layer);
        let num = 255.0_f32 * (1.0 - ctx.state.gfx_quality) + 30.0_f32 * ctx.state.gfx_quality;
        // (byte)float: the CLR converts through int, so values above 255 wrap instead of saturating
        self.high_quality_lighting_requirement.set_r(num as i32 as u8);
        self.high_quality_lighting_requirement.set_g((num as f64 * 1.1) as i32 as u8);
        self.high_quality_lighting_requirement.set_b((num as f64 * 1.2) as i32 as u8);
        let num2 = 50.0_f32 * (1.0 - ctx.state.gfx_quality) + 2.0_f32 * ctx.state.gfx_quality;
        self.medium_quality_lighting_requirement.set_r(num2 as i32 as u8);
        self.medium_quality_lighting_requirement.set_g((num2 as f64 * 1.1) as i32 as u8);
        self.medium_quality_lighting_requirement.set_b((num2 as f64 * 1.2) as i32 as u8);
        if debug_options::DEV_LIGHT_TILES_CHEAT {
            self.high_quality_lighting_requirement.set_r(255);
            self.high_quality_lighting_requirement.set_g(255);
            self.high_quality_lighting_requirement.set_b(255);
            self.medium_quality_lighting_requirement.set_r(255);
            self.medium_quality_lighting_requirement.set_g(255);
            self.medium_quality_lighting_requirement.set_b(255);
        }
        let (draw_off_set, first_tile_x, last_tile_x, first_tile_y, last_tile_y) = get_screen_draw_area(ctx, !ctx.state.draw_to_screen);
        self.draw_black_helper = DrawBlackHelper::new(LAYER_TILES, draw_off_set);
        let b = (100.0_f32 + 150.0_f32 * ctx.state.martian_light) as i32 as u8;
        self.martian_glow = Color::new(b, b, b, 0);
        self.last_paint_lookup_key = TileVariationKey { tile_type: -1, tile_style: 0, paint_color: 0 };
        for i in first_tile_y..last_tile_y + 4 {
            for j in first_tile_x - 2..last_tile_x + 2 {
                let tile = *ctx.g.main.tile.get(j, i);
                if !tile.active() || self.is_tile_draw_layer_solid(ctx, tile.type_) != solid_layer || (debug_options::SHOW_UNBREAKABLE_WALL && tile.wall == 350) {
                    continue;
                }
                if solid_layer {
                    ctx.tile_batch.set_layer(LAYER_LIQUID_BEHIND_TILES, 0);
                    crate::game_content::drawing::tile_drawing_liquid::draw_tile_liquid_behind_tile(ctx, solid_layer, water_style_override, unscaled_position, draw_off_set, j, i, &tile);
                }
                ctx.tile_batch.set_layer(LAYER_TILES, 0);
                let type_ = tile.type_;
                let frame_x = tile.frame_x;
                let frame_y = tile.frame_y;
                ctx.tile_texture(type_ as i32); // `if (!TextureAssets.Tile[type].IsLoaded) Main.instance.LoadTiles(type)`
                match type_ {
                    52 | 62 | 115 | 205 | 382 | 528 | 636 | 638 => {
                        self.crawl_to_top_of_vine_and_add_special_point(ctx, i, j);
                        continue;
                    }
                    549 => {
                        self.crawl_to_bottom_of_reverse_vine_and_add_special_point(ctx, i, j);
                        continue;
                    }
                    34 => {
                        if frame_x % 54 == 0 && frame_y % 54 == 0 {
                            self.add_special_point(j, i, TileCounterType::MultiTileVine);
                        }
                        continue;
                    }
                    698 => {
                        if frame_x % 18 == 0 && frame_y == 0 {
                            self.add_special_point(j, i, TileCounterType::MultiTileVine);
                        }
                        continue;
                    }
                    454 => {
                        if frame_x % 72 == 0 && frame_y % 54 == 0 {
                            self.add_special_point(j, i, TileCounterType::MultiTileVine);
                        }
                        continue;
                    }
                    42 | 270 | 271 | 572 | 581 | 660 => {
                        if frame_x % 18 == 0 && frame_y % 36 == 0 {
                            self.add_special_point(j, i, TileCounterType::MultiTileVine);
                        }
                        continue;
                    }
                    91 => {
                        if frame_x % 18 == 0 && frame_y % 54 == 0 {
                            self.add_special_point(j, i, TileCounterType::MultiTileVine);
                        }
                        continue;
                    }
                    95 | 126 | 444 => {
                        if frame_x % 36 == 0 && frame_y % 36 == 0 {
                            self.add_special_point(j, i, TileCounterType::MultiTileVine);
                        }
                        continue;
                    }
                    465 | 591 | 592 => {
                        if frame_x % 36 == 0 && frame_y % 54 == 0 {
                            self.add_special_point(j, i, TileCounterType::MultiTileVine);
                        }
                        continue;
                    }
                    27 => {
                        if frame_x % 36 == 0 && frame_y == 0 {
                            self.add_special_point(j, i, TileCounterType::MultiTileGrass);
                        }
                        continue;
                    }
                    236 | 238 | 702 => {
                        if frame_x % 36 == 0 && frame_y == 0 {
                            self.add_special_point(j, i, TileCounterType::MultiTileGrass);
                        }
                        continue;
                    }
                    233 => {
                        if frame_y == 0 && frame_x % 54 == 0 {
                            self.add_special_point(j, i, TileCounterType::MultiTileGrass);
                        }
                        if frame_y == 36 && frame_x % 36 == 0 {
                            self.add_special_point(j, i, TileCounterType::MultiTileGrass);
                        }
                        continue;
                    }
                    652 => {
                        if frame_x % 36 == 0 {
                            self.add_special_point(j, i, TileCounterType::MultiTileGrass);
                        }
                        continue;
                    }
                    651 => {
                        if frame_x % 54 == 0 {
                            self.add_special_point(j, i, TileCounterType::MultiTileGrass);
                        }
                        continue;
                    }
                    530 => {
                        if frame_x < 270 {
                            if frame_x % 54 == 0 && frame_y == 0 {
                                self.add_special_point(j, i, TileCounterType::MultiTileGrass);
                            }
                            continue;
                        }
                    }
                    705 => {
                        if frame_x % 486 < 270 {
                            if frame_x % 54 == 0 && frame_y % 36 == 0 {
                                self.add_special_point(j, i, TileCounterType::MultiTileGrass);
                            }
                            continue;
                        }
                    }
                    485 | 489 | 490 => {
                        if frame_y == 0 && frame_x % 36 == 0 {
                            self.add_special_point(j, i, TileCounterType::MultiTileGrass);
                        }
                        continue;
                    }
                    521 | 522 | 523 | 524 | 525 | 526 | 527 => {
                        if frame_y == 0 && frame_x % 36 == 0 {
                            self.add_special_point(j, i, TileCounterType::MultiTileGrass);
                        }
                        continue;
                    }
                    493 => {
                        if frame_y == 0 && frame_x % 18 == 0 {
                            self.add_special_point(j, i, TileCounterType::MultiTileGrass);
                        }
                        continue;
                    }
                    519 => {
                        if frame_x / 18 <= 4 {
                            self.add_special_point(j, i, TileCounterType::MultiTileGrass);
                        }
                        continue;
                    }
                    373 | 374 | 375 | 461 | 709 => {
                        unported!("TileDrawing.EmitLiquidDrops");
                        continue;
                    }
                    491 => {
                        if frame_x == 18 && frame_y == 18 {
                            self.add_special_point(j, i, TileCounterType::VoidLens);
                        }
                    }
                    597 => {
                        if frame_x % 54 == 0 && frame_y == 0 {
                            self.add_special_point(j, i, TileCounterType::TeleportationPylon);
                        }
                    }
                    617 => {
                        if frame_x % 54 == 0 && frame_y % 72 == 0 {
                            self.add_special_point(j, i, TileCounterType::MasterTrophy);
                        }
                    }
                    184 => {
                        self.add_special_point(j, i, TileCounterType::AnyDirectionalGrass);
                        continue;
                    }
                    _ => {
                        if self.should_sway_in_wind(ctx, j, i, &tile) {
                            self.add_special_point(j, i, TileCounterType::WindyGrass);
                            continue;
                        }
                    }
                }
                self.draw_single_tile(ctx, unscaled_position, draw_off_set, j, i);
            }
        }
        self.draw_black_helper.end_strip(ctx);
        self.base.restart_layered_batch(ctx);
        if solid_layer {
            unported!("Main.DrawTileCracks(1, hitReplace)");
            unported!("Main.DrawTileCracks(1, hitTile)");
            self.base.restart_sprite_batch(ctx);
        }
        self.draw_special_tiles_legacy(ctx, unscaled_position, ZERO);
        unported!("TileObject.DrawPreview"); // `if (TileObject.objectPreview.Active && ...)`: no placement preview without a player
    }

    /// `CrawlToTopOfVineAndAddSpecialPoint(int j, int i)` (TileDrawing.cs:750)
    fn crawl_to_top_of_vine_and_add_special_point(&mut self, ctx: &mut DrawContext, j: i32, i: i32) {
        let mut y = j;
        let mut num = j - 1;
        while num > 0 {
            let tile = ctx.g.main.tile.try_get(i, num);
            if unported!("WorldGen.BottomEdgeCanBeAttachedTo", false) || !tile.map(|t| t.active()).unwrap_or(false) {
                y = num + 1;
                break;
            }
            num -= 1;
        }
        let item = Point { x: i, y };
        if !self.vine_roots_positions.contains(&item) {
            self.vine_roots_positions.push(item);
            self.add_special_point(i, y, TileCounterType::Vine);
        }
    }

    /// `CrawlToBottomOfReverseVineAndAddSpecialPoint(int j, int i)` (TileDrawing.cs:770)
    fn crawl_to_bottom_of_reverse_vine_and_add_special_point(&mut self, ctx: &mut DrawContext, j: i32, i: i32) {
        let mut y = j;
        let mut k = j;
        while k < ctx.g.main.max_tiles_y {
            let tile = ctx.g.main.tile.try_get(i, k);
            if unported!("WorldGen.TopEdgeCanBeAttachedTo", false) || !tile.map(|t| t.active()).unwrap_or(false) {
                y = k - 1;
                break;
            }
            k += 1;
        }
        let item = Point { x: i, y };
        if !self.reverse_vine_roots_positions.contains(&item) {
            self.reverse_vine_roots_positions.push(item);
            self.add_special_point(i, y, TileCounterType::ReverseVine);
        }
    }

    /// `DrawSingleTile(Vector2 screenPosition, Vector2 screenOffset, int tileX, int tileY)` (TileDrawing.cs:872)
    fn draw_single_tile(&mut self, ctx: &mut DrawContext, screen_position: Vector2, screen_offset: Vector2, tile_x: i32, tile_y: i32) {
        let mut tile_draw_info = TileDrawInfo::new();
        tile_draw_info.tile_cache = *ctx.g.main.tile.get(tile_x, tile_y);
        tile_draw_info.type_cache = tile_draw_info.tile_cache.type_;
        tile_draw_info.tile_frame_x = tile_draw_info.tile_cache.frame_x;
        tile_draw_info.tile_frame_y = tile_draw_info.tile_cache.frame_y;
        tile_draw_info.tile_light = lighting_get_color(ctx.g, tile_x, tile_y);
        if tile_draw_info.tile_cache.liquid > 0 && tile_draw_info.tile_cache.type_ == 518 {
            return;
        }
        let tile_cache = tile_draw_info.tile_cache;
        let type_cache = tile_draw_info.type_cache;
        self.get_tile_draw_data(ctx, tile_x, tile_y, &tile_cache, type_cache, &mut tile_draw_info);
        if tile_draw_info.tile_top < 0 {
            ctx.tile_batch.set_layer(LAYER_OVER_TILES, 0);
        } else if tile_draw_info.tile_top + tile_draw_info.tile_height <= 16 {
            ctx.tile_batch.set_layer(LAYER_TILES, 0);
        } else {
            ctx.tile_batch.set_layer(LAYER_BEHIND_TILES, 0);
        }
        tile_draw_info.draw_texture = self.get_tile_draw_texture(ctx, &tile_cache, tile_x, tile_y);
        let mut highlight_texture: Option<Texture2D> = None;
        let mut highlight_color = Color::TRANSPARENT;
        if tile_sets::HAS_OUTLINES[type_cache as usize] {
            self.get_tile_outline_info(ctx, tile_x, tile_y, type_cache, &mut tile_draw_info.tile_light, &mut highlight_texture, &mut highlight_color);
        }
        if unported!("Player.dangerSense", false) && unported!("TileDrawing.IsTileDangerous", false) {
            if tile_draw_info.tile_light.r() < 255 {
                tile_draw_info.tile_light.set_r(255);
            }
            if tile_draw_info.tile_light.g() < 50 {
                tile_draw_info.tile_light.set_g(50);
            }
            if tile_draw_info.tile_light.b() < 50 {
                tile_draw_info.tile_light.set_b(50);
            }
            if self.is_active_and_not_paused && self.rand.next(30) == 0 {
                unported!("Dust.NewDust");
            }
        }
        if unported!("Player.findTreasure", false) && unported!("Main.IsTileSpelunkable", false) {
            if tile_draw_info.tile_light.r() < 200 {
                tile_draw_info.tile_light.set_r(200);
            }
            if tile_draw_info.tile_light.g() < 170 {
                tile_draw_info.tile_light.set_g(170);
            }
            if self.is_active_and_not_paused && self.rand.next(60) == 0 {
                unported!("Dust.NewDust");
            }
        }
        if unported!("Player.biomeSight", false) {
            let mut sight_color = Color::WHITE;
            if unported!("Main.IsTileBiomeSightable", false) {
                if tile_draw_info.tile_light.r() < sight_color.r() {
                    tile_draw_info.tile_light.set_r(sight_color.r());
                }
                if tile_draw_info.tile_light.g() < sight_color.g() {
                    tile_draw_info.tile_light.set_g(sight_color.g());
                }
                if tile_draw_info.tile_light.b() < sight_color.b() {
                    tile_draw_info.tile_light.set_b(sight_color.b());
                }
                if self.is_active_and_not_paused && self.rand.next(480) == 0 {
                    sight_color = Color::WHITE;
                    let _ = sight_color;
                    unported!("Dust.NewDust");
                }
            }
        }
        if self.is_active_and_not_paused && (!lighting_update_every_frame(ctx.g) || unported!("FastRandom.WithModifier(tileX, tileY).Next(4) == 0", false)) {
            self.draw_tiles_emit_particles(ctx, tile_y, tile_x, &tile_cache, type_cache, tile_draw_info.tile_frame_x, tile_draw_info.tile_frame_y, tile_draw_info.tile_light);
        }
        tile_draw_info.tile_light = self.draw_tiles_get_light_override(ctx, tile_y, tile_x, &tile_cache, type_cache, tile_draw_info.tile_frame_x, tile_draw_info.tile_frame_y, tile_draw_info.tile_light);
        let mut flag = false;
        if tile_draw_info.glow_texture.is_some() || ctx.g.main.tile_glow_mask[type_cache as usize] != -1 || ctx.g.main.tile_flame[type_cache as usize] {
            flag = true;
        }
        if tile_draw_info.tile_light.r() >= 1 || tile_draw_info.tile_light.g() >= 1 || tile_draw_info.tile_light.b() >= 1 || tile_sets::IGNORE_DRAW_LIGHT_CONDITIONS[type_cache as usize] {
            flag = true;
        }
        if tile_cache.wall > 0 && (tile_cache.wall == 318 || tile_cache.fullbright_wall()) {
            flag = true;
        }
        let flag2 = self.is_visible(&tile_cache);
        if !flag2 {
            flag = false;
        }
        if !flag && flag2 && DRAW_OWN_BLACKS {
            self.draw_black_helper.draw_black(ctx, tile_x, tile_y);
        }
        self.cache_special_draws_part1(tile_x, tile_y, type_cache as i32, tile_draw_info.tile_frame_x as i32, tile_draw_info.tile_frame_y as i32, !flag);
        self.cache_special_draws_part2(tile_x, tile_y, &mut tile_draw_info);
        if tile_draw_info.type_cache == 72 && tile_draw_info.tile_frame_x >= 36 {
            let mut num4 = 0;
            if tile_draw_info.tile_frame_y == 18 {
                num4 = 1;
            } else if tile_draw_info.tile_frame_y == 36 {
                num4 = 2;
            }
            let shroom_cap = ctx.shroom_cap();
            let light = lighting_get_color(ctx.g, tile_x, tile_y);
            ctx.tile_batch.draw(
                shroom_cap.as_ref(),
                Vector2::new((tile_x * 16 - screen_position.x as i32 - 22) as f32, (tile_y * 16 - screen_position.y as i32 - 26) as f32) + screen_offset,
                Rectangle::new(num4 * 62, 0, 60, 42),
                light.into(),
                ZERO,
                1.0,
                tile_draw_info.tile_sprite_effect,
            );
        }
        let mut rectangle = Rectangle::new(
            tile_draw_info.tile_frame_x as i32 + tile_draw_info.add_fr_x,
            tile_draw_info.tile_frame_y as i32 + tile_draw_info.add_fr_y,
            tile_draw_info.tile_width,
            tile_draw_info.tile_height - tile_draw_info.half_brick_height,
        );
        let mut num5 = (tile_draw_info.tile_width as f32 - 16.0) / 2.0;
        if tile_sets::DO_NOT_ADJUST_DRAW_POSITION_BASED_ON_TILE_WIDTH[tile_draw_info.type_cache as usize] {
            num5 = 0.0;
        }
        let mut vector = Vector2::new(
            (tile_x * 16 - screen_position.x as i32) as f32 - num5,
            (tile_y * 16 - screen_position.y as i32 + tile_draw_info.tile_top + tile_draw_info.half_brick_height) as f32,
        ) + screen_offset;
        if !flag {
            return;
        }
        tile_draw_info.color_tint = Color::WHITE;
        tile_draw_info.final_color = get_final_light(ctx, &tile_cache, type_cache, tile_draw_info.tile_light, tile_draw_info.color_tint);
        match tile_draw_info.type_cache {
            751 => {
                if tile_draw_info.tile_frame_x != 0 || tile_draw_info.tile_cache.frame_y != 0 {
                    return;
                }
                vector.x += 11.0;
                vector.y -= 8.0;
            }
            752 => {
                if tile_draw_info.tile_frame_x != 0 || tile_draw_info.tile_frame_y != 0 {
                    return;
                }
                vector.x += 8.0;
            }
            136 => match tile_draw_info.tile_frame_x / 18 {
                1 => vector.x += -2.0,
                2 => vector.x += 2.0,
                _ => {}
            },
            442 => {
                let num7 = tile_draw_info.tile_frame_x / 22;
                if num7 == 3 {
                    vector.x += 2.0;
                }
            }
            726 => {
                vector.x -= 2.0;
                match tile_draw_info.tile_cache.block_type() {
                    3 => {
                        vector.x -= 6.0;
                        vector.y += 2.0;
                    }
                    2 => {
                        vector.x += 6.0;
                        vector.y += 2.0;
                    }
                    5 => {
                        vector.x -= 6.0;
                    }
                    4 => {
                        vector.x += 6.0;
                    }
                    _ => {}
                }
            }
            51 | 697 => {
                tile_draw_info.final_color = tile_draw_info.tile_light * 0.5_f32;
            }
            160 | 692 => {
                let mut color = Color::from_ints(ctx.g.main.disco_r, ctx.g.main.disco_g, ctx.g.main.disco_b, 255);
                if tile_draw_info.tile_cache.in_active() {
                    color = tile_draw_info.tile_cache.act_color(color);
                }
                tile_draw_info.final_color = color;
            }
            129 => {
                tile_draw_info.final_color = Color::from_ints(255, 255, 255, 100);
                let num6 = 2;
                if tile_draw_info.tile_frame_x >= 324 {
                    tile_draw_info.final_color = Color::TRANSPARENT;
                }
                if tile_draw_info.tile_frame_y < 36 {
                    vector.y += (num6 * to_direction_int(tile_draw_info.tile_frame_y == 0)) as f32;
                } else {
                    vector.x += (num6 * to_direction_int(tile_draw_info.tile_frame_y == 36)) as f32;
                }
            }
            723 | 724 => match tile_draw_info.tile_frame_x / 18 {
                0 => vector += Vector2::new(0.0, 2.0),
                1 => vector += Vector2::new(0.0, -2.0),
                2 => vector += Vector2::new(-2.0, 0.0),
                3 => vector += Vector2::new(2.0, 0.0),
                _ => {}
            },
            272 => {
                let mut num8 = ctx.state.tile_frame[tile_draw_info.type_cache as usize];
                num8 += tile_x % 2;
                num8 += tile_y % 2;
                num8 += tile_x % 3;
                num8 += tile_y % 3;
                num8 %= 2;
                num8 *= 90;
                tile_draw_info.add_fr_y += num8;
                rectangle.y += num8;
            }
            80 => {
                let (evil, good, crimson) = crate::pending::world_gen_get_cactus_type(ctx.g, tile_x, tile_y, tile_draw_info.tile_frame_x as i32, tile_draw_info.tile_frame_y as i32);
                if evil {
                    rectangle.y += 54;
                }
                if good {
                    rectangle.y += 108;
                }
                if crimson {
                    rectangle.y += 162;
                }
            }
            83 => {
                tile_draw_info.draw_texture = self.get_tile_draw_texture(ctx, &tile_cache, tile_x, tile_y);
            }
            323 => {
                if tile_draw_info.tile_cache.frame_x <= 132 && tile_draw_info.tile_cache.frame_x >= 88 {
                    return;
                }
                vector.x += tile_draw_info.tile_cache.frame_y as f32;
            }
            114 => {
                if tile_draw_info.tile_frame_y > 0 {
                    rectangle.height += 2;
                }
            }
            _ => {}
        }
        if tile_draw_info.type_cache == 314 {
            unported!("TileDrawing.DrawTile_MinecartTrack");
        } else if tile_draw_info.type_cache == 171 {
            self.draw_xmas_tree(ctx, screen_position, screen_offset, tile_x, tile_y, &mut tile_draw_info);
        } else {
            self.draw_basic_tile(ctx, screen_position, screen_offset, tile_x, tile_y, &mut tile_draw_info, rectangle, vector);
        }
        if ctx.g.main.tile_glow_mask[tile_draw_info.tile_cache.type_ as usize] != -1 {
            let num9 = ctx.g.main.tile_glow_mask[tile_draw_info.tile_cache.type_ as usize];
            // `TextureAssets.GlowMask.IndexInRange(num9)`: GlowMaskID.Count entries
            if num9 >= 0 && num9 < GLOW_MASK_COUNT {
                tile_draw_info.draw_texture = ctx.glow_mask(num9);
            }
            let num10 = ctx.g.main.time_for_visual_effects * 0.08;
            let mut color2 = Color::WHITE;
            let mut flag3 = false;
            match tile_draw_info.tile_cache.type_ {
                718 => {
                    color2 = Color::from_ints(0, 0, 0, 0);
                }
                717 => {
                    let num13 = unported!("TileDrawing.LavaLightA", 0.0_f32); // neutral: no glow
                    color2 = Color::from_floats(num13, num13, num13, num13 / 2.0);
                }
                633 => {
                    color2 = Color::lerp(Color::WHITE, tile_draw_info.final_color, 0.75);
                }
                659 | 667 | 708 => {
                    color2 = unported!("LiquidRenderer.GetShimmerGlitterColor", Color::TRANSPARENT);
                }
                350 => {
                    let angle = if (num10 / 6.283) as i32 % 3 == 1 { num10 } else { 0.0 };
                    let v = ((0.0 - angle.cos()) * 0.2 + 0.2) as f32;
                    color2 = Color::from_vector4(Vector4::splat(v));
                }
                381 | 517 | 687 => {
                    color2 = self.lava_moss_glow;
                }
                534 | 535 | 689 => {
                    color2 = self.krypton_moss_glow;
                }
                536 | 537 | 690 => {
                    color2 = self.xenon_moss_glow;
                }
                539 | 540 | 688 => {
                    color2 = self.argon_moss_glow;
                }
                625 | 626 | 691 => {
                    color2 = self.violet_moss_glow;
                }
                627 | 628 | 692 => {
                    color2 = Color::from_ints_rgb(ctx.g.main.disco_r, ctx.g.main.disco_g, ctx.g.main.disco_b);
                }
                699 => {
                    color2 = Color::WHITE;
                }
                370 | 390 => {
                    color2 = self.meteor_glow;
                }
                391 => {
                    color2 = Color::from_ints(250, 250, 250, 200);
                }
                209 => {
                    color2 = unported!("PortalHelper.GetPortalColor", Color::TRANSPARENT);
                }
                429 | 445 => {
                    tile_draw_info.draw_texture = self.get_tile_draw_texture(ctx, &tile_cache, tile_x, tile_y);
                    tile_draw_info.add_fr_y = 18;
                }
                129 => {
                    if tile_draw_info.tile_frame_x < 324 {
                        flag3 = true;
                    } else {
                        tile_draw_info.draw_texture = self.get_tile_draw_texture(ctx, &tile_cache, tile_x, tile_y);
                        color2 = crate::main_draw::hsl_to_rgb(
                            0.7 + ((std::f32::consts::PI * 2.0 * ctx.g.main.global_time_wrapped_hourly * 0.16 + tile_x as f32 * 0.3 + tile_y as f32 * 0.7) as f64).sin() as f32 * 0.16,
                            1.0,
                            0.5,
                            255,
                        );
                        color2.set_a(color2.a() / 2);
                        color2 = color2 * 0.3_f32;
                        let num11 = 72;
                        let mut num12 = 0.0_f32;
                        while num12 < std::f32::consts::PI * 2.0 {
                            let position = vector + to_rotation_vector2(num12) * 2.0_f32;
                            let source = Rectangle::new(
                                tile_draw_info.tile_frame_x as i32 + tile_draw_info.add_fr_x,
                                tile_draw_info.tile_frame_y as i32 + tile_draw_info.add_fr_y + num11,
                                tile_draw_info.tile_width,
                                tile_draw_info.tile_height,
                            );
                            ctx.tile_batch.draw(tile_draw_info.draw_texture.as_ref(), position, source, color2.into(), ZERO, 1.0, SpriteEffects::NONE);
                            num12 += std::f32::consts::PI / 2.0;
                        }
                        color2 = Color::from_ints(255, 255, 255, 100);
                    }
                }
                725 => {
                    let opacity = unported!("Filters.Scene[\"Noir\"].Opacity", 0.0_f32);
                    if opacity > 0.0 && tile_draw_info.tile_frame_x % 36 == 0 && tile_draw_info.tile_frame_y == 54 {
                        let position = vector + Vector2::new(16.0, 24.0);
                        let effects = if tile_draw_info.tile_frame_x >= 36 { SpriteEffects::FLIP_HORIZONTALLY } else { SpriteEffects::NONE };
                        color2 = Color::from_ints(255, 255, 255, 100) * opacity;
                        if let Some(t) = tile_draw_info.draw_texture.as_ref() {
                            let frame = Rectangle::new(0, 0, t.width, t.height); // Texture2D.Frame()
                            let center = Vector2::new((frame.x + frame.width / 2) as f32, (frame.y + frame.height / 2) as f32); // Rectangle.Center.ToVector2()
                            ctx.tile_batch.draw(tile_draw_info.draw_texture.as_ref(), position, frame, color2.into(), center, 1.0, effects);
                        }
                    }
                    flag3 = true;
                }
                _ => {}
            }
            if !flag3 {
                let tc = tile_draw_info.tile_cache;
                let full_rect = Rectangle::new(
                    tile_draw_info.tile_frame_x as i32 + tile_draw_info.add_fr_x,
                    tile_draw_info.tile_frame_y as i32 + tile_draw_info.add_fr_y,
                    tile_draw_info.tile_width,
                    tile_draw_info.tile_height,
                );
                if tc.slope() == 0 && !tc.half_brick() {
                    ctx.tile_batch.draw(tile_draw_info.draw_texture.as_ref(), vector, full_rect, color2.into(), ZERO, 1.0, SpriteEffects::NONE);
                } else if tc.half_brick() {
                    ctx.tile_batch.draw(tile_draw_info.draw_texture.as_ref(), vector, rectangle, color2.into(), ZERO, 1.0, SpriteEffects::NONE);
                } else if tile_sets::HAS_SLOPE_FRAMES[tc.type_ as usize] {
                    let r = Rectangle::new(tile_draw_info.tile_frame_x as i32 + tile_draw_info.add_fr_x, tile_draw_info.tile_frame_y as i32 + tile_draw_info.add_fr_y, 16, 16);
                    ctx.tile_batch.draw(tile_draw_info.draw_texture.as_ref(), vector, r, color2.into(), ZERO, 1.0, tile_draw_info.tile_sprite_effect);
                } else {
                    let num14 = tc.slope() as i32;
                    let num15 = 2;
                    for i in 0..8 {
                        let mut num16 = i * -2;
                        let mut num17 = 16 - i * 2;
                        let mut num18 = 16 - num17;
                        let num19;
                        match num14 {
                            1 => {
                                num16 = 0;
                                num19 = i * 2;
                                num17 = 14 - i * 2;
                                num18 = 0;
                            }
                            2 => {
                                num16 = 0;
                                num19 = 16 - i * 2 - 2;
                                num17 = 14 - i * 2;
                                num18 = 0;
                            }
                            3 => {
                                num19 = i * 2;
                            }
                            _ => {
                                num19 = 16 - i * 2 - 2;
                            }
                        }
                        let r = Rectangle::new(tile_draw_info.tile_frame_x as i32 + tile_draw_info.add_fr_x + num19, tile_draw_info.tile_frame_y as i32 + tile_draw_info.add_fr_y + num18, num15, num17);
                        ctx.tile_batch.draw(tile_draw_info.draw_texture.as_ref(), vector + Vector2::new(num19 as f32, (i * num15 + num16) as f32), r, color2.into(), ZERO, 1.0, tile_draw_info.tile_sprite_effect);
                    }
                    let num20 = if num14 <= 2 { 14 } else { 0 };
                    let r = Rectangle::new(tile_draw_info.tile_frame_x as i32 + tile_draw_info.add_fr_x, tile_draw_info.tile_frame_y as i32 + tile_draw_info.add_fr_y + num20, 16, 2);
                    ctx.tile_batch.draw(tile_draw_info.draw_texture.as_ref(), vector + Vector2::new(0.0, num20 as f32), r, color2.into(), ZERO, 1.0, tile_draw_info.tile_sprite_effect);
                }
            }
        }
        if tile_draw_info.glow_texture.is_some() {
            if tile_draw_info.type_cache == 412 {
                let num21 = ctx.state.tile_frame[tile_draw_info.type_cache as usize] / 60;
                let num22 = (num21 + 1) % 4;
                let num23 = (ctx.state.tile_frame[tile_draw_info.type_cache as usize] % 60) as f32 / 60.0;
                let mut glow_source_rect = tile_draw_info.glow_source_rect;
                glow_source_rect.y += num21 * 18 * 3;
                let mut glow_source_rect2 = tile_draw_info.glow_source_rect;
                glow_source_rect2.y += num22 * 18 * 3;
                let position2 = Vector2::new(
                    (tile_x * 16 - screen_position.x as i32) as f32 - (tile_draw_info.tile_width as f32 - 16.0) / 2.0,
                    (tile_y * 16 - screen_position.y as i32 + tile_draw_info.tile_top) as f32,
                ) + screen_offset;
                ctx.tile_batch.draw(tile_draw_info.glow_texture.as_ref(), position2, glow_source_rect, (tile_draw_info.glow_color * (1.0 - num23)).into(), ZERO, 1.0, tile_draw_info.tile_sprite_effect);
                ctx.tile_batch.draw(tile_draw_info.glow_texture.as_ref(), position2, glow_source_rect2, (tile_draw_info.glow_color * num23).into(), ZERO, 1.0, tile_draw_info.tile_sprite_effect);
            } else {
                let mut vector2 = Vector2::new(
                    (tile_x * 16 - screen_position.x as i32) as f32 - (tile_draw_info.tile_width as f32 - 16.0) / 2.0,
                    (tile_y * 16 - screen_position.y as i32 + tile_draw_info.tile_top) as f32,
                ) + screen_offset;
                if tile_sets::PLATFORMS[tile_draw_info.type_cache as usize] {
                    vector2 = vector;
                }
                ctx.tile_batch.set_layer(LAYER_TILES, 1);
                ctx.tile_batch.draw(tile_draw_info.glow_texture.as_ref(), vector2, tile_draw_info.glow_source_rect, tile_draw_info.glow_color.into(), ZERO, 1.0, tile_draw_info.tile_sprite_effect);
                if tile_sets::PLATFORMS[tile_draw_info.type_cache as usize] && tile_draw_info.tile_cache.slope() != 0 {
                    let tile = tile_at(ctx, tile_x, tile_y + 1);
                    let tile2 = tile_at(ctx, tile_x - 1, tile_y + 1);
                    let tile3 = tile_at(ctx, tile_x + 1, tile_y + 1);
                    let should_show_invisible_blocks = self.should_show_invisible_blocks;
                    if tile_draw_info.tile_cache.slope() == 1
                        && tile3.active()
                        && (should_show_invisible_blocks || !tile3.invisible_block())
                        && ctx.g.main.tile_solid[tile3.type_ as usize]
                        && tile3.slope() != 2
                        && !tile3.half_brick()
                        && (!tile.active() || (!should_show_invisible_blocks && tile.invisible_block()) || (tile.block_type() != 0 && tile.block_type() != 5) || !tile_sets::BLOCKS_STAIRS[tile.type_ as usize])
                    {
                        let mut glow_source_rect3 = tile_draw_info.glow_source_rect;
                        if tile_sets::PLATFORMS[tile3.type_ as usize] && tile3.slope() == 0 {
                            glow_source_rect3.x = 324;
                        } else {
                            glow_source_rect3.x = 198;
                        }
                        ctx.tile_batch.set_layer(LAYER_BEHIND_TILES, 1);
                        ctx.tile_batch.draw(tile_draw_info.glow_texture.as_ref(), vector2 + Vector2::new(0.0, 16.0), glow_source_rect3, tile_draw_info.glow_color.into(), ZERO, 1.0, tile_draw_info.tile_sprite_effect);
                    } else if tile_draw_info.tile_cache.slope() == 2
                        && tile2.active()
                        && (should_show_invisible_blocks || !tile2.invisible_block())
                        && ctx.g.main.tile_solid[tile2.type_ as usize]
                        && tile2.slope() != 1
                        && !tile2.half_brick()
                        && (!tile.active() || (!should_show_invisible_blocks && tile.invisible_block()) || (tile.block_type() != 0 && tile.block_type() != 4) || !tile_sets::BLOCKS_STAIRS[tile.type_ as usize])
                    {
                        let mut glow_source_rect4 = tile_draw_info.glow_source_rect;
                        if tile_sets::PLATFORMS[tile2.type_ as usize] && tile2.slope() == 0 {
                            glow_source_rect4.x = 306;
                        } else {
                            glow_source_rect4.x = 162;
                        }
                        ctx.tile_batch.set_layer(LAYER_BEHIND_TILES, 1);
                        ctx.tile_batch.draw(tile_draw_info.glow_texture.as_ref(), vector2 + Vector2::new(0.0, 16.0), glow_source_rect4, tile_draw_info.glow_color.into(), ZERO, 1.0, tile_draw_info.tile_sprite_effect);
                    }
                }
            }
        }
        if highlight_texture.is_some() {
            let empty = Rectangle::new(
                tile_draw_info.tile_frame_x as i32 + tile_draw_info.add_fr_x,
                tile_draw_info.tile_frame_y as i32 + tile_draw_info.add_fr_y,
                tile_draw_info.tile_width,
                tile_draw_info.tile_height,
            );
            let num24 = 0;
            let num25 = 0;
            ctx.tile_batch.draw(
                highlight_texture.as_ref(),
                Vector2::new(
                    (tile_x * 16 - screen_position.x as i32) as f32 - (tile_draw_info.tile_width as f32 - 16.0) / 2.0 + num24 as f32,
                    (tile_y * 16 - screen_position.y as i32 + tile_draw_info.tile_top + num25) as f32,
                ) + screen_offset,
                empty,
                highlight_color.into(),
                ZERO,
                1.0,
                tile_draw_info.tile_sprite_effect,
            );
        }
    }

    /// `IsVisible(Tile tile)` (TileDrawing.cs:1431)
    pub(super) fn is_visible(&self, tile: &Tile) -> bool {
        let mut flag = tile.invisible_block();
        match tile.type_ {
            19 => {
                if tile.frame_y / 18 == 48 {
                    flag = true;
                }
            }
            541 | 631 => {
                flag = true;
            }
            _ => {}
        }
        if flag {
            return self.should_show_invisible_blocks;
        }
        true
    }

    /// `GetTileDrawTexture(Tile tile, int tileX, int tileY)` (TileDrawing.cs:1454)
    pub fn get_tile_draw_texture(&mut self, ctx: &mut DrawContext, tile: &Tile, tile_x: i32, tile_y: i32) -> Option<Texture2D> {
        let mut key = TileVariationKey { tile_type: tile.type_ as i32, tile_style: 0, paint_color: tile.color() as i32 };
        match tile.type_ {
            5 => {
                key.tile_style = get_tree_biome(ctx.g, tile_x, tile_y, tile.frame_x as i32, tile.frame_y as i32);
            }
            323 => {
                key.tile_style = get_palm_tree_biome(ctx.g, tile_x, tile_y);
            }
            83 => {
                if world_gen_is_alchemy_plant_harvestable(ctx.g, tile.frame_x as i32 / 18, tile_y) {
                    key.tile_type = 84;
                }
            }
            _ => {}
        }
        self.get_tile_draw_texture_key(ctx, key)
    }

    /// `GetTileDrawTexture(int tileType, int paintColor)` (TileDrawing.cs:1480); the minecart track code is its caller
    #[allow(dead_code)]
    pub fn get_tile_draw_texture_type_paint(&mut self, ctx: &mut DrawContext, tile_type: i32, paint_color: i32) -> Option<Texture2D> {
        self.get_tile_draw_texture_key(ctx, TileVariationKey { tile_type, tile_style: 0, paint_color })
    }

    /// `GetTileDrawTexture(TilePaintSystemV2.TileVariationkey key)` (TileDrawing.cs:1489)
    pub fn get_tile_draw_texture_key(&mut self, ctx: &mut DrawContext, key: TileVariationKey) -> Option<Texture2D> {
        if self.last_paint_lookup_key == key {
            return self.last_paint_lookup_texture.clone();
        }
        self.last_paint_lookup_key = key;
        self.last_paint_lookup_texture = self.lookup_tile_draw_texture(ctx, key);
        self.last_paint_lookup_texture.clone()
    }

    /// `LookupTileDrawTexture(TilePaintSystemV2.TileVariationkey key)` (TileDrawing.cs:1500)
    fn lookup_tile_draw_texture(&mut self, ctx: &mut DrawContext, key: TileVariationKey) -> Option<Texture2D> {
        ctx.tile_texture(key.tile_type); // Main.instance.LoadTiles(key.TileType)
        if key.paint_color != 0 || key.tile_style != 0 {
            // the paint system renders the variant into a target; until it is ready (which is every frame here) it answers null
            let texture2_d: Option<Texture2D> = unported!("TilePaintSystemV2.TryGetTileAndRequestIfNotReady", None);
            if texture2_d.is_some() {
                return texture2_d;
            }
        }
        ctx.tile_texture(key.tile_type)
    }

    /// `LookupCageTopDrawTexture(TilePaintSystemV2.CageTopVariationkey key)` (TileDrawing.cs:1514)
    fn lookup_cage_top_draw_texture(&mut self, ctx: &mut DrawContext, key: CageTopVariationKey) -> Option<Texture2D> {
        if key.paint_color != 0 {
            let texture2_d: Option<Texture2D> = unported!("TilePaintSystemV2.TryGetCageTopAndRequestIfNotReady", None);
            if texture2_d.is_some() {
                return texture2_d;
            }
        }
        ctx.cage_top(key.cage_style)
    }

    /// `DrawBasicTile(Vector2 screenPosition, Vector2 screenOffset, int tileX, int tileY, TileDrawInfo drawData,
    /// Rectangle normalTileRect, Vector2 normalTilePosition)` (TileDrawing.cs:1527)
    fn draw_basic_tile(&mut self, ctx: &mut DrawContext, screen_position: Vector2, screen_offset: Vector2, tile_x: i32, tile_y: i32, draw_data: &mut TileDrawInfo, normal_tile_rect: Rectangle, normal_tile_position: Vector2) {
        let flag = !tile_sets::DONT_DRAW_TILE_SLICED[draw_data.tile_cache.type_ as usize];
        let flag2 = !tile_sets::DONT_DRAW_TILE_SLOPES[draw_data.tile_cache.type_ as usize];
        if draw_data.type_cache == 380 || tile_sets::PLATFORMS[draw_data.type_cache as usize] {
            self.draw_tile_back_rope(ctx, screen_position, screen_offset, tile_x, tile_y, draw_data);
        }
        let dt = draw_data.draw_texture.clone();
        let fc: VertexColors = draw_data.final_color.into();
        let eff = draw_data.tile_sprite_effect;
        if flag2 && draw_data.tile_cache.slope() > 0 {
            if tile_sets::PLATFORMS[draw_data.tile_cache.type_ as usize] {
                let tile = tile_at(ctx, tile_x, tile_y + 1);
                let tile2 = tile_at(ctx, tile_x - 1, tile_y + 1);
                let tile3 = tile_at(ctx, tile_x + 1, tile_y + 1);
                let should_show_invisible_blocks = self.should_show_invisible_blocks;
                ctx.tile_batch.draw(dt.as_ref(), normal_tile_position, normal_tile_rect, fc, ZERO, 1.0, eff);
                if draw_data.tile_cache.slope() == 1
                    && tile3.active()
                    && (should_show_invisible_blocks || !tile3.invisible_block())
                    && ctx.g.main.tile_solid[tile3.type_ as usize]
                    && tile3.slope() != 2
                    && !tile3.half_brick()
                    && (!tile.active() || (!should_show_invisible_blocks && tile.invisible_block()) || (tile.block_type() != 0 && tile.block_type() != 5) || !tile_sets::BLOCKS_STAIRS[tile.type_ as usize])
                {
                    ctx.tile_batch.set_layer(LAYER_BEHIND_TILES, 0);
                    let mut source_rectangle = Rectangle::new(198, draw_data.tile_frame_y as i32, 16, 16);
                    if tile_sets::PLATFORMS[tile3.type_ as usize] && tile3.slope() == 0 {
                        source_rectangle.x = 324;
                    }
                    ctx.tile_batch.draw(dt.as_ref(), normal_tile_position + Vector2::new(0.0, 16.0), source_rectangle, fc, ZERO, 1.0, eff);
                } else if draw_data.tile_cache.slope() == 2
                    && tile2.active()
                    && (should_show_invisible_blocks || !tile2.invisible_block())
                    && ctx.g.main.tile_solid[tile2.type_ as usize]
                    && tile2.slope() != 1
                    && !tile2.half_brick()
                    && (!tile.active() || (!should_show_invisible_blocks && tile.invisible_block()) || (tile.block_type() != 0 && tile.block_type() != 4) || !tile_sets::BLOCKS_STAIRS[tile.type_ as usize])
                {
                    ctx.tile_batch.set_layer(LAYER_BEHIND_TILES, 0);
                    let mut source_rectangle2 = Rectangle::new(162, draw_data.tile_frame_y as i32, 16, 16);
                    if tile_sets::PLATFORMS[tile2.type_ as usize] && tile2.slope() == 0 {
                        source_rectangle2.x = 306;
                    }
                    ctx.tile_batch.draw(dt.as_ref(), normal_tile_position + Vector2::new(0.0, 16.0), source_rectangle2, fc, ZERO, 1.0, eff);
                }
                return;
            }
            let (fx, fy) = (draw_data.tile_frame_x as i32 + draw_data.add_fr_x, draw_data.tile_frame_y as i32 + draw_data.add_fr_y);
            if tile_sets::HAS_SLOPE_FRAMES[draw_data.tile_cache.type_ as usize] {
                ctx.tile_batch.draw(dt.as_ref(), normal_tile_position, Rectangle::new(fx, fy, 16, 16), fc, ZERO, 1.0, eff);
                return;
            }
            let num = draw_data.tile_cache.slope() as i32;
            let num2 = 2;
            for i in 0..8 {
                let mut num3 = i * -2;
                let mut num4 = 16 - i * 2;
                let mut num5 = 16 - num4;
                let num6;
                match num {
                    1 => {
                        num3 = 0;
                        num6 = i * 2;
                        num4 = 14 - i * 2;
                        num5 = 0;
                    }
                    2 => {
                        num3 = 0;
                        num6 = 16 - i * 2 - 2;
                        num4 = 14 - i * 2;
                        num5 = 0;
                    }
                    3 => {
                        num6 = i * 2;
                    }
                    _ => {
                        num6 = 16 - i * 2 - 2;
                    }
                }
                ctx.tile_batch.draw(dt.as_ref(), normal_tile_position + Vector2::new(num6 as f32, (i * num2 + num3) as f32), Rectangle::new(fx + num6, fy + num5, num2, num4), fc, ZERO, 1.0, eff);
            }
            let num7 = if num <= 2 { 14 } else { 0 };
            ctx.tile_batch.draw(dt.as_ref(), normal_tile_position + Vector2::new(0.0, num7 as f32), Rectangle::new(fx, fy + num7, 16, 2), fc, ZERO, 1.0, eff);
            return;
        }
        let left_half = tile_at(ctx, tile_x - 1, tile_y).half_brick();
        let right_half = tile_at(ctx, tile_x + 1, tile_y).half_brick();
        if flag2
            && !tile_sets::PLATFORMS[draw_data.type_cache as usize]
            && !tile_sets::IGNORES_NEARBY_HALFBRICKS_WHEN_DRAWN[draw_data.type_cache as usize]
            && ctx.g.main.tile_solid[draw_data.type_cache as usize]
            && !tile_sets::NOT_REALLY_SOLID[draw_data.type_cache as usize]
            && !draw_data.tile_cache.half_brick()
            && (left_half || right_half)
        {
            let (tfx, tfy, afx, afy, tw, th) = (draw_data.tile_frame_x as i32, draw_data.tile_frame_y as i32, draw_data.add_fr_x, draw_data.add_fr_y, draw_data.tile_width, draw_data.tile_height);
            if left_half && right_half {
                ctx.tile_batch.draw(dt.as_ref(), normal_tile_position + Vector2::new(0.0, 8.0), Rectangle::new(tfx + afx, afy + tfy + 8, tw, 8), fc, ZERO, 1.0, eff);
                let mut source_rectangle3 = Rectangle::new(126 + afx, afy, 16, 8);
                let above = tile_at(ctx, tile_x, tile_y - 1);
                if above.active() && !above.bottom_slope() && above.type_ == draw_data.type_cache {
                    source_rectangle3 = Rectangle::new(90 + afx, afy, 16, 8);
                }
                ctx.tile_batch.draw(dt.as_ref(), normal_tile_position, source_rectangle3, fc, ZERO, 1.0, eff);
            } else if left_half {
                let mut num8 = 4;
                if tile_sets::ALL_BLOCKS_WITH_SMOOTH_BORDERS_TO_RESOLVE_HALF_BLOCK_ISSUE[draw_data.type_cache as usize] {
                    num8 = 2;
                }
                ctx.tile_batch.draw(dt.as_ref(), normal_tile_position + Vector2::new(0.0, 8.0), Rectangle::new(tfx + afx, afy + tfy + 8, tw, 8), fc, ZERO, 1.0, eff);
                ctx.tile_batch.draw(dt.as_ref(), normal_tile_position + Vector2::new(num8 as f32, 0.0), Rectangle::new(tfx + num8 + afx, afy + tfy, tw - num8, th), fc, ZERO, 1.0, eff);
                ctx.tile_batch.draw(dt.as_ref(), normal_tile_position, Rectangle::new(144 + afx, afy, num8, 8), fc, ZERO, 1.0, eff);
                if num8 == 2 {
                    ctx.tile_batch.draw(dt.as_ref(), normal_tile_position, Rectangle::new(148 + afx, afy, 2, 2), fc, ZERO, 1.0, eff);
                }
            } else if right_half {
                let mut num9 = 4;
                if tile_sets::ALL_BLOCKS_WITH_SMOOTH_BORDERS_TO_RESOLVE_HALF_BLOCK_ISSUE[draw_data.type_cache as usize] {
                    num9 = 2;
                }
                ctx.tile_batch.draw(dt.as_ref(), normal_tile_position + Vector2::new(0.0, 8.0), Rectangle::new(tfx + afx, afy + tfy + 8, tw, 8), fc, ZERO, 1.0, eff);
                ctx.tile_batch.draw(dt.as_ref(), normal_tile_position, Rectangle::new(tfx + afx, afy + tfy, tw - num9, th), fc, ZERO, 1.0, eff);
                ctx.tile_batch.draw(dt.as_ref(), normal_tile_position + Vector2::new((16 - num9) as f32, 0.0), Rectangle::new(144 + (16 - num9), 0, num9, 8), fc, ZERO, 1.0, eff);
                if num9 == 2 {
                    ctx.tile_batch.draw(dt.as_ref(), normal_tile_position + Vector2::new(14.0, 0.0), Rectangle::new(156, 0, 2, 2), fc, ZERO, 1.0, eff);
                }
            }
            return;
        }
        if flag && lighting_not_retro(ctx.g) && ctx.g.main.tile_solid[draw_data.type_cache as usize] && !draw_data.tile_cache.half_brick() {
            self.draw_single_tile_sliced_block(ctx, normal_tile_position, tile_x, tile_y, draw_data);
            return;
        }
        let below = tile_at(ctx, tile_x, tile_y + 1);
        if draw_data.half_brick_height == 8 && (!below.active() || !ctx.g.main.tile_solid[below.type_ as usize] || below.half_brick()) {
            if tile_sets::PLATFORMS[draw_data.type_cache as usize] {
                ctx.tile_batch.draw(dt.as_ref(), normal_tile_position, normal_tile_rect, fc, ZERO, 1.0, eff);
            } else {
                ctx.tile_batch.draw(dt.as_ref(), normal_tile_position, modified(normal_tile_rect, 0, 0, 0, -4), fc, ZERO, 1.0, eff);
                ctx.tile_batch.draw(dt.as_ref(), normal_tile_position + Vector2::new(0.0, 4.0), Rectangle::new(144 + draw_data.add_fr_x, 66 + draw_data.add_fr_y, draw_data.tile_width, 4), fc, ZERO, 1.0, eff);
            }
        } else if tile_sets::CRITTER_CAGE_LID_STYLE[draw_data.type_cache as usize] >= 0 {
            let num10 = tile_sets::CRITTER_CAGE_LID_STYLE[draw_data.type_cache as usize];
            if (num10 < 3 && normal_tile_rect.y % 54 == 0) || (num10 >= 3 && normal_tile_rect.y % 36 == 0) {
                let mut position = normal_tile_position;
                position.y += 8.0;
                let mut source_rectangle4 = normal_tile_rect;
                source_rectangle4.y += 8;
                source_rectangle4.height -= 8;
                ctx.tile_batch.draw(dt.as_ref(), position, source_rectangle4, fc, ZERO, 1.0, eff);
                position = normal_tile_position;
                position.y -= 2.0;
                source_rectangle4 = normal_tile_rect;
                if num10 == 0 {
                    source_rectangle4.x = normal_tile_rect.x % 108;
                }
                source_rectangle4.y = 0;
                source_rectangle4.height = 10;
                let texture = self.lookup_cage_top_draw_texture(ctx, CageTopVariationKey { cage_style: num10, paint_color: draw_data.tile_cache.color() as i32 });
                ctx.tile_batch.draw(texture.as_ref(), position, source_rectangle4, fc, ZERO, 1.0, eff);
            } else {
                ctx.tile_batch.draw(dt.as_ref(), normal_tile_position, normal_tile_rect, fc, ZERO, 1.0, eff);
            }
        } else if draw_data.type_cache == 711 {
            let rectangle = Rectangle::new(draw_data.tile_frame_x as i32, draw_data.tile_frame_y as i32, draw_data.tile_width, draw_data.tile_height);
            if normal_tile_rect.x == 0 && normal_tile_rect.y == 0 {
                let mut rectangle2 = rectangle;
                rectangle2.x += 38;
                let mut num11 = 0.0_f32;
                while num11 < 1.0 {
                    let num12 = ctx.g.main.global_time_wrapped_hourly % 2.0 / 2.0;
                    let mut color = crate::main_draw::hsl_to_rgb((num12 + num11) % 1.0, 1.0, 0.5, 255);
                    color.set_a(0);
                    color = color * 0.3_f32;
                    for j in 0..2 {
                        if j == 1 {
                            rectangle2.width = rectangle.width + 2;
                        } else {
                            rectangle2.width = rectangle.width;
                        }
                        for k in 0..2 {
                            if k == 1 {
                                rectangle2.height = rectangle.height + 2;
                            } else {
                                rectangle2.height = rectangle.height;
                            }
                            let p = normal_tile_position + Vector2::new((j * 16) as f32, (k * 16) as f32) + to_rotation_vector2((num12 + num11) * (std::f32::consts::PI * 2.0)) * 4.0_f32;
                            let p = Vector2::new((p.x as f64).floor() as f32, (p.y as f64).floor() as f32); // Vector2.Floor()
                            ctx.tile_batch.draw(dt.as_ref(), p, Rectangle::new(rectangle2.x + j * 18, rectangle2.y + k * 18, rectangle2.width, rectangle2.height), color.into(), ZERO, 1.0, eff);
                        }
                    }
                    num11 += 1.0 / 3.0;
                }
            }
            ctx.tile_batch.draw(dt.as_ref(), normal_tile_position, rectangle, fc, ZERO, 1.0, eff);
        } else {
            ctx.tile_batch.draw(dt.as_ref(), normal_tile_position, normal_tile_rect, fc, ZERO, 1.0, eff);
        }
        self.draw_single_tile_flames(ctx, screen_position, screen_offset, tile_x, tile_y, draw_data);
    }

    /// `DrawSingleTile_SlicedBlock(Vector2 normalTilePosition, int tileX, int tileY, TileDrawInfo drawData)` (TileDrawing.cs:3542)
    fn draw_single_tile_sliced_block(&mut self, ctx: &mut DrawContext, normal_tile_position: Vector2, tile_x: i32, tile_y: i32, draw_data: &mut TileDrawInfo) {
        let mut color;
        let origin = Vector2::ZERO;
        let mut source_rectangle = Rectangle::EMPTY;
        let mut tile_light = Vector3::ZERO;
        let mut position = Vector2::ZERO;
        let dt = draw_data.draw_texture.clone();
        let hq = self.high_quality_lighting_requirement;
        let mq = self.medium_quality_lighting_requirement;
        if draw_data.tile_light.r() > hq.r() || draw_data.tile_light.g() > hq.g() || draw_data.tile_light.b() > hq.b() {
            let mut slices = draw_data.color_slices;
            lighting_get_color_9_slice(ctx.g, tile_x, tile_y, &mut slices);
            draw_data.color_slices = slices;
            let vector = draw_data.tile_light.to_vector3();
            let mut tint = draw_data.color_tint.to_vector3();
            if draw_data.tile_cache.fullbright_block() {
                slices = self.glow_paint_color_slices;
            }
            for i in 0..9usize {
                source_rectangle.x = 0;
                source_rectangle.y = 0;
                source_rectangle.width = 4;
                source_rectangle.height = 4;
                match i {
                    1 => {
                        source_rectangle.width = 8;
                        source_rectangle.x = 4;
                    }
                    2 => {
                        source_rectangle.x = 12;
                    }
                    3 => {
                        source_rectangle.height = 8;
                        source_rectangle.y = 4;
                    }
                    4 => {
                        source_rectangle.width = 8;
                        source_rectangle.height = 8;
                        source_rectangle.x = 4;
                        source_rectangle.y = 4;
                    }
                    5 => {
                        source_rectangle.x = 12;
                        source_rectangle.y = 4;
                        source_rectangle.height = 8;
                    }
                    6 => {
                        source_rectangle.y = 12;
                    }
                    7 => {
                        source_rectangle.width = 8;
                        source_rectangle.height = 4;
                        source_rectangle.x = 4;
                        source_rectangle.y = 12;
                    }
                    8 => {
                        source_rectangle.x = 12;
                        source_rectangle.y = 12;
                    }
                    _ => {}
                }
                tile_light.x = (slices[i].x + vector.x) * 0.5;
                tile_light.y = (slices[i].y + vector.y) * 0.5;
                tile_light.z = (slices[i].z + vector.z) * 0.5;
                get_final_light_vec3(ctx, &draw_data.tile_cache, draw_data.type_cache, &mut tile_light, &mut tint);
                position.x = normal_tile_position.x + source_rectangle.x as f32;
                position.y = normal_tile_position.y + source_rectangle.y as f32;
                source_rectangle.x += draw_data.tile_frame_x as i32 + draw_data.add_fr_x;
                source_rectangle.y += draw_data.tile_frame_y as i32 + draw_data.add_fr_y;
                let mut num = (tile_light.x * 255.0) as i32;
                let mut num2 = (tile_light.y * 255.0) as i32;
                let mut num3 = (tile_light.z * 255.0) as i32;
                if num > 255 {
                    num = 255;
                }
                if num2 > 255 {
                    num2 = 255;
                }
                if num3 > 255 {
                    num3 = 255;
                }
                num3 <<= 16;
                num2 <<= 8;
                color = Color::from_packed_value((num | num2 | num3 | -16777216) as u32);
                ctx.tile_batch.draw(dt.as_ref(), position, source_rectangle, color.into(), origin, 1.0, draw_data.tile_sprite_effect);
            }
        } else if draw_data.tile_light.r() > mq.r() || draw_data.tile_light.g() > mq.g() || draw_data.tile_light.b() > mq.b() {
            let mut slices2 = draw_data.color_slices;
            lighting_get_color_4_slice(ctx.g, tile_x, tile_y, &mut slices2);
            draw_data.color_slices = slices2;
            let vector2 = draw_data.tile_light.to_vector3();
            let mut tint2 = draw_data.color_tint.to_vector3();
            if draw_data.tile_cache.fullbright_block() {
                slices2 = self.glow_paint_color_slices;
            }
            source_rectangle.width = 8;
            source_rectangle.height = 8;
            for j in 0..4usize {
                source_rectangle.x = 0;
                source_rectangle.y = 0;
                match j {
                    1 => {
                        source_rectangle.x = 8;
                    }
                    2 => {
                        source_rectangle.y = 8;
                    }
                    3 => {
                        source_rectangle.x = 8;
                        source_rectangle.y = 8;
                    }
                    _ => {}
                }
                tile_light.x = (slices2[j].x + vector2.x) * 0.5;
                tile_light.y = (slices2[j].y + vector2.y) * 0.5;
                tile_light.z = (slices2[j].z + vector2.z) * 0.5;
                get_final_light_vec3(ctx, &draw_data.tile_cache, draw_data.type_cache, &mut tile_light, &mut tint2);
                position.x = normal_tile_position.x + source_rectangle.x as f32;
                position.y = normal_tile_position.y + source_rectangle.y as f32;
                source_rectangle.x += draw_data.tile_frame_x as i32 + draw_data.add_fr_x;
                source_rectangle.y += draw_data.tile_frame_y as i32 + draw_data.add_fr_y;
                let mut num4 = (tile_light.x * 255.0) as i32;
                let mut num5 = (tile_light.y * 255.0) as i32;
                let mut num6 = (tile_light.z * 255.0) as i32;
                if num4 > 255 {
                    num4 = 255;
                }
                if num5 > 255 {
                    num5 = 255;
                }
                if num6 > 255 {
                    num6 = 255;
                }
                num6 <<= 16;
                num5 <<= 8;
                color = Color::from_packed_value((num4 | num5 | num6 | -16777216) as u32);
                ctx.tile_batch.draw(dt.as_ref(), position, source_rectangle, color.into(), origin, 1.0, draw_data.tile_sprite_effect);
            }
        } else {
            ctx.tile_batch.draw(
                dt.as_ref(),
                normal_tile_position,
                Rectangle::new(draw_data.tile_frame_x as i32 + draw_data.add_fr_x, draw_data.tile_frame_y as i32 + draw_data.add_fr_y, draw_data.tile_width, draw_data.tile_height),
                draw_data.final_color.into(),
                ZERO,
                1.0,
                draw_data.tile_sprite_effect,
            );
        }
    }

    /// `DrawXmasTree(Vector2 screenPosition, Vector2 screenOffset, int tileX, int tileY, TileDrawInfo drawData)` (TileDrawing.cs:3696)
    fn draw_xmas_tree(&mut self, ctx: &mut DrawContext, screen_position: Vector2, screen_offset: Vector2, tile_x: i32, tile_y: i32, draw_data: &mut TileDrawInfo) {
        let num = 2;
        let mut tile_frame_x = draw_data.tile_frame_x as i32;
        let mut tile_frame_y = draw_data.tile_frame_y as i32;
        if tile_y - tile_frame_y > 0 && tile_frame_y == 7 && ctx.g.main.tile.try_get(tile_x, tile_y - tile_frame_y).is_some() {
            draw_data.tile_top -= 16 * tile_frame_y;
            let t = *ctx.g.main.tile.get(tile_x, tile_y - tile_frame_y);
            tile_frame_x = t.frame_x as i32;
            tile_frame_y = t.frame_y as i32;
            draw_data.tile_frame_x = t.frame_x;
            draw_data.tile_frame_y = t.frame_y;
        }
        if tile_frame_x < 10 {
            return;
        }
        let mut num2 = 0;
        if (tile_frame_y & 1) == 1 {
            num2 += 1;
        }
        if (tile_frame_y & 2) == 2 {
            num2 += 2;
        }
        if (tile_frame_y & 4) == 4 {
            num2 += 4;
        }
        let mut num3 = 0;
        if (tile_frame_y & 8) == 8 {
            num3 += 1;
        }
        if (tile_frame_y & 0x10) == 16 {
            num3 += 2;
        }
        if (tile_frame_y & 0x20) == 32 {
            num3 += 4;
        }
        let mut num4 = 0;
        if (tile_frame_y & 0x40) == 64 {
            num4 += 1;
        }
        if (tile_frame_y & 0x80) == 128 {
            num4 += 2;
        }
        if (tile_frame_y & 0x100) == 256 {
            num4 += 4;
        }
        if (tile_frame_y & 0x200) == 512 {
            num4 += 8;
        }
        let mut num5 = 0;
        if (tile_frame_y & 0x400) == 1024 {
            num5 += 1;
        }
        if (tile_frame_y & 0x800) == 2048 {
            num5 += 2;
        }
        if (tile_frame_y & 0x1000) == 4096 {
            num5 += 4;
        }
        if (tile_frame_y & 0x2000) == 8192 {
            num5 += 8;
        }
        let color = lighting_get_color(ctx.g, tile_x + 1, tile_y - 3);
        let pos = Vector2::new(
            (tile_x * 16 - screen_position.x as i32) as f32 - (draw_data.tile_width as f32 - 16.0) / 2.0,
            (tile_y * 16 - screen_position.y as i32 + draw_data.tile_top + num) as f32,
        ) + screen_offset;
        let t0 = ctx.xmas_tree(0);
        ctx.tile_batch.draw(t0.as_ref(), pos, Rectangle::new(0, 0, 64, 128), color.into(), ZERO, 1.0, SpriteEffects::NONE);
        if num2 > 0 {
            num2 -= 1;
            let mut color2 = color;
            if num2 != 3 {
                color2 = Color::from_ints(255, 255, 255, 255);
            }
            let t = ctx.xmas_tree(3);
            ctx.tile_batch.draw(t.as_ref(), pos, Rectangle::new(66 * num2, 0, 64, 128), color2.into(), ZERO, 1.0, SpriteEffects::NONE);
        }
        if num3 > 0 {
            num3 -= 1;
            let t = ctx.xmas_tree(1);
            ctx.tile_batch.draw(t.as_ref(), pos, Rectangle::new(66 * num3, 0, 64, 128), color.into(), ZERO, 1.0, SpriteEffects::NONE);
        }
        if num4 > 0 {
            num4 -= 1;
            let t = ctx.xmas_tree(2);
            ctx.tile_batch.draw(t.as_ref(), pos, Rectangle::new(66 * num4, 0, 64, 128), color.into(), ZERO, 1.0, SpriteEffects::NONE);
        }
        if num5 > 0 {
            num5 -= 1;
            let t = ctx.xmas_tree(4);
            let f = ctx.state.tile_frame[171];
            ctx.tile_batch.draw(t.as_ref(), pos, Rectangle::new(66 * num5, 130 * f, 64, 128), Color::from_ints(255, 255, 255, 255).into(), ZERO, 1.0, SpriteEffects::NONE);
        }
    }

    /// `DrawTile_BackRope(Vector2 screenPosition, Vector2 screenOffset, int tileX, int tileY, TileDrawInfo drawData)` (TileDrawing.cs:3798)
    fn draw_tile_back_rope(&mut self, ctx: &mut DrawContext, screen_position: Vector2, screen_offset: Vector2, tile_x: i32, tile_y: i32, draw_data: &mut TileDrawInfo) {
        if !terraria_sim::world_gen::tile_frame::in_world(ctx.g, tile_x, tile_y, 1) {
            return;
        }
        let (is_rope, top_rope_y, _bottom_rope_y) = world_gen_is_rope_out(ctx.g, tile_x, tile_y, 5);
        if is_rope {
            // `Main.tile[tileX, topRopeY]` throws for -1 (a rope tile answers true with no ends); the port reads it only when it exists
            if let Some(tile) = ctx.g.main.tile.try_get(tile_x, top_rope_y) {
                let y = (tile_y + tile_x) % 3 * 18;
                let tile_draw_texture = self.get_tile_draw_texture(ctx, &tile, tile_x, tile_y);
                ctx.tile_batch.draw(
                    tile_draw_texture.as_ref(),
                    Vector2::new((tile_x * 16 - screen_position.x as i32) as f32, (tile_y * 16 - screen_position.y as i32) as f32) + screen_offset,
                    Rectangle::new(90, y, 16, 16),
                    draw_data.tile_light.into(),
                    ZERO,
                    1.0,
                    draw_data.tile_sprite_effect,
                );
            }
        }
    }

    /// `CacheSpecialDraws_Part1(int tileX, int tileY, int tileType, int drawDataTileFrameX, int drawDataTileFrameY, bool skipDraw)` (TileDrawing.cs:4184)
    fn cache_special_draws_part1(&mut self, tile_x: i32, tile_y: i32, tile_type: i32, draw_data_tile_frame_x: i32, draw_data_tile_frame_y: i32, skip_draw: bool) {
        if tile_type == 395 {
            let mut point = Point { x: tile_x, y: tile_y };
            if draw_data_tile_frame_x % 36 != 0 {
                point.x -= 1;
            }
            if draw_data_tile_frame_y % 36 != 0 {
                point.y -= 1;
            }
            if !self.item_frame_tile_entity_positions.contains_key(&point) {
                let found = unported!("TileEntityType<TEItemFrame>.Find", -1);
                self.item_frame_tile_entity_positions.insert(point, found);
                if found != -1 {
                    self.add_special_legacy_point(point);
                }
            }
        }
        if tile_type == 698 {
            let mut point2 = Point { x: tile_x, y: tile_y };
            if draw_data_tile_frame_x % 18 != 0 {
                point2.x -= 1;
            }
            if draw_data_tile_frame_y % 36 != 0 {
                point2.y -= 1;
            }
            if !self.dead_cells_display_jar_tile_entity_positions.contains_key(&point2) {
                let found = unported!("TileEntityType<TEDeadCellsDisplayJar>.Find", -1);
                self.dead_cells_display_jar_tile_entity_positions.insert(point2, found);
                if found != -1 {
                    self.add_special_legacy_point(point2);
                }
            }
        }
        if tile_type == 520 {
            let point3 = Point { x: tile_x, y: tile_y };
            if !self.food_platter_tile_entity_positions.contains_key(&point3) {
                let found = unported!("TileEntityType<TEFoodPlatter>.Find", -1);
                self.food_platter_tile_entity_positions.insert(point3, found);
                if found != -1 {
                    self.add_special_legacy_point(point3);
                }
            }
        }
        if tile_type == 471 {
            let mut point4 = Point { x: tile_x, y: tile_y };
            point4.x -= draw_data_tile_frame_x % 54 / 18;
            point4.y -= draw_data_tile_frame_y % 54 / 18;
            if !self.weapon_rack_tile_entity_positions.contains_key(&point4) {
                let found = unported!("TileEntityType<TEWeaponsRack>.Find", -1);
                self.weapon_rack_tile_entity_positions.insert(point4, found);
                if found != -1 {
                    self.add_special_legacy_point(point4);
                }
            }
        }
        if tile_type == 470 {
            let mut point5 = Point { x: tile_x, y: tile_y };
            point5.x -= draw_data_tile_frame_x % 36 / 18;
            point5.y -= draw_data_tile_frame_y % 54 / 18;
            if !self.display_doll_tile_entity_positions.contains_key(&point5) {
                let found = unported!("TileEntityType<TEDisplayDoll>.Find", -1);
                self.display_doll_tile_entity_positions.insert(point5, found);
                if found != -1 {
                    self.add_special_legacy_point(point5);
                }
            }
        }
        if tile_type == 475 {
            let mut point6 = Point { x: tile_x, y: tile_y };
            point6.x -= draw_data_tile_frame_x % 54 / 18;
            point6.y -= draw_data_tile_frame_y % 72 / 18;
            if !self.hat_rack_tile_entity_positions.contains_key(&point6) {
                let found = unported!("TileEntityType<TEHatRack>.Find", -1);
                self.hat_rack_tile_entity_positions.insert(point6, found);
                if found != -1 {
                    self.add_special_legacy_point(point6);
                }
            }
        }
        if tile_type == 620 && draw_data_tile_frame_x == 0 && draw_data_tile_frame_y == 0 {
            self.add_special_legacy_point_xy(tile_x, tile_y);
        }
        if tile_type == 237 && draw_data_tile_frame_x == 18 && draw_data_tile_frame_y == 0 {
            self.add_special_legacy_point_xy(tile_x, tile_y);
        }
        if skip_draw {
            return;
        }
        match tile_type {
            323 => {
                if draw_data_tile_frame_x <= 132 && draw_data_tile_frame_x >= 88 {
                    self.add_special_point(tile_x, tile_y, TileCounterType::Tree);
                }
            }
            5 | 583 | 584 | 585 | 586 | 587 | 588 | 589 | 596 | 616 | 634 => {
                if draw_data_tile_frame_y >= 198 && draw_data_tile_frame_x >= 22 {
                    self.add_special_point(tile_x, tile_y, TileCounterType::Tree);
                }
            }
            _ => {}
        }
    }

    /// `CacheSpecialDraws_Part2(int tileX, int tileY, TileDrawInfo drawData)` (TileDrawing.cs:4319)
    fn cache_special_draws_part2(&mut self, tile_x: i32, tile_y: i32, draw_data: &mut TileDrawInfo) {
        let tile_frame_x = draw_data.tile_frame_x as i32;
        let tile_frame_y = draw_data.tile_frame_y as i32;
        if tile_sets::BASIC_CHEST[draw_data.type_cache as usize] {
            let mut key = Point { x: tile_x, y: tile_y };
            if tile_frame_x % 36 != 0 {
                key.x -= 1;
            }
            if tile_frame_y % 36 != 0 {
                key.y -= 1;
            }
            if !self.chest_positions.contains_key(&key) {
                self.chest_positions.insert(key, unported!("Chest.FindChest", -1));
            }
            let num = tile_frame_x / 18;
            let num2 = tile_frame_y / 18;
            let num3 = tile_frame_x / 36;
            let num4 = num * 18;
            draw_data.add_fr_x = num4 - tile_frame_x;
            let mut num5 = num2 * 18;
            if self.chest_positions[&key] != -1 {
                let frame = unported!("Main.chest[].frame", 0);
                if frame == 1 {
                    num5 += 38;
                }
                if frame == 2 {
                    num5 += 76;
                }
            }
            draw_data.add_fr_y = num5 - tile_frame_y;
            if num2 != 0 {
                draw_data.tile_height = 18;
            }
            if draw_data.type_cache == 21 && (num3 == 48 || num3 == 49) {
                draw_data.glow_source_rect = Rectangle::new(16 * (num % 2), tile_frame_y + draw_data.add_fr_y, draw_data.tile_width, draw_data.tile_height);
            }
        }
        if draw_data.type_cache != 378 {
            return;
        }
        let mut key2 = Point { x: tile_x, y: tile_y };
        if tile_frame_x % 36 != 0 {
            key2.x -= 1;
        }
        if tile_frame_y % 54 != 0 {
            key2.y -= tile_frame_y / 18;
        }
        if !self.training_dummy_tile_entity_positions.contains_key(&key2) {
            self.training_dummy_tile_entity_positions.insert(key2, unported!("TileEntityType<TETrainingDummy>.Find", -1));
        }
        if self.training_dummy_tile_entity_positions[&key2] != -1 && unported!("TileEntity.TryGet<TETrainingDummy>", false) {
            unported!("Main.npc[].frame.Y"); // addFrY from the dummy's npc frame
        }
    }

    /// `AddSpecialLegacyPoint(Point p)` (TileDrawing.cs:6713)
    fn add_special_legacy_point(&mut self, p: Point) {
        self.add_special_legacy_point_xy(p.x, p.y);
    }

    /// `AddSpecialLegacyPoint(int x, int y)` (TileDrawing.cs:6718)
    fn add_special_legacy_point_xy(&mut self, x: i32, y: i32) {
        self.special_tile_x.push(x);
        self.special_tile_y.push(y);
        self.special_tiles_count += 1;
    }

    /// `ClearCachedTileDraws(bool solidLayer)` (TileDrawing.cs:6700)
    fn clear_cached_tile_draws(&mut self, solid_layer: bool) {
        if solid_layer {
            self.display_doll_tile_entity_positions.clear();
            self.hat_rack_tile_entity_positions.clear();
        } else {
            self.clear_special_block_counts();
        }
    }

    /// `ClearLegacyCachedDraws()` (TileDrawing.cs:6725)
    fn clear_legacy_cached_draws(&mut self) {
        self.chest_positions.clear();
        self.training_dummy_tile_entity_positions.clear();
        self.food_platter_tile_entity_positions.clear();
        self.item_frame_tile_entity_positions.clear();
        self.dead_cells_display_jar_tile_entity_positions.clear();
        self.weapon_rack_tile_entity_positions.clear();
        self.special_tiles_count = 0;
        self.special_tile_x.clear();
        self.special_tile_y.clear();
    }

    /// `IsTileDrawLayerSolid(ushort typeCache)` (TileDrawing.cs:4509)
    fn is_tile_draw_layer_solid(&self, ctx: &DrawContext, type_cache: u16) -> bool {
        if let Some(v) = draw_tile_in_solid_layer(type_cache) {
            return v;
        }
        ctx.g.main.tile_solid[type_cache as usize]
    }

    /// `GetTileOutlineInfo(int x, int y, ushort typeCache, ref Color tileLight, ref Texture2D highlightTexture, ref Color highlightColor)` (TileDrawing.cs:4518)
    #[allow(clippy::too_many_arguments)]
    fn get_tile_outline_info(&mut self, ctx: &mut DrawContext, x: i32, y: i32, type_cache: u16, tile_light: &mut Color, highlight_texture: &mut Option<Texture2D>, highlight_color: &mut Color) {
        let _ = (x, y);
        if unported!("Main.InSmartCursorHighlightArea", false) {
            let num = (tile_light.r() as i32 + tile_light.g() as i32 + tile_light.b() as i32) / 3;
            if num > 10 {
                *highlight_texture = ctx.texture(&format!("Images/Tiles_{}_Highlight", type_cache)); // TextureAssets.HighlightMask[typeCache]
                *highlight_color = unported!("Colors.GetSelectionGlowColor", Color::TRANSPARENT);
            }
        }
    }

    /// `ShouldSwayInWind(int x, int y, Tile tileCache)` (TileDrawing.cs:7842). `Main.SettingsEnabled_TilesSwayInWind` is true by default.
    fn should_sway_in_wind(&self, ctx: &DrawContext, _x: i32, _y: i32, tile_cache: &Tile) -> bool {
        if !ctx.state.settings_enabled_tiles_sway_in_wind {
            return false;
        }
        if !tile_sets::SWAYS_IN_WIND_BASIC[tile_cache.type_ as usize] {
            return false;
        }
        if tile_cache.type_ == 227 && (tile_cache.frame_x == 204 || tile_cache.frame_x == 238 || tile_cache.frame_x == 408 || tile_cache.frame_x == 442 || tile_cache.frame_x == 476) {
            return false;
        }
        true
    }

    /// `DrawTiles_GetLightOverride(int j, int i, Tile tileCache, ushort typeCache, short tileFrameX, short tileFrameY, Color tileLight)` (TileDrawing.cs:6736)
    #[allow(clippy::too_many_arguments)]
    pub(super) fn draw_tiles_get_light_override(&self, ctx: &DrawContext, j: i32, _i: i32, tile_cache: &Tile, type_cache: u16, tile_frame_x: i16, tile_frame_y: i16, mut tile_light: Color) -> Color {
        if tile_cache.fullbright_block() {
            return Color::WHITE;
        }
        let mouse_text_color = ctx.g.main.mouse_text_color;
        match type_cache {
            541 | 631 => return Color::WHITE,
            19 => {
                if tile_frame_y / 18 == 48 {
                    return Color::WHITE;
                }
            }
            83 => {
                let num2 = tile_frame_x as i32 / 18;
                if world_gen_is_alchemy_plant_harvestable(ctx.g, num2, j) && num2 == 5 {
                    tile_light.set_a(mouse_text_color / 2);
                    tile_light.set_g(mouse_text_color);
                    tile_light.set_b(mouse_text_color);
                }
            }
            84 => {
                if tile_frame_x / 18 == 6 {
                    let b6 = ((mouse_text_color as i32 + tile_light.g() as i32 * 2) / 3) as u8;
                    let b7 = ((mouse_text_color as i32 + tile_light.b() as i32 * 2) / 3) as u8;
                    if b6 > tile_light.g() {
                        tile_light.set_g(b6);
                    }
                    if b7 > tile_light.b() {
                        tile_light.set_b(b7);
                    }
                }
            }
            61 | 703 => {
                if tile_frame_x == 144 {
                    let b = (245.0_f32 - mouse_text_color as f32 * 1.5) as i32 as u8;
                    tile_light.set_b(b);
                    tile_light.set_g(b);
                    tile_light.set_r(b);
                    tile_light.set_a(b);
                }
            }
            481 | 482 | 483 => {
                let num = 1.0_f32 + ((ctx.g.main.global_time_wrapped_hourly / 1.5 * (std::f32::consts::PI * 2.0)) as f64).sin() as f32 * 0.15;
                let a = tile_light.a();
                tile_light = tile_light * num;
                tile_light.set_a(a);
            }
            _ => {}
        }
        tile_light
    }
}

impl Default for TileDrawing {
    fn default() -> Self {
        Self::new()
    }
}

/// `GlowMaskID.Count` (GlowMaskID.cs): the length of `TextureAssets.GlowMask`
const GLOW_MASK_COUNT: i32 = 384;

/// `TileID.Sets.DrawTileInSolidLayer` (ID/TileID.cs:219), a `bool?` set: `true` for 11, 470, 475, 78 and 579, null elsewhere
fn draw_tile_in_solid_layer(type_cache: u16) -> Option<bool> {
    match type_cache {
        11 | 470 | 475 | 78 | 579 => Some(true),
        _ => None,
    }
}

/// Safe `Main.tile[x, y]` for the neighbour reads of the drawing code. Outside the world the original has a null tile
/// (and would throw when it is used); the port uses an empty tile, which every caller treats as "nothing there".
fn tile_at(ctx: &DrawContext, x: i32, y: i32) -> Tile {
    ctx.g.main.tile.try_get(x, y).unwrap_or_default()
}

/// `Rectangle.Modified(this Rectangle r, int x, int y, int w, int h)` (Utils.cs:1620)
fn modified(r: Rectangle, x: i32, y: i32, w: i32, h: i32) -> Rectangle {
    Rectangle::new(r.x + x, r.y + y, r.width + w, r.height + h)
}

/// `Utils.ToRotationVector2(this float f)`: `new Vector2((float)Math.Cos(f), (float)Math.Sin(f))`
fn to_rotation_vector2(f: f32) -> Vector2 {
    Vector2::new((f as f64).cos() as f32, (f as f64).sin() as f32)
}

/// `Utils.ToDirectionInt(this bool value)`: `value ? 1 : -1`
fn to_direction_int(value: bool) -> i32 {
    if value {
        1
    } else {
        -1
    }
}

/// `TileDrawing.GetFinalLight(Tile tileCache, ushort typeCache, Color tileLight, Color tint)` (TileDrawing.cs:4394)
pub fn get_final_light(ctx: &DrawContext, tile_cache: &Tile, type_cache: u16, mut tile_light: Color, tint: Color) -> Color {
    let mut num = ((tile_light.r() as i32 * tint.r() as i32) as f32 / 255.0) as i32;
    let mut num2 = ((tile_light.g() as i32 * tint.g() as i32) as f32 / 255.0) as i32;
    let mut num3 = ((tile_light.b() as i32 * tint.b() as i32) as f32 / 255.0) as i32;
    if num > 255 {
        num = 255;
    }
    if num2 > 255 {
        num2 = 255;
    }
    if num3 > 255 {
        num3 = 255;
    }
    num3 <<= 16;
    num2 <<= 8;
    tile_light.set_packed_value((num | num2 | num3 | -16777216) as u32);
    if tile_cache.fullbright_block() {
        tile_light = Color::WHITE;
    }
    if tile_cache.in_active() {
        tile_light = tile_cache.act_color(tile_light);
    } else if should_tile_shine(ctx, type_cache, tile_cache.frame_x) {
        tile_light = shine(ctx.g, ctx.state, tile_light, type_cache as i32);
    }
    tile_light
}

/// `TileDrawing.GetFinalLight(Tile tileCache, ushort typeCache, ref Vector3 tileLight, ref Vector3 tint)` (TileDrawing.cs:4429)
pub fn get_final_light_vec3(ctx: &DrawContext, tile_cache: &Tile, type_cache: u16, tile_light: &mut Vector3, tint: &mut Vector3) {
    *tile_light = *tile_light * *tint;
    if tile_cache.in_active() {
        tile_cache.act_color_vec3(tile_light);
    } else if should_tile_shine(ctx, type_cache, tile_cache.frame_x) {
        crate::main_draw::shine_vec3(ctx.g, ctx.state, tile_light, type_cache as i32);
    }
}

/// `TileDrawing.ShouldTileShine(ushort type, short frameX)` (TileDrawing.cs:4442)
fn should_tile_shine(ctx: &DrawContext, type_: u16, frame_x: i16) -> bool {
    if (ctx.state.shimmer_alpha > 0.0 && ctx.g.main.tile_solid[type_ as usize]) || type_ == 165 || type_ == 693 || type_ == 694 {
        return true;
    }
    if !ctx.g.main.tile_shine2[type_ as usize] {
        return false;
    }
    match type_ {
        467 | 468 => frame_x >= 144 && frame_x < 178,
        21 | 441 => frame_x >= 36 && frame_x < 178,
        _ => true,
    }
}

/// `GetScreenDrawArea(bool useOffscreenRange, out Vector2 drawOffSet, out int firstTileX, out int lastTileX, out int firstTileY,
/// out int lastTileY)` (TileDrawing.cs:6661). Returns `(drawOffSet, firstTileX, lastTileX, firstTileY, lastTileY)`.
/// `Main.Camera.ScaledPosition` / `ScaledSize` are the screen position and size (zoom 1, no view translation).
pub fn get_screen_draw_area(ctx: &mut DrawContext, use_offscreen_range: bool) -> (Vector2, i32, i32, i32, i32) {
    let scaled_position = Vector2::new(ctx.g.main.screen_position.0, ctx.g.main.screen_position.1);
    let scaled_size = Vector2::new(ctx.g.main.screen_width as f32, ctx.g.main.screen_height as f32);
    let draw_off_set = if use_offscreen_range { Vector2::new(ctx.state.off_screen_range as f32, ctx.state.off_screen_range as f32) } else { Vector2::ZERO };
    let mut first_tile_x = ((scaled_position.x - draw_off_set.x) / 16.0 - 1.0) as i32;
    let mut last_tile_x = ((scaled_position.x + scaled_size.x + draw_off_set.x) / 16.0) as i32 + 2;
    let mut first_tile_y = ((scaled_position.y - draw_off_set.y) / 16.0 - 1.0) as i32;
    let mut last_tile_y = ((scaled_position.y + scaled_size.y + draw_off_set.y) / 16.0) as i32 + 5;
    if first_tile_x < 4 {
        first_tile_x = 4;
    }
    if last_tile_x > ctx.g.main.max_tiles_x - 4 {
        last_tile_x = ctx.g.main.max_tiles_x - 4;
    }
    if first_tile_y < 4 {
        first_tile_y = 4;
    }
    if last_tile_y > ctx.g.main.max_tiles_y - 4 {
        last_tile_y = ctx.g.main.max_tiles_y - 4;
    }
    if ctx.state.section_manager.any_unfinished_sections(ctx.g) {
        ctx.state.section_manager.section_tile_frame_with_check(ctx.g, first_tile_x, first_tile_y, last_tile_x, last_tile_y);
    }
    if unported!("Main.sectionManager.AnyNeedRefresh", false) {
        crate::pending::world_gen_refresh_sections(ctx.g, first_tile_x, first_tile_y, last_tile_x, last_tile_y);
    }
    (draw_off_set, first_tile_x, last_tile_x, first_tile_y, last_tile_y)
}

/// `GetTreeBiome(int tileX, int tileY, int tileFrameX, int tileFrameY)` (TileDrawing.cs:1755)
pub fn get_tree_biome(g: &terraria_sim::Game, tile_x: i32, tile_y: i32, tile_frame_x: i32, tile_frame_y: i32) -> i32 {
    let mut num = tile_x;
    let mut i = tile_y;
    let type_ = g.main.tile.get(num, i).type_ as i32;
    if tile_frame_x == 66 && tile_frame_y <= 45 {
        num += 1;
    }
    if tile_frame_x == 88 && tile_frame_y >= 66 && tile_frame_y <= 110 {
        num -= 1;
    }
    if tile_frame_y >= 198 {
        match tile_frame_x {
            66 => num -= 1,
            44 => num += 1,
            _ => {}
        }
    } else if tile_frame_y >= 132 {
        match tile_frame_x {
            22 => num -= 1,
            44 => num += 1,
            _ => {}
        }
    }
    while g.main.tile.try_get(num, i).map(|t| t.active() && t.type_ as i32 == type_).unwrap_or(false) {
        i += 1;
    }
    get_tree_variant(g, num, i)
}

/// `GetTreeVariant(int x, int y)` (TileDrawing.cs:1798)
pub fn get_tree_variant(g: &terraria_sim::Game, x: i32, y: i32) -> i32 {
    let Some(tile) = g.main.tile.try_get(x, y) else {
        return -1;
    };
    if !tile.active() {
        return -1;
    }
    match tile.type_ {
        23 | 661 => 0,
        60 => {
            if !((y as f64) > g.main.world_surface) {
                1
            } else {
                5
            }
        }
        70 => 6,
        109 | 492 => 2,
        147 => 3,
        199 | 662 => 4,
        _ => -1,
    }
}

/// `GetPalmTreeBiome(int tileX, int tileY)` (TileDrawing.cs:1746)
pub fn get_palm_tree_biome(g: &terraria_sim::Game, tile_x: i32, tile_y: i32) -> i32 {
    let mut i = tile_y;
    while g.main.tile.try_get(tile_x, i).map(|t| t.active() && t.type_ == 323).unwrap_or(false) {
        i += 1;
    }
    get_palm_tree_variant(g, tile_x, i)
}

/// `GetPalmTreeVariant(int x, int y)` (TileDrawing.cs:3516)
pub fn get_palm_tree_variant(g: &terraria_sim::Game, x: i32, y: i32) -> i32 {
    let mut num = -1;
    let tile = g.main.tile.try_get(x, y).unwrap_or_default();
    if tile.active() && tile.type_ == 53 {
        num = 0;
    }
    if tile.active() && tile.type_ == 234 {
        num = 1;
    }
    if tile.active() && tile.type_ == 116 {
        num = 2;
    }
    if tile.active() && tile.type_ == 112 {
        num = 3;
    }
    if world_gen_is_palm_oasis_tree(g, x) {
        num += 4;
    }
    num
}

impl TileDrawing {
    /// `GetWaterAnimalCageFrame(int x, int y, int tileFrameX, int tileFrameY)` (TileDrawing.cs:6640)
    pub(super) fn get_water_animal_cage_frame(&self, ctx: &DrawContext, x: i32, y: i32, tile_frame_x: i32, tile_frame_y: i32) -> i32 {
        let num = x - tile_frame_x / 18;
        let num2 = y - tile_frame_y / 18;
        num / 2 * (num2 / 3) % ctx.state.cage_frames
    }

    /// `GetSmallAnimalCageFrame(int x, int y, int tileFrameX, int tileFrameY)` (TileDrawing.cs:6647)
    pub(super) fn get_small_animal_cage_frame(&self, ctx: &DrawContext, x: i32, y: i32, tile_frame_x: i32, tile_frame_y: i32) -> i32 {
        let num = x - tile_frame_x / 18;
        let num2 = y - tile_frame_y / 18;
        num / 3 * (num2 / 3) % ctx.state.cage_frames
    }

    /// `GetBigAnimalCageFrame(int x, int y, int tileFrameX, int tileFrameY)` (TileDrawing.cs:6654)
    pub(super) fn get_big_animal_cage_frame(&self, ctx: &DrawContext, x: i32, y: i32, tile_frame_x: i32, tile_frame_y: i32) -> i32 {
        let num = x - tile_frame_x / 18;
        let num2 = y - tile_frame_y / 18;
        num / 6 * (num2 / 4) % ctx.state.cage_frames
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::draw_context::test_support::with_ctx;

    #[test]
    fn screen_draw_area_is_the_view_in_tiles_clamped_to_the_world() {
        with_ctx(|ctx| {
            // TileDrawing.cs:6666-6685, view at (1600, 800), 1280 x 720, no off-screen range, world 200 x 200 tiles
            ctx.g.main.screen_position = (1600.0, 800.0);
            ctx.g.main.screen_width = 1280;
            ctx.g.main.screen_height = 720;
            let (off, first_x, last_x, first_y, last_y) = get_screen_draw_area(ctx, false);
            assert_eq!(off, Vector2::ZERO);
            // (1600 / 16 - 1) = 99; (1600 + 1280) / 16 + 2 = 182 (the world edge is 196); (800 / 16 - 1) = 49; (800 + 720) / 16 + 5 = 100
            assert_eq!((first_x, last_x, first_y, last_y), (99, 182, 49, 100));
            // with the off-screen range of 200 px every side grows by 12.5 tiles
            let (off, first_x, last_x, first_y, last_y) = get_screen_draw_area(ctx, true);
            assert_eq!(off, Vector2::new(200.0, 200.0));
            assert_eq!((first_x, last_x, first_y, last_y), (86, 194, 36, 112));
            // near the world corner the lower bound is 4 and the upper bound is the world size minus 4
            ctx.g.main.screen_position = (0.0, 0.0);
            let (_, first_x, last_x, first_y, last_y) = get_screen_draw_area(ctx, false);
            assert_eq!((first_x, last_x, first_y, last_y), (4, 82, 4, 50));
        });
    }

    #[test]
    fn final_light_multiplies_by_the_tint_in_whole_bytes() {
        with_ctx(|ctx| {
            // TileDrawing.cs:4396-4413: channel = (int)(light * tint / 255f), alpha forced to 255
            let mut t = Tile::default();
            t.type_ = 0;
            t.set_active(true);
            let c = get_final_light(ctx, &t, 0, Color::new(200, 100, 50, 7), Color::new(255, 128, 255, 255));
            assert_eq!((c.r(), c.g(), c.b(), c.a()), (200, 50, 50, 255));
            // 4414-4417: fullbright blocks are white
            t.set_fullbright_block(true);
            assert_eq!(get_final_light(ctx, &t, 0, Color::new(1, 2, 3, 255), Color::WHITE), Color::WHITE);
            // 4418-4421: an actuated (inactive) block is dimmed to 40%, as Tile.actColor does
            let mut t = Tile::default();
            t.set_active(true);
            t.set_in_active(true);
            let c = get_final_light(ctx, &t, 0, Color::new(200, 100, 50, 255), Color::WHITE);
            assert_eq!((c.r(), c.g(), c.b(), c.a()), (80, 40, 20, 255));
        });
    }

    #[test]
    fn solid_layer_tiles_are_the_solid_ones_plus_the_listed_exceptions() {
        with_ctx(|ctx| {
            let tiles = TileDrawing::new();
            // 4509-4516: DrawTileInSolidLayer (11, 78, 470, 475, 579) overrides Main.tileSolid
            assert!(tiles.is_tile_draw_layer_solid(ctx, 1)); // stone is solid
            assert!(!tiles.is_tile_draw_layer_solid(ctx, 4)); // torch is not
            assert!(tiles.is_tile_draw_layer_solid(ctx, 11)); // closed door: not solid, but in the solid layer
        });
    }
}
