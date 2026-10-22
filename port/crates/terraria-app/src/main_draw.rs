//! `Terraria.Main`'s drawing methods: the order the world layers are drawn in, and the helpers they use.
//!
//! This is the `Main.drawToScreen == true` path (everything is drawn straight into the back buffer; the render
//! targets `wallTarget` / `tileTarget` / `tile2Target` are not used yet). Every layer between the ported ones that is
//! not part of the tile and wall port is an `unported!` call at the place the original makes it, so the remaining
//! draw order can be read off `terraria_sim::unported::report()`.

use terraria_core::xna::{Color, Matrix, Rectangle, Vector2, Vector3};
use terraria_data::sets::wall_id::sets as wall_sets;
use terraria_sim::unported;
use terraria_sim::Game;

use crate::draw_context::{DrawContext, MainDrawState};
use crate::game_content::drawing::tile_drawing::{get_screen_draw_area, TileDrawing};
use crate::game_content::drawing::tile_drawing_base::DRAW_OWN_BLACKS;
use crate::game_content::drawing::wall_drawing::WallDrawing;
use crate::gfx::{BlendState, SamplerState, SpriteEffects};
use crate::pending::{main_color_of_the_skies, sky_manager_process_tile_color, lighting_brightness, lighting_light_tiles, lighting_mode, world_gen_solid_tile_tile, LightMode};
use crate::testing::debug_options;

/// `Main.TilesRenderer` and `Main.WallsRenderer`
pub struct Renderers {
    pub tiles_renderer: TileDrawing,
    pub walls_renderer: WallDrawing,
}

impl Renderers {
    pub fn new() -> Self {
        Self { tiles_renderer: TileDrawing::new(), walls_renderer: WallDrawing::new() }
    }
}

impl Default for Renderers {
    fn default() -> Self {
        Self::new()
    }
}

/// `Main.Transform` (Main.cs:2912) = `GameViewMatrix.TransformationMatrix`: zoom 1 and no translation is the identity.
pub fn transform() -> Matrix {
    Matrix::IDENTITY
}

/// `spriteBatch.Begin(SpriteSortMode.Deferred, BlendState.AlphaBlend, DefaultSamplerState, DepthStencilState.None, Rasterizer, null, Transform)`.
/// `Main.DefaultSamplerState` is point sampling with clamping (as main.rs set it up).
fn sprite_batch_begin_transform(ctx: &mut DrawContext) {
    ctx.sprite_batch.begin(BlendState::AlphaBlend, SamplerState::PointClamp, transform());
}

/// `Main.DrawTiles(bool solidLayer, bool intoRenderTargets = false, int waterStyleOverride = -1)` (Main.cs:22059)
pub fn draw_tiles(ctx: &mut DrawContext, r: &mut Renderers, solid_layer: bool, into_render_targets: bool, water_style_override: i32) {
    let matrix = if into_render_targets { Matrix::IDENTITY } else { transform() };
    r.tiles_renderer.base.begin(ctx, matrix);
    r.tiles_renderer.draw(ctx, solid_layer, into_render_targets, water_style_override);
    r.tiles_renderer.base.end(ctx);
}

/// `Main.DrawWalls(bool intoRenderTargets = false)` (Main.cs:59001)
pub fn draw_walls(ctx: &mut DrawContext, r: &mut Renderers, into_render_targets: bool) {
    let matrix = if into_render_targets { Matrix::IDENTITY } else { transform() };
    r.walls_renderer.base.begin(ctx, matrix);
    r.walls_renderer.draw_walls(ctx);
    r.walls_renderer.base.end(ctx);
}

