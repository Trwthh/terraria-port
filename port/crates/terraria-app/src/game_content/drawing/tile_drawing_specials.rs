//! The part of `TileDrawing` (TileDrawing.cs) that draws what `Draw` recorded with `AddSpecialPoint`: trees, wind-swayed grass
//! and plants, vines, and the pylon / trophy / lens specials (TileDrawing.cs:419-447 `PostDrawTiles`, 7683-8032 leaf emission and
//! wind cycle, 8198-9852 the `Draw*` consumers).
//!
//! Split out of `tile_drawing.rs` only because of its size; it is a method set of the same class. The statement order is the
//! original's, local variables are the decompiler's names in snake case.

use terraria_core::xna::{Color, Point, Rectangle, Vector2};
use terraria_core::Tile;
use terraria_data::sets::tile_id::sets as tile_sets;
use terraria_data::sets::wall_id::sets as wall_sets;
use terraria_sim::unported;
use terraria_sim::world_gen::tile_queries::{does_wind_blow_at_this_height, in_a_place_with_wind, is_below_a_non_hammered_platform, solid_tile, solid_tile_tile};
use terraria_sim::world_gen::tree_foliage::{
    get_ash_tree_foliage_data, get_common_tree_foliage_data, get_gem_tree_foliage_data, get_tree_frame, get_tree_leaf, get_vanity_tree_foliage_data, GetTreeFoliageDataMethod,
    TreeFoliageOut,
};

use crate::data_structures::sprite_batch_beginner::SpriteBatchBeginner;
use crate::data_structures::tile_draw_info::TileDrawInfo;
use crate::draw_context::DrawContext;
use crate::gfx::{BlendState, SamplerState, SpriteEffects, Texture2D};
use crate::main_draw::{hsl_to_rgb, transform};
use crate::pending::{lighting_get_color, utils_get_lerp_value, world_gen_is_alchemy_plant_harvestable};

use super::nature_renderer::SideFlags;
use super::tile_drawing::{get_palm_tree_biome, TileCounterType, TileDrawing};
use super::tile_drawing_flames::TileFlameData;

/// `Utils.RandomInt(ref ulong seed, int min, int max)` and the generator under it (Utils.cs:2314-2349)
pub fn utils_random_next_seed(seed: u64) -> u64 {
    seed.wrapping_mul(25214903917).wrapping_add(11) & 0xFFFF_FFFF_FFFF
}

/// `Utils.RandomNext(ref ulong seed, int bits)` (Utils.cs:2314)
fn utils_random_next(seed: &mut u64, bits: i32) -> i32 {
    *seed = utils_random_next_seed(*seed);
    (*seed >> (48 - bits)) as i32
}

/// `Utils.RandomInt(ref ulong seed, int max)` (Utils.cs:2330)
pub fn utils_random_int_max(seed: &mut u64, max: i32) -> i32 {
    if (max & max.wrapping_neg()) == max {
        return ((max as i64).wrapping_mul(utils_random_next(seed, 31) as i64) >> 31) as i32;
    }
    loop {
        let num = utils_random_next(seed, 31);
        let num2 = num % max;
        if num.wrapping_sub(num2).wrapping_add(max - 1) >= 0 {
            return num2;
        }
    }
}

/// `Utils.RandomInt(ref ulong seed, int min, int max)` (Utils.cs:2347)
pub fn utils_random_int(seed: &mut u64, min: i32, max: i32) -> i32 {
    utils_random_int_max(seed, max - min) + min
}

/// `Main.spriteBatch.Draw(Texture2D texture, Vector2 position, Rectangle? sourceRectangle, Color color, float rotation,
/// Vector2 origin, float scale, SpriteEffects effects, float layerDepth)`
#[allow(clippy::too_many_arguments)]
fn sprite_batch_draw(ctx: &mut DrawContext, texture: Option<&Texture2D>, position: Vector2, source: Rectangle, color: Color, rotation: f32, origin: Vector2, scale: f32, effects: SpriteEffects) {
    if let Some(t) = texture {
        ctx.sprite_batch.draw(t, position, Some(source), color, rotation, origin, Vector2::new(scale, scale), effects);
    }
}

/// `Utils.ToRotationVector2(this float f)`
fn to_rotation_vector2(f: f32) -> Vector2 {
    Vector2::new((f as f64).cos() as f32, (f as f64).sin() as f32)
}

/// `Point.ToWorldCoordinates(float autoAddX = 8f, float autoAddY = 8f)` (Utils.cs:1781)
fn point_to_world_coordinates(p: Point, auto_add_x: f32, auto_add_y: f32) -> Vector2 {
    Vector2::new(p.x as f32, p.y as f32) * 16.0_f32 + Vector2::new(auto_add_x, auto_add_y)
}

/// `Texture2D.Frame(int horizontalFrames, int verticalFrames, int frameX, int frameY)` (Utils.cs:1361)
fn texture_frame(tex: &Texture2D, horizontal_frames: i32, vertical_frames: i32, frame_x: i32, frame_y: i32) -> Rectangle {
    let num = tex.width / horizontal_frames;
    let num2 = tex.height / vertical_frames;
    Rectangle::new(num * frame_x, num2 * frame_y, num, num2)
}

/// `Main.tile[x, y]`: the original throws outside the world; the drawing loops that read tiles this way sit inside the
/// per-tile `try { } catch { }` of the caller or only reach tiles the draw pass already visited, so a missing tile is an empty one.
fn tile_at(ctx: &DrawContext, x: i32, y: i32) -> Tile {
    ctx.g.main.tile.try_get(x, y).unwrap_or_default()
}

impl TileDrawing {
    /// `PostDrawTiles(bool solidLayer)` (TileDrawing.cs:419)
    pub fn post_draw_tiles(&mut self, ctx: &mut DrawContext, solid_layer: bool) {
        if !solid_layer {
            let beginner = SpriteBatchBeginner::new(BlendState::AlphaBlend, SamplerState::PointClamp, transform());
            beginner.begin(ctx.sprite_batch);
            self.draw_multi_tile_vines(ctx);
            self.draw_multi_tile_grass(ctx);
            self.draw_void_lenses(ctx);
            self.draw_teleportation_pylons(ctx);
            self.draw_master_trophies(ctx);
            self.draw_grass(ctx);
            self.draw_any_directional_grass(ctx);
            self.draw_trees(ctx);
            self.draw_vines(ctx);
            self.draw_reverse_vines(ctx);
            ctx.sprite_batch.end(ctx.list);
            let mut nature_renderer = std::mem::take(&mut self.nature_renderer);
            nature_renderer.draw_after_all_objects(ctx, &beginner);
            self.nature_renderer = nature_renderer;
        }
        if solid_layer {
            unported!("TileDrawing.DrawEntities_HatRacks");
            unported!("TileDrawing.DrawEntities_DisplayDolls");
        }
    }

    /// The `GetTileDrawData(x, y, tile, type, ref tileFrameX, ref tileFrameY, out ...)` call of the `Draw*` consumers: the
    /// `ref` / `out` parameters are the fields of the returned `TileDrawInfo` of the same names (as `get_tile_draw_data` has them).
    fn get_tile_draw_data_info(&mut self, ctx: &mut DrawContext, x: i32, y: i32, tile: &Tile, type_: u16) -> TileDrawInfo {
        let mut d = TileDrawInfo::new();
        d.tile_cache = *tile;
        d.type_cache = type_;
        d.tile_frame_x = tile.frame_x;
        d.tile_frame_y = tile.frame_y;
        self.get_tile_draw_data(ctx, x, y, tile, type_, &mut d);
        d
    }

    /// `GetWindCycle(int x, int y, double windCounter)` (TileDrawing.cs:7814)
    pub(super) fn get_wind_cycle(&self, ctx: &DrawContext, x: i32, y: i32, wind_counter: f64) -> f32 {
        if !ctx.state.settings_enabled_tiles_sway_in_wind {
            return 0.0;
        }
        let num = x as f32 * 0.5 + (y / 100) as f32 * 0.5;
        let mut num2 = (wind_counter * 6.2831854820251465 + num as f64).cos() as f32 * 0.5;
        if ctx.g.main.remix_world {
            if !((y as f64) > ctx.g.main.world_surface) {
                return 0.0;
            }
            num2 += ctx.g.main.wind_for_visuals();
        } else {
            if !((y as f64) < ctx.g.main.world_surface) {
                return 0.0;
            }
            num2 += ctx.g.main.wind_for_visuals();
        }
        let lerp_value = utils_get_lerp_value(0.08, 0.18, ctx.g.main.wind_for_visuals().abs(), true);
        num2 * lerp_value
    }

    /// `UpdateLeafFrequency()` (TileDrawing.cs:7859)
    pub(super) fn update_leaf_frequency(&mut self, ctx: &DrawContext) {
        let num = ctx.g.main.wind_for_visuals().abs();
        if num <= 0.1 {
            self.leaf_frequency = 2000;
        } else if num <= 0.2 {
            self.leaf_frequency = 1000;
        } else if num <= 0.3 {
            self.leaf_frequency = 450;
        } else if num <= 0.4 {
            self.leaf_frequency = 300;
        } else if num <= 0.5 {
            self.leaf_frequency = 200;
        } else if num <= 0.6 {
            self.leaf_frequency = 130;
        } else if num <= 0.7 {
            self.leaf_frequency = 75;
        } else if num <= 0.8 {
            self.leaf_frequency = 50;
        } else if num <= 0.9 {
            self.leaf_frequency = 40;
        } else if num <= 1.0 {
            self.leaf_frequency = 30;
        } else if num <= 1.1 {
            self.leaf_frequency = 20;
        } else {
            self.leaf_frequency = 10;
        }
        self.leaf_frequency *= 7;
    }

    /// `EmitLivingTreeLeaf(int i, int j, int leafGoreType)` (TileDrawing.cs:7683). Called by `DrawTiles_EmitParticles`.
    #[allow(dead_code)]
    pub(super) fn emit_living_tree_leaf(&mut self, ctx: &mut DrawContext, i: i32, j: i32, leaf_gore_type: i32) {
        self.emit_living_tree_leaf_below(ctx, i, j, leaf_gore_type);
        if self.rand.next(2) == 0 {
            self.emit_living_tree_leaf_sideways(ctx, i, j, leaf_gore_type);
        }
    }

