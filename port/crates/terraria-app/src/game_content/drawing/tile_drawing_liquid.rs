//! `TileDrawing.DrawLiquidBehindTiles`, `DrawTile_LiquidBehindTile` and `DrawPartialLiquid` (TileDrawing.cs:449, 3856, 4531): the liquid that
//! shows around and behind solid tiles (a tile's own half of a liquid column, the sides of slopes, the film on top of a floor).

use terraria_core::xna::{Rectangle, Vector2};
use terraria_core::Tile;
use terraria_data::sets::tile_id::sets as tile_sets;
use terraria_sim::world_gen::tile_queries::solid_tile;

use crate::draw_context::DrawContext;
use crate::game_content::drawing::tile_drawing::get_screen_draw_area;
use crate::gfx::SpriteEffects;
use crate::main_liquid::{is_liquid_style_water, HIDE_WATER};
use crate::pending::{lighting_get_corner_colors, VertexColors};

/// `TileDrawing.DrawLiquidBehindTiles(int waterStyleOverride = -1)` (TileDrawing.cs:449)
pub fn draw_liquid_behind_tiles(ctx: &mut DrawContext, water_style_override: i32) {
    ctx.tile_batch.restart(ctx.list);
    let unscaled_position = Vector2::new(ctx.g.main.screen_position.0, ctx.g.main.screen_position.1);
    let (draw_off_set, first_tile_x, last_tile_x, first_tile_y, last_tile_y) = get_screen_draw_area(ctx, !ctx.state.draw_to_screen);
    for i in first_tile_y..last_tile_y + 4 {
        for j in first_tile_x - 2..last_tile_x + 2 {
            if let Some(tile) = ctx.g.main.tile.try_get(j, i) {
                ctx.tile_batch.set_layer(0, 0);
                draw_tile_liquid_behind_tile(ctx, false, water_style_override, unscaled_position, draw_off_set, j, i, &tile);
            }
        }
    }
    let _value = ctx.tile_batch.end(ctx.list); // TimeLogger.LiquidBackgroundDrawCalls.Add(value)
}

