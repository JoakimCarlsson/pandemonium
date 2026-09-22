//! A box that is ticked, empty, or neither.

use crate::div::{Div, v_flex};
use crate::icons::{IconName, IconSize, icon};
use crate::style::Styled;
use crate::theme::Theme;

/// The side of the square the box is drawn at.
const SIDE: f32 = 15.0;

/// What a box that can be ticked is saying.
///
/// The third state is what a box says about several things at once when they
/// do not agree: some of this file is staged and some of it is not, and a box
/// that showed either of the other two states would be wrong about half of it.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ToggleState {
    /// None of it.
    #[default]
    Off,
    /// Some of it, but not all.
    Mixed,
    /// All of it.
    On,
}

impl ToggleState {
    /// The state of a whole made of `all` parts, `some` of which are on.
    pub fn of(some: usize, all: usize) -> Self {
        match some {
            0 => Self::Off,
            some if some == all => Self::On,
            _ => Self::Mixed,
        }
    }

    /// Whether the box is filled at all.
    fn is_filled(self) -> bool {
        self != Self::Off
    }

    /// The mark drawn inside it, when it has one.
    fn mark(self) -> Option<IconName> {
        match self {
            Self::Off => None,
            Self::Mixed => Some(IconName::Minus),
            Self::On => Some(IconName::Check),
        }
    }
}

/// A box in `state` that sends `message` when it is clicked.
pub fn checkbox<M>(theme: &Theme, state: ToggleState, message: M) -> Div<M> {
    v_flex()
        .size_px(SIDE)
        .items_center()
        .justify_center()
        .rounded(theme.radius.sm)
        .border_1(match state.is_filled() {
            true => theme.colors.accent,
            false => theme.colors.border_selected,
        })
        .when(state.is_filled(), |box_| box_.bg(theme.colors.accent))
        .hover_bg(match state.is_filled() {
            true => theme.colors.accent_hover,
            false => theme.colors.surface_hover,
        })
        .on_click(message)
        .when_some(state.mark(), |box_, mark| {
            box_.child(
                icon(mark)
                    .size(IconSize::XSmall)
                    .color(theme.colors.text_on_accent),
            )
        })
}
