//! `Microsoft.Xna.Framework.Vector2` (reference/xna/Vector2.cs).
//! Shared members come from `vector_common!` (see mod.rs).

use super::Matrix;

#[derive(Clone, Copy, Debug, PartialEq, Default)]
#[repr(C)]
pub struct Vector2 {
    pub x: f32,
    pub y: f32,
}

vector_common!(Vector2, x, y);

impl Vector2 {
    pub const ZERO: Vector2 = Vector2 { x: 0.0, y: 0.0 };
    pub const ONE: Vector2 = Vector2 { x: 1.0, y: 1.0 };
    pub const UNIT_X: Vector2 = Vector2 { x: 1.0, y: 0.0 };
    pub const UNIT_Y: Vector2 = Vector2 { x: 0.0, y: 1.0 };

    pub const fn new(x: f32, y: f32) -> Vector2 {
        Vector2 { x, y }
    }

    /// `Vector2(float value)`: both components set to `value`.
    pub const fn splat(value: f32) -> Vector2 {
        Vector2 { x: value, y: value }
    }

    /// `Vector2.Transform(Vector2 position, Matrix matrix)`.
    pub fn transform(position: Vector2, matrix: &Matrix) -> Vector2 {
        let x = position.x * matrix.m11 + position.y * matrix.m21 + matrix.m41;
        let y = position.x * matrix.m12 + position.y * matrix.m22 + matrix.m42;
        Vector2 { x, y }
    }

