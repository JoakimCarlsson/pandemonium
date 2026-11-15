//! What the page is told about the pointer and the keyboard, in terms that need no engine types.

use serde::{Deserialize, Serialize};

/// Which modifiers and mouse buttons are held, as the engine counts them.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
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
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum Button {
    /// The primary button.
    Left,
    /// The wheel button.
    Middle,
    /// The secondary button.
    Right,
}

/// One press or release of a key.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
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
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
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

/// One thing the reader did to a page with the pointer or the keyboard, in logical pixels from the page's corner.
#[derive(Clone, Debug, PartialEq)]
pub enum PageInput {
    /// The pointer moved to `x`, `y`, or out of the page when `left`.
    Moved {
        /// Across, from the left edge.
        x: i32,
        /// Down, from the top edge.
        y: i32,
        /// The modifiers and buttons held.
        modifiers: Modifiers,
        /// Whether the pointer left the page.
        left: bool,
    },
    /// A button went down or up at `x`, `y`.
    Button {
        /// Across, from the left edge.
        x: i32,
        /// Down, from the top edge.
        y: i32,
        /// Which button.
        button: Button,
        /// Whether it went down, not up.
        pressed: bool,
        /// How many quick presses in a row this is.
        clicks: i32,
        /// The modifiers and buttons held.
        modifiers: Modifiers,
    },
    /// The wheel turned by `dx`, `dy` with the pointer at `x`, `y`; positive `dy` scrolls up.
    Scrolled {
        /// Across, from the left edge.
        x: i32,
        /// Down, from the top edge.
        y: i32,
        /// How far across.
        dx: i32,
        /// How far up.
        dy: i32,
        /// The modifiers held.
        modifiers: Modifiers,
    },
    /// A key went down or up.
    Key(Key),
    /// An input method finished typing this text.
    Text(String),
}
