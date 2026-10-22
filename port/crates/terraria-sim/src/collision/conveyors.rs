//! `Terraria.Collision`, conveyor belts: `ApplyConveyorBeltMovementToVelocity`, `StepConveyorBelt`,
//! `TryFindingConveyorBeltRising` (Collision.cs:3948-4393).
//!
//! `WorldItem` and `Entity` are mirrored by [`EntityBox`] (`position`, `width`, `height`: the only fields read);
//! the `Player` fields `StepConveyorBelt` reads are in [`PlayerConveyorFields`].

use terraria_core::xna::{Point, Vector2};
use terraria_data::sets::tile_id;

use super::geometry::check_aab_bv_line_collision2;
use super::slopes::build_tile_contacts;
use super::stepping::get_entity_edge_tiles;
use super::tile_collision::tile_collision;
use super::utils::{math_max_f32, math_min_f32, math_sign, tile_at, to_tile_coordinates};
use super::{EntityBox, PlayerConveyorFields, TileContactSide};
use crate::game::Game;
use crate::world_gen::tile_frame::in_world;

/// `Collision.ApplyConveyorBeltMovementToVelocity(WorldItem item, ref Vector2 velocity)` (Collision.cs:3948).
/// `item` may be moved (its `position`); the `contacts` list is `g.collision.contacts`.
pub fn apply_conveyor_belt_movement_to_velocity(g: &mut Game, item: &mut EntityBox, velocity: &mut Vector2) -> bool {
    let mut flag = false;
    // `contacts` is a static list of the original; taken out of `g` while `BuildTileContacts` reads the world
    let mut contacts = std::mem::take(&mut g.collision.contacts);
    build_tile_contacts(g, item.position, item.width, item.height, &mut contacts);
    if contacts.len() > 0 {
        let mut num = -1;
        let mut num2 = -1;
        let mut flag2 = false;
        let mut tile_contact_side = TileContactSide::Top;
        let mut tile_contact_side2 = TileContactSide::Top;
        let mut zero = Vector2::ZERO;
        let mut flag3 = false;
        let mut flag4 = false;
        let mut flag5 = false;
        for i in 0..contacts.len() {
            let num3 = tile_id::sets::CONVEYOR_DIRECTION[contacts[i].type_ as usize];
            match contacts[i].side {
                TileContactSide::Left => {
                    zero.y += (num3 * contacts[i].overlap) as f32;
                    flag3 = flag3 || num3 != 0;
                }
                TileContactSide::Right => {
                    zero.y += (-num3 * contacts[i].overlap) as f32;
                    flag3 = flag3 || num3 != 0;
                }
                TileContactSide::Top => {
                    zero.x += (-num3 * contacts[i].overlap) as f32;
                    flag4 = num3 != 0;
                }
                TileContactSide::Bottom => {
                    flag5 = true;
                    zero.x += (num3 * contacts[i].overlap) as f32;
                    flag4 = num3 != 0;
                    if contacts[i].slope == 1 {
                        if ((contacts[i].x * 16) as f32) < item.position.x {
                            let tile3 = tile_at(g, contacts[i].x, contacts[i].y - 1);
                            if tile3.active() && tile3.slope() == 3 {
                                num2 = i as i32;
                            }
                        }
                    } else if contacts[i].slope == 2 && ((contacts[i].x * 16 + 16) as f32) > item.right().x {
                        let tile4 = tile_at(g, contacts[i].x, contacts[i].y - 1);
                        if tile4.active() && tile4.slope() == 4 {
                            num = i as i32;
                        }
                    }
                }
                TileContactSide::BottomLeft => {
                    if num3 == -1 {
                        let x2 = contacts[i].x;
                        let y2 = contacts[i].y;
                        let tile2 = tile_at(g, x2, y2 - 1);
                        let b2 = tile2.slope();
                        if !tile2.active() || (b2 == 1 && tile_id::sets::CONVEYOR_DIRECTION[tile2.type_ as usize] == -1) {
                            tile_contact_side = TileContactSide::BottomLeft;
                            flag2 = tile_at(g, x2, y2).half_brick();
                        }
                    }
                    if num3 == 1 {
                        tile_contact_side2 = TileContactSide::BottomLeft;
                    }
                }
                TileContactSide::BottomRight => {
                    if num3 == 1 {
                        let x = contacts[i].x;
                        let y = contacts[i].y;
                        let tile = tile_at(g, x, y - 1);
                        let b = tile.slope();
                        if !tile.active() || (b == 2 && tile_id::sets::CONVEYOR_DIRECTION[tile.type_ as usize] == 1) {
                            tile_contact_side = TileContactSide::BottomRight;
                            flag2 = tile_at(g, x, y).half_brick();
                        }
                    }
                    if num3 == -1 {
                        tile_contact_side2 = TileContactSide::BottomRight;
                    }
                }
            }
        }
        if zero.x < 0f32 {
            if zero.x < -8f32 {
                zero.x = -2.5f32;
            } else if zero.x < -4f32 {
                zero.x = -1.25f32;
            } else {
                zero.x = -0.75f32;
            }
        } else if zero.x > 0f32 {
            if zero.x > 8f32 {
                zero.x = 2.5f32;
            } else if zero.x > 4f32 {
                zero.x = 1.25f32;
            } else {
                zero.x = 0.75f32;
            }
        }
        if zero.y < 0f32 {
            if zero.y < 8f32 {
                zero.y = -2.5f32;
            } else {
                zero.y = -1.25f32;
            }
        } else if zero.y > 0f32 {
            if zero.y > 8f32 {
                zero.y = 2.5f32;
            } else {
                zero.y = 1.25f32;
            }
        } else if flag3 && velocity.y <= 1f32 {
            velocity.y = 0f32;
        }
        if zero.y == 0f32 {
            if velocity.y < 0.11f32 && tile_contact_side != TileContactSide::Top && !flag5 {
                if tile_contact_side == TileContactSide::BottomLeft {
                    flag = true;
                    let mut bottom = item.bottom();
                    let mut num4 = bottom.y as i32;
                    num4 = (num4 + 8) / 16;
                    num4 *= 16;
                    if flag2 {
                        num4 += 8;
                    }
                    bottom.y = num4 as f32;
                    item.set_bottom(bottom);
                    velocity.y = 0f32;
                    velocity.x = -1.25f32;
                    zero.x = -2.5f32;
                }
                if tile_contact_side == TileContactSide::BottomRight {
                    flag = true;
                    let mut bottom2 = item.bottom();
                    let mut num5 = bottom2.y as i32;
                    num5 = (num5 + 8) / 16;
                    num5 *= 16;
                    if flag2 {
                        num5 += 8;
                    }
                    bottom2.y = num5 as f32;
                    item.set_bottom(bottom2);
                    velocity.y = 0f32;
                    velocity.x = 1.25f32;
                    zero.x = 2.5f32;
                }
            } else if (velocity.y as f64) < 0.3 && !flag5 {
                match tile_contact_side2 {
                    TileContactSide::BottomRight => {
                        if velocity.x <= 0.75f32 && velocity.x >= -2.5f32 && !flag3 {
                            flag = true;
                            let mut num7 = item.position.x as i32;
                            num7 = (num7 + 8) / 16;
                            num7 *= 16;
                            item.position.x = num7 as f32;
                            velocity.x = 0f32;
                            velocity.y = 0.75f32;
                            zero.y = 2.5f32;
                        }
                    }
                    TileContactSide::BottomLeft => {
                        if velocity.x >= -0.75f32 && velocity.x <= 2.5f32 && !flag3 {
                            flag = true;
                            let mut right = item.right();
                            let mut num6 = right.x as i32;
                            num6 = (num6 + 8) / 16;
                            num6 *= 16;
                            right.x = num6 as f32;
                            item.set_right(right);
                            velocity.x = 0f32;
                            velocity.y = 0.75f32;
                            zero.y = 2.5f32;
                        }
                    }
                    _ => {}
                }
            }
        }
        flag = flag || flag4 || flag3;
        if zero.y < 0f32 {
            velocity.y = math_max_f32(velocity.y + zero.y * 6f32 / 60f32, zero.y);
        } else if zero.y > 0f32 {
            velocity.y = math_min_f32(velocity.y + zero.y * 6f32 / 60f32, zero.y);
        }
        if zero.x < 0f32 && velocity.x > zero.x {
            velocity.x = math_max_f32(velocity.x + zero.x * 6f32 / 60f32, zero.x);
        }
        if zero.x > 0f32 && velocity.x < zero.x {
            velocity.x = math_min_f32(velocity.x + zero.x * 6f32 / 60f32, zero.x);
        }
        if num != -1 && velocity.x > 0f32 {
            let num8 = contacts[num as usize].x * 16 + (16 - item.height / 2).max(0);
            let mut right2 = item.right();
            if right2.x + velocity.x > num8 as f32 {
                if right2.x >= num8 as f32 {
                    right2.x = num8 as f32;
                    velocity.x = 0f32;
                    item.set_right(right2);
                } else {
                    velocity.x = num8 as f32 - right2.x;
                }
            }
        }
        if num2 != -1 && velocity.x < 0f32 {
            let num9 = contacts[num2 as usize].x * 16 + (item.height / 2).min(16);
            let mut position = item.position;
            if position.x + velocity.x < num9 as f32 {
                if position.x <= num9 as f32 {
                    position.x = num9 as f32;
                    velocity.x = 0f32;
                    item.position = position;
                } else {
                    velocity.x = num9 as f32 - position.x;
                }
            }
        }
    }
    g.collision.contacts = contacts;
    flag
}