    /// `EmitLivingTreeLeaf_Below(int x, int y, int leafGoreType)` (TileDrawing.cs:7692)
    #[allow(dead_code)]
    fn emit_living_tree_leaf_below(&mut self, ctx: &mut DrawContext, x: i32, y: i32, _leaf_gore_type: i32) {
        let tile = tile_at(ctx, x, y + 1);
        if !solid_tile_tile(ctx.g, &tile) && tile.liquid <= 0 {
            let wind_for_visuals = ctx.g.main.wind_for_visuals();
            if (!(wind_for_visuals < -0.2) || (!solid_tile_tile(ctx.g, &tile_at(ctx, x - 1, y + 1)) && !solid_tile_tile(ctx.g, &tile_at(ctx, x - 2, y + 1))))
                && (!(wind_for_visuals > 0.2) || (!solid_tile_tile(ctx.g, &tile_at(ctx, x + 1, y + 1)) && !solid_tile_tile(ctx.g, &tile_at(ctx, x + 2, y + 1))))
            {
                unported!("Gore.NewGorePerfect"); // (x * 16, y * 16 + 16), no velocity, leafGoreType; Frame.CurrentColumn = Main.tile[x, y].color()
            }
        }
    }

    /// `EmitLivingTreeLeaf_Sideways(int x, int y, int leafGoreType)` (TileDrawing.cs:7705)
    #[allow(dead_code)]
    fn emit_living_tree_leaf_sideways(&mut self, ctx: &mut DrawContext, x: i32, y: i32, _leaf_gore_type: i32) {
        let mut num = 0;
        if ctx.g.main.wind_for_visuals() > 0.2 {
            num = 1;
        } else if ctx.g.main.wind_for_visuals() < -0.2 {
            num = -1;
        }
        let tile = tile_at(ctx, x + num, y);
        if !solid_tile_tile(ctx.g, &tile) && tile.liquid <= 0 {
            let mut num2 = 0;
            if num == -1 {
                num2 = -10;
            }
            let _position = Vector2::new((x * 16 + 8 + 4 * num + num2) as f32, (y * 16 + 8) as f32);
            unported!("Gore.NewGorePerfect"); // _position, no velocity, leafGoreType; Frame.CurrentColumn = Main.tile[x, y].color()
        }
    }

    /// `EmitTreeLeaves(int tilePosX, int tilePosY, int grassPosX, int grassPosY)` (TileDrawing.cs:7919)
    fn emit_tree_leaves(&mut self, ctx: &mut DrawContext, tile_pos_x: i32, tile_pos_y: i32, grass_pos_x: i32, grass_pos_y: i32) {
        if !self.is_active_and_not_paused {
            return;
        }
        let mut tree_height = grass_pos_y - tile_pos_y;
        let tile = tile_at(ctx, tile_pos_x, tile_pos_y);
        if tile.liquid > 0 {
            return;
        }
        let grass_tile = tile_at(ctx, grass_pos_x, grass_pos_y);
        let (_tree_frame, pass_style) = get_tree_leaf(ctx.g, tile_pos_x, &tile, &grass_tile, &mut tree_height);
        let num = match pass_style {
            -1 | 912 | 913 | 1278 => return,
            917..=925 => 1,
            _ => {
                if pass_style >= 1113 && pass_style <= 1121 {
                    1
                } else {
                    0
                }
            }
        };
        let flag = (num as u8) != 0;
        let mut num2 = self.leaf_frequency;
        let flag2 = tile_pos_x - grass_pos_x != 0;
        if flag {
            num2 /= 2;
        }
        if !does_wind_blow_at_this_height(ctx.g, tile_pos_y) {
            num2 = 10000;
        }
        if flag2 {
            num2 *= 3;
        }
        if self.rand.next(num2) != 0 {
            return;
        }
        let _num3 = 2;
        let mut vector = Vector2::new((tile_pos_x * 16 + 8) as f32, (tile_pos_y * 16 + 8) as f32);
        if flag2 {
            let num4 = tile_pos_x - grass_pos_x;
            vector.x += (num4 * 12) as f32;
            let mut num5 = 0;
            if tile.frame_y == 220 {
                num5 = 1;
            } else if tile.frame_y == 242 {
                num5 = 2;
            }
            if tile.frame_x == 66 {
                match num5 {
                    0 => vector += Vector2::new(0.0, -6.0),
                    1 => vector += Vector2::new(0.0, -6.0),
                    2 => vector += Vector2::new(0.0, 8.0),
                    _ => {}
                }
            } else {
                match num5 {
                    0 => vector += Vector2::new(0.0, 4.0),
                    1 => vector += Vector2::new(2.0, -6.0),
                    2 => vector += Vector2::new(6.0, -6.0),
                    _ => {}
                }
            }
        } else {
            vector += Vector2::new(-16.0, -16.0);
            if flag {
                vector.y -= (ctx.g.main.rand.next_range(0, 28) * 4) as f32;
            }
        }
        // `vector.ToTileCoordinates()`: (int)(x / 16), (int)(y / 16)
        if !solid_tile(ctx.g, (vector.x / 16.0) as i32, (vector.y / 16.0) as i32, false) {
            // Gore.NewGoreDirect(vector, Utils.RandomVector2(Main.rand, -num3, num3), passStyle, 0.7f + Main.rand.NextFloat() * 0.6f)
            //     .Frame.CurrentColumn = Main.tile[tilePosX, tilePosY].color();
            unported!("Gore.NewGoreDirect");
        }
    }

