//! `Terraria.WaterfallManager` (WaterfallManager.cs): finds the waterfalls (and lavafalls, honeyfalls, shimmerfalls, rain / snow / lava-rain
//! columns of the special tiles) around the view every 30 frames, steps their animation frames, and draws each column through `Main.tileBatch`.
//!
//! Not ported, as `unported!` at their original place: `Dust.NewDust` (the sparkles of `TrySparkling`), `BindTo` / `Configuration_OnLoad`
//! (the `WaterfallDrawLimit` setting: the default 1000 is used) and `LoadContent` (the textures are looked up where they are drawn).

use terraria_core::xna::{Color, Rectangle, Vector2};
use terraria_core::Tile;
use terraria_data::sets::tile_id::sets as tile_sets;
use terraria_sim::lighting::add_light;
use terraria_sim::unported;
use terraria_sim::world_gen::tile_frame::in_world;
use terraria_sim::world_gen::tile_queries::solid_tile_tile;
use terraria_sim::Game;

use crate::draw_context::DrawContext;
use crate::game_content::liquid::liquid_renderer::{set_shimmer_vertex_colors, set_shimmer_vertex_colors_sparkle};
use crate::gfx::{SpriteEffects, Texture2D};
use crate::pending::{lighting_get_color, lighting_get_corner_colors, VertexColors};

/// `WaterfallManager.minWet` (WaterfallManager.cs:24); nothing reads it
#[allow(dead_code)]
const MIN_WET: i32 = 160;
/// `WaterfallManager.maxWaterfallCountDefault`
const MAX_WATERFALL_COUNT_DEFAULT: i32 = 1000;
/// `WaterfallManager.maxLength` (WaterfallManager.cs:28); nothing reads it
#[allow(dead_code)]
const MAX_LENGTH: i32 = 100;
/// `WaterfallManager.maxTypes`
const MAX_TYPES: usize = 28;
/// `WaterfallManager.Layer_Rain`, `Layer_Waterfall`
const LAYER_RAIN: u32 = 0;
const LAYER_WATERFALL: u32 = 1;

/// `WaterfallManager.WaterfallData`
#[derive(Clone, Copy, Default)]
struct WaterfallData {
    x: i32,
    y: i32,
    type_: i32,
    stop_at_step: i32,
}

/// `Main.waterfallManager`
pub struct WaterfallManager {
    /// `maxWaterfallCount`
    max_waterfall_count: i32,
    quality_max: i32,
    current_max: i32,
    waterfalls: Vec<WaterfallData>,
    w_fall_fr_counter: i32,
    regular_frame: i32,
    w_fall_fr_counter2: i32,
    slow_frame: i32,
    rain_frame_counter: i32,
    rain_frame_foreground: i32,
    rain_frame_background: i32,
    lava_rain_frame_counter: i32,
    lava_rain_frame_foreground: i32,
    lava_rain_frame_background: i32,
    snow_frame_counter: i32,
    snow_frame_foreground: i32,
    find_waterfall_count: i32,
    waterfall_dist: i32,
    /// `_shouldShowInvisibleBlocksAndWalls` (static)
    should_show_invisible_blocks_and_walls: bool,
}

impl Default for WaterfallManager {
    fn default() -> Self {
        Self::new()
    }
}

impl WaterfallManager {
    /// `new WaterfallManager()` (field initialisers, WaterfallManager.cs:32-68)
    pub fn new() -> Self {
        Self {
            max_waterfall_count: MAX_WATERFALL_COUNT_DEFAULT,
            quality_max: 0,
            current_max: 0,
            waterfalls: vec![WaterfallData::default(); MAX_WATERFALL_COUNT_DEFAULT as usize],
            w_fall_fr_counter: 0,
            regular_frame: 0,
            w_fall_fr_counter2: 0,
            slow_frame: 0,
            rain_frame_counter: 0,
            rain_frame_foreground: 0,
            rain_frame_background: 0,
            lava_rain_frame_counter: 0,
            lava_rain_frame_foreground: 0,
            lava_rain_frame_background: 0,
            snow_frame_counter: 0,
            snow_frame_foreground: 0,
            find_waterfall_count: 0,
            waterfall_dist: 100,
            should_show_invisible_blocks_and_walls: false,
        }
    }

    /// `CheckForWaterfall(int i, int j)` (WaterfallManager.cs:95)
    pub fn check_for_waterfall(&self, i: i32, j: i32) -> bool {
        for k in 0..self.current_max as usize {
            if self.waterfalls[k].x == i && self.waterfalls[k].y == j {
                return true;
            }
        }
        false
    }

