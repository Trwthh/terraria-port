//! `TileDrawing.GetTileDrawData` (TileDrawing.cs:4567-6620), the frame / texture / offset rules per tile type.
//!
//! Split out of `tile_drawing.rs` only because of its size; it is a method of the same class. The statement
//! order is the original's, local variables are the decompiler's names in snake case.

use terraria_core::framing::get_tile_safely;
use terraria_core::xna::{Color, Rectangle};
use terraria_core::Tile;
use terraria_data::sets::tile_id::sets as tile_sets;
use terraria_sim::unported;

use crate::data_structures::tile_draw_info::TileDrawInfo;
use crate::draw_context::DrawContext;
use crate::gfx::{SpriteEffects, Texture2D};
use crate::pending::{WORLD_GEN_BEACH_DISTANCE, lighting_get_color, world_gen_get_biome_influence, world_gen_get_cactus_type, world_gen_in_a_place_with_wind, world_gen_solid_tile};

use super::tile_drawing::{get_palm_tree_biome, get_tree_biome, TileDrawing};

impl TileDrawing {
    /// `TileDrawing.GetTileDrawData(int x, int y, Tile tileCache, ushort typeCache, ref short tileFrameX, ref short tileFrameY,
    /// out int tileWidth, out int tileHeight, out int tileTop, out int halfBrickHeight, out int addFrX, out int addFrY,
    /// out SpriteEffects tileSpriteEffect, out Texture2D glowTexture, out Rectangle glowSourceRect, out Color glowColor)`
    /// (TileDrawing.cs:4567). The `ref`/`out` parameters are the fields of `d` of the same names.
    #[allow(unused_mut, unused_assignments, unused_variables, clippy::needless_late_init)]
    pub(super) fn get_tile_draw_data(&mut self, ctx: &mut DrawContext, x: i32, y: i32, tile_cache: &Tile, type_cache: u16, d: &mut TileDrawInfo) {
        let mut tile_frame_x: i32 = d.tile_frame_x as i32;
        let mut tile_frame_y: i32 = d.tile_frame_y as i32;
        let mut tile_top: i32 = 0;
        let mut tile_width: i32 = 16;
        let mut tile_height: i32 = 16;
        let mut half_brick_height: i32 = 0;
        let mut add_fr_y: i32 = ctx.state.tile_frame[type_cache as usize] * 38;
        let mut add_fr_x: i32 = 0;
        let mut tile_sprite_effect = SpriteEffects::NONE;
        let mut glow_texture: Option<Texture2D> = None;
        let mut glow_source_rect = Rectangle::EMPTY;
        let mut glow_color = Color::TRANSPARENT;
        let color = lighting_get_color(ctx.g, x, y);
        match type_cache {
            752 => {
                tile_height = 38;
                tile_width = 36;
                tile_top = 2;
            }
            751 => {
                tile_height = 46;
                tile_width = 56;
                let mut num3: i32 = (x + y * 2) % 7;
                tile_frame_y = (tile_frame_y + (num3 * 46) as i16 as i32) as i16 as i32;
            }
            739 | 748 => {
                let mut num19: i32 = ctx.state.tile_frame[type_cache as usize];
                add_fr_y = num19 * 90;
            }
            726 => {
                tile_frame_x = 0;
                tile_frame_y = 0;
                tile_width = 20;
                tile_height = 20;
            }
            719 => {
                let mut num34: i32 = (x + y + (ctx.g.main.global_time_wrapped_hourly * 15f32) as i32) % 14;
                let mut num35: i32 = num34 / 4;
                let mut num36: i32 = num34 % 4;
                add_fr_x += 288 * num35;
                add_fr_y += 270 * num36;
            }
            443 => {
                if tile_frame_x / 36 >= 2 {
                    tile_top = -2;
                } else {
                    tile_top = 2;
                }
            }
            571 => {
                if x % 2 == 0 {
                    tile_sprite_effect = SpriteEffects::FLIP_HORIZONTALLY;
                }
                tile_top = 2;
            }
            698 => {
                tile_width = 36;
                let mut num62: i32 = tile_frame_x / 18;
                tile_frame_x = (num62 * 38) as i16 as i32;
                tile_height = 44;
            }
            136 => {
                if tile_frame_x == 0 {
                    tile_top = 2;
                }
            }
            561 => {
                tile_top -= 2;
                tile_height = 20;
                add_fr_y = tile_frame_y / 18 * 4;
            }
            518 => {
                let mut num28: i32 = tile_cache.liquid as i32 / 16;
                num28 -= 3;
                if world_gen_solid_tile(ctx.g, x, y - 1, false) && num28 > 8 {
                    num28 = 8;
                }
                if tile_cache.liquid == 0 {
                    let mut tile_safely = get_tile_safely(&ctx.g.main.tile, x, y + 1);
                    if tile_safely.nactive() {
                        match tile_safely.block_type() {
                            1 => {
                                num28 = -16 + 8.max(tile_safely.liquid as i32 / 16);
                            }
                            2 | 3 => {
                                num28 -= 4;
                            }
                            _ => {}
                        }
                    }
                }
                tile_top -= num28;
            }
            330 | 331 | 332 | 333 => {
                tile_top += 2;
            }
            129 => {
                add_fr_y = 0;
                if tile_frame_x >= 324 {
                    let mut num56: i32 = (tile_frame_x - 324) / 18;
                    let mut num57: i32 = (num56 + ctx.state.tile_frame[type_cache as usize]) % 6 - num56;
                    add_fr_x = num57 * 18;
                }
            }
            5 => {
                tile_width = 20;
                tile_height = 20;
                let mut tree_biome: i32 = get_tree_biome(ctx.g, x, y, tile_frame_x, tile_frame_y);
                tile_frame_x = (tile_frame_x + (176 * (tree_biome + 1)) as i16 as i32) as i16 as i32;
            }
            583 | 584 | 585 | 586 | 587 | 588 | 589 | 596 | 616 | 634 => {
                tile_width = 20;
                tile_height = 20;
            }
            476 => {
                tile_width = 20;
                tile_height = 18;
            }
            323 => {
                tile_width = 20;
                tile_height = 20;
                let mut palm_tree_biome: i32 = get_palm_tree_biome(ctx.g, x, y);
                tile_frame_y = (22 * palm_tree_biome) as i16 as i32;
            }
            4 => {
                tile_width = 20;
                tile_height = 20;
                if world_gen_solid_tile(ctx.g, x, y - 1, false) {
                    tile_top = 4;
                }
            }
            78 | 85 | 133 | 134 | 173 | 210 | 233 | 254 | 283 | 378 | 457 | 466 | 520 | 651 | 652 => {
                tile_top = 2;
            }
            100 => {
                tile_top = 2;
                let mut num58: i32 = tile_frame_y / 2016;
                add_fr_y -= 2016 * num58;
                add_fr_x += 72 * num58;
            }
            530 => {
                let mut num51: i32 = y - tile_frame_y % 36 / 18 + 2;
                let mut num52: i32 = x - tile_frame_x % 54 / 18;
                let (corrupt_count2, crimson_count2, hallowed_count2) = world_gen_get_biome_influence(ctx.g, num52, num52 + 3, num51, num51);
                let mut num53: i32 = corrupt_count2;
                if num53 < crimson_count2 {
                    num53 = crimson_count2;
                }
                if num53 < hallowed_count2 {
                    num53 = hallowed_count2;
                }
                let mut num54: i32 = 0;
                num54 = if corrupt_count2 != 0 || crimson_count2 != 0 || hallowed_count2 != 0 { if hallowed_count2 == num53 { 1 } else if crimson_count2 != num53 { 3 } else { 2 } } else { 0 };
                add_fr_y += 36 * num54;
                tile_top = 2;
            }
            705 => {
                tile_top = 2;
            }
            485 => {
                tile_top = 2;
                let mut num14: i32 = ctx.state.tile_frame_counter[type_cache as usize];
                num14 /= 5;
                let mut num15: i32 = y - tile_frame_y / 18;
                let mut num16: i32 = x - tile_frame_x / 18;
                num14 += num15 + num16;
                num14 %= 4;
                add_fr_y = num14 * 36;
            }
            489 => {
                tile_top = 2;
                let mut num47: i32 = y - tile_frame_y / 18;
                let mut num48: i32 = x - tile_frame_x / 18;
                if world_gen_in_a_place_with_wind(ctx.g, num48, num47, 2, 3) {
                    let mut num49: i32 = ctx.state.tile_frame_counter[type_cache as usize];
                    num49 /= 5;
                    num49 += num47 + num48;
                    num49 %= 16;
                    add_fr_y = num49 * 54;
                }
            }
            490 => {
                tile_top = 2;
                let mut y2: i32 = y - tile_frame_y / 18;
                let mut x2: i32 = x - tile_frame_x / 18;
                let mut num20: bool = world_gen_in_a_place_with_wind(ctx.g, x2, y2, 2, 2);
                let mut num21: i32 = if num20 { ctx.state.tile_frame[type_cache as usize] } else { 0 };
                let mut num22: i32 = 0;
                if num20 {
                    if ctx.g.main.wind_for_visuals().abs() > 0.5f32 {
                        match ctx.state.weather_vane_bobframe {
                            0 => {
                                num22 = 0;
                            }
                            1 => {
                                num22 = 1;
                            }
                            2 => {
                                num22 = 2;
                            }
                            3 => {
                                num22 = 1;
                            }
                            4 => {
                                num22 = 0;
                            }
                            5 => {
                                num22 = -1;
                            }
                            6 => {
                                num22 = -2;
                            }
                            7 => {
                                num22 = -1;
                            }
                            _ => {}
                        }
                    } else {
                        match ctx.state.weather_vane_bobframe {
                            0 => {
                                num22 = 0;
                            }
                            1 => {
                                num22 = 1;
                            }
                            2 => {
                                num22 = 0;
                            }
                            3 => {
                                num22 = -1;
                            }
                            4 => {
                                num22 = 0;
                            }
                            5 => {
                                num22 = 1;
                            }
                            6 => {
                                num22 = 0;
                            }
                            7 => {
                                num22 = -1;
                            }
                            _ => {}
                        }
                    }
                }
                num21 += num22;
                if num21 < 0 {
                    num21 += 12;
                }
                num21 %= 12;
                add_fr_y = num21 * 36;
            }
            33 | 49 | 174 | 372 | 646 => {
                tile_height = 20;
                tile_top = -4;
            }
            529 => {
                let mut num38: i32 = y + 1;
                let (corrupt_count, crimson_count, hallowed_count) = world_gen_get_biome_influence(ctx.g, x, x, num38, num38);
                let mut num39: i32 = corrupt_count;
                if num39 < crimson_count {
                    num39 = crimson_count;
                }
                if num39 < hallowed_count {
                    num39 = hallowed_count;
                }
                let mut num40: i32 = 0;
                num40 = if corrupt_count == 0 && crimson_count == 0 && hallowed_count == 0 { if x < WORLD_GEN_BEACH_DISTANCE || x > ctx.g.main.max_tiles_x - WORLD_GEN_BEACH_DISTANCE { 1 } else { 0 } } else if hallowed_count == num39 { 2 } else if crimson_count != num39 { 4 } else { 3 };
                add_fr_y += 34 * num40 - tile_frame_y;
                tile_height = 32;
                tile_top = -14;
                if x % 2 == 0 {
                    tile_sprite_effect = SpriteEffects::FLIP_HORIZONTALLY;
                }
            }
            3 | 24 | 61 | 71 | 110 | 201 | 637 | 703 => {
                tile_height = 20;
                if x % 2 == 0 {
                    tile_sprite_effect = SpriteEffects::FLIP_HORIZONTALLY;
                }
            }
            20 | 590 | 595 => {
                tile_height = 18;
                if x % 2 == 0 {
                    tile_sprite_effect = SpriteEffects::FLIP_HORIZONTALLY;
                }
            }
            615 => {
                tile_height = 18;
                if x % 2 == 0 {
                    tile_sprite_effect = SpriteEffects::FLIP_HORIZONTALLY;
                }
            }
            16 | 17 | 26 | 32 | 69 | 72 | 77 | 124 | 137 | 138 | 352 | 462 | 487 | 488 | 574 | 575 | 576 | 577 | 578 | 664 | 695 | 704 | 712 | 713 | 714 | 715 | 716 => {
                tile_height = 18;
            }
            79 => {
                tile_height = 18;
                let mut num33: i32 = tile_frame_y / 2016;
                add_fr_y -= 2016 * num33;
                add_fr_x += 144 * num33;
            }
            90 => {
                let mut num32: i32 = tile_frame_y / 2016;
                add_fr_y -= 2016 * num32;
                add_fr_x += 144 * num32;
            }
            18 => {
                let mut num31: i32 = tile_frame_x / 2016;
                add_fr_x -= 2016 * num31;
                add_fr_y += 20 * num31;
            }
            711 => {
                if tile_frame_x > 0 {
                    tile_width = 18;
                }
                tile_height = 20;
                glow_texture = ctx.tile_texture(711);
                glow_source_rect = Rectangle::new(tile_frame_x + add_fr_x, tile_frame_y + add_fr_y, tile_width, tile_height);
            }
            654 => {
                tile_top += 2;
            }
            14 | 21 | 411 | 467 | 469 => {
                if tile_frame_y == 18 {
                    tile_height = 18;
                }
            }
            15 | 497 => {
                if tile_frame_y % 40 == 18 {
                    tile_height = 18;
                }
            }
            172 | 376 => {
                if tile_frame_y % 38 == 18 {
                    tile_height = 18;
                }
            }
            27 => {
                if tile_frame_y % 74 == 54 {
                    tile_height = 18;
                }
            }
            132 | 135 => {
                tile_top = 2;
                tile_height = 18;
            }
            82 | 83 | 84 => {
                tile_height = 20;
                tile_top = -2;
                if x % 2 == 0 {
                    tile_sprite_effect = SpriteEffects::FLIP_HORIZONTALLY;
                }
            }
            324 => {
                tile_width = 20;
                tile_height = 20;
                tile_top = -2;
                if x % 2 == 0 {
                    tile_sprite_effect = SpriteEffects::FLIP_HORIZONTALLY;
                }
            }
            494 => {
                tile_top = 2;
            }
            52 | 62 | 115 | 205 | 382 | 528 | 636 | 638 => {
                tile_top = -2;
                if x % 2 == 0 {
                    tile_sprite_effect = SpriteEffects::FLIP_HORIZONTALLY;
                }
            }
            80 | 142 | 143 => {
                tile_top = 2;
            }
            139 => {
                tile_top = 2;
                let mut num5: i32 = tile_frame_y / 2016;
                add_fr_y -= 2016 * num5;
                add_fr_x += 72 * num5;
            }
            73 | 74 | 113 => {
                tile_top = -12;
                tile_height = 32;
                if x % 2 == 0 {
                    tile_sprite_effect = SpriteEffects::FLIP_HORIZONTALLY;
                }
            }
            388 | 389 => {
                let mut num59: i32 = 94;
                tile_top = -2;
                if tile_frame_y == num59 - 20 || tile_frame_y == num59 * 2 - 20 || tile_frame_y == 0 || tile_frame_y == num59 {
                    tile_height = 18;
                }
                if tile_frame_y != 0 && tile_frame_y != num59 {
                    tile_top = 0;
                }
            }
            227 => {
                tile_width = 32;
                tile_height = 38;
                if tile_frame_x == 238 {
                    tile_top -= 6;
                } else {
                    tile_top -= 20;
                }
                if tile_frame_x == 204 {
                    let (evil, good, crimson) = world_gen_get_cactus_type(ctx.g, x, y, tile_frame_x, tile_frame_y);
                    if good {
                        tile_frame_x += 238;
                    }
                    if evil {
                        tile_frame_x += 204;
                    }
                    if crimson {
                        tile_frame_x += 272;
                    }
                }
                if x % 2 == 0 {
                    tile_sprite_effect = SpriteEffects::FLIP_HORIZONTALLY;
                }
            }
            624 | 700 => {
                tile_width = 20;
                tile_height = 16;
                tile_top += 2;
                if x % 2 == 0 {
                    tile_sprite_effect = SpriteEffects::FLIP_HORIZONTALLY;
                }
            }
            656 | 701 => {
                tile_width = 24;
                tile_height = 34;
                tile_top -= 16;
                if x % 2 == 0 {
                    tile_sprite_effect = SpriteEffects::FLIP_HORIZONTALLY;
                }
            }
            579 => {
                tile_width = 20;
                tile_height = 20;
                tile_top -= 2;
                let mut flag: bool = ((x * 16 + 8) as f32) > unported!("Main.LocalPlayer.Center.X", 0.0_f32); // neutral: the player is at the world origin
                if tile_frame_x > 0 {
                    if flag {
                        add_fr_y = 22;
                    } else {
                        add_fr_y = 0;
                    }
                } else if flag {
                    add_fr_y = 0;
                } else {
                    add_fr_y = 22;
                }
            }
            567 => {
                tile_width = 26;
                tile_height = 18;
                if tile_frame_y == 0 {
                    tile_top = -2;
                }
                if x % 2 == 0 {
                    tile_sprite_effect = SpriteEffects::FLIP_HORIZONTALLY;
                }
            }
            185 | 186 | 187 => {
                tile_top = 2;
                match type_cache {
                    185 => {
                        if tile_frame_y == 18 && tile_frame_x >= 576 && tile_frame_x <= 882 {
                            ctx.g.main.tile_shine2[185] = true;
                        } else {
                            ctx.g.main.tile_shine2[185] = false;
                        }
                        if tile_frame_y == 18 {
                            let mut num27: i32 = tile_frame_x / 1908;
                            add_fr_x -= 1908 * num27;
                            add_fr_y += 18 * num27;
                        }
                    }
                    186 => {
                        if tile_frame_x >= 864 && tile_frame_x <= 1170 {
                            ctx.g.main.tile_shine2[186] = true;
                        } else {
                            ctx.g.main.tile_shine2[186] = false;
                        }
                    }
                    187 => {
                        let mut num26: i32 = tile_frame_x / 1890;
                        add_fr_x -= 1890 * num26;
                        add_fr_y += 36 * num26;
                    }
                    _ => {}
                }
            }
            650 => {
                tile_top = 2;
            }
            649 => {
                tile_top = 2;
                let mut num25: i32 = tile_frame_x / 1908;
                add_fr_x -= 1908 * num25;
                add_fr_y += 18 * num25;
            }
            647 | 706 => {
                tile_top = 2;
            }
            648 => {
                tile_top = 2;
                let mut num24: i32 = tile_frame_x / 1890;
                add_fr_x -= 1890 * num24;
                add_fr_y += 36 * num24;
            }
            178 => {
                if tile_frame_y <= 36 {
                    tile_top = 2;
                }
            }
            184 => {
                tile_width = 20;
                if tile_frame_y <= 36 {
                    tile_top = 2;
                } else if tile_frame_y <= 108 {
                    tile_top = -2;
                }
            }
            519 => {
                tile_top = 2;
                if x % 2 == 0 {
                    tile_sprite_effect = SpriteEffects::FLIP_HORIZONTALLY;
                }
            }
            493 => {
                if tile_frame_y == 0 {
                    let mut num6: i32 = ctx.state.tile_frame_counter[type_cache as usize];
                    let mut num7: f32 = ctx.g.main.wind_for_visuals().abs();
                    let mut num8: i32 = y - tile_frame_y / 18;
                    let mut num9: i32 = x - tile_frame_x / 18;
                    if !world_gen_in_a_place_with_wind(ctx.g, x, num8, 1, 1) {
                        num7 = 0f32;
                    }
                    if !(num7 < 0.1f32) {
                        if num7 < 0.5f32 {
                            num6 /= 20;
                            num6 += num8 + num9;
                            num6 %= 6;
                            num6 = if !(ctx.g.main.wind_for_visuals() < 0f32) { num6 + 1 } else { 6 - num6 };
                            add_fr_y = num6 * 36;
                        } else {
                            num6 /= 10;
                            num6 += num8 + num9;
                            num6 %= 6;
                            num6 = if !(ctx.g.main.wind_for_visuals() < 0f32) { num6 + 7 } else { 12 - num6 };
                            add_fr_y = num6 * 36;
                        }
                    }
                }
                tile_top = 2;
            }
            28 | 105 | 470 | 475 | 506 | 547 | 548 | 552 | 560 | 597 | 613 | 621 | 622 | 623 | 653 | 699 => {
                tile_top = 2;
            }
            617 => {
                tile_top = 2;
                tile_frame_y %= 144;
                tile_frame_x %= 54;
            }
            614 => {
                add_fr_x = ctx.state.tile_frame[type_cache as usize] * 54;
                add_fr_y = 0;
                tile_top = 2;
            }
            81 => {
                tile_top -= 8;
                tile_height = 26;
                tile_width = 24;
                if x % 2 == 0 {
                    tile_sprite_effect = SpriteEffects::FLIP_HORIZONTALLY;
                }
            }
            272 => {
                add_fr_y = 0;
            }
            106 => {
                add_fr_y = ctx.state.tile_frame[type_cache as usize] * 54;
            }
            300 | 301 | 302 | 303 | 304 | 305 | 306 | 307 | 308 | 354 | 355 | 499 => {
                add_fr_y = ctx.state.tile_frame[type_cache as usize] * 54;
                tile_top = 2;
            }
            377 => {
                add_fr_y = ctx.state.tile_frame[type_cache as usize] * 38;
                tile_top = 2;
            }
            463 | 464 => {
                add_fr_y = ctx.state.tile_frame[type_cache as usize] * 72;
                tile_top = 2;
            }
            491 => {
                tile_top = 2;
                add_fr_x = 54;
            }
            379 => {
                add_fr_y = ctx.state.tile_frame[type_cache as usize] * 90;
            }
            349 => {
                tile_top = 2;
                let mut num63: i32 = tile_frame_x % 36;
                let mut num64: i32 = tile_frame_y % 54;
                let frame_data4: i32 = 0; // out value of Animation.GetTemporaryFrame; unused while it reports false
                if unported!("Animation.GetTemporaryFrame", false) {
                    tile_frame_x = (36 * frame_data4 + num63) as i16 as i32;
                }
            }
            441 | 468 => {
                if tile_frame_y == 18 {
                    tile_height = 18;
                }
                let mut num60: i32 = tile_frame_x % 36;
                let mut num61: i32 = tile_frame_y % 38;
                let frame_data3: i32 = 0; // out value of Animation.GetTemporaryFrame; unused while it reports false
                if unported!("Animation.GetTemporaryFrame", false) {
                    tile_frame_y = (38 * frame_data3 + num61) as i16 as i32;
                }
            }
            390 => {
                add_fr_y = ctx.state.tile_frame[type_cache as usize] * 36;
            }
            412 => {
                add_fr_y = 0;
                tile_top = 2;
            }
            36 => {
                tile_top = 2;
            }
            406 => {
                tile_height = 16;
                if tile_frame_y % 54 >= 36 {
                    tile_height = 18;
                }
                let mut num55: i32 = ctx.state.tile_frame[type_cache as usize];
                if tile_frame_y >= 108 {
                    num55 = 6 - tile_frame_y / 54;
                } else if tile_frame_y >= 54 {
                    num55 = ctx.state.tile_frame[type_cache as usize] - 1;
                }
                add_fr_y = num55 * 56;
                add_fr_y += tile_frame_y / 54 * 2;
            }
            452 => {
                let mut num50: i32 = ctx.state.tile_frame[type_cache as usize];
                if tile_frame_x >= 54 {
                    num50 = 0;
                }
                add_fr_y = num50 * 54;
            }
            455 => {
                add_fr_y = 0;
                tile_top = 2;
                let mut num46: i32 = 1 + ctx.state.tile_frame[type_cache as usize];
                if !unported!("BirthdayParty.PartyIsUp", false) {
                    num46 = 0;
                }
                add_fr_y = num46 * 54;
            }
            454 => {
                add_fr_y = ctx.state.tile_frame[type_cache as usize] * 54;
            }
            453 => {
                let mut num44: i32 = ctx.state.tile_frame_counter[type_cache as usize];
                num44 /= 20;
                let mut num45: i32 = y - tile_frame_y / 18;
                num44 += num45 + x;
                num44 %= 3;
                add_fr_y = num44 * 54;
            }
            456 => {
                let mut num41: i32 = ctx.state.tile_frame_counter[type_cache as usize];
                num41 /= 20;
                let mut num42: i32 = y - tile_frame_y / 18;
                let mut num43: i32 = x - tile_frame_x / 18;
                num41 += num42 + num43;
                num41 %= 4;
                add_fr_y = num41 * 54;
            }
            405 => {
                tile_height = 16;
                if tile_frame_y > 0 {
                    tile_height = 18;
                }
                let mut num37: i32 = ctx.state.tile_frame[type_cache as usize];
                if tile_frame_x >= 54 {
                    num37 = 0;
                }
                add_fr_y = num37 * 38;
            }
            12 | 31 | 96 | 639 | 665 | 696 => {
                add_fr_y = ctx.state.tile_frame[type_cache as usize] * 36;
            }
            238 => {
                tile_top = 2;
                add_fr_y = ctx.state.tile_frame[type_cache as usize] * 36;
            }
            593 => {
                if tile_frame_x >= 18 {
                    add_fr_x = -18;
                }
                tile_top = 2;
                let frame_data2: i32 = 0; // out value of Animation.GetTemporaryFrame; unused while it reports false
                if unported!("Animation.GetTemporaryFrame", false) {
                    add_fr_y = (18 * frame_data2) as i16 as i32;
                } else if tile_frame_x < 18 {
                    add_fr_y = ctx.state.tile_frame[type_cache as usize] * 18;
                } else {
                    add_fr_y = 0;
                }
            }
            594 => {
                if tile_frame_x >= 36 {
                    add_fr_x = -36;
                }
                tile_top = 2;
                let mut num29: i32 = tile_frame_x % 36;
                let mut num30: i32 = tile_frame_y % 36;
                let frame_data: i32 = 0; // out value of Animation.GetTemporaryFrame; unused while it reports false
                if unported!("Animation.GetTemporaryFrame", false) {
                    add_fr_y = (36 * frame_data) as i16 as i32;
                } else if tile_frame_x < 36 {
                    add_fr_y = ctx.state.tile_frame[type_cache as usize] * 36;
                } else {
                    add_fr_y = 0;
                }
            }
            592 => {
                add_fr_y = ctx.state.tile_frame[type_cache as usize] * 54;
            }
            228 | 231 | 243 | 247 => {
                tile_top = 2;
                add_fr_y = ctx.state.tile_frame[type_cache as usize] * 54;
            }
            244 => {
                tile_top = 2;
                if tile_frame_x < 54 {
                    add_fr_y = ctx.state.tile_frame[type_cache as usize] * 36;
                } else {
                    add_fr_y = 0;
                }
            }
            565 => {
                tile_top = 2;
                if tile_frame_x < 36 {
                    add_fr_y = ctx.state.tile_frame[type_cache as usize] * 36;
                } else {
                    add_fr_y = 0;
                }
            }
            235 => {
                add_fr_y = ctx.state.tile_frame[type_cache as usize] * 18;
            }
            217 | 218 | 564 => {
                add_fr_y = ctx.state.tile_frame[type_cache as usize] * 36;
                tile_top = 2;
            }
            219 | 220 | 642 => {
                add_fr_y = ctx.state.tile_frame[type_cache as usize] * 54;
                tile_top = 2;
            }
            270 | 271 | 581 => {
                let mut num23: i32 = ctx.state.tile_frame[type_cache as usize] + x % 6;
                if x % 2 == 0 {
                    num23 += 3;
                }
                if x % 3 == 0 {
                    num23 += 3;
                }
                if x % 4 == 0 {
                    num23 += 3;
                }
                while num23 > 5 {
                    num23 -= 6;
                }
                add_fr_x = num23 * 18;
                add_fr_y = 0;
                if x % 2 == 0 {
                    tile_sprite_effect = SpriteEffects::FLIP_HORIZONTALLY;
                }
            }
            660 => {
                let mut num18: i32 = ctx.state.tile_frame[type_cache as usize] + x % 5;
                if x % 2 == 0 {
                    num18 += 3;
                }
                if x % 3 == 0 {
                    num18 += 3;
                }
                if x % 4 == 0 {
                    num18 += 3;
                }
                while num18 > 4 {
                    num18 -= 5;
                }
                add_fr_x = num18 * 18;
                add_fr_y = 0;
                if x % 2 == 0 {
                    tile_sprite_effect = SpriteEffects::FLIP_HORIZONTALLY;
                }
            }
            572 => {
                let mut num17: i32;
                num17 = ctx.state.tile_frame[type_cache as usize] + x % 4;
                while num17 > 3 {
                    num17 -= 4;
                }
                add_fr_x = num17 * 18;
                add_fr_y = 0;
                if x % 2 == 0 {
                    tile_sprite_effect = SpriteEffects::FLIP_HORIZONTALLY;
                }
            }
            428 => {
                tile_top += 4;
                if unported!("PressurePlateHelper.PressurePlatesPressed.ContainsKey", false) {
                    add_fr_x += 18;
                }
            }
            442 => {
                tile_width = 20;
                tile_height = 20;
                match tile_frame_x / 22 {
                    1 => {
                        tile_top = -4;
                    }
                    2 => {
                        tile_top = -2;
                        tile_width = 24;
                    }
                    3 => {
                        tile_top = -2;
                    }
                    _ => {}
                }
            }
            275 | 276 | 277 | 278 | 279 | 280 | 281 | 296 | 297 | 309 | 358 | 359 | 413 | 414 | 542 | 550 | 551 | 553 | 554 | 558 | 559 | 599 | 600 | 601 | 602 | 603 | 604 | 605 | 606 | 607 | 608 | 609 | 610 | 611 | 612 | 632 | 640 | 643 | 644 | 645 | 710 => {
                tile_top = 2;
                ctx.state.critter_cage = true;
                let mut big_animal_cage_frame: i32 = self.get_big_animal_cage_frame(ctx, x, y, tile_frame_x, tile_frame_y);
                match type_cache {
                    275 | 359 | 599 | 600 | 601 | 602 | 603 | 604 | 605 => {
                        add_fr_y = ctx.state.bunny_cage_frame[(big_animal_cage_frame) as usize] * 54;
                    }
                    550 | 551 => {
                        add_fr_y = ctx.state.turtle_cage_frame[(big_animal_cage_frame) as usize] * 54;
                    }
                    542 => {
                        add_fr_y = ctx.state.owl_cage_frame[(big_animal_cage_frame) as usize] * 54;
                    }
                    276 | 413 | 414 | 606 | 607 | 608 | 609 | 610 | 611 | 612 => {
                        add_fr_y = ctx.state.squirrel_cage_frame[(big_animal_cage_frame) as usize] * 54;
                    }
                    277 => {
                        add_fr_y = ctx.state.mallard_cage_frame[(big_animal_cage_frame) as usize] * 54;
                    }
                    278 => {
                        add_fr_y = ctx.state.duck_cage_frame[(big_animal_cage_frame) as usize] * 54;
                    }
                    553 => {
                        add_fr_y = ctx.state.grebe_cage_frame[(big_animal_cage_frame) as usize] * 54;
                    }
                    554 => {
                        add_fr_y = ctx.state.seagull_cage_frame[(big_animal_cage_frame) as usize] * 54;
                    }
                    279 | 358 => {
                        add_fr_y = ctx.state.bird_cage_frame[(big_animal_cage_frame) as usize] * 54;
                    }
                    280 => {
                        add_fr_y = ctx.state.blue_bird_cage_frame[(big_animal_cage_frame) as usize] * 54;
                    }
                    281 => {
                        add_fr_y = ctx.state.red_bird_cage_frame[(big_animal_cage_frame) as usize] * 54;
                    }
                    632 | 640 | 643 | 644 | 645 => {
                        add_fr_y = ctx.state.macaw_cage_frame[(big_animal_cage_frame) as usize] * 54;
                    }
                    296 | 297 => {
                        add_fr_y = ctx.state.scorpion_cage_frame[(0) as usize][(big_animal_cage_frame) as usize] * 54;
                    }
                    309 => {
                        add_fr_y = ctx.state.penguin_cage_frame[(big_animal_cage_frame) as usize] * 54;
                    }
                    558 | 559 => {
                        add_fr_y = ctx.state.seahorse_cage_frame[(big_animal_cage_frame) as usize] * 54;
                    }
                    710 => {
                        let mut num13: i32 = ctx.state.pufferfish_cage_frame[(big_animal_cage_frame) as usize] / 33;
                        add_fr_x = 108 * num13;
                        add_fr_y = (ctx.state.pufferfish_cage_frame[(big_animal_cage_frame) as usize] - num13 * 33) * 54;
                    }
                    _ => {}
                }
            }
            285 | 286 | 298 | 299 | 310 | 339 | 361 | 362 | 363 | 364 | 391 | 392 | 393 | 394 | 532 | 533 | 538 | 544 | 555 | 556 | 582 | 619 | 629 => {
                tile_top = 2;
                ctx.state.critter_cage = true;
                let mut small_animal_cage_frame2: i32 = self.get_small_animal_cage_frame(ctx, x, y, tile_frame_x, tile_frame_y);
                match type_cache {
                    285 => {
                        add_fr_y = ctx.state.snail_cage_frame[(small_animal_cage_frame2) as usize] * 36;
                    }
                    286 | 582 => {
                        add_fr_y = ctx.state.snail2_cage_frame[(small_animal_cage_frame2) as usize] * 36;
                    }
                    298 | 361 => {
                        add_fr_y = ctx.state.frog_cage_frame[(small_animal_cage_frame2) as usize] * 36;
                    }
                    339 | 362 => {
                        add_fr_y = ctx.state.grasshopper_cage_frame[(small_animal_cage_frame2) as usize] * 36;
                    }
                    299 | 363 => {
                        add_fr_y = ctx.state.mouse_cage_frame[(small_animal_cage_frame2) as usize] * 36;
                    }
                    310 | 364 | 391 | 619 => {
                        add_fr_y = ctx.state.worm_cage_frame[(small_animal_cage_frame2) as usize] * 36;
                    }
                    392 | 393 | 394 => {
                        add_fr_y = ctx.state.slug_cage_frame[(type_cache - 392) as usize][(small_animal_cage_frame2) as usize] * 36;
                    }
                    532 => {
                        add_fr_y = ctx.state.maggot_cage_frame[(small_animal_cage_frame2) as usize] * 36;
                    }
                    533 => {
                        add_fr_y = ctx.state.rat_cage_frame[(small_animal_cage_frame2) as usize] * 36;
                    }
                    538 | 544 | 629 => {
                        add_fr_y = ctx.state.ladybug_cage_frame[(small_animal_cage_frame2) as usize] * 36;
                    }
                    555 | 556 => {
                        add_fr_y = ctx.state.water_strider_cage_frame[(small_animal_cage_frame2) as usize] * 36;
                    }
                    _ => {}
                }
            }
            282 | 505 | 543 => {
                tile_top = 2;
                ctx.state.critter_cage = true;
                let mut water_animal_cage_frame5: i32 = self.get_water_animal_cage_frame(ctx, x, y, tile_frame_x, tile_frame_y);
                add_fr_y = ctx.state.fish_bowl_frame[(water_animal_cage_frame5) as usize] * 36;
            }
            598 => {
                tile_top = 2;
                ctx.state.critter_cage = true;
                let mut water_animal_cage_frame4: i32 = self.get_water_animal_cage_frame(ctx, x, y, tile_frame_x, tile_frame_y);
                add_fr_y = ctx.state.lava_fish_bowl_frame[(water_animal_cage_frame4) as usize] * 36;
            }
            568 | 569 | 570 => {
                tile_top = 2;
                ctx.state.critter_cage = true;
                let mut water_animal_cage_frame3: i32 = self.get_water_animal_cage_frame(ctx, x, y, tile_frame_x, tile_frame_y);
                add_fr_y = ctx.state.fairy_jar_frame[(water_animal_cage_frame3) as usize] * 36;
            }
            288 | 289 | 290 | 291 | 292 | 293 | 294 | 295 | 360 | 580 | 620 => {
                tile_top = 2;
                ctx.state.critter_cage = true;
                let mut water_animal_cage_frame2: i32 = self.get_water_animal_cage_frame(ctx, x, y, tile_frame_x, tile_frame_y);
                let mut num12: i32 = type_cache as i32 - 288;
                if type_cache == 360 || type_cache == 580 || type_cache == 620 {
                    num12 = 8;
                }
                add_fr_y = ctx.state.butterfly_cage_frame[(num12) as usize][(water_animal_cage_frame2) as usize] * 36;
            }
            521 | 522 | 523 | 524 | 525 | 526 | 527 => {
                tile_top = 2;
                ctx.state.critter_cage = true;
                let mut water_animal_cage_frame: i32 = self.get_water_animal_cage_frame(ctx, x, y, tile_frame_x, tile_frame_y);
                let mut num11: i32 = type_cache as i32 - 521;
                add_fr_y = ctx.state.dragonfly_jar_frame[(num11) as usize][(water_animal_cage_frame) as usize] * 36;
            }
            316 | 317 | 318 => {
                tile_top = 2;
                ctx.state.critter_cage = true;
                let mut small_animal_cage_frame: i32 = self.get_small_animal_cage_frame(ctx, x, y, tile_frame_x, tile_frame_y);
                let mut num10: i32 = type_cache as i32 - 316;
                add_fr_y = ctx.state.jellyfish_cage_frame[(num10) as usize][(small_animal_cage_frame) as usize] * 36;
            }
            207 => {
                tile_top = 2;
                if tile_frame_y >= 72 {
                    add_fr_y = ctx.state.tile_frame[type_cache as usize];
                    let mut num4: i32 = x;
                    if tile_frame_x % 36 != 0 {
                        num4 -= 1;
                    }
                    add_fr_y += num4 % 6;
                    if add_fr_y >= 6 {
                        add_fr_y -= 6;
                    }
                    add_fr_y *= 72;
                } else {
                    add_fr_y = 0;
                }
            }
            410 => {
                if tile_frame_y == 36 {
                    tile_height = 18;
                }
                if tile_frame_y >= 56 {
                    add_fr_y = ctx.state.tile_frame[type_cache as usize];
                    add_fr_y *= 56;
                } else {
                    add_fr_y = 0;
                }
            }
            480 | 509 | 657 | 720 | 721 | 725 => {
                tile_top = 2;
                if tile_frame_y >= 54 {
                    add_fr_y = ctx.state.tile_frame[type_cache as usize];
                    add_fr_y *= 54;
                } else {
                    add_fr_y = 0;
                }
            }
            658 => {
                tile_top = 2;
                match tile_frame_y / 54 {
                    1 => {
                        add_fr_y = ctx.state.tile_frame[type_cache as usize];
                        add_fr_y *= 54;
                        add_fr_y += 486;
                    }
                    2 => {
                        add_fr_y = ctx.state.tile_frame[type_cache as usize];
                        add_fr_y *= 54;
                        add_fr_y += 972;
                    }
                    // `default:` comes first in the original; it is the arm for every other value (including 0)
                    _ => {
                        add_fr_y = ctx.state.tile_frame[type_cache as usize];
                        add_fr_y *= 54;
                    }
                }
            }
            733 => {
                tile_top = 2;
                if tile_frame_y < 54 {
                    add_fr_x += 54;
                }
                tile_frame_x %= 54;
                tile_frame_y %= 54;
            }
            326 | 327 | 328 | 329 | 345 | 351 | 421 | 422 | 458 | 459 | 708 => {
                add_fr_y = ctx.state.tile_frame[type_cache as usize] * 90;
            }
            541 => {
                add_fr_y = if !self.should_show_invisible_blocks { 90 } else { 0 };
            }
            507 | 508 => {
                let mut num: i32 = 20;
                let mut num2: i32 = (ctx.state.tile_frame_counter[type_cache as usize] + x * 11 + y * 27) % (num * 8);
                add_fr_y = 90 * (num2 / num);
            }
            336 | 340 | 341 | 342 | 343 | 344 => {
                add_fr_y = ctx.state.tile_frame[type_cache as usize] * 90;
                tile_top = 2;
            }
            89 => {
                tile_top = 2;
            }
            102 => {
                tile_top = 2;
            }
            753 => {
                tile_top = 2;
            }
            _ => {}
        }
        if tile_sets::CAMPFIRES[(tile_cache.type_) as usize] {
            if tile_frame_y < 36 {
                add_fr_y = ctx.state.tile_frame[type_cache as usize] * 36;
            } else {
                add_fr_y = 252;
            }
            tile_top = 2;
        }
        if tile_cache.half_brick() {
            half_brick_height = 8;
        }
        match type_cache {
            412 => {
                glow_texture = ctx.glow_mask(202);
                glow_source_rect = Rectangle::new(tile_frame_x, tile_frame_y + add_fr_y, tile_width, tile_height);
                glow_color = Color::from_ints(255, 255, 255, 255);
            }
            657 => {
                if tile_frame_y >= 54 {
                    glow_texture = ctx.glow_mask(330);
                    glow_source_rect = Rectangle::new(tile_frame_x, tile_frame_y + add_fr_y, tile_width, tile_height);
                    glow_color = Color::WHITE;
                }
            }
            656 | 701 => {
                glow_texture = ctx.glow_mask(329);
                glow_source_rect = Rectangle::new(tile_frame_x, tile_frame_y + add_fr_y, tile_width, tile_height);
                glow_color = Color::from_ints(255, 255, 255, 0) * ((ctx.g.main.mouse_text_color as f32) / 255f32);
            }
            634 => {
                glow_texture = ctx.glow_mask(315);
                glow_source_rect = Rectangle::new(tile_frame_x, tile_frame_y + add_fr_y, tile_width, tile_height);
                glow_color = Color::WHITE;
            }
            637 => {
                glow_texture = self.get_tile_draw_texture(ctx, tile_cache, x, y);
                glow_source_rect = Rectangle::new(tile_frame_x + add_fr_x, tile_frame_y + add_fr_y, tile_width, tile_height);
                glow_color = Color::lerp(Color::WHITE, color, 0.75f32);
            }
            638 => {
                glow_texture = ctx.glow_mask(327);
                glow_source_rect = Rectangle::new(tile_frame_x + add_fr_x, tile_frame_y + add_fr_y, tile_width, tile_height - half_brick_height);
                glow_color = Color::lerp(Color::WHITE, color, 0.75f32);
            }
            568 => {
                glow_texture = ctx.glow_mask(268);
                glow_source_rect = Rectangle::new(tile_frame_x, tile_frame_y + add_fr_y, tile_width, tile_height);
                glow_color = Color::WHITE;
            }
            569 => {
                glow_texture = ctx.glow_mask(269);
                glow_source_rect = Rectangle::new(tile_frame_x, tile_frame_y + add_fr_y, tile_width, tile_height);
                glow_color = Color::WHITE;
            }
            570 => {
                glow_texture = ctx.glow_mask(270);
                glow_source_rect = Rectangle::new(tile_frame_x, tile_frame_y + add_fr_y, tile_width, tile_height);
                glow_color = Color::WHITE;
            }
            580 => {
                glow_texture = ctx.glow_mask(289);
                glow_source_rect = Rectangle::new(tile_frame_x, tile_frame_y + add_fr_y, tile_width, tile_height);
                glow_color = Color::from_ints(225, 110, 110, 0);
            }
            564 => {
                if tile_cache.frame_x < 36 {
                    glow_texture = ctx.glow_mask(267);
                    glow_source_rect = Rectangle::new(tile_frame_x, tile_frame_y + add_fr_y, tile_width, tile_height);
                    glow_color = Color::from_ints(200, 200, 200, 0) * ((ctx.g.main.mouse_text_color as f32) / 255f32);
                }
                add_fr_y = 0;
            }
            184 => {
                if tile_cache.frame_x == 110 {
                    glow_texture = ctx.glow_mask(127);
                    glow_source_rect = Rectangle::new(tile_frame_x, tile_frame_y, tile_width, tile_height);
                    glow_color = self.lava_moss_glow;
                }
                if tile_cache.frame_x == 132 {
                    glow_texture = ctx.glow_mask(127);
                    glow_source_rect = Rectangle::new(tile_frame_x, tile_frame_y, tile_width, tile_height);
                    glow_color = self.krypton_moss_glow;
                }
                if tile_cache.frame_x == 154 {
                    glow_texture = ctx.glow_mask(127);
                    glow_source_rect = Rectangle::new(tile_frame_x, tile_frame_y, tile_width, tile_height);
                    glow_color = self.xenon_moss_glow;
                }
                if tile_cache.frame_x == 176 {
                    glow_texture = ctx.glow_mask(127);
                    glow_source_rect = Rectangle::new(tile_frame_x, tile_frame_y, tile_width, tile_height);
                    glow_color = self.argon_moss_glow;
                }
                if tile_cache.frame_x == 198 {
                    glow_texture = ctx.glow_mask(127);
                    glow_source_rect = Rectangle::new(tile_frame_x, tile_frame_y, tile_width, tile_height);
                    glow_color = self.violet_moss_glow;
                }
                if tile_cache.frame_x == 220 {
                    glow_texture = ctx.glow_mask(127);
                    glow_source_rect = Rectangle::new(tile_frame_x, tile_frame_y, tile_width, tile_height);
                    glow_color = Color::from_ints_rgb(ctx.g.main.disco_r, ctx.g.main.disco_g, ctx.g.main.disco_b);
                }
            }
            463 => {
                glow_texture = ctx.glow_mask(243);
                glow_source_rect = Rectangle::new(tile_frame_x, tile_frame_y + add_fr_y, tile_width, tile_height);
                glow_color = Color::from_ints(127, 127, 127, 0);
            }
            19 => {
                let mut num65: i32 = tile_frame_y / 18;
                if num65 == 26 {
                    glow_texture = ctx.glow_mask(65);
                    glow_source_rect = Rectangle::new(tile_frame_x, tile_frame_y % 18, tile_width, tile_height);
                    glow_color = self.martian_glow;
                }
                if num65 == 27 {
                    glow_texture = ctx.glow_mask(112);
                    glow_source_rect = Rectangle::new(tile_frame_x, tile_frame_y % 18, tile_width, tile_height);
                    glow_color = self.meteor_glow;
                }
            }
            90 => {
                let mut num65: i32 = tile_frame_y / 36;
                if num65 == 27 {
                    glow_texture = ctx.glow_mask(52);
                    glow_source_rect = Rectangle::new(tile_frame_x, tile_frame_y % 36, tile_width, tile_height);
                    glow_color = self.martian_glow;
                }
                if num65 == 28 {
                    glow_texture = ctx.glow_mask(113);
                    glow_source_rect = Rectangle::new(tile_frame_x, tile_frame_y % 36, tile_width, tile_height);
                    glow_color = self.meteor_glow;
                }
            }
            79 => {
                let mut num65: i32 = tile_frame_y / 36;
                if num65 == 27 {
                    glow_texture = ctx.glow_mask(53);
                    glow_source_rect = Rectangle::new(tile_frame_x, tile_frame_y % 36, tile_width, tile_height);
                    glow_color = self.martian_glow;
                }
                if num65 == 28 {
                    glow_texture = ctx.glow_mask(114);
                    glow_source_rect = Rectangle::new(tile_frame_x, tile_frame_y % 36, tile_width, tile_height);
                    glow_color = self.meteor_glow;
                }
            }
            89 => {
                let mut num65: i32 = tile_frame_x / 54;
                let mut num68: i32 = tile_frame_x / 1998;
                add_fr_x -= 1998 * num68;
                add_fr_y += 36 * num68;
                if num65 == 29 {
                    glow_texture = ctx.glow_mask(66);
                    glow_source_rect = Rectangle::new(tile_frame_x % 54, tile_frame_y, tile_width, tile_height);
                    glow_color = self.martian_glow;
                }
                if num65 == 30 {
                    glow_texture = ctx.glow_mask(123);
                    glow_source_rect = Rectangle::new(tile_frame_x % 54, tile_frame_y, tile_width, tile_height);
                    glow_color = self.meteor_glow;
                }
            }
            100 => {
                if tile_frame_x / 36 == 0 {
                    let mut num65: i32 = tile_frame_y / 36;
                    if num65 == 27 {
                        glow_texture = ctx.glow_mask(68);
                        glow_source_rect = Rectangle::new(tile_frame_x, tile_frame_y % 36, tile_width, tile_height);
                        glow_color = self.martian_glow;
                    }
                }
            }
            33 => {
                if tile_frame_x / 18 == 0 {
                    let mut num65: i32 = tile_frame_y / 22;
                    if num65 == 26 {
                        glow_texture = ctx.glow_mask(61);
                        glow_source_rect = Rectangle::new(tile_frame_x, tile_frame_y % 22, tile_width, tile_height);
                        glow_color = self.martian_glow;
                    }
                }
            }
            15 | 497 => {
                let mut num65: i32 = tile_frame_y / 40;
                let mut num72: i32 = num65 / 51;
                add_fr_y -= 2040 * num72;
                add_fr_x += 36 * num72;
                if type_cache == 15 {
                    if num65 == 32 {
                        glow_texture = ctx.glow_mask(54);
                        glow_source_rect = Rectangle::new(tile_frame_x, tile_frame_y % 40, tile_width, tile_height);
                        glow_color = self.martian_glow;
                    }
                    if num65 == 33 {
                        glow_texture = ctx.glow_mask(116);
                        glow_source_rect = Rectangle::new(tile_frame_x, tile_frame_y % 40, tile_width, tile_height);
                        glow_color = self.meteor_glow;
                    }
                }
            }
            34 => {
                if tile_frame_x / 54 == 0 {
                    let mut num65: i32 = tile_frame_y / 54;
                    if num65 == 33 {
                        glow_texture = ctx.glow_mask(55);
                        glow_source_rect = Rectangle::new(tile_frame_x, tile_frame_y % 54, tile_width, tile_height);
                        glow_color = self.martian_glow;
                    }
                }
            }
            21 | 467 => {
                let mut num65: i32 = tile_frame_x / 36;
                if num65 == 48 {
                    glow_texture = ctx.glow_mask(56);
                    glow_source_rect = Rectangle::new(tile_frame_x % 36, tile_frame_y, tile_width, tile_height);
                    glow_color = self.martian_glow;
                }
                if num65 == 49 {
                    glow_texture = ctx.glow_mask(117);
                    glow_source_rect = Rectangle::new(tile_frame_x % 36, tile_frame_y, tile_width, tile_height);
                    glow_color = self.meteor_glow;
                }
            }
            441 | 468 => {
                let mut num65: i32 = tile_frame_x / 36;
                if num65 == 48 {
                    glow_texture = ctx.glow_mask(56);
                    glow_source_rect = Rectangle::new(tile_frame_x % 36, tile_frame_y, tile_width, tile_height);
                    glow_color = self.martian_glow;
                }
                if num65 == 49 {
                    glow_texture = ctx.glow_mask(117);
                    glow_source_rect = Rectangle::new(tile_frame_x % 36, tile_frame_y, tile_width, tile_height);
                    glow_color = self.meteor_glow;
                }
            }
            10 => {
                let mut num65: i32 = tile_frame_y / 54;
                if tile_frame_x < 54 && num65 == 32 {
                    glow_texture = ctx.glow_mask(57);
                    glow_source_rect = Rectangle::new(tile_frame_x, tile_frame_y % 54, tile_width, tile_height);
                    glow_color = self.martian_glow;
                }
            }
            11 => {
                let mut num65: i32 = tile_frame_y / 54;
                if tile_frame_x < 54 {
                    if num65 == 32 {
                        glow_texture = ctx.glow_mask(58);
                        glow_source_rect = Rectangle::new(tile_frame_x, tile_frame_y % 54, tile_width, tile_height);
                        glow_color = self.martian_glow;
                    }
                    if num65 == 33 {
                        glow_texture = ctx.glow_mask(119);
                        glow_source_rect = Rectangle::new(tile_frame_x, tile_frame_y % 54, tile_width, tile_height);
                        glow_color = self.meteor_glow;
                    }
                }
            }
            88 => {
                let mut num65: i32 = tile_frame_x / 54;
                let mut num74: i32 = tile_frame_x / 1998;
                add_fr_x -= 1998 * num74;
                add_fr_y += 36 * num74;
                if num65 == 24 {
                    glow_texture = ctx.glow_mask(59);
                    glow_source_rect = Rectangle::new(tile_frame_x % 54, tile_frame_y, tile_width, tile_height);
                    glow_color = self.martian_glow;
                }
                if num65 == 25 {
                    glow_texture = ctx.glow_mask(120);
                    glow_source_rect = Rectangle::new(tile_frame_x % 54, tile_frame_y, tile_width, tile_height);
                    glow_color = self.meteor_glow;
                }
            }
            42 => {
                let mut num65: i32 = tile_frame_y / 36;
                let mut num73: i32 = tile_frame_y / 2016;
                add_fr_y -= 2016 * num73;
                add_fr_x += 36 * num73;
                if num65 == 33 {
                    glow_texture = ctx.glow_mask(63);
                    glow_source_rect = Rectangle::new(tile_frame_x, tile_frame_y % 36, tile_width, tile_height);
                    glow_color = self.martian_glow;
                }
            }
            87 => {
                let mut num65: i32 = tile_frame_x / 54;
                let mut num71: i32 = tile_frame_x / 1998;
                add_fr_x -= 1998 * num71;
                add_fr_y += 36 * num71;
                if num65 == 26 {
                    glow_texture = ctx.glow_mask(64);
                    glow_source_rect = Rectangle::new(tile_frame_x % 54, tile_frame_y, tile_width, tile_height);
                    glow_color = self.martian_glow;
                }
                if num65 == 27 {
                    glow_texture = ctx.glow_mask(121);
                    glow_source_rect = Rectangle::new(tile_frame_x % 54, tile_frame_y, tile_width, tile_height);
                    glow_color = self.meteor_glow;
                }
            }
            14 => {
                let mut num65: i32 = tile_frame_x / 54;
                if num65 == 31 {
                    glow_texture = ctx.glow_mask(67);
                    glow_source_rect = Rectangle::new(tile_frame_x % 54, tile_frame_y, tile_width, tile_height);
                    glow_color = self.martian_glow;
                }
                if num65 == 32 {
                    glow_texture = ctx.glow_mask(124);
                    glow_source_rect = Rectangle::new(tile_frame_x % 54, tile_frame_y, tile_width, tile_height);
                    glow_color = self.meteor_glow;
                }
            }
            93 => {
                let mut num65: i32 = tile_frame_y / 54;
                let mut num70: i32 = tile_frame_y / 1998;
                add_fr_y -= 1998 * num70;
                add_fr_x += 36 * num70;
                tile_top += 2;
                if num65 == 27 {
                    glow_texture = ctx.glow_mask(62);
                    glow_source_rect = Rectangle::new(tile_frame_x, tile_frame_y % 54, tile_width, tile_height);
                    glow_color = self.martian_glow;
                }
            }
            18 => {
                let mut num65: i32 = tile_frame_x / 36;
                if num65 == 27 {
                    glow_texture = ctx.glow_mask(69);
                    glow_source_rect = Rectangle::new(tile_frame_x % 36, tile_frame_y, tile_width, tile_height);
                    glow_color = self.martian_glow;
                }
                if num65 == 28 {
                    glow_texture = ctx.glow_mask(125);
                    glow_source_rect = Rectangle::new(tile_frame_x % 36, tile_frame_y, tile_width, tile_height);
                    glow_color = self.meteor_glow;
                }
            }
            101 => {
                let mut num65: i32 = tile_frame_x / 54;
                let mut num69: i32 = tile_frame_x / 1998;
                add_fr_x -= 1998 * num69;
                add_fr_y += 72 * num69;
                if num65 == 28 {
                    glow_texture = ctx.glow_mask(60);
                    glow_source_rect = Rectangle::new(tile_frame_x % 54, tile_frame_y, tile_width, tile_height);
                    glow_color = self.martian_glow;
                }
                if num65 == 29 {
                    glow_texture = ctx.glow_mask(115);
                    glow_source_rect = Rectangle::new(tile_frame_x % 54, tile_frame_y, tile_width, tile_height);
                    glow_color = self.meteor_glow;
                }
            }
            104 => {
                let mut num65: i32 = tile_frame_x / 36;
                let mut num67: i32 = tile_frame_x / 2016;
                add_fr_x -= 2016 * num67;
                add_fr_y += 90 * num67;
                tile_top = 2;
                if num65 == 24 {
                    glow_texture = ctx.glow_mask(51);
                    glow_source_rect = Rectangle::new(tile_frame_x % 36, tile_frame_y, tile_width, tile_height);
                    glow_color = self.martian_glow;
                }
                if num65 == 25 {
                    glow_texture = ctx.glow_mask(118);
                    glow_source_rect = Rectangle::new(tile_frame_x % 36, tile_frame_y, tile_width, tile_height);
                    glow_color = self.meteor_glow;
                }
            }
            172 => {
                let mut num65: i32 = tile_frame_y / 38;
                let mut num66: i32 = tile_frame_y / 2014;
                add_fr_y -= 2014 * num66;
                add_fr_x += 36 * num66;
                if num65 == 28 {
                    glow_texture = ctx.glow_mask(88);
                    glow_source_rect = Rectangle::new(tile_frame_x, tile_frame_y % 38, tile_width, tile_height);
                    glow_color = self.martian_glow;
                }
                if num65 == 29 {
                    glow_texture = ctx.glow_mask(122);
                    glow_source_rect = Rectangle::new(tile_frame_x, tile_frame_y % 38, tile_width, tile_height);
                    glow_color = self.meteor_glow;
                }
            }
            _ => {}
        }
        d.tile_frame_x = tile_frame_x as i16;
        d.tile_frame_y = tile_frame_y as i16;
        d.tile_width = tile_width;
        d.tile_height = tile_height;
        d.tile_top = tile_top;
        d.half_brick_height = half_brick_height;
        d.add_fr_x = add_fr_x;
        d.add_fr_y = add_fr_y;
        d.tile_sprite_effect = tile_sprite_effect;
        d.glow_texture = glow_texture;
        d.glow_source_rect = glow_source_rect;
        d.glow_color = glow_color;
    }
}

