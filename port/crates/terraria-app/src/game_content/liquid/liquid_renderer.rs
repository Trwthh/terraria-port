//! `Terraria.GameContent.Liquid.LiquidRenderer` (GameContent/Liquid/LiquidRenderer.cs): turns the liquid levels of the tiles on screen
//! into quads (`PrepareDraw`), animates them (`Update`) and draws them through `Main.tileBatch` (`DrawNormalLiquids`, `DrawShimmer`).
//!
//! The original works on `LiquidCache*` pointers into one array; here a pointer is an index into the same `Vec` (`cache[p]`, `cache[p - 1]`,
//! `ptr2[-rectangle.Height]` is `cache[p - h]`), and every `LiquidCache liquidCache = ptr2[-1]` copy is a copy of the struct. The cache arrays
//! live between calls like the originals' fields do, because some reads (the border of the prepared area) see the previous call's values.
//!
//! Not ported here, as `unported!` at their original place: the dust the lava emits (`Dust.NewDust`), the `WaveFilters` event that feeds
//! `WaterShaderData` (the screen-space water distortion), `FocusHelper.PauseLiquidRenderer`.

use terraria_core::xna::{Color, Point, Rectangle, Vector2, Vector4};
use terraria_core::{Tile, UnifiedRandom};
use terraria_data::sets::tile_id::sets as tile_sets;
use terraria_sim::unported;
use terraria_sim::Game;

use crate::draw_context::DrawContext;
use crate::game_content::drawing::tile_drawing::TileDrawing;
use crate::gfx::{SpriteEffects, Texture2D};
use crate::pending::{lighting_get_corner_colors, utils_remap, VertexColors};

/// `LiquidRenderer.WATERFALL_LENGTH` (LiquidRenderer.cs:106)
const WATERFALL_LENGTH: [i32; 4] = [10, 3, 2, 10];
/// `LiquidRenderer.DEFAULT_OPACITY` (LiquidRenderer.cs:108)
const DEFAULT_OPACITY: [f32; 4] = [0.6, 0.95, 0.95, 0.75];
/// `LiquidRenderer.WAVE_MASK_STRENGTH` (LiquidRenderer.cs:110): `new byte[5]`
const WAVE_MASK_STRENGTH: [u8; 5] = [0; 5];
/// `LiquidRenderer.VISCOSITY_MASK` (LiquidRenderer.cs:112)
const VISCOSITY_MASK: [u8; 5] = [0, 200, 240, 0, 0];
/// `LiquidRenderer.MIN_LIQUID_SIZE` (LiquidRenderer.cs:114); nothing reads it
#[allow(dead_code)]
pub const MIN_LIQUID_SIZE: f32 = 0.25;
/// `LiquidRenderer.ANIMATION_FRAME_COUNT` (LiquidRenderer.cs:100); nothing reads it
#[allow(dead_code)]
const ANIMATION_FRAME_COUNT: i32 = 16;

/// `LiquidRenderer.LiquidCache`
#[derive(Clone, Copy, Default)]
struct LiquidCache {
    liquid_level: f32,
    visible_liquid_level: f32,
    opacity: f32,
    is_solid: bool,
    is_half_brick: bool,
    has_liquid: bool,
    has_visible_liquid: bool,
    has_wall: bool,
    frame_offset: Point,
    has_left_edge: bool,
    has_right_edge: bool,
    has_top_edge: bool,
    has_bottom_edge: bool,
    left_wall: f32,
    right_wall: f32,
    bottom_wall: f32,
    top_wall: f32,
    visible_left_wall: f32,
    visible_right_wall: f32,
    visible_bottom_wall: f32,
    visible_top_wall: f32,
    type_: u8,
    visible_type: u8,
}

/// `LiquidRenderer.LiquidDrawCache`
#[derive(Clone, Copy, Default)]
struct LiquidDrawCache {
    source_rectangle: Rectangle,
    liquid_offset: Vector2,
    is_visible: bool,
    opacity: f32,
    type_: u8,
    is_surface_liquid: bool,
    has_wall: bool,
}

/// `LiquidRenderer.SpecialLiquidDrawCache` (all fields of the original are kept; `DrawShimmer` does not read `Type` and `HasWall`)
#[derive(Clone, Copy, Default)]
#[allow(dead_code)]
struct SpecialLiquidDrawCache {
    x: i32,
    y: i32,
    source_rectangle: Rectangle,
    liquid_offset: Vector2,
    is_visible: bool,
    opacity: f32,
    type_: u8,
    is_surface_liquid: bool,
    has_wall: bool,
}

/// A CPU stand-in for the `Texture2D` that `LiquidRenderer.SetWaveMaskData` fills (the texture `WaterShaderData` samples).
#[derive(Default)]
#[allow(dead_code)] // only `WaterShaderData` (not ported) reads it
pub struct WaveMaskTexture {
    pub width: i32,
    pub height: i32,
    pub data: Vec<Color>,
}

