//! The check box: a small control that is ticked or not, as a square or as a switch.

use pm_icon::IconName;
use pm_input::FocusId;
use pm_style::{
    Radius, Role, bg, border, border_color, cursor_pointer, flex, h, items_center, justify_center,
    justify_end, justify_start, px, rounded, rounded_full, size, w,
};
use pm_ui::{Node, node};

use crate::parts::glyph;

/// How a check box looks.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Variant {
    /// A small square with a tick.
    #[default]
    Box,
    /// A track with a knob that sits at the end while it is on.
    Switch,
}

/// A check box being described: whether it is ticked, what it answers to and how it looks.
///
/// Build it with [`Checkbox::new`], change it with [`Checkbox::variant`], and
/// turn it into a node with [`Checkbox::build`].
#[derive(Clone, Copy, Debug)]
pub struct Checkbox {
    /// Whether it is ticked.
    checked: bool,
    /// Whether it is dashed, standing for some of what it covers being ticked.
    mixed: bool,
    /// The name it answers to.
    id: FocusId,
    /// How it looks.
    variant: Variant,
}

impl Checkbox {
    /// A check box answering to `id`, ticked while `checked`.
    pub fn new(checked: bool, id: FocusId) -> Self {
        Self {
            checked,
            mixed: false,
            id,
            variant: Variant::default(),
        }
    }

    /// The check box dashed instead of ticked while `mixed` and not `checked`.
    pub fn mixed(mut self, mixed: bool) -> Self {
        self.mixed = mixed;
        self
    }

    /// The check box looking like `variant`.
    pub fn variant(mut self, variant: Variant) -> Self {
        self.variant = variant;
        self
    }

    /// The node that draws the check box.
    pub fn build(self) -> Node {
        match self.variant {
            Variant::Box => self.build_box(),
            Variant::Switch => self.build_switch(),
        }
    }

    /// The node that draws the square.
    fn build_box(self) -> Node {
        let mut styles = vec![
            flex(),
            items_center(),
            justify_center(),
            size(4),
            rounded(Radius::Sm),
            border(),
            cursor_pointer(),
        ];
        let filled = self.checked || self.mixed;
        if filled {
            styles.extend([bg(Role::Accent), border_color(Role::Accent)]);
        } else {
            styles.push(border_color(Role::Subtle));
        }
        let boxed = node().styles(styles);
        let boxed = if self.checked {
            boxed.child(glyph(IconName::Check, 3.0, Role::OnAccent))
        } else if self.mixed {
            boxed.child(glyph(IconName::Minus, 3.0, Role::OnAccent))
        } else {
            boxed
        };
        boxed.focusable(self.id)
    }

    /// The node that draws the switch.
    fn build_switch(self) -> Node {
        let track = node().styles([
            flex(),
            items_center(),
            w(9),
            h(5),
            px(0.5),
            rounded_full(),
            cursor_pointer(),
        ]);
        let track = if self.checked {
            track.styles([justify_end(), bg(Role::Text)])
        } else {
            track.styles([justify_start(), bg(Role::SurfaceHover)])
        };
        let knob = node().styles([
            size(4),
            rounded_full(),
            bg(if self.checked {
                Role::Background
            } else {
                Role::Muted
            }),
        ]);
        track.child(knob).focusable(self.id)
    }
}
