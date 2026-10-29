//! How each setting stands and changes: the control it is shown with, what pressing that control does, the text of a setting written as text, the choices of one picked from a list, and copying it from one set of settings to another, which is how it is reset and told apart from how it comes.

use pm_config::{
    Channel, Density, Finish, GitMarks, Launch, Notification, TerminalCursor, ThemeMode,
};
use pm_editor::{
    CursorShape, DiagnosticsShown, FindInSelection, LightbulbShown, Preset, SoftWrap, VimClipboard,
};

use crate::settings_catalog::Setting;
use crate::settings_state::Kept;
use crate::state::WorkspaceState;

/// What a setting is changed with, and how it stands.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Control {
    /// A switch, on or off.
    Switch(bool),
    /// One of a few choices, the one picked.
    Choice(&'static [&'static str], usize),
    /// A number made smaller or bigger, as it is written.
    Number(String),
    /// A line of text, saying the words given while it is empty.
    Text(&'static str),
    /// One of a list too long to show beside it, as the one picked is called.
    Picker(String),
    /// A list of lines, each removed on its own, with a box to add one.
    List(Vec<String>),
}

/// The modes an agent can be started in, as the choices name them.
const MODES: &[&str] = &["Agent's own", "Ask", "Accept edits", "Plan", "Bypass"];

/// The ids agents give the modes, in the order of [`MODES`]; nothing for the agent's own.
const MODE_IDS: [Option<&str>; 5] = [
    None,
    Some("default"),
    Some("acceptEdits"),
    Some("plan"),
    Some("bypassPermissions"),
];

/// The presets of keys, as the choices name them, in the order of [`Preset::ALL`].
const PRESETS: &[&str] = &[
    Preset::Pandemonium.title(),
    Preset::VsCode.title(),
    Preset::JetBrains.title(),
    Preset::Sublime.title(),
];

/// What the picker of fonts calls the font the window ships with.
const DEFAULT_FONT: &str = "Default";

/// What the picker of servers calls the server on this machine.
const THIS_MACHINE: &str = "This machine";

/// The place of `value` among `values`, or the first when it is not there.
fn place_of<T: PartialEq>(values: &[T], value: &T) -> usize {
    values.iter().position(|held| held == value).unwrap_or(0)
}

/// The value at `option` among `values`, or the last when there are fewer.
fn chosen<T: Copy>(values: &[T], option: usize) -> T {
    values[option.min(values.len() - 1)]
}

/// `value` moved by `by`, down for option 0 and up otherwise, kept between `low` and `high`.
fn stepped(value: i64, by: i64, option: usize, low: i64, high: i64) -> i64 {
    let by = if option == 0 { -by } else { by };
    (value + by).clamp(low, high)
}