/// `Terraria.GameContent.Liquid.LiquidRenderer` (the instance `LiquidRenderer.Instance`)
pub struct LiquidRenderer {
    /// `_cache`
    cache: Vec<LiquidCache>,
    /// `_drawCache`
    draw_cache: Vec<LiquidDrawCache>,
    /// `_drawCacheForShimmer`
    draw_cache_for_shimmer: Vec<SpecialLiquidDrawCache>,
    /// `_animationFrame`
    animation_frame: i32,
    /// `_waterfallAnimationFrame`
    waterfall_animation_frame: i32,
    /// `_drawArea` (`new Rectangle(0, 0, 1, 1)`)
    draw_area: Rectangle,
    /// `_random` (`new UnifiedRandom()`: seeded from the clock in the original; it only decides when lava emits dust)
    random: UnifiedRandom,
    /// `_waveMask`
    wave_mask: Vec<Color>,
    /// `_frameState`
    frame_state: f32,
    /// `_waterfallFrameState`
    waterfall_frame_state: f32,
}

impl Default for LiquidRenderer {
    fn default() -> Self {
        Self::new()
    }
}

/// `WorldGen.SolidOrSlopedTile(Tile tile, bool includePlatforms = false)` (WorldGen.cs:70796), `tile` never null here.
fn world_gen_solid_or_sloped_tile(g: &Game, tile: &Tile) -> bool {
    if tile.active() && g.main.tile_solid[tile.type_ as usize] {
        let num = if !g.main.tile_solid_top[tile.type_ as usize] { 1 } else { 0 }; // includePlatforms is false
        if num != 0 {
            return !tile.in_active();
        }
    }
    false
}

impl LiquidRenderer {
    /// `new LiquidRenderer()` field initialisers (LiquidRenderer.cs:118-138). The textures (`PrepareAssets`) are looked up through
    /// `DrawContext::misc_water_texture` where they are used.
    pub fn new() -> Self {
        Self {
            cache: vec![LiquidCache::default(); 1],
            draw_cache: vec![LiquidDrawCache::default(); 1],
            draw_cache_for_shimmer: vec![SpecialLiquidDrawCache::default(); 1],
            animation_frame: 0,
            waterfall_animation_frame: 0,
            draw_area: Rectangle::new(0, 0, 1, 1),
            random: UnifiedRandom::new(0),
            wave_mask: vec![Color::default(); 1],
            frame_state: 0.0,
            waterfall_frame_state: 0.0,
        }
    }

