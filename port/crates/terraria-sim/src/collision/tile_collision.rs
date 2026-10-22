//! `Terraria.Collision`, moving boxes against tiles: `TileCollision`, `TileCollisionInStepsOf16`,
//! `TryChangingSizeFromBottomCenter`, `IsClearSpotTest`, `FindCollisionTile`, `FindCollisionDirection`,
//! `SolidCollision`, `WaterCollision`, `AnyCollisionWithSpecificTiles`, `AnyCollision` (Collision.cs:2309-3037).

use terraria_core::xna::{Point, Rectangle, Vector2, Vector4};
use terraria_data::sets::tile_id;

use super::slopes::slope_collision;
use super::utils::{clamp_i32, math_min_f32, platform_proper_top_frame, tile_at, xy, yzw, zw, xzw};
use crate::game::Game;

/// `Collision.TileCollision(Position, Velocity, Width, Height, fallThrough = false, fall2 = false, gravDir = 1,
/// ignoreDoors = false, ignoreAetheriumPlatforms = false, hoik = true)` (Collision.cs:2309). Sets `g.collision.up` / `down`.
pub fn tile_collision(
    g: &mut Game,
    position: Vector2,
    velocity: Vector2,
    width: i32,
    height: i32,
    fall_through: bool,
    fall2: bool,
    grav_dir: i32,
    ignore_doors: bool,
    ignore_aetherium_platforms: bool,
    hoik: bool,
) -> Vector2 {
    g.collision.up = false;
    g.collision.down = false;
    let mut result = velocity;
    let vector = velocity;
    let vector2 = position + velocity;
    let vector3 = position;
    let value = (position.x / 16f32) as i32 - 1;
    let mut value2 = ((position.x + width as f32) / 16f32) as i32 + 2;
    let mut value3 = (position.y / 16f32) as i32 - 1;
    let mut value4 = ((position.y + height as f32) / 16f32) as i32 + 2;
    let mut num = -1;
    let mut num2 = -1;
    let mut num3 = -1;
    let mut num4 = -1;
    let num5 = clamp_i32(value, 0, g.main.max_tiles_x - 1);
    value2 = clamp_i32(value2, 0, g.main.max_tiles_x - 1);
    value3 = clamp_i32(value3, 0, g.main.max_tiles_y - 40);
    value4 = clamp_i32(value4, 0, g.main.max_tiles_y - 40);
    let mut num6 = ((value4 + 3) * 16) as f32;
    let mut vector4 = Vector2::default();
    for i in num5..value2 {
        for j in value3..value4 {
            let tile = tile_at(g, i, j);
            if !tile.active() || tile.in_active() {
                continue;
            }
            let mut flag = g.main.tile_solid[tile.type_ as usize];
            if g.main.tile_solid_top[tile.type_ as usize] && tile.frame_y == 0 {
                flag = true;
            }
            if ignore_doors && tile_id::sets::for_advanced_collision::CLOSED_DOORS[tile.type_ as usize] {
                flag = false;
            }
            if ignore_aetherium_platforms && tile.type_ == 19 && tile.frame_y / 18 == 50 {
                flag = false;
            }
            if !flag {
                continue;
            }
            vector4.x = (i * 16) as f32;
            vector4.y = (j * 16) as f32;
            let mut num7 = 16;
            if tile_at(g, i, j).half_brick() {
                vector4.y += 8f32;
                num7 -= 8;
            }
            if !(vector2.x + width as f32 > vector4.x) || !(vector2.x < vector4.x + 16f32) || !(vector2.y + height as f32 > vector4.y) || !(vector2.y < vector4.y + num7 as f32) {
                continue;
            }
            let mut flag2 = false;
            let mut flag3 = false;
            if tile_at(g, i, j).slope() > 2 {
                if tile_at(g, i, j).slope() == 3 && vector3.y + velocity.x.abs() >= vector4.y && vector3.x >= vector4.x {
                    flag3 = true;
                }
                if tile_at(g, i, j).slope() == 4 && vector3.y + velocity.x.abs() >= vector4.y && vector3.x + width as f32 <= vector4.x + 16f32 {
                    flag3 = true;
                }
            } else if tile_at(g, i, j).slope() > 0 {
                flag2 = true;
                if tile_at(g, i, j).slope() == 1 && vector3.y + height as f32 - velocity.x.abs() <= vector4.y + num7 as f32 && vector3.x >= vector4.x {
                    flag3 = true;
                }
                if tile_at(g, i, j).slope() == 2 && vector3.y + height as f32 - velocity.x.abs() <= vector4.y + num7 as f32 && vector3.x + width as f32 <= vector4.x + 16f32 {
                    flag3 = true;
                }
            }
            if flag3 {
                continue;
            }
            if vector3.y + height as f32 <= vector4.y {
                g.collision.down = true;
                if (!(g.main.tile_solid_top[tile_at(g, i, j).type_ as usize] && fall_through) || !(velocity.y <= 1f32 || fall2)) && num6 > vector4.y {
                    num3 = i;
                    num4 = j;
                    if num7 < 16 {
                        num4 += 1;
                    }
                    if num3 != num && !flag2 {
                        result.y = vector4.y - (vector3.y + height as f32) + (if grav_dir == -1 { -0.01f32 } else { 0f32 });
                        num6 = vector4.y;
                    }
                }
            } else if vector3.x + width as f32 <= vector4.x && !g.main.tile_solid_top[tile_at(g, i, j).type_ as usize] {
                // `if (i >= 1 && Main.tile[i - 1, j] == null) Main.tile[i - 1, j] = new Tile();`: the index is inside the world, so the tile exists
                if !hoik || i < 1 || (tile_at(g, i - 1, j).slope() != 2 && tile_at(g, i - 1, j).slope() != 4) {
                    num = i;
                    num2 = j;
                    if num2 != num4 {
                        result.x = vector4.x - (vector3.x + width as f32);
                    }
                    if num3 == num {
                        result.y = vector.y;
                    }
                }
            } else if vector3.x >= vector4.x + 16f32 && !g.main.tile_solid_top[tile_at(g, i, j).type_ as usize] {
                // `if (Main.tile[i + 1, j] == null) Main.tile[i + 1, j] = new Tile();`: the index is inside the world, so the tile exists
                if !hoik || (tile_at(g, i + 1, j).slope() != 1 && tile_at(g, i + 1, j).slope() != 3) {
                    num = i;
                    num2 = j;
                    if num2 != num4 {
                        result.x = vector4.x + 16f32 - vector3.x;
                    }
                    if num3 == num {
                        result.y = vector.y;
                    }
                }
            } else if vector3.y >= vector4.y + num7 as f32 && !g.main.tile_solid_top[tile_at(g, i, j).type_ as usize] {
                g.collision.up = true;
                num3 = i;
                num4 = j;
                result.y = vector4.y + num7 as f32 - vector3.y + (if grav_dir == 1 { 0.01f32 } else { 0f32 });
                if num4 == num2 {
                    result.x = vector.x;
                }
            }
        }
    }
    result
}

