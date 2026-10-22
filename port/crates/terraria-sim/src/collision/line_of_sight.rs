//! `Terraria.Collision`, lines of sight and walls along a line: `CanHit*`, `CanHitLine`, `HitLine`,
//! `AnyWallOfTypeOnLine`, `HitLineWall`, `EmptyTile` (Collision.cs:383-1393).
//!
//! `try { } catch { }` of the original is `utils::catch`; `Main.tile[x, y] == null` is `utils::tile_slot` giving `None`
//! (outside the world, inside the array), an exception from `Main.tile[x, y]` is `utils::tile_at` throwing.

use terraria_core::xna::{Point, Rectangle, Vector2};

use super::utils::{catch, clamp_i32, math_sign, null_check, tile_at, tile_slot, to_point};
use super::EntityBox;
use crate::game::Game;
use crate::unported;

/// `Collision.CanHit(Entity source, Entity target)` (Collision.cs:383). `Entity` is mirrored by [`EntityBox`]
/// (`position`, `width`, `height`: the only fields read).
pub fn can_hit_entity(g: &Game, source: &EntityBox, target: &EntityBox) -> bool {
    can_hit_vector2(g, source.position, source.width, source.height, target.position, target.width, target.height)
}

/// `Collision.CanHit(Entity source, NPCAimedTarget target)` (Collision.cs:388). `NPCAimedTarget.Position/Width/Height`
/// are mirrored by an [`EntityBox`].
pub fn can_hit_aimed_target(g: &Game, source: &EntityBox, target: &EntityBox) -> bool {
    can_hit_vector2(g, source.position, source.width, source.height, target.position, target.width, target.height)
}

/// `Collision.CanHit(Vector2 Position1, int Width1, int Height1, Vector2 Position2, int Width2, int Height2)` (Collision.cs:393)
pub fn can_hit_vector2(g: &Game, position1: Vector2, width1: i32, height1: i32, position2: Vector2, width2: i32, height2: i32) -> bool {
    can_hit(g, to_point(position1), width1, height1, to_point(position2), width2, height2)
}

/// `Collision.CanHit(Point Position1, int Width1, int Height1, Point Position2, int Width2, int Height2)` (Collision.cs:398)
pub fn can_hit(g: &Game, position1: Point, width1: i32, height1: i32, position2: Point, width2: i32, height2: i32) -> bool {
    let mut num = (position1.x + width1 / 2) / 16;
    let mut num2 = (position1.y + height1 / 2) / 16;
    let mut num3 = (position2.x + width2 / 2) / 16;
    let mut num4 = (position2.y + height2 / 2) / 16;
    if num <= 1 {
        num = 1;
    }
    if num >= g.main.max_tiles_x {
        num = g.main.max_tiles_x - 1;
    }
    if num3 <= 1 {
        num3 = 1;
    }
    if num3 >= g.main.max_tiles_x {
        num3 = g.main.max_tiles_x - 1;
    }
    if num2 <= 1 {
        num2 = 1;
    }
    if num2 >= g.main.max_tiles_y - 40 {
        num2 = g.main.max_tiles_y - 40;
    }
    if num4 <= 1 {
        num4 = 1;
    }
    if num4 >= g.main.max_tiles_y - 40 {
        num4 = g.main.max_tiles_y - 40;
    }
    catch(|| {
        loop {
            let num5 = (num - num3).abs();
            let num6 = (num2 - num4).abs();
            if num == num3 && num2 == num4 {
                return true;
            }
            if num5 > num6 {
                num = if num >= num3 { num - 1 } else { num + 1 };
                let Some(above) = tile_slot(g, num, num2 - 1) else { return false }; // `== null` -> return false
                let Some(below) = tile_slot(g, num, num2 + 1) else { return false };
                if !above.in_active()
                    && above.active()
                    && g.main.tile_solid[above.type_ as usize]
                    && !g.main.tile_solid_top[above.type_ as usize]
                    && above.slope() == 0
                    && !above.half_brick()
                    && !below.in_active()
                    && below.active()
                    && g.main.tile_solid[below.type_ as usize]
                    && !g.main.tile_solid_top[below.type_ as usize]
                    && below.slope() == 0
                    && !below.half_brick()
                {
                    return false;
                }
            } else {
                num2 = if num2 >= num4 { num2 - 1 } else { num2 + 1 };
                let Some(left) = tile_slot(g, num - 1, num2) else { return false };
                let Some(right) = tile_slot(g, num + 1, num2) else { return false };
                if !left.in_active()
                    && left.active()
                    && g.main.tile_solid[left.type_ as usize]
                    && !g.main.tile_solid_top[left.type_ as usize]
                    && left.slope() == 0
                    && !left.half_brick()
                    && !right.in_active()
                    && right.active()
                    && g.main.tile_solid[right.type_ as usize]
                    && !g.main.tile_solid_top[right.type_ as usize]
                    && right.slope() == 0
                    && !right.half_brick()
                {
                    return false;
                }
            }
            let Some(here) = tile_slot(g, num, num2) else { return false };
            if !(here.in_active() || !here.active() || !g.main.tile_solid[here.type_ as usize] || g.main.tile_solid_top[here.type_ as usize]) {
                break;
            }
        }
        false
    })
    .unwrap_or(false)
}

