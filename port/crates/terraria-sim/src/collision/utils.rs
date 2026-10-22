//! Helpers `Terraria.Collision` takes from `Terraria.Utils`, `System.Math` and the `Microsoft.Xna.Framework` extension
//! methods, plus the tile accessors that mirror `Main.tile[x, y]` (private to the collision module).
//!
//! Exceptions: the original throws (`IndexOutOfRangeException` / `NullReferenceException` from `Main.tile[x, y]`, an
//! `Exception` in `DistFromSeg`) and some methods catch with `try { } catch { }`. Here `throw` unwinds with a payload and no
//! panic message (`resume_unwind` does not run the panic hook) and `catch` is the `try { } catch { }` block, so the control
//! flow of the original is kept.
//!
//! `Main.tile` is the array `new Tile[maxTilesX, maxTilesY]` created when `Main` is initialised, with the default
//! `maxTilesX = (int)rightWorld / 16 + 1 = 8401` and `maxTilesY = (int)bottomWorld / 16 + 1 = 2401` (Main.cs:1081-1089, 1689);
//! loading a smaller world fills only the part inside the world. An index inside the world gives a tile, an index inside the
//! array but outside the world gives `null` (so `Main.tile[x, y] == null` is true there and member access throws
//! `NullReferenceException`), an index outside the array throws `IndexOutOfRangeException`. [`tile_slot`] is the read that
//! can give `null` (`None`); [`tile_at`] is a read that is followed by a member access.

use std::panic::{catch_unwind, resume_unwind, AssertUnwindSafe};

use terraria_core::xna::{Point, Vector2, Vector3, Vector4};
use terraria_core::Tile;

use crate::game::Game;
use crate::unported;

