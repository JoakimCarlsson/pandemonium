//! The stepper: a track with a stop for each level, the current one marked.

use pm_input::FocusId;
use pm_style::{
    Role, bg, cursor_pointer, flex, h, items_center, justify_between, justify_center, px,
    rounded_full, size, w_px,
};
use pm_ui::{Node, node};

/// A stepper being described: how many stops it has, which is current, and what its stops answer to.
///
/// Build it with [`Stepper::new`] and turn it into a node with [`Stepper::build`].
#[derive(Clone, Copy, Debug)]
pub struct Stepper {
    /// How many stops the track has.
    stops: usize,
    /// The stop that is current, if one is.
    current: Option<usize>,
    /// The name of the first stop; the rest follow it.
    first: u64,
}

impl Stepper {
    /// A stepper of `stops` stops, the first answering to `first` and the rest to the names after it.
    pub fn new(stops: usize, first: u64) -> Self {
        Self {
            stops,
            current: None,
            first,
        }
    }

    /// The stepper with the stop at `current` marked.
    pub fn current(mut self, current: Option<usize>) -> Self {
        self.current = current;
        self
    }

    /// The node that draws the stepper.
    pub fn build(self) -> Node {
        node()
            .styles([
                flex(),
                items_center(),
                justify_between(),
                w_px(112.0),
                h(6),
                px(1),
                rounded_full(),
                bg(Role::SurfaceHover),
            ])
            .children((0..self.stops).map(|position| {
                let dot = if self.current == Some(position) {
                    node().styles([size(4), rounded_full(), bg(Role::Text)])
                } else {
                    node().styles([size(1), rounded_full(), bg(Role::Subtle)])
                };
                node()
                    .styles([
                        flex(),
                        items_center(),
                        justify_center(),
                        size(4),
                        cursor_pointer(),
                    ])
                    .child(dot)
                    .focusable(FocusId(self.first + position as u64))
            }))
    }
}