/// `Collision.CanHitWithCheck(Entity source, Entity target, Utils.TileActionAttempt check)` (Collision.cs:492)
pub fn can_hit_with_check_entity(g: &Game, source: &EntityBox, target: &EntityBox, check: &mut dyn FnMut(i32, i32) -> bool) -> bool {
    can_hit_with_check(g, source.position, source.width, source.height, target.position, target.width, target.height, check)
}

/// `Collision.CanHit(Entity source, NPCAimedTarget target, Utils.TileActionAttempt check)` (Collision.cs:497); the
/// original calls `CanHitWithCheck` from it.
pub fn can_hit_aimed_target_with_check(g: &Game, source: &EntityBox, target: &EntityBox, check: &mut dyn FnMut(i32, i32) -> bool) -> bool {
    can_hit_with_check(g, source.position, source.width, source.height, target.position, target.width, target.height, check)
}

/// `Collision.CanHitWithCheck(Vector2 Position1, int Width1, int Height1, Vector2 Position2, int Width2, int Height2, Utils.TileActionAttempt check)` (Collision.cs:502)
pub fn can_hit_with_check(
    g: &Game,
    position1: Vector2,
    width1: i32,
    height1: i32,
    position2: Vector2,
    width2: i32,
    height2: i32,
    check: &mut dyn FnMut(i32, i32) -> bool,
) -> bool {
    let mut num = ((position1.x + (width1 / 2) as f32) / 16f32) as i32;
    let mut num2 = ((position1.y + (height1 / 2) as f32) / 16f32) as i32;
    let mut num3 = ((position2.x + (width2 / 2) as f32) / 16f32) as i32;
    let mut num4 = ((position2.y + (height2 / 2) as f32) / 16f32) as i32;
    if num <= 1 {
        num = 1;
    }
    if num >= g.main.max_tiles_x {
        num = g.main.max_tiles_x - 1;
    }
    if num3 <= 1 {
        num3 = 1;
    }
    if num3 >= g.main.max_tiles_x {
        num3 = g.main.max_tiles_x - 1;
    }
    if num2 <= 1 {
        num2 = 1;
    }
    if num2 >= g.main.max_tiles_y - 40 {
        num2 = g.main.max_tiles_y - 40;
    }
    if num4 <= 1 {
        num4 = 1;
    }
    if num4 >= g.main.max_tiles_y - 40 {
        num4 = g.main.max_tiles_y - 40;
    }
    catch(|| {
        loop {
            let num5 = (num - num3).abs();
            let num6 = (num2 - num4).abs();
            if num == num3 && num2 == num4 {
                return true;
            }
            if num5 > num6 {
                num = if num >= num3 { num - 1 } else { num + 1 };
                let Some(above) = tile_slot(g, num, num2 - 1) else { return false }; // `== null` -> return false
                let Some(below) = tile_slot(g, num, num2 + 1) else { return false };
                if !above.in_active()
                    && above.active()
                    && g.main.tile_solid[above.type_ as usize]
                    && !g.main.tile_solid_top[above.type_ as usize]
                    && above.slope() == 0
                    && !above.half_brick()
                    && !below.in_active()
                    && below.active()
                    && g.main.tile_solid[below.type_ as usize]
                    && !g.main.tile_solid_top[below.type_ as usize]
                    && below.slope() == 0
                    && !below.half_brick()
                {
                    return false;
                }
            } else {
                num2 = if num2 >= num4 { num2 - 1 } else { num2 + 1 };
                let Some(left) = tile_slot(g, num - 1, num2) else { return false };
                let Some(right) = tile_slot(g, num + 1, num2) else { return false };
                if !left.in_active()
                    && left.active()
                    && g.main.tile_solid[left.type_ as usize]
                    && !g.main.tile_solid_top[left.type_ as usize]
                    && left.slope() == 0
                    && !left.half_brick()
                    && !right.in_active()
                    && right.active()
                    && g.main.tile_solid[right.type_ as usize]
                    && !g.main.tile_solid_top[right.type_ as usize]
                    && right.slope() == 0
                    && !right.half_brick()
                {
                    return false;
                }
            }
            let Some(here) = tile_slot(g, num, num2) else { return false };
            if !here.in_active() && here.active() && g.main.tile_solid[here.type_ as usize] && !g.main.tile_solid_top[here.type_ as usize] {
                return false;
            }
            if !check(num, num2) {
                break;
            }
        }
        false
    })
    .unwrap_or(false)
}

