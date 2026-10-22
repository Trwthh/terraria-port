//! `Terraria.Graphics.Light.LightMap` (LightMap.cs): a grid of light colours plus a per-cell mask, and the
//! two-pass blur that spreads light through it.

use terraria_core::xna::Vector3;

use super::light_mask_mode::LightMaskMode;
use crate::light_support::FastRandom;

/// `LightMap`
pub struct LightMap {
    /// `_colors`
    colors: Vec<Vector3>,
    /// `_mask`
    mask: Vec<LightMaskMode>,
    /// `_random` (`FastRandom.CreateWithRandomSeed()`)
    random: FastRandom,
    /// `NonVisiblePadding`
    pub non_visible_padding: i32,
    width: i32,
    height: i32,
    /// `LightDecayThroughAir`
    pub light_decay_through_air: f32,
    /// `LightDecayThroughSolid`
    pub light_decay_through_solid: f32,
    /// `LightDecayThroughCrackedBrick`
    pub light_decay_through_cracked_brick: f32,
    /// `LightDecayThroughWater`
    pub light_decay_through_water: Vector3,
    /// `LightDecayThroughHoney`
    pub light_decay_through_honey: Vector3,
}

impl LightMap {
    const DEFAULT_WIDTH: i32 = 203;
    const DEFAULT_HEIGHT: i32 = 203;

    /// `LightMap()` (LightMap.cs:57)
    pub fn new() -> LightMap {
        LightMap {
            colors: vec![Vector3::ZERO; 41209],
            mask: vec![LightMaskMode::None; 41209],
            random: FastRandom::create_with_random_seed(),
            non_visible_padding: 0,
            width: Self::DEFAULT_WIDTH,
            height: Self::DEFAULT_HEIGHT,
            light_decay_through_air: 0.91f32,
            light_decay_through_solid: 0.56f32,
            light_decay_through_cracked_brick: 0.8f32,
            light_decay_through_water: Vector3::new(0.88f32, 0.96f32, 1.015f32) * 0.91f32,
            light_decay_through_honey: Vector3::new(0.75f32, 0.7f32, 0.6f32) * 0.91f32,
        }
    }

    /// `Width`
    pub fn width(&self) -> i32 {
        self.width
    }

    /// `Height`
    pub fn height(&self) -> i32 {
        self.height
    }

    /// The seed of `_random` (the original has no accessor; used to make runs repeatable).
    pub fn set_random_seed(&mut self, seed: u64) {
        self.random = FastRandom::new(seed);
    }

    /// `_random` as it is now.
    pub fn random(&self) -> FastRandom {
        self.random
    }

    /// `this[int x, int y]` getter
    pub fn get(&self, x: i32, y: i32) -> Vector3 {
        self.colors[self.index_of(x, y) as usize]
    }

    /// `this[int x, int y]` setter
    pub fn set(&mut self, x: i32, y: i32, value: Vector3) {
        let i = self.index_of(x, y) as usize;
        self.colors[i] = value;
    }

    /// `GetLight(int x, int y, out Vector3 color)`
    pub fn get_light(&self, x: i32, y: i32) -> Vector3 {
        self.colors[self.index_of(x, y) as usize]
    }

    /// `GetMask(int x, int y)`
    pub fn get_mask(&self, x: i32, y: i32) -> LightMaskMode {
        self.mask[self.index_of(x, y) as usize]
    }

    /// `Clear()`
    pub fn clear(&mut self) {
        for i in 0..self.colors.len() {
            self.colors[i].x = 0f32;
            self.colors[i].y = 0f32;
            self.colors[i].z = 0f32;
            self.mask[i] = LightMaskMode::None;
        }
    }

    /// `SetMaskAt(int x, int y, LightMaskMode mode)`
    pub fn set_mask_at(&mut self, x: i32, y: i32, mode: LightMaskMode) {
        let i = self.index_of(x, y) as usize;
        self.mask[i] = mode;
    }

    /// `Blur()`
    pub fn blur(&mut self) {
        self.blur_pass();
        self.blur_pass();
        self.random.next_seed();
    }

