//! `Terraria.Main`'s liquid drawing and the liquid parts of its update: `DrawWaters`, `DrawLiquid`, `DrawTileInWater`, `CalculateWaterStyle`,
//! `IsLiquidStyleWater`, `DoDraw_Waterfalls`, the `liquidAlpha[]` fade between water styles, and the calls `Main.DoUpdate` / `WorldGen.UpdateWorld`
//! make into `LiquidRenderer`, `WaterfallManager` and `Liquid.UpdateLiquid`.
//!
//! This is the `Main.drawToScreen == true` path: liquids are drawn straight into the back buffer, as the tile and wall layers are. The original's
//! render-target path (`backWaterTarget`, `waterTarget` and `WaterShaderData`, the screen-space ripple / distortion shader behind
//! `Filters.Scene["WaterDistortion"]`) is not ported: it is `unported!` at its call sites (`LiquidRenderer.WaveFilters`, `Main.backWaterTarget`,
//! `Main.waterTarget`). `Main.oldDrawWater` (the retro-lighting path `DrawLiquid` dispatches to) is `unported!` as well.

use terraria_core::xna::{Rectangle, Vector2};
use terraria_core::UnifiedRandom;
use terraria_sim::unported;
use terraria_sim::Game;

use crate::draw_context::DrawContext;
use crate::game_content::drawing::tile_drawing::{get_screen_draw_area, TileDrawing};
use crate::game_content::drawing::tile_drawing_liquid::draw_liquid_behind_tiles;
use crate::game_content::liquid::liquid_renderer::LiquidRenderer;
use crate::gfx::{SpriteEffects, Texture2D};
use crate::main_draw::{transform, Renderers};
use crate::pending::{lighting_get_color, lighting_not_retro};
use crate::waterfall_manager::WaterfallManager;

/// `DebugOptions.hideWater` (DebugOptions.cs:56)
pub const HIDE_WATER: bool = false;

/// The client statics of `Main` that only the liquid drawing reads or writes, and the two renderer objects.
pub struct MainLiquidState {
    /// `LiquidRenderer.Instance`
    pub renderer: LiquidRenderer,
    /// `Main.instance.waterfallManager`
    pub waterfall_manager: WaterfallManager,
    /// `Main.drewLava` (Main.cs:1177). Only `Main.oldDrawWater` sets it to true.
    pub drew_lava: bool,
    /// `Main.ambientWaterfallX`, `Y`, `Strength` (Main.cs:2356-2360): written by `WaterfallManager.DrawWaterfall`, read by the ambient sound code
    pub ambient_waterfall_x: f32,
    pub ambient_waterfall_y: f32,
    pub ambient_waterfall_strength: f32,
    /// `Main.ambientLavafallX`, `Y`, `Strength` (Main.cs:2362-2366)
    pub ambient_lavafall_x: f32,
    pub ambient_lavafall_y: f32,
    pub ambient_lavafall_strength: f32,
    /// `Main.player[Main.myPlayer].lavaOpacity` (Player.cs:1533, `1f`): written by `Player.Update` (lava vision), which is not ported
    pub lava_opacity: f32,
    /// the generator `Main.SwapRandom("UpdateLiquid")` swaps in around `Liquid.UpdateLiquid` (WorldGen.cs:72371)
    update_liquid_rand: UnifiedRandom,
}

impl Default for MainLiquidState {
    fn default() -> Self {
        Self::new()
    }
}

impl MainLiquidState {
    pub fn new() -> Self {
        Self {
            renderer: LiquidRenderer::new(),
            waterfall_manager: WaterfallManager::new(),
            drew_lava: false,
            ambient_waterfall_x: -1.0,
            ambient_waterfall_y: -1.0,
            ambient_waterfall_strength: 0.0,
            ambient_lavafall_x: -1.0,
            ambient_lavafall_y: -1.0,
            ambient_lavafall_strength: 0.0,
            lava_opacity: 1.0,
            // `new UnifiedRandom(ActiveWorldFileData.Seed)` (Main.cs:3393): the world seed is not kept by the loader yet
            update_liquid_rand: UnifiedRandom::new(0),
        }
    }
}

