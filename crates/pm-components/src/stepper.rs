//! The stepper: stops along a line, the current one marked, as a track to press or as a bar that counts.

use pm_input::FocusId;
use pm_style::{
    Role, bg, cursor_pointer, flex, gap, h, items_center, justify_between, justify_center, px,
    rounded_full, size, w, w_px,
};
use pm_ui::{Node, node};

use crate::parts::line;

/// How a stepper looks.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Variant {
    /// A track with a stop for each level that answers to a press.
    #[default]
    Track,
    /// A bar with a segment for each step, filled as far as the reader has got, and the step said in words.
    Bar,
}

/// A stepper being described: how many stops it has, which is current, and what its stops answer to.
///
/// Build it with [`Stepper::new`], change it with [`Stepper::current`] and
/// [`Stepper::variant`], and turn it into a node with [`Stepper::build`].
#[derive(Clone, Copy, Debug)]
pub struct Stepper {
    /// How many stops the track has.
    stops: usize,
    /// The stop that is current, if one is.
    current: Option<usize>,
    /// The name of the first stop; the rest follow it.
    first: u64,
    /// How it looks.
    variant: Variant,
}

impl Stepper {
    /// A stepper of `stops` stops, the first answering to `first` and the rest to the names after it.
    pub fn new(stops: usize, first: u64) -> Self {
        Self {
            stops,
            current: None,
            first,
            variant: Variant::default(),
        }
    }

    /// The stepper with the stop at `current` marked, counted from zero.
    pub fn current(mut self, current: Option<usize>) -> Self {
        self.current = current;
        self
    }

    /// The stepper looking like `variant`.
    pub fn variant(mut self, variant: Variant) -> Self {
        self.variant = variant;
        self
    }

    /// The node that draws the stepper.
    pub fn build(self) -> Node {
        match self.variant {
            Variant::Track => self.track(),
            Variant::Bar => self.bar(),
        }
    }

    /// The track: a stop for each level, each a target for a press.
    fn track(self) -> Node {
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

    /// The bar: a segment for each step up to the current one filled, and the step in words.
    fn bar(self) -> Node {
        let at = self.current.unwrap_or(0);
        node()
            .styles([flex(), items_center(), gap(3)])
            .child(
                node()
                    .styles([flex(), items_center(), gap(2)])
                    .children((0..self.stops).map(|candidate| {
                        let tone = if candidate <= at {
                            Role::Text
                        } else {
                            Role::SurfaceActive
                        };
                        node().styles([w(8), h(1.5), rounded_full(), bg(tone)])
                    })),
            )
            .child(line(
                format!("Step {} of {}", at + 1, self.stops),
                12,
                1.4,
                Role::Subtle,
            ))
    }
}
