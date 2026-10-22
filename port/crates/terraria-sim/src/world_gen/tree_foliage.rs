//! `WorldGen` tree foliage queries (WorldGen.cs): which top / branch texture a tree tile uses, where the tree stands,
//! and which leaf gore it sheds. Read by `TileDrawing.DrawTrees` / `EmitTreeLeaves`.

use terraria_core::framing::get_tile_safely;
use terraria_core::Tile;
use terraria_data::sets::tile_id::sets as tile_sets;

use crate::game::Game;

/// The `out` parameters of `WorldGen.GetTreeFoliageDataMethod` (`floorY`, `topTextureFrameWidth`, `topTextureFrameHeight`).
/// They are assigned before the first return in every implementation, so the caller can read them whatever the result.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TreeFoliageOut {
    pub floor_y: i32,
    pub top_texture_frame_width: i32,
    pub top_texture_frame_height: i32,
}

/// `WorldGen.GetTreeFoliageDataMethod` (WorldGen.cs): `bool (int i, int j, int xoffset, ref int treeFrame, ref int treeStyle, out ...)`
pub type GetTreeFoliageDataMethod = fn(&Game, i32, i32, i32, &mut i32, &mut i32, &mut TreeFoliageOut) -> bool;

/// `WorldGen.GetAshTreeFoliageData` (WorldGen.cs:63349)
pub fn get_ash_tree_foliage_data(g: &Game, i: i32, j: i32, xoffset: i32, _tree_frame: &mut i32, tree_style: &mut i32, out: &mut TreeFoliageOut) -> bool {
    let num = i + xoffset;
    out.top_texture_frame_width = 116;
    out.top_texture_frame_height = 96;
    out.floor_y = j;
    for k in 0..100 {
        out.floor_y = j + k;
        // `Main.tile[num, floorY] == null` (outside the world)
        let Some(tile) = g.main.tile.try_get(num, out.floor_y) else {
            return false;
        };
        if tile.type_ == 633 {
            *tree_style = 31;
            return true;
        }
    }
    false
}

/// `WorldGen.GetVanityTreeFoliageData` (WorldGen.cs:63373)
pub fn get_vanity_tree_foliage_data(g: &Game, i: i32, j: i32, xoffset: i32, _tree_frame: &mut i32, tree_style: &mut i32, out: &mut TreeFoliageOut) -> bool {
    let tile = *g.main.tile.get(i, j);
    let num = i + xoffset;
    out.top_texture_frame_width = 118;
    out.top_texture_frame_height = 96;
    out.floor_y = j;
    for k in 0..100 {
        out.floor_y = j + k;
        let Some(tile2) = g.main.tile.try_get(num, out.floor_y) else {
            return false;
        };
        if tile_sets::conversion::GRASS[tile2.type_ as usize] {
            match tile.type_ {
                596 => {
                    *tree_style = 29;
                    return true;
                }
                616 => {
                    *tree_style = 30;
                    return true;
                }
                _ => {}
            }
        }
    }
    false
}

/// `WorldGen.GemTreeGroundTest(int tileType)` (WorldGen.cs:30502)
pub fn gem_tree_ground_test(tile_type: i32) -> bool {
    if tile_type < 0 {
        return false;
    }
    if tile_sets::conversion::STONE[tile_type as usize] || tile_sets::conversion::MOSS[tile_type as usize] {
        return true;
    }
    false
}

/// `WorldGen.GetGemTreeFoliageData` (WorldGen.cs:63404). `GrowTreeSettings.Profiles.TryGetFromTreeId(tile.type)` succeeds for the
/// tree tile types 583-589 (gem trees, `GroundTest = GemTreeGroundTest`) and 596, 616, 634 (other ground tests; this method is
/// only reached through the table in `DrawTrees` for 583-589).
pub fn get_gem_tree_foliage_data(g: &Game, i: i32, j: i32, xoffset: i32, _tree_frame: &mut i32, tree_style: &mut i32, out: &mut TreeFoliageOut) -> bool {
    let tile = *g.main.tile.get(i, j);
    let num = i + xoffset;
    out.top_texture_frame_width = 116;
    out.top_texture_frame_height = 96;
    out.floor_y = j;
    // TryGetFromTreeId: the ground test of the profile for this tree type
    let ground_test: fn(i32) -> bool = match tile.type_ {
        583..=589 => gem_tree_ground_test,
        596 | 616 => vanity_tree_ground_test,
        634 => ash_tree_ground_test,
        _ => return false,
    };
    for k in 0..100 {
        out.floor_y = j + k;
        let Some(tile2) = g.main.tile.try_get(num, out.floor_y) else {
            return false;
        };
        if ground_test(tile2.type_ as i32) {
            match tile.type_ {
                583 => {
                    *tree_style = 22;
                    return true;
                }
                584 => {
                    *tree_style = 23;
                    return true;
                }
                585 => {
                    *tree_style = 24;
                    return true;
                }
                586 => {
                    *tree_style = 25;
                    return true;
                }
                587 => {
                    *tree_style = 26;
                    return true;
                }
                588 => {
                    *tree_style = 27;
                    return true;
                }
                589 => {
                    *tree_style = 28;
                    return true;
                }
                _ => {}
            }
        }
    }
    false
}

