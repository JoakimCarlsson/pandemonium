//! Which mode a buffer is in, and what each one looks like.

/// The mode a buffer is being edited in.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Mode {
    /// Keys are commands: they move the cursor and operate on the text.
    #[default]
    Normal,
    /// Keys type text, the way they do without modal editing.
    Insert,
    /// Keys type over the text instead of in front of it.
    Replace,
    /// A span of characters is selected, and keys move its far end.
    Visual,
    /// Whole lines are selected, and keys move the far end.
    VisualLine,
}

/// How the cursor is drawn in a mode.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Shape {
    /// Whatever shape the reader chose for typing.
    Typing,
    /// A block over the character under the cursor.
    Block,
    /// A line under the character under the cursor.
    Underline,
}

impl Mode {
    /// The mode's name, as the status bar shows it.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Normal => "NORMAL",
            Self::Insert => "INSERT",
            Self::Replace => "REPLACE",
            Self::Visual => "VISUAL",
            Self::VisualLine => "VISUAL LINE",
        }
    }

    /// Whether the mode selects text.
    pub const fn is_visual(self) -> bool {
        matches!(self, Self::Visual | Self::VisualLine)
    }

    /// Whether keys typed in the mode put text in.
    pub const fn is_typing(self) -> bool {
        matches!(self, Self::Insert | Self::Replace)
    }
}