/// `Collision.CanHitLine` (Collision.cs:600)
pub fn can_hit_line(g: &Game, position1: Vector2, width1: i32, height1: i32, position2: Vector2, width2: i32, height2: i32) -> bool {
    let mut num = ((position1.x + (width1 / 2) as f32) / 16f32) as i32;
    let mut num2 = ((position1.y + (height1 / 2) as f32) / 16f32) as i32;
    let mut num3 = ((position2.x + (width2 / 2) as f32) / 16f32) as i32;
    let mut num4 = ((position2.y + (height2 / 2) as f32) / 16f32) as i32;
    if num <= 1 {
        num = 1;
    }
    if num >= g.main.max_tiles_x {
        num = g.main.max_tiles_x - 1;
    }
    if num3 <= 1 {
        num3 = 1;
    }
    if num3 >= g.main.max_tiles_x {
        num3 = g.main.max_tiles_x - 1;
    }
    if num2 <= 1 {
        num2 = 1;
    }
    if num2 >= g.main.max_tiles_y - 40 {
        num2 = g.main.max_tiles_y - 40;
    }
    if num4 <= 1 {
        num4 = 1;
    }
    if num4 >= g.main.max_tiles_y - 40 {
        num4 = g.main.max_tiles_y - 40;
    }
    let num5 = (num - num3).abs() as f32;
    let num6 = (num2 - num4).abs() as f32;
    if num5 == 0f32 && num6 == 0f32 {
        return true;
    }
    let mut num7 = 1f32;
    let mut num8 = 1f32;
    if num5 == 0f32 || num6 == 0f32 {
        if num5 == 0f32 {
            num7 = 0f32;
        }
        if num6 == 0f32 {
            num8 = 0f32;
        }
    } else if num5 > num6 {
        num7 = num5 / num6;
    } else {
        num8 = num6 / num5;
    }
    let mut num9 = 0f32;
    let mut num10 = 0f32;
    let mut num11 = 1;
    if num2 < num4 {
        num11 = 2;
    }
    let mut num12 = num5 as i32;
    let mut num13 = num6 as i32;
    let num14 = math_sign(num3 - num);
    let num15 = math_sign(num4 - num2);
    let mut flag = false;
    let mut flag2 = false;
    catch(|| {
        loop {
            match num11 {
                2 => {
                    num9 += num7;
                    let num17 = num9 as i32;
                    num9 -= num17 as f32;
                    for _j in 0..num17 {
                        let Some(_) = tile_slot(g, num, num2 - 1) else { return false };
                        let Some(_) = tile_slot(g, num, num2) else { return false };
                        let Some(_) = tile_slot(g, num, num2 + 1) else { return false };
                        let tile4 = tile_at(g, num, num2 - 1);
                        let tile5 = tile_at(g, num, num2 + 1);
                        let tile6 = tile_at(g, num, num2);
                        if (!tile4.in_active() && tile4.active() && g.main.tile_solid[tile4.type_ as usize] && !g.main.tile_solid_top[tile4.type_ as usize])
                            || (!tile5.in_active() && tile5.active() && g.main.tile_solid[tile5.type_ as usize] && !g.main.tile_solid_top[tile5.type_ as usize])
                            || (!tile6.in_active() && tile6.active() && g.main.tile_solid[tile6.type_ as usize] && !g.main.tile_solid_top[tile6.type_ as usize])
                        {
                            return false;
                        }
                        if num12 == 0 && num13 == 0 {
                            flag = true;
                            break;
                        }
                        num += num14;
                        num12 -= 1;
                        if num12 == 0 && num13 == 0 && num17 == 1 {
                            flag2 = true;
                        }
                    }
                    if num13 != 0 {
                        num11 = 1;
                    }
                }
                1 => {
                    num10 += num8;
                    let num16 = num10 as i32;
                    num10 -= num16 as f32;
                    for _i in 0..num16 {
                        let Some(_) = tile_slot(g, num - 1, num2) else { return false };
                        let Some(_) = tile_slot(g, num, num2) else { return false };
                        let Some(_) = tile_slot(g, num + 1, num2) else { return false };
                        let tile = tile_at(g, num - 1, num2);
                        let tile2 = tile_at(g, num + 1, num2);
                        let tile3 = tile_at(g, num, num2);
                        if (!tile.in_active() && tile.active() && g.main.tile_solid[tile.type_ as usize] && !g.main.tile_solid_top[tile.type_ as usize])
                            || (!tile2.in_active() && tile2.active() && g.main.tile_solid[tile2.type_ as usize] && !g.main.tile_solid_top[tile2.type_ as usize])
                            || (!tile3.in_active() && tile3.active() && g.main.tile_solid[tile3.type_ as usize] && !g.main.tile_solid_top[tile3.type_ as usize])
                        {
                            return false;
                        }
                        if num12 == 0 && num13 == 0 {
                            flag = true;
                            break;
                        }
                        num2 += num15;
                        num13 -= 1;
                        if num12 == 0 && num13 == 0 && num16 == 1 {
                            flag2 = true;
                        }
                    }
                    if num12 != 0 {
                        num11 = 2;
                    }
                }
                _ => {}
            }
            let Some(tile7) = tile_slot(g, num, num2) else { return false };
            if !tile7.in_active() && tile7.active() && g.main.tile_solid[tile7.type_ as usize] && !g.main.tile_solid_top[tile7.type_ as usize] {
                return false;
            }
            if flag || flag2 {
                break;
            }
        }
        true
    })
    .unwrap_or(false)
}