/// `WorldGen.VanityTreeGroundTest(int tileType)` (WorldGen.cs:30515)
pub fn vanity_tree_ground_test(tile_type: i32) -> bool {
    if tile_type < 0 {
        return false;
    }
    if tile_sets::conversion::GRASS[tile_type as usize] && tile_type != 23 && tile_type != 199 {
        return true;
    }
    false
}

/// `WorldGen.AshTreeGroundTest(int tileType)` (WorldGen.cs:30528)
pub fn ash_tree_ground_test(tile_type: i32) -> bool {
    if tile_type < 0 {
        return false;
    }
    if tile_type == 633 {
        return true;
    }
    false
}

/// `WorldGen.GetCommonTreeFoliageData` (WorldGen.cs:63454)
pub fn get_common_tree_foliage_data(g: &Game, i: i32, j: i32, xoffset: i32, tree_frame: &mut i32, tree_style: &mut i32, out: &mut TreeFoliageOut) -> bool {
    let num = i + xoffset;
    out.top_texture_frame_width = 80;
    out.top_texture_frame_height = 80;
    out.floor_y = j;
    let num2;
    for k in 0..100 {
        out.floor_y = j + k;
        let Some(tile) = g.main.tile.try_get(num, out.floor_y) else {
            return false;
        };
        match tile.type_ {
            2 | 477 => {
                let tree_top = |a: usize| g.world_gen.tree_tops[a];
                let num4 = if num <= g.main.tree_x[0] {
                    tree_top(0)
                } else if num <= g.main.tree_x[1] {
                    tree_top(1)
                } else if num > g.main.tree_x[2] {
                    tree_top(3)
                } else {
                    tree_top(2)
                };
                match num4 {
                    0 => *tree_style = 0,
                    5 => *tree_style = 10,
                    _ => *tree_style = 5 + num4,
                }
                return true;
            }
            23 | 661 => {
                *tree_style = 1;
                return true;
            }
            70 => {
                *tree_style = 14;
                return true;
            }
            199 | 662 => {
                *tree_style = 5;
                return true;
            }
            60 => {
                out.top_texture_frame_height = 96;
                out.top_texture_frame_width = 114;
                *tree_style = 2;
                num2 = g.world_gen.tree_tops[5];
                if num2 == 1 {
                    *tree_style = 11;
                    out.top_texture_frame_width = 116;
                }
                if (out.floor_y as f64) > g.main.world_surface {
                    *tree_style = 13;
                    out.top_texture_frame_width = 116;
                }
                return true;
            }
            147 => {
                *tree_style = 4;
                num2 = g.world_gen.tree_tops[6];
                if num2 == 0 {
                    *tree_style = 12;
                    if i % 10 == 0 {
                        *tree_style = 18;
                    }
                }
                if num2 == 2 || num2 == 3 || num2 == 32 || num2 == 4 || num2 == 42 || num2 == 5 || num2 == 7 {
                    if num2 % 2 == 0 {
                        if i < g.main.max_tiles_x / 2 {
                            *tree_style = 16;
                        } else {
                            *tree_style = 17;
                        }
                    } else if i > g.main.max_tiles_x / 2 {
                        *tree_style = 16;
                    } else {
                        *tree_style = 17;
                    }
                }
                return true;
            }
            109 | 492 => {
                out.top_texture_frame_height = 140;
                let num3 = get_hollow_tree_foliage_style(g);
                *tree_style = num3;
                if num3 == 19 {
                    out.top_texture_frame_width = 120;
                }
                if num3 == 20 {
                    *tree_style = 20;
                    if i % 6 == 1 {
                        *tree_frame += 3;
                    } else if i % 6 == 2 {
                        *tree_frame += 6;
                    } else if i % 6 == 3 {
                        *tree_frame += 9;
                    } else if i % 6 == 4 {
                        *tree_frame += 12;
                    } else if i % 6 == 5 {
                        *tree_frame += 15;
                    }
                } else if i % 3 == 1 {
                    *tree_frame += 3;
                } else if i % 3 == 2 {
                    *tree_frame += 6;
                }
                return true;
            }
            _ => {}
        }
    }
    false
}

