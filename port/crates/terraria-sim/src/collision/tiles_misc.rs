//! `Terraria.Collision`, tile queries with side effects or on tile lists: `HitTilesInACircle`, `HitTiles`,
//! `AnyHurtingTiles`, `HurtTiles`, `CanTileHurt`, `SwitchTiles`, `StickyTiles`, `SolidTiles*`, `SolidFullTiles`
//! (Collision.cs:3039-3585).
//!
//! Not ported, called through `unported!` at the original's call site: `WorldGen.KillTile`, `Wiring.HitSwitch`,
//! `NetMessage.SendData`, `Dust.NewDust`.

use terraria_core::xna::Vector2;
use terraria_data::sets::tile_id;

use super::utils::{float_intersect, tile_at};
use super::HurtTile;
use crate::game::Game;
use crate::unported;

/// `Collision.HitTilesInACircle(Position, Velocity, Width, Height)` (Collision.cs:3039).
/// `WorldGen.KillTile(i, j, fail: true, effectOnly: true)` is `unported!("WorldGen.KillTile")`; the neutral continuation
/// is "no effect" (with `fail` and `effectOnly` the original only spawns dust and plays a sound).
pub fn hit_tiles_in_a_circle(g: &mut Game, position: Vector2, velocity: Vector2, width: i32, height: i32) {
    let vector = position + velocity;
    let mut num = (position.x / 16f32) as i32 - 1;
    let mut num2 = ((position.x + width as f32) / 16f32) as i32 + 2;
    let mut num3 = (position.y / 16f32) as i32 - 1;
    let mut num4 = ((position.y + height as f32) / 16f32) as i32 + 2;
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
    let mut vector2 = Vector2::default();
    for i in num..num2 {
        for j in num3..num4 {
            let t = tile_at(g, i, j);
            if !t.in_active() && t.active() && (g.main.tile_solid[t.type_ as usize] || (g.main.tile_solid_top[t.type_ as usize] && t.frame_y == 0)) {
                vector2.x = (i * 16) as f32;
                vector2.y = (j * 16) as f32;
                let mut num5 = 16;
                if t.half_brick() {
                    vector2.y += 8f32;
                    num5 -= 8;
                }
                if vector.x + width as f32 >= vector2.x
                    && vector.x <= vector2.x + 16f32
                    && vector.y + height as f32 >= vector2.y
                    && vector.y <= vector2.y + num5 as f32
                    && (Vector2::new(vector.x + (width / 2) as f32, vector.y + (height / 2) as f32) - Vector2::new(vector2.x + 8f32, vector2.y + 8f32)).length() < ((width + height) / 4) as f32
                {
                    unported!("WorldGen.KillTile");
                }
            }
        }
    }
}

/// `Collision.HitTiles(Position, Velocity, Width, Height)` (Collision.cs:3086).
/// `WorldGen.KillTile(i, j, fail: true, effectOnly: true)` is `unported!("WorldGen.KillTile")`; the neutral continuation
/// is "no effect".
pub fn hit_tiles(g: &mut Game, position: Vector2, velocity: Vector2, width: i32, height: i32) {
    let vector = position + velocity;
    let mut num = (position.x / 16f32) as i32 - 1;
    let mut num2 = ((position.x + width as f32) / 16f32) as i32 + 2;
    let mut num3 = (position.y / 16f32) as i32 - 1;
    let mut num4 = ((position.y + height as f32) / 16f32) as i32 + 2;
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
    let mut vector2 = Vector2::default();
    for i in num..num2 {
        for j in num3..num4 {
            let t = tile_at(g, i, j);
            if !t.in_active() && t.active() && (g.main.tile_solid[t.type_ as usize] || (g.main.tile_solid_top[t.type_ as usize] && t.frame_y == 0)) {
                vector2.x = (i * 16) as f32;
                vector2.y = (j * 16) as f32;
                let mut num5 = 16;
                if t.half_brick() {
                    vector2.y += 8f32;
                    num5 -= 8;
                }
                if vector.x + width as f32 >= vector2.x && vector.x <= vector2.x + 16f32 && vector.y + height as f32 >= vector2.y && vector.y <= vector2.y + num5 as f32 {
                    unported!("WorldGen.KillTile");
                }
            }
        }
    }
}