/// `Collision.HitLine` (Collision.cs:792). `col` (`out Point`) is the second element of the result.
pub fn hit_line(g: &Game, x1: i32, y1: i32, x2: i32, y2: i32, ignore_x: i32, ignore_y: i32, ignore_targets: &[Point]) -> (bool, Point) {
    let mut value = x1;
    let mut value2 = y1;
    let mut value3 = x2;
    let mut value4 = y2;
    value = clamp_i32(value, 1, g.main.max_tiles_x - 1);
    value3 = clamp_i32(value3, 1, g.main.max_tiles_x - 1);
    value2 = clamp_i32(value2, 1, g.main.max_tiles_y - 40);
    value4 = clamp_i32(value4, 1, g.main.max_tiles_y - 40);
    let num = (value - value3).abs() as f32;
    let num2 = (value2 - value4).abs() as f32;
    if num == 0f32 && num2 == 0f32 {
        return (true, Point::new(value, value2));
    }
    let mut num3 = 1f32;
    let mut num4 = 1f32;
    if num == 0f32 || num2 == 0f32 {
        if num == 0f32 {
            num3 = 0f32;
        }
        if num2 == 0f32 {
            num4 = 0f32;
        }
    } else if num > num2 {
        num3 = num / num2;
    } else {
        num4 = num2 / num;
    }
    let mut num5 = 0f32;
    let mut num6 = 0f32;
    let mut num7 = 1;
    if value2 < value4 {
        num7 = 2;
    }
    let mut num8 = num as i32;
    let mut num9 = num2 as i32;
    let num10 = math_sign(value3 - value);
    let num11 = math_sign(value4 - value2);
    let mut flag = false;
    let mut flag2 = false;
    catch(|| {
        loop {
            match num7 {
                2 => {
                    num5 += num3;
                    let num13 = num5 as i32;
                    num5 -= num13 as f32;
                    for _j in 0..num13 {
                        let Some(tile4) = tile_slot(g, value, value2 - 1) else { return (false, Point::new(value, value2 - 1)) };
                        let Some(tile5) = tile_slot(g, value, value2 + 1) else { return (false, Point::new(value, value2 + 1)) };
                        let tile6 = tile_slot(g, value, value2); // may be null: member access below then throws
                        if !ignore_targets.contains(&Point::new(value, value2))
                            && !ignore_targets.contains(&Point::new(value, value2 - 1))
                            && !ignore_targets.contains(&Point::new(value, value2 + 1))
                        {
                            if ignore_y != -1 && num11 < 0 && !tile4.in_active() && tile4.active() && g.main.tile_solid[tile4.type_ as usize] && !g.main.tile_solid_top[tile4.type_ as usize] {
                                return (true, Point::new(value, value2 - 1));
                            }
                            if ignore_y != 1 && num11 > 0 && !tile5.in_active() && tile5.active() && g.main.tile_solid[tile5.type_ as usize] && !g.main.tile_solid_top[tile5.type_ as usize] {
                                return (true, Point::new(value, value2 + 1));
                            }
                            let tile6 = null_check(tile6);
                            if !tile6.in_active() && tile6.active() && g.main.tile_solid[tile6.type_ as usize] && !g.main.tile_solid_top[tile6.type_ as usize] {
                                return (true, Point::new(value, value2));
                            }
                        }
                        if num8 == 0 && num9 == 0 {
                            flag = true;
                            break;
                        }
                        value += num10;
                        num8 -= 1;
                        if num8 == 0 && num9 == 0 && num13 == 1 {
                            flag2 = true;
                        }
                    }
                    if num9 != 0 {
                        num7 = 1;
                    }
                }
                1 => {
                    num6 += num4;
                    let num12 = num6 as i32;
                    num6 -= num12 as f32;
                    for _i in 0..num12 {
                        let Some(tile) = tile_slot(g, value - 1, value2) else { return (false, Point::new(value - 1, value2)) };
                        let Some(tile2) = tile_slot(g, value + 1, value2) else { return (false, Point::new(value + 1, value2)) };
                        let tile3 = tile_slot(g, value, value2); // may be null: member access below then throws
                        if !ignore_targets.contains(&Point::new(value, value2))
                            && !ignore_targets.contains(&Point::new(value - 1, value2))
                            && !ignore_targets.contains(&Point::new(value + 1, value2))
                        {
                            if ignore_x != -1 && num10 < 0 && !tile.in_active() && tile.active() && g.main.tile_solid[tile.type_ as usize] && !g.main.tile_solid_top[tile.type_ as usize] {
                                return (true, Point::new(value - 1, value2));
                            }
                            if ignore_x != 1 && num10 > 0 && !tile2.in_active() && tile2.active() && g.main.tile_solid[tile2.type_ as usize] && !g.main.tile_solid_top[tile2.type_ as usize] {
                                return (true, Point::new(value + 1, value2));
                            }
                            let tile3 = null_check(tile3);
                            if !tile3.in_active() && tile3.active() && g.main.tile_solid[tile3.type_ as usize] && !g.main.tile_solid_top[tile3.type_ as usize] {
                                return (true, Point::new(value, value2));
                            }
                        }
                        if num8 == 0 && num9 == 0 {
                            flag = true;
                            break;
                        }
                        value2 += num11;
                        num9 -= 1;
                        if num8 == 0 && num9 == 0 && num12 == 1 {
                            flag2 = true;
                        }
                    }
                    if num8 != 0 {
                        num7 = 2;
                    }
                }
                _ => {}
            }
            let Some(tile7) = tile_slot(g, value, value2) else { return (false, Point::new(value, value2)) };
            if !ignore_targets.contains(&Point::new(value, value2)) && !tile7.in_active() && tile7.active() && g.main.tile_solid[tile7.type_ as usize] && !g.main.tile_solid_top[tile7.type_ as usize] {
                return (true, Point::new(value, value2));
            }
            if flag || flag2 {
                break;
            }
        }
        (true, Point::new(value, value2))
    })
    .unwrap_or((false, Point::new(x1, y1)))
}