/// `Main.DrawBlack(bool intoRenderTargets = false, bool force = false)` (Main.cs:58909)
pub fn draw_black(ctx: &mut DrawContext, into_render_targets: bool, force: bool) {
    if ctx.state.shimmer_alpha == 1.0 || DRAW_OWN_BLACKS {
        return;
    }
    let matrix = if into_render_targets { Matrix::IDENTITY } else { transform() };
    ctx.sprite_batch.begin(BlendState::AlphaBlend, SamplerState::PointClamp, matrix);
    let tile_color = ctx.g.main.tile_color;
    let num = (tile_color.r() as i32 + tile_color.g() as i32 + tile_color.b() as i32) / 3;
    let mut num2 = ((num as f64 * 0.4) as f32) / 255.0;
    if lighting_mode(ctx.g) == LightMode::Retro {
        num2 = (tile_color.r() as i32 - 55) as f32 / 255.0;
        if num2 < 0.0 {
            num2 = 0.0;
        }
    } else if lighting_mode(ctx.g) == LightMode::Trippy {
        num2 = (num - 55) as f32 / 255.0;
        if num2 < 0.0 {
            num2 = 0.0;
        }
    }
    let (draw_off_set, first_tile_x, last_tile_x, mut first_tile_y, mut last_tile_y) = get_screen_draw_area(ctx, !ctx.state.draw_to_screen);
    if !force {
        if first_tile_y < ctx.g.main.max_tiles_y / 2 {
            last_tile_y = last_tile_y.min(ctx.g.main.world_surface as i32 + 1);
            first_tile_y = first_tile_y.min(ctx.g.main.world_surface as i32 + 1);
        } else {
            let underworld_layer = ctx.g.main.max_tiles_y - 200;
            last_tile_y = last_tile_y.max(underworld_layer);
            first_tile_y = first_tile_y.max(underworld_layer);
        }
    }
    let flag = ctx.g.main.should_show_invisible_blocks_and_walls();
    let screen_position = Vector2::new(ctx.g.main.screen_position.0, ctx.g.main.screen_position.1);
    let black_tile = ctx.black_tile();
    let mut i = first_tile_y;
    while i < last_tile_y {
        let flag2 = i >= ctx.g.main.max_tiles_y - 200;
        if flag2 {
            num2 = 0.2;
        }
        let mut j = first_tile_x;
        while j < last_tile_x {
            let num3 = j;
            while j < last_tile_x {
                if !terraria_sim::world_gen::tile_frame::in_world(ctx.g, j, i, 0) {
                    ctx.sprite_batch.end(ctx.list);
                    return;
                }
                let tile = *ctx.g.main.tile.get(j, i);
                let mut num4 = lighting_brightness(ctx.g, j, i);
                num4 = ((num4 * 255.0) as f64).floor() as f32 / 255.0;
                let b = tile.liquid;
                let num5 = num4 <= num2 && ((!flag2 && b < 250) || world_gen_solid_tile_tile(ctx.g, &tile) || (b >= 200 && num4 == 0.0));
                let flag3 = tile.active() && ctx.g.main.tile_block_light[tile.type_ as usize] && (!tile.invisible_block() || flag);
                let flag4 = !wall_sets::TRANSPARENT[tile.wall as usize] && (!tile.invisible_wall() || flag);
                if !num5 || (!flag4 && !flag3) || (!ctx.state.draw_to_screen && ctx.state.liquid.renderer.has_full_water(j, i) && tile.wall == 0 && !tile.half_brick() && !((i as f64) <= ctx.g.main.world_surface)) {
                    break;
                }
                j += 1;
            }
            if j - num3 > 0 {
                if let Some(t) = black_tile.as_ref() {
                    let position = Vector2::new((num3 << 4) as f32, (i << 4) as f32) - screen_position + draw_off_set;
                    ctx.sprite_batch.draw(t, position, Some(Rectangle::new(0, 0, (j - num3) << 4, 16)), Color::BLACK, 0.0, Vector2::ZERO, Vector2::ONE, SpriteEffects::NONE);
                }
            }
            j += 1;
        }
        i += 1;
    }
    ctx.sprite_batch.end(ctx.list);
}

/// `Main.DrawTileEntities(bool solidLayer)` (Main.cs:21997): `TilesRenderer.PostDrawTiles(solidLayer)`
fn draw_tile_entities(ctx: &mut DrawContext, r: &mut Renderers, solid_layer: bool) {
    r.tiles_renderer.post_draw_tiles(ctx, solid_layer);
}

/// `Main.DoDraw_WallsAndBlacks()` (Main.cs:64449)
fn do_draw_walls_and_blacks(ctx: &mut DrawContext, r: &mut Renderers) {
    if ctx.state.draw_to_screen {
        ctx.sprite_batch.end(ctx.list);
        draw_black(ctx, false, false);
        draw_walls(ctx, r, false);
        sprite_batch_begin_transform(ctx);
    } else {
        unported!("Main.wallTarget (spriteBatch.Draw of the wall render target)");
    }
    unported!("Overlays.Scene.Draw(RenderLayers.Walls)");
}

