//! `Terraria.Collision`, stepping and tile lists: `StepDown`, `StepUp`, `InTileBounds`, `GetTileRotation`,
//! `GetEntityEdgeTiles`, `GetTilesIn`, `ExpandVertically`, `AdvancedTileCollision`, `LaserScan`, `AimingLaserScan`
//! (Collision.cs:3587-3946, 4395-4635).

use terraria_core::xna::{Point, Rectangle, Vector2};
use terraria_data::sets::tile_id;

use super::line_of_sight::hit_line;
use super::utils::{clamp_i32, float_intersect, has_nans, rotated_by, solid_tile3, tile_at, tile_slot, to_tile_coordinates};
use super::EntityBox;
use crate::game::Game;
use crate::world_gen::tile_frame::in_world;
use terraria_core::Tile;

/// `Collision.StepDown(ref position, ref velocity, width, height, ref stepSpeed, ref gfxOffY, gravDir = 1, waterWalk = false)` (Collision.cs:3587)
pub fn step_down(g: &Game, position: &mut Vector2, velocity: &mut Vector2, width: i32, height: i32, step_speed: &mut f32, gfx_off_y: &mut f32, _grav_dir: i32, water_walk: bool) {
    let mut vector = *position;
    vector.x += velocity.x;
    vector.y = (((vector.y + height as f32) / 16f32) as f64).floor() as f32 * 16f32 - height as f32;
    let flag = false;
    let num = (vector.x / 16f32) as i32;
    let num2 = ((vector.x + width as f32) / 16f32) as i32;
    let num3 = ((vector.y + height as f32 + 4f32) / 16f32) as i32;
    let num4 = height / 16 + (if height % 16 != 0 { 1 } else { 0 });
    let mut num5 = ((num3 + num4) * 16) as f32;
    let num6 = g.main.bottom_world / 16f32 - 42f32;
    for i in num..=num2 {
        for j in num3..=num3 + 1 {
            if !in_world(g, i, j, 1) {
                continue;
            }
            // `if (Main.tile[i, j] == null) ...` and the same for `[i, j - 1]`: the index is inside the world, so the tile exists
            if water_walk && tile_at(g, i, j).liquid > 0 && tile_at(g, i, j - 1).liquid == 0 {
                let num7 = tile_at(g, i, j).liquid as i32 / 32 * 2 + 2;
                let num8 = j * 16 + 16 - num7;
                if Rectangle::new(i * 16, j * 16 - 17, 16, 16).intersects(Rectangle::new(position.x as i32, position.y as i32, width, height)) && (num8 as f32) < num5 {
                    num5 = num8 as f32;
                }
            }
            if j as f32 >= num6 || (tile_at(g, i, j).nactive() && (g.main.tile_solid[tile_at(g, i, j).type_ as usize] || g.main.tile_solid_top[tile_at(g, i, j).type_ as usize])) {
                let mut num9 = j * 16;
                if tile_at(g, i, j).half_brick() {
                    num9 += 8;
                }
                if float_intersect((i * 16) as f32, (j * 16 - 17) as f32, 16f32, 16f32, position.x, position.y, width as f32, height as f32) && (num9 as f32) < num5 {
                    num5 = num9 as f32;
                }
            }
        }
    }
    let num10 = num5 - (position.y + height as f32);
    if num10 > 7f32 && num10 < 17f32 && !flag {
        *step_speed = 1.5f32;
        if num10 > 9f32 {
            *step_speed = 2.5f32;
        }
        *gfx_off_y += position.y + height as f32 - num5;
        position.y = num5 - height as f32;
    }
}

