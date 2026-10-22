//! `Microsoft.Xna.Framework.Matrix` (reference/xna/Matrix.cs), row-vector convention.
//! Only the members Terraria's rendering path needs are ported; the rest of the
//! reference (billboards, look-at, perspective, quaternion, plane members) is not.

use super::Vector3;
use core::ops::Mul;

#[derive(Clone, Copy, Debug, PartialEq, Default)]
#[repr(C)]
pub struct Matrix {
    pub m11: f32,
    pub m12: f32,
    pub m13: f32,
    pub m14: f32,
    pub m21: f32,
    pub m22: f32,
    pub m23: f32,
    pub m24: f32,
    pub m31: f32,
    pub m32: f32,
    pub m33: f32,
    pub m34: f32,
    pub m41: f32,
    pub m42: f32,
    pub m43: f32,
    pub m44: f32,
}

impl Matrix {
    pub const IDENTITY: Matrix = Matrix {
        m11: 1.0,
        m12: 0.0,
        m13: 0.0,
        m14: 0.0,
        m21: 0.0,
        m22: 1.0,
        m23: 0.0,
        m24: 0.0,
        m31: 0.0,
        m32: 0.0,
        m33: 1.0,
        m34: 0.0,
        m41: 0.0,
        m42: 0.0,
        m43: 0.0,
        m44: 1.0,
    };

    #[allow(clippy::too_many_arguments)]
    pub const fn new(
        m11: f32,
        m12: f32,
        m13: f32,
        m14: f32,
        m21: f32,
        m22: f32,
        m23: f32,
        m24: f32,
        m31: f32,
        m32: f32,
        m33: f32,
        m34: f32,
        m41: f32,
        m42: f32,
        m43: f32,
        m44: f32,
    ) -> Matrix {
        Matrix {
            m11,
            m12,
            m13,
            m14,
            m21,
            m22,
            m23,
            m24,
            m31,
            m32,
            m33,
            m34,
            m41,
            m42,
            m43,
            m44,
        }
    }

    /// `Translation` getter: row 4, columns 1..3.
    pub const fn translation(&self) -> Vector3 {
        Vector3 {
            x: self.m41,
            y: self.m42,
            z: self.m43,
        }
    }

    /// `Translation` setter.
    pub fn set_translation(&mut self, value: Vector3) {
        self.m41 = value.x;
        self.m42 = value.y;
        self.m43 = value.z;
    }

    /// `CreateTranslation(Vector3 position)`.
    pub fn create_translation_vector3(position: Vector3) -> Matrix {
        Matrix::create_translation(position.x, position.y, position.z)
    }

    /// `CreateTranslation(float xPosition, float yPosition, float zPosition)`.
    pub fn create_translation(x_position: f32, y_position: f32, z_position: f32) -> Matrix {
        Matrix {
            m41: x_position,
            m42: y_position,
            m43: z_position,
            ..Matrix::IDENTITY
        }
    }

    /// `CreateScale(float xScale, float yScale, float zScale)`.
    pub fn create_scale(x_scale: f32, y_scale: f32, z_scale: f32) -> Matrix {
        Matrix {
            m11: x_scale,
            m22: y_scale,
            m33: z_scale,
            ..Matrix::IDENTITY
        }
    }

    /// `CreateScale(Vector3 scales)`.
    pub fn create_scale_vector3(scales: Vector3) -> Matrix {
        Matrix::create_scale(scales.x, scales.y, scales.z)
    }

    /// `CreateScale(float scale)`: the same factor on all three axes.
    pub fn create_scale_uniform(scale: f32) -> Matrix {
        Matrix::create_scale(scale, scale, scale)
    }

    pub fn create_rotation_x(radians: f32) -> Matrix {
        let num = (radians as f64).cos() as f32;
        let num2 = (radians as f64).sin() as f32;
        Matrix {
            m22: num,
            m23: num2,
            m32: 0.0 - num2,
            m33: num,
            ..Matrix::IDENTITY
        }
    }

    pub fn create_rotation_y(radians: f32) -> Matrix {
        let num = (radians as f64).cos() as f32;
        let num2 = (radians as f64).sin() as f32;
        Matrix {
            m11: num,
            m13: 0.0 - num2,
            m31: num2,
            m33: num,
            ..Matrix::IDENTITY
        }
    }

    pub fn create_rotation_z(radians: f32) -> Matrix {
        let num = (radians as f64).cos() as f32;
        let num2 = (radians as f64).sin() as f32;
        Matrix {
            m11: num,
            m12: num2,
            m21: 0.0 - num2,
            m22: num,
            ..Matrix::IDENTITY
        }
    }