    /// `InternalPrepareDraw(Rectangle drawArea)` (LiquidRenderer.cs:161)
    fn internal_prepare_draw(&mut self, g: &Game, draw_area: Rectangle) {
        let rectangle = Rectangle::new(draw_area.x - 2, draw_area.y - 2, draw_area.width + 4, draw_area.height + 4);
        self.draw_area = draw_area;
        if (self.cache.len() as i32) < rectangle.width * rectangle.height + 1 {
            self.cache = vec![LiquidCache::default(); (rectangle.width * rectangle.height + 1) as usize];
        }
        if (self.draw_cache.len() as i32) < draw_area.width * draw_area.height + 1 {
            self.draw_cache = vec![LiquidDrawCache::default(); (draw_area.width * draw_area.height + 1) as usize];
        }
        if (self.draw_cache_for_shimmer.len() as i32) < draw_area.width * draw_area.height + 1 {
            self.draw_cache_for_shimmer = vec![SpecialLiquidDrawCache::default(); (draw_area.width * draw_area.height + 1) as usize];
        }
        if (self.wave_mask.len() as i32) < draw_area.width * draw_area.height {
            self.wave_mask = vec![Color::default(); (draw_area.width * draw_area.height) as usize];
        }
        // `fixed (LiquidCache* ptr = &_cache[1])`: pointers are indices; `ptr` is index 1
        let ptr: usize = 1;
        let h = rectangle.height as usize;
        let num = (rectangle.height * 2 + 2) as usize;
        let mut p = ptr;
        for i in rectangle.x..rectangle.x + rectangle.width {
            for j in rectangle.y..rectangle.y + rectangle.height {
                let tile = g.main.tile.try_get(i, j).unwrap_or_default(); // `if (tile == null) tile = new Tile()`
                self.cache[p].liquid_level = tile.liquid as i32 as f32 / 255.0;
                self.cache[p].is_half_brick = tile.half_brick() && self.cache[p - 1].has_liquid && !tile_sets::PLATFORMS[tile.type_ as usize];
                self.cache[p].is_solid = world_gen_solid_or_sloped_tile(g, &tile);
                self.cache[p].has_liquid = tile.liquid != 0;
                self.cache[p].visible_liquid_level = 0.0;
                self.cache[p].has_wall = tile.wall != 0;
                self.cache[p].type_ = tile.liquid_type();
                if self.cache[p].is_half_brick && !self.cache[p].has_liquid {
                    self.cache[p].type_ = self.cache[p - 1].type_;
                }
                p += 1;
            }
        }
        p = ptr;
        let mut num2;
        p += num;
        for _k in 2..rectangle.width - 2 {
            for _l in 2..rectangle.height - 2 {
                num2 = 0.0_f32;
                if self.cache[p].is_half_brick && self.cache[p - 1].has_liquid {
                    num2 = 1.0;
                } else if !self.cache[p].has_liquid {
                    let liquid_cache = self.cache[p - 1];
                    let liquid_cache2 = self.cache[p + 1];
                    let liquid_cache3 = self.cache[p - h];
                    let liquid_cache4 = self.cache[p + h];
                    if liquid_cache.has_liquid && liquid_cache2.has_liquid && liquid_cache.type_ == liquid_cache2.type_ && !liquid_cache.is_solid && !liquid_cache2.is_solid {
                        num2 = liquid_cache.liquid_level + liquid_cache2.liquid_level;
                        self.cache[p].type_ = liquid_cache.type_;
                    }
                    if liquid_cache3.has_liquid && liquid_cache4.has_liquid && liquid_cache3.type_ == liquid_cache4.type_ && !liquid_cache3.is_solid && !liquid_cache4.is_solid {
                        num2 = num2.max(liquid_cache3.liquid_level + liquid_cache4.liquid_level);
                        self.cache[p].type_ = liquid_cache3.type_;
                    }
                    num2 *= 0.5;
                } else {
                    num2 = self.cache[p].liquid_level;
                }
                self.cache[p].visible_liquid_level = num2;
                self.cache[p].has_visible_liquid = num2 != 0.0;
                p += 1;
            }
            p += 4;
        }
        p = ptr;
        for _m in 0..rectangle.width {
            for _n in 0..rectangle.height - 10 {
                if self.cache[p].has_visible_liquid && (!self.cache[p].is_solid || self.cache[p].is_half_brick) {
                    self.cache[p].opacity = 1.0;
                    self.cache[p].visible_type = self.cache[p].type_;
                    let waterfall_length = WATERFALL_LENGTH[self.cache[p].type_ as usize];
                    let num3 = 1.0 / (waterfall_length + 1) as f32;
                    let mut num4 = 1.0_f32;
                    for num5 in 1..=waterfall_length {
                        num4 -= num3;
                        let q = p + num5 as usize;
                        if self.cache[q].is_solid {
                            break;
                        }
                        self.cache[q].visible_liquid_level = self.cache[q].visible_liquid_level.max(self.cache[p].visible_liquid_level * num4);
                        self.cache[q].opacity = num4;
                        self.cache[q].visible_type = self.cache[p].type_;
                    }
                }
                if self.cache[p].is_solid && !self.cache[p].is_half_brick {
                    self.cache[p].visible_liquid_level = 1.0;
                    self.cache[p].has_visible_liquid = false;
                } else {
                    self.cache[p].has_visible_liquid = self.cache[p].visible_liquid_level != 0.0;
                }
                p += 1;
            }
            p += 10;
        }
        p = ptr;
        p += num;
        for _num6 in 2..rectangle.width - 2 {
            for num7 in 2..rectangle.height - 2 {
                if !self.cache[p].has_visible_liquid {
                    self.cache[p].has_left_edge = false;
                    self.cache[p].has_top_edge = false;
                    self.cache[p].has_right_edge = false;
                    self.cache[p].has_bottom_edge = false;
                } else {
                    let liquid_cache = self.cache[p - 1];
                    let liquid_cache2 = self.cache[p + 1];
                    let liquid_cache3 = self.cache[p - h];
                    let liquid_cache4 = self.cache[p + h];
                    let mut num8 = 0.0_f32;
                    let mut num9 = 1.0_f32;
                    let mut num10 = 0.0_f32;
                    let mut num11 = 1.0_f32;
                    let visible_liquid_level = self.cache[p].visible_liquid_level;
                    if !liquid_cache.has_visible_liquid {
                        num10 += liquid_cache2.visible_liquid_level * (1.0 - visible_liquid_level);
                    }
                    if !liquid_cache2.has_visible_liquid && !liquid_cache2.is_solid && !liquid_cache2.is_half_brick {
                        num11 -= liquid_cache.visible_liquid_level * (1.0 - visible_liquid_level);
                    }
                    if !liquid_cache3.has_visible_liquid && !liquid_cache3.is_solid && !liquid_cache3.is_half_brick {
                        num8 += liquid_cache4.visible_liquid_level * (1.0 - visible_liquid_level);
                    }
                    if !liquid_cache4.has_visible_liquid && !liquid_cache4.is_solid && !liquid_cache4.is_half_brick {
                        num9 -= liquid_cache3.visible_liquid_level * (1.0 - visible_liquid_level);
                    }
                    self.cache[p].left_wall = num8;
                    self.cache[p].right_wall = num9;
                    self.cache[p].bottom_wall = num11;
                    self.cache[p].top_wall = num10;
                    let mut zero = Point { x: 0, y: 0 };
                    self.cache[p].has_top_edge = (!liquid_cache.has_visible_liquid && !liquid_cache.is_solid) || num10 != 0.0;
                    self.cache[p].has_bottom_edge = (!liquid_cache2.has_visible_liquid && !liquid_cache2.is_solid) || num11 != 1.0;
                    self.cache[p].has_left_edge = (!liquid_cache3.has_visible_liquid && !liquid_cache3.is_solid) || num8 != 0.0;
                    self.cache[p].has_right_edge = (!liquid_cache4.has_visible_liquid && !liquid_cache4.is_solid) || num9 != 1.0;
                    if !self.cache[p].has_left_edge {
                        if self.cache[p].has_right_edge {
                            zero.x += 32;
                        } else {
                            zero.x += 16;
                        }
                    }
                    if self.cache[p].has_left_edge && self.cache[p].has_right_edge {
                        zero.x = 16;
                        zero.y += 32;
                        if self.cache[p].has_top_edge {
                            zero.y = 16;
                        }
                    } else if !self.cache[p].has_top_edge {
                        if !self.cache[p].has_left_edge && !self.cache[p].has_right_edge {
                            zero.y += 48;
                        } else {
                            zero.y += 16;
                        }
                    }
                    if zero.y == 16 && (self.cache[p].has_left_edge ^ self.cache[p].has_right_edge) && (num7 + rectangle.y) % 2 == 0 {
                        zero.y += 16;
                    }
                    self.cache[p].frame_offset = zero;
                }
                p += 1;
            }
            p += 4;
        }
        p = ptr;
        p += num;
        for _num12 in 2..rectangle.width - 2 {
            for _num13 in 2..rectangle.height - 2 {
                if self.cache[p].has_visible_liquid {
                    let liquid_cache = self.cache[p - 1];
                    let liquid_cache2 = self.cache[p + 1];
                    let liquid_cache3 = self.cache[p - h];
                    let liquid_cache4 = self.cache[p + h];
                    self.cache[p].visible_left_wall = self.cache[p].left_wall;
                    self.cache[p].visible_right_wall = self.cache[p].right_wall;
                    self.cache[p].visible_top_wall = self.cache[p].top_wall;
                    self.cache[p].visible_bottom_wall = self.cache[p].bottom_wall;
                    if liquid_cache.has_visible_liquid && liquid_cache2.has_visible_liquid {
                        if self.cache[p].has_left_edge {
                            self.cache[p].visible_left_wall = (self.cache[p].left_wall * 2.0 + liquid_cache.left_wall + liquid_cache2.left_wall) * 0.25;
                        }
                        if self.cache[p].has_right_edge {
                            self.cache[p].visible_right_wall = (self.cache[p].right_wall * 2.0 + liquid_cache.right_wall + liquid_cache2.right_wall) * 0.25;
                        }
                    }
                    if liquid_cache3.has_visible_liquid && liquid_cache4.has_visible_liquid {
                        if self.cache[p].has_top_edge {
                            self.cache[p].visible_top_wall = (self.cache[p].top_wall * 2.0 + liquid_cache3.top_wall + liquid_cache4.top_wall) * 0.25;
                        }
                        if self.cache[p].has_bottom_edge {
                            self.cache[p].visible_bottom_wall = (self.cache[p].bottom_wall * 2.0 + liquid_cache3.bottom_wall + liquid_cache4.bottom_wall) * 0.25;
                        }
                    }
                }
                p += 1;
            }
            p += 4;
        }
        p = ptr;
        p += num;
        for _num14 in 2..rectangle.width - 2 {
            for _num15 in 2..rectangle.height - 2 {
                if self.cache[p].has_liquid {
                    let liquid_cache2 = self.cache[p + 1];
                    let liquid_cache3 = self.cache[p - h];
                    let liquid_cache4 = self.cache[p + h];
                    if self.cache[p].has_top_edge && !self.cache[p].has_bottom_edge && (self.cache[p].has_left_edge ^ self.cache[p].has_right_edge) {
                        if self.cache[p].has_right_edge {
                            self.cache[p].visible_right_wall = liquid_cache2.visible_right_wall;
                            self.cache[p].visible_top_wall = liquid_cache3.visible_top_wall;
                        } else {
                            self.cache[p].visible_left_wall = liquid_cache2.visible_left_wall;
                            self.cache[p].visible_top_wall = liquid_cache4.visible_top_wall;
                        }
                    } else if liquid_cache2.frame_offset.x == 16 && liquid_cache2.frame_offset.y == 32 {
                        if self.cache[p].visible_left_wall > 0.5 {
                            self.cache[p].visible_left_wall = 0.0;
                            self.cache[p].frame_offset = Point { x: 0, y: 0 };
                        } else if self.cache[p].visible_right_wall < 0.5 {
                            self.cache[p].visible_right_wall = 1.0;
                            self.cache[p].frame_offset = Point { x: 32, y: 0 };
                        }
                    }
                }
                p += 1;
            }
            p += 4;
        }
        p = ptr;
        p += num;
        for _num16 in 2..rectangle.width - 2 {
            for _num17 in 2..rectangle.height - 2 {
                if self.cache[p].has_liquid {
                    let liquid_cache = self.cache[p - 1];
                    let liquid_cache3 = self.cache[p - h];
                    let liquid_cache4 = self.cache[p + h];
                    if !self.cache[p].has_bottom_edge && !self.cache[p].has_left_edge && !self.cache[p].has_top_edge && !self.cache[p].has_right_edge {
                        if liquid_cache3.has_top_edge && liquid_cache.has_left_edge {
                            self.cache[p].frame_offset.x = 4.max((16.0_f32 - liquid_cache.visible_left_wall * 16.0) as i32) - 4;
                            self.cache[p].frame_offset.y = 48 + 4.max((16.0_f32 - liquid_cache3.visible_top_wall * 16.0) as i32) - 4;
                            self.cache[p].visible_left_wall = 0.0;
                            self.cache[p].visible_top_wall = 0.0;
                            self.cache[p].visible_right_wall = 1.0;
                            self.cache[p].visible_bottom_wall = 1.0;
                        } else if liquid_cache4.has_top_edge && liquid_cache.has_right_edge {
                            self.cache[p].frame_offset.x = 32 - 16.min((liquid_cache.visible_right_wall * 16.0) as i32 - 4);
                            self.cache[p].frame_offset.y = 48 + 4.max((16.0_f32 - liquid_cache4.visible_top_wall * 16.0) as i32) - 4;
                            self.cache[p].visible_left_wall = 0.0;
                            self.cache[p].visible_top_wall = 0.0;
                            self.cache[p].visible_right_wall = 1.0;
                            self.cache[p].visible_bottom_wall = 1.0;
                        }
                    }
                }
                p += 1;
            }
            p += 4;
        }
        p = ptr;
        p += num;
        {
            let mut p5: usize = 0; // `LiquidDrawCache* ptr5 = ptr3`
            let mut p6: usize = 0; // `Color* ptr6 = ptr4`
            for _num18 in 2..rectangle.width - 2 {
                for num19 in 2..rectangle.height - 2 {
                    if self.cache[p].has_visible_liquid {
                        let c = self.cache[p];
                        let num20 = 0.75_f32.min(c.visible_left_wall);
                        let num21 = 0.25_f32.max(c.visible_right_wall);
                        let num22 = 0.75_f32.min(c.visible_top_wall);
                        let mut num23 = 0.25_f32.max(c.visible_bottom_wall);
                        if c.is_half_brick && c.is_solid && num23 > 0.5 {
                            num23 = 0.5;
                        }
                        self.draw_cache[p5].is_visible = c.has_wall || !c.is_half_brick || !c.has_liquid || !(c.liquid_level < 1.0);
                        self.draw_cache[p5].source_rectangle = Rectangle::new(
                            (16.0_f32 - num21 * 16.0) as i32 + c.frame_offset.x,
                            (16.0_f32 - num23 * 16.0) as i32 + c.frame_offset.y,
                            (((num21 - num20) * 16.0) as f64).ceil() as i32,
                            (((num23 - num22) * 16.0) as f64).ceil() as i32,
                        );
                        self.draw_cache[p5].is_surface_liquid = c.frame_offset.x == 16 && c.frame_offset.y == 0 && ((num19 + rectangle.y) as f64) > g.main.world_surface - 40.0;
                        self.draw_cache[p5].opacity = c.opacity;
                        self.draw_cache[p5].liquid_offset = Vector2::new(((num20 * 16.0) as f64).floor() as f32, ((num22 * 16.0) as f64).floor() as f32);
                        self.draw_cache[p5].type_ = c.visible_type;
                        self.draw_cache[p5].has_wall = c.has_wall;
                        let b = WAVE_MASK_STRENGTH[c.visible_type as usize];
                        let g_ = b >> 1;
                        self.wave_mask[p6] = Color::new(g_, g_, VISCOSITY_MASK[c.visible_type as usize], b);
                        let ptr7 = self.cache[p - 1];
                        if num19 != 2 && !ptr7.has_visible_liquid && !ptr7.is_solid && !ptr7.is_half_brick {
                            self.wave_mask[p6 - 1] = self.wave_mask[p6];
                        }
                    } else {
                        self.draw_cache[p5].is_visible = false;
                        let c = self.cache[p];
                        let num24 = if !c.is_solid && !c.is_half_brick { 4 } else { 3 };
                        let b3 = WAVE_MASK_STRENGTH[num24];
                        let g2 = b3 >> 1;
                        self.wave_mask[p6] = Color::new(g2, g2, VISCOSITY_MASK[num24], b3);
                    }
                    p += 1;
                    p5 += 1;
                    p6 += 1;
                }
                p += 4;
            }
        }
        p = ptr;
        for num25 in rectangle.x..rectangle.x + rectangle.width {
            for num26 in rectangle.y..rectangle.y + rectangle.height {
                if self.cache[p].visible_type == 1 && self.cache[p].has_visible_liquid && unported!("Dust.lavaBubbles", 0) < 200 {
                    // neutral for `Dust.lavaBubbles`: 0, so the rolls below happen; the dust itself is not made
                    if self.random.next(700) == 0 {
                        unported!("Dust.NewDust (lava bubble)");
                        let _ = (num25 * 16, num26 * 16);
                    }
                    if self.random.next(350) == 0 {
                        unported!("Dust.NewDust (lava ember)");
                        // `Main.dust[num27].velocity.Y -= (float)_random.Next(1, 7) * 0.1f; if (_random.Next(10) == 0) velocity.Y *= _random.Next(2, 5)`
                        let _ = self.random.next_range(1, 7);
                        if self.random.next(10) == 0 {
                            let _ = self.random.next_range(2, 5);
                        }
                    }
                }
                p += 1;
            }
        }
        {
            let mut p10: usize = 0; // `LiquidDrawCache* ptr10`
            let mut p11: usize = 0; // `SpecialLiquidDrawCache* ptr11`
            for num28 in 2..rectangle.width - 2 {
                for num29 in 2..rectangle.height - 2 {
                    if self.draw_cache[p10].is_visible && self.draw_cache[p10].type_ == 3 {
                        let d = self.draw_cache[p10];
                        self.draw_cache_for_shimmer[p11] = SpecialLiquidDrawCache {
                            x: num28,
                            y: num29,
                            source_rectangle: d.source_rectangle,
                            liquid_offset: d.liquid_offset,
                            is_visible: d.is_visible,
                            opacity: d.opacity,
                            type_: d.type_,
                            is_surface_liquid: d.is_surface_liquid,
                            has_wall: d.has_wall,
                        };
                        self.draw_cache[p10].is_visible = false;
                        p11 += 1;
                    }
                    p10 += 1;
                }
            }
            self.draw_cache_for_shimmer[p11].is_visible = false;
        }
        // `if (this.WaveFilters != null) this.WaveFilters(_waveMask, GetCachedDrawArea())`: WaterShaderData subscribes in the game
        unported!("LiquidRenderer.WaveFilters (WaterShaderData)");
    }

