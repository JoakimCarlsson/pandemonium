//! The editor's pages of the settings panel: how files look, how they are edited and saved, what the language server adds, and searching and scrolling.

use pm_editor::{DiagnosticsShown, EditorSettings};
use pm_input::FocusId;
use pm_style::{
    Radius, Role, bg, border, border_color, cursor_pointer, flex, flex_col, flex_none, font_mono,
    gap, h, items_center, justify_between, justify_center, min_w_px, px, rounded, w_px,
};
use pm_ui::{Node, node};

use crate::parts::{line, wrap};
use crate::settings::Section;
use crate::state::WorkspaceState;

/// The first of the controls of the editor's settings; each setting has eight after its own base, its place among them times eight.
const EDITOR_SETTING_BASE: u64 = 20_001_000;

/// The pages of the editor's settings, in the order the list of sections shows them.
const PAGES: [Section; 5] = [
    Section::EditorAppearance,
    Section::EditorEditing,
    Section::EditorSaving,
    Section::EditorIntelligence,
    Section::EditorSearch,
];

/// The room between what a setting says and its control, in logical pixels.
const ROW_GAP: f32 = 24.0;

/// The least room what a setting says is given, in logical pixels, however wide its control.
const MIN_WORDS: f32 = 120.0;

/// How wide a character of the choices is taken to be, in logical pixels, generously so that the words beside never run under them.
const CHOICE_CHARACTER: f32 = 7.5;

/// How many controls a setting has room for.
const PER_SETTING: u64 = 8;

/// One setting of the editor.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Setting {
    /// How big the text is.
    FontSize,
    /// Whether lines have numbers.
    LineNumbers,
    /// Whether the numbers count from the caret.
    RelativeLineNumbers,
    /// Whether the caret's line is marked.
    CurrentLine,
    /// Whether indentation has upright lines.
    IndentGuides,
    /// Whether a line shows where lines should end.
    WrapGuide,
    /// Where lines should end.
    LineLength,
    /// Whether lines are marked by how they differ from the last commit.
    GitGutter,
    /// Whether the caret's line says who changed it.
    InlineBlame,
    /// Whether the uses of the name at the caret are marked.
    Occurrences,
    /// How many columns a level of indentation is.
    TabSize,
    /// Whether indentation is tabs.
    HardTabs,
    /// Whether brackets and quotes close themselves.
    AutoClose,
    /// Whether a bracket typed over a selection goes around it.
    AutoSurround,
    /// Whether new lines are indented.
    AutoIndent,
    /// Whether a new line after a comment is a comment.
    ExtendComments,
    /// Whether saving formats.
    FormatOnSave,
    /// Whether saving takes off trailing spaces.
    TrimWhitespace,
    /// Whether saving ends the file with a line break.
    FinalNewline,
    /// Whether files save themselves.
    Autosave,
    /// Whether completions open as one types.
    CompletionsOnInput,
    /// Whether completions show their documentation.
    CompletionDocs,
    /// Whether resting on a name shows what is known about it.
    Hover,
    /// How long the pointer rests first.
    HoverDelay,
    /// Whether the signature of a call shows.
    SignatureHelp,
    /// How bad something must be to be marked.
    Diagnostics,
    /// Whether finding goes round the end.
    SearchWrap,
    /// Whether finding starts from the selection.
    SeedSearch,
    /// Whether a capital letter makes finding match case.
    Smartcase,
    /// Whether the file scrolls past its end.
    ScrollBeyond,
    /// How many lines are kept around the caret.
    ScrollMargin,
}

/// What a setting is changed with, and how it stands.
enum Control {
    /// A switch, on or off.
    Switch(bool),
    /// One of several choices, the one picked.
    Choice(&'static [&'static str], usize),
    /// A number made smaller or bigger, as it is written.
    Number(String),
}