/// `Collision.StepConveyorBelt(Entity entity, float gravDir, bool artificialRising = false)` (Collision.cs:4229).
/// `player` is `Some` when `entity is Player` (and then holds the fields of that player the method reads).
/// `entity.Hitbox.Inflate(2, 2)` and the discarded `entity.TopLeft/TopRight/BottomLeft/BottomRight` reads have no effect
/// in the original (they work on copies) and are left out.
pub fn step_conveyor_belt(g: &mut Game, entity: &mut EntityBox, player: Option<&PlayerConveyorFields>, grav_dir: f32, artificial_rising: bool) {
    if let Some(player) = player {
        if player.gfx_off_y.abs() > 2f32 || player.grap_count > 0 || player.pulley {
            return;
        }
        entity.height -= 5;
        entity.position.y += 5f32;
    }
    let mut num = 0;
    let mut num2 = 0;
    let mut flag = false;
    let num3 = entity.position.y as i32 + entity.height;
    let mut cache_for_conveyor_belts: Vec<Point> = std::mem::take(&mut g.collision.cache_for_conveyor_belts);
    cache_for_conveyor_belts.clear();
    get_entity_edge_tiles(&mut cache_for_conveyor_belts, entity, false, false, true, true);
    let vector = Vector2::splat(0.0001f32);
    let mut line_start = Vector2::default();
    let mut line_start2 = Vector2::default();
    let mut line_end = Vector2::default();
    let mut line_end2 = Vector2::default();
    for i in 0..cache_for_conveyor_belts.len() {
        let point = cache_for_conveyor_belts[i];
        if !in_world(g, point.x, point.y, 0) || (player.map_or(false, |p| p.on_track) && point.y < num3) {
            continue;
        }
        let tile = tile_at(g, point.x, point.y);
        if !tile.active() || !tile.nactive() {
            continue;
        }
        let num4 = tile_id::sets::CONVEYOR_DIRECTION[tile.type_ as usize];
        if num4 == 0 {
            continue;
        }
        line_start.x = (point.x * 16) as f32;
        line_start2.x = line_start.x;
        line_end.x = (point.x * 16 + 16) as f32;
        line_end2.x = line_end.x;
        match tile.slope() {
            1 => {
                line_start2.y = (point.y * 16) as f32;
                line_start.y = (point.y * 16 + 16) as f32;
                line_end.y = line_start.y;
                line_end2.y = line_end.y;
            }
            2 => {
                line_end2.y = (point.y * 16) as f32;
                line_start.y = (point.y * 16 + 16) as f32;
                line_end.y = line_start.y;
                line_start2.y = line_end.y;
            }
            3 => {
                line_start2.y = (point.y * 16) as f32;
                line_end2.y = line_start2.y;
                line_end.y = line_end2.y;
                line_start.y = (point.y * 16 + 16) as f32;
            }
            4 => {
                line_end2.y = (point.y * 16) as f32;
                line_start2.y = line_end2.y;
                line_start.y = line_start2.y;
                line_end.y = (point.y * 16 + 16) as f32;
            }
            _ => {
                if tile.half_brick() {
                    line_end2.y = (point.y * 16 + 8) as f32;
                    line_start2.y = line_end2.y;
                } else {
                    line_end2.y = (point.y * 16) as f32;
                    line_start2.y = line_end2.y;
                }
                line_end.y = (point.y * 16 + 16) as f32;
                line_start.y = line_end.y;
            }
        }
        let mut num5 = 0;
        let size = Vector2::new(entity.width as f32, entity.height as f32);
        if !tile_id::sets::PLATFORMS[tile.type_ as usize] && check_aab_bv_line_collision2(entity.position - vector, size + vector * 2f32, line_start, line_end) {
            num5 -= 1;
        }
        if check_aab_bv_line_collision2(entity.position - vector, size + vector * 2f32, line_start2, line_end2) {
            num5 += 1;
        }
        if num5 != 0 {
            flag = true;
            num += num4 * num5 * (grav_dir as i32);
            if tile.left_slope() {
                num2 += (grav_dir as i32) * -num4;
            }
            if tile.right_slope() {
                num2 -= (grav_dir as i32) * -num4;
            }
        }
    }
    g.collision.cache_for_conveyor_belts = cache_for_conveyor_belts;
    if player.is_some() {
        entity.height += 5;
        entity.position.y -= 5f32;
    }
    if !flag {
        return;
    }
    if artificial_rising {
        num2 = -1;
    }
    if num != 0 || artificial_rising {
        num = math_sign(num);
        num2 = math_sign(num2);
        let mut velocity = Vector2::normalized(Vector2::new(num as f32 * grav_dir, num2 as f32)) * 2.5f32;
        let mut vector2 = tile_collision(g, entity.position, velocity, entity.width, entity.height, false, false, grav_dir as i32, false, false, true);
        entity.position += vector2;
        if !artificial_rising {
            vector2 = tile_collision(g, entity.position, Vector2::new(0f32, 2.5f32 * grav_dir), entity.width, entity.height, false, false, grav_dir as i32, false, false, true);
            entity.position += vector2;
        }
        if artificial_rising {
            velocity = Vector2::new(num as f32, num2 as f32);
            vector2 = tile_collision(g, entity.position - velocity, velocity, entity.width, entity.height, false, false, grav_dir as i32, false, false, true);
            entity.position += vector2;
        }
    }
}

