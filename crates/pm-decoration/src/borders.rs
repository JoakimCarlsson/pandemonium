//! Borders, corner radii and outlines.

use pm_color::Color;
use pm_primitives::{CornerGroup, Corners, Edges, Number, Rule, Sides};

use crate::Radius;

/// The pattern of a border or outline.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum BorderStyle {
    /// One continuous line.
    #[default]
    Solid,
    /// A row of dashes.
    Dashed,
    /// A row of dots.
    Dotted,
    /// Two parallel lines.
    Double,
    /// Drawn with no width.
    Hidden,
}

/// A border-like line: which pattern a stroke takes.
pub type LineKind = BorderStyle;

/// A line drawn outside the border box that takes no layout space.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Outline {
    /// The thickness in logical pixels.
    pub width: f32,
    /// The gap between the border box and the outline in logical pixels.
    pub offset: f32,
    /// The colour; the text colour when unset.
    pub color: Option<Color>,
    /// The pattern.
    pub style: BorderStyle,
}

/// The border and corner shape of a box.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Borders {
    /// The thickness on each side, drawn inside the box's bounds.
    pub width: Edges<f32>,
    /// The colour on each side; the text colour when unset.
    pub color: Edges<Option<Color>>,
    /// The pattern.
    pub style: BorderStyle,
    /// The radius at each corner, in logical pixels.
    pub radius: Corners<f32>,
    /// The outline.
    pub outline: Outline,
}

/// One declaration about borders.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum BorderRule {
    /// Thickness on the given sides, in logical pixels.
    Width(Sides, f32),
    /// Colour on the given sides.
    Color(Sides, Color),
    /// The pattern.
    Style(BorderStyle),
    /// Radius at the given corners, in logical pixels.
    Radius(CornerGroup, f32),
    /// Outline thickness in logical pixels.
    OutlineWidth(f32),
    /// Outline gap in logical pixels.
    OutlineOffset(f32),
    /// Outline colour.
    OutlineColor(Color),
    /// Outline pattern.
    OutlineStyle(BorderStyle),
}

impl Rule for BorderRule {
    type Target = Borders;

    /// Writes the declaration into `target`.
    fn apply(self, target: &mut Borders) {
        match self {
            Self::Width(sides, pixels) => target.width.set_sides(sides, pixels),
            Self::Color(sides, color) => target.color.set_sides(sides, Some(color)),
            Self::Style(style) => target.style = style,
            Self::Radius(group, pixels) => target.radius.set_group(group, pixels),
            Self::OutlineWidth(pixels) => target.outline.width = pixels,
            Self::OutlineOffset(pixels) => target.outline.offset = pixels,
            Self::OutlineColor(color) => target.outline.color = Some(color),
            Self::OutlineStyle(style) => target.outline.style = style,
        }
    }
}

impl Radius {
    /// A radius larger than any box is tall, which rounds the ends of a bar
    /// into a pill.
    pub const FULL: f32 = 9999.0;
}

impl Number for Radius {
    /// The radius in logical pixels.
    fn value(self) -> f32 {
        self.pixels()
    }
}
