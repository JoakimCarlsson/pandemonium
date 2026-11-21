//! The sizes of the panels around the main area, held as observable state: dragging the
//! handle of one changes only its size, so only the composables that lay it out follow,
//! and the workspace's own state is not written while the handle moves.

use crate::Held;

/// How big the panels around the main area are, each held as observable state.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PanelSizes {
    /// How wide the sidebar is, in logical pixels.
    pub sidebar_width: Held<f32>,
    /// How wide the review panel is, in logical pixels.
    pub review_width: Held<f32>,
    /// How wide the plan panel was dragged to be, in logical pixels, or zero while it takes the width it would by itself.
    pub plan_width: Held<f32>,
    /// How tall the terminal dock is while it is not maximized, in logical pixels.
    pub terminal_height: Held<f32>,
    /// Whether the terminal is taller than usual.
    pub terminal_maximized: Held<bool>,
}

impl PanelSizes {
    /// The sizes a new window opens with.
    pub fn opening() -> Self {
        Self {
            sidebar_width: Held::new(crate::sidebar::WIDTH),
            review_width: Held::new(crate::review::WIDTH),
            plan_width: Held::default(),
            terminal_height: Held::new(crate::terminal::HEIGHT),
            terminal_maximized: Held::default(),
        }
    }
}
