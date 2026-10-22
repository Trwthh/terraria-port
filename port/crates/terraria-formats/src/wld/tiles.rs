//! Tile section: `WorldFile.LoadWorldTiles` (WorldFile.cs 2534) and `WorldFile.SaveWorldTiles` (1446).
//!
//! Each tile is a run: up to four header bytes (the first byte says which of the others exist), then the
//! tile type / frames / colour, wall, liquid, high wall byte, and the run length (number of further tiles
//! below it that are identical for saving purposes).
//!
//! Header byte 1 (`b4` in the loader, `b` in the saver): 0 has header 2 | 1 active | 2 wall | 3-4 liquid
//! (1 water, 2 lava, 3 honey) | 5 16-bit tile type | 6 run length in one byte | 7 run length in two bytes.
//! Header byte 2: 0 has header 3 | 1 wire | 2 wire2 | 3 wire3 | 4-6 half brick (1) or slope + 1.
//! Header byte 3: 0 has header 4 | 1 actuator | 2 inactive | 3 tile colour | 4 wall colour | 5 wire4 |
//! 6 high wall byte | 7 shimmer.  Header byte 4: 1 invisible block | 2 invisible wall | 3 fullbright block |
//! 4 fullbright wall.

use super::binio::{Reader, Writer};
use super::error::WldError;
use super::sets::{allows_save_compression_batching, save_slopes, WALL_COUNT};
use terraria_core::tile::{Tile, TileMap, LIQUID_LAVA, LIQUID_SHIMMER};

/// Largest world area accepted (tiles). Large world is 8400 x 2400 = 20,160,000.
pub const MAX_TILES: i64 = 1 << 26;

/// `Tile.isTheSameAs(compTile)` with `Main.tileFrameImportant` passed in. Asymmetric like the original:
/// `active()`, `type` and frames come from `a`, the liquid test looks at `b`.
pub fn is_the_same_as(a: &Tile, b: &Tile, importance: &[bool]) -> bool {
    if a.s_tile_header != b.s_tile_header {
        return false;
    }
    if a.active() {
        if a.type_ != b.type_ {
            return false;
        }
        let important = importance.get(a.type_ as usize).copied().unwrap_or(false);
        if important && (a.frame_x != b.frame_x || a.frame_y != b.frame_y) {
            return false;
        }
    }
    if a.wall != b.wall || a.liquid != b.liquid {
        return false;
    }
    if b.liquid == 0 {
        if a.wall_color() != b.wall_color() {
            return false;
        }
        if a.wire4() != b.wire4() {
            return false;
        }
    } else if a.b_tile_header != b.b_tile_header {
        return false;
    }
    a.invisible_block() == b.invisible_block()
        && a.invisible_wall() == b.invisible_wall()
        && a.fullbright_block() == b.fullbright_block()
        && a.fullbright_wall() == b.fullbright_wall()
}

/// Reads all columns (`for i in 0..maxTilesX`).
pub(crate) fn read_tiles(r: &mut Reader, importance: &[bool], width: i32, height: i32) -> Result<TileMap, WldError> {
    let mut map = TileMap::new(width, height);
    for x in 0..width {
        let column = read_column(r, importance, height)?;
        for (y, t) in column.into_iter().enumerate() {
            *map.get_mut(x, y as i32) = t;
        }
    }
    Ok(map)
}

/// Reads one column of `height` tiles.
pub fn read_column(r: &mut Reader, importance: &[bool], height: i32) -> Result<Vec<Tile>, WldError> {
    let mut column = vec![Tile::default(); height as usize];
    let mut j: i32 = 0;
    while j < height {
        let start = r.pos();
        let (tile, run) = read_tile(r, importance)?;
        column[j as usize] = tile;
        if run > 0 {
            if j + run >= height {
                return Err(r.invalid_at(start, format!("run of {run} tiles at y={j} leaves the {height}-tile column")));
            }
            for k in 1..=run {
                column[(j + k) as usize] = tile;
            }
        }
        j += run + 1;
    }
    Ok(column)
}

