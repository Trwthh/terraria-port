//! Small callees of the lighting code that belong to other classes of the original and are not ported
//! anywhere else yet. Each function names its original; when the home module of that class is ported the
//! function moves there and this file shrinks.
//!
//! * `Terraria.Utilities.FastRandom` (FastRandom.cs)
//! * `Terraria.ID.TorchID.TorchColor` (TorchID.cs:171, table filled by `TorchID.Initialize`, TorchID.cs:138)
//! * `Terraria.Main.hslToRgb` / `hue2rgb` (Main.cs:48747, 48774)
//! * `Terraria.WorldGen.paintColor` (WorldGen.cs:44357), `WorldGen.SolidTile3` (WorldGen.cs:71277, 71286)
//! * `Terraria.GameContent.Liquid.LiquidRenderer.GetShimmerWave` / `GetShimmerBaseColor` (LiquidRenderer.cs:761, 803)
//! * `Terraria.GameContent.PortalHelper.GetPortalColor` (PortalHelper.cs:382)
//! * `Terraria.Utils.MultiplyRGBA` (Utils.cs:2015)

use terraria_core::xna::{math_helper, Color, Vector4};

use crate::game::Game;
use crate::world_gen::tile_frame::in_world;

/// `Terraria.Utilities.FastRandom` (FastRandom.cs): a 48-bit LCG with a per-call modifier. Value type in the
/// original (a struct), so `Clone + Copy` here.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FastRandom {
    seed: u64,
}

const RANDOM_MULTIPLIER: u64 = 25214903917;
const RANDOM_ADD: u64 = 11;
const RANDOM_MASK: u64 = 281474976710655;

impl FastRandom {
    /// `FastRandom(ulong seed)`
    pub const fn new(seed: u64) -> FastRandom {
        FastRandom { seed }
    }

    /// `FastRandom(int seed)`: `Seed = (ulong)seed` (sign-extends).
    pub const fn from_i32(seed: i32) -> FastRandom {
        FastRandom { seed: seed as i64 as u64 }
    }

    /// `FastRandom.Seed`
    pub const fn seed(&self) -> u64 {
        self.seed
    }

    /// `FastRandom.CreateWithRandomSeed()`: `(ulong)Guid.NewGuid().GetHashCode()`. A GUID has no counterpart in
    /// the port; the wall clock and a process-wide counter stand in for it. The value is only ever used as a
    /// starting seed, and callers that need repeatable output seed the engine explicitly
    /// (`LightingEngine::set_random_seeds`).
    pub fn create_with_random_seed() -> FastRandom {
        use std::sync::atomic::{AtomicU64, Ordering};
        use std::time::{SystemTime, UNIX_EPOCH};
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let nanos = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_nanos() as u64).unwrap_or(0);
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let mixed = nanos ^ n.wrapping_mul(0x9E37_79B9_7F4A_7C15);
        let hash = (mixed ^ (mixed >> 32)) as u32 as i32;
        FastRandom::from_i32(hash)
    }

    /// `WithModifier(ulong modifier)`
    pub fn with_modifier(&self, modifier: u64) -> FastRandom {
        FastRandom::new(next_seed_of(modifier) ^ self.seed)
    }

    /// `WithModifier(int x, int y)`: `WithModifier((ulong)(x + 2654435769u + ((long)y << 6)) + ((ulong)y >> 2))`
    pub fn with_modifier_xy(&self, x: i32, y: i32) -> FastRandom {
        // `x + 2654435769u` is long arithmetic (int + uint); `(ulong)y` of an int sign-extends.
        let a = (x as i64).wrapping_add(2654435769).wrapping_add((y as i64) << 6) as u64;
        self.with_modifier(a.wrapping_add((y as i64 as u64) >> 2))
    }

    /// `NextSeed()`
    pub fn next_seed(&mut self) {
        self.seed = next_seed_of(self.seed);
    }

    fn next_bits(&mut self, bits: i32) -> i32 {
        self.seed = next_seed_of(self.seed);
        (self.seed >> (48 - bits)) as i32
    }

    /// `NextFloat()`
    pub fn next_float(&mut self) -> f32 {
        self.next_bits(24) as f32 * 5.9604645E-08f32
    }

    /// `NextDouble()`: the product is a float, widened afterwards, as in the original.
    pub fn next_double(&mut self) -> f64 {
        (self.next_bits(32) as f32 * 4.656613E-10f32) as f64
    }

    /// `Next(int max)`
    pub fn next(&mut self, max: i32) -> i32 {
        if (max & max.wrapping_neg()) == max {
            return ((max as i64).wrapping_mul(self.next_bits(31) as i64) >> 31) as i32;
        }
        loop {
            let num = self.next_bits(31);
            let num2 = num.wrapping_rem(max);
            if num.wrapping_sub(num2).wrapping_add(max.wrapping_sub(1)) >= 0 {
                return num2;
            }
        }
    }

    /// `Next(int min, int max)`
    pub fn next_range(&mut self, min: i32, max: i32) -> i32 {
        self.next(max.wrapping_sub(min)).wrapping_add(min)
    }
}

