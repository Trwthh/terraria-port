//! Tests of the sky colour and the parallax maths (`main_backgrounds.rs`, `sky_gen::set_back_color`, `main_draw::apply_color_of_the_skies_to_tiles`).

use super::*;
use terraria_sim::main_time::InfoToSetBackColor;
use terraria_sim::Game;

/// The state `Main.DoDraw` sees in play (single player, no menu, the reference world's size and surface).
fn game() -> Game {
    let mut g = Game::new();
    g.main.game_menu = false;
    g.main.net_mode = 0;
    g.main.max_tiles_x = 4200;
    g.main.world_surface = 331.0;
    g
}

/// `DoDraw`'s sequence: `SetBackColor`, `ApplyColorOfTheSkiesToTiles`, `UpdateAtmosphereTransparencyToSkyColor` (atmo as the frame before left it)
fn sky_at(g: &mut Game, info: &InfoToSetBackColor, atmo_y_tiles: f64) -> (Color, Color, Color, Color) {
    g.main.atmo = 1.0;
    terraria_sim::sky_gen::update_atmosphere_transparency_to_sky_color(g, (atmo_y_tiles * 16.0) as f32);
    let (mut sun, mut moon) = (Color::WHITE, Color::WHITE);
    terraria_sim::sky_gen::set_back_color(g, info, &mut sun, &mut moon);
    crate::main_draw::apply_color_of_the_skies_to_tiles(g);
    terraria_sim::sky_gen::update_atmosphere_transparency_to_sky_color(g, (atmo_y_tiles * 16.0) as f32);
    (sun, moon, g.main.color_of_the_skies, g.main.tile_color)
}

fn rgba(c: Color) -> (u8, u8, u8, u8) {
    (c.r(), c.g(), c.b(), c.a())
}

#[test]
fn sky_colour_at_noon_dawn_dusk_and_night_matches_the_hand_derivation() {
    let mut g = game();
    let info = InfoToSetBackColor::default();
    // noon (27000): none of the dawn / dusk branches: white; the horizon tint has no visibility (sunrise ends at 10800, sunset starts at 43200)
    g.main.day_time = true;
    g.main.time = 27000.0;
    let (sun, _moon, sky, tile) = sky_at(&mut g, &info, 250.0);
    assert_eq!(rgba(sky), (255, 255, 255, 255));
    assert_eq!(rgba(sun), (255, 255, 255, 255));
    assert_eq!(rgba(tile), (255, 255, 255, 255)); // (255 * 3 + 255 * 7) / 10
    // dawn (6750): num2 = 0.5 -> R = (byte)(0.5 * 230 + 25) = 140, G = (byte)(0.5 * 220 + 35) = 145, B = 145; sun (155, 165, 130)
    // sunrise visibility = Remap(6750, 0, 2700, 0, 1) * Remap(6750, 3600, 10800, 1, 0) = 1 * 0.5625; HorizonPhase = 7 -> Sunrise_Aluminum[7] = (138, 95, 155)
    // blend target (max per channel) = (140, 145, 155); Color.Lerp by 0.5625 (16.16 fixed point: 36864): B = 145 + ((10 * 36864) >> 16) = 150
    g.main.time = 6750.0;
    let (sun, _moon, sky, tile) = sky_at(&mut g, &info, 250.0);
    assert_eq!(rgba(sun), (155, 165, 130, 255));
    assert_eq!(rgba(sky), (140, 145, 150, 255));
    // tile colour: sum = 435; R = (435 + 140 * 7) / 10 = 141, G = (435 + 145 * 7) / 10 = 145, B = (435 + 150 * 7) / 10 = 148
    assert_eq!(rgba(tile), (141, 145, 148, 255));
    // dusk (50000): num2 = 1 - (50000 / 54000 - 0.85) * 6.666667 = 0.493827 -> (133, 76, 101); sunset visibility 0.9444 of Sunset_Blue[7] = (171, 103, 125)
    // Lerp by 0.9444 (61895 / 65536): R = 133 + ((38 * 61895) >> 16) = 168, G = 76 + ((27 * 61895) >> 16) = 101, B = 101 + ((24 * 61895) >> 16) = 123
    g.main.time = 50000.0;
    let (_sun, _moon, sky, _tile) = sky_at(&mut g, &info, 250.0);
    assert_eq!(rgba(sky), (168, 101, 123, 255));
    // midnight, full moon (moonPhase 0): minimal light 19. t = 16200: num2 = 0 -> 5, raised to 19; t = 0: 35; t = 8100: num2 = 0.5 -> 20
    g.main.day_time = false;
    for (t, want) in [(16200.0, 19u8), (0.0, 35), (8100.0, 20)] {
        g.main.time = t;
        let (_sun, moon, sky, _tile) = sky_at(&mut g, &info, 250.0);
        assert_eq!(rgba(sky), (want, want, want, 255), "night t = {t}");
        assert_eq!(rgba(moon), (255, 255, 255, 255)); // no blood moon: moonColor = White
    }
    // new moon (moonPhase 4): minimal light 11 -> t = 16200 gives 11
    g.main.moon_phase = 4;
    g.main.time = 16200.0;
    let (_sun, _moon, sky, _tile) = sky_at(&mut g, &info, 250.0);
    assert_eq!(rgba(sky), (11, 11, 11, 255));
}