    /// `DrawTrees()` (TileDrawing.cs:8198). The `try { } catch { }` around the body of each iteration swallows the exceptions of
    /// out-of-world reads; the `continue`s inside it are plain `continue`s.
    fn draw_trees(&mut self, ctx: &mut DrawContext) {
        let unscaled_position = Vector2::new(ctx.g.main.screen_position.0, ctx.g.main.screen_position.1);
        let zero = Vector2::ZERO;
        let num = 0usize;
        let num2 = self.specials_count[num];
        let num3 = 0.08_f32;
        let num4 = 0.06_f32;
        for i in 0..num2 {
            let point = self.special_positions[num][i as usize];
            let x = point.x;
            let y = point.y;
            let Some(tile) = ctx.g.main.tile.try_get(x, y) else {
                continue;
            };
            if !tile.active() {
                continue;
            }
            let type_ = tile.type_;
            let frame_x = tile.frame_x;
            let frame_y = tile.frame_y;
            let flag = tile.wall > 0;
            let mut get_tree_foliage_data_method: Option<GetTreeFoliageDataMethod> = None;
            let mut flag2 = false;
            match type_ {
                5 => {
                    flag2 = true;
                    get_tree_foliage_data_method = Some(get_common_tree_foliage_data);
                }
                583 | 584 | 585 | 586 | 587 | 588 | 589 => {
                    flag2 = true;
                    get_tree_foliage_data_method = Some(get_gem_tree_foliage_data);
                }
                596 | 616 => {
                    flag2 = true;
                    get_tree_foliage_data_method = Some(get_vanity_tree_foliage_data);
                }
                634 => {
                    flag2 = true;
                    get_tree_foliage_data_method = Some(get_ash_tree_foliage_data);
                }
                _ => {}
            }
            if flag2 && frame_y >= 198 && frame_x >= 22 {
                let mut tree_frame = get_tree_frame(&tile);
                let get_tree_foliage_data_method = get_tree_foliage_data_method.expect("set with flag2");
                match frame_x {
                    22 => {
                        let mut tree_style3 = 0;
                        let num13 = 0;
                        let grass_pos_x = x + num13;
                        let mut foliage = TreeFoliageOut { floor_y: y, top_texture_frame_width: 80, top_texture_frame_height: 80 };
                        if !get_tree_foliage_data_method(ctx.g, x, y, num13, &mut tree_frame, &mut tree_style3, &mut foliage) {
                            continue;
                        }
                        let floor_y3 = foliage.floor_y;
                        let top_texture_frame_width3 = foliage.top_texture_frame_width;
                        let top_texture_frame_height3 = foliage.top_texture_frame_height;
                        self.emit_tree_leaves(ctx, x, y, grass_pos_x, floor_y3);
                        if tree_style3 == 14 {
                            let mut num14 = self.rand.next_range(28, 42) as f32 * 0.005;
                            num14 += (270 - ctx.g.main.mouse_text_color as i32) as f32 / 1000.0;
                            if tile.color() == 0 {
                                terraria_sim::lighting::add_light(ctx.g, x, y, 0.1, 0.2 + num14 / 2.0, 0.7 + num14);
                            } else {
                                let color5 = terraria_sim::light_support::paint_color(tile.color() as i32);
                                let r3 = color5.r() as f32 / 255.0;
                                let g3 = color5.g() as f32 / 255.0;
                                let b3 = color5.b() as f32 / 255.0;
                                terraria_sim::lighting::add_light(ctx.g, x, y, r3, g3, b3);
                            }
                        }
                        let tile_color3 = tile.color();
                        let tree_top_texture = self.get_tree_top_texture(ctx, tree_style3, 0, tile_color3);
                        let mut position3 = Vector2::new((x * 16 - unscaled_position.x as i32 + 8) as f32, (y * 16 - unscaled_position.y as i32 + 16) as f32) + zero;
                        let mut num15 = 0.0_f32;
                        if !flag {
                            num15 = self.get_wind_cycle(ctx, x, y, self.tree_wind_counter);
                        }
                        position3.x += num15 * 2.0;
                        position3.y += num15.abs() * 2.0;
                        let mut color6 = lighting_get_color(ctx.g, x, y);
                        if tile.fullbright_block() {
                            color6 = Color::WHITE;
                        }
                        let source = Rectangle::new(tree_frame * (top_texture_frame_width3 + 2), 0, top_texture_frame_width3, top_texture_frame_height3);
                        let origin = Vector2::new((top_texture_frame_width3 / 2) as f32, top_texture_frame_height3 as f32);
                        self.nature_renderer.draw_nature(tree_top_texture.as_ref(), position3, source, color6, num15 * num3, origin, 1.0, SpriteEffects::NONE, 0.0, SideFlags::NONE);
                        if type_ == 634 {
                            let value3 = ctx.glow_mask(316);
                            let white3 = Color::WHITE;
                            self.nature_renderer.draw_glowmask(value3.as_ref(), position3, Some(source), white3, num15 * num3, origin, 1.0, SpriteEffects::NONE, 0.0);
                        }
                    }
                    44 => {
                        let mut tree_style2 = 0;
                        let num9 = x;
                        let num10 = 1;
                        let mut foliage = TreeFoliageOut { floor_y: y, top_texture_frame_width: 80, top_texture_frame_height: 80 };
                        if !get_tree_foliage_data_method(ctx.g, x, y, num10, &mut tree_frame, &mut tree_style2, &mut foliage) {
                            continue;
                        }
                        let floor_y2 = foliage.floor_y;
                        self.emit_tree_leaves(ctx, x, y, num9 + num10, floor_y2);
                        if tree_style2 == 14 {
                            let mut num11 = self.rand.next_range(28, 42) as f32 * 0.005;
                            num11 += (270 - ctx.g.main.mouse_text_color as i32) as f32 / 1000.0;
                            if tile.color() == 0 {
                                terraria_sim::lighting::add_light(ctx.g, x, y, 0.1, 0.2 + num11 / 2.0, 0.7 + num11);
                            } else {
                                let color3 = terraria_sim::light_support::paint_color(tile.color() as i32);
                                let r2 = color3.r() as f32 / 255.0;
                                let g2 = color3.g() as f32 / 255.0;
                                let b2 = color3.b() as f32 / 255.0;
                                terraria_sim::lighting::add_light(ctx.g, x, y, r2, g2, b2);
                            }
                        }
                        let tile_color2 = tile.color();
                        let tree_branch_texture2 = self.get_tree_branch_texture(ctx, tree_style2, 0, tile_color2);
                        let mut position2 = Vector2::new((x * 16) as f32, (y * 16) as f32) - vector2_floor(unscaled_position) + zero + Vector2::new(16.0, 12.0);
                        let mut num12 = 0.0_f32;
                        if !flag {
                            num12 = self.get_wind_cycle(ctx, x, y, self.tree_wind_counter);
                        }
                        if num12 > 0.0 {
                            position2.x += num12;
                        }
                        position2.x += num12.abs() * 2.0;
                        let mut color4 = lighting_get_color(ctx.g, x, y);
                        if tile.fullbright_block() {
                            color4 = Color::WHITE;
                        }
                        let source = Rectangle::new(0, tree_frame * 42, 40, 40);
                        let origin = Vector2::new(40.0, 24.0);
                        self.nature_renderer.draw_nature(tree_branch_texture2.as_ref(), position2, source, color4, num12 * num4, origin, 1.0, SpriteEffects::NONE, 0.0, SideFlags::NONE);
                        if type_ == 634 {
                            let value2 = ctx.glow_mask(317);
                            let white2 = Color::WHITE;
                            self.nature_renderer.draw_glowmask(value2.as_ref(), position2, Some(source), white2, num12 * num4, origin, 1.0, SpriteEffects::NONE, 0.0);
                        }
                    }
                    66 => {
                        let mut tree_style = 0;
                        let num5 = x;
                        let num6 = -1;
                        let mut foliage = TreeFoliageOut { floor_y: y, top_texture_frame_width: 80, top_texture_frame_height: 80 };
                        if !get_tree_foliage_data_method(ctx.g, x, y, num6, &mut tree_frame, &mut tree_style, &mut foliage) {
                            continue;
                        }
                        let floor_y = foliage.floor_y;
                        self.emit_tree_leaves(ctx, x, y, num5 + num6, floor_y);
                        if tree_style == 14 {
                            let mut num7 = self.rand.next_range(28, 42) as f32 * 0.005;
                            num7 += (270 - ctx.g.main.mouse_text_color as i32) as f32 / 1000.0;
                            if tile.color() == 0 {
                                terraria_sim::lighting::add_light(ctx.g, x, y, 0.1, 0.2 + num7 / 2.0, 0.7 + num7);
                            } else {
                                let color = terraria_sim::light_support::paint_color(tile.color() as i32);
                                let r = color.r() as f32 / 255.0;
                                let g = color.g() as f32 / 255.0;
                                let b = color.b() as f32 / 255.0;
                                terraria_sim::lighting::add_light(ctx.g, x, y, r, g, b);
                            }
                        }
                        let tile_color = tile.color();
                        let tree_branch_texture = self.get_tree_branch_texture(ctx, tree_style, 0, tile_color);
                        let mut position = Vector2::new((x * 16) as f32, (y * 16) as f32) - vector2_floor(unscaled_position) + zero + Vector2::new(0.0, 18.0);
                        let mut num8 = 0.0_f32;
                        if !flag {
                            num8 = self.get_wind_cycle(ctx, x, y, self.tree_wind_counter);
                        }
                        if num8 < 0.0 {
                            position.x += num8;
                        }
                        position.x -= num8.abs() * 2.0;
                        let mut color2 = lighting_get_color(ctx.g, x, y);
                        if tile.fullbright_block() {
                            color2 = Color::WHITE;
                        }
                        let source = Rectangle::new(42, tree_frame * 42, 40, 40);
                        let origin = Vector2::new(0.0, 30.0);
                        self.nature_renderer.draw_nature(tree_branch_texture.as_ref(), position, source, color2, num8 * num4, origin, 1.0, SpriteEffects::NONE, 0.0, SideFlags::NONE);
                        if type_ == 634 {
                            let value = ctx.glow_mask(317);
                            let white = Color::WHITE;
                            self.nature_renderer.draw_glowmask(value.as_ref(), position, Some(source), white, num8 * num4, origin, 1.0, SpriteEffects::NONE, 0.0);
                        }
                    }
                    _ => {}
                }
            }
            if type_ == 323 && frame_x >= 88 && frame_x <= 132 {
                let mut num16 = 0;
                match frame_x {
                    110 => num16 = 1,
                    132 => num16 = 2,
                    _ => {}
                }
                let mut tree_texture_index = 15;
                let mut num17 = 80;
                let mut num18 = 80;
                let mut num19 = 32;
                let mut num20 = 0;
                let palm_tree_biome = get_palm_tree_biome(ctx.g, x, y);
                let mut y2 = palm_tree_biome * 82;
                if palm_tree_biome >= 4 && palm_tree_biome <= 7 {
                    tree_texture_index = 21;
                    num17 = 114;
                    num18 = 98;
                    y2 = (palm_tree_biome - 4) * 98;
                    num19 = 48;
                    num20 = 2;
                }
                let frame_y2 = tile_at(ctx, x, y).frame_y as i32;
                let tile_color4 = tile.color();
                let tree_top_texture2 = self.get_tree_top_texture(ctx, tree_texture_index, palm_tree_biome, tile_color4);
                let mut position4 = Vector2::new((x * 16 - unscaled_position.x as i32 - num19 + frame_y2 + num17 / 2) as f32, (y * 16 - unscaled_position.y as i32 + 16 + num20) as f32) + zero;
                let mut num21 = 0.0_f32;
                if !flag {
                    num21 = self.get_wind_cycle(ctx, x, y, self.tree_wind_counter);
                }
                position4.x += num21 * 2.0;
                position4.y += num21.abs() * 2.0;
                let mut color7 = lighting_get_color(ctx.g, x, y);
                if tile.fullbright_block() {
                    color7 = Color::WHITE;
                }
                self.nature_renderer.draw_nature(
                    tree_top_texture2.as_ref(),
                    position4,
                    Rectangle::new(num16 * (num17 + 2), y2, num17, num18),
                    color7,
                    num21 * num3,
                    Vector2::new((num17 / 2) as f32, num18 as f32),
                    1.0,
                    SpriteEffects::NONE,
                    0.0,
                    SideFlags::NONE,
                );
            }
        }
    }

    /// `GetTreeTopTexture(int treeTextureIndex, int treeTextureStyle, byte tileColor)` (TileDrawing.cs:8474)
    fn get_tree_top_texture(&mut self, ctx: &mut DrawContext, tree_texture_index: i32, _tree_texture_style: i32, _tile_color: u8) -> Option<Texture2D> {
        // `_paintSystem.TryGetTreeTopAndRequestIfNotReady(...)` renders the painted variant into a target, which is never ready here
        let texture2_d: Option<Texture2D> = unported!("TilePaintSystemV2.TryGetTreeTopAndRequestIfNotReady", None);
        if texture2_d.is_some() {
            return texture2_d;
        }
        ctx.tree_top(tree_texture_index)
    }

    /// `GetTreeBranchTexture(int treeTextureIndex, int treeTextureStyle, byte tileColor)` (TileDrawing.cs:8484)
    fn get_tree_branch_texture(&mut self, ctx: &mut DrawContext, tree_texture_index: i32, _tree_texture_style: i32, _tile_color: u8) -> Option<Texture2D> {
        let texture2_d: Option<Texture2D> = unported!("TilePaintSystemV2.TryGetTreeBranchAndRequestIfNotReady", None);
        if texture2_d.is_some() {
            return texture2_d;
        }
        ctx.tree_branch(tree_texture_index)
    }

    /// `DrawGrass()` (TileDrawing.cs:8494)
    fn draw_grass(&mut self, ctx: &mut DrawContext) {
        let unscaled_position = Vector2::new(ctx.g.main.screen_position.0, ctx.g.main.screen_position.1);
        let zero = Vector2::ZERO;
        let num = 1usize;
        let num2 = self.specials_count[num];
        for i in 0..num2 {
            let point = self.special_positions[num][i as usize];
            let x = point.x;
            let y = point.y;
            let Some(tile) = ctx.g.main.tile.try_get(x, y) else {
                continue;
            };
            if !tile.active() || !self.is_visible(&tile) {
                continue;
            }
            let mut type_ = tile.type_;
            let d = self.get_tile_draw_data_info(ctx, x, y, &tile, type_);
            let (tile_frame_x, tile_frame_y) = (d.tile_frame_x, d.tile_frame_y);
            let flag = self.rand.next(4) == 0;
            let mut tile_light = lighting_get_color(ctx.g, x, y);
            self.draw_animated_tile_adjust_for_vision_changers(ctx, x, y, &tile, type_, tile_frame_x, tile_frame_y, &mut tile_light, flag);
            tile_light = self.draw_tiles_get_light_override(ctx, y, x, &tile, type_, tile_frame_x, tile_frame_y, tile_light);
            if self.is_active_and_not_paused && flag {
                self.draw_tiles_emit_particles(ctx, y, x, &tile, type_, tile_frame_x, tile_frame_y, tile_light);
            }
            if type_ == 83 && world_gen_is_alchemy_plant_harvestable(ctx.g, tile_frame_x as i32 / 18, y) {
                type_ = 84;
                ctx.tile_texture(type_ as i32); // Main.instance.LoadTiles(type)
            }
            let mut position = Vector2::new((x * 16 - unscaled_position.x as i32 + 8) as f32, (y * 16 - unscaled_position.y as i32 + 16) as f32) + zero;
            let mut num3 = self.get_wind_cycle(ctx, x, y, self.grass_wind_counter);
            if !wall_sets::ALLOWS_WIND[tile.wall as usize] {
                num3 = 0.0;
            }
            if !in_a_place_with_wind(ctx.g, x, y, 1, 1) {
                num3 = 0.0;
            }
            num3 += self.get_wind_grid_push(x, y, 20, 0.35);
            position.x += num3 * 1.0;
            position.y += num3.abs() * 1.0;
            let tile_draw_texture = self.get_tile_draw_texture(ctx, &tile, x, y);
            if tile_draw_texture.is_some() {
                let origin = Vector2::new((d.tile_width / 2) as f32, (16 - d.half_brick_height - d.tile_top) as f32);
                let source = Rectangle::new(tile_frame_x as i32 + d.add_fr_x, tile_frame_y as i32 + d.add_fr_y, d.tile_width, d.tile_height - d.half_brick_height);
                self.nature_renderer.draw_nature(tile_draw_texture.as_ref(), position, source, tile_light, num3 * 0.1, origin, 1.0, d.tile_sprite_effect, 0.0, SideFlags::NONE);
                if d.glow_texture.is_some() {
                    self.nature_renderer.draw_glowmask(d.glow_texture.as_ref(), position, Some(d.glow_source_rect), d.glow_color, num3 * 0.1, origin, 1.0, d.tile_sprite_effect, 0.0);
                }
            }
        }
    }