    /// `DrawNormalLiquids(SpriteBatch spriteBatch, Vector2 drawOffset, int waterStyle, float globalAlpha, bool isBackgroundDraw, bool waterOnly)` (LiquidRenderer.cs:622).
    /// `tiles` is `Main.instance.TilesRenderer` (for `Main.DrawTileInWater`).
    #[allow(clippy::too_many_arguments)]
    pub fn draw_normal_liquids(&mut self, ctx: &mut DrawContext, tiles: &mut TileDrawing, draw_offset: Vector2, water_style: i32, global_alpha: f32, is_background_draw: bool, water_only: bool) {
        let draw_area = self.draw_area;
        ctx.tile_batch.restart(ctx.list);
        let textures: Vec<Option<Texture2D>> = (0..15).map(|n| ctx.misc_water_texture(n)).collect();
        let mut p = 0usize;
        for i in draw_area.x..draw_area.x + draw_area.width {
            for j in draw_area.y..draw_area.y + draw_area.height {
                let c = self.draw_cache[p];
                if c.is_visible && (!water_only || c.type_ == 0) {
                    ctx.tile_batch.set_layer(0, 0);
                    let mut source_rectangle = c.source_rectangle;
                    if c.is_surface_liquid {
                        source_rectangle.y = 1280;
                    } else if source_rectangle.x == 16 {
                        source_rectangle.y += self.waterfall_animation_frame * 80;
                    } else {
                        source_rectangle.y += self.animation_frame * 80;
                    }
                    let liquid_offset = c.liquid_offset;
                    let mut num = c.opacity * if is_background_draw { 1.0 } else { DEFAULT_OPACITY[c.type_ as usize] };
                    let mut num2 = c.type_ as i32;
                    match num2 {
                        0 => {
                            num2 = water_style;
                            num *= global_alpha;
                        }
                        1 => {
                            num *= ctx.state.liquid.lava_opacity; // Main.player[Main.myPlayer].lavaOpacity
                        }
                        2 => {
                            num2 = 11;
                        }
                        _ => {}
                    }
                    num = num.min(1.0);
                    let mut vertices = lighting_get_corner_colors(ctx.g, i, j);
                    vertices.bottom_left_color = vertices.bottom_left_color * num;
                    vertices.bottom_right_color = vertices.bottom_right_color * num;
                    vertices.top_left_color = vertices.top_left_color * num;
                    vertices.top_right_color = vertices.top_right_color * num;
                    crate::main_liquid::draw_tile_in_water(ctx, tiles, draw_offset, i, j);
                    ctx.tile_batch.draw(
                        textures[num2 as usize].as_ref(),
                        Vector2::new((i << 4) as f32, (j << 4) as f32) + draw_offset + liquid_offset,
                        source_rectangle,
                        vertices,
                        Vector2::ZERO,
                        1.0,
                        SpriteEffects::NONE,
                    );
                }
                p += 1;
            }
        }
        let _value = ctx.tile_batch.end(ctx.list); // `(isBackgroundDraw ? TimeLogger.LiquidBackgroundDrawCalls : TimeLogger.LiquidDrawCalls).Add(value)`
    }

