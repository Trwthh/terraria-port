//! Hand-derived tests for `GetTileFlameData` (TileDrawing.cs:1870), the furniture flame colours (1830-1868) and the wind cycle
//! (TileDrawing.cs:7814). Values come from the C# source, worked out by hand.

use terraria_core::xna::Color;

use crate::draw_context::test_support::with_ctx;

use super::tile_drawing::TileDrawing;
use super::tile_drawing_specials::{utils_random_int, utils_random_next_seed};

#[test]
fn flame_data_of_a_torch_is_the_default_arm() {
    with_ctx(|ctx| {
        let tiles = TileDrawing::new();
        ctx.g.main.tile_flame[4] = true;
        ctx.state.tile_frame_seed = 0x1234;
        // TileDrawing.cs:2690-2700 (the `default:` of `switch (num)`): 7 flames, (100, 100, 100, 0), x in [-10, 11) * 0.15, y in [-10, 1) * 0.35
        let d = tiles.get_tile_flame_data(ctx, 40, 50, 4, 0);
        assert_eq!(d.flame_count, 7);
        assert_eq!(d.flame_color, Color::from_ints(100, 100, 100, 0));
        assert_eq!((d.flame_range_x_min, d.flame_range_x_max, d.flame_range_y_min, d.flame_range_y_max), (-10, 11, -10, 1));
        assert_eq!((d.flame_range_mult_x, d.flame_range_mult_y), (0.15, 0.35));
        // TileDrawing.cs:1920: flameSeed = Main.TileFrameSeed ^ (ulong)(((long)tileX << 32) | (uint)tileY)
        assert_eq!(d.flame_seed, 0x1234 ^ ((40_u64 << 32) | 50));
        // frameY / 22 == 14 is the rainbow torch: the colour follows Main.DiscoR/G/B with alpha 0
        ctx.g.main.disco_r = 255;
        ctx.g.main.disco_g = 0;
        ctx.g.main.disco_b = 255;
        let d = tiles.get_tile_flame_data(ctx, 40, 50, 4, 14 * 22);
        assert_eq!(d.flame_color, Color::from_floats(1.0, 0.0, 1.0, 0.0));
    });
}

#[test]
fn flame_data_of_jack_o_lantern_candles_and_jars() {
    with_ctx(|ctx| {
        let tiles = TileDrawing::new();
        // type 35 maps to texture 7 (TileDrawing.cs:1970-1972): four steady flames, (50, 50, 50, 0), no jitter (multipliers 0)
        ctx.g.main.tile_flame[35] = true;
        let d = tiles.get_tile_flame_data(ctx, 5, 6, 35, 0);
        assert_eq!((d.flame_count, d.flame_color), (4, Color::from_ints(50, 50, 50, 0)));
        assert_eq!((d.flame_range_y_min, d.flame_range_y_max, d.flame_range_mult_x, d.flame_range_mult_y), (-10, 10, 0.0, 0.0));
        // the firefly jar (270) and the lightning bug jar (271): one flame, (200, 200, 200, 0), no seed (TileDrawing.cs:1872-1886)
        for t in [270, 271] {
            let d = tiles.get_tile_flame_data(ctx, 5, 6, t, 0);
            assert_eq!((d.flame_count, d.flame_color, d.flame_seed), (1, Color::from_ints(200, 200, 200, 0), 0));
        }
        // the hanging lantern (581): GlowMask 291, (200, 100, 100, 0)
        let d = tiles.get_tile_flame_data(ctx, 5, 6, 581, 0);
        assert_eq!((d.flame_count, d.flame_color), (1, Color::from_ints(200, 100, 100, 0)));
        // a tile without `Main.tileFlame` has no flames at all
        ctx.g.main.tile_flame[3] = false;
        assert_eq!(tiles.get_tile_flame_data(ctx, 5, 6, 3, 0).flame_count, 0);
    });
}

#[test]
fn furniture_flame_colours_follow_the_wrapped_lerp_of_the_clock() {
    with_ctx(|ctx| {
        let tiles = TileDrawing::new();
        // GlobalTimeWrappedHourly % 2 / 2 = 0 -> WrappedLerp(0.5, 1, 0) = 0.5: (int)(150 * 0.5) = 75 (TileDrawing.cs:1830-1835)
        ctx.g.main.global_time_wrapped_hourly = 0.0;
        assert_eq!(tiles.get_fallen_star_furniture_flame_color(ctx), Color::from_ints(150, 75, 75, 50));
        // time 1 -> percent 0.5 -> num = 1 -> WrappedLerp = 1.0
        ctx.g.main.global_time_wrapped_hourly = 1.0;
        assert_eq!(tiles.get_hallowed_furniture_flame_color(ctx), Color::from_ints(170, 170, 170, 75));
        // GetCloudFurnitureFlameColor: white (alpha 0) * WrappedLerp(0.75, 1, 0.5) = white * 1
        assert_eq!(tiles.get_cloud_furniture_flame_color(ctx), Color::from_ints(255, 255, 255, 0));
    });
}