    /// `DrawAnyDirectionalGrass()` (TileDrawing.cs:8552)
    fn draw_any_directional_grass(&mut self, ctx: &mut DrawContext) {
        let unscaled_position = Vector2::new(ctx.g.main.screen_position.0, ctx.g.main.screen_position.1);
        let zero = Vector2::ZERO;
        let num = 10usize;
        let num2 = self.specials_count[num];
        for i in 0..num2 {
            let point = self.special_positions[num][i as usize];
            let x = point.x;
            let y = point.y;
            let Some(tile) = ctx.g.main.tile.try_get(x, y) else {
                continue;
            };
            if !tile.active() || !self.is_visible(&tile) {
                continue;
            }
            let mut type_ = tile.type_;
            let d = self.get_tile_draw_data_info(ctx, x, y, &tile, type_);
            let (tile_frame_x, tile_frame_y) = (d.tile_frame_x, d.tile_frame_y);
            let flag = self.rand.next(4) == 0;
            let mut tile_light = lighting_get_color(ctx.g, x, y);
            self.draw_animated_tile_adjust_for_vision_changers(ctx, x, y, &tile, type_, tile_frame_x, tile_frame_y, &mut tile_light, flag);
            tile_light = self.draw_tiles_get_light_override(ctx, y, x, &tile, type_, tile_frame_x, tile_frame_y, tile_light);
            if self.is_active_and_not_paused && flag {
                self.draw_tiles_emit_particles(ctx, y, x, &tile, type_, tile_frame_x, tile_frame_y, tile_light);
            }
            if type_ == 83 && world_gen_is_alchemy_plant_harvestable(ctx.g, tile_frame_x as i32 / 18, y) {
                type_ = 84;
                ctx.tile_texture(type_ as i32); // Main.instance.LoadTiles(type)
            }
            let mut position = Vector2::new((x * 16 - unscaled_position.x as i32) as f32, (y * 16 - unscaled_position.y as i32) as f32) + zero;
            let mut num3 = self.get_wind_cycle(ctx, x, y, self.grass_wind_counter);
            if !wall_sets::ALLOWS_WIND[tile.wall as usize] {
                num3 = 0.0;
            }
            if !in_a_place_with_wind(ctx.g, x, y, 1, 1) {
                num3 = 0.0;
            }
            let (push_x, push_y) = self.get_wind_grid_push_2_axis(x, y, 20, 0.35);
            let mut num4 = 1;
            let mut num5 = 0;
            let mut origin = Vector2::new((d.tile_width / 2) as f32, (16 - d.half_brick_height - d.tile_top) as f32);
            match tile_frame_y as i32 / 54 {
                0 => {
                    num4 = 1;
                    num5 = 0;
                    origin = Vector2::new((d.tile_width / 2) as f32, (16 - d.half_brick_height - d.tile_top) as f32);
                    position.x += 8.0;
                    position.y += 16.0;
                    position.x += num3;
                    position.y += num3.abs();
                }
                1 => {
                    num3 *= -1.0;
                    num4 = -1;
                    num5 = 0;
                    origin = Vector2::new((d.tile_width / 2) as f32, (-d.tile_top) as f32);
                    position.x += 8.0;
                    position.x += 0.0 - num3;
                    position.y += 0.0 - num3.abs();
                }
                2 => {
                    num4 = 0;
                    num5 = 1;
                    origin = Vector2::new(2.0, ((16 - d.half_brick_height - d.tile_top) / 2) as f32);
                    position.y += 8.0;
                    position.y += num3;
                    position.x += 0.0 - num3.abs();
                }
                3 => {
                    num3 *= -1.0;
                    num4 = 0;
                    num5 = -1;
                    origin = Vector2::new(14.0, ((16 - d.half_brick_height - d.tile_top) / 2) as f32);
                    position.x += 16.0;
                    position.y += 8.0;
                    position.y += 0.0 - num3;
                    position.x += num3.abs();
                }
                _ => {}
            }
            num3 += push_x * num4 as f32 + push_y * num5 as f32;
            let tile_draw_texture = self.get_tile_draw_texture(ctx, &tile, x, y);
            if tile_draw_texture.is_some() {
                let source = Rectangle::new(tile_frame_x as i32 + d.add_fr_x, tile_frame_y as i32 + d.add_fr_y, d.tile_width, d.tile_height - d.half_brick_height);
                self.nature_renderer.draw_nature(tile_draw_texture.as_ref(), position, source, tile_light, num3 * 0.1, origin, 1.0, d.tile_sprite_effect, 0.0, SideFlags::NONE);
                if d.glow_texture.is_some() {
                    self.nature_renderer.draw_glowmask(d.glow_texture.as_ref(), position, Some(source), d.glow_color, num3 * 0.1, origin, 1.0, d.tile_sprite_effect, 0.0);
                }
            }
        }
    }

    /// `DrawAnimatedTile_AdjustForVisionChangers(int i, int j, Tile tileCache, ushort typeCache, short tileFrameX, short tileFrameY,
    /// ref Color tileLight, bool canDoDust)` (TileDrawing.cs:8651). `_perspectivePlayer` (dangerSense, findTreasure, biomeSight) is a
    /// `Player`, which is not ported: every flag reads false (a fresh player has none of the effects).
    #[allow(clippy::too_many_arguments)]
    pub(super) fn draw_animated_tile_adjust_for_vision_changers(&mut self, ctx: &mut DrawContext, i: i32, j: i32, tile_cache: &Tile, type_cache: u16, _tile_frame_x: i16, _tile_frame_y: i16, tile_light: &mut Color, can_do_dust: bool) {
        let _ = (ctx, i, j, tile_cache, type_cache);
        if unported!("Player.dangerSense", false) && unported!("TileDrawing.IsTileDangerous", false) {
            if tile_light.r() < 255 {
                tile_light.set_r(255);
            }
            if tile_light.g() < 50 {
                tile_light.set_g(50);
            }
            if tile_light.b() < 50 {
                tile_light.set_b(50);
            }
            if self.is_active_and_not_paused && can_do_dust && self.rand.next(30) == 0 {
                unported!("Dust.NewDust");
            }
        }
        if unported!("Player.findTreasure", false) && unported!("Main.IsTileSpelunkable", false) {
            if tile_light.r() < 200 {
                tile_light.set_r(200);
            }
            if tile_light.g() < 170 {
                tile_light.set_g(170);
            }
            if self.is_active_and_not_paused && self.rand.next(60) == 0 && can_do_dust {
                unported!("Dust.NewDust");
            }
        }
        if !unported!("Player.biomeSight", false) {
            return;
        }
        let mut sight_color = Color::WHITE;
        if unported!("Main.IsTileBiomeSightable", false) {
            if tile_light.r() < sight_color.r() {
                tile_light.set_r(sight_color.r());
            }
            if tile_light.g() < sight_color.g() {
                tile_light.set_g(sight_color.g());
            }
            if tile_light.b() < sight_color.b() {
                tile_light.set_b(sight_color.b());
            }
            if self.is_active_and_not_paused && can_do_dust && self.rand.next(480) == 0 {
                let _new_color = sight_color;
                sight_color = Color::WHITE;
                let _ = sight_color;
                unported!("Dust.NewDust");
            }
        }
    }

    /// `GetWindGridPush(int i, int j, int pushAnimationTimeTotal, float pushForcePerFrame)` (TileDrawing.cs:8725)
    pub(super) fn get_wind_grid_push(&self, i: i32, j: i32, push_animation_time_total: i32, push_force_per_frame: f32) -> f32 {
        let (wind_time_left, direction_x, _direction_y) = self.wind_grid.get_wind_time(i, j, push_animation_time_total);
        if wind_time_left >= push_animation_time_total / 2 {
            return (push_animation_time_total - wind_time_left) as f32 * push_force_per_frame * direction_x as f32;
        }
        wind_time_left as f32 * push_force_per_frame * direction_x as f32
    }

    /// `GetWindGridPush2Axis(int i, int j, int pushAnimationTimeTotal, float pushForcePerFrame, out float pushX, out float pushY)`
    /// (TileDrawing.cs:8735). Returns `(pushX, pushY)`.
    fn get_wind_grid_push_2_axis(&self, i: i32, j: i32, push_animation_time_total: i32, push_force_per_frame: f32) -> (f32, f32) {
        let (wind_time_left, direction_x, direction_y) = self.wind_grid.get_wind_time(i, j, push_animation_time_total);
        if wind_time_left >= push_animation_time_total / 2 {
            (
                (push_animation_time_total - wind_time_left) as f32 * push_force_per_frame * direction_x as f32,
                (push_animation_time_total - wind_time_left) as f32 * push_force_per_frame * direction_y as f32,
            )
        } else {
            (wind_time_left as f32 * push_force_per_frame * direction_x as f32, wind_time_left as f32 * push_force_per_frame * direction_y as f32)
        }
    }