/// Expected values are worked out by hand from TileDrawing.cs:4567-6620 (line numbers in each case).
#[cfg(test)]
mod tests {
    use super::*;
    use crate::draw_context::test_support::with_ctx;

    /// Puts `tile` at (x, y), runs `GetTileDrawData` on it and returns the resulting `TileDrawInfo`.
    fn run(ctx: &mut DrawContext, x: i32, y: i32, tile: Tile) -> TileDrawInfo {
        *ctx.g.main.tile.get_mut(x, y) = tile;
        let mut d = TileDrawInfo::new();
        d.tile_cache = tile;
        d.type_cache = tile.type_;
        d.tile_frame_x = tile.frame_x;
        d.tile_frame_y = tile.frame_y;
        TileDrawing::new().get_tile_draw_data(ctx, x, y, &tile, tile.type_, &mut d);
        d
    }

    fn tile(type_: u16, frame_x: i16, frame_y: i16) -> Tile {
        let mut t = Tile::default();
        t.type_ = type_;
        t.set_active(true);
        t.frame_x = frame_x;
        t.frame_y = frame_y;
        t
    }

    #[test]
    fn plain_block_uses_the_defaults() {
        with_ctx(|ctx| {
            // 4573: addFrY = Main.tileFrame[typeCache] * 38 for every tile; 4569-4574 the other defaults
            ctx.state.tile_frame[0] = 3;
            let d = run(ctx, 10, 10, tile(0, 18, 18));
            assert_eq!((d.tile_width, d.tile_height, d.tile_top, d.half_brick_height, d.add_fr_x, d.add_fr_y), (16, 16, 0, 0, 0, 114));
            assert_eq!(d.tile_sprite_effect, SpriteEffects::NONE);
            assert!(d.glow_texture.is_none());
            assert_eq!((d.tile_frame_x, d.tile_frame_y), (18, 18));
        });
    }

