//! A reflected field value, for comparing translated objects with `refdata/*.json` (written by `tools/refdump`).

use super::string::Str;
use terraria_core::xna::{Color, Point, Rectangle, Vector2};

#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Null,
    Bool(bool),
    Int(i128),
    F32(f32),
    F64(f64),
    Char(u16),
    Str(String),
    Color { r: u8, g: u8, b: u8, a: u8 },
    Vector2 { x: f32, y: f32 },
    Rectangle { x: i32, y: i32, width: i32, height: i32 },
    Point { x: i32, y: i32 },
    /// Elements of a `T[]`.
    Arr(Vec<Value>),
    /// Public fields of a nested object, by C# name.
    Obj(Vec<(&'static str, Value)>),
    /// The generator has no representation for this member (the C# type is named).
    Skip(&'static str),
}

impl Value {
    pub fn str(s: Str) -> Value {
        match s.get() {
            Some(t) => Value::Str(t.to_owned()),
            None => Value::Null,
        }
    }
    pub fn color(c: Color) -> Value {
        Value::Color { r: c.r(), g: c.g(), b: c.b(), a: c.a() }
    }
    pub fn vector2(v: Vector2) -> Value {
        Value::Vector2 { x: v.x, y: v.y }
    }
    pub fn rectangle(r: Rectangle) -> Value {
        Value::Rectangle { x: r.x, y: r.y, width: r.width, height: r.height }
    }
    pub fn point(p: Point) -> Value {
        Value::Point { x: p.x, y: p.y }
    }
}