    /// `DrawShimmer(SpriteBatch spriteBatch, Vector2 drawOffset, bool isBackgroundDraw)` (LiquidRenderer.cs:682)
    pub fn draw_shimmer(&mut self, ctx: &mut DrawContext, tiles: &mut TileDrawing, draw_offset: Vector2, is_background_draw: bool) {
        let draw_area = self.draw_area;
        ctx.tile_batch.restart(ctx.list);
        let texture = ctx.misc_water_texture(14);
        let num = self.draw_cache_for_shimmer.len();
        for i in 0..num {
            let c = self.draw_cache_for_shimmer[i];
            if !c.is_visible {
                break;
            }
            ctx.tile_batch.set_layer(0, 0);
            let mut source_rectangle = c.source_rectangle;
            if c.is_surface_liquid {
                source_rectangle.y = 1280;
            } else {
                source_rectangle.y += self.animation_frame * 80;
            }
            let liquid_offset = c.liquid_offset;
            let mut val = c.opacity * if is_background_draw { 1.0 } else { 0.75 };
            val = val.min(1.0);
            let num3 = c.x + draw_area.x - 2;
            let num4 = c.y + draw_area.y - 2;
            let mut vertices = lighting_get_corner_colors(ctx.g, num3, num4);
            set_shimmer_vertex_colors(ctx.g, &mut vertices, val, num3, num4);
            crate::main_liquid::draw_tile_in_water(ctx, tiles, draw_offset, num3, num4);
            ctx.tile_batch.draw(texture.as_ref(), Vector2::new((num3 << 4) as f32, (num4 << 4) as f32) + draw_offset + liquid_offset, source_rectangle, vertices, Vector2::ZERO, 1.0, SpriteEffects::NONE);
            source_rectangle = c.source_rectangle;
            let flag = source_rectangle.x != 16 || source_rectangle.y % 80 != 48;
            if flag || (num3 + num4) % 2 == 0 {
                source_rectangle.x += 48;
                source_rectangle.y += 80 * self.get_shimmer_frame(ctx.g, flag, num3 as f32, num4 as f32);
                set_shimmer_vertex_colors_sparkle(ctx.g, &mut vertices, c.opacity, num3, num4, flag);
                ctx.tile_batch.draw(texture.as_ref(), Vector2::new((num3 << 4) as f32, (num4 << 4) as f32) + draw_offset + liquid_offset, source_rectangle, vertices, Vector2::ZERO, 1.0, SpriteEffects::NONE);
            }
        }
        let _value = ctx.tile_batch.end(ctx.list); // `(isBackgroundDraw ? TimeLogger.LiquidBackgroundDrawCalls : TimeLogger.LiquidDrawCalls).Add(value)`
    }