/// `Collision.StepUp(ref position, ref velocity, width, height, ref stepSpeed, ref gfxOffY, gravDir = 1, holdsMatching = false,
/// specialChecksMode = 0)` (Collision.cs:3651)
pub fn step_up(
    g: &Game,
    position: &mut Vector2,
    velocity: &mut Vector2,
    width: i32,
    height: i32,
    step_speed: &mut f32,
    gfx_off_y: &mut f32,
    grav_dir: i32,
    holds_matching: bool,
    special_checks_mode: i32,
) {
    let mut num = 0;
    if velocity.x < 0f32 {
        num = -1;
    }
    if velocity.x > 0f32 {
        num = 1;
    }
    let mut vector = *position;
    vector.x += velocity.x;
    let num2 = ((vector.x + (width / 2) as f32 + ((width / 2 + 1) * num) as f32) / 16f32) as i32;
    let mut num3 = (((vector.y as f64) + 0.1) / 16.0) as i32;
    if grav_dir == 1 {
        num3 = ((vector.y + height as f32 - 1f32) / 16f32) as i32;
    }
    let num4 = height / 16 + (if height % 16 != 0 { 1 } else { 0 });
    if !in_world(g, num2, num3, 1) || num3 >= g.main.max_tiles_y - 40 {
        return;
    }
    let mut flag = true;
    let mut flag2 = true;
    // `if (Main.tile[num2, num3] == null) return;`: the index is inside the world, so the tile exists
    for i in 1..num4 + 2 {
        if !in_world(g, num2, num3 - i * grav_dir, 0) {
            return;
        }
    }
    if !in_world(g, num2 - num, num3 - num4 * grav_dir, 0) {
        return;
    }
    let mut tile: Tile;
    for j in 2..num4 + 1 {
        if !in_world(g, num2, num3 - j * grav_dir, 0) {
            return;
        }
        tile = tile_at(g, num2, num3 - j * grav_dir);
        flag = flag && (!tile.nactive() || !g.main.tile_solid[tile.type_ as usize] || g.main.tile_solid_top[tile.type_ as usize]);
    }
    tile = tile_at(g, num2 - num, num3 - num4 * grav_dir);
    flag2 = flag2 && (!tile.nactive() || !g.main.tile_solid[tile.type_ as usize] || g.main.tile_solid_top[tile.type_ as usize]);
    let mut flag3 = true;
    let mut flag4 = true;
    let mut flag5 = true;
    let mut tile2: Tile;
    if grav_dir == 1 {
        tile = tile_at(g, num2, num3 - grav_dir);
        tile2 = tile_at(g, num2, num3 - (num4 + 1) * grav_dir);
        flag3 = flag3
            && (!tile.nactive()
                || !g.main.tile_solid[tile.type_ as usize]
                || g.main.tile_solid_top[tile.type_ as usize]
                || (tile.slope() == 1 && position.x + (width / 2) as f32 > (num2 * 16) as f32)
                || (tile.slope() == 2 && position.x + ((width / 2) as f32) < (num2 * 16 + 16) as f32)
                || (tile.half_brick() && (!tile2.nactive() || !g.main.tile_solid[tile2.type_ as usize] || g.main.tile_solid_top[tile2.type_ as usize])));
        tile = tile_at(g, num2, num3);
        tile2 = tile_at(g, num2, num3 - 1);
        if special_checks_mode == 1 {
            flag5 = !tile_id::sets::IGNORED_BY_NPC_STEP_UP[tile.type_ as usize];
        }
        flag4 = flag4
            && ((tile.nactive()
                && (!tile.top_slope()
                    || (tile.slope() == 1 && position.x + ((width / 2) as f32) < (num2 * 16) as f32)
                    || (tile.slope() == 2 && position.x + (width / 2) as f32 > (num2 * 16 + 16) as f32))
                && (!tile.top_slope() || position.y + height as f32 > (num3 * 16) as f32)
                && ((g.main.tile_solid[tile.type_ as usize] && !g.main.tile_solid_top[tile.type_ as usize])
                    || (holds_matching
                        && ((g.main.tile_solid_top[tile.type_ as usize] && tile.frame_y == 0) || tile_id::sets::PLATFORMS[tile.type_ as usize] || tile.type_ == 380)
                        && (!g.main.tile_solid[tile2.type_ as usize] || !tile2.nactive())
                        && flag5)))
                || (tile2.half_brick() && tile2.nactive()));
        flag4 &= !g.main.tile_solid_top[tile.type_ as usize] || !g.main.tile_solid_top[tile2.type_ as usize];
    } else {
        tile = tile_at(g, num2, num3 - grav_dir);
        tile2 = tile_at(g, num2, num3 - (num4 + 1) * grav_dir);
        flag3 = flag3
            && (!tile.nactive()
                || !g.main.tile_solid[tile.type_ as usize]
                || g.main.tile_solid_top[tile.type_ as usize]
                || tile.slope() != 0
                || (tile.half_brick() && (!tile2.nactive() || !g.main.tile_solid[tile2.type_ as usize] || g.main.tile_solid_top[tile2.type_ as usize])));
        tile = tile_at(g, num2, num3);
        tile2 = tile_at(g, num2, num3 + 1);
        flag4 = flag4
            && ((tile.nactive()
                && ((g.main.tile_solid[tile.type_ as usize] && !g.main.tile_solid_top[tile.type_ as usize])
                    || (holds_matching && g.main.tile_solid_top[tile.type_ as usize] && tile.frame_y == 0 && (!g.main.tile_solid[tile2.type_ as usize] || !tile2.nactive()))))
                || (tile2.half_brick() && tile2.nactive()));
    }
    if !(((num2 * 16) as f32) < vector.x + width as f32) || !((num2 * 16 + 16) as f32 > vector.x) {
        return;
    }
    if grav_dir == 1 {
        if !(flag4 && flag3 && flag && flag2) {
            return;
        }
        let mut num5 = (num3 * 16) as f32;
        if tile_at(g, num2, num3 - 1).half_brick() {
            num5 -= 8f32;
        } else if tile_at(g, num2, num3).half_brick() {
            num5 += 8f32;
        }
        if !(num5 < vector.y + height as f32) {
            return;
        }
        let num6 = vector.y + height as f32 - num5;
        if (num6 as f64) <= 16.1 {
            *gfx_off_y += position.y + height as f32 - num5;
            position.y = num5 - height as f32;
            if num6 < 9f32 {
                *step_speed = 1f32;
            } else {
                *step_speed = 2f32;
            }
        }
    } else {
        if !(flag4 && flag3 && flag && flag2) || tile_at(g, num2, num3).bottom_slope() || tile_id::sets::PLATFORMS[tile2.type_ as usize] {
            return;
        }
        let num7 = (num3 * 16 + 16) as f32;
        if !(num7 > vector.y) {
            return;
        }
        let num8 = num7 - vector.y;
        if (num8 as f64) <= 16.1 {
            *gfx_off_y -= num7 - position.y;
            position.y = num7;
            velocity.y = 0f32;
            if num8 < 9f32 {
                *step_speed = 1f32;
            } else {
                *step_speed = 2f32;
            }
        }
    }
}

