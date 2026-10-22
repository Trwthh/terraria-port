//! `Terraria.Framing` (decompile/Terraria/Terraria/Framing.cs): the 8-way self framing used by
//! gemspark-style blocks and the wall framing used by every wall.

use crate::rand::UnifiedRandom;
use crate::tile::{Tile, TileMap};

/// The game tables `Framing` reads. Borrowed from the loaded game data.
pub struct FramingData<'a> {
    /// `TileID.Sets.ForcedDirtMerging`
    pub forced_dirt_merging: &'a [bool],
    /// `Main.tileBrick`
    pub tile_brick: &'a [bool],
    /// `TileID.Sets.GemsparkFramingTypes`
    pub gemspark_framing_types: &'a [i32],
    /// `TileID.Sets.TruncatesWalls`
    pub truncates_walls: &'a [bool],
    /// `Main.wallLargeFrames`
    pub wall_large_frames: &'a [i32],
    /// `WallID.Count`
    pub wall_count: u16,
}

/// `selfFrame8WayLookup`, in tile-sheet cells (the original stores them multiplied by 18).
/// `None` = the original array slot is null (that neighbour mask cannot occur).
const fn build_8way() -> [Option<[(i16, i16); 3]>; 256] {
    let mut t: [Option<[(i16, i16); 3]>; 256] = [None; 256];
    macro_rules! a3 {
        ($i:expr, $x1:expr, $y1:expr, $x2:expr, $y2:expr, $x3:expr, $y3:expr) => {
            t[$i] = Some([($x1 * 18, $y1 * 18), ($x2 * 18, $y2 * 18), ($x3 * 18, $y3 * 18)]);
        };
    }
    macro_rules! a1 {
        ($i:expr, $x:expr, $y:expr) => {
            t[$i] = Some([($x * 18, $y * 18); 3]);
        };
    }
    a3!(0, 9, 3, 10, 3, 11, 3);
    a3!(1, 6, 3, 7, 3, 8, 3);
    a3!(2, 12, 0, 12, 1, 12, 2);
    a1!(3, 15, 2);
    a3!(4, 9, 0, 9, 1, 9, 2);
    a1!(5, 13, 2);
    a3!(6, 6, 4, 7, 4, 8, 4);
    a1!(7, 14, 2);
    a3!(8, 6, 0, 7, 0, 8, 0);
    a3!(9, 5, 0, 5, 1, 5, 2);
    a1!(10, 15, 0);
    a1!(11, 15, 1);
    a1!(12, 13, 0);
    a1!(13, 13, 1);
    a1!(14, 14, 0);
    a1!(15, 14, 1);
    a3!(19, 1, 4, 3, 4, 5, 4);
    a1!(23, 16, 3);
    a1!(27, 17, 0);
    a1!(31, 13, 4);
    a3!(37, 0, 4, 2, 4, 4, 4);
    a1!(39, 17, 3);
    a1!(45, 16, 0);
    a1!(47, 12, 4);
    a3!(55, 1, 2, 2, 2, 3, 2);
    a3!(63, 6, 2, 7, 2, 8, 2);
    a3!(74, 1, 3, 3, 3, 5, 3);
    a1!(75, 17, 1);
    a1!(78, 16, 2);
    a1!(79, 13, 3);
    a3!(91, 4, 0, 4, 1, 4, 2);
    a3!(95, 11, 0, 11, 1, 11, 2);
    a1!(111, 17, 4);
    a1!(127, 14, 3);
    a3!(140, 0, 3, 2, 3, 4, 3);
    a1!(141, 16, 1);
    a1!(142, 17, 2);
    a1!(143, 12, 3);
    a1!(159, 16, 4);
    a3!(173, 0, 0, 0, 1, 0, 2);
    a3!(175, 10, 0, 10, 1, 10, 2);
    a1!(191, 15, 3);
    a3!(206, 1, 0, 2, 0, 3, 0);
    a3!(207, 6, 1, 7, 1, 8, 1);
    a1!(223, 14, 4);
    a1!(239, 15, 4);
    a3!(255, 1, 1, 2, 1, 3, 1);
    t
}
pub static SELF_FRAME_8WAY_LOOKUP: [Option<[(i16, i16); 3]>; 256] = build_8way();

