//! `Terraria.Tile` (decompile/Terraria/Terraria/Tile.cs). Same fields, same bit layout, so values read
//! from a world file or compared against a dump of the original are identical.
//!
//! sTileHeader   0-4 color | 5 active | 6 inActive | 7 wire | 8 wire2 | 9 wire3 | 10 halfBrick | 11 actuator | 12-14 slope | 15 fullbrightWall
//! bTileHeader   0-4 wallColor | 5-6 liquidType | 7 wire4
//! bTileHeader2  0-3 wallFrameX/36 | 4-5 frameNumber | 6-7 wallFrameNumber
//! bTileHeader3  0-2 wallFrameY/36 | 3 checkingLiquid | 4 skipLiquid | 5 invisibleBlock | 6 invisibleWall | 7 fullbrightBlock

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(C)]
pub struct Tile {
    pub type_: u16,
    pub wall: u16,
    pub liquid: u8,
    pub s_tile_header: u16,
    pub b_tile_header: u8,
    pub b_tile_header2: u8,
    pub b_tile_header3: u8,
    pub frame_x: i16,
    pub frame_y: i16,
}

pub const LIQUID_WATER: u8 = 0;
pub const LIQUID_LAVA: u8 = 1;
pub const LIQUID_HONEY: u8 = 2;
pub const LIQUID_SHIMMER: u8 = 3;

macro_rules! flag {
    ($get:ident, $set:ident, $field:ident, $mask:expr) => {
        #[inline]
        pub fn $get(&self) -> bool {
            self.$field & $mask == $mask
        }
        #[inline]
        pub fn $set(&mut self, v: bool) {
            if v {
                self.$field |= $mask;
            } else {
                self.$field &= !$mask;
            }
        }
    };
}

impl Tile {
    flag!(active, set_active, s_tile_header, 0x20);
    flag!(in_active, set_in_active, s_tile_header, 0x40);
    flag!(wire, set_wire, s_tile_header, 0x80);
    flag!(wire2, set_wire2, s_tile_header, 0x100);
    flag!(wire3, set_wire3, s_tile_header, 0x200);
    flag!(half_brick, set_half_brick, s_tile_header, 0x400);
    flag!(actuator, set_actuator, s_tile_header, 0x800);
    flag!(fullbright_wall, set_fullbright_wall, s_tile_header, 0x8000);
    flag!(wire4, set_wire4, b_tile_header, 0x80);
    flag!(checking_liquid, set_checking_liquid, b_tile_header3, 0x08);
    flag!(skip_liquid, set_skip_liquid, b_tile_header3, 0x10);
    flag!(invisible_block, set_invisible_block, b_tile_header3, 0x20);
    flag!(invisible_wall, set_invisible_wall, b_tile_header3, 0x40);
    flag!(fullbright_block, set_fullbright_block, b_tile_header3, 0x80);

