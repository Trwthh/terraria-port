//! `Terraria.DataStructures.Point16`
//! (decompile/Terraria/Terraria/DataStructures/Point16.cs).

use super::Point;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
#[repr(C)]
pub struct Point16 {
    pub x: i16,
    pub y: i16,
}

impl Point16 {
    pub const ZERO: Point16 = Point16 { x: 0, y: 0 };
    pub const NEGATIVE_ONE: Point16 = Point16 { x: -1, y: -1 };

    /// `Point16(short X, short Y)`.
    pub const fn new(x: i16, y: i16) -> Point16 {
        Point16 { x, y }
    }

    /// `Point16(int X, int Y)`: truncates to 16 bits like `(short)X`.
    pub const fn from_ints(x: i32, y: i32) -> Point16 {
        Point16 {
            x: x as i16,
            y: y as i16,
        }
    }

    /// `Point16(Point point)`: truncates to 16 bits.
    pub const fn from_point(point: Point) -> Point16 {
        Point16 {
            x: point.x as i16,
            y: point.y as i16,
        }
    }

    /// Static `Point16.Max(int firstX, int firstY, int secondX, int secondY)`.
    pub const fn max_of(first_x: i32, first_y: i32, second_x: i32, second_y: i32) -> Point16 {
        Point16::from_ints(
            if first_x > second_x { first_x } else { second_x },
            if first_y > second_y { first_y } else { second_y },
        )
    }

    /// Instance `Max(int compareX, int compareY)`.
    pub const fn max_xy(&self, compare_x: i32, compare_y: i32) -> Point16 {
        Point16::from_ints(
            if self.x as i32 > compare_x { self.x as i32 } else { compare_x },
            if self.y as i32 > compare_y { self.y as i32 } else { compare_y },
        )
    }

    /// Instance `Max(Point16 compareTo)`.
    pub const fn max(&self, compare_to: Point16) -> Point16 {
        Point16 {
            x: if self.x > compare_to.x { self.x } else { compare_to.x },
            y: if self.y > compare_to.y { self.y } else { compare_to.y },
        }
    }

    /// The implicit `Point16 -> Point` conversion.
    pub const fn to_point(&self) -> Point {
        Point {
            x: self.x as i32,
            y: self.y as i32,
        }
    }

    /// `GetHashCode()`: `(X << 16) | (ushort)Y`.
    pub const fn hash_code(&self) -> i32 {
        ((self.x as i32) << 16) | (self.y as u16 as i32)
    }
}

impl From<Point16> for Point {
    fn from(p: Point16) -> Point {
        p.to_point()
    }
}

impl core::fmt::Display for Point16 {
    /// `ToString()`: `{X, Y}`.
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{{{}, {}}}", self.x, self.y)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constants_and_constructors() {
        assert_eq!(Point16::ZERO, Point16::new(0, 0));
        assert_eq!(Point16::NEGATIVE_ONE, Point16::new(-1, -1));
        assert_eq!(Point16::default(), Point16::ZERO);
        assert_eq!(Point16::from_ints(5, -7), Point16::new(5, -7));
        // (short)65537 == 1, (short)40000 == -25536
        assert_eq!(Point16::from_ints(65537, 40000), Point16::new(1, -25536));
        assert_eq!(Point16::from_point(Point::new(70000, -3)), Point16::new(4464, -3));
    }

    #[test]
    fn max_overloads() {
        assert_eq!(Point16::max_of(1, 9, 4, 2), Point16::new(4, 9));
        assert_eq!(Point16::new(3, 8).max_xy(5, 2), Point16::new(5, 8));
        assert_eq!(Point16::new(3, 8).max(Point16::new(5, 2)), Point16::new(5, 8));
        assert_eq!(Point16::new(-3, -8).max(Point16::NEGATIVE_ONE), Point16::NEGATIVE_ONE);
    }

    #[test]
    fn conversion_hash_and_text() {
        assert_eq!(Point16::new(-2, 7).to_point(), Point::new(-2, 7));
        assert_eq!(Point::from(Point16::new(100, -100)), Point::new(100, -100));
        assert_eq!(Point16::new(1, 2).hash_code(), (1 << 16) | 2);
        // (ushort)(-1) = 0xFFFF ORed in: -1 << 16 | 0xFFFF = -1
        assert_eq!(Point16::new(-1, -1).hash_code(), -1);
        assert_eq!(Point16::new(3, -4).to_string(), "{3, -4}");
    }

    #[test]
    fn equality() {
        assert_eq!(Point16::new(1, 2), Point16::new(1, 2));
        assert_ne!(Point16::new(1, 2), Point16::new(1, 3));
        assert_ne!(Point16::new(1, 2), Point16::new(0, 2));
    }
}