/// `Collision.TileCollisionInStepsOf16(Position, normalizedDirection, amount, Width, Height)` (Collision.cs:2472)
pub fn tile_collision_in_steps_of_16(g: &mut Game, position: Vector2, normalized_direction: Vector2, amount: f32, width: i32, height: i32) -> f32 {
    let num = 16f32;
    let vector = Vector2::new(position.x + 1f32, position.y);
    let width_ = width - 1;
    let mut num2 = 0f32;
    while num2 < amount {
        let vector2 = normalized_direction * num2;
        let position_ = position + vector2;
        let vector3 = vector + vector2;
        let num3 = math_min_f32(num, amount - num2);
        let vector4 = normalized_direction * num3;
        let mut vec = slope_collision(g, vector3, vector4, width_, height, 0f32, false, false);
        if xy(vec) != vector3 || zw(vec) != vector4 {
            let num4 = (xy(vec) - vector3).length();
            return num2 - num4;
        }
        vec = slope_collision(g, vector3 + vector4, vector4, width_, height, 0f32, false, false);
        if xy(vec) != vector3 + vector4 || zw(vec) != vector4 {
            let num5 = (xy(vec) - vector3 - vector4).length();
            return num2 + num3 - num5;
        }
        let vector5 = tile_collision(g, position_, vector4, width, height, false, false, 1, false, false, true);
        if vector5 != vector4 {
            return num2 + vector5.length();
        }
        num2 += num;
    }
    amount
}

