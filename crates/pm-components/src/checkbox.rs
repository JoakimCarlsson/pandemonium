//! The check box: a small square that is ticked or not.

use pm_icon::IconName;
use pm_input::FocusId;
use pm_style::{
    Radius, Role, bg, border, border_color, cursor_pointer, flex, items_center, justify_center,
    rounded, size,
};
use pm_ui::{Node, node};

use crate::parts::glyph;

/// A check box being described: whether it is ticked and what it answers to.
///
/// Build it with [`Checkbox::new`] and turn it into a node with [`Checkbox::build`].
#[derive(Clone, Copy, Debug)]
pub struct Checkbox {
    /// Whether it is ticked.
    checked: bool,
    /// The name it answers to.
    id: FocusId,
}

impl Checkbox {
    /// A check box answering to `id`, ticked while `checked`.
    pub fn new(checked: bool, id: FocusId) -> Self {
        Self { checked, id }
    }

    /// The node that draws the check box.
    pub fn build(self) -> Node {
        let mut styles = vec![
            flex(),
            items_center(),
            justify_center(),
            size(4),
            rounded(Radius::Sm),
            border(),
            cursor_pointer(),
        ];
        if self.checked {
            styles.extend([bg(Role::Accent), border_color(Role::Accent)]);
        } else {
            styles.push(border_color(Role::Subtle));
        }
        let boxed = node().styles(styles);
        let boxed = if self.checked {
            boxed.child(glyph(IconName::Check, 3.0, Role::OnAccent))
        } else {
            boxed
        };
        boxed.focusable(self.id)
    }
}