/// `WorldGen.GetHollowTreeFoliageStyle()` (WorldGen.cs:63600)
pub fn get_hollow_tree_foliage_style(g: &Game) -> i32 {
    // `TreeTops.GetTreeStyle(7);` (result unused in the original)
    match g.world_gen.hallow_bg {
        4 => 19,
        2 | 3 => 20,
        _ => 3,
    }
}

/// `WorldGen.GetTreeFrame(Tile t)` (WorldGen.cs:63615)
pub fn get_tree_frame(t: &Tile) -> i32 {
    if t.frame_y == 220 {
        return 1;
    }
    if t.frame_y == 242 {
        return 2;
    }
    0
}

/// `WorldGen.GetTreeBottom(int i, int j, out int x, out int y)` (WorldGen.cs:64043). Returns `(x, y)`.
pub fn get_tree_bottom(g: &Game, i: i32, j: i32) -> (i32, i32) {
    let mut x = i;
    let mut y = j;
    let mut tile_safely = get_tile_safely(&g.main.tile, x, y);
    if tile_safely.type_ == 323 {
        while y < g.main.max_tiles_y - 1 && (!tile_safely.active() || tile_safely.type_ == 323) {
            y += 1;
            tile_safely = get_tile_safely(&g.main.tile, x, y);
        }
        return (x, y);
    }
    let num = tile_safely.frame_x as i32 / 22;
    let num2 = tile_safely.frame_y as i32 / 22;
    if num == 3 && num2 <= 2 {
        x += 1;
    } else if num == 4 && num2 >= 3 && num2 <= 5 {
        x -= 1;
    } else if num == 1 && num2 >= 6 && num2 <= 8 {
        x -= 1;
    } else if num == 2 && num2 >= 6 && num2 <= 8 {
        x += 1;
    } else if num == 2 && num2 >= 9 {
        x += 1;
    } else if num == 3 && num2 >= 9 {
        x -= 1;
    }
    tile_safely = get_tile_safely(&g.main.tile, x, y);
    while y < g.main.max_tiles_y - 1 && (!tile_safely.active() || tile_sets::IS_A_TREE_TRUNK[tile_safely.type_ as usize] || tile_safely.type_ == 72) {
        y += 1;
        tile_safely = get_tile_safely(&g.main.tile, x, y);
    }
    (x, y)
}

/// `WorldGen.TreeTypes` (WorldGen.cs): the tree kinds `GetTreeType` answers
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TreeTypes {
    None,
    Forest,
    Corrupt,
    Mushroom,
    Crimson,
    Jungle,
    Snow,
    Hallowed,
    Palm,
    PalmCorrupt,
    PalmCrimson,
    PalmHallowed,
    Ash,
}

/// `WorldGen.GetTreeType(int tileType)` (WorldGen.cs:63648)
pub fn get_tree_type(tile_type: i32) -> TreeTypes {
    match tile_type {
        2 | 477 => TreeTypes::Forest,
        23 | 661 => TreeTypes::Corrupt,
        70 => TreeTypes::Mushroom,
        199 | 662 => TreeTypes::Crimson,
        60 => TreeTypes::Jungle,
        147 => TreeTypes::Snow,
        109 | 492 => TreeTypes::Hallowed,
        53 => TreeTypes::Palm,
        112 => TreeTypes::PalmCorrupt,
        234 => TreeTypes::PalmCrimson,
        116 => TreeTypes::PalmHallowed,
        633 => TreeTypes::Ash,
        _ => TreeTypes::None,
    }
}

