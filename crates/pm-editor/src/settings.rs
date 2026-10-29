//! How the reader wants the editor to look and behave, kept between runs.

use serde::{Deserialize, Serialize};

/// How bad something a language server finds wrong must be for the editor to mark it.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticsShown {
    /// Errors, warnings, information and hints.
    #[default]
    All,
    /// Errors and warnings.
    Warnings,
    /// Errors only.
    Errors,
    /// Nothing.
    Off,
}

impl DiagnosticsShown {
    /// Whether something of `severity` is marked.
    pub fn shows(self, severity: pm_protocol::Severity) -> bool {
        use pm_protocol::Severity;
        match self {
            Self::All => true,
            Self::Warnings => matches!(severity, Severity::Error | Severity::Warning),
            Self::Errors => severity == Severity::Error,
            Self::Off => false,
        }
    }
}

/// Where long lines of a file break onto the next row.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SoftWrap {
    /// Nowhere: a long line runs on past the edge.
    #[default]
    Off,
    /// At the edge of the view.
    Edge,
    /// At the preferred line length, or the edge of the view when that comes first.
    LineLength,
}

/// When finding looks only in the lines that were selected as the bar for it opens.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FindInSelection {
    /// Never: finding looks in the whole file until the reader asks otherwise.
    #[default]
    Never,
    /// Whenever something is selected.
    Always,
    /// When the selection runs over more than one line.
    Multiline,
}

impl FindInSelection {
    /// Whether finding is limited to `selected` as the bar opens over it.
    pub fn limits_to(self, selected: &str) -> bool {
        match self {
            Self::Never => false,
            Self::Always => !selected.is_empty(),
            Self::Multiline => selected.trim_end_matches('\n').contains('\n'),
        }
    }
}

/// Where the marker that says the language server offers changes shows.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LightbulbShown {
    /// Nowhere.
    Off,
    /// Beside the line of the caret, where changes are offered there.
    #[default]
    CaretLine,
    /// Beside the line of the caret only while that line has a problem marked.
    Problems,
}

/// How the caret of a file is drawn.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CursorShape {
    /// A thin upright bar between two characters.
    #[default]
    Bar,
    /// The character after it filled.
    Block,
    /// A line under the character after it.
    Underline,
}

/// How much vim's unnamed register shares with the system clipboard.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VimClipboard {
    /// Every yank and delete goes to the clipboard, and pastes read it.
    #[default]
    Always,
    /// Only yanks go to the clipboard; pastes still read it.
    OnYank,
    /// The clipboard is only reached through `"+` and `"*`.
    Never,
}

impl From<VimClipboard> for pm_vim::ClipboardUse {
    /// The sharing the engine knows by the same name.
    fn from(sharing: VimClipboard) -> Self {
        match sharing {
            VimClipboard::Always => Self::Always,
            VimClipboard::OnYank => Self::OnYank,
            VimClipboard::Never => Self::Never,
        }
    }
}

