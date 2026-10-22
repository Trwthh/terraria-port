//! `Terraria.Collision`, pure geometry: `CheckLinevLine`, the AABB / line tests (Collision.cs:79-381).
//!
//! An exception thrown by the original (`DistFromSeg`) is `utils::throw` here.

use terraria_core::xna::Vector2;

use super::utils::{math_max_f32, math_min_f32, rectangle_line_collision, rotated_by, throw};
use crate::game::Game;

/// `Collision.CheckLinevLine` (Collision.cs:79). `Epsilon` is `g.collision.epsilon`.
pub fn check_linev_line(g: &Game, a1: Vector2, a2: Vector2, b1: Vector2, b2: Vector2) -> Vec<Vector2> {
    let epsilon = g.collision.epsilon;
    if a1 == a2 && b1 == b2 {
        if a1 == b1 {
            return vec![a1];
        }
        return Vec::new();
    }
    if b1 == b2 {
        if point_on_line(g, b1, a1, a2) {
            return vec![b1];
        }
        return Vec::new();
    }
    if a1 == a2 {
        if point_on_line(g, a1, b1, b2) {
            return vec![a1];
        }
        return Vec::new();
    }
    let num = (b2.x - b1.x) * (a1.y - b1.y) - (b2.y - b1.y) * (a1.x - b1.x);
    let num2 = (a2.x - a1.x) * (a1.y - b1.y) - (a2.y - a1.y) * (a1.x - b1.x);
    let num3 = (b2.y - b1.y) * (a2.x - a1.x) - (b2.x - b1.x) * (a2.y - a1.y);
    if !(0f32 - epsilon < num3) || !(num3 < epsilon) {
        let num4 = num / num3;
        let num5 = num2 / num3;
        if 0f32 <= num4 && num4 <= 1f32 && 0f32 <= num5 && num5 <= 1f32 {
            return vec![Vector2::new(a1.x + num4 * (a2.x - a1.x), a1.y + num4 * (a2.y - a1.y))];
        }
        return Vec::new();
    }
    if (0f32 - epsilon < num && num < epsilon) || (0f32 - epsilon < num2 && num2 < epsilon) {
        if a1 == a2 {
            return one_dimensional_intersection(b1, b2, a1, a2);
        }
        return one_dimensional_intersection(a1, a2, b1, b2);
    }
    Vec::new()
}

/// `Collision.DistFromSeg` (Collision.cs:132). `u` is only passed through in the original and never written.
/// Throws where the original throws `Exception("Expected line segment, not point.")`.
fn dist_from_seg(g: &Game, p: Vector2, q0: Vector2, q1: Vector2, _radius: f64, _u: &mut f32) -> f64 {
    let num = (q1.x - q0.x) as f64;
    let num2 = (q1.y - q0.y) as f64;
    let num3 = (q0.x - p.x) as f64;
    let num4 = (q0.y - p.y) as f64;
    let num5 = (num * num + num2 * num2).sqrt();
    if num5 < g.collision.epsilon as f64 {
        throw("Exception: Expected line segment, not point.");
    }
    (num * num4 - num3 * num2).abs() / num5
}

/// `Collision.PointOnLine` (Collision.cs:146)
fn point_on_line(g: &Game, p: Vector2, a1: Vector2, a2: Vector2) -> bool {
    let mut u = 0f32;
    dist_from_seg(g, p, a1, a2, g.collision.epsilon as f64, &mut u) < g.collision.epsilon as f64
}

/// `Collision.OneDimensionalIntersection` (Collision.cs:152)
fn one_dimensional_intersection(a1: Vector2, a2: Vector2, b1: Vector2, b2: Vector2) -> Vec<Vector2> {
    let num = a2.x - a1.x;
    let num2 = a2.y - a1.y;
    let relative_point;
    let relative_point2;
    if num.abs() > num2.abs() {
        relative_point = (b1.x - a1.x) / num;
        relative_point2 = (b2.x - a1.x) / num;
    } else {
        relative_point = (b1.y - a1.y) / num2;
        relative_point2 = (b2.y - a1.y) / num2;
    }
    let mut list: Vec<Vector2> = Vec::new();
    let array = find_overlap_points(relative_point, relative_point2);
    for num3 in array {
        let x = a2.x * num3 + a1.x * (1f32 - num3);
        let y = a2.y * num3 + a1.y * (1f32 - num3);
        list.push(Vector2::new(x, y));
    }
    list
}