    pub fn create_orthographic(width: f32, height: f32, z_near_plane: f32, z_far_plane: f32) -> Matrix {
        Matrix {
            m11: 2.0 / width,
            m22: 2.0 / height,
            m33: 1.0 / (z_near_plane - z_far_plane),
            m43: z_near_plane / (z_near_plane - z_far_plane),
            m44: 1.0,
            ..Matrix::default()
        }
    }

    pub fn create_orthographic_off_center(
        left: f32,
        right: f32,
        bottom: f32,
        top: f32,
        z_near_plane: f32,
        z_far_plane: f32,
    ) -> Matrix {
        Matrix {
            m11: 2.0 / (right - left),
            m22: 2.0 / (top - bottom),
            m33: 1.0 / (z_near_plane - z_far_plane),
            m41: (left + right) / (left - right),
            m42: (top + bottom) / (bottom - top),
            m43: z_near_plane / (z_near_plane - z_far_plane),
            m44: 1.0,
            ..Matrix::default()
        }
    }

    pub fn transpose(matrix: Matrix) -> Matrix {
        Matrix {
            m11: matrix.m11,
            m12: matrix.m21,
            m13: matrix.m31,
            m14: matrix.m41,
            m21: matrix.m12,
            m22: matrix.m22,
            m23: matrix.m32,
            m24: matrix.m42,
            m31: matrix.m13,
            m32: matrix.m23,
            m33: matrix.m33,
            m34: matrix.m43,
            m41: matrix.m14,
            m42: matrix.m24,
            m43: matrix.m34,
            m44: matrix.m44,
        }
    }

    pub fn determinant(&self) -> f32 {
        let (m, m2, m3, m4) = (self.m11, self.m12, self.m13, self.m14);
        let (m5, m6, m7, m8) = (self.m21, self.m22, self.m23, self.m24);
        let (m9, m10, m11, m12) = (self.m31, self.m32, self.m33, self.m34);
        let (m13, m14, m15, m16) = (self.m41, self.m42, self.m43, self.m44);
        let num = m11 * m16 - m12 * m15;
        let num2 = m10 * m16 - m12 * m14;
        let num3 = m10 * m15 - m11 * m14;
        let num4 = m9 * m16 - m12 * m13;
        let num5 = m9 * m15 - m11 * m13;
        let num6 = m9 * m14 - m10 * m13;
        m * (m6 * num - m7 * num2 + m8 * num3) - m2 * (m5 * num - m7 * num4 + m8 * num5)
            + m3 * (m5 * num2 - m6 * num4 + m8 * num6)
            - m4 * (m5 * num3 - m6 * num5 + m7 * num6)
    }

