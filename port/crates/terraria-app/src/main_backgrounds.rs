//! The sky, the sun, moon and stars, the horizon gradients and the clouds of `Terraria.Main` (the hand-written part), and the
//! helpers the generated background drawing (`main_backgrounds_gen.rs`, from `Main.DrawBG` / `DrawSurfaceBG*` / `DrawBackground*`)
//! calls. `Main.drawToScreen == true` path only: everything is drawn into the back buffer, as the tile layer does.
//!
//! `HorizonRenderer` is `NextHorizonRenderer` (Main.cs:2460). Its clouds are drawn with the `HorizonClouds` shader
//! (`CloudsEnd`); the shader is not ported, so each cloud is drawn with the sprite batch in the same order and with the same
//! colour the shader receives (`unported!("GameShaders.Misc[HorizonClouds]")`).

use terraria_core::xna::{Color, Rectangle, Vector2};
use terraria_sim::main_time::{get_visibilities, horizon_phase};
use terraria_sim::unported;

use crate::draw_context::DrawContext;
use crate::gfx::{SpriteEffects, Texture2D};

pub(crate) use crate::main_backgrounds_gen::*;

/// `Main.SceneArea` (Main.cs:190): what `DrawSunAndMoon` / `DrawStarsInBackground` are given
#[derive(Clone, Copy, Debug)]
pub struct SceneArea {
    pub total_width: f32,
    pub total_height: f32,
    pub bg_top_y: i32,
}

/// Start-up of the sky assets: `AssetInitializer.cs:422-423` (`Main.instance.LoadBackground(0)` and `(49)`).
pub fn initialize_backgrounds(ctx: &mut DrawContext) {
    if !ctx.g.main.background_loaded[0] {
        load_background(ctx, 0);
        load_background(ctx, 49);
    }
}

/// `Main.LoadBackground(int i)` (Main.cs:60021): requests `TextureAssets.Background[i]` the first time and records its size.
pub fn load_background(ctx: &mut DrawContext, i: i32) {
    if i >= 0 && !ctx.g.main.background_loaded[i as usize] {
        ctx.g.main.background_loaded[i as usize] = true;
        let (w, h) = ctx.texture(&format!("Images/Background_{i}")).map_or((0, 0), |t| (t.width, t.height));
        ctx.g.main.background_width[i as usize] = w;
        ctx.g.main.background_height[i as usize] = h;
        match i {
            219 | 220 | 221 | 235 | 271 | 272 | 273 | 281 | 302 | 303 | 305 | 307 | 309 | 311 | 313 | 315 | 317 | 326 | 337 | 338 | 341 => {
                ctx.g.main.background_width[i as usize] /= 2;
                ctx.g.main.background_height[i as usize] /= 2;
            }
            _ => {}
        }
    }
}

/// `Utils.Frame(this Texture2D tex, int horizontalFrames, int verticalFrames, int frameX, int frameY, int sizeOffsetX = 0, int sizeOffsetY = 0)` (Utils.cs:1361)
/// through `Utils.Frame(this Asset<Texture2D>, ...)` (Utils.cs:1336): an asset that is not loaded gives `Rectangle.Empty`.
pub fn texture_frame(tex: &Option<Texture2D>, horizontal_frames: i32, vertical_frames: i32, frame_x: i32, frame_y: i32) -> Rectangle {
    let Some(tex) = tex else { return Rectangle::EMPTY };
    let num = tex.width / horizontal_frames;
    let num2 = tex.height / vertical_frames;
    Rectangle::new(num * frame_x, num2 * frame_y, num, num2)
}

/// `Main.WallLightAt(int i, int j)` (Main.cs:59... `WallLightAt(i, j, ShouldShowInvisibleBlocksAndWalls())`)
pub fn wall_light_at(ctx: &mut DrawContext, i: i32, j: i32) -> bool {
    let show_invisible = ctx.g.main.should_show_invisible_blocks_and_walls();
    // `Main.tile[i, j]` is null / out of range outside the world: callers check `WorldGen.InWorld(i, j, 1)` first
    let Some(tile) = ctx.g.main.tile.try_get(i, j) else { return false };
    if !ctx.g.main.wall_light[tile.wall as usize] {
        if !show_invisible {
            return tile.invisible_wall();
        }
        return false;
    }
    true
}

/// `Main.ShouldDrawBackgroundTileAt(int i, int j)` (Main.cs:56031)
pub fn should_draw_background_tile_at(ctx: &mut DrawContext, i: i32, j: i32) -> bool {
    if ctx.g.main.cave_parallax != 0.0 {
        if wall_light_at(ctx, i, j) || wall_light_at(ctx, i - 1, j) || wall_light_at(ctx, i + 1, j) {
            return true;
        }
    } else if wall_light_at(ctx, i, j) {
        return true;
    }
    false
}

/// `Main.ShouldDrawSurfaceBackground()` (Main.cs:60620)
pub fn should_draw_surface_background(ctx: &mut DrawContext) -> bool {
    terraria_sim::main_time::should_draw_surface_background(ctx.g)
}

/// `Main.DrawSurfaceBG_GetFogPower()` (Main.cs:60713)
pub fn draw_surface_bg_get_fog_power(ctx: &mut DrawContext) -> f32 {
    ctx.g.main.cloud_alpha.max(ctx.g.main.graveyard_visual_intensity * 0.92)
}

/// `Main.UpdateOceanWaterLineForAmbience()` (Main.cs:60958): feeds the ambient sky sounds / sprites
pub fn update_ocean_water_line_for_ambience(_ctx: &mut DrawContext) {
    unported!("AmbientSkyDrawCache.Instance.SetOceanLineInfo");
}

// ---------------------------------------------------------------------------------------------------------------------
// background visibility fades (Main.cs:65111-65306)
// ---------------------------------------------------------------------------------------------------------------------

/// `Main.UpdateBGVisibility_FrontLayer_IsDesertVariantSwap(int targetBiome, int originalBiome)` (Main.cs:65144)
fn update_bg_visibility_front_layer_is_desert_variant_swap(ctx: &DrawContext, target_biome: i32, original_biome: i32) -> bool {
    use terraria_data::sets::surface_background_id::sets::IS_DESERT_VARIANT;
    if target_biome == original_biome {
        return false;
    }
    if !IS_DESERT_VARIANT[target_biome as usize] || !IS_DESERT_VARIANT[original_biome as usize] {
        return false;
    }
    let func = |bg: i32| -> bool {
        match bg {
            2 => ctx.g.main.desert_background_set.pure.has_any(),
            5 => ctx.g.main.desert_background_set.corrupt.has_any(),
            13 => ctx.g.main.desert_background_set.hallow.has_any(),
            14 => ctx.g.main.desert_background_set.crimson.has_any(),
            _ => false,
        }
    };
    if func(target_biome) {
        return func(original_biome);
    }
    false
}

