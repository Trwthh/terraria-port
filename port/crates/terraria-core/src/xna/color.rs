//! `Microsoft.Xna.Framework.Color` (reference/xna/Color.cs) and the
//! `PackUtils` helpers it uses (reference/xna/PackUtils.cs).
//!
//! The colour is stored as the packed u32 exactly like the reference:
//! R in bits 0..8, G in 8..16, B in 16..24, A in 24..32. On little-endian
//! targets that is the R,G,B,A byte order in memory.

use super::{Vector3, Vector4};
use core::ops::Mul;

/// `PackUtils.ClampAndRound`. Returns the clamped, banker's-rounded value.
fn clamp_and_round(value: f32, min: f32, max: f32) -> f64 {
    if value.is_nan() {
        return 0.0;
    }
    if value.is_infinite() {
        return if value < 0.0 { min as f64 } else { max as f64 };
    }
    if value < min {
        return min as f64;
    }
    if value > max {
        return max as f64;
    }
    (value as f64).round_ties_even()
}

/// `PackUtils.PackUNorm`.
fn pack_unorm(bitmask: f32, value: f32) -> u32 {
    let value = value * bitmask;
    clamp_and_round(value, 0.0, bitmask) as u32
}

/// `PackUtils.UnpackUNorm`.
fn unpack_unorm(bitmask: u32, value: u32) -> f32 {
    let value = value & bitmask;
    value as f32 / bitmask as f32
}

fn pack_helper(x: f32, y: f32, z: f32, w: f32) -> u32 {
    let num = pack_unorm(255.0, x);
    let num2 = pack_unorm(255.0, y) << 8;
    let num3 = pack_unorm(255.0, z) << 16;
    let num4 = pack_unorm(255.0, w) << 24;
    num | num2 | num3 | num4
}

fn clamp_to_byte32(value: i32) -> i32 {
    if value < 0 {
        0
    } else if value > 255 {
        255
    } else {
        value
    }
}

