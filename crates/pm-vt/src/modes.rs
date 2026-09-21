//! The modes a program turns on and off, and what the terminal does with them.

/// Every mode the emulator honours, as the current program has set them.
#[derive(Clone, Copy, Debug)]
pub struct Modes {
    /// DECCKM: cursor keys send `SS3` sequences rather than `CSI` ones.
    pub application_cursor: bool,
    /// DECKPAM: the keypad sends application sequences.
    pub application_keypad: bool,
    /// DECAWM: writing past the last column wraps to the next line.
    pub wrap: bool,
    /// IRM: a written character pushes the rest of the line right.
    pub insert: bool,
    /// DECTCEM: the cursor is drawn.
    pub cursor_visible: bool,
    /// Bracketed paste: pasted text is fenced in `ESC[200~` and `ESC[201~`.
    pub bracketed_paste: bool,
    /// Focus reporting: the terminal tells the program when the pane is focused.
    pub focus_reporting: bool,
}

impl Modes {
    /// The modes a terminal starts in, and returns to when it is reset.
    pub const DEFAULT: Self = Self {
        application_cursor: false,
        application_keypad: false,
        wrap: true,
        insert: false,
        cursor_visible: true,
        bracketed_paste: false,
        focus_reporting: false,
    };
}

impl Default for Modes {
    /// The modes a terminal starts in.
    fn default() -> Self {
        Self::DEFAULT
    }
}
