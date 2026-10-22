//! Port of the XNA 4.0 value types (`Microsoft.Xna.Framework`) that Terraria uses,
//! plus `Terraria.DataStructures.Point16`. Behaviour comes from `reference/xna/*.cs`;
//! f32 arithmetic keeps the reference's operation order.

/// `a.f0 * b.f0 + a.f1 * b.f1 + ...` evaluated left to right (the reference's dot/length-squared shape).
macro_rules! dot_sum {
    ($a:expr, $b:expr; $f0:ident $($f:ident)*) => {
        $a.$f0 * $b.$f0 $(+ $a.$f * $b.$f)*
    };
}

/// Members shared by `Vector2`/`Vector3`/`Vector4`. Every component expression is
/// the reference's, evaluated in the same order. Fields are listed in axis order.
macro_rules! vector_common {
    ($name:ident, $($f:ident),+) => {
        impl $name {
            pub fn length_squared(&self) -> f32 {
                dot_sum!(self, self; $($f)+)
            }

            pub fn length(&self) -> f32 {
                let num = dot_sum!(self, self; $($f)+);
                (num as f64).sqrt() as f32
            }

            pub fn distance_squared(value1: $name, value2: $name) -> f32 {
                let d = $name { $($f: value1.$f - value2.$f),+ };
                dot_sum!(d, d; $($f)+)
            }

            pub fn distance(value1: $name, value2: $name) -> f32 {
                let d = $name { $($f: value1.$f - value2.$f),+ };
                let num = dot_sum!(d, d; $($f)+);
                (num as f64).sqrt() as f32
            }

            pub fn dot(value1: $name, value2: $name) -> f32 {
                dot_sum!(value1, value2; $($f)+)
            }

            /// Instance `Normalize()`: normalizes in place.
            pub fn normalize(&mut self) {
                let num = dot_sum!(self, self; $($f)+);
                let num2 = 1.0f32 / (num as f64).sqrt() as f32;
                $(self.$f *= num2;)+
            }

            /// Static `Normalize(value)`: returns the normalized copy.
            pub fn normalized(value: $name) -> $name {
                let num = dot_sum!(value, value; $($f)+);
                let num2 = 1.0f32 / (num as f64).sqrt() as f32;
                $name { $($f: value.$f * num2),+ }
            }

            pub fn reflect(vector: $name, normal: $name) -> $name {
                let num = dot_sum!(vector, normal; $($f)+);
                $name { $($f: vector.$f - 2.0 * num * normal.$f),+ }
            }

            pub fn min(value1: $name, value2: $name) -> $name {
                $name { $($f: if value1.$f < value2.$f { value1.$f } else { value2.$f }),+ }
            }

            pub fn max(value1: $name, value2: $name) -> $name {
                $name { $($f: if value1.$f > value2.$f { value1.$f } else { value2.$f }),+ }
            }

            pub fn clamp(value1: $name, min: $name, max: $name) -> $name {
                $name {
                    $($f: {
                        let v = value1.$f;
                        let v = if v > max.$f { max.$f } else { v };
                        if v < min.$f { min.$f } else { v }
                    }),+
                }
            }

            pub fn lerp(value1: $name, value2: $name, amount: f32) -> $name {
                $name { $($f: value1.$f + (value2.$f - value1.$f) * amount),+ }
            }

            pub fn barycentric(
                value1: $name,
                value2: $name,
                value3: $name,
                amount1: f32,
                amount2: f32,
            ) -> $name {
                $name {
                    $($f: value1.$f + amount1 * (value2.$f - value1.$f)
                        + amount2 * (value3.$f - value1.$f)),+
                }
            }

            pub fn smooth_step(value1: $name, value2: $name, amount: f32) -> $name {
                let amount = if amount > 1.0 {
                    1.0
                } else if amount < 0.0 {
                    0.0
                } else {
                    amount
                };
                let amount = amount * amount * (3.0 - 2.0 * amount);
                $name { $($f: value1.$f + (value2.$f - value1.$f) * amount),+ }
            }

            pub fn catmull_rom(
                value1: $name,
                value2: $name,
                value3: $name,
                value4: $name,
                amount: f32,
            ) -> $name {
                let num = amount * amount;
                let num2 = amount * num;
                $name {
                    $($f: 0.5
                        * (2.0 * value2.$f
                            + (0.0 - value1.$f + value3.$f) * amount
                            + (2.0 * value1.$f - 5.0 * value2.$f + 4.0 * value3.$f - value4.$f) * num
                            + (0.0 - value1.$f + 3.0 * value2.$f - 3.0 * value3.$f + value4.$f)
                                * num2)),+
                }
            }

            pub fn hermite(
                value1: $name,
                tangent1: $name,
                value2: $name,
                tangent2: $name,
                amount: f32,
            ) -> $name {
                let num = amount * amount;
                let num2 = amount * num;
                let num3 = 2.0 * num2 - 3.0 * num + 1.0;
                let num4 = -2.0 * num2 + 3.0 * num;
                let num5 = num2 - 2.0 * num + amount;
                let num6 = num2 - num;
                $name {
                    $($f: value1.$f * num3 + value2.$f * num4 + tangent1.$f * num5
                        + tangent2.$f * num6),+
                }
            }

            /// `-value` per component: the reference's machine code is `fchs` (`Vector2.op_UnaryNegation` in the
            /// entity reference runs, work/jit/ref/entities_proj), so `-UnitY` is (-0, -1), not (0 - 0, -1) = (+0, -1)
            /// (refdata/entities: the portal gun bolt, type 601, flies with velocity.X = -0.0).
            pub fn negate(value: $name) -> $name {
                $name { $($f: -value.$f),+ }
            }

            pub fn add(value1: $name, value2: $name) -> $name {
                $name { $($f: value1.$f + value2.$f),+ }
            }

            pub fn subtract(value1: $name, value2: $name) -> $name {
                $name { $($f: value1.$f - value2.$f),+ }
            }

            pub fn multiply(value1: $name, value2: $name) -> $name {
                $name { $($f: value1.$f * value2.$f),+ }
            }

            pub fn multiply_scalar(value1: $name, scale_factor: f32) -> $name {
                $name { $($f: value1.$f * scale_factor),+ }
            }

            pub fn divide(value1: $name, value2: $name) -> $name {
                $name { $($f: value1.$f / value2.$f),+ }
            }

            /// Multiplies by the reciprocal (`1f / divider`), as the reference does.
            pub fn divide_scalar(value1: $name, divider: f32) -> $name {
                let num = 1.0f32 / divider;
                $name { $($f: value1.$f * num),+ }
            }
        }

        impl core::ops::Neg for $name {
            type Output = $name;
            fn neg(self) -> $name {
                Self::negate(self)
            }
        }

        impl core::ops::Add for $name {
            type Output = $name;
            fn add(self, rhs: $name) -> $name {
                $name { $($f: self.$f + rhs.$f),+ }
            }
        }

        impl core::ops::Sub for $name {
            type Output = $name;
            fn sub(self, rhs: $name) -> $name {
                $name { $($f: self.$f - rhs.$f),+ }
            }
        }

        impl core::ops::Mul for $name {
            type Output = $name;
            fn mul(self, rhs: $name) -> $name {
                $name { $($f: self.$f * rhs.$f),+ }
            }
        }

        impl core::ops::Mul<f32> for $name {
            type Output = $name;
            fn mul(self, scale_factor: f32) -> $name {
                $name { $($f: self.$f * scale_factor),+ }
            }
        }

        impl core::ops::Mul<$name> for f32 {
            type Output = $name;
            fn mul(self, value: $name) -> $name {
                $name { $($f: value.$f * self),+ }
            }
        }

        impl core::ops::Div for $name {
            type Output = $name;
            fn div(self, rhs: $name) -> $name {
                $name { $($f: self.$f / rhs.$f),+ }
            }
        }

        impl core::ops::Div<f32> for $name {
            type Output = $name;
            fn div(self, divider: f32) -> $name {
                let num = 1.0f32 / divider;
                $name { $($f: self.$f * num),+ }
            }
        }

        impl core::ops::AddAssign for $name {
            fn add_assign(&mut self, rhs: $name) {
                $(self.$f = self.$f + rhs.$f;)+
            }
        }

        impl core::ops::SubAssign for $name {
            fn sub_assign(&mut self, rhs: $name) {
                $(self.$f = self.$f - rhs.$f;)+
            }
        }

        impl core::ops::MulAssign for $name {
            fn mul_assign(&mut self, rhs: $name) {
                $(self.$f = self.$f * rhs.$f;)+
            }
        }

        impl core::ops::MulAssign<f32> for $name {
            fn mul_assign(&mut self, scale_factor: f32) {
                $(self.$f = self.$f * scale_factor;)+
            }
        }

        impl core::ops::DivAssign for $name {
            fn div_assign(&mut self, rhs: $name) {
                $(self.$f = self.$f / rhs.$f;)+
            }
        }

        impl core::ops::DivAssign<f32> for $name {
            fn div_assign(&mut self, divider: f32) {
                let num = 1.0f32 / divider;
                $(self.$f = self.$f * num;)+
            }
        }
    };
}

pub mod math_helper;

mod color;
mod matrix;
mod point;
mod point16;
mod rectangle;
mod vector2;
mod vector3;
mod vector4;

pub use color::Color;
pub use matrix::Matrix;
pub use point::Point;
pub use point16::Point16;
pub use rectangle::Rectangle;
pub use vector2::Vector2;
pub use vector3::Vector3;
pub use vector4::Vector4;
