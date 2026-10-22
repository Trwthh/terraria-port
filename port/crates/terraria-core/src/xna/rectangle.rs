//! `Microsoft.Xna.Framework.Rectangle` (reference/xna/Rectangle.cs).
//! Integer arithmetic wraps on overflow like unchecked C# `int`.

use super::Point;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
#[repr(C)]
pub struct Rectangle {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

impl Rectangle {
    pub const EMPTY: Rectangle = Rectangle { x: 0, y: 0, width: 0, height: 0 };

    pub const fn new(x: i32, y: i32, width: i32, height: i32) -> Rectangle {
        Rectangle { x, y, width, height }
    }

    pub const fn left(&self) -> i32 {
        self.x
    }

    pub const fn right(&self) -> i32 {
        self.x.wrapping_add(self.width)
    }

    pub const fn top(&self) -> i32 {
        self.y
    }

    pub const fn bottom(&self) -> i32 {
        self.y.wrapping_add(self.height)
    }

    pub const fn location(&self) -> Point {
        Point { x: self.x, y: self.y }
    }

    pub fn set_location(&mut self, value: Point) {
        self.x = value.x;
        self.y = value.y;
    }

    /// `Center`: `X + Width / 2`, `Y + Height / 2` (integer division truncates).
    pub const fn center(&self) -> Point {
        Point {
            x: self.x.wrapping_add(self.width / 2),
            y: self.y.wrapping_add(self.height / 2),
        }
    }

    pub const fn is_empty(&self) -> bool {
        self.width == 0 && self.height == 0 && self.x == 0 && self.y == 0
    }

    /// `Offset(Point amount)`.
    pub fn offset(&mut self, amount: Point) {
        self.x = self.x.wrapping_add(amount.x);
        self.y = self.y.wrapping_add(amount.y);
    }

    /// `Offset(int offsetX, int offsetY)`.
    pub fn offset_xy(&mut self, offset_x: i32, offset_y: i32) {
        self.x = self.x.wrapping_add(offset_x);
        self.y = self.y.wrapping_add(offset_y);
    }

    pub fn inflate(&mut self, horizontal_amount: i32, vertical_amount: i32) {
        self.x = self.x.wrapping_sub(horizontal_amount);
        self.y = self.y.wrapping_sub(vertical_amount);
        self.width = self.width.wrapping_add(horizontal_amount.wrapping_mul(2));
        self.height = self.height.wrapping_add(vertical_amount.wrapping_mul(2));
    }

    /// `Contains(int x, int y)`: right and bottom edges are exclusive.
    pub const fn contains_xy(&self, x: i32, y: i32) -> bool {
        self.x <= x
            && x < self.x.wrapping_add(self.width)
            && self.y <= y
            && y < self.y.wrapping_add(self.height)
    }

    /// `Contains(Point value)`.
    pub const fn contains_point(&self, value: Point) -> bool {
        self.contains_xy(value.x, value.y)
    }

    /// `Contains(Rectangle value)`: true when `value` lies fully inside (edges may touch).
    pub const fn contains(&self, value: Rectangle) -> bool {
        self.x <= value.x
            && value.x.wrapping_add(value.width) <= self.x.wrapping_add(self.width)
            && self.y <= value.y
            && value.y.wrapping_add(value.height) <= self.y.wrapping_add(self.height)
    }

    /// `Intersects(Rectangle value)`: rectangles that only touch edges do not intersect.
    pub const fn intersects(&self, value: Rectangle) -> bool {
        value.x < self.x.wrapping_add(self.width)
            && self.x < value.x.wrapping_add(value.width)
            && value.y < self.y.wrapping_add(self.height)
            && self.y < value.y.wrapping_add(value.height)
    }

    /// Static `Intersect`: the overlap, or the empty rectangle if there is none.
    pub fn intersect(value1: Rectangle, value2: Rectangle) -> Rectangle {
        let num = value1.x.wrapping_add(value1.width);
        let num2 = value2.x.wrapping_add(value2.width);
        let num3 = value1.y.wrapping_add(value1.height);
        let num4 = value2.y.wrapping_add(value2.height);
        let num5 = if value1.x > value2.x { value1.x } else { value2.x };
        let num6 = if value1.y > value2.y { value1.y } else { value2.y };
        let num7 = if num < num2 { num } else { num2 };
        let num8 = if num3 < num4 { num3 } else { num4 };
        if num7 > num5 && num8 > num6 {
            Rectangle {
                x: num5,
                y: num6,
                width: num7.wrapping_sub(num5),
                height: num8.wrapping_sub(num6),
            }
        } else {
            Rectangle::EMPTY
        }
    }