/// The textures of the liquid drawing: `TextureAssets.Liquid[n]`, `LiquidSlope[n]`, the `Images/Misc/water_<n>` textures `LiquidRenderer` requests
/// (`PrepareAssets`, LiquidRenderer.cs:156) and `WaterfallManager.waterfallTexture[n]` (WaterfallManager.cs:91).
impl DrawContext<'_> {
    /// `LiquidRenderer._liquidTextures[n].Value` (`Images/Misc/water_<n>`)
    pub fn misc_water_texture(&mut self, n: i32) -> Option<Texture2D> {
        self.texture(&format!("Images/Misc/water_{n}"))
    }

    /// `TextureAssets.Liquid[n].Value` (`Images/Liquid_<n>`, AssetInitializer.cs:469)
    pub fn liquid_texture(&mut self, n: i32) -> Option<Texture2D> {
        self.texture(&format!("Images/Liquid_{n}"))
    }

    /// `TextureAssets.LiquidSlope[n].Value` (`Images/LiquidSlope_<n>`, AssetInitializer.cs:470)
    pub fn liquid_slope_texture(&mut self, n: i32) -> Option<Texture2D> {
        self.texture(&format!("Images/LiquidSlope_{n}"))
    }

    /// `WaterfallManager.waterfallTexture[n].Value` (`Images/Waterfall_<n>`)
    pub fn waterfall_texture(&mut self, n: i32) -> Option<Texture2D> {
        self.texture(&format!("Images/Waterfall_{n}"))
    }
}

/// `Main.IsLiquidStyleWater(int liquidStyle)` (Main.cs:58407)
pub fn is_liquid_style_water(liquid_style: i32) -> bool {
    if liquid_style != 1 && liquid_style != 11 {
        return liquid_style != 14;
    }
    false
}

/// `Main.CalculateWaterStyle(bool ignoreFountains = false)` (Main.cs:58339)
pub fn calculate_water_style(g: &Game, ignore_fountains: bool) -> i32 {
    // `SceneMetrics.ActiveFountainColor` (-1 when no water fountain is near; the scan that sets it is not ported)
    let active_fountain_color = unported!("SceneMetrics.ActiveFountainColor", -1);
    if active_fountain_color >= 0 && !ignore_fountains {
        return active_fountain_color;
    }
    if g.main.blood_moon && !g.main.day_time {
        return 9;
    }
    let scene = &g.main.scene_metrics;
    match g.main.bg_style {
        1 | 5 => 2,
        3 => 3,
        8 | 14 => 10,
        6 | 13 => 4,
        7 => 5,
        2 => {
            if g.main.remix_world {
                return 6;
            }
            if scene.below_surface {
                return 12;
            }
            6
        }
        4 => 13,
        _ => {
            if g.main.remix_world {
                if (scene.tile_center.1 as f64) > g.main.rock_layer {
                    return 7;
                }
                if scene.below_surface {
                    return 8;
                }
            } else {
                if (scene.tile_center.1 as f64) > g.main.rock_layer + 40.0 {
                    if scene.zone_glowshroom {
                        return 7;
                    }
                    return 8;
                }
                if scene.below_surface {
                    return 7;
                }
            }
            0
        }
    }
}

/// `Main.DrawWaters(bool isBackground = false)` (Main.cs:58416)
pub fn draw_waters(ctx: &mut DrawContext, r: &mut Renderers, is_background: bool) {
    if HIDE_WATER {
        return;
    }
    ctx.state.liquid.drew_lava = false;
    if !is_background {
        ctx.g.main.water_style = calculate_water_style(ctx.g, false);
        for i in 0..15 {
            if is_liquid_style_water(ctx.g.main.water_style) {
                if ctx.g.main.water_style != i {
                    ctx.g.main.liquid_alpha[i as usize] = (ctx.g.main.liquid_alpha[i as usize] - 0.2).max(0.0);
                } else {
                    ctx.g.main.liquid_alpha[i as usize] = (ctx.g.main.liquid_alpha[i as usize] + 0.2).min(1.0);
                }
            }
        }
    }
    if !ctx.state.draw_to_screen && !is_background {
        let (_draw_off_set, first_tile_x, last_tile_x, first_tile_y, last_tile_y) = get_screen_draw_area(ctx, !ctx.state.draw_to_screen);
        let draw_area = Rectangle::new(first_tile_x, first_tile_y, last_tile_x - first_tile_x, last_tile_y - first_tile_y);
        ctx.state.liquid.renderer.prepare_draw(ctx.g, draw_area);
    } else if ctx.state.draw_to_screen {
        // The original never combines `drawToScreen` with the new liquid renderer (`drawToScreen` is `Lighting.UpdateEveryFrame`, which is
        // true only in retro lighting, where `DrawLiquid` goes to `oldDrawWater`), so nothing prepares the cache on this path. The port draws
        // directly to the screen in every light mode: the cache is prepared here for the screen area, as the `!drawToScreen` branch above does.
        let (_draw_off_set, first_tile_x, last_tile_x, first_tile_y, last_tile_y) = get_screen_draw_area(ctx, false);
        let draw_area = Rectangle::new(first_tile_x, first_tile_y, last_tile_x - first_tile_x, last_tile_y - first_tile_y);
        ctx.state.liquid.renderer.prepare_draw(ctx.g, draw_area);
        // `Main.tileBatch` is whatever the last `Begin` left in the original; the port begins it with `Transform`
        ctx.tile_batch.begin(transform());
    }
    let mut flag = false;
    for j in 0..15 {
        if is_liquid_style_water(j) && ctx.g.main.liquid_alpha[j as usize] > 0.0 && j != ctx.g.main.water_style {
            let alpha = if is_background { 1.0 } else { ctx.g.main.liquid_alpha[j as usize] };
            draw_liquid(ctx, r, is_background, j, alpha, true);
            flag = true;
        }
    }
    let water_style = ctx.g.main.water_style;
    let alpha = if flag { ctx.g.main.liquid_alpha[water_style as usize] } else { 1.0 };
    draw_liquid(ctx, r, is_background, water_style, alpha, false);
}

