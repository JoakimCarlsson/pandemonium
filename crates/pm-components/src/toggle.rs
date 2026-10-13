//! The toggle: a small switch that is on or off.

use pm_input::FocusId;
use pm_style::{
    Role, bg, cursor_pointer, flex, h, items_center, justify_end, justify_start, px, rounded_full,
    size, w,
};
use pm_ui::{Node, node};

/// A toggle being described: whether it is on and what it answers to.
///
/// Build it with [`Toggle::new`] and turn it into a node with [`Toggle::build`].
#[derive(Clone, Copy, Debug)]
pub struct Toggle {
    /// Whether it is on.
    on: bool,
    /// The name it answers to.
    id: FocusId,
}

impl Toggle {
    /// A toggle answering to `id`, on while `on`.
    pub fn new(on: bool, id: FocusId) -> Self {
        Self { on, id }
    }

    /// The node that draws the toggle.
    pub fn build(self) -> Node {
        let track = node().styles([
            flex(),
            items_center(),
            w(9),
            h(5),
            px(0.5),
            rounded_full(),
            cursor_pointer(),
        ]);
        let track = if self.on {
            track.styles([justify_end(), bg(Role::Text)])
        } else {
            track.styles([justify_start(), bg(Role::SurfaceHover)])
        };
        let knob = node().styles([
            size(4),
            rounded_full(),
            bg(if self.on {
                Role::Background
            } else {
                Role::Muted
            }),
        ]);
        track.child(knob).focusable(self.id)
    }
}