/// `Collision.AnyWallOfTypeOnLine` (Collision.cs:985)
pub fn any_wall_of_type_on_line(g: &Game, x1: i32, y1: i32, x2: i32, y2: i32, wall_id: u16) -> bool {
    let mut num = x1;
    let mut num2 = y1;
    let mut num3 = x2;
    let mut num4 = y2;
    if num <= 1 {
        num = 1;
    }
    if num >= g.main.max_tiles_x {
        num = g.main.max_tiles_x - 1;
    }
    if num3 <= 1 {
        num3 = 1;
    }
    if num3 >= g.main.max_tiles_x {
        num3 = g.main.max_tiles_x - 1;
    }
    if num2 <= 1 {
        num2 = 1;
    }
    if num2 >= g.main.max_tiles_y - 40 {
        num2 = g.main.max_tiles_y - 40;
    }
    if num4 <= 1 {
        num4 = 1;
    }
    if num4 >= g.main.max_tiles_y - 40 {
        num4 = g.main.max_tiles_y - 40;
    }
    let num5 = (num - num3).abs() as f32;
    let num6 = (num2 - num4).abs() as f32;
    if num5 == 0f32 && num6 == 0f32 {
        return false;
    }
    let mut num7 = 1f32;
    let mut num8 = 1f32;
    if num5 == 0f32 || num6 == 0f32 {
        if num5 == 0f32 {
            num7 = 0f32;
        }
        if num6 == 0f32 {
            num8 = 0f32;
        }
    } else if num5 > num6 {
        num7 = num5 / num6;
    } else {
        num8 = num6 / num5;
    }
    let mut num9 = 0f32;
    let mut num10 = 0f32;
    let mut num11 = 1;
    if num2 < num4 {
        num11 = 2;
    }
    let mut num12 = num5 as i32;
    let mut num13 = num6 as i32;
    let num14 = math_sign(num3 - num);
    let num15 = math_sign(num4 - num2);
    let mut flag = false;
    let mut flag2 = false;
    catch(|| {
        loop {
            match num11 {
                2 => {
                    num9 += num7;
                    let num17 = num9 as i32;
                    num9 -= num17 as f32;
                    for _j in 0..num17 {
                        let _ = tile_slot(g, num, num2);
                        if hit_specific_wall_substep(g, num, num2, wall_id) {
                            return true;
                        }
                        if num12 == 0 && num13 == 0 {
                            flag = true;
                            break;
                        }
                        num += num14;
                        num12 -= 1;
                        if num12 == 0 && num13 == 0 && num17 == 1 {
                            flag2 = true;
                        }
                    }
                    if num13 != 0 {
                        num11 = 1;
                    }
                }
                1 => {
                    num10 += num8;
                    let num16 = num10 as i32;
                    num10 -= num16 as f32;
                    for _i in 0..num16 {
                        let _ = tile_slot(g, num, num2);
                        if hit_specific_wall_substep(g, num, num2, wall_id) {
                            return true;
                        }
                        if num12 == 0 && num13 == 0 {
                            flag = true;
                            break;
                        }
                        num2 += num15;
                        num13 -= 1;
                        if num12 == 0 && num13 == 0 && num16 == 1 {
                            flag2 = true;
                        }
                    }
                    if num12 != 0 {
                        num11 = 2;
                    }
                }
                _ => {}
            }
            let Some(_) = tile_slot(g, num, num2) else { return false };
            let _ = tile_slot(g, num, num2);
            if hit_specific_wall_substep(g, num, num2, wall_id) {
                return true;
            }
            if flag || flag2 {
                break;
            }
        }
        false
    })
    .unwrap_or(false)
}