/// `Collision.AnyHurtingTiles(Position, Width, Height)` (Collision.cs:3133): `HurtTiles(Position, Width, Height, null).type >= 0`.
pub fn any_hurting_tiles(g: &Game, position: Vector2, width: i32, height: i32) -> bool {
    hurt_tiles(g, position, width, height, None).type_ >= 0
}

/// `Collision.HurtTiles(Position, Width, Height, Player player)` (Collision.cs:3138). The only `Player` field read
/// (by `CanTileHurt`) is `fireWalk`: `player` is `None` for `null`, else `Some(player.fireWalk)`.
pub fn hurt_tiles(g: &Game, position: Vector2, width: i32, height: i32, player: Option<bool>) -> HurtTile {
    let mut num = (position.x / 16f32) as i32 - 1;
    let mut num2 = ((position.x + width as f32) / 16f32) as i32 + 2;
    let mut num3 = (position.y / 16f32) as i32 - 1;
    let mut num4 = ((position.y + height as f32) / 16f32) as i32 + 2;
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
    let mut vector = Vector2::default();
    for i in num..num2 {
        for j in num3..num4 {
            let tile = tile_at(g, i, j);
            if tile.in_active() || !tile.active() {
                continue;
            }
            vector.x = (i * 16) as f32;
            vector.y = (j * 16) as f32;
            let mut num5 = 16;
            if tile.half_brick() {
                vector.y += 8f32;
                num5 -= 8;
            }
            let mut num6 = 0;
            if tile_id::sets::SUFFOCATE[tile.type_ as usize] {
                num6 = 2;
            }
            if position.x + width as f32 - (num6 as f32) < vector.x
                || position.x + (num6 as f32) > vector.x + 16f32
                || position.y + height as f32 - (num6 as f32) < vector.y - 0.5f32
                || position.y + (num6 as f32) > vector.y + num5 as f32 + 0.5f32
                || !can_tile_hurt(g, tile.type_, i, j, player)
            {
                continue;
            }
            if tile.slope() > 0 {
                if num6 > 0 {
                    continue;
                }
                let mut num7 = 0;
                if tile.right_slope() && position.x > vector.x {
                    num7 += 1;
                }
                if tile.left_slope() && position.x + (width as f32) < vector.x + 16f32 {
                    num7 += 1;
                }
                if tile.bottom_slope() && position.y > vector.y {
                    num7 += 1;
                }
                if tile.top_slope() && position.y + (height as f32) < vector.y + num5 as f32 {
                    num7 += 1;
                }
                if num7 == 2 {
                    continue;
                }
            }
            return HurtTile { type_: tile.type_ as i32, x: i, y: j };
        }
    }
    HurtTile { type_: -1, x: 0, y: 0 }
}

/// `Collision.CanTileHurt(type, i, j, Player player)` (Collision.cs:3229). `player` is `None` for `null`, else
/// `Some(player.fireWalk)`. Reads `Main.getGoodWorld` and `Main.dontStarveWorld`.
pub fn can_tile_hurt(g: &Game, type_: u16, _i: i32, _j: i32, player: Option<bool>) -> bool {
    if type_ == 230 && !g.main.get_good_world {
        return false;
    }
    if type_ == 80 && !g.main.dont_starve_world {
        return false;
    }
    if tile_id::sets::TOUCH_DAMAGE_BLEEDING[type_ as usize] || tile_id::sets::SUFFOCATE[type_ as usize] || tile_id::sets::TOUCH_DAMAGE_IMMEDIATE[type_ as usize] > 0 {
        return true;
    }
    if tile_id::sets::TOUCH_DAMAGE_HOT[type_ as usize] && (player.is_none() || !player.unwrap_or(false)) {
        return true;
    }
    false
}