    /// `BlurPass()` (LightMap.cs:104).
    ///
    /// Both `FastParallel.For` calls of the original run here as plain loops in index order. That gives the
    /// same result because the ranges never touch each other:
    /// * first `For` (over `i` in `0..Width`): iteration `i` reads and writes only the cells
    ///   `i * Height .. (i + 1) * Height` (its own column; its two `BlurLine`s run one after the other inside the
    ///   iteration, as here) and only reads `_mask`, the decay settings and `_random.WithModifier(index)`, which
    ///   does not change `_random`;
    /// * second `For` (over `i` in `0..Height`): iteration `i` touches only the cells `x * Height + i` (its own
    ///   row), reads the same read-only data;
    /// * the second `For` starts after the first has finished (the original waits for the countdown), as here.
    fn blur_pass(&mut self) {
        // FastParallel.For(0, Width, ...) -- columns are independent
        for i in 0..self.width {
            self.blur_line(self.index_of(i, 0), self.index_of(i, self.height - 1 - self.non_visible_padding), 1);
            self.blur_line(self.index_of(i, self.height - 1), self.index_of(i, self.non_visible_padding), -1);
        }
        // FastParallel.For(0, Height, ...) -- rows are independent
        for i in 0..self.height {
            self.blur_line(self.index_of(0, i), self.index_of(self.width - 1 - self.non_visible_padding, i), self.height);
            self.blur_line(self.index_of(self.width - 1, i), self.index_of(self.non_visible_padding, i), -self.height);
        }
    }

    /// `BlurLine(int startIndex, int endIndex, int stride)` (LightMap.cs:127)
    fn blur_line(&mut self, start_index: i32, end_index: i32, stride: i32) {
        let mut zero = Vector3::ZERO;
        let mut flag = false;
        let mut flag2 = false;
        let mut flag3 = false;
        let mut i = start_index;
        while i != end_index + stride {
            let iu = i as usize;
            if zero.x < self.colors[iu].x {
                zero.x = self.colors[iu].x;
                flag = false;
            } else if !flag {
                if zero.x < 0.0185f32 {
                    flag = true;
                } else {
                    self.colors[iu].x = zero.x;
                }
            }
            if zero.y < self.colors[iu].y {
                zero.y = self.colors[iu].y;
                flag2 = false;
            } else if !flag2 {
                if zero.y < 0.0185f32 {
                    flag2 = true;
                } else {
                    self.colors[iu].y = zero.y;
                }
            }
            if zero.z < self.colors[iu].z {
                zero.z = self.colors[iu].z;
                flag3 = false;
            } else if !flag3 {
                if zero.z < 0.0185f32 {
                    flag3 = true;
                } else {
                    self.colors[iu].z = zero.z;
                }
            }
            if flag && flag3 && flag2 {
                i += stride;
                continue;
            }
            match self.mask[iu] {
                LightMaskMode::None => {
                    if !flag {
                        zero.x *= self.light_decay_through_air;
                    }
                    if !flag2 {
                        zero.y *= self.light_decay_through_air;
                    }
                    if !flag3 {
                        zero.z *= self.light_decay_through_air;
                    }
                }
                LightMaskMode::Solid => {
                    if !flag {
                        zero.x *= self.light_decay_through_solid;
                    }
                    if !flag2 {
                        zero.y *= self.light_decay_through_solid;
                    }
                    if !flag3 {
                        zero.z *= self.light_decay_through_solid;
                    }
                }
                LightMaskMode::CrackedBricks => {
                    if !flag {
                        zero.x *= self.light_decay_through_cracked_brick;
                    }
                    if !flag2 {
                        zero.y *= self.light_decay_through_cracked_brick;
                    }
                    if !flag3 {
                        zero.z *= self.light_decay_through_cracked_brick;
                    }
                }
                LightMaskMode::Water => {
                    let num = self.random.with_modifier(i as u64).next_range(98, 100) as f32 / 100f32;
                    if !flag {
                        zero.x *= self.light_decay_through_water.x * num;
                    }
                    if !flag2 {
                        zero.y *= self.light_decay_through_water.y * num;
                    }
                    if !flag3 {
                        zero.z *= self.light_decay_through_water.z * num;
                    }
                }
                LightMaskMode::Honey => {
                    if !flag {
                        zero.x *= self.light_decay_through_honey.x;
                    }
                    if !flag2 {
                        zero.y *= self.light_decay_through_honey.y;
                    }
                    if !flag3 {
                        zero.z *= self.light_decay_through_honey.z;
                    }
                }
            }
            i += stride;
        }
    }

    /// `IndexOf(int x, int y)`
    fn index_of(&self, x: i32, y: i32) -> i32 {
        x * self.height + y
    }

    /// `SetSize(int width, int height)`
    pub fn set_size(&mut self, width: i32, height: i32) {
        if width * height > self.colors.len() as i32 {
            self.colors = vec![Vector3::ZERO; (width * height) as usize];
            self.mask = vec![LightMaskMode::None; (width * height) as usize];
        }
        self.width = width;
        self.height = height;
    }
}

impl Default for LightMap {
    fn default() -> Self {
        LightMap::new()
    }
}