/// `Collision.InTileBounds(x, y, lx, ly, hx, hy)` (Collision.cs:3802)
pub fn in_tile_bounds(x: i32, y: i32, lx: i32, ly: i32, hx: i32, hy: i32) -> bool {
    if x < lx || x > hx || y < ly || y > hy {
        return false;
    }
    true
}

/// `Collision.GetTileRotation(Vector2 position)` (Collision.cs:3811)
pub fn get_tile_rotation(g: &Game, position: Vector2) -> f32 {
    let pi_4 = std::f64::consts::PI as f32 / 4f32; // `(float)Math.PI / 4f`
    let num = position.y % 16f32;
    let num2 = (position.x / 16f32) as i32;
    let mut num3 = (position.y / 16f32) as i32;
    let Some(mut tile) = tile_slot(g, num2, num3) else { return 0f32 }; // `if (tile == null) return 0f;`
    let mut flag = false;
    let mut num4 = 2;
    while num4 >= 0 {
        if tile.active() {
            if g.main.tile_solid[tile.type_ as usize] {
                let num5 = tile.block_type();
                if tile.type_ != 19 {
                    match num5 {
                        1 => return 0f32,
                        2 => return pi_4,
                        3 => return -pi_4,
                        _ => return 0f32,
                    }
                }
                let num6 = tile.frame_x as i32 / 18;
                if ((num6 >= 0 && num6 <= 7) || (num6 >= 12 && num6 <= 16)) && (num == 0f32 || flag) {
                    return 0f32;
                }
                match num6 {
                    8 | 19 | 21 | 23 => return -pi_4,
                    10 | 20 | 22 | 24 => return pi_4,
                    25 | 26 => {
                        if !flag {
                            match num5 {
                                2 => return pi_4,
                                3 => return -pi_4,
                                _ => {}
                            }
                        } else {
                            return 0f32;
                        }
                    }
                    _ => {}
                }
            } else if g.main.tile_solid_top[tile.type_ as usize] && tile.frame_y == 0 && flag {
                return 0f32;
            }
        }
        num3 += 1;
        if num3 >= g.main.max_tiles_y {
            return 0f32;
        }
        let Some(t) = tile_slot(g, num2, num3) else { return 0f32 };
        tile = t;
        flag = true;
        num4 -= 1;
    }
    0f32
}