/// `Collision.HitSpecificWallSubstep` (Collision.cs:1149)
pub fn hit_specific_wall_substep(g: &Game, x: i32, y: i32, wall_id: u16) -> bool {
    if tile_at(g, x, y).wall == wall_id {
        return true;
    }
    false
}

/// `Collision.HitLineWall` (Collision.cs:1158)
pub fn hit_line_wall(g: &Game, x1: i32, y1: i32, x2: i32, y2: i32) -> Point {
    let mut num = x1;
    let mut num2 = y1;
    let mut num3 = x2;
    let mut num4 = y2;
    if num <= 1 {
        num = 1;
    }
    if num >= g.main.max_tiles_x {
        num = g.main.max_tiles_x - 1;
    }
    if num3 <= 1 {
        num3 = 1;
    }
    if num3 >= g.main.max_tiles_x {
        num3 = g.main.max_tiles_x - 1;
    }
    if num2 <= 1 {
        num2 = 1;
    }
    if num2 >= g.main.max_tiles_y - 40 {
        num2 = g.main.max_tiles_y - 40;
    }
    if num4 <= 1 {
        num4 = 1;
    }
    if num4 >= g.main.max_tiles_y - 40 {
        num4 = g.main.max_tiles_y - 40;
    }
    let num5 = (num - num3).abs() as f32;
    let num6 = (num2 - num4).abs() as f32;
    if num5 == 0f32 && num6 == 0f32 {
        return Point::new(num, num2);
    }
    let mut num7 = 1f32;
    let mut num8 = 1f32;
    if num5 == 0f32 || num6 == 0f32 {
        if num5 == 0f32 {
            num7 = 0f32;
        }
        if num6 == 0f32 {
            num8 = 0f32;
        }
    } else if num5 > num6 {
        num7 = num5 / num6;
    } else {
        num8 = num6 / num5;
    }
    let mut num9 = 0f32;
    let mut num10 = 0f32;
    let mut num11 = 1;
    if num2 < num4 {
        num11 = 2;
    }
    let mut num12 = num5 as i32;
    let mut num13 = num6 as i32;
    let num14 = math_sign(num3 - num);
    let num15 = math_sign(num4 - num2);
    let mut flag = false;
    let mut flag2 = false;
    catch(|| {
        loop {
            match num11 {
                2 => {
                    num9 += num7;
                    let num17 = num9 as i32;
                    num9 -= num17 as f32;
                    for _j in 0..num17 {
                        let _ = tile_slot(g, num, num2);
                        if hit_wall_substep(g, num, num2) {
                            return Point::new(num, num2);
                        }
                        if num12 == 0 && num13 == 0 {
                            flag = true;
                            break;
                        }
                        num += num14;
                        num12 -= 1;
                        if num12 == 0 && num13 == 0 && num17 == 1 {
                            flag2 = true;
                        }
                    }
                    if num13 != 0 {
                        num11 = 1;
                    }
                }
                1 => {
                    num10 += num8;
                    let num16 = num10 as i32;
                    num10 -= num16 as f32;
                    for _i in 0..num16 {
                        let _ = tile_slot(g, num, num2);
                        if hit_wall_substep(g, num, num2) {
                            return Point::new(num, num2);
                        }
                        if num12 == 0 && num13 == 0 {
                            flag = true;
                            break;
                        }
                        num2 += num15;
                        num13 -= 1;
                        if num12 == 0 && num13 == 0 && num16 == 1 {
                            flag2 = true;
                        }
                    }
                    if num12 != 0 {
                        num11 = 2;
                    }
                }
                _ => {}
            }
            let Some(_) = tile_slot(g, num, num2) else { return Point::new(-1, -1) };
            let _ = tile_slot(g, num, num2);
            if hit_wall_substep(g, num, num2) {
                return Point::new(num, num2);
            }
            if flag || flag2 {
                break;
            }
        }
        Point::new(num, num2)
    })
    .unwrap_or(Point::new(-1, -1))
}