    #[test]
    fn half_brick_sets_half_brick_height() {
        with_ctx(|ctx| {
            let mut t = tile(0, 18, 18);
            t.set_half_brick(true);
            // 6145-6148
            assert_eq!(run(ctx, 10, 10, t).half_brick_height, 8);
        });
    }

    #[test]
    fn plants_are_20_high_and_flip_on_even_columns() {
        with_ctx(|ctx| {
            // 4924-4937: tile 3: tileHeight = 20, FlipHorizontally when x % 2 == 0
            let even = run(ctx, 10, 10, tile(3, 0, 0));
            assert_eq!((even.tile_width, even.tile_height, even.tile_top), (16, 20, 0));
            assert_eq!(even.tile_sprite_effect, SpriteEffects::FLIP_HORIZONTALLY);
            let odd = run(ctx, 11, 10, tile(3, 0, 0));
            assert_eq!(odd.tile_sprite_effect, SpriteEffects::NONE);
        });
    }

    #[test]
    fn torch_hangs_lower_below_a_solid_block() {
        with_ctx(|ctx| {
            // 4729-4736: tile 4 is 20 x 20; tileTop = 4 when WorldGen.SolidTile(x, y - 1)
            let free = run(ctx, 10, 10, tile(4, 0, 0));
            assert_eq!((free.tile_width, free.tile_height, free.tile_top), (20, 20, 0));
            *ctx.g.main.tile.get_mut(10, 9) = tile(1, 18, 18);
            let under_stone = run(ctx, 10, 10, tile(4, 0, 0));
            assert_eq!((under_stone.tile_width, under_stone.tile_height, under_stone.tile_top), (20, 20, 4));
        });
    }