fn clamp_to_byte64(value: i64) -> i32 {
    if value < 0 {
        0
    } else if value > 255 {
        255
    } else {
        value as i32
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
#[repr(transparent)]
pub struct Color {
    packed_value: u32,
}

impl Color {
    /// Private constructor `Color(uint packedValue)` of the reference, made public.
    pub const fn from_packed_value(packed_value: u32) -> Color {
        Color { packed_value }
    }

    /// Bytes are used as-is (no clamping needed, same result as the int constructor).
    pub const fn new(r: u8, g: u8, b: u8, a: u8) -> Color {
        Color {
            packed_value: (r as u32) | ((g as u32) << 8) | ((b as u32) << 16) | ((a as u32) << 24),
        }
    }

    /// `Color(int r, int g, int b)`: alpha is 255, channels clamp to 0..=255.
    pub fn from_ints_rgb(r: i32, g: i32, b: i32) -> Color {
        let (mut r, mut g, mut b) = (r, g, b);
        if ((r | g | b) & -256) != 0 {
            r = clamp_to_byte64(r as i64);
            g = clamp_to_byte64(g as i64);
            b = clamp_to_byte64(b as i64);
        }
        g <<= 8;
        b <<= 16;
        Color {
            packed_value: (r | g | b | -16777216) as u32,
        }
    }

    /// `Color(int r, int g, int b, int a)`: channels clamp to 0..=255.
    pub fn from_ints(r: i32, g: i32, b: i32, a: i32) -> Color {
        let (mut r, mut g, mut b, mut a) = (r, g, b, a);
        if ((r | g | b | a) & -256) != 0 {
            r = clamp_to_byte32(r);
            g = clamp_to_byte32(g);
            b = clamp_to_byte32(b);
            a = clamp_to_byte32(a);
        }
        g <<= 8;
        b <<= 16;
        a <<= 24;
        Color {
            packed_value: (r | g | b | a) as u32,
        }
    }

    /// `Color(float r, float g, float b)`: alpha is 1.
    pub fn from_floats_rgb(r: f32, g: f32, b: f32) -> Color {
        Color {
            packed_value: pack_helper(r, g, b, 1.0),
        }
    }

    /// `Color(float r, float g, float b, float a)`.
    pub fn from_floats(r: f32, g: f32, b: f32, a: f32) -> Color {
        Color {
            packed_value: pack_helper(r, g, b, a),
        }
    }

    /// `Color(Vector3 vector)`: alpha is 1.
    pub fn from_vector3(vector: Vector3) -> Color {
        Color {
            packed_value: pack_helper(vector.x, vector.y, vector.z, 1.0),
        }
    }

    /// `Color(Vector4 vector)`.
    pub fn from_vector4(vector: Vector4) -> Color {
        Color {
            packed_value: pack_helper(vector.x, vector.y, vector.z, vector.w),
        }
    }

    /// `Color.FromNonPremultiplied(Vector4)`.
    pub fn from_non_premultiplied_vector4(vector: Vector4) -> Color {
        Color {
            packed_value: pack_helper(
                vector.x * vector.w,
                vector.y * vector.w,
                vector.z * vector.w,
                vector.w,
            ),
        }
    }

    /// `Color.FromNonPremultiplied(int r, int g, int b, int a)`.
    pub fn from_non_premultiplied(r: i32, g: i32, b: i32, a: i32) -> Color {
        let r = clamp_to_byte64(r as i64 * a as i64 / 255);
        let mut g = clamp_to_byte64(g as i64 * a as i64 / 255);
        let mut b = clamp_to_byte64(b as i64 * a as i64 / 255);
        let mut a = clamp_to_byte32(a);
        g <<= 8;
        b <<= 16;
        a <<= 24;
        Color {
            packed_value: (r | g | b | a) as u32,
        }
    }

    pub const fn packed_value(&self) -> u32 {
        self.packed_value
    }

    pub fn set_packed_value(&mut self, value: u32) {
        self.packed_value = value;
    }

    pub const fn r(&self) -> u8 {
        self.packed_value as u8
    }

    pub const fn g(&self) -> u8 {
        (self.packed_value >> 8) as u8
    }

    pub const fn b(&self) -> u8 {
        (self.packed_value >> 16) as u8
    }

    pub const fn a(&self) -> u8 {
        (self.packed_value >> 24) as u8
    }

    pub fn set_r(&mut self, value: u8) {
        self.packed_value = (self.packed_value & 0xFFFF_FF00) | value as u32;
    }

    pub fn set_g(&mut self, value: u8) {
        self.packed_value = (self.packed_value & 0xFFFF_00FF) | ((value as u32) << 8);
    }

    pub fn set_b(&mut self, value: u8) {
        self.packed_value = (self.packed_value & 0xFF00_FFFF) | ((value as u32) << 16);
    }

    pub fn set_a(&mut self, value: u8) {
        self.packed_value = (self.packed_value & 0x00FF_FFFF) | ((value as u32) << 24);
    }

    /// Channel bytes in R, G, B, A order (the renderer's upload format).
    pub const fn to_rgba_bytes(&self) -> [u8; 4] {
        self.packed_value.to_le_bytes()
    }

    pub fn to_vector3(&self) -> Vector3 {
        Vector3 {
            x: unpack_unorm(255, self.packed_value),
            y: unpack_unorm(255, self.packed_value >> 8),
            z: unpack_unorm(255, self.packed_value >> 16),
        }
    }

    pub fn to_vector4(&self) -> Vector4 {
        Vector4 {
            x: unpack_unorm(255, self.packed_value),
            y: unpack_unorm(255, self.packed_value >> 8),
            z: unpack_unorm(255, self.packed_value >> 16),
            w: unpack_unorm(255, self.packed_value >> 24),
        }
    }

    /// `Color.Lerp`: 16.16 fixed-point blend per channel.
    pub fn lerp(value1: Color, value2: Color, amount: f32) -> Color {
        let num = value1.packed_value;
        let num2 = value2.packed_value;
        let num3 = num as u8 as i32;
        let num4 = (num >> 8) as u8 as i32;
        let num5 = (num >> 16) as u8 as i32;
        let num6 = (num >> 24) as u8 as i32;
        let num7 = num2 as u8 as i32;
        let num8 = (num2 >> 8) as u8 as i32;
        let num9 = (num2 >> 16) as u8 as i32;
        let num10 = (num2 >> 24) as u8 as i32;
        let num11 = pack_unorm(65536.0, amount) as i32;
        let num12 = num3 + (((num7 - num3) * num11) >> 16);
        let num13 = num4 + (((num8 - num4) * num11) >> 16);
        let num14 = num5 + (((num9 - num5) * num11) >> 16);
        let num15 = num6 + (((num10 - num6) * num11) >> 16);
        Color {
            packed_value: (num12 | (num13 << 8) | (num14 << 16) | (num15 << 24)) as u32,
        }
    }

    /// `Color.Multiply(Color, float)`, identical to `operator *`.
    pub fn multiply(value: Color, scale: f32) -> Color {
        let num = value.packed_value;
        let mut num2 = num as u8 as u32;
        let mut num3 = (num >> 8) as u8 as u32;
        let mut num4 = (num >> 16) as u8 as u32;
        let mut num5 = (num >> 24) as u8 as u32;
        let scale = scale * 65536.0;
        // NaN falls through both tests and converts to 0, as `(uint)NaN` does on x64.
        let num6: u32 = if !(scale < 0.0) {
            if !(scale > 16777215.0) {
                scale as u32
            } else {
                16777215
            }
        } else {
            0
        };
        num2 = (num2 * num6) >> 16;
        num3 = (num3 * num6) >> 16;
        num4 = (num4 * num6) >> 16;
        num5 = (num5 * num6) >> 16;
        if num2 > 255 {
            num2 = 255;
        }
        if num3 > 255 {
            num3 = 255;
        }
        if num4 > 255 {
            num4 = 255;
        }
        if num5 > 255 {
            num5 = 255;
        }
        Color {
            packed_value: num2 | (num3 << 8) | (num4 << 16) | (num5 << 24),
        }
    }


    pub const TRANSPARENT: Color = Color::from_packed_value(0);
    pub const ALICE_BLUE: Color = Color::from_packed_value(4294965488);
    pub const ANTIQUE_WHITE: Color = Color::from_packed_value(4292340730);
    pub const AQUA: Color = Color::from_packed_value(4294967040);
    pub const AQUAMARINE: Color = Color::from_packed_value(4292149119);
    pub const AZURE: Color = Color::from_packed_value(4294967280);
    pub const BEIGE: Color = Color::from_packed_value(4292670965);
    pub const BISQUE: Color = Color::from_packed_value(4291093759);
    pub const BLACK: Color = Color::from_packed_value(4278190080);
    pub const BLANCHED_ALMOND: Color = Color::from_packed_value(4291685375);
    pub const BLUE: Color = Color::from_packed_value(4294901760);
    pub const BLUE_VIOLET: Color = Color::from_packed_value(4293012362);
    pub const BROWN: Color = Color::from_packed_value(4280953509);
    pub const BURLY_WOOD: Color = Color::from_packed_value(4287084766);
    pub const CADET_BLUE: Color = Color::from_packed_value(4288716383);
    pub const CHARTREUSE: Color = Color::from_packed_value(4278255487);
    pub const CHOCOLATE: Color = Color::from_packed_value(4280183250);
    pub const CORAL: Color = Color::from_packed_value(4283465727);
    pub const CORNFLOWER_BLUE: Color = Color::from_packed_value(4293760356);
    pub const CORNSILK: Color = Color::from_packed_value(4292671743);
    pub const CRIMSON: Color = Color::from_packed_value(4282127580);
    pub const CYAN: Color = Color::from_packed_value(4294967040);
    pub const DARK_BLUE: Color = Color::from_packed_value(4287299584);
    pub const DARK_CYAN: Color = Color::from_packed_value(4287335168);
    pub const DARK_GOLDENROD: Color = Color::from_packed_value(4278945464);
    pub const DARK_GRAY: Color = Color::from_packed_value(4289309097);
    pub const DARK_GREEN: Color = Color::from_packed_value(4278215680);
    pub const DARK_KHAKI: Color = Color::from_packed_value(4285249469);
    pub const DARK_MAGENTA: Color = Color::from_packed_value(4287299723);
    pub const DARK_OLIVE_GREEN: Color = Color::from_packed_value(4281297749);
    pub const DARK_ORANGE: Color = Color::from_packed_value(4278226175);
    pub const DARK_ORCHID: Color = Color::from_packed_value(4291572377);
    pub const DARK_RED: Color = Color::from_packed_value(4278190219);
    pub const DARK_SALMON: Color = Color::from_packed_value(4286224105);
    pub const DARK_SEA_GREEN: Color = Color::from_packed_value(4287347855);
    pub const DARK_SLATE_BLUE: Color = Color::from_packed_value(4287315272);
    pub const DARK_SLATE_GRAY: Color = Color::from_packed_value(4283387695);
    pub const DARK_TURQUOISE: Color = Color::from_packed_value(4291939840);
    pub const DARK_VIOLET: Color = Color::from_packed_value(4292018324);
    pub const DEEP_PINK: Color = Color::from_packed_value(4287829247);
    pub const DEEP_SKY_BLUE: Color = Color::from_packed_value(4294950656);
    pub const DIM_GRAY: Color = Color::from_packed_value(4285098345);
    pub const DODGER_BLUE: Color = Color::from_packed_value(4294938654);
    pub const FIREBRICK: Color = Color::from_packed_value(4280427186);
    pub const FLORAL_WHITE: Color = Color::from_packed_value(4293982975);
    pub const FOREST_GREEN: Color = Color::from_packed_value(4280453922);
    pub const FUCHSIA: Color = Color::from_packed_value(4294902015);
    pub const GAINSBORO: Color = Color::from_packed_value(4292664540);
    pub const GHOST_WHITE: Color = Color::from_packed_value(4294965496);
    pub const GOLD: Color = Color::from_packed_value(4278245375);
    pub const GOLDENROD: Color = Color::from_packed_value(4280329690);
    pub const GRAY: Color = Color::from_packed_value(4286611584);
    pub const GREEN: Color = Color::from_packed_value(4278222848);
    pub const GREEN_YELLOW: Color = Color::from_packed_value(4281335725);
    pub const HONEYDEW: Color = Color::from_packed_value(4293984240);
    pub const HOT_PINK: Color = Color::from_packed_value(4290013695);
    pub const INDIAN_RED: Color = Color::from_packed_value(4284243149);
    pub const INDIGO: Color = Color::from_packed_value(4286709835);
    pub const IVORY: Color = Color::from_packed_value(4293984255);
    pub const KHAKI: Color = Color::from_packed_value(4287424240);
    pub const LAVENDER: Color = Color::from_packed_value(4294633190);
    pub const LAVENDER_BLUSH: Color = Color::from_packed_value(4294308095);
    pub const LAWN_GREEN: Color = Color::from_packed_value(4278254716);
    pub const LEMON_CHIFFON: Color = Color::from_packed_value(4291689215);
    pub const LIGHT_BLUE: Color = Color::from_packed_value(4293318829);
    pub const LIGHT_CORAL: Color = Color::from_packed_value(4286611696);
    pub const LIGHT_CYAN: Color = Color::from_packed_value(4294967264);
    pub const LIGHT_GOLDENROD_YELLOW: Color = Color::from_packed_value(4292016890);
    pub const LIGHT_GREEN: Color = Color::from_packed_value(4287688336);
    pub const LIGHT_GRAY: Color = Color::from_packed_value(4292072403);
    pub const LIGHT_PINK: Color = Color::from_packed_value(4290885375);
    pub const LIGHT_SALMON: Color = Color::from_packed_value(4286226687);
    pub const LIGHT_SEA_GREEN: Color = Color::from_packed_value(4289376800);
    pub const LIGHT_SKY_BLUE: Color = Color::from_packed_value(4294626951);
    pub const LIGHT_SLATE_GRAY: Color = Color::from_packed_value(4288252023);
    pub const LIGHT_STEEL_BLUE: Color = Color::from_packed_value(4292789424);
    pub const LIGHT_YELLOW: Color = Color::from_packed_value(4292935679);
    pub const LIME: Color = Color::from_packed_value(4278255360);
    pub const LIME_GREEN: Color = Color::from_packed_value(4281519410);
    pub const LINEN: Color = Color::from_packed_value(4293325050);
    pub const MAGENTA: Color = Color::from_packed_value(4294902015);
    pub const MAROON: Color = Color::from_packed_value(4278190208);
    pub const MEDIUM_AQUAMARINE: Color = Color::from_packed_value(4289383782);
    pub const MEDIUM_BLUE: Color = Color::from_packed_value(4291624960);
    pub const MEDIUM_ORCHID: Color = Color::from_packed_value(4292040122);
    pub const MEDIUM_PURPLE: Color = Color::from_packed_value(4292571283);
    pub const MEDIUM_SEA_GREEN: Color = Color::from_packed_value(4285641532);
    pub const MEDIUM_SLATE_BLUE: Color = Color::from_packed_value(4293814395);
    pub const MEDIUM_SPRING_GREEN: Color = Color::from_packed_value(4288346624);
    pub const MEDIUM_TURQUOISE: Color = Color::from_packed_value(4291613000);
    pub const MEDIUM_VIOLET_RED: Color = Color::from_packed_value(4286911943);
    pub const MIDNIGHT_BLUE: Color = Color::from_packed_value(4285536537);
    pub const MINT_CREAM: Color = Color::from_packed_value(4294639605);
    pub const MISTY_ROSE: Color = Color::from_packed_value(4292994303);
    pub const MOCCASIN: Color = Color::from_packed_value(4290110719);
    pub const NAVAJO_WHITE: Color = Color::from_packed_value(4289584895);
    pub const NAVY: Color = Color::from_packed_value(4286578688);
    pub const OLD_LACE: Color = Color::from_packed_value(4293326333);
    pub const OLIVE: Color = Color::from_packed_value(4278222976);
    pub const OLIVE_DRAB: Color = Color::from_packed_value(4280520299);
    pub const ORANGE: Color = Color::from_packed_value(4278232575);
    pub const ORANGE_RED: Color = Color::from_packed_value(4278207999);
    pub const ORCHID: Color = Color::from_packed_value(4292243674);
    pub const PALE_GOLDENROD: Color = Color::from_packed_value(4289390830);
    pub const PALE_GREEN: Color = Color::from_packed_value(4288215960);
    pub const PALE_TURQUOISE: Color = Color::from_packed_value(4293848751);
    pub const PALE_VIOLET_RED: Color = Color::from_packed_value(4287852763);
    pub const PAPAYA_WHIP: Color = Color::from_packed_value(4292210687);
    pub const PEACH_PUFF: Color = Color::from_packed_value(4290370303);
    pub const PERU: Color = Color::from_packed_value(4282353101);
    pub const PINK: Color = Color::from_packed_value(4291543295);
    pub const PLUM: Color = Color::from_packed_value(4292714717);
    pub const POWDER_BLUE: Color = Color::from_packed_value(4293320880);
    pub const PURPLE: Color = Color::from_packed_value(4286578816);
    pub const RED: Color = Color::from_packed_value(4278190335);
    pub const ROSY_BROWN: Color = Color::from_packed_value(4287598524);
    pub const ROYAL_BLUE: Color = Color::from_packed_value(4292962625);
    pub const SADDLE_BROWN: Color = Color::from_packed_value(4279453067);
    pub const SALMON: Color = Color::from_packed_value(4285694202);
    pub const SANDY_BROWN: Color = Color::from_packed_value(4284523764);
    pub const SEA_GREEN: Color = Color::from_packed_value(4283927342);
    pub const SEA_SHELL: Color = Color::from_packed_value(4293850623);
    pub const SIENNA: Color = Color::from_packed_value(4281160352);
    pub const SILVER: Color = Color::from_packed_value(4290822336);
    pub const SKY_BLUE: Color = Color::from_packed_value(4293643911);
    pub const SLATE_BLUE: Color = Color::from_packed_value(4291648106);
    pub const SLATE_GRAY: Color = Color::from_packed_value(4287660144);
    pub const SNOW: Color = Color::from_packed_value(4294638335);
    pub const SPRING_GREEN: Color = Color::from_packed_value(4286578432);
    pub const STEEL_BLUE: Color = Color::from_packed_value(4290019910);
    pub const TAN: Color = Color::from_packed_value(4287411410);
    pub const TEAL: Color = Color::from_packed_value(4286611456);
    pub const THISTLE: Color = Color::from_packed_value(4292394968);
    pub const TOMATO: Color = Color::from_packed_value(4282868735);
    pub const TURQUOISE: Color = Color::from_packed_value(4291878976);
    pub const VIOLET: Color = Color::from_packed_value(4293821166);
    pub const WHEAT: Color = Color::from_packed_value(4289978101);
    pub const WHITE: Color = Color::from_packed_value(4294967295);
    pub const WHITE_SMOKE: Color = Color::from_packed_value(4294309365);
    pub const YELLOW: Color = Color::from_packed_value(4278255615);
    pub const YELLOW_GREEN: Color = Color::from_packed_value(4281519514);
}

impl Mul<f32> for Color {
    type Output = Color;

    /// `Color.operator *(Color, float)`.
    fn mul(self, scale: f32) -> Color {
        Color::multiply(self, scale)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_transparent() {
        assert_eq!(Color::default(), Color::TRANSPARENT);
        assert_eq!(Color::default().packed_value(), 0);
    }

    #[test]
    fn channel_accessors_and_bytes() {
        let c = Color::new(1, 2, 3, 4);
        assert_eq!((c.r(), c.g(), c.b(), c.a()), (1, 2, 3, 4));
        assert_eq!(c.packed_value(), 0x0403_0201);
        assert_eq!(c.to_rgba_bytes(), [1, 2, 3, 4]);
        let mut d = Color::WHITE;
        d.set_r(10);
        d.set_g(20);
        d.set_b(30);
        d.set_a(40);
        assert_eq!(d.packed_value(), 0x281E_140A);
        d.set_a(255);
        d.set_g(255);
        assert_eq!(d.packed_value(), 0xFF1E_FF0A);
        d.set_packed_value(7);
        assert_eq!(d.r(), 7);
    }

    #[test]
    fn from_ints_clamps() {
        let c = Color::from_ints(-5, 300, 128, 1000);
        assert_eq!((c.r(), c.g(), c.b(), c.a()), (0, 255, 128, 255));
        let c = Color::from_ints(0, 255, 1, 254);
        assert_eq!((c.r(), c.g(), c.b(), c.a()), (0, 255, 1, 254));
        let c = Color::from_ints(i32::MIN, i32::MAX, 0, -1);
        assert_eq!((c.r(), c.g(), c.b(), c.a()), (0, 255, 0, 0));
        // three-int constructor: alpha forced to 255
        let c = Color::from_ints_rgb(-5, 300, 17);
        assert_eq!((c.r(), c.g(), c.b(), c.a()), (0, 255, 17, 255));
        let c = Color::from_ints_rgb(1, 2, 3);
        assert_eq!(c.packed_value(), 0xFF03_0201);
    }

    #[test]
    fn from_floats_rounding() {
        // 0.5 * 255 = 127.5, ties to even -> 128
        let c = Color::from_floats(0.5, 0.5, 0.5, 0.5);
        assert_eq!((c.r(), c.g(), c.b(), c.a()), (128, 128, 128, 128));
        // just below the tie rounds down
        let below = f32::from_bits(0.5f32.to_bits() - 1);
        assert_eq!(Color::from_floats(below, 0.0, 0.0, 0.0).r(), 127);
        // 0.2 * 255 = 51.000004 -> 51
        assert_eq!(Color::from_floats(0.2, 0.0, 0.0, 0.0).r(), 51);
        // clamping, NaN, infinities
        let c = Color::from_floats(-1.0, 2.0, f32::NAN, f32::INFINITY);
        assert_eq!((c.r(), c.g(), c.b(), c.a()), (0, 255, 0, 255));
        assert_eq!(Color::from_floats(f32::NEG_INFINITY, 0.0, 0.0, 0.0).r(), 0);
        // alpha defaults to 1 for the rgb overloads
        let c = Color::from_floats_rgb(1.0, 0.0, 0.0);
        assert_eq!(c.packed_value(), 0xFF00_00FF);
        let c = Color::from_vector3(Vector3 { x: 0.0, y: 1.0, z: 0.0 });
        assert_eq!(c.packed_value(), 0xFF00_FF00);
        let c = Color::from_vector4(Vector4 { x: 0.0, y: 0.0, z: 1.0, w: 0.0 });
        assert_eq!(c.packed_value(), 0x00FF_0000);
    }

    #[test]
    fn multiply_fixed_point() {
        // scale 0.5 -> 32768/65536; v*32768>>16 == floor(v/2)
        let c = Color::new(255, 1, 200, 100) * 0.5;
        assert_eq!((c.r(), c.g(), c.b(), c.a()), (127, 0, 100, 50));
        // scale 1.0 is the identity
        let c = Color::new(255, 1, 200, 100) * 1.0;
        assert_eq!((c.r(), c.g(), c.b(), c.a()), (255, 1, 200, 100));
        // scale 2.0 clamps to 255
        let c = Color::new(255, 1, 200, 100) * 2.0;
        assert_eq!((c.r(), c.g(), c.b(), c.a()), (255, 2, 255, 200));
        // negative scale -> 0, NaN -> 0
        assert_eq!(Color::WHITE * -1.0, Color::from_packed_value(0));
        assert_eq!(Color::WHITE * f32::NAN, Color::from_packed_value(0));
        // huge scale caps the multiplier at 16777215: 1*16777215>>16 = 255
        let c = Color::new(1, 0, 255, 2) * 1.0e9;
        assert_eq!((c.r(), c.g(), c.b(), c.a()), (255, 0, 255, 255));
        // scale 0.3: 0.3f*65536f = 19660.8 -> 19660; 255*19660>>16 = 76
        let c = Color::new(255, 100, 0, 255) * 0.3;
        assert_eq!((c.r(), c.g(), c.b(), c.a()), (76, 29, 0, 76));
        assert_eq!(Color::multiply(Color::WHITE, 0.5), Color::WHITE * 0.5);
    }

    #[test]
    fn lerp_exact() {
        let a = Color::new(0, 255, 10, 200);
        let b = Color::new(255, 0, 20, 100);
        assert_eq!(Color::lerp(a, b, 0.0), a);
        assert_eq!(Color::lerp(a, b, 1.0), b);
        // amount 0.5 -> 32768; 255*32768>>16 = 127; (-255*32768)>>16 = -128 (arithmetic shift)
        // r: 0+127; g: 255-128; b: 10 + (10*32768>>16 = 5); a: 200 + (-100*32768>>16 = -50)
        let m = Color::lerp(a, b, 0.5);
        assert_eq!((m.r(), m.g(), m.b(), m.a()), (127, 127, 15, 150));
        // amount is clamped to 0..=1
        assert_eq!(Color::lerp(a, b, 5.0), b);
        assert_eq!(Color::lerp(a, b, -5.0), a);
        // amount 0.25 -> 16384: 255*16384>>16 = 63
        assert_eq!(Color::lerp(Color::BLACK, Color::WHITE, 0.25).r(), 63);
    }

    #[test]
    fn from_non_premultiplied_both() {
        // r = 255*128/255 = 128, g = 128*128/255 = 64 (64.25), b = 0, a = 128
        let c = Color::from_non_premultiplied(255, 128, 0, 128);
        assert_eq!((c.r(), c.g(), c.b(), c.a()), (128, 64, 0, 128));
        // out-of-range inputs clamp
        let c = Color::from_non_premultiplied(1000, -50, 0, 300);
        assert_eq!((c.r(), c.g(), c.b(), c.a()), (255, 0, 0, 255));
        // vector form: x*w*255 = 127.5 -> 128; 0.25*255 = 63.75 -> 64; w -> 128
        let c = Color::from_non_premultiplied_vector4(Vector4 { x: 1.0, y: 0.5, z: 0.0, w: 0.5 });
        assert_eq!((c.r(), c.g(), c.b(), c.a()), (128, 64, 0, 128));
    }

    #[test]
    fn to_vectors() {
        let c = Color::new(51, 102, 255, 0);
        let v = c.to_vector4();
        assert_eq!(v.x, 51.0 / 255.0);
        assert_eq!(v.y, 102.0 / 255.0);
        assert_eq!(v.z, 1.0);
        assert_eq!(v.w, 0.0);
        let v3 = c.to_vector3();
        assert_eq!((v3.x, v3.y, v3.z), (51.0 / 255.0, 102.0 / 255.0, 1.0));
        // round trip through float constructor
        assert_eq!(Color::from_vector4(v), c);
    }

    #[test]
    fn named_colors() {
        let c = Color::CORNFLOWER_BLUE;
        assert_eq!((c.r(), c.g(), c.b(), c.a()), (100, 149, 237, 255));
        assert_eq!(c.packed_value(), 4293760356);
        assert_eq!(Color::WHITE.packed_value(), 0xFFFF_FFFF);
        assert_eq!(Color::BLACK.packed_value(), 0xFF00_0000);
        assert_eq!(Color::TRANSPARENT.packed_value(), 0);
        let red = Color::RED;
        assert_eq!((red.r(), red.g(), red.b(), red.a()), (255, 0, 0, 255));
        let blue = Color::BLUE;
        assert_eq!((blue.r(), blue.g(), blue.b(), blue.a()), (0, 0, 255, 255));
        let lime = Color::LIME;
        assert_eq!((lime.r(), lime.g(), lime.b(), lime.a()), (0, 255, 0, 255));
        let gold = Color::GOLD;
        assert_eq!((gold.r(), gold.g(), gold.b(), gold.a()), (255, 215, 0, 255));
        assert_eq!(Color::CYAN, Color::AQUA);
        assert_eq!(Color::MAGENTA, Color::FUCHSIA);
        let yg = Color::YELLOW_GREEN;
        assert_eq!((yg.r(), yg.g(), yg.b(), yg.a()), (154, 205, 50, 255));
        let ab = Color::ALICE_BLUE;
        assert_eq!((ab.r(), ab.g(), ab.b(), ab.a()), (240, 248, 255, 255));
    }

    #[test]
    fn equality() {
        assert_eq!(Color::new(1, 2, 3, 4), Color::from_ints(1, 2, 3, 4));
        assert_ne!(Color::new(1, 2, 3, 4), Color::new(1, 2, 3, 5));
    }
}