/// `wallFrameLookup`, in pixels (cells of 36).
pub static WALL_FRAME_LOOKUP: [[(i16, i16); 4]; 20] = {
    const fn w(a: [i16; 8]) -> [(i16, i16); 4] {
        [(a[0] * 36, a[1] * 36), (a[2] * 36, a[3] * 36), (a[4] * 36, a[5] * 36), (a[6] * 36, a[7] * 36)]
    }
    [
        w([9, 3, 10, 3, 11, 3, 6, 6]),
        w([6, 3, 7, 3, 8, 3, 4, 6]),
        w([12, 0, 12, 1, 12, 2, 12, 5]),
        w([1, 4, 3, 4, 5, 4, 3, 6]),
        w([9, 0, 9, 1, 9, 2, 9, 5]),
        w([0, 4, 2, 4, 4, 4, 2, 6]),
        w([6, 4, 7, 4, 8, 4, 5, 6]),
        w([1, 2, 2, 2, 3, 2, 3, 5]),
        w([6, 0, 7, 0, 8, 0, 6, 5]),
        w([5, 0, 5, 1, 5, 2, 5, 5]),
        w([1, 3, 3, 3, 5, 3, 1, 6]),
        w([4, 0, 4, 1, 4, 2, 4, 5]),
        w([0, 3, 2, 3, 4, 3, 0, 6]),
        w([0, 0, 0, 1, 0, 2, 0, 5]),
        w([1, 0, 2, 0, 3, 0, 1, 5]),
        w([1, 1, 2, 1, 3, 1, 2, 5]),
        w([6, 1, 7, 1, 8, 1, 7, 5]),
        w([6, 2, 7, 2, 8, 2, 8, 5]),
        w([10, 0, 10, 1, 10, 2, 10, 5]),
        w([11, 0, 11, 1, 11, 2, 11, 5]),
    ]
};

pub const PHLEBAS_TILE_FRAME_NUMBER_LOOKUP: [[i32; 3]; 4] = [[2, 4, 2], [1, 3, 1], [2, 2, 4], [1, 1, 3]];
pub const LAZURE_TILE_FRAME_NUMBER_LOOKUP: [[i32; 2]; 2] = [[1, 3], [2, 4]];
pub const CENTER_WALL_FRAME_LOOKUP: [[i32; 3]; 3] = [[2, 0, 0], [0, 1, 4], [0, 3, 0]];

#[derive(Clone, Copy, Default)]
struct BlockStyle {
    top: bool,
    bottom: bool,
    left: bool,
    right: bool,
}

/// `blockStyleLookup[tile.blockType()]`
fn find_block_style(t: &Tile) -> BlockStyle {
    const fn s(top: bool, bottom: bool, left: bool, right: bool) -> BlockStyle {
        BlockStyle { top, bottom, left, right }
    }
    const LOOKUP: [BlockStyle; 6] = [
        s(true, true, true, true),
        s(false, true, true, true),
        s(false, true, true, false),
        s(false, true, false, true),
        s(true, false, true, false),
        s(true, false, false, true),
    ];
    LOOKUP[t.block_type() as usize]
}

fn will_it_blend(d: &FramingData, my_type: u16, other_type: u16) -> bool {
    if d.forced_dirt_merging[my_type as usize] && other_type == 0 {
        return true;
    }
    if d.tile_brick[my_type as usize] && d.tile_brick[other_type as usize] {
        return true;
    }
    d.gemspark_framing_types[other_type as usize] == d.gemspark_framing_types[my_type as usize]
}

/// `Framing.GetTileSafely`: an empty tile outside the world.
#[inline]
pub fn get_tile_safely(map: &TileMap, i: i32, j: i32) -> Tile {
    map.try_get(i, j).unwrap_or_default()
}