/// `Main.UpdateBGVisibility_FrontLayer(int? targetBiomeOverride, float? transitionAmountOverride)` (Main.cs:65111)
pub fn update_bg_visibility_front_layer(ctx: &mut DrawContext, target_biome_override: Option<i32>, transition_amount_override: Option<f32>) {
    let mut value = ctx.g.main.bg_style;
    if let Some(v) = target_biome_override {
        value = v;
    }
    let mut value2 = ctx.g.main.background_layer_transition_speed;
    if let Some(v) = transition_amount_override {
        value2 = v;
    }
    for i in 0..ctx.g.main.bg_alpha_front_layer.len() {
        if value == i as i32 {
            ctx.g.main.bg_alpha_front_layer[i] += value2;
            if ctx.g.main.bg_alpha_front_layer[i] > 1.0 {
                ctx.g.main.bg_alpha_front_layer[i] = 1.0;
            }
        } else if ctx.g.main.bg_alpha_front_layer[i] != 0.0
            && (!update_bg_visibility_front_layer_is_desert_variant_swap(ctx, value, i as i32) || !(ctx.g.main.bg_alpha_front_layer[value as usize] < 1.0))
        {
            ctx.g.main.bg_alpha_front_layer[i] -= value2;
            if ctx.g.main.bg_alpha_front_layer[i] < 0.0 {
                ctx.g.main.bg_alpha_front_layer[i] = 0.0;
            }
        }
    }
}

/// `Main.UpdateBGVisibility_BackLayer(int? targetBiomeOverride, float? transitionAmountOverride)` (Main.cs:65177)
pub fn update_bg_visibility_back_layer(ctx: &mut DrawContext, target_biome_override: Option<i32>, transition_amount_override: Option<f32>) {
    let mut value = ctx.g.main.bg_style;
    if let Some(v) = target_biome_override {
        value = v;
    }
    let _ = ctx.g.main.background_layer_transition_speed;
    let _ = transition_amount_override;
    match value {
        2 => draw_bg_modify_bg_far_back_layer_alpha(ctx, 1, None, transition_amount_override),
        3 => {
            if ctx.g.world_gen.jungle_bg == 5 {
                draw_bg_modify_bg_far_back_layer_alpha(ctx, 15, None, transition_amount_override);
            } else {
                draw_bg_modify_bg_far_back_layer_alpha(ctx, 0, None, transition_amount_override);
            }
        }
        1 | 5 | 13 | 14 => draw_bg_modify_bg_far_back_layer_alpha(ctx, 2, None, transition_amount_override),
        6 => {
            if ctx.g.world_gen.hallow_bg == 3 {
                draw_bg_modify_bg_far_back_layer_alpha(ctx, 6, None, transition_amount_override);
            } else {
                draw_bg_modify_bg_far_back_layer_alpha(ctx, 2, None, transition_amount_override);
            }
        }
        4 => draw_bg_modify_bg_far_back_layer_alpha(ctx, 3, None, transition_amount_override),
        7 => draw_bg_modify_bg_far_back_layer_alpha(ctx, 4, None, transition_amount_override),
        8 => draw_bg_modify_bg_far_back_layer_alpha(ctx, 5, None, transition_amount_override),
        9 => draw_bg_modify_bg_far_back_layer_alpha(ctx, 0, Some(6), transition_amount_override),
        10 | 11 | 12 => draw_bg_modify_bg_far_back_layer_alpha(ctx, value, None, transition_amount_override),
        _ => draw_bg_modify_bg_far_back_layer_alpha(ctx, 0, None, transition_amount_override),
    }
}