    /// `GetWindGridPushComplex(int i, int j, int pushAnimationTimeTotal, float totalPushForce, int loops, bool flipDirectionPerLoop)`
    /// (TileDrawing.cs:8750)
    fn get_wind_grid_push_complex(&self, i: i32, j: i32, push_animation_time_total: i32, total_push_force: f32, loops: i32, flip_direction_per_loop: bool) -> f32 {
        let (wind_time_left, mut direction_x, _direction_y) = self.wind_grid.get_wind_time(i, j, push_animation_time_total);
        let num = wind_time_left as f32 / push_animation_time_total as f32;
        let num2 = (num * loops as f32) as i32;
        let num3 = num * loops as f32 % 1.0;
        if flip_direction_per_loop && num2 % 2 == 1 {
            direction_x *= -1;
        }
        if num * loops as f32 % 1.0 > 0.5 {
            return (1.0 - num3) * total_push_force * direction_x as f32 * (loops - num2) as f32;
        }
        num3 * total_push_force * direction_x as f32 * (loops - num2) as f32
    }

    /// `DrawMasterTrophies()` (TileDrawing.cs:8768)
    fn draw_master_trophies(&mut self, ctx: &mut DrawContext) {
        let num = 9usize;
        let num2 = self.specials_count[num];
        for i in 0..num2 {
            let p = self.special_positions[num][i as usize];
            let Some(tile) = ctx.g.main.tile.try_get(p.x, p.y) else {
                continue;
            };
            if !tile.active() {
                continue;
            }
            let Some(tile2) = ctx.g.main.tile.try_get(p.x + 1, p.y + 1) else {
                continue;
            };
            if tile2.active() && self.is_visible(&tile2) {
                let value = ctx.extra(198);
                let frame_y = tile.frame_x as i32 / 54;
                let num3 = tile.frame_y / 72 != 0;
                let horizontal_frames = 1;
                let vertical_frames = 28;
                let Some(value) = value else {
                    continue;
                };
                let rectangle = texture_frame(&value, horizontal_frames, vertical_frames, 0, frame_y);
                let origin = Vector2::new(rectangle.width as f32, rectangle.height as f32) / 2.0_f32;
                let vector = point_to_world_coordinates(p, 24.0, 64.0);
                let num4 = (ctx.g.main.global_time_wrapped_hourly * (std::f32::consts::PI * 2.0) / 5.0).sin();
                let vector2 = vector + Vector2::new(0.0, -40.0) + Vector2::new(0.0, num4 * 4.0);
                let mut color = lighting_get_color(ctx.g, p.x, p.y);
                if tile2.fullbright_block() {
                    color = Color::WHITE;
                }
                let effects = if num3 { SpriteEffects::FLIP_HORIZONTALLY } else { SpriteEffects::NONE };
                let screen_position = Vector2::new(ctx.g.main.screen_position.0, ctx.g.main.screen_position.1);
                sprite_batch_draw(ctx, Some(&value), vector2 - screen_position, rectangle, color, 0.0, origin, 1.0, effects);
                let num5 = (ctx.g.main.global_time_wrapped_hourly * (std::f32::consts::PI * 2.0) / 2.0).sin() * 0.3 + 0.7;
                let mut color2 = color;
                color2.set_a(0);
                color2 = color2 * 0.1_f32 * num5;
                let mut num6 = 0.0_f32;
                while num6 < 1.0 {
                    sprite_batch_draw(ctx, Some(&value), vector2 - screen_position + to_rotation_vector2(std::f32::consts::PI * 2.0 * num6) * (6.0 + num4 * 2.0), rectangle, color2, 0.0, origin, 1.0, effects);
                    num6 += 1.0 / 6.0;
                }
            }
        }
    }

    /// `DrawTeleportationPylons()` (TileDrawing.cs:8812)
    fn draw_teleportation_pylons(&mut self, ctx: &mut DrawContext) {
        let num = 8usize;
        let num2 = self.specials_count[num];
        for i in 0..num2 {
            let p = self.special_positions[num][i as usize];
            let Some(tile) = ctx.g.main.tile.try_get(p.x, p.y) else {
                continue;
            };
            if !tile.active() {
                continue;
            }
            let Some(tile2) = ctx.g.main.tile.try_get(p.x + 1, p.y + 1) else {
                continue;
            };
            if !tile2.active() || !self.is_visible(&tile2) {
                continue;
            }
            let Some(value) = ctx.extra(181) else {
                continue;
            };
            let num3 = tile.frame_x as i32 / 54;
            let num4 = 3;
            let horizontal_frames = num4 + 11;
            let vertical_frames = 8;
            let frame_y = (ctx.state.tile_frame_counter[597] + p.x + p.y) % 64 / 8;
            let rectangle = texture_frame(&value, horizontal_frames, vertical_frames, num4 + num3, frame_y);
            let value2 = texture_frame(&value, horizontal_frames, vertical_frames, 2, frame_y);
            let _ = texture_frame(&value, horizontal_frames, vertical_frames, 0, frame_y);
            let origin = Vector2::new(rectangle.width as f32, rectangle.height as f32) / 2.0_f32;
            let vector = point_to_world_coordinates(p, 24.0, 64.0);
            let num5 = (ctx.g.main.global_time_wrapped_hourly * (std::f32::consts::PI * 2.0) / 5.0).sin();
            let vector2 = vector + Vector2::new(0.0, -40.0) + Vector2::new(0.0, num5 * 4.0);
            let flag = self.rand.next(4) == 0;
            if self.is_active_and_not_paused && flag && self.rand.next(10) == 0 {
                let _dust_box = Rectangle::new((vector2.x - rectangle.width as f32 / 2.0) as i32, (vector2.y - rectangle.height as f32 / 2.0) as i32, rectangle.width, rectangle.height);
                unported!("TeleportPylonsSystem.SpawnInWorldDust");
            }
            let mut value3 = lighting_get_color(ctx.g, p.x, p.y);
            if tile2.fullbright_block() {
                value3 = Color::WHITE;
            }
            value3 = Color::lerp(value3, Color::WHITE, 0.8);
            let screen_position = Vector2::new(ctx.g.main.screen_position.0, ctx.g.main.screen_position.1);
            sprite_batch_draw(ctx, Some(&value), vector2 - screen_position, rectangle, value3 * 0.7_f32, 0.0, origin, 1.0, SpriteEffects::NONE);
            let num6 = (ctx.g.main.global_time_wrapped_hourly * (std::f32::consts::PI * 2.0) / 1.0).sin() * 0.2 + 0.8;
            let color = Color::from_ints(255, 255, 255, 0) * 0.1_f32 * num6;
            let mut num7 = 0.0_f32;
            while num7 < 1.0 {
                sprite_batch_draw(ctx, Some(&value), vector2 - screen_position + to_rotation_vector2(std::f32::consts::PI * 2.0 * num7) * (6.0 + num5 * 2.0), rectangle, color, 0.0, origin, 1.0, SpriteEffects::NONE);
                num7 += 1.0 / 6.0;
            }
            let mut num8 = 0;
            let actually_selected = false;
            if unported!("Main.InSmartCursorHighlightArea", false) {
                num8 = 1;
                if actually_selected {
                    num8 = 2;
                }
            }
            if num8 != 0 {
                let num9 = (value3.r() as i32 + value3.g() as i32 + value3.b() as i32) / 3;
                if num9 > 10 {
                    let selection_glow_color = unported!("Colors.GetSelectionGlowColor", Color::TRANSPARENT);
                    sprite_batch_draw(ctx, Some(&value), vector2 - screen_position, value2, selection_glow_color, 0.0, origin, 1.0, SpriteEffects::NONE);
                }
            }
        }
    }

    /// `DrawVoidLenses()` (TileDrawing.cs:8882). `VoidLensHelper` (the lens animation and its `DrawData`) is not ported.
    fn draw_void_lenses(&mut self, ctx: &mut DrawContext) {
        let num = 6usize;
        let num2 = self.specials_count[num];
        // `_voidLensData.Clear()`: the list stays empty without the helper
        for i in 0..num2 {
            let p = self.special_positions[num][i as usize];
            let _ = p;
            unported!("VoidLensHelper"); // new VoidLensHelper(p.ToWorldCoordinates(), 1f); Update() unless gamePaused; DrawToDrawData(_voidLensData, selectionMode)
            if unported!("Main.InSmartCursorHighlightArea", false) {
                // selectionMode = 1 or 2: only read by DrawToDrawData
            }
        }
        // foreach (DrawData voidLensDatum in _voidLensData) voidLensDatum.Draw(Main.spriteBatch): nothing queued
        let _ = ctx;
    }

    /// `DrawMultiTileGrass()` (TileDrawing.cs:8912)
    fn draw_multi_tile_grass(&mut self, ctx: &mut DrawContext) {
        let unscaled_position = Vector2::new(ctx.g.main.screen_position.0, ctx.g.main.screen_position.1);
        let zero = Vector2::ZERO;
        let num = 2usize;
        let num2 = self.specials_count[num];
        for i in 0..num2 {
            let point = self.special_positions[num][i as usize];
            let x = point.x;
            let mut num3 = point.y;
            let mut size_x = 1;
            let mut num4 = 1;
            let Some(tile) = ctx.g.main.tile.try_get(x, num3) else {
                continue;
            };
            if tile.active() {
                match tile_at(ctx, x, num3).type_ {
                    27 => {
                        size_x = 2;
                        num4 = 5;
                    }
                    236 | 238 | 702 => {
                        size_x = 2;
                        num4 = 2;
                    }
                    233 => {
                        size_x = if tile_at(ctx, x, num3).frame_y != 0 { 2 } else { 3 };
                        num4 = 2;
                    }
                    530 | 651 | 705 => {
                        size_x = 3;
                        num4 = 2;
                    }
                    485 | 490 | 521 | 522 | 523 | 524 | 525 | 526 | 527 | 652 => {
                        size_x = 2;
                        num4 = 2;
                    }
                    489 => {
                        size_x = 2;
                        num4 = 3;
                    }
                    493 => {
                        size_x = 1;
                        num4 = 2;
                    }
                    519 => {
                        size_x = 1;
                        num4 = self.climb_cat_tail(ctx, x, num3);
                        num3 -= num4 - 1;
                    }
                    _ => {}
                }
                self.draw_multi_tile_grass_in_wind(ctx, unscaled_position, zero, x, num3, size_x, num4);
            }
        }
    }

    /// `ClimbCatTail(int originx, int originy)` (TileDrawing.cs:8981)
    fn climb_cat_tail(&self, ctx: &DrawContext, originx: i32, originy: i32) -> i32 {
        let mut num = 0;
        let mut num2 = originy;
        while num2 > 10 {
            let tile = tile_at(ctx, originx, num2);
            if !tile.active() || tile.type_ != 519 {
                break;
            }
            if tile.frame_x >= 180 {
                num += 1;
                break;
            }
            num2 -= 1;
            num += 1;
        }
        num
    }