    /// `FindWaterfalls(bool forced = false)` (WaterfallManager.cs:107)
    pub fn find_waterfalls(&mut self, g: &Game, gfx_quality: f32, forced: bool) {
        self.find_waterfall_count += 1;
        if self.find_waterfall_count < 30 && !forced {
            return;
        }
        self.find_waterfall_count = 0;
        self.waterfall_dist = (75.0 * gfx_quality) as i32 + 25;
        self.quality_max = (self.max_waterfall_count as f32 * gfx_quality) as i32;
        self.current_max = 0;
        let mut num = (g.main.screen_position.0 / 16.0 - 1.0) as i32;
        let mut num2 = ((g.main.screen_position.0 + g.main.screen_width as f32) / 16.0) as i32 + 2;
        let mut num3 = (g.main.screen_position.1 / 16.0 - 1.0) as i32;
        let mut num4 = ((g.main.screen_position.1 + g.main.screen_height as f32) / 16.0) as i32 + 2;
        num -= self.waterfall_dist;
        num2 += self.waterfall_dist;
        num3 -= self.waterfall_dist;
        num4 += 20;
        if num <= 0 {
            num = 1;
        }
        if num2 >= g.main.max_tiles_x {
            num2 = g.main.max_tiles_x - 1;
        }
        if num3 <= 0 {
            num3 = 1;
        }
        if num4 >= g.main.max_tiles_y {
            num4 = g.main.max_tiles_y - 1;
        }
        // `Main.tile[x, y] == null` creates a blank tile in the original; every coordinate read here is inside the world, so it never triggers
        for i in num..num2 {
            for j in num3..num4 {
                let tile = *g.main.tile.get(i, j);
                if !tile.active() {
                    continue;
                }
                if tile.half_brick() {
                    let tile2 = *g.main.tile.get(i, j - 1);
                    if tile2.liquid < 16 || solid_tile_tile(g, &tile2) {
                        let tile3 = *g.main.tile.get(i - 1, j);
                        let tile4 = *g.main.tile.get(i + 1, j);
                        if (tile3.liquid > 160 || tile4.liquid > 160)
                            && ((tile3.liquid == 0 && !solid_tile_tile(g, &tile3) && tile3.slope() == 0) || (tile4.liquid == 0 && !solid_tile_tile(g, &tile4) && tile4.slope() == 0))
                            && self.current_max < self.quality_max
                        {
                            let cm = self.current_max as usize;
                            self.waterfalls[cm].type_ = 0;
                            if tile2.lava() || tile4.lava() || tile3.lava() {
                                self.waterfalls[cm].type_ = 1;
                            } else if tile2.honey() || tile4.honey() || tile3.honey() {
                                self.waterfalls[cm].type_ = 14;
                            } else if tile2.shimmer() || tile4.shimmer() || tile3.shimmer() {
                                self.waterfalls[cm].type_ = 25;
                            } else {
                                self.waterfalls[cm].type_ = 0;
                            }
                            self.waterfalls[cm].x = i;
                            self.waterfalls[cm].y = j;
                            self.current_max += 1;
                        }
                    }
                }
                if tile.type_ == 196 {
                    let tile5 = *g.main.tile.get(i, j + 1);
                    if !solid_tile_tile(g, &tile5) && tile5.liquid == 0 && tile5.slope() == 0 && self.current_max < self.quality_max {
                        let cm = self.current_max as usize;
                        self.waterfalls[cm].type_ = 11;
                        self.waterfalls[cm].x = i;
                        self.waterfalls[cm].y = j + 1;
                        self.current_max += 1;
                    }
                }
                if tile.type_ == 460 {
                    let tile6 = *g.main.tile.get(i, j + 1);
                    if !solid_tile_tile(g, &tile6) && tile6.liquid == 0 && tile6.slope() == 0 && self.current_max < self.quality_max {
                        let cm = self.current_max as usize;
                        self.waterfalls[cm].type_ = 22;
                        self.waterfalls[cm].x = i;
                        self.waterfalls[cm].y = j + 1;
                        self.current_max += 1;
                    }
                }
                if tile.type_ == 717 {
                    let tile7 = *g.main.tile.get(i, j + 1);
                    if !solid_tile_tile(g, &tile7) && tile7.liquid == 0 && tile7.slope() == 0 && self.current_max < self.quality_max {
                        let cm = self.current_max as usize;
                        self.waterfalls[cm].type_ = 26;
                        self.waterfalls[cm].x = i;
                        self.waterfalls[cm].y = j + 1;
                        self.current_max += 1;
                    }
                }
            }
        }
    }

    /// `UpdateFrame()` (WaterfallManager.cs:257)
    pub fn update_frame(&mut self) {
        self.w_fall_fr_counter += 1;
        if self.w_fall_fr_counter > 2 {
            self.w_fall_fr_counter = 0;
            self.regular_frame += 1;
            if self.regular_frame > 15 {
                self.regular_frame = 0;
            }
        }
        self.w_fall_fr_counter2 += 1;
        if self.w_fall_fr_counter2 > 6 {
            self.w_fall_fr_counter2 = 0;
            self.slow_frame += 1;
            if self.slow_frame > 15 {
                self.slow_frame = 0;
            }
        }
        self.rain_frame_counter += 1;
        if self.rain_frame_counter > 0 {
            self.rain_frame_foreground += 1;
            if self.rain_frame_foreground > 7 {
                self.rain_frame_foreground -= 8;
            }
            if self.rain_frame_counter > 2 {
                self.rain_frame_counter = 0;
                self.rain_frame_background -= 1;
                if self.rain_frame_background < 0 {
                    self.rain_frame_background = 7;
                }
            }
        }
        self.lava_rain_frame_counter += 1;
        if self.lava_rain_frame_counter == 1 || self.lava_rain_frame_counter == 3 {
            self.lava_rain_frame_foreground += 1;
            if self.lava_rain_frame_foreground > 7 {
                self.lava_rain_frame_foreground -= 8;
            }
        } else if self.lava_rain_frame_counter > 3 {
            self.lava_rain_frame_counter = 0;
            self.lava_rain_frame_background -= 1;
            if self.lava_rain_frame_background < 0 {
                self.lava_rain_frame_background = 7;
            }
        }
        self.snow_frame_counter += 1;
        if self.snow_frame_counter > 3 {
            self.snow_frame_counter = 0;
            self.snow_frame_foreground += 1;
            if self.snow_frame_foreground > 7 {
                self.snow_frame_foreground = 0;
            }
        }
    }

