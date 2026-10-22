//! `Microsoft.Xna.Framework.Vector3` (reference/xna/Vector3.cs).
//! Shared members come from `vector_common!` (see mod.rs).

use super::{Matrix, Vector2};

#[derive(Clone, Copy, Debug, PartialEq, Default)]
#[repr(C)]
pub struct Vector3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

vector_common!(Vector3, x, y, z);

impl Vector3 {
    pub const ZERO: Vector3 = Vector3 { x: 0.0, y: 0.0, z: 0.0 };
    pub const ONE: Vector3 = Vector3 { x: 1.0, y: 1.0, z: 1.0 };
    pub const UNIT_X: Vector3 = Vector3 { x: 1.0, y: 0.0, z: 0.0 };
    pub const UNIT_Y: Vector3 = Vector3 { x: 0.0, y: 1.0, z: 0.0 };
    pub const UNIT_Z: Vector3 = Vector3 { x: 0.0, y: 0.0, z: 1.0 };
    pub const UP: Vector3 = Vector3 { x: 0.0, y: 1.0, z: 0.0 };
    pub const DOWN: Vector3 = Vector3 { x: 0.0, y: -1.0, z: 0.0 };
    pub const RIGHT: Vector3 = Vector3 { x: 1.0, y: 0.0, z: 0.0 };
    pub const LEFT: Vector3 = Vector3 { x: -1.0, y: 0.0, z: 0.0 };
    pub const FORWARD: Vector3 = Vector3 { x: 0.0, y: 0.0, z: -1.0 };
    pub const BACKWARD: Vector3 = Vector3 { x: 0.0, y: 0.0, z: 1.0 };

    pub const fn new(x: f32, y: f32, z: f32) -> Vector3 {
        Vector3 { x, y, z }
    }

    /// `Vector3(float value)`: all components set to `value`.
    pub const fn splat(value: f32) -> Vector3 {
        Vector3 { x: value, y: value, z: value }
    }

    /// `Vector3(Vector2 value, float z)`.
    pub const fn from_vector2(value: Vector2, z: f32) -> Vector3 {
        Vector3 { x: value.x, y: value.y, z }
    }

    pub fn cross(vector1: Vector3, vector2: Vector3) -> Vector3 {
        Vector3 {
            x: vector1.y * vector2.z - vector1.z * vector2.y,
            y: vector1.z * vector2.x - vector1.x * vector2.z,
            z: vector1.x * vector2.y - vector1.y * vector2.x,
        }
    }

    /// `Vector3.Transform(Vector3 position, Matrix matrix)`.
    pub fn transform(position: Vector3, matrix: &Matrix) -> Vector3 {
        let x = position.x * matrix.m11 + position.y * matrix.m21 + position.z * matrix.m31 + matrix.m41;
        let y = position.x * matrix.m12 + position.y * matrix.m22 + position.z * matrix.m32 + matrix.m42;
        let z = position.x * matrix.m13 + position.y * matrix.m23 + position.z * matrix.m33 + matrix.m43;
        Vector3 { x, y, z }
    }