/// `Collision.FindOverlapPoints` (Collision.cs:179)
fn find_overlap_points(relative_point1: f32, relative_point2: f32) -> Vec<f32> {
    let val = math_min_f32(relative_point1, relative_point2);
    let val2 = math_max_f32(relative_point1, relative_point2);
    let num = math_max_f32(0f32, val);
    let num2 = math_min_f32(1f32, val2);
    if num > num2 {
        return Vec::new();
    }
    if num != num2 {
        return vec![num, num2];
    }
    vec![num]
}

/// `Collision.CheckAABBvAABBCollision` (Collision.cs:196)
pub fn check_aab_bv_aabb_collision(position1: Vector2, dimensions1: Vector2, position2: Vector2, dimensions2: Vector2) -> bool {
    if position1.x < position2.x + dimensions2.x && position1.y < position2.y + dimensions2.y && position1.x + dimensions1.x > position2.x {
        return position1.y + dimensions1.y > position2.y;
    }
    false
}

/// `Collision.collisionOutcode` (Collision.cs:205)
fn collision_outcode(aabb_position: Vector2, aabb_dimensions: Vector2, point: Vector2) -> i32 {
    let num = aabb_position.x + aabb_dimensions.x;
    let num2 = aabb_position.y + aabb_dimensions.y;
    let mut num3 = 0;
    if aabb_dimensions.x <= 0f32 {
        num3 |= 5;
    } else if point.x < aabb_position.x {
        num3 |= 1;
    } else if point.x - num > 0f32 {
        num3 |= 4;
    }
    if aabb_dimensions.y <= 0f32 {
        num3 |= 0xA;
    } else if point.y < aabb_position.y {
        num3 |= 2;
    } else if point.y - num2 > 0f32 {
        num3 |= 8;
    }
    num3
}

/// `Collision.CheckAABBvLineCollision(aabbPosition, aabbDimensions, lineStart, lineEnd)` (Collision.cs:237),
/// the 4-argument overload.
pub fn check_aab_bv_line_collision(aabb_position: Vector2, aabb_dimensions: Vector2, mut line_start: Vector2, line_end: Vector2) -> bool {
    let num = collision_outcode(aabb_position, aabb_dimensions, line_end);
    if num == 0 {
        return true;
    }
    loop {
        let num2 = collision_outcode(aabb_position, aabb_dimensions, line_start);
        if num2 == 0 {
            break;
        }
        if (num2 & num) != 0 {
            return false;
        }
        if (num2 & 5) != 0 {
            let mut num3 = aabb_position.x;
            if (num2 & 4) != 0 {
                num3 += aabb_dimensions.x;
            }
            line_start.y += (num3 - line_start.x) * (line_end.y - line_start.y) / (line_end.x - line_start.x);
            line_start.x = num3;
        } else {
            let mut num4 = aabb_position.y;
            if (num2 & 8) != 0 {
                num4 += aabb_dimensions.y;
            }
            line_start.x += (num4 - line_start.y) * (line_end.x - line_start.x) / (line_end.y - line_start.y);
            line_start.y = num4;
        }
    }
    true
}

/// `Collision.CheckAABBvLineCollision2` (Collision.cs:275)
pub fn check_aab_bv_line_collision2(aabb_position: Vector2, aabb_dimensions: Vector2, line_start: Vector2, line_end: Vector2) -> bool {
    let mut collision_point = 0f32;
    if !rectangle_line_collision(aabb_position, aabb_position + aabb_dimensions, line_start, line_end) {
        return check_aab_bv_line_collision_with_width(aabb_position, aabb_dimensions, line_start, line_end, 0.0001f32, &mut collision_point);
    }
    true
}

