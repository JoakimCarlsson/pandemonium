//! What a pane of text draws around and over the text itself.
//!
//! These are the reader's to decide, so they arrive from the preferences
//! rather than being settled by the view: the view only asks which of them
//! is on.

/// How the caret is drawn.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum CursorShape {
    /// A thin upright line between two characters.
    #[default]
    Bar,
    /// A cell-wide block over the character after the caret.
    Block,
    /// A line under the character after the caret.
    Underline,
}

impl CursorShape {
    /// Every shape, in the order a toggle offers them.
    pub const ALL: [Self; 3] = [Self::Bar, Self::Block, Self::Underline];

    /// The shape to draw in a mode that asks for `shape`, this being the
    /// one the reader chose for typing.
    pub const fn modal(self, shape: pm_vim::Shape) -> Self {
        match shape {
            pm_vim::Shape::Typing => self,
            pm_vim::Shape::Block => Self::Block,
            pm_vim::Shape::Underline => Self::Underline,
        }
    }

    /// The shape's label in the toggle that picks it.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Bar => "Bar",
            Self::Block => "Block",
            Self::Underline => "Underline",
        }
    }
}

/// Which of the things drawn around the text a pane draws.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Display {
    /// Whether the gutter numbers the lines.
    pub line_numbers: bool,
    /// Whether the numbers count away from the cursor's line.
    pub relative_line_numbers: bool,
    /// Whether the cursor's line is washed while nothing is selected.
    pub current_line: bool,
    /// Whether every other place the word at the cursor appears is washed.
    pub occurrences: bool,
    /// Whether a line is drawn at every step of indentation.
    pub indent_guides: bool,
    /// Whether the lines the top of the pane is inside stay pinned above it.
    pub sticky_scroll: bool,
    /// Whether the scrollbars are drawn.
    pub scrollbars: bool,
    /// Whether the whole file is drawn in miniature down the right edge.
    pub minimap: bool,
    /// Whether the path of the file and the blocks the cursor is inside are
    /// named in a bar above the text.
    pub breadcrumbs: bool,
    /// The column a guide is drawn down, if any.
    pub wrap_guide: Option<usize>,
    /// How the caret is drawn.
    pub cursor_shape: CursorShape,
    /// Whether the selection stands for the whole lines it touches, as
    /// modal editing's line selection does.
    pub whole_lines: bool,
}

impl Default for Display {
    /// Everything on but the minimap, and the caret a bar.
    fn default() -> Self {
        Self {
            line_numbers: true,
            relative_line_numbers: false,
            current_line: true,
            occurrences: true,
            indent_guides: true,
            sticky_scroll: true,
            scrollbars: true,
            minimap: false,
            breadcrumbs: true,
            wrap_guide: None,
            cursor_shape: CursorShape::Bar,
            whole_lines: false,
        }
    }
}
