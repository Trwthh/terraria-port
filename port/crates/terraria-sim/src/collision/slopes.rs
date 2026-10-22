//! `Terraria.Collision`, slopes: `WalkDownSlope`, `SlopeCollision`, `noSlopeCollision`, `BuildTileContacts`
//! (Collision.cs:1701-2307).

use terraria_core::xna::{Rectangle, Vector2, Vector4};

use super::utils::{clamp_i32, math_max_f32, math_min_f32, tile_at, BitsByte};
use super::{tile_collision, TileContact, TileContactSide};
use crate::game::Game;
use terraria_data::sets::tile_id;

/// `Collision.WalkDownSlope(Position, Velocity, Width, Height, gravity = 0f)` (Collision.cs:1701)
pub fn walk_down_slope(g: &Game, position: Vector2, mut velocity: Vector2, width: i32, height: i32, gravity: f32) -> Vector4 {
    if velocity.y != gravity {
        return Vector4::from_vector2(position, velocity.x, velocity.y);
    }
    let vector = position;
    let mut value = (vector.x / 16f32) as i32;
    let mut value2 = ((vector.x + width as f32) / 16f32) as i32;
    let mut value3 = ((position.y + height as f32 + 4f32) / 16f32) as i32;
    value = clamp_i32(value, 0, g.main.max_tiles_x - 1);
    value2 = clamp_i32(value2, 0, g.main.max_tiles_x - 1);
    value3 = clamp_i32(value3, 0, g.main.max_tiles_y - 2 - 40);
    let mut num = ((value3 + 3) * 16) as f32;
    let mut num2 = -1;
    let mut num3 = -1;
    let mut num4 = 1;
    if velocity.x < 0f32 {
        num4 = 2;
    }
    for i in value..=value2 {
        for j in value3..=value3 + 1 {
            let t = tile_at(g, i, j);
            if !t.nactive() || (!g.main.tile_solid[t.type_ as usize] && !g.main.tile_solid_top[t.type_ as usize]) {
                continue;
            }
            let mut num5 = j * 16;
            if t.half_brick() {
                num5 += 8;
            }
            if !Rectangle::new(i * 16, j * 16 - 17, 16, 16).intersects(Rectangle::new(position.x as i32, position.y as i32, width, height)) || !(num5 as f32 <= num) {
                continue;
            }
            if num == num5 as f32 {
                if t.slope() == 0 {
                    continue;
                }
                if num2 != -1 && num3 != -1 && tile_at(g, num2, num3).slope() != 0 {
                    if t.slope() as i32 == num4 {
                        num = num5 as f32;
                        num2 = i;
                        num3 = j;
                    }
                } else {
                    num = num5 as f32;
                    num2 = i;
                    num3 = j;
                }
            } else {
                num = num5 as f32;
                num2 = i;
                num3 = j;
            }
        }
    }
    let num6 = num2;
    let num7 = num3;
    if num2 != -1 && num3 != -1 && tile_at(g, num6, num7).slope() > 0 {
        let num8 = tile_at(g, num6, num7).slope();
        let mut vector2 = Vector2::default();
        vector2.x = (num6 * 16) as f32;
        vector2.y = (num7 * 16) as f32;
        match num8 {
            2 => {
                let num9 = vector2.x + 16f32 - (position.x + width as f32);
                if position.y + height as f32 >= vector2.y + num9 && velocity.x < 0f32 {
                    velocity.y += velocity.x.abs();
                }
            }
            1 => {
                let num9 = position.x - vector2.x;
                if position.y + height as f32 >= vector2.y + num9 && velocity.x > 0f32 {
                    velocity.y += velocity.x.abs();
                }
            }
            _ => {}
        }
    }
    Vector4::from_vector2(position, velocity.x, velocity.y)
}