/// `Main.DoDraw_Tiles_NonSolid()` (Main.cs:64371)
fn do_draw_tiles_non_solid(ctx: &mut DrawContext, r: &mut Renderers) {
    if ctx.state.draw_to_screen {
        ctx.sprite_batch.end(ctx.list);
        draw_tiles(ctx, r, false, false, -1);
    } else {
        unported!("Main.tile2Target (spriteBatch.Draw of the non-solid tile render target)");
        ctx.sprite_batch.end(ctx.list);
    }
    if !debug_options::HIDE_TILES2 {
        draw_tile_entities(ctx, r, false);
    }
    sprite_batch_begin_transform(ctx);
}

/// `Main.DoDraw_Tiles_Solid()` (Main.cs:64335)
fn do_draw_tiles_solid(ctx: &mut DrawContext, r: &mut Renderers) {
    if ctx.state.draw_to_screen {
        draw_tiles(ctx, r, true, false, -1);
    } else {
        sprite_batch_begin_transform(ctx);
        unported!("Main.tileTarget (spriteBatch.Draw of the solid tile render target)");
        ctx.sprite_batch.end(ctx.list);
    }
    if !debug_options::HIDE_TILES {
        draw_tile_entities(ctx, r, true);
    }
    sprite_batch_begin_transform(ctx);
    unported!("HitTile.DrawFreshAnimations(hitReplace)");
    unported!("HitTile.DrawFreshAnimations(hitTile)");
    ctx.sprite_batch.end(ctx.list);
}

/// `Main.DoDraw_DrawNPCsOverTiles()` (Main.cs:64397)
fn do_draw_draw_npcs_over_tiles(ctx: &mut DrawContext) {
    unported!("Main.DrawCachedProjs(DrawCacheProjsBehindNPCs)");
    sprite_batch_begin_transform(ctx);
    unported!("Main.DrawNPCs");
    unported!("Main.DrawCachedNPCs(DrawCacheNPCProjectiles)");
    ctx.sprite_batch.end(ctx.list);
}

/// `Main.DoDraw_DrawNPCsBehindTiles()` (Main.cs:64413)
fn do_draw_draw_npcs_behind_tiles(ctx: &mut DrawContext) {
    unported!("Main.DrawCachedProjs(DrawCacheProjsBehindNPCsAndTiles)");
    sprite_batch_begin_transform(ctx);
    unported!("Main.DrawNPCs(behindTiles: true)");
    ctx.sprite_batch.end(ctx.list);
}

/// `Main.DoDraw_WallsTilesNPCs()` (Main.cs:64278). The `try { } catch { TimeLogger.DrawException }` blocks only log.
fn do_draw_walls_tiles_npcs(ctx: &mut DrawContext, r: &mut Renderers) {
    unported!("Main.CacheNPCDraws");
    unported!("Main.CacheProjDraws");
    unported!("Main.DrawCachedNPCs(DrawCacheNPCsMoonMoon)");
    do_draw_walls_and_blacks(ctx, r);
    unported!("Main.DrawWoF");
    unported!("Main.DrawBackGore");
    unported!("MoonlordDeathDrama.DrawPieces");
    unported!("MoonlordDeathDrama.DrawExplosions");
    unported!("Main.DrawCachedNPCs(DrawCacheNPCsBehindNonSolidTiles)");
    do_draw_tiles_non_solid(ctx, r);
    crate::main_liquid::do_draw_waterfalls(ctx);
    ctx.sprite_batch.end(ctx.list);
    let detect_creature = unported!("Player.detectCreature", false);
    if !detect_creature {
        do_draw_draw_npcs_behind_tiles(ctx);
    }
    do_draw_tiles_solid(ctx, r);
    if detect_creature {
        do_draw_draw_npcs_behind_tiles(ctx);
    }
    unported!("Main.DrawPlayers_BehindNPCs");
    do_draw_draw_npcs_over_tiles(ctx);
    sprite_batch_begin_transform(ctx);
}