    /// `DrawWaterfall(int Style = 0, float Alpha = 1f)` (WaterfallManager.cs:325)
    // the locals keep the decompiler's initialisers (`int num32 = 0;`) even where every path assigns them again
    #[allow(unused_assignments)]
    fn draw_waterfall_style(&mut self, ctx: &mut DrawContext, textures: &[Option<Texture2D>], style: i32, alpha: f32) {
        ctx.g.main.tile_solid[546] = false;
        let mut num = 0.0_f32;
        let mut num2 = 99999.0_f32;
        let mut num3 = 99999.0_f32;
        let mut num4 = -1;
        let mut num5 = -1;
        let mut num6 = 0.0_f32;
        let mut num7 = 99999.0_f32;
        let mut num8 = 99999.0_f32;
        let mut num9 = -1;
        let mut num10 = -1;
        let screen_position = Vector2::new(ctx.g.main.screen_position.0, ctx.g.main.screen_position.1);
        let screen_width = ctx.g.main.screen_width;
        let screen_height = ctx.g.main.screen_height;
        // `Main.tile[x, y]` as a copy; outside the world the original has `null` (a fresh tile is stored) or throws; a blank tile is the neutral answer
        let tile_at = |g: &Game, x: i32, y: i32| -> Tile { g.main.tile.try_get(x, y).unwrap_or_default() };
        'waterfalls: for i in 0..self.current_max as usize {
            let mut num11 = 0;
            let mut num12 = self.waterfalls[i].type_;
            let mut num13 = self.waterfalls[i].x;
            let mut num14 = self.waterfalls[i].y;
            let mut num15 = 0;
            let mut num16 = 0;
            let mut num17 = 0;
            let mut num18 = 0;
            let mut num19 = 0;
            let mut num20 = 0;
            let num21;
            let mut num22;
            if num12 == 1 || num12 == 14 || num12 == 25 {
                if ctx.state.liquid.drew_lava || self.waterfalls[i].stop_at_step == 0 {
                    continue;
                }
                num21 = 32 * self.slow_frame;
            } else {
                match num12 {
                    11 | 22 | 26 => {
                        if ctx.state.liquid.drew_lava {
                            continue;
                        }
                        num22 = self.waterfall_dist / 4;
                        if num12 == 22 {
                            num22 = self.waterfall_dist / 2;
                        }
                        if self.waterfalls[i].stop_at_step > num22 {
                            self.waterfalls[i].stop_at_step = num22;
                        }
                        if self.waterfalls[i].stop_at_step == 0
                            || ((num14 + num22) as f32) < screen_position.y / 16.0
                            || (num13 as f32) < screen_position.x / 16.0 - 20.0
                            || (num13 as f32) > (screen_position.x + screen_width as f32) / 16.0 + 20.0
                        {
                            continue;
                        }
                        let mut num23 = 0;
                        let mut num24 = 0;
                        if num13 % 2 == 0 {
                            match num12 {
                                22 => {
                                    num23 = self.snow_frame_foreground + 3;
                                    if num23 > 7 {
                                        num23 -= 8;
                                    }
                                }
                                26 => {
                                    num23 = self.lava_rain_frame_foreground + 3;
                                    if num23 > 7 {
                                        num23 -= 8;
                                    }
                                    num24 = self.lava_rain_frame_background + 2;
                                    if num24 > 7 {
                                        num24 -= 8;
                                    }
                                }
                                _ => {
                                    num23 = self.rain_frame_foreground + 3;
                                    if num23 > 7 {
                                        num23 -= 8;
                                    }
                                    num24 = self.rain_frame_background + 2;
                                    if num24 > 7 {
                                        num24 -= 8;
                                    }
                                }
                            }
                        } else {
                            match num12 {
                                22 => num23 = self.snow_frame_foreground,
                                26 => {
                                    num23 = self.lava_rain_frame_foreground;
                                    num24 = self.lava_rain_frame_background;
                                }
                                _ => {
                                    num23 = self.rain_frame_foreground;
                                    num24 = self.rain_frame_background;
                                }
                            }
                        }
                        let mut source_rectangle = Rectangle::new(num24 * 18, 0, 16, 16);
                        let mut source_rectangle2 = Rectangle::new(num23 * 18, 0, 16, 16);
                        let origin = Vector2::new(8.0, 8.0);
                        let mut position = if num14 % 2 != 0 {
                            Vector2::new((num13 * 16 + 8) as f32, (num14 * 16 + 8) as f32) - screen_position
                        } else {
                            Vector2::new((num13 * 16 + 9) as f32, (num14 * 16 + 8) as f32) - screen_position
                        };
                        if !in_world(ctx.g, num13, num14 - 1, 0) {
                            continue;
                        }
                        let tile = tile_at(ctx.g, num13, num14 - 1);
                        if tile.active() && tile.bottom_slope() {
                            position.y -= 16.0;
                        }
                        let mut flag = false;
                        for j in 0..num22 {
                            ctx.tile_batch.set_layer(LAYER_RAIN, 0);
                            let mut color = lighting_get_color(ctx.g, num13, num14);
                            let mut num25 = 0.6_f32;
                            let mut num26 = 0.3_f32;
                            if num12 == 26 {
                                color = Color::new(255, 255, 255, 127);
                                add_light_for(ctx.g, num12, num13, num14);
                                num25 = 0.9;
                                num26 = 0.4;
                            }
                            if j > num22 - 8 {
                                let num27 = (num22 - j) as f32 / 8.0;
                                num25 *= num27;
                                num26 *= num27;
                            }
                            let color2 = color * num25;
                            let color3 = color * num26;
                            match num12 {
                                22 => {
                                    ctx.tile_batch.draw(textures[22].as_ref(), position, source_rectangle2, VertexColors::new(color2), origin, 1.0, SpriteEffects::NONE);
                                }
                                26 => {
                                    ctx.tile_batch.draw(textures[27].as_ref(), position, source_rectangle, VertexColors::new(color3), origin, 1.0, SpriteEffects::NONE);
                                    ctx.tile_batch.draw(textures[26].as_ref(), position, source_rectangle2, VertexColors::new(color2), origin, 1.0, SpriteEffects::NONE);
                                }
                                _ => {
                                    ctx.tile_batch.draw(textures[12].as_ref(), position, source_rectangle, VertexColors::new(color3), origin, 1.0, SpriteEffects::NONE);
                                    ctx.tile_batch.draw(textures[11].as_ref(), position, source_rectangle2, VertexColors::new(color2), origin, 1.0, SpriteEffects::NONE);
                                }
                            }
                            if flag {
                                break;
                            }
                            num14 += 1;
                            if num14 >= ctx.g.main.max_tiles_y {
                                break;
                            }
                            let tile2 = tile_at(ctx.g, num13, num14);
                            if solid_tile_tile(ctx.g, &tile2) {
                                flag = true;
                            }
                            if tile2.liquid > 0 {
                                let num28 = ((16.0_f32 * (tile2.liquid as i32 as f32 / 255.0)) as i32) & 0xFE;
                                if num28 >= 15 {
                                    break;
                                }
                                source_rectangle2.height -= num28;
                                source_rectangle.height -= num28;
                            }
                            if num14 % 2 == 0 {
                                position.x += 1.0;
                            } else {
                                position.x -= 1.0;
                            }
                            position.y += 16.0;
                        }
                        self.waterfalls[i].stop_at_step = 0;
                        continue 'waterfalls;
                    }
                    0 => {
                        num12 = style;
                    }
                    2 => {
                        if ctx.state.liquid.drew_lava {
                            continue;
                        }
                    }
                    _ => {}
                }
                num21 = 32 * self.regular_frame;
            }
            let mut num29 = 0;
            num22 = self.waterfall_dist;
            let mut color4 = Color::WHITE;
            let mut k = 0;
            while k < num22 {
                if num29 >= 2 {
                    break;
                }
                add_light_for(ctx.g, num12, num13, num14);
                let tile3 = tile_at(ctx.g, num13, num14);
                if tile3.nactive() && ctx.g.main.tile_solid[tile3.type_ as usize] && !ctx.g.main.tile_solid_top[tile3.type_ as usize] && !tile_sets::PLATFORMS[tile3.type_ as usize] && tile3.block_type() == 0 {
                    break;
                }
                let tile4 = tile_at(ctx.g, num13 - 1, num14);
                let tile5 = tile_at(ctx.g, num13, num14 + 1);
                let tile6 = tile_at(ctx.g, num13 + 1, num14);
                if solid_tile_tile(ctx.g, &tile5) && !tile3.half_brick() {
                    num11 = 8;
                } else if num16 != 0 {
                    num11 = 0;
                }
                let mut num30 = 0;
                let num31 = num18;
                let mut num32 = 0;
                let mut num33 = 0;
                let mut flag2 = false;
                if tile5.top_slope() && !tile3.half_brick() && tile5.type_ != 19 {
                    flag2 = true;
                    if tile5.slope() == 1 {
                        num30 = 1;
                        num32 = 1;
                        num17 = 1;
                        num18 = num17;
                    } else {
                        num30 = -1;
                        num32 = -1;
                        num17 = -1;
                        num18 = num17;
                    }
                    num33 = 1;
                } else if (!solid_tile_tile(ctx.g, &tile5) && !tile5.bottom_slope() && !tile3.half_brick()) || (!tile5.active() && !tile3.half_brick()) {
                    num29 = 0;
                    num33 = 1;
                    num32 = 0;
                } else if (solid_tile_tile(ctx.g, &tile4) || tile4.top_slope() || tile4.liquid > 0) && !solid_tile_tile(ctx.g, &tile6) && tile6.liquid == 0 {
                    if num17 == -1 {
                        num29 += 1;
                    }
                    num32 = 1;
                    num33 = 0;
                    num17 = 1;
                } else if (solid_tile_tile(ctx.g, &tile6) || tile6.top_slope() || tile6.liquid > 0) && !solid_tile_tile(ctx.g, &tile4) && tile4.liquid == 0 {
                    if num17 == 1 {
                        num29 += 1;
                    }
                    num32 = -1;
                    num33 = 0;
                    num17 = -1;
                } else if ((!solid_tile_tile(ctx.g, &tile6) && !tile3.top_slope()) || tile6.liquid == 0) && !solid_tile_tile(ctx.g, &tile4) && !tile3.top_slope() && tile4.liquid == 0 {
                    num33 = 0;
                    num32 = num17;
                } else {
                    num29 += 1;
                    num33 = 0;
                    num32 = 0;
                }
                if num29 >= 2 {
                    num17 *= -1;
                    num32 *= -1;
                }
                let mut num34 = -1;
                if num12 != 1 && num12 != 14 && num12 != 25 {
                    if tile5.active() {
                        num34 = tile5.type_ as i32;
                    }
                    if tile3.active() {
                        num34 = tile3.type_ as i32;
                    }
                }
                match num34 {
                    160 => num12 = 2,
                    262..=268 => num12 = 15 + num34 - 262,
                    _ => {}
                }
                let mut color5 = lighting_get_color(ctx.g, num13, num14);
                if k > 50 {
                    try_sparkling(ctx.g, num13, num14, num17, color5);
                }
                let alpha_ = self.get_alpha(ctx.g, alpha, num22, num12, num14, k, &tile3);
                color5 = stylize_color(ctx.g, alpha_, num22, num12, num14, k, &tile3, color5);
                if num12 == 1 {
                    let num35 = ((num13 * 16 + 8) as f32 - (screen_position.x + (screen_width / 2) as f32)).abs();
                    let num36 = ((num14 * 16 + 8) as f32 - (screen_position.y + (screen_height / 2) as f32)).abs();
                    if num35 < (screen_width * 2) as f32 && num36 < (screen_height * 2) as f32 {
                        let num37 = ((num35 * num35 + num36 * num36) as f64).sqrt() as f32;
                        let num38 = 1.0 - num37 / (screen_width as f32 * 0.75);
                        if num38 > 0.0 {
                            num6 += num38;
                        }
                    }
                    if num35 < num7 {
                        num7 = num35;
                        num9 = num13 * 16 + 8;
                    }
                    if num36 < num8 {
                        num8 = num35; // as in the original (it stores num35, not num36)
                        num10 = num14 * 16 + 8;
                    }
                } else if num12 != 1 && num12 != 14 && num12 != 25 && num12 != 11 && num12 != 12 && num12 != 22 {
                    let num39 = ((num13 * 16 + 8) as f32 - (screen_position.x + (screen_width / 2) as f32)).abs();
                    let num40 = ((num14 * 16 + 8) as f32 - (screen_position.y + (screen_height / 2) as f32)).abs();
                    if num39 < (screen_width * 2) as f32 && num40 < (screen_height * 2) as f32 {
                        let num41 = ((num39 * num39 + num40 * num40) as f64).sqrt() as f32;
                        let num42 = 1.0 - num41 / (screen_width as f32 * 0.75);
                        if num42 > 0.0 {
                            num += num42;
                        }
                    }
                    if num39 < num2 {
                        num2 = num39;
                        num4 = num13 * 16 + 8;
                    }
                    if num40 < num3 {
                        num3 = num39; // as in the original (it stores num39, not num40)
                        num5 = num14 * 16 + 8;
                    }
                }
                let num43 = tile3.liquid as i32 / 16;
                ctx.tile_batch.set_layer(LAYER_WATERFALL, 0);
                // `Main.tile[num13, num14]` in the draw calls below is read live; nothing in this loop writes tiles, so `tile3` is the same tile
                let live = tile3;
                if flag2 && num17 != num31 {
                    let num44 = 2;
                    if num31 == 1 {
                        self.draw_waterfall(ctx, textures, num12, num13, num14, alpha_, Vector2::new((num13 * 16 - 16) as f32, (num14 * 16 + 16 - num44) as f32) - screen_position, Rectangle::new(num21, 24, 32, 16 - num43 - num44), color5, SpriteEffects::FLIP_HORIZONTALLY);
                    } else {
                        self.draw_waterfall(ctx, textures, num12, num13, num14, alpha_, Vector2::new((num13 * 16) as f32, (num14 * 16 + 16 - num44) as f32) - screen_position, Rectangle::new(num21, 24, 32, 16 - num43 - num44), color5, SpriteEffects::NONE);
                    }
                }
                if num15 == 0 && num30 != 0 && num16 == 1 && num17 != num18 {
                    num30 = 0;
                    num17 = num18;
                    color5 = Color::WHITE;
                    if num17 == 1 {
                        self.draw_waterfall(ctx, textures, num12, num13, num14, alpha_, Vector2::new((num13 * 16 - 16) as f32, (num14 * 16 + 16) as f32) - screen_position, Rectangle::new(num21, 24, 32, 16 - num43), color5, SpriteEffects::FLIP_HORIZONTALLY);
                    } else {
                        self.draw_waterfall(ctx, textures, num12, num13, num14, alpha_, Vector2::new((num13 * 16 - 16) as f32, (num14 * 16 + 16) as f32) - screen_position, Rectangle::new(num21, 24, 32, 16 - num43), color5, SpriteEffects::FLIP_HORIZONTALLY);
                    }
                }
                if num19 != 0 && num32 == 0 && num33 == 1 {
                    if num17 == 1 {
                        if num20 != num12 {
                            self.draw_waterfall(ctx, textures, num12, num13, num14, alpha_, Vector2::new((num13 * 16) as f32, (num14 * 16 + num11 + 8) as f32) - screen_position, Rectangle::new(num21, 0, 16, 16 - num43 - 8), color4, SpriteEffects::FLIP_HORIZONTALLY);
                        } else {
                            self.draw_waterfall(ctx, textures, num12, num13, num14, alpha_, Vector2::new((num13 * 16) as f32, (num14 * 16 + num11 + 8) as f32) - screen_position, Rectangle::new(num21, 0, 16, 16 - num43 - 8), color5, SpriteEffects::FLIP_HORIZONTALLY);
                        }
                    } else {
                        self.draw_waterfall(ctx, textures, num12, num13, num14, alpha_, Vector2::new((num13 * 16) as f32, (num14 * 16 + num11 + 8) as f32) - screen_position, Rectangle::new(num21, 0, 16, 16 - num43 - 8), color5, SpriteEffects::NONE);
                    }
                }
                if num11 == 8 && num16 == 1 && num19 == 0 {
                    if num18 == -1 {
                        if num20 != num12 {
                            self.draw_waterfall(ctx, textures, num20, num13, num14, alpha_, Vector2::new((num13 * 16) as f32, (num14 * 16) as f32) - screen_position, Rectangle::new(num21, 24, 32, 8), color4, SpriteEffects::NONE);
                        } else {
                            self.draw_waterfall(ctx, textures, num12, num13, num14, alpha_, Vector2::new((num13 * 16) as f32, (num14 * 16) as f32) - screen_position, Rectangle::new(num21, 24, 32, 8), color5, SpriteEffects::NONE);
                        }
                    } else if num20 != num12 {
                        self.draw_waterfall(ctx, textures, num20, num13, num14, alpha_, Vector2::new((num13 * 16 - 16) as f32, (num14 * 16) as f32) - screen_position, Rectangle::new(num21, 24, 32, 8), color4, SpriteEffects::FLIP_HORIZONTALLY);
                    } else {
                        self.draw_waterfall(ctx, textures, num12, num13, num14, alpha_, Vector2::new((num13 * 16 - 16) as f32, (num14 * 16) as f32) - screen_position, Rectangle::new(num21, 24, 32, 8), color5, SpriteEffects::FLIP_HORIZONTALLY);
                    }
                }
                if num30 != 0 && num15 == 0 {
                    if num31 == 1 {
                        if num20 != num12 {
                            self.draw_waterfall(ctx, textures, num20, num13, num14, alpha_, Vector2::new((num13 * 16 - 16) as f32, (num14 * 16) as f32) - screen_position, Rectangle::new(num21, 24, 32, 16 - num43), color4, SpriteEffects::FLIP_HORIZONTALLY);
                        } else {
                            self.draw_waterfall(ctx, textures, num12, num13, num14, alpha_, Vector2::new((num13 * 16 - 16) as f32, (num14 * 16) as f32) - screen_position, Rectangle::new(num21, 24, 32, 16 - num43), color5, SpriteEffects::FLIP_HORIZONTALLY);
                        }
                    } else if num20 != num12 {
                        self.draw_waterfall(ctx, textures, num20, num13, num14, alpha_, Vector2::new((num13 * 16) as f32, (num14 * 16) as f32) - screen_position, Rectangle::new(num21, 24, 32, 16 - num43), color4, SpriteEffects::NONE);
                    } else {
                        self.draw_waterfall(ctx, textures, num12, num13, num14, alpha_, Vector2::new((num13 * 16) as f32, (num14 * 16) as f32) - screen_position, Rectangle::new(num21, 24, 32, 16 - num43), color5, SpriteEffects::NONE);
                    }
                }
                if num33 == 1 && num30 == 0 && num19 == 0 {
                    if num17 == -1 {
                        if num16 == 0 {
                            self.draw_waterfall(ctx, textures, num12, num13, num14, alpha_, Vector2::new((num13 * 16) as f32, (num14 * 16 + num11) as f32) - screen_position, Rectangle::new(num21, 0, 16, 16 - num43), color5, SpriteEffects::NONE);
                        } else if num20 != num12 {
                            self.draw_waterfall(ctx, textures, num20, num13, num14, alpha_, Vector2::new((num13 * 16) as f32, (num14 * 16) as f32) - screen_position, Rectangle::new(num21, 24, 32, 16 - num43), color4, SpriteEffects::NONE);
                        } else {
                            self.draw_waterfall(ctx, textures, num12, num13, num14, alpha_, Vector2::new((num13 * 16) as f32, (num14 * 16) as f32) - screen_position, Rectangle::new(num21, 24, 32, 16 - num43), color5, SpriteEffects::NONE);
                        }
                    } else if num16 == 0 {
                        self.draw_waterfall(ctx, textures, num12, num13, num14, alpha_, Vector2::new((num13 * 16) as f32, (num14 * 16 + num11) as f32) - screen_position, Rectangle::new(num21, 0, 16, 16 - num43), color5, SpriteEffects::FLIP_HORIZONTALLY);
                    } else if num20 != num12 {
                        self.draw_waterfall(ctx, textures, num20, num13, num14, alpha_, Vector2::new((num13 * 16 - 16) as f32, (num14 * 16) as f32) - screen_position, Rectangle::new(num21, 24, 32, 16 - num43), color4, SpriteEffects::FLIP_HORIZONTALLY);
                    } else {
                        self.draw_waterfall(ctx, textures, num12, num13, num14, alpha_, Vector2::new((num13 * 16 - 16) as f32, (num14 * 16) as f32) - screen_position, Rectangle::new(num21, 24, 32, 16 - num43), color5, SpriteEffects::FLIP_HORIZONTALLY);
                    }
                } else {
                    match num32 {
                        1 => {
                            if !(live.liquid > 0 && !live.half_brick()) {
                                if num30 == 1 {
                                    for m in 0..8 {
                                        let num48 = m * 2;
                                        let num49 = 14 - m * 2;
                                        let mut num50 = num48;
                                        num11 = 8;
                                        if num15 == 0 && m < 2 {
                                            num50 = 4;
                                        }
                                        self.draw_waterfall(ctx, textures, num12, num13, num14, alpha_, Vector2::new((num13 * 16 + num48) as f32, (num14 * 16 + num11 + num50) as f32) - screen_position, Rectangle::new(16 + num21 + num49, 0, 2, 16 - num11), color5, SpriteEffects::FLIP_HORIZONTALLY);
                                    }
                                } else {
                                    let mut height2 = 16;
                                    if tile_sets::BLOCKS_WATER_DRAWING_BEHIND_SELF[tile_at(ctx.g, num13, num14).type_ as usize] {
                                        height2 = 8;
                                    } else if tile_sets::BLOCKS_WATER_DRAWING_BEHIND_SELF[tile_at(ctx.g, num13, num14 + 1).type_ as usize] {
                                        height2 = 8;
                                    }
                                    self.draw_waterfall(ctx, textures, num12, num13, num14, alpha_, Vector2::new((num13 * 16) as f32, (num14 * 16 + num11) as f32) - screen_position, Rectangle::new(16 + num21, 0, 16, height2), color5, SpriteEffects::FLIP_HORIZONTALLY);
                                }
                            }
                        }
                        -1 => {
                            if !(live.liquid > 0 && !live.half_brick()) {
                                if num30 == -1 {
                                    for l in 0..8 {
                                        let num45 = l * 2;
                                        let num46 = l * 2;
                                        let mut num47 = 14 - l * 2;
                                        num11 = 8;
                                        if num15 == 0 && l > 5 {
                                            num47 = 4;
                                        }
                                        self.draw_waterfall(ctx, textures, num12, num13, num14, alpha_, Vector2::new((num13 * 16 + num45) as f32, (num14 * 16 + num11 + num47) as f32) - screen_position, Rectangle::new(16 + num21 + num46, 0, 2, 16 - num11), color5, SpriteEffects::FLIP_HORIZONTALLY);
                                    }
                                } else {
                                    let mut height = 16;
                                    if tile_sets::BLOCKS_WATER_DRAWING_BEHIND_SELF[tile_at(ctx.g, num13, num14).type_ as usize] {
                                        height = 8;
                                    } else if tile_sets::BLOCKS_WATER_DRAWING_BEHIND_SELF[tile_at(ctx.g, num13, num14 + 1).type_ as usize] {
                                        height = 8;
                                    }
                                    self.draw_waterfall(ctx, textures, num12, num13, num14, alpha_, Vector2::new((num13 * 16) as f32, (num14 * 16 + num11) as f32) - screen_position, Rectangle::new(16 + num21, 0, 16, height), color5, SpriteEffects::NONE);
                                }
                            }
                        }
                        0 => {
                            if num33 == 0 {
                                if live.liquid == 0 || live.half_brick() {
                                    self.draw_waterfall(ctx, textures, num12, num13, num14, alpha_, Vector2::new((num13 * 16) as f32, (num14 * 16 + num11) as f32) - screen_position, Rectangle::new(16 + num21, 0, 16, 16), color5, SpriteEffects::NONE);
                                }
                                k = 1000;
                            }
                        }
                        _ => {}
                    }
                }
                if tile3.liquid > 0 && !tile3.half_brick() {
                    k = 1000;
                }
                num16 = num33;
                num18 = num17;
                num15 = num32;
                num13 += num32;
                num14 += num33;
                num19 = num30;
                color4 = color5;
                if num20 != num12 {
                    num20 = num12;
                }
                if (tile4.active() && (tile4.type_ == 189 || tile4.type_ == 196)) || (tile6.active() && (tile6.type_ == 189 || tile6.type_ == 196)) || (tile5.active() && (tile5.type_ == 189 || tile5.type_ == 196)) {
                    num22 = (40.0_f32 * (ctx.g.main.max_tiles_x as f32 / 4200.0) * ctx.state.gfx_quality) as i32;
                }
                if !in_world(ctx.g, num13, num14, 0) {
                    break;
                }
                k += 1;
            }
        }
        // Main.ambientWaterfallX .. Main.ambientLavafallStrength: read by the ambient sound code (not ported)
        let l = &mut ctx.state.liquid;
        l.ambient_waterfall_x = num4 as f32;
        l.ambient_waterfall_y = num5 as f32;
        l.ambient_waterfall_strength = num;
        l.ambient_lavafall_x = num9 as f32;
        l.ambient_lavafall_y = num10 as f32;
        l.ambient_lavafall_strength = num6;
        ctx.g.main.tile_solid[546] = true;
    }

    /// `DrawWaterfall(int waterfallType, int x, int y, float opacity, Vector2 position, Rectangle sourceRect, Color color, SpriteEffects effects)` (WaterfallManager.cs:971)
    #[allow(clippy::too_many_arguments)]
    fn draw_waterfall(&self, ctx: &mut DrawContext, textures: &[Option<Texture2D>], waterfall_type: i32, x: i32, y: i32, opacity: f32, position: Vector2, mut source_rect: Rectangle, color: Color, effects: SpriteEffects) {
        let value = textures[waterfall_type as usize].as_ref();
        if waterfall_type == 25 {
            let mut vertices = lighting_get_corner_colors(ctx.g, x, y);
            set_shimmer_vertex_colors(ctx.g, &mut vertices, opacity, x, y);
            ctx.tile_batch.draw(value, position + Vector2::new(0.0, 0.0), source_rect, vertices, Vector2::ZERO, 1.0, effects);
            source_rect.y += 42;
            set_shimmer_vertex_colors_sparkle(ctx.g, &mut vertices, opacity, x, y, true);
            ctx.tile_batch.draw(value, position + Vector2::new(0.0, 0.0), source_rect, vertices, Vector2::ZERO, 1.0, effects);
        } else {
            ctx.tile_batch.draw(value, position, source_rect, VertexColors::new(color), Vector2::ZERO, 1.0, effects);
        }
    }

    /// `GetAlpha(float Alpha, int maxSteps, int waterfallType, int y, int s, Tile tileCache)` (WaterfallManager.cs:1032)
    fn get_alpha(&self, g: &Game, alpha: f32, max_steps: i32, waterfall_type: i32, y: i32, s: i32, tile_cache: &Tile) -> f32 {
        let mut num = match waterfall_type {
            1 => 1.0,
            14 => 0.8,
            25 => 0.75,
            _ => {
                if (tile_cache.wall != 0 && (self.should_show_invisible_blocks_and_walls || (tile_cache.wall != 318 && !tile_cache.invisible_wall()))) || !((y as f64) < g.main.world_surface) {
                    0.6 * alpha
                } else {
                    alpha
                }
            }
        };
        if s > max_steps - 10 {
            num *= (max_steps - s) as f32 / 10.0;
        }
        num
    }

    /// `Draw()` (WaterfallManager.cs:1174)
    pub fn draw(&mut self, ctx: &mut DrawContext) {
        self.should_show_invisible_blocks_and_walls = ctx.g.main.should_show_invisible_blocks_and_walls();
        for i in 0..self.current_max as usize {
            self.waterfalls[i].stop_at_step = self.waterfall_dist;
        }
        ctx.state.liquid.drew_lava = false;
        let textures: Vec<Option<Texture2D>> = (0..MAX_TYPES as i32).map(|n| ctx.waterfall_texture(n)).collect();
        // (liquidAlpha index, waterfall style) in the order Draw() calls them
        let order: [(usize, i32); 12] = [(0, 0), (2, 3), (3, 4), (4, 5), (5, 6), (6, 7), (7, 8), (8, 9), (9, 10), (10, 13), (12, 23), (13, 24)];
        for (alpha_index, style) in order {
            if ctx.g.main.liquid_alpha[alpha_index] > 0.0 {
                let a = ctx.g.main.liquid_alpha[alpha_index];
                self.draw_waterfall_style(ctx, &textures, style, a);
            }
        }
    }
}