/// `Collision.SlopeCollision(Position, Velocity, Width, Height, gravity = 0f, fall = false, ignoreAetheriumPlatforms = false)`
/// (Collision.cs:1806). Sets `stair`, `stairFall`, `sloping` and (through `TileCollision`) `up`, `down`, `honey`... of `g.collision`.
pub fn slope_collision(g: &mut Game, position: Vector2, velocity: Vector2, width: i32, height: i32, gravity: f32, fall: bool, ignore_aetherium_platforms: bool) -> Vector4 {
    let _ = gravity; // unused in the original as well
    g.collision.stair = false;
    g.collision.stair_fall = false;
    let mut bits_byte = BitsByte::new(0);
    let mut y = position.y;
    let mut y2 = position.y;
    g.collision.sloping = false;
    let vector = position;
    let mut vector2 = position;
    let mut vector3 = velocity;
    let value = (position.x / 16f32) as i32 - 1;
    let mut value2 = ((position.x + width as f32) / 16f32) as i32 + 2;
    let mut value3 = (position.y / 16f32) as i32 - 1;
    let mut value4 = ((position.y + height as f32) / 16f32) as i32 + 2;
    let num = clamp_i32(value, 0, g.main.max_tiles_x - 1);
    value2 = clamp_i32(value2, 0, g.main.max_tiles_x - 1);
    value3 = clamp_i32(value3, 0, g.main.max_tiles_y - 40);
    value4 = clamp_i32(value4, 0, g.main.max_tiles_y - 40);
    let mut vector4 = Vector2::default();
    for i in num..value2 {
        for j in value3..value4 {
            let tile = tile_at(g, i, j);
            if !tile.active() || tile.in_active() {
                continue;
            }
            let mut flag = g.main.tile_solid[tile.type_ as usize];
            if g.main.tile_solid_top[tile.type_ as usize] && tile.frame_y == 0 {
                flag = true;
            }
            if ignore_aetherium_platforms && tile.type_ == 19 && tile.frame_y / 18 == 50 {
                flag = false;
            }
            if !flag {
                continue;
            }
            vector4.x = (i * 16) as f32;
            vector4.y = (j * 16) as f32;
            let mut num2 = 16;
            if tile_at(g, i, j).half_brick() {
                vector4.y += 8f32;
                num2 -= 8;
            }
            if !(position.x + width as f32 > vector4.x) || !(position.x < vector4.x + 16f32) || !(position.y + height as f32 > vector4.y) || !(position.y < vector4.y + num2 as f32) {
                continue;
            }
            let mut flag2 = true;
            if tile_id::sets::PLATFORMS[tile_at(g, i, j).type_ as usize] {
                if velocity.y < 0f32 {
                    flag2 = false;
                }
                if position.y + (height as f32) < (j * 16) as f32 || position.y + height as f32 - (1f32 + velocity.x.abs()) > (j * 16 + 16) as f32 {
                    flag2 = false;
                }
                if ((tile_at(g, i, j).slope() == 1 && velocity.x >= 0f32) || (tile_at(g, i, j).slope() == 2 && velocity.x <= 0f32))
                    && (position.y + height as f32) / 16f32 - 1f32 == j as f32
                {
                    flag2 = false;
                }
            }
            if !flag2 {
                continue;
            }
            let mut flag3 = false;
            if fall && tile_id::sets::PLATFORMS[tile_at(g, i, j).type_ as usize] {
                flag3 = true;
            }
            let num3 = tile_at(g, i, j).slope() as i32;
            vector4.x = (i * 16) as f32;
            vector4.y = (j * 16) as f32;
            if !(position.x + width as f32 > vector4.x) || !(position.x < vector4.x + 16f32) || !(position.y + height as f32 > vector4.y) || !(position.y < vector4.y + 16f32) {
                continue;
            }
            let mut num4 = 0f32;
            if num3 == 3 || num3 == 4 {
                if num3 == 3 {
                    num4 = position.x - vector4.x;
                }
                if num3 == 4 {
                    num4 = vector4.x + 16f32 - (position.x + width as f32);
                }
                if num4 >= 0f32 {
                    if position.y <= vector4.y + 16f32 - num4 {
                        let num5 = vector4.y + 16f32 - vector.y - num4;
                        if position.y + num5 > y2 {
                            vector2.y = position.y + num5;
                            y2 = vector2.y;
                            if vector3.y < 0.0101f32 {
                                vector3.y = 0.0101f32;
                            }
                            bits_byte.set(num3, true);
                        }
                    }
                } else if position.y > vector4.y {
                    let num6 = vector4.y + 16f32;
                    if vector2.y < num6 {
                        vector2.y = num6;
                        if vector3.y < 0.0101f32 {
                            vector3.y = 0.0101f32;
                        }
                    }
                }
            }
            if num3 != 1 && num3 != 2 {
                continue;
            }
            if num3 == 1 {
                num4 = position.x - vector4.x;
            }
            if num3 == 2 {
                num4 = vector4.x + 16f32 - (position.x + width as f32);
            }
            if num4 >= 0f32 {
                if !(position.y + height as f32 >= vector4.y + num4) {
                    continue;
                }
                let num7 = vector4.y - (vector.y + height as f32) + num4;
                if !(position.y + num7 < y) {
                    continue;
                }
                if flag3 {
                    g.collision.stair_fall = true;
                    continue;
                }
                if tile_id::sets::PLATFORMS[tile_at(g, i, j).type_ as usize] {
                    g.collision.stair = true;
                } else {
                    g.collision.stair = false;
                }
                vector2.y = position.y + num7;
                y = vector2.y;
                if vector3.y > 0f32 {
                    vector3.y = 0f32;
                }
                bits_byte.set(num3, true);
                continue;
            }
            if tile_id::sets::PLATFORMS[tile_at(g, i, j).type_ as usize] && !(position.y + height as f32 - 4f32 - velocity.x.abs() <= vector4.y) {
                if flag3 {
                    g.collision.stair_fall = true;
                }
                continue;
            }
            let num8 = vector4.y - height as f32;
            if !(vector2.y > num8) {
                continue;
            }
            if flag3 {
                g.collision.stair_fall = true;
                continue;
            }
            if tile_id::sets::PLATFORMS[tile_at(g, i, j).type_ as usize] {
                g.collision.stair = true;
            } else {
                g.collision.stair = false;
            }
            vector2.y = num8;
            if vector3.y > 0f32 {
                vector3.y = 0f32;
            }
        }
    }
    let position_ = position;
    let velocity_ = vector2 - position;
    let vector5 = tile_collision(g, position_, velocity_, width, height, false, false, 1, false, false, true);
    if vector5.y > velocity_.y {
        let num9 = velocity_.y - vector5.y;
        vector2.y = position.y + vector5.y;
        if bits_byte.get(1) {
            vector2.x = position.x - num9;
        }
        if bits_byte.get(2) {
            vector2.x = position.x + num9;
        }
        vector3.x = 0f32;
        vector3.y = 0f32;
        g.collision.up = false;
    } else if vector5.y < velocity_.y {
        let num10 = vector5.y - velocity_.y;
        vector2.y = position.y + vector5.y;
        if bits_byte.get(3) {
            vector2.x = position.x - num10;
        }
        if bits_byte.get(4) {
            vector2.x = position.x + num10;
        }
        vector3.x = 0f32;
        vector3.y = 0f32;
    }
    Vector4::from_vector2(vector2, vector3.x, vector3.y)
}

