//! The pointer as the host's own logic follows it: which buttons are down
//! and where it is, folded from the window's events. The interface gets the
//! same events through rustle; this is for the host's own drags and for the
//! state logic that asks where the pointer is.

/// A pointer button.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Button {
    /// The primary button.
    Left,
    /// The secondary button.
    Right,
    /// The middle button or wheel press.
    Middle,
}

/// What the pointer did.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PointerEvent {
    /// It moved to a position in logical pixels.
    Moved(f32, f32),
    /// A button went down.
    Pressed(Button),
    /// A button came up.
    Released(Button),
    /// It left the window.
    Left,
}

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