/// `Collision.TryChangingSizeFromBottomCenter(hitbox, targetWidth, targetHeight, out changedHitbox)` (Collision.cs:2505).
/// Returns `(result, changedHitbox)`.
pub fn try_changing_size_from_bottom_center(g: &mut Game, hitbox: Rectangle, target_width: i32, target_height: i32) -> (bool, Rectangle) {
    let mut changed_hitbox = hitbox;
    let num = target_height - hitbox.height;
    if num > 0 {
        let top_left = Vector2::new(changed_hitbox.x as f32, changed_hitbox.y as f32);
        let num2 = tile_collision_in_steps_of_16(g, top_left, Vector2::UNIT_Y, num as f32, changed_hitbox.width, changed_hitbox.height) as i32;
        let top_left = Vector2::new(changed_hitbox.x as f32, changed_hitbox.y as f32);
        let num3 = tile_collision_in_steps_of_16(g, top_left, -Vector2::UNIT_Y, num as f32, changed_hitbox.width, changed_hitbox.height) as i32;
        if num3 + num2 < num {
            return (false, changed_hitbox);
        }
        let mut num4 = num.min(num3);
        let mut num5 = 0;
        let mut val = num - num4 - num5;
        num4 += val.min(num3 - num4);
        val = num - num4 - num5;
        num5 += val.min(num2 - num5);
        changed_hitbox.offset_xy(0, -num4);
        changed_hitbox.height += num4 + num5;
    } else {
        changed_hitbox.offset_xy(0, -num);
        changed_hitbox.height = target_height;
    }
    let num6 = target_width - hitbox.width;
    if num6 > 0 {
        let top_left = Vector2::new(changed_hitbox.x as f32, changed_hitbox.y as f32);
        let num7 = tile_collision_in_steps_of_16(g, top_left, Vector2::UNIT_X, num6 as f32, changed_hitbox.width, changed_hitbox.height) as i32;
        let top_left = Vector2::new(changed_hitbox.x as f32, changed_hitbox.y as f32);
        let num8 = tile_collision_in_steps_of_16(g, top_left, -Vector2::UNIT_X, num6 as f32, changed_hitbox.width, changed_hitbox.height) as i32;
        if num8 + num7 < num6 {
            return (false, changed_hitbox);
        }
        let mut num10;
        let mut num9 = {
            num10 = (num6 / 2).min(num8.min(num7));
            num10
        };
        let mut val2 = num6 - num10 - num9;
        num10 += val2.min(num8 - num10);
        val2 = num6 - num10 - num9;
        num9 += val2.min(num7 - num9);
        changed_hitbox.offset_xy(-num10, 0);
        changed_hitbox.width += num10 + num9;
    } else {
        changed_hitbox.offset_xy(num6 / 2, 0);
        changed_hitbox.width = target_width;
    }
    (true, changed_hitbox)
}

/// `Collision.IsClearSpotTest(position, testMagnitude, Width, Height, fallThrough = false, fall2 = false, gravDir = 1,
/// checkCardinals = true, checkSlopes = false)` (Collision.cs:2557)
pub fn is_clear_spot_test(
    g: &mut Game,
    position: Vector2,
    test_magnitude: f32,
    width: i32,
    height: i32,
    fall_through: bool,
    fall2: bool,
    grav_dir: i32,
    check_cardinals: bool,
    check_slopes: bool,
) -> bool {
    if check_cardinals {
        let mut vector = Vector2::UNIT_X * test_magnitude;
        if tile_collision(g, position - vector, vector, width, height, fall_through, fall2, grav_dir, false, false, true) != vector {
            return false;
        }
        vector = -Vector2::UNIT_X * test_magnitude;
        if tile_collision(g, position - vector, vector, width, height, fall_through, fall2, grav_dir, false, false, true) != vector {
            return false;
        }
        vector = Vector2::UNIT_Y * test_magnitude;
        if tile_collision(g, position - vector, vector, width, height, fall_through, fall2, grav_dir, false, false, true) != vector {
            return false;
        }
        vector = -Vector2::UNIT_Y * test_magnitude;
        if tile_collision(g, position - vector, vector, width, height, fall_through, fall2, grav_dir, false, false, true) != vector {
            return false;
        }
    }
    if check_slopes {
        let mut vector = Vector2::UNIT_X * test_magnitude;
        let mut vector2 = Vector4::from_vector2(position, test_magnitude, 0f32);
        if slope_collision(g, position, vector, width, height, grav_dir as f32, fall_through, false) != vector2 {
            return false;
        }
        vector = -Vector2::UNIT_X * test_magnitude;
        vector2 = Vector4::from_vector2(position, 0f32 - test_magnitude, 0f32);
        if slope_collision(g, position, vector, width, height, grav_dir as f32, fall_through, false) != vector2 {
            return false;
        }
        vector = Vector2::UNIT_Y * test_magnitude;
        vector2 = Vector4::from_vector2(position, 0f32, test_magnitude);
        if slope_collision(g, position, vector, width, height, grav_dir as f32, fall_through, false) != vector2 {
            return false;
        }
        vector = -Vector2::UNIT_Y * test_magnitude;
        vector2 = Vector4::from_vector2(position, 0f32, 0f32 - test_magnitude);
        if slope_collision(g, position, vector, width, height, grav_dir as f32, fall_through, false) != vector2 {
            return false;
        }
    }
    true
}

