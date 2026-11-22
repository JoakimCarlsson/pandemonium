//! The stepper: stops along a track, the current one marked, each answering to a press.

use crate::events;
use crate::target::Target;
use crate::theme;
use crate::theme::{Radius, Role};
use rustle::Modifier;
use rustle::{Alignment, Arrangement, row_arranged};

/// A stepper being described: how many stops it has, which is current, and what its stops answer to.
///
/// Build it with [`Stepper::new`], change it with [`Stepper::current`], and
/// emit it with [`Stepper::build`].
#[derive(Clone, Debug)]
pub struct Stepper {
    /// How many stops the track has.
    stops: usize,
    /// The stop that is current, if one is.
    current: Option<usize>,
    /// The target of the first stop; the rest follow it as [`Target::offset`] counts them.
    first: Target,
}

impl Stepper {
    /// A stepper of `stops` stops, the first answering to `first` and the rest to the targets after it, as [`Target::offset`] counts them.
    pub fn new(stops: usize, first: Target) -> Self {
        Self {
            stops,
            current: None,
            first,
        }
    }

    /// The stepper with the stop at `current` marked, counted from zero.
    pub fn current(mut self, current: Option<usize>) -> Self {
        self.current = current;
        self
    }

    /// Emits the stepper: a track with a stop for each level, each a target for a press.
    #[track_caller]
    pub fn build(self) {
        let (stops, current, first) = (self.stops, self.current, self.first);
        let own = Modifier::new()
            .width(112.0)
            .height(theme::sp(6))
            .background_rounded(theme::color(Role::SurfaceHover), Radius::Full.px())
            .padding_xy(theme::sp(1), 0.0);
        row_arranged(
            own,
            Arrangement::SpaceBetween,
            Alignment::Center,
            move || {
                for position in 0..stops {
                    let id = first.offset(position);
                    row_arranged(
                        events::pressed(Modifier::new().size(theme::sp(4), theme::sp(4)), id),
                        Arrangement::Center,
                        Alignment::Center,
                        move || {
                            let (side, tone) = if current == Some(position) {
                                (4, Role::Text)
                            } else {
                                (1, Role::Subtle)
                            };
                            rustle::spacer(
                                Modifier::new()
                                    .size(theme::sp(side), theme::sp(side))
                                    .background_rounded(theme::color(tone), Radius::Full.px()),
                            );
                        },
                    );
                }
            },
        );
    }
}