/// `Collision.HitWallSubstep` (Collision.cs:1322)
pub fn hit_wall_substep(g: &Game, x: i32, y: i32) -> bool {
    if tile_at(g, x, y).wall == 0 {
        return false;
    }
    let mut flag = false;
    if g.main.wall_house[tile_at(g, x, y).wall as usize] {
        flag = true;
    }
    if !flag {
        for i in -1..2 {
            for j in -1..2 {
                if (i != 0 || j != 0) && tile_at(g, x + i, y + j).wall == 0 {
                    flag = true;
                }
            }
        }
    }
    if tile_at(g, x, y).active() && flag {
        let mut flag2 = true;
        for k in -1..2 {
            for l in -1..2 {
                if k != 0 || l != 0 {
                    let tile = tile_at(g, x + k, y + l);
                    if !tile.active() || !g.main.tile_solid[tile.type_ as usize] || g.main.tile_solid_top[tile.type_ as usize] {
                        flag2 = false;
                    }
                }
            }
        }
        if flag2 {
            flag = false;
        }
    }
    flag
}

/// `Collision.EmptyTile` (Collision.cs:1371)
pub fn empty_tile(g: &Game, i: i32, j: i32, ignore_tiles: bool) -> bool {
    let _rectangle = Rectangle::new(i * 16, j * 16, 16, 16); // only used by the player / NPC loops below (unported)
    if tile_at(g, i, j).active() && !ignore_tiles {
        return false;
    }
    // `for k in 0..255 { Main.player[k]; if active && !dead && !ghost && rectangle.Intersects(player box) return false }` and
    // `for l in 0..Main.maxNPCs { Main.npc[l]; if active && rectangle.Intersects(npc box) return false }`:
    // players and NPCs are not ported; continues as if no player and no NPC is active.
    unported!("Main.player[k] / Main.npc[l] (Collision.EmptyTile)");
    true
}