/// `Collision.CheckAABBvLineCollision(objectPosition, objectDimensions, lineStart, lineEnd, lineWidth, ref collisionPoint)`
/// (Collision.cs:285), the 6-argument overload.
pub fn check_aab_bv_line_collision_with_width(
    object_position: Vector2,
    object_dimensions: Vector2,
    line_start: Vector2,
    line_end: Vector2,
    line_width: f32,
    collision_point: &mut f32,
) -> bool {
    let num = line_width * 0.5f32;
    let mut position = line_start;
    let mut dimensions = line_end - line_start;
    if dimensions.x > 0f32 {
        dimensions.x += line_width;
        position.x -= num;
    } else {
        position.x += dimensions.x - num;
        dimensions.x = 0f32 - dimensions.x + line_width;
    }
    if dimensions.y > 0f32 {
        dimensions.y += line_width;
        position.y -= num;
    } else {
        position.y += dimensions.y - num;
        dimensions.y = 0f32 - dimensions.y + line_width;
    }
    if !check_aab_bv_aabb_collision(object_position, object_dimensions, position, dimensions) {
        return false;
    }
    let vector = object_position - line_start;
    let spinningpoint = vector + object_dimensions;
    let spinningpoint2 = Vector2::new(vector.x, spinningpoint.y);
    let spinningpoint3 = Vector2::new(spinningpoint.x, vector.y);
    let vector2 = line_end - line_start;
    let num2 = vector2.length();
    let num3 = (vector2.y as f64).atan2(vector2.x as f64) as f32;
    let array: [Vector2; 4] = [
        rotated_by(vector, (0f32 - num3) as f64),
        rotated_by(spinningpoint3, (0f32 - num3) as f64),
        rotated_by(spinningpoint, (0f32 - num3) as f64),
        rotated_by(spinningpoint2, (0f32 - num3) as f64),
    ];
    *collision_point = num2;
    let mut result = false;
    for i in 0..array.len() {
        if array[i].y.abs() < num && array[i].x < *collision_point && array[i].x >= 0f32 {
            *collision_point = array[i].x;
            result = true;
        }
    }
    let vector3 = Vector2::new(0f32, num);
    let vector4 = Vector2::new(num2, num);
    let vector5 = Vector2::new(0f32, 0f32 - num);
    let vector6 = Vector2::new(num2, 0f32 - num);
    for j in 0..array.len() {
        let num4 = (j + 1) % array.len();
        let mut vector7 = vector4 - vector3;
        let vector8 = array[num4] - array[j];
        let mut num5 = vector7.x * vector8.y - vector7.y * vector8.x;
        if num5 != 0f32 {
            let vector9 = array[j] - vector3;
            let num6 = (vector9.x * vector8.y - vector9.y * vector8.x) / num5;
            if num6 >= 0f32 && num6 <= 1f32 {
                let num7 = (vector9.x * vector7.y - vector9.y * vector7.x) / num5;
                if num7 >= 0f32 && num7 <= 1f32 {
                    result = true;
                    *collision_point = math_min_f32(*collision_point, vector3.x + num6 * vector7.x);
                }
            }
        }
        vector7 = vector6 - vector5;
        num5 = vector7.x * vector8.y - vector7.y * vector8.x;
        if num5 == 0f32 {
            continue;
        }
        let vector10 = array[j] - vector5;
        let num8 = (vector10.x * vector8.y - vector10.y * vector8.x) / num5;
        if num8 >= 0f32 && num8 <= 1f32 {
            let num9 = (vector10.x * vector7.y - vector10.y * vector7.x) / num5;
            if num9 >= 0f32 && num9 <= 1f32 {
                result = true;
                *collision_point = math_min_f32(*collision_point, vector5.x + num8 * vector7.x);
            }
        }
    }
    result
}