/// `Main.DrawLiquid(bool bg, int waterStyle, float Alpha = 1f, bool waterOnly = false)` (Main.cs:58468)
pub fn draw_liquid(ctx: &mut DrawContext, r: &mut Renderers, bg: bool, water_style: i32, alpha: f32, water_only: bool) {
    if !lighting_not_retro(ctx.g) {
        unported!("Main.oldDrawWater");
        return;
    }
    let draw_offset = (if ctx.state.draw_to_screen { Vector2::ZERO } else { Vector2::new(ctx.state.off_screen_range as f32, ctx.state.off_screen_range as f32) })
        - Vector2::new(ctx.g.main.screen_position.0, ctx.g.main.screen_position.1);
    if bg {
        draw_liquid_behind_tiles(ctx, water_style);
    }
    // the renderer is taken out of the draw state while it draws, because it needs the whole context
    let mut renderer = std::mem::take(&mut ctx.state.liquid.renderer);
    renderer.draw_normal_liquids(ctx, &mut r.tiles_renderer, draw_offset, water_style, alpha, bg, water_only);
    if !water_only {
        renderer.draw_shimmer(ctx, &mut r.tiles_renderer, draw_offset, bg);
    }
    ctx.state.liquid.renderer = renderer;
}

/// `Main.DrawTileInWater(Vector2 drawOffset, int x, int y)` (Main.cs:58487): the lily pad (tile 518) is drawn at the liquid's height
pub fn draw_tile_in_water(ctx: &mut DrawContext, tiles: &mut TileDrawing, draw_offset: Vector2, x: i32, y: i32) {
    let Some(tile) = ctx.g.main.tile.try_get(x, y) else { return };
    if tile.active() && tile.type_ == 518 {
        ctx.tile_texture(tile.type_ as i32); // `instance.LoadTiles(Main.tile[x, y].type)`
        let mut num = tile.liquid as i32 / 16;
        num -= 3;
        if terraria_sim::world_gen::tile_queries::solid_tile(ctx.g, x, y - 1, false) && num > 8 {
            num = 8;
        }
        let value = Rectangle::new(tile.frame_x as i32, tile.frame_y as i32, 16, 16);
        let tile_draw_texture = tiles.get_tile_draw_texture(ctx, &tile, x, y);
        if let Some(t) = tile_draw_texture {
            let color = lighting_get_color(ctx.g, x, y);
            ctx.sprite_batch.draw(&t, Vector2::new((x * 16) as f32, (y * 16 - num) as f32) + draw_offset, Some(value), color, 0.0, Vector2::ZERO, Vector2::ONE, SpriteEffects::NONE);
        }
    }
}

/// `Main.DoDraw_Waterfalls()` (Main.cs:64323)
pub fn do_draw_waterfalls(ctx: &mut DrawContext) {
    if !HIDE_WATER {
        ctx.tile_batch.begin(transform());
        let mut manager = std::mem::take(&mut ctx.state.liquid.waterfall_manager);
        manager.draw(ctx);
        ctx.state.liquid.waterfall_manager = manager;
        let _ = ctx.tile_batch.end(ctx.list);
    }
}

/// `waterfallManager.FindWaterfalls(instantBGTransitionCounter == 10)` (Main.cs:62840, inside `DoDraw` under `!gameMenu && !onlyDrawFancyUI`)
pub fn find_waterfalls(ctx: &mut DrawContext) {
    let forced = ctx.g.main.instant_bg_transition_counter == 10;
    ctx.state.liquid.waterfall_manager.find_waterfalls(ctx.g, ctx.state.gfx_quality, forced);
}