impl Control {
    /// How wide it is drawn, in logical pixels, at most.
    fn width(&self) -> f32 {
        match self {
            Self::Switch(_) => 36.0,
            Self::Number(_) => 136.0,
            Self::Choice(names, _) => {
                8.0 + names
                    .iter()
                    .map(|name| 22.0 + name.chars().count() as f32 * CHOICE_CHARACTER)
                    .sum::<f32>()
            }
        }
    }
}

impl Setting {
    /// Every setting, in the order the pages show them.
    const ALL: [Self; 31] = [
        Self::FontSize,
        Self::LineNumbers,
        Self::RelativeLineNumbers,
        Self::CurrentLine,
        Self::IndentGuides,
        Self::WrapGuide,
        Self::LineLength,
        Self::GitGutter,
        Self::InlineBlame,
        Self::Occurrences,
        Self::TabSize,
        Self::HardTabs,
        Self::AutoClose,
        Self::AutoSurround,
        Self::AutoIndent,
        Self::ExtendComments,
        Self::FormatOnSave,
        Self::TrimWhitespace,
        Self::FinalNewline,
        Self::Autosave,
        Self::CompletionsOnInput,
        Self::CompletionDocs,
        Self::Hover,
        Self::HoverDelay,
        Self::SignatureHelp,
        Self::Diagnostics,
        Self::SearchWrap,
        Self::SeedSearch,
        Self::Smartcase,
        Self::ScrollBeyond,
        Self::ScrollMargin,
    ];

    /// The page it is on.
    fn page(self) -> Section {
        match self {
            Self::FontSize
            | Self::LineNumbers
            | Self::RelativeLineNumbers
            | Self::CurrentLine
            | Self::IndentGuides
            | Self::WrapGuide
            | Self::LineLength
            | Self::GitGutter
            | Self::InlineBlame
            | Self::Occurrences => Section::EditorAppearance,
            Self::TabSize
            | Self::HardTabs
            | Self::AutoClose
            | Self::AutoSurround
            | Self::AutoIndent
            | Self::ExtendComments => Section::EditorEditing,
            Self::FormatOnSave | Self::TrimWhitespace | Self::FinalNewline | Self::Autosave => {
                Section::EditorSaving
            }
            Self::CompletionsOnInput
            | Self::CompletionDocs
            | Self::Hover
            | Self::HoverDelay
            | Self::SignatureHelp
            | Self::Diagnostics => Section::EditorIntelligence,
            Self::SearchWrap
            | Self::SeedSearch
            | Self::Smartcase
            | Self::ScrollBeyond
            | Self::ScrollMargin => Section::EditorSearch,
        }
    }