/// `Collision.FindCollisionTile(Direction, position, testMagnitude, Width, Height, fallThrough = false, fall2 = false,
/// gravDir = 1, checkCardinals = true, checkSlopes = false)` (Collision.cs:2612)
pub fn find_collision_tile(
    g: &mut Game,
    direction: i32,
    position: Vector2,
    test_magnitude: f32,
    width: i32,
    height: i32,
    fall_through: bool,
    fall2: bool,
    grav_dir: i32,
    check_cardinals: bool,
    check_slopes: bool,
) -> Vec<Point> {
    let mut list: Vec<Point> = Vec::new();
    if (direction as u32) > 1u32 {
        if ((direction.wrapping_sub(2)) as u32) <= 1u32 {
            let vector = if direction == 2 { Vector2::UNIT_Y * test_magnitude } else { -Vector2::UNIT_Y * test_magnitude };
            let vec = Vector4::from_vector2(position, vector.x, vector.y);
            let y = (position.y + (if direction == 2 { height } else { 0 }) as f32) as i32 / 16;
            let num = math_min_f32(16f32 - position.x % 16f32, width as f32);
            let mut num2 = num;
            if check_cardinals && tile_collision(g, position - vector, vector, num as i32, height, fall_through, fall2, grav_dir, false, false, true) != vector {
                list.push(Point::new(position.x as i32 / 16, y));
            } else if check_slopes && yzw(slope_collision(g, position, vector, num as i32, height, grav_dir as f32, fall_through, false)) != yzw(vec) {
                list.push(Point::new(position.x as i32 / 16, y));
            }
            while num2 + 16f32 <= (width - 16) as f32 {
                if check_cardinals && tile_collision(g, position - vector + Vector2::UNIT_X * num2, vector, 16, height, fall_through, fall2, grav_dir, false, false, true) != vector {
                    list.push(Point::new((position.x + num2) as i32 / 16, y));
                } else if check_slopes && yzw(slope_collision(g, position + Vector2::UNIT_X * num2, vector, 16, height, grav_dir as f32, fall_through, false)) != yzw(vec) {
                    list.push(Point::new((position.x + num2) as i32 / 16, y));
                }
                num2 += 16f32;
            }
            let width_ = width - num2 as i32;
            if check_cardinals && tile_collision(g, position - vector + Vector2::UNIT_X * num2, vector, width_, height, fall_through, fall2, grav_dir, false, false, true) != vector {
                list.push(Point::new((position.x + num2) as i32 / 16, y));
            } else if check_slopes && yzw(slope_collision(g, position + Vector2::UNIT_X * num2, vector, width_, height, grav_dir as f32, fall_through, false)) != yzw(vec) {
                list.push(Point::new((position.x + num2) as i32 / 16, y));
            }
        }
    } else {
        let vector = if direction == 0 { Vector2::UNIT_X * test_magnitude } else { -Vector2::UNIT_X * test_magnitude };
        let vec = Vector4::from_vector2(position, vector.x, vector.y);
        let y = (position.x + (if direction == 0 { width } else { 0 }) as f32) as i32 / 16;
        let num3 = math_min_f32(16f32 - position.y % 16f32, height as f32);
        let mut num4 = num3;
        if check_cardinals && tile_collision(g, position - vector, vector, width, num3 as i32, fall_through, fall2, grav_dir, false, false, true) != vector {
            list.push(Point::new(y, position.y as i32 / 16));
        } else if check_slopes && xzw(slope_collision(g, position, vector, width, num3 as i32, grav_dir as f32, fall_through, false)) != xzw(vec) {
            list.push(Point::new(y, position.y as i32 / 16));
        }
        while num4 + 16f32 <= (height - 16) as f32 {
            if check_cardinals && tile_collision(g, position - vector + Vector2::UNIT_Y * num4, vector, width, 16, fall_through, fall2, grav_dir, false, false, true) != vector {
                list.push(Point::new(y, (position.y + num4) as i32 / 16));
            } else if check_slopes && xzw(slope_collision(g, position + Vector2::UNIT_Y * num4, vector, width, 16, grav_dir as f32, fall_through, false)) != xzw(vec) {
                list.push(Point::new(y, (position.y + num4) as i32 / 16));
            }
            num4 += 16f32;
        }
        let height_ = height - num4 as i32;
        if check_cardinals && tile_collision(g, position - vector + Vector2::UNIT_Y * num4, vector, width, height_, fall_through, fall2, grav_dir, false, false, true) != vector {
            list.push(Point::new(y, (position.y + num4) as i32 / 16));
        } else if check_slopes && xzw(slope_collision(g, position + Vector2::UNIT_Y * num4, vector, width, height_, grav_dir as f32, fall_through, false)) != xzw(vec) {
            list.push(Point::new(y, (position.y + num4) as i32 / 16));
        }
    }
    list
}

