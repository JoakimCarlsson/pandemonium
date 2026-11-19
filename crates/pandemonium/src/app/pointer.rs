//! The pointer as the app's own handling of window events follows it: which
//! buttons are down and where it is, folded from the window's events after the
//! interface handled them. The controls get the same events through rustle;
//! this is what the app's remaining drags (a pane's tab, a project header, a
//! row of the files, the conversation's text) and the embedded browser read.

/// A pointer button.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) enum Button {
    /// The primary button.
    Left,
    /// The secondary button.
    Right,
}

/// What the pointer did.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum PointerEvent {
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
pub(crate) struct PointerState {
    /// Where it is in logical pixels, when it is over the window.
    pub(crate) position: Option<(f32, f32)>,
    /// Whether the primary button is down.
    pub(crate) left: bool,
    /// Whether the secondary button is down.
    pub(crate) right: bool,
}

impl PointerState {
    /// Folds one event into the state.
    pub(crate) fn apply(&mut self, event: PointerEvent) {
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
        }
    }
}