    /// What it is called, and what it does.
    fn words(self) -> (&'static str, &'static str) {
        match self {
            Self::FontSize => ("Font size", "How big the text of a file is."),
            Self::LineNumbers => ("Line numbers", "Show each line's number beside it."),
            Self::RelativeLineNumbers => (
                "Relative line numbers",
                "Count the numbers from the line of the caret.",
            ),
            Self::CurrentLine => (
                "Highlight the current line",
                "Mark the line the caret is on.",
            ),
            Self::IndentGuides => (
                "Indent guides",
                "Show each level of indentation with an upright line.",
            ),
            Self::WrapGuide => (
                "Wrap guide",
                "Show where lines should end with an upright line.",
            ),
            Self::LineLength => (
                "Preferred line length",
                "Where the wrap guide stands, in characters.",
            ),
            Self::GitGutter => (
                "Git changes beside the line numbers",
                "Mark lines added, changed and taken out since the last commit.",
            ),
            Self::InlineBlame => (
                "Inline blame",
                "Show who last changed the line of the caret, and when.",
            ),
            Self::Occurrences => (
                "Highlight uses of the name at the caret",
                "Mark every place the name under the caret is used in the file.",
            ),
            Self::TabSize => (
                "Tab size",
                "How many columns a level of indentation is; Language keeps each language's own.",
            ),
            Self::HardTabs => (
                "Indent with",
                "Spaces or tabs; Language keeps each language's own.",
            ),
            Self::AutoClose => (
                "Close brackets and quotes",
                "Write the closing one as the opening one is typed.",
            ),
            Self::AutoSurround => (
                "Surround the selection",
                "A bracket or quote typed over a selection goes around it.",
            ),
            Self::AutoIndent => (
                "Indent new lines",
                "Indent a new line as the language says.",
            ),
            Self::ExtendComments => (
                "Continue comments",
                "A new line after a line comment starts with the comment too.",
            ),
            Self::FormatOnSave => (
                "Format on save",
                "Format the file as its language says when it is saved.",
            ),
            Self::TrimWhitespace => (
                "Remove trailing whitespace",
                "Take spaces and tabs off the ends of lines when saving.",
            ),
            Self::FinalNewline => (
                "End with a line break",
                "Make the file end with a line break when saving.",
            ),
            Self::Autosave => (
                "Autosave",
                "Save a file a moment after it was last changed.",
            ),
            Self::CompletionsOnInput => (
                "Show completions while typing",
                "Open the list of completions as you type; Ctrl+Space always opens it.",
            ),
            Self::CompletionDocs => (
                "Completion documentation",
                "Show the documentation of the completion picked beside the list.",
            ),
            Self::Hover => (
                "Hover",
                "Show what the language server says about the name the pointer rests on.",
            ),
            Self::HoverDelay => (
                "Hover delay",
                "How long the pointer rests before that, in milliseconds.",
            ),
            Self::SignatureHelp => (
                "Signature help",
                "Show the signature of the call the caret is in as you type.",
            ),
            Self::Diagnostics => (
                "Show problems",
                "How bad something the language server finds must be for it to be marked.",
            ),
            Self::SearchWrap => ("Wrap around", "Go round to the first match after the last."),
            Self::SeedSearch => (
                "Search the selection",
                "Start finding with what is selected.",
            ),
            Self::Smartcase => (
                "Smart case",
                "Match case when what you look for has a capital letter.",
            ),
            Self::ScrollBeyond => (
                "Scroll beyond the last line",
                "Scroll on until the last line is near the top.",
            ),
            Self::ScrollMargin => (
                "Scroll margin",
                "How many lines are kept between the caret and the edges of the view.",
            ),
        }
    }

    /// How it stands in `settings`.
    fn control(self, settings: &EditorSettings) -> Control {
        const TABS: &[&str] = &["Language", "2", "4", "8"];
        const INDENT: &[&str] = &["Language", "Spaces", "Tabs"];
        const PROBLEMS: &[&str] = &["All", "Warnings", "Errors", "Off"];
        match self {
            Self::FontSize => Control::Number(format!("{} px", settings.font_size)),
            Self::LineNumbers => Control::Switch(settings.line_numbers),
            Self::RelativeLineNumbers => Control::Switch(settings.relative_line_numbers),
            Self::CurrentLine => Control::Switch(settings.current_line_highlight),
            Self::IndentGuides => Control::Switch(settings.indent_guides),
            Self::WrapGuide => Control::Switch(settings.wrap_guide),
            Self::LineLength => Control::Number(settings.preferred_line_length.to_string()),
            Self::GitGutter => Control::Switch(settings.git_gutter),
            Self::InlineBlame => Control::Switch(settings.inline_blame),
            Self::Occurrences => Control::Switch(settings.occurrence_highlights),
            Self::TabSize => Control::Choice(
                TABS,
                match settings.tab_size {
                    None => 0,
                    Some(2) => 1,
                    Some(4) => 2,
                    Some(_) => 3,
                },
            ),
            Self::HardTabs => Control::Choice(
                INDENT,
                match settings.hard_tabs {
                    None => 0,
                    Some(false) => 1,
                    Some(true) => 2,
                },
            ),
            Self::AutoClose => Control::Switch(settings.auto_close),
            Self::AutoSurround => Control::Switch(settings.auto_surround),
            Self::AutoIndent => Control::Switch(settings.auto_indent),
            Self::ExtendComments => Control::Switch(settings.extend_comment_on_newline),
            Self::FormatOnSave => Control::Switch(settings.format_on_save),
            Self::TrimWhitespace => Control::Switch(settings.remove_trailing_whitespace_on_save),
            Self::FinalNewline => Control::Switch(settings.ensure_final_newline_on_save),
            Self::Autosave => Control::Switch(settings.autosave),
            Self::CompletionsOnInput => Control::Switch(settings.completions_on_input),
            Self::CompletionDocs => Control::Switch(settings.completion_documentation),
            Self::Hover => Control::Switch(settings.hover),
            Self::HoverDelay => Control::Number(format!("{} ms", settings.hover_delay)),
            Self::SignatureHelp => Control::Switch(settings.signature_help),
            Self::Diagnostics => Control::Choice(
                PROBLEMS,
                match settings.diagnostics {
                    DiagnosticsShown::All => 0,
                    DiagnosticsShown::Warnings => 1,
                    DiagnosticsShown::Errors => 2,
                    DiagnosticsShown::Off => 3,
                },
            ),
            Self::SearchWrap => Control::Switch(settings.search_wrap),
            Self::SeedSearch => Control::Switch(settings.seed_search_from_selection),
            Self::Smartcase => Control::Switch(settings.smartcase),
            Self::ScrollBeyond => Control::Switch(settings.scroll_beyond_last_line),
            Self::ScrollMargin => Control::Number(settings.vertical_scroll_margin.to_string()),
        }
    }