/// `DrawTile_LiquidBehindTile(bool solidLayer, int waterStyleOverride, Vector2 screenPosition, Vector2 screenOffset, int tileX, int tileY, Tile tileCache)` (TileDrawing.cs:3856)
#[allow(clippy::too_many_arguments)]
pub fn draw_tile_liquid_behind_tile(ctx: &mut DrawContext, solid_layer: bool, water_style_override: i32, screen_position: Vector2, screen_offset: Vector2, tile_x: i32, tile_y: i32, tile_cache: &Tile) {
    let mut num = 0;
    let mut num2 = 0;
    let mut num3 = 0;
    let mut num4 = 0;
    let mut num5 = 0;
    let mut num6 = 0;
    let mut num7 = 0;
    let mut num8 = 0;
    if let Some(tile) = ctx.g.main.tile.try_get(tile_x + 1, tile_y) {
        if tile.type_ != 379 {
            num = tile.liquid as i32;
            num2 = tile.liquid_type() as i32;
        }
    }
    if let Some(tile2) = ctx.g.main.tile.try_get(tile_x - 1, tile_y) {
        if tile2.type_ != 379 {
            num3 = tile2.liquid as i32;
            num4 = tile2.liquid_type() as i32;
        }
    }
    if let Some(tile3) = ctx.g.main.tile.try_get(tile_x, tile_y - 1) {
        if tile3.type_ != 379 {
            num5 = tile3.liquid as i32;
            num6 = tile3.liquid_type() as i32;
        }
    }
    if let Some(tile4) = ctx.g.main.tile.try_get(tile_x, tile_y + 1) {
        if tile4.type_ != 379 {
            num7 = tile4.liquid as i32;
            num8 = tile4.liquid_type() as i32;
        }
    }
    if HIDE_WATER
        || !tile_cache.active()
        || tile_cache.in_active()
        || ctx.g.main.tile_solid_top[tile_cache.type_ as usize]
        || (tile_cache.half_brick() && (num3 > 160 || num > 160) && ctx.state.liquid.waterfall_manager.check_for_waterfall(tile_x, tile_y))
        || (tile_sets::BLOCKS_WATER_DRAWING_BEHIND_SELF[tile_cache.type_ as usize] && tile_cache.slope() == 0)
    {
        return;
    }
    let mut num9 = 0;
    let mut flag = false;
    let mut flag2 = false;
    let mut flag3 = false;
    let mut flag4 = false;
    let mut flag5 = false;
    let mut num10 = 0;
    let mut flag6 = false;
    let num11 = tile_cache.slope() as i32;
    let num12 = tile_cache.block_type();
    if tile_cache.type_ == 379 && tile_cache.liquid > 0 {
        return;
    }
    if tile_cache.type_ == 546 && tile_cache.liquid > 0 {
        flag5 = true;
        flag4 = true;
        flag = true;
        flag2 = true;
        match tile_cache.liquid_type() {
            0 => flag6 = true,
            1 => num10 = 1,
            2 => num10 = 11,
            3 => num10 = 14,
            _ => {}
        }
        num9 = tile_cache.liquid as i32;
    } else {
        if tile_cache.liquid > 0 && num12 != 0 && (num12 != 1 || tile_cache.liquid > 160) {
            flag5 = true;
            match tile_cache.liquid_type() {
                0 => flag6 = true,
                1 => num10 = 1,
                2 => num10 = 11,
                3 => num10 = 14,
                _ => {}
            }
            if tile_cache.liquid as i32 > num9 {
                num9 = tile_cache.liquid as i32;
            }
        }
        if num3 > 0 && num11 != 1 && num11 != 3 {
            flag = true;
            match num4 {
                0 => flag6 = true,
                1 => num10 = 1,
                2 => num10 = 11,
                3 => num10 = 14,
                _ => {}
            }
            if num3 > num9 {
                num9 = num3;
            }
        }
        if num > 0 && num11 != 2 && num11 != 4 {
            flag2 = true;
            match num2 {
                0 => flag6 = true,
                1 => num10 = 1,
                2 => num10 = 11,
                3 => num10 = 14,
                _ => {}
            }
            if num > num9 {
                num9 = num;
            }
        }
        if num5 > 0 && num11 != 3 && num11 != 4 {
            flag3 = true;
            match num6 {
                0 => flag6 = true,
                1 => num10 = 1,
                2 => num10 = 11,
                3 => num10 = 14,
                _ => {}
            }
        }
        if num7 > 0 && num11 != 1 && num11 != 2 {
            if num7 > 240 {
                flag4 = true;
            }
            match num8 {
                0 => flag6 = true,
                1 => num10 = 1,
                2 => num10 = 11,
                3 => num10 = 14,
                _ => {}
            }
        }
    }
    if !flag3 && !flag4 && !flag && !flag2 && !flag5 {
        return;
    }
    if water_style_override != -1 {
        ctx.g.main.water_style = water_style_override;
    }
    if num10 == 0 {
        num10 = ctx.g.main.water_style;
    }
    let mut vertices = lighting_get_corner_colors(ctx.g, tile_x, tile_y);
    let mut vector = Vector2::new((tile_x * 16) as f32, (tile_y * 16) as f32);
    let mut liquid_size = Rectangle::new(0, 4, 16, 16);
    if flag4 && (flag || flag2) {
        flag = true;
        flag2 = true;
    }
    if tile_cache.active() && (ctx.g.main.tile_solid_top[tile_cache.type_ as usize] || !ctx.g.main.tile_solid[tile_cache.type_ as usize]) {
        return;
    }
    if (!flag3 || !(flag || flag2)) && !(flag4 && flag3) {
        if flag3 {
            liquid_size = Rectangle::new(0, 4, 16, 4);
            if tile_cache.half_brick() || tile_cache.slope() != 0 {
                liquid_size = Rectangle::new(0, 4, 16, 12);
            }
        } else if flag4 && !flag && !flag2 {
            vector = Vector2::new((tile_x * 16) as f32, (tile_y * 16 + 12) as f32);
            liquid_size = Rectangle::new(0, 4, 16, 4);
        } else {
            let num13 = (256 - num9) as f32 / 32.0;
            let mut y = 4;
            if num5 == 0 && (num12 != 0 || !solid_tile(ctx.g, tile_x, tile_y - 1, false)) {
                y = 0;
            }
            let num14 = (num13 as i32) * 2;
            if tile_cache.slope() != 0 {
                vector = Vector2::new((tile_x * 16) as f32, (tile_y * 16 + num14) as f32);
                liquid_size = Rectangle::new(0, num14, 16, 16 - num14);
            } else if (flag && flag2) || tile_cache.half_brick() {
                vector = Vector2::new((tile_x * 16) as f32, (tile_y * 16 + num14) as f32);
                liquid_size = Rectangle::new(0, y, 16, 16 - num14);
            } else if flag {
                vector = Vector2::new((tile_x * 16) as f32, (tile_y * 16 + num14) as f32);
                liquid_size = Rectangle::new(0, y, 4, 16 - num14);
            } else {
                vector = Vector2::new((tile_x * 16 + 12) as f32, (tile_y * 16 + num14) as f32);
                liquid_size = Rectangle::new(0, y, 4, 16 - num14);
            }
        }
    }
    let mut position = vector - screen_position + screen_offset;
    let mut num15 = 0.5_f32;
    match num10 {
        1 => num15 = ctx.state.liquid.lava_opacity, // Main.player[Main.myPlayer].lavaOpacity
        11 => num15 = (num15 * 1.7).max(1.0),
        _ => {}
    }
    if (num10 != 1 || !(ctx.state.liquid.lava_opacity < 1.0)) && ((tile_y as f64) <= ctx.g.main.world_surface || num15 > 1.0) {
        num15 = 1.0;
        if tile_cache.wall == 21 {
            num15 = 0.9;
        } else if tile_cache.wall > 0 {
            num15 = 0.6;
        }
    }
    if tile_cache.half_brick() && num5 > 0 && tile_cache.wall > 0 {
        num15 = 0.0;
    }
    if num11 == 4 && num3 == 0 && !solid_tile(ctx.g, tile_x - 1, tile_y, false) {
        num15 = 0.0;
    }
    if num11 == 3 && num == 0 && !solid_tile(ctx.g, tile_x + 1, tile_y, false) {
        num15 = 0.0;
    }
    vertices.bottom_left_color = vertices.bottom_left_color * num15;
    vertices.bottom_right_color = vertices.bottom_right_color * num15;
    vertices.top_left_color = vertices.top_left_color * num15;
    vertices.top_right_color = vertices.top_right_color * num15;
    if tile_cache.half_brick() && num5 > 0 && (tile_y as f64) > ctx.g.main.world_surface {
        vertices.top_left_color = vertices.top_left_color * 0.0;
        vertices.top_right_color = vertices.top_right_color * 0.0;
    }
    let mut flag7 = false;
    if flag6 {
        for i in 0..15 {
            if is_liquid_style_water(i) && ctx.g.main.liquid_alpha[i as usize] > 0.0 && i != num10 {
                draw_partial_liquid(ctx, !solid_layer, tile_cache, &mut position, &mut liquid_size, i, &vertices);
                flag7 = true;
                break;
            }
        }
    }
    let mut colors = vertices;
    let num16 = if flag7 { ctx.g.main.liquid_alpha[num10 as usize] } else { 1.0 };
    colors.bottom_left_color = colors.bottom_left_color * num16;
    colors.bottom_right_color = colors.bottom_right_color * num16;
    colors.top_left_color = colors.top_left_color * num16;
    colors.top_right_color = colors.top_right_color * num16;
    if num10 == 14 {
        crate::game_content::liquid::liquid_renderer::set_shimmer_vertex_colors(ctx.g, &mut colors, if solid_layer { 0.75 } else { 1.0 }, tile_x, tile_y);
    }
    draw_partial_liquid(ctx, !solid_layer, tile_cache, &mut position, &mut liquid_size, num10, &colors);
}