/// `Collision.noSlopeCollision(Position, Velocity, Width, Height, fallThrough = false, fall2 = false)` (Collision.cs:2048).
/// Sets `up` and `down` of `g.collision`.
pub fn no_slope_collision(g: &mut Game, position: Vector2, velocity: Vector2, width: i32, height: i32, fall_through: bool, fall2: bool) -> Vector2 {
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
            let t = tile_at(g, i, j);
            if !t.active() || (!g.main.tile_solid[t.type_ as usize] && (!g.main.tile_solid_top[t.type_ as usize] || t.frame_y != 0)) {
                continue;
            }
            vector4.x = (i * 16) as f32;
            vector4.y = (j * 16) as f32;
            let mut num7 = 16;
            if t.half_brick() {
                vector4.y += 8f32;
                num7 -= 8;
            }
            if !(vector2.x + width as f32 > vector4.x) || !(vector2.x < vector4.x + 16f32) || !(vector2.y + height as f32 > vector4.y) || !(vector2.y < vector4.y + num7 as f32) {
                continue;
            }
            if vector3.y + height as f32 <= vector4.y {
                g.collision.down = true;
                if (!(g.main.tile_solid_top[t.type_ as usize] && fall_through) || !(velocity.y <= 1f32 || fall2)) && num6 > vector4.y {
                    num3 = i;
                    num4 = j;
                    if num7 < 16 {
                        num4 += 1;
                    }
                    if num3 != num {
                        result.y = vector4.y - (vector3.y + height as f32);
                        num6 = vector4.y;
                    }
                }
            } else if vector3.x + width as f32 <= vector4.x && !g.main.tile_solid_top[t.type_ as usize] {
                num = i;
                num2 = j;
                if num2 != num4 {
                    result.x = vector4.x - (vector3.x + width as f32);
                }
                if num3 == num {
                    result.y = vector.y;
                }
            } else if vector3.x >= vector4.x + 16f32 && !g.main.tile_solid_top[t.type_ as usize] {
                num = i;
                num2 = j;
                if num2 != num4 {
                    result.x = vector4.x + 16f32 - vector3.x;
                }
                if num3 == num {
                    result.y = vector.y;
                }
            } else if vector3.y >= vector4.y + num7 as f32 && !g.main.tile_solid_top[t.type_ as usize] {
                g.collision.up = true;
                num3 = i;
                num4 = j;
                result.y = vector4.y + num7 as f32 - vector3.y + 0.01f32;
                if num4 == num2 {
                    result.x = vector.x;
                }
            }
        }
    }
    result
}