#[test]
fn atmosphere_darkens_the_sky_towards_black_above_the_atmosphere_line() {
    let mut g = game();
    g.main.day_time = true;
    g.main.time = 27000.0;
    let info = InfoToSetBackColor::default();
    // y = 100 tiles: num = (4200 / 4200)^2 = 1; atmo = (100 - (65 + 10)) / (331 / 5) = 25 / 66.2 = 0.37764
    let (_s, _m, sky, _t) = sky_at(&mut g, &info, 100.0);
    assert!((g.main.atmo - 0.37764).abs() < 1e-4);
    assert_eq!(sky.r(), (255.0_f32 * g.main.atmo) as u8);
    // y = 70 tiles: atmo < 0 -> 0 -> the sky is black
    let (_s, _m, sky, _t) = sky_at(&mut g, &info, 70.0);
    assert_eq!(g.main.atmo, 0.0);
    assert_eq!(rgba(sky), (0, 0, 0, 255));
}

/// `tools/refdump_sky` runs the game's own `SetBackColor`, `ApplyColorOfTheSkiesToTiles` and `UpdateAtmosphereTransparencyToSkyColor` headless for
/// 5,427 combinations of time of day, moon phase, biome influences and cloud alpha (`refdata/sky/sky_colors.csv`): every byte must agree.
#[test]
fn sky_and_tile_colour_match_the_game_for_every_reference_row() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../refdata/sky/sky_colors.csv");
    let Ok(text) = std::fs::read_to_string(&path) else {
        eprintln!("skipping: {} not found (run tools/refdump_sky/run.cmd)", path.display());
        return;
    };
    let mut rows = 0;
    for line in text.lines().skip(1) {
        let c: Vec<&str> = line.split(',').collect();
        let f = |i: usize| c[i].parse::<f32>().unwrap();
        let u = |i: usize| c[i].parse::<u8>().unwrap();
        let mut g = game();
        g.main.day_time = c[1] == "1";
        g.main.time = c[2].parse::<f64>().unwrap();
        g.main.moon_phase = c[3].parse().unwrap();
        g.main.blood_moon = c[4] == "1";
        g.main.cloud_alpha = f(5);
        g.main.smoothed_mushroom_light_influence = f(7);
        let info = InfoToSetBackColor {
            is_in_game_menu_or_is_server: false,
            corruption_biome_influence: f(8),
            crimson_biome_influence: f(9),
            jungle_biome_influence: f(10),
            mushroom_biome_influence: f(7),
            graveyard_influence: f(11),
            blood_moon_active: g.main.blood_moon,
            lantern_night_active: false,
        };
        let (sun, moon, sky, tile) = sky_at(&mut g, &info, c[6].parse::<f64>().unwrap());
        assert_eq!(rgba(sun), (u(12), u(13), u(14), u(15)), "sun, row {line}");
        assert_eq!(rgba(moon), (u(16), u(17), u(18), u(19)), "moon, row {line}");
        assert_eq!(rgba(sky), (u(20), u(21), u(22), u(23)), "ColorOfTheSkies, row {line}");
        assert_eq!(rgba(tile), (u(24), u(25), u(26), u(27)), "tileColor, row {line}");
        assert_eq!(g.main.atmo.to_bits(), f(28).to_bits(), "atmo, row {line}");
        rows += 1;
    }
    assert!(rows > 5000, "only {rows} reference rows");
}

#[test]
fn parallax_positions_follow_the_ieee_remainder_of_the_screen_position() {
    // DrawSurfaceBG_BackMountainsStep1: bgWidthScaled = (int)(1024 * 2) = 2048, bgParallax 0.15,
    // bgStartX = (int)(0 - IEEERemainder(screenX * 0.15, 2048) - 1024)
    let start = |screen_x: f64| (0.0 - terraria_sim::main_time::ieee_remainder(screen_x * 0.15, 2048.0) - 1024.0) as i32;
    assert_eq!(start(0.0), -1024);
    // 0.15 * 10000 = 1500: round(1500 / 2048) = 1 -> 1500 - 2048 = -548 -> start = 548 - 1024
    assert_eq!(start(10000.0), -476);
    // 0.15 * 20000 = 3000: round(1.46) = 1 -> 952 -> start = -952 - 1024
    assert_eq!(start(20000.0), -1976);
}