/// `Main.DoDraw(GameTime)` (Main.cs:62706) from `GraphicsDevice.Clear(Color.Black)` (Main.cs:62983) to the end of the
/// world layers (Main.cs:63333), for `drawToScreen == true`, not in the menu, no full-screen map, no UI. The caller has
/// set the back buffer as render target. Layers that are not ported are `unported!` at their place.
pub fn do_draw(ctx: &mut DrawContext, r: &mut Renderers) {
    // Main.cs:62719 `if (_drawCycleCounter == 0) TileFrameSeed = Utils.RandomNextSeed(TileFrameSeed)`, then the counter steps mod 5
    if ctx.state.draw_cycle_counter == 0 {
        ctx.state.tile_frame_seed = crate::game_content::drawing::tile_drawing_specials::utils_random_next_seed(ctx.state.tile_frame_seed);
    }
    ctx.state.draw_cycle_counter = (ctx.state.draw_cycle_counter + 1) % 5;
    // Main.cs:62833 `sunCircle += 0.01f; if ((double)sunCircle > 6.285) sunCircle -= 6.285f;` (after DoDraw_UpdateCameraPosition)
    ctx.state.sun_circle += 0.01;
    if ctx.state.sun_circle as f64 > 6.285 {
        ctx.state.sun_circle -= 6.285;
    }
    // Main.cs:62838-62840 `if (!gameMenu && !onlyDrawFancyUI) waterfallManager.FindWaterfalls(instantBGTransitionCounter == 10)`
    if !ctx.g.main.game_menu {
        crate::main_liquid::find_waterfalls(ctx);
    }
    unported!("Main.TilesRenderer.PreparePaintForTilesOnScreen");
    unported!("TilePaintSystem.PrepareAllRequests");
    ctx.list.clear(Color::BLACK); // base.GraphicsDevice.Clear(Color.Black)
    let (sun_color, moon_color, temp_mushroom_influence) = crate::main_backgrounds::do_draw_set_back_color(ctx); // Main.cs:62833-62981
    apply_color_of_the_skies_to_tiles(ctx.g);
    crate::main_backgrounds::update_atmosphere_transparency_to_sky_color(ctx); // Main.cs:62982
    // sky and background
    crate::main_backgrounds::do_draw_sky(ctx, sun_color, moon_color, temp_mushroom_influence); // Main.cs:62994-63047
    sprite_batch_begin_transform(ctx);
    unported!("Overlays.Scene.Draw(RenderLayers.Sky)");
    ctx.sprite_batch.end(ctx.list);
    sprite_batch_begin_transform(ctx); // parentSpriteBatchBeginner.Begin(spriteBatch)
    crate::main_backgrounds::draw_bg_call(ctx);
    ctx.sprite_batch.end(ctx.list);
    ctx.sprite_batch.begin(BlendState::AlphaBlend, SamplerState::LinearClamp, transform());
    crate::main_backgrounds::draw_background_black_fill(ctx);
    ctx.sprite_batch.end(ctx.list);
    unported!("Overlays.Scene.Draw(RenderLayers.Landscape)");
    lighting_light_tiles(ctx.g, get_area_to_light(ctx));
    sprite_batch_begin_transform(ctx);
    if ctx.state.shimmer_alpha > 0.0 {
        unported!("spriteBatch.Draw(MagicPixel) shimmer darkening");
    }
    unported!("Overlays.Scene.Draw(RenderLayers.InWorldUI)");
    if ctx.state.draw_to_screen {
        crate::main_liquid::draw_waters(ctx, r, true);
    } else {
        unported!("Main.backWaterTarget");
    }
    unported!("Overlays.Scene.Draw(RenderLayers.BackgroundWater)");
    if ctx.state.shimmer_alpha > 0.0 {
        ctx.sprite_batch.end(ctx.list);
        sprite_batch_begin_transform(ctx); // parentSpriteBatchBeginner.Begin(spriteBatch)
        let scene_area = crate::main_backgrounds::scene_area(ctx);
        crate::main_backgrounds::draw_stars_in_background(ctx, scene_area, true);
        ctx.sprite_batch.end(ctx.list);
        sprite_batch_begin_transform(ctx);
    }
    if ctx.state.draw_to_screen {
        crate::main_backgrounds::draw_background(ctx);
    } else {
        unported!("Main.backgroundTarget");
    }
    unported!("Overlays.Scene.Draw(RenderLayers.Background)");
    unported!("ScreenDarkness.DrawBack");
    crate::main_backgrounds::step_magma_bg_frame(ctx);
    do_draw_walls_tiles_npcs(ctx, r);
    unported!("Overlays.Scene.Draw(RenderLayers.TilesAndNPCs)");
    ctx.sprite_batch.end(ctx.list);
    unported!("Main.SortDrawCacheWorms");
    unported!("Main.DrawSuperSpecialProjectiles(DrawCacheFirstFractals)");
    unported!("Main.DrawCachedProjs(DrawCacheProjsBehindProjectiles)");
    unported!("Main.DrawProjectiles");
    unported!("ParticleSystem_World_BehindPlayers.Draw");
    unported!("Main.DrawPlayers_AfterProjectiles");
    unported!("Main.DrawCachedProjs(DrawCacheProjsOverPlayers)");
    unported!("ParticleSystem_World_OverPlayers.Draw");
    unported!("Main.DrawCachedNPCs(DrawCacheNPCsOverPlayers)");
    unported!("Main.DrawItems");
    unported!("Main.DrawRain");
    unported!("Main.DrawGore");
    unported!("Main.DrawDust");
    sprite_batch_begin_transform(ctx); // Main.cs:63294 `spriteBatch.Begin(...)` (open to the `End` at Main.cs:63318)
    unported!("Main.DrawProjectileHitboxes");
    unported!("Main.DrawNPCAggro");
    unported!("Overlays.Scene.Draw(RenderLayers.Entities)");
    if ctx.state.draw_to_screen {
        crate::main_liquid::draw_waters(ctx, r, false);
        unported!("Main.DrawWires");
    } else {
        unported!("Main.waterTarget");
    }
    unported!("Overlays.Scene.Draw(RenderLayers.ForegroundWater)");
    unported!("Main.DrawCachedProjs(DrawCacheProjsOverWiresUI)");
    unported!("Main.DrawInfernoRings");
    unported!("Main.DrawPaladinsShield");
    ctx.sprite_batch.end(ctx.list); // Main.cs:63318
    unported!("Main.DrawLensFlare");
    unported!("ScreenDarkness.DrawFront");
    unported!("MoonlordDeathDrama.DrawWhite");
    unported!("ScreenObstruction.Draw");
    unported!("Overlays.Scene.Draw(RenderLayers.All)");
    unported!("DebugVisualizer.World.Draw");
    unported!("Main.DrawInterface (chat bubbles, combat text, inventory, cursor)");
}