/// `Collision.BuildTileContacts(Position, Width, Height, List<TileContact> contactTiles)` (Collision.cs:2150)
pub fn build_tile_contacts(g: &Game, mut position: Vector2, width: i32, height: i32, contact_tiles: &mut Vec<TileContact>) {
    position.x = (position.x as i32) as f32;
    position.y = (position.y as i32) as f32;
    contact_tiles.clear();
    let value = ((position.x - 1f32) / 16f32) as i32 - 1;
    let mut value2 = ((position.x + 1f32 + width as f32) / 16f32) as i32 + 1;
    let mut value3 = ((position.y - 1f32) / 16f32) as i32 - 1;
    let mut value4 = ((position.y + 3f32 + height as f32) / 16f32) as i32 + 1;
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
            let type_ = tile.type_;
            let mut flag = g.main.tile_solid[type_ as usize];
            if g.main.tile_solid_top[type_ as usize] && tile.frame_y == 0 {
                flag = true;
            }
            if !flag {
                continue;
            }
            vector.x = (i * 16) as f32;
            vector.y = (j * 16) as f32;
            let mut num2 = 16;
            if tile.half_brick() {
                vector.y += 8f32;
                num2 -= 8;
            }
            let b = tile.slope();
            if (position.x - (vector.x + 16f32)).abs() < 0.1f32 && position.y + height as f32 > vector.y && position.y < vector.y + num2 as f32 && b != 3 && b != 1 {
                let num3 = math_max_f32(math_min_f32(position.y + height as f32, vector.y + num2 as f32) - math_max_f32(position.y, vector.y) + 0.5f32, 1f32);
                contact_tiles.push(TileContact::new(TileContactSide::Left, i, j, type_ as i32, b as i32, num3 as i32));
            }
            if (position.x + width as f32 - vector.x).abs() < 0.1f32 && position.y + height as f32 > vector.y && position.y < vector.y + num2 as f32 && b != 4 && b != 2 {
                let num4 = math_max_f32(math_min_f32(position.y + height as f32, vector.y + num2 as f32) - math_max_f32(position.y, vector.y) + 0.5f32, 1f32);
                contact_tiles.push(TileContact::new(TileContactSide::Right, i, j, type_ as i32, b as i32, num4 as i32));
            }
            if !(position.y + 3f32 + height as f32 > vector.y) || !(position.y - 1f32 < vector.y + num2 as f32) {
                continue;
            }
            if position.x + width as f32 > vector.x && position.x < vector.x + 16f32 {
                let num5 = math_max_f32(math_min_f32(position.x + width as f32, vector.x + 16f32) - math_max_f32(position.x, vector.x) + 0.5f32, 1f32);
                match b {
                    0 => {
                        if (position.y - (vector.y + num2 as f32)).abs() < 0.1f32 {
                            contact_tiles.push(TileContact::new(TileContactSide::Top, i, j, type_ as i32, b as i32, num5 as i32));
                        }
                        if (position.y + height as f32 - vector.y).abs() < 0.1f32 {
                            contact_tiles.push(TileContact::new(TileContactSide::Bottom, i, j, type_ as i32, b as i32, num5 as i32));
                        }
                    }
                    1 => {
                        if (position.y - (vector.y + num2 as f32)).abs() < 0.1f32 {
                            contact_tiles.push(TileContact::new(TileContactSide::Top, i, j, type_ as i32, b as i32, num5 as i32));
                        }
                        let num9 = math_max_f32(position.x - vector.x, 0f32);
                        let num10 = position.y + height as f32;
                        if num10 - vector.y > -0.1f32 && num10 - (vector.y + num9) < 0.1f32 {
                            contact_tiles.push(TileContact::new(TileContactSide::Bottom, i, j, type_ as i32, b as i32, num5 as i32));
                        }
                    }
                    2 => {
                        if (position.y - (vector.y + num2 as f32)).abs() < 0.1f32 {
                            contact_tiles.push(TileContact::new(TileContactSide::Top, i, j, type_ as i32, b as i32, num5 as i32));
                        }
                        let num7 = math_max_f32(vector.x + 16f32 - (position.x + width as f32), 0f32);
                        let num8 = position.y + height as f32;
                        if num8 - vector.y > -0.1f32 && num8 - (vector.y + num7) < 0.1f32 {
                            contact_tiles.push(TileContact::new(TileContactSide::Bottom, i, j, type_ as i32, b as i32, num5 as i32));
                        }
                    }
                    3 => {
                        let num11 = math_max_f32(position.x - vector.x, 0f32);
                        if (position.y - (vector.y + num2 as f32 - num11)).abs() < 0.1f32 {
                            contact_tiles.push(TileContact::new(TileContactSide::Top, i, j, type_ as i32, b as i32, num5 as i32));
                        }
                        if (position.y + height as f32 - vector.y).abs() < 0.1f32 {
                            contact_tiles.push(TileContact::new(TileContactSide::Bottom, i, j, type_ as i32, b as i32, num5 as i32));
                        }
                    }
                    4 => {
                        let num6 = math_max_f32(vector.x + 16f32 - (position.x + width as f32), 0f32);
                        if (position.y - (vector.y + num2 as f32 - num6)).abs() < 0.1f32 {
                            contact_tiles.push(TileContact::new(TileContactSide::Top, i, j, type_ as i32, b as i32, num5 as i32));
                        }
                        if (position.y + height as f32 - vector.y).abs() < 0.1f32 {
                            contact_tiles.push(TileContact::new(TileContactSide::Bottom, i, j, type_ as i32, b as i32, num5 as i32));
                        }
                    }
                    _ => {}
                }
            } else {
                if !(position.x + 3f32 + width as f32 > vector.x) || !(position.x - 3f32 < vector.x + 16f32) || !(position.y < vector.y) {
                    continue;
                }
                let tile_contact_side = if vector.x < position.x { TileContactSide::BottomLeft } else { TileContactSide::BottomRight };
                match b {
                    0 | 3 | 4 => {
                        contact_tiles.push(TileContact::new(tile_contact_side, i, j, type_ as i32, b as i32, 0));
                    }
                    1 => {
                        if tile_contact_side == TileContactSide::BottomRight {
                            contact_tiles.push(TileContact::new(tile_contact_side, i, j, type_ as i32, b as i32, 0));
                        }
                    }
                    2 => {
                        if tile_contact_side == TileContactSide::BottomLeft {
                            contact_tiles.push(TileContact::new(tile_contact_side, i, j, type_ as i32, b as i32, 0));
                        }
                    }
                    _ => {}
                }
            }
        }
    }
}
