//! What the views tell the app of the pointer, as typed values: each control
//! handles its own presses, drags, hovers and wheel turns with rustle's
//! modifiers and hands the app what happened through an [`Operations`]
//! method, carrying what the app needs, such as the control, the byte of text
//! or the cell of the shell under the pointer, how many presses in a row it
//! made and the modifier keys held. Positions are logical pixels from the
//! window's top left unless a field says otherwise.
//!
//! [`Operations`]: crate::events::Operations

use crate::panes::Side;
use crate::target::Target;
use crate::tree_actions::TreeTarget;

use rustle::{Modifiers, Rect};

/// A press of the primary or secondary button on a control, or a click of it, as the
/// control tells it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Press {
    /// The control pressed.
    pub id: Target,
    /// Where the control is laid out in the window.
    pub area: Rect,
    /// Where the pointer is in the window.
    pub at: (f32, f32),
    /// How many presses in a row this makes, as rustle counted them.
    pub clicks: u32,
    /// The modifier keys held.
    pub modifiers: Modifiers,
}

/// What the primary button did on text the pointer selects: going down, moving while
/// held, or coming up.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Stroke {
    /// The button went down, the `clicks`-th press in a row, with `modifiers` held.
    Down {
        /// How many presses in a row this makes, as rustle counted them.
        clicks: u32,
        /// The modifier keys held.
        modifiers: Modifiers,
    },
    /// The pointer moved while the button was held, wherever it is.
    Move,
    /// The button came up, wherever the pointer is.
    Up,
}

/// The primary button on the code text of the field `owner`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CodePointer {
    /// The field whose text it is: the code editor's, or a file of the excerpts.
    pub owner: Target,
    /// The byte of the text shown nearest the pointer, when any of it is drawn.
    pub offset: Option<usize>,
    /// Where the pointer is in the window.
    pub at: (f32, f32),
    /// What the button did.
    pub stroke: Stroke,
}

/// The primary button on the window of the shell.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TerminalPointer {
    /// The cell of the window under the pointer, by row and column from its top left.
    pub cell: (usize, usize),
    /// What the button did.
    pub stroke: Stroke,
}

/// What the pointer did over a web page.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum BrowserPointing {
    /// It moved, over the page or while a button pressed on it is held.
    Moved,
    /// It left the page.
    Left,
    /// A button went down or came up.
    Button {
        /// Whether it is the secondary button rather than the primary.
        secondary: bool,
        /// Whether it went down.
        down: bool,
        /// How many presses in a row it makes, as rustle counted them.
        clicks: u32,
    },
    /// The wheel or a trackpad turned this far across and down, the way the content moves.
    Wheel(f32, f32),
    /// The mouse's back button went down, or its forward button when not `true`.
    History(bool),
}

/// The pointer over the window of a web page or of its tools.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BrowserPointer {
    /// The window of the page or of its tools.
    pub surface: Target,
    /// Where the pointer is, from the top left of that window.
    pub at: (f32, f32),
    /// What it did.
    pub pointing: BrowserPointing,
}

/// What a control carried in the window is over, as the place under the pointer that
/// takes it tells.
#[derive(Clone, Debug, PartialEq)]
pub enum Over {
    /// The project at this position of the sidebar: its header, its row for new work or
    /// one of its sessions.
    Project(usize),
    /// The pane at this position, and the edge of it the pointer is near.
    Pane(usize, Side),
    /// A row of the project's files, or their root for the space under them.
    Tree(TreeTarget),
}
