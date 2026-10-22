//! The per-tile light rules of `TileLightScanner` (TileLightScanner.cs), one hand-derived expectation per rule. The tile
//! tables (`Main.tileLighted`, `Main.tileBlockLight`, `Main.wallLight`, `TileID.Sets.CrackedBricks`) are the real game's.

use terraria_core::xna::{Color, Vector3};
use terraria_core::{Tile, TileMap};
use terraria_data::sets::tile_id;
use terraria_sim::graphics::light::{LightMaskMode, TileLightScanner};
use terraria_sim::Game;

const W: i32 = 30;
const H: i32 = 2400;

/// A world 30 wide and 2400 high, surface at y = 10, underworld from y = 2200, daytime irrelevant, `Main.mouseTextColor` 0.
fn game() -> Game {
    let mut g = Game::new();
    g.main.tile = TileMap::new(W, H);
    g.main.max_tiles_x = W;
    g.main.max_tiles_y = H;
    g.main.world_surface = 10.0;
    g.main.game_menu = false;
    g
}

fn light_at(g: &mut Game, x: i32, y: i32) -> Vector3 {
    let scanner = TileLightScanner::new();
    let mut out = Vector3::ZERO;
    scanner.get_tile_light(g, x, y, &mut out);
    out
}

fn active(type_: u16) -> Tile {
    let mut t = Tile { type_, ..Tile::default() };
    t.set_active(true);
    t
}

#[test]
fn empty_cave_is_dark() {
    let mut g = game();
    assert_eq!(light_at(&mut g, 5, 100), Vector3::ZERO);
}

#[test]
fn glowing_wall_154_is_red_and_blue() {
    // case 154: R = 0.6, B = 0.6
    let mut g = game();
    g.main.tile.get_mut(5, 100).wall = 154;
    assert_eq!(light_at(&mut g, 5, 100), Vector3::new(0.6, 0.0, 0.6));
}

#[test]
fn disco_wall_44_follows_main_disco() {
    // case 44: R = DiscoR / 255 * 0.15, ... (DiscoR 255, DiscoG 0, DiscoB 0 initially)
    let mut g = game();
    g.main.tile.get_mut(5, 100).wall = 44;
    let want = Vector3::new(255f32 / 255f32 * 0.15f32, 0.0, 0.0);
    assert_eq!(light_at(&mut g, 5, 100), want);
    g.main.disco_g = 51;
    assert_eq!(light_at(&mut g, 5, 100).y, 51f32 / 255f32 * 0.15f32);
}

#[test]
fn torches_use_torch_id_colours() {
    // case 4: TorchID.TorchColor(frameY / 22): torch 0 is (1, 0.95, 0.8), blue torch (id 1) is (0, 0.1, 1.3)
    let mut g = game();
    *g.main.tile.get_mut(5, 100) = active(4);
    assert_eq!(light_at(&mut g, 5, 100), Vector3::new(1.0, 0.95, 0.8));
    g.main.tile.get_mut(5, 100).frame_y = 22;
    assert_eq!(light_at(&mut g, 5, 100), Vector3::new(0.0, 0.1, 1.3));
    // frameX >= 66 is a torch that is not lit
    g.main.tile.get_mut(5, 100).frame_x = 66;
    assert_eq!(light_at(&mut g, 5, 100), Vector3::ZERO);
}

#[test]
fn demon_torch_depends_on_main_demon_torch() {
    // TorchID 7: (0.5 * demonTorch + (1 - demonTorch), 0.3, demonTorch + 0.5 * (1 - demonTorch))
    let mut g = game();
    *g.main.tile.get_mut(5, 100) = active(4);
    g.main.tile.get_mut(5, 100).frame_y = 22 * 7;
    g.main.demon_torch = 1.0;
    assert_eq!(light_at(&mut g, 5, 100), Vector3::new(0.5, 0.3, 1.0));
    g.main.demon_torch = 0.0;
    assert_eq!(light_at(&mut g, 5, 100), Vector3::new(1.0, 0.3, 0.5));
}

#[test]
fn lava_glows_with_the_mouse_text_pulse() {
    // ApplyLiquidLight: num = 0.55 + (270 - mouseTextColor) / 900; (num, num * 0.6, num * 0.2)
    let mut g = game();
    let t = g.main.tile.get_mut(5, 100);
    t.liquid = 100;
    t.set_liquid_type(1);
    let num = 0.55f32 + 270f32 / 900f32;
    assert_eq!(light_at(&mut g, 5, 100), Vector3::new(num, num * 0.6f32, num * 0.2f32));
    g.main.mouse_text_color = 90;
    let num = 0.55f32 + 180f32 / 900f32;
    assert_eq!(light_at(&mut g, 5, 100), Vector3::new(num, num * 0.6f32, num * 0.2f32));
}

