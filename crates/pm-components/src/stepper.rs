//! The stepper: stops along a line, the current one marked, as a track to press or as a bar that counts.

use crate::events::Interactive;
use crate::events::{Event, send};
use crate::kit;
use crate::target::Target;
use crate::theme;
use crate::theme::{Radius, Role};
use rustle::{Alignment, Arrangement, row_arranged};
use rustle::{CursorIcon, Modifier};

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
    /// The target of the first stop; the rest follow it as [`Target::offset`] counts them.
    first: Target,
    /// How it looks.
    variant: Variant,
    /// What the caller puts around its box.
    modifier: Modifier,
}

impl Stepper {
    /// A stepper of `stops` stops, the first answering to `first` and the rest to the targets after it, as [`Target::offset`] counts them.
    pub fn new(stops: usize, first: Target) -> Self {
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
            .height(theme::sp(6))
            .background_rounded(theme::color(Role::SurfaceHover), Radius::Full.px())
            .padding_xy(theme::sp(1), 0.0);
        row_arranged(
            self.modifier.chain(own),
            Arrangement::SpaceBetween,
            Alignment::Center,
            move || {
                for position in 0..stops {
                    let id = first.offset(position);
                    row_arranged(
                        Modifier::new()
                            .size(theme::sp(4), theme::sp(4))
                            .cursor(CursorIcon::Pointer)
                            .control(id, send(Event::pressing(id))),
                        Arrangement::Center,
                        Alignment::Center,
                        move || {
                            if current == Some(position) {
                                rustle::spacer(
                                    Modifier::new()
                                        .size(theme::sp(4), theme::sp(4))
                                        .background_rounded(
                                            theme::color(Role::Text),
                                            Radius::Full.px(),
                                        ),
                                );
                            } else {
                                rustle::spacer(
                                    Modifier::new()
                                        .size(theme::sp(1), theme::sp(1))
                                        .background_rounded(
                                            theme::color(Role::Subtle),
                                            Radius::Full.px(),
                                        ),
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
        kit::row(self.modifier, 3, move || {
            kit::row(Modifier::new(), 2, move || {
                for candidate in 0..stops {
                    let tone = if candidate <= at {
                        Role::Text
                    } else {
                        Role::SurfaceActive
                    };
                    rustle::spacer(
                        Modifier::new()
                            .width(theme::sp(8))
                            .height(theme::sp(1.5))
                            .background_rounded(theme::color(tone), Radius::Full.px()),
                    );
                }
            });
            line(
                format!("Step {} of {}", at + 1, stops),
                12,
                1.4,
                Role::Subtle,
            );
        });
    }
}
