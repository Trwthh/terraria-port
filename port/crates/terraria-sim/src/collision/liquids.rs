//! `Terraria.Collision`, liquids and points: `DrownCollision`, `IsWorldPointSolid`, `GetWaterLine`,
//! `GetWaterLineIterate`, `WetCollision`, `LavaCollision` (Collision.cs:1395-1699).

use terraria_core::xna::{Point, Vector2};
use terraria_data::sets::tile_id;

use super::utils::{clamp_i32, tile_at, tile_or_new, tile_slot, to_tile_coordinates};
use crate::game::Game;
use crate::world_gen::tile_frame::in_world;

/// `Collision.DrownCollision(Position, Width, Height, gravDir = -1f, includeSlopes = false)` (Collision.cs:1395)
pub fn drown_collision(g: &Game, position: Vector2, width: i32, height: i32, grav_dir: f32, include_slopes: bool) -> bool {
    let mut vector = Vector2::new(position.x + (width / 2) as f32, position.y + (height / 2) as f32);
    let mut num = 10;
    let mut num2 = 12;
    if num > width {
        num = width;
    }
    if num2 > height {
        num2 = height;
    }
    vector = Vector2::new(vector.x - (num / 2) as f32, position.y + -2f32);
    if grav_dir == -1f32 {
        vector.y += (height / 2 - 6) as f32;
    }
    let value = (position.x / 16f32) as i32 - 1;
    let mut value2 = ((position.x + width as f32) / 16f32) as i32 + 2;
    let mut value3 = (position.y / 16f32) as i32 - 1;
    let mut value4 = ((position.y + height as f32) / 16f32) as i32 + 2;
    let num3 = clamp_i32(value, 0, g.main.max_tiles_x - 1);
    value2 = clamp_i32(value2, 0, g.main.max_tiles_x - 1);
    value3 = clamp_i32(value3, 0, g.main.max_tiles_y - 40);
    value4 = clamp_i32(value4, 0, g.main.max_tiles_y - 40);
    let num4 = if grav_dir == 1f32 { value3 } else { value4 - 1 };
    let mut vector2 = Vector2::default();
    for i in num3..value2 {
        for j in value3..value4 {
            let Some(tile) = tile_slot(g, i, j) else { continue }; // `tile != null &&`
            if tile.liquid > 0
                && !tile.lava()
                && !tile.shimmer()
                && (j != num4 || !tile.active() || !g.main.tile_solid[tile.type_ as usize] || g.main.tile_solid_top[tile.type_ as usize] || (include_slopes && tile.block_type() != 0))
            {
                vector2.x = (i * 16) as f32;
                vector2.y = (j * 16) as f32;
                let mut num5 = 16;
                let mut num6 = (256 - tile_at(g, i, j).liquid as i32) as f32;
                num6 /= 32f32;
                vector2.y += num6 * 2f32;
                num5 -= (num6 * 2f32) as i32;
                if vector.x + num as f32 > vector2.x && vector.x < vector2.x + 16f32 && vector.y + num2 as f32 > vector2.y && vector.y < vector2.y + num5 as f32 {
                    return true;
                }
            }
        }
    }
    false
}

/// `Collision.IsWorldPointSolid(pos, treatPlatformsAsNonSolid = false)` (Collision.cs:1447)
pub fn is_world_point_solid(g: &Game, pos: Vector2, treat_platforms_as_non_solid: bool) -> bool {
    let point = to_tile_coordinates(pos);
    if !in_world(g, point.x, point.y, 1) {
        return false;
    }
    let tile = tile_at(g, point.x, point.y);
    if !tile.active() || tile.in_active() || !g.main.tile_solid[tile.type_ as usize] {
        return false;
    }
    if treat_platforms_as_non_solid && tile.type_ > 0 && tile.type_ as i32 <= terraria_data::ids::tile_id::COUNT && (tile_id::sets::PLATFORMS[tile.type_ as usize] || tile.type_ == 380) {
        return false;
    }
    let num = tile.block_type();
    match num {
        0 => {
            if pos.x >= (point.x * 16) as f32 && pos.x <= (point.x * 16 + 16) as f32 && pos.y >= (point.y * 16) as f32 {
                return pos.y <= (point.y * 16 + 16) as f32;
            }
            return false;
        }
        1 => {
            if pos.x >= (point.x * 16) as f32 && pos.x <= (point.x * 16 + 16) as f32 && pos.y >= (point.y * 16 + 8) as f32 {
                return pos.y <= (point.y * 16 + 16) as f32;
            }
            return false;
        }
        2 | 3 | 4 | 5 => {
            if pos.x < (point.x * 16) as f32 && pos.x > (point.x * 16 + 16) as f32 && pos.y < (point.y * 16) as f32 && pos.y > (point.y * 16 + 16) as f32 {
                return false;
            }
            let num2 = pos.x % 16f32;
            let num3 = pos.y % 16f32;
            match num {
                3 => return num2 + num3 >= 16f32,
                2 => return num3 >= num2,
                5 => return num3 <= num2,
                4 => return num2 + num3 <= 16f32,
                _ => {}
            }
        }
        _ => {}
    }
    false
}