    /// `Vector3.TransformNormal(Vector3 normal, Matrix matrix)`.
    pub fn transform_normal(normal: Vector3, matrix: &Matrix) -> Vector3 {
        let x = normal.x * matrix.m11 + normal.y * matrix.m21 + normal.z * matrix.m31;
        let y = normal.x * matrix.m12 + normal.y * matrix.m22 + normal.z * matrix.m32;
        let z = normal.x * matrix.m13 + normal.y * matrix.m23 + normal.z * matrix.m33;
        Vector3 { x, y, z }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(x: f32, y: f32, z: f32) -> Vector3 {
        Vector3::new(x, y, z)
    }

    #[test]
    fn constants_and_ctors() {
        assert_eq!(Vector3::UP, v(0.0, 1.0, 0.0));
        assert_eq!(Vector3::DOWN, v(0.0, -1.0, 0.0));
        assert_eq!(Vector3::RIGHT, Vector3::UNIT_X);
        assert_eq!(Vector3::LEFT, v(-1.0, 0.0, 0.0));
        assert_eq!(Vector3::FORWARD, v(0.0, 0.0, -1.0));
        assert_eq!(Vector3::BACKWARD, Vector3::UNIT_Z);
        assert_eq!(Vector3::ONE, Vector3::splat(1.0));
        assert_eq!(Vector3::from_vector2(Vector2::new(1.0, 2.0), 3.0), v(1.0, 2.0, 3.0));
        assert_eq!(Vector3::default(), Vector3::ZERO);
    }

    #[test]
    fn length_distance_dot_cross() {
        assert_eq!(v(2.0, 3.0, 6.0).length(), 7.0);
        assert_eq!(v(2.0, 3.0, 6.0).length_squared(), 49.0);
        assert_eq!(Vector3::distance(v(1.0, 2.0, 3.0), v(3.0, 6.0, 7.0)), 6.0);
        assert_eq!(Vector3::distance_squared(v(1.0, 2.0, 3.0), v(3.0, 6.0, 7.0)), 36.0);
        assert_eq!(Vector3::dot(v(1.0, 2.0, 3.0), v(4.0, 5.0, 6.0)), 32.0);
        assert_eq!(Vector3::cross(Vector3::UNIT_X, Vector3::UNIT_Y), Vector3::UNIT_Z);
        assert_eq!(Vector3::cross(Vector3::UNIT_Y, Vector3::UNIT_X), v(0.0, 0.0, -1.0));
        // (1,2,3) x (4,5,6) = (2*6-3*5, 3*4-1*6, 1*5-2*4)
        assert_eq!(Vector3::cross(v(1.0, 2.0, 3.0), v(4.0, 5.0, 6.0)), v(-3.0, 6.0, -3.0));
    }

    #[test]
    fn normalize_and_reflect() {
        // 1/sqrt(9) = 1/3f ; 1*(1/3f), 2*(1/3f), 2*(1/3f)
        let third = 1.0f32 / 3.0;
        let n = Vector3::normalized(v(1.0, 2.0, 2.0));
        assert_eq!(n, v(1.0 * third, 2.0 * third, 2.0 * third));
        let mut m = v(1.0, 2.0, 2.0);
        m.normalize();
        assert_eq!(m, n);
        // dot = 0*0+(-2)*1+0 = -2 ; y = -2 - 2*(-2)*1 = 2
        assert_eq!(Vector3::reflect(v(1.0, -2.0, 0.0), Vector3::UNIT_Y), v(1.0, 2.0, 0.0));
    }

    #[test]
    fn min_max_clamp_lerp() {
        assert_eq!(Vector3::min(v(1.0, 5.0, 3.0), v(2.0, 3.0, 3.0)), v(1.0, 3.0, 3.0));
        assert_eq!(Vector3::max(v(1.0, 5.0, 3.0), v(2.0, 3.0, 4.0)), v(2.0, 5.0, 4.0));
        assert_eq!(
            Vector3::clamp(v(-1.0, 5.0, 0.5), Vector3::ZERO, Vector3::ONE),
            v(0.0, 1.0, 0.5)
        );
        assert_eq!(
            Vector3::lerp(Vector3::ZERO, v(10.0, 20.0, 40.0), 0.25),
            v(2.5, 5.0, 10.0)
        );
        assert_eq!(
            Vector3::barycentric(Vector3::ZERO, v(2.0, 0.0, 0.0), v(0.0, 4.0, 8.0), 0.5, 0.25),
            v(1.0, 1.0, 2.0)
        );
        assert_eq!(
            Vector3::smooth_step(Vector3::ZERO, v(10.0, 20.0, 30.0), 0.5),
            v(5.0, 10.0, 15.0)
        );
        assert_eq!(
            Vector3::catmull_rom(
                v(0.0, 0.0, 0.0),
                v(1.0, 2.0, 3.0),
                v(2.0, 4.0, 6.0),
                v(3.0, 6.0, 9.0),
                0.5
            ),
            v(1.5, 3.0, 4.5)
        );
        assert_eq!(
            Vector3::hermite(v(2.0, 0.0, 0.0), v(4.0, 0.0, 0.0), v(6.0, 0.0, 0.0), v(8.0, 0.0, 0.0), 1.0),
            v(6.0, 0.0, 0.0)
        );
    }

    #[test]
    fn operators() {
        assert_eq!(-v(1.0, -2.0, 3.0), v(-1.0, 2.0, -3.0));
        assert_eq!(Vector3::negate(v(1.0, -2.0, 3.0)), v(-1.0, 2.0, -3.0));
        assert_eq!(v(1.0, 2.0, 3.0) + v(1.0, 1.0, 1.0), v(2.0, 3.0, 4.0));
        assert_eq!(v(1.0, 2.0, 3.0) - v(1.0, 1.0, 1.0), v(0.0, 1.0, 2.0));
        assert_eq!(v(1.0, 2.0, 3.0) * v(2.0, 2.0, 2.0), v(2.0, 4.0, 6.0));
        assert_eq!(v(1.0, 2.0, 3.0) * 3.0, v(3.0, 6.0, 9.0));
        assert_eq!(3.0 * v(1.0, 2.0, 3.0), v(3.0, 6.0, 9.0));
        assert_eq!(v(2.0, 4.0, 6.0) / v(2.0, 2.0, 2.0), v(1.0, 2.0, 3.0));
        // reciprocal multiply: 10 * (1f/3f) = 3.3333335
        assert_eq!((v(10.0, 10.0, 10.0) / 3.0).z.to_bits(), 3.3333335f32.to_bits());
    }

    #[test]
    fn transform_by_matrix() {
        let t = Matrix::create_translation(1.0, 2.0, 3.0);
        assert_eq!(Vector3::transform(v(10.0, 20.0, 30.0), &t), v(11.0, 22.0, 33.0));
        assert_eq!(Vector3::transform_normal(v(10.0, 20.0, 30.0), &t), v(10.0, 20.0, 30.0));
        let s = Matrix::create_scale(2.0, 3.0, 4.0);
        assert_eq!(Vector3::transform(v(1.0, 1.0, 1.0), &s), v(2.0, 3.0, 4.0));
        // row-vector convention: x' = x*m11 + y*m21 + z*m31 (+ m41)
        let mut m = Matrix::IDENTITY;
        m.m21 = 5.0;
        m.m31 = 7.0;
        m.m41 = 11.0;
        assert_eq!(Vector3::transform(v(1.0, 1.0, 1.0), &m), v(1.0 + 5.0 + 7.0 + 11.0, 1.0, 1.0));
        assert_eq!(Vector3::transform_normal(v(1.0, 1.0, 1.0), &m), v(13.0, 1.0, 1.0));
    }
}