/// `Collision.SwitchTiles(Entity entity, Position, Width, Height, oldPosition, objType)` (Collision.cs:3250).
/// The `entity` parameter is never read by the original and is left out.
/// Not ported: `Wiring.HitSwitch`, `NetMessage.SendData`, `WorldGen.KillTile`; each is `unported!` at its call and the
/// method goes on exactly as the original does after the call (the returned value does not depend on them).
pub fn switch_tiles(g: &mut Game, position: Vector2, width: i32, height: i32, old_position: Vector2, obj_type: i32) -> bool {
    let mut num = (position.x / 16f32) as i32 - 1;
    let mut num2 = ((position.x + width as f32) / 16f32) as i32 + 2;
    let mut num3 = (position.y / 16f32) as i32 - 1;
    let mut num4 = ((position.y + height as f32) / 16f32) as i32 + 2;
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
    let mut vector = Vector2::default();
    for i in num..num2 {
        for j in num3..num4 {
            let type_ = tile_at(g, i, j).type_ as i32;
            if !tile_at(g, i, j).active() || (type_ != 135 && type_ != 210 && type_ != 443 && type_ != 442) {
                continue;
            }
            vector.x = (i * 16) as f32;
            vector.y = (j * 16 + 12) as f32;
            let mut flag = false;
            if type_ == 442 {
                if obj_type == 4 {
                    let mut r1_start_x = 0f32;
                    let mut r1_start_y = 0f32;
                    let mut r1_width = 0f32;
                    let mut r1_height = 0f32;
                    match tile_at(g, i, j).frame_x as i32 / 22 {
                        0 => {
                            r1_start_x = (i * 16) as f32;
                            r1_start_y = (j * 16 + 16 - 10) as f32;
                            r1_width = 16f32;
                            r1_height = 10f32;
                        }
                        1 => {
                            r1_start_x = (i * 16) as f32;
                            r1_start_y = (j * 16) as f32;
                            r1_width = 16f32;
                            r1_height = 10f32;
                        }
                        2 => {
                            r1_start_x = (i * 16) as f32;
                            r1_start_y = (j * 16) as f32;
                            r1_width = 10f32;
                            r1_height = 16f32;
                        }
                        3 => {
                            r1_start_x = (i * 16 + 16 - 10) as f32;
                            r1_start_y = (j * 16) as f32;
                            r1_width = 10f32;
                            r1_height = 16f32;
                        }
                        _ => {}
                    }
                    if float_intersect(r1_start_x, r1_start_y, r1_width, r1_height, position.x, position.y, width as f32, height as f32)
                        && !float_intersect(r1_start_x, r1_start_y, r1_width, r1_height, old_position.x, old_position.y, width as f32, height as f32)
                    {
                        unported!("Wiring.HitSwitch");
                        unported!("NetMessage.SendData");
                        return true;
                    }
                }
                flag = true;
            }
            if flag
                || !(position.x + width as f32 > vector.x)
                || !(position.x < vector.x + 16f32)
                || !(position.y + height as f32 > vector.y)
                || !((position.y as f64) < (vector.y as f64) + 4.01)
                || (old_position.x + width as f32 > vector.x
                    && old_position.x < vector.x + 16f32
                    && old_position.y + height as f32 > vector.y
                    && (old_position.y as f64) < (vector.y as f64) + 16.01)
            {
                continue;
            }
            match type_ {
                210 => {
                    unported!("Wiring.HitSwitch");
                    unported!("NetMessage.SendData");
                    continue;
                }
                443 => {
                    if obj_type == 1 || obj_type == 5 {
                        unported!("Wiring.HitSwitch");
                        unported!("NetMessage.SendData");
                    }
                    continue;
                }
                _ => {}
            }
            let num5 = tile_at(g, i, j).frame_y as i32 / 18;
            let mut flag2 = true;
            if (num5 == 4 || num5 == 2 || num5 == 3 || num5 == 6 || num5 == 7) && obj_type != 5 {
                flag2 = false;
            }
            if num5 == 5 && (obj_type == 1 || obj_type == 4 || obj_type == 5) {
                flag2 = false;
            }
            if !flag2 {
                continue;
            }
            if g.main.net_mode == 1 && obj_type == 5 {
                unported!("NetMessage.SendData");
            }
            unported!("Wiring.HitSwitch");
            unported!("NetMessage.SendData");
            if num5 == 7 {
                unported!("WorldGen.KillTile");
                if g.main.net_mode == 1 {
                    unported!("NetMessage.SendData");
                }
            }
            return true;
        }
    }
    false
}

