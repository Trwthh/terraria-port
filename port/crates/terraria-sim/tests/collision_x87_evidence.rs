//! Why a few `Terraria.Collision` families differ from the real game in the last bits (see `collision_reference.rs`).
//!
//! `TerrariaServer.exe` is an x86 program; .NET Framework's x86 JIT computes `float` expressions on the x87 unit and
//! does not round locals and temporaries to 32 bits (the harness records the proof in `refdata/collision/collision.json`,
//! `fpProbe`: `float x = a + b; return x - a;` gives 1 for a = 2^24, b = 1; SSE single precision gives 0). The port computes in
//! `f32`, like the source says, so it is one rounding more often than the game.
//!
//! This test proves the explanation on the cleanest case: `Collision.CanHitLine` has 16 results out of 20,000 that differ from
//! the game (a different `bool`, not only a different last bit) when its float locals are `f32`, and none when the same code
//! keeps `num5 .. num10` in `f64` (53-bit mantissa, never rounded to 32 bits: the x87 behaviour). The `f64` copy below is the
//! port's `can_hit_line` with exactly that change and nothing else.

mod collision_common;

use collision_common::*;
use terraria_core::xna::Vector2;
use terraria_core::Tile;
use terraria_sim::collision::utils::{MAIN_TILE_LEN_X, MAIN_TILE_LEN_Y};
use terraria_sim::collision::can_hit_line;
use terraria_sim::Game;

fn slot(g: &Game, x: i32, y: i32) -> Result<Option<Tile>, ()> {
    if x < 0 || y < 0 || x >= MAIN_TILE_LEN_X || y >= MAIN_TILE_LEN_Y {
        return Err(()); // IndexOutOfRangeException
    }
    Ok(g.main.tile.try_get(x, y))
}

fn blocks(g: &Game, t: &Tile) -> bool {
    !t.in_active() && t.active() && g.main.tile_solid[t.type_ as usize] && !g.main.tile_solid_top[t.type_ as usize]
}

/// `can_hit_line` of the port with `num5 .. num10` as `f64`.
fn can_hit_line_extended(g: &Game, position1: Vector2, width1: i32, height1: i32, position2: Vector2, width2: i32, height2: i32) -> bool {
    let mut num = ((position1.x + (width1 / 2) as f32) / 16f32) as i32;
    let mut num2 = ((position1.y + (height1 / 2) as f32) / 16f32) as i32;
    let mut num3 = ((position2.x + (width2 / 2) as f32) / 16f32) as i32;
    let mut num4 = ((position2.y + (height2 / 2) as f32) / 16f32) as i32;
    let (mx, my) = (g.main.max_tiles_x, g.main.max_tiles_y);
    if num <= 1 {
        num = 1;
    }
    if num >= mx {
        num = mx - 1;
    }
    if num3 <= 1 {
        num3 = 1;
    }
    if num3 >= mx {
        num3 = mx - 1;
    }
    if num2 <= 1 {
        num2 = 1;
    }
    if num2 >= my - 40 {
        num2 = my - 40;
    }
    if num4 <= 1 {
        num4 = 1;
    }
    if num4 >= my - 40 {
        num4 = my - 40;
    }
    let num5 = (num - num3).abs() as f64;
    let num6 = (num2 - num4).abs() as f64;
    if num5 == 0.0 && num6 == 0.0 {
        return true;
    }
    let mut num7 = 1f64;
    let mut num8 = 1f64;
    if num5 == 0.0 || num6 == 0.0 {
        if num5 == 0.0 {
            num7 = 0.0;
        }
        if num6 == 0.0 {
            num8 = 0.0;
        }
    } else if num5 > num6 {
        num7 = num5 / num6;
    } else {
        num8 = num6 / num5;
    }
    let mut num9 = 0f64;
    let mut num10 = 0f64;
    let mut num11 = 1;
    if num2 < num4 {
        num11 = 2;
    }
    let mut num12 = num5 as i32;
    let mut num13 = num6 as i32;
    let num14 = (num3 - num).signum();
    let num15 = (num4 - num2).signum();
    let mut flag = false;
    let mut flag2 = false;
    let mut run = || -> Result<bool, ()> {
        loop {
            match num11 {
                2 => {
                    num9 += num7;
                    let num17 = num9 as i32;
                    num9 -= num17 as f64;
                    for _ in 0..num17 {
                        let (Some(a), Some(_), Some(b)) = (slot(g, num, num2 - 1)?, slot(g, num, num2)?, slot(g, num, num2 + 1)?) else {
                            return Ok(false);
                        };
                        let c = slot(g, num, num2)?.ok_or(())?;
                        if blocks(g, &a) || blocks(g, &b) || blocks(g, &c) {
                            return Ok(false);
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
                    num10 -= num16 as f64;
                    for _ in 0..num16 {
                        let (Some(a), Some(_), Some(b)) = (slot(g, num - 1, num2)?, slot(g, num, num2)?, slot(g, num + 1, num2)?) else {
                            return Ok(false);
                        };
                        let c = slot(g, num, num2)?.ok_or(())?;
                        if blocks(g, &a) || blocks(g, &b) || blocks(g, &c) {
                            return Ok(false);
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
            let Some(t7) = slot(g, num, num2)? else { return Ok(false) };
            if blocks(g, &t7) {
                return Ok(false);
            }
            if flag || flag2 {
                break;
            }
        }
        Ok(true)
    };
    run().unwrap_or(false)
}

#[test]
fn can_hit_line_matches_the_game_when_its_float_locals_are_not_rounded() {
    let Some(g) = reference_game() else { return };
    let cases = read_cases("CanHitLine");
    let (mut port_wrong, mut extended_wrong) = (0, 0);
    for case in &cases {
        let mut r = R::new(&case.inputs);
        let _seed = r.u32();
        let (p1, w1, h1, p2, w2, h2) = (r.v2(), r.i32(), r.i32(), r.v2(), r.i32(), r.i32());
        let game = case.outputs[0] != 0;
        if can_hit_line(&g, p1, w1, h1, p2, w2, h2) != game {
            port_wrong += 1;
        }
        if can_hit_line_extended(&g, p1, w1, h1, p2, w2, h2) != game {
            extended_wrong += 1;
        }
    }
    eprintln!("CanHitLine over {} cases: f32 locals differ from the game in {port_wrong}, f64 locals in {extended_wrong}", cases.len());
    assert!(port_wrong > 0, "the f32 port is expected to differ from the x87 game in a few cases");
    assert_eq!(extended_wrong, 0, "with unrounded float locals the results must equal the game's");
}
