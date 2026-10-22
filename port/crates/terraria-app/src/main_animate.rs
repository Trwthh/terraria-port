//! The tile, wall and colour animation `Main.DoUpdate` runs every tick (Main.cs): `DoUpdate_AnimateCursorColors`,
//! `DoUpdate_AnimateTileGlows`, `DoUpdate_AnimateDiscoRGB`, `DoUpdate_AnimateWalls`, `AnimateTiles` (+ `AnimateTiles_WeatherVane`,
//! `AnimateTiles_CritterCages`). They advance `Main.tileFrame[]` / `tileFrameCounter[]` / `wallFrame[]` / the cage frames, which the tile
//! drawing code reads for the animated tiles (torches, fountains, banners, ...).
//!
//! The big bodies were produced by `tools/port_scripts/cs_main_anim.py` (assembled by `assemble_main_animate.py`) and then fixed by
//! hand where the output did not compile; statement order and every constant are the original's. Local variables keep the
//! decompiler's names. Client statics are in `MainDrawState` (`ctx.state`), simulation statics in `g.main`.

#![allow(unused_mut, unused_assignments, unused_variables, unused_parens, clippy::needless_late_init, clippy::unnecessary_cast, clippy::identity_op)]

use terraria_sim::unported;

use crate::draw_context::DrawContext;

/// `Math.Floor(double)` applied to a float, result back to float: `(float)Math.Floor(x)`
fn math_floor_f32(x: f32) -> f32 {
    (x as f64).floor() as f32
}

/// `Math.Sign(float)`: -1, 0 or 1 (a NaN throws in the original; the port answers 0)
fn math_sign(x: f32) -> i32 {
    if x > 0.0 {
        1
    } else if x < 0.0 {
        -1
    } else {
        0
    }
}

/// `UpdateOwlCageFrames()` (Main.cs:16807)
fn update_owl_cage_frames(ctx: &mut DrawContext) {
    for i in 0..ctx.state.cage_frames {
        // `UpdateOwlCageFrame(owlCageFrame, owlCageFrameCounter, i)` (Main.cs:16826)
        let style = i as usize;
        if ctx.state.owl_cage_frame_counter[style] % 40 != 0 || ctx.g.main.rand.next(80) == 0 {
            ctx.state.owl_cage_frame_counter[style] += 1;
        }
        if (ctx.state.owl_cage_frame_counter[style] + 1) % 40 == 39 {
            ctx.state.owl_cage_frame_counter[style] = 40 * ctx.g.main.rand.next(4);
        }
        let mut num = ctx.state.owl_cage_frame_counter[style] % 40 / 10;
        let num2 = ctx.state.owl_cage_frame_counter[style] / 40;
        let mut num3 = 0;
        match num2 {
            0 => {
                if num == 3 {
                    num = 1;
                }
                num3 = num;
            }
            1 => {
                if num == 3 {
                    num = 1;
                }
                num3 = 0;
                if num != 0 {
                    num3 = 8 - num;
                }
            }
            2 => {
                num3 = 0;
                if num != 0 {
                    num3 = 7 + num;
                }
            }
            3 => {
                num3 = 0;
                if num != 0 {
                    num3 = 11 + num;
                }
            }
            _ => {}
        }
        ctx.state.owl_cage_frame[style] = num3;
    }
}

/// `UpdateDragonflyJarFrames()` (Main.cs:16815) with `UpdateDragonflyJarFrame(frames, frameCounters, style, variation)` (Main.cs:16877)
fn update_dragonfly_jar_frames(ctx: &mut DrawContext) {
    for style in 0..ctx.state.dragonfly_jar_frame_counter.len() {
        for variation in 0..ctx.state.cage_frames as usize {
            ctx.state.dragonfly_jar_frame_counter[style][variation] += 1;
            match ctx.state.dragonfly_jar_frame[style][variation] {
                0 => {
                    if ctx.state.dragonfly_jar_frame_counter[style][variation] >= 300 && ctx.g.main.rand.next(60) == 0 {
                        ctx.state.dragonfly_jar_frame_counter[style][variation] = 0;
                        // `rand.NextFromList<int>(1, 5, 9)`: list[Next(list.Length)]
                        let list = [1, 5, 9];
                        ctx.state.dragonfly_jar_frame[style][variation] = list[ctx.g.main.rand.next(list.len() as i32) as usize];
                    }
                }
                1 | 5 | 9 => {
                    if ctx.state.dragonfly_jar_frame_counter[style][variation] >= 100 {
                        ctx.state.dragonfly_jar_frame_counter[style][variation] = 0;
                        if ctx.state.dragonfly_jar_frame[style][variation] == 1 {
                            let list = [0, 5, 5, 9, 9];
                            ctx.state.dragonfly_jar_frame[style][variation] = list[ctx.g.main.rand.next(list.len() as i32) as usize];
                        } else {
                            ctx.state.dragonfly_jar_frame[style][variation] = 1;
                        }
                    } else if ctx.state.dragonfly_jar_frame_counter[style][variation] >= 6 {
                        ctx.state.dragonfly_jar_frame_counter[style][variation] = 0;
                        ctx.state.dragonfly_jar_frame[style][variation] += 1;
                    }
                }
                2 | 3 | 4 | 6 | 7 | 8 | 10 | 11 | 12 => {
                    let num = ctx.state.dragonfly_jar_frame[style][variation] - 1;
                    let mut num2 = num % 4;
                    num2 -= 1;
                    let mut num3 = ctx.state.dragonfly_jar_frame_counter[style][variation] / 4 % 4;
                    if num3 > 2 {
                        num3 = 4 - num3;
                    }
                    let num4 = num - num2 + 1 + num3;
                    ctx.state.dragonfly_jar_frame[style][variation] = num4;
                    if ctx.state.dragonfly_jar_frame_counter[style][variation] >= 40 && ctx.g.main.rand.next(30) == 0 {
                        ctx.state.dragonfly_jar_frame_counter[style][variation] = 100;
                        ctx.state.dragonfly_jar_frame[style][variation] = num - num2 + 1 - 1;
                    }
                }
                _ => {}
            }
        }
    }
}

/// The calls `Main.DoUpdate` makes between `SkyManager.Instance.Update` and `DoUpdate_HandleInput` (Main.cs:17355-17362), in order.
/// Calls whose class is not ported are `unported!` at their place.
pub fn do_update_animate(ctx: &mut DrawContext) {
    do_update_animate_cursor_colors(ctx);
    do_update_animate_tile_glows(ctx);
    do_update_animate_disco_rgb(ctx);
    unported!("Main.DoUpdate_AnimateVisualPlayerAura");
    unported!("Main.DoUpdate_AnimateWaterfalls");
    do_update_animate_walls(ctx);
    animate_tiles(ctx);
    unported!("Main.DoUpdate_AnimateItemIcons");
}

/// `Main.Initialize_TileAndNPCData2` (Main.cs:6988): `critterCage = true; for 3600 AnimateTiles_CritterCages(); critterCage = false;`
/// spreads the cage frames over their cycle before the first frame.
pub fn initialize_critter_cages(ctx: &mut DrawContext) {
    ctx.state.critter_cage = true;
    for _i in 0..3600 {
        animate_tiles_critter_cages(ctx);
    }
    ctx.state.critter_cage = false;
}

/// `DoUpdate_AnimateCursorColors()` (Main.cs:18429)
fn do_update_animate_cursor_colors(ctx: &mut DrawContext) {
    unported!("Main.CursorColor"); // the cursor's own colour pulse: UI state
    ctx.g.main.mouse_text_color = ctx.g.main.mouse_text_color.wrapping_add(ctx.state.mouse_text_color_change as u8);
    if ctx.g.main.mouse_text_color >= 255 {
        ctx.state.mouse_text_color_change = -1;
    }
    if ctx.g.main.mouse_text_color <= 190 {
        ctx.state.mouse_text_color_change = 1;
    }
    ctx.state.master_color += ctx.state.master_color_dir as f32 * 0.05;
    if ctx.state.master_color > 1.0 {
        ctx.state.master_color = 1.0;
        ctx.state.master_color_dir = -1;
    }
    if ctx.state.master_color < 0.0 {
        ctx.state.master_color = 0.0;
        ctx.state.master_color_dir = 1;
    }
}

/// `DoUpdate_AnimateTileGlows()` (Main.cs:18454)
fn do_update_animate_tile_glows(ctx: &mut DrawContext) {
    ctx.g.main.demon_torch += ctx.state.demon_torch_dir as f32 * 0.01;
    if ctx.g.main.demon_torch > 1.0 {
        ctx.g.main.demon_torch = 1.0;
        ctx.state.demon_torch_dir = -1;
    }
    if ctx.g.main.demon_torch < 0.0 {
        ctx.g.main.demon_torch = 0.0;
        ctx.state.demon_torch_dir = 1;
    }
    ctx.state.martian_light += ctx.state.martian_light_dir as f32 * 0.015;
    if ctx.state.martian_light > 1.0 {
        ctx.state.martian_light = 1.0;
        ctx.state.martian_light_dir = -1;
    }
    if ctx.state.martian_light < 0.0 {
        ctx.state.martian_light = 0.0;
        ctx.state.martian_light_dir = 1;
    }
}

/// `DoUpdate_AnimateDiscoRGB()` (Main.cs:19807)
fn do_update_animate_disco_rgb(ctx: &mut DrawContext) {
    let num = 7;
    if ctx.state.disco_style == 0 {
        ctx.g.main.disco_g += num;
        if ctx.g.main.disco_g >= 255 {
            ctx.g.main.disco_g = 255;
            ctx.state.disco_style += 1;
        }
    }
    if ctx.state.disco_style == 1 {
        ctx.g.main.disco_r -= num;
        if ctx.g.main.disco_r <= 0 {
            ctx.g.main.disco_r = 0;
            ctx.state.disco_style += 1;
        }
    }
    if ctx.state.disco_style == 2 {
        ctx.g.main.disco_b += num;
        if ctx.g.main.disco_b >= 255 {
            ctx.g.main.disco_b = 255;
            ctx.state.disco_style += 1;
        }
    }
    if ctx.state.disco_style == 3 {
        ctx.g.main.disco_g -= num;
        if ctx.g.main.disco_g <= 0 {
            ctx.g.main.disco_g = 0;
            ctx.state.disco_style += 1;
        }
    }
    if ctx.state.disco_style == 4 {
        ctx.g.main.disco_r += num;
        if ctx.g.main.disco_r >= 255 {
            ctx.g.main.disco_r = 255;
            ctx.state.disco_style += 1;
        }
    }
    if ctx.state.disco_style == 5 {
        ctx.g.main.disco_b -= num;
        if ctx.g.main.disco_b <= 0 {
            ctx.g.main.disco_b = 0;
            ctx.state.disco_style = 0;
        }
    }
}

/// `DoUpdate_AnimateWalls()` (Main.cs:19547)
fn do_update_animate_walls(ctx: &mut DrawContext) {
    ctx.state.wall_frame_counter[(357) as usize] += 1;
    if (ctx.state.wall_frame_counter[357] as i32) >= 5 {
        ctx.state.wall_frame_counter[(357) as usize] = (0) as u8;
        ctx.state.wall_frame[(357) as usize] += 1;
        if (ctx.state.wall_frame[357] as i32) >= 8 {
            ctx.state.wall_frame[(357) as usize] = (0) as u8;
        }
    }
    ctx.state.wall_frame_counter[(365) as usize] += 1;
    if (ctx.state.wall_frame_counter[365] as i32) >= 8 {
        ctx.state.wall_frame_counter[(365) as usize] = (0) as u8;
        ctx.state.wall_frame[(365) as usize] += 1;
        if (ctx.state.wall_frame[365] as i32) >= 8 {
            ctx.state.wall_frame[(365) as usize] = (0) as u8;
        }
    }
    ctx.state.wall_frame_counter[(136) as usize] += 1;
    if (ctx.state.wall_frame_counter[136] as i32) >= 5 {
        ctx.state.wall_frame_counter[(136) as usize] = (0) as u8;
        ctx.state.wall_frame[(136) as usize] += 1;
        if (ctx.state.wall_frame[136] as i32) > 7 {
            ctx.state.wall_frame[(136) as usize] = (0) as u8;
        }
    }
    ctx.state.wall_frame_counter[(137) as usize] += 1;
    if (ctx.state.wall_frame_counter[137] as i32) >= 10 {
        ctx.state.wall_frame_counter[(137) as usize] = (0) as u8;
        ctx.state.wall_frame[(137) as usize] += 1;
        if (ctx.state.wall_frame[137] as i32) > 7 {
            ctx.state.wall_frame[(137) as usize] = (0) as u8;
        }
    }
    let mut num: i32 = 226;
    ctx.state.wall_frame_counter[(num) as usize] += 1;
    if (ctx.state.wall_frame_counter[(num) as usize] as i32) >= 10 {
        ctx.state.wall_frame_counter[(num) as usize] = (0) as u8;
        ctx.state.wall_frame[(num) as usize] += 1;
        if (ctx.state.wall_frame[(num) as usize] as i32) > 7 {
            ctx.state.wall_frame[(num) as usize] = (0) as u8;
        }
    }
    num = 227;
    ctx.state.wall_frame_counter[(num) as usize] += 1;
    if (ctx.state.wall_frame_counter[(num) as usize] as i32) >= 5 {
        ctx.state.wall_frame_counter[(num) as usize] = (0) as u8;
        ctx.state.wall_frame[(num) as usize] += 1;
        if (ctx.state.wall_frame[(num) as usize] as i32) > 7 {
            ctx.state.wall_frame[(num) as usize] = (0) as u8;
        }
    }
    num = 225;
    ctx.state.wall_frame_counter[(num) as usize] += 1;
    if (ctx.state.wall_frame_counter[(num) as usize] as i32) >= 5 {
        ctx.state.wall_frame_counter[(num) as usize] = (0) as u8;
        ctx.state.wall_frame[(num) as usize] += 1;
        if (ctx.state.wall_frame[(num) as usize] as i32) > 1 {
            ctx.state.wall_frame[(num) as usize] = (0) as u8;
        }
    }
    ctx.state.wall_frame_counter[(172) as usize] += 1;
    if (ctx.state.wall_frame_counter[172] as i32) >= 10 {
        ctx.state.wall_frame_counter[(172) as usize] = (0) as u8;
        ctx.state.wall_frame[(172) as usize] += 1;
        if (ctx.state.wall_frame[172] as i32) > 7 {
            ctx.state.wall_frame[(172) as usize] = (0) as u8;
        }
    }
    ctx.state.wall_frame_counter[(347) as usize] += 1;
    if (ctx.state.wall_frame_counter[347] as i32) >= 10 {
        ctx.state.wall_frame_counter[(347) as usize] = (0) as u8;
        ctx.state.wall_frame[(347) as usize] += 1;
        if (ctx.state.wall_frame[347] as i32) > 7 {
            ctx.state.wall_frame[(347) as usize] = (0) as u8;
        }
    }
    ctx.state.wall_frame_counter[(168) as usize] += 1;
    if (ctx.state.wall_frame_counter[168] as i32) >= 5 {
        ctx.state.wall_frame_counter[(168) as usize] = (0) as u8;
        ctx.state.wall_frame[(168) as usize] += 1;
        if (ctx.state.wall_frame[168] as i32) > 7 {
            ctx.state.wall_frame[(168) as usize] = (0) as u8;
        }
    }
    ctx.state.wall_frame_counter[(169) as usize] += 1;
    if (ctx.state.wall_frame_counter[169] as i32) >= 5 {
        ctx.state.wall_frame_counter[(169) as usize] = (0) as u8;
        ctx.state.wall_frame[(169) as usize] += 1;
        if (ctx.state.wall_frame[169] as i32) > 7 {
            ctx.state.wall_frame[(169) as usize] = (0) as u8;
        }
    }
    let mut num2: i32 = 20;
    if { ctx.state.wall_frame_counter[242] += 1; ctx.state.wall_frame_counter[242] as i32 } >= num2 * 8 {
        ctx.state.wall_frame_counter[(242) as usize] = (0) as u8;
    }
    if { ctx.state.wall_frame_counter[243] += 1; ctx.state.wall_frame_counter[243] as i32 } >= num2 * 8 {
        ctx.state.wall_frame_counter[(243) as usize] = (0) as u8;
    }
    ctx.state.wall_frame_counter[(144) as usize] += 1;
    let mut num3: i32 = 5;
    let mut num4: i32 = 10;
    if (ctx.state.wall_frame_counter[144] as i32) < num3 {
        ctx.state.wall_frame[(144) as usize] = (0) as u8;
        return;
    }
    if (ctx.state.wall_frame_counter[144] as i32) < num3 {
        ctx.state.wall_frame[(144) as usize] = (1) as u8;
        return;
    }
    if (ctx.state.wall_frame_counter[144] as i32) < num3 * 2 {
        ctx.state.wall_frame[(144) as usize] = (2) as u8;
        return;
    }
    if (ctx.state.wall_frame_counter[144] as i32) < num3 * 3 {
        ctx.state.wall_frame[(144) as usize] = (3) as u8;
        return;
    }
    if (ctx.state.wall_frame_counter[144] as i32) < num3 * 4 {
        ctx.state.wall_frame[(144) as usize] = (4) as u8;
        return;
    }
    if (ctx.state.wall_frame_counter[144] as i32) < num3 * 5 {
        ctx.state.wall_frame[(144) as usize] = (5) as u8;
        return;
    }
    if (ctx.state.wall_frame_counter[144] as i32) < num3 * 6 {
        ctx.state.wall_frame[(144) as usize] = (6) as u8;
        return;
    }
    if (ctx.state.wall_frame_counter[144] as i32) < num3 * 7 {
        ctx.state.wall_frame[(144) as usize] = (7) as u8;
        return;
    }
    if (ctx.state.wall_frame_counter[144] as i32) < num3 * (8 + num4) {
        ctx.state.wall_frame[(144) as usize] = (8) as u8;
        return;
    }
    if (ctx.state.wall_frame_counter[144] as i32) < num3 * (9 + num4) {
        ctx.state.wall_frame[(144) as usize] = (7) as u8;
        return;
    }
    if (ctx.state.wall_frame_counter[144] as i32) < num3 * (10 + num4) {
        ctx.state.wall_frame[(144) as usize] = (6) as u8;
        return;
    }
    if (ctx.state.wall_frame_counter[144] as i32) < num3 * (11 + num4) {
        ctx.state.wall_frame[(144) as usize] = (5) as u8;
        return;
    }
    if (ctx.state.wall_frame_counter[144] as i32) < num3 * (12 + num4) {
        ctx.state.wall_frame[(144) as usize] = (4) as u8;
        return;
    }
    if (ctx.state.wall_frame_counter[144] as i32) < num3 * (13 + num4) {
        ctx.state.wall_frame[(144) as usize] = (3) as u8;
        return;
    }
    if (ctx.state.wall_frame_counter[144] as i32) < num3 * (14 + num4) {
        ctx.state.wall_frame[(144) as usize] = (2) as u8;
        return;
    }
    if (ctx.state.wall_frame_counter[144] as i32) < num3 * (15 + num4) {
        ctx.state.wall_frame[(144) as usize] = (1) as u8;
        return;
    }
    ctx.state.wall_frame[(144) as usize] = (0) as u8;
    if (ctx.state.wall_frame_counter[144] as i32) > num3 * (16 + num4 * 2) {
        ctx.state.wall_frame_counter[(144) as usize] = (0) as u8;
    }
}

