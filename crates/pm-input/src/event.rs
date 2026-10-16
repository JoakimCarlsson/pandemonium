//! The events the window hands the app.

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

/// A key, named for what it means rather than where it sits.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Key {
    /// A key that types this character.
    Character(char),
    /// Enter or Return.
    Enter,
    /// Escape.
    Escape,
    /// Tab.
    Tab,
    /// Backspace.
    Backspace,
    /// Delete, the key that removes the character after the caret.
    Delete,
    /// Home.
    Home,
    /// End.
    End,
    /// Page Up.
    PageUp,
    /// Page Down.
    PageDown,
    /// The space bar.
    Space,
    /// The up arrow.
    ArrowUp,
    /// The down arrow.
    ArrowDown,
    /// The left arrow.
    ArrowLeft,
    /// The right arrow.
    ArrowRight,
    /// A function key, by its number: 12 for F12.
    Function(u8),
}

/// Which modifier keys are held.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Modifiers {
    /// Shift.
    pub shift: bool,
    /// Control.
    pub control: bool,
    /// Alt or Option.
    pub alt: bool,
    /// The platform key: Command, Windows or Super.
    pub logo: bool,
}

/// A key going down or coming up.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct KeyEvent {
    /// Which key.
    pub key: Key,
    /// Which modifiers were held.
    pub modifiers: Modifiers,
    /// Whether it went down rather than came up.
    pub pressed: bool,
}