    /// `DrawMultiTileVines()` (TileDrawing.cs:9003)
    fn draw_multi_tile_vines(&mut self, ctx: &mut DrawContext) {
        let unscaled_position = Vector2::new(ctx.g.main.screen_position.0, ctx.g.main.screen_position.1);
        let zero = Vector2::ZERO;
        let num = 3usize;
        let num2 = self.specials_count[num];
        for i in 0..num2 {
            let point = self.special_positions[num][i as usize];
            let x = point.x;
            let y = point.y;
            let mut size_x = 1;
            let mut size_y = 1;
            let Some(tile) = ctx.g.main.tile.try_get(x, y) else {
                continue;
            };
            if tile.active() {
                match tile_at(ctx, x, y).type_ {
                    34 => {
                        size_x = 3;
                        size_y = 3;
                    }
                    454 => {
                        size_x = 4;
                        size_y = 3;
                    }
                    42 | 270 | 271 | 572 | 581 | 660 => {
                        size_x = 1;
                        size_y = 2;
                    }
                    91 => {
                        size_x = 1;
                        size_y = 3;
                    }
                    95 | 126 | 444 => {
                        size_x = 2;
                        size_y = 2;
                    }
                    465 | 591 | 592 => {
                        size_x = 2;
                        size_y = 3;
                    }
                    698 => {
                        size_x = 1;
                        size_y = 1;
                    }
                    _ => {}
                }
                self.draw_multi_tile_vines_in_wind(ctx, unscaled_position, zero, x, y, size_x, size_y);
            }
        }
    }

    /// `DrawVines()` (TileDrawing.cs:9064)
    fn draw_vines(&mut self, ctx: &mut DrawContext) {
        let unscaled_position = Vector2::new(ctx.g.main.screen_position.0, ctx.g.main.screen_position.1);
        let zero = Vector2::ZERO;
        let num = 4usize;
        let num2 = self.specials_count[num];
        for i in 0..num2 {
            let point = self.special_positions[num][i as usize];
            let x = point.x;
            let y = point.y;
            self.draw_vine_strip(ctx, unscaled_position, zero, x, y);
        }
    }

    /// `DrawReverseVines()` (TileDrawing.cs:9079)
    fn draw_reverse_vines(&mut self, ctx: &mut DrawContext) {
        let unscaled_position = Vector2::new(ctx.g.main.screen_position.0, ctx.g.main.screen_position.1);
        let zero = Vector2::ZERO;
        let num = 7usize;
        let num2 = self.specials_count[num];
        for i in 0..num2 {
            let point = self.special_positions[num][i as usize];
            let x = point.x;
            let y = point.y;
            self.draw_rising_vine_strip(ctx, unscaled_position, zero, x, y);
        }
    }

    /// `DrawMultiTileGrassInWind(Vector2 screenPosition, Vector2 offSet, int topLeftX, int topLeftY, int sizeX, int sizeY)` (TileDrawing.cs:9094)
    #[allow(clippy::too_many_arguments)]
    fn draw_multi_tile_grass_in_wind(&mut self, ctx: &mut DrawContext, screen_position: Vector2, off_set: Vector2, top_left_x: i32, top_left_y: i32, size_x: i32, size_y: i32) {
        let wind_cycle = self.get_wind_cycle(ctx, top_left_x, top_left_y, self.sunflower_wind_counter);
        let vector = Vector2::new(
            (top_left_x * 16 - screen_position.x as i32) as f32 + size_x as f32 * 16.0 * 0.5,
            (top_left_y * 16 - screen_position.y as i32 + 16 * size_y) as f32,
        ) + off_set;
        let mut num = 0.07_f32;
        let type_ = tile_at(ctx, top_left_x, top_left_y).type_ as i32;
        let mut texture2_d: Option<Texture2D> = None;
        let mut color = Color::TRANSPARENT;
        let mut flag = in_a_place_with_wind(ctx.g, top_left_x, top_left_y, size_x, size_y);
        let mut flag2 = false;
        let mut num2 = 0;
        match type_ {
            27 => {
                color = Color::WHITE;
                flag2 = true;
                num2 = 74;
            }
            519 => {
                flag = in_a_place_with_wind(ctx.g, top_left_x, top_left_y, size_x, 1);
            }
            521 | 522 | 523 | 524 | 525 | 526 | 527 => {
                num = 0.0;
                flag = false;
            }
            _ => {
                num = 0.15;
            }
        }
        for i in top_left_x..top_left_x + size_x {
            for j in top_left_y..top_left_y + size_y {
                let tile = tile_at(ctx, i, j);
                let type2 = tile.type_;
                if type2 as i32 != type_ || !self.is_visible(&tile) {
                    continue;
                }
                let _ = ((i - top_left_x) as f32 + 0.5) / size_x as f32 - 0.5; // Math.Abs(...): the result is unused in the original
                let mut num3 = 1.0 - (j - top_left_y + 1) as f32 / size_y as f32;
                if num3 == 0.0 {
                    num3 = 0.1;
                }
                if !flag {
                    num3 = 0.0;
                }
                let mut d = self.get_tile_draw_data_info(ctx, i, j, &tile, type2);
                let (tile_frame_x, tile_frame_y) = (d.tile_frame_x, d.tile_frame_y);
                let flag3 = self.rand.next(4) == 0;
                let mut tile_light = lighting_get_color(ctx.g, i, j);
                self.draw_animated_tile_adjust_for_vision_changers(ctx, i, j, &tile, type2, tile_frame_x, tile_frame_y, &mut tile_light, flag3);
                tile_light = self.draw_tiles_get_light_override(ctx, j, i, &tile, type2, tile_frame_x, tile_frame_y, tile_light);
                if self.is_active_and_not_paused && flag3 {
                    self.draw_tiles_emit_particles(ctx, j, i, &tile, type2, tile_frame_x, tile_frame_y, tile_light);
                }
                let mut vector2 = Vector2::new((i * 16 - screen_position.x as i32) as f32, (j * 16 - screen_position.y as i32 + d.tile_top) as f32) + off_set;
                if tile.type_ == 493 && tile.frame_y == 0 {
                    if ctx.g.main.wind_for_visuals() >= 0.0 {
                        d.tile_sprite_effect = SpriteEffects(d.tile_sprite_effect.0 ^ SpriteEffects::FLIP_HORIZONTALLY.0);
                    }
                    if (d.tile_sprite_effect.0 & SpriteEffects::FLIP_HORIZONTALLY.0) == 0 {
                        vector2.x -= 6.0;
                    } else {
                        vector2.x += 6.0;
                    }
                }
                let vector3 = Vector2::new(wind_cycle * 1.0, wind_cycle.abs() * 2.0 * num3);
                let origin = vector - vector2;
                let tile_draw_texture = self.get_tile_draw_texture(ctx, &tile, i, j);
                if tile_draw_texture.is_some() {
                    if flag2 {
                        texture2_d = tile_draw_texture.clone();
                    }
                    let mut side_flags = SideFlags::NONE;
                    if i > top_left_x {
                        side_flags |= SideFlags::LEFT;
                    }
                    if i < top_left_x + size_x - 1 {
                        side_flags |= SideFlags::RIGHT;
                    }
                    if j > top_left_y {
                        side_flags |= SideFlags::TOP;
                    }
                    if j < top_left_y + size_y - 1 {
                        side_flags |= SideFlags::BOTTOM;
                    }
                    self.nature_renderer.draw_nature(
                        tile_draw_texture.as_ref(),
                        vector + Vector2::new(0.0, vector3.y),
                        Rectangle::new(tile_frame_x as i32 + d.add_fr_x, tile_frame_y as i32 + d.add_fr_y, d.tile_width, d.tile_height - d.half_brick_height),
                        tile_light,
                        wind_cycle * num * num3,
                        origin,
                        1.0,
                        d.tile_sprite_effect,
                        0.0,
                        side_flags,
                    );
                    if texture2_d.is_some() {
                        self.nature_renderer.draw_glowmask(
                            texture2_d.as_ref(),
                            vector + Vector2::new(0.0, vector3.y),
                            Some(Rectangle::new(tile_frame_x as i32 + d.add_fr_x, tile_frame_y as i32 + d.add_fr_y + num2, d.tile_width, d.tile_height - d.half_brick_height)),
                            color,
                            wind_cycle * num * num3,
                            origin,
                            1.0,
                            d.tile_sprite_effect,
                            0.0,
                        );
                    }
                }
            }
        }
    }

