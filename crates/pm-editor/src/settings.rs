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

/// How the editor looks and behaves.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct EditorSettings {
    /// How big the text of a file is, in logical pixels.
    pub font_size: u32,
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
    /// The bindings of keys the keymap starts from.
    pub keymap: crate::keymap::Preset,
    /// The reader's own bindings over the preset, by the name each action is kept under; an empty list leaves the action without keys.
    pub keys: std::collections::BTreeMap<String, Vec<String>>,
}

impl Default for EditorSettings {
    /// The editor as it comes.
    fn default() -> Self {
        Self {
            font_size: 12,
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
            keymap: crate::keymap::Preset::default(),
            keys: std::collections::BTreeMap::new(),
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
        (self.font_size as f32 * 5.0 / 3.0).round()
    }
}