/// `WorldGen.GetTreeTypeAndTreeBottom(int x, int y, out int bottomX, out int bottomY)` (WorldGen.cs:64018).
/// Returns `(type, bottomX, bottomY)`.
pub fn get_tree_type_and_tree_bottom(g: &Game, x: i32, y: i32) -> (TreeTypes, i32, i32) {
    let (bottom_x, bottom_y) = get_tree_bottom(g, x, y);
    let Some(tile) = g.main.tile.try_get(bottom_x, bottom_y) else {
        return (TreeTypes::None, bottom_x, bottom_y);
    };
    if !tile.active() {
        return (TreeTypes::None, bottom_x, bottom_y);
    }
    (get_tree_type(tile.type_ as i32), bottom_x, bottom_y)
}

/// `WorldGen.GetTreeType(int x, int y)` (WorldGen.cs:63641)
pub fn get_tree_type_at(g: &Game, x: i32, y: i32) -> TreeTypes {
    get_tree_type_and_tree_bottom(g, x, y).0
}

/// `WorldGen.IsTileALeafyTreeTop(Tile t)` (WorldGen.cs:29877)
pub fn is_tile_a_leafy_tree_top_tile(t: &Tile) -> bool {
    if t.active() && tile_sets::GETS_CHECKED_FOR_LEAVES[t.type_ as usize] {
        if t.type_ == 323 && t.frame_x >= 88 {
            return true;
        }
        if t.frame_x == 22 && t.frame_y >= 198 && t.frame_y <= 242 {
            return true;
        }
    }
    false
}

/// `WorldGen.IsTileALeafyTreeTop(int i, int j)` (WorldGen.cs:29872)
pub fn is_tile_a_leafy_tree_top(g: &Game, i: i32, j: i32) -> bool {
    is_tile_a_leafy_tree_top_tile(&get_tile_safely(&g.main.tile, i, j))
}

/// `WorldGen.GetTreeLeaf(int x, Tile topTile, Tile t, ref int treeHeight, out int treeFrame, out int passStyle)` (WorldGen.cs:29581).
/// Returns `(treeFrame, passStyle)`; `tree_height` is the `ref` parameter.
pub fn get_tree_leaf(g: &Game, x: i32, top_tile: &Tile, t: &Tile, tree_height: &mut i32) -> (i32, i32) {
    let mut tree_frame = 0;
    let mut pass_style = -1;
    if top_tile.frame_x == 22 || top_tile.frame_x == 44 || top_tile.frame_x == 66 {
        if top_tile.frame_y == 220 {
            tree_frame = 1;
        } else if top_tile.frame_y == 242 {
            tree_frame = 2;
        }
    }
    if top_tile.frame_x == 44 || top_tile.frame_x == 66 {
        if top_tile.frame_y == 220 {
            tree_frame = 1;
        } else if top_tile.frame_y == 242 {
            tree_frame = 2;
        }
    }
    match top_tile.type_ {
        596 => pass_style = 1248,
        616 => pass_style = 1257,
        634 => pass_style = 1278,
        583..=589 => pass_style = 1249 + (top_tile.type_ as i32 - 583),
        _ => {}
    }
    if pass_style > -1 {
        return (tree_frame, pass_style);
    }
    match t.type_ {
        2 | 477 => pass_style = 910,
        60 => pass_style = 914,
        70 => pass_style = 912,
        23 | 112 => pass_style = 915,
        199 | 234 => pass_style = 916,
        53 => pass_style = 911,
        116 => pass_style = 919,
        147 => pass_style = 913,
        633 => pass_style = 1278,
        109 | 492 => {
            let num = 917;
            let hollow_tree_foliage_style = get_hollow_tree_foliage_style(g);
            if hollow_tree_foliage_style != 20 {
                if x % 3 == 1 {
                    tree_frame += 3;
                }
                if x % 3 == 2 {
                    tree_frame += 6;
                }
                match tree_frame {
                    0 => pass_style = 2,
                    1 => pass_style = 1,
                    2 => pass_style = 7,
                    3 => pass_style = 4,
                    4 => pass_style = 5,
                    5 => pass_style = 6,
                    6 => pass_style = 3,
                    7 => pass_style = 8,
                    8 => pass_style = 0,
                    _ => {}
                }
            } else {
                let num2 = 196;
                if x % 6 == 1 {
                    tree_frame += 3;
                } else if x % 6 == 2 {
                    tree_frame += 6;
                } else if x % 6 == 3 {
                    tree_frame += 9;
                } else if x % 6 == 4 {
                    tree_frame += 12;
                } else if x % 6 == 5 {
                    tree_frame += 15;
                }
                match tree_frame {
                    0 => pass_style = num2,
                    1 => pass_style = num2,
                    2 => pass_style = num2,
                    3 => pass_style = num2 + 1,
                    4 => pass_style = num2 + 2,
                    5 => pass_style = num2 + 1,
                    6 => pass_style = num2 + 3,
                    7 => pass_style = num2 + 4,
                    8 => pass_style = num2 + 5,
                    9 => pass_style = num2 + 6,
                    10 => pass_style = num2 + 6,
                    11 => pass_style = num2 + 6,
                    12 => pass_style = num2 + 7,
                    13 => pass_style = num2 + 7,
                    14 => pass_style = num2 + 7,
                    15 => pass_style = num2 + 8,
                    16 => pass_style = num2 + 8,
                    17 => pass_style = num2 + 8,
                    _ => {}
                }
            }
            pass_style += num;
            *tree_height += 5;
        }
        _ => {}
    }
    (tree_frame, pass_style)
}