    /// `Invert`: no singularity check, exactly like the reference (a zero
    /// determinant yields infinities/NaNs).
    pub fn invert(matrix: Matrix) -> Matrix {
        let (m, m2, m3, m4) = (matrix.m11, matrix.m12, matrix.m13, matrix.m14);
        let (m5, m6, m7, m8) = (matrix.m21, matrix.m22, matrix.m23, matrix.m24);
        let (m9, m10, m11, m12) = (matrix.m31, matrix.m32, matrix.m33, matrix.m34);
        let (m13, m14, m15, m16) = (matrix.m41, matrix.m42, matrix.m43, matrix.m44);
        let num = m11 * m16 - m12 * m15;
        let num2 = m10 * m16 - m12 * m14;
        let num3 = m10 * m15 - m11 * m14;
        let num4 = m9 * m16 - m12 * m13;
        let num5 = m9 * m15 - m11 * m13;
        let num6 = m9 * m14 - m10 * m13;
        let num7 = m6 * num - m7 * num2 + m8 * num3;
        let num8 = 0.0 - (m5 * num - m7 * num4 + m8 * num5);
        let num9 = m5 * num2 - m6 * num4 + m8 * num6;
        let num10 = 0.0 - (m5 * num3 - m6 * num5 + m7 * num6);
        let num11 = 1.0 / (m * num7 + m2 * num8 + m3 * num9 + m4 * num10);
        let r_m11 = num7 * num11;
        let r_m21 = num8 * num11;
        let r_m31 = num9 * num11;
        let r_m41 = num10 * num11;
        let r_m12 = (0.0 - (m2 * num - m3 * num2 + m4 * num3)) * num11;
        let r_m22 = (m * num - m3 * num4 + m4 * num5) * num11;
        let r_m32 = (0.0 - (m * num2 - m2 * num4 + m4 * num6)) * num11;
        let r_m42 = (m * num3 - m2 * num5 + m3 * num6) * num11;
        let num12 = m7 * m16 - m8 * m15;
        let num13 = m6 * m16 - m8 * m14;
        let num14 = m6 * m15 - m7 * m14;
        let num15 = m5 * m16 - m8 * m13;
        let num16 = m5 * m15 - m7 * m13;
        let num17 = m5 * m14 - m6 * m13;
        let r_m13 = (m2 * num12 - m3 * num13 + m4 * num14) * num11;
        let r_m23 = (0.0 - (m * num12 - m3 * num15 + m4 * num16)) * num11;
        let r_m33 = (m * num13 - m2 * num15 + m4 * num17) * num11;
        let r_m43 = (0.0 - (m * num14 - m2 * num16 + m3 * num17)) * num11;
        let num18 = m7 * m12 - m8 * m11;
        let num19 = m6 * m12 - m8 * m10;
        let num20 = m6 * m11 - m7 * m10;
        let num21 = m5 * m12 - m8 * m9;
        let num22 = m5 * m11 - m7 * m9;
        let num23 = m5 * m10 - m6 * m9;
        let r_m14 = (0.0 - (m2 * num18 - m3 * num19 + m4 * num20)) * num11;
        let r_m24 = (m * num18 - m3 * num21 + m4 * num22) * num11;
        let r_m34 = (0.0 - (m * num19 - m2 * num21 + m4 * num23)) * num11;
        let r_m44 = (m * num20 - m2 * num22 + m3 * num23) * num11;
        Matrix {
            m11: r_m11,
            m12: r_m12,
            m13: r_m13,
            m14: r_m14,
            m21: r_m21,
            m22: r_m22,
            m23: r_m23,
            m24: r_m24,
            m31: r_m31,
            m32: r_m32,
            m33: r_m33,
            m34: r_m34,
            m41: r_m41,
            m42: r_m42,
            m43: r_m43,
            m44: r_m44,
        }
    }

    /// `Multiply(Matrix, Matrix)`, identical to `operator *`.
    pub fn multiply(matrix1: Matrix, matrix2: Matrix) -> Matrix {
        let (a, b) = (matrix1, matrix2);
        Matrix {
            m11: a.m11 * b.m11 + a.m12 * b.m21 + a.m13 * b.m31 + a.m14 * b.m41,
            m12: a.m11 * b.m12 + a.m12 * b.m22 + a.m13 * b.m32 + a.m14 * b.m42,
            m13: a.m11 * b.m13 + a.m12 * b.m23 + a.m13 * b.m33 + a.m14 * b.m43,
            m14: a.m11 * b.m14 + a.m12 * b.m24 + a.m13 * b.m34 + a.m14 * b.m44,
            m21: a.m21 * b.m11 + a.m22 * b.m21 + a.m23 * b.m31 + a.m24 * b.m41,
            m22: a.m21 * b.m12 + a.m22 * b.m22 + a.m23 * b.m32 + a.m24 * b.m42,
            m23: a.m21 * b.m13 + a.m22 * b.m23 + a.m23 * b.m33 + a.m24 * b.m43,
            m24: a.m21 * b.m14 + a.m22 * b.m24 + a.m23 * b.m34 + a.m24 * b.m44,
            m31: a.m31 * b.m11 + a.m32 * b.m21 + a.m33 * b.m31 + a.m34 * b.m41,
            m32: a.m31 * b.m12 + a.m32 * b.m22 + a.m33 * b.m32 + a.m34 * b.m42,
            m33: a.m31 * b.m13 + a.m32 * b.m23 + a.m33 * b.m33 + a.m34 * b.m43,
            m34: a.m31 * b.m14 + a.m32 * b.m24 + a.m33 * b.m34 + a.m34 * b.m44,
            m41: a.m41 * b.m11 + a.m42 * b.m21 + a.m43 * b.m31 + a.m44 * b.m41,
            m42: a.m41 * b.m12 + a.m42 * b.m22 + a.m43 * b.m32 + a.m44 * b.m42,
            m43: a.m41 * b.m13 + a.m42 * b.m23 + a.m43 * b.m33 + a.m44 * b.m43,
            m44: a.m41 * b.m14 + a.m42 * b.m24 + a.m43 * b.m34 + a.m44 * b.m44,
        }
    }
}

impl Mul for Matrix {
    type Output = Matrix;