impl Setting {
    /// How it stands in `state`.
    pub(crate) fn control(self, state: &WorkspaceState) -> Control {
        const TABS: &[&str] = &["Language", "2", "4", "8"];
        const INDENT: &[&str] = &["Language", "Spaces", "Tabs"];
        const PROBLEMS: &[&str] = &["All", "Warnings", "Errors", "Off"];
        const LANGUAGE_TABS: &[&str] = &["Editor", "2", "4", "8"];
        const LANGUAGE_INDENT: &[&str] = &["Editor", "Spaces", "Tabs"];
        const LANGUAGE_FORMAT: &[&str] = &["Editor", "On", "Off"];
        const FOLLOW: &[&str] = &["Follow editor", "Off"];
        let editor = &state.editor;
        let window = &state.window;
        let machine = &state.machine;
        let language = self.language_preferences(state);
        match self {
            Self::Launch => Control::Choice(
                &["Last workspace", "Fresh start"],
                place_of(&[Launch::Restore, Launch::Fresh], &window.general.launch),
            ),
            Self::ConfirmClose => Control::Switch(window.general.confirm_close),
            Self::CheckUpdates => Control::Switch(window.general.check_updates),
            Self::OfferLanguages => Control::Switch(window.general.offer_languages),
            Self::EmbeddedBrowser => Control::Switch(window.general.embedded_browser),
            Self::OpenDevServers => Control::Switch(window.general.open_dev_servers),
            Self::Channel => Control::Choice(
                &["Stable", "Preview"],
                place_of(
                    &[Channel::Stable, Channel::Preview],
                    &window.general.channel,
                ),
            ),
            Self::ThemeMode => Control::Choice(
                &["System", "Light", "Dark"],
                place_of(
                    &[ThemeMode::System, ThemeMode::Light, ThemeMode::Dark],
                    &window.appearance.theme_mode,
                ),
            ),
            Self::LightTheme => Control::Picker(window.appearance.light_theme.clone()),
            Self::DarkTheme => Control::Picker(window.appearance.dark_theme.clone()),
            Self::InterfaceFont => Control::Picker(font_shown(&window.appearance.interface_font)),
            Self::InterfaceSize => {
                Control::Number(format!("{} px", window.appearance.interface_size))
            }
            Self::CodeFont => Control::Picker(font_shown(&window.appearance.code_font)),
            Self::CodeSize => Control::Number(format!("{} px", editor.font_size)),
            Self::LineHeight => {
                Control::Number(format!("{:.2}", editor.line_height as f32 / 100.0))
            }
            Self::Density => Control::Choice(
                &["Comfortable", "Compact"],
                place_of(
                    &[Density::Comfortable, Density::Compact],
                    &window.appearance.density,
                ),
            ),
            Self::LineNumbers => Control::Switch(editor.line_numbers),
            Self::RelativeLineNumbers => Control::Switch(editor.relative_line_numbers),
            Self::CurrentLine => Control::Switch(editor.current_line_highlight),
            Self::IndentGuides => Control::Switch(editor.indent_guides),
            Self::WrapGuide => Control::Switch(editor.wrap_guide),
            Self::LineLength => Control::Number(editor.preferred_line_length.to_string()),
            Self::GitGutter => Control::Switch(editor.git_gutter),
            Self::InlineBlame => Control::Switch(editor.inline_blame),
            Self::Occurrences => Control::Switch(editor.occurrence_highlights),
            Self::BracketColors => Control::Switch(editor.bracket_colors),
            Self::RedactPrivateValues => Control::Switch(editor.redact_private_values),
            Self::DependencyHints => Control::Switch(editor.dependency_hints),
            Self::SemanticHighlighting => Control::Switch(editor.semantic_highlighting),
            Self::InlayHints => Control::Switch(editor.inlay_hints),
            Self::CodeLens => Control::Switch(editor.code_lens),
            Self::Minimap => Control::Switch(editor.minimap),
            Self::StickyScroll => Control::Switch(editor.sticky_scroll),
            Self::FormatOnType => Control::Switch(editor.format_on_type),
            Self::LinkedEditing => Control::Switch(editor.linked_editing),
            Self::SnippetsOnTab => Control::Switch(editor.snippets_on_tab),
            Self::Emmet => Control::Switch(editor.emmet),
            Self::CursorShape => Control::Choice(
                &["Bar", "Block", "Underline"],
                place_of(
                    &[CursorShape::Bar, CursorShape::Block, CursorShape::Underline],
                    &editor.cursor_shape,
                ),
            ),
            Self::CursorBlink => Control::Switch(editor.cursor_blink),
            Self::SoftWrap => Control::Choice(
                &["Off", "Window edge", "Line length"],
                place_of(
                    &[SoftWrap::Off, SoftWrap::Edge, SoftWrap::LineLength],
                    &editor.soft_wrap,
                ),
            ),
            Self::TabSize => Control::Choice(
                TABS,
                match editor.tab_size {
                    None => 0,
                    Some(2) => 1,
                    Some(4) => 2,
                    Some(_) => 3,
                },
            ),
            Self::HardTabs => Control::Choice(
                INDENT,
                match editor.hard_tabs {
                    None => 0,
                    Some(false) => 1,
                    Some(true) => 2,
                },
            ),
            Self::AutoClose => Control::Switch(editor.auto_close),
            Self::AutoSurround => Control::Switch(editor.auto_surround),
            Self::AutoIndent => Control::Switch(editor.auto_indent),
            Self::ExtendComments => Control::Switch(editor.extend_comment_on_newline),
            Self::FormatOnSave => Control::Switch(editor.format_on_save),
            Self::TrimWhitespace => Control::Switch(editor.remove_trailing_whitespace_on_save),
            Self::FinalNewline => Control::Switch(editor.ensure_final_newline_on_save),
            Self::Autosave => Control::Switch(editor.autosave),
            Self::CompletionsOnInput => Control::Switch(editor.completions_on_input),
            Self::CompletionDocs => Control::Switch(editor.completion_documentation),
            Self::Hover => Control::Switch(editor.hover),
            Self::HoverDelay => Control::Number(format!("{} ms", editor.hover_delay)),
            Self::SignatureHelp => Control::Switch(editor.signature_help),
            Self::Diagnostics => Control::Choice(
                PROBLEMS,
                place_of(
                    &[
                        DiagnosticsShown::All,
                        DiagnosticsShown::Warnings,
                        DiagnosticsShown::Errors,
                        DiagnosticsShown::Off,
                    ],
                    &editor.diagnostics,
                ),
            ),
            Self::SearchWrap => Control::Switch(editor.search_wrap),
            Self::SeedSearch => Control::Switch(editor.seed_search_from_selection),
            Self::Smartcase => Control::Switch(editor.smartcase),
            Self::ScrollBeyond => Control::Switch(editor.scroll_beyond_last_line),
            Self::ScrollMargin => Control::Number(editor.vertical_scroll_margin.to_string()),
            Self::ScrollbarMarks => Control::Switch(editor.scrollbar_marks),
            Self::VimMode => Control::Switch(editor.vim_mode),
            Self::VimClipboard => Control::Choice(
                &["Always", "On yank", "Never"],
                place_of(
                    &[
                        VimClipboard::Always,
                        VimClipboard::OnYank,
                        VimClipboard::Never,
                    ],
                    &editor.vim_clipboard,
                ),
            ),
            Self::VimReplayTyping => Control::Switch(editor.vim_replay_typing),
            Self::VimSyntaxObjects => Control::Switch(editor.vim_syntax_objects),
            Self::VimBindings => Control::List(self.lines(state)),
            Self::AutoFindInSelection => Control::Choice(
                &["Never", "Always", "Multiline"],
                place_of(
                    &[
                        FindInSelection::Never,
                        FindInSelection::Always,
                        FindInSelection::Multiline,
                    ],
                    &editor.auto_find_in_selection,
                ),
            ),
            Self::DragAndDrop => Control::Switch(editor.drag_and_drop),
            Self::Lightbulb => Control::Choice(
                &["Off", "Caret line", "Problem lines"],
                place_of(
                    &[
                        LightbulbShown::Off,
                        LightbulbShown::CaretLine,
                        LightbulbShown::Problems,
                    ],
                    &editor.lightbulb,
                ),
            ),
            Self::Ligatures => Control::Switch(editor.ligatures),
            Self::VisualCaret => Control::Switch(editor.visual_caret_movement),
            Self::CompareIgnoreWhitespace => Control::Switch(editor.compare_ignore_whitespace),
            Self::Language => Control::Picker(
                state
                    .settings_language()
                    .and_then(|id| {
                        state
                            .installed_languages()
                            .into_iter()
                            .find(|(held, _)| *held == id)
                            .map(|(_, name)| name)
                    })
                    .unwrap_or_else(|| "None installed".to_owned()),
            ),
            Self::LanguageTabSize => Control::Choice(
                LANGUAGE_TABS,
                match language.tab_size {
                    None => 0,
                    Some(2) => 1,
                    Some(4) => 2,
                    Some(_) => 3,
                },
            ),
            Self::LanguageIndent => Control::Choice(
                LANGUAGE_INDENT,
                match language.hard_tabs {
                    None => 0,
                    Some(false) => 1,
                    Some(true) => 2,
                },
            ),
            Self::LanguageFormat => Control::Choice(
                LANGUAGE_FORMAT,
                match language.format_on_save {
                    None => 0,
                    Some(true) => 1,
                    Some(false) => 2,
                },
            ),
            Self::LanguageOrganize => Control::Choice(
                LANGUAGE_FORMAT,
                match language.organize_imports_on_save {
                    None => 0,
                    Some(true) => 1,
                    Some(false) => 2,
                },
            ),
            Self::LanguageFixAll => Control::Choice(
                LANGUAGE_FORMAT,
                match language.fix_all_on_save {
                    None => 0,
                    Some(true) => 1,
                    Some(false) => 2,
                },
            ),
            Self::OrganizeOnSave => Control::Switch(editor.organize_imports_on_save),
            Self::FixAllOnSave => Control::Switch(editor.fix_all_on_save),
            Self::SemanticRangeLines => Control::Number(if editor.semantic_range_lines == 0 {
                "Never".to_owned()
            } else {
                editor.semantic_range_lines.to_string()
            }),
            Self::SemanticRangeMargin => Control::Number(editor.semantic_range_margin.to_string()),
            Self::WorkspaceRepositories => Control::Switch(machine.workspace.attach_repositories),
            Self::WorkspaceFolders => Control::Text("e.g. ../shared"),
            Self::LanguageServer => Control::Switch(!language.server_off),
            Self::LanguageServerCommand => Control::Text("The one it comes with"),
            Self::LanguageServers => Control::Text("The ones it comes with"),
            Self::BaseKeymap => Control::Choice(PRESETS, place_of(&Preset::ALL, &editor.keymap)),
            Self::Shell => Control::Text("Your login shell"),
            Self::ShellArguments => Control::Text("None"),
            Self::TerminalFontSize => Control::Number(format!(
                "{} px",
                window.terminal.font_size.unwrap_or(editor.font_size)
            )),
            Self::TerminalCursor => Control::Choice(
                &["Block", "Bar", "Underline"],
                place_of(
                    &[
                        TerminalCursor::Block,
                        TerminalCursor::Bar,
                        TerminalCursor::Underline,
                    ],
                    &window.terminal.cursor,
                ),
            ),
            Self::Scrollback => Control::Number(window.terminal.scrollback.to_string()),
            Self::CopyOnSelect => Control::Switch(window.terminal.copy_on_select),
            Self::AutoFetch => Control::Switch(machine.git.auto_fetch),
            Self::FetchInterval => Control::Number(format!("{} min", machine.git.fetch_minutes)),
            Self::LocalHistory => Control::Switch(machine.history.enabled),
            Self::HistoryDays => Control::Number(format!("{} days", machine.history.days)),
            Self::HistorySize => Control::Number(format!("{} MB", machine.history.megabytes)),
            Self::BranchTemplate => Control::Text("No branch, e.g. pm/{title}"),
            Self::WorktreeFolder => Control::Text("Inside each project"),
            Self::WorktreeInclude => Control::List(self.lines(state)),
            Self::GitBlame => Control::Choice(
                FOLLOW,
                place_of(&[GitMarks::FollowEditor, GitMarks::Off], &window.git.blame),
            ),
            Self::GitGutterMarks => Control::Choice(
                FOLLOW,
                place_of(&[GitMarks::FollowEditor, GitMarks::Off], &window.git.gutter),
            ),
            Self::GitWordDiff => Control::Switch(window.git.word_diff),
            Self::GitHosts => Control::Text("e.g. git.example.com=gitlab"),
            Self::DefaultAgent => {
                let options = self.options(state);
                let picked = self.picked(state).unwrap_or(0);
                Control::Picker(options.get(picked).cloned().unwrap_or_default())
            }
            Self::PermissionMode => Control::Choice(
                MODES,
                place_of(&MODE_IDS, &machine.agents.permission_mode.as_deref()),
            ),
            Self::OnFinish => Control::Choice(
                &["Nothing", "Notify", "Open review"],
                place_of(
                    &[Finish::Nothing, Finish::Notify, Finish::OpenReview],
                    &window.agents.on_finish,
                ),
            ),
            Self::Notification => Control::Choice(
                &["Sound", "Desktop", "None"],
                place_of(
                    &[
                        Notification::Sound,
                        Notification::Desktop,
                        Notification::None,
                    ],
                    &window.agents.notification,
                ),
            ),
            Self::DefaultServer => Control::Picker(
                window
                    .servers
                    .default_target()
                    .map_or_else(|| THIS_MACHINE.to_owned(), |server| server.name.clone()),
            ),
        }
    }

