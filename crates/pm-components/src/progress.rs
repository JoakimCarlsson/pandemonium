//! The progress bar: one segment for each step, filled as far as the reader has got.

use pm_style::{Role, bg, flex, gap, h, items_center, rounded_full, w};
use pm_ui::{Node, node};

use crate::parts::line;

/// A progress bar being described: how many steps there are and how far through them the reader is.
///
/// Build it with [`Progress::new`] and turn it into a node with [`Progress::build`].
#[derive(Clone, Copy, Debug)]
pub struct Progress {
    /// How many steps there are.
    steps: usize,
    /// The step the reader is on, counted from zero.
    at: usize,
}

impl Progress {
    /// A bar of `steps` steps with the reader on the step at `at`, counted from zero.
    pub fn new(steps: usize, at: usize) -> Self {
        Self { steps, at }
    }

    /// The node that draws the bar and says which step it is.
    pub fn build(self) -> Node {
        node()
            .styles([flex(), items_center(), gap(3)])
            .child(
                node()
                    .styles([flex(), items_center(), gap(2)])
                    .children((0..self.steps).map(|candidate| {
                        let tone = if candidate <= self.at {
                            Role::Text
                        } else {
                            Role::SurfaceActive
                        };
                        node().styles([w(8), h(1.5), rounded_full(), bg(tone)])
                    })),
            )
            .child(line(
                format!("Step {} of {}", self.at + 1, self.steps),
                12,
                1.4,
                Role::Subtle,
            ))
    }
}
