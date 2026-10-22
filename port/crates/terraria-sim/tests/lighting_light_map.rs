//! `LightMap.Blur` and the light masks on small maps. Every expected number is derived by hand from the formulas of
//! `LightMap.BlurLine` (LightMap.cs:127): a lit cell loses `LightDecayThrough*` per cell it leaves, a cell whose value is
//! below the running value takes the running value, and a channel stops being tracked once it is below 0.0185.
//! The maps have Height 1 so that the vertical lines are single cells (no-ops) and only the row lines act; the 3x3 case
//! uses both directions.

use terraria_core::xna::Vector3;
use terraria_sim::graphics::light::{LightMap, LightMaskMode};

fn map(width: i32, height: i32) -> LightMap {
    let mut m = LightMap::new();
    m.set_size(width, height);
    m.non_visible_padding = 0;
    m
}

fn row(m: &LightMap, width: i32) -> Vec<Vector3> {
    (0..width).map(|x| m.get(x, 0)).collect()
}

#[test]
fn air_spreads_by_091_per_cell_in_both_directions() {
    // 1.0 in the red channel of cell 2 of 5.
    // forward line: cells 3, 4 get 0.91, 0.91*0.91; backward line: cells 1, 0 get the same.
    // Second pass changes nothing (every cell already equals what the running value would write).
    let mut m = map(5, 1);
    m.set(2, 0, Vector3::new(1.0, 0.0, 0.0));
    m.blur();
    let d = 0.91f32;
    let got = row(&m, 5);
    assert_eq!(got[2], Vector3::new(1.0, 0.0, 0.0));
    assert_eq!(got[1].x, d);
    assert_eq!(got[3].x, d);
    assert_eq!(got[0].x, d * d);
    assert_eq!(got[4].x, d * d);
    // the other channels stay 0 (below 0.0185 they are not tracked at all)
    assert!(got.iter().all(|c| c.y == 0.0 && c.z == 0.0));
}

#[test]
fn solid_cells_attenuate_by_056() {
    // cell 3 is solid: light entering it is written (0.91) and then multiplied by 0.56 before cell 4.
    let mut m = map(5, 1);
    m.set(2, 0, Vector3::new(1.0, 0.0, 0.0));
    m.set_mask_at(3, 0, LightMaskMode::Solid);
    m.blur();
    let got = row(&m, 5);
    assert_eq!(got[3].x, 0.91f32);
    assert_eq!(got[4].x, 0.91f32 * 0.56f32);
    // the backward direction is unaffected by the solid cell (it is to the right of the source)
    assert_eq!(got[1].x, 0.91f32);
    assert_eq!(got[0].x, 0.91f32 * 0.91f32);
}

#[test]
fn cracked_bricks_attenuate_by_08() {
    let mut m = map(5, 1);
    m.set(2, 0, Vector3::new(0.0, 1.0, 0.0));
    m.set_mask_at(3, 0, LightMaskMode::CrackedBricks);
    m.blur();
    let got = row(&m, 5);
    assert_eq!(got[3].y, 0.91f32);
    assert_eq!(got[4].y, 0.91f32 * 0.8f32);
}

#[test]
fn honey_attenuates_each_channel_differently() {
    // white light in cell 0, honey in cell 1: the value written into cell 2 is 0.91 * (0.75, 0.7, 0.6) * 0.91 per channel
    let mut m = map(3, 1);
    m.set(0, 0, Vector3::new(1.0, 1.0, 1.0));
    m.set_mask_at(1, 0, LightMaskMode::Honey);
    m.blur();
    let got = row(&m, 3);
    assert_eq!(got[1], Vector3::new(0.91, 0.91, 0.91));
    assert_eq!(got[2].x, 0.91f32 * (0.75f32 * 0.91f32));
    assert_eq!(got[2].y, 0.91f32 * (0.7f32 * 0.91f32));
    assert_eq!(got[2].z, 0.91f32 * (0.6f32 * 0.91f32));
}

#[test]
fn water_uses_the_cell_random_factor() {
    // The factor is (98 + _random.WithModifier(index).Next(2)) / 100. With seed 0x1234, FastRandom's LCG
    // (seed * 25214903917 + 11) & (2^48 - 1) gives, for index 1: seed' = next(1) ^ 0x1234, bits = next(seed') >> 17,
    // Next(2) = (2 * bits) >> 31 = 0, so the factor is 0.98; for index 2 it is 1, factor 0.99 (worked out by hand,
    // see the comment on `fast_random_by_hand` below).
    let wd = 0.88f32 * 0.91f32;
    let mut m = map(3, 1);
    m.set_random_seed(0x1234);
    m.set(0, 0, Vector3::new(1.0, 0.0, 0.0));
    m.set_mask_at(1, 0, LightMaskMode::Water);
    m.blur();
    let got = row(&m, 3);
    assert_eq!(got[1].x, 0.91f32);
    assert_eq!(got[2].x, 0.91f32 * (wd * (98f32 / 100f32)));

    let mut m = map(4, 1);
    m.set_random_seed(0x1234);
    m.set(0, 0, Vector3::new(1.0, 0.0, 0.0));
    m.set_mask_at(2, 0, LightMaskMode::Water);
    m.blur();
    let got = row(&m, 4);
    assert_eq!(got[2].x, 0.91f32 * 0.91f32);
    assert_eq!(got[3].x, (0.91f32 * 0.91f32) * (wd * (99f32 / 100f32)));
}