    /// How the language the languages page shows is handled.
    fn language_preferences(self, state: &WorkspaceState) -> pm_protocol::LanguagePreferences {
        state
            .settings_language()
            .map(|id| state.machine.language(&id))
            .unwrap_or_default()
    }

    /// Changes `state` as pressing its control at `option` does: a switch flips, a choice is picked, and a number goes down for the first and up for the second.
    pub(crate) fn press(self, state: &mut WorkspaceState, option: usize) {
        let language = state.settings_language();
        let editor = &mut state.editor;
        let window = &mut state.window;
        let machine = &mut state.machine;
        match self {
            Self::Launch => window.general.launch = [Launch::Restore, Launch::Fresh][option.min(1)],
            Self::ConfirmClose => window.general.confirm_close ^= true,
            Self::CheckUpdates => window.general.check_updates ^= true,
            Self::OfferLanguages => window.general.offer_languages ^= true,
            Self::EmbeddedBrowser => window.general.embedded_browser ^= true,
            Self::OpenDevServers => window.general.open_dev_servers ^= true,
            Self::Channel => {
                window.general.channel = [Channel::Stable, Channel::Preview][option.min(1)];
            }
            Self::ThemeMode => {
                let modes = [ThemeMode::System, ThemeMode::Light, ThemeMode::Dark];
                window.appearance.theme_mode = chosen(&modes, option);
            }
            Self::InterfaceSize => {
                window.appearance.interface_size =
                    stepped(window.appearance.interface_size.into(), 1, option, 10, 20) as u32;
            }
            Self::CodeSize => {
                editor.font_size = stepped(editor.font_size.into(), 1, option, 9, 28) as u32
            }
            Self::LineHeight => {
                editor.line_height = stepped(editor.line_height.into(), 5, option, 100, 250) as u32;
            }
            Self::Density => {
                window.appearance.density = [Density::Comfortable, Density::Compact][option.min(1)];
            }
            Self::LineNumbers => editor.line_numbers ^= true,
            Self::RelativeLineNumbers => editor.relative_line_numbers ^= true,
            Self::CurrentLine => editor.current_line_highlight ^= true,
            Self::IndentGuides => editor.indent_guides ^= true,
            Self::WrapGuide => editor.wrap_guide ^= true,
            Self::LineLength => {
                editor.preferred_line_length =
                    stepped(editor.preferred_line_length as i64, 10, option, 40, 200) as usize;
            }
            Self::GitGutter => editor.git_gutter ^= true,
            Self::InlineBlame => editor.inline_blame ^= true,
            Self::Occurrences => editor.occurrence_highlights ^= true,
            Self::BracketColors => editor.bracket_colors ^= true,
            Self::RedactPrivateValues => editor.redact_private_values ^= true,
            Self::DependencyHints => editor.dependency_hints ^= true,
            Self::SemanticHighlighting => editor.semantic_highlighting ^= true,
            Self::InlayHints => editor.inlay_hints ^= true,
            Self::CodeLens => editor.code_lens ^= true,
            Self::Minimap => editor.minimap ^= true,
            Self::StickyScroll => editor.sticky_scroll ^= true,
            Self::FormatOnType => editor.format_on_type ^= true,
            Self::LinkedEditing => editor.linked_editing ^= true,
            Self::SnippetsOnTab => editor.snippets_on_tab ^= true,
            Self::Emmet => editor.emmet ^= true,
            Self::CursorShape => {
                let shapes = [CursorShape::Bar, CursorShape::Block, CursorShape::Underline];
                editor.cursor_shape = chosen(&shapes, option);
            }
            Self::CursorBlink => editor.cursor_blink ^= true,
            Self::SoftWrap => {
                let wraps = [SoftWrap::Off, SoftWrap::Edge, SoftWrap::LineLength];
                editor.soft_wrap = chosen(&wraps, option);
            }
            Self::TabSize => editor.tab_size = [None, Some(2), Some(4), Some(8)][option.min(3)],
            Self::HardTabs => editor.hard_tabs = [None, Some(false), Some(true)][option.min(2)],
            Self::AutoClose => editor.auto_close ^= true,
            Self::AutoSurround => editor.auto_surround ^= true,
            Self::AutoIndent => editor.auto_indent ^= true,
            Self::ExtendComments => editor.extend_comment_on_newline ^= true,
            Self::FormatOnSave => editor.format_on_save ^= true,
            Self::TrimWhitespace => editor.remove_trailing_whitespace_on_save ^= true,
            Self::FinalNewline => editor.ensure_final_newline_on_save ^= true,
            Self::Autosave => editor.autosave ^= true,
            Self::CompletionsOnInput => editor.completions_on_input ^= true,
            Self::CompletionDocs => editor.completion_documentation ^= true,
            Self::Hover => editor.hover ^= true,
            Self::HoverDelay => {
                editor.hover_delay = stepped(editor.hover_delay as i64, 50, option, 0, 2000) as u64;
            }
            Self::SignatureHelp => editor.signature_help ^= true,
            Self::Diagnostics => {
                editor.diagnostics = [
                    DiagnosticsShown::All,
                    DiagnosticsShown::Warnings,
                    DiagnosticsShown::Errors,
                    DiagnosticsShown::Off,
                ][option.min(3)];
            }
            Self::SearchWrap => editor.search_wrap ^= true,
            Self::SeedSearch => editor.seed_search_from_selection ^= true,
            Self::Smartcase => editor.smartcase ^= true,
            Self::ScrollBeyond => editor.scroll_beyond_last_line ^= true,
            Self::ScrollMargin => {
                editor.vertical_scroll_margin =
                    stepped(editor.vertical_scroll_margin as i64, 1, option, 0, 20) as usize;
            }
            Self::ScrollbarMarks => editor.scrollbar_marks ^= true,
            Self::VimMode => editor.vim_mode ^= true,
            Self::VimReplayTyping => editor.vim_replay_typing ^= true,
            Self::VimSyntaxObjects => editor.vim_syntax_objects ^= true,
            Self::VimClipboard => {
                let sharing = [
                    VimClipboard::Always,
                    VimClipboard::OnYank,
                    VimClipboard::Never,
                ];
                editor.vim_clipboard = chosen(&sharing, option);
            }
            Self::OrganizeOnSave => editor.organize_imports_on_save ^= true,
            Self::FixAllOnSave => editor.fix_all_on_save ^= true,
            Self::SemanticRangeLines => {
                editor.semantic_range_lines =
                    stepped(editor.semantic_range_lines as i64, 1000, option, 0, 100_000) as usize;
            }
            Self::SemanticRangeMargin => {
                editor.semantic_range_margin =
                    stepped(editor.semantic_range_margin as i64, 50, option, 0, 2000) as usize;
            }
            Self::WorkspaceRepositories => machine.workspace.attach_repositories ^= true,
            Self::AutoFindInSelection => {
                editor.auto_find_in_selection = chosen(
                    &[
                        FindInSelection::Never,
                        FindInSelection::Always,
                        FindInSelection::Multiline,
                    ],
                    option,
                );
            }
            Self::DragAndDrop => editor.drag_and_drop ^= true,
            Self::Lightbulb => {
                editor.lightbulb = chosen(
                    &[
                        LightbulbShown::Off,
                        LightbulbShown::CaretLine,
                        LightbulbShown::Problems,
                    ],
                    option,
                );
            }
            Self::Ligatures => editor.ligatures ^= true,
            Self::VisualCaret => editor.visual_caret_movement ^= true,
            Self::CompareIgnoreWhitespace => editor.compare_ignore_whitespace ^= true,
            Self::LanguageTabSize
            | Self::LanguageIndent
            | Self::LanguageFormat
            | Self::LanguageOrganize
            | Self::LanguageFixAll
            | Self::LanguageServer => {
                let Some(id) = language else {
                    return;
                };
                let mut held = machine.language(&id);
                match self {
                    Self::LanguageTabSize => {
                        held.tab_size = [None, Some(2), Some(4), Some(8)][option.min(3)];
                    }
                    Self::LanguageIndent => {
                        held.hard_tabs = [None, Some(false), Some(true)][option.min(2)];
                    }
                    Self::LanguageFormat => {
                        held.format_on_save = [None, Some(true), Some(false)][option.min(2)];
                    }
                    Self::LanguageOrganize => {
                        held.organize_imports_on_save =
                            [None, Some(true), Some(false)][option.min(2)];
                    }
                    Self::LanguageFixAll => {
                        held.fix_all_on_save = [None, Some(true), Some(false)][option.min(2)];
                    }
                    _ => held.server_off ^= true,
                }
                machine.set_language(&id, held);
            }
            Self::BaseKeymap => editor.keymap = chosen(&Preset::ALL, option),
            Self::TerminalFontSize => {
                let now = window.terminal.font_size.unwrap_or(editor.font_size);
                window.terminal.font_size = Some(stepped(now.into(), 1, option, 8, 28) as u32);
            }
            Self::TerminalCursor => {
                let cursors = [
                    TerminalCursor::Block,
                    TerminalCursor::Bar,
                    TerminalCursor::Underline,
                ];
                window.terminal.cursor = chosen(&cursors, option);
            }
            Self::Scrollback => {
                window.terminal.scrollback = stepped(
                    window.terminal.scrollback as i64,
                    1000,
                    option,
                    1000,
                    100_000,
                ) as usize;
            }
            Self::CopyOnSelect => window.terminal.copy_on_select ^= true,
            Self::AutoFetch => machine.git.auto_fetch ^= true,
            Self::FetchInterval => {
                machine.git.fetch_minutes =
                    stepped(machine.git.fetch_minutes.into(), 5, option, 1, 240) as u32;
            }
            Self::LocalHistory => machine.history.enabled ^= true,
            Self::HistoryDays => {
                machine.history.days =
                    stepped(machine.history.days.into(), 5, option, 1, 3650) as u32;
            }
            Self::HistorySize => {
                machine.history.megabytes =
                    stepped(machine.history.megabytes.into(), 64, option, 16, 65_536) as u32;
            }
            Self::GitBlame => {
                window.git.blame = [GitMarks::FollowEditor, GitMarks::Off][option.min(1)]
            }
            Self::GitGutterMarks => {
                window.git.gutter = [GitMarks::FollowEditor, GitMarks::Off][option.min(1)];
            }
            Self::GitWordDiff => window.git.word_diff ^= true,
            Self::PermissionMode => {
                machine.agents.permission_mode = chosen(&MODE_IDS, option).map(str::to_owned);
            }
            Self::OnFinish => {
                let finishes = [Finish::Nothing, Finish::Notify, Finish::OpenReview];
                window.agents.on_finish = chosen(&finishes, option);
            }
            Self::Notification => {
                let ways = [
                    Notification::Sound,
                    Notification::Desktop,
                    Notification::None,
                ];
                window.agents.notification = chosen(&ways, option);
            }
            Self::LightTheme
            | Self::DarkTheme
            | Self::InterfaceFont
            | Self::CodeFont
            | Self::Language
            | Self::LanguageServerCommand
            | Self::LanguageServers
            | Self::Shell
            | Self::ShellArguments
            | Self::BranchTemplate
            | Self::WorktreeFolder
            | Self::WorktreeInclude
            | Self::VimBindings
            | Self::GitHosts
            | Self::WorkspaceFolders
            | Self::DefaultAgent
            | Self::DefaultServer => {}
        }
    }

