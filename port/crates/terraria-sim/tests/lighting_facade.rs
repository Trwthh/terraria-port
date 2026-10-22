//! `Terraria.Lighting` (Lighting.cs): the public entry points, on a tiny world with one torch, with numbers worked out by
//! hand from the source.

use terraria_core::xna::{Color, Point, Rectangle, Vector2, Vector3};
use terraria_core::{Tile, TileMap};
use terraria_sim::graphics::light::{ILightingEngine, LightMode};
use terraria_sim::{lighting, Game};

const W: i32 = 40;
const H: i32 = 2400;

/// 40 x 2400 world, torch (TorchID 0: (1, 0.95, 0.8)) at (15, 100), `Main.gameUpdateCount` 1.
fn lit_game() -> Game {
    let mut g = Game::new();
    g.main.tile = TileMap::new(W, H);
    g.main.max_tiles_x = W;
    g.main.max_tiles_y = H;
    g.main.world_surface = 10.0;
    g.main.game_menu = false;
    g.main.map_enabled = false;
    g.main.game_update_count = 1;
    let mut t = Tile { type_: 4, ..Tile::default() };
    t.set_active(true);
    *g.main.tile.get_mut(15, 100) = t;
    g
}

/// Four `LightTiles` calls (minimap, metrics, scan, blur) around the area (10, 95, 10, 10).
fn run(g: &mut Game) {
    for _ in 0..4 {
        lighting::light_tiles(g, Rectangle::new(10, 95, 10, 10));
    }
}

fn raw(g: &Game, x: i32, y: i32) -> Vector3 {
    g.lighting.new_engine().get_color(x, y)
}

#[test]
fn state_after_main_initialize() {
    let g = Game::new();
    assert_eq!(lighting::mode(&g), LightMode::Color);
    assert!(lighting::using_new_lighting(&g));
    assert!(lighting::not_retro(&g));
    assert_eq!(lighting::global_brightness(&g), 1.2);
    assert_eq!(g.lighting.off_screen_tiles, 35);
    // UpdateEveryFrame = !Main.RenderTargetsRequired && !NotRetro (RenderTargetsRequired is false at the default zoom)
    assert!(!lighting::update_every_frame(&g));
}

#[test]
fn nothing_is_lit_before_the_first_blur() {
    let mut g = lit_game();
    // two calls = minimap + metrics: no scan yet, `_activeProcessedArea` is empty, GetColor returns black
    lighting::light_tiles(&mut g, Rectangle::new(10, 95, 10, 10));
    lighting::light_tiles(&mut g, Rectangle::new(10, 95, 10, 10));
    assert_eq!(lighting::get_color(&g, 15, 100), Color::new(0, 0, 0, 255));
    assert!(g.main.render);
}

#[test]
fn torch_colour_packing() {
    let mut g = lit_game();
    run(&mut g);
    // centre: R = (int)(1 * 306) -> 255, G = (int)(0.95 * 306) = 290 -> 255, B = (int)(0.8 * 306) = 244 (num = 1.2f * 255f)
    assert_eq!(lighting::get_color(&g, 15, 100).packed_value(), 0xFFF4_FFFF);
    // one tile to the right: 0.91 of each channel: B = (int)(0.8 * 0.91 * 306) = 222
    assert_eq!(lighting::get_color(&g, 16, 100).packed_value(), 0xFFDE_FFFF);
    // outside the processed area (the area inflated by 28) it is black
    assert_eq!(lighting::get_color(&g, 100, 100).packed_value(), 0xFF00_0000);
}

#[test]
fn brightness_is_global_brightness_times_the_channel_mean() {
    let mut g = lit_game();
    run(&mut g);
    let want = 1.2f32 * (1.0f32 + 0.95f32 + 0.8f32) / 3f32;
    assert_eq!(lighting::brightness(&g, 15, 100), want);
}

#[test]
fn tinted_and_clamped_lookups() {
    let mut g = lit_game();
    run(&mut g);
    // GetColor(x, y, oldColor) multiplies the raw light (not scaled by GlobalBrightness) by oldColor
    let old = Color::new(255, 128, 0, 255);
    let c = lighting::get_color_tinted(&g, 15, 100, old);
    assert_eq!(c, Color::from_vector3(Vector3::new(1.0, 0.95, 0.8) * old.to_vector3()));
    assert_eq!(lighting::get_color_point_tinted(&g, Point::new(15, 100), old), c);
    assert_eq!(lighting::get_color_clamped(&g, 15, 100, old), c);
    assert_eq!(lighting::get_color_point(&g, Point::new(15, 100)), Color::from_vector3(Vector3::new(1.0, 0.95, 0.8) * 1.2f32));
    // the menu short-circuits
    g.main.game_menu = true;
    assert_eq!(lighting::get_color(&g, 15, 100), Color::WHITE);
    assert_eq!(lighting::get_color_tinted(&g, 15, 100, old), old);
}