    /// `GetShimmerFrame(bool top, float worldPositionX, float worldPositionY)` (LiquidRenderer.cs:791)
    pub fn get_shimmer_frame(&self, g: &Game, top: bool, mut world_position_x: f32, mut world_position_y: f32) -> i32 {
        world_position_x += 0.5;
        world_position_y += 0.5;
        let mut num = ((world_position_x + world_position_y / 6.0) / 10.0) as f64 - g.main.time_for_visual_effects / 360.0;
        if !top {
            num += (world_position_x + world_position_y) as f64;
        }
        ((num as i32) % 16 + 16) % 16
    }

    /// `HasFullWater(int x, int y)` (LiquidRenderer.cs:809)
    pub fn has_full_water(&self, mut x: i32, mut y: i32) -> bool {
        x -= self.draw_area.x;
        y -= self.draw_area.y;
        let num = x * self.draw_area.height + y;
        if num >= 0 && (num as usize) < self.draw_cache.len() {
            if self.draw_cache[num as usize].is_visible {
                return !self.draw_cache[num as usize].is_surface_liquid;
            }
            return false;
        }
        true
    }

    /// `GetVisibleLiquid(int x, int y)` (LiquidRenderer.cs:825). Read by the map; nothing in the port draws the map yet.
    #[allow(dead_code)]
    pub fn get_visible_liquid(&self, mut x: i32, mut y: i32) -> f32 {
        x -= self.draw_area.x;
        y -= self.draw_area.y;
        if x < 0 || x >= self.draw_area.width || y < 0 || y >= self.draw_area.height {
            return 0.0;
        }
        let num = (x + 2) * (self.draw_area.height + 4) + y + 2;
        if !self.cache[num as usize].has_visible_liquid {
            return 0.0;
        }
        self.cache[num as usize].visible_liquid_level
    }