/// `AnimateTiles()` (Main.cs:18732)
fn animate_tiles(ctx: &mut DrawContext) {
    ctx.state.tile_frame_counter[(12) as usize] += 1;
    if ctx.state.tile_frame_counter[12] > 5 {
        ctx.state.tile_frame_counter[(12) as usize] = 0;
        ctx.state.tile_frame[(12) as usize] += 1;
        if ctx.state.tile_frame[12] >= 10 {
            ctx.state.tile_frame[(12) as usize] = 0;
        }
    }
    ctx.state.tile_frame_counter[(665) as usize] = ctx.state.tile_frame_counter[12];
    ctx.state.tile_frame[(665) as usize] = ctx.state.tile_frame[12];
    ctx.state.tile_frame_counter[(639) as usize] += 1;
    if ctx.state.tile_frame_counter[639] > 5 {
        ctx.state.tile_frame_counter[(639) as usize] = 0;
        ctx.state.tile_frame[(639) as usize] += 1;
        if ctx.state.tile_frame[639] >= 10 {
            ctx.state.tile_frame[(639) as usize] = 0;
        }
    }
    ctx.state.tile_frame_counter[(739) as usize] += 1;
    if ctx.state.tile_frame_counter[739] > 10 {
        ctx.state.tile_frame_counter[(739) as usize] = 0;
        ctx.state.tile_frame[(739) as usize] += 1;
        if ctx.state.tile_frame[739] >= 4 {
            ctx.state.tile_frame[(739) as usize] = 0;
        }
    }
    ctx.state.tile_frame_counter[(748) as usize] += 1;
    if ctx.state.tile_frame_counter[748] > 7 {
        ctx.state.tile_frame_counter[(748) as usize] = 0;
        ctx.state.tile_frame[(748) as usize] += 1;
        if ctx.state.tile_frame[748] >= 8 {
            ctx.state.tile_frame[(748) as usize] = 0;
        }
    }
    ctx.state.tile_frame_counter[(17) as usize] += 1;
    if ctx.state.tile_frame_counter[17] > 5 {
        ctx.state.tile_frame_counter[(17) as usize] = 0;
        ctx.state.tile_frame[(17) as usize] += 1;
        if ctx.state.tile_frame[17] >= 12 {
            ctx.state.tile_frame[(17) as usize] = 0;
        }
    }
    if { ctx.state.tile_frame_counter[133] += 1; ctx.state.tile_frame_counter[133] } >= 4 {
        ctx.state.tile_frame_counter[(133) as usize] = 0;
        if { ctx.state.tile_frame[133] += 1; ctx.state.tile_frame[133] } >= 6 {
            ctx.state.tile_frame[(133) as usize] = 0;
        }
    }
    ctx.state.tile_frame_counter[(31) as usize] += 1;
    if ctx.state.tile_frame_counter[31] >= 8 {
        ctx.state.tile_frame_counter[(31) as usize] = 0;
        ctx.state.tile_frame[(31) as usize] += 1;
        if ctx.state.tile_frame[31] >= 4 {
            ctx.state.tile_frame[(31) as usize] = 0;
        }
    }
    ctx.state.tile_frame[(696) as usize] = ctx.state.tile_frame[31];
    ctx.state.tile_frame_counter[(77) as usize] += 1;
    if ctx.state.tile_frame_counter[77] > 5 {
        ctx.state.tile_frame_counter[(77) as usize] = 0;
        ctx.state.tile_frame[(77) as usize] += 1;
        if ctx.state.tile_frame[77] >= 12 {
            ctx.state.tile_frame[(77) as usize] = 0;
        }
    }
    ctx.state.tile_frame_counter[(106) as usize] += 1;
    if ctx.state.tile_frame_counter[106] > 4 {
        ctx.state.tile_frame_counter[(106) as usize] = 0;
        ctx.state.tile_frame[(106) as usize] += 1;
        if ctx.state.tile_frame[106] >= 2 {
            ctx.state.tile_frame[(106) as usize] = 0;
        }
    }
    ctx.state.tile_frame_counter[(207) as usize] += 1;
    if ctx.state.tile_frame_counter[207] > 4 {
        ctx.state.tile_frame_counter[(207) as usize] = 0;
        ctx.state.tile_frame[(207) as usize] += 1;
        if ctx.state.tile_frame[207] >= 6 {
            ctx.state.tile_frame[(207) as usize] = 0;
        }
    }
    ctx.state.tile_frame_counter[(215) as usize] += 1;
    if ctx.state.tile_frame_counter[215] >= 4 {
        ctx.state.tile_frame_counter[(215) as usize] = 0;
        ctx.state.tile_frame[(215) as usize] += 1;
        if ctx.state.tile_frame[215] >= 8 {
            ctx.state.tile_frame[(215) as usize] = 0;
        }
    }
    ctx.state.tile_frame_counter[(592) as usize] += 1;
    if ctx.state.tile_frame_counter[592] >= 5 {
        ctx.state.tile_frame_counter[(592) as usize] = 0;
        ctx.state.tile_frame[(592) as usize] += 1;
        if ctx.state.tile_frame[592] >= 8 {
            ctx.state.tile_frame[(592) as usize] = 0;
        }
    }
    ctx.state.tile_frame_counter[(217) as usize] += 1;
    if ctx.state.tile_frame_counter[217] > 4 {
        ctx.state.tile_frame_counter[(217) as usize] = 0;
        ctx.state.tile_frame[(217) as usize] += 1;
        if ctx.state.tile_frame[217] >= 5 {
            ctx.state.tile_frame[(217) as usize] = 0;
        }
    }
    ctx.state.tile_frame_counter[(218) as usize] += 1;
    if ctx.state.tile_frame_counter[218] > 4 {
        ctx.state.tile_frame_counter[(218) as usize] = 0;
        ctx.state.tile_frame[(218) as usize] += 1;
        if ctx.state.tile_frame[218] >= 2 {
            ctx.state.tile_frame[(218) as usize] = 0;
        }
    }
    ctx.state.tile_frame_counter[(219) as usize] += 1;
    if ctx.state.tile_frame_counter[219] > 4 {
        ctx.state.tile_frame_counter[(219) as usize] = 0;
        ctx.state.tile_frame[(219) as usize] += 1;
        if ctx.state.tile_frame[219] >= 10 {
            ctx.state.tile_frame[(219) as usize] = 0;
        }
    }
    ctx.state.tile_frame_counter[(642) as usize] += 1;
    if ctx.state.tile_frame_counter[642] > 4 {
        ctx.state.tile_frame_counter[(642) as usize] = 0;
        ctx.state.tile_frame[(642) as usize] += 1;
        if ctx.state.tile_frame[642] >= 6 {
            ctx.state.tile_frame[(642) as usize] = 0;
        }
    }
    ctx.state.tile_frame_counter[(220) as usize] += 1;
    if ctx.state.tile_frame_counter[220] > 4 {
        ctx.state.tile_frame_counter[(220) as usize] = 0;
        ctx.state.tile_frame[(220) as usize] += 1;
        if ctx.state.tile_frame[220] >= 4 {
            ctx.state.tile_frame[(220) as usize] = 0;
        }
    }
    ctx.state.tile_frame_counter[(231) as usize] += 1;
    if ctx.state.tile_frame_counter[231] > 16 {
        ctx.state.tile_frame_counter[(231) as usize] = 0;
        ctx.state.tile_frame[(231) as usize] += 1;
        if ctx.state.tile_frame[231] >= 7 {
            ctx.state.tile_frame[(231) as usize] = 0;
        }
    }
    ctx.state.tile_frame_counter[(235) as usize] += 1;
    if ctx.state.tile_frame_counter[235] > 20 {
        ctx.state.tile_frame_counter[(235) as usize] = 0;
        ctx.state.tile_frame[(235) as usize] += 1;
        if ctx.state.tile_frame[235] >= 4 {
            ctx.state.tile_frame[(235) as usize] = 0;
        }
    }
    ctx.state.tile_frame_counter[(238) as usize] += 1;
    if ctx.state.tile_frame_counter[238] > 20 {
        ctx.state.tile_frame_counter[(238) as usize] = 0;
        ctx.state.tile_frame[(238) as usize] += 1;
        if ctx.state.tile_frame[238] >= 4 {
            ctx.state.tile_frame[(238) as usize] = 0;
        }
    }
    ctx.state.tile_frame_counter[(243) as usize] += 1;
    if ctx.state.tile_frame_counter[243] > 4 {
        ctx.state.tile_frame_counter[(243) as usize] = 0;
        ctx.state.tile_frame[(243) as usize] += 1;
        if ctx.state.tile_frame[243] >= 6 {
            ctx.state.tile_frame[(243) as usize] = 0;
        }
    }
    ctx.state.tile_frame_counter[(244) as usize] += 1;
    if ctx.state.tile_frame_counter[244] > 4 {
        ctx.state.tile_frame_counter[(244) as usize] = 0;
        ctx.state.tile_frame[(244) as usize] += 1;
        if ctx.state.tile_frame[244] >= 6 {
            ctx.state.tile_frame[(244) as usize] = 0;
        }
    }
    ctx.state.tile_frame_counter[(247) as usize] += 1;
    if ctx.state.tile_frame_counter[247] > 4 {
        ctx.state.tile_frame_counter[(247) as usize] = 0;
        ctx.state.tile_frame[(247) as usize] += 1;
        if ctx.state.tile_frame[247] > 7 {
            ctx.state.tile_frame[(247) as usize] = 0;
        }
    }
    ctx.state.tile_frame_counter[(96) as usize] += 1;
    if ctx.state.tile_frame_counter[96] > 4 {
        ctx.state.tile_frame_counter[(96) as usize] = 0;
        ctx.state.tile_frame[(96) as usize] += 1;
        if ctx.state.tile_frame[96] > 3 {
            ctx.state.tile_frame[(96) as usize] = 0;
        }
    }
    ctx.state.tile_frame_counter[(171) as usize] += 1;
    if ctx.state.tile_frame_counter[171] > 16 {
        ctx.state.tile_frame_counter[(171) as usize] = 0;
        ctx.state.tile_frame[(171) as usize] += 1;
        if ctx.state.tile_frame[171] > 3 {
            ctx.state.tile_frame[(171) as usize] = 0;
        }
    }
    ctx.state.tile_frame_counter[(270) as usize] += 1;
    if ctx.state.tile_frame_counter[270] > 8 {
        ctx.state.tile_frame_counter[(270) as usize] = 0;
        ctx.state.tile_frame[(270) as usize] += 1;
        if ctx.state.tile_frame[270] > 5 {
            ctx.state.tile_frame[(270) as usize] = 0;
        }
    }
    let mut num: i32 = ctx.state.tile_frame[270];
    ctx.state.tile_frame[(271) as usize] = num;
    ctx.state.tile_frame[(581) as usize] = num;
    ctx.state.tile_frame_counter[(660) as usize] += 1;
    if ctx.state.tile_frame_counter[660] > 8 {
        ctx.state.tile_frame_counter[(660) as usize] = 0;
        ctx.state.tile_frame[(660) as usize] += 1;
        if ctx.state.tile_frame[660] > 4 {
            ctx.state.tile_frame[(660) as usize] = 0;
        }
    }
    ctx.state.tile_frame_counter[(272) as usize] += 1;
    if ctx.state.tile_frame_counter[272] >= 10 {
        ctx.state.tile_frame_counter[(272) as usize] = 0;
        ctx.state.tile_frame[(272) as usize] += 1;
        if ctx.state.tile_frame[272] > 1 {
            ctx.state.tile_frame[(272) as usize] = 0;
        }
    }
    ctx.state.tile_frame_counter[(300) as usize] += 1;
    if ctx.state.tile_frame_counter[300] >= 5 {
        ctx.state.tile_frame_counter[(300) as usize] = 0;
        ctx.state.tile_frame[(300) as usize] += 1;
        if ctx.state.tile_frame[300] > 6 {
            ctx.state.tile_frame[(300) as usize] = 0;
        }
    }
    ctx.state.tile_frame_counter[(301) as usize] += 1;
    if ctx.state.tile_frame_counter[301] >= 5 {
        ctx.state.tile_frame_counter[(301) as usize] = 0;
        ctx.state.tile_frame[(301) as usize] += 1;
        if ctx.state.tile_frame[301] > 7 {
            ctx.state.tile_frame[(301) as usize] = 0;
        }
    }
    ctx.state.tile_frame_counter[(302) as usize] += 1;
    if ctx.state.tile_frame_counter[302] >= 5 {
        ctx.state.tile_frame_counter[(302) as usize] = 0;
        ctx.state.tile_frame[(302) as usize] += 1;
        if ctx.state.tile_frame[302] > 3 {
            ctx.state.tile_frame[(302) as usize] = 0;
        }
    }
    ctx.state.tile_frame_counter[(303) as usize] += 1;
    if ctx.state.tile_frame_counter[303] >= 5 {
        ctx.state.tile_frame_counter[(303) as usize] = 0;
        ctx.state.tile_frame[(303) as usize] += 1;
        if ctx.state.tile_frame[303] > 4 {
            ctx.state.tile_frame[(303) as usize] = 0;
        }
    }
    ctx.state.tile_frame_counter[(305) as usize] += 1;
    if ctx.state.tile_frame_counter[305] >= 5 {
        ctx.state.tile_frame_counter[(305) as usize] = 0;
        ctx.state.tile_frame[(305) as usize] += 1;
        if ctx.state.tile_frame[305] > 11 {
            ctx.state.tile_frame[(305) as usize] = 0;
        }
    }
    ctx.state.tile_frame_counter[(306) as usize] += 1;
    if ctx.state.tile_frame_counter[306] >= 5 {
        ctx.state.tile_frame_counter[(306) as usize] = 0;
        ctx.state.tile_frame[(306) as usize] += 1;
        if ctx.state.tile_frame[306] > 11 {
            ctx.state.tile_frame[(306) as usize] = 0;
        }
    }
    ctx.state.tile_frame_counter[(307) as usize] += 1;
    if ctx.state.tile_frame_counter[307] >= 5 {
        ctx.state.tile_frame_counter[(307) as usize] = 0;
        ctx.state.tile_frame[(307) as usize] += 1;
        if ctx.state.tile_frame[307] > 1 {
            ctx.state.tile_frame[(307) as usize] = 0;
        }
    }
    ctx.state.tile_frame_counter[(308) as usize] += 1;
    if ctx.state.tile_frame_counter[308] >= 5 {
        ctx.state.tile_frame_counter[(308) as usize] = 0;
        ctx.state.tile_frame[(308) as usize] += 1;
        if ctx.state.tile_frame[308] > 7 {
            ctx.state.tile_frame[(308) as usize] = 0;
        }
    }
    ctx.state.tile_frame_counter[(314) as usize] += 1;
    if ctx.state.tile_frame_counter[314] >= 10 {
        ctx.state.tile_frame_counter[(314) as usize] = 0;
        ctx.state.tile_frame[(314) as usize] += 1;
        if ctx.state.tile_frame[314] > 4 {
            ctx.state.tile_frame[(314) as usize] = 0;
        }
    }
    ctx.state.tile_frame_counter[(326) as usize] += 1;
    if ctx.state.tile_frame_counter[326] >= 5 {
        ctx.state.tile_frame_counter[(326) as usize] = 0;
        ctx.state.tile_frame[(326) as usize] += 1;
        if ctx.state.tile_frame[326] > 7 {
            ctx.state.tile_frame[(326) as usize] = 0;
        }
    }
    ctx.state.tile_frame_counter[(327) as usize] += 1;
    if ctx.state.tile_frame_counter[327] >= 10 {
        ctx.state.tile_frame_counter[(327) as usize] = 0;
        ctx.state.tile_frame[(327) as usize] += 1;
        if ctx.state.tile_frame[327] > 7 {
            ctx.state.tile_frame[(327) as usize] = 0;
        }
    }
    ctx.state.tile_frame_counter[(345) as usize] += 1;
    if ctx.state.tile_frame_counter[345] >= 10 {
        ctx.state.tile_frame_counter[(345) as usize] = 0;
        ctx.state.tile_frame[(345) as usize] += 1;
        if ctx.state.tile_frame[345] > 7 {
            ctx.state.tile_frame[(345) as usize] = 0;
        }
    }
    ctx.state.tile_frame_counter[(458) as usize] += 1;
    if ctx.state.tile_frame_counter[458] >= 10 {
        ctx.state.tile_frame_counter[(458) as usize] = 0;
        ctx.state.tile_frame[(458) as usize] += 1;
        if ctx.state.tile_frame[458] > 7 {
            ctx.state.tile_frame[(458) as usize] = 0;
        }
    }
    ctx.state.tile_frame_counter[(459) as usize] += 1;
    if ctx.state.tile_frame_counter[459] >= 10 {
        ctx.state.tile_frame_counter[(459) as usize] = 0;
        ctx.state.tile_frame[(459) as usize] += 1;
        if ctx.state.tile_frame[459] > 7 {
            ctx.state.tile_frame[(459) as usize] = 0;
        }
    }
    ctx.state.tile_frame_counter[(708) as usize] += 1;
    if ctx.state.tile_frame_counter[708] >= 10 {
        ctx.state.tile_frame_counter[(708) as usize] = 0;
        ctx.state.tile_frame[(708) as usize] += 1;
        if ctx.state.tile_frame[708] > 7 {
            ctx.state.tile_frame[(708) as usize] = 0;
        }
    }
    ctx.state.tile_frame_counter[(336) as usize] += 1;
    if ctx.state.tile_frame_counter[336] >= 5 {
        ctx.state.tile_frame_counter[(336) as usize] = 0;
        ctx.state.tile_frame[(336) as usize] += 1;
        if ctx.state.tile_frame[336] > 3 {
            ctx.state.tile_frame[(336) as usize] = 0;
        }
    }
    ctx.state.tile_frame_counter[(328) as usize] += 1;
    if ctx.state.tile_frame_counter[328] >= 5 {
        ctx.state.tile_frame_counter[(328) as usize] = 0;
        ctx.state.tile_frame[(328) as usize] += 1;
        if ctx.state.tile_frame[328] > 7 {
            ctx.state.tile_frame[(328) as usize] = 0;
        }
    }
    ctx.state.tile_frame_counter[(329) as usize] += 1;
    if ctx.state.tile_frame_counter[329] >= 5 {
        ctx.state.tile_frame_counter[(329) as usize] = 0;
        ctx.state.tile_frame[(329) as usize] += 1;
        if ctx.state.tile_frame[329] > 7 {
            ctx.state.tile_frame[(329) as usize] = 0;
        }
    }
    let mut num2: i32 = 20;
    if { ctx.state.tile_frame_counter[507] += 1; ctx.state.tile_frame_counter[507] } >= num2 * 8 {
        ctx.state.tile_frame_counter[(507) as usize] = 0;
    }
    if { ctx.state.tile_frame_counter[508] += 1; ctx.state.tile_frame_counter[508] } >= num2 * 8 {
        ctx.state.tile_frame_counter[(508) as usize] = 0;
    }
    for i in 340..=344 {
        ctx.state.tile_frame_counter[(i) as usize] += 1;
        if ctx.state.tile_frame_counter[(i) as usize] >= 5 {
            ctx.state.tile_frame_counter[(i) as usize] = 0;
            ctx.state.tile_frame[(i) as usize] += 1;
            if ctx.state.tile_frame[(i) as usize] > 3 {
                ctx.state.tile_frame[(i) as usize] = 0;
            }
        }
    }
    ctx.state.tile_frame_counter[(351) as usize] += 1;
    if ctx.state.tile_frame_counter[351] >= 5 {
        ctx.state.tile_frame_counter[(351) as usize] = 0;
        ctx.state.tile_frame[(351) as usize] += 1;
        if ctx.state.tile_frame[351] > 2 {
            ctx.state.tile_frame[(351) as usize] = 0;
        }
    }
    ctx.state.tile_frame_counter[(354) as usize] += 1;
    if ctx.state.tile_frame_counter[354] >= 5 {
        ctx.state.tile_frame_counter[(354) as usize] = 0;
        ctx.state.tile_frame[(354) as usize] += 1;
        if ctx.state.tile_frame[354] >= 8 {
            ctx.state.tile_frame[(354) as usize] = 0;
        }
    }
    ctx.state.tile_frame[(355) as usize] = ctx.state.tile_frame[354];
    ctx.state.tile_frame_counter[(377) as usize] += 1;
    if ctx.state.tile_frame_counter[377] >= 5 {
        ctx.state.tile_frame_counter[(377) as usize] = 0;
        ctx.state.tile_frame[(377) as usize] += 1;
        if ctx.state.tile_frame[377] >= 4 {
            ctx.state.tile_frame[(377) as usize] = 0;
        }
    }
    ctx.state.tile_frame_counter[(379) as usize] += 1;
    if ctx.state.tile_frame_counter[379] >= 10 {
        ctx.state.tile_frame_counter[(379) as usize] = 0;
        ctx.state.tile_frame[(379) as usize] += 1;
        if ctx.state.tile_frame[379] >= 4 {
            ctx.state.tile_frame[(379) as usize] = 0;
        }
    }
    if { ctx.state.tile_frame_counter[390] += 1; ctx.state.tile_frame_counter[390] } >= 8 {
        ctx.state.tile_frame_counter[(390) as usize] = 0;
        if { ctx.state.tile_frame[390] += 1; ctx.state.tile_frame[390] } >= 7 {
            ctx.state.tile_frame[(390) as usize] = 0;
        }
    }
    if { ctx.state.tile_frame_counter[228] += 1; ctx.state.tile_frame_counter[228] } >= 5 {
        ctx.state.tile_frame_counter[(228) as usize] = 0;
        if { ctx.state.tile_frame[228] += 1; ctx.state.tile_frame[228] } >= 3 {
            ctx.state.tile_frame[(228) as usize] = 0;
        }
    }
    if { ctx.state.tile_frame_counter[405] += 1; ctx.state.tile_frame_counter[405] } >= 5 {
        ctx.state.tile_frame_counter[(405) as usize] = 0;
        if { ctx.state.tile_frame[405] += 1; ctx.state.tile_frame[405] } >= 8 {
            ctx.state.tile_frame[(405) as usize] = 0;
        }
    }
    if { ctx.state.tile_frame_counter[406] += 1; ctx.state.tile_frame_counter[406] } >= 8 {
        ctx.state.tile_frame_counter[(406) as usize] = 0;
        if { ctx.state.tile_frame[406] += 1; ctx.state.tile_frame[406] } >= 6 {
            ctx.state.tile_frame[(406) as usize] = 0;
        }
    }
    if { ctx.state.tile_frame_counter[452] += 1; ctx.state.tile_frame_counter[452] } >= 5 {
        ctx.state.tile_frame_counter[(452) as usize] = 0;
        if { ctx.state.tile_frame[452] += 1; ctx.state.tile_frame[452] } >= 15 {
            ctx.state.tile_frame[(452) as usize] = 0;
        }
    }
    if { ctx.state.tile_frame_counter[455] += 1; ctx.state.tile_frame_counter[455] } >= 5 {
        ctx.state.tile_frame_counter[(455) as usize] = 0;
        if { ctx.state.tile_frame[455] += 1; ctx.state.tile_frame[455] } >= 6 {
            ctx.state.tile_frame[(455) as usize] = 0;
        }
    }
    if { ctx.state.tile_frame_counter[499] += 1; ctx.state.tile_frame_counter[499] } >= 5 {
        ctx.state.tile_frame_counter[(499) as usize] = 0;
        if { ctx.state.tile_frame[499] += 1; ctx.state.tile_frame[499] } >= 8 {
            ctx.state.tile_frame[(499) as usize] = 0;
        }
    }
    if { ctx.state.tile_frame_counter[129] += 1; ctx.state.tile_frame_counter[129] } >= 8 {
        ctx.state.tile_frame_counter[(129) as usize] = 0;
        if { ctx.state.tile_frame[129] += 1; ctx.state.tile_frame[129] } >= 6 {
            ctx.state.tile_frame[(129) as usize] = 0;
        }
    }
    ctx.state.tile_frame_counter[(453) as usize] += (if !ctx.g.world_gen.generating_world { ctx.g.main.rand.next(3) } else { 0 });
    if { ctx.state.tile_frame_counter[453] += 1; ctx.state.tile_frame_counter[453] } >= 60 {
        ctx.state.tile_frame_counter[(453) as usize] = 0;
    }
    if { ctx.state.tile_frame[412] += 1; ctx.state.tile_frame[412] } >= 240 {
        ctx.state.tile_frame[(412) as usize] = 0;
    }
    ctx.state.tile_frame_counter[(456) as usize] += (if !ctx.g.world_gen.generating_world { ctx.g.main.rand.next(3) } else { 0 });
    if { ctx.state.tile_frame_counter[456] += 1; ctx.state.tile_frame_counter[456] } >= 80 {
        ctx.state.tile_frame_counter[(456) as usize] = 0;
    }
    if { ctx.state.tile_frame[456] += 1; ctx.state.tile_frame[456] } >= 240 {
        ctx.state.tile_frame[(456) as usize] = 0;
    }
    if { ctx.state.tile_frame_counter[410] += 1; ctx.state.tile_frame_counter[410] } >= 8 {
        ctx.state.tile_frame_counter[(410) as usize] = 0;
        if { ctx.state.tile_frame[410] += 1; ctx.state.tile_frame[410] } >= 8 {
            ctx.state.tile_frame[(410) as usize] = 0;
        }
    }
    if { ctx.state.tile_frame_counter[480] += 1; ctx.state.tile_frame_counter[480] } >= 8 {
        ctx.state.tile_frame_counter[(480) as usize] = 0;
        if { ctx.state.tile_frame[480] += 1; ctx.state.tile_frame[480] } >= 8 {
            ctx.state.tile_frame[(480) as usize] = 0;
        }
    }
    if { ctx.state.tile_frame_counter[509] += 1; ctx.state.tile_frame_counter[509] } >= 8 {
        ctx.state.tile_frame_counter[(509) as usize] = 0;
        if { ctx.state.tile_frame[509] += 1; ctx.state.tile_frame[509] } >= 8 {
            ctx.state.tile_frame[(509) as usize] = 0;
        }
    }
    if { ctx.state.tile_frame_counter[657] += 1; ctx.state.tile_frame_counter[657] } >= 8 {
        ctx.state.tile_frame_counter[(657) as usize] = 0;
        if { ctx.state.tile_frame[657] += 1; ctx.state.tile_frame[657] } >= 8 {
            ctx.state.tile_frame[(657) as usize] = 0;
        }
    }
    if { ctx.state.tile_frame_counter[658] += 1; ctx.state.tile_frame_counter[658] } >= 4 {
        ctx.state.tile_frame_counter[(658) as usize] = 0;
        if { ctx.state.tile_frame[658] += 1; ctx.state.tile_frame[658] } >= 10 {
            ctx.state.tile_frame[(658) as usize] = 0;
        }
    }
    if { ctx.state.tile_frame_counter[720] += 1; ctx.state.tile_frame_counter[720] } >= 8 {
        ctx.state.tile_frame_counter[(720) as usize] = 0;
        if { ctx.state.tile_frame[720] += 1; ctx.state.tile_frame[720] } >= 16 {
            ctx.state.tile_frame[(720) as usize] = 0;
        }
    }
    if { ctx.state.tile_frame_counter[721] += 1; ctx.state.tile_frame_counter[721] } >= 8 {
        ctx.state.tile_frame_counter[(721) as usize] = 0;
        if { ctx.state.tile_frame[721] += 1; ctx.state.tile_frame[721] } >= 32 {
            ctx.state.tile_frame[(721) as usize] = 0;
        }
    }
    if { ctx.state.tile_frame_counter[725] += 1; ctx.state.tile_frame_counter[725] } >= 8 {
        ctx.state.tile_frame_counter[(725) as usize] = 0;
        if { ctx.state.tile_frame[725] += 1; ctx.state.tile_frame[725] } >= 4 {
            ctx.state.tile_frame[(725) as usize] = 0;
        }
    }
    if { ctx.state.tile_frame_counter[421] += 1; ctx.state.tile_frame_counter[421] } >= 4 {
        ctx.state.tile_frame_counter[(421) as usize] = 0;
        if { ctx.state.tile_frame[421] += 1; ctx.state.tile_frame[421] } >= 4 {
            ctx.state.tile_frame[(421) as usize] = 0;
        }
    }
    if { ctx.state.tile_frame_counter[422] += 1; ctx.state.tile_frame_counter[422] } >= 4 {
        ctx.state.tile_frame_counter[(422) as usize] = 0;
        if { ctx.state.tile_frame[422] -= 1; ctx.state.tile_frame[422] } < 0 {
            ctx.state.tile_frame[(422) as usize] = 3;
        }
    }
    if { ctx.state.tile_frame_counter[463] += 1; ctx.state.tile_frame_counter[463] } >= 10 {
        ctx.state.tile_frame_counter[(463) as usize] = 0;
        if { ctx.state.tile_frame[463] += 1; ctx.state.tile_frame[463] } >= 6 {
            ctx.state.tile_frame[(463) as usize] = 0;
        }
    }
    if { ctx.state.tile_frame_counter[464] += 1; ctx.state.tile_frame_counter[464] } >= 5 {
        ctx.state.tile_frame_counter[(464) as usize] = 0;
        if { ctx.state.tile_frame[464] += 1; ctx.state.tile_frame[464] } >= 23 {
            ctx.state.tile_frame[(464) as usize] = 0;
        }
    }
    if { ctx.state.tile_frame_counter[485] += 1; ctx.state.tile_frame_counter[485] } >= 20 {
        ctx.state.tile_frame_counter[(485) as usize] = 0;
    }
    if { ctx.state.tile_frame_counter[491] += 1; ctx.state.tile_frame_counter[491] } >= 40 {
        ctx.state.tile_frame_counter[(491) as usize] = 0;
    }
    if { ctx.state.tile_frame_counter[564] += 1; ctx.state.tile_frame_counter[564] } >= 5 {
        ctx.state.tile_frame_counter[(564) as usize] = 0;
        ctx.state.tile_frame[(564) as usize] += 1;
        if ctx.state.tile_frame[564] >= 36 {
            ctx.state.tile_frame[(564) as usize] = 0;
        }
    }
    if { ctx.state.tile_frame_counter[593] += 1; ctx.state.tile_frame_counter[593] } >= 5 {
        ctx.state.tile_frame_counter[(593) as usize] = 0;
        ctx.state.tile_frame[(593) as usize] += 1;
        if ctx.state.tile_frame[593] >= 9 {
            ctx.state.tile_frame[(593) as usize] = 5;
        }
    }
    if { ctx.state.tile_frame_counter[594] += 1; ctx.state.tile_frame_counter[594] } >= 5 {
        ctx.state.tile_frame_counter[(594) as usize] = 0;
        ctx.state.tile_frame[(594) as usize] += 1;
        if ctx.state.tile_frame[594] >= 9 {
            ctx.state.tile_frame[(594) as usize] = 5;
        }
    }
    if { ctx.state.tile_frame_counter[614] += 1; ctx.state.tile_frame_counter[614] } >= 5 {
        ctx.state.tile_frame_counter[(614) as usize] = 0;
        ctx.state.tile_frame[(614) as usize] += 1;
        if ctx.state.tile_frame[614] >= 6 {
            ctx.state.tile_frame[(614) as usize] = 0;
        }
    }
    if { ctx.state.tile_frame_counter[565] += 1; ctx.state.tile_frame_counter[565] } >= 4 {
        ctx.state.tile_frame_counter[(565) as usize] = 0;
        ctx.state.tile_frame[(565) as usize] += 1;
        if ctx.state.tile_frame[565] >= 5 {
            ctx.state.tile_frame[(565) as usize] = 0;
        }
    }
    if { ctx.state.tile_frame_counter[572] += 1; ctx.state.tile_frame_counter[572] } >= 6 {
        ctx.state.tile_frame_counter[(572) as usize] = 0;
        if { ctx.state.tile_frame[572] += 1; ctx.state.tile_frame[572] } >= 4 {
            ctx.state.tile_frame[(572) as usize] = 0;
        }
    }
    if { ctx.state.tile_frame_counter[597] += 1; ctx.state.tile_frame_counter[597] } >= 64 {
        ctx.state.tile_frame_counter[(597) as usize] = 0;
    }
    let mut num3: i32 = (terraria_core::xna::math_helper::clamp((math_floor_f32(f32::abs(ctx.g.main.wind_for_visuals()) * 10.0_f32) as f32) * (math_sign(ctx.g.main.wind_for_visuals()) as f32), -5.0_f32, 5.0_f32) as i32);
    ctx.state.tile_frame_counter[(489) as usize] += num3;
    ctx.state.tile_frame_counter[(489) as usize] %= 320;
    if ctx.state.tile_frame_counter[489] < 0 {
        ctx.state.tile_frame_counter[(489) as usize] += 320;
    }
    animate_tiles_weather_vane(ctx);
    let mut num4: i32 = (terraria_core::xna::math_helper::clamp((math_floor_f32(f32::abs(ctx.g.main.wind_for_visuals()) * 10.0_f32) as f32) * (math_sign(ctx.g.main.wind_for_visuals()) as f32), -5.0_f32, 5.0_f32) as i32);
    ctx.state.tile_frame_counter[(493) as usize] += num4;
    ctx.state.tile_frame_counter[(493) as usize] %= 120;
    if ctx.state.tile_frame_counter[493] < 0 {
        ctx.state.tile_frame_counter[(493) as usize] += 120;
    }
    animate_tiles_critter_cages(ctx);
}

