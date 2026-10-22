//! `Terraria.Utilities.UnifiedRandom` (decompile/Terraria/Terraria/Utilities/UnifiedRandom.cs).
//! Bit-exact: world generation and every random roll in the game depend on this sequence.

const MSEED: i32 = 161803398;

#[derive(Clone, Debug)]
pub struct UnifiedRandom {
    inext: u32,
    seed_array: [i32; 56],
}

impl UnifiedRandom {
    pub fn new(seed: i32) -> Self {
        let mut r = Self { inext: 0, seed_array: [0; 56] };
        r.set_seed(seed);
        r
    }

    pub fn set_seed(&mut self, seed: i32) {
        self.seed_array = [0; 56];
        let num = if seed == i32::MIN { i32::MAX } else { seed.abs() };
        let mut num2 = MSEED - num;
        self.seed_array[55] = num2;
        let mut num3: i32 = 1;
        for j in 1..55 {
            let num4 = 21 * j % 55;
            self.seed_array[num4] = num3;
            // C# int arithmetic wraps unless checked; these stay in range but wrap to match in every case
            num3 = num2.wrapping_sub(num3);
            if num3 < 0 {
                num3 = num3.wrapping_add(i32::MAX);
            }
            num2 = self.seed_array[num4];
        }
        for _ in 1..5 {
            for l in 1..56 {
                let other = self.seed_array[1 + (l + 30) % 55];
                self.seed_array[l] = self.seed_array[l].wrapping_sub(other);
                if self.seed_array[l] < 0 {
                    self.seed_array[l] = self.seed_array[l].wrapping_add(i32::MAX);
                }
            }
        }
        self.inext = 0;
    }

    fn internal_sample(&mut self) -> i32 {
        let mut num = self.inext + 1;
        if num > 55 {
            num = 1;
        }
        let mut num2 = num + 21;
        if num2 > 55 {
            num2 -= 55;
        }
        let mut num3 = self.seed_array[num as usize].wrapping_sub(self.seed_array[num2 as usize]);
        if num3 == i32::MAX {
            num3 -= 1;
        }
        num3 = num3.wrapping_add((num3 >> 31) & 0x7FFF_FFFF);
        self.seed_array[num as usize] = num3;
        self.inext = num;
        num3
    }

    #[inline]
    fn sample(&mut self) -> f64 {
        self.internal_sample() as f64 * 4.656612875245797E-10
    }

    /// The whole state (`inext`, `SeedArray`), as the reference harnesses read it by reflection (Common.cs RandState).
    pub fn state(&self) -> (u32, [i32; 56]) {
        (self.inext, self.seed_array)
    }

    /// `Peek()`: the next raw difference without advancing.
    pub fn peek(&self) -> i32 {
        let mut num = self.inext + 1;
        if num > 55 {
            num = 1;
        }
        let mut num2 = num + 21;
        if num2 > 55 {
            num2 -= 55;
        }
        self.seed_array[num as usize].wrapping_sub(self.seed_array[num2 as usize])
    }

    /// `Next()`
    #[inline]
    pub fn next_raw(&mut self) -> i32 {
        self.internal_sample()
    }

    fn sample_for_large_range(&mut self) -> f64 {
        let mut num = self.internal_sample();
        if self.internal_sample() % 2 == 0 {
            num = -num;
        }
        (num as f64 + 2147483646.0) / 4294967293.0
    }

    /// `Next(int maxValue)`. The original throws for a negative argument.
    #[inline]
    pub fn next(&mut self, max_value: i32) -> i32 {
        assert!(max_value >= 0, "maxValue must be positive.");
        (self.sample() * max_value as f64) as i32
    }

    /// `Next(int minValue, int maxValue)`
    pub fn next_range(&mut self, min_value: i32, max_value: i32) -> i32 {
        assert!(min_value <= max_value, "minValue must be less than maxValue");
        let num = max_value as i64 - min_value as i64;
        if num <= i32::MAX as i64 {
            (self.sample() * num as f64) as i32 + min_value
        } else {
            ((self.sample_for_large_range() * num as f64) as i64 + min_value as i64) as i32
        }
    }

    /// `NextDouble()`
    #[inline]
    pub fn next_double(&mut self) -> f64 {
        self.sample()
    }

    /// `NextFloat()` (Terraria.Utils extension): `(float)random.NextDouble()`.
    #[inline]
    pub fn next_float(&mut self) -> f32 {
        self.sample() as f32
    }

    /// `NextBytes(byte[])`
    pub fn next_bytes(&mut self, buffer: &mut [u8]) {
        for b in buffer {
            *b = (self.internal_sample() % 256) as u8;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// UnifiedRandom is System.Random's algorithm (Knuth subtractive generator); these are the
    /// well-known first outputs of `new System.Random(0)` and `new System.Random(42)` on .NET Framework.
    #[test]
    fn matches_dotnet_framework_random() {
        let mut r = UnifiedRandom::new(0);
        assert_eq!([r.next_raw(), r.next_raw(), r.next_raw()], [1559595546, 1755192844, 1649316166]);
        let mut r = UnifiedRandom::new(42);
        assert_eq!([r.next_raw(), r.next_raw(), r.next_raw()], [1434747710, 302596119, 269548474]);
        let mut r = UnifiedRandom::new(42);
        assert_eq!(r.next(100), 66);
    }

    #[test]
    fn peek_does_not_advance() {
        let mut r = UnifiedRandom::new(7);
        let p = r.peek();
        let n = r.next_raw();
        // Peek returns the raw difference; Next adds int.MaxValue when it is negative
        assert!(p == n || p + i32::MAX == n);
    }
}
