//! `Microsoft.Xna.Framework.MathHelper` (reference/xna/MathHelper.cs).
//! All arithmetic is f32 in the same operation order as the reference.

pub const E: f32 = core::f32::consts::E;
pub const LOG2_E: f32 = 1.442695;
pub const LOG10_E: f32 = 0.4342945;
pub const PI: f32 = core::f32::consts::PI;
pub const TWO_PI: f32 = PI * 2.0;
pub const PI_OVER2: f32 = PI / 2.0;
pub const PI_OVER4: f32 = PI / 4.0;

pub fn to_radians(degrees: f32) -> f32 {
    degrees * (PI / 180.0)
}

pub fn to_degrees(radians: f32) -> f32 {
    radians * (180.0 / PI)
}

pub fn distance(value1: f32, value2: f32) -> f32 {
    (value1 - value2).abs()
}

/// `Math.Min(float, float)`: NaN propagates, and -0 is smaller than +0.
pub fn min(value1: f32, value2: f32) -> f32 {
    if value1 < value2 {
        value1
    } else if !value1.is_nan() {
        if value1 == value2 && value1.is_sign_negative() {
            value1
        } else {
            value2
        }
    } else {
        value1
    }
}

/// `Math.Max(float, float)`: NaN propagates, and +0 is larger than -0.
pub fn max(value1: f32, value2: f32) -> f32 {
    if value1 > value2 {
        value1
    } else if !value1.is_nan() {
        if value1 == value2 && value2.is_sign_negative() {
            value1
        } else {
            value2
        }
    } else {
        value1
    }
}

pub fn clamp(value: f32, min: f32, max: f32) -> f32 {
    let value = if value > max { max } else { value };
    if value < min {
        min
    } else {
        value
    }
}

pub fn lerp(value1: f32, value2: f32, amount: f32) -> f32 {
    value1 + (value2 - value1) * amount
}

pub fn barycentric(value1: f32, value2: f32, value3: f32, amount1: f32, amount2: f32) -> f32 {
    value1 + amount1 * (value2 - value1) + amount2 * (value3 - value1)
}

pub fn smooth_step(value1: f32, value2: f32, amount: f32) -> f32 {
    let num = clamp(amount, 0.0, 1.0);
    lerp(value1, value2, num * num * (3.0 - 2.0 * num))
}

pub fn catmull_rom(value1: f32, value2: f32, value3: f32, value4: f32, amount: f32) -> f32 {
    let num = amount * amount;
    let num2 = amount * num;
    0.5 * (2.0 * value2
        + (0.0 - value1 + value3) * amount
        + (2.0 * value1 - 5.0 * value2 + 4.0 * value3 - value4) * num
        + (0.0 - value1 + 3.0 * value2 - 3.0 * value3 + value4) * num2)
}

pub fn hermite(value1: f32, tangent1: f32, value2: f32, tangent2: f32, amount: f32) -> f32 {
    let num = amount * amount;
    let num2 = amount * num;
    let num3 = 2.0 * num2 - 3.0 * num + 1.0;
    let num4 = -2.0 * num2 + 3.0 * num;
    let num5 = num2 - 2.0 * num + amount;
    let num6 = num2 - num;
    value1 * num3 + value2 * num4 + tangent1 * num5 + tangent2 * num6
}

/// `Math.IEEERemainder(x, y)`: `x - y * n` where `n` is `x / y` rounded to the
/// nearest integer, ties to even. Computed exactly through `fmod`.
pub fn ieee_remainder(x: f64, y: f64) -> f64 {
    if x.is_nan() {
        return x;
    }
    if y.is_nan() {
        return y;
    }
    if x.is_infinite() {
        return f64::NAN;
    }
    if y.is_infinite() {
        return x;
    }
    let regular_mod = x % y;
    if regular_mod.is_nan() {
        return f64::NAN;
    }
    if regular_mod == 0.0 && x.is_sign_negative() {
        return -0.0;
    }
    let sign = if x > 0.0 {
        1.0
    } else if x < 0.0 {
        -1.0
    } else {
        0.0
    };
    let alternative = regular_mod - y.abs() * sign;
    if alternative.abs() == regular_mod.abs() {
        let division = x / y;
        let rounded = division.round_ties_even();
        if rounded.abs() > division.abs() {
            alternative
        } else {
            regular_mod
        }
    } else if alternative.abs() < regular_mod.abs() {
        alternative
    } else {
        regular_mod
    }
}