/// `FastRandom.NextSeed(ulong seed)` (private static)
fn next_seed_of(seed: u64) -> u64 {
    seed.wrapping_mul(RANDOM_MULTIPLIER).wrapping_add(RANDOM_ADD) & RANDOM_MASK
}

/// `TorchID.TorchColor(int torchID, out float R, out float G, out float B)` (TorchID.cs:171).
/// The `_lights` table of `TorchID.Initialize` (TorchID.cs:138) is the `match`; `ConstantTorchLight` entries
/// are literals, `DemonTorchLight`, `DiscoTorchLight` and `ShimmerTorchLight` read `Main` state.
pub fn torch_color(g: &Game, torch_id: i32, r: &mut f32, g_: &mut f32, b: &mut f32) {
    if !(0..24).contains(&torch_id) {
        *r = 0.0;
        *g_ = 0.0;
        *b = 0.0;
        return;
    }
    let (cr, cg, cb) = match torch_id {
        0 => (1f32, 0.95f32, 0.8f32),
        1 => (0f32, 0.1f32, 1.3f32),
        2 => (1f32, 0.1f32, 0.1f32),
        3 => (0f32, 1f32, 0.1f32),
        4 => (0.9f32, 0f32, 0.9f32),
        5 => (1.4f32, 1.4f32, 1.4f32),
        6 => (0.9f32, 0.9f32, 0f32),
        // DemonTorchLight.GetRGB (TorchID.cs:48)
        7 => (
            0.5f32 * g.main.demon_torch + (1f32 - g.main.demon_torch),
            0.3f32,
            g.main.demon_torch + 0.5f32 * (1f32 - g.main.demon_torch),
        ),
        8 => (1f32, 1.6f32, 0.5f32),
        9 => (0.75f32, 0.85f32, 1.4f32),
        10 => (1f32, 0.5f32, 0f32),
        11 => (1.4f32, 1.4f32, 0.7f32),
        12 => (0.75f32, 1.3499999f32, 1.5f32),
        13 => (0.95f32, 0.75f32, 1.3f32),
        // DiscoTorchLight.GetRGB (TorchID.cs:71)
        14 => (
            g.main.disco_r as f32 / 255f32,
            g.main.disco_g as f32 / 255f32,
            g.main.disco_b as f32 / 255f32,
        ),
        15 => (1f32, 0f32, 1f32),
        16 => (1.4f32, 0.85f32, 0.55f32),
        17 => (0.25f32, 1.3f32, 0.8f32),
        18 => (0.95f32, 0.4f32, 1.4f32),
        19 => (1.4f32, 0.7f32, 0.5f32),
        20 => (1.25f32, 0.6f32, 1.2f32),
        21 => (0.75f32, 1.45f32, 0.9f32),
        22 => (0.3f32, 0.78f32, 1.2f32),
        // ShimmerTorchLight.GetRGB (TorchID.cs:56)
        23 => {
            let mut num = 0.9f32;
            let mut num2 = 0.9f32;
            num += (270 - g.main.mouse_text_color as i32) as f32 / 900f32;
            num2 += (270 - g.main.mouse_text_color as i32) as f32 / 125f32;
            num = math_helper::clamp(num, 0f32, 1f32);
            num2 = math_helper::clamp(num2, 0f32, 1f32);
            (num * 0.9f32, num2 * 0.55f32, num * 1.2f32)
        }
        _ => (0f32, 0f32, 0f32),
    };
    *r = cr;
    *g_ = cg;
    *b = cb;
}