#[test]
fn flame_jitter_draws_come_from_utils_random_int() {
    // RandomNextSeed(0) = (0 * 25214903917 + 11) & (2^48 - 1) = 11; the 31-bit draw is 11 >> 17 = 0, so Next(21) = 0 and the value is -10.
    // The next state is 11 * 25214903917 + 11 = 277363943098 (< 2^48): hand-computed.
    let mut seed = 0_u64;
    assert_eq!(utils_random_int(&mut seed, -10, 11), -10);
    assert_eq!(seed, 11);
    assert_eq!(utils_random_next_seed(11), 277_363_943_098);
}

#[test]
fn wind_cycle_is_a_cosine_of_the_counter_scaled_by_the_wind_strength() {
    with_ctx(|ctx| {
        let tiles = TileDrawing::new();
        ctx.g.main.world_surface = 100.0;
        // x = 10, y = 50, counter 0: phase = 10 * 0.5 + (50 / 100) * 0.5 = 5 (int division), cos(5) = 0.2836622;
        // wind 0.2: (0.2836622 * 0.5 + 0.2) = 0.3418311; GetLerpValue(0.08, 0.18, 0.2, clamped) = 1 (TileDrawing.cs:7814-7840)
        ctx.g.main.wind_speed_current = 0.2;
        let v = tiles.get_wind_cycle(ctx, 10, 50, 0.0);
        assert!((v - 0.3418311).abs() < 1e-5, "{v}");
        // wind 0.13: lerp value (0.13 - 0.08) / (0.18 - 0.08) = 0.5
        ctx.g.main.wind_speed_current = 0.13;
        let v = tiles.get_wind_cycle(ctx, 10, 50, 0.0);
        assert!((v - (0.2836622 * 0.5 + 0.13) * 0.5).abs() < 1e-5, "{v}");
        // no sway below the surface line (y >= worldSurface) or with the setting off
        assert_eq!(tiles.get_wind_cycle(ctx, 10, 100, 0.0), 0.0);
        ctx.state.settings_enabled_tiles_sway_in_wind = false;
        assert_eq!(tiles.get_wind_cycle(ctx, 10, 50, 0.0), 0.0);
        // remix worlds sway below the surface instead
        ctx.state.settings_enabled_tiles_sway_in_wind = true;
        ctx.g.main.remix_world = true;
        assert_eq!(tiles.get_wind_cycle(ctx, 10, 50, 0.0), 0.0);
        assert!(tiles.get_wind_cycle(ctx, 10, 150, 0.0) != 0.0);
    });
}

#[test]
fn wind_counters_and_leaf_frequency_follow_the_wind() {
    with_ctx(|ctx| {
        let mut tiles = TileDrawing::new();
        // TileDrawing.Update (371-391) with no wind: the counters step by their base rates, the leaf frequency is 2000 * 7
        ctx.g.main.wind_speed_current = 0.0;
        tiles.update(ctx);
        assert!((tiles.tree_wind_counter - 1.0 / 240.0).abs() < 1e-12);
        assert!((tiles.grass_wind_counter - 1.0 / 180.0).abs() < 1e-12);
        assert!((tiles.sunflower_wind_counter - 1.0 / 420.0).abs() < 1e-12);
        assert!((tiles.vine_wind_counter - 1.0 / 120.0).abs() < 1e-12);
        assert_eq!(tiles.leaf_frequency, 14000);
        // wind 1.2 (the maximum): GetLerpValue(0.08, 1.2, 1.2) = 1, so the tree counter steps 3 / 240; |wind| in (1.1, inf): frequency 10 * 7
        ctx.g.main.wind_speed_current = -1.2;
        let before = tiles.tree_wind_counter;
        tiles.update(ctx);
        assert!((tiles.tree_wind_counter - before - 3.0 / 240.0).abs() < 1e-9);
        assert_eq!(tiles.leaf_frequency, 70);
        // 0.25 is in (0.2, 0.3]: 450 * 7
        ctx.g.main.wind_speed_current = 0.25;
        tiles.update(ctx);
        assert_eq!(tiles.leaf_frequency, 3150);
    });
}