/// `Main.ApplyColorOfTheSkiesToTiles()` (Main.cs:64186)
pub(crate) fn apply_color_of_the_skies_to_tiles(g: &mut Game) {
    let color_of_the_skies = main_color_of_the_skies(g);
    let (r, gc, b) = (color_of_the_skies.r() as i32, color_of_the_skies.g() as i32, color_of_the_skies.b() as i32);
    let mut tile_color = g.main.tile_color;
    tile_color.set_a(255);
    tile_color.set_r(((r + gc + b + r * 7) / 10) as u8);
    tile_color.set_g(((r + gc + b + gc * 7) / 10) as u8);
    tile_color.set_b(((r + gc + b + b * 7) / 10) as u8);
    g.main.tile_color = sky_manager_process_tile_color(g, tile_color);
}

/// `Main.GetAreaToLight()` (Main.cs:64940): `Lighting.UsingNewLighting` is true in `LightMode.Color`, so the scaled camera is used.
pub(crate) fn get_area_to_light(ctx: &DrawContext) -> Rectangle {
    let vector = Vector2::new(ctx.g.main.screen_position.0, ctx.g.main.screen_position.1);
    let vector2 = Vector2::new(ctx.g.main.screen_width as f32, ctx.g.main.screen_height as f32);
    let num = ((vector.x / 16.0) as f64).floor() as i32 - 1;
    let num2 = (((vector.x + vector2.x) / 16.0) as f64).floor() as i32 + 2;
    let num3 = ((vector.y / 16.0) as f64).floor() as i32 - 1;
    let num4 = (((vector.y + vector2.y) / 16.0) as f64).floor() as i32 + 2;
    Rectangle::new(num, num3, num2 - num, num4 - num3)
}