/// `Main.hslToRgb(float Hue, float Saturation, float Luminosity, byte a = byte.MaxValue)` (Main.cs:48747).
/// `(byte)Math.Round(double)` is round-half-to-even; values leave 0..=255 only for hues outside 0..1.
pub fn hsl_to_rgb(hue: f32, saturation: f32, luminosity: f32, a: u8) -> Color {
    let r: u8;
    let g: u8;
    let b: u8;
    if saturation == 0f32 {
        r = (luminosity as f64 * 255.0).round_ties_even() as u8;
        g = (luminosity as f64 * 255.0).round_ties_even() as u8;
        b = (luminosity as f64 * 255.0).round_ties_even() as u8;
    } else {
        let num = hue as f64;
        let num2 = if !((luminosity as f64) < 0.5) {
            (luminosity + saturation - luminosity * saturation) as f64
        } else {
            luminosity as f64 * (1.0 + saturation as f64)
        };
        let t = 2.0 * luminosity as f64 - num2;
        let mut c = num + 1.0 / 3.0;
        let mut c2 = num;
        let c3 = num - 1.0 / 3.0;
        c = hue2rgb(c, t, num2);
        c2 = hue2rgb(c2, t, num2);
        let num3 = hue2rgb(c3, t, num2);
        r = (c * 255.0).round_ties_even() as u8;
        g = (c2 * 255.0).round_ties_even() as u8;
        b = (num3 * 255.0).round_ties_even() as u8;
    }
    Color::new(r, g, b, a)
}

/// `Main.hue2rgb(double c, double t1, double t2)` (Main.cs:48774)
pub fn hue2rgb(c: f64, t1: f64, t2: f64) -> f64 {
    let mut c = c;
    if c < 0.0 {
        c += 1.0;
    }
    if c > 1.0 {
        c -= 1.0;
    }
    if 6.0 * c < 1.0 {
        return t1 + (t2 - t1) * 6.0 * c;
    }
    if 2.0 * c < 1.0 {
        return t2;
    }
    if 3.0 * c < 2.0 {
        return t1 + (t2 - t1) * (2.0 / 3.0 - c) * 6.0;
    }
    t1
}

/// `WorldGen.paintColor(int color)` (WorldGen.cs:44357)
pub fn paint_color(color: i32) -> Color {
    let mut white = Color::WHITE;
    if color == 1 || color == 13 {
        white.set_r(255);
        white.set_g(0);
        white.set_b(0);
    }
    if color == 2 || color == 14 {
        white.set_r(255);
        white.set_g(127);
        white.set_b(0);
    }
    if color == 3 || color == 15 {
        white.set_r(255);
        white.set_g(255);
        white.set_b(0);
    }
    if color == 4 || color == 16 {
        white.set_r(127);
        white.set_g(255);
        white.set_b(0);
    }
    if color == 5 || color == 17 {
        white.set_r(0);
        white.set_g(255);
        white.set_b(0);
    }
    if color == 6 || color == 18 {
        white.set_r(0);
        white.set_g(255);
        white.set_b(127);
    }
    if color == 7 || color == 19 {
        white.set_r(0);
        white.set_g(255);
        white.set_b(255);
    }
    if color == 8 || color == 20 {
        white.set_r(0);
        white.set_g(127);
        white.set_b(255);
    }
    if color == 9 || color == 21 {
        white.set_r(0);
        white.set_g(0);
        white.set_b(255);
    }
    if color == 10 || color == 22 {
        white.set_r(127);
        white.set_g(0);
        white.set_b(255);
    }
    if color == 11 || color == 23 {
        white.set_r(255);
        white.set_g(0);
        white.set_b(255);
    }
    if color == 12 || color == 24 {
        white.set_r(255);
        white.set_g(0);
        white.set_b(127);
    }
    if color == 25 {
        white.set_r(75);
        white.set_g(75);
        white.set_b(75);
    }
    if color == 26 {
        white.set_r(255);
        white.set_g(255);
        white.set_b(255);
    }
    if color == 27 {
        white.set_r(175);
        white.set_g(175);
        white.set_b(175);
    }
    if color == 28 {
        white.set_r(255);
        white.set_g(178);
        white.set_b(125);
    }
    if color == 29 {
        white.set_r(25);
        white.set_g(25);
        white.set_b(25);
    }
    if color == 30 {
        white.set_r(200);
        white.set_g(200);
        white.set_b(200);
        white.set_a(150);
    }
    white
}

