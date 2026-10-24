//! How the shells of the terminal dock are drawn and what selecting in them does.

use serde::{Deserialize, Serialize};

/// How the cursor of a shell is drawn.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TerminalCursor {
    /// The whole cell filled.
    #[default]
    Block,
    /// An upright bar before the cell.
    Bar,
    /// A line under the cell.
    Underline,
}

/// How many lines a shell keeps above its window as it comes.
pub const SCROLLBACK: usize = 5_000;

/// How the shells are drawn and what selecting in them does.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct TerminalSettings {
    /// How big the text of a shell is, in logical pixels, or the code font's size when nothing.
    pub font_size: Option<u32>,
    /// How the cursor is drawn.
    pub cursor: TerminalCursor,
    /// How many lines are kept above the window to scroll back to.
    pub scrollback: usize,
    /// Whether text is copied as soon as it is selected.
    pub copy_on_select: bool,
}

impl Default for TerminalSettings {
    /// The shells as they come.
    fn default() -> Self {
        Self {
            font_size: None,
            cursor: TerminalCursor::Block,
            scrollback: SCROLLBACK,
            copy_on_select: false,
        }
    }
}
