//! How much a press takes at a time, and how a drag grows what it took.

use std::time::{Duration, Instant};

/// How much of a text a press takes: one click places a point, two take a
/// word and three or more take a line, or the paragraph where text is laid
/// out in blocks.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Reach {
    /// A point between two characters.
    #[default]
    Point,
    /// A word, or a run of space or of other characters.
    Word,
    /// A line, or the rows that run on without a blank line between them.
    Line,
}

impl Reach {
    /// What the `clicks`th press in a row takes.
    pub fn of_clicks(clicks: u8) -> Self {
        match clicks {
            0 | 1 => Self::Point,
            2 => Self::Word,
            _ => Self::Line,
        }
    }
}

/// The last press, to tell whether the next one lands on the same target soon enough to count with it.
#[derive(Clone, Copy, Debug)]
pub struct Clicks<K> {
    /// When and where the last press was and how many in a row it made.
    last: Option<(Instant, K, u8)>,
}

impl<K> Default for Clicks<K> {
    /// No press yet.
    fn default() -> Self {
        Self { last: None }
    }
}

impl<K: PartialEq> Clicks<K> {
    /// Notes a press on `target` and says how many in a row it makes: one more than the last when that was on `target` less than `within` ago, one otherwise.
    pub fn press(&mut self, target: K, within: Duration) -> u8 {
        let clicks = match &self.last {
            Some((at, last, clicks)) if *last == target && at.elapsed() < within => {
                clicks.saturating_add(1)
            }
            _ => 1,
        };
        self.last = Some((Instant::now(), target, clicks));
        clicks
    }
}

/// Somewhere text can be selected with the pointer, whatever its positions are made of.
pub trait Surface {
    /// A place in the text: ordered, so that the earlier of two can be told.
    type Pos: Ord + Copy;

    /// The first and last position of the word or line `reach` names around `at`.
    fn reach_around(&self, reach: Reach, at: Self::Pos) -> (Self::Pos, Self::Pos);

    /// Drops the selection and puts the caret, or nothing where there is none, at `at`.
    fn place(&mut self, at: Self::Pos);

    /// Selects from `anchor`, which stays, to `head`, which moves.
    fn select(&mut self, anchor: Self::Pos, head: Self::Pos);
}

/// A press that is held down on a [`Surface`]: what it took, which dragging grows by the same amount at a time.
#[derive(Clone, Copy, Debug)]
pub struct Gesture<P> {
    /// How much each step takes, and the stretch the press took.
    held: Option<(Reach, (P, P))>,
}

impl<P> Default for Gesture<P> {
    /// No press held.
    fn default() -> Self {
        Self { held: None }
    }
}

impl<P: Ord + Copy> Gesture<P> {
    /// Takes the press at `at` as `reach`, selecting what that takes around it.
    pub fn press<S: Surface<Pos = P>>(&mut self, surface: &mut S, at: P, reach: Reach) {
        let span = match reach {
            Reach::Point => {
                surface.place(at);
                (at, at)
            }
            _ => {
                let span = surface.reach_around(reach, at);
                surface.select(span.0, span.1);
                span
            }
        };
        self.held = Some((reach, span));
    }

    /// Takes the press at `at` as one that runs the selection held at `from` out to it, as a press with Shift held does.
    pub fn press_from<S: Surface<Pos = P>>(&mut self, surface: &mut S, from: P, at: P) {
        surface.select(from, at);
        self.held = Some((Reach::Point, (from, from)));
    }

    /// Extends the held press to the position `at`, by whole words or lines when it took them.
    pub fn drag<S: Surface<Pos = P>>(&self, surface: &mut S, at: P) {
        let Some((reach, held)) = self.held else {
            return;
        };
        let around = match reach {
            Reach::Point => (at, at),
            _ => surface.reach_around(reach, at),
        };
        let (anchor, head) = match around.0 < held.0 {
            true => (held.1, around.0),
            false => (held.0, around.1.max(held.1)),
        };
        match (reach, anchor == head) {
            (Reach::Point, true) => surface.place(anchor),
            _ => surface.select(anchor, head),
        }
    }

    /// Lets go of the press, and says whether one was held.
    pub fn release(&mut self) -> bool {
        self.held.take().is_some()
    }

    /// Whether a press is held.
    pub fn is_held(&self) -> bool {
        self.held.is_some()
    }
}
