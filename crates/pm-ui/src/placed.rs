//! Where the characters of a run of text came out when it was painted.
//!
//! A screen that lets the reader pick text out with the pointer has to turn
//! a point back into a character, and only the paint pass knows where each
//! character landed. A text element given [`Placements`] writes down its
//! bounds and the caret before each of its characters as it paints, under a
//! key the caller chose, and the caller reads the point back from them.

use std::cell::RefCell;
use std::rc::Rc;

use pm_gfx::{Point, Rect};

/// Every run painted with a key this frame, in paint order.
pub type Placements = Rc<RefCell<Vec<Placed>>>;

/// One run of text as it was painted.
#[derive(Clone, Debug)]
pub struct Placed {
    /// What the caller called the run when it asked for it to be placed.
    pub key: usize,
    /// The line box the run was painted in.
    pub bounds: Rect,
    /// Where a caret sits before each character, and after the last one, in
    /// window coordinates.
    pub carets: Vec<f32>,
}

impl Placed {
    /// How far `point` is from this run: how far above or below its line
    /// box first, then how far to either side of it.
    pub fn distance(&self, point: Point) -> (f32, f32) {
        let bounds = self.bounds;
        let apart = |at: f32, low: f32, high: f32| (low - at).max(at - high).max(0.0);
        (
            apart(point.y, bounds.top(), bounds.bottom()),
            apart(point.x, bounds.left(), bounds.right()),
        )
    }

    /// The character boundary nearest `x`, counted in characters.
    pub fn caret_at(&self, x: f32) -> usize {
        self.carets
            .iter()
            .enumerate()
            .min_by(|(_, one), (_, other)| (*one - x).abs().total_cmp(&(*other - x).abs()))
            .map_or(0, |(at, _)| at)
    }
}

/// The run painted nearest `point` among `placements`, rows before columns.
pub fn nearest(placements: &[Placed], point: Point) -> Option<&Placed> {
    placements.iter().min_by(|one, other| {
        let (one, other) = (one.distance(point), other.distance(point));
        one.0.total_cmp(&other.0).then(one.1.total_cmp(&other.1))
    })
}
