//! Hand-derived tests for the animation counters of `Main.AnimateTiles` / `DoUpdate_AnimateWalls` / the colour pulses (Main.cs).

use crate::draw_context::test_support::with_ctx;
use crate::main_animate::{animate_tiles_critter_cages, do_update_animate};

#[test]
fn tile_frame_counters_step_and_wrap_at_the_listed_limits() {
    with_ctx(|ctx| {
        // Main.cs:18734-18742: tileFrameCounter[12]++; if (> 5) { counter = 0; tileFrame[12]++; if (>= 10) frame = 0 } then 665 copies 12
        for _ in 0..5 {
            do_update_animate(ctx);
        }
        assert_eq!((ctx.state.tile_frame_counter[12], ctx.state.tile_frame[12]), (5, 0));
        do_update_animate(ctx); // the 6th tick: counter 6 > 5
        assert_eq!((ctx.state.tile_frame_counter[12], ctx.state.tile_frame[12]), (0, 1));
        assert_eq!((ctx.state.tile_frame_counter[665], ctx.state.tile_frame[665]), (0, 1));
        // 9 more steps of 6 ticks reach frame 10 -> wraps to 0 (Main.cs:18738-18741)
        for _ in 0..(6 * 9) {
            do_update_animate(ctx);
        }
        assert_eq!(ctx.state.tile_frame[12], 0);
        // tile 31 steps every 8 ticks (Main.cs:18787-18797): after 16 ticks it is on frame 2, and 696 copies it
        assert_eq!(ctx.state.tile_frame[31], (60 / 8) % 4); // 60 ticks so far -> 7 steps -> frame 3
        assert_eq!(ctx.state.tile_frame[696], ctx.state.tile_frame[31]);
    });
}

#[test]
fn wall_frames_step_and_the_colour_pulses_stay_in_range() {
    with_ctx(|ctx| {
        // Main.cs:19549-19557: wallFrameCounter[357]++; if (>= 5) { 0; wallFrame[357]++; if (>= 8) 0 }
        for _ in 0..5 {
            do_update_animate(ctx);
        }
        assert_eq!((ctx.state.wall_frame_counter[357], ctx.state.wall_frame[357]), (0, 1));
        // DoUpdate_AnimateDiscoRGB (Main.cs:19807): style 0 raises G by 7 per tick up to 255 (from 0), then style 1 lowers R from 255.
        // After 5 ticks G = 35; 37 ticks bring it to 255 (36 * 7 = 252 < 255 <= 37 * 7).
        assert_eq!((ctx.g.main.disco_g, ctx.g.main.disco_r), (35, 255));
        for _ in 0..32 {
            do_update_animate(ctx);
        }
        assert_eq!((ctx.g.main.disco_g, ctx.state.disco_style), (255, 1));
        // DoUpdate_AnimateCursorColors (Main.cs:18429): mouseTextColor starts at 0 and rises by 1 (mouseTextColorChange = 1) per tick
        assert_eq!(ctx.g.main.mouse_text_color, 37);
        // DoUpdate_AnimateTileGlows: demonTorch starts at 1 going up, so the first tick clamps it at 1 and turns it round
        assert_eq!(ctx.state.demon_torch_dir, -1);
        assert!(ctx.g.main.demon_torch < 1.0 && ctx.g.main.demon_torch > 0.5);
    });
}

#[test]
fn critter_cages_only_run_while_a_cage_was_drawn() {
    with_ctx(|ctx| {
        // Main.cs:13722: `if (!critterCage) return;`
        for _ in 0..2000 {
            animate_tiles_critter_cages(ctx);
        }
        assert!(ctx.state.bunny_cage_frame.iter().all(|&f| f == 0));
        assert!(ctx.state.bunny_cage_frame_counter.iter().all(|&f| f == 0));
        // with a cage on screen the idle counters count up (bunnyCageFrame == 0 branch, Main.cs:13728-13731)
        ctx.state.critter_cage = true;
        animate_tiles_critter_cages(ctx);
        assert!(ctx.state.bunny_cage_frame_counter.iter().all(|&f| f == 1));
    });
}