    fn mul(self, rhs: Matrix) -> Matrix {
        Matrix::multiply(self, rhs)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::xna::math_helper;
    use crate::xna::Vector2;

    fn rows(m: &Matrix) -> [f32; 16] {
        [
            m.m11, m.m12, m.m13, m.m14, m.m21, m.m22, m.m23, m.m24, m.m31, m.m32, m.m33, m.m34,
            m.m41, m.m42, m.m43, m.m44,
        ]
    }

    #[test]
    fn identity_and_default() {
        assert_eq!(
            rows(&Matrix::IDENTITY),
            [1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0]
        );
        assert_eq!(rows(&Matrix::default()), [0.0; 16]);
    }

    #[test]
    fn translation_and_scale() {
        let t = Matrix::create_translation(1.0, 2.0, 3.0);
        assert_eq!(t.translation(), Vector3::new(1.0, 2.0, 3.0));
        assert_eq!(t.m11, 1.0);
        assert_eq!(t.m44, 1.0);
        assert_eq!(Matrix::create_translation_vector3(Vector3::new(1.0, 2.0, 3.0)), t);
        let mut m = Matrix::IDENTITY;
        m.set_translation(Vector3::new(4.0, 5.0, 6.0));
        assert_eq!((m.m41, m.m42, m.m43, m.m44), (4.0, 5.0, 6.0, 1.0));
        let s = Matrix::create_scale(2.0, 3.0, 4.0);
        assert_eq!(
            rows(&s),
            [2.0, 0.0, 0.0, 0.0, 0.0, 3.0, 0.0, 0.0, 0.0, 0.0, 4.0, 0.0, 0.0, 0.0, 0.0, 1.0]
        );
        assert_eq!(Matrix::create_scale_vector3(Vector3::new(2.0, 3.0, 4.0)), s);
        assert_eq!(Matrix::create_scale_uniform(5.0), Matrix::create_scale(5.0, 5.0, 5.0));
    }

    #[test]
    fn rotations() {
        assert_eq!(Matrix::create_rotation_x(0.0), Matrix::IDENTITY);
        assert_eq!(Matrix::create_rotation_y(0.0), Matrix::IDENTITY);
        assert_eq!(Matrix::create_rotation_z(0.0), Matrix::IDENTITY);
        // 90 degrees about Z: cos ~ -4.37e-8 (pi/2 rounded to f32), sin = 1
        let z = Matrix::create_rotation_z(math_helper::PI_OVER2);
        assert!((z.m11 + 4.371139e-8).abs() < 1e-12);
        assert_eq!(z.m12, 1.0);
        assert_eq!(z.m21, -1.0);
        assert_eq!(z.m22, z.m11);
        assert_eq!((z.m33, z.m44), (1.0, 1.0));
        // row vector (1,0) rotates to (cos, sin) ~ (0,1)
        let p = Vector2::transform(Vector2::new(1.0, 0.0), &z);
        assert!(p.x.abs() < 1e-7 && p.y == 1.0);
        let x = Matrix::create_rotation_x(math_helper::PI_OVER2);
        assert_eq!((x.m23, x.m32), (1.0, -1.0));
        let y = Matrix::create_rotation_y(math_helper::PI_OVER2);
        assert_eq!((y.m13, y.m31), (-1.0, 1.0));
    }

    #[test]
    fn orthographic_off_center() {
        // Screen-style projection: left 0, right 1024, bottom 512, top 0, near 0, far 1
        let m = Matrix::create_orthographic_off_center(0.0, 1024.0, 512.0, 0.0, 0.0, 1.0);
        assert_eq!(m.m11, 2.0 / 1024.0); // 2 / (right - left)
        assert_eq!(m.m22, -2.0 / 512.0); // 2 / (top - bottom)
        assert_eq!(m.m33, -1.0); // 1 / (near - far)
        assert_eq!(m.m41, -1.0); // (left + right) / (left - right)
        assert_eq!(m.m42, 1.0); // (top + bottom) / (bottom - top)
        assert_eq!(m.m43, 0.0); // near / (near - far)
        assert_eq!(m.m44, 1.0);
        for v in [m.m12, m.m13, m.m14, m.m21, m.m23, m.m24, m.m31, m.m32, m.m34] {
            assert_eq!(v, 0.0);
        }
        // top-right corner maps to (1, 1), bottom-left to (-1, -1)
        assert_eq!(Vector2::transform(Vector2::new(1024.0, 0.0), &m), Vector2::new(1.0, 1.0));
        assert_eq!(Vector2::transform(Vector2::new(0.0, 512.0), &m), Vector2::new(-1.0, -1.0));
        // asymmetric box: l=-10 r=30 b=-5 t=15 n=1 f=3
        let a = Matrix::create_orthographic_off_center(-10.0, 30.0, -5.0, 15.0, 1.0, 3.0);
        assert_eq!(a.m11, 2.0 / 40.0);
        assert_eq!(a.m22, 2.0 / 20.0);
        assert_eq!(a.m33, 1.0 / -2.0);
        assert_eq!(a.m41, 20.0 / -40.0);
        assert_eq!(a.m42, 10.0 / -20.0);
        assert_eq!(a.m43, 1.0 / -2.0);
    }

    #[test]
    fn orthographic_centered() {
        let m = Matrix::create_orthographic(8.0, 4.0, 0.0, 2.0);
        assert_eq!((m.m11, m.m22, m.m33, m.m43, m.m44), (0.25, 0.5, -0.5, 0.0, 1.0));
        assert_eq!((m.m41, m.m42), (0.0, 0.0));
    }

    #[test]
    fn transpose_and_determinant() {
        let m = Matrix::new(
            1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0, 11.0, 12.0, 13.0, 14.0, 15.0, 16.0,
        );
        let t = Matrix::transpose(m);
        assert_eq!(
            rows(&t),
            [1.0, 5.0, 9.0, 13.0, 2.0, 6.0, 10.0, 14.0, 3.0, 7.0, 11.0, 15.0, 4.0, 8.0, 12.0, 16.0]
        );
        assert_eq!(Matrix::transpose(t), m);
        assert_eq!(Matrix::IDENTITY.determinant(), 1.0);
        assert_eq!(Matrix::create_scale(2.0, 4.0, 8.0).determinant(), 64.0);
        // rank-deficient 1..16 matrix has determinant 0
        assert_eq!(m.determinant(), 0.0);
    }

    #[test]
    fn invert_known_matrices() {
        // p' = p * M = (2x+1, 4y+2, 8z+3) so the inverse is x = 0.5x' - 0.5, ...
        let m = Matrix::create_scale(2.0, 4.0, 8.0) * Matrix::create_translation(1.0, 2.0, 3.0);
        let inv = Matrix::invert(m);
        assert_eq!(
            rows(&inv),
            [
                0.5, 0.0, 0.0, 0.0, 0.0, 0.25, 0.0, 0.0, 0.0, 0.0, 0.125, 0.0, -0.5, -0.5, -0.375,
                1.0
            ]
        );
        assert_eq!(m * inv, Matrix::IDENTITY);
        // upper-left 2x2 [[1,2],[3,4]] has inverse [[-2,1],[1.5,-0.5]]
        let g = Matrix::new(
            1.0, 2.0, 0.0, 0.0, 3.0, 4.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
        );
        assert_eq!(g.determinant(), -2.0);
        assert_eq!(
            rows(&Matrix::invert(g)),
            [-2.0, 1.0, 0.0, 0.0, 1.5, -0.5, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0]
        );
        assert_eq!(Matrix::invert(Matrix::IDENTITY), Matrix::IDENTITY);
        // singular input is not checked: infinity/NaN come out
        assert!(!Matrix::invert(Matrix::default()).m11.is_finite());
    }

    #[test]
    fn multiply_order() {
        // row-vector convention: translate first, then scale
        let m = Matrix::create_translation(1.0, 2.0, 3.0) * Matrix::create_scale(2.0, 2.0, 2.0);
        assert_eq!(m.translation(), Vector3::new(2.0, 4.0, 6.0));
        assert_eq!((m.m11, m.m22, m.m33, m.m44), (2.0, 2.0, 2.0, 1.0));
        // scale first, then translate keeps the translation
        let n = Matrix::create_scale(2.0, 2.0, 2.0) * Matrix::create_translation(1.0, 2.0, 3.0);
        assert_eq!(n.translation(), Vector3::new(1.0, 2.0, 3.0));
        assert_eq!(Matrix::multiply(Matrix::IDENTITY, n), n);
        assert_eq!(n * Matrix::IDENTITY, n);
    }

    #[test]
    fn equality() {
        let a = Matrix::create_translation(1.0, 2.0, 3.0);
        assert_eq!(a, Matrix::create_translation(1.0, 2.0, 3.0));
        assert_ne!(a, Matrix::create_translation(1.0, 2.0, 4.0));
        let mut b = a;
        b.m14 = 1.0;
        assert_ne!(a, b);
    }
}