fn read_tile(r: &mut Reader, importance: &[bool]) -> Result<(Tile, i32), WldError> {
    let start = r.pos();
    let mut tile = Tile::default();
    let (mut b2, mut b3) = (0u8, 0u8);
    let mut b = 0u8;
    let b4 = r.u8()?;
    let mut flag = false;
    if b4 & 1 == 1 {
        flag = true;
        b = r.u8()?;
    }
    let mut flag2 = false;
    if flag && b & 1 == 1 {
        flag2 = true;
        b2 = r.u8()?;
    }
    if flag2 && b2 & 1 == 1 {
        b3 = r.u8()?;
    }
    // Bits no branch of the original reads. A file written by the game never sets them.
    if b & 0x80 != 0 {
        return Err(r.invalid_at(start, "tile header 2 bit 7 is set but unused"));
    }
    if b3 & 0xE0 != 0 {
        return Err(r.invalid_at(start, "tile header 4 bits 5-7 are set but unused"));
    }
    if b4 & 2 == 2 {
        tile.set_active(true);
        let type_ = if b4 & 0x20 == 0x20 {
            let lo = r.u8()? as i32;
            let hi = r.u8()? as i32;
            (hi << 8) | lo
        } else {
            r.u8()? as i32
        };
        tile.type_ = type_ as u16;
        let important = match importance.get(type_ as usize) {
            Some(v) => *v,
            None => return Err(r.invalid_at(start, format!("tile type {type_} outside the importance table"))),
        };
        if important {
            tile.frame_x = r.i16()?;
            tile.frame_y = r.i16()?;
            if tile.type_ == 144 {
                tile.frame_y = 0;
            }
        } else {
            tile.frame_x = -1;
            tile.frame_y = -1;
        }
        if b2 & 8 == 8 {
            tile.set_color(r.u8()?);
        }
    }
    if b4 & 4 == 4 {
        tile.wall = r.u8()? as u16;
        if tile.wall >= WALL_COUNT {
            tile.wall = 0;
        }
        if b2 & 0x10 == 16 {
            tile.set_wall_color(r.u8()?);
        }
    }
    let liquid_bits = (b4 & 0x18) >> 3;
    if liquid_bits != 0 {
        tile.liquid = r.u8()?;
        if b2 & 0x80 == 128 {
            tile.set_liquid_type(LIQUID_SHIMMER);
        } else if liquid_bits > 1 {
            if liquid_bits == 2 {
                tile.set_liquid_type(LIQUID_LAVA);
            } else {
                tile.set_liquid_type(2); // honey(true)
            }
        }
    }
    if b > 1 {
        if b & 2 == 2 {
            tile.set_wire(true);
        }
        if b & 4 == 4 {
            tile.set_wire2(true);
        }
        if b & 8 == 8 {
            tile.set_wire3(true);
        }
        let s = (b & 0x70) >> 4;
        if s != 0 && save_slopes(tile.type_) {
            if s == 1 {
                tile.set_half_brick(true);
            } else {
                tile.set_slope(s - 1);
            }
        }
    }
    if b2 > 1 {
        if b2 & 2 == 2 {
            tile.set_actuator(true);
        }
        if b2 & 4 == 4 {
            tile.set_in_active(true);
        }
        if b2 & 0x20 == 32 {
            tile.set_wire4(true);
        }
        if b2 & 0x40 == 64 {
            let hi = r.u8()? as u16;
            tile.wall = (hi << 8) | tile.wall;
            if tile.wall >= WALL_COUNT {
                tile.wall = 0;
            }
        }
    }
    if b3 > 1 {
        if b3 & 2 == 2 {
            tile.set_invisible_block(true);
        }
        if b3 & 4 == 4 {
            tile.set_invisible_wall(true);
        }
        if b3 & 8 == 8 {
            tile.set_fullbright_block(true);
        }
        if b3 & 0x10 == 16 {
            tile.set_fullbright_wall(true);
        }
    }
    let run = match (b4 & 0xC0) >> 6 {
        0 => 0,
        1 => r.u8()? as i32,
        2 => {
            let at = r.pos();
            let v = r.i16()?;
            if v < 0 {
                return Err(r.invalid_at(at, format!("negative run length {v}")));
            }
            v as i32
        }
        _ => return Err(r.invalid_at(start, "run length bits 0xC0 are never written by SaveWorldTiles")),
    };
    Ok((tile, run))
}

/// Writes all columns (`for i in 0..maxTilesX`).
pub(crate) fn write_tiles(w: &mut Writer, tiles: &TileMap, importance: &[bool]) -> Result<(), WldError> {
    let h = tiles.height as usize;
    if h == 0 {
        return Ok(());
    }
    for column in tiles.as_slice().chunks(h) {
        write_column(w, column, importance)?;
    }
    Ok(())
}

