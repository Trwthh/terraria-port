//! `Terraria.Graphics.VertexColors` (VertexColors.cs).

use terraria_core::xna::Color;

/// The four corner colours of a tile quad. `default(VertexColors)` is four transparent colours.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct VertexColors {
    pub top_left_color: Color,
    pub top_right_color: Color,
    pub bottom_left_color: Color,
    pub bottom_right_color: Color,
}

impl VertexColors {
    /// `VertexColors(Color color)`: all four corners the same.
    pub fn new(color: Color) -> VertexColors {
        VertexColors { top_left_color: color, top_right_color: color, bottom_right_color: color, bottom_left_color: color }
    }

    /// `VertexColors(Color topLeft, Color topRight, Color bottomRight, Color bottomLeft)` (note the argument order).
    pub fn from_corners(top_left: Color, top_right: Color, bottom_right: Color, bottom_left: Color) -> VertexColors {
        VertexColors {
            top_left_color: top_left,
            top_right_color: top_right,
            bottom_left_color: bottom_left,
            bottom_right_color: bottom_right,
        }
    }
}

/// `implicit operator VertexColors(Color color)`
impl From<Color> for VertexColors {
    fn from(color: Color) -> VertexColors {
        VertexColors::new(color)
    }
}