#[test]
fn nine_and_four_slices() {
    let mut g = lit_game();
    run(&mut g);
    let mut v = [Vector3::ZERO; 9];
    lighting::get_color9_slice_vector3(&g, 15, 100, &mut v);
    // slices[4] is the centre, slices[7] is y + 1, slices[5] is x + 1 (index = 3 * (dx + 1) + (dy + 1))
    assert_eq!(v[4], Vector3::new(1.0, 0.95, 0.8) * 1.2f32);
    assert_eq!(v[5], raw(&g, 16, 100) * 1.2f32);
    assert_eq!(v[7], raw(&g, 15, 101) * 1.2f32);
    let mut c = [Color::default(); 9];
    lighting::get_color9_slice(&g, 15, 100, &mut c);
    assert_eq!(c[4].packed_value(), 0xFFF4_FFFF);
    assert_eq!(c[7].packed_value(), lighting::get_color(&g, 15, 101).packed_value());
    assert_eq!(c[5].packed_value(), lighting::get_color(&g, 16, 100).packed_value());
    // 4 slices at (16, 100): left (15, 100) is the torch (channel sum 2.75); up (16, 99), down (16, 101) and right (17, 100)
    // are all 0.91^2 of it. `if (up >= left) slices[0] = left else slices[0] = up`: up < left, so up; `up >= right`: equal
    // sums, so right; `down >= left`: no, so down; `down >= right`: equal, so right.
    let mut q = [Vector3::ZERO; 4];
    lighting::get_color4_slice_vector3(&g, 16, 100, &mut q);
    assert_eq!(q[0], raw(&g, 16, 99) * 1.2f32);
    assert_eq!(q[1], raw(&g, 17, 100) * 1.2f32);
    assert_eq!(q[2], raw(&g, 16, 101) * 1.2f32);
    assert_eq!(q[3], raw(&g, 17, 100) * 1.2f32);
    let mut qc = [Color::default(); 4];
    lighting::get_color4_slice(&g, 16, 100, &mut qc);
    assert!(qc.iter().all(|c| c.a() == 255));
    // slices[0] (up) = (int)(255 * c * 1.2f) per channel
    let up = raw(&g, 16, 99);
    assert_eq!(qc[0].r() as i32, ((255f32 * up.x * 1.2f32) as i32).min(255));
    assert_eq!(qc[0].b() as i32, ((255f32 * up.z * 1.2f32) as i32).min(255));
}

#[test]
fn corner_colors_sum_four_tiles() {
    let mut g = lit_game();
    run(&mut g);
    // top-left corner of (15, 100) sums up (15, 99), up-left (14, 99), left (14, 100) and the centre; the colour
    // is (int)(sum * GlobalBrightness * scale * 63.75), clamped to 255
    let (c, up, upl, left) = (raw(&g, 15, 100), raw(&g, 15, 99), raw(&g, 14, 99), raw(&g, 14, 100));
    let num = 1.2f32 * 1f32 * 63.75f32;
    let r = (((up.x + upl.x + left.x + c.x) * num) as i32).min(255);
    let b = (((up.z + upl.z + left.z + c.z) * num) as i32).min(255);
    let v = lighting::get_corner_colors(&g, 15, 100, 1.0);
    assert_eq!(v.top_left_color.r() as i32, r);
    assert_eq!(v.top_left_color.b() as i32, b);
    assert_eq!(v.top_left_color.a(), 255);
    // scale halves the sums
    let half = lighting::get_corner_colors(&g, 15, 100, 0.5);
    assert_eq!(half.bottom_right_color.b() as i32, (((raw(&g, 15, 101).z + raw(&g, 16, 101).z + raw(&g, 16, 100).z + c.z) * (1.2f32 * 0.5f32 * 63.75f32)) as i32).min(255));
}

