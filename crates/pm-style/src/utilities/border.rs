//! Borders, corner radii and outlines.

use pm_color::Color;
use pm_decoration::{BorderRule, BorderStyle, Radius};
use pm_primitives::{Corner, CornerGroup, Number, Side, Sides};

use crate::Utility;

/// Border thickness on `sides`, in raw logical pixels.
pub fn border_width(sides: Sides, pixels: impl Number) -> Utility {
    BorderRule::Width(sides, pixels.value()).into()
}

/// Border colour on `sides`.
pub fn border_color_on(sides: Sides, color: impl Into<Color>) -> Utility {
    BorderRule::Color(sides, color.into()).into()
}

/// Border colour on every side.
pub fn border_color(color: impl Into<Color>) -> Utility {
    border_color_on(Sides::All, color)
}

/// Corner radius on `group`, in raw logical pixels.
pub fn rounded_on(group: CornerGroup, pixels: impl Number) -> Utility {
    BorderRule::Radius(group, pixels.value()).into()
}

/// Corner radius on every corner, in raw logical pixels or as a named [`Radius`](crate::Radius).
pub fn rounded(radius: impl Number) -> Utility {
    rounded_on(CornerGroup::All, radius)
}

/// Radius on the two top corners.
pub fn rounded_t(radius: impl Number) -> Utility {
    rounded_on(CornerGroup::Side(Side::Top), radius)
}

/// Radius on the two right corners.
pub fn rounded_r(radius: impl Number) -> Utility {
    rounded_on(CornerGroup::Side(Side::Right), radius)
}

/// Radius on the two bottom corners.
pub fn rounded_b(radius: impl Number) -> Utility {
    rounded_on(CornerGroup::Side(Side::Bottom), radius)
}

/// Radius on the two left corners.
pub fn rounded_l(radius: impl Number) -> Utility {
    rounded_on(CornerGroup::Side(Side::Left), radius)
}

/// Radius on the top-left corner.
pub fn rounded_tl(radius: impl Number) -> Utility {
    rounded_on(CornerGroup::One(Corner::TopLeft), radius)
}

/// Radius on the top-right corner.
pub fn rounded_tr(radius: impl Number) -> Utility {
    rounded_on(CornerGroup::One(Corner::TopRight), radius)
}

/// Radius on the bottom-right corner.
pub fn rounded_br(radius: impl Number) -> Utility {
    rounded_on(CornerGroup::One(Corner::BottomRight), radius)
}

/// Radius on the bottom-left corner.
pub fn rounded_bl(radius: impl Number) -> Utility {
    rounded_on(CornerGroup::One(Corner::BottomLeft), radius)
}

/// Outline thickness, in raw logical pixels.
pub fn outline(pixels: impl Number) -> Utility {
    BorderRule::OutlineWidth(pixels.value()).into()
}

/// Gap between border box and outline, in raw logical pixels.
pub fn outline_offset(pixels: impl Number) -> Utility {
    BorderRule::OutlineOffset(pixels.value()).into()
}

/// Outline colour.
pub fn outline_color(color: impl Into<Color>) -> Utility {
    BorderRule::OutlineColor(color.into()).into()
}

constants! {
    /// A one-pixel border on every side.
    border = BorderRule::Width(Sides::All, 1.0);
    /// A one-pixel border on the top side.
    border_t = BorderRule::Width(Sides::Top, 1.0);
    /// A one-pixel border on the right side.
    border_r = BorderRule::Width(Sides::Right, 1.0);
    /// A one-pixel border on the bottom side.
    border_b = BorderRule::Width(Sides::Bottom, 1.0);
    /// A one-pixel border on the left side.
    border_l = BorderRule::Width(Sides::Left, 1.0);
    /// A one-pixel border left and right.
    border_x = BorderRule::Width(Sides::Horizontal, 1.0);
    /// A one-pixel border top and bottom.
    border_y = BorderRule::Width(Sides::Vertical, 1.0);
    /// No border.
    border_0 = BorderRule::Width(Sides::All, 0.0);
    /// A two-pixel border on every side.
    border_2 = BorderRule::Width(Sides::All, 2.0);
    /// A four-pixel border on every side.
    border_4 = BorderRule::Width(Sides::All, 4.0);
    /// An eight-pixel border on every side.
    border_8 = BorderRule::Width(Sides::All, 8.0);
    /// A transparent border.
    border_transparent = BorderRule::Color(Sides::All, Color::Transparent);
    /// A solid border.
    border_solid = BorderRule::Style(BorderStyle::Solid);
    /// A dashed border.
    border_dashed = BorderRule::Style(BorderStyle::Dashed);
    /// A dotted border.
    border_dotted = BorderRule::Style(BorderStyle::Dotted);
    /// A double border.
    border_double = BorderRule::Style(BorderStyle::Double);
    /// A border that is not drawn.
    border_hidden = BorderRule::Style(BorderStyle::Hidden);
    /// Square corners.
    rounded_none = BorderRule::Radius(CornerGroup::All, 0.0);
    /// Fully rounded ends.
    rounded_full = BorderRule::Radius(CornerGroup::All, Radius::FULL);
    /// A two-pixel radius.
    rounded_xs = BorderRule::Radius(CornerGroup::All, 2.0);
    /// A four-pixel radius.
    rounded_sm = BorderRule::Radius(CornerGroup::All, 4.0);
    /// A six-pixel radius.
    rounded_md = BorderRule::Radius(CornerGroup::All, 6.0);
    /// An eight-pixel radius.
    rounded_lg = BorderRule::Radius(CornerGroup::All, 8.0);
    /// A twelve-pixel radius.
    rounded_xl = BorderRule::Radius(CornerGroup::All, 12.0);
    /// A sixteen-pixel radius.
    rounded_2xl = BorderRule::Radius(CornerGroup::All, 16.0);
    /// A twenty-four-pixel radius.
    rounded_3xl = BorderRule::Radius(CornerGroup::All, 24.0);
    /// A thirty-two-pixel radius.
    rounded_4xl = BorderRule::Radius(CornerGroup::All, 32.0);
    /// A solid outline.
    outline_solid = BorderRule::OutlineStyle(BorderStyle::Solid);
    /// A dashed outline.
    outline_dashed = BorderRule::OutlineStyle(BorderStyle::Dashed);
    /// A dotted outline.
    outline_dotted = BorderRule::OutlineStyle(BorderStyle::Dotted);
    /// A double outline.
    outline_double = BorderRule::OutlineStyle(BorderStyle::Double);
}
