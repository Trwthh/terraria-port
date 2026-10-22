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
//@WALLS@
}

/// `AnimateTiles()` (Main.cs:18732)
fn animate_tiles(ctx: &mut DrawContext) {
//@TILES@
}

/// `AnimateTiles_WeatherVane()` (Main.cs:19522)
fn animate_tiles_weather_vane(ctx: &mut DrawContext) {
//@VANE@
}

/// `AnimateTiles_CritterCages()` (Main.cs:13720)
pub fn animate_tiles_critter_cages(ctx: &mut DrawContext) {
//@CAGES@
}