    /// What its list of choices holds, for a setting picked from a list.
    pub(crate) fn options(self, state: &WorkspaceState) -> Vec<String> {
        let fonts = |families: &[String]| {
            std::iter::once(DEFAULT_FONT.to_owned())
                .chain(families.iter().cloned())
                .collect()
        };
        match self {
            Self::LightTheme | Self::DarkTheme => pm_theme::families()
                .iter()
                .map(|family| family.name.to_owned())
                .collect(),
            Self::InterfaceFont => fonts(&state.settings_page.fonts),
            Self::CodeFont => fonts(&state.settings_page.mono_fonts),
            Self::Language => state
                .installed_languages()
                .into_iter()
                .map(|(_, name)| name)
                .collect(),
            Self::DefaultAgent => state
                .settings_servers
                .agents
                .iter()
                .map(|agent| agent.name.clone())
                .collect(),
            Self::DefaultServer => std::iter::once(THIS_MACHINE.to_owned())
                .chain(
                    state
                        .window
                        .servers
                        .known
                        .iter()
                        .map(|server| server.name.clone()),
                )
                .collect(),
            _ => Vec::new(),
        }
    }

    /// The place of the choice picked among its options, for a setting picked from a list.
    pub(crate) fn picked(self, state: &WorkspaceState) -> Option<usize> {
        let options = self.options(state);
        let named = |name: &str| options.iter().position(|held| held == name);
        match self {
            Self::LightTheme => named(&state.window.appearance.light_theme),
            Self::DarkTheme => named(&state.window.appearance.dark_theme),
            Self::InterfaceFont => named(&font_shown(&state.window.appearance.interface_font)),
            Self::CodeFont => named(&font_shown(&state.window.appearance.code_font)),
            Self::Language => {
                let id = state.settings_language()?;
                state
                    .installed_languages()
                    .iter()
                    .position(|(held, _)| *held == id)
            }
            Self::DefaultAgent => match state.machine.agents.default_agent.as_deref() {
                Some(id) => state
                    .settings_servers
                    .agents
                    .iter()
                    .position(|agent| agent.id == id),
                None => Some(0),
            },
            Self::DefaultServer => match state.window.servers.default_target() {
                Some(server) => named(&server.name),
                None => Some(0),
            },
            _ => None,
        }
    }