#[test]
fn sub_light_interpolates_between_tiles() {
    let mut g = lit_game();
    run(&mut g);
    // the exact middle of tile (15, 100): position / 16 - 0.5 = (15, 100) -> the torch tile itself
    let p = Vector2::new(15.5f32 * 16f32, 100.5f32 * 16f32);
    assert_eq!(lighting::get_sub_light(&g, p), Vector3::new(1.0, 0.95, 0.8));
    // a quarter of the way to the right: Lerp(torch, 0.91 torch, 0.25)
    let p = Vector2::new(15.75f32 * 16f32, 100.5f32 * 16f32);
    let a = raw(&g, 15, 100);
    let b = raw(&g, 16, 100);
    assert_eq!(lighting::get_sub_light(&g, p).x, a.x + (b.x - a.x) * 0.25f32);
}

#[test]
fn added_light_is_applied_at_the_next_blur() {
    let mut g = lit_game();
    // 10 tiles away from the torch it has faded below 0.5 in every channel (0.91^10 = 0.39); AddLight(.5, .5, .5) is
    // written into the working map before the blur, so the tile holds exactly 0.5
    lighting::add_light(&mut g, 5, 100, 0.5, 0.5, 0.5);
    run(&mut g);
    assert_eq!(raw(&g, 5, 100), Vector3::new(0.5, 0.5, 0.5));
    // a server (netMode 2) and a paused game ignore AddLight
    let mut g = lit_game();
    g.main.net_mode = 2;
    lighting::add_light(&mut g, 5, 100, 0.5, 0.5, 0.5);
    run(&mut g);
    assert_ne!(raw(&g, 5, 100), Vector3::new(0.5, 0.5, 0.5));
    let mut g = lit_game();
    g.main.game_paused = true;
    lighting::add_light(&mut g, 5, 100, 0.5, 0.5, 0.5);
    run(&mut g);
    assert_ne!(raw(&g, 5, 100), Vector3::new(0.5, 0.5, 0.5));
}

#[test]
fn add_light_overloads_convert_positions() {
    let mut g = lit_game();
    // AddLight(Vector2 position, Vector3 rgb): (int)(position / 16): -157 / 16 = -9.8 -> -9
    lighting::add_light_vector(&mut g, Vector2::new(-9.0 * 16.0 - 13.0, 100.0 * 16.0 + 3.0), Vector3::new(0.5, 0.5, 0.5));
    // AddLight(int, int, int torchID, float): torch 0 times 0.25 at (30, 100): (0.25, 0.2375, 0.2)
    lighting::add_light_torch(&mut g, 30, 100, 0, 0.25);
    // AddLight(Vector2 position, int torchID): (int)position.X / 16 -> (45, 100), torch 2 is (1, 0.1, 0.1)
    lighting::add_light_position_torch(&mut g, Vector2::new(45.0 * 16.0 + 15.0, 100.0 * 16.0 + 15.0), 2);
    run(&mut g);
    assert_eq!(raw(&g, -9, 100), Vector3::new(0.5, 0.5, 0.5));
    assert_eq!(raw(&g, 30, 100), Vector3::new(0.25, 0.2375, 0.2));
    assert_eq!(raw(&g, 45, 100), Vector3::new(1.0, 0.1, 0.1));
}

#[test]
fn clear_and_light_modes() {
    let mut g = lit_game();
    run(&mut g);
    lighting::clear(&mut g);
    assert_eq!(raw(&g, 15, 100), Vector3::ZERO);
    // NextLightMode: Color -> White (the undefined value 4 falls back to White), Retro, Trippy, Color
    g.main.render_count = 5;
    lighting::next_light_mode(&mut g);
    assert_eq!(lighting::mode(&g), LightMode::White);
    assert!(lighting::not_retro(&g));
    assert_eq!(g.main.render_count, 0);
    lighting::next_light_mode(&mut g);
    assert_eq!(lighting::mode(&g), LightMode::Retro);
    assert!(!lighting::not_retro(&g));
    lighting::next_light_mode(&mut g);
    assert_eq!(lighting::mode(&g), LightMode::Trippy);
    lighting::next_light_mode(&mut g);
    assert_eq!(lighting::mode(&g), LightMode::Color);
    assert_eq!(g.lighting.off_screen_tiles, 35);
    // legacy modes are unported: the lookup is black and the registry has the call
    lighting::set_mode(&mut g, LightMode::Retro);
    assert_eq!(lighting::get_color(&g, 15, 100), Color::new(0, 0, 0, 255));
    assert!(terraria_sim::unported::report().iter().any(|(n, _)| *n == "LegacyLighting.GetColor"));
}