    /// Static `Union`: the bounding rectangle of both.
    pub fn union(value1: Rectangle, value2: Rectangle) -> Rectangle {
        let num = value1.x.wrapping_add(value1.width);
        let num2 = value2.x.wrapping_add(value2.width);
        let num3 = value1.y.wrapping_add(value1.height);
        let num4 = value2.y.wrapping_add(value2.height);
        let num5 = if value1.x < value2.x { value1.x } else { value2.x };
        let num6 = if value1.y < value2.y { value1.y } else { value2.y };
        let num7 = if num > num2 { num } else { num2 };
        let num8 = if num3 > num4 { num3 } else { num4 };
        Rectangle {
            x: num5,
            y: num6,
            width: num7.wrapping_sub(num5),
            height: num8.wrapping_sub(num6),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(x: i32, y: i32, w: i32, h: i32) -> Rectangle {
        Rectangle::new(x, y, w, h)
    }

    #[test]
    fn edges_location_center() {
        let a = r(10, 20, 30, 40);
        assert_eq!((a.left(), a.right(), a.top(), a.bottom()), (10, 40, 20, 60));
        assert_eq!(a.location(), Point::new(10, 20));
        assert_eq!(a.center(), Point::new(25, 40));
        // odd sizes: 5 + 7/2 = 8 (truncating division)
        assert_eq!(r(5, 5, 7, 9).center(), Point::new(8, 9));
        // negative sizes truncate toward zero: 0 + (-3)/2 = -1
        assert_eq!(r(0, 0, -3, -3).center(), Point::new(-1, -1));
        let mut b = a;
        b.set_location(Point::new(-1, -2));
        assert_eq!(b, r(-1, -2, 30, 40));
    }

    #[test]
    fn empty() {
        assert!(Rectangle::EMPTY.is_empty());
        assert!(Rectangle::default().is_empty());
        assert!(!r(0, 0, 0, 1).is_empty());
        assert!(!r(1, 0, 0, 0).is_empty());
        assert!(!r(0, 1, 0, 0).is_empty());
        assert!(!r(0, 0, 1, 0).is_empty());
    }

    #[test]
    fn offset_inflate() {
        let mut a = r(10, 20, 30, 40);
        a.offset(Point::new(1, -2));
        assert_eq!(a, r(11, 18, 30, 40));
        a.offset_xy(-11, 2);
        assert_eq!(a, r(0, 20, 30, 40));
        a.inflate(5, 3);
        assert_eq!(a, r(-5, 17, 40, 46));
        a.inflate(-5, -3);
        assert_eq!(a, r(0, 20, 30, 40));
        // wraps instead of panicking
        let mut w = r(i32::MAX, 0, 1, 0);
        w.offset_xy(1, 0);
        assert_eq!(w.x, i32::MIN);
    }

    #[test]
    fn contains_edges() {
        let a = r(0, 0, 10, 10);
        assert!(a.contains_xy(0, 0));
        assert!(a.contains_xy(9, 9));
        assert!(!a.contains_xy(10, 5)); // right edge exclusive
        assert!(!a.contains_xy(5, 10)); // bottom edge exclusive
        assert!(!a.contains_xy(-1, 5));
        assert!(a.contains_point(Point::new(5, 5)));
        assert!(!a.contains_point(Point::new(10, 10)));
        assert!(a.contains(r(0, 0, 10, 10)));
        assert!(a.contains(r(2, 2, 8, 8))); // touching the far edges is fine
        assert!(!a.contains(r(2, 2, 9, 8)));
        assert!(!a.contains(r(-1, 0, 5, 5)));
        assert!(!a.contains(r(0, 0, 10, 11)));
        // zero-size rectangle contains nothing by point
        assert!(!r(0, 0, 0, 0).contains_xy(0, 0));
    }

    #[test]
    fn intersects_edges() {
        let a = r(0, 0, 10, 10);
        assert!(a.intersects(r(5, 5, 10, 10)));
        assert!(a.intersects(r(9, 9, 5, 5)));
        assert!(a.intersects(r(2, 2, 2, 2))); // fully inside
        // touching edges do not intersect
        assert!(!a.intersects(r(10, 0, 5, 5)));
        assert!(!a.intersects(r(0, 10, 5, 5)));
        assert!(!a.intersects(r(-5, 0, 5, 5)));
        assert!(!a.intersects(r(0, -5, 5, 5)));
        assert!(!a.intersects(r(20, 20, 5, 5)));
        // symmetric
        assert!(r(5, 5, 10, 10).intersects(a));
    }

    #[test]
    fn intersect_and_union() {
        assert_eq!(
            Rectangle::intersect(r(0, 0, 10, 10), r(5, 6, 10, 10)),
            r(5, 6, 5, 4)
        );
        assert_eq!(
            Rectangle::intersect(r(0, 0, 10, 10), r(2, 3, 2, 2)),
            r(2, 3, 2, 2)
        );
        // touching or disjoint -> empty (all zeros, not the touching edge)
        assert_eq!(Rectangle::intersect(r(0, 0, 10, 10), r(10, 0, 5, 5)), Rectangle::EMPTY);
        assert_eq!(Rectangle::intersect(r(0, 0, 10, 10), r(0, 10, 5, 5)), Rectangle::EMPTY);
        assert_eq!(Rectangle::intersect(r(0, 0, 10, 10), r(50, 50, 5, 5)), Rectangle::EMPTY);
        assert_eq!(
            Rectangle::union(r(0, 0, 10, 10), r(5, -5, 10, 10)),
            r(0, -5, 15, 15)
        );
        // union with a disjoint rectangle spans the gap
        assert_eq!(
            Rectangle::union(r(0, 0, 1, 1), r(10, 10, 1, 1)),
            r(0, 0, 11, 11)
        );
    }

    #[test]
    fn equality() {
        assert_eq!(r(1, 2, 3, 4), r(1, 2, 3, 4));
        assert_ne!(r(1, 2, 3, 4), r(1, 2, 3, 5));
        assert_ne!(r(1, 2, 3, 4), r(1, 2, 4, 4));
        assert_ne!(r(1, 2, 3, 4), r(0, 2, 3, 4));
        assert_ne!(r(1, 2, 3, 4), r(1, 0, 3, 4));
    }
}
