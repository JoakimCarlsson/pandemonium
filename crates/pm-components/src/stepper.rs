//! The stepper: stops along a line, the current one marked, as a track to press or as a bar that counts.

use crate::kit::sp;
use crate::kit::{FocusId, Interactive, Radius, Role, Styled};
use rustle::{CursorIcon, Modifier};
use rustle::{Flex, flex_row};

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
/// [`Stepper::variant`], and emit it with [`Stepper::build`].
#[derive(Clone, Debug)]
pub struct Stepper {
    /// How many stops the track has.
    stops: usize,
    /// The stop that is current, if one is.
    current: Option<usize>,
    /// The name of the first stop; the rest follow it.
    first: u64,
    /// How it looks.
    variant: Variant,
    /// What the caller puts around its box.
    modifier: Modifier,
}

impl Stepper {
    /// A stepper of `stops` stops, the first answering to `first` and the rest to the names after it.
    pub fn new(stops: usize, first: u64) -> Self {
        Self {
            stops,
            current: None,
            first,
            variant: Variant::default(),
            modifier: Modifier::new(),
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

    /// The stepper with `modifier` put around its box, outside its own: sizes, weight, placement.
    pub fn modifier(mut self, modifier: Modifier) -> Self {
        self.modifier = self.modifier.chain(modifier);
        self
    }

    /// Emits the stepper.
    #[track_caller]
    pub fn build(self) {
        match self.variant {
            Variant::Track => self.track(),
            Variant::Bar => self.bar(),
        }
    }

    /// Emits the track: a stop for each level, each a target for a press.
    #[track_caller]
    fn track(self) {
        let (stops, current, first) = (self.stops, self.current, self.first);
        let own = Modifier::new()
            .width(112.0)
            .h(6)
            .fill_rounded(Role::SurfaceHover, Radius::Full)
            .pad_x(1);
        flex_row(
            self.modifier.chain(own),
            Flex::new().items_center().justify_between(),
            move || {
                for position in 0..stops {
                    let id = FocusId(first + position as u64);
                    flex_row(
                        Modifier::new()
                            .square(4)
                            .cursor(CursorIcon::Pointer)
                            .target(id),
                        Flex::new().items_center().justify_center(),
                        move || {
                            if current == Some(position) {
                                rustle::spacer(
                                    Modifier::new()
                                        .square(4)
                                        .fill_rounded(Role::Text, Radius::Full),
                                );
                            } else {
                                rustle::spacer(
                                    Modifier::new()
                                        .square(1)
                                        .fill_rounded(Role::Subtle, Radius::Full),
                                );
                            }
                        },
                    );
                }
            },
        );
    }

    /// Emits the bar: a segment for each step up to the current one filled, and the step in words.
    #[track_caller]
    fn bar(self) {
        let at = self.current.unwrap_or(0);
        let stops = self.stops;
        flex_row(
            self.modifier,
            Flex::new().items_center().gap(sp(3)),
            move || {
                flex_row(
                    Modifier::new(),
                    Flex::new().items_center().gap(sp(2)),
                    move || {
                        for candidate in 0..stops {
                            let tone = if candidate <= at {
                                Role::Text
                            } else {
                                Role::SurfaceActive
                            };
                            rustle::spacer(
                                Modifier::new().w(8).h(1.5).fill_rounded(tone, Radius::Full),
                            );
                        }
                    },
                );
                line(
                    format!("Step {} of {}", at + 1, stops),
                    12,
                    1.4,
                    Role::Subtle,
                );
            },
        );
    }
}