/// `Collision.FindCollisionDirection(out int Direction, position, Width, Height, fallThrough = false, fall2 = false, gravDir = 1)`
/// (Collision.cs:2693). Returns `(result, Direction)`.
pub fn find_collision_direction(g: &mut Game, position: Vector2, width: i32, height: i32, fall_through: bool, fall2: bool, grav_dir: i32) -> (bool, i32) {
    let mut vector = Vector2::UNIT_X * 16f32;
    if tile_collision(g, position - vector, vector, width, height, fall_through, fall2, grav_dir, false, false, true) != vector {
        return (true, 0);
    }
    vector = -Vector2::UNIT_X * 16f32;
    if tile_collision(g, position - vector, vector, width, height, fall_through, fall2, grav_dir, false, false, true) != vector {
        return (true, 1);
    }
    vector = Vector2::UNIT_Y * 16f32;
    if tile_collision(g, position - vector, vector, width, height, fall_through, fall2, grav_dir, false, false, true) != vector {
        return (true, 2);
    }
    vector = -Vector2::UNIT_Y * 16f32;
    if tile_collision(g, position - vector, vector, width, height, fall_through, fall2, grav_dir, false, false, true) != vector {
        return (true, 3);
    }
    (false, -1)
}

/// `Collision.SolidCollision(Vector2 Position, int Width, int Height)` (Collision.cs:2723), the 3-argument overload.
pub fn solid_collision(g: &Game, position: Vector2, width: i32, height: i32) -> bool {
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
            let t = tile_at(g, i, j);
            if !t.in_active() && t.active() && g.main.tile_solid[t.type_ as usize] && !g.main.tile_solid_top[t.type_ as usize] {
                vector.x = (i * 16) as f32;
                vector.y = (j * 16) as f32;
                let mut num2 = 16;
                if t.half_brick() {
                    vector.y += 8f32;
                    num2 -= 8;
                }
                if position.x + width as f32 > vector.x && position.x < vector.x + 16f32 && position.y + height as f32 > vector.y && position.y < vector.y + num2 as f32 {
                    return true;
                }
            }
        }
    }
    false
}

/// `Collision.SolidCollision(Vector2 Position, int Width, int Height, bool acceptTopSurfaces)` (Collision.cs:2758), the
/// 4-argument overload.
pub fn solid_collision_accept_top_surfaces(g: &Game, position: Vector2, width: i32, height: i32, accept_top_surfaces: bool) -> bool {
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
            let tile = tile_at(g, i, j);
            if !tile.active() || tile.in_active() {
                continue;
            }
            let mut flag = g.main.tile_solid[tile.type_ as usize] && !g.main.tile_solid_top[tile.type_ as usize];
            if accept_top_surfaces {
                flag = if !tile_id::sets::PLATFORMS[tile.type_ as usize] {
                    flag | (g.main.tile_solid_top[tile.type_ as usize] && tile.frame_y == 0)
                } else {
                    flag | platform_proper_top_frame(tile.frame_x)
                };
            }
            if flag {
                vector.x = (i * 16) as f32;
                vector.y = (j * 16) as f32;
                let mut num2 = 16;
                if tile.half_brick() {
                    vector.y += 8f32;
                    num2 -= 8;
                }
                if position.x + width as f32 > vector.x && position.x < vector.x + 16f32 && position.y + height as f32 > vector.y && position.y < vector.y + num2 as f32 {
                    return true;
                }
            }
        }
    }
    false
}