/// The two factors above, recomputed here in integer arithmetic exactly as `FastRandom` is written in C#
/// (FastRandom.cs: `NextSeed(s) = (s * 25214903917 + 11) & 0xFFFFFFFFFFFF`, `NextBits(31) = (int)(seed >> 17)`,
/// `Next(2) = (int)((long)2 * NextBits(31) >> 31)`, `WithModifier(m) = new FastRandom(NextSeed(m) ^ Seed)`).
#[test]
fn fast_random_by_hand() {
    fn next_seed(s: u64) -> u64 {
        s.wrapping_mul(25214903917).wrapping_add(11) & 0xFFFF_FFFF_FFFF
    }
    let factor = |seed: u64, index: u64| -> u64 {
        let s = next_seed(index) ^ seed;
        let s = next_seed(s);
        let bits = (s >> 17) as i64;
        ((2 * bits) >> 31) as u64
    };
    assert_eq!(factor(0x1234, 1), 0);
    assert_eq!(factor(0x1234, 2), 1);
    let mut r = terraria_sim::light_support::FastRandom::new(0x1234).with_modifier(1);
    assert_eq!(r.next_range(98, 100), 98);
    let mut r = terraria_sim::light_support::FastRandom::new(0x1234).with_modifier(2);
    assert_eq!(r.next_range(98, 100), 99);
}

#[test]
fn non_visible_padding_shortens_the_lines() {
    // Width 5, padding 1: the forward row line runs over cells 0..=3 and the backward one over 4 down to 1.
    // Light at cell 4 therefore spreads leftwards only down to cell 1; cell 0 is never visited by the backward line.
    let mut m = map(5, 1);
    m.non_visible_padding = 1;
    m.set(4, 0, Vector3::new(1.0, 0.0, 0.0));
    m.blur();
    let got = row(&m, 5);
    let d = 0.91f32;
    assert_eq!(got[4].x, 1.0);
    assert_eq!(got[3].x, d);
    assert_eq!(got[2].x, d * d);
    assert_eq!(got[1].x, d * d * d);
    assert_eq!(got[0].x, 0.0);
}

#[test]
fn light_below_the_cutoff_is_dropped() {
    // 0.0185 is the cutoff: a running value below it stops being written (flag set) and costs no more decay.
    // 0.9 decays through cells 0.. with 0.91: 0.0185 is crossed after k steps, k = ceil(ln(0.0185/1)/ln(0.91)) = 42.
    let mut m = map(60, 1);
    m.set(0, 0, Vector3::new(1.0, 0.0, 0.0));
    m.blur();
    let mut expect = 1.0f32;
    for x in 1..60 {
        expect *= 0.91f32;
        let v = m.get(x, 0).x;
        if expect >= 0.0185f32 {
            assert_eq!(v, expect, "cell {x}");
        } else {
            assert_eq!(v, 0.0, "cell {x} must not be written once the running value is below 0.0185");
        }
    }
}

#[test]
fn two_dimensional_blur_runs_columns_then_rows() {
    // 3x3, red 1.0 in the centre. First pass: the centre column becomes [0.91, 1, 0.91] (down then up), then every row
    // is blurred: rows y=0 and y=2 become [0.91^2, 0.91, 0.91^2], row y=1 becomes [0.91, 1, 0.91].
    // The second pass changes nothing.
    let mut m = map(3, 3);
    m.set(1, 1, Vector3::new(1.0, 0.0, 0.0));
    m.blur();
    let d = 0.91f32;
    let want = [[d * d, d, d * d], [d, 1.0, d], [d * d, d, d * d]]; // [y][x]
    for y in 0..3 {
        for x in 0..3 {
            assert_eq!(m.get(x, y).x, want[y as usize][x as usize], "cell ({x}, {y})");
        }
    }
}

#[test]
fn clear_resets_colours_and_masks() {
    let mut m = map(3, 1);
    m.set(1, 0, Vector3::new(1.0, 1.0, 1.0));
    m.set_mask_at(1, 0, LightMaskMode::Solid);
    m.clear();
    assert_eq!(m.get(1, 0), Vector3::ZERO);
    assert_eq!(m.get_mask(1, 0), LightMaskMode::None);
}