/// `AnimateTiles_WeatherVane()` (Main.cs:19522)
fn animate_tiles_weather_vane(ctx: &mut DrawContext) {
    let mut num: i32 = math_sign(ctx.g.main.wind_for_visuals());
    let mut num2: i32 = (terraria_core::xna::math_helper::clamp((math_floor_f32(f32::abs(ctx.g.main.wind_for_visuals()) * 10.0_f32) as f32), -5.0_f32, 5.0_f32) as i32);
    let mut num3: i32 = 6;
    ctx.state.tile_frame_counter[(490) as usize] += num2;
    if ctx.state.tile_frame_counter[490] < num3 {
        return;
    }
    ctx.state.tile_frame_counter[(490) as usize] -= num3 * ctx.state.tile_frame_counter[490];
    if (ctx.state.tile_frame[490] != 0 || num != -1) && (ctx.state.tile_frame[490] != 6 || num != 1) {
        if { ctx.state.tile_frame[490] += 1; ctx.state.tile_frame[490] } >= 12 {
            ctx.state.tile_frame[(490) as usize] = 0;
            ctx.state.weather_vane_bobframe = 0;
        }
    } else if ctx.g.main.rand.next_float() < f32::abs(ctx.g.main.wind_for_visuals()) * 0.5_f32 && { ctx.state.weather_vane_bobframe += 1; ctx.state.weather_vane_bobframe } == 8 {
        ctx.state.weather_vane_bobframe = 0;
    }
}