    #[test]
    fn tables_are_18_high_only_on_their_second_row() {
        with_ctx(|ctx| {
            // 5017-5026: tiles 14, 21, 411, 467, 469: tileHeight = 18 when tileFrameY == 18
            assert_eq!(run(ctx, 10, 10, tile(14, 0, 18)).tile_height, 18);
            assert_eq!(run(ctx, 10, 10, tile(14, 0, 0)).tile_height, 16);
        });
    }

    #[test]
    fn beds_wrap_their_frame_row_of_2016() {
        with_ctx(|ctx| {
            // 4983-4990 (tile 79): tileHeight = 18, num33 = tileFrameY / 2016 = 1, addFrY = 0 * 38 - 2016, addFrX = 144.
            // The glow switch (6291-6307) computes tileFrameY / 36 = 57: neither 27 nor 28, so no glow mask.
            let d = run(ctx, 10, 10, tile(79, 0, 2016 + 36));
            assert_eq!((d.tile_height, d.add_fr_y, d.add_fr_x), (18, -2016, 144));
            assert!(d.glow_texture.is_none());
        });
    }

    #[test]
    fn animated_tiles_read_main_tile_frame() {
        with_ctx(|ctx| {
            // 5378-5380 (tile 106): addFrY = Main.tileFrame[typeCache] * 54
            ctx.state.tile_frame[106] = 2;
            assert_eq!(run(ctx, 10, 10, tile(106, 0, 0)).add_fr_y, 108);
            // 6133-6144: campfires animate on their first row (frameY < 36) and show a fixed row (252) below
            assert!(tile_sets::CAMPFIRES[215]);
            ctx.state.tile_frame[215] = 3;
            let lit = run(ctx, 10, 10, tile(215, 0, 0));
            assert_eq!((lit.add_fr_y, lit.tile_top), (108, 2));
            assert_eq!(run(ctx, 10, 10, tile(215, 0, 36)).add_fr_y, 252);
        });
    }