/// `Collision.GetWaterLine(Point pt, out float waterLineHeight)` (Collision.cs:1506). Returns `(result, waterLineHeight)`.
pub fn get_water_line_point(g: &Game, pt: Point) -> (bool, f32) {
    get_water_line(g, pt.x, pt.y)
}

/// `Collision.GetWaterLine(int X, int Y, out float waterLineHeight)` (Collision.cs:1511). Returns `(result, waterLineHeight)`.
pub fn get_water_line(g: &Game, x: i32, y: i32) -> (bool, f32) {
    let mut water_line_height = 0f32;
    if !in_world(g, x, y, 10) {
        return (false, water_line_height);
    }
    // `if (Main.tile[X, Y - 2] == null) Main.tile[X, Y - 2] = new Tile();` and the same for Y - 1, Y, Y + 1: unreachable, InWorld(X, Y, 10) holds
    if tile_at(g, x, y - 2).liquid > 0 {
        return (false, water_line_height);
    }
    if tile_at(g, x, y - 1).liquid > 0 {
        water_line_height = (y * 16) as f32;
        water_line_height -= (tile_at(g, x, y - 1).liquid / 16) as f32;
        return (true, water_line_height);
    }
    if tile_at(g, x, y).liquid > 0 {
        water_line_height = ((y + 1) * 16) as f32;
        water_line_height -= (tile_at(g, x, y).liquid / 16) as f32;
        return (true, water_line_height);
    }
    if tile_at(g, x, y + 1).liquid > 0 {
        water_line_height = ((y + 2) * 16) as f32;
        water_line_height -= (tile_at(g, x, y + 1).liquid / 16) as f32;
        return (true, water_line_height);
    }
    (false, water_line_height)
}

/// `Collision.GetWaterLineIterate(Point pt, out float waterLineHeight)` (Collision.cs:1559). Returns `(result, waterLineHeight)`.
pub fn get_water_line_iterate_point(g: &Game, pt: Point) -> (bool, f32) {
    get_water_line_iterate(g, pt.x, pt.y)
}

/// `Collision.GetWaterLineIterate(int X, int Y, out float waterLineHeight)` (Collision.cs:1564). Returns `(result, waterLineHeight)`.
pub fn get_water_line_iterate(g: &Game, x: i32, mut y: i32) -> (bool, f32) {
    let mut water_line_height = 0f32;
    // `Framing.GetTileSafely(X, Y)` is an empty tile outside the world (Framing.cs:434)
    while y > 0 && terraria_core::framing::get_tile_safely(&g.main.tile, x, y).liquid > 0 {
        y -= 1;
    }
    y += 1;
    // `if (Main.tile[X, Y] == null) Main.tile[X, Y] = new Tile();`: Y can be outside the world here (GetTileSafely stops the loop there)
    if tile_or_new(g, x, y).liquid > 0 {
        water_line_height = (y * 16) as f32;
        water_line_height -= (tile_at(g, x, y - 1).liquid / 16) as f32;
        return (true, water_line_height);
    }
    (false, water_line_height)
}