/// `Collision.StickyTiles(Position, Velocity, Width, Height)` (Collision.cs:3385).
/// `Main.rand.Next(30)` is made as in the original; `Dust.NewDust` is `unported!("Dust.NewDust")` (it would draw more
/// numbers from `Main.rand`).
pub fn sticky_tiles(g: &mut Game, position: Vector2, velocity: Vector2, width: i32, height: i32) -> Vector2 {
    let vector = position;
    let mut num = (position.x / 16f32) as i32 - 1;
    let mut num2 = ((position.x + width as f32) / 16f32) as i32 + 2;
    let mut num3 = (position.y / 16f32) as i32 - 1;
    let mut num4 = ((position.y + height as f32) / 16f32) as i32 + 2;
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
    let mut vector2 = Vector2::default();
    for i in num..num2 {
        for j in num3..num4 {
            if !tile_at(g, i, j).active() || tile_at(g, i, j).in_active() {
                continue;
            }
            if tile_at(g, i, j).type_ == 51 {
                let num5 = 0;
                vector2.x = (i * 16) as f32;
                vector2.y = (j * 16) as f32;
                if vector.x + width as f32 > vector2.x - num5 as f32 && vector.x < vector2.x + 16f32 + num5 as f32 && vector.y + height as f32 > vector2.y && (vector.y as f64) < (vector2.y as f64) + 16.01 {
                    if tile_at(g, i, j).type_ == 51 && ((velocity.x.abs() + velocity.y.abs()) as f64) > 0.7 && g.main.rand.next(30) == 0 {
                        unported!("Dust.NewDust");
                    }
                    return Vector2::new(i as f32, j as f32);
                }
            } else {
                if tile_at(g, i, j).type_ != 229 || tile_at(g, i, j).slope() != 0 {
                    continue;
                }
                let num6 = 1;
                vector2.x = (i * 16) as f32;
                vector2.y = (j * 16) as f32;
                let mut num7 = 16.01f32;
                if tile_at(g, i, j).half_brick() {
                    vector2.y += 8f32;
                    num7 -= 8f32;
                }
                if vector.x + width as f32 > vector2.x - num6 as f32 && vector.x < vector2.x + 16f32 + num6 as f32 && vector.y + height as f32 > vector2.y && vector.y < vector2.y + num7 {
                    // as in the original: this inner condition can never hold here (the tile type is 229, not 51)
                    if tile_at(g, i, j).type_ == 51 && ((velocity.x.abs() + velocity.y.abs()) as f64) > 0.7 && g.main.rand.next(30) == 0 {
                        unported!("Dust.NewDust");
                    }
                    return Vector2::new(i as f32, j as f32);
                }
            }
        }
    }
    Vector2::new(-1f32, -1f32)
}

/// `Collision.SolidTilesVersatile(startX, endX, startY, endY)` (Collision.cs:3460)
pub fn solid_tiles_versatile(g: &Game, mut start_x: i32, mut end_x: i32, mut start_y: i32, mut end_y: i32) -> bool {
    if start_x > end_x {
        std::mem::swap(&mut start_x, &mut end_x);
    }
    if start_y > end_y {
        std::mem::swap(&mut start_y, &mut end_y);
    }
    solid_tiles(g, start_x, end_x, start_y, end_y)
}