    #[test]
    fn dye_plants_are_32_by_38_and_sit_20_above() {
        with_ctx(|ctx| {
            // 5126-5157 (tile 227): 32 x 38; tileTop = -20, or -6 for frameX 238; flipped on even columns
            let d = run(ctx, 12, 10, tile(227, 0, 0));
            assert_eq!((d.tile_width, d.tile_height, d.tile_top), (32, 38, -20));
            assert_eq!(d.tile_sprite_effect, SpriteEffects::FLIP_HORIZONTALLY);
            assert_eq!(run(ctx, 13, 10, tile(227, 238, 0)).tile_top, -6);
        });
    }

    #[test]
    fn tree_frames_move_by_176_per_biome() {
        with_ctx(|ctx| {
            // 4696-4703: tile 5 is 20 x 20 and tileFrameX += 176 * (GetTreeBiome + 1).
            // GetTreeBiome (1755-1796) walks down the trunk to the first non-trunk tile; GetTreeVariant (1798-1827): 2 (grass) -> -1, 23 (corrupt grass) -> 0.
            *ctx.g.main.tile.get_mut(20, 11) = tile(2, 18, 18);
            let forest = run(ctx, 20, 10, tile(5, 0, 0));
            assert_eq!((forest.tile_width, forest.tile_height, forest.tile_frame_x), (20, 20, 0));
            *ctx.g.main.tile.get_mut(20, 11) = tile(23, 18, 18);
            let corrupt = run(ctx, 20, 10, tile(5, 0, 0));
            assert_eq!(corrupt.tile_frame_x, 176);
        });
    }

    #[test]
    fn tiles_with_a_glow_mask_name_their_rectangle() {
        with_ctx(|ctx| {
            // 6257-6272 (tile 19, a platform): frameY / 18 == 26 is the martian platform: GlowMask[65], rectangle
            // (frameX, frameY % 18, tileWidth, tileHeight). The texture file is not in the test install, so only the rectangle is checked.
            let d = run(ctx, 10, 10, tile(19, 36, 26 * 18 + 4));
            assert_eq!(d.glow_source_rect, Rectangle::new(36, 4, 16, 16));
        });
    }
}
