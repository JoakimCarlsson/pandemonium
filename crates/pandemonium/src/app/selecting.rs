//! What a press held on text the pointer selects took, for the code editor's text and the
//! shell's window: a press takes a point, a word or a line around where it went down, as
//! the number of presses in a row rustle counted says, and dragging stretches the selection
//! by the same amount at a time. Text fields, the conversation and the card about a place
//! select with rustle's own gestures.

use pm_buffer::Reach;

/// What a press or a drag selects.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Selected<P> {
    /// Nothing, the caret going to this place.
    Point(P),
    /// From the first place, which stays, to the second, which moves.
    Span(P, P),
}

/// What a press held down took: how much each step of a drag takes, and the stretch the
/// press took.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct Hold<P> {
    /// How much each step takes.
    reach: Reach,
    /// The stretch the press took.
    span: (P, P),
}

impl<P: Ord + Copy> Hold<P> {
    /// The press at `at` that is the `clicks`-th in a row, `around` giving the first and last
    /// place of the word or line a reach takes around a place: what it holds and selects.
    pub(super) fn press(
        at: P,
        clicks: u32,
        around: impl FnOnce(Reach, P) -> (P, P),
    ) -> (Self, Selected<P>) {
        let reach = Reach::of_clicks(u8::try_from(clicks).unwrap_or(u8::MAX));
        if reach == Reach::Point {
            return (
                Self {
                    reach,
                    span: (at, at),
                },
                Selected::Point(at),
            );
        }
        let span = around(reach, at);
        (Self { reach, span }, Selected::Span(span.0, span.1))
    }

    /// The press at `at` that runs the selection held at `from` out to it, as a press with
    /// Shift held does: what it holds and selects.
    pub(super) fn extending(from: P, at: P) -> (Self, Selected<P>) {
        (
            Self {
                reach: Reach::Point,
                span: (from, from),
            },
            Selected::Span(from, at),
        )
    }

    /// What dragging the held press to `at` selects, by whole words or lines when it took
    /// them, `around` giving the stretch a reach takes around a place.
    pub(super) fn drag(&self, at: P, around: impl FnOnce(Reach, P) -> (P, P)) -> Selected<P> {
        let near = match self.reach {
            Reach::Point => (at, at),
            reach => around(reach, at),
        };
        let (anchor, head) = match near.0 < self.span.0 {
            true => (self.span.1, near.0),
            false => (self.span.0, near.1.max(self.span.1)),
        };
        match (self.reach, anchor == head) {
            (Reach::Point, true) => Selected::Point(anchor),
            _ => Selected::Span(anchor, head),
        }
    }
}
