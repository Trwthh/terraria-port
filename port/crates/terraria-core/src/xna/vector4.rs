//! `Microsoft.Xna.Framework.Vector4` (reference/xna/Vector4.cs).
//! Shared members come from `vector_common!` (see mod.rs).

use super::{Matrix, Vector2, Vector3};

#[derive(Clone, Copy, Debug, PartialEq, Default)]
#[repr(C)]
pub struct Vector4 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub w: f32,
}

vector_common!(Vector4, x, y, z, w);

impl Vector4 {
    pub const ZERO: Vector4 = Vector4 { x: 0.0, y: 0.0, z: 0.0, w: 0.0 };
    pub const ONE: Vector4 = Vector4 { x: 1.0, y: 1.0, z: 1.0, w: 1.0 };
    pub const UNIT_X: Vector4 = Vector4 { x: 1.0, y: 0.0, z: 0.0, w: 0.0 };
    pub const UNIT_Y: Vector4 = Vector4 { x: 0.0, y: 1.0, z: 0.0, w: 0.0 };
    pub const UNIT_Z: Vector4 = Vector4 { x: 0.0, y: 0.0, z: 1.0, w: 0.0 };
    pub const UNIT_W: Vector4 = Vector4 { x: 0.0, y: 0.0, z: 0.0, w: 1.0 };

    pub const fn new(x: f32, y: f32, z: f32, w: f32) -> Vector4 {
        Vector4 { x, y, z, w }
    }

    /// `Vector4(float value)`: all components set to `value`.
    pub const fn splat(value: f32) -> Vector4 {
        Vector4 { x: value, y: value, z: value, w: value }
    }

    /// `Vector4(Vector2 value, float z, float w)`.
    pub const fn from_vector2(value: Vector2, z: f32, w: f32) -> Vector4 {
        Vector4 { x: value.x, y: value.y, z, w }
    }

    /// `Vector4(Vector3 value, float w)`.
    pub const fn from_vector3(value: Vector3, w: f32) -> Vector4 {
        Vector4 { x: value.x, y: value.y, z: value.z, w }
    }

    /// `Vector4.Transform(Vector2 position, Matrix matrix)`.
    pub fn transform_vector2(position: Vector2, matrix: &Matrix) -> Vector4 {
        let x = position.x * matrix.m11 + position.y * matrix.m21 + matrix.m41;
        let y = position.x * matrix.m12 + position.y * matrix.m22 + matrix.m42;
        let z = position.x * matrix.m13 + position.y * matrix.m23 + matrix.m43;
        let w = position.x * matrix.m14 + position.y * matrix.m24 + matrix.m44;
        Vector4 { x, y, z, w }
    }

    /// `Vector4.Transform(Vector3 position, Matrix matrix)`.
    pub fn transform_vector3(position: Vector3, matrix: &Matrix) -> Vector4 {
        let x = position.x * matrix.m11 + position.y * matrix.m21 + position.z * matrix.m31 + matrix.m41;
        let y = position.x * matrix.m12 + position.y * matrix.m22 + position.z * matrix.m32 + matrix.m42;
        let z = position.x * matrix.m13 + position.y * matrix.m23 + position.z * matrix.m33 + matrix.m43;
        let w = position.x * matrix.m14 + position.y * matrix.m24 + position.z * matrix.m34 + matrix.m44;
        Vector4 { x, y, z, w }
    }