#[test]
fn surface_light_comes_from_tile_color_through_open_walls() {
    // ApplySurfaceLight: above Main.worldSurface a tile whose wall lets light through (wall 73 here, or a wall with
    // Main.wallLight) takes Main.tileColor / 255 per channel
    let mut g = game();
    g.main.tile_color = Color::new(204, 102, 51, 255);
    g.main.tile.get_mut(5, 5).wall = 73;
    assert_eq!(light_at(&mut g, 5, 5), Vector3::new(204f32 / 255f32, 102f32 / 255f32, 51f32 / 255f32));
    // no wall at all: lit exactly when Main.wallLight[0] is set
    let lit_without_wall = light_at(&mut g, 6, 5) != Vector3::ZERO;
    assert_eq!(lit_without_wall, g.main.wall_light[0]);
    // below the surface the same wall gives nothing
    g.main.tile.get_mut(5, 100).wall = 73;
    assert_eq!(light_at(&mut g, 5, 100), Vector3::ZERO);
}

#[test]
fn hell_light_below_the_underworld_layer() {
    // ApplyHellLight at GlobalTimeWrappedHourly 0: num4 = 0.55 + sin(0) * 0.08 = 0.55; (num4, num4 * 0.6, num4 * 0.2)
    let mut g = game();
    g.main.tile.get_mut(5, 2300).wall = 73;
    let num4 = 0.55f32;
    assert_eq!(light_at(&mut g, 5, 2300), Vector3::new(num4, num4 * 0.6f32, num4 * 0.2f32));
    // y = UnderworldLayer itself is not "greater than", so it gets no hell light
    g.main.tile.get_mut(5, 2200).wall = 73;
    assert_eq!(light_at(&mut g, 5, 2200), Vector3::ZERO);
}

#[test]
fn painted_gem_light_uses_the_paint_colour() {
    // case 93 with frameY / 54 == 19: R,G,B = (0.37, 0.8, 1) unless painted, then `flag` makes the paint colour / 255
    // replace it. Paint 3 is (255, 255, 0).
    let mut g = game();
    let mut t = active(93);
    t.frame_y = 54 * 19;
    *g.main.tile.get_mut(5, 100) = t;
    assert_eq!(light_at(&mut g, 5, 100), Vector3::new(0.37, 0.8, 1.0));
    g.main.tile.get_mut(5, 100).set_color(3);
    assert_eq!(light_at(&mut g, 5, 100), Vector3::new(255f32 / 255f32, 255f32 / 255f32, 0.0));
}

#[test]
fn masks() {
    let g = game();
    let scanner = TileLightScanner::new();
    let mask = |g: &mut Game, t: Tile| {
        *g.main.tile.get_mut(5, 100) = t;
        scanner.get_mask_mode(g, 5, 100)
    };
    let mut g = g;
    // light-blocking active tile (dirt) is Solid; actuated (inactive) or sloped it is not
    assert!(g.main.tile_block_light[0]);
    assert_eq!(mask(&mut g, active(0)), LightMaskMode::Solid);
    let mut t = active(0);
    t.set_in_active(true);
    assert_eq!(mask(&mut g, t), LightMaskMode::None);
    let mut t = active(0);
    t.set_slope(1);
    assert_eq!(mask(&mut g, t), LightMaskMode::None);
    // tile 131 never blocks
    assert_eq!(mask(&mut g, active(131)), LightMaskMode::None);
    // cracked bricks
    let cracked = (0..tile_id::sets::CRACKED_BRICKS.len()).find(|&i| tile_id::sets::CRACKED_BRICKS[i] && g.main.tile_block_light[i]).unwrap();
    assert_eq!(mask(&mut g, active(cracked as u16)), LightMaskMode::CrackedBricks);
    // liquids: more than 128 of water or honey, but not lava
    let mut t = Tile::default();
    t.liquid = 129;
    assert_eq!(mask(&mut g, t), LightMaskMode::Water);
    t.liquid = 128;
    assert_eq!(mask(&mut g, t), LightMaskMode::None);
    t.liquid = 200;
    t.set_liquid_type(2);
    assert_eq!(mask(&mut g, t), LightMaskMode::Honey);
    t.set_liquid_type(1);
    assert_eq!(mask(&mut g, t), LightMaskMode::None);
    // invisible blocks block only while invisible blocks are shown (`_drawInvisibleWalls`, false for a fresh scanner)
    let mut t = active(0);
    t.set_invisible_block(true);
    assert_eq!(mask(&mut g, t), LightMaskMode::None);
}
