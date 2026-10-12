//! How a box takes part in layout, what it does with overflow, and whether it shows.

use pm_primitives::{Axis, Rule};

/// The kind of layout a box takes part in and establishes for its children.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Display {
    /// Stacked along the vertical axis, filling the offered width.
    #[default]
    Block,
    /// Flowing along a line with its siblings.
    Inline,
    /// Flowing along a line, laid out inside like a block.
    InlineBlock,
    /// A flex container.
    Flex,
    /// A flex container that flows along a line.
    InlineFlex,
    /// A grid container.
    Grid,
    /// A grid container that flows along a line.
    InlineGrid,
    /// No box of its own: its children join the parent's layout.
    Contents,
    /// Not laid out and not drawn.
    None,
}

/// What happens to content that does not fit the box on one axis.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Overflow {
    /// Drawn outside the box.
    #[default]
    Visible,
    /// Clipped, and scrollable by code.
    Hidden,
    /// Clipped exactly at the box.
    Clip,
    /// Clipped, with a scrollbar always shown.
    Scroll,
    /// Clipped, with a scrollbar when the content overflows.
    Auto,
}

/// Whether a laid-out box is drawn.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Visibility {
    /// Drawn.
    #[default]
    Visible,
    /// Takes space but is not drawn.
    Hidden,
    /// Hidden inside a collapsing container.
    Collapse,
}

/// The display-related slice of a style.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct DisplayStyle {
    /// The layout kind.
    pub display: Display,
    /// Overflow along the horizontal axis.
    pub overflow_x: Overflow,
    /// Overflow along the vertical axis.
    pub overflow_y: Overflow,
    /// Whether the box is drawn.
    pub visibility: Visibility,
}

/// One declaration about display.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DisplayRule {
    /// The layout kind.
    Display(Display),
    /// Overflow on both axes.
    Overflow(Overflow),
    /// Overflow on one axis.
    OverflowAxis(Axis, Overflow),
    /// Visibility.
    Visibility(Visibility),
}

impl Rule for DisplayRule {
    type Target = DisplayStyle;

    /// Writes the declaration into `target`.
    fn apply(self, target: &mut DisplayStyle) {
        match self {
            Self::Display(display) => target.display = display,
            Self::Overflow(overflow) => {
                target.overflow_x = overflow;
                target.overflow_y = overflow;
            }
            Self::OverflowAxis(Axis::Horizontal, overflow) => target.overflow_x = overflow,
            Self::OverflowAxis(Axis::Vertical, overflow) => target.overflow_y = overflow,
            Self::Visibility(visibility) => target.visibility = visibility,
        }
    }
}
