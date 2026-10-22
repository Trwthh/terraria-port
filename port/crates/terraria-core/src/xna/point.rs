//! `Microsoft.Xna.Framework.Point` (reference/xna/Point.cs).
//! The reference defines only the two fields, `Zero` and equality.

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
#[repr(C)]
pub struct Point {
    pub x: i32,
    pub y: i32,
}

impl Point {
    pub const ZERO: Point = Point { x: 0, y: 0 };

    pub const fn new(x: i32, y: i32) -> Point {
        Point { x, y }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_and_equality() {
        assert_eq!(Point::ZERO, Point::new(0, 0));
        assert_eq!(Point::default(), Point::ZERO);
        assert_eq!(Point::new(3, -4), Point { x: 3, y: -4 });
        assert_ne!(Point::new(3, -4), Point::new(3, 4));
        assert_ne!(Point::new(3, -4), Point::new(-3, -4));
    }
}