/// `Main.DoUpdate`'s liquid statics for a frame in which no world is being generated or loaded (Main.cs:17287-17324, `gfxQuality` from the context):
/// `Liquid.cycles`, `Liquid.curMaxLiquid`.
pub fn do_update_liquid_statics(g: &mut Game, gfx_quality: f32) {
    if !g.world_gen.is_generating_or_loading_world {
        g.liquid.cycles = (17.0_f32 - 10.0_f32 * gfx_quality) as i32;
        g.liquid.cur_max_liquid = (terraria_sim::liquid::MAX_LIQUID as f64 * 0.25 + terraria_sim::liquid::MAX_LIQUID as f64 * 0.75 * gfx_quality as f64) as i32;
        if g.liquid.setting_use_reduced_max_liquids {
            g.liquid.cur_max_liquid = (2500.0_f32 + 2500.0_f32 * gfx_quality) as i32;
        }
    }
    if !g.world_gen.is_generating_or_loading_world && g.liquid.quick_settle {
        g.liquid.cur_max_liquid = terraria_sim::liquid::MAX_LIQUID;
        if g.liquid.setting_use_reduced_max_liquids {
            g.liquid.cur_max_liquid = 5000;
        }
        g.liquid.cycles = 1;
    }
}

/// The liquid calls of one `Main.DoUpdate` tick, at the places they sit in the original: `LiquidRenderer.Instance.Update(gameTime)` (Main.cs:17254),
/// the `Liquid` statics (Main.cs:17287-17324), `waterfallManager.UpdateFrame()` (Main.cs:19782, the end of `DoUpdate_AnimateWaterfalls`) and
/// `WorldGen.UpdateWorld`'s `Liquid.skipCount++ ... Liquid.UpdateLiquid()` (WorldGen.cs:72368-72376). `gameTime.ElapsedGameTime` is the fixed 1/60 s.
pub fn update(ctx: &mut DrawContext) {
    ctx.state.liquid.renderer.update(ctx.g, 1.0 / 60.0);
    do_update_liquid_statics(ctx.g, ctx.state.gfx_quality);
    ctx.state.liquid.waterfall_manager.update_frame();
    ctx.g.liquid.skip_count += 1;
    if ctx.g.liquid.skip_count > 1 {
        // using (Main.SwapRandom("UpdateLiquid")) { Liquid.UpdateLiquid(); }
        std::mem::swap(&mut ctx.g.main.rand, &mut ctx.state.liquid.update_liquid_rand);
        terraria_sim::liquid::update_liquid(ctx.g);
        std::mem::swap(&mut ctx.g.main.rand, &mut ctx.state.liquid.update_liquid_rand);
        ctx.g.liquid.skip_count = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::draw_context::test_support::with_ctx;

    #[test]
    fn liquid_style_water_excludes_lava_honey_shimmer() {
        let water: Vec<i32> = (0..15).filter(|&i| is_liquid_style_water(i)).collect();
        assert_eq!(water, vec![0, 2, 3, 4, 5, 6, 7, 8, 9, 10, 12, 13]);
    }

    #[test]
    fn water_style_follows_the_background_style_and_depth() {
        let mut g = Game::new();
        g.main.rock_layer = 400.0;
        g.main.world_surface = 300.0;
        assert_eq!(calculate_water_style(&g, false), 0);
        g.main.scene_metrics.below_surface = true;
        assert_eq!(calculate_water_style(&g, false), 7);
        g.main.scene_metrics.tile_center = (100, 500);
        assert_eq!(calculate_water_style(&g, false), 8);
        g.main.scene_metrics.zone_glowshroom = true;
        assert_eq!(calculate_water_style(&g, false), 7);
        g.main.bg_style = 1;
        assert_eq!(calculate_water_style(&g, false), 2);
        g.main.blood_moon = true;
        g.main.day_time = false;
        assert_eq!(calculate_water_style(&g, false), 9);
    }

    /// The foreground pass fades `liquidAlpha` by 0.2 per call towards the current style.
    #[test]
    fn liquid_alpha_fades_with_the_water_style() {
        with_ctx(|ctx| {
            let mut r = Renderers::new();
            ctx.g.main.screen_width = 160;
            ctx.g.main.screen_height = 160;
            ctx.g.main.screen_position = (800.0, 800.0);
            for _ in 0..3 {
                draw_waters(ctx, &mut r, false);
            }
            assert!((ctx.g.main.liquid_alpha[0] - 0.6).abs() < 1e-5, "style 0 alpha {}", ctx.g.main.liquid_alpha[0]);
            assert_eq!(ctx.g.main.liquid_alpha[2], 0.0);
        });
    }
}