/// `Collision.WaterCollision(Position, Velocity, Width, Height, fallThrough = false, fall2 = false, lavaWalk = true)` (Collision.cs:2803)
pub fn water_collision(g: &Game, position: Vector2, velocity: Vector2, width: i32, height: i32, fall_through: bool, _fall2: bool, lava_walk: bool) -> Vector2 {
    let mut result = velocity;
    let vector = position + velocity;
    let vector2 = position;
    let value = (position.x / 16f32) as i32 - 1;
    let mut value2 = ((position.x + width as f32) / 16f32) as i32 + 2;
    let mut value3 = (position.y / 16f32) as i32 - 1;
    let mut value4 = ((position.y + height as f32) / 16f32) as i32 + 2;
    let num = clamp_i32(value, 0, g.main.max_tiles_x - 1);
    value2 = clamp_i32(value2, 0, g.main.max_tiles_x - 1);
    value3 = clamp_i32(value3, 0, g.main.max_tiles_y - 40);
    value4 = clamp_i32(value4, 0, g.main.max_tiles_y - 40);
    let mut vector3 = Vector2::default();
    for i in num..value2 {
        for j in value3..value4 {
            if tile_at(g, i, j).liquid > 0 && tile_at(g, i, j - 1).liquid == 0 && (!tile_at(g, i, j).lava() || lava_walk) {
                let num2 = tile_at(g, i, j).liquid as i32 / 32 * 2 + 2;
                vector3.x = (i * 16) as f32;
                vector3.y = (j * 16 + 16 - num2) as f32;
                if vector.x + width as f32 > vector3.x
                    && vector.x < vector3.x + 16f32
                    && vector.y + height as f32 > vector3.y
                    && vector.y < vector3.y + num2 as f32
                    && vector2.y + height as f32 <= vector3.y
                    && !fall_through
                {
                    result.y = vector3.y - (vector2.y + height as f32);
                }
            }
        }
    }
    result
}

/// `Collision.AnyCollisionWithSpecificTiles(Position, Velocity, Width, Height, bool[] tilesWeCanCollideWithByType, evenActuated = false)`
/// (Collision.cs:2836)
pub fn any_collision_with_specific_tiles(
    g: &Game,
    position: Vector2,
    velocity: Vector2,
    width: i32,
    height: i32,
    tiles_we_can_collide_with_by_type: &[bool],
    even_actuated: bool,
) -> Vector2 {
    let mut result = velocity;
    let vector = velocity;
    let vector2 = position + velocity;
    let vector3 = position;
    let mut num = (position.x / 16f32) as i32 - 1;
    let mut num2 = ((position.x + width as f32) / 16f32) as i32 + 2;
    let mut num3 = (position.y / 16f32) as i32 - 1;
    let mut num4 = ((position.y + height as f32) / 16f32) as i32 + 2;
    let mut num5 = -1;
    let mut num6 = -1;
    let mut num7 = -1;
    let mut num8 = -1;
    if num < 0 {
        num = 0;
    }
    if num2 > g.main.max_tiles_x {
        num2 = g.main.max_tiles_x;
    }
    if num3 < 0 {
        num3 = 0;
    }
    if num4 > g.main.max_tiles_y - 40 {
        num4 = g.main.max_tiles_y - 40;
    }
    let mut vector4 = Vector2::default();
    for i in num..num2 {
        for j in num3..num4 {
            let tile = tile_at(g, i, j);
            if !tile.active() || (!even_actuated && tile.in_active()) || !tiles_we_can_collide_with_by_type[tile.type_ as usize] {
                continue;
            }
            vector4.x = (i * 16) as f32;
            vector4.y = (j * 16) as f32;
            let mut num9 = 16;
            if tile.half_brick() {
                vector4.y += 8f32;
                num9 -= 8;
            }
            if !(vector2.x + width as f32 > vector4.x) || !(vector2.x < vector4.x + 16f32) || !(vector2.y + height as f32 > vector4.y) || !(vector2.y < vector4.y + num9 as f32) {
                continue;
            }
            if vector3.y + height as f32 <= vector4.y {
                num7 = i;
                num8 = j;
                if num7 != num5 {
                    result.y = vector4.y - (vector3.y + height as f32);
                }
            } else if vector3.x + width as f32 <= vector4.x && !g.main.tile_solid_top[tile.type_ as usize] {
                num5 = i;
                num6 = j;
                if num6 != num8 {
                    result.x = vector4.x - (vector3.x + width as f32);
                }
                if num7 == num5 {
                    result.y = vector.y;
                }
            } else if vector3.x >= vector4.x + 16f32 && !g.main.tile_solid_top[tile.type_ as usize] {
                num5 = i;
                num6 = j;
                if num6 != num8 {
                    result.x = vector4.x + 16f32 - vector3.x;
                }
                if num7 == num5 {
                    result.y = vector.y;
                }
            } else if vector3.y >= vector4.y + num9 as f32 && !g.main.tile_solid_top[tile.type_ as usize] {
                num7 = i;
                num8 = j;
                result.y = vector4.y + num9 as f32 - vector3.y + 0.01f32;
                if num8 == num6 {
                    result.x = vector.x + 0.01f32;
                }
            }
        }
    }
    result
}

