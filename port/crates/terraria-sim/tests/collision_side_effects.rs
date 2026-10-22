//! `Terraria.Collision` methods whose job is a side effect on systems that are not ported (`HitTiles`, `HitTilesInACircle`,
//! `SwitchTiles`, `StickyTiles`, `EmptyTile`): checked against results derived by hand from the source, on a small
//! hand-built world. The geometry (which tiles are reached, what is returned) is that of Collision.cs; the calls into
//! `WorldGen.KillTile`, `Wiring.HitSwitch`, `NetMessage.SendData`, `Dust.NewDust` are the `unported!` sites, counted here.
//!
//! One `#[test]` only: the `unported` registry is global to the test binary.

use terraria_core::xna::Vector2;
use terraria_sim::collision::*;
use terraria_sim::{unported, Game};

fn hits(name: &str) -> u64 {
    unported::report().iter().find(|(n, _)| *n == name).map_or(0, |(_, c)| *c)
}

/// A 64 x 64 world of air (`maxTilesY - 40` is the bottom the collision code looks at, so the world is 64 tiles wide and 120 tall).
fn world() -> Game {
    let mut g = Game::new();
    g.main.tile = terraria_core::TileMap::new(64, 120);
    g.main.max_tiles_x = 64;
    g.main.max_tiles_y = 120;
    g
}

fn put(g: &mut Game, x: i32, y: i32, type_: u16) {
    let t = g.main.tile.get_mut(x, y);
    t.type_ = type_;
    t.set_active(true);
}