/// How the editor looks and behaves.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct EditorSettings {
    /// How big the text of a file is, in logical pixels.
    pub font_size: u32,
    /// How tall a line of a file is, in hundredths of its text's size.
    pub line_height: u32,
    /// Where long lines break onto the next row.
    pub soft_wrap: SoftWrap,
    /// How the caret is drawn.
    pub cursor_shape: CursorShape,
    /// Whether the caret blinks.
    pub cursor_blink: bool,
    /// Whether the scrollbar marks where the problems, the changes and the matches of a search are.
    pub scrollbar_marks: bool,
    /// Whether each line has its number beside it.
    pub line_numbers: bool,
    /// Whether the numbers count from the line of the caret.
    pub relative_line_numbers: bool,
    /// Whether the line of the caret is marked.
    pub current_line_highlight: bool,
    /// Whether upright lines show each level of indentation.
    pub indent_guides: bool,
    /// Whether an upright line shows where lines should end.
    pub wrap_guide: bool,
    /// How many characters long a line should be at most.
    pub preferred_line_length: usize,
    /// Whether marks beside the line numbers show how lines differ from the last commit.
    pub git_gutter: bool,
    /// Whether the line of the caret says who last changed it.
    pub inline_blame: bool,
    /// Whether the uses of the name at the caret are marked.
    pub occurrence_highlights: bool,
    /// How many columns a level of indentation is, or the language's own when nothing.
    pub tab_size: Option<usize>,
    /// Whether a level of indentation is a tab, or the language's own when nothing.
    pub hard_tabs: Option<bool>,
    /// Whether brackets and quotes typed are closed at once.
    pub auto_close: bool,
    /// Whether a bracket or quote typed over a selection goes around it.
    pub auto_surround: bool,
    /// Whether a new line is indented as the language says.
    pub auto_indent: bool,
    /// Whether a new line after a line comment starts with the comment too.
    pub extend_comment_on_newline: bool,
    /// Whether the file is formatted as its language says when it is saved.
    pub format_on_save: bool,
    /// Whether what was just typed is formatted as its language says.
    pub format_on_type: bool,
    /// Whether typing in one of several stretches that belong together, such as the two names of a tag, types in the others too.
    pub linked_editing: bool,
    /// Whether brackets that pair are coloured by how deeply they are nested.
    pub bracket_colors: bool,
    /// Whether names are coloured by what the language server says they are, over what the syntax says.
    pub semantic_highlighting: bool,
    /// Whether the language server's hints, such as the type of a name, are written inside the lines.
    pub inlay_hints: bool,
    /// Whether the language server's lenses, such as how many times a function is used, are shown beside the lines.
    pub code_lens: bool,
    /// Whether a word that is the name of a snippet is written out when Tab is pressed after it.
    pub snippets_on_tab: bool,
    /// Whether an Emmet abbreviation written in a markup or style file is written out when Tab is pressed after it.
    pub emmet: bool,
    /// The reader's own snippets, by the language they are for, or for every language under `*`.
    pub snippets: std::collections::BTreeMap<String, Vec<crate::snippet::UserSnippet>>,
    /// Whether a map of the whole file is drawn beside it.
    pub minimap: bool,
    /// Whether the lines that open the blocks the first line shown is in stay at the top as the file scrolls.
    pub sticky_scroll: bool,
    /// Whether spaces at the ends of lines are taken off when the file is saved.
    pub remove_trailing_whitespace_on_save: bool,
    /// Whether the file is made to end with a line break when it is saved.
    pub ensure_final_newline_on_save: bool,
    /// Whether a file is saved a moment after it was last changed.
    pub autosave: bool,
    /// Whether completions open as the reader types.
    pub completions_on_input: bool,
    /// Whether the documentation of the completion picked shows beside the list.
    pub completion_documentation: bool,
    /// Whether resting the pointer on a name shows what the language server says about it.
    pub hover: bool,
    /// How long the pointer rests before that, in milliseconds.
    pub hover_delay: u64,
    /// Whether the signature of the call the caret is in shows as it is typed.
    pub signature_help: bool,
    /// How bad something must be to be marked.
    pub diagnostics: DiagnosticsShown,
    /// Whether going past the last match goes round to the first.
    pub search_wrap: bool,
    /// Whether finding starts from what is selected.
    pub seed_search_from_selection: bool,
    /// Whether a search with a capital letter in it matches case.
    pub smartcase: bool,
    /// Whether the file scrolls on until its last line is at the top.
    pub scroll_beyond_last_line: bool,
    /// How many lines are kept between the caret and the top and bottom of the view.
    pub vertical_scroll_margin: usize,
    /// Whether the values a language marks as private are hidden behind blocks, so that sharing the screen does not give them away.
    pub redact_private_values: bool,
    /// Whether the newest versions of the dependencies a manifest lists are looked up in their registry and offered as links that update them.
    pub dependency_hints: bool,
    /// The reader's own rules for what the types of tokens a language server names look like, which come before the language's.
    pub semantic_token_rules: Vec<pm_lsp::TokenRule>,
    /// Whether files are edited with vim's keys.
    pub vim_mode: bool,
    /// How much vim's unnamed register shares with the system clipboard.
    pub vim_clipboard: VimClipboard,
    /// The reader's own vim bindings over the ones that come with it, each written as `keys => Action`, with `@ context` after when it holds only in one.
    pub vim_bindings: Vec<String>,
    /// Whether text typed in insert mode and replayed by a count, `.` or a macro is typed as live typing is: pairs closed and stepped over, new lines indented as the language says.
    pub vim_replay_typing: bool,
    /// Whether vim's function, class and comment objects and motions go by the file's syntax tree.
    pub vim_syntax_objects: bool,
    /// The bindings of keys the keymap starts from.
    pub keymap: crate::keymap::Preset,
    /// The reader's own bindings over the preset, by the name each action is kept under; an empty list leaves the action without keys.
    pub keys: std::collections::BTreeMap<String, Vec<String>>,
    /// Whether saving first organizes the imports of the file, as its language server says.
    pub organize_imports_on_save: bool,
    /// Whether saving first applies every fix of the language server that is safe to apply at once.
    pub fix_all_on_save: bool,
    /// How many lines a file has before the names of the lines in view are asked of the language server first and those of the rest after; none for never.
    pub semantic_range_lines: usize,
    /// How many lines above and below the view are asked for with the lines in view.
    pub semantic_range_margin: usize,
    /// When finding looks only in the lines that were selected as the bar for it opens.
    pub auto_find_in_selection: FindInSelection,
    /// Whether selected text is moved by pressing on it and dragging it, and copied when the copy key is held.
    pub drag_and_drop: bool,
    /// Where the marker for the changes the language server offers shows.
    pub lightbulb: LightbulbShown,
    /// Whether the code font joins characters such as `->` and `!=` into one glyph where the font has such ligatures.
    pub ligatures: bool,
    /// Whether the caret of a line with right-to-left text moves the way the arrow keys point on screen.
    pub visual_caret_movement: bool,
    /// Whether comparing two texts leaves out lines that differ only in white space.
    pub compare_ignore_whitespace: bool,
}