/// `DrawPartialLiquid(bool behindBlocks, Tile tileCache, ref Vector2 position, ref Rectangle liquidSize, int liquidType, ref VertexColors colors)` (TileDrawing.cs:4531)
fn draw_partial_liquid(ctx: &mut DrawContext, behind_blocks: bool, tile_cache: &Tile, position: &mut Vector2, liquid_size: &mut Rectangle, liquid_type: i32, colors: &VertexColors) {
    let num = tile_cache.slope() as i32;
    let mut flag = !tile_sets::BLOCKS_WATER_DRAWING_BEHIND_SELF[tile_cache.type_ as usize];
    if !behind_blocks {
        flag = false;
    }
    if flag || num == 0 {
        let texture = ctx.liquid_texture(liquid_type);
        ctx.tile_batch.draw(texture.as_ref(), *position, *liquid_size, *colors, Vector2::ZERO, 1.0, SpriteEffects::NONE);
        return;
    }
    liquid_size.x += 18 * (num - 1);
    match num {
        1..=4 => {
            let texture = ctx.liquid_slope_texture(liquid_type);
            ctx.tile_batch.draw(texture.as_ref(), *position, *liquid_size, *colors, Vector2::ZERO, 1.0, SpriteEffects::NONE);
        }
        _ => {}
    }
}