/// `Framing.SelfFrame8Way(i, j, centerTile, resetFrame)`
pub fn self_frame_8way(map: &mut TileMap, d: &FramingData, gen_rand: &mut UnifiedRandom, i: i32, j: i32, reset_frame: bool) {
    let center = *map.get(i, j);
    if !center.active() {
        return;
    }
    let style = find_block_style(&center);
    let mut num = 0usize;
    // a neighbour's style counts only when it blends and faces the centre; otherwise it is all-false
    let side = |map: &TileMap, x: i32, y: i32, faces: fn(&BlockStyle) -> bool| -> (bool, BlockStyle) {
        let t = get_tile_safely(map, x, y);
        if t.active() && will_it_blend(d, center.type_, t.type_) {
            let s = find_block_style(&t);
            if faces(&s) {
                return (true, s);
            }
        }
        (false, BlockStyle::default())
    };
    let (mut up, mut left, mut right, mut down) = (BlockStyle::default(), BlockStyle::default(), BlockStyle::default(), BlockStyle::default());
    if style.top {
        let (hit, s) = side(map, i, j - 1, |s| s.bottom);
        up = s;
        if hit {
            num |= 1;
        }
    }
    if style.left {
        let (hit, s) = side(map, i - 1, j, |s| s.right);
        left = s;
        if hit {
            num |= 2;
        }
    }
    if style.right {
        let (hit, s) = side(map, i + 1, j, |s| s.left);
        right = s;
        if hit {
            num |= 4;
        }
    }
    if style.bottom {
        let (hit, s) = side(map, i, j + 1, |s| s.top);
        down = s;
        if hit {
            num |= 8;
        }
    }
    let corner = |map: &TileMap, x: i32, y: i32, ok: fn(&BlockStyle) -> bool| -> bool {
        let t = get_tile_safely(map, x, y);
        t.active() && will_it_blend(d, center.type_, t.type_) && ok(&find_block_style(&t))
    };
    if up.left && left.top && corner(map, i - 1, j - 1, |s| s.right && s.bottom) {
        num |= 0x10;
    }
    if up.right && right.top && corner(map, i + 1, j - 1, |s| s.left && s.bottom) {
        num |= 0x20;
    }
    if down.left && left.bottom && corner(map, i - 1, j + 1, |s| s.right && s.top) {
        num |= 0x40;
    }
    if down.right && right.bottom && corner(map, i + 1, j + 1, |s| s.left && s.top) {
        num |= 0x80;
    }
    let tile = map.get_mut(i, j);
    if reset_frame {
        tile.set_frame_number(gen_rand.next_range(0, 3) as u8);
    }
    // The original indexes a null array for impossible masks and throws; WorldGen.TileFrame swallows it.
    if let Some(points) = SELF_FRAME_8WAY_LOOKUP[num] {
        // frameNumber is 2 bits; a value of 3 would index past the 3 entries and throw in the original too
        if let Some(&(x, y)) = points.get(tile.frame_number() as usize) {
            tile.frame_x = x;
            tile.frame_y = y;
        }
    }
}

/// `Framing.WallFrame(i, j, resetFrame)`. `show_invisible` is `Main.ShouldShowInvisibleBlocksAndWalls()`.
pub fn wall_frame(map: &mut TileMap, d: &FramingData, gen_rand: &mut UnifiedRandom, i: i32, j: i32, reset_frame: bool, show_invisible: bool) {
    if i <= 0 || j <= 0 || i >= map.width - 1 || j >= map.height - 1 {
        return;
    }
    if map.get(i, j).wall >= d.wall_count {
        map.get_mut(i, j).wall = 0;
    }
    let tile = *map.get(i, j);
    if tile.wall == 0 {
        let t = map.get_mut(i, j);
        t.set_wall_color(0);
        // ClearWallPaintAndCoating
        t.set_wall_color(0);
        t.set_fullbright_wall(false);
        t.set_invisible_wall(false);
        return;
    }
    let joins = |x: i32, y: i32| -> bool {
        let t = map.get(x, y);
        (t.wall > 0 || (t.active() && d.truncates_walls[t.type_ as usize])) && (show_invisible || !t.invisible_wall())
    };
    let mut num = 0usize;
    if joins(i, j - 1) {
        num = 1;
    }
    if joins(i - 1, j) {
        num |= 2;
    }
    if joins(i + 1, j) {
        num |= 4;
    }
    if joins(i, j + 1) {
        num |= 8;
    }
    let large = d.wall_large_frames[tile.wall as usize];
    let num2: i32 = if large == 1 {
        PHLEBAS_TILE_FRAME_NUMBER_LOOKUP[(j % 4) as usize][(i % 3) as usize] - 1
    } else if large == 2 {
        LAZURE_TILE_FRAME_NUMBER_LOOKUP[(i % 2) as usize][(j % 2) as usize] - 1
    } else if reset_frame {
        let mut n = gen_rand.next_range(0, 3);
        if tile.wall == 21 && gen_rand.next(2) == 0 {
            n = 2;
        }
        n
    } else {
        tile.wall_frame_number() as i32
    };
    if num == 15 {
        num += CENTER_WALL_FRAME_LOOKUP[(i % 3) as usize][(j % 3) as usize] as usize;
    }
    let t = map.get_mut(i, j);
    if large == 1 || large == 2 || reset_frame {
        t.set_wall_frame_number(num2 as u8);
    }
    let (x, y) = WALL_FRAME_LOOKUP[num][num2 as usize];
    t.set_wall_frame_x(x as i32);
    t.set_wall_frame_y(y as i32);
}