/// `Main.DrawBG_ModifyBGFarBackLayerAlpha(int desiredBG, int? desiredBG2 = null, float? transitionAmountOverride = null)` (Main.cs:65290)
fn draw_bg_modify_bg_far_back_layer_alpha(ctx: &mut DrawContext, desired_bg: i32, desired_bg2: Option<i32>, transition_amount_override: Option<f32>) {
    let mut value = ctx.g.main.background_layer_transition_speed;
    if let Some(v) = transition_amount_override {
        value = v;
    }
    for i in 0..ctx.g.main.bg_alpha_far_back_layer.len() {
        let mut flag = desired_bg == i as i32;
        flag |= desired_bg2.is_some() && desired_bg2 == Some(i as i32);
        if flag
            || (ctx.g.main.bg_alpha_far_back_layer[i] != 0.0
                && (!update_bg_visibility_front_layer_is_desert_variant_swap(ctx, desired_bg, i as i32) || !(ctx.g.main.bg_alpha_far_back_layer[desired_bg as usize] < 1.0)))
        {
            ctx.g.main.bg_alpha_far_back_layer[i] =
                terraria_core::xna::math_helper::clamp(ctx.g.main.bg_alpha_far_back_layer[i] + (if flag { value } else { 0.0 - value }), 0.0, 1.0);
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// NextHorizonRenderer: gradients, sun flare, clouds
// ---------------------------------------------------------------------------------------------------------------------

const SUNRISE_TEXTURES: [&str; 4] = ["Images/Misc/Sunrise/Sunrise_Blue", "Images/Misc/Sunrise/Sunrise_Violet", "Images/Misc/Sunrise/Sunrise_Yellow", "Images/Misc/Sunrise/Sunrise_Aluminum"];
const SUNSET_TEXTURES: [&str; 4] = ["Images/Misc/Sunset/Sunset_Blue", "Images/Misc/Sunset/Sunset_Dark", "Images/Misc/Sunset/Sunset_Pink", "Images/Misc/Sunset/Sunset_Red"];

/// `NextHorizonRenderer.GetGradientRect()` (NextHorizonRenderer.cs)
fn horizon_get_gradient_rect(ctx: &DrawContext) -> Rectangle {
    let num = 400;
    // Utils.GetLerpValue(40.0, Main.worldSurface, Main.screenPosition.Y / 16f) (double overload, not clamped)
    let lerp = ((ctx.g.main.screen_position.1 / 16.0) as f64 - 40.0) / (ctx.g.main.world_surface - 40.0);
    let val = ((1.0 - lerp) * num as f64) as i32;
    let y = val.max(0) - num;
    Rectangle::new(0, y, ctx.g.main.screen_width, ctx.g.main.screen_height + num)
}

/// `BackgroundGradientDrawer` entries of `SunGradients.BackgroundDrawers` (SunGradients.cs): colour, weight layer, texture list, indexes to look for.
#[derive(Clone, Copy)]
enum GradientSource {
    TreeBgSet(usize),
    DesertPure,
    SnowBg,
    JungleBg,
    Ocean,
}

const BACKGROUND_DRAWERS: [((u8, u8, u8), usize, GradientSource, &[i32]); 13] = [
    ((116, 191, 255), 0, GradientSource::TreeBgSet(1), &[176]),
    ((157, 192, 255), 0, GradientSource::TreeBgSet(1), &[179]),
    ((116, 191, 255), 10, GradientSource::TreeBgSet(2), &[176]),
    ((157, 192, 255), 10, GradientSource::TreeBgSet(2), &[179]),
    ((116, 191, 255), 11, GradientSource::TreeBgSet(3), &[176]),
    ((157, 192, 255), 11, GradientSource::TreeBgSet(3), &[179]),
    ((116, 191, 255), 12, GradientSource::TreeBgSet(4), &[176]),
    ((157, 192, 255), 12, GradientSource::TreeBgSet(4), &[179]),
    ((184, 211, 245), 2, GradientSource::DesertPure, &[248]),
    ((169, 241, 255), 7, GradientSource::SnowBg, &[263, 268, 269]),
    ((220, 255, 109), 3, GradientSource::JungleBg, &[59]),
    ((141, 232, 131), 3, GradientSource::JungleBg, &[284]),
    ((157, 192, 255), 4, GradientSource::Ocean, &[283]),
];

/// `BackgroundGradientDrawer.Draw()` (BackgroundGradientDrawer.cs:29) for one drawer
fn background_gradient_draw(ctx: &mut DrawContext, color: (u8, u8, u8), layer: usize, source: GradientSource, indexes: &[i32]) {
    if !ctx.g.main.background_enabled {
        return;
    }
    let num = ctx.g.main.bg_alpha_front_layer[layer];
    if num <= 0.0 {
        return;
    }
    let textures: Vec<i32> = match source {
        GradientSource::TreeBgSet(1) => ctx.g.main.tree_bg_set1.to_vec(),
        GradientSource::TreeBgSet(2) => ctx.g.main.tree_bg_set2.to_vec(),
        GradientSource::TreeBgSet(3) => ctx.g.main.tree_bg_set3.to_vec(),
        GradientSource::TreeBgSet(_) => ctx.g.main.tree_bg_set4.to_vec(),
        GradientSource::DesertPure => ctx.g.main.desert_background_set.pure.backgrounds.to_vec(),
        GradientSource::SnowBg => ctx.g.main.snow_bg.to_vec(),
        GradientSource::JungleBg => ctx.g.main.jungle_bg.to_vec(),
        GradientSource::Ocean => vec![ctx.g.main.ocean_bg],
    };
    // ShouldDrawForTextures()
    let mut should = false;
    for n in indexes {
        for item in &textures {
            if n == item {
                should = true;
            }
        }
    }
    if !should || !terraria_sim::main_time::should_draw_surface_background(ctx.g) {
        return;
    }
    // BackgroundGradientDrawer.GetGradientRect()
    let num2 = 400;
    let y = (((ctx.g.main.world_surface * 16.0 - ctx.g.main.screen_position.1 as f64 - 2400.0) * 0.10000000149011612) as i32).max(0) - num2;
    let rect = Rectangle::new(0, y, ctx.g.main.screen_width, ctx.g.main.screen_height + num2);
    let base = ctx.g.main.color_of_surface_backgrounds_base;
    let tint = Color::new(color.0, color.1, color.2, 255).to_vector3() * base.to_vector3();
    let c = Color::from_vector3(tint) * num;
    if let Some(t) = ctx.texture("Images/Misc/Sunflare/colorgradientdither") {
        ctx.sprite_batch.draw_rect(&t, rect, None, c, 0.0, Vector2::ZERO, SpriteEffects::NONE);
    }
}

/// `NextHorizonRenderer.DrawHorizon()`
pub fn horizon_draw_horizon(ctx: &mut DrawContext) {
    if !terraria_sim::main_time::should_draw_surface_background(ctx.g) {
        return;
    }
    let sunrise_sunset_texture_index = horizon_phase(ctx.g) as usize;
    let (sunset_visibility, sunrise_visibility, _celestial) = get_visibilities(ctx.g);
    let gradient_rect = horizon_get_gradient_rect(ctx);
    for (color, layer, source, indexes) in BACKGROUND_DRAWERS {
        background_gradient_draw(ctx, color, layer, source, indexes);
    }
    if sunrise_visibility != 0.0 {
        if let Some(t) = ctx.texture(SUNRISE_TEXTURES[sunrise_sunset_texture_index % 4]) {
            ctx.sprite_batch.draw_rect(&t, gradient_rect, None, Color::WHITE * sunrise_visibility, 0.0, Vector2::ZERO, SpriteEffects::NONE);
        }
    }
    if sunset_visibility != 0.0 {
        if let Some(t) = ctx.texture(SUNSET_TEXTURES[sunrise_sunset_texture_index % 4]) {
            ctx.sprite_batch.draw_rect(&t, gradient_rect, None, Color::WHITE * sunset_visibility, 0.0, Vector2::ZERO, SpriteEffects::NONE);
        }
    }
}

/// `NextHorizonRenderer.DrawSurfaceLayer(int layerIndex)`: only layer 3 draws (the sunflare gradient, FlipHorizontally for sunrise).
/// The `switch` on `layerIndex` computes `num`, which layer 3 then overwrites with 1f: odd code as in the original.
#[allow(unused_assignments)]
pub fn horizon_draw_surface_layer(ctx: &mut DrawContext, layer_index: i32) {
    if terraria_sim::main_time::should_draw_surface_background(ctx.g) {
        let gradient_rect = horizon_get_gradient_rect(ctx);
        let (sunset_visibility, sunrise_visibility, _celestial) = get_visibilities(ctx.g);
        let idx = horizon_phase(ctx.g) as usize;
        let array = terraria_sim::main_time::SUNRISE_FIRST[idx % 4];
        let array2 = terraria_sim::main_time::SUNSET_FIRST[idx % 4];
        let mut color = Color::TRANSPARENT;
        terraria_sim::main_time::blend_color(&mut color, array2, sunset_visibility);
        terraria_sim::main_time::blend_color(&mut color, array, sunrise_visibility);
        let mut num = 1.0_f32;
        match layer_index {
            0 => num = 1.0,
            1 => num = 0.75,
            2 => num = 0.5,
            3 => num = 0.5,
            _ => {}
        }
        if layer_index == 3 {
            let num2 = 0.6_f32;
            num = 1.0;
            if let Some(t) = ctx.texture("Images/Misc/Sunflare/colorgradient") {
                ctx.sprite_batch.draw_rect(&t, gradient_rect, None, array * num * sunrise_visibility * num2, 0.0, Vector2::ZERO, SpriteEffects::FLIP_HORIZONTALLY);
                ctx.sprite_batch.draw_rect(&t, gradient_rect, None, array2 * num * sunset_visibility * num2, 0.0, Vector2::ZERO, SpriteEffects::NONE);
            }
        }
    }
}

/// `NextHorizonRenderer.DrawSun(Vector2 sunPosition)`
fn horizon_draw_sun(ctx: &mut DrawContext, sun_position: Vector2) {
    let (mut sunset_visibility, mut sunrise_visibility, celestial_visibility) = get_visibilities(ctx.g);
    sunset_visibility *= celestial_visibility;
    sunrise_visibility *= celestial_visibility;
    let color = Color::new(255, 255, 255, 0);
    let (Some(f1), Some(f2)) = (ctx.texture("Images/Misc/Sunflare/flare1"), ctx.texture("Images/Misc/Sunflare/flare2")) else { return };
    let o1 = Vector2::new(f1.width as f32, f1.height as f32) / 2.0;
    let o2 = Vector2::new(f2.width as f32, f2.height as f32) / 2.0;
    let sb = &mut *ctx.sprite_batch;
    sb.draw(&f1, sun_position, None, color * sunset_visibility * 0.75, 0.0, o1, Vector2::splat(3.0), SpriteEffects::NONE);
    sb.draw(&f1, sun_position, None, color * sunset_visibility * 0.35, 0.0, o1, Vector2::splat(2.0), SpriteEffects::NONE);
    sb.draw(&f2, sun_position, None, color * sunrise_visibility * 0.7 * 0.5, 0.0, o2, Vector2::splat(2.0), SpriteEffects::NONE);
    sb.draw(&f2, sun_position, None, color * sunrise_visibility * 0.3 * 0.5, 0.0, o2, Vector2::splat(1.5), SpriteEffects::NONE);
    sb.draw(&f2, sun_position, None, color * sunrise_visibility * 0.2 * 0.5, 0.0, o2, Vector2::splat(1.0), SpriteEffects::NONE);
}

/// `NextHorizonRenderer.DrawCloud(float globalCloudAlpha, Cloud theCloud, int cloudPass, float cY)` + `OriginalColorsForCloud`,
/// drawn straight away (see the module comment about `CloudsEnd`).
fn horizon_draw_cloud(ctx: &mut DrawContext, global_cloud_alpha: f32, cloud_index: usize, cloud_pass: i32, c_y: f32) {
    let cloud = ctx.g.main.cloud[cloud_index];
    let Some(asset) = ctx.texture(&format!("Images/Cloud_{}", cloud.type_)) else { return };
    let size = Vector2::new(asset.width as f32, asset.height as f32);
    let position = Vector2::new(cloud.position.x, c_y) + size / 2.0;
    let mut cloud_color = terraria_sim::cloud::cloud_color(&cloud, ctx.g.main.color_of_the_skies);
    // OriginalColorsForCloud
    if cloud_pass == 1 {
        let num = cloud.scale * 0.8;
        let num2 = (cloud.scale + 1.0) / 2.0 * 0.9;
        cloud_color.set_r((cloud_color.r() as i32 as f32 * num) as u8);
        cloud_color.set_g((cloud_color.g() as i32 as f32 * num2) as u8);
    }
    if ctx.g.main.atmo < 1.0 {
        cloud_color = cloud_color * ctx.g.main.atmo;
    }
    let effects = if cloud.sprite_dir == terraria_sim::cloud::SpriteDir::FlipHorizontally { SpriteEffects::FLIP_HORIZONTALLY } else { SpriteEffects::NONE };
    ctx.sprite_batch.draw(&asset, position, None, cloud_color * global_cloud_alpha, cloud.rotation, size / 2.0, Vector2::splat(cloud.scale), effects);
}

/// `Main.DrawClouds_Closest(float globalCloudAlpha)` (Main.cs:60634)
pub fn draw_clouds_closest(ctx: &mut DrawContext, global_cloud_alpha: f32) {
    let num = ctx.g.main.bg_top_y * 1.01 - 150.0;
    if !((ctx.g.main.screen_position.1 as f64) < ctx.g.main.world_surface * 16.0 + 16.0) {
        return;
    }
    unported!("HorizonRenderer.CloudsStart");
    for i in 0..200 {
        let cloud = ctx.g.main.cloud[i];
        if cloud.active && cloud.scale >= 1.15 {
            let num2 = cloud.position.y * (ctx.g.main.screen_height as f32 / 600.0) - 100.0;
            horizon_draw_cloud(ctx, global_cloud_alpha, i, 3, num2 + num);
        }
    }
    unported!("GameShaders.Misc[HorizonClouds] (HorizonRenderer.CloudsEnd)");
}

/// `Main.DrawClouds_Closer(float globalCloudAlpha)` (Main.cs:60654)
pub fn draw_clouds_closer(ctx: &mut DrawContext, global_cloud_alpha: f32) {
    let num = ctx.g.main.bg_top_y - 50.0;
    if !((ctx.g.main.screen_position.1 as f64) < ctx.g.main.world_surface * 16.0 + 16.0) {
        return;
    }
    unported!("HorizonRenderer.CloudsStart");
    for i in 0..200 {
        let cloud = ctx.g.main.cloud[i];
        if cloud.active && (cloud.scale as f64) < 1.15 && cloud.scale >= 1.0 {
            let num2 = cloud.position.y * (ctx.g.main.screen_height as f32 / 600.0);
            horizon_draw_cloud(ctx, global_cloud_alpha, i, 2, num2 + num + 200.0);
        }
    }
    unported!("GameShaders.Misc[HorizonClouds] (HorizonRenderer.CloudsEnd)");
}

/// `Main.DrawClouds_Distant(bool drawBackground, float backgroundTopMagicNumber, int pushBGTopHack, float globalCloudAlpha)` (Main.cs:60674)
pub fn draw_clouds_distant(ctx: &mut DrawContext, draw_background: bool, background_top_magic_number: f32, push_bg_top_hack: i32, global_cloud_alpha: f32) {
    if !draw_background {
        return;
    }
    let num = background_top_magic_number * 750.0 + 830.0 + ctx.g.main.sc_adj + push_bg_top_hack as f32;
    if !((ctx.g.main.screen_position.1 as f64) < ctx.g.main.world_surface * 16.0 + 16.0) {
        return;
    }
    unported!("HorizonRenderer.CloudsStart");
    for i in 0..200 {
        let cloud = ctx.g.main.cloud[i];
        if cloud.active && cloud.scale < 1.0 {
            let y = cloud.position.y;
            horizon_draw_cloud(ctx, global_cloud_alpha, i, 1, y + num);
        }
    }
    unported!("GameShaders.Misc[HorizonClouds] (HorizonRenderer.CloudsEnd)");
}

// ---------------------------------------------------------------------------------------------------------------------
// DoDraw: colour of the sky, the sky itself, sun, moon, stars
// ---------------------------------------------------------------------------------------------------------------------

/// `Main.DoDraw` Main.cs:62833-62981 (from `Cloud.UpdateCloudParallax()`): the cloud alpha follow-up, the mushroom light smoothing and
/// `SetBackColor`. Returns `(sunColor, moonColor, tempMushroomInfluence)`; the caller then runs `ApplyColorOfTheSkiesToTiles`.
///
/// `Main.UpdateSceneMetrics()` is called by the lighting engine in the original (`LightingEngine.cs:99`, `ExportMetrics` state; it is an
/// `unported!` in `lighting_engine.rs`), so it runs here once per frame, which is how often that state is reached in colour lighting.
pub fn do_draw_set_back_color(ctx: &mut DrawContext) -> (Color, Color, f32) {
    initialize_backgrounds(ctx);
    terraria_sim::cloud::update_cloud_parallax(ctx.g);
    update_scene_metrics(ctx);
    let mut num4 = ctx.g.main.day_rate;
    if num4 < 1 {
        num4 = 1;
    }
    let mut num5 = 0.0005_f32 * num4 as f32;
    if ctx.g.main.game_menu {
        num5 *= 20.0;
    }
    if ctx.g.main.raining {
        if ctx.g.main.cloud_alpha > ctx.g.main.max_raining {
            ctx.g.main.cloud_alpha -= num5;
            if ctx.g.main.cloud_alpha < ctx.g.main.max_raining {
                ctx.g.main.cloud_alpha = ctx.g.main.max_raining;
            }
        } else if ctx.g.main.cloud_alpha < ctx.g.main.max_raining {
            ctx.g.main.cloud_alpha += num5;
            if ctx.g.main.cloud_alpha > ctx.g.main.max_raining {
                ctx.g.main.cloud_alpha = ctx.g.main.max_raining;
            }
        }
    } else {
        ctx.g.main.cloud_alpha -= num5;
        if ctx.g.main.cloud_alpha < 0.0 {
            ctx.g.main.cloud_alpha = 0.0;
        }
    }
    if ctx.g.main.game_menu || ctx.g.main.net_mode == 2 {
        ctx.g.main.bg_delay = 1000;
        ctx.g.main.scene_metrics.evil_tile_count = (ctx.g.main.bg_alpha_front_layer[1] * terraria_sim::scene_metrics::CORRUPTION_TILE_MAX as f32) as i32;
    }
    let mut moon_color = Color::WHITE;
    let mut sun_color = Color::WHITE;
    let num6 = ctx.g.main.scene_metrics.mushroom_tile_count as f32 / terraria_sim::scene_metrics::MUSHROOM_TILE_MAX as f32;
    let mut num7 = ctx.g.main.smoothed_mushroom_light_influence;
    if num6 > 0.0 {
        if num6 > num7 {
            num7 += 0.01;
        }
        if num6 < num7 {
            num7 -= 0.01;
        }
    } else {
        num7 -= 0.02;
    }
    if num7 < 0.0 {
        num7 = 0.0;
    }
    if num7 > 1.0 {
        num7 = 1.0;
    }
    ctx.g.main.smoothed_mushroom_light_influence = num7;
    let sm = &ctx.g.main.scene_metrics;
    let info = terraria_sim::main_time::InfoToSetBackColor {
        is_in_game_menu_or_is_server: ctx.g.main.game_menu || ctx.g.main.net_mode == 2,
        corruption_biome_influence: sm.evil_tile_count as f32 / terraria_sim::scene_metrics::CORRUPTION_TILE_MAX as f32,
        crimson_biome_influence: sm.blood_tile_count as f32 / terraria_sim::scene_metrics::CRIMSON_TILE_MAX as f32,
        jungle_biome_influence: sm.jungle_tile_count as f32 / terraria_sim::scene_metrics::JUNGLE_TILE_MAX as f32,
        mushroom_biome_influence: ctx.g.main.smoothed_mushroom_light_influence,
        graveyard_influence: ctx.g.main.graveyard_visual_intensity,
        blood_moon_active: ctx.g.main.blood_moon || sm.blood_moon_monolith,
        lantern_night_active: unported!("LanternNight.LanternsUp", false),
    };
    terraria_sim::sky_gen::set_back_color(ctx.g, &info, &mut sun_color, &mut moon_color);
    (sun_color, moon_color, num7)
}

/// `Main.UpdateSceneMetrics()` (Main.cs:63522) for the local player. `LocalPlayer.Center` is the centre of the screen until `Player` is ported.
pub fn update_scene_metrics(ctx: &mut DrawContext) {
    let tracked_position = crate::pending::main_local_player_center(ctx.g);
    let mut area_to_light = crate::main_draw::get_area_to_light(ctx);
    area_to_light.inflate(28, 28);
    terraria_sim::scene_metrics::scan(ctx.g, tracked_position, Some(area_to_light));
}

/// `Main.UpdateAtmosphereTransparencyToSkyColor(screenPosition.Y + (float)(screenHeight / 2))` (Main.cs:62982)
pub fn update_atmosphere_transparency_to_sky_color(ctx: &mut DrawContext) {
    let y = ctx.g.main.screen_position.1 + (ctx.g.main.screen_height / 2) as f32;
    terraria_sim::sky_gen::update_atmosphere_transparency_to_sky_color(ctx.g, y);
}

/// `Main.DoDraw` Main.cs:62994-63047 (`DoDraw` has the back buffer cleared already): the parallax values of the sky quad, the sky quad, the
/// horizon gradients, then the stars, sun and moon. `BackgroundViewMatrix` is the identity at zoom 1.
pub fn do_draw_sky(ctx: &mut DrawContext, sun_color: Color, moon_color: Color, temp_mushroom_influence: f32) {
    ctx.g.main.bg_parallax = 0.1;
    let bw = ctx.g.main.background_width[ctx.g.main.background as usize];
    ctx.g.main.bg_start_x = (0.0 - terraria_sim::main_time::ieee_remainder(ctx.g.main.screen_position.0 as f64 * ctx.g.main.bg_parallax, bw as f64) - (bw / 2) as f64) as i32;
    ctx.g.main.bg_loops = ctx.g.main.screen_width / bw + 2;
    ctx.g.main.bg_start_y = 0;
    ctx.g.main.bg_loops_y = 0;
    ctx.g.main.bg_top_y = (0.0 - ctx.g.main.screen_position.1) / (ctx.g.main.world_surface as f32 * 16.0 - 600.0) * 200.0;
    if ctx.g.main.game_menu || ctx.g.main.net_mode == 2 {
        ctx.g.main.bg_top_y = 0.0;
    }
    let scene_area = SceneArea { bg_top_y: ctx.g.main.bg_top_y as i32, total_height: ctx.g.main.screen_height as f32, total_width: ctx.g.main.screen_width as f32 };
    if !ctx.g.main.map_fullscreen {
        if (ctx.g.main.screen_position.1 as f64) < ctx.g.main.world_surface * 16.0 + 16.0 {
            ctx.sprite_batch.begin(crate::gfx::BlendState::AlphaBlend, crate::gfx::SamplerState::PointClamp, crate::main_draw::transform());
            let background = ctx.g.main.background;
            let asset = ctx.texture(&format!("Images/Background_{background}"));
            if let Some(asset) = asset {
                let mut destination_rectangle = Rectangle::new(ctx.g.main.bg_start_x, ctx.g.main.bg_top_y as i32, asset.width, ctx.g.main.screen_height.max(asset.height));
                if destination_rectangle.y + destination_rectangle.height < asset.height {
                    let num10 = asset.height - (destination_rectangle.y + destination_rectangle.height);
                    destination_rectangle.height += num10;
                }
                for num12 in 0..ctx.g.main.bg_loops {
                    destination_rectangle.x = ctx.g.main.bg_start_x + asset.width * num12;
                    let c = ctx.g.main.color_of_the_skies;
                    ctx.sprite_batch.draw_rect(&asset, destination_rectangle, None, c, 0.0, Vector2::ZERO, SpriteEffects::NONE);
                }
            }
            horizon_draw_horizon(ctx);
            ctx.sprite_batch.end(ctx.list);
        }
        ctx.sprite_batch.begin(crate::gfx::BlendState::AlphaBlend, crate::gfx::SamplerState::PointClamp, crate::main_draw::transform());
        if ctx.state.shimmer_alpha != 1.0 {
            draw_stars_in_background(ctx, scene_area, false);
        }
        if ((ctx.g.main.screen_position.1 / 16.0) as f64) < ctx.g.main.world_surface + 2.0 {
            draw_sun_and_moon(ctx, scene_area, moon_color, sun_color, temp_mushroom_influence);
        }
        ctx.sprite_batch.end(ctx.list);
    }
}

/// `SceneArea` as `DoDraw` builds it (Main.cs:63004-63011), for the second `DrawStarsInBackground` call in the shimmer path
pub fn scene_area(ctx: &DrawContext) -> SceneArea {
    let mut bg_top_y = (0.0 - ctx.g.main.screen_position.1) / (ctx.g.main.world_surface as f32 * 16.0 - 600.0) * 200.0;
    if ctx.g.main.game_menu || ctx.g.main.net_mode == 2 {
        bg_top_y = 0.0;
    }
    SceneArea { bg_top_y: bg_top_y as i32, total_height: ctx.g.main.screen_height as f32, total_width: ctx.g.main.screen_width as f32 }
}

/// `Main.DrawBG(SpriteBatchBeginner)` wrapper: the batch is begun by the caller (`parentSpriteBatchBeginner.Begin(spriteBatch)`)
pub fn draw_bg_call(ctx: &mut DrawContext) {
    initialize_backgrounds(ctx);
    draw_bg(ctx);
}

/// `Main.DrawSunAndMoon(SceneArea sceneArea, Color moonColor, Color sunColor, float tempMushroomInfluence)` (Main.cs:63857)
pub fn draw_sun_and_moon(ctx: &mut DrawContext, scene_area: SceneArea, mut moon_color: Color, sun_color: Color, temp_mushroom_influence: f32) {
    let Some(sun) = ctx.texture("Images/Sun") else { return };
    let mut num = ctx.g.main.moon_type;
    if !(0..9).contains(&num) {
        num = num.clamp(0, 8);
    }
    let Some(moon) = ctx.texture(&format!("Images/Moon_{num}")) else { return };
    let mut value = sun.clone();
    let time = ctx.g.main.time;
    let num2 = scene_area.bg_top_y;
    let num3 = (time / 54000.0 * (scene_area.total_width + (value.width * 2) as f32) as f64) as i32 - value.width;
    let mut num4 = 0;
    let mut num5 = 1.0_f32;
    let rotation = (time / 54000.0) as f32 * 2.0 - 7.3;
    let num6 = (time / 32400.0 * (scene_area.total_width + (moon.width * 2) as f32) as f64) as i32 - moon.width;
    let mut num7 = 0;
    let mut num8 = 1.0_f32;
    let num9 = (time / 32400.0) as f32 * 2.0 - 7.3;
    if ctx.g.main.day_time {
        let num10 = (time / 54000.0 * 2.0 - 1.0).abs().powf(2.0);
        num4 = (num2 as f64 + num10 * 250.0 + 180.0) as i32;
        num5 = (1.2 - num10 * 0.4) as f32;
    } else {
        let num11 = (time / 32400.0 * 2.0 - 1.0).abs().powf(2.0);
        num7 = (num2 as f64 + num11 * 250.0 + 180.0) as i32;
        num8 = (1.2 - num11 * 0.4) as f32;
    }
    if unported!("Main.starGame", false) {
        unported!("Main.DrawSunAndMoon starGame (title screen star mini game)");
    } else {
        unported!("Main.starsHit = 0");
    }
    if ctx.g.main.day_time {
        if (ctx.g.main.remix_world && !ctx.g.main.game_menu) || ctx.g.world_gen.remix_world_gen {
            return;
        }
        num5 *= 1.1;
        let mut num12 = 1.0_f32 - temp_mushroom_influence;
        num12 -= ctx.g.main.cloud_alpha * 1.5 * ctx.g.main.atmo;
        if num12 < 0.0 {
            num12 = 0.0;
        }
        let mut color = Color::new((255.0 * num12) as u8, (sun_color.g() as i32 as f32 * num12) as u8, (sun_color.b() as i32 as f32 * num12) as u8, (255.0 * num12) as u8);
        let mut color2 = Color::new(
            (sun_color.r() as i32 as f32 * num12) as u8,
            (sun_color.g() as i32 as f32 * num12) as u8,
            (sun_color.b() as i32 as f32 * num12) as u8,
            (sun_color.b() as i32 as f32 * num12) as u8,
        );
        if !ctx.g.main.eclipse {
            color = Color::WHITE;
        }
        let mut flag = false;
        let mut flag2 = false;
        if ctx.g.main.eclipse {
            if let Some(t) = ctx.texture("Images/Sun3") {
                value = t;
            }
            flag = true;
        } else if !ctx.g.main.game_menu && unported!("Main.player[myPlayer].head == 12", false) {
            if let Some(t) = ctx.texture("Images/Sun2") {
                value = t;
            }
            flag2 = true;
            flag = true;
        }
        if flag {
            color2 = Color::new(
                (sun_color.r() as i32 as f32 * num12) as u8,
                (sun_color.g() as i32 as f32 * num12) as u8,
                (sun_color.b() as i32 as f32 * num12) as u8,
                ((sun_color.b() as i32 - 60) as f32 * num12) as u8,
            );
        }
        let origin = Vector2::new(value.width as f32, value.height as f32) / 2.0;
        let vector = Vector2::new(num3 as f32, (num4 + ctx.g.main.sun_mod_y as i32) as f32);
        ctx.sprite_batch.draw(&value, vector, None, color, rotation, origin, Vector2::splat(num5), SpriteEffects::NONE);
        ctx.sprite_batch.draw(&value, vector, None, color2, rotation, origin, Vector2::splat(num5), SpriteEffects::NONE);
        horizon_draw_sun(ctx, vector);
        if flag2 {
            if let Some(value3) = ctx.texture("Images/Extra_280") {
                let color3 = Color::new(color.r(), color.g(), color.b(), 100);
                ctx.sprite_batch.draw(&value3, vector, None, color3, rotation, origin, Vector2::splat(num5), SpriteEffects::NONE);
            }
        }
    }
    if !ctx.g.main.day_time {
        let mut num13 = 1.0_f32 - ctx.g.main.cloud_alpha * 1.5 * ctx.g.main.atmo;
        if num13 < 0.0 {
            num13 = 0.0;
        }
        moon_color = moon_color * num13;
        let position = Vector2::new(num6 as f32, (num7 + ctx.g.main.moon_mod_y as i32) as f32);
        if ctx.g.world_gen.drunk_world_gen {
            if let Some(t) = ctx.texture("Images/Moon_Smiley") {
                ctx.sprite_batch.draw(&t, position, Some(Rectangle::new(0, 0, t.width, t.height)), moon_color, num9 / 2.0 + std::f32::consts::PI, Vector2::new((t.width / 2) as f32, (t.height / 2) as f32), Vector2::splat(num8), SpriteEffects::NONE);
            }
        } else if ctx.g.main.pumpkin_moon {
            if let Some(t) = ctx.texture("Images/Moon_Pumpkin") {
                ctx.sprite_batch.draw(&t, position, Some(Rectangle::new(0, t.width * ctx.g.main.moon_phase, t.width, t.width)), moon_color, num9, Vector2::new((t.width / 2) as f32, (t.width / 2) as f32), Vector2::splat(num8), SpriteEffects::NONE);
            }
        } else if ctx.g.main.snow_moon {
            if let Some(t) = ctx.texture("Images/Moon_Snow") {
                ctx.sprite_batch.draw(&t, position, Some(Rectangle::new(0, t.width * ctx.g.main.moon_phase, t.width, t.width)), moon_color, num9, Vector2::new((t.width / 2) as f32, (t.width / 2) as f32), Vector2::splat(num8), SpriteEffects::NONE);
            }
        } else {
            ctx.sprite_batch.draw(&moon, position, Some(Rectangle::new(0, moon.width * ctx.g.main.moon_phase, moon.width, moon.width)), moon_color, num9, Vector2::new((moon.width / 2) as f32, (moon.width / 2) as f32), Vector2::splat(num8), SpriteEffects::NONE);
        }
    }
    // value4 (the grab rectangle) is only read by the title-screen sun / moon dragging
    ctx.g.main.sun_mod_y = (ctx.g.main.sun_mod_y as f64 * 0.999) as i16;
    ctx.g.main.moon_mod_y = (ctx.g.main.moon_mod_y as f64 * 0.999) as i16;
    if ctx.g.main.game_menu && ctx.g.main.net_mode != 1 {
        unported!("Main.DrawSunAndMoon title-screen sun / moon dragging (mouse)");
    }
    ctx.g.main.last_celestial_body_position = if ctx.g.main.day_time {
        Vector2::new(num3 as f32, (num4 + ctx.g.main.sun_mod_y as i32) as f32)
    } else {
        Vector2::new(num6 as f32, (num7 + ctx.g.main.moon_mod_y as i32) as f32)
    };
    ctx.g.main.last_celestial_body_position = ctx.g.main.last_celestial_body_position / Vector2::new(ctx.g.main.screen_width as f32, ctx.g.main.screen_height as f32);
}

/// `Main.DrawStarsInBackground(SceneArea sceneArea, bool artificial)` (Main.cs:64040)
pub fn draw_stars_in_background(ctx: &mut DrawContext, scene_area: SceneArea, artificial: bool) {
    if ctx.g.main.net_mode == 2 {
        return;
    }
    let mut num = 1.0_f32;
    if ctx.g.main.graveyard_visual_intensity > 0.0 {
        let num2 = 1.0 - ctx.g.main.graveyard_visual_intensity * 1.4;
        if num2 <= 0.0 {
            return;
        }
        num *= num2;
    }
    let mut color_of_the_skies = ctx.g.main.color_of_the_skies;
    if ctx.state.shimmer_alpha > 0.0 {
        color_of_the_skies = color_of_the_skies * (1.0 - ctx.state.shimmer_alpha);
    }
    if ctx.g.main.remix_world || !(255.0 * (1.0 - ctx.g.main.cloud_alpha * ctx.g.main.atmo) - color_of_the_skies.r() as i32 as f32 - 25.0 <= 0.0) {
        for i in 0..ctx.g.main.num_stars as usize {
            draw_star(ctx, scene_area, num, color_of_the_skies, i, artificial);
        }
    }
}

/// `Main.DrawStar(ref SceneArea sceneArea, float starOpacity, Color bgColorForStars, int i, Star theStar, bool artificial)` (Main.cs:64070)
fn draw_star(ctx: &mut DrawContext, scene_area: SceneArea, star_opacity: f32, bg_color_for_stars: Color, i: usize, artificial: bool) {
    let the_star = ctx.g.main.star[i];
    if the_star.hidden {
        return;
    }
    let num = ctx.state.shimmer_alpha;
    let mut color = Color::default();
    let num2 = 1.0 - the_star.fade_in;
    let mut num3 = ((255 - bg_color_for_stars.r() as i32 - 100) as f32 * the_star.twinkle * num2) as i32;
    let num4 = ((255 - bg_color_for_stars.g() as i32 - 100) as f32 * the_star.twinkle * num2) as i32;
    let num5 = ((255 - bg_color_for_stars.b() as i32 - 100) as f32 * the_star.twinkle * num2) as i32;
    num3 = (num3 + num5 + num4) / 3;
    if num3 <= 0 {
        return;
    }
    num3 = (num3 as f64 * 1.4) as i32;
    if num3 > 255 {
        num3 = 255;
    }
    let num4 = num3;
    let num5 = num3;
    color.set_r(num3 as u8);
    color.set_g(num4 as u8);
    color.set_b(num5 as u8);
    color = color * star_opacity;
    if num > 0.0 {
        let mut value = color;
        if i % 3 == 0 {
            value.set_g((color.g() as i32 as f32 * (1.0 - num * 0.0) * the_star.twinkle * the_star.twinkle) as u8);
        } else if i % 3 == 1 {
            value.set_b((color.b() as i32 as f32 * (1.0 - num * 0.0) * the_star.twinkle * the_star.twinkle) as u8);
        } else if i % 3 == 2 {
            value.set_r((color.r() as i32 as f32 * (1.0 - num * 0.0) * the_star.twinkle * the_star.twinkle) as u8);
        }
        color = Color::lerp(color, value, num);
        if artificial {
            color = color * num;
        } else {
            color = color * (1.0 - num);
        }
    }
    let vector = Vector2::new(the_star.position.x / 1920.0, the_star.position.y / 1200.0);
    let mut num6 = scene_area.bg_top_y;
    if ctx.g.main.world_surface <= 30.0 {
        num6 = 0;
    }
    let mut vector2 = vector * Vector2::new(scene_area.total_width, scene_area.total_height) + Vector2::new(0.0, num6 as f32);
    if !the_star.falling {
        if vector2.x < 0.0 {
            vector2.x += scene_area.total_width;
        }
        if vector2.x > scene_area.total_width {
            vector2.x -= scene_area.total_width;
        }
        if vector2.y < 0.0 {
            vector2.y += scene_area.total_height;
        }
        if vector2.y > scene_area.total_height {
            vector2.y -= scene_area.total_height;
        }
    }
    let Some(value2) = ctx.texture(&format!("Images/Star_{}", the_star.type_)) else { return };
    let origin = Vector2::new(value2.width as f32, value2.height as f32) / 2.0;
    if the_star.falling {
        ctx.g.main.star[i].fade_in = 0.0;
        let mut num7 = the_star.fall_time;
        let num8 = 30.0_f32;
        if num7 as f32 > num8 {
            num7 = num8 as i32;
        }
        for j in 1..num7 {
            let vector3 = the_star.fall_speed * j as f32 * 0.4;
            let num9 = the_star.scale * (1.0 - j as f32 * 1.0 / num8);
            let mut color2 = color;
            color2 = color2 * (1.0 - j as f32 * 1.0 / num8);
            ctx.sprite_batch.draw(&value2, vector2 - vector3, None, color2, the_star.rotation, origin, Vector2::splat(num9 * the_star.twinkle), SpriteEffects::NONE);
        }
        unported!("Main.starGame star hit test");
    }
    ctx.sprite_batch.draw(&value2, vector2, None, color, the_star.rotation, origin, Vector2::splat(the_star.scale * the_star.twinkle), SpriteEffects::NONE);
}

/// `Main.DoDraw` Main.cs:63218-63226: the magma background animation frame
pub fn step_magma_bg_frame(ctx: &mut DrawContext) {
    ctx.g.main.magma_bg_frame_counter += 1;
    if ctx.g.main.magma_bg_frame_counter >= 8 {
        ctx.g.main.magma_bg_frame_counter = 0;
        ctx.g.main.magma_bg_frame += 1;
        if ctx.g.main.magma_bg_frame >= 3 {
            ctx.g.main.magma_bg_frame = 0;
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// Main.DoUpdate
// ---------------------------------------------------------------------------------------------------------------------

/// `Main.DoUpdate_AnimateBackgrounds()` (Main.cs:19866)
pub fn do_update_animate_backgrounds(ctx: &mut DrawContext) {
    terraria_sim::main_time::do_update_animate_backgrounds(ctx.g);
}

/// `Main.DoUpdate` Main.cs:17429-17488: the background flash, the weather and the star / cloud updates
pub fn do_update_sky(ctx: &mut DrawContext) {
    let g = &mut *ctx.g;
    g.world_gen.backgrounds_cache.update_flash_values();
    unported!("Rain.MakeRain");
    if g.main.net_mode != 1 {
        unported!("Main.updateCloudLayer");
    }
    for _i in 0..g.main.day_rate {
        unported!("Main.UpdateWeather");
    }
    if g.main.net_mode != 2 {
        if (g.main.screen_position.1 as f64) < g.main.world_surface * 16.0 + 16.0 {
            terraria_sim::star::update_stars(g);
            terraria_sim::cloud::update_clouds(g);
        } else if ctx.state.shimmer_alpha > 0.0 {
            terraria_sim::star::update_stars(g);
            unported!("Main.DoUpdate shimmer falling stars / Star.SpawnStars(j)");
        }
    }
}

/// `Main.DoUpdateInWorld_Inner` Main.cs:17962 (`_gameUpdateCount++`) and `UpdateWorld_Time` (Main.cs:18299)
pub fn do_update_world_time(ctx: &mut DrawContext) {
    ctx.g.main.game_update_count = ctx.g.main.game_update_count.wrapping_add(1);
    terraria_sim::main_time::update_time(ctx.g);
}

/// The `Images/Cloud_<n>` sizes (`TextureAssets.Cloud[n].Width()/Height()`) read from the files, for `Cloud.addCloud` / `Cloud.Update`.
pub fn read_cloud_texture_sizes(content_root: &std::path::Path) -> Vec<(i32, i32)> {
    (0..42)
        .map(|n| {
            let path = content_root.join(format!("Images/Cloud_{n}.xnb"));
            match std::fs::read(&path).ok().and_then(|b| terraria_formats::xnb::read_xnb(&b).ok()) {
                Some(f) => match f.asset {
                    terraria_formats::xnb::Asset::Texture2D(t) => (t.width as i32, t.height as i32),
                    _ => (0, 0),
                },
                None => (0, 0),
            }
        })
        .collect()
}

#[cfg(test)]
#[path = "main_backgrounds_tests.rs"]
mod tests;
