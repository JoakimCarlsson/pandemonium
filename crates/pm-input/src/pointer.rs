//! Where the pointer is and which buttons are down.

use crate::{Button, PointerEvent};

/// The pointer as the events so far left it.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PointerState {
    /// Where it is in logical pixels, when it is over the window.
    pub position: Option<(f32, f32)>,
    /// Whether the primary button is down.
    pub left: bool,
    /// Whether the secondary button is down.
    pub right: bool,
    /// Whether the middle button is down.
    pub middle: bool,
}

impl PointerState {
    /// Folds one event into the state.
    pub fn apply(&mut self, event: PointerEvent) {
        match event {
            PointerEvent::Moved(x, y) => self.position = Some((x, y)),
            PointerEvent::Left => self.position = None,
            PointerEvent::Pressed(button) => self.set(button, true),
            PointerEvent::Released(button) => self.set(button, false),
        }
    }

    /// Records whether `button` is down.
    fn set(&mut self, button: Button, down: bool) {
        match button {
            Button::Left => self.left = down,
            Button::Right => self.right = down,
            Button::Middle => self.middle = down,
        }
    }
}