    /// `Vector4.Transform(Vector4 vector, Matrix matrix)`.
    pub fn transform(vector: Vector4, matrix: &Matrix) -> Vector4 {
        let x = vector.x * matrix.m11 + vector.y * matrix.m21 + vector.z * matrix.m31 + vector.w * matrix.m41;
        let y = vector.x * matrix.m12 + vector.y * matrix.m22 + vector.z * matrix.m32 + vector.w * matrix.m42;
        let z = vector.x * matrix.m13 + vector.y * matrix.m23 + vector.z * matrix.m33 + vector.w * matrix.m43;
        let w = vector.x * matrix.m14 + vector.y * matrix.m24 + vector.z * matrix.m34 + vector.w * matrix.m44;
        Vector4 { x, y, z, w }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(x: f32, y: f32, z: f32, w: f32) -> Vector4 {
        Vector4::new(x, y, z, w)
    }

    #[test]
    fn constants_and_ctors() {
        assert_eq!(Vector4::UNIT_W, v(0.0, 0.0, 0.0, 1.0));
        assert_eq!(Vector4::UNIT_Z, v(0.0, 0.0, 1.0, 0.0));
        assert_eq!(Vector4::ONE, Vector4::splat(1.0));
        assert_eq!(Vector4::from_vector2(Vector2::new(1.0, 2.0), 3.0, 4.0), v(1.0, 2.0, 3.0, 4.0));
        assert_eq!(Vector4::from_vector3(Vector3::new(1.0, 2.0, 3.0), 4.0), v(1.0, 2.0, 3.0, 4.0));
        assert_eq!(Vector4::default(), Vector4::ZERO);
    }

    #[test]
    fn length_distance_dot_normalize() {
        assert_eq!(v(1.0, 2.0, 2.0, 4.0).length(), 5.0);
        assert_eq!(v(1.0, 2.0, 2.0, 4.0).length_squared(), 25.0);
        assert_eq!(Vector4::distance(v(0.0, 0.0, 0.0, 0.0), v(1.0, 2.0, 2.0, 4.0)), 5.0);
        assert_eq!(Vector4::distance_squared(v(1.0, 1.0, 1.0, 1.0), v(2.0, 3.0, 4.0, 5.0)), 30.0);
        assert_eq!(Vector4::dot(v(1.0, 2.0, 3.0, 4.0), v(5.0, 6.0, 7.0, 8.0)), 70.0);
        // 1/sqrt(4) = 0.5 exactly
        let n = Vector4::normalized(v(2.0, 0.0, 0.0, 0.0));
        assert_eq!(n, Vector4::UNIT_X);
        let mut m = v(0.0, 0.0, 0.0, 2.0);
        m.normalize();
        assert_eq!(m, Vector4::UNIT_W);
    }

    #[test]
    fn min_max_clamp_interpolation() {
        assert_eq!(
            Vector4::min(v(1.0, 5.0, 3.0, 9.0), v(2.0, 3.0, 3.0, 8.0)),
            v(1.0, 3.0, 3.0, 8.0)
        );
        assert_eq!(
            Vector4::max(v(1.0, 5.0, 3.0, 9.0), v(2.0, 3.0, 3.0, 8.0)),
            v(2.0, 5.0, 3.0, 9.0)
        );
        assert_eq!(
            Vector4::clamp(v(-1.0, 5.0, 0.5, 2.0), Vector4::ZERO, Vector4::ONE),
            v(0.0, 1.0, 0.5, 1.0)
        );
        assert_eq!(
            Vector4::lerp(Vector4::ZERO, v(10.0, 20.0, 40.0, 80.0), 0.25),
            v(2.5, 5.0, 10.0, 20.0)
        );
        assert_eq!(
            Vector4::barycentric(Vector4::ZERO, v(2.0, 0.0, 0.0, 4.0), v(0.0, 4.0, 8.0, 0.0), 0.5, 0.25),
            v(1.0, 1.0, 2.0, 2.0)
        );
        assert_eq!(
            Vector4::smooth_step(Vector4::ZERO, v(10.0, 20.0, 30.0, 40.0), 0.5),
            v(5.0, 10.0, 15.0, 20.0)
        );
        assert_eq!(
            Vector4::catmull_rom(
                v(0.0, 0.0, 0.0, 0.0),
                v(1.0, 2.0, 3.0, 4.0),
                v(2.0, 4.0, 6.0, 8.0),
                v(3.0, 6.0, 9.0, 12.0),
                0.5
            ),
            v(1.5, 3.0, 4.5, 6.0)
        );
        assert_eq!(
            Vector4::hermite(
                v(2.0, 0.0, 0.0, 1.0),
                Vector4::ZERO,
                v(6.0, 0.0, 0.0, 3.0),
                Vector4::ZERO,
                0.0
            ),
            v(2.0, 0.0, 0.0, 1.0)
        );
    }

    #[test]
    fn operators() {
        assert_eq!(-v(1.0, -2.0, 3.0, -4.0), v(-1.0, 2.0, -3.0, 4.0));
        assert_eq!(v(1.0, 2.0, 3.0, 4.0) + Vector4::ONE, v(2.0, 3.0, 4.0, 5.0));
        assert_eq!(v(1.0, 2.0, 3.0, 4.0) - Vector4::ONE, v(0.0, 1.0, 2.0, 3.0));
        assert_eq!(v(1.0, 2.0, 3.0, 4.0) * v(2.0, 2.0, 2.0, 2.0), v(2.0, 4.0, 6.0, 8.0));
        assert_eq!(v(1.0, 2.0, 3.0, 4.0) * 2.0, v(2.0, 4.0, 6.0, 8.0));
        assert_eq!(2.0 * v(1.0, 2.0, 3.0, 4.0), v(2.0, 4.0, 6.0, 8.0));
        assert_eq!(v(2.0, 4.0, 6.0, 8.0) / v(2.0, 2.0, 2.0, 2.0), v(1.0, 2.0, 3.0, 4.0));
        assert_eq!((v(10.0, 10.0, 10.0, 10.0) / 3.0).w.to_bits(), 3.3333335f32.to_bits());
        assert_eq!(Vector4::reflect(v(1.0, -1.0, 0.0, 0.0), Vector4::UNIT_Y), v(1.0, 1.0, 0.0, 0.0));
    }

    #[test]
    fn transform_by_matrix() {
        let t = Matrix::create_translation(1.0, 2.0, 3.0);
        // Vector2/Vector3 overloads treat the input as a point (add row 4, w from m44)
        assert_eq!(
            Vector4::transform_vector2(Vector2::new(10.0, 20.0), &t),
            v(11.0, 22.0, 3.0, 1.0)
        );
        assert_eq!(
            Vector4::transform_vector3(Vector3::new(10.0, 20.0, 30.0), &t),
            v(11.0, 22.0, 33.0, 1.0)
        );
        // Vector4 overload uses the vector's own w
        assert_eq!(Vector4::transform(v(10.0, 20.0, 30.0, 1.0), &t), v(11.0, 22.0, 33.0, 1.0));
        assert_eq!(Vector4::transform(v(10.0, 20.0, 30.0, 0.0), &t), v(10.0, 20.0, 30.0, 0.0));
        let mut m = Matrix::IDENTITY;
        m.m14 = 2.0;
        m.m24 = 3.0;
        m.m34 = 4.0;
        m.m44 = 5.0;
        // w = x*m14 + y*m24 + z*m34 + w*m44
        assert_eq!(Vector4::transform(v(1.0, 1.0, 1.0, 1.0), &m).w, 14.0);
    }
}