    /// Changes `settings` as pressing its control at `option` does: a switch flips, a choice is picked, and a number goes down for the first and up for the second.
    fn press(self, settings: &mut EditorSettings, option: usize) {
        let step = |value: i64, by: i64, low: i64, high: i64| {
            let by = if option == 0 { -by } else { by };
            (value + by).clamp(low, high)
        };
        match self {
            Self::FontSize => settings.font_size = step(settings.font_size.into(), 1, 9, 28) as u32,
            Self::LineNumbers => settings.line_numbers ^= true,
            Self::RelativeLineNumbers => settings.relative_line_numbers ^= true,
            Self::CurrentLine => settings.current_line_highlight ^= true,
            Self::IndentGuides => settings.indent_guides ^= true,
            Self::WrapGuide => settings.wrap_guide ^= true,
            Self::LineLength => {
                settings.preferred_line_length =
                    step(settings.preferred_line_length as i64, 10, 40, 200) as usize;
            }
            Self::GitGutter => settings.git_gutter ^= true,
            Self::InlineBlame => settings.inline_blame ^= true,
            Self::Occurrences => settings.occurrence_highlights ^= true,
            Self::TabSize => settings.tab_size = [None, Some(2), Some(4), Some(8)][option.min(3)],
            Self::HardTabs => settings.hard_tabs = [None, Some(false), Some(true)][option.min(2)],
            Self::AutoClose => settings.auto_close ^= true,
            Self::AutoSurround => settings.auto_surround ^= true,
            Self::AutoIndent => settings.auto_indent ^= true,
            Self::ExtendComments => settings.extend_comment_on_newline ^= true,
            Self::FormatOnSave => settings.format_on_save ^= true,
            Self::TrimWhitespace => settings.remove_trailing_whitespace_on_save ^= true,
            Self::FinalNewline => settings.ensure_final_newline_on_save ^= true,
            Self::Autosave => settings.autosave ^= true,
            Self::CompletionsOnInput => settings.completions_on_input ^= true,
            Self::CompletionDocs => settings.completion_documentation ^= true,
            Self::Hover => settings.hover ^= true,
            Self::HoverDelay => {
                settings.hover_delay = step(settings.hover_delay as i64, 50, 0, 2000) as u64;
            }
            Self::SignatureHelp => settings.signature_help ^= true,
            Self::Diagnostics => {
                settings.diagnostics = [
                    DiagnosticsShown::All,
                    DiagnosticsShown::Warnings,
                    DiagnosticsShown::Errors,
                    DiagnosticsShown::Off,
                ][option.min(3)];
            }
            Self::SearchWrap => settings.search_wrap ^= true,
            Self::SeedSearch => settings.seed_search_from_selection ^= true,
            Self::Smartcase => settings.smartcase ^= true,
            Self::ScrollBeyond => settings.scroll_beyond_last_line ^= true,
            Self::ScrollMargin => {
                settings.vertical_scroll_margin =
                    step(settings.vertical_scroll_margin as i64, 1, 0, 20) as usize;
            }
        }
    }

