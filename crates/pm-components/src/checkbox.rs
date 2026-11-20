//! The check box: a small control that is ticked or not, as a square or as a switch,
//! drawn by rustle's check box and switch in the theme's colours. The box itself does
//! not answer the pointer: the control around it, named by its id, does.

use crate::kit;
use crate::target::Target;
use rustle::{CheckboxColors, CheckboxDefaults, Modifier, SwitchColors, ToggleableState};

use crate::theme;
use crate::theme::Role;

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
    id: Target,
    /// How it looks.
    variant: Variant,
}

impl Checkbox {
    /// A check box answering to `id`, ticked while `checked`.
    pub fn new(checked: bool, id: Target) -> Self {
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

    /// Emits the check box.
    #[track_caller]
    pub fn build(self) {
        match self.variant {
            Variant::Box => self.build_box(),
            Variant::Switch => self.build_switch(),
        }
    }

    /// Emits the square, four steps on a side: filled in the accent with its mark while
    /// ticked or dashed, outlined in the subtle colour while clear.
    fn build_box(self) {
        let state = match (self.checked, self.mixed) {
            (true, _) => ToggleableState::On,
            (false, true) => ToggleableState::Indeterminate,
            (false, false) => ToggleableState::Off,
        };
        let side = theme::sp(4);
        let colors = CheckboxColors {
            checked_checkmark_color: theme::color(Role::OnAccent),
            checked_box_color: theme::color(Role::Accent),
            checked_border_color: theme::color(Role::Accent),
            unchecked_border_color: theme::color(Role::Subtle),
            ..CheckboxDefaults::colors()
        };
        rustle::tri_state_checkbox(
            kit::pressed(Modifier::new().size(side, side), self.id),
            state,
            None,
            true,
            colors,
        );
    }

    /// Emits the switch, nine steps by five: a track in the text colour with its knob at
    /// the end on the background while on, a hovered surface with the knob at the start
    /// in the muted colour while off.
    fn build_switch(self) {
        let colors = SwitchColors {
            checked_thumb_color: theme::color(Role::Background),
            checked_track_color: theme::color(Role::Text),
            unchecked_thumb_color: theme::color(Role::Muted),
            unchecked_track_color: theme::color(Role::SurfaceHover),
        };
        rustle::switch_with(
            kit::pressed(Modifier::new().size(theme::sp(9), theme::sp(5)), self.id),
            self.checked,
            None,
            true,
            colors,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::areas::ChatEvent;
    use crate::tagged::lay_out;

    /// The square is four steps on a side and the switch nine by five, each named by its id
    /// and reached by Tab, drawn by rustle's check box and switch.
    #[test]
    fn boxes_and_switches_keep_their_size_and_name() {
        let (square, switch) = (
            Target::Chat(ChatEvent::Choice(901)),
            Target::Chat(ChatEvent::Choice(902)),
        );
        let laid = lay_out(200.0, 100.0, None, move || {
            rustle::column(|| {
                Checkbox::new(false, square).mixed(true).build();
                Checkbox::new(true, switch).variant(Variant::Switch).build();
            });
        });
        let boxed = laid.node(square).expect("square laid out");
        assert_eq!(
            (boxed.bounds.size.width, boxed.bounds.size.height),
            (16.0, 16.0)
        );
        let switched = laid.node(switch).expect("switch laid out");
        assert_eq!(
            (switched.bounds.size.width, switched.bounds.size.height),
            (36.0, 20.0)
        );
        assert_eq!(laid.focus_order(), vec![square, switch]);
    }
}