    /// `Update(GameTime gameTime)` (LiquidRenderer.cs:841); `elapsed_game_time` is `gameTime.ElapsedGameTime.TotalSeconds` (1/60 at the fixed step).
    pub fn update(&mut self, g: &Game, elapsed_game_time: f64) {
        if !unported!("FocusHelper.PauseLiquidRenderer", false) {
            let mut num = g.main.wind_speed_current * 25.0;
            num = if !(num < 0.0) { num + 6.0 } else { num - 6.0 };
            self.frame_state += num * elapsed_game_time as f32;
            self.waterfall_frame_state += 0.5 * elapsed_game_time as f32;
            if self.frame_state < 0.0 {
                self.frame_state += 16.0;
            }
            self.frame_state %= 16.0;
            self.waterfall_frame_state %= 16.0;
            self.animation_frame = self.frame_state as i32;
            self.waterfall_animation_frame = self.waterfall_frame_state as i32;
        }
    }

    /// `PrepareDraw(Rectangle drawArea)` (LiquidRenderer.cs:860)
    pub fn prepare_draw(&mut self, g: &Game, draw_area: Rectangle) {
        self.internal_prepare_draw(g, draw_area);
    }

    /// `SetWaveMaskData(ref Texture2D texture)` (LiquidRenderer.cs:865), on a CPU stand-in for the texture
    #[allow(dead_code)] // only WaterShaderData (not ported) calls it
    pub fn set_wave_mask_data(&self, texture: &mut Option<WaveMaskTexture>) {
        let create = match texture {
            None => true,
            Some(t) => t.width < self.draw_area.height || t.height < self.draw_area.width,
        };
        if create {
            *texture = Some(WaveMaskTexture { width: self.draw_area.height, height: self.draw_area.width, data: Vec::new() });
        }
        // `texture.SetData(0, new Rectangle(0, 0, _drawArea.Height, _drawArea.Width), _waveMask, 0, _drawArea.Width * _drawArea.Height)`
        let t = texture.as_mut().unwrap();
        t.data = self.wave_mask[..(self.draw_area.width * self.draw_area.height) as usize].to_vec();
    }

    /// `GetCachedDrawArea()` (LiquidRenderer.cs:893)
    #[allow(dead_code)]
    pub fn get_cached_draw_area(&self) -> Rectangle {
        self.draw_area
    }
}

/// `LiquidRenderer.SetShimmerVertexColors(ref VertexColors colors, float opacity, int x, int y)` (LiquidRenderer.cs:745)
pub fn set_shimmer_vertex_colors(g: &Game, colors: &mut VertexColors, opacity: f32, x: i32, y: i32) {
    colors.bottom_left_color = Color::WHITE;
    colors.bottom_right_color = Color::WHITE;
    colors.top_left_color = Color::WHITE;
    colors.top_right_color = Color::WHITE;
    colors.bottom_left_color = colors.bottom_left_color * opacity;
    colors.bottom_right_color = colors.bottom_right_color * opacity;
    colors.top_left_color = colors.top_left_color * opacity;
    colors.top_right_color = colors.top_right_color * opacity;
    let base = |wx: f32, wy: f32| -> Vector4 { terraria_sim::light_support::get_shimmer_base_color(g, wx, wy) };
    colors.bottom_left_color = Color::from_vector4(colors.bottom_left_color.to_vector4() * base(x as f32, (y + 1) as f32));
    colors.bottom_right_color = Color::from_vector4(colors.bottom_right_color.to_vector4() * base((x + 1) as f32, (y + 1) as f32));
    colors.top_left_color = Color::from_vector4(colors.top_left_color.to_vector4() * base(x as f32, y as f32));
    colors.top_right_color = Color::from_vector4(colors.top_right_color.to_vector4() * base((x + 1) as f32, y as f32));
}

