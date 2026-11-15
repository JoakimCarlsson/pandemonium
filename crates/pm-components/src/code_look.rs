//! How the code editor looks and behaves as the editor settings say,
//! provided through a composition local by [`crate::app_theme`] so every
//! editor in every window follows the settings as soon as they change.

use pm_editor::{
    CursorShape, DiagnosticsShown, EditorSettings, FindInSelection, LightbulbShown, SoftWrap,
};
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
    /// How many columns a level of indentation is, or the language's own when nothing.
    pub tab_size: Option<usize>,
    /// Whether a level of indentation is a tab, or the language's own when nothing.
    pub hard_tabs: Option<bool>,
    /// Whether a bracket or quote typed over a selection goes around it.
    pub auto_surround: bool,
    /// Whether a new line after a line comment starts with the comment too.
    pub extend_comments: bool,
    /// Whether Tab after the name of a snippet writes the snippet out.
    pub snippets_on_tab: bool,
    /// Whether the file is saved a moment after it was last changed.
    pub autosave: bool,
    /// Whether completions open as the reader types.
    pub completions_on_input: bool,
    /// Whether resting the pointer on a name shows what the server says of it.
    pub hover: bool,
    /// How long the pointer rests before that, in milliseconds.
    pub hover_delay: u64,
    /// Whether the signature of the call the caret is in shows as it is typed.
    pub signature_help: bool,
    /// Whether the caret blinks.
    pub cursor_blink: bool,
    /// Where long lines break onto the next row.
    pub soft_wrap: SoftWrap,
    /// How many characters long a line should be at most.
    pub line_length: usize,
    /// Whether the file is formatted as its language says when it is saved.
    pub format_on_save: bool,
    /// Whether what was just typed is formatted as its language says.
    pub format_on_type: bool,
    /// How bad a problem must be to be marked in the text.
    pub diagnostics: DiagnosticsShown,
    /// Whether the caret's line says who last changed it.
    pub inline_blame: bool,
    /// Whether the words that changed inside a changed line of a diff are drawn apart.
    pub word_diff: bool,
    /// Whether stepping past the last match of the find bar goes round to the first.
    pub search_wrap: bool,
    /// Whether the find bar opens on the selected text.
    pub seed_search: bool,
    /// Whether a query with an upper-case letter in it matches case even with the case toggle off.
    pub smartcase: bool,
    /// When the find bar, as it opens, looks only in the selection.
    pub find_in_selection: FindInSelection,
    /// Where the lightbulb stands when changes are offered at the caret.
    pub lightbulb: LightbulbShown,
    /// Whether selected text can be dragged to another place.
    pub drag_and_drop: bool,
    /// Whether the rest of a completion, such as its documentation, is asked for as it is chosen.
    pub completion_docs: bool,
    /// Whether the minimap's edge marks matches, problems and breakpoints.
    pub scrollbar_marks: bool,
    /// Whether the text scrolls on past its last line until that line is at the top.
    pub scroll_beyond: bool,
    /// Whether brackets are coloured by how deep they nest.
    pub bracket_colors: bool,
    /// Whether values that look private, such as keys in a `.env` file, are hidden until the caret is on their line.
    pub redact: bool,
    /// Whether the arrows move the caret through right-to-left text the way they point on screen.
    pub visual_caret: bool,
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

    /// This look with the caret's line blamed only when `blamed` holds as well as the editor's own setting, and the changed words of a diff drawn apart only when `word_diff` holds.
    pub fn with_git_marks(self, blamed: bool, word_diff: bool) -> Self {
        Self {
            inline_blame: self.inline_blame && blamed,
            word_diff,
            ..self
        }
    }

    /// This look with indentation `tab_size` columns wide and made of tabs as `hard_tabs` says, each only when given.
    pub fn with_indent(self, tab_size: Option<usize>, hard_tabs: Option<bool>) -> Self {
        Self {
            tab_size: tab_size.or(self.tab_size),
            hard_tabs: hard_tabs.or(self.hard_tabs),
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
            tab_size: settings.tab_size,
            hard_tabs: settings.hard_tabs,
            auto_surround: settings.auto_surround,
            extend_comments: settings.extend_comment_on_newline,
            snippets_on_tab: settings.snippets_on_tab,
            autosave: settings.autosave,
            completions_on_input: settings.completions_on_input,
            hover: settings.hover,
            hover_delay: settings.hover_delay,
            signature_help: settings.signature_help,
            cursor_blink: settings.cursor_blink,
            soft_wrap: settings.soft_wrap,
            line_length: settings.preferred_line_length,
            format_on_save: settings.format_on_save,
            format_on_type: settings.format_on_type,
            diagnostics: settings.diagnostics,
            inline_blame: settings.inline_blame,
            word_diff: true,
            search_wrap: settings.search_wrap,
            seed_search: settings.seed_search_from_selection,
            smartcase: settings.smartcase,
            find_in_selection: settings.auto_find_in_selection,
            lightbulb: settings.lightbulb,
            drag_and_drop: settings.drag_and_drop,
            completion_docs: settings.completion_documentation,
            scrollbar_marks: settings.scrollbar_marks,
            scroll_beyond: settings.scroll_beyond_last_line,
            bracket_colors: settings.bracket_colors,
            redact: settings.redact_private_values,
            visual_caret: settings.visual_caret_movement,
        }
    }
}

/// The code look in effect, as the editor comes until [`crate::app_theme`] provides another.
pub(crate) static LOCAL_CODE_LOOK: CompositionLocal<CodeLook> =
    CompositionLocal::new(CodeLook::default);

/// Emits `content` with `look` as the code look in effect, such as one a file's language changes.
pub fn provide_code_look(look: CodeLook, content: impl FnOnce()) {
    LOCAL_CODE_LOOK.provide(look, content)
}

/// The code look in effect at this point of the composition.
pub fn code_look() -> CodeLook {
    LOCAL_CODE_LOOK.current()
}

thread_local! {
    /// The whole editor settings, for what the code look cannot carry by
    /// copy, such as the reader's snippets.
    static EDITOR_SETTINGS: std::cell::RefCell<std::rc::Rc<EditorSettings>> =
        std::cell::RefCell::new(std::rc::Rc::new(EditorSettings::default()));
}

/// Makes `settings` the editor settings every code editor reads its
/// snippets from; does nothing when they are already in effect.
pub fn use_editor_settings(settings: &EditorSettings) {
    EDITOR_SETTINGS.with(|held| {
        if **held.borrow() != *settings {
            *held.borrow_mut() = std::rc::Rc::new(settings.clone());
        }
    });
}

/// The editor settings in effect, as [`use_editor_settings`] last made them.
pub fn editor_settings() -> std::rc::Rc<EditorSettings> {
    EDITOR_SETTINGS.with(|held| held.borrow().clone())
}
