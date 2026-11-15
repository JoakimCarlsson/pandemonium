//! How the code editor looks and behaves as the editor settings say,
//! provided through a composition local by [`crate::app_theme`] so every
//! editor in every window follows the settings as soon as they change.

use pm_editor::{CursorShape, EditorSettings};
use rustle::CompositionLocal;

/// The editor settings the code editor reads while it draws and takes keys.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CodeLook {
    /// Whether each line has its number beside it.
    pub line_numbers: bool,
    /// Whether the numbers count from the line of the caret.
    pub relative_line_numbers: bool,
    /// Whether the line of the caret is marked.
    pub current_line: bool,
    /// Whether marks beside the numbers show how lines differ from the last commit.
    pub git_gutter: bool,
    /// Whether the uses of the name at the caret are marked.
    pub occurrences: bool,
    /// Whether the language server's hints are written inside the lines.
    pub inlay_hints: bool,
    /// Whether the language server's lenses are shown after the lines.
    pub code_lens: bool,
    /// Whether a map of the file runs down its side.
    pub minimap: bool,
    /// Whether the lines opening the blocks in view stay at the top.
    pub sticky: bool,
    /// Whether upright lines show each level of indentation.
    pub indent_guides: bool,
    /// The column an upright line marks as where lines should end, when one is drawn.
    pub wrap_guide: Option<usize>,
    /// How the caret is drawn while typing.
    pub cursor: CursorShape,
    /// Whether the code font joins characters such as `->` into one glyph.
    pub ligatures: bool,
    /// Whether brackets and quotes typed in vim's insert mode close at once.
    pub auto_close: bool,
    /// Whether new lines typed in vim's insert mode are indented as the language says.
    pub auto_indent: bool,
    /// Whether text vim replays is typed as live typing is.
    pub replay_typing: bool,
    /// Whether vim's syntax objects go by the file's syntax tree.
    pub syntax_objects: bool,
    /// How many lines vim keeps between the caret and the edges of the view.
    pub scroll_margin: usize,
    /// How the cursor of a shell in the terminal is drawn.
    pub terminal_cursor: CursorShape,
}

impl Default for CodeLook {
    /// The editor as it comes.
    fn default() -> Self {
        Self::of(&EditorSettings::default())
    }
}

impl CodeLook {
    /// This look with git's line marks drawn only when `shown` holds as well as the editor's own setting.
    pub fn with_git_gutter(self, shown: bool) -> Self {
        Self {
            git_gutter: self.git_gutter && shown,
            ..self
        }
    }

    /// This look with the terminal's cursor drawn as `shape`.
    pub fn with_terminal_cursor(self, shape: CursorShape) -> Self {
        Self {
            terminal_cursor: shape,
            ..self
        }
    }

    /// The look and behaviour `settings` ask for, with the terminal's cursor a block.
    pub fn of(settings: &EditorSettings) -> Self {
        Self {
            line_numbers: settings.line_numbers,
            relative_line_numbers: settings.relative_line_numbers,
            current_line: settings.current_line_highlight,
            git_gutter: settings.git_gutter,
            occurrences: settings.occurrence_highlights,
            inlay_hints: settings.inlay_hints,
            code_lens: settings.code_lens,
            minimap: settings.minimap,
            sticky: settings.sticky_scroll,
            indent_guides: settings.indent_guides,
            wrap_guide: settings
                .wrap_guide
                .then_some(settings.preferred_line_length),
            cursor: settings.cursor_shape,
            ligatures: settings.ligatures,
            auto_close: settings.auto_close,
            auto_indent: settings.auto_indent,
            replay_typing: settings.vim_replay_typing,
            syntax_objects: settings.vim_syntax_objects,
            scroll_margin: settings.vertical_scroll_margin,
            terminal_cursor: CursorShape::Block,
        }
    }
}

/// The code look in effect, as the editor comes until [`crate::app_theme`] provides another.
pub(crate) static LOCAL_CODE_LOOK: CompositionLocal<CodeLook> =
    CompositionLocal::new(CodeLook::default);

/// The code look in effect at this point of the composition.
pub fn code_look() -> CodeLook {
    LOCAL_CODE_LOOK.current()
}