    /// The name of its control at `option`.
    fn id(self, option: usize) -> FocusId {
        let place = Self::ALL
            .iter()
            .position(|held| *held == self)
            .unwrap_or_default() as u64;
        FocusId(EDITOR_SETTING_BASE + place * PER_SETTING + option as u64)
    }

    /// The setting, and the option of its control, that `id` names.
    fn of(id: FocusId) -> Option<(Self, usize)> {
        let offset = id.0.checked_sub(EDITOR_SETTING_BASE)?;
        let setting = Self::ALL.get((offset / PER_SETTING) as usize)?;
        Some((*setting, (offset % PER_SETTING) as usize))
    }
}

/// Whether `id` names a control of the editor's settings.
pub fn is_editor_setting(id: FocusId) -> bool {
    Setting::of(id).is_some()
}

impl WorkspaceState {
    /// Changes the editor's settings as pressing the control `id` does, and puts them to work at once; says whether it was such a control.
    pub(crate) fn editor_setting_pressed(&mut self, id: FocusId) -> bool {
        let Some((setting, option)) = Setting::of(id) else {
            return false;
        };
        setting.press(&mut self.editor, option);
        self.apply_indentation();
        true
    }

    /// The state with the editor's settings `settings`, read from where they were kept, put to work.
    pub fn with_editor_settings(mut self, settings: EditorSettings) -> Self {
        self.editor = settings;
        self.editor_loaded = true;
        self.apply_indentation();
        self
    }

    /// Indents every open file as the editor's settings say, over its language's own.
    pub(crate) fn apply_indentation(&mut self) {
        let (columns, tabs) = (self.editor.tab_size, self.editor.hard_tabs);
        for (_, viewer) in self.every_viewer() {
            if let crate::open_files::Contents::Text(document) = &mut viewer.contents {
                document.set_indentation(columns, tabs);
            }
        }
    }
}

/// What a page of the editor's settings says about itself under its heading.
pub(crate) fn summary(section: Section) -> &'static str {
    match section {
        Section::Editor => {
            "How files look, how they are edited and saved, and what the language server adds as you write."
        }
        Section::EditorAppearance => {
            "The text, the line numbers and what is marked beside and over the lines."
        }
        Section::EditorEditing => {
            "Indentation, brackets and quotes, and what happens when a line is broken."
        }
        Section::EditorSaving => "What is done to a file as it is saved, and when it saves itself.",
        Section::EditorIntelligence => {
            "Completions, hovers, signatures and the problems the language server finds."
        }
        Section::EditorSearch => "Finding in a file, and how the view follows the caret.",
        _ => "",
    }
}

/// A switch, lit when `on`.
fn switch(on: bool, id: FocusId) -> Node {
    crate::settings_servers::switch(on, id)
}

/// The choices of a setting side by side, `picked` marked, each its own control from `setting`'s base on.
fn choices(setting: Setting, names: &[&str], picked: usize) -> Node {
    node()
        .styles([
            flex(),
            items_center(),
            flex_none(),
            gap(0.5),
            px(0.5),
            h(8),
            rounded(Radius::Md),
            border(),
            border_color(Role::Border),
        ])
        .children(names.iter().enumerate().map(|(option, name)| {
            let mut held = node()
                .styles([
                    flex(),
                    items_center(),
                    justify_center(),
                    h(6.5),
                    px(2.5),
                    rounded(Radius::Sm),
                    cursor_pointer(),
                ])
                .focusable(setting.id(option))
                .child(line(
                    *name,
                    12,
                    1.4,
                    if option == picked {
                        Role::Text
                    } else {
                        Role::Muted
                    },
                ));
            held = if option == picked {
                held.styles([bg(Role::SurfaceSelected)])
            } else {
                held.styles([bg(Role::SurfaceHover).hover()])
            };
            held
        }))
}

