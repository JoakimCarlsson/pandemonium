//! The fill behind an element's content.

use pm_color::Color;
use pm_primitives::Rule;

/// The direction a gradient runs in.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum GradientDirection {
    /// Towards the top edge.
    Top,
    /// Towards the top-right corner.
    TopRight,
    /// Towards the right edge.
    Right,
    /// Towards the bottom-right corner.
    BottomRight,
    /// Towards the bottom edge.
    Bottom,
    /// Towards the bottom-left corner.
    BottomLeft,
    /// Towards the left edge.
    Left,
    /// Towards the top-left corner.
    TopLeft,
    /// An angle in degrees, clockwise from pointing up.
    Angle(f32),
}

impl GradientDirection {
    /// The direction as degrees clockwise from pointing up, for a box of
    /// `width` by `height` (corners point at the box's corner).
    pub fn degrees(self, width: f32, height: f32) -> f32 {
        let corner = width.atan2(height).to_degrees();
        match self {
            Self::Top => 0.0,
            Self::TopRight => corner,
            Self::Right => 90.0,
            Self::BottomRight => 180.0 - corner,
            Self::Bottom => 180.0,
            Self::BottomLeft => 180.0 + corner,
            Self::Left => 270.0,
            Self::TopLeft => 360.0 - corner,
            Self::Angle(degrees) => degrees,
        }
    }
}

/// A colour ramp across the background.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Gradient {
    /// The direction the ramp runs in.
    pub direction: GradientDirection,
    /// The colour at the start.
    pub from: Color,
    /// An optional colour in the middle.
    pub via: Option<Color>,
    /// The colour at the end.
    pub to: Color,
}

/// What is painted behind the content.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Background {
    /// A flat fill.
    pub color: Option<Color>,
    /// A gradient painted over the flat fill.
    pub gradient: Option<Gradient>,
}

impl Background {
    /// The gradient being declared, starting a bottom-bound one if none is.
    fn gradient_mut(&mut self) -> &mut Gradient {
        self.gradient.get_or_insert(Gradient {
            direction: GradientDirection::Bottom,
            from: Color::Transparent,
            via: None,
            to: Color::Transparent,
        })
    }
}

/// One declaration about the background.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum BackgroundRule {
    /// A flat fill.
    Color(Color),
    /// Removes the gradient.
    NoGradient,
    /// Starts a gradient in the given direction.
    Gradient(GradientDirection),
    /// The gradient's start colour.
    From(Color),
    /// The gradient's middle colour.
    Via(Color),
    /// The gradient's end colour.
    To(Color),
}

impl Rule for BackgroundRule {
    type Target = Background;

    /// Writes the declaration into `target`.
    fn apply(self, target: &mut Background) {
        match self {
            Self::Color(color) => target.color = Some(color),
            Self::NoGradient => target.gradient = None,
            Self::Gradient(direction) => target.gradient_mut().direction = direction,
            Self::From(color) => target.gradient_mut().from = color,
            Self::Via(color) => target.gradient_mut().via = Some(color),
            Self::To(color) => target.gradient_mut().to = color,
        }
    }
}