    /// Picks the choice at `index` of its options, for a setting picked from a list.
    pub(crate) fn pick(self, state: &mut WorkspaceState, index: usize) {
        let Some(name) = self.options(state).get(index).cloned() else {
            return;
        };
        let font = |name: String| {
            if name == DEFAULT_FONT {
                String::new()
            } else {
                name
            }
        };
        match self {
            Self::LightTheme => state.window.appearance.light_theme = name,
            Self::DarkTheme => state.window.appearance.dark_theme = name,
            Self::InterfaceFont => state.window.appearance.interface_font = font(name),
            Self::CodeFont => state.window.appearance.code_font = font(name),
            Self::Language => {
                state.settings_page.language = state
                    .installed_languages()
                    .get(index)
                    .map(|(id, _)| id.clone());
                state
                    .settings_page
                    .fields
                    .remove(&Self::LanguageServerCommand);
                state.settings_page.fields.remove(&Self::LanguageServers);
                state.fill_fields();
            }
            Self::DefaultAgent => {
                state.machine.agents.default_agent = state
                    .settings_servers
                    .agents
                    .get(index)
                    .map(|agent| agent.id.clone());
                if let Some(position) = state
                    .machine
                    .agents
                    .default_agent
                    .as_deref()
                    .and_then(|id| state.agent_position(id))
                {
                    state.agent = position;
                }
            }
            Self::DefaultServer => {
                state.window.servers.default_server = (index > 0).then_some(name);
            }
            _ => {}
        }
    }