#[test]
fn side_effect_methods_match_the_source() {
    let mut g = world();
    let solid = (0..754u16).find(|&t| g.main.tile_solid[t as usize] && !g.main.tile_solid_top[t as usize]).unwrap();

    // ---- HitTiles: Position (32, 32), 16 x 16, no velocity: the box is [32, 48] x [32, 48]. The loops run over tiles 1..=4 in
    // both directions (num = 32 / 16 - 1 = 1, num2 = 48 / 16 + 2 = 5 exclusive) and the test is inclusive at the edges, so a solid
    // 6 x 6 block at tiles 0..=5 is reached on tiles 1, 2, 3 in both directions (tile 1 spans [16, 32], tile 3 [48, 64]; tile 4 starts
    // at 64 > 48): 9 calls of WorldGen.KillTile.
    for x in 0..6 {
        for y in 0..6 {
            put(&mut g, x, y, solid);
        }
    }
    let before = hits("WorldGen.KillTile");
    hit_tiles(&mut g, Vector2::new(32.0, 32.0), Vector2::ZERO, 16, 16);
    assert_eq!(hits("WorldGen.KillTile") - before, 9);
    // an actuated-off tile is skipped, a half brick at (3, 3) covers [56, 64] in y and the box ends at 48: skipped too
    g.main.tile.get_mut(2, 2).set_in_active(true);
    g.main.tile.get_mut(3, 3).set_half_brick(true);
    let before = hits("WorldGen.KillTile");
    hit_tiles(&mut g, Vector2::new(32.0, 32.0), Vector2::ZERO, 16, 16);
    assert_eq!(hits("WorldGen.KillTile") - before, 7);
    // with a velocity of (16, 0) the moved box [48, 64] x [32, 48] reaches tiles 2..=4 in x (tile 2 spans [32, 48], 48 <= 48) and 1..=3 in y;
    // the loop over x is still 1..=4 (it uses Position, not Position + Velocity): tiles 2, 3, 4 x 1, 2, 3, minus (2, 2) inactive
    // and (3, 3) half brick -> tile x = 4 is solid only for y <= 5 (all of them), so 3 x 3 - 2 = 7
    let before = hits("WorldGen.KillTile");
    hit_tiles(&mut g, Vector2::new(32.0, 32.0), Vector2::new(16.0, 0.0), 16, 16);
    assert_eq!(hits("WorldGen.KillTile") - before, 7);

    // ---- HitTilesInACircle: centre of the moved box (40, 40); only the tile centred on it ((2, 2), centre (40, 40)) is closer
    // than (16 + 16) / 4 = 8; (2, 2) is inactive above, so reactivate it first
    g.main.tile.get_mut(2, 2).set_in_active(false);
    let before = hits("WorldGen.KillTile");
    hit_tiles_in_a_circle(&mut g, Vector2::new(32.0, 32.0), Vector2::ZERO, 16, 16);
    assert_eq!(hits("WorldGen.KillTile") - before, 1);

    // ---- StickyTiles: tile 51 (cobweb) at (10, 10), 16 x 16 box at (160, 160): returns the tile, no velocity so no Main.rand draw
    // that could spawn dust
    let mut g = world();
    put(&mut g, 10, 10, 51);
    let r = sticky_tiles(&mut g, Vector2::new(160.0, 160.0), Vector2::ZERO, 16, 16);
    assert_eq!((r.x, r.y), (10.0, 10.0));
    let r = sticky_tiles(&mut g, Vector2::new(400.0, 400.0), Vector2::ZERO, 16, 16);
    assert_eq!((r.x, r.y), (-1.0, -1.0));
    // tile 229 needs slope 0; with a slope it is not sticky
    put(&mut g, 20, 20, 229);
    assert_eq!(sticky_tiles(&mut g, Vector2::new(320.0, 320.0), Vector2::ZERO, 16, 16), Vector2::new(20.0, 20.0));
    g.main.tile.get_mut(20, 20).set_slope(1);
    assert_eq!(sticky_tiles(&mut g, Vector2::new(320.0, 320.0), Vector2::ZERO, 16, 16), Vector2::new(-1.0, -1.0));

    // ---- SwitchTiles: pressure plate 135 (frameY 0) at (10, 10): its trigger band starts at y = 10 * 16 + 12 = 172. A 16 x 16 box at
    // (160, 160.5) has its feet at 176.5 > 172 and its top at 160.5 < 172 + 4.01; the old position (160, 100) is above the tile,
    // so the box has just entered: with objType 1 the plate is hit (frameY / 18 = 0 is allowed for every objType but 5 ... see source)
    let mut g = world();
    put(&mut g, 10, 10, 135);
    let before = (hits("Wiring.HitSwitch"), hits("NetMessage.SendData"));
    let v = switch_tiles(&mut g, Vector2::new(160.0, 160.5), 16, 16, Vector2::new(160.0, 100.0), 1);
    assert!(v);
    assert_eq!((hits("Wiring.HitSwitch") - before.0, hits("NetMessage.SendData") - before.1), (1, 1));
    // the old position already inside the band: nothing happens
    let before = hits("Wiring.HitSwitch");
    assert!(!switch_tiles(&mut g, Vector2::new(160.0, 160.5), 16, 16, Vector2::new(160.0, 160.5), 1));
    assert_eq!(hits("Wiring.HitSwitch") - before, 0);
    // frameY / 18 == 5 does not react to objType 1, 4 and 5
    g.main.tile.get_mut(10, 10).frame_y = 5 * 18;
    assert!(!switch_tiles(&mut g, Vector2::new(160.0, 160.5), 16, 16, Vector2::new(160.0, 100.0), 4));
    assert!(switch_tiles(&mut g, Vector2::new(160.0, 160.5), 16, 16, Vector2::new(160.0, 100.0), 2));
    // frameY / 18 == 2 reacts to objType 5 only
    g.main.tile.get_mut(10, 10).frame_y = 2 * 18;
    assert!(!switch_tiles(&mut g, Vector2::new(160.0, 160.5), 16, 16, Vector2::new(160.0, 100.0), 1));
    assert!(switch_tiles(&mut g, Vector2::new(160.0, 160.5), 16, 16, Vector2::new(160.0, 100.0), 5));
    // frameY / 18 == 7 also kills the tile
    g.main.tile.get_mut(10, 10).frame_y = 7 * 18;
    let before = hits("WorldGen.KillTile");
    assert!(switch_tiles(&mut g, Vector2::new(160.0, 160.5), 16, 16, Vector2::new(160.0, 100.0), 5));
    assert_eq!(hits("WorldGen.KillTile") - before, 1);
    // land mine 210: `continue`s after the hit, so the call returns false unless another tile answers
    let mut g = world();
    put(&mut g, 10, 10, 210);
    let before = hits("Wiring.HitSwitch");
    assert!(!switch_tiles(&mut g, Vector2::new(160.0, 160.5), 16, 16, Vector2::new(160.0, 100.0), 1));
    assert_eq!(hits("Wiring.HitSwitch") - before, 1);

    // ---- the Entity / NPCAimedTarget overloads of CanHit forward to the Vector2 one
    let g = world();
    let (a, b) = (EntityBox::new(Vector2::new(100.0, 200.0), 20, 42), EntityBox::new(Vector2::new(400.0, 260.0), 16, 16));
    assert_eq!(can_hit_entity(&g, &a, &b), can_hit_vector2(&g, a.position, a.width, a.height, b.position, b.width, b.height));
    assert_eq!(can_hit_aimed_target(&g, &a, &b), can_hit_vector2(&g, a.position, a.width, a.height, b.position, b.width, b.height));
    assert!(can_hit_entity(&g, &a, &b)); // empty world
    let mut seen = 0;
    assert!(can_hit_with_check_entity(&g, &a, &b, &mut |_, _| { seen += 1; true }));
    assert!(seen > 0);

    // ---- EmptyTile: an active tile is not empty unless ignoreTiles; no player / NPC is active (unported)
    let mut g = world();
    put(&mut g, 3, 3, 1);
    assert!(!empty_tile(&g, 3, 3, false));
    assert!(empty_tile(&g, 3, 3, true));
    assert!(empty_tile(&g, 4, 4, false));
    assert!(hits("Main.player[k] / Main.npc[l] (Collision.EmptyTile)") >= 2);
}