/// `Collision.GetEntityEdgeTiles(List<Point> p, Entity entity, left = true, right = true, up = true, down = true)`
/// (Collision.cs:3896). `Entity` is mirrored by [`EntityBox`]; `entity.Right.X` is `position.X + width` and
/// `entity.Bottom.Y` is `position.Y + height` (Entity.cs:74, 122).
pub fn get_entity_edge_tiles(p: &mut Vec<Point>, entity: &EntityBox, left: bool, right: bool, up: bool, down: bool) {
    let mut num = entity.position.x as i32;
    let mut num2 = entity.position.y as i32;
    let mut num3 = (entity.position.x + entity.width as f32) as i32;
    let mut num4 = (entity.position.y + entity.height as f32) as i32;
    if num % 16 == 0 {
        num -= 1;
    }
    if num2 % 16 == 0 {
        num2 -= 1;
    }
    if num3 % 16 == 0 {
        num3 += 1;
    }
    if num4 % 16 == 0 {
        num4 += 1;
    }
    let num5 = num3 / 16 - num / 16;
    let num6 = num4 / 16 - num2 / 16;
    num /= 16;
    num2 /= 16;
    for i in num..=num + num5 {
        if up {
            p.push(Point::new(i, num2));
        }
        if down {
            p.push(Point::new(i, num2 + num6));
        }
    }
    for j in num2..num2 + num6 {
        if left {
            p.push(Point::new(num, j));
        }
        if right {
            p.push(Point::new(num + num5, j));
        }
    }
}

/// `Collision.GetTilesIn(Vector2 TopLeft, Vector2 BottomRight)` (Collision.cs:4395)
pub fn get_tiles_in(g: &Game, top_left: Vector2, bottom_right: Vector2) -> Vec<Point> {
    let mut list: Vec<Point> = Vec::new();
    let point = to_tile_coordinates(top_left);
    let point2 = to_tile_coordinates(bottom_right);
    let num = clamp_i32(point.x, 0, g.main.max_tiles_x - 1);
    let num2 = clamp_i32(point.y, 0, g.main.max_tiles_y - 40);
    let num3 = clamp_i32(point2.x, 0, g.main.max_tiles_x - 1);
    let num4 = clamp_i32(point2.y, 0, g.main.max_tiles_y - 40);
    for i in num..=num3 {
        for j in num2..=num4 {
            if tile_slot(g, i, j).is_some() {
                list.push(Point::new(i, j));
            }
        }
    }
    list
}

