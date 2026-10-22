//! Small checks of the pure helpers around `WorldGen.TileFrame`, each read off the decompiled source
//! (line numbers in the doc comments of `terraria_sim::world_gen::tile_frame`). The whole-world comparison with the real
//! game is `tile_frame_reference_world.rs`.

use terraria_core::TileMap;
use terraria_data::sets::tile_id;
use terraria_sim::world_gen::tile_frame::*;
use terraria_sim::Game;

fn game(w: i32, h: i32) -> Game {
    let mut g = Game::new();
    g.main.tile = TileMap::new(w, h);
    g.main.max_tiles_x = w;
    g.main.max_tiles_y = h;
    g
}

#[test]
fn tile_merge_attempt_replaces_every_neighbour_that_matches() {
    let mut n = [3, 4, 5, 3, 4, 5, 3, 4];
    {
        let [up, down, left, right, ul, ur, dl, dr] = &mut n;
        tile_merge_attempt(9, 4, up, down, left, right, ul, ur, dl, dr);
    }
    assert_eq!(n, [3, 9, 5, 3, 9, 5, 3, 9]);
    // the four-neighbour overload leaves the corners alone
    let mut m = [3, 4, 5, 4, 4, 4, 4, 4];
    {
        let [up, down, left, right, ..] = &mut m;
        tile_merge_attempt_4(9, 4, up, down, left, right);
    }
    assert_eq!(m, [3, 9, 5, 9, 4, 4, 4, 4]);
}

#[test]
fn tile_merge_attempt_with_a_table_ignores_negative_neighbours_and_honours_exclusions() {
    let mut lookfor = vec![false; 754];
    lookfor[7] = true;
    lookfor[8] = true;
    let mut exclude = vec![false; 754];
    exclude[8] = true;
    let mut n = [7, 8, -1, -2, 7, 8, 1, 7];
    {
        let [up, down, left, right, ul, ur, dl, dr] = &mut n;
        tile_merge_attempt_tbl(0, &lookfor, up, down, left, right, ul, ur, dl, dr);
    }
    assert_eq!(n, [0, 0, -1, -2, 0, 0, 1, 0]);

    let mut n = [7, 8, -1, 7, 7, 8, 1, 7];
    {
        let [up, down, left, right, ul, ur, dl, dr] = &mut n;
        tile_merge_attempt_tbl_exclude_tbl(0, &lookfor, &exclude, up, down, left, right, ul, ur, dl, dr);
    }
    assert_eq!(n, [0, 8, -1, 0, 0, 8, 1, 0]);

    let mut n = [7, 8, -1, 7, 7, 8, 1, 7];
    {
        let [up, down, left, right, ul, ur, dl, dr] = &mut n;
        tile_merge_attempt_tbl_exclude_type(0, &lookfor, 7, up, down, left, right, ul, ur, dl, dr);
    }
    assert_eq!(n, [7, 0, -1, 7, 7, 0, 1, 7]);
}

#[test]
fn tile_merge_attempt_weird_turns_foreign_solid_neighbours_into_the_change_to_value() {
    let mut exclude = vec![false; 754];
    exclude[3] = true; // a tile type the caller keeps
    let mut n = [2, 3, 5, -1, 2, 5, 0, 2];
    {
        let [up, down, left, right, ul, ur, dl, dr] = &mut n;
        tile_merge_attempt_weird(2, -1, &exclude, up, down, left, right, ul, ur, dl, dr);
    }
    // 2 is my own type (kept), 3 is excluded (kept), 5 and 0 are foreign (-> -1), -1 stays
    assert_eq!(n, [2, 3, -1, -1, 2, -1, -1, 2]);
}

#[test]
fn cull_cache_clears_only_the_flagged_neighbours() {
    let c = TileMergeCullCache { cull_top: true, cull_bottom_right: true, cull_left: true, ..Default::default() };
    let mut n = [1, 2, 3, 4, 5, 6, 7, 8];
    {
        let [up, down, left, right, ul, ur, dl, dr] = &mut n;
        c.cull(up, down, left, right, ul, ur, dl, dr);
    }
    assert_eq!(n, [-1, 2, -1, 4, 5, 6, 7, -1]);
}