/// `WorldGen.SolidTile3(int i, int j)` (WorldGen.cs:71277) with `SolidTile3(Tile t)` (WorldGen.cs:71286) inlined.
pub fn solid_tile3(g: &Game, i: i32, j: i32) -> bool {
    if !in_world(g, i, j, 1) {
        return false;
    }
    let t = g.main.tile.get(i, j);
    // `t == null` cannot happen: tiles are never null in the port.
    if t.nactive() && g.main.tile_solid[t.type_ as usize] {
        return !g.main.tile_solid_top[t.type_ as usize];
    }
    false
}

/// `LiquidRenderer.GetShimmerWave(ref float worldPositionX, ref float worldPositionY)` (LiquidRenderer.cs:761)
pub fn get_shimmer_wave(g: &Game, world_position_x: f32, world_position_y: f32) -> f32 {
    (((((world_position_x + world_position_y / 6f32) / 10f32) as f64 - g.main.time_for_visual_effects / 360.0)
        * 6.2831854820251465_f64)
        .sin()) as f32
}

/// `LiquidRenderer.GetShimmerBaseColor(float worldPositionX, float worldPositionY)` (LiquidRenderer.cs:803)
pub fn get_shimmer_base_color(g: &Game, world_position_x: f32, world_position_y: f32) -> Vector4 {
    let shimmer_wave = get_shimmer_wave(g, world_position_x, world_position_y);
    Vector4::lerp(
        Vector4::new(0.64705884f32, 26f32 / 51f32, 14f32 / 15f32, 1f32),
        Vector4::new(41f32 / 51f32, 41f32 / 51f32, 1f32, 1f32),
        0.1f32 + shimmer_wave * 0.4f32,
    )
}

/// `PortalHelper.GetPortalColor(int player, int portal)` (PortalHelper.cs:382)
pub fn get_portal_color(g: &Game, player: i32, portal: i32) -> Color {
    // The original starts from `Color.White` and overwrites it in both branches.
    let mut white = if g.main.net_mode == 0 {
        if portal != 0 { hsl_to_rgb(0.52f32, 1f32, 0.6f32, 255) } else { hsl_to_rgb(0.12f32, 1f32, 0.5f32, 255) }
    } else {
        let num = 0.08f32;
        hsl_to_rgb((0.5f32 + player as f32 * (num * 2f32) + portal as f32 * num) % 1f32, 1f32, 0.5f32, 255)
    };
    white.set_a(66);
    white
}

/// `Utils.MultiplyRGBA(this Color firstColor, Color secondColor)` (Utils.cs:2015)
pub fn multiply_rgba(first_color: Color, second_color: Color) -> Color {
    Color::new(
        ((first_color.r() as i32 * second_color.r() as i32) as f32 / 255f32) as u8,
        ((first_color.g() as i32 * second_color.g() as i32) as f32 / 255f32) as u8,
        ((first_color.b() as i32 * second_color.b() as i32) as f32 / 255f32) as u8,
        ((first_color.a() as i32 * second_color.a() as i32) as f32 / 255f32) as u8,
    )
}