/// `Main.shine(Color newColor, int type)` (Main.cs:21796)
pub fn shine(g: &Game, state: &MainDrawState, new_color: Color, type_: i32) -> Color {
    let mut num = new_color.r() as i32;
    let mut num2 = new_color.g() as i32;
    let mut num3 = new_color.b() as i32;
    let mut num4 = 0.6_f32;
    let (r, gc, b) = (new_color.r() as f32, new_color.g() as f32, new_color.b() as f32);
    match type_ {
        25 => {
            num = (r * 0.95) as i32;
            num2 = (gc * 0.85) as i32;
            num3 = (b as f64 * 1.1) as i32;
        }
        117 => {
            num = (r * 1.1) as i32;
            num2 = (gc * 1.0) as i32;
            num3 = (b as f64 * 1.2) as i32;
        }
        204 => {
            num4 = 0.3 + g.main.mouse_text_color as f32 / 300.0;
            num = (r * (1.3 * num4)) as i32;
            if num > 255 {
                num = 255;
            }
            return Color::from_ints(num, num2, num3, 255);
        }
        211 => {
            num4 = 0.3 + g.main.mouse_text_color as f32 / 300.0;
            num2 = (gc * (1.5 * num4)) as i32;
            num3 = (b * (1.1 * num4)) as i32;
        }
        147 | 161 => {
            num = (r * 1.1) as i32;
            num2 = (gc * 1.12) as i32;
            num3 = (b as f64 * 1.15) as i32;
        }
        163 => {
            num = (r * 1.05) as i32;
            num2 = (gc * 1.1) as i32;
            num3 = (b as f64 * 1.15) as i32;
        }
        164 => {
            num = (r * 1.1) as i32;
            num2 = (gc * 1.1) as i32;
            num3 = (b as f64 * 1.2) as i32;
        }
        178 => {
            num4 = 0.5;
            num = (r * (1.0 + num4)) as i32;
            num2 = (gc * (1.0 + num4)) as i32;
            num3 = (b * (1.0 + num4)) as i32;
        }
        185 | 186 => {
            num4 = 0.3;
            num = (r * (1.0 + num4)) as i32;
            num2 = (gc * (1.0 + num4)) as i32;
            num3 = (b * (1.0 + num4)) as i32;
        }
        262..=268 => {
            num3 += 100;
            num += 100;
            num2 += 100;
        }
        _ => {
            if g.main.tile_shine2[type_ as usize] {
                num = (r * (1.0 + num4)) as i32;
                num2 = (gc * (1.0 + num4)) as i32;
                num3 = (b * (1.0 + num4)) as i32;
            }
        }
    }
    let num5 = state.shimmer_alpha;
    if num5 > 0.0 {
        num = (num as f32 * (1.0 - num5) + num as f32 * state.shimmer_shine.x * num5) as i32;
        num2 = (num2 as f32 * (1.0 - num5) + num2 as f32 * state.shimmer_shine.y * num5) as i32;
        num3 = (num3 as f32 * (1.0 - num5) + num3 as f32 * state.shimmer_shine.z * num5) as i32;
    }
    if num > 255 {
        num = 255;
    }
    if num2 > 255 {
        num2 = 255;
    }
    if num3 > 255 {
        num3 = 255;
    }
    Color::new(num as u8, num2 as u8, num3 as u8, new_color.a())
}