impl Default for EditorSettings {
    /// The editor as it comes.
    fn default() -> Self {
        Self {
            font_size: 12,
            line_height: 167,
            soft_wrap: SoftWrap::Off,
            cursor_shape: CursorShape::Bar,
            cursor_blink: true,
            scrollbar_marks: true,
            line_numbers: true,
            relative_line_numbers: false,
            current_line_highlight: true,
            indent_guides: true,
            wrap_guide: false,
            preferred_line_length: 100,
            git_gutter: true,
            inline_blame: true,
            occurrence_highlights: true,
            tab_size: None,
            hard_tabs: None,
            auto_close: true,
            auto_surround: true,
            auto_indent: true,
            extend_comment_on_newline: true,
            format_on_save: true,
            format_on_type: true,
            linked_editing: true,
            bracket_colors: true,
            semantic_highlighting: true,
            inlay_hints: true,
            code_lens: true,
            snippets_on_tab: true,
            emmet: true,
            vim_mode: false,
            vim_clipboard: VimClipboard::Always,
            vim_bindings: Vec::new(),
            vim_replay_typing: true,
            vim_syntax_objects: true,
            snippets: std::collections::BTreeMap::new(),
            minimap: true,
            sticky_scroll: true,
            remove_trailing_whitespace_on_save: true,
            ensure_final_newline_on_save: true,
            autosave: false,
            completions_on_input: true,
            completion_documentation: true,
            hover: true,
            hover_delay: 450,
            signature_help: true,
            diagnostics: DiagnosticsShown::All,
            search_wrap: true,
            seed_search_from_selection: true,
            smartcase: false,
            scroll_beyond_last_line: true,
            vertical_scroll_margin: 3,
            redact_private_values: false,
            dependency_hints: true,
            semantic_token_rules: Vec::new(),
            keymap: crate::keymap::Preset::default(),
            keys: std::collections::BTreeMap::new(),
            organize_imports_on_save: false,
            fix_all_on_save: false,
            semantic_range_lines: 5000,
            semantic_range_margin: 200,
            auto_find_in_selection: FindInSelection::Never,
            drag_and_drop: true,
            lightbulb: LightbulbShown::CaretLine,
            ligatures: true,
            visual_caret_movement: true,
            compare_ignore_whitespace: false,
        }
    }
}

impl EditorSettings {
    /// The settings written as `text`, or the defaults for what it does not say or cannot be read.
    pub fn read(text: &str) -> Self {
        serde_json::from_str(text).unwrap_or_default()
    }

    /// The settings written down.
    pub fn write(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_default()
    }

    /// The keys bound to each action.
    pub fn keymap(&self) -> crate::keymap::Keymap {
        crate::keymap::Keymap::new(self.keymap, &self.keys)
    }

    /// How tall a line of a file is, in logical pixels.
    pub fn row_height(&self) -> f32 {
        (self.font_size as f32 * self.line_height as f32 / 100.0).round()
    }
}