/// `Collision.ExpandVertically(startX, startY, out topY, out bottomY, maxExpandUp = 100, maxExpandDown = 100)`
/// (Collision.cs:4417). Returns `(topY, bottomY)`.
pub fn expand_vertically(g: &Game, start_x: i32, start_y: i32, max_expand_up: i32, max_expand_down: i32) -> (i32, i32) {
    let mut top_y = start_y;
    let mut bottom_y = start_y;
    if !in_world(g, start_x, start_y, 10) {
        return (top_y, bottom_y);
    }
    for _i in 0..max_expand_up {
        if top_y <= 0 {
            break;
        }
        if top_y < 10 {
            break;
        }
        if tile_slot(g, start_x, top_y).is_none() {
            break;
        }
        if solid_tile3(g, start_x, top_y) {
            break;
        }
        top_y -= 1;
    }
    for _j in 0..max_expand_down {
        if bottom_y >= g.main.max_tiles_y - 10 {
            break;
        }
        if bottom_y > g.main.max_tiles_y - 10 {
            break;
        }
        if tile_slot(g, start_x, bottom_y).is_none() {
            break;
        }
        if solid_tile3(g, start_x, bottom_y) {
            break;
        }
        bottom_y += 1;
    }
    (top_y, bottom_y)
}

/// `Collision.AdvancedTileCollision(bool[] forcedIgnoredTiles, Position, Velocity, Width, Height, fallThrough = false,
/// fall2 = false, gravDir = 1)` (Collision.cs:4467). Sets `up` and `down` of `g.collision`.
pub fn advanced_tile_collision(g: &mut Game, forced_ignored_tiles: &[bool], position: Vector2, velocity: Vector2, width: i32, height: i32, fall_through: bool, fall2: bool, grav_dir: i32) -> Vector2 {
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
            if !tile.active()
                || tile.in_active()
                || forced_ignored_tiles[tile.type_ as usize]
                || (!g.main.tile_solid[tile.type_ as usize] && (!g.main.tile_solid_top[tile.type_ as usize] || tile.frame_y != 0))
            {
                continue;
            }
            vector4.x = (i * 16) as f32;
            vector4.y = (j * 16) as f32;
            let mut num7 = 16;
            if tile.half_brick() {
                vector4.y += 8f32;
                num7 -= 8;
            }
            if !(vector2.x + width as f32 > vector4.x) || !(vector2.x < vector4.x + 16f32) || !(vector2.y + height as f32 > vector4.y) || !(vector2.y < vector4.y + num7 as f32) {
                continue;
            }
            let mut flag = false;
            let mut flag2 = false;
            if tile.slope() > 2 {
                if tile.slope() == 3 && vector3.y + velocity.x.abs() >= vector4.y && vector3.x >= vector4.x {
                    flag2 = true;
                }
                if tile.slope() == 4 && vector3.y + velocity.x.abs() >= vector4.y && vector3.x + width as f32 <= vector4.x + 16f32 {
                    flag2 = true;
                }
            } else if tile.slope() > 0 {
                flag = true;
                if tile.slope() == 1 && vector3.y + height as f32 - velocity.x.abs() <= vector4.y + num7 as f32 && vector3.x >= vector4.x {
                    flag2 = true;
                }
                if tile.slope() == 2 && vector3.y + height as f32 - velocity.x.abs() <= vector4.y + num7 as f32 && vector3.x + width as f32 <= vector4.x + 16f32 {
                    flag2 = true;
                }
            }
            if flag2 {
                continue;
            }
            if vector3.y + height as f32 <= vector4.y {
                g.collision.down = true;
                if (!(g.main.tile_solid_top[tile.type_ as usize] && fall_through) || !(velocity.y <= 1f32 || fall2)) && num6 > vector4.y {
                    num3 = i;
                    num4 = j;
                    if num7 < 16 {
                        num4 += 1;
                    }
                    if num3 != num && !flag {
                        result.y = vector4.y - (vector3.y + height as f32) + (if grav_dir == -1 { -0.01f32 } else { 0f32 });
                        num6 = vector4.y;
                    }
                }
            } else if vector3.x + width as f32 <= vector4.x && !g.main.tile_solid_top[tile.type_ as usize] {
                // `if (Main.tile[i - 1, j] == null) ...`: the index is inside the world, so the tile exists; `i - 1` below the array throws as in the original
                if tile_at(g, i - 1, j).slope() != 2 && tile_at(g, i - 1, j).slope() != 4 {
                    num = i;
                    num2 = j;
                    if num2 != num4 {
                        result.x = vector4.x - (vector3.x + width as f32);
                    }
                    if num3 == num {
                        result.y = vector.y;
                    }
                }
            } else if vector3.x >= vector4.x + 16f32 && !g.main.tile_solid_top[tile.type_ as usize] {
                // `if (Main.tile[i + 1, j] == null) ...`: the index is inside the world, so the tile exists
                if tile_at(g, i + 1, j).slope() != 1 && tile_at(g, i + 1, j).slope() != 3 {
                    num = i;
                    num2 = j;
                    if num2 != num4 {
                        result.x = vector4.x + 16f32 - vector3.x;
                    }
                    if num3 == num {
                        result.y = vector.y;
                    }
                }
            } else if vector3.y >= vector4.y + num7 as f32 && !g.main.tile_solid_top[tile.type_ as usize] {
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

/// `Collision.LaserScan(samplingPoint, directionUnit, samplingWidth, maxDistance, float[] samples)` (Collision.cs:4613)
pub fn laser_scan(g: &Game, sampling_point: Vector2, direction_unit: Vector2, sampling_width: f32, max_distance: f32, samples: &mut [f32]) {
    for i in 0..samples.len() {
        let num = i as f32 / (samples.len() as i32 - 1) as f32;
        let vector = sampling_point + rotated_by(direction_unit, 1.5707963705062866) * (num - 0.5f32) * sampling_width;
        let num2 = vector.x as i32 / 16;
        let num3 = vector.y as i32 / 16;
        let vector2 = vector + direction_unit * max_distance;
        let num4 = vector2.x as i32 / 16;
        let num5 = vector2.y as i32 / 16;
        let _num6 = 0f32;
        let (hit, col) = hit_line(g, num2, num3, num4, num5, 0, 0, &Vec::new());
        let num6 = if hit {
            if col.x != num4 || col.y != num5 {
                Vector2::new((num2 - col.x).abs() as f32, (num3 - col.y).abs() as f32).length() * 16f32
            } else {
                max_distance
            }
        } else {
            Vector2::new((num2 - col.x).abs() as f32, (num3 - col.y).abs() as f32).length() * 16f32
        };
        samples[i] = num6;
    }
}

/// `Collision.AimingLaserScan(startPoint, endPoint, samplingWidth, samplesToTake, out Vector2 vectorTowardsTarget,
/// out float[] samples)` (Collision.cs:4630). Returns `(vectorTowardsTarget, samples)`.
pub fn aiming_laser_scan(g: &Game, start_point: Vector2, end_point: Vector2, sampling_width: f32, samples_to_take: i32) -> (Vector2, Vec<f32>) {
    let mut samples = vec![0f32; samples_to_take as usize];
    let vector_towards_target = end_point - start_point;
    // `vectorTowardsTarget.SafeNormalize(Vector2.Zero)` (Utils.cs:1856)
    let direction_unit = if vector_towards_target == Vector2::ZERO || has_nans(vector_towards_target) { Vector2::ZERO } else { Vector2::normalized(vector_towards_target) };
    laser_scan(g, start_point, direction_unit, sampling_width, vector_towards_target.length(), &mut samples);
    (vector_towards_target, samples)
}
