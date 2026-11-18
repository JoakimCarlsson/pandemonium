//! The check box: a small control that is ticked or not, as a square or as a switch.

use rustle::{Alignment, Arrangement, row_arranged};
use rustle::{CursorIcon, Modifier};

use crate::kit::{FocusId, IconName, Interactive, Radius, Role, Styled, icon};

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
/// emit it with [`Checkbox::build`].
#[derive(Clone, Debug)]
pub struct Checkbox {
    /// Whether it is ticked.
    checked: bool,
    /// Whether it is dashed, standing for some of what it covers being ticked.
    mixed: bool,
    /// The name it answers to.
    id: FocusId,
    /// How it looks.
    variant: Variant,
    /// What the caller puts around its box.
    modifier: Modifier,
}

impl Checkbox {
    /// A check box answering to `id`, ticked while `checked`.
    pub fn new(checked: bool, id: FocusId) -> Self {
        Self {
            checked,
            mixed: false,
            id,
            variant: Variant::default(),
            modifier: Modifier::new(),
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

    /// The check box with `modifier` chained outside its own, for what the caller puts around it.
    pub fn modifier(mut self, modifier: Modifier) -> Self {
        self.modifier = self.modifier.chain(modifier);
        self
    }

    /// Emits the check box.
    #[track_caller]
    pub fn build(self) {
        match self.variant {
            Variant::Box => self.build_box(),
            Variant::Switch => self.build_switch(),
        }
    }

    /// Emits the square.
    #[track_caller]
    fn build_box(self) {
        let filled = self.checked || self.mixed;
        let own = Modifier::new().square(4);
        let own = if filled {
            own.fill_rounded(Role::Accent, Radius::Sm)
                .stroke(Role::Accent, Radius::Sm)
        } else {
            own.stroke(Role::Subtle, Radius::Sm)
        };
        let own = own.cursor(CursorIcon::Pointer).target(self.id);
        let (checked, mixed) = (self.checked, self.mixed);
        row_arranged(
            self.modifier.chain(own),
            Arrangement::Center,
            Alignment::Center,
            move || {
                if checked {
                    icon(IconName::Check, 3.0, Role::OnAccent);
                } else if mixed {
                    icon(IconName::Minus, 3.0, Role::OnAccent);
                }
            },
        );
    }

    /// Emits the switch.
    #[track_caller]
    fn build_switch(self) {
        let on = self.checked;
        let own = Modifier::new()
            .w(9)
            .h(5)
            .fill_rounded(
                if on { Role::Text } else { Role::SurfaceHover },
                Radius::Full,
            )
            .cursor(CursorIcon::Pointer)
            .target(self.id)
            .pad_x(0.5);
        let arrangement = if on {
            Arrangement::End
        } else {
            Arrangement::Start
        };
        row_arranged(
            self.modifier.chain(own),
            arrangement,
            Alignment::Center,
            move || {
                rustle::spacer(Modifier::new().square(4).fill_rounded(
                    if on { Role::Background } else { Role::Muted },
                    Radius::Full,
                ));
            },
        );
    }
}