pub fn wrap_angle(angle: f32) -> f32 {
    // 6.2831854820251465 is TWO_PI widened to double.
    let mut angle = ieee_remainder(angle as f64, 6.2831854820251465) as f32;
    if angle <= -PI {
        angle += PI * 2.0;
    } else if angle > PI {
        angle -= PI * 2.0;
    }
    angle
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constants() {
        assert_eq!(PI.to_bits(), 0x4049_0fdb);
        assert_eq!(TWO_PI.to_bits(), 0x40c9_0fdb);
        assert_eq!(PI_OVER2.to_bits(), 0x3fc9_0fdb);
        assert_eq!(PI_OVER4.to_bits(), 0x3f49_0fdb);
        assert_eq!(TWO_PI as f64, 6.2831854820251465);
        assert_eq!(LOG2_E, 1.442695);
        assert_eq!(LOG10_E, 0.4342945);
    }

    #[test]
    fn radians_degrees() {
        assert_eq!(to_radians(0.0), 0.0);
        let k = PI / 180.0;
        assert_eq!(to_radians(90.0), 90.0 * k);
        assert!((to_radians(180.0) - PI).abs() < 1e-6);
        assert!((to_degrees(PI) - 180.0).abs() < 1e-4);
        assert_eq!(to_degrees(2.0), 2.0 * (180.0 / PI));
    }

    #[test]
    fn min_max_clamp_distance() {
        assert_eq!(min(1.0, 2.0), 1.0);
        assert_eq!(max(1.0, 2.0), 2.0);
        assert!(min(f32::NAN, 1.0).is_nan());
        assert!(max(1.0, f32::NAN).is_nan());
        assert!(min(-0.0, 0.0).is_sign_negative());
        assert!(max(-0.0, 0.0).is_sign_positive());
        assert_eq!(clamp(5.0, 0.0, 1.0), 1.0);
        assert_eq!(clamp(-5.0, 0.0, 1.0), 0.0);
        assert_eq!(clamp(0.25, 0.0, 1.0), 0.25);
        // min > max: max applied first, then min wins
        assert_eq!(clamp(0.5, 1.0, 0.0), 1.0);
        assert_eq!(distance(3.0, 7.5), 4.5);
    }

    #[test]
    fn lerp_family() {
        assert_eq!(lerp(10.0, 20.0, 0.25), 12.5);
        assert_eq!(barycentric(1.0, 3.0, 7.0, 0.5, 0.25), 1.0 + 0.5 * 2.0 + 0.25 * 6.0);
        // smooth_step(0,10,0.5): t = 0.5*0.5*(3-1) = 0.5
        assert_eq!(smooth_step(0.0, 10.0, 0.5), 5.0);
        assert_eq!(smooth_step(0.0, 10.0, 2.0), 10.0);
        assert_eq!(smooth_step(0.0, 10.0, -1.0), 0.0);
        // hermite at amount 0 returns value1, at 1 returns value2
        assert_eq!(hermite(2.0, 5.0, 9.0, -3.0, 0.0), 2.0);
        assert_eq!(hermite(2.0, 5.0, 9.0, -3.0, 1.0), 9.0);
        // amount 0.5: weights 0.5, 0.5, 0.125, -0.125
        assert_eq!(hermite(2.0, 4.0, 6.0, 8.0, 0.5), 2.0 * 0.5 + 6.0 * 0.5 + 4.0 * 0.125 + 8.0 * -0.125);
        // catmull_rom at 0 -> value2, at 1 -> value3
        assert_eq!(catmull_rom(0.0, 1.0, 2.0, 3.0, 0.0), 1.0);
        assert_eq!(catmull_rom(0.0, 1.0, 2.0, 3.0, 1.0), 2.0);
        // collinear points interpolate linearly: 0.5 -> 1.5
        assert_eq!(catmull_rom(0.0, 1.0, 2.0, 3.0, 0.5), 1.5);
    }

    #[test]
    fn ieee_remainder_cases() {
        assert_eq!(ieee_remainder(5.0, 3.0), -1.0); // 5/3=1.67 -> 2
        assert_eq!(ieee_remainder(7.0, 3.0), 1.0); // 2.33 -> 2
        assert_eq!(ieee_remainder(3.0, 2.0), -1.0); // 1.5 tie -> 2 (even)
        assert_eq!(ieee_remainder(5.0, 2.0), 1.0); // 2.5 tie -> 2 (even)
        assert_eq!(ieee_remainder(-5.0, 2.0), -1.0);
        assert!(ieee_remainder(-4.0, 2.0).is_sign_negative());
        assert!(ieee_remainder(1.0, 0.0).is_nan());
        assert!(ieee_remainder(f64::INFINITY, 2.0).is_nan());
        assert_eq!(ieee_remainder(3.0, f64::INFINITY), 3.0);
    }

    #[test]
    fn wrap_angle_around_pi() {
        // x/y is exactly 0.5 -> ties to even (0) -> remainder is x itself
        assert_eq!(wrap_angle(PI), PI);
        // -pi: remainder -pi, which is <= -pi, so += 2pi gives +pi
        assert_eq!(wrap_angle(-PI), PI);
        assert_eq!(wrap_angle(0.0), 0.0);
        assert_eq!(wrap_angle(TWO_PI), 0.0);
        assert_eq!(wrap_angle(1.0), 1.0);
        assert_eq!(wrap_angle(4.0), (4.0f64 - 6.2831854820251465) as f32);
        assert_eq!(wrap_angle(7.0), (7.0f64 - 6.2831854820251465) as f32);
        assert_eq!(wrap_angle(-4.0), (-4.0f64 + 6.2831854820251465) as f32);
        // just above pi in f32 wraps to a negative angle
        let above = f32::from_bits(PI.to_bits() + 1);
        assert!(wrap_angle(above) < 0.0);
        // just below pi stays
        let below = f32::from_bits(PI.to_bits() - 1);
        assert_eq!(wrap_angle(below), below);
    }
}