/// Writes one column the way `SaveWorldTiles` does for one value of `i`.
pub fn write_column(w: &mut Writer, column: &[Tile], importance: &[bool]) -> Result<(), WldError> {
    const SECTION: &str = "Tiles";
    if column.len() > i16::MAX as usize {
        return Err(WldError::Unrepresentable { section: SECTION, reason: format!("column of {} tiles", column.len()) });
    }
    let max_y = column.len();
    let mut array = [0u8; 16];
    let mut y = 0usize;
    while y < max_y {
        let tile = column[y];
        let mut num3 = 4usize;
        let (mut b, mut b2, mut b3, mut b4) = (0u8, 0u8, 0u8, 0u8);
        if tile.active() {
            b |= 2;
            array[num3] = tile.type_ as u8;
            num3 += 1;
            if tile.type_ > 255 {
                array[num3] = (tile.type_ >> 8) as u8;
                num3 += 1;
                b |= 0x20;
            }
            let important = match importance.get(tile.type_ as usize) {
                Some(v) => *v,
                None => {
                    return Err(WldError::Unrepresentable {
                        section: SECTION,
                        reason: format!("tile type {} outside the importance table", tile.type_),
                    })
                }
            };
            if important {
                let fx = tile.frame_x.to_le_bytes();
                let fy = tile.frame_y.to_le_bytes();
                array[num3..num3 + 4].copy_from_slice(&[fx[0], fx[1], fy[0], fy[1]]);
                num3 += 4;
            }
            if tile.color() != 0 {
                b3 |= 8;
                array[num3] = tile.color();
                num3 += 1;
            }
        }
        if tile.wall != 0 {
            b |= 4;
            array[num3] = tile.wall as u8;
            num3 += 1;
            if tile.wall_color() != 0 {
                b3 |= 0x10;
                array[num3] = tile.wall_color();
                num3 += 1;
            }
        }
        if tile.liquid != 0 {
            if !tile.shimmer() {
                b = if tile.lava() {
                    b | 0x10
                } else if !tile.honey() {
                    b | 8
                } else {
                    b | 0x18
                };
            } else {
                b3 |= 0x80;
                b |= 8;
            }
            array[num3] = tile.liquid;
            num3 += 1;
        }
        if tile.wire() {
            b2 |= 2;
        }
        if tile.wire2() {
            b2 |= 4;
        }
        if tile.wire3() {
            b2 |= 8;
        }
        let num4: u32 = if tile.half_brick() {
            16
        } else if tile.slope() != 0 {
            (tile.slope() as u32 + 1) << 4
        } else {
            0
        };
        b2 |= num4 as u8;
        if tile.actuator() {
            b3 |= 2;
        }
        if tile.in_active() {
            b3 |= 4;
        }
        if tile.wire4() {
            b3 |= 0x20;
        }
        if tile.wall > 255 {
            array[num3] = (tile.wall >> 8) as u8;
            num3 += 1;
            b3 |= 0x40;
        }
        if tile.invisible_block() {
            b4 |= 2;
        }
        if tile.invisible_wall() {
            b4 |= 4;
        }
        if tile.fullbright_block() {
            b4 |= 8;
        }
        if tile.fullbright_wall() {
            b4 |= 0x10;
        }
        let mut num5 = 3usize;
        if b4 != 0 {
            b3 |= 1;
            array[num5] = b4;
            num5 -= 1;
        }
        if b3 != 0 {
            b2 |= 1;
            array[num5] = b3;
            num5 -= 1;
        }
        if b2 != 0 {
            b |= 1;
            array[num5] = b2;
            num5 -= 1;
        }
        let mut run: usize = 0;
        let mut next = y + 1;
        while next < max_y
            && is_the_same_as(&tile, &column[next], importance)
            && allows_save_compression_batching(tile.type_)
        {
            run += 1;
            next += 1;
        }
        y += run;
        if run > 0 {
            array[num3] = (run & 0xFF) as u8;
            num3 += 1;
            if run > 255 {
                b |= 0x80;
                array[num3] = ((run & 0xFF00) >> 8) as u8;
                num3 += 1;
            } else {
                b |= 0x40;
            }
        }
        array[num5] = b;
        w.bytes(&array[num5..num3]);
        y += 1;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const HEIGHT: i32 = 1200;

    /// Importance table where only tile types 4 (torch), 21 (chest), 144 (timer), 300 and 600 are frame-important.
    fn importance() -> Vec<bool> {
        let mut v = vec![false; 754];
        for t in [4usize, 21, 144, 300, 600] {
            v[t] = true;
        }
        v
    }

    fn encode(col: &[Tile]) -> Vec<u8> {
        let mut w = Writer::default();
        write_column(&mut w, col, &importance()).expect("encode");
        w.buf
    }

    fn decode(bytes: &[u8], height: i32) -> Vec<Tile> {
        let mut r = Reader::new(bytes);
        let col = read_column(&mut r, &importance(), height).expect("decode");
        assert_eq!(r.remaining(), 0, "decoder must consume the whole column");
        col
    }

    fn active(type_: u16) -> Tile {
        let mut t = Tile::default();
        t.set_active(true);
        t.type_ = type_;
        t.frame_x = -1;
        t.frame_y = -1;
        t
    }

    #[test]
    fn empty_column_is_one_run() {
        let col = vec![Tile::default(); 1200];
        // header 0b1000_0000 (two-byte run), run = 1199
        assert_eq!(encode(&col), vec![0x80, 0xAF, 0x04]);
        assert_eq!(decode(&encode(&col), 1200), col);
    }

    #[test]
    fn run_of_exactly_255_extra_tiles_uses_one_byte_and_256_uses_two() {
        // 256 identical tiles: run = 255 -> one byte (0x40); 257 identical -> run = 256 -> two bytes (0x80)
        let mut col = vec![Tile::default(); 256];
        col.extend(std::iter::repeat(active(1)).take(10));
        let e = encode(&col);
        assert_eq!(&e[..2], &[0x40, 0xFF]);
        assert_eq!(decode(&e, 266), col);

        let mut col = vec![Tile::default(); 257];
        col.push(active(1));
        let e = encode(&col);
        assert_eq!(&e[..3], &[0x80, 0x00, 0x01]);
        assert_eq!(decode(&e, 258), col);
    }

    #[test]
    fn non_important_tile_has_no_frames_and_loads_with_minus_one() {
        let col = vec![active(1), active(1), active(2)];
        let e = encode(&col);
        // tile 1 run 1: 0b0100_0010, type, run; tile 2: 0b0000_0010, type
        assert_eq!(e, vec![0x42, 1, 1, 0x02, 2]);
        let d = decode(&e, 3);
        assert_eq!((d[0].frame_x, d[0].frame_y), (-1, -1));
        assert_eq!(d, col);
    }

    #[test]
    fn important_tile_stores_frames_and_runs_split_on_frame_change() {
        let mut a = active(21);
        a.frame_x = 36;
        a.frame_y = 18;
        let mut b = a;
        b.frame_y = 36;
        let col = vec![a, a, b];
        let e = encode(&col);
        assert_eq!(e, vec![0x42, 21, 36, 0, 18, 0, 1, 0x02, 21, 36, 0, 36, 0]);
        assert_eq!(decode(&e, 3), col);
    }

    #[test]
    fn timer_tile_144_loses_frame_y_on_load() {
        let mut t = active(144);
        t.frame_x = 18;
        t.frame_y = 54;
        let d = decode(&encode(&[t]), 1);
        assert_eq!((d[0].frame_x, d[0].frame_y), (18, 0));
    }

    #[test]
    fn sixteen_bit_tile_and_wall_ids() {
        let mut t = active(600); // > 255: flag 0x20 and a high type byte
        t.frame_x = 0;
        t.frame_y = 72;
        t.wall = 300; // > 255: header-3 bit 6 and a high wall byte
        let e = encode(&[t]);
        // header 1 = active | wall | 16-bit type | has header 2; header 2 = has header 3; header 3 = high wall byte;
        // then type low/high, four frame bytes, wall low, wall high
        assert_eq!(e, vec![0x27, 0x01, 0x40, 0x58, 0x02, 0x00, 0x00, 0x48, 0x00, 0x2C, 0x01]);
        let d = decode(&e, 1);
        assert_eq!(d[0], t);
        assert_eq!(d[0].type_, 600);
        assert_eq!(d[0].wall, 300);
    }

    #[test]
    fn wall_id_at_or_above_wall_count_loads_as_zero() {
        // wall 367 = WallID.Count: stored as low byte 111, high byte 1
        let mut t = Tile::default();
        t.wall = 367;
        let d = decode(&encode(&[t]), 1);
        assert_eq!(d[0].wall, 0);
        let mut t = Tile::default();
        t.wall = 366;
        assert_eq!(decode(&encode(&[t]), 1)[0].wall, 366);
    }

    #[test]
    fn liquids_of_each_type() {
        let mut water = Tile::default();
        water.liquid = 255;
        let mut lava = water;
        lava.set_liquid_type(LIQUID_LAVA);
        lava.liquid = 128;
        let mut honey = water;
        honey.set_liquid_type(2);
        honey.liquid = 7;
        let mut shimmer = water;
        shimmer.set_liquid_type(LIQUID_SHIMMER);
        shimmer.liquid = 1;
        let col = vec![water, lava, honey, shimmer];
        let e = encode(&col);
        assert_eq!(e[0] & 0x18, 0x08); // water
        let d = decode(&e, 4);
        assert_eq!(d, col);
        assert!(d[1].lava() && d[2].honey() && d[3].shimmer() && !d[0].lava());
    }

    #[test]
    fn every_flag_survives() {
        let mut t = active(4);
        t.frame_x = 22;
        t.frame_y = 66;
        t.set_color(30);
        t.wall = 5;
        t.set_wall_color(29);
        t.liquid = 9;
        t.set_liquid_type(LIQUID_LAVA);
        t.set_wire(true);
        t.set_wire2(true);
        t.set_wire3(true);
        t.set_wire4(true);
        t.set_actuator(true);
        t.set_in_active(true);
        t.set_invisible_block(true);
        t.set_invisible_wall(true);
        t.set_fullbright_block(true);
        t.set_fullbright_wall(true);
        // slope on a SaveSlopes tile type (4 is a torch, not in the set: use 1 for slope, 0 for half brick)
        let mut sloped = active(1);
        sloped.set_slope(3);
        let mut half = active(1);
        half.set_half_brick(true);
        let col = vec![t, sloped, half, active(0)];
        // type 0 is "dirt" and is a SaveSlopes tile, tile 4 is not important-free: build expected from decode
        let d = decode(&encode(&col), 4);
        // wire/actuator/colour/invisible flags are all kept; the torch (type 4) is not in SaveSlopes
        assert_eq!(d[0], t);
        assert_eq!(d[1].slope(), 3);
        assert!(d[2].half_brick());
        assert_eq!(d[3].type_, 0);
    }

    #[test]
    fn slope_bits_on_tile_types_outside_save_slopes_are_dropped_on_load() {
        let mut t = active(4); // torch: not solid, not in NonSolidSaveSlopes
        t.set_slope(2);
        t.frame_x = 0;
        t.frame_y = 0;
        let d = decode(&encode(&[t]), 1);
        assert_eq!(d[0].slope(), 0);
        let mut h = active(4);
        h.set_half_brick(true);
        h.frame_x = 0;
        h.frame_y = 0;
        assert!(!decode(&encode(&[h]), 1)[0].half_brick());
    }

    #[test]
    fn batching_is_disabled_for_the_four_excluded_types() {
        let t = active(520);
        let col = vec![t, t, t];
        let e = encode(&col);
        // three separate records (header, two type bytes), no run bits
        assert_eq!(e.len(), 9);
        assert!(e.chunks(3).all(|c| c[0] == 0x22));
        assert_eq!(decode(&e, 3), col);
    }

    #[test]
    fn run_past_the_end_of_the_column_is_an_error() {
        let bytes = [0x42u8, 1, 5]; // active dirt-like tile with run 5 in a 3-tile column
        let mut r = Reader::new(&bytes);
        assert!(read_column(&mut r, &importance(), 3).is_err());
    }

    #[test]
    fn tile_type_outside_importance_table_is_an_error() {
        let bytes = [0x22u8, 0xFF, 0xFF]; // 16-bit type 65535
        let mut r = Reader::new(&bytes);
        assert!(read_column(&mut r, &importance(), 1).is_err());
    }

    #[test]
    fn mixed_column_round_trips_byte_for_byte() {
        let mut col = Vec::new();
        for y in 0..HEIGHT {
            let mut t = Tile::default();
            if y > 300 {
                t = active(if y % 97 < 40 { 1 } else { 2 });
            }
            if y % 211 == 0 {
                t.liquid = (y % 255) as u8;
            }
            if y > 600 {
                t.wall = (y / 10) as u16 % 100;
            }
            col.push(t);
        }
        let e = encode(&col);
        let d = decode(&e, HEIGHT);
        assert_eq!(d, col);
        assert_eq!(encode(&d), e);
    }
}