/// `Collision.WetCollision(Position, Width, Height)` (Collision.cs:1585). Sets `g.collision.honey` and `g.collision.shimmer`.
pub fn wet_collision(g: &mut Game, position: Vector2, width: i32, height: i32) -> bool {
    g.collision.honey = false;
    g.collision.shimmer = false;
    let mut vector = Vector2::new(position.x + (width / 2) as f32, position.y + (height / 2) as f32);
    let mut num = 10;
    let mut num2 = height / 2;
    if num > width {
        num = width;
    }
    if num2 > height {
        num2 = height;
    }
    vector = Vector2::new(vector.x - (num / 2) as f32, vector.y - (num2 / 2) as f32);
    let value = (position.x / 16f32) as i32 - 1;
    let mut value2 = ((position.x + width as f32) / 16f32) as i32 + 2;
    let mut value3 = (position.y / 16f32) as i32 - 1;
    let mut value4 = ((position.y + height as f32) / 16f32) as i32 + 2;
    let num3 = clamp_i32(value, 0, g.main.max_tiles_x - 1);
    value2 = clamp_i32(value2, 0, g.main.max_tiles_x - 1);
    value3 = clamp_i32(value3, 0, g.main.max_tiles_y - 40);
    value4 = clamp_i32(value4, 0, g.main.max_tiles_y - 40);
    let mut vector2 = Vector2::default();
    for i in num3..value2 {
        for j in value3..value4 {
            if tile_at(g, i, j).liquid > 0 {
                vector2.x = (i * 16) as f32;
                vector2.y = (j * 16) as f32;
                let mut num4 = 16;
                let mut num5 = (256 - tile_at(g, i, j).liquid as i32) as f32;
                num5 /= 32f32;
                vector2.y += num5 * 2f32;
                num4 -= (num5 * 2f32) as i32;
                if vector.x + num as f32 > vector2.x && vector.x < vector2.x + 16f32 && vector.y + num2 as f32 > vector2.y && vector.y < vector2.y + num4 as f32 {
                    if tile_at(g, i, j).honey() {
                        g.collision.honey = true;
                    }
                    if tile_at(g, i, j).shimmer() {
                        g.collision.shimmer = true;
                    }
                    return true;
                }
            } else {
                if !tile_at(g, i, j).active() || tile_at(g, i, j).slope() == 0 || j <= 0 || tile_at(g, i, j - 1).liquid == 0 {
                    continue;
                }
                vector2.x = (i * 16) as f32;
                vector2.y = (j * 16) as f32;
                let num6 = 16;
                if vector.x + num as f32 > vector2.x && vector.x < vector2.x + 16f32 && vector.y + num2 as f32 > vector2.y && vector.y < vector2.y + num6 as f32 {
                    if tile_at(g, i, j - 1).honey() {
                        g.collision.honey = true;
                    } else if tile_at(g, i, j - 1).shimmer() {
                        g.collision.shimmer = true;
                    }
                    return true;
                }
            }
        }
    }
    false
}

/// `Collision.LavaCollision(Position, Width, Height)` (Collision.cs:1667)
pub fn lava_collision(g: &Game, position: Vector2, width: i32, height: i32) -> bool {
    let value = (position.x / 16f32) as i32 - 1;
    let mut value2 = ((position.x + width as f32) / 16f32) as i32 + 2;
    let mut value3 = (position.y / 16f32) as i32 - 1;
    let mut value4 = ((position.y + height as f32) / 16f32) as i32 + 2;
    let num = clamp_i32(value, 0, g.main.max_tiles_x - 1);
    value2 = clamp_i32(value2, 0, g.main.max_tiles_x - 1);
    value3 = clamp_i32(value3, 0, g.main.max_tiles_y - 40);
    value4 = clamp_i32(value4, 0, g.main.max_tiles_y - 40);
    let mut vector = Vector2::default();
    for i in num..value2 {
        for j in value3..value4 {
            if tile_at(g, i, j).liquid > 0 && tile_at(g, i, j).lava() {
                vector.x = (i * 16) as f32;
                vector.y = (j * 16) as f32;
                let mut num2 = 16;
                let mut num3 = (256 - tile_at(g, i, j).liquid as i32) as f32;
                num3 /= 32f32;
                vector.y += num3 * 2f32;
                num2 -= (num3 * 2f32) as i32;
                if position.x + width as f32 > vector.x && position.x < vector.x + 16f32 && position.y + height as f32 > vector.y && position.y < vector.y + num2 as f32 {
                    return true;
                }
            }
        }
    }
    false
}