/// `Collision.AnyCollision(Position, Velocity, Width, Height, evenActuated = false)` (Collision.cs:2938)
pub fn any_collision(g: &Game, position: Vector2, velocity: Vector2, width: i32, height: i32, even_actuated: bool) -> Vector2 {
    let mut result = velocity;
    let vector = velocity;
    let vector2 = position + velocity;
    let vector3 = position;
    let mut num = (position.x / 16f32) as i32 - 1;
    let mut num2 = ((position.x + width as f32) / 16f32) as i32 + 2;
    let mut num3 = (position.y / 16f32) as i32 - 1;
    let mut num4 = ((position.y + height as f32) / 16f32) as i32 + 2;
    let mut num5 = -1;
    let mut num6 = -1;
    let mut num7 = -1;
    let mut num8 = -1;
    if num < 0 {
        num = 0;
    }
    if num2 > g.main.max_tiles_x {
        num2 = g.main.max_tiles_x;
    }
    if num3 < 0 {
        num3 = 0;
    }
    if num4 > g.main.max_tiles_y - 40 {
        num4 = g.main.max_tiles_y - 40;
    }
    let mut vector4 = Vector2::default();
    for i in num..num2 {
        for j in num3..num4 {
            let t = tile_at(g, i, j);
            if !t.active() || (!even_actuated && t.in_active()) {
                continue;
            }
            vector4.x = (i * 16) as f32;
            vector4.y = (j * 16) as f32;
            let mut num9 = 16;
            if t.half_brick() {
                vector4.y += 8f32;
                num9 -= 8;
            }
            if !(vector2.x + width as f32 > vector4.x) || !(vector2.x < vector4.x + 16f32) || !(vector2.y + height as f32 > vector4.y) || !(vector2.y < vector4.y + num9 as f32) {
                continue;
            }
            if vector3.y + height as f32 <= vector4.y {
                num7 = i;
                num8 = j;
                if num7 != num5 {
                    result.y = vector4.y - (vector3.y + height as f32);
                }
            } else if vector3.x + width as f32 <= vector4.x && !g.main.tile_solid_top[t.type_ as usize] {
                num5 = i;
                num6 = j;
                if num6 != num8 {
                    result.x = vector4.x - (vector3.x + width as f32);
                }
                if num7 == num5 {
                    result.y = vector.y;
                }
            } else if vector3.x >= vector4.x + 16f32 && !g.main.tile_solid_top[t.type_ as usize] {
                num5 = i;
                num6 = j;
                if num6 != num8 {
                    result.x = vector4.x + 16f32 - vector3.x;
                }
                if num7 == num5 {
                    result.y = vector.y;
                }
            } else if vector3.y >= vector4.y + num9 as f32 && !g.main.tile_solid_top[t.type_ as usize] {
                num7 = i;
                num8 = j;
                result.y = vector4.y + num9 as f32 - vector3.y + 0.01f32;
                if num8 == num6 {
                    result.x = vector.x + 0.01f32;
                }
            }
        }
    }
    result
}