#[cfg(test)]
mod tests {
    use super::*;
    use terraria_core::TileMap;

    fn world() -> Game {
        let mut g = Game::new();
        g.main.tile = TileMap::new(100, 100);
        g.main.max_tiles_x = 100;
        g.main.max_tiles_y = 100;
        g.main.world_surface = 50.0;
        g
    }

    fn set(g: &mut Game, x: i32, y: i32, type_: u16) {
        let t = g.main.tile.get_mut(x, y);
        t.type_ = type_;
        t.set_active(true);
    }

    #[test]
    fn common_tree_style_by_ground_tile() {
        let mut g = world();
        let mut frame = 0;
        let mut style = -1;
        let mut out = TreeFoliageOut::default();
        // corruption grass (23) three tiles below the tree top: style 1, 80 x 80 top (WorldGen.cs:63485)
        set(&mut g, 10, 13, 23);
        assert!(get_common_tree_foliage_data(&g, 10, 10, 0, &mut frame, &mut style, &mut out));
        assert_eq!((style, out.floor_y, out.top_texture_frame_width, out.top_texture_frame_height), (1, 13, 80, 80));
        // jungle grass (60) below the surface line: style 13, 116 x 96 (WorldGen.cs:63506-63518); treeTops[5] = 0 here
        set(&mut g, 11, 60, 60);
        assert!(get_common_tree_foliage_data(&g, 11, 58, 0, &mut frame, &mut style, &mut out));
        assert_eq!((style, out.floor_y, out.top_texture_frame_width, out.top_texture_frame_height), (13, 60, 116, 96));
        // jungle grass above the surface line with treeTops[5] == 1: style 11, 116 x 96
        g.world_gen.tree_tops[5] = 1;
        set(&mut g, 12, 40, 60);
        assert!(get_common_tree_foliage_data(&g, 12, 38, 0, &mut frame, &mut style, &mut out));
        assert_eq!((style, out.floor_y, out.top_texture_frame_width, out.top_texture_frame_height), (11, 40, 116, 96));
        // forest grass (2): the style comes from the tree area (treeX = 30, 60, 90) and its variation
        g.main.tree_x = [30, 60, 90];
        g.world_gen.tree_tops[0] = 0; // area 0 -> style 0
        g.world_gen.tree_tops[1] = 5; // area 1 -> style 10
        g.world_gen.tree_tops[2] = 3; // area 2 -> style 8
        g.world_gen.tree_tops[3] = 4; // area 3 -> style 9
        for (x, expected) in [(20, 0), (45, 10), (75, 8), (95, 9)] {
            set(&mut g, x, 21, 2);
            assert!(get_common_tree_foliage_data(&g, x, 20, 0, &mut frame, &mut style, &mut out));
            assert_eq!(style, expected, "x = {x}");
        }
        // snow (147) with treeTops[6] == 2 and an even variation: left half of the world 16, right half 17 (WorldGen.cs:63527-63548)
        g.world_gen.tree_tops[6] = 2;
        set(&mut g, 10, 31, 147);
        set(&mut g, 90, 31, 147);
        assert!(get_common_tree_foliage_data(&g, 10, 30, 0, &mut frame, &mut style, &mut out));
        assert_eq!(style, 16);
        assert!(get_common_tree_foliage_data(&g, 90, 30, 0, &mut frame, &mut style, &mut out));
        assert_eq!(style, 17);
        // nothing grows within 100 tiles: false, floorY ends at j + 99
        assert!(!get_common_tree_foliage_data(&g, 5, 0, 0, &mut frame, &mut style, &mut out));
    }