    /// `Vector2.TransformNormal(Vector2 normal, Matrix matrix)`.
    pub fn transform_normal(normal: Vector2, matrix: &Matrix) -> Vector2 {
        let x = normal.x * matrix.m11 + normal.y * matrix.m21;
        let y = normal.x * matrix.m12 + normal.y * matrix.m22;
        Vector2 { x, y }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(x: f32, y: f32) -> Vector2 {
        Vector2::new(x, y)
    }

    #[test]
    fn constants_and_ctors() {
        assert_eq!(Vector2::ZERO, v(0.0, 0.0));
        assert_eq!(Vector2::ONE, v(1.0, 1.0));
        assert_eq!(Vector2::UNIT_X, v(1.0, 0.0));
        assert_eq!(Vector2::UNIT_Y, v(0.0, 1.0));
        assert_eq!(Vector2::splat(2.5), v(2.5, 2.5));
        assert_eq!(Vector2::default(), Vector2::ZERO);
    }

    #[test]
    fn length_distance_dot() {
        assert_eq!(v(3.0, 4.0).length(), 5.0);
        assert_eq!(v(3.0, 4.0).length_squared(), 25.0);
        assert_eq!(Vector2::distance(v(1.0, 1.0), v(4.0, 5.0)), 5.0);
        assert_eq!(Vector2::distance_squared(v(1.0, 1.0), v(4.0, 5.0)), 25.0);
        assert_eq!(Vector2::dot(v(1.0, 2.0), v(3.0, 4.0)), 11.0);
    }

    #[test]
    fn normalize_3_4() {
        // 1/sqrt(25) = 0.2f; 3*0.2f and 4*0.2f round to 0.6f and 0.8f
        let n = Vector2::normalized(v(3.0, 4.0));
        assert_eq!(n.x, 3.0f32 * (1.0f32 / 5.0));
        assert_eq!(n.y, 4.0f32 * (1.0f32 / 5.0));
        assert_eq!(n.x, 0.6);
        assert_eq!(n.y, 0.8);
        let mut m = v(3.0, 4.0);
        m.normalize();
        assert_eq!(m, n);
        // zero vector: 1/0 = inf, 0*inf = NaN
        let z = Vector2::normalized(Vector2::ZERO);
        assert!(z.x.is_nan() && z.y.is_nan());
    }

    #[test]
    fn reflect_min_max_clamp() {
        // dot = -1; x = 1 - 2*(-1)*0 = 1; y = -1 - 2*(-1)*1 = 1
        assert_eq!(Vector2::reflect(v(1.0, -1.0), v(0.0, 1.0)), v(1.0, 1.0));
        assert_eq!(Vector2::min(v(1.0, 5.0), v(2.0, 3.0)), v(1.0, 3.0));
        assert_eq!(Vector2::max(v(1.0, 5.0), v(2.0, 3.0)), v(2.0, 5.0));
        assert_eq!(
            Vector2::clamp(v(-5.0, 5.0), v(0.0, 0.0), v(1.0, 2.0)),
            v(0.0, 2.0)
        );
    }

    #[test]
    fn interpolation() {
        assert_eq!(Vector2::lerp(v(0.0, 0.0), v(10.0, 20.0), 0.25), v(2.5, 5.0));
        // 0 + 0.5*(2-0) + 0.25*(0-0) ; 0 + 0.5*0 + 0.25*4
        assert_eq!(
            Vector2::barycentric(v(0.0, 0.0), v(2.0, 0.0), v(0.0, 4.0), 0.5, 0.25),
            v(1.0, 1.0)
        );
        assert_eq!(
            Vector2::smooth_step(v(0.0, 0.0), v(10.0, 20.0), 0.5),
            v(5.0, 10.0)
        );
        assert_eq!(
            Vector2::smooth_step(v(0.0, 0.0), v(10.0, 20.0), 3.0),
            v(10.0, 20.0)
        );
        assert_eq!(
            Vector2::catmull_rom(v(0.0, 0.0), v(1.0, 2.0), v(2.0, 4.0), v(3.0, 6.0), 0.5),
            v(1.5, 3.0)
        );
        // weights at 0.5 are 0.5, 0.5, 0.125, -0.125
        assert_eq!(
            Vector2::hermite(v(2.0, 0.0), v(4.0, 0.0), v(6.0, 0.0), v(8.0, 0.0), 0.5),
            v(3.5, 0.0)
        );
    }

    #[test]
    fn arithmetic_and_operators() {
        assert_eq!(Vector2::negate(v(1.0, -2.0)), v(-1.0, 2.0));
        // fchs: the sign of a zero flips
        assert_eq!((-Vector2::UNIT_Y).x.to_bits(), (-0.0f32).to_bits());
        assert_eq!(-v(1.0, -2.0), v(-1.0, 2.0));
        assert_eq!(v(1.0, 2.0) + v(3.0, 4.0), v(4.0, 6.0));
        assert_eq!(v(1.0, 2.0) - v(3.0, 4.0), v(-2.0, -2.0));
        assert_eq!(v(1.0, 2.0) * v(3.0, 4.0), v(3.0, 8.0));
        assert_eq!(v(1.0, 2.0) * 2.0, v(2.0, 4.0));
        assert_eq!(2.0 * v(1.0, 2.0), v(2.0, 4.0));
        assert_eq!(v(3.0, 8.0) / v(3.0, 4.0), v(1.0, 2.0));
        assert_eq!(Vector2::add(v(1.0, 1.0), v(1.0, 2.0)), v(2.0, 3.0));
        assert_eq!(Vector2::subtract(v(1.0, 1.0), v(1.0, 2.0)), v(0.0, -1.0));
        assert_eq!(Vector2::multiply(v(2.0, 3.0), v(4.0, 5.0)), v(8.0, 15.0));
        assert_eq!(Vector2::multiply_scalar(v(2.0, 3.0), 2.0), v(4.0, 6.0));
        assert_eq!(Vector2::divide(v(8.0, 15.0), v(4.0, 5.0)), v(2.0, 3.0));
        let mut a = v(1.0, 2.0);
        a += v(1.0, 1.0);
        a -= v(0.0, 1.0);
        a *= 2.0;
        assert_eq!(a, v(4.0, 4.0));
        a /= 2.0;
        assert_eq!(a, v(2.0, 2.0));
    }

    #[test]
    fn divide_by_scalar_uses_reciprocal() {
        // 1f/3f = 0.33333334; 10 * 0.33333334 = 3.3333335 (not 10f/3f = 3.3333333)
        let q = v(10.0, 10.0) / 3.0;
        assert_eq!(q.x.to_bits(), 3.3333335f32.to_bits());
        assert_ne!(q.x.to_bits(), (10.0f32 / 3.0).to_bits());
        assert_eq!(Vector2::divide_scalar(v(10.0, 10.0), 3.0), q);
    }

    #[test]
    fn equality_is_ieee() {
        assert_eq!(v(0.0, 0.0), v(-0.0, 0.0));
        assert_ne!(v(f32::NAN, 0.0), v(f32::NAN, 0.0));
    }

    #[test]
    fn transform_by_matrix() {
        assert_eq!(
            Vector2::transform(v(1.0, 2.0), &Matrix::create_translation(10.0, 20.0, 0.0)),
            v(11.0, 22.0)
        );
        assert_eq!(
            Vector2::transform_normal(v(1.0, 2.0), &Matrix::create_translation(10.0, 20.0, 0.0)),
            v(1.0, 2.0)
        );
        let mut m = Matrix::IDENTITY;
        m.m11 = 1.0;
        m.m21 = 2.0;
        m.m41 = 3.0;
        m.m12 = 4.0;
        m.m22 = 5.0;
        m.m42 = 6.0;
        // x = 10*1 + 100*2 + 3 ; y = 10*4 + 100*5 + 6
        assert_eq!(Vector2::transform(v(10.0, 100.0), &m), v(213.0, 546.0));
        // x = 10*1 + 100*2 ; y = 10*4 + 100*5
        assert_eq!(Vector2::transform_normal(v(10.0, 100.0), &m), v(210.0, 540.0));
    }
}