/// `LiquidRenderer.SetShimmerVertexColors_Sparkle(ref VertexColors colors, float opacity, int x, int y, bool top)` (LiquidRenderer.cs:732)
pub fn set_shimmer_vertex_colors_sparkle(g: &Game, colors: &mut VertexColors, opacity: f32, x: i32, y: i32, top: bool) {
    colors.bottom_left_color = get_shimmer_glitter_color(g, top, x as f32, (y + 1) as f32);
    colors.bottom_right_color = get_shimmer_glitter_color(g, top, (x + 1) as f32, (y + 1) as f32);
    colors.top_left_color = get_shimmer_glitter_color(g, top, x as f32, y as f32);
    colors.top_right_color = get_shimmer_glitter_color(g, top, (x + 1) as f32, y as f32);
    colors.bottom_left_color = colors.bottom_left_color * opacity;
    colors.bottom_right_color = colors.bottom_right_color * opacity;
    colors.top_left_color = colors.top_left_color * opacity;
    colors.top_right_color = colors.top_right_color * opacity;
}

/// `LiquidRenderer.GetShimmerGlitterColor(bool top, float worldPositionX, float worldPositionY)` (LiquidRenderer.cs:766)
pub fn get_shimmer_glitter_color(g: &Game, top: bool, world_position_x: f32, world_position_y: f32) -> Color {
    let hue = (((world_position_x + world_position_y / 6.0) as f64 + g.main.time_for_visual_effects / 30.0) / 6.0) as f32 % 1.0;
    let mut color = crate::main_draw::hsl_to_rgb(hue, 1.0, 0.5, 255);
    color.set_a(0);
    Color::from_vector4(color.to_vector4() * get_shimmer_glitter_opacity(g, top, world_position_x, world_position_y))
}

/// `LiquidRenderer.GetShimmerGlitterOpacity(bool top, float worldPositionX, float worldPositionY)` (LiquidRenderer.cs:773)
pub fn get_shimmer_glitter_opacity(g: &Game, top: bool, world_position_x: f32, world_position_y: f32) -> f32 {
    if top {
        return 0.5;
    }
    let num = utils_remap(
        (((((world_position_x + world_position_y / 6.0) / 10.0) as f64 - g.main.time_for_visual_effects / 360.0) * 6.2831854820251465_f64).sin()) as f32,
        -0.5,
        1.0,
        0.0,
        0.35,
    );
    let num2 = (((simple_white_noise(world_position_x as u32, world_position_y as u32) as f32 / 10.0) as f64) + g.main.time_for_visual_effects / 180.0).sin() as f32;
    utils_remap(num * num2, 0.0, 0.5, 0.0, 1.0)
}

/// `LiquidRenderer.SimpleWhiteNoise(uint x, uint y)` (LiquidRenderer.cs:784)
fn simple_white_noise(mut x: u32, mut y: u32) -> u32 {
    x = 36469u32.wrapping_mul(x & 0xFFFF).wrapping_add(x >> 16);
    y = 18012u32.wrapping_mul(y & 0xFFFF).wrapping_add(y >> 16);
    (x << 16).wrapping_add(y)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::draw_context::test_support::with_ctx;
    use terraria_core::Tile;

    /// A 3-tile-deep pool: the prepared draw cache has visible liquid exactly where the tiles have liquid, with a surface row.
    #[test]
    fn prepare_draw_marks_the_pool_visible_and_finds_its_surface() {
        with_ctx(|ctx| {
            for x in 20..30 {
                for y in 50..53 {
                    let mut t = Tile::default();
                    t.liquid = 255;
                    *ctx.g.main.tile.get_mut(x, y) = t;
                }
            }
            let mut r = LiquidRenderer::new();
            r.prepare_draw(ctx.g, Rectangle::new(10, 40, 30, 20));
            // draw cache index: (x - 10) * 20 + (y - 40)
            let at = |x: i32, y: i32| r.draw_cache[((x - 10) * 20 + (y - 40)) as usize];
            assert!(at(25, 51).is_visible && at(25, 52).is_visible);
            assert!(!at(25, 45).is_visible && !at(15, 51).is_visible);
            // the top row of the pool uses the surface frame (x offset 16: a middle piece) and is surface liquid (worldSurface is 0 here);
            // the rows below it are full water; the pool's left edge uses the left-edge frame (x offset 0)
            assert_eq!(at(25, 50).source_rectangle.x, 16);
            assert!(at(25, 50).is_surface_liquid && !at(25, 51).is_surface_liquid);
            assert!(!r.has_full_water(25, 50) && r.has_full_water(25, 51));
            assert_eq!(at(20, 51).source_rectangle.x, 0);
        });
    }

    #[test]
    fn simple_white_noise_matches_the_formula() {
        assert_eq!(simple_white_noise(0, 0), 0);
        assert_eq!(simple_white_noise(1, 2), ((36469u32) << 16) + 18012 * 2);
    }
}