/// `Main.shine(ref Vector3 newColor, int type)` (Main.cs:21901)
pub fn shine_vec3(g: &Game, state: &MainDrawState, new_color: &mut Vector3, type_: i32) {
    let mut num = 0.6_f32;
    match type_ {
        25 => {
            new_color.x *= 0.95;
            new_color.y *= 0.85;
            new_color.z *= 1.1;
        }
        117 => {
            new_color.x *= 1.1;
            new_color.z *= 1.2;
        }
        204 => {
            num = 0.3 + g.main.mouse_text_color as f32 / 300.0;
            new_color.x *= 1.3 * num;
        }
        211 => {
            num = 0.3 + g.main.mouse_text_color as f32 / 300.0;
            new_color.y *= 1.5 * num;
            new_color.z *= 1.1 * num;
        }
        147 | 161 => {
            new_color.x *= 1.1;
            new_color.y *= 1.12;
            new_color.z *= 1.15;
        }
        163 => {
            new_color.x *= 1.05;
            new_color.y *= 1.1;
            new_color.z *= 1.15;
        }
        164 => {
            new_color.x *= 1.1;
            new_color.y *= 1.1;
            new_color.z *= 1.2;
        }
        178 => {
            num = 0.5;
            new_color.x *= 1.0 + num;
            new_color.y *= 1.0 + num;
            new_color.z *= 1.0 + num;
        }
        185 | 186 => {
            num = 0.3;
            new_color.x *= 1.0 + num;
            new_color.y *= 1.0 + num;
            new_color.z *= 1.0 + num;
        }
        262..=268 => {
            new_color.x += 0.39;
            new_color.y += 0.39;
            new_color.z += 0.39;
        }
        _ => {
            if g.main.tile_shine2[type_ as usize] {
                new_color.x *= 1.0 + num;
                new_color.y *= 1.0 + num;
                new_color.z *= 1.0 + num;
            }
        }
    }
    let num2 = state.shimmer_alpha;
    if num2 > 0.0 {
        // Main.shimmerShine is assigned (1.2, 1, 1.6) here in the original before it is used
        let shimmer_shine = Vector3::new(1.2, 1.0, 1.6);
        new_color.x = new_color.x * (1.0 - num2) + new_color.x * shimmer_shine.x * num2;
        new_color.y = new_color.y * (1.0 - num2) + new_color.y * shimmer_shine.y * num2;
        new_color.z = new_color.z * (1.0 - num2) + new_color.z * shimmer_shine.z * num2;
        if new_color.x > 1.0 {
            new_color.x = 1.0;
        }
        if new_color.y > 1.0 {
            new_color.y = 1.0;
        }
        if new_color.z > 1.0 {
            new_color.z = 1.0;
        }
    }
}

/// `Main.hslToRgb(float Hue, float Saturation, float Luminosity, byte a = byte.MaxValue)` (Main.cs:48747)
pub fn hsl_to_rgb(hue: f32, saturation: f32, luminosity: f32, a: u8) -> Color {
    let (r, g, b);
    if saturation == 0.0 {
        let v = (luminosity as f64 * 255.0).round_ties_even() as i32 as u8;
        r = v;
        g = v;
        b = v;
    } else {
        let num = hue as f64;
        let num2 = if !((luminosity as f64) < 0.5) { (luminosity + saturation - luminosity * saturation) as f64 } else { luminosity as f64 * (1.0 + saturation as f64) };
        let t = 2.0 * luminosity as f64 - num2;
        let mut c = num + 1.0 / 3.0;
        let mut c2 = num;
        let c3 = num - 1.0 / 3.0;
        c = hue2rgb(c, t, num2);
        c2 = hue2rgb(c2, t, num2);
        let num3 = hue2rgb(c3, t, num2);
        r = (c * 255.0).round_ties_even() as i32 as u8;
        g = (c2 * 255.0).round_ties_even() as i32 as u8;
        b = (num3 * 255.0).round_ties_even() as i32 as u8;
    }
    Color::new(r, g, b, a)
}

/// `Main.hue2rgb(double c, double t1, double t2)` (Main.cs:48776)
fn hue2rgb(mut c: f64, t1: f64, t2: f64) -> f64 {
    if c < 0.0 {
        c += 1.0;
    }
    if c > 1.0 {
        c -= 1.0;
    }
    if 6.0 * c < 1.0 {
        return t1 + (t2 - t1) * 6.0 * c;
    }
    if 2.0 * c < 1.0 {
        return t2;
    }
    if 3.0 * c < 2.0 {
        return t1 + (t2 - t1) * (2.0 / 3.0 - c) * 6.0;
    }
    t1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hsl_to_rgb_matches_hand_computed_values() {
        // saturation 0: grey of round(0.5 * 255) = round-half-even(127.5) = 128
        let c = hsl_to_rgb(0.3, 0.0, 0.5, 255);
        assert_eq!((c.r(), c.g(), c.b(), c.a()), (128, 128, 128, 255));
        // pure red: hue 0, saturation 1, luminosity 0.5 -> t2 = 1, t1 = 0; c = 1/3 -> 2*c < 1 -> t2 = 1 (255); c2 = 0 -> 6c < 1 -> t1 = 0; c3 = -1/3 -> +1 = 2/3 -> t1 = 0
        let c = hsl_to_rgb(0.0, 1.0, 0.5, 255);
        assert_eq!((c.r(), c.g(), c.b()), (255, 0, 0));
    }
}