/// `Collision.TryFindingConveyorBeltRising(Entity entity, float gravDir)` (Collision.cs:4365).
/// `entity is WorldItem` is `is_world_item`.
pub fn try_finding_conveyor_belt_rising(g: &Game, entity: &EntityBox, is_world_item: bool, grav_dir: f32) -> bool {
    if is_world_item {
        let p = to_tile_coordinates(entity.bottom_left() + Vector2::new(-0.5f32, -0.01f32));
        let mut num = 0f32;
        if in_world(g, p.x, p.y, 0) {
            let tile = tile_at(g, p.x, p.y);
            if tile.active() && tile.nactive() && tile.slope() == 0 {
                let num2 = tile_id::sets::CONVEYOR_DIRECTION[tile.type_ as usize];
                num += num2 as f32 * grav_dir;
            }
        }
        let p2 = to_tile_coordinates(entity.bottom_right() + Vector2::new(0.5f32, -0.01f32));
        if in_world(g, p2.x, p2.y, 0) {
            let tile2 = tile_at(g, p2.x, p2.y);
            if tile2.active() && tile2.nactive() && tile2.slope() == 0 {
                let num3 = tile_id::sets::CONVEYOR_DIRECTION[tile2.type_ as usize];
                num += num3 as f32 * grav_dir * -1f32;
            }
        }
        return num < 0f32;
    }
    false
}