    /// `DrawVineStrip(Vector2 screenPosition, Vector2 offSet, int x, int startY)` (TileDrawing.cs:9212)
    fn draw_vine_strip(&mut self, ctx: &mut DrawContext, screen_position: Vector2, off_set: Vector2, x: i32, start_y: i32) {
        let mut num = 0;
        let mut num2 = 0;
        let mut vector = Vector2::new((x * 16 + 8) as f32, (start_y * 16 - 2) as f32);
        let mut amount = ctx.g.main.wind_for_visuals().abs() / 1.2;
        amount = terraria_core::xna::math_helper::lerp(0.2, 1.0, amount);
        let mut num3 = -0.08_f32 * amount;
        let wind_cycle = self.get_wind_cycle(ctx, x, start_y, self.vine_wind_counter);
        let mut num4 = 0.0_f32;
        let mut num5 = 0.0_f32;
        let mut i = start_y;
        while i < ctx.g.main.max_tiles_y - 10 {
            let Some(tile) = ctx.g.main.tile.try_get(x, i) else {
                break;
            };
            let type_ = tile.type_;
            if !tile.active() || !tile_sets::VINE_THREADS[type_ as usize] {
                break;
            }
            num += 1;
            if num2 >= 5 {
                num3 += 0.0075 * amount;
            }
            if num2 >= 2 {
                num3 += 0.0025;
            }
            if ctx.g.main.remix_world {
                if wall_sets::ALLOWS_WIND[tile.wall as usize] && (i as f64) > ctx.g.main.world_surface {
                    num2 += 1;
                }
            } else if wall_sets::ALLOWS_WIND[tile.wall as usize] && (i as f64) < ctx.g.main.world_surface {
                num2 += 1;
            }
            let wind_grid_push = self.get_wind_grid_push(x, i, 20, 0.01);
            num4 = if wind_grid_push != 0.0 || num5 == 0.0 { num4 - wind_grid_push } else { num4 * -0.78 };
            num5 = wind_grid_push;
            let mut color = lighting_get_color(ctx.g, x, i);
            let d = self.get_tile_draw_data_info(ctx, x, i, &tile, type_);
            let (tile_frame_x, tile_frame_y) = (d.tile_frame_x, d.tile_frame_y);
            let position = Vector2::new(-(screen_position.x as i32) as f32, -(screen_position.y as i32) as f32) + off_set + vector;
            if tile.fullbright_block() {
                color = Color::WHITE;
            }
            let num6 = num2 as f32 * num3 * wind_cycle + num4;
            if unported!("Player.biomeSight", false) {
                let mut sight_color = Color::WHITE;
                if unported!("Main.IsTileBiomeSightable", false) {
                    if color.r() < sight_color.r() {
                        color.set_r(sight_color.r());
                    }
                    if color.g() < sight_color.g() {
                        color.set_g(sight_color.g());
                    }
                    if color.b() < sight_color.b() {
                        color.set_b(sight_color.b());
                    }
                    if self.is_active_and_not_paused && self.rand.next(480) == 0 {
                        let _new_color = sight_color;
                        sight_color = Color::WHITE;
                        let _ = sight_color;
                        unported!("Dust.NewDust");
                    }
                }
            }
            let tile_draw_texture = self.get_tile_draw_texture(ctx, &tile, x, i);
            if tile_draw_texture.is_none() {
                break;
            }
            if self.is_visible(&tile) {
                let tile2 = tile_at(ctx, x, i + 1);
                let flag = tile2.active() && tile_sets::VINE_THREADS[tile2.type_ as usize];
                let origin = Vector2::new((d.tile_width / 2) as f32, (d.half_brick_height - d.tile_top) as f32);
                self.nature_renderer.draw_nature(
                    tile_draw_texture.as_ref(),
                    position,
                    Rectangle::new(tile_frame_x as i32 + d.add_fr_x, tile_frame_y as i32 + d.add_fr_y, d.tile_width, d.tile_height - d.half_brick_height),
                    color,
                    num6,
                    origin,
                    1.0,
                    d.tile_sprite_effect,
                    0.0,
                    if flag { SideFlags::BOTTOM } else { SideFlags::NONE },
                );
                if d.glow_texture.is_some() {
                    self.nature_renderer.draw_glowmask(d.glow_texture.as_ref(), position, Some(d.glow_source_rect), d.glow_color, num6, origin, 1.0, d.tile_sprite_effect, 0.0);
                }
            }
            vector += to_rotation_vector2(num6 + std::f32::consts::PI / 2.0) * 16.0_f32;
            i += 1;
        }
        let _ = num;
    }

    /// `DrawRisingVineStrip(Vector2 screenPosition, Vector2 offSet, int x, int startY)` (TileDrawing.cs:9315)
    fn draw_rising_vine_strip(&mut self, ctx: &mut DrawContext, screen_position: Vector2, off_set: Vector2, x: i32, start_y: i32) {
        let mut num = 0;
        let mut num2 = 0;
        let mut vector = Vector2::new((x * 16 + 8) as f32, (start_y * 16 + 16 + 2) as f32);
        let mut amount = ctx.g.main.wind_for_visuals().abs() / 1.2;
        amount = terraria_core::xna::math_helper::lerp(0.2, 1.0, amount);
        let mut num3 = -0.08_f32 * amount;
        let wind_cycle = self.get_wind_cycle(ctx, x, start_y, self.vine_wind_counter);
        let mut num4 = 0.0_f32;
        let mut num5 = 0.0_f32;
        let mut num6 = start_y;
        while num6 > 10 {
            if let Some(tile) = ctx.g.main.tile.try_get(x, num6) {
                let type_ = tile.type_;
                if !tile.active() || !tile_sets::REVERSE_VINE_THREADS[type_ as usize] {
                    break;
                }
                num += 1;
                if num2 >= 5 {
                    num3 += 0.0075 * amount;
                }
                if num2 >= 2 {
                    num3 += 0.0025;
                }
                if wall_sets::ALLOWS_WIND[tile.wall as usize] && (num6 as f64) < ctx.g.main.world_surface {
                    num2 += 1;
                }
                let wind_grid_push = self.get_wind_grid_push(x, num6, 40, -0.004);
                num4 = if wind_grid_push != 0.0 || num5 == 0.0 { num4 - wind_grid_push } else { num4 * -0.78 };
                num5 = wind_grid_push;
                let mut color = lighting_get_color(ctx.g, x, num6);
                let d = self.get_tile_draw_data_info(ctx, x, num6, &tile, type_);
                let (tile_frame_x, tile_frame_y) = (d.tile_frame_x, d.tile_frame_y);
                let position = Vector2::new(-(screen_position.x as i32) as f32, -(screen_position.y as i32) as f32) + off_set + vector;
                if tile.fullbright_block() {
                    color = Color::WHITE;
                }
                let num7 = num2 as f32 * (0.0 - num3) * wind_cycle + num4;
                let tile_draw_texture = self.get_tile_draw_texture(ctx, &tile, x, num6);
                if tile_draw_texture.is_none() {
                    break;
                }
                if self.is_visible(&tile) {
                    let tile2 = tile_at(ctx, x, num6 - 1);
                    let flag = tile2.active() && tile_sets::REVERSE_VINE_THREADS[tile2.type_ as usize];
                    self.nature_renderer.draw_nature(
                        tile_draw_texture.as_ref(),
                        position,
                        Rectangle::new(tile_frame_x as i32 + d.add_fr_x, tile_frame_y as i32 + d.add_fr_y, d.tile_width, d.tile_height - d.half_brick_height),
                        color,
                        num7,
                        Vector2::new((d.tile_width / 2) as f32, (d.half_brick_height - d.tile_top + d.tile_height) as f32),
                        1.0,
                        d.tile_sprite_effect,
                        0.0,
                        if flag { SideFlags::TOP } else { SideFlags::NONE },
                    );
                }
                vector += to_rotation_vector2(num7 - std::f32::consts::PI / 2.0) * 16.0_f32;
            }
            num6 -= 1;
        }
        let _ = num;
    }

    /// `GetAverageWindGridPush(int topLeftX, int topLeftY, int sizeX, int sizeY, int totalPushTime, float pushForcePerFrame)` (TileDrawing.cs:9378)
    #[allow(dead_code)]
    fn get_average_wind_grid_push(&self, top_left_x: i32, top_left_y: i32, size_x: i32, size_y: i32, total_push_time: i32, push_force_per_frame: f32) -> f32 {
        let mut num = 0.0_f32;
        let mut num2 = 0;
        for i in 0..size_x {
            for j in 0..size_y {
                let wind_grid_push = self.get_wind_grid_push(top_left_x + i, top_left_y + j, total_push_time, push_force_per_frame);
                if wind_grid_push != 0.0 {
                    num += wind_grid_push;
                    num2 += 1;
                }
            }
        }
        if num2 == 0 {
            return 0.0;
        }
        num / num2 as f32
    }

    /// `GetHighestWindGridPushComplex(int topLeftX, int topLeftY, int sizeX, int sizeY, int totalPushTime, float pushForcePerFrame,
    /// int loops, bool swapLoopDir)` (TileDrawing.cs:9401)
    #[allow(clippy::too_many_arguments)]
    fn get_highest_wind_grid_push_complex(&self, top_left_x: i32, top_left_y: i32, size_x: i32, size_y: i32, total_push_time: i32, push_force_per_frame: f32, loops: i32, swap_loop_dir: bool) -> f32 {
        let mut result = 0.0_f32;
        let mut num = i32::MAX;
        for i in 0..1 {
            for j in 0..size_y {
                let (wind_time_left, _, _) = self.wind_grid.get_wind_time(top_left_x + i + size_x / 2, top_left_y + j, total_push_time);
                let wind_grid_push_complex = self.get_wind_grid_push_complex(top_left_x + i, top_left_y + j, total_push_time, push_force_per_frame, loops, swap_loop_dir);
                if wind_time_left < num && wind_time_left != 0 {
                    result = wind_grid_push_complex;
                    num = wind_time_left;
                }
            }
        }
        result
    }