/// `Collision.SolidTiles(Vector2 position, int width, int height)` (Collision.cs:3473)
pub fn solid_tiles_at(g: &Game, position: Vector2, width: i32, height: i32) -> bool {
    solid_tiles(g, (position.x / 16f32) as i32, ((position.x + width as f32) / 16f32) as i32, (position.y / 16f32) as i32, ((position.y + height as f32) / 16f32) as i32)
}

/// `Collision.SolidTiles(int startX, int endX, int startY, int endY)` (Collision.cs:3478)
pub fn solid_tiles(g: &Game, start_x: i32, end_x: i32, start_y: i32, end_y: i32) -> bool {
    if start_x < 0 {
        return true;
    }
    if end_x >= g.main.max_tiles_x {
        return true;
    }
    if start_y < 0 {
        return true;
    }
    if end_y >= g.main.max_tiles_y - 40 {
        return true;
    }
    for i in start_x..end_x + 1 {
        for j in start_y..end_y + 1 {
            let t = tile_at(g, i, j);
            if t.active() && !t.in_active() && g.main.tile_solid[t.type_ as usize] && !g.main.tile_solid_top[t.type_ as usize] {
                return true;
            }
        }
    }
    false
}

/// `Collision.SolidTiles(Vector2 position, int width, int height, bool allowTopSurfaces)` (Collision.cs:3513)
pub fn solid_tiles_at_allow_top_surfaces(g: &Game, position: Vector2, width: i32, height: i32, allow_top_surfaces: bool) -> bool {
    solid_tiles_allow_top_surfaces(
        g,
        (position.x / 16f32) as i32,
        ((position.x + width as f32) / 16f32) as i32,
        (position.y / 16f32) as i32,
        ((position.y + height as f32) / 16f32) as i32,
        allow_top_surfaces,
    )
}

/// `Collision.SolidTiles(int startX, int endX, int startY, int endY, bool allowTopSurfaces)` (Collision.cs:3518)
pub fn solid_tiles_allow_top_surfaces(g: &Game, start_x: i32, end_x: i32, start_y: i32, end_y: i32, allow_top_surfaces: bool) -> bool {
    if start_x < 0 {
        return true;
    }
    if end_x >= g.main.max_tiles_x {
        return true;
    }
    if start_y < 0 {
        return true;
    }
    if end_y >= g.main.max_tiles_y - 40 {
        return true;
    }
    for i in start_x..end_x + 1 {
        for j in start_y..end_y + 1 {
            let tile = tile_at(g, i, j);
            if tile.active() && !tile_at(g, i, j).in_active() {
                let type_ = tile.type_;
                let mut flag = g.main.tile_solid[type_ as usize] && !g.main.tile_solid_top[type_ as usize];
                if allow_top_surfaces {
                    flag |= g.main.tile_solid_top[type_ as usize] && tile.frame_y == 0;
                }
                if flag {
                    return true;
                }
            }
        }
    }
    false
}

/// `Collision.SolidFullTiles(Vector2 pos, Vector2 size)` (Collision.cs:3563)
pub fn solid_full_tiles(g: &Game, pos: Vector2, size: Vector2) -> bool {
    let val = (pos.x / 16f32) as i32;
    let mut val2 = (pos.y / 16f32) as i32;
    let mut val3 = (((pos.x + size.x) / 16f32) as f64).ceil() as i32;
    let mut val4 = (((pos.y + size.y) / 16f32) as f64).ceil() as i32;
    let num = val.max(0);
    val2 = val2.max(0);
    val3 = val3.min(g.main.max_tiles_x - 1);
    val4 = val4.min(g.main.max_tiles_y - 1);
    for i in num..val3 {
        for j in val2..val4 {
            let tile = tile_at(g, i, j);
            if tile.nactive() && tile.block_type() == 0 && g.main.tile_solid[tile.type_ as usize] && !g.main.tile_solid_top[tile.type_ as usize] {
                return true;
            }
        }
    }
    false
}