    /// Whether it is written as text.
    pub(crate) fn is_text(self) -> bool {
        matches!(
            self,
            Self::LanguageServerCommand
                | Self::LanguageServers
                | Self::Shell
                | Self::ShellArguments
                | Self::BranchTemplate
                | Self::WorktreeFolder
                | Self::GitHosts
                | Self::WorkspaceFolders
        )
    }

    /// What the button that adds a line to its list says, for a setting written as a list.
    pub(crate) fn add_label(self) -> &'static str {
        match self {
            Self::VimBindings => "Add Binding",
            _ => "Add Pattern",
        }
    }

    /// What the box that a line of its list is written in says while it is empty, for a setting written as a list.
    pub(crate) fn add_placeholder(self) -> &'static str {
        match self {
            Self::VimBindings => "e.g. gh => Hover",
            _ => "e.g. dist/ or .env.*",
        }
    }

    /// The lines of the list it holds, for a setting written as a list.
    pub(crate) fn lines(self, state: &WorkspaceState) -> Vec<String> {
        match self {
            Self::WorktreeInclude => state
                .machine
                .git
                .worktree_include
                .split([',', '\n'])
                .map(str::trim)
                .filter(|line| !line.is_empty())
                .map(str::to_owned)
                .collect(),
            Self::VimBindings => state.editor.vim_bindings.clone(),
            _ => Vec::new(),
        }
    }

    /// Has it hold `lines`, for a setting written as a list.
    pub(crate) fn set_lines(self, state: &mut WorkspaceState, lines: &[String]) {
        match self {
            Self::WorktreeInclude => state.machine.git.worktree_include = lines.join(", "),
            Self::VimBindings => state.editor.vim_bindings = lines.to_vec(),
            _ => {}
        }
    }

    /// The text it holds, for a setting written as text.
    pub(crate) fn text(self, state: &WorkspaceState) -> String {
        let machine = &state.machine;
        match self {
            Self::LanguageServerCommand => self.language_preferences(state).server_command,
            Self::LanguageServers => self.language_preferences(state).servers.join(", "),
            Self::Shell => machine.shell.program.clone(),
            Self::ShellArguments => machine.shell.arguments.clone(),
            Self::BranchTemplate => machine.git.branch_template.clone(),
            Self::WorktreeFolder => machine.git.worktree_root.clone(),
            Self::GitHosts => machine.git.hosts_text(),
            Self::WorkspaceFolders => machine.workspace.folders_text(),
            _ => String::new(),
        }
    }

    /// Has it hold `text`, for a setting written as text.
    pub(crate) fn set_text(self, state: &mut WorkspaceState, text: String) {
        let language = state.settings_language();
        let machine = &mut state.machine;
        match self {
            Self::LanguageServerCommand => {
                if let Some(id) = language {
                    let mut held = machine.language(&id);
                    held.server_command = text;
                    machine.set_language(&id, held);
                }
            }
            Self::LanguageServers => {
                if let Some(id) = language {
                    let mut held = machine.language(&id);
                    held.servers = text
                        .split([',', '\n'])
                        .map(|name| name.trim().to_owned())
                        .filter(|name| !name.is_empty())
                        .collect();
                    machine.set_language(&id, held);
                }
            }
            Self::Shell => machine.shell.program = text,
            Self::ShellArguments => machine.shell.arguments = text,
            Self::BranchTemplate => machine.git.branch_template = text,
            Self::WorktreeFolder => machine.git.worktree_root = text,
            Self::GitHosts => machine.git.set_hosts_text(&text),
            Self::WorkspaceFolders => machine.workspace.set_folders_text(&text),
            _ => {}
        }
    }

    /// Copies it from `from` into `into`; a setting of one language is copied for the language `language`.
    pub(crate) fn take(self, into: &mut Kept, from: &Kept, language: Option<&str>) {
        let (editor, source) = (&mut into.editor, &from.editor);
        let (window, own) = (&mut into.window, &from.window);
        let (machine, kept) = (&mut into.machine, &from.machine);
        match self {
            Self::Launch => window.general.launch = own.general.launch,
            Self::ConfirmClose => window.general.confirm_close = own.general.confirm_close,
            Self::CheckUpdates => window.general.check_updates = own.general.check_updates,
            Self::OfferLanguages => {
                window.general.offer_languages = own.general.offer_languages;
            }
            Self::EmbeddedBrowser => {
                window.general.embedded_browser = own.general.embedded_browser;
            }
            Self::OpenDevServers => {
                window.general.open_dev_servers = own.general.open_dev_servers;
            }
            Self::Channel => window.general.channel = own.general.channel,
            Self::ThemeMode => window.appearance.theme_mode = own.appearance.theme_mode,
            Self::LightTheme => window
                .appearance
                .light_theme
                .clone_from(&own.appearance.light_theme),
            Self::DarkTheme => window
                .appearance
                .dark_theme
                .clone_from(&own.appearance.dark_theme),
            Self::InterfaceFont => {
                window
                    .appearance
                    .interface_font
                    .clone_from(&own.appearance.interface_font);
            }
            Self::InterfaceSize => window.appearance.interface_size = own.appearance.interface_size,
            Self::CodeFont => window
                .appearance
                .code_font
                .clone_from(&own.appearance.code_font),
            Self::CodeSize => editor.font_size = source.font_size,
            Self::LineHeight => editor.line_height = source.line_height,
            Self::Density => window.appearance.density = own.appearance.density,
            Self::LineNumbers => editor.line_numbers = source.line_numbers,
            Self::RelativeLineNumbers => {
                editor.relative_line_numbers = source.relative_line_numbers
            }
            Self::CurrentLine => editor.current_line_highlight = source.current_line_highlight,
            Self::IndentGuides => editor.indent_guides = source.indent_guides,
            Self::WrapGuide => editor.wrap_guide = source.wrap_guide,
            Self::LineLength => editor.preferred_line_length = source.preferred_line_length,
            Self::GitGutter => editor.git_gutter = source.git_gutter,
            Self::InlineBlame => editor.inline_blame = source.inline_blame,
            Self::Occurrences => editor.occurrence_highlights = source.occurrence_highlights,
            Self::BracketColors => editor.bracket_colors = source.bracket_colors,
            Self::RedactPrivateValues => {
                editor.redact_private_values = source.redact_private_values
            }
            Self::DependencyHints => editor.dependency_hints = source.dependency_hints,
            Self::SemanticHighlighting => {
                editor.semantic_highlighting = source.semantic_highlighting
            }
            Self::InlayHints => editor.inlay_hints = source.inlay_hints,
            Self::CodeLens => editor.code_lens = source.code_lens,
            Self::Minimap => editor.minimap = source.minimap,
            Self::StickyScroll => editor.sticky_scroll = source.sticky_scroll,
            Self::FormatOnType => editor.format_on_type = source.format_on_type,
            Self::LinkedEditing => editor.linked_editing = source.linked_editing,
            Self::SnippetsOnTab => editor.snippets_on_tab = source.snippets_on_tab,
            Self::Emmet => editor.emmet = source.emmet,
            Self::CursorShape => editor.cursor_shape = source.cursor_shape,
            Self::CursorBlink => editor.cursor_blink = source.cursor_blink,
            Self::SoftWrap => editor.soft_wrap = source.soft_wrap,
            Self::TabSize => editor.tab_size = source.tab_size,
            Self::HardTabs => editor.hard_tabs = source.hard_tabs,
            Self::AutoClose => editor.auto_close = source.auto_close,
            Self::AutoSurround => editor.auto_surround = source.auto_surround,
            Self::AutoIndent => editor.auto_indent = source.auto_indent,
            Self::ExtendComments => {
                editor.extend_comment_on_newline = source.extend_comment_on_newline;
            }
            Self::FormatOnSave => editor.format_on_save = source.format_on_save,
            Self::OrganizeOnSave => {
                editor.organize_imports_on_save = source.organize_imports_on_save;
            }
            Self::FixAllOnSave => editor.fix_all_on_save = source.fix_all_on_save,
            Self::SemanticRangeLines => editor.semantic_range_lines = source.semantic_range_lines,
            Self::SemanticRangeMargin => {
                editor.semantic_range_margin = source.semantic_range_margin;
            }
            Self::WorkspaceRepositories => {
                machine.workspace.attach_repositories = kept.workspace.attach_repositories;
            }
            Self::WorkspaceFolders => machine
                .workspace
                .folders
                .clone_from(&kept.workspace.folders),
            Self::TrimWhitespace => {
                editor.remove_trailing_whitespace_on_save =
                    source.remove_trailing_whitespace_on_save;
            }
            Self::FinalNewline => {
                editor.ensure_final_newline_on_save = source.ensure_final_newline_on_save;
            }
            Self::Autosave => editor.autosave = source.autosave,
            Self::CompletionsOnInput => editor.completions_on_input = source.completions_on_input,
            Self::CompletionDocs => {
                editor.completion_documentation = source.completion_documentation;
            }
            Self::Hover => editor.hover = source.hover,
            Self::HoverDelay => editor.hover_delay = source.hover_delay,
            Self::SignatureHelp => editor.signature_help = source.signature_help,
            Self::Diagnostics => editor.diagnostics = source.diagnostics,
            Self::SearchWrap => editor.search_wrap = source.search_wrap,
            Self::SeedSearch => {
                editor.seed_search_from_selection = source.seed_search_from_selection;
            }
            Self::Smartcase => editor.smartcase = source.smartcase,
            Self::ScrollBeyond => editor.scroll_beyond_last_line = source.scroll_beyond_last_line,
            Self::ScrollMargin => editor.vertical_scroll_margin = source.vertical_scroll_margin,
            Self::ScrollbarMarks => editor.scrollbar_marks = source.scrollbar_marks,
            Self::VimMode => editor.vim_mode = source.vim_mode,
            Self::VimReplayTyping => editor.vim_replay_typing = source.vim_replay_typing,
            Self::VimSyntaxObjects => editor.vim_syntax_objects = source.vim_syntax_objects,
            Self::VimClipboard => editor.vim_clipboard = source.vim_clipboard,
            Self::VimBindings => editor.vim_bindings.clone_from(&source.vim_bindings),
            Self::AutoFindInSelection => {
                editor.auto_find_in_selection = source.auto_find_in_selection;
            }
            Self::DragAndDrop => editor.drag_and_drop = source.drag_and_drop,
            Self::Lightbulb => editor.lightbulb = source.lightbulb,
            Self::Ligatures => editor.ligatures = source.ligatures,
            Self::VisualCaret => editor.visual_caret_movement = source.visual_caret_movement,
            Self::CompareIgnoreWhitespace => {
                editor.compare_ignore_whitespace = source.compare_ignore_whitespace;
            }
            Self::Language => {}
            Self::LanguageTabSize
            | Self::LanguageIndent
            | Self::LanguageFormat
            | Self::LanguageOrganize
            | Self::LanguageFixAll
            | Self::LanguageServer
            | Self::LanguageServerCommand
            | Self::LanguageServers => {
                let Some(id) = language else {
                    return;
                };
                let (mut held, given) = (machine.language(id), kept.language(id));
                match self {
                    Self::LanguageTabSize => held.tab_size = given.tab_size,
                    Self::LanguageIndent => held.hard_tabs = given.hard_tabs,
                    Self::LanguageFormat => held.format_on_save = given.format_on_save,
                    Self::LanguageOrganize => {
                        held.organize_imports_on_save = given.organize_imports_on_save;
                    }
                    Self::LanguageFixAll => held.fix_all_on_save = given.fix_all_on_save,
                    Self::LanguageServer => held.server_off = given.server_off,
                    Self::LanguageServers => held.servers = given.servers,
                    _ => held.server_command = given.server_command,
                }
                machine.set_language(id, held);
            }
            Self::BaseKeymap => editor.keymap = source.keymap,
            Self::Shell => machine.shell.program.clone_from(&kept.shell.program),
            Self::ShellArguments => machine.shell.arguments.clone_from(&kept.shell.arguments),
            Self::TerminalFontSize => window.terminal.font_size = own.terminal.font_size,
            Self::TerminalCursor => window.terminal.cursor = own.terminal.cursor,
            Self::Scrollback => window.terminal.scrollback = own.terminal.scrollback,
            Self::CopyOnSelect => window.terminal.copy_on_select = own.terminal.copy_on_select,
            Self::AutoFetch => machine.git.auto_fetch = kept.git.auto_fetch,
            Self::FetchInterval => machine.git.fetch_minutes = kept.git.fetch_minutes,
            Self::LocalHistory => machine.history.enabled = kept.history.enabled,
            Self::HistoryDays => machine.history.days = kept.history.days,
            Self::HistorySize => machine.history.megabytes = kept.history.megabytes,
            Self::BranchTemplate => machine
                .git
                .branch_template
                .clone_from(&kept.git.branch_template),
            Self::WorktreeFolder => machine
                .git
                .worktree_root
                .clone_from(&kept.git.worktree_root),
            Self::WorktreeInclude => machine
                .git
                .worktree_include
                .clone_from(&kept.git.worktree_include),
            Self::GitBlame => window.git.blame = own.git.blame,
            Self::GitGutterMarks => window.git.gutter = own.git.gutter,
            Self::GitWordDiff => window.git.word_diff = own.git.word_diff,
            Self::GitHosts => machine.git.hosts.clone_from(&kept.git.hosts),
            Self::DefaultAgent => machine
                .agents
                .default_agent
                .clone_from(&kept.agents.default_agent),
            Self::PermissionMode => {
                machine
                    .agents
                    .permission_mode
                    .clone_from(&kept.agents.permission_mode);
            }
            Self::OnFinish => window.agents.on_finish = own.agents.on_finish,
            Self::Notification => window.agents.notification = own.agents.notification,
            Self::DefaultServer => {
                window
                    .servers
                    .default_server
                    .clone_from(&own.servers.default_server);
            }
        }
    }

    /// Whether it stands other than as it comes in `state`.
    pub(crate) fn changed(self, state: &WorkspaceState) -> bool {
        let now = state.kept();
        let mut probe = now.clone();
        let language = state.settings_language();
        self.take(&mut probe, &Kept::defaults(), language.as_deref());
        probe != now
    }

    /// Puts it back as it comes in `state`.
    pub(crate) fn reset(self, state: &mut WorkspaceState) {
        let mut now = state.kept();
        let language = state.settings_language();
        self.take(&mut now, &Kept::defaults(), language.as_deref());
        state.put_back(now);
        state.settings_page.fields.remove(&self);
        state.fill_fields();
    }
}

/// What the picker of fonts shows for the family `name`: its name, or what it calls the one the window ships with.
fn font_shown(name: &str) -> String {
    if name.is_empty() {
        DEFAULT_FONT.to_owned()
    } else {
        name.to_owned()
    }
}