/// The payload of a thrown exception (the .NET exception type and message).
#[derive(Debug)]
pub struct Exception(pub &'static str);

/// `throw new ...`
pub(crate) fn throw(what: &'static str) -> ! {
    resume_unwind(Box::new(Exception(what)))
}

/// `try { body } catch { }`: `None` when the body threw.
pub(crate) fn catch<T>(body: impl FnOnce() -> T) -> Option<T> {
    catch_unwind(AssertUnwindSafe(body)).ok()
}

/// First dimension of `Main.tile` (see the module doc).
pub const MAIN_TILE_LEN_X: i32 = 8401;
/// Second dimension of `Main.tile` (see the module doc).
pub const MAIN_TILE_LEN_Y: i32 = 2401;

/// `Main.tile[x, y]` as an expression that may be compared with `null`: a copy of the tile, `None` for `null` (inside the array,
/// outside the world), throws `IndexOutOfRangeException` outside the array.
#[inline]
pub(crate) fn tile_slot(g: &Game, x: i32, y: i32) -> Option<Tile> {
    if x < 0 || y < 0 || x >= MAIN_TILE_LEN_X || y >= MAIN_TILE_LEN_Y {
        throw("IndexOutOfRangeException: Main.tile[x, y]");
    }
    g.main.tile.try_get(x, y)
}

/// `Main.tile[x, y].member`: like [`tile_slot`] but throws `NullReferenceException` for `null`.
#[inline]
pub(crate) fn tile_at(g: &Game, x: i32, y: i32) -> Tile {
    match tile_slot(g, x, y) {
        Some(t) => t,
        None => throw("NullReferenceException: Main.tile[x, y]"),
    }
}

/// A tile read earlier with [`tile_slot`] whose member is used now: throws `NullReferenceException` when it was `null`.
pub(crate) fn null_check(t: Option<Tile>) -> Tile {
    match t {
        Some(t) => t,
        None => throw("NullReferenceException: Main.tile[x, y]"),
    }
}

/// `if (Main.tile[x, y] == null) Main.tile[x, y] = new Tile();` followed by a read of `Main.tile[x, y]`.
/// A tile outside the world cannot be stored (`TileMap` has the world's size only): the allocation is `unported!` and the read
/// gives the empty tile the allocation would have made.
pub(crate) fn tile_or_new(g: &Game, x: i32, y: i32) -> Tile {
    match tile_slot(g, x, y) {
        Some(t) => t,
        None => {
            unported!("Main.tile[x, y] = new Tile() outside the world");
            Tile::default()
        }
    }
}
/// `Math.Min(float, float)` of .NET Framework: `val1 < val2 ? val1 : (IsNaN(val1) ? val1 : val2)`.
pub(crate) fn math_min_f32(val1: f32, val2: f32) -> f32 {
    if val1 < val2 {
        return val1;
    }
    if val1.is_nan() {
        return val1;
    }
    val2
}

/// `Math.Max(float, float)` of .NET Framework: `val1 > val2 ? val1 : (IsNaN(val1) ? val1 : val2)`.
pub(crate) fn math_max_f32(val1: f32, val2: f32) -> f32 {
    if val1 > val2 {
        return val1;
    }
    if val1.is_nan() {
        return val1;
    }
    val2
}

/// `Math.Sign(int)`
#[inline]
pub(crate) fn math_sign(value: i32) -> i32 {
    value.signum()
}

/// `Utils.Clamp<int>(value, min, max)` (Utils.cs:223)
#[inline]
pub(crate) fn clamp_i32(value: i32, min: i32, max: i32) -> i32 {
    if value > max {
        return max;
    }
    if value < min {
        return min;
    }
    value
}

/// `Utils.ToPoint(this Vector2)` (Utils.cs:1836): `new Point((int)v.X, (int)v.Y)`.
#[inline]
pub(crate) fn to_point(v: Vector2) -> Point {
    Point::new(v.x as i32, v.y as i32)
}

/// `Utils.ToTileCoordinates(this Vector2)` (Utils.cs:1826): `new Point((int)vec.X >> 4, (int)vec.Y >> 4)`.
#[inline]
pub(crate) fn to_tile_coordinates(vec: Vector2) -> Point {
    Point::new((vec.x as i32) >> 4, (vec.y as i32) >> 4)
}

/// `Utils.RotatedBy(this Vector2, double radians, Vector2 center = default)` (Utils.cs:1694), `center` = zero.
pub(crate) fn rotated_by(spinningpoint: Vector2, radians: f64) -> Vector2 {
    let num = radians.cos() as f32;
    let num2 = radians.sin() as f32;
    let center = Vector2::ZERO;
    let vector = spinningpoint - center;
    let mut result = center;
    result.x += vector.x * num - vector.y * num2;
    result.y += vector.x * num2 + vector.y * num;
    result
}

/// `Utils.Between(this Vector2, Vector2 minimum, Vector2 maximum)` (Utils.cs:1737)
fn between(vec: Vector2, minimum: Vector2, maximum: Vector2) -> bool {
    if vec.x >= minimum.x && vec.x <= maximum.x && vec.y >= minimum.y {
        return vec.y <= maximum.y;
    }
    false
}

/// `Utils.ClosestPointOnLine(this Vector2 P, Vector2 A, Vector2 B)` (Utils.cs:1879)
fn closest_point_on_line(p: Vector2, a: Vector2, b: Vector2) -> Vector2 {
    let value = p - a;
    let vector = b - a;
    let num = vector.length_squared();
    let num2 = Vector2::dot(value, vector) / num;
    if num2 < 0f32 {
        return a;
    }
    if num2 > 1f32 {
        return b;
    }
    a + vector * num2
}

/// `Utils.RectangleLineCollision(rectTopLeft, rectBottomRight, lineStart, lineEnd)` (Utils.cs:1913).
/// Note the original tests `array[0]` on every pass of its loop (as here).
pub(crate) fn rectangle_line_collision(rect_top_left: Vector2, rect_bottom_right: Vector2, line_start: Vector2, line_end: Vector2) -> bool {
    if between(line_start, rect_top_left, rect_bottom_right) || between(line_end, rect_top_left, rect_bottom_right) {
        return true;
    }
    let p = Vector2::new(rect_bottom_right.x, rect_top_left.y);
    let vector = Vector2::new(rect_top_left.x, rect_bottom_right.y);
    let array: [Vector2; 4] = [
        closest_point_on_line(rect_top_left, line_start, line_end),
        closest_point_on_line(p, line_start, line_end),
        closest_point_on_line(vector, line_start, line_end),
        closest_point_on_line(rect_bottom_right, line_start, line_end),
    ];
    for _i in 0..array.len() {
        if between(array[0], rect_top_left, vector) {
            return true;
        }
    }
    false
}

/// `Utils.FloatIntersect(r1StartX, r1StartY, r1Width, r1Height, r2StartX, r2StartY, r2Width, r2Height)` (Utils.cs:998)
pub(crate) fn float_intersect(r1_start_x: f32, r1_start_y: f32, r1_width: f32, r1_height: f32, r2_start_x: f32, r2_start_y: f32, r2_width: f32, r2_height: f32) -> bool {
    if r1_start_x > r2_start_x + r2_width || r1_start_y > r2_start_y + r2_height || r1_start_x + r1_width < r2_start_x || r1_start_y + r1_height < r2_start_y {
        return false;
    }
    true
}

/// `Vector2.HasNaNs()` (Utils.cs): `float.IsNaN(X) || float.IsNaN(Y)`.
pub(crate) fn has_nans(v: Vector2) -> bool {
    v.x.is_nan() || v.y.is_nan()
}

/// `Utils.XY(this Vector4)` (Utils.cs:1990)
pub(crate) fn xy(vec: Vector4) -> Vector2 {
    Vector2::new(vec.x, vec.y)
}

/// `Utils.ZW(this Vector4)` (Utils.cs:1995)
pub(crate) fn zw(vec: Vector4) -> Vector2 {
    Vector2::new(vec.z, vec.w)
}

/// `Utils.XZW(this Vector4)` (Utils.cs:2000)
pub(crate) fn xzw(vec: Vector4) -> Vector3 {
    Vector3::new(vec.x, vec.z, vec.w)
}

/// `Utils.YZW(this Vector4)` (Utils.cs:2005)
pub(crate) fn yzw(vec: Vector4) -> Vector3 {
    Vector3::new(vec.y, vec.z, vec.w)
}

/// `WorldGen.PlatformProperTopFrame(short frameX)` (WorldGen.cs:70965). `TileObjectData.PlatformFrameWidth()` is
/// `_data[19].CoordinateFullWidth`, which is 18 in the game's own data (refdata/tile_object_data.json, tile 19).
pub(crate) fn platform_proper_top_frame(frame_x: i16) -> bool {
    let num = frame_x as i32 / 18;
    if (num < 0 || num > 7) && (num < 12 || num > 16) {
        if num >= 25 {
            return num <= 26;
        }
        return false;
    }
    true
}

/// `WorldGen.SolidTile3(int i, int j)` (WorldGen.cs:71277) with `SolidTile3(Tile t)` (WorldGen.cs:71286) inlined
/// (`t == null` cannot happen in the port).
pub(crate) fn solid_tile3(g: &Game, i: i32, j: i32) -> bool {
    if !crate::world_gen::tile_frame::in_world(g, i, j, 1) {
        return false;
    }
    let t = tile_at(g, i, j);
    if t.nactive() && g.main.tile_solid[t.type_ as usize] {
        return !g.main.tile_solid_top[t.type_ as usize];
    }
    false
}

/// `Terraria.BitsByte` (BitsByte.cs): eight flags in one byte, indexer = bit number.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct BitsByte {
    value: u8,
}

impl BitsByte {
    pub(crate) fn new(value: u8) -> Self {
        Self { value }
    }
    pub(crate) fn get(&self, key: i32) -> bool {
        (self.value & (1 << key)) != 0
    }
    pub(crate) fn set(&mut self, key: i32, flag: bool) {
        if flag {
            self.value |= (1 << key) as u8;
        } else {
            self.value &= !(1 << key) as u8;
        }
    }
}