#[test]
fn merge_culling_depends_on_invisible_blocks_and_on_the_echo_setting() {
    let mut g = game(8, 8);
    g.main.tile.get_mut(1, 1).set_invisible_block(true);
    let c = *g.main.tile.get(1, 1);
    let n = *g.main.tile.get(1, 2);
    let culled = get_tile_merge_culling(&g, c, n, n, n, n, n, n, n, n);
    assert!(culled.cull_top && culled.cull_bottom && culled.cull_left && culled.cull_right);
    assert!(culled.cull_top_left && culled.cull_top_right && culled.cull_bottom_left && culled.cull_bottom_right);
    let same = get_tile_merge_culling(&g, c, c, c, c, c, c, c, c, c);
    assert!(!same.cull_top && !same.cull_bottom_right);
    g.main.echo_monolith = true; // Main.ShouldShowInvisibleBlocksAndWalls() -> true: nothing is culled
    let shown = get_tile_merge_culling(&g, c, n, n, n, n, n, n, n, n);
    assert!(!shown.cull_top && !shown.cull_left);
}

#[test]
fn section_bounds_are_clamped_like_the_original() {
    let g = game(4200, 1200);
    assert_eq!(get_section_bounds(&g, 0, 0, 20, 7), (1, 4198, 1, 1198));
    assert_eq!(get_section_bounds(&g, 1, 2, 1, 3), (200, 400, 300, 600));
    assert_eq!(get_section_bounds(&g, 30, 30, 31, 31), (4198, 4198, 1198, 1198));
}

#[test]
fn ropes_are_found_between_two_rope_ends() {
    let rope = (0..754).find(|&t| tile_id::sets::PLATFORMS[t] == false && Game::new().main.tile_rope[t]).expect("a rope tile type");
    let mut g = game(20, 30);
    let put = |g: &mut Game, y: i32, t: u16| {
        let tile = g.main.tile.get_mut(10, y);
        tile.set_active(true);
        tile.type_ = t;
    };
    put(&mut g, 5, rope as u16);
    for y in 6..=8 {
        put(&mut g, y, 19); // TileID.Platforms
    }
    put(&mut g, 9, rope as u16);
    assert!(is_rope(&g, 10, 5, 5));
    assert!(is_rope(&g, 10, 7, 5), "a platform between two ropes counts");
    assert!(!is_rope(&g, 10, 12, 5), "an empty tile is no rope");
    assert_eq!(get_rope_ends(&g, 10, 7, false, 5), (5, 9));
    assert!(in_world(&g, 0, 0, 0) && !in_world(&g, 20, 0, 0) && !in_world(&g, 0, 0, 1));
}

#[test]
fn tile_frame_clears_the_decoration_of_an_empty_tile_and_ignores_the_border() {
    let mut g = game(20, 20);
    {
        let t = g.main.tile.get_mut(10, 10);
        t.set_half_brick(true);
        t.set_slope(2);
        t.set_color(7);
    }
    tile_frame(&mut g, 10, 10, false, false);
    let t = g.main.tile.get(10, 10);
    assert!(!t.half_brick() && t.slope() == 0 && t.color() == 0);
    // within 5 tiles of the edge nothing happens at all
    g.main.tile.get_mut(3, 3).set_half_brick(true);
    tile_frame(&mut g, 3, 3, false, false);
    assert!(g.main.tile.get(3, 3).half_brick());
}

#[test]
fn an_isolated_dirt_block_gets_the_all_sides_open_cell() {
    // WorldGen.cs:85661-85676: up, down, left and right all differ from the tile -> (162 / 180 / 198, 54) by frame number
    for (frame_number, x) in [(0u8, 162i16), (1, 180), (2, 198)] {
        let mut g = game(20, 20);
        let t = g.main.tile.get_mut(10, 10);
        t.set_active(true);
        t.type_ = 0;
        t.frame_x = -1;
        t.frame_y = -1;
        t.set_frame_number(frame_number);
        terraria_sim::world_gen::tile_frame_cosmetic(&mut g, 10, 10, false);
        let t = g.main.tile.get(10, 10);
        assert_eq!((t.frame_x, t.frame_y), (x, 54), "frame number {frame_number}");
    }
}