/// A number with the buttons that make it smaller and bigger.
fn number(setting: Setting, shown: String) -> Node {
    let button = |label: &'static str, option: usize| {
        node()
            .styles([
                flex(),
                items_center(),
                justify_center(),
                w_px(28.0),
                h(7),
                rounded(Radius::Sm),
                cursor_pointer(),
                bg(Role::SurfaceHover).hover(),
            ])
            .focusable(setting.id(option))
            .child(line(label, 14, 1.2, Role::Text))
    };
    node()
        .styles([
            flex(),
            items_center(),
            flex_none(),
            gap(1),
            px(0.5),
            h(8),
            rounded(Radius::Md),
            border(),
            border_color(Role::Border),
        ])
        .child(button("−", 0))
        .child(
            node()
                .styles([flex(), justify_center(), w_px(64.0)])
                .child(line(shown, 12, 1.4, Role::Text).styles([font_mono()])),
        )
        .child(button("+", 1))
}

/// One row of a page, `room` logical pixels across: what the setting is called and does, broken into lines beside its control, and the control.
fn row(setting: Setting, settings: &EditorSettings, room: f32) -> Node {
    let (title, about) = setting.words();
    let shown = setting.control(settings);
    let words = (room - shown.width() - ROW_GAP).max(MIN_WORDS);
    let control = match shown {
        Control::Switch(on) => switch(on, setting.id(0)),
        Control::Choice(names, picked) => choices(setting, names, picked),
        Control::Number(shown) => number(setting, shown),
    };
    node()
        .styles([
            flex(),
            items_center(),
            justify_between(),
            gap(6),
            pm_style::py(3),
            flex_none(),
        ])
        .child(
            node()
                .styles([flex(), flex_col(), gap(0.5), min_w_px(0)])
                .children(
                    wrap(title, 13, words)
                        .into_iter()
                        .map(|words| line(words, 13, 1.4, Role::Text)),
                )
                .children(
                    wrap(about, 12, words)
                        .into_iter()
                        .map(|words| line(words, 12, 1.4, Role::Muted)),
                ),
        )
        .child(control)
}

/// The rows of the settings on the page `section`, `room` logical pixels across, side by side with their controls, with a hairline between each and the next.
pub(crate) fn rows(state: &WorkspaceState, section: Section, room: f32) -> Node {
    let shown: Vec<Setting> = Setting::ALL
        .into_iter()
        .filter(|setting| setting.page() == section)
        .collect();
    let last = shown.len().saturating_sub(1);
    node()
        .styles([flex(), flex_col(), flex_none()])
        .children(shown.into_iter().enumerate().map(|(place, setting)| {
            let held = row(setting, &state.editor, room);
            if place < last {
                held.styles([pm_style::border_b(), border_color(Role::Border)])
            } else {
                held
            }
        }))
}

/// Every page of the editor's settings one under another, each under its name, `room` logical pixels across.
pub(crate) fn groups(state: &WorkspaceState, room: f32) -> impl Iterator<Item = Node> + '_ {
    PAGES.into_iter().map(move |page| {
        node()
            .styles([flex(), flex_col(), flex_none(), pm_style::pt(2)])
            .child(
                node()
                    .styles([
                        flex(),
                        items_center(),
                        flex_none(),
                        pm_style::pb(1),
                        pm_style::border_b(),
                        border_color(Role::Border),
                    ])
                    .child(
                        line(page.title(), 12, 1.4, Role::Muted).styles([pm_style::font_medium()]),
                    ),
            )
            .child(rows(state, page, room))
    })
}
