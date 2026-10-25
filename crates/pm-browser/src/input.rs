//! What the page is told about the pointer and the keyboard, in terms that need no engine types.

/// Which modifiers and mouse buttons are held, as the engine counts them.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Modifiers(pub u32);

impl Modifiers {
    /// Shift is held.
    pub const SHIFT: u32 = 2;
    /// Control is held.
    pub const CONTROL: u32 = 4;
    /// Alt is held.
    pub const ALT: u32 = 8;
    /// The left mouse button is held.
    pub const LEFT_BUTTON: u32 = 16;
    /// The middle mouse button is held.
    pub const MIDDLE_BUTTON: u32 = 32;
    /// The right mouse button is held.
    pub const RIGHT_BUTTON: u32 = 64;
    /// The command or super key is held.
    pub const COMMAND: u32 = 128;
}

/// A mouse button.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Button {
    /// The primary button.
    Left,
    /// The wheel button.
    Middle,
    /// The secondary button.
    Right,
}

/// One press or release of a key.
#[derive(Clone, Debug)]
pub struct Key {
    /// Whether the key went down, not up.
    pub pressed: bool,
    /// The key as a Windows virtual-key code, which is how the engine names keys.
    pub code: i32,
    /// The text the key typed, if it typed any.
    pub text: Option<String>,
    /// The modifiers held.
    pub modifiers: Modifiers,
}

/// How the pointer looks over the page.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Cursor {
    /// The ordinary arrow.
    #[default]
    Arrow,
    /// The hand over a link.
    Hand,
    /// The caret over text.
    Text,
    /// The crosshair.
    Cross,
    /// Busy.
    Wait,
    /// Help is available.
    Help,
    /// A resize handle running east and west.
    ResizeHorizontal,
    /// A resize handle running north and south.
    ResizeVertical,
}