    /// `nactive()`: active and not switched off by an actuator.
    #[inline]
    pub fn nactive(&self) -> bool {
        self.s_tile_header & 0x60 == 0x20
    }
    /// `Tile.actColor(Color oldColor)` (Tile.cs:272)
    pub fn act_color(&self, old_color: crate::xna::Color) -> crate::xna::Color {
        if !self.in_active() {
            return old_color;
        }
        let num = 0.4_f64;
        crate::xna::Color::new(
            (num * old_color.r() as f64) as u8,
            (num * old_color.g() as f64) as u8,
            (num * old_color.b() as f64) as u8,
            old_color.a(),
        )
    }
    /// `Tile.actColor(ref Vector3 oldColor)` (Tile.cs:282)
    pub fn act_color_vec3(&self, old_color: &mut crate::xna::Vector3) {
        if self.in_active() {
            *old_color *= 0.4_f32;
        }
    }
    #[inline]
    pub fn color(&self) -> u8 {
        (self.s_tile_header & 0x1F) as u8
    }
    #[inline]
    pub fn set_color(&mut self, color: u8) {
        self.s_tile_header = (self.s_tile_header & 0xFFE0) | color as u16;
    }
    #[inline]
    pub fn slope(&self) -> u8 {
        ((self.s_tile_header & 0x7000) >> 12) as u8
    }
    #[inline]
    pub fn set_slope(&mut self, slope: u8) {
        self.s_tile_header = (self.s_tile_header & 0x8FFF) | (((slope & 7) as u16) << 12);
    }
    #[inline]
    pub fn top_slope(&self) -> bool {
        matches!(self.slope(), 1 | 2)
    }
    #[inline]
    pub fn bottom_slope(&self) -> bool {
        matches!(self.slope(), 3 | 4)
    }
    #[inline]
    pub fn left_slope(&self) -> bool {
        matches!(self.slope(), 2 | 4)
    }
    #[inline]
    pub fn right_slope(&self) -> bool {
        matches!(self.slope(), 1 | 3)
    }
    #[inline]
    pub fn has_same_slope(&self, other: &Tile) -> bool {
        self.s_tile_header & 0x7400 == other.s_tile_header & 0x7400
    }
    #[inline]
    pub fn wall_color(&self) -> u8 {
        self.b_tile_header & 0x1F
    }
    #[inline]
    pub fn set_wall_color(&mut self, c: u8) {
        self.b_tile_header = (self.b_tile_header & 0xE0) | c;
    }
    #[inline]
    pub fn liquid_type(&self) -> u8 {
        (self.b_tile_header & 0x60) >> 5
    }
    #[inline]
    pub fn set_liquid_type(&mut self, t: u8) {
        self.b_tile_header = (self.b_tile_header & 0x9F) | ((t & 3) << 5);
    }
    #[inline]
    pub fn lava(&self) -> bool {
        self.liquid_type() == LIQUID_LAVA
    }
    #[inline]
    pub fn honey(&self) -> bool {
        self.liquid_type() == LIQUID_HONEY
    }
    #[inline]
    pub fn shimmer(&self) -> bool {
        self.liquid_type() == LIQUID_SHIMMER
    }
    #[inline]
    pub fn wall_frame_x(&self) -> i32 {
        (self.b_tile_header2 & 0xF) as i32 * 36
    }
    #[inline]
    pub fn set_wall_frame_x(&mut self, v: i32) {
        self.b_tile_header2 = (self.b_tile_header2 & 0xF0) | ((v / 36) & 0xF) as u8;
    }
    #[inline]
    pub fn frame_number(&self) -> u8 {
        (self.b_tile_header2 & 0x30) >> 4
    }
    #[inline]
    pub fn set_frame_number(&mut self, v: u8) {
        self.b_tile_header2 = (self.b_tile_header2 & 0xCF) | ((v & 3) << 4);
    }
    #[inline]
    pub fn wall_frame_number(&self) -> u8 {
        (self.b_tile_header2 & 0xC0) >> 6
    }
    #[inline]
    pub fn set_wall_frame_number(&mut self, v: u8) {
        self.b_tile_header2 = (self.b_tile_header2 & 0x3F) | ((v & 3) << 6);
    }
    #[inline]
    pub fn wall_frame_y(&self) -> i32 {
        (self.b_tile_header3 & 7) as i32 * 36
    }
    #[inline]
    pub fn set_wall_frame_y(&mut self, v: i32) {
        self.b_tile_header3 = (self.b_tile_header3 & 0xF8) | ((v / 36) & 7) as u8;
    }
    /// `ClearBlockPaintAndCoating()` (Tile.cs:904): `color(0)`, `fullbrightBlock(false)`, `invisibleBlock(false)`.
    #[inline]
    pub fn clear_block_paint_and_coating(&mut self) {
        self.set_color(0);
        self.set_fullbright_block(false);
        self.set_invisible_block(false);
    }
    /// `blockType()`: 0 full, 1 half brick, 2..5 slopes.
    #[inline]
    pub fn block_type(&self) -> i32 {
        if self.half_brick() {
            return 1;
        }
        let num = self.slope() as i32;
        if num > 0 {
            num + 1
        } else {
            num
        }
    }
    #[inline]
    pub fn any_wire(&self) -> bool {
        self.s_tile_header & 0x380 != 0 || self.b_tile_header & 0x80 != 0
    }
}

/// `Main.tile`: column-major like the original (`tile[x, y]`), one contiguous allocation.
pub struct TileMap {
    pub width: i32,
    pub height: i32,
    tiles: Vec<Tile>,
}

impl TileMap {
    pub fn new(width: i32, height: i32) -> Self {
        Self { width, height, tiles: vec![Tile::default(); width as usize * height as usize] }
    }
    #[inline]
    pub fn in_bounds(&self, x: i32, y: i32) -> bool {
        x >= 0 && y >= 0 && x < self.width && y < self.height
    }
    #[inline]
    fn idx(&self, x: i32, y: i32) -> usize {
        debug_assert!(self.in_bounds(x, y), "tile ({x}, {y}) outside {}x{}", self.width, self.height);
        x as usize * self.height as usize + y as usize
    }
    #[inline]
    pub fn get(&self, x: i32, y: i32) -> &Tile {
        &self.tiles[self.idx(x, y)]
    }
    #[inline]
    pub fn get_mut(&mut self, x: i32, y: i32) -> &mut Tile {
        let i = self.idx(x, y);
        &mut self.tiles[i]
    }
    /// A copy, or `None` outside the world (the original returns null there).
    #[inline]
    pub fn try_get(&self, x: i32, y: i32) -> Option<Tile> {
        self.in_bounds(x, y).then(|| self.tiles[self.idx(x, y)])
    }
    pub fn as_slice(&self) -> &[Tile] {
        &self.tiles
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bit_layout_matches_tile_cs() {
        let mut t = Tile::default();
        t.set_active(true);
        assert_eq!(t.s_tile_header, 32); // Tile.ResetToType sets sTileHeader = 32
        t.set_slope(5);
        t.set_half_brick(true);
        assert_eq!(t.s_tile_header & 0x7400, 0x5400);
        t.set_color(31);
        assert_eq!(t.color(), 31);
        t.set_liquid_type(LIQUID_SHIMMER);
        assert_eq!(t.b_tile_header & 0x60, 96);
        t.set_wall_frame_x(36 * 9);
        t.set_frame_number(2);
        t.set_wall_frame_number(3);
        assert_eq!(t.b_tile_header2, 9 | (2 << 4) | (3 << 6));
        t.set_wall_frame_y(36 * 4);
        t.set_fullbright_block(true);
        assert_eq!(t.b_tile_header3, 4 | 0x80);
        assert!(t.nactive());
        t.set_in_active(true);
        assert!(!t.nactive());
    }
}
