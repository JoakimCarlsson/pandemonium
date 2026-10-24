//! Padding, margin and gap.

use pm_layout::{Margin, SpacingRule};
use pm_primitives::{Number, Sides};

use crate::Utility;

/// Padding on `sides`, in raw logical pixels.
pub fn padding(sides: Sides, pixels: impl Number) -> Utility {
    SpacingRule::Padding(sides, pixels.value()).into()
}

/// Margin on `sides`.
pub fn margin(sides: Sides, margin: Margin) -> Utility {
    SpacingRule::Margin(sides, margin).into()
}

/// Padding on every side, in steps.
pub fn p(steps: impl Number) -> Utility {
    padding(Sides::All, steps.step_pixels())
}

/// Padding left and right, in steps.
pub fn px(steps: impl Number) -> Utility {
    padding(Sides::Horizontal, steps.step_pixels())
}

/// Padding top and bottom, in steps.
pub fn py(steps: impl Number) -> Utility {
    padding(Sides::Vertical, steps.step_pixels())
}

/// Padding on the top side, in steps.
pub fn pt(steps: impl Number) -> Utility {
    padding(Sides::Top, steps.step_pixels())
}

/// Padding on the right side, in steps.
pub fn pr(steps: impl Number) -> Utility {
    padding(Sides::Right, steps.step_pixels())
}

/// Padding on the bottom side, in steps.
pub fn pb(steps: impl Number) -> Utility {
    padding(Sides::Bottom, steps.step_pixels())
}

/// Padding on the left side, in steps.
pub fn pl(steps: impl Number) -> Utility {
    padding(Sides::Left, steps.step_pixels())
}

/// Margin on every side, in steps; negative pulls outward.
pub fn m(steps: impl Number) -> Utility {
    margin(Sides::All, Margin::Px(steps.step_pixels()))
}

/// Margin left and right, in steps.
pub fn mx(steps: impl Number) -> Utility {
    margin(Sides::Horizontal, Margin::Px(steps.step_pixels()))
}

/// Margin top and bottom, in steps.
pub fn my(steps: impl Number) -> Utility {
    margin(Sides::Vertical, Margin::Px(steps.step_pixels()))
}

/// Margin on the top side, in steps.
pub fn mt(steps: impl Number) -> Utility {
    margin(Sides::Top, Margin::Px(steps.step_pixels()))
}

/// Margin on the right side, in steps.
pub fn mr(steps: impl Number) -> Utility {
    margin(Sides::Right, Margin::Px(steps.step_pixels()))
}

/// Margin on the bottom side, in steps.
pub fn mb(steps: impl Number) -> Utility {
    margin(Sides::Bottom, Margin::Px(steps.step_pixels()))
}

/// Margin on the left side, in steps.
pub fn ml(steps: impl Number) -> Utility {
    margin(Sides::Left, Margin::Px(steps.step_pixels()))
}

/// The gap between rows and columns, in steps.
pub fn gap(steps: impl Number) -> Utility {
    SpacingRule::Gap(steps.step_pixels()).into()
}

/// The gap between columns, in steps.
pub fn gap_x(steps: impl Number) -> Utility {
    SpacingRule::ColumnGap(steps.step_pixels()).into()
}

/// The gap between rows, in steps.
pub fn gap_y(steps: impl Number) -> Utility {
    SpacingRule::RowGap(steps.step_pixels()).into()
}

constants! {
    /// Equal automatic margin on every side.
    m_auto = SpacingRule::Margin(Sides::All, Margin::Auto);
    /// Automatic margin left and right: centres a sized box.
    mx_auto = SpacingRule::Margin(Sides::Horizontal, Margin::Auto);
    /// Automatic margin top and bottom.
    my_auto = SpacingRule::Margin(Sides::Vertical, Margin::Auto);
    /// Automatic margin on the top side.
    mt_auto = SpacingRule::Margin(Sides::Top, Margin::Auto);
    /// Automatic margin on the right side: pushes the box left.
    mr_auto = SpacingRule::Margin(Sides::Right, Margin::Auto);
    /// Automatic margin on the bottom side.
    mb_auto = SpacingRule::Margin(Sides::Bottom, Margin::Auto);
    /// Automatic margin on the left side: pushes the box right.
    ml_auto = SpacingRule::Margin(Sides::Left, Margin::Auto);
}