    #[test]
    fn hallowed_tree_frames_depend_on_the_column() {
        let mut g = world();
        g.world_gen.hallow_bg = 2; // hollow style 20, 140 high, frame offset by x % 6 * 3
        let mut frame = 0;
        let mut style = 0;
        let mut out = TreeFoliageOut::default();
        set(&mut g, 13, 12, 109);
        assert!(get_common_tree_foliage_data(&g, 13, 10, 0, &mut frame, &mut style, &mut out));
        assert_eq!((style, frame, out.top_texture_frame_height, out.top_texture_frame_width), (20, 3, 140, 80)); // 13 % 6 == 1
        g.world_gen.hallow_bg = 4; // style 19 is 120 wide; x % 3 == 2 adds 6
        frame = 0;
        set(&mut g, 14, 12, 109);
        assert!(get_common_tree_foliage_data(&g, 14, 10, 0, &mut frame, &mut style, &mut out));
        assert_eq!((style, frame, out.top_texture_frame_width), (19, 6, 120));
    }

    #[test]
    fn gem_and_vanity_trees() {
        let mut g = world();
        let mut frame = 0;
        let mut style = 0;
        let mut out = TreeFoliageOut::default();
        // a ruby tree (587) on stone (1): style 26 (WorldGen.cs:63429)
        set(&mut g, 20, 10, 587);
        set(&mut g, 20, 14, 1);
        assert!(get_gem_tree_foliage_data(&g, 20, 10, 0, &mut frame, &mut style, &mut out));
        assert_eq!((style, out.floor_y, out.top_texture_frame_width, out.top_texture_frame_height), (26, 14, 116, 96));
        // a sakura tree (596) on grass (2): style 29, 118 wide
        set(&mut g, 30, 10, 596);
        set(&mut g, 30, 12, 2);
        assert!(get_vanity_tree_foliage_data(&g, 30, 10, 0, &mut frame, &mut style, &mut out));
        assert_eq!((style, out.floor_y, out.top_texture_frame_width), (29, 12, 118));
    }

    #[test]
    fn tree_frame_and_leaf_styles() {
        let mut t = Tile::default();
        t.frame_y = 220;
        assert_eq!(get_tree_frame(&t), 1);
        t.frame_y = 242;
        assert_eq!(get_tree_frame(&t), 2);
        t.frame_y = 0;
        assert_eq!(get_tree_frame(&t), 0);
        // GetTreeLeaf on a forest tree (grass 2): gore 910, frame from frameY (WorldGen.cs:29589-29660, 29651)
        let g = world();
        let mut top = Tile::default();
        top.type_ = 5;
        top.frame_x = 22;
        top.frame_y = 242;
        let mut grass = Tile::default();
        grass.type_ = 2;
        let mut h = 10;
        assert_eq!(get_tree_leaf(&g, 10, &top, &grass, &mut h), (2, 910));
        assert_eq!(h, 10);
        // a gem tree top overrides the ground: 1249 + (588 - 583)
        top.type_ = 588;
        assert_eq!(get_tree_leaf(&g, 10, &top, &grass, &mut h).1, 1254);
    }

    #[test]
    fn tree_bottom_walks_down_the_trunk() {
        let mut g = world();
        // a trunk of type 5 from y = 10 to 14 on grass at y = 15; frameX 0 -> x unchanged
        for y in 10..15 {
            set(&mut g, 10, y, 5);
        }
        set(&mut g, 10, 15, 2);
        assert_eq!(get_tree_bottom(&g, 10, 10), (10, 15));
        assert_eq!(get_tree_type_and_tree_bottom(&g, 10, 10), (TreeTypes::Forest, 10, 15));
        // a root tile (frameX 66 = 3 * 22, frameY 0 <= 2) is one column to the right of its trunk: x + 1
        let t = g.main.tile.get_mut(10, 12);
        t.frame_x = 66;
        assert_eq!(get_tree_bottom(&g, 10, 12).0, 11);
    }
}