    /// `DrawMultiTileVinesInWind(Vector2 screenPosition, Vector2 offSet, int topLeftX, int topLeftY, int sizeX, int sizeY)` (TileDrawing.cs:9421)
    #[allow(clippy::too_many_arguments, unused_assignments)]
    fn draw_multi_tile_vines_in_wind(&mut self, ctx: &mut DrawContext, screen_position: Vector2, mut off_set: Vector2, top_left_x: i32, top_left_y: i32, size_x: i32, size_y: i32) {
        let mut wind_cycle = self.get_wind_cycle(ctx, top_left_x, top_left_y, self.sunflower_wind_counter);
        let num = wind_cycle;
        let total_push_time = 60;
        let push_force_per_frame = 1.26_f32;
        let highest_wind_grid_push_complex = self.get_highest_wind_grid_push_complex(top_left_x, top_left_y, size_x, size_y, total_push_time, push_force_per_frame, 3, true);
        wind_cycle += highest_wind_grid_push_complex;
        let mut vector = Vector2::new((top_left_x * 16 - screen_position.x as i32) as f32 + size_x as f32 * 16.0 * 0.5, (top_left_y * 16 - screen_position.y as i32) as f32) + off_set;
        let mut num2;
        let tile = tile_at(ctx, top_left_x, top_left_y);
        let type_ = tile.type_ as i32;
        let mut vector2 = Vector2::new(0.0, -2.0);
        vector += vector2;
        let flag = false;
        let _ = flag;
        let below_platform = if type_ != 465 && (type_ - 591) as u32 > 1 {
            size_x == 1 && is_below_a_non_hammered_platform(ctx.g, top_left_x, top_left_y)
        } else {
            is_below_a_non_hammered_platform(ctx.g, top_left_x, top_left_y) && is_below_a_non_hammered_platform(ctx.g, top_left_x + 1, top_left_y)
        };
        if below_platform {
            vector.y -= 8.0;
            vector2.y -= 8.0;
        }
        let mut texture2_d: Option<Texture2D> = None;
        let mut color = Color::TRANSPARENT;
        let mut num3: Option<f32> = None;
        let mut num4 = 1.0_f32;
        let mut num5 = -4.0_f32;
        let mut flag2 = false;
        let mut flag3 = false;
        num2 = 0.15_f32;
        match type_ {
            34 | 126 => {
                num3 = Some(1.0);
                num5 = 0.0;
                match tile.frame_y as i32 / 54 + tile.frame_x as i32 / 108 * 37 {
                    9 => {
                        num3 = None;
                        num5 = -1.0;
                        flag2 = true;
                        num2 *= 0.3;
                    }
                    11 => {
                        num2 *= 0.5;
                    }
                    12 => {
                        num3 = None;
                        num5 = -1.0;
                    }
                    18 => {
                        num3 = None;
                        num5 = -1.0;
                    }
                    21 => {
                        num3 = None;
                        num5 = -1.0;
                    }
                    23 => {
                        num3 = Some(0.0);
                    }
                    25 => {
                        num3 = None;
                        num5 = -1.0;
                        flag2 = true;
                    }
                    32 => {
                        num2 *= 0.5;
                    }
                    33 => {
                        num2 *= 0.5;
                    }
                    35 => {
                        num3 = Some(0.0);
                    }
                    36 => {
                        num3 = None;
                        num5 = -1.0;
                        flag2 = true;
                    }
                    37 => {
                        num3 = None;
                        num5 = -1.0;
                        flag2 = true;
                        num2 *= 0.5;
                    }
                    39 => {
                        num3 = None;
                        num5 = -1.0;
                        flag2 = true;
                    }
                    40 | 41 | 42 | 43 => {
                        num3 = None;
                        num5 = -2.0;
                        flag2 = true;
                        num2 *= 0.5;
                    }
                    44 => {
                        num3 = None;
                        num5 = -3.0;
                    }
                    54 | 55 | 60 | 65 | 68 | 70 => {
                        num3 = Some(0.0);
                    }
                    67 => {
                        num3 = Some(1.0);
                    }
                    _ => {}
                }
            }
            42 => {
                num3 = Some(1.0);
                num5 = 0.0;
                match tile.frame_y as i32 / 36 {
                    0 => {
                        num3 = None;
                        num5 = -1.0;
                    }
                    9 => {
                        num3 = Some(0.0);
                    }
                    12 => {
                        num3 = None;
                        num5 = -1.0;
                    }
                    14 => {
                        num3 = None;
                        num5 = -1.0;
                    }
                    28 => {
                        num3 = None;
                        num5 = -1.0;
                    }
                    30 => {
                        num3 = Some(0.0);
                    }
                    32 => {
                        num3 = Some(0.0);
                    }
                    33 => {
                        num3 = Some(0.0);
                    }
                    34 => {
                        num3 = None;
                        num5 = -1.0;
                    }
                    35 => {
                        num3 = Some(0.0);
                    }
                    38 => {
                        num3 = None;
                        num5 = -1.0;
                    }
                    39 => {
                        num3 = None;
                        num5 = -1.0;
                        flag2 = true;
                    }
                    40 | 41 | 42 | 43 => {
                        num3 = Some(0.0);
                        num3 = None; // as in the original: the value assigned first is overwritten
                        num5 = -1.0;
                        flag2 = true;
                    }
                    54 | 55 | 60 | 65 | 67 | 70 => {
                        num3 = Some(0.0);
                    }
                    _ => {}
                }
            }
            95 | 270 | 271 | 444 | 454 | 572 | 581 | 660 => {
                num3 = Some(1.0);
                num5 = 0.0;
            }
            591 => {
                num4 = 0.5;
                num5 = -2.0;
            }
            592 => {
                num4 = 0.5;
                num5 = -2.0;
                texture2_d = ctx.glow_mask(294);
                color = Color::from_ints(255, 255, 255, 0);
            }
            698 => {
                num4 = 0.5;
                num5 = -1.0;
                off_set.x -= 10.0;
                flag3 = true;
            }
            _ => {}
        }
        if flag2 {
            vector += Vector2::new(0.0, 16.0);
        }
        num2 *= -1.0;
        let flag4 = in_a_place_with_wind(ctx.g, top_left_x, top_left_y, size_x, size_y);
        if flag3 || !flag4 {
            wind_cycle -= num;
        }
        let mut num6: u64 = 0;
        for i in top_left_x..top_left_x + size_x {
            for j in top_left_y..top_left_y + size_y {
                let tile2 = tile_at(ctx, i, j);
                let type2 = tile2.type_;
                if type2 as i32 != type_ || !self.is_visible(&tile2) {
                    continue;
                }
                let _ = ((i - top_left_x) as f32 + 0.5) / size_x as f32 - 0.5; // Math.Abs(...): the result is unused in the original
                let mut num7 = (j - top_left_y + 1) as f32 / size_y as f32;
                if num7 == 0.0 {
                    num7 = 0.1;
                }
                if let Some(v) = num3 {
                    num7 = v;
                }
                if flag2 && j == top_left_y {
                    num7 = 0.0;
                }
                let d = self.get_tile_draw_data_info(ctx, i, j, &tile2, type2);
                let (tile_frame_x, tile_frame_y) = (d.tile_frame_x, d.tile_frame_y);
                let flag5 = self.rand.next(4) == 0;
                let mut tile_light = lighting_get_color(ctx.g, i, j);
                self.draw_animated_tile_adjust_for_vision_changers(ctx, i, j, &tile2, type2, tile_frame_x, tile_frame_y, &mut tile_light, flag5);
                tile_light = self.draw_tiles_get_light_override(ctx, j, i, &tile2, type2, tile_frame_x, tile_frame_y, tile_light);
                if self.is_active_and_not_paused && flag5 {
                    self.draw_tiles_emit_particles(ctx, j, i, &tile2, type2, tile_frame_x, tile_frame_y, tile_light);
                }
                let mut vector3 = Vector2::new((i * 16 - screen_position.x as i32) as f32, (j * 16 - screen_position.y as i32 + d.tile_top) as f32) + off_set;
                vector3 += vector2;
                let vector4 = Vector2::new(wind_cycle * num4, wind_cycle.abs() * num5 * num7);
                let vector5 = vector - vector3;
                let tile_draw_texture = self.get_tile_draw_texture(ctx, &tile2, i, j);
                if tile_draw_texture.is_none() {
                    continue;
                }
                let vector6 = vector + Vector2::new(0.0, vector4.y);
                let rectangle = Rectangle::new(tile_frame_x as i32 + d.add_fr_x, tile_frame_y as i32 + d.add_fr_y, d.tile_width, d.tile_height - d.half_brick_height);
                let num8 = wind_cycle * num2 * num7;
                let tse = d.tile_sprite_effect;
                if type2 == 660 && j == top_left_y + size_y - 1 {
                    let value = ctx.extra(260);
                    // `_ = ((float)((i + j) % 200) * 0.11f + (float)Main.timeForVisualEffects / 360f) % 1f;` (result unused)
                    let white = Color::WHITE;
                    sprite_batch_draw(ctx, value.as_ref(), vector6, rectangle, white, num8, vector5, 1.0, tse);
                }
                sprite_batch_draw(ctx, tile_draw_texture.as_ref(), vector6, rectangle, tile_light, num8, vector5, 1.0, tse);
                if type2 == 660 && j == top_left_y + size_y - 1 {
                    let value2 = ctx.extra(260);
                    let mut color2 = hsl_to_rgb((((i + j) % 200) as f32 * 0.11 + ctx.g.main.time_for_visual_effects as f32 / 360.0) % 1.0, 1.0, 0.8, 255);
                    color2.set_a(127);
                    let value3 = rectangle;
                    let position = vector6;
                    let origin = vector5;
                    sprite_batch_draw(ctx, value2.as_ref(), position, value3, color2, num8, origin, 1.0, tse);
                }
                if type2 == 698 && unported!("TileEntity.TryGetAt<TEDeadCellsDisplayJar>", false) {
                    // the jar's item: the glow passes of the jar texture (frame + 46, + 92) and the item icon over them
                    unported!("TEDeadCellsDisplayJar.item (ItemSlot.DrawItemIcon)");
                }
                if texture2_d.is_some() {
                    sprite_batch_draw(ctx, texture2_d.as_ref(), vector6, rectangle, color, num8, vector5, 1.0, tse);
                }
                let mut tile_flame_data = self.get_tile_flame_data(ctx, i, j, type2 as i32, tile_frame_y as i32);
                if num6 == 0 {
                    num6 = tile_flame_data.flame_seed;
                }
                tile_flame_data.flame_seed = num6;
                for _k in 0..tile_flame_data.flame_count {
                    let x2 = utils_random_int(&mut tile_flame_data.flame_seed, tile_flame_data.flame_range_x_min, tile_flame_data.flame_range_x_max) as f32 * tile_flame_data.flame_range_mult_x;
                    let y = utils_random_int(&mut tile_flame_data.flame_seed, tile_flame_data.flame_range_y_min, tile_flame_data.flame_range_y_max) as f32 * tile_flame_data.flame_range_mult_y;
                    sprite_batch_draw(ctx, tile_flame_data.flame_texture.as_ref(), vector6 + Vector2::new(x2, y), rectangle, tile_flame_data.flame_color, num8, vector5, 1.0, tse);
                }
            }
        }
        let _ = TileCounterType::Tree;
        let _ = TileFlameData::default;
    }

    /// `EmitAlchemyHerbParticles(int j, int i, int style)` (TileDrawing.cs:9802). Called by `DrawTiles_EmitParticles`; every branch
    /// only spawns a `Dust`, which is not ported.
    #[allow(dead_code)]
    pub(super) fn emit_alchemy_herb_particles(&mut self, _ctx: &mut DrawContext, _j: i32, _i: i32, style: i32) {
        if style == 0 && self.rand.next(100) == 0 {
            unported!("Dust.NewDust");
        }
        if style == 1 && self.rand.next(100) == 0 {
            unported!("Dust.NewDust");
        }
        if style == 3 {
            if self.rand.next(200) == 0 {
                unported!("Dust.NewDust");
            }
            if self.rand.next(75) == 0 {
                unported!("Dust.NewDust");
            }
        }
        if style == 4 && self.rand.next(150) == 0 {
            unported!("Dust.NewDust");
        }
        if style == 5 && self.rand.next(40) == 0 {
            unported!("Dust.NewDust");
        }
        if style == 6 && self.rand.next(30) == 0 {
            unported!("Dust.NewDust");
        }
    }
}

/// `Utils.Floor(this Vector2 vec)` (Utils.cs:1721): both components truncated through `(int)`
fn vector2_floor(vec: Vector2) -> Vector2 {
    Vector2::new(vec.x as i32 as f32, vec.y as i32 as f32)
}