/// `WaterfallManager.StylizeColor(float alpha, int maxSteps, int waterfallType, int y, int s, Tile tileCache, Color aColor)` (WaterfallManager.cs:989)
#[allow(clippy::too_many_arguments)]
fn stylize_color(g: &Game, alpha: f32, _max_steps: i32, waterfall_type: i32, _y: i32, _s: i32, _tile_cache: &Tile, a_color: Color) -> Color {
    let mut num = a_color.r() as f32 * alpha;
    let mut num2 = a_color.g() as f32 * alpha;
    let mut num3 = a_color.b() as f32 * alpha;
    let num4 = a_color.a() as f32 * alpha;
    match waterfall_type {
        1 => {
            if num < 190.0 * alpha {
                num = 190.0 * alpha;
            }
            if num2 < 190.0 * alpha {
                num2 = 190.0 * alpha;
            }
            if num3 < 190.0 * alpha {
                num3 = 190.0 * alpha;
            }
        }
        2 => {
            num = g.main.disco_r as f32 * alpha;
            num2 = g.main.disco_g as f32 * alpha;
            num3 = g.main.disco_b as f32 * alpha;
        }
        15..=21 => {
            num = 255.0 * alpha;
            num2 = 255.0 * alpha;
            num3 = 255.0 * alpha;
        }
        _ => {}
    }
    Color::from_ints(num as i32, num2 as i32, num3 as i32, num4 as i32)
}