/// `AnimateTiles_CritterCages()` (Main.cs:13720)
pub fn animate_tiles_critter_cages(ctx: &mut DrawContext) {
    if !ctx.state.critter_cage {
        return;
    }
    for i in 0..ctx.state.cage_frames {
        if ctx.state.bunny_cage_frame[(i) as usize] == 0 {
            ctx.state.bunny_cage_frame_counter[(i) as usize] += 1;
            if ctx.state.bunny_cage_frame_counter[(i) as usize] <= ctx.g.main.rand.next_range(30, 900) {
                continue;
            }
            if ctx.g.main.rand.next(3) != 0 {
                let mut num: i32 = ctx.g.main.rand.next(7);
                if num == 0 {
                    ctx.state.bunny_cage_frame[(i) as usize] = 4;
                } else if num <= 2 {
                    ctx.state.bunny_cage_frame[(i) as usize] = 2;
                } else {
                    ctx.state.bunny_cage_frame[(i) as usize] = 1;
                }
            }
            ctx.state.bunny_cage_frame_counter[(i) as usize] = 0;
        } else if ctx.state.bunny_cage_frame[(i) as usize] == 1 {
            ctx.state.bunny_cage_frame_counter[(i) as usize] += 1;
            if ctx.state.bunny_cage_frame_counter[(i) as usize] >= 10 {
                ctx.state.bunny_cage_frame_counter[(i) as usize] = 0;
                ctx.state.bunny_cage_frame[(i) as usize] = 0;
            }
        } else if ctx.state.bunny_cage_frame[(i) as usize] >= 2 && ctx.state.bunny_cage_frame[(i) as usize] <= 3 {
            ctx.state.bunny_cage_frame_counter[(i) as usize] += 1;
            if ctx.state.bunny_cage_frame_counter[(i) as usize] >= 10 {
                ctx.state.bunny_cage_frame_counter[(i) as usize] = 0;
                ctx.state.bunny_cage_frame[(i) as usize] += 1;
            }
            if ctx.state.bunny_cage_frame[(i) as usize] > 3 {
                ctx.state.bunny_cage_frame[(i) as usize] = 0;
            }
        } else if ctx.state.bunny_cage_frame[(i) as usize] >= 4 && ctx.state.bunny_cage_frame[(i) as usize] <= 10 {
            ctx.state.bunny_cage_frame_counter[(i) as usize] += 1;
            if ctx.state.bunny_cage_frame_counter[(i) as usize] >= 5 {
                ctx.state.bunny_cage_frame_counter[(i) as usize] = 0;
                ctx.state.bunny_cage_frame[(i) as usize] += 1;
            }
        } else if ctx.state.bunny_cage_frame[(i) as usize] == 11 {
            ctx.state.bunny_cage_frame_counter[(i) as usize] += 1;
            if ctx.state.bunny_cage_frame_counter[(i) as usize] <= ctx.g.main.rand.next_range(30, 900) {
                continue;
            }
            if ctx.g.main.rand.next(3) != 0 {
                if ctx.g.main.rand.next(7) == 0 {
                    ctx.state.bunny_cage_frame[(i) as usize] = 13;
                } else {
                    ctx.state.bunny_cage_frame[(i) as usize] = 12;
                }
            }
            ctx.state.bunny_cage_frame_counter[(i) as usize] = 0;
        } else if ctx.state.bunny_cage_frame[(i) as usize] == 12 {
            ctx.state.bunny_cage_frame_counter[(i) as usize] += 1;
            if ctx.state.bunny_cage_frame_counter[(i) as usize] >= 10 {
                ctx.state.bunny_cage_frame_counter[(i) as usize] = 0;
                ctx.state.bunny_cage_frame[(i) as usize] = 11;
            }
        } else if ctx.state.bunny_cage_frame[(i) as usize] >= 13 {
            ctx.state.bunny_cage_frame_counter[(i) as usize] += 1;
            if ctx.state.bunny_cage_frame_counter[(i) as usize] >= 5 {
                ctx.state.bunny_cage_frame_counter[(i) as usize] = 0;
                ctx.state.bunny_cage_frame[(i) as usize] += 1;
            }
            if ctx.state.bunny_cage_frame[(i) as usize] > 21 {
                ctx.state.bunny_cage_frame[(i) as usize] = 0;
            }
        }
    }
    for j in 0..ctx.state.cage_frames {
        if ctx.state.squirrel_cage_frame[(j) as usize] == 0 {
            ctx.state.squirrel_cage_frame_counter[(j) as usize] += 1;
            if ctx.state.squirrel_cage_frame_counter[(j) as usize] <= ctx.g.main.rand.next_range(30, 900) {
                continue;
            }
            if ctx.g.main.rand.next(3) != 0 {
                let mut num: i32 = ctx.g.main.rand.next(7);
                if num == 0 {
                    ctx.state.squirrel_cage_frame[(j) as usize] = 4;
                } else if num <= 2 {
                    ctx.state.squirrel_cage_frame[(j) as usize] = 2;
                } else {
                    ctx.state.squirrel_cage_frame[(j) as usize] = 1;
                }
            }
            ctx.state.squirrel_cage_frame_counter[(j) as usize] = 0;
        } else if ctx.state.squirrel_cage_frame[(j) as usize] == 1 {
            ctx.state.squirrel_cage_frame_counter[(j) as usize] += 1;
            if ctx.state.squirrel_cage_frame_counter[(j) as usize] >= 10 {
                ctx.state.squirrel_cage_frame_counter[(j) as usize] = 0;
                ctx.state.squirrel_cage_frame[(j) as usize] = 0;
            }
        } else if ctx.state.squirrel_cage_frame[(j) as usize] >= 2 && ctx.state.squirrel_cage_frame[(j) as usize] <= 3 {
            ctx.state.squirrel_cage_frame_counter[(j) as usize] += 1;
            if ctx.state.squirrel_cage_frame_counter[(j) as usize] >= 5 {
                ctx.state.squirrel_cage_frame_counter[(j) as usize] = 0;
                ctx.state.squirrel_cage_frame[(j) as usize] += 1;
            }
            if ctx.state.squirrel_cage_frame[(j) as usize] > 3 {
                if ctx.g.main.rand.next(5) == 0 {
                    ctx.state.squirrel_cage_frame[(j) as usize] = 0;
                } else {
                    ctx.state.squirrel_cage_frame[(j) as usize] = 2;
                }
            }
        } else if ctx.state.squirrel_cage_frame[(j) as usize] >= 4 && ctx.state.squirrel_cage_frame[(j) as usize] <= 8 {
            ctx.state.squirrel_cage_frame_counter[(j) as usize] += 1;
            if ctx.state.squirrel_cage_frame_counter[(j) as usize] >= 5 {
                ctx.state.squirrel_cage_frame_counter[(j) as usize] = 0;
                ctx.state.squirrel_cage_frame[(j) as usize] += 1;
            }
        } else if ctx.state.squirrel_cage_frame[(j) as usize] == 9 {
            ctx.state.squirrel_cage_frame_counter[(j) as usize] += 1;
            if ctx.state.squirrel_cage_frame_counter[(j) as usize] <= ctx.g.main.rand.next_range(30, 900) {
                continue;
            }
            if ctx.g.main.rand.next(3) != 0 {
                let mut num: i32 = ctx.g.main.rand.next(7);
                if num == 0 {
                    ctx.state.squirrel_cage_frame[(j) as usize] = 13;
                } else if num <= 2 {
                    ctx.state.squirrel_cage_frame[(j) as usize] = 11;
                } else {
                    ctx.state.squirrel_cage_frame[(j) as usize] = 10;
                }
            }
            ctx.state.squirrel_cage_frame_counter[(j) as usize] = 0;
        } else if ctx.state.squirrel_cage_frame[(j) as usize] == 10 {
            ctx.state.squirrel_cage_frame_counter[(j) as usize] += 1;
            if ctx.state.squirrel_cage_frame_counter[(j) as usize] >= 10 {
                ctx.state.squirrel_cage_frame_counter[(j) as usize] = 0;
                ctx.state.squirrel_cage_frame[(j) as usize] = 9;
            }
        } else if ctx.state.squirrel_cage_frame[(j) as usize] == 11 || ctx.state.squirrel_cage_frame[(j) as usize] == 12 {
            ctx.state.squirrel_cage_frame_counter[(j) as usize] += 1;
            if ctx.state.squirrel_cage_frame_counter[(j) as usize] < 5 {
                continue;
            }
            ctx.state.squirrel_cage_frame[(j) as usize] += 1;
            if ctx.state.squirrel_cage_frame[(j) as usize] > 12 {
                if ctx.g.main.rand.next(5) != 0 {
                    ctx.state.squirrel_cage_frame[(j) as usize] = 11;
                } else {
                    ctx.state.squirrel_cage_frame[(j) as usize] = 9;
                }
            }
            ctx.state.squirrel_cage_frame_counter[(j) as usize] = 0;
        } else if ctx.state.squirrel_cage_frame[(j) as usize] >= 13 {
            ctx.state.squirrel_cage_frame_counter[(j) as usize] += 1;
            if ctx.state.squirrel_cage_frame_counter[(j) as usize] >= 5 {
                ctx.state.squirrel_cage_frame_counter[(j) as usize] = 0;
                ctx.state.squirrel_cage_frame[(j) as usize] += 1;
            }
            if ctx.state.squirrel_cage_frame[(j) as usize] > 17 {
                ctx.state.squirrel_cage_frame[(j) as usize] = 0;
            }
        }
    }
    for k in 0..ctx.state.cage_frames {
        if ctx.state.squirrel_cage_frame_orange[(k) as usize] == 0 {
            ctx.state.squirrel_cage_frame_counter_orange[(k) as usize] += 1;
            if ctx.state.squirrel_cage_frame_counter_orange[(k) as usize] <= ctx.g.main.rand.next_range(30, 900) {
                continue;
            }
            if ctx.g.main.rand.next(3) != 0 {
                let mut num: i32 = ctx.g.main.rand.next(7);
                if num == 0 {
                    ctx.state.squirrel_cage_frame_orange[(k) as usize] = 4;
                } else if num <= 2 {
                    ctx.state.squirrel_cage_frame_orange[(k) as usize] = 2;
                } else {
                    ctx.state.squirrel_cage_frame_orange[(k) as usize] = 1;
                }
            }
            ctx.state.squirrel_cage_frame_counter_orange[(k) as usize] = 0;
        } else if ctx.state.squirrel_cage_frame_orange[(k) as usize] == 1 {
            ctx.state.squirrel_cage_frame_counter_orange[(k) as usize] += 1;
            if ctx.state.squirrel_cage_frame_counter_orange[(k) as usize] >= 10 {
                ctx.state.squirrel_cage_frame_counter_orange[(k) as usize] = 0;
                ctx.state.squirrel_cage_frame_orange[(k) as usize] = 0;
            }
        } else if ctx.state.squirrel_cage_frame_orange[(k) as usize] >= 2 && ctx.state.squirrel_cage_frame_orange[(k) as usize] <= 3 {
            ctx.state.squirrel_cage_frame_counter_orange[(k) as usize] += 1;
            if ctx.state.squirrel_cage_frame_counter_orange[(k) as usize] >= 5 {
                ctx.state.squirrel_cage_frame_counter_orange[(k) as usize] = 0;
                ctx.state.squirrel_cage_frame_orange[(k) as usize] += 1;
            }
            if ctx.state.squirrel_cage_frame_orange[(k) as usize] > 3 {
                if ctx.g.main.rand.next(5) == 0 {
                    ctx.state.squirrel_cage_frame_orange[(k) as usize] = 0;
                } else {
                    ctx.state.squirrel_cage_frame_orange[(k) as usize] = 2;
                }
            }
        } else if ctx.state.squirrel_cage_frame_orange[(k) as usize] >= 4 && ctx.state.squirrel_cage_frame_orange[(k) as usize] <= 8 {
            ctx.state.squirrel_cage_frame_counter_orange[(k) as usize] += 1;
            if ctx.state.squirrel_cage_frame_counter_orange[(k) as usize] >= 5 {
                ctx.state.squirrel_cage_frame_counter_orange[(k) as usize] = 0;
                ctx.state.squirrel_cage_frame_orange[(k) as usize] += 1;
            }
        } else if ctx.state.squirrel_cage_frame_orange[(k) as usize] == 9 {
            ctx.state.squirrel_cage_frame_counter_orange[(k) as usize] += 1;
            if ctx.state.squirrel_cage_frame_counter_orange[(k) as usize] <= ctx.g.main.rand.next_range(30, 900) {
                continue;
            }
            if ctx.g.main.rand.next(3) != 0 {
                let mut num: i32 = ctx.g.main.rand.next(7);
                if num == 0 {
                    ctx.state.squirrel_cage_frame_orange[(k) as usize] = 13;
                } else if num <= 2 {
                    ctx.state.squirrel_cage_frame_orange[(k) as usize] = 11;
                } else {
                    ctx.state.squirrel_cage_frame_orange[(k) as usize] = 10;
                }
            }
            ctx.state.squirrel_cage_frame_counter_orange[(k) as usize] = 0;
        } else if ctx.state.squirrel_cage_frame_orange[(k) as usize] == 10 {
            ctx.state.squirrel_cage_frame_counter_orange[(k) as usize] += 1;
            if ctx.state.squirrel_cage_frame_counter_orange[(k) as usize] >= 10 {
                ctx.state.squirrel_cage_frame_counter_orange[(k) as usize] = 0;
                ctx.state.squirrel_cage_frame_orange[(k) as usize] = 9;
            }
        } else if ctx.state.squirrel_cage_frame_orange[(k) as usize] == 11 || ctx.state.squirrel_cage_frame_orange[(k) as usize] == 12 {
            ctx.state.squirrel_cage_frame_counter_orange[(k) as usize] += 1;
            if ctx.state.squirrel_cage_frame_counter_orange[(k) as usize] < 5 {
                continue;
            }
            ctx.state.squirrel_cage_frame_orange[(k) as usize] += 1;
            if ctx.state.squirrel_cage_frame_orange[(k) as usize] > 12 {
                if ctx.g.main.rand.next(5) != 0 {
                    ctx.state.squirrel_cage_frame_orange[(k) as usize] = 11;
                } else {
                    ctx.state.squirrel_cage_frame_orange[(k) as usize] = 9;
                }
            }
            ctx.state.squirrel_cage_frame_counter_orange[(k) as usize] = 0;
        } else if ctx.state.squirrel_cage_frame_orange[(k) as usize] >= 13 {
            ctx.state.squirrel_cage_frame_counter_orange[(k) as usize] += 1;
            if ctx.state.squirrel_cage_frame_counter_orange[(k) as usize] >= 5 {
                ctx.state.squirrel_cage_frame_counter_orange[(k) as usize] = 0;
                ctx.state.squirrel_cage_frame_orange[(k) as usize] += 1;
            }
            if ctx.state.squirrel_cage_frame_orange[(k) as usize] > 17 {
                ctx.state.squirrel_cage_frame_orange[(k) as usize] = 0;
            }
        }
    }
    for l in 0..ctx.state.cage_frames {
        if ctx.state.mallard_cage_frame[(l) as usize] == 0 || ctx.state.mallard_cage_frame[(l) as usize] == 4 {
            ctx.state.mallard_cage_frame_counter[(l) as usize] += 1;
            if ctx.state.mallard_cage_frame_counter[(l) as usize] <= ctx.g.main.rand.next_range(45, 2700) {
                continue;
            }
            if (ctx.state.mallard_cage_frame[(l) as usize] == 0 && ctx.g.main.rand.next(3) != 0) || (ctx.state.mallard_cage_frame[(l) as usize] == 4 && ctx.g.main.rand.next(5) == 0) {
                if ctx.g.main.rand.next(6) == 0 {
                    ctx.state.mallard_cage_frame[(l) as usize] = 5;
                } else if ctx.g.main.rand.next(3) == 0 {
                    if ctx.state.mallard_cage_frame[(l) as usize] == 4 {
                        ctx.state.mallard_cage_frame[(l) as usize] = 0;
                    } else {
                        ctx.state.mallard_cage_frame[(l) as usize] = 4;
                    }
                } else {
                    ctx.state.mallard_cage_frame[(l) as usize] = 1;
                }
            }
            ctx.state.mallard_cage_frame_counter[(l) as usize] = 0;
        } else if ctx.state.mallard_cage_frame[(l) as usize] >= 1 && ctx.state.mallard_cage_frame[(l) as usize] <= 3 {
            ctx.state.mallard_cage_frame_counter[(l) as usize] += 1;
            if ctx.state.mallard_cage_frame_counter[(l) as usize] >= 5 {
                ctx.state.mallard_cage_frame_counter[(l) as usize] = 0;
                ctx.state.mallard_cage_frame[(l) as usize] += 1;
            }
            if ctx.state.mallard_cage_frame[(l) as usize] > 3 {
                if ctx.g.main.rand.next(5) == 0 {
                    ctx.state.mallard_cage_frame[(l) as usize] = 0;
                } else {
                    ctx.state.mallard_cage_frame[(l) as usize] = 1;
                }
            }
        } else if ctx.state.mallard_cage_frame[(l) as usize] >= 5 && ctx.state.mallard_cage_frame[(l) as usize] <= 11 {
            ctx.state.mallard_cage_frame_counter[(l) as usize] += 1;
            if ctx.state.mallard_cage_frame_counter[(l) as usize] >= 5 {
                ctx.state.mallard_cage_frame_counter[(l) as usize] = 0;
                ctx.state.mallard_cage_frame[(l) as usize] += 1;
            }
        } else if ctx.state.mallard_cage_frame[(l) as usize] == 12 || ctx.state.mallard_cage_frame[(l) as usize] == 16 {
            ctx.state.mallard_cage_frame_counter[(l) as usize] += 1;
            if ctx.state.mallard_cage_frame_counter[(l) as usize] <= ctx.g.main.rand.next_range(45, 2700) {
                continue;
            }
            if (ctx.state.mallard_cage_frame[(l) as usize] == 12 && ctx.g.main.rand.next(3) != 0) || (ctx.state.mallard_cage_frame[(l) as usize] == 16 && ctx.g.main.rand.next(5) == 0) {
                if ctx.g.main.rand.next(6) == 0 {
                    ctx.state.mallard_cage_frame[(l) as usize] = 17;
                } else if ctx.g.main.rand.next(3) == 0 {
                    if ctx.state.mallard_cage_frame[(l) as usize] == 16 {
                        ctx.state.mallard_cage_frame[(l) as usize] = 12;
                    } else {
                        ctx.state.mallard_cage_frame[(l) as usize] = 16;
                    }
                } else {
                    ctx.state.mallard_cage_frame[(l) as usize] = 13;
                }
            }
            ctx.state.mallard_cage_frame_counter[(l) as usize] = 0;
        } else if ctx.state.mallard_cage_frame[(l) as usize] >= 13 && ctx.state.mallard_cage_frame[(l) as usize] <= 15 {
            ctx.state.mallard_cage_frame_counter[(l) as usize] += 1;
            if ctx.state.mallard_cage_frame_counter[(l) as usize] < 5 {
                continue;
            }
            ctx.state.mallard_cage_frame[(l) as usize] += 1;
            if ctx.state.mallard_cage_frame[(l) as usize] > 15 {
                if ctx.g.main.rand.next(5) != 0 {
                    ctx.state.mallard_cage_frame[(l) as usize] = 12;
                } else {
                    ctx.state.mallard_cage_frame[(l) as usize] = 13;
                }
            }
            ctx.state.mallard_cage_frame_counter[(l) as usize] = 0;
        } else if ctx.state.mallard_cage_frame[(l) as usize] >= 17 {
            ctx.state.mallard_cage_frame_counter[(l) as usize] += 1;
            if ctx.state.mallard_cage_frame_counter[(l) as usize] >= 5 {
                ctx.state.mallard_cage_frame_counter[(l) as usize] = 0;
                ctx.state.mallard_cage_frame[(l) as usize] += 1;
            }
            if ctx.state.mallard_cage_frame[(l) as usize] > 23 {
                ctx.state.mallard_cage_frame[(l) as usize] = 0;
            }
        }
    }
    for m in 0..ctx.state.cage_frames {
        if ctx.state.duck_cage_frame[(m) as usize] == 0 || ctx.state.duck_cage_frame[(m) as usize] == 4 {
            ctx.state.duck_cage_frame_counter[(m) as usize] += 1;
            if ctx.state.duck_cage_frame_counter[(m) as usize] <= ctx.g.main.rand.next_range(45, 2700) {
                continue;
            }
            if (ctx.state.duck_cage_frame[(m) as usize] == 0 && ctx.g.main.rand.next(3) != 0) || (ctx.state.duck_cage_frame[(m) as usize] == 4 && ctx.g.main.rand.next(5) == 0) {
                if ctx.g.main.rand.next(6) == 0 {
                    ctx.state.duck_cage_frame[(m) as usize] = 5;
                } else if ctx.g.main.rand.next(3) == 0 {
                    if ctx.state.duck_cage_frame[(m) as usize] == 4 {
                        ctx.state.duck_cage_frame[(m) as usize] = 0;
                    } else {
                        ctx.state.duck_cage_frame[(m) as usize] = 4;
                    }
                } else {
                    ctx.state.duck_cage_frame[(m) as usize] = 1;
                }
            }
            ctx.state.duck_cage_frame_counter[(m) as usize] = 0;
        } else if ctx.state.duck_cage_frame[(m) as usize] >= 1 && ctx.state.duck_cage_frame[(m) as usize] <= 3 {
            ctx.state.duck_cage_frame_counter[(m) as usize] += 1;
            if ctx.state.duck_cage_frame_counter[(m) as usize] >= 5 {
                ctx.state.duck_cage_frame_counter[(m) as usize] = 0;
                ctx.state.duck_cage_frame[(m) as usize] += 1;
            }
            if ctx.state.duck_cage_frame[(m) as usize] > 3 {
                if ctx.g.main.rand.next(5) == 0 {
                    ctx.state.duck_cage_frame[(m) as usize] = 0;
                } else {
                    ctx.state.duck_cage_frame[(m) as usize] = 1;
                }
            }
        } else if ctx.state.duck_cage_frame[(m) as usize] >= 5 && ctx.state.duck_cage_frame[(m) as usize] <= 11 {
            ctx.state.duck_cage_frame_counter[(m) as usize] += 1;
            if ctx.state.duck_cage_frame_counter[(m) as usize] >= 5 {
                ctx.state.duck_cage_frame_counter[(m) as usize] = 0;
                ctx.state.duck_cage_frame[(m) as usize] += 1;
            }
        } else if ctx.state.duck_cage_frame[(m) as usize] == 12 || ctx.state.duck_cage_frame[(m) as usize] == 16 {
            ctx.state.duck_cage_frame_counter[(m) as usize] += 1;
            if ctx.state.duck_cage_frame_counter[(m) as usize] <= ctx.g.main.rand.next_range(45, 2700) {
                continue;
            }
            if (ctx.state.duck_cage_frame[(m) as usize] == 12 && ctx.g.main.rand.next(3) != 0) || (ctx.state.duck_cage_frame[(m) as usize] == 16 && ctx.g.main.rand.next(5) == 0) {
                if ctx.g.main.rand.next(6) == 0 {
                    ctx.state.duck_cage_frame[(m) as usize] = 17;
                } else if ctx.g.main.rand.next(3) == 0 {
                    if ctx.state.duck_cage_frame[(m) as usize] == 16 {
                        ctx.state.duck_cage_frame[(m) as usize] = 12;
                    } else {
                        ctx.state.duck_cage_frame[(m) as usize] = 16;
                    }
                } else {
                    ctx.state.duck_cage_frame[(m) as usize] = 13;
                }
            }
            ctx.state.duck_cage_frame_counter[(m) as usize] = 0;
        } else if ctx.state.duck_cage_frame[(m) as usize] >= 13 && ctx.state.duck_cage_frame[(m) as usize] <= 15 {
            ctx.state.duck_cage_frame_counter[(m) as usize] += 1;
            if ctx.state.duck_cage_frame_counter[(m) as usize] < 5 {
                continue;
            }
            ctx.state.duck_cage_frame[(m) as usize] += 1;
            if ctx.state.duck_cage_frame[(m) as usize] > 15 {
                if ctx.g.main.rand.next(5) != 0 {
                    ctx.state.duck_cage_frame[(m) as usize] = 12;
                } else {
                    ctx.state.duck_cage_frame[(m) as usize] = 13;
                }
            }
            ctx.state.duck_cage_frame_counter[(m) as usize] = 0;
        } else if ctx.state.duck_cage_frame[(m) as usize] >= 17 {
            ctx.state.duck_cage_frame_counter[(m) as usize] += 1;
            if ctx.state.duck_cage_frame_counter[(m) as usize] >= 5 {
                ctx.state.duck_cage_frame_counter[(m) as usize] = 0;
                ctx.state.duck_cage_frame[(m) as usize] += 1;
            }
            if ctx.state.duck_cage_frame[(m) as usize] > 23 {
                ctx.state.duck_cage_frame[(m) as usize] = 0;
            }
        }
    }
    for n in 0..ctx.state.cage_frames {
        ctx.state.grebe_cage_frame_counter[(n) as usize] += 1;
        let mut flag: bool = ctx.state.grebe_cage_frame[(n) as usize] == 0 || ctx.state.grebe_cage_frame[(n) as usize] == 7;
        let mut num2: bool = ctx.state.grebe_cage_frame[(n) as usize] == 16 || ctx.state.grebe_cage_frame[(n) as usize] == 20;
        let mut num3: i32 = 5;
        if flag {
            num3 = ctx.g.main.rand.next_range(300, 400);
        }
        if num2 {
            num3 = ctx.g.main.rand.next_range(480, 600);
        }
        if ctx.state.grebe_cage_frame_counter[(n) as usize] < num3 {
            continue;
        }
        ctx.state.grebe_cage_frame_counter[(n) as usize] = 0;
        if ctx.state.grebe_cage_frame[(n) as usize] >= 25 && ctx.state.grebe_cage_frame[(n) as usize] <= 27 {
            ctx.state.grebe_cage_frame[(n) as usize] += 1;
            if ctx.state.grebe_cage_frame[(n) as usize] > 27 {
                if ctx.g.main.rand.next(5) == 0 {
                    ctx.state.grebe_cage_frame[(n) as usize] = 7;
                } else {
                    ctx.state.grebe_cage_frame[(n) as usize] = 25;
                }
            }
        } else if ctx.state.grebe_cage_frame[(n) as usize] >= 22 && ctx.state.grebe_cage_frame[(n) as usize] <= 24 {
            ctx.state.grebe_cage_frame[(n) as usize] += 1;
            if ctx.state.grebe_cage_frame[(n) as usize] > 24 {
                if ctx.g.main.rand.next(5) == 0 {
                    ctx.state.grebe_cage_frame[(n) as usize] = 0;
                } else {
                    ctx.state.grebe_cage_frame[(n) as usize] = 22;
                }
            }
        } else if ctx.state.grebe_cage_frame[(n) as usize] == 0 && ctx.g.main.rand.next(3) == 0 {
            ctx.state.grebe_cage_frame[(n) as usize] = (if ctx.g.main.rand.next(2) == 0 { 22 } else { 14 });
        } else if ctx.state.grebe_cage_frame[(n) as usize] == 7 && ctx.g.main.rand.next(3) == 0 {
            ctx.state.grebe_cage_frame[(n) as usize] = (if ctx.g.main.rand.next(2) == 0 { 25 } else { 18 });
        } else if ctx.state.grebe_cage_frame[(n) as usize] == 13 || ctx.state.grebe_cage_frame[(n) as usize] == 17 {
            ctx.state.grebe_cage_frame[(n) as usize] = 0;
        } else if ctx.state.grebe_cage_frame[(n) as usize] == 21 {
            ctx.state.grebe_cage_frame[(n) as usize] = 7;
        } else {
            ctx.state.grebe_cage_frame[(n) as usize] += 1;
        }
    }
    for num4 in 0..ctx.state.cage_frames {
        ctx.state.seagull_cage_frame_counter[(num4) as usize] += 1;
        let mut flag2: bool = ctx.state.seagull_cage_frame[(num4) as usize] == 0 || ctx.state.seagull_cage_frame[(num4) as usize] == 14;
        let mut flag3: bool = ctx.state.seagull_cage_frame[(num4) as usize] >= 15 && ctx.state.seagull_cage_frame[(num4) as usize] <= 18;
        let mut num5: bool = ctx.state.seagull_cage_frame[(num4) as usize] >= 9 && ctx.state.seagull_cage_frame[(num4) as usize] <= 13;
        let mut num6: i32 = 6;
        if flag2 {
            num6 = ctx.g.main.rand.next_range(180, 250);
        }
        if flag3 {
            num6 = 66;
        }
        if num5 {
            num6 = 78;
        }
        if ctx.state.seagull_cage_frame_counter[(num4) as usize] >= num6 {
            ctx.state.seagull_cage_frame_counter[(num4) as usize] = 0;
            if ctx.state.seagull_cage_frame[(num4) as usize] == 0 && ctx.g.main.rand.next(3) == 0 {
                ctx.state.seagull_cage_frame[(num4) as usize] = 9;
                continue;
            }
            if ctx.state.seagull_cage_frame[(num4) as usize] == 14 {
                if ctx.g.main.rand.next(2) == 0 {
                    ctx.state.seagull_cage_frame[(num4) as usize] = 15;
                } else {
                    ctx.state.seagull_cage_frame[(num4) as usize] = 19;
                }
                continue;
            }
            if ctx.state.seagull_cage_frame[(num4) as usize] == 9 {
                ctx.state.seagull_cage_frame[(num4) as usize] = 0;
                continue;
            }
            if ctx.state.seagull_cage_frame[(num4) as usize] == 8 || ctx.state.seagull_cage_frame[(num4) as usize] == 15 {
                ctx.state.seagull_cage_frame[(num4) as usize] = 14;
                continue;
            }
            ctx.state.seagull_cage_frame[(num4) as usize] += 1;
            if ctx.state.seagull_cage_frame[(num4) as usize] > 26 {
                ctx.state.seagull_cage_frame[(num4) as usize] = 0;
            }
        } else if ctx.state.seagull_cage_frame[(num4) as usize] >= 15 && ctx.state.seagull_cage_frame[(num4) as usize] <= 18 {
            let mut num7: i32 = ctx.state.seagull_cage_frame_counter[(num4) as usize] % 66;
            if num7 > 60 {
                ctx.state.seagull_cage_frame[(num4) as usize] = 15;
            } else if num7 > 54 {
                ctx.state.seagull_cage_frame[(num4) as usize] = 16;
            } else if num7 > 48 {
                ctx.state.seagull_cage_frame[(num4) as usize] = 17;
            } else if num7 > 42 {
                ctx.state.seagull_cage_frame[(num4) as usize] = 18;
            } else if num7 > 36 {
                ctx.state.seagull_cage_frame[(num4) as usize] = 17;
            } else if num7 > 30 {
                ctx.state.seagull_cage_frame[(num4) as usize] = 16;
            } else if num7 > 24 {
                ctx.state.seagull_cage_frame[(num4) as usize] = 17;
            } else if num7 > 18 {
                ctx.state.seagull_cage_frame[(num4) as usize] = 18;
            } else if num7 > 12 {
                ctx.state.seagull_cage_frame[(num4) as usize] = 17;
            } else if num7 > 6 {
                ctx.state.seagull_cage_frame[(num4) as usize] = 16;
            } else {
                ctx.state.seagull_cage_frame[(num4) as usize] = 15;
            }
        } else if ctx.state.seagull_cage_frame[(num4) as usize] >= 9 && ctx.state.seagull_cage_frame[(num4) as usize] <= 13 {
            let mut num8: i32 = ctx.state.seagull_cage_frame_counter[(num4) as usize] % 78;
            if num8 > 72 {
                ctx.state.seagull_cage_frame[(num4) as usize] = 9;
            } else if num8 > 66 {
                ctx.state.seagull_cage_frame[(num4) as usize] = 10;
            } else if num8 > 60 {
                ctx.state.seagull_cage_frame[(num4) as usize] = 11;
            } else if num8 > 54 {
                ctx.state.seagull_cage_frame[(num4) as usize] = 12;
            } else if num8 > 48 {
                ctx.state.seagull_cage_frame[(num4) as usize] = 13;
            } else if num8 > 42 {
                ctx.state.seagull_cage_frame[(num4) as usize] = 12;
            } else if num8 > 36 {
                ctx.state.seagull_cage_frame[(num4) as usize] = 11;
            } else if num8 > 30 {
                ctx.state.seagull_cage_frame[(num4) as usize] = 12;
            } else if num8 > 24 {
                ctx.state.seagull_cage_frame[(num4) as usize] = 13;
            } else if num8 > 18 {
                ctx.state.seagull_cage_frame[(num4) as usize] = 12;
            } else if num8 > 12 {
                ctx.state.seagull_cage_frame[(num4) as usize] = 11;
            } else if num8 > 6 {
                ctx.state.seagull_cage_frame[(num4) as usize] = 10;
            } else {
                ctx.state.seagull_cage_frame[(num4) as usize] = 9;
            }
        }
    }
    for num9 in 0..ctx.state.cage_frames {
        if ctx.state.bird_cage_frame[(num9) as usize] == 0 {
            ctx.state.bird_cage_frame_counter[(num9) as usize] += 1;
            if ctx.state.bird_cage_frame_counter[(num9) as usize] <= ctx.g.main.rand.next_range(30, 2700) {
                continue;
            }
            if ctx.g.main.rand.next(3) != 0 {
                if ctx.g.main.rand.next(3) != 0 {
                    ctx.state.bird_cage_frame[(num9) as usize] = 2;
                } else {
                    ctx.state.bird_cage_frame[(num9) as usize] = 1;
                }
            }
            ctx.state.bird_cage_frame_counter[(num9) as usize] = 0;
        } else if ctx.state.bird_cage_frame[(num9) as usize] == 1 {
            ctx.state.bird_cage_frame_counter[(num9) as usize] += 1;
            if ctx.state.bird_cage_frame_counter[(num9) as usize] > ctx.g.main.rand.next_range(900, 18000) && ctx.g.main.rand.next(3) == 0 {
                ctx.state.bird_cage_frame_counter[(num9) as usize] = 0;
                ctx.state.bird_cage_frame[(num9) as usize] = 0;
            }
        } else if ctx.state.bird_cage_frame[(num9) as usize] >= 2 && ctx.state.bird_cage_frame[(num9) as usize] <= 5 {
            ctx.state.bird_cage_frame_counter[(num9) as usize] += 1;
            if ctx.state.bird_cage_frame_counter[(num9) as usize] >= 5 {
                ctx.state.bird_cage_frame_counter[(num9) as usize] = 0;
                if ctx.state.bird_cage_frame[(num9) as usize] == 3 && ctx.g.main.rand.next(3) == 0 {
                    ctx.state.bird_cage_frame[(num9) as usize] = 13;
                } else {
                    ctx.state.bird_cage_frame[(num9) as usize] += 1;
                }
            }
        } else if ctx.state.bird_cage_frame[(num9) as usize] == 6 {
            ctx.state.bird_cage_frame_counter[(num9) as usize] += 1;
            if ctx.state.bird_cage_frame_counter[(num9) as usize] <= ctx.g.main.rand.next_range(45, 2700) {
                continue;
            }
            if ctx.g.main.rand.next(3) != 0 {
                if ctx.g.main.rand.next(6) == 0 {
                    ctx.state.bird_cage_frame[(num9) as usize] = 7;
                } else if ctx.g.main.rand.next(6) == 0 {
                    ctx.state.bird_cage_frame[(num9) as usize] = 11;
                }
            }
            ctx.state.bird_cage_frame_counter[(num9) as usize] = 0;
        } else if ctx.state.bird_cage_frame[(num9) as usize] >= 7 && ctx.state.bird_cage_frame[(num9) as usize] <= 10 {
            ctx.state.bird_cage_frame_counter[(num9) as usize] += 1;
            if ctx.state.bird_cage_frame_counter[(num9) as usize] >= 5 {
                ctx.state.bird_cage_frame[(num9) as usize] += 1;
                if ctx.state.bird_cage_frame[(num9) as usize] > 10 {
                    ctx.state.bird_cage_frame[(num9) as usize] = 0;
                }
                ctx.state.bird_cage_frame_counter[(num9) as usize] = 0;
            }
        } else if ctx.state.bird_cage_frame[(num9) as usize] >= 11 && ctx.state.bird_cage_frame[(num9) as usize] <= 13 {
            ctx.state.bird_cage_frame_counter[(num9) as usize] += 1;
            if ctx.state.bird_cage_frame_counter[(num9) as usize] >= 5 {
                ctx.state.bird_cage_frame[(num9) as usize] += 1;
                ctx.state.bird_cage_frame_counter[(num9) as usize] = 0;
            }
        } else if ctx.state.bird_cage_frame[(num9) as usize] == 14 {
            ctx.state.bird_cage_frame_counter[(num9) as usize] += 1;
            if ctx.state.bird_cage_frame_counter[(num9) as usize] > ctx.g.main.rand.next_range(5, 600) {
                if ctx.g.main.rand.next(20) == 0 {
                    ctx.state.bird_cage_frame[(num9) as usize] = 16;
                } else if ctx.g.main.rand.next(20) == 0 {
                    ctx.state.bird_cage_frame[(num9) as usize] = 4;
                } else {
                    ctx.state.bird_cage_frame[(num9) as usize] = 15;
                }
                ctx.state.bird_cage_frame_counter[(num9) as usize] = 0;
            }
        } else if ctx.state.bird_cage_frame[(num9) as usize] == 15 {
            ctx.state.bird_cage_frame_counter[(num9) as usize] += 1;
            if ctx.state.bird_cage_frame_counter[(num9) as usize] >= 10 {
                ctx.state.bird_cage_frame_counter[(num9) as usize] = 0;
                ctx.state.bird_cage_frame[(num9) as usize] = 14;
            }
        } else {
            if ctx.state.bird_cage_frame[(num9) as usize] < 16 || ctx.state.bird_cage_frame[(num9) as usize] > 18 {
                continue;
            }
            ctx.state.bird_cage_frame_counter[(num9) as usize] += 1;
            if ctx.state.bird_cage_frame_counter[(num9) as usize] >= 5 {
                ctx.state.bird_cage_frame[(num9) as usize] += 1;
                if ctx.state.bird_cage_frame[(num9) as usize] > 18 {
                    ctx.state.bird_cage_frame[(num9) as usize] = 0;
                }
                ctx.state.bird_cage_frame_counter[(num9) as usize] = 0;
            }
        }
    }
    for num10 in 0..ctx.state.cage_frames {
        if ctx.state.blue_bird_cage_frame[(num10) as usize] == 0 {
            ctx.state.blue_bird_cage_frame_counter[(num10) as usize] += 1;
            if ctx.state.blue_bird_cage_frame_counter[(num10) as usize] <= ctx.g.main.rand.next_range(30, 2700) {
                continue;
            }
            if ctx.g.main.rand.next(3) != 0 {
                if ctx.g.main.rand.next(3) != 0 {
                    ctx.state.blue_bird_cage_frame[(num10) as usize] = 2;
                } else {
                    ctx.state.blue_bird_cage_frame[(num10) as usize] = 1;
                }
            }
            ctx.state.blue_bird_cage_frame_counter[(num10) as usize] = 0;
        } else if ctx.state.blue_bird_cage_frame[(num10) as usize] == 1 {
            ctx.state.blue_bird_cage_frame_counter[(num10) as usize] += 1;
            if ctx.state.blue_bird_cage_frame_counter[(num10) as usize] > ctx.g.main.rand.next_range(900, 18000) && ctx.g.main.rand.next(3) == 0 {
                ctx.state.blue_bird_cage_frame_counter[(num10) as usize] = 0;
                ctx.state.blue_bird_cage_frame[(num10) as usize] = 0;
            }
        } else if ctx.state.blue_bird_cage_frame[(num10) as usize] >= 2 && ctx.state.blue_bird_cage_frame[(num10) as usize] <= 5 {
            ctx.state.blue_bird_cage_frame_counter[(num10) as usize] += 1;
            if ctx.state.blue_bird_cage_frame_counter[(num10) as usize] >= 5 {
                ctx.state.blue_bird_cage_frame_counter[(num10) as usize] = 0;
                if ctx.state.blue_bird_cage_frame[(num10) as usize] == 3 && ctx.g.main.rand.next(3) == 0 {
                    ctx.state.blue_bird_cage_frame[(num10) as usize] = 13;
                } else {
                    ctx.state.blue_bird_cage_frame[(num10) as usize] += 1;
                }
            }
        } else if ctx.state.blue_bird_cage_frame[(num10) as usize] == 6 {
            ctx.state.blue_bird_cage_frame_counter[(num10) as usize] += 1;
            if ctx.state.blue_bird_cage_frame_counter[(num10) as usize] <= ctx.g.main.rand.next_range(45, 2700) {
                continue;
            }
            if ctx.g.main.rand.next(3) != 0 {
                if ctx.g.main.rand.next(6) == 0 {
                    ctx.state.blue_bird_cage_frame[(num10) as usize] = 7;
                } else if ctx.g.main.rand.next(6) == 0 {
                    ctx.state.blue_bird_cage_frame[(num10) as usize] = 11;
                }
            }
            ctx.state.blue_bird_cage_frame_counter[(num10) as usize] = 0;
        } else if ctx.state.blue_bird_cage_frame[(num10) as usize] >= 7 && ctx.state.blue_bird_cage_frame[(num10) as usize] <= 10 {
            ctx.state.blue_bird_cage_frame_counter[(num10) as usize] += 1;
            if ctx.state.blue_bird_cage_frame_counter[(num10) as usize] >= 5 {
                ctx.state.blue_bird_cage_frame[(num10) as usize] += 1;
                if ctx.state.blue_bird_cage_frame[(num10) as usize] > 10 {
                    ctx.state.blue_bird_cage_frame[(num10) as usize] = 0;
                }
                ctx.state.blue_bird_cage_frame_counter[(num10) as usize] = 0;
            }
        } else if ctx.state.blue_bird_cage_frame[(num10) as usize] >= 11 && ctx.state.blue_bird_cage_frame[(num10) as usize] <= 13 {
            ctx.state.blue_bird_cage_frame_counter[(num10) as usize] += 1;
            if ctx.state.blue_bird_cage_frame_counter[(num10) as usize] >= 5 {
                ctx.state.blue_bird_cage_frame[(num10) as usize] += 1;
                ctx.state.blue_bird_cage_frame_counter[(num10) as usize] = 0;
            }
        } else if ctx.state.blue_bird_cage_frame[(num10) as usize] == 14 {
            ctx.state.blue_bird_cage_frame_counter[(num10) as usize] += 1;
            if ctx.state.blue_bird_cage_frame_counter[(num10) as usize] > ctx.g.main.rand.next_range(5, 600) {
                if ctx.g.main.rand.next(20) == 0 {
                    ctx.state.blue_bird_cage_frame[(num10) as usize] = 16;
                } else if ctx.g.main.rand.next(20) == 0 {
                    ctx.state.blue_bird_cage_frame[(num10) as usize] = 4;
                } else {
                    ctx.state.blue_bird_cage_frame[(num10) as usize] = 15;
                }
                ctx.state.blue_bird_cage_frame_counter[(num10) as usize] = 0;
            }
        } else if ctx.state.blue_bird_cage_frame[(num10) as usize] == 15 {
            ctx.state.blue_bird_cage_frame_counter[(num10) as usize] += 1;
            if ctx.state.blue_bird_cage_frame_counter[(num10) as usize] >= 10 {
                ctx.state.blue_bird_cage_frame_counter[(num10) as usize] = 0;
                ctx.state.blue_bird_cage_frame[(num10) as usize] = 14;
            }
        } else {
            if ctx.state.blue_bird_cage_frame[(num10) as usize] < 16 || ctx.state.blue_bird_cage_frame[(num10) as usize] > 18 {
                continue;
            }
            ctx.state.blue_bird_cage_frame_counter[(num10) as usize] += 1;
            if ctx.state.blue_bird_cage_frame_counter[(num10) as usize] >= 5 {
                ctx.state.blue_bird_cage_frame[(num10) as usize] += 1;
                if ctx.state.blue_bird_cage_frame[(num10) as usize] > 18 {
                    ctx.state.blue_bird_cage_frame[(num10) as usize] = 0;
                }
                ctx.state.blue_bird_cage_frame_counter[(num10) as usize] = 0;
            }
        }
    }
    for num11 in 0..ctx.state.cage_frames {
        if ctx.state.red_bird_cage_frame[(num11) as usize] == 0 {
            ctx.state.red_bird_cage_frame_counter[(num11) as usize] += 1;
            if ctx.state.red_bird_cage_frame_counter[(num11) as usize] <= ctx.g.main.rand.next_range(30, 2700) {
                continue;
            }
            if ctx.g.main.rand.next(3) != 0 {
                if ctx.g.main.rand.next(3) != 0 {
                    ctx.state.red_bird_cage_frame[(num11) as usize] = 2;
                } else {
                    ctx.state.red_bird_cage_frame[(num11) as usize] = 1;
                }
            }
            ctx.state.red_bird_cage_frame_counter[(num11) as usize] = 0;
        } else if ctx.state.red_bird_cage_frame[(num11) as usize] == 1 {
            ctx.state.red_bird_cage_frame_counter[(num11) as usize] += 1;
            if ctx.state.red_bird_cage_frame_counter[(num11) as usize] > ctx.g.main.rand.next_range(900, 18000) && ctx.g.main.rand.next(3) == 0 {
                ctx.state.red_bird_cage_frame_counter[(num11) as usize] = 0;
                ctx.state.red_bird_cage_frame[(num11) as usize] = 0;
            }
        } else if ctx.state.red_bird_cage_frame[(num11) as usize] >= 2 && ctx.state.red_bird_cage_frame[(num11) as usize] <= 5 {
            ctx.state.red_bird_cage_frame_counter[(num11) as usize] += 1;
            if ctx.state.red_bird_cage_frame_counter[(num11) as usize] >= 5 {
                ctx.state.red_bird_cage_frame_counter[(num11) as usize] = 0;
                if ctx.state.red_bird_cage_frame[(num11) as usize] == 3 && ctx.g.main.rand.next(3) == 0 {
                    ctx.state.red_bird_cage_frame[(num11) as usize] = 13;
                } else {
                    ctx.state.red_bird_cage_frame[(num11) as usize] += 1;
                }
            }
        } else if ctx.state.red_bird_cage_frame[(num11) as usize] == 6 {
            ctx.state.red_bird_cage_frame_counter[(num11) as usize] += 1;
            if ctx.state.red_bird_cage_frame_counter[(num11) as usize] <= ctx.g.main.rand.next_range(45, 2700) {
                continue;
            }
            if ctx.g.main.rand.next(3) != 0 {
                if ctx.g.main.rand.next(6) == 0 {
                    ctx.state.red_bird_cage_frame[(num11) as usize] = 7;
                } else if ctx.g.main.rand.next(6) == 0 {
                    ctx.state.red_bird_cage_frame[(num11) as usize] = 11;
                }
            }
            ctx.state.red_bird_cage_frame_counter[(num11) as usize] = 0;
        } else if ctx.state.red_bird_cage_frame[(num11) as usize] >= 7 && ctx.state.red_bird_cage_frame[(num11) as usize] <= 10 {
            ctx.state.red_bird_cage_frame_counter[(num11) as usize] += 1;
            if ctx.state.red_bird_cage_frame_counter[(num11) as usize] >= 5 {
                ctx.state.red_bird_cage_frame[(num11) as usize] += 1;
                if ctx.state.red_bird_cage_frame[(num11) as usize] > 10 {
                    ctx.state.red_bird_cage_frame[(num11) as usize] = 0;
                }
                ctx.state.red_bird_cage_frame_counter[(num11) as usize] = 0;
            }
        } else if ctx.state.red_bird_cage_frame[(num11) as usize] >= 11 && ctx.state.red_bird_cage_frame[(num11) as usize] <= 13 {
            ctx.state.red_bird_cage_frame_counter[(num11) as usize] += 1;
            if ctx.state.red_bird_cage_frame_counter[(num11) as usize] >= 5 {
                ctx.state.red_bird_cage_frame[(num11) as usize] += 1;
                ctx.state.red_bird_cage_frame_counter[(num11) as usize] = 0;
            }
        } else if ctx.state.red_bird_cage_frame[(num11) as usize] == 14 {
            ctx.state.red_bird_cage_frame_counter[(num11) as usize] += 1;
            if ctx.state.red_bird_cage_frame_counter[(num11) as usize] > ctx.g.main.rand.next_range(5, 600) {
                if ctx.g.main.rand.next(20) == 0 {
                    ctx.state.red_bird_cage_frame[(num11) as usize] = 16;
                } else if ctx.g.main.rand.next(20) == 0 {
                    ctx.state.red_bird_cage_frame[(num11) as usize] = 4;
                } else {
                    ctx.state.red_bird_cage_frame[(num11) as usize] = 15;
                }
                ctx.state.red_bird_cage_frame_counter[(num11) as usize] = 0;
            }
        } else if ctx.state.red_bird_cage_frame[(num11) as usize] == 15 {
            ctx.state.red_bird_cage_frame_counter[(num11) as usize] += 1;
            if ctx.state.red_bird_cage_frame_counter[(num11) as usize] >= 10 {
                ctx.state.red_bird_cage_frame_counter[(num11) as usize] = 0;
                ctx.state.red_bird_cage_frame[(num11) as usize] = 14;
            }
        } else {
            if ctx.state.red_bird_cage_frame[(num11) as usize] < 16 || ctx.state.red_bird_cage_frame[(num11) as usize] > 18 {
                continue;
            }
            ctx.state.red_bird_cage_frame_counter[(num11) as usize] += 1;
            if ctx.state.red_bird_cage_frame_counter[(num11) as usize] >= 5 {
                ctx.state.red_bird_cage_frame[(num11) as usize] += 1;
                if ctx.state.red_bird_cage_frame[(num11) as usize] > 18 {
                    ctx.state.red_bird_cage_frame[(num11) as usize] = 0;
                }
                ctx.state.red_bird_cage_frame_counter[(num11) as usize] = 0;
            }
        }
    }
    for num12 in 0..ctx.state.cage_frames {
        if ctx.state.macaw_cage_frame[(num12) as usize] == 0 {
            ctx.state.macaw_cage_frame_counter[(num12) as usize] += 1;
            if ctx.state.macaw_cage_frame_counter[(num12) as usize] <= ctx.g.main.rand.next_range(300, 2700) {
                continue;
            }
            if ctx.g.main.rand.next(3) != 0 {
                if ctx.g.main.rand.next(3) != 0 {
                    ctx.state.macaw_cage_frame[(num12) as usize] = 16;
                } else {
                    ctx.state.macaw_cage_frame[(num12) as usize] = 1;
                }
            }
            ctx.state.macaw_cage_frame_counter[(num12) as usize] = 0;
        } else if ctx.state.macaw_cage_frame[(num12) as usize] >= 1 && ctx.state.macaw_cage_frame[(num12) as usize] <= 5 {
            ctx.state.macaw_cage_frame_counter[(num12) as usize] += 1;
            if ctx.state.macaw_cage_frame_counter[(num12) as usize] > 5 {
                ctx.state.macaw_cage_frame_counter[(num12) as usize] = 0;
                ctx.state.macaw_cage_frame[(num12) as usize] += 1;
                if ctx.state.macaw_cage_frame[(num12) as usize] > 5 {
                    ctx.state.macaw_cage_frame[(num12) as usize] = 6;
                }
            }
        } else if ctx.state.macaw_cage_frame[(num12) as usize] >= 16 && ctx.state.macaw_cage_frame[(num12) as usize] <= 19 {
            ctx.state.macaw_cage_frame_counter[(num12) as usize] += 1;
            if ctx.state.macaw_cage_frame_counter[(num12) as usize] > 6 {
                ctx.state.macaw_cage_frame_counter[(num12) as usize] = 0;
                ctx.state.macaw_cage_frame[(num12) as usize] += 1;
                if ctx.state.macaw_cage_frame[(num12) as usize] > 19 {
                    ctx.state.macaw_cage_frame[(num12) as usize] = (if ctx.g.main.rand.next(3) != 0 { 16 } else { 0 });
                }
            }
        } else if ctx.state.macaw_cage_frame[(num12) as usize] == 6 {
            ctx.state.macaw_cage_frame_counter[(num12) as usize] += 1;
            if ctx.state.macaw_cage_frame_counter[(num12) as usize] <= ctx.g.main.rand.next_range(300, 2700) {
                continue;
            }
            if ctx.g.main.rand.next(3) != 0 {
                if ctx.g.main.rand.next(3) != 0 {
                    ctx.state.macaw_cage_frame[(num12) as usize] = 7;
                } else {
                    ctx.state.macaw_cage_frame[(num12) as usize] = 11;
                }
            }
            ctx.state.macaw_cage_frame_counter[(num12) as usize] = 0;
        } else if ctx.state.macaw_cage_frame[(num12) as usize] >= 11 && ctx.state.macaw_cage_frame[(num12) as usize] <= 15 {
            ctx.state.macaw_cage_frame_counter[(num12) as usize] += 1;
            if ctx.state.macaw_cage_frame_counter[(num12) as usize] > 5 {
                ctx.state.macaw_cage_frame_counter[(num12) as usize] = 0;
                ctx.state.macaw_cage_frame[(num12) as usize] += 1;
                if ctx.state.macaw_cage_frame[(num12) as usize] > 15 {
                    ctx.state.macaw_cage_frame[(num12) as usize] = 0;
                }
            }
        } else {
            if ctx.state.macaw_cage_frame[(num12) as usize] < 7 || ctx.state.macaw_cage_frame[(num12) as usize] > 10 {
                continue;
            }
            ctx.state.macaw_cage_frame_counter[(num12) as usize] += 1;
            if ctx.state.macaw_cage_frame_counter[(num12) as usize] > 6 {
                ctx.state.macaw_cage_frame_counter[(num12) as usize] = 0;
                ctx.state.macaw_cage_frame[(num12) as usize] += 1;
                if ctx.state.macaw_cage_frame[(num12) as usize] > 10 {
                    ctx.state.macaw_cage_frame[(num12) as usize] = (if ctx.g.main.rand.next(3) == 0 { 6 } else { 7 });
                }
            }
        }
    }
    for num13 in 0..2 {
        for num14 in 0..ctx.state.cage_frames {
            if ctx.state.scorpion_cage_frame[(num13) as usize][(num14) as usize] == 0 || ctx.state.scorpion_cage_frame[(num13) as usize][(num14) as usize] == 7 {
                ctx.state.scorpion_cage_frame_counter[(num13) as usize][(num14) as usize] += 1;
                if ctx.state.scorpion_cage_frame_counter[(num13) as usize][(num14) as usize] <= ctx.g.main.rand.next_range(30, 3600) {
                    continue;
                }
                if ctx.state.scorpion_cage_frame[(num13) as usize][(num14) as usize] == 7 {
                    ctx.state.scorpion_cage_frame[(num13) as usize][(num14) as usize] = 0;
                } else if ctx.g.main.rand.next(3) == 0 {
                    if ctx.g.main.rand.next(7) == 0 {
                        ctx.state.scorpion_cage_frame[(num13) as usize][(num14) as usize] = 1;
                    } else if ctx.g.main.rand.next(4) == 0 {
                        ctx.state.scorpion_cage_frame[(num13) as usize][(num14) as usize] = 8;
                    } else if ctx.g.main.rand.next(3) == 0 {
                        ctx.state.scorpion_cage_frame[(num13) as usize][(num14) as usize] = 7;
                    } else {
                        ctx.state.scorpion_cage_frame[(num13) as usize][(num14) as usize] = 14;
                    }
                }
                ctx.state.scorpion_cage_frame_counter[(num13) as usize][(num14) as usize] = 0;
            } else if ctx.state.scorpion_cage_frame[(num13) as usize][(num14) as usize] >= 1 && ctx.state.scorpion_cage_frame[(num13) as usize][(num14) as usize] <= 2 {
                ctx.state.scorpion_cage_frame_counter[(num13) as usize][(num14) as usize] += 1;
                if ctx.state.scorpion_cage_frame_counter[(num13) as usize][(num14) as usize] >= 10 {
                    ctx.state.scorpion_cage_frame_counter[(num13) as usize][(num14) as usize] = 0;
                    ctx.state.scorpion_cage_frame[(num13) as usize][(num14) as usize] += 1;
                }
            } else if ctx.state.scorpion_cage_frame[(num13) as usize][(num14) as usize] >= 8 && ctx.state.scorpion_cage_frame[(num13) as usize][(num14) as usize] <= 10 {
                ctx.state.scorpion_cage_frame_counter[(num13) as usize][(num14) as usize] += 1;
                if ctx.state.scorpion_cage_frame_counter[(num13) as usize][(num14) as usize] >= 10 {
                    ctx.state.scorpion_cage_frame_counter[(num13) as usize][(num14) as usize] = 0;
                    ctx.state.scorpion_cage_frame[(num13) as usize][(num14) as usize] += 1;
                }
            } else if ctx.state.scorpion_cage_frame[(num13) as usize][(num14) as usize] == 11 {
                ctx.state.scorpion_cage_frame_counter[(num13) as usize][(num14) as usize] += 1;
                if ctx.state.scorpion_cage_frame_counter[(num13) as usize][(num14) as usize] > ctx.g.main.rand.next_range(45, 5400) {
                    if ctx.g.main.rand.next(6) == 0 {
                        ctx.state.scorpion_cage_frame[(num13) as usize][(num14) as usize] = 12;
                    }
                    ctx.state.scorpion_cage_frame_counter[(num13) as usize][(num14) as usize] = 0;
                }
            } else if ctx.state.scorpion_cage_frame[(num13) as usize][(num14) as usize] >= 12 && ctx.state.scorpion_cage_frame[(num13) as usize][(num14) as usize] <= 13 {
                ctx.state.scorpion_cage_frame_counter[(num13) as usize][(num14) as usize] += 1;
                if ctx.state.scorpion_cage_frame_counter[(num13) as usize][(num14) as usize] >= 10 {
                    ctx.state.scorpion_cage_frame_counter[(num13) as usize][(num14) as usize] = 0;
                    ctx.state.scorpion_cage_frame[(num13) as usize][(num14) as usize] += 1;
                    if ctx.state.scorpion_cage_frame[(num13) as usize][(num14) as usize] > 13 {
                        ctx.state.scorpion_cage_frame[(num13) as usize][(num14) as usize] = 0;
                    }
                }
            } else if ctx.state.scorpion_cage_frame[(num13) as usize][(num14) as usize] >= 14 && ctx.state.scorpion_cage_frame[(num13) as usize][(num14) as usize] <= 15 {
                ctx.state.scorpion_cage_frame_counter[(num13) as usize][(num14) as usize] += 1;
                if ctx.state.scorpion_cage_frame_counter[(num13) as usize][(num14) as usize] >= 5 {
                    ctx.state.scorpion_cage_frame_counter[(num13) as usize][(num14) as usize] = 0;
                    ctx.state.scorpion_cage_frame[(num13) as usize][(num14) as usize] += 1;
                    if ctx.state.scorpion_cage_frame[(num13) as usize][(num14) as usize] > 15 {
                        ctx.state.scorpion_cage_frame[(num13) as usize][(num14) as usize] = 14;
                    }
                    if ctx.g.main.rand.next(5) == 0 {
                        ctx.state.scorpion_cage_frame[(num13) as usize][(num14) as usize] = 0;
                    }
                }
            } else if ctx.state.scorpion_cage_frame[(num13) as usize][(num14) as usize] == 4 || ctx.state.scorpion_cage_frame[(num13) as usize][(num14) as usize] == 3 {
                ctx.state.scorpion_cage_frame_counter[(num13) as usize][(num14) as usize] += 1;
                if ctx.state.scorpion_cage_frame_counter[(num13) as usize][(num14) as usize] <= ctx.g.main.rand.next_range(30, 3600) {
                    continue;
                }
                if ctx.state.scorpion_cage_frame[(num13) as usize][(num14) as usize] == 3 {
                    ctx.state.scorpion_cage_frame[(num13) as usize][(num14) as usize] = 4;
                } else if ctx.g.main.rand.next(3) == 0 {
                    if ctx.g.main.rand.next(5) == 0 {
                        ctx.state.scorpion_cage_frame[(num13) as usize][(num14) as usize] = 5;
                    } else if ctx.g.main.rand.next(3) == 0 {
                        ctx.state.scorpion_cage_frame[(num13) as usize][(num14) as usize] = 3;
                    } else {
                        ctx.state.scorpion_cage_frame[(num13) as usize][(num14) as usize] = 16;
                    }
                }
                ctx.state.scorpion_cage_frame_counter[(num13) as usize][(num14) as usize] = 0;
            } else if ctx.state.scorpion_cage_frame[(num13) as usize][(num14) as usize] >= 5 && ctx.state.scorpion_cage_frame[(num13) as usize][(num14) as usize] <= 6 {
                ctx.state.scorpion_cage_frame_counter[(num13) as usize][(num14) as usize] += 1;
                if ctx.state.scorpion_cage_frame_counter[(num13) as usize][(num14) as usize] >= 10 {
                    ctx.state.scorpion_cage_frame_counter[(num13) as usize][(num14) as usize] = 0;
                    ctx.state.scorpion_cage_frame[(num13) as usize][(num14) as usize] += 1;
                    if ctx.state.scorpion_cage_frame[(num13) as usize][(num14) as usize] > 7 {
                        ctx.state.scorpion_cage_frame[(num13) as usize][(num14) as usize] = 0;
                    }
                }
            } else {
                if ctx.state.scorpion_cage_frame[(num13) as usize][(num14) as usize] < 16 || ctx.state.scorpion_cage_frame[(num13) as usize][(num14) as usize] > 17 {
                    continue;
                }
                ctx.state.scorpion_cage_frame_counter[(num13) as usize][(num14) as usize] += 1;
                if ctx.state.scorpion_cage_frame_counter[(num13) as usize][(num14) as usize] >= 5 {
                    ctx.state.scorpion_cage_frame_counter[(num13) as usize][(num14) as usize] = 0;
                    ctx.state.scorpion_cage_frame[(num13) as usize][(num14) as usize] += 1;
                    if ctx.state.scorpion_cage_frame[(num13) as usize][(num14) as usize] > 17 {
                        ctx.state.scorpion_cage_frame[(num13) as usize][(num14) as usize] = 16;
                    }
                    if ctx.g.main.rand.next(5) == 0 {
                        ctx.state.scorpion_cage_frame[(num13) as usize][(num14) as usize] = 4;
                    }
                }
            }
        }
    }
    for num15 in 0..ctx.state.cage_frames {
        if ctx.state.penguin_cage_frame[(num15) as usize] == 0 {
            ctx.state.penguin_cage_frame_counter[(num15) as usize] += 1;
            if ctx.state.penguin_cage_frame_counter[(num15) as usize] <= ctx.g.main.rand.next_range(30, 1800) {
                continue;
            }
            if ctx.g.main.rand.next(2) == 0 {
                if ctx.g.main.rand.next(10) == 0 {
                    ctx.state.penguin_cage_frame[(num15) as usize] = 4;
                } else if ctx.g.main.rand.next(7) == 0 {
                    ctx.state.penguin_cage_frame[(num15) as usize] = 15;
                } else if ctx.g.main.rand.next(3) == 0 {
                    ctx.state.penguin_cage_frame[(num15) as usize] = 2;
                } else {
                    ctx.state.penguin_cage_frame[(num15) as usize] = 1;
                }
            }
            ctx.state.penguin_cage_frame_counter[(num15) as usize] = 0;
        } else if ctx.state.penguin_cage_frame[(num15) as usize] == 1 {
            ctx.state.penguin_cage_frame_counter[(num15) as usize] += 1;
            if ctx.state.penguin_cage_frame_counter[(num15) as usize] >= 10 {
                ctx.state.penguin_cage_frame_counter[(num15) as usize] = 0;
                ctx.state.penguin_cage_frame[(num15) as usize] = 0;
            }
        } else if ctx.state.penguin_cage_frame[(num15) as usize] >= 2 && ctx.state.penguin_cage_frame[(num15) as usize] <= 3 {
            ctx.state.penguin_cage_frame_counter[(num15) as usize] += 1;
            if ctx.state.penguin_cage_frame_counter[(num15) as usize] < 5 {
                continue;
            }
            ctx.state.penguin_cage_frame_counter[(num15) as usize] = 0;
            ctx.state.penguin_cage_frame[(num15) as usize] += 1;
            if ctx.state.penguin_cage_frame[(num15) as usize] > 3 {
                if ctx.g.main.rand.next(3) == 0 {
                    ctx.state.penguin_cage_frame[(num15) as usize] = 0;
                } else {
                    ctx.state.penguin_cage_frame[(num15) as usize] = 2;
                }
            }
        } else if ctx.state.penguin_cage_frame[(num15) as usize] >= 4 && ctx.state.penguin_cage_frame[(num15) as usize] <= 6 {
            ctx.state.penguin_cage_frame_counter[(num15) as usize] += 1;
            if ctx.state.penguin_cage_frame_counter[(num15) as usize] >= 10 {
                ctx.state.penguin_cage_frame_counter[(num15) as usize] = 0;
                ctx.state.penguin_cage_frame[(num15) as usize] += 1;
            }
        } else if ctx.state.penguin_cage_frame[(num15) as usize] == 15 {
            ctx.state.penguin_cage_frame_counter[(num15) as usize] += 1;
            if ctx.state.penguin_cage_frame_counter[(num15) as usize] > ctx.g.main.rand.next_range(10, 1800) {
                if ctx.g.main.rand.next(2) == 0 {
                    ctx.state.penguin_cage_frame[(num15) as usize] = 0;
                }
                ctx.state.penguin_cage_frame_counter[(num15) as usize] = 0;
            }
        } else if ctx.state.penguin_cage_frame[(num15) as usize] == 8 {
            ctx.state.penguin_cage_frame_counter[(num15) as usize] += 1;
            if ctx.state.penguin_cage_frame_counter[(num15) as usize] <= ctx.g.main.rand.next_range(30, 3600) {
                continue;
            }
            if ctx.g.main.rand.next(2) == 0 {
                if ctx.g.main.rand.next(10) == 0 {
                    ctx.state.penguin_cage_frame[(num15) as usize] = 12;
                } else if ctx.g.main.rand.next(7) == 0 {
                    ctx.state.penguin_cage_frame[(num15) as usize] = 7;
                } else if ctx.g.main.rand.next(3) == 0 {
                    ctx.state.penguin_cage_frame[(num15) as usize] = 10;
                } else {
                    ctx.state.penguin_cage_frame[(num15) as usize] = 9;
                }
            }
            ctx.state.penguin_cage_frame_counter[(num15) as usize] = 0;
        } else if ctx.state.penguin_cage_frame[(num15) as usize] == 9 {
            ctx.state.penguin_cage_frame_counter[(num15) as usize] += 1;
            if ctx.state.penguin_cage_frame_counter[(num15) as usize] >= 10 {
                ctx.state.penguin_cage_frame_counter[(num15) as usize] = 0;
                ctx.state.penguin_cage_frame[(num15) as usize] = 8;
            }
        } else if ctx.state.penguin_cage_frame[(num15) as usize] >= 10 && ctx.state.penguin_cage_frame[(num15) as usize] <= 11 {
            ctx.state.penguin_cage_frame_counter[(num15) as usize] += 1;
            if ctx.state.penguin_cage_frame_counter[(num15) as usize] < 5 {
                continue;
            }
            ctx.state.penguin_cage_frame_counter[(num15) as usize] = 0;
            ctx.state.penguin_cage_frame[(num15) as usize] += 1;
            if ctx.state.penguin_cage_frame[(num15) as usize] > 3 {
                if ctx.g.main.rand.next(3) == 0 {
                    ctx.state.penguin_cage_frame[(num15) as usize] = 8;
                } else {
                    ctx.state.penguin_cage_frame[(num15) as usize] = 10;
                }
            }
        } else if ctx.state.penguin_cage_frame[(num15) as usize] >= 12 && ctx.state.penguin_cage_frame[(num15) as usize] <= 14 {
            ctx.state.penguin_cage_frame_counter[(num15) as usize] += 1;
            if ctx.state.penguin_cage_frame_counter[(num15) as usize] >= 10 {
                ctx.state.penguin_cage_frame_counter[(num15) as usize] = 0;
                ctx.state.penguin_cage_frame[(num15) as usize] += 1;
            }
        } else {
            if ctx.state.penguin_cage_frame[(num15) as usize] != 7 {
                continue;
            }
            ctx.state.penguin_cage_frame_counter[(num15) as usize] += 1;
            if ctx.state.penguin_cage_frame_counter[(num15) as usize] > ctx.g.main.rand.next_range(10, 3600) {
                if ctx.g.main.rand.next(2) == 0 {
                    ctx.state.penguin_cage_frame[(num15) as usize] = 8;
                }
                ctx.state.penguin_cage_frame_counter[(num15) as usize] = 0;
            }
        }
    }
    for num16 in 0..ctx.state.cage_frames {
        ctx.state.turtle_cage_frame_counter[(num16) as usize] += 1;
        let mut num17: bool = ctx.state.turtle_cage_frame[(num16) as usize] == 0 || ctx.state.turtle_cage_frame[(num16) as usize] == 15 || ctx.state.turtle_cage_frame[(num16) as usize] == 31;
        let mut num18: i32 = 8;
        if num17 {
            num18 = ctx.g.main.rand.next_range(180, 250);
        }
        if ctx.state.turtle_cage_frame_counter[(num16) as usize] < num18 {
            continue;
        }
        ctx.state.turtle_cage_frame_counter[(num16) as usize] = 0;
        if ctx.state.turtle_cage_frame[(num16) as usize] == 29 {
            ctx.state.turtle_cage_frame[(num16) as usize] = 0;
            continue;
        }
        if ctx.state.turtle_cage_frame[(num16) as usize] == 15 && ctx.g.main.rand.next(3) == 0 {
            ctx.state.turtle_cage_frame[(num16) as usize] = 30;
            continue;
        }
        ctx.state.turtle_cage_frame[(num16) as usize] += 1;
        if ctx.state.turtle_cage_frame[(num16) as usize] > 32 {
            ctx.state.turtle_cage_frame[(num16) as usize] = 15;
        }
    }
    for num19 in 0..ctx.state.cage_frames {
        ctx.state.seahorse_cage_frame_counter[(num19) as usize] += 1;
        let mut flag4: bool = ctx.state.seahorse_cage_frame[(num19) as usize] == 0 || ctx.state.seahorse_cage_frame[(num19) as usize] == 13;
        let mut num20: bool = ctx.state.seahorse_cage_frame[(num19) as usize] == 4 || ctx.state.seahorse_cage_frame[(num19) as usize] == 9 || ctx.state.seahorse_cage_frame[(num19) as usize] == 17 || ctx.state.seahorse_cage_frame[(num19) as usize] == 22;
        let mut flag5: bool = ctx.state.seahorse_cage_frame[(num19) as usize] >= 25;
        let mut num21: i32 = 7;
        if flag4 {
            num21 = ctx.g.main.rand.next_range(220, 340);
        }
        if num20 {
            num21 = 10;
        }
        if flag5 {
            num21 = 6;
        }
        if ctx.state.seahorse_cage_frame_counter[(num19) as usize] < num21 {
            continue;
        }
        ctx.state.seahorse_cage_frame_counter[(num19) as usize] = 0;
        if ctx.state.seahorse_cage_frame[(num19) as usize] == 0 && ctx.g.main.rand.next(2) == 0 {
            ctx.state.seahorse_cage_frame[(num19) as usize] = 25;
            continue;
        }
        if ctx.state.seahorse_cage_frame[(num19) as usize] == 24 {
            ctx.state.seahorse_cage_frame[(num19) as usize] = 0;
            continue;
        }
        ctx.state.seahorse_cage_frame[(num19) as usize] += 1;
        if ctx.state.seahorse_cage_frame[(num19) as usize] > 33 {
            ctx.state.seahorse_cage_frame[(num19) as usize] = 0;
        }
    }
    let mut num22: i32 = 40;
    for num23 in 0..ctx.state.cage_frames {
        if ctx.state.pufferfish_cage_frame[(num23) as usize] >= 0 && ctx.state.pufferfish_cage_frame[(num23) as usize] <= num22 {
            ctx.state.pufferfish_cage_frame_counter[(num23) as usize] += 1;
            if ctx.state.pufferfish_cage_frame_counter[(num23) as usize] < 7 {
                continue;
            }
            ctx.state.pufferfish_cage_frame_counter[(num23) as usize] = 0;
            ctx.state.pufferfish_cage_frame[(num23) as usize] += 1;
            if ctx.state.pufferfish_cage_frame[(num23) as usize] > num22 {
                ctx.state.pufferfish_cage_frame[(num23) as usize] = 0;
            }
            if ctx.g.main.rand.next(75) == 0 {
                match ctx.state.pufferfish_cage_frame[(num23) as usize] {
                    0 | 1 | 2 => {
                        ctx.state.pufferfish_cage_frame[(num23) as usize] = 41;
                    }
                    8 | 9 | 10 => {
                        ctx.state.pufferfish_cage_frame[(num23) as usize] = 46;
                    }
                    17 | 18 | 19 => {
                        ctx.state.pufferfish_cage_frame[(num23) as usize] = 51;
                    }
                    20 | 21 | 22 => {
                        ctx.state.pufferfish_cage_frame[(num23) as usize] = 56;
                    }
                    30 | 31 | 32 => {
                        ctx.state.pufferfish_cage_frame[(num23) as usize] = 61;
                    }
                    37 | 38 | 39 | 40 => {
                        ctx.state.pufferfish_cage_frame[(num23) as usize] = 66;
                    }
                    _ => {}
                }
            }
            continue;
        }
        let mut num24: i32 = 7;
        let mut num25: i32 = 5 * num24 * 5 + 8;
        let mut num26: i32 = num25 - 8;
        ctx.state.pufferfish_cage_frame_counter[(num23) as usize] += 1;
        if ctx.state.pufferfish_cage_frame_counter[(num23) as usize] < num26 {
            let mut num27: i32 = ctx.state.pufferfish_cage_frame_counter[(num23) as usize] / num24;
            if num27 > 3 {
                num27 = 3;
            }
            match ctx.state.pufferfish_cage_frame[(num23) as usize] {
                46 | 47 | 48 | 49 | 50 => {
                    ctx.state.pufferfish_cage_frame[(num23) as usize] = 46 + num27;
                }
                51 | 52 | 53 | 54 | 55 => {
                    ctx.state.pufferfish_cage_frame[(num23) as usize] = 51 + num27;
                }
                56 | 57 | 58 | 59 | 60 => {
                    ctx.state.pufferfish_cage_frame[(num23) as usize] = 56 + num27;
                }
                61 | 62 | 63 | 64 | 65 => {
                    ctx.state.pufferfish_cage_frame[(num23) as usize] = 61 + num27;
                }
                66 | 67 | 68 | 69 | 70 => {
                    ctx.state.pufferfish_cage_frame[(num23) as usize] = 66 + num27;
                }
                _ => {
                    ctx.state.pufferfish_cage_frame[(num23) as usize] = 41 + num27;
                }
            }
        } else if ctx.state.pufferfish_cage_frame_counter[(num23) as usize] >= num26 {
            match ctx.state.pufferfish_cage_frame[(num23) as usize] {
                46 | 47 | 48 | 49 | 50 => {
                    ctx.state.pufferfish_cage_frame[(num23) as usize] = 50;
                }
                51 | 52 | 53 | 54 | 55 => {
                    ctx.state.pufferfish_cage_frame[(num23) as usize] = 55;
                }
                56 | 57 | 58 | 59 | 60 => {
                    ctx.state.pufferfish_cage_frame[(num23) as usize] = 60;
                }
                61 | 62 | 63 | 64 | 65 => {
                    ctx.state.pufferfish_cage_frame[(num23) as usize] = 65;
                }
                66 | 67 | 68 | 69 | 70 => {
                    ctx.state.pufferfish_cage_frame[(num23) as usize] = 70;
                }
                _ => {
                    ctx.state.pufferfish_cage_frame[(num23) as usize] = 45;
                }
            }
        }
        if ctx.state.pufferfish_cage_frame_counter[(num23) as usize] >= num25 {
            ctx.state.pufferfish_cage_frame_counter[(num23) as usize] = 0;
            match ctx.state.pufferfish_cage_frame[(num23) as usize] {
                41 | 42 | 43 | 44 | 45 => {
                    ctx.state.pufferfish_cage_frame[(num23) as usize] = 1;
                }
                46 | 47 | 48 | 49 | 50 => {
                    ctx.state.pufferfish_cage_frame[(num23) as usize] = 9;
                }
                51 | 52 | 53 | 54 | 55 => {
                    ctx.state.pufferfish_cage_frame[(num23) as usize] = 18;
                }
                56 | 57 | 58 | 59 | 60 => {
                    ctx.state.pufferfish_cage_frame[(num23) as usize] = 21;
                }
                61 | 62 | 63 | 64 | 65 => {
                    ctx.state.pufferfish_cage_frame[(num23) as usize] = 31;
                }
                66 | 67 | 68 | 69 | 70 => {
                    ctx.state.pufferfish_cage_frame[(num23) as usize] = 38;
                }
                _ => {}
            }
        }
    }
    update_owl_cage_frames(ctx);
    for num28 in 0..ctx.state.cage_frames {
        if ctx.state.snail_cage_frame[(num28) as usize] >= 0 && ctx.state.snail_cage_frame[(num28) as usize] <= 13 {
            ctx.state.snail_cage_frame_counter[(num28) as usize] += 1;
            if ctx.state.snail_cage_frame_counter[(num28) as usize] <= ctx.g.main.rand.next_range(45, 3600) {
                continue;
            }
            if ctx.state.snail_cage_frame[(num28) as usize] == 8 && ctx.g.main.rand.next(2) == 0 {
                ctx.state.snail_cage_frame[(num28) as usize] = 14;
            } else if ctx.state.snail_cage_frame[(num28) as usize] == 1 && ctx.g.main.rand.next(3) == 0 {
                ctx.state.snail_cage_frame[(num28) as usize] = 19;
            } else if ctx.state.snail_cage_frame[(num28) as usize] == 1 && ctx.g.main.rand.next(3) == 0 {
                ctx.state.snail_cage_frame[(num28) as usize] = 20;
            } else {
                ctx.state.snail_cage_frame[(num28) as usize] += 1;
                if ctx.state.snail_cage_frame[(num28) as usize] > 13 {
                    ctx.state.snail_cage_frame[(num28) as usize] = 0;
                }
            }
            ctx.state.snail_cage_frame_counter[(num28) as usize] = 0;
        } else if ctx.state.snail_cage_frame[(num28) as usize] >= 14 && ctx.state.snail_cage_frame[(num28) as usize] <= 18 {
            ctx.state.snail_cage_frame_counter[(num28) as usize] += 1;
            if ctx.state.snail_cage_frame_counter[(num28) as usize] >= 5 {
                ctx.state.snail_cage_frame_counter[(num28) as usize] = 0;
                ctx.state.snail_cage_frame[(num28) as usize] += 1;
            }
            if ctx.state.snail_cage_frame[(num28) as usize] > 18 {
                ctx.state.snail_cage_frame[(num28) as usize] = 20;
            }
        } else {
            if ctx.state.snail_cage_frame[(num28) as usize] != 19 && ctx.state.snail_cage_frame[(num28) as usize] != 20 {
                continue;
            }
            ctx.state.snail_cage_frame_counter[(num28) as usize] += 1;
            if ctx.state.snail_cage_frame_counter[(num28) as usize] <= ctx.g.main.rand.next_range(60, 7200) {
                continue;
            }
            ctx.state.snail_cage_frame_counter[(num28) as usize] = 0;
            if ctx.g.main.rand.next(4) == 0 {
                if ctx.g.main.rand.next(3) == 0 {
                    ctx.state.snail_cage_frame[(num28) as usize] = 2;
                } else if ctx.state.snail_cage_frame[(num28) as usize] == 19 {
                    ctx.state.snail_cage_frame[(num28) as usize] = 20;
                } else {
                    ctx.state.snail_cage_frame[(num28) as usize] = 19;
                }
            }
        }
    }
    for num29 in 0..ctx.state.cage_frames {
        if ctx.state.snail2_cage_frame[(num29) as usize] >= 0 && ctx.state.snail2_cage_frame[(num29) as usize] <= 13 {
            ctx.state.snail2_cage_frame_counter[(num29) as usize] += 1;
            if ctx.state.snail2_cage_frame_counter[(num29) as usize] <= ctx.g.main.rand.next_range(30, 2700) {
                continue;
            }
            if ctx.state.snail2_cage_frame[(num29) as usize] == 8 && ctx.g.main.rand.next(2) == 0 {
                ctx.state.snail2_cage_frame[(num29) as usize] = 14;
            } else if ctx.state.snail2_cage_frame[(num29) as usize] == 1 && ctx.g.main.rand.next(3) == 0 {
                ctx.state.snail2_cage_frame[(num29) as usize] = 19;
            } else if ctx.state.snail2_cage_frame[(num29) as usize] == 1 && ctx.g.main.rand.next(3) == 0 {
                ctx.state.snail2_cage_frame[(num29) as usize] = 20;
            } else {
                ctx.state.snail2_cage_frame[(num29) as usize] += 1;
                if ctx.state.snail2_cage_frame[(num29) as usize] > 13 {
                    ctx.state.snail2_cage_frame[(num29) as usize] = 0;
                }
            }
            ctx.state.snail2_cage_frame_counter[(num29) as usize] = 0;
        } else if ctx.state.snail2_cage_frame[(num29) as usize] >= 14 && ctx.state.snail2_cage_frame[(num29) as usize] <= 18 {
            ctx.state.snail2_cage_frame_counter[(num29) as usize] += 1;
            if ctx.state.snail2_cage_frame_counter[(num29) as usize] >= 5 {
                ctx.state.snail2_cage_frame_counter[(num29) as usize] = 0;
                ctx.state.snail2_cage_frame[(num29) as usize] += 1;
            }
            if ctx.state.snail2_cage_frame[(num29) as usize] > 18 {
                ctx.state.snail2_cage_frame[(num29) as usize] = 20;
            }
        } else {
            if ctx.state.snail2_cage_frame[(num29) as usize] != 19 && ctx.state.snail2_cage_frame[(num29) as usize] != 20 {
                continue;
            }
            ctx.state.snail2_cage_frame_counter[(num29) as usize] += 1;
            if ctx.state.snail2_cage_frame_counter[(num29) as usize] <= ctx.g.main.rand.next_range(45, 5400) {
                continue;
            }
            ctx.state.snail2_cage_frame_counter[(num29) as usize] = 0;
            if ctx.g.main.rand.next(4) == 0 {
                if ctx.g.main.rand.next(3) == 0 {
                    ctx.state.snail2_cage_frame[(num29) as usize] = 2;
                } else if ctx.state.snail2_cage_frame[(num29) as usize] == 19 {
                    ctx.state.snail2_cage_frame[(num29) as usize] = 20;
                } else {
                    ctx.state.snail2_cage_frame[(num29) as usize] = 19;
                }
            }
        }
    }
    for num30 in 0..ctx.state.cage_frames {
        if ctx.state.frog_cage_frame[(num30) as usize] == 0 {
            ctx.state.frog_cage_frame_counter[(num30) as usize] += 1;
            if ctx.state.frog_cage_frame_counter[(num30) as usize] > ctx.g.main.rand.next_range(45, 3600) {
                if ctx.g.main.rand.next(10) == 0 {
                    ctx.state.frog_cage_frame[(num30) as usize] = 1;
                } else {
                    ctx.state.frog_cage_frame[(num30) as usize] = 12;
                }
                ctx.state.frog_cage_frame_counter[(num30) as usize] = 0;
            }
        } else if ctx.state.frog_cage_frame[(num30) as usize] >= 1 && ctx.state.frog_cage_frame[(num30) as usize] <= 5 {
            ctx.state.frog_cage_frame_counter[(num30) as usize] += 1;
            if ctx.state.frog_cage_frame_counter[(num30) as usize] >= 5 {
                ctx.state.frog_cage_frame[(num30) as usize] += 1;
                ctx.state.frog_cage_frame_counter[(num30) as usize] = 0;
            }
        } else if ctx.state.frog_cage_frame[(num30) as usize] >= 12 && ctx.state.frog_cage_frame[(num30) as usize] <= 17 {
            ctx.state.frog_cage_frame_counter[(num30) as usize] += 1;
            if ctx.state.frog_cage_frame_counter[(num30) as usize] >= 5 {
                ctx.state.frog_cage_frame_counter[(num30) as usize] = 0;
                ctx.state.frog_cage_frame[(num30) as usize] += 1;
            }
            if ctx.state.frog_cage_frame[(num30) as usize] > 17 {
                if ctx.g.main.rand.next(3) == 0 {
                    ctx.state.frog_cage_frame[(num30) as usize] = 0;
                } else {
                    ctx.state.frog_cage_frame[(num30) as usize] = 12;
                }
            }
        } else if ctx.state.frog_cage_frame[(num30) as usize] == 6 {
            ctx.state.frog_cage_frame_counter[(num30) as usize] += 1;
            if ctx.state.frog_cage_frame_counter[(num30) as usize] > ctx.g.main.rand.next_range(45, 3600) {
                if ctx.g.main.rand.next(10) == 0 {
                    ctx.state.frog_cage_frame[(num30) as usize] = 7;
                } else {
                    ctx.state.frog_cage_frame[(num30) as usize] = 18;
                }
                ctx.state.frog_cage_frame_counter[(num30) as usize] = 0;
            }
        } else if ctx.state.frog_cage_frame[(num30) as usize] >= 7 && ctx.state.frog_cage_frame[(num30) as usize] <= 11 {
            ctx.state.frog_cage_frame_counter[(num30) as usize] += 1;
            if ctx.state.frog_cage_frame_counter[(num30) as usize] >= 5 {
                ctx.state.frog_cage_frame[(num30) as usize] += 1;
                ctx.state.frog_cage_frame_counter[(num30) as usize] = 0;
                if ctx.state.frog_cage_frame[(num30) as usize] > 11 {
                    ctx.state.frog_cage_frame[(num30) as usize] = 0;
                }
            }
        } else {
            if ctx.state.frog_cage_frame[(num30) as usize] < 18 || ctx.state.frog_cage_frame[(num30) as usize] > 23 {
                continue;
            }
            ctx.state.frog_cage_frame_counter[(num30) as usize] += 1;
            if ctx.state.frog_cage_frame_counter[(num30) as usize] >= 5 {
                ctx.state.frog_cage_frame_counter[(num30) as usize] = 0;
                ctx.state.frog_cage_frame[(num30) as usize] += 1;
            }
            if ctx.state.frog_cage_frame[(num30) as usize] > 17 {
                if ctx.g.main.rand.next(3) == 0 {
                    ctx.state.frog_cage_frame[(num30) as usize] = 6;
                } else {
                    ctx.state.frog_cage_frame[(num30) as usize] = 18;
                }
            }
        }
    }
    for num31 in 0..ctx.state.cage_frames {
        if ctx.state.mouse_cage_frame[(num31) as usize] >= 0 && ctx.state.mouse_cage_frame[(num31) as usize] <= 1 {
            ctx.state.mouse_cage_frame_counter[(num31) as usize] += 1;
            if ctx.state.mouse_cage_frame_counter[(num31) as usize] >= 5 {
                ctx.state.mouse_cage_frame[(num31) as usize] += 1;
                if ctx.state.mouse_cage_frame[(num31) as usize] > 1 {
                    ctx.state.mouse_cage_frame[(num31) as usize] = 0;
                }
                ctx.state.mouse_cage_frame_counter[(num31) as usize] = 0;
                if ctx.g.main.rand.next(15) == 0 {
                    ctx.state.mouse_cage_frame[(num31) as usize] = 4;
                }
            }
        } else if ctx.state.mouse_cage_frame[(num31) as usize] >= 4 && ctx.state.mouse_cage_frame[(num31) as usize] <= 7 {
            ctx.state.mouse_cage_frame_counter[(num31) as usize] += 1;
            if ctx.state.mouse_cage_frame_counter[(num31) as usize] >= 5 {
                ctx.state.mouse_cage_frame_counter[(num31) as usize] = 0;
                ctx.state.mouse_cage_frame[(num31) as usize] += 1;
            }
            if ctx.state.mouse_cage_frame[(num31) as usize] > 7 {
                ctx.state.mouse_cage_frame[(num31) as usize] = 2;
            }
        } else if ctx.state.mouse_cage_frame[(num31) as usize] >= 2 && ctx.state.mouse_cage_frame[(num31) as usize] <= 3 {
            ctx.state.mouse_cage_frame_counter[(num31) as usize] += 1;
            if ctx.state.mouse_cage_frame_counter[(num31) as usize] >= 5 {
                ctx.state.mouse_cage_frame[(num31) as usize] += 1;
                if ctx.state.mouse_cage_frame[(num31) as usize] > 3 {
                    ctx.state.mouse_cage_frame[(num31) as usize] = 2;
                }
                ctx.state.mouse_cage_frame_counter[(num31) as usize] = 0;
                if ctx.g.main.rand.next(15) == 0 {
                    ctx.state.mouse_cage_frame[(num31) as usize] = 8;
                } else if ctx.g.main.rand.next(15) == 0 {
                    ctx.state.mouse_cage_frame[(num31) as usize] = 12;
                }
            }
        } else if ctx.state.mouse_cage_frame[(num31) as usize] >= 8 && ctx.state.mouse_cage_frame[(num31) as usize] <= 11 {
            ctx.state.mouse_cage_frame_counter[(num31) as usize] += 1;
            if ctx.state.mouse_cage_frame_counter[(num31) as usize] >= 5 {
                ctx.state.mouse_cage_frame_counter[(num31) as usize] = 0;
                ctx.state.mouse_cage_frame[(num31) as usize] += 1;
            }
            if ctx.state.mouse_cage_frame[(num31) as usize] > 11 {
                ctx.state.mouse_cage_frame[(num31) as usize] = 0;
            }
        } else if ctx.state.mouse_cage_frame[(num31) as usize] >= 12 && ctx.state.mouse_cage_frame[(num31) as usize] <= 13 {
            ctx.state.mouse_cage_frame_counter[(num31) as usize] += 1;
            if ctx.state.mouse_cage_frame_counter[(num31) as usize] >= 5 {
                ctx.state.mouse_cage_frame_counter[(num31) as usize] = 0;
                ctx.state.mouse_cage_frame[(num31) as usize] += 1;
            }
        } else if ctx.state.mouse_cage_frame[(num31) as usize] >= 14 && ctx.state.mouse_cage_frame[(num31) as usize] <= 17 {
            ctx.state.mouse_cage_frame_counter[(num31) as usize] += 1;
            if ctx.state.mouse_cage_frame_counter[(num31) as usize] >= 5 {
                ctx.state.mouse_cage_frame_counter[(num31) as usize] = 0;
                ctx.state.mouse_cage_frame[(num31) as usize] += 1;
                if ctx.state.mouse_cage_frame[(num31) as usize] > 17 && ctx.g.main.rand.next(20) != 0 {
                    ctx.state.mouse_cage_frame[(num31) as usize] = 14;
                }
            }
        } else {
            if ctx.state.mouse_cage_frame[(num31) as usize] < 18 || ctx.state.mouse_cage_frame[(num31) as usize] > 19 {
                continue;
            }
            ctx.state.mouse_cage_frame_counter[(num31) as usize] += 1;
            if ctx.state.mouse_cage_frame_counter[(num31) as usize] >= 5 {
                ctx.state.mouse_cage_frame_counter[(num31) as usize] = 0;
                ctx.state.mouse_cage_frame[(num31) as usize] += 1;
                if ctx.state.mouse_cage_frame[(num31) as usize] > 19 {
                    ctx.state.mouse_cage_frame[(num31) as usize] = 0;
                }
            }
        }
    }
    for num32 in 0..ctx.state.cage_frames {
        ctx.state.worm_cage_frame_counter[(num32) as usize] += 1;
        if ctx.state.worm_cage_frame_counter[(num32) as usize] < ctx.g.main.rand.next_range(30, 91) {
            continue;
        }
        ctx.state.worm_cage_frame_counter[(num32) as usize] = 0;
        if ctx.g.main.rand.next(4) != 0 {
            continue;
        }
        ctx.state.worm_cage_frame[(num32) as usize] += 1;
        if ctx.state.worm_cage_frame[(num32) as usize] == 9 && ctx.g.main.rand.next(2) == 0 {
            ctx.state.worm_cage_frame[(num32) as usize] = 0;
        }
        if ctx.state.worm_cage_frame[(num32) as usize] > 18 {
            if ctx.g.main.rand.next(2) == 0 {
                ctx.state.worm_cage_frame[(num32) as usize] = 9;
            } else {
                ctx.state.worm_cage_frame[(num32) as usize] = 0;
            }
        }
    }
    let mut num33: i32 = 0;
    for num34 in 0..3 {
        match num34 {
            0 => {
                num33 = 24;
            }
            1 => {
                num33 = 31;
            }
            2 => {
                num33 = 34;
            }
            _ => {}
        }
        for num35 in 0..ctx.state.cage_frames {
            if { ctx.state.slug_cage_frame_counter[(num34) as usize][(num35) as usize] += 1; ctx.state.slug_cage_frame_counter[(num34) as usize][(num35) as usize] } >= ctx.g.main.rand.next_range(5, 15) {
                ctx.state.slug_cage_frame_counter[(num34) as usize][(num35) as usize] = 0;
                if { ctx.state.slug_cage_frame[(num34) as usize][(num35) as usize] += 1; ctx.state.slug_cage_frame[(num34) as usize][(num35) as usize] } >= num33 {
                    ctx.state.slug_cage_frame[(num34) as usize][(num35) as usize] = 0;
                }
            }
        }
    }
    for num36 in 0..ctx.state.cage_frames {
        if ctx.state.grasshopper_cage_frame[(num36) as usize] >= 0 && ctx.state.grasshopper_cage_frame[(num36) as usize] <= 1 {
            ctx.state.grasshopper_cage_frame_counter[(num36) as usize] += 1;
            if ctx.state.grasshopper_cage_frame_counter[(num36) as usize] >= 5 {
                ctx.state.grasshopper_cage_frame[(num36) as usize] += 1;
                if ctx.state.grasshopper_cage_frame[(num36) as usize] > 1 {
                    ctx.state.grasshopper_cage_frame[(num36) as usize] = 0;
                }
                ctx.state.grasshopper_cage_frame_counter[(num36) as usize] = 0;
                if ctx.g.main.rand.next(15) == 0 {
                    ctx.state.grasshopper_cage_frame[(num36) as usize] = 2;
                }
            }
        } else if ctx.state.grasshopper_cage_frame[(num36) as usize] >= 2 && ctx.state.grasshopper_cage_frame[(num36) as usize] <= 5 {
            ctx.state.grasshopper_cage_frame_counter[(num36) as usize] += 1;
            if ctx.state.grasshopper_cage_frame_counter[(num36) as usize] >= 5 {
                ctx.state.grasshopper_cage_frame_counter[(num36) as usize] = 0;
                ctx.state.grasshopper_cage_frame[(num36) as usize] += 1;
            }
            if ctx.state.grasshopper_cage_frame[(num36) as usize] > 5 {
                ctx.state.grasshopper_cage_frame[(num36) as usize] = 6;
            }
        } else if ctx.state.grasshopper_cage_frame[(num36) as usize] >= 6 && ctx.state.grasshopper_cage_frame[(num36) as usize] <= 7 {
            ctx.state.grasshopper_cage_frame_counter[(num36) as usize] += 1;
            if ctx.state.grasshopper_cage_frame_counter[(num36) as usize] >= 5 {
                ctx.state.grasshopper_cage_frame[(num36) as usize] += 1;
                if ctx.state.grasshopper_cage_frame[(num36) as usize] > 7 {
                    ctx.state.grasshopper_cage_frame[(num36) as usize] = 6;
                }
                ctx.state.grasshopper_cage_frame_counter[(num36) as usize] = 0;
                if ctx.g.main.rand.next(15) == 0 {
                    ctx.state.grasshopper_cage_frame[(num36) as usize] = 8;
                }
            }
        } else if ctx.state.grasshopper_cage_frame[(num36) as usize] >= 8 && ctx.state.grasshopper_cage_frame[(num36) as usize] <= 11 {
            ctx.state.grasshopper_cage_frame_counter[(num36) as usize] += 1;
            if ctx.state.grasshopper_cage_frame_counter[(num36) as usize] >= 5 {
                ctx.state.grasshopper_cage_frame_counter[(num36) as usize] = 0;
                ctx.state.grasshopper_cage_frame[(num36) as usize] += 1;
            }
            if ctx.state.grasshopper_cage_frame[(num36) as usize] > 11 {
                ctx.state.grasshopper_cage_frame[(num36) as usize] = 0;
            }
        }
    }
    for num37 in 0..ctx.state.cage_frames {
        ctx.state.maggot_cage_frame_counter[(num37) as usize] += 1;
        let mut num38: bool = ctx.state.maggot_cage_frame[(num37) as usize] == 0 || ctx.state.maggot_cage_frame[(num37) as usize] == 6 || ctx.state.maggot_cage_frame[(num37) as usize] == 18;
        let mut num39: i32 = 8;
        if num38 {
            num39 = ctx.g.main.rand.next_range(100, 140);
        }
        if ctx.state.maggot_cage_frame_counter[(num37) as usize] < num39 {
            continue;
        }
        ctx.state.maggot_cage_frame_counter[(num37) as usize] = 0;
        if ctx.state.maggot_cage_frame[(num37) as usize] == 0 {
            if ctx.g.main.rand.next(3) == 0 {
                ctx.state.maggot_cage_frame[(num37) as usize] = 13;
            } else {
                ctx.state.maggot_cage_frame[(num37) as usize] = 1;
            }
            continue;
        }
        if ctx.state.maggot_cage_frame[(num37) as usize] == 12 {
            ctx.state.maggot_cage_frame[(num37) as usize] = 0;
            continue;
        }
        ctx.state.maggot_cage_frame[(num37) as usize] += 1;
        if ctx.state.maggot_cage_frame[(num37) as usize] > 23 {
            ctx.state.maggot_cage_frame[(num37) as usize] = 6;
        }
    }
    for num40 in 0..ctx.state.cage_frames {
        ctx.state.ladybug_cage_frame_counter[(num40) as usize] += 1;
        let mut flag6: bool = ctx.state.ladybug_cage_frame[(num40) as usize] == 3 || ctx.state.ladybug_cage_frame[(num40) as usize] == 7 || ctx.state.ladybug_cage_frame[(num40) as usize] >= 17;
        let mut num41: i32 = ctx.g.main.rand.next_range(10, 12);
        if ctx.state.ladybug_cage_frame[(num40) as usize] == 18 {
            num41 = ctx.g.main.rand.next_range(160, 241);
        } else if flag6 {
            num41 = ctx.g.main.rand.next_range(198, 206);
        } else if ctx.state.ladybug_cage_frame[(num40) as usize] >= 8 && ctx.state.ladybug_cage_frame[(num40) as usize] <= 16 {
            num41 = 5;
        }
        if ctx.state.ladybug_cage_frame_counter[(num40) as usize] < num41 {
            continue;
        }
        ctx.state.ladybug_cage_frame_counter[(num40) as usize] = 0;
        if ctx.state.ladybug_cage_frame[(num40) as usize] < 18 {
            if (ctx.state.ladybug_cage_frame[(num40) as usize] == 2 || ctx.state.ladybug_cage_frame[(num40) as usize] == 5) && ctx.g.main.rand.next(3) == 0 {
                ctx.state.ladybug_cage_frame[(num40) as usize] = 17;
            } else if ctx.state.ladybug_cage_frame[(num40) as usize] == 3 || ctx.state.ladybug_cage_frame[(num40) as usize] == 12 {
                match ctx.g.main.rand.next(3) {
                    0 | 1 => {
                        ctx.state.ladybug_cage_frame[(num40) as usize] = 4;
                    }
                    2 => {
                        ctx.state.ladybug_cage_frame[(num40) as usize] = 13;
                    }
                    _ => {}
                }
            } else if ctx.state.ladybug_cage_frame[(num40) as usize] == 7 || ctx.state.ladybug_cage_frame[(num40) as usize] == 16 {
                match ctx.g.main.rand.next(3) {
                    0 | 1 => {
                        ctx.state.ladybug_cage_frame[(num40) as usize] = 0;
                    }
                    2 => {
                        ctx.state.ladybug_cage_frame[(num40) as usize] = 8;
                    }
                    _ => {}
                }
            } else {
                ctx.state.ladybug_cage_frame[(num40) as usize] += 1;
            }
        } else {
            ctx.state.ladybug_cage_frame[(num40) as usize] = (if ctx.g.main.rand.next(2) == 0 { 13 } else { 4 });
        }
    }
    for num42 in 0..ctx.state.cage_frames {
        ctx.state.rat_cage_frame_counter[(num42) as usize] += 1;
        let mut flag7: bool = ctx.state.rat_cage_frame[(num42) as usize] == 0 || ctx.state.rat_cage_frame[(num42) as usize] == 4;
        let mut num43: bool = ctx.state.rat_cage_frame[(num42) as usize] == 8 || ctx.state.rat_cage_frame[(num42) as usize] == 9 || ctx.state.rat_cage_frame[(num42) as usize] == 10 || ctx.state.rat_cage_frame[(num42) as usize] == 11;
        let mut flag8: bool = ctx.state.rat_cage_frame[(num42) as usize] > 11;
        let mut num44: i32 = 5;
        if flag7 {
            num44 = ctx.g.main.rand.next_range(40, 70);
        }
        if num43 {
            num44 = ctx.g.main.rand.next_range(70, 110);
        }
        if flag8 {
            num44 = 180;
        }
        if ctx.state.rat_cage_frame_counter[(num42) as usize] >= num44 {
            ctx.state.rat_cage_frame_counter[(num42) as usize] = 0;
            if ctx.state.rat_cage_frame[(num42) as usize] == 0 && ctx.g.main.rand.next(2) == 0 {
                if ctx.g.main.rand.next(3) != 0 {
                    ctx.state.rat_cage_frame[(num42) as usize] = 12;
                } else {
                    ctx.state.rat_cage_frame[(num42) as usize] = 8;
                }
                continue;
            }
            if ctx.state.rat_cage_frame[(num42) as usize] == 4 && ctx.g.main.rand.next(2) == 0 {
                ctx.state.rat_cage_frame[(num42) as usize] = 10;
                continue;
            }
            if ctx.state.rat_cage_frame[(num42) as usize] == 7 || ctx.state.rat_cage_frame[(num42) as usize] == 9 || ctx.state.rat_cage_frame[(num42) as usize] == 12 {
                ctx.state.rat_cage_frame[(num42) as usize] = 0;
                continue;
            }
            if ctx.state.rat_cage_frame[(num42) as usize] == 11 {
                ctx.state.rat_cage_frame[(num42) as usize] = 4;
                continue;
            }
            ctx.state.rat_cage_frame[(num42) as usize] += 1;
            if ctx.state.rat_cage_frame[(num42) as usize] > 16 {
                ctx.state.rat_cage_frame[(num42) as usize] = 0;
            }
        } else if ctx.state.rat_cage_frame[(num42) as usize] > 11 {
            let mut num45: i32 = ctx.state.rat_cage_frame_counter[(num42) as usize] % 90;
            if num45 > 80 {
                ctx.state.rat_cage_frame[(num42) as usize] = 12;
            } else if num45 > 70 {
                ctx.state.rat_cage_frame[(num42) as usize] = 13;
            } else if num45 > 60 {
                ctx.state.rat_cage_frame[(num42) as usize] = 14;
            } else if num45 > 50 {
                ctx.state.rat_cage_frame[(num42) as usize] = 15;
            } else if num45 > 40 {
                ctx.state.rat_cage_frame[(num42) as usize] = 16;
            } else if num45 > 30 {
                ctx.state.rat_cage_frame[(num42) as usize] = 15;
            } else if num45 > 20 {
                ctx.state.rat_cage_frame[(num42) as usize] = 14;
            } else if num45 > 10 {
                ctx.state.rat_cage_frame[(num42) as usize] = 13;
            } else {
                ctx.state.rat_cage_frame[(num42) as usize] = 12;
            }
        } else if ctx.state.rat_cage_frame_counter[(num42) as usize] % 10 == 0 {
            if ctx.state.rat_cage_frame[(num42) as usize] == 8 || ctx.state.rat_cage_frame[(num42) as usize] == 10 {
                ctx.state.rat_cage_frame[(num42) as usize] += 1;
            } else if ctx.state.rat_cage_frame[(num42) as usize] == 9 || ctx.state.rat_cage_frame[(num42) as usize] == 11 {
                ctx.state.rat_cage_frame[(num42) as usize] -= 1;
            }
        }
    }
    for num46 in 0..ctx.state.cage_frames {
        ctx.state.water_strider_cage_frame_counter[(num46) as usize] += 1;
        let mut flag9: bool = ctx.state.water_strider_cage_frame[(num46) as usize] == 0 || ctx.state.water_strider_cage_frame[(num46) as usize] == 8;
        let mut num47: bool = ctx.state.water_strider_cage_frame[(num46) as usize] == 17 || ctx.state.water_strider_cage_frame[(num46) as usize] == 20;
        let mut num48: i32 = 5;
        if flag9 {
            num48 = ctx.g.main.rand.next_range(110, 210);
        }
        if num47 {
            num48 = ctx.g.main.rand.next_range(150, 260);
        }
        if ctx.state.water_strider_cage_frame_counter[(num46) as usize] < num48 {
            continue;
        }
        ctx.state.water_strider_cage_frame_counter[(num46) as usize] = 0;
        if ctx.state.water_strider_cage_frame[(num46) as usize] == 0 && ctx.g.main.rand.next(2) == 0 {
            ctx.state.water_strider_cage_frame[(num46) as usize] = 16;
            continue;
        }
        if ctx.state.water_strider_cage_frame[(num46) as usize] == 8 && ctx.g.main.rand.next(2) == 0 {
            ctx.state.water_strider_cage_frame[(num46) as usize] = 19;
            continue;
        }
        if ctx.state.water_strider_cage_frame[(num46) as usize] == 15 || ctx.state.water_strider_cage_frame[(num46) as usize] == 18 {
            ctx.state.water_strider_cage_frame[(num46) as usize] = 0;
            continue;
        }
        ctx.state.water_strider_cage_frame[(num46) as usize] += 1;
        if ctx.state.water_strider_cage_frame[(num46) as usize] > 21 {
            ctx.state.water_strider_cage_frame[(num46) as usize] = 8;
        }
    }
    ctx.state.fairy_jar_frame_counter[(0) as usize] += 1;
    if ctx.state.fairy_jar_frame_counter[0] > 5 {
        ctx.state.fairy_jar_frame_counter[(0) as usize] = 0;
        ctx.state.fairy_jar_frame[(0) as usize] += 1;
        if ctx.state.fairy_jar_frame[0] > 11 {
            ctx.state.fairy_jar_frame[(0) as usize] = 0;
        }
    }
    for num49 in 1..ctx.state.cage_frames {
        let mut num50: i32 = ctx.state.fairy_jar_frame[0] + num49 % 12;
        if num49 % 2 == 0 {
            num50 += 1;
        }
        if num49 % 5 == 0 {
            num50 += 1;
        }
        if num49 % 8 == 0 {
            num50 += 1;
        }
        ctx.state.fairy_jar_frame[(num49) as usize] = num50;
    }
    for num51 in 0..ctx.state.cage_frames {
        let mut max_value: u8 = 5;
        if (ctx.state.fish_bowl_frame_mode[(num51) as usize] as i32) == 1 {
            if ctx.g.main.rand.next(900) == 0 {
                ctx.state.fish_bowl_frame_mode[(num51) as usize] = ((ctx.g.main.rand.next(max_value as i32) as u8)) as u8;
            }
            ctx.state.fish_bowl_frame_counter[(num51) as usize] += 1;
            if ctx.state.fish_bowl_frame_counter[(num51) as usize] < 5 {
                continue;
            }
            ctx.state.fish_bowl_frame_counter[(num51) as usize] = 0;
            if ctx.state.fish_bowl_frame[(num51) as usize] == 10 {
                if ctx.g.main.rand.next(20) == 0 {
                    ctx.state.fish_bowl_frame[(num51) as usize] = 11;
                    ctx.state.fish_bowl_frame_mode[(num51) as usize] = (0) as u8;
                } else {
                    ctx.state.fish_bowl_frame[(num51) as usize] = 1;
                }
            } else {
                ctx.state.fish_bowl_frame[(num51) as usize] += 1;
            }
        } else if (ctx.state.fish_bowl_frame_mode[(num51) as usize] as i32) == 2 {
            if ctx.g.main.rand.next(3600) == 0 {
                ctx.state.fish_bowl_frame_mode[(num51) as usize] = ((ctx.g.main.rand.next(max_value as i32) as u8)) as u8;
            }
            ctx.state.fish_bowl_frame_counter[(num51) as usize] += 1;
            if ctx.state.fish_bowl_frame_counter[(num51) as usize] < 20 {
                continue;
            }
            ctx.state.fish_bowl_frame_counter[(num51) as usize] = 0;
            if ctx.state.fish_bowl_frame[(num51) as usize] == 10 {
                if ctx.g.main.rand.next(20) == 0 {
                    ctx.state.fish_bowl_frame[(num51) as usize] = 11;
                    ctx.state.fish_bowl_frame_mode[(num51) as usize] = (0) as u8;
                } else {
                    ctx.state.fish_bowl_frame[(num51) as usize] = 1;
                }
            } else {
                ctx.state.fish_bowl_frame[(num51) as usize] += 1;
            }
        } else if (ctx.state.fish_bowl_frame_mode[(num51) as usize] as i32) == 3 {
            if ctx.g.main.rand.next(3600) == 0 {
                ctx.state.fish_bowl_frame_mode[(num51) as usize] = ((ctx.g.main.rand.next(max_value as i32) as u8)) as u8;
            }
            ctx.state.fish_bowl_frame_counter[(num51) as usize] += 1;
            if ctx.state.fish_bowl_frame_counter[(num51) as usize] < ctx.g.main.rand.next_range(5, 3600) {
                continue;
            }
            ctx.state.fish_bowl_frame_counter[(num51) as usize] = 0;
            if ctx.state.fish_bowl_frame[(num51) as usize] == 10 {
                if ctx.g.main.rand.next(20) == 0 {
                    ctx.state.fish_bowl_frame[(num51) as usize] = 11;
                    ctx.state.fish_bowl_frame_mode[(num51) as usize] = (0) as u8;
                } else {
                    ctx.state.fish_bowl_frame[(num51) as usize] = 1;
                }
            } else {
                ctx.state.fish_bowl_frame[(num51) as usize] += 1;
            }
        } else if ctx.state.fish_bowl_frame[(num51) as usize] <= 10 {
            if ctx.g.main.rand.next(3600) == 0 {
                ctx.state.fish_bowl_frame_mode[(num51) as usize] = ((ctx.g.main.rand.next(max_value as i32) as u8)) as u8;
            }
            ctx.state.fish_bowl_frame_counter[(num51) as usize] += 1;
            if ctx.state.fish_bowl_frame_counter[(num51) as usize] < 10 {
                continue;
            }
            ctx.state.fish_bowl_frame_counter[(num51) as usize] = 0;
            if ctx.state.fish_bowl_frame[(num51) as usize] == 10 {
                if ctx.g.main.rand.next(12) == 0 {
                    ctx.state.fish_bowl_frame[(num51) as usize] = 11;
                } else {
                    ctx.state.fish_bowl_frame[(num51) as usize] = 1;
                }
            } else {
                ctx.state.fish_bowl_frame[(num51) as usize] += 1;
            }
        } else if ctx.state.fish_bowl_frame[(num51) as usize] == 12 || ctx.state.fish_bowl_frame[(num51) as usize] == 13 {
            ctx.state.fish_bowl_frame_counter[(num51) as usize] += 1;
            if ctx.state.fish_bowl_frame_counter[(num51) as usize] < 10 {
                continue;
            }
            ctx.state.fish_bowl_frame_counter[(num51) as usize] = 0;
            ctx.state.fish_bowl_frame[(num51) as usize] += 1;
            if ctx.state.fish_bowl_frame[(num51) as usize] > 13 {
                if ctx.g.main.rand.next(20) == 0 {
                    ctx.state.fish_bowl_frame[(num51) as usize] = 14;
                } else {
                    ctx.state.fish_bowl_frame[(num51) as usize] = 12;
                }
            }
        } else {
            if ctx.state.fish_bowl_frame[(num51) as usize] < 11 {
                continue;
            }
            ctx.state.fish_bowl_frame_counter[(num51) as usize] += 1;
            if ctx.state.fish_bowl_frame_counter[(num51) as usize] >= 10 {
                ctx.state.fish_bowl_frame_counter[(num51) as usize] = 0;
                ctx.state.fish_bowl_frame[(num51) as usize] += 1;
                if ctx.state.fish_bowl_frame[(num51) as usize] > 16 {
                    ctx.state.fish_bowl_frame[(num51) as usize] = 4;
                }
            }
        }
    }
    for num52 in 0..ctx.state.cage_frames {
        ctx.state.lava_fish_bowl_frame_counter[(num52) as usize] += 1;
        if ctx.state.lava_fish_bowl_frame_counter[(num52) as usize] > 4 + ctx.g.main.rand.next(3) {
            ctx.state.lava_fish_bowl_frame_counter[(num52) as usize] = 0;
            ctx.state.lava_fish_bowl_frame[(num52) as usize] += 1;
            if ctx.state.lava_fish_bowl_frame[(num52) as usize] > 26 {
                ctx.state.lava_fish_bowl_frame[(num52) as usize] = 0;
            }
        }
    }
    for num53 in 0..9 {
        for num54 in 0..ctx.state.cage_frames {
            ctx.state.butterfly_cage_frame_counter[(num53) as usize][(num54) as usize] += 1;
            if ctx.g.main.rand.next(3600) == 0 {
                ctx.state.butterfly_cage_mode[(num53) as usize][(num54) as usize] = ((ctx.g.main.rand.next(5) as u8)) as u8;
                if ctx.g.main.rand.next(2) == 0 {
                    ctx.state.butterfly_cage_mode[(num53) as usize][(num54) as usize] = ((ctx.state.butterfly_cage_mode[(num53) as usize][(num54) as usize] as i32) + (10)) as u8;
                }
            }
            let mut num55: i32 = ctx.g.main.rand.next_range(3, 16);
            if (ctx.state.butterfly_cage_mode[(num53) as usize][(num54) as usize] as i32) == 1 || (ctx.state.butterfly_cage_mode[(num53) as usize][(num54) as usize] as i32) == 11 {
                num55 = 3;
            }
            if (ctx.state.butterfly_cage_mode[(num53) as usize][(num54) as usize] as i32) == 2 || (ctx.state.butterfly_cage_mode[(num53) as usize][(num54) as usize] as i32) == 12 {
                num55 = 5;
            }
            if (ctx.state.butterfly_cage_mode[(num53) as usize][(num54) as usize] as i32) == 3 || (ctx.state.butterfly_cage_mode[(num53) as usize][(num54) as usize] as i32) == 13 {
                num55 = 10;
            }
            if (ctx.state.butterfly_cage_mode[(num53) as usize][(num54) as usize] as i32) == 4 || (ctx.state.butterfly_cage_mode[(num53) as usize][(num54) as usize] as i32) == 14 {
                num55 = 15;
            }
            if (ctx.state.butterfly_cage_mode[(num53) as usize][(num54) as usize] as i32) >= 10 {
                if ctx.state.butterfly_cage_frame[(num53) as usize][(num54) as usize] <= 7 {
                    if ctx.state.butterfly_cage_frame_counter[(num53) as usize][(num54) as usize] < num55 {
                        continue;
                    }
                    ctx.state.butterfly_cage_frame_counter[(num53) as usize][(num54) as usize] = 0;
                    ctx.state.butterfly_cage_frame[(num53) as usize][(num54) as usize] -= 1;
                    if ctx.state.butterfly_cage_frame[(num53) as usize][(num54) as usize] < 0 {
                        ctx.state.butterfly_cage_frame[(num53) as usize][(num54) as usize] = 7;
                    }
                    if ctx.state.butterfly_cage_frame[(num53) as usize][(num54) as usize] != 1 && ctx.state.butterfly_cage_frame[(num53) as usize][(num54) as usize] != 4 && ctx.state.butterfly_cage_frame[(num53) as usize][(num54) as usize] != 6 {
                        continue;
                    }
                    if ctx.g.main.rand.next(20) == 0 {
                        ctx.state.butterfly_cage_frame[(num53) as usize][(num54) as usize] += 8;
                    } else if ctx.g.main.rand.next(6) == 0 {
                        if (ctx.state.butterfly_cage_mode[(num53) as usize][(num54) as usize] as i32) >= 10 {
                            ctx.state.butterfly_cage_mode[(num53) as usize][(num54) as usize] = ((ctx.state.butterfly_cage_mode[(num53) as usize][(num54) as usize] as i32) - (10)) as u8;
                        } else {
                            ctx.state.butterfly_cage_mode[(num53) as usize][(num54) as usize] = ((ctx.state.butterfly_cage_mode[(num53) as usize][(num54) as usize] as i32) + (10)) as u8;
                        }
                    }
                } else {
                    if ctx.state.butterfly_cage_frame_counter[(num53) as usize][(num54) as usize] < num55 {
                        continue;
                    }
                    ctx.state.butterfly_cage_frame_counter[(num53) as usize][(num54) as usize] = 0;
                    ctx.state.butterfly_cage_frame[(num53) as usize][(num54) as usize] -= 1;
                    if ctx.state.butterfly_cage_frame[(num53) as usize][(num54) as usize] < 8 {
                        ctx.state.butterfly_cage_frame[(num53) as usize][(num54) as usize] = 14;
                    }
                    if ctx.state.butterfly_cage_frame[(num53) as usize][(num54) as usize] != 9 && ctx.state.butterfly_cage_frame[(num53) as usize][(num54) as usize] != 12 && ctx.state.butterfly_cage_frame[(num53) as usize][(num54) as usize] != 14 {
                        continue;
                    }
                    if ctx.g.main.rand.next(20) == 0 {
                        ctx.state.butterfly_cage_frame[(num53) as usize][(num54) as usize] -= 8;
                    } else if ctx.g.main.rand.next(6) == 0 {
                        if (ctx.state.butterfly_cage_mode[(num53) as usize][(num54) as usize] as i32) >= 10 {
                            ctx.state.butterfly_cage_mode[(num53) as usize][(num54) as usize] = ((ctx.state.butterfly_cage_mode[(num53) as usize][(num54) as usize] as i32) - (10)) as u8;
                        } else {
                            ctx.state.butterfly_cage_mode[(num53) as usize][(num54) as usize] = ((ctx.state.butterfly_cage_mode[(num53) as usize][(num54) as usize] as i32) + (10)) as u8;
                        }
                    }
                }
            } else if ctx.state.butterfly_cage_frame[(num53) as usize][(num54) as usize] <= 7 {
                if ctx.state.butterfly_cage_frame_counter[(num53) as usize][(num54) as usize] >= num55 {
                    ctx.state.butterfly_cage_frame_counter[(num53) as usize][(num54) as usize] = 0;
                    ctx.state.butterfly_cage_frame[(num53) as usize][(num54) as usize] += 1;
                    if ctx.state.butterfly_cage_frame[(num53) as usize][(num54) as usize] > 7 {
                        ctx.state.butterfly_cage_frame[(num53) as usize][(num54) as usize] = 0;
                    }
                    if (ctx.state.butterfly_cage_frame[(num53) as usize][(num54) as usize] == 1 || ctx.state.butterfly_cage_frame[(num53) as usize][(num54) as usize] == 4 || ctx.state.butterfly_cage_frame[(num53) as usize][(num54) as usize] == 6) && ctx.g.main.rand.next(10) == 0 {
                        ctx.state.butterfly_cage_frame[(num53) as usize][(num54) as usize] += 8;
                    }
                }
            } else if ctx.state.butterfly_cage_frame_counter[(num53) as usize][(num54) as usize] >= num55 {
                ctx.state.butterfly_cage_frame_counter[(num53) as usize][(num54) as usize] = 0;
                ctx.state.butterfly_cage_frame[(num53) as usize][(num54) as usize] += 1;
                if ctx.state.butterfly_cage_frame[(num53) as usize][(num54) as usize] > 15 {
                    ctx.state.butterfly_cage_frame[(num53) as usize][(num54) as usize] = 8;
                }
                if (ctx.state.butterfly_cage_frame[(num53) as usize][(num54) as usize] == 9 || ctx.state.butterfly_cage_frame[(num53) as usize][(num54) as usize] == 12 || ctx.state.butterfly_cage_frame[(num53) as usize][(num54) as usize] == 14) && ctx.g.main.rand.next(10) == 0 {
                    ctx.state.butterfly_cage_frame[(num53) as usize][(num54) as usize] -= 8;
                }
            }
        }
    }
    update_dragonfly_jar_frames(ctx);
    for num56 in 0..3 {
        for num57 in 0..ctx.state.cage_frames {
            ctx.state.jellyfish_cage_frame_counter[(num56) as usize][(num57) as usize] += 1;
            if (ctx.g.main.jellyfish_cage_mode[(num56) as usize][(num57) as usize] as i32) == 0 && ctx.g.main.rand.next(1800) == 0 {
                ctx.g.main.jellyfish_cage_mode[(num56) as usize][(num57) as usize] = (1) as u8;
            }
            if (ctx.g.main.jellyfish_cage_mode[(num56) as usize][(num57) as usize] as i32) == 2 && ctx.g.main.rand.next(60) == 0 {
                ctx.g.main.jellyfish_cage_mode[(num56) as usize][(num57) as usize] = (3) as u8;
            }
            let mut num58: i32 = 1;
            if (ctx.g.main.jellyfish_cage_mode[(num56) as usize][(num57) as usize] as i32) == 0 {
                num58 = ctx.g.main.rand.next_range(10, 20);
            }
            if (ctx.g.main.jellyfish_cage_mode[(num56) as usize][(num57) as usize] as i32) == 1 {
                num58 = ctx.g.main.rand.next_range(15, 25);
            }
            if (ctx.g.main.jellyfish_cage_mode[(num56) as usize][(num57) as usize] as i32) == 2 {
                num58 = ctx.g.main.rand.next_range(4, 9);
            }
            if (ctx.g.main.jellyfish_cage_mode[(num56) as usize][(num57) as usize] as i32) == 3 {
                num58 = ctx.g.main.rand.next_range(15, 25);
            }
            if (ctx.g.main.jellyfish_cage_mode[(num56) as usize][(num57) as usize] as i32) == 0 && ctx.state.jellyfish_cage_frame[(num56) as usize][(num57) as usize] <= 3 && ctx.state.jellyfish_cage_frame_counter[(num56) as usize][(num57) as usize] >= num58 {
                ctx.state.jellyfish_cage_frame_counter[(num56) as usize][(num57) as usize] = 0;
                ctx.state.jellyfish_cage_frame[(num56) as usize][(num57) as usize] += 1;
                if ctx.state.jellyfish_cage_frame[(num56) as usize][(num57) as usize] >= 4 {
                    ctx.state.jellyfish_cage_frame[(num56) as usize][(num57) as usize] = 0;
                }
            }
            if (ctx.g.main.jellyfish_cage_mode[(num56) as usize][(num57) as usize] as i32) == 1 && ctx.state.jellyfish_cage_frame[(num56) as usize][(num57) as usize] <= 7 && ctx.state.jellyfish_cage_frame_counter[(num56) as usize][(num57) as usize] >= num58 {
                ctx.state.jellyfish_cage_frame_counter[(num56) as usize][(num57) as usize] = 0;
                ctx.state.jellyfish_cage_frame[(num56) as usize][(num57) as usize] += 1;
                if ctx.state.jellyfish_cage_frame[(num56) as usize][(num57) as usize] >= 7 {
                    ctx.g.main.jellyfish_cage_mode[(num56) as usize][(num57) as usize] = (2) as u8;
                }
            }
            if (ctx.g.main.jellyfish_cage_mode[(num56) as usize][(num57) as usize] as i32) == 2 && ctx.state.jellyfish_cage_frame[(num56) as usize][(num57) as usize] <= 9 && ctx.state.jellyfish_cage_frame_counter[(num56) as usize][(num57) as usize] >= num58 {
                ctx.state.jellyfish_cage_frame_counter[(num56) as usize][(num57) as usize] = 0;
                ctx.state.jellyfish_cage_frame[(num56) as usize][(num57) as usize] += 1;
                if ctx.state.jellyfish_cage_frame[(num56) as usize][(num57) as usize] >= 9 {
                    ctx.state.jellyfish_cage_frame[(num56) as usize][(num57) as usize] = 7;
                }
            }
            if (ctx.g.main.jellyfish_cage_mode[(num56) as usize][(num57) as usize] as i32) == 3 && ctx.state.jellyfish_cage_frame[(num56) as usize][(num57) as usize] <= 10 && ctx.state.jellyfish_cage_frame_counter[(num56) as usize][(num57) as usize] >= num58 {
                ctx.state.jellyfish_cage_frame_counter[(num56) as usize][(num57) as usize] = 0;
                ctx.state.jellyfish_cage_frame[(num56) as usize][(num57) as usize] += 1;
                if ctx.state.jellyfish_cage_frame[(num56) as usize][(num57) as usize] >= 10 {
                    ctx.state.jellyfish_cage_frame[(num56) as usize][(num57) as usize] = 3;
                    ctx.g.main.jellyfish_cage_mode[(num56) as usize][(num57) as usize] = (0) as u8;
                }
            }
        }
    }
}