/// `WaterfallManager.TrySparkling(int x, int y, int direction, Color aColor2)` (WaterfallManager.cs:1057)
fn try_sparkling(g: &mut Game, x: i32, y: i32, direction: i32, a_color2: Color) {
    if a_color2.r() > 20 || a_color2.b() > 20 || a_color2.g() > 20 {
        let mut num = a_color2.r() as f32;
        if (a_color2.g() as f32) > num {
            num = a_color2.g() as f32;
        }
        if (a_color2.b() as f32) > num {
            num = a_color2.b() as f32;
        }
        if (g.main.rand.next(20000) as f32) < num / 30.0 {
            unported!("Dust.NewDust (waterfall sparkle)");
            let _ = (x * 16 - direction * 7, y * 16 + 6);
        }
    }
}

/// `WaterfallManager.AddLight(int waterfallType, int x, int y)` (WaterfallManager.cs:1078)
fn add_light_for(g: &mut Game, waterfall_type: i32, x: i32, y: i32) {
    let mouse_text_color = g.main.mouse_text_color as f32;
    match waterfall_type {
        1 => {
            let num4 = (0.55 + (270.0 - mouse_text_color) / 900.0) * 0.4;
            let r = num4;
            let g_ = num4 * 0.3;
            let b = num4 * 0.1;
            add_light(g, x, y, r, g_, b);
        }
        26 => {
            let num3 = (0.55 + (270.0 - mouse_text_color) / 900.0) * 0.6;
            let r = num3;
            let g_ = num3 * 0.3;
            let b = num3 * 0.1;
            add_light(g, x, y, r, g_, b);
        }
        2 => {
            let mut r = g.main.disco_r as f32 / 255.0;
            let mut g_ = g.main.disco_g as f32 / 255.0;
            let mut b = g.main.disco_b as f32 / 255.0;
            r *= 0.2;
            g_ *= 0.2;
            b *= 0.2;
            add_light(g, x, y, r, g_, b);
        }
        15 => add_light(g, x, y, 0.0, 0.0, 0.2),
        16 => add_light(g, x, y, 0.0, 0.2, 0.0),
        17 => add_light(g, x, y, 0.0, 0.0, 0.2),
        18 => add_light(g, x, y, 0.0, 0.2, 0.0),
        19 => add_light(g, x, y, 0.2, 0.0, 0.0),
        20 => add_light(g, x, y, 0.2, 0.2, 0.2),
        21 => add_light(g, x, y, 0.2, 0.0, 0.0),
        25 => {
            let mut num = 0.7_f32;
            let mut num2 = 0.7_f32;
            num += (270.0 - mouse_text_color) / 900.0;
            num2 += (270.0 - mouse_text_color) / 125.0;
            add_light(g, x, y, num * 0.6, num2 * 0.25, num * 0.9);
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::draw_context::test_support::with_ctx;

    /// A half brick with water to its left (above 160) and open air to its right is a waterfall start (type 0); the same next to lava is type 1.
    #[test]
    fn find_waterfalls_finds_a_half_brick_spout_and_its_liquid_type() {
        with_ctx(|ctx| {
            ctx.g.main.screen_position = (100.0 * 16.0, 100.0 * 16.0);
            ctx.g.main.screen_width = 320;
            ctx.g.main.screen_height = 320;
            for (x, lava) in [(110, false), (130, true)] {
                let t = ctx.g.main.tile.get_mut(x, 110);
                t.set_active(true);
                t.set_half_brick(true);
                t.type_ = 0;
                ctx.g.main.tile_solid[0] = true;
                let w = ctx.g.main.tile.get_mut(x - 1, 110);
                w.liquid = 200;
                if lava {
                    w.set_liquid_type(1);
                }
            }
            let mut m = WaterfallManager::new();
            m.find_waterfalls(ctx.g, 1.0, true);
            assert_eq!(m.current_max, 2);
            assert_eq!((m.waterfalls[0].x, m.waterfalls[0].y, m.waterfalls[0].type_), (110, 110, 0));
            assert_eq!((m.waterfalls[1].x, m.waterfalls[1].y, m.waterfalls[1].type_), (130, 110, 1));
            assert!(m.check_for_waterfall(110, 110) && !m.check_for_waterfall(111, 110));
            // `FindWaterfalls` only scans every 30th call unless forced
            m.find_waterfalls(ctx.g, 1.0, false);
            assert_eq!(m.current_max, 2);
        });
    }

    #[test]
    fn update_frame_steps_the_frame_counters_like_the_original() {
        let mut m = WaterfallManager::new();
        for _ in 0..3 {
            m.update_frame();
        }
        // wFallFrCounter > 2 on the third call: regularFrame 1; wFallFrCounter2 > 6 not yet: slowFrame 0
        assert_eq!((m.regular_frame, m.slow_frame), (1, 0));
        for _ in 0..4 {
            m.update_frame();
        }
        assert_eq!(m.slow_frame, 1);
        // rainFrameForeground steps every call and wraps at 8
        assert_eq!(m.rain_frame_foreground, 7 % 8);
    }
}
