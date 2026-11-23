//! How each setting stands and changes: the control it is shown with, what pressing that control does, the text of a setting written as text, the choices of one picked from a list, and copying it from one set of settings to another, which is how it is reset and told apart from how it comes.

use pm_config::{
    Channel, Density, Finish, GitMarks, Launch, Notification, SearchEngine, TerminalCursor,
    ThemeMode,
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
const MODES: &[&str] = &["Agent's own", "Ask", "Plan", "Run freely"];

/// The ids the harness gives the modes, in the order of [`MODES`]; nothing for the agent's own.
const MODE_IDS: [Option<&str>; 4] = [None, Some("default"), Some("plan"), Some("yolo")];

/// The efforts a new conversation can start at, as the choices name them.
const EFFORTS: &[&str] = &["Auto", "Low", "Medium", "High", "X-high", "Max"];

/// The ids the harness gives the efforts, in the order of [`EFFORTS`]; nothing to leave it to the harness.
const EFFORT_IDS: [Option<&str>; 6] = [
    None,
    Some("low"),
    Some("medium"),
    Some("high"),
    Some("xhigh"),
    Some("max"),
];

/// The prompt cache lifetimes, as the choices name them.
const CACHES: &[&str] = &["5 minutes", "1 hour"];

/// The ids the harness gives the prompt cache lifetimes, in the order of [`CACHES`].
const CACHE_IDS: [&str; 2] = ["5m", "1h"];

/// The subagent budgets, as the choices name them.
const BUDGETS: &[&str] = &["Off", "$1", "$2", "$5", "$10", "$25"];

/// The values the harness reads the subagent budgets as, in the order of [`BUDGETS`].
const BUDGET_IDS: [&str; 6] = [pm_protocol::BUDGET_OFF, "1", "2", "5", "10", "25"];

/// The presets of keys, as the choices name them, in the order of [`Preset::ALL`].
const PRESETS: &[&str] = &[
    Preset::Pandemonium.title(),
    Preset::VsCode.title(),
    Preset::JetBrains.title(),
    Preset::Sublime.title(),
];

/// The search engines, as the choices name them, in the order of [`SearchEngine::ALL`].
const ENGINES: [&str; 5] = [
    SearchEngine::DuckDuckGo.title(),
    SearchEngine::Google.title(),
    SearchEngine::Bing.title(),
    SearchEngine::Brave.title(),
    SearchEngine::Startpage.title(),
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

/// The tab sizes the choices of a tab size set, in their order; nothing follows another setting.
const TAB_SIZES: [Option<usize>; 4] = [None, Some(2), Some(4), Some(8)];

/// Whether the choices of indentation indent with tabs, in their order; nothing follows another setting.
const INDENTS: [Option<bool>; 3] = [None, Some(false), Some(true)];

/// What the choices that follow the editor, turn on or turn off set, in their order.
const FOLLOWS: [Option<bool>; 3] = [None, Some(true), Some(false)];

/// The place of the tab size `size` among the choices of [`TAB_SIZES`], any size not offered taken as the last.
fn tab_place(size: Option<usize>) -> usize {
    match size {
        None => 0,
        Some(2) => 1,
        Some(4) => 2,
        Some(_) => 3,
    }
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
            Self::AgentOrigins => Control::List(self.lines(state)),
            Self::SearchEngine => Control::Choice(
                &ENGINES,
                place_of(&SearchEngine::ALL, &window.general.search_engine),
            ),
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
            Self::CursorShape => Control::Choice(
                &["Bar", "Block", "Underline"],
                place_of(
                    &[CursorShape::Bar, CursorShape::Block, CursorShape::Underline],
                    &editor.cursor_shape,
                ),
            ),
            Self::SoftWrap => Control::Choice(
                &["Off", "Window edge", "Line length"],
                place_of(
                    &[SoftWrap::Off, SoftWrap::Edge, SoftWrap::LineLength],
                    &editor.soft_wrap,
                ),
            ),
            Self::TabSize => Control::Choice(TABS, tab_place(editor.tab_size)),
            Self::HardTabs => Control::Choice(INDENT, place_of(&INDENTS, &editor.hard_tabs)),
            Self::HoverDelay => Control::Number(format!("{} ms", editor.hover_delay)),
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
            Self::ScrollMargin => Control::Number(editor.vertical_scroll_margin.to_string()),
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
            Self::LanguageTabSize => Control::Choice(LANGUAGE_TABS, tab_place(language.tab_size)),
            Self::LanguageIndent => {
                Control::Choice(LANGUAGE_INDENT, place_of(&INDENTS, &language.hard_tabs))
            }
            Self::LanguageFormat => Control::Choice(
                LANGUAGE_FORMAT,
                place_of(&FOLLOWS, &language.format_on_save),
            ),
            Self::LanguageOrganize => Control::Choice(
                LANGUAGE_FORMAT,
                place_of(&FOLLOWS, &language.organize_imports_on_save),
            ),
            Self::LanguageFixAll => Control::Choice(
                LANGUAGE_FORMAT,
                place_of(&FOLLOWS, &language.fix_all_on_save),
            ),
            Self::SemanticRangeLines => Control::Number(if editor.semantic_range_lines == 0 {
                "Never".to_owned()
            } else {
                editor.semantic_range_lines.to_string()
            }),
            Self::SemanticRangeMargin => Control::Number(editor.semantic_range_margin.to_string()),
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
            Self::FetchInterval => Control::Number(format!("{} min", machine.git.fetch_minutes)),
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
            Self::Effort => Control::Choice(
                EFFORTS,
                place_of(&EFFORT_IDS, &machine.agents.effort.as_deref()),
            ),
            Self::SubagentBudget => Control::Choice(
                BUDGETS,
                place_of(&BUDGET_IDS, &machine.agents.subagent_budget.as_str()),
            ),
            Self::PromptCache => {
                Control::Choice(CACHES, place_of(&CACHE_IDS, &machine.agents.cache.as_str()))
            }
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
            Self::LineLength => Control::Number(editor.preferred_line_length.to_string()),
            _ => Control::Switch(self.flag(&mut state.kept()).is_some_and(|on| *on)),
        }
    }

    /// How the language the languages page shows is handled.
    fn language_preferences(self, state: &WorkspaceState) -> pm_protocol::LanguagePreferences {
        state
            .settings_language()
            .map(|id| state.machine.language(&id))
            .unwrap_or_default()
    }

    /// The switch it is in `kept`, for a setting that is only turned on or off.
    fn flag(self, kept: &mut Kept) -> Option<&mut bool> {
        let (editor, window, machine) = (&mut kept.editor, &mut kept.window, &mut kept.machine);
        Some(match self {
            Self::ConfirmClose => &mut window.general.confirm_close,
            Self::CheckUpdates => &mut window.general.check_updates,
            Self::OfferLanguages => &mut window.general.offer_languages,
            Self::EmbeddedBrowser => &mut window.general.embedded_browser,
            Self::OpenDevServers => &mut window.general.open_dev_servers,
            Self::OpenLocalLinks => &mut window.general.open_local_links,
            Self::BlockTrackers => &mut window.general.block_trackers,
            Self::SavePasswords => &mut window.general.save_passwords,
            Self::ProjectBrowserProfiles => &mut window.general.project_browser_profiles,
            Self::RestrictAgentBrowsing => &mut window.general.restrict_agent_browsing,
            Self::LineNumbers => &mut editor.line_numbers,
            Self::RelativeLineNumbers => &mut editor.relative_line_numbers,
            Self::CurrentLine => &mut editor.current_line_highlight,
            Self::IndentGuides => &mut editor.indent_guides,
            Self::WrapGuide => &mut editor.wrap_guide,
            Self::GitGutter => &mut editor.git_gutter,
            Self::InlineBlame => &mut editor.inline_blame,
            Self::Occurrences => &mut editor.occurrence_highlights,
            Self::BracketColors => &mut editor.bracket_colors,
            Self::RedactPrivateValues => &mut editor.redact_private_values,
            Self::DependencyHints => &mut editor.dependency_hints,
            Self::SemanticHighlighting => &mut editor.semantic_highlighting,
            Self::InlayHints => &mut editor.inlay_hints,
            Self::CodeLens => &mut editor.code_lens,
            Self::Minimap => &mut editor.minimap,
            Self::StickyScroll => &mut editor.sticky_scroll,
            Self::FormatOnType => &mut editor.format_on_type,
            Self::LinkedEditing => &mut editor.linked_editing,
            Self::SnippetsOnTab => &mut editor.snippets_on_tab,
            Self::Emmet => &mut editor.emmet,
            Self::CursorBlink => &mut editor.cursor_blink,
            Self::AutoClose => &mut editor.auto_close,
            Self::AutoSurround => &mut editor.auto_surround,
            Self::AutoIndent => &mut editor.auto_indent,
            Self::ExtendComments => &mut editor.extend_comment_on_newline,
            Self::FormatOnSave => &mut editor.format_on_save,
            Self::TrimWhitespace => &mut editor.remove_trailing_whitespace_on_save,
            Self::FinalNewline => &mut editor.ensure_final_newline_on_save,
            Self::Autosave => &mut editor.autosave,
            Self::CompletionsOnInput => &mut editor.completions_on_input,
            Self::CompletionDocs => &mut editor.completion_documentation,
            Self::Hover => &mut editor.hover,
            Self::SignatureHelp => &mut editor.signature_help,
            Self::SearchWrap => &mut editor.search_wrap,
            Self::SeedSearch => &mut editor.seed_search_from_selection,
            Self::Smartcase => &mut editor.smartcase,
            Self::ScrollBeyond => &mut editor.scroll_beyond_last_line,
            Self::ScrollbarMarks => &mut editor.scrollbar_marks,
            Self::VimMode => &mut editor.vim_mode,
            Self::VimReplayTyping => &mut editor.vim_replay_typing,
            Self::VimSyntaxObjects => &mut editor.vim_syntax_objects,
            Self::DragAndDrop => &mut editor.drag_and_drop,
            Self::Ligatures => &mut editor.ligatures,
            Self::VisualCaret => &mut editor.visual_caret_movement,
            Self::CompareIgnoreWhitespace => &mut editor.compare_ignore_whitespace,
            Self::OrganizeOnSave => &mut editor.organize_imports_on_save,
            Self::FixAllOnSave => &mut editor.fix_all_on_save,
            Self::WorkspaceRepositories => &mut machine.workspace.attach_repositories,
            Self::CopyOnSelect => &mut window.terminal.copy_on_select,
            Self::AutoFetch => &mut machine.git.auto_fetch,
            Self::LocalHistory => &mut machine.history.enabled,
            Self::GitWordDiff => &mut window.git.word_diff,
            Self::Memory => &mut machine.agents.memory,
            Self::Checkpoints => &mut machine.agents.checkpoints,
            Self::Orchestrator => &mut machine.agents.orchestrator,
            Self::CacheWarming => &mut machine.agents.cache_warming,
            Self::PlanAutosave => &mut machine.agents.plan_autosave,
            _ => return None,
        })
    }

    /// Changes `state` as pressing its control at `option` does: a switch flips, a choice is picked, and a number goes down for the first and up for the second.
    pub(crate) fn press(self, state: &mut WorkspaceState, option: usize) {
        let mut kept = state.kept();
        if let Some(flag) = self.flag(&mut kept) {
            *flag ^= true;
            state.put_back(kept);
            return;
        }
        let language = state.settings_language();
        let editor = &mut state.editor;
        let window = &mut state.window;
        let machine = &mut state.machine;
        match self {
            Self::Launch => window.general.launch = [Launch::Restore, Launch::Fresh][option.min(1)],
            Self::SearchEngine => {
                window.general.search_engine = chosen(&SearchEngine::ALL, option);
            }
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
            Self::LineLength => {
                editor.preferred_line_length =
                    stepped(editor.preferred_line_length as i64, 10, option, 40, 200) as usize;
            }
            Self::CursorShape => {
                let shapes = [CursorShape::Bar, CursorShape::Block, CursorShape::Underline];
                editor.cursor_shape = chosen(&shapes, option);
            }
            Self::SoftWrap => {
                let wraps = [SoftWrap::Off, SoftWrap::Edge, SoftWrap::LineLength];
                editor.soft_wrap = chosen(&wraps, option);
            }
            Self::TabSize => editor.tab_size = chosen(&TAB_SIZES, option),
            Self::HardTabs => editor.hard_tabs = chosen(&INDENTS, option),
            Self::HoverDelay => {
                editor.hover_delay = stepped(editor.hover_delay as i64, 50, option, 0, 2000) as u64;
            }
            Self::Diagnostics => {
                editor.diagnostics = [
                    DiagnosticsShown::All,
                    DiagnosticsShown::Warnings,
                    DiagnosticsShown::Errors,
                    DiagnosticsShown::Off,
                ][option.min(3)];
            }
            Self::ScrollMargin => {
                editor.vertical_scroll_margin =
                    stepped(editor.vertical_scroll_margin as i64, 1, option, 0, 20) as usize;
            }
            Self::VimClipboard => {
                let sharing = [
                    VimClipboard::Always,
                    VimClipboard::OnYank,
                    VimClipboard::Never,
                ];
                editor.vim_clipboard = chosen(&sharing, option);
            }
            Self::SemanticRangeLines => {
                editor.semantic_range_lines =
                    stepped(editor.semantic_range_lines as i64, 1000, option, 0, 100_000) as usize;
            }
            Self::SemanticRangeMargin => {
                editor.semantic_range_margin =
                    stepped(editor.semantic_range_margin as i64, 50, option, 0, 2000) as usize;
            }
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
                    Self::LanguageTabSize => held.tab_size = chosen(&TAB_SIZES, option),
                    Self::LanguageIndent => held.hard_tabs = chosen(&INDENTS, option),
                    Self::LanguageFormat => held.format_on_save = chosen(&FOLLOWS, option),
                    Self::LanguageOrganize => {
                        held.organize_imports_on_save = chosen(&FOLLOWS, option);
                    }
                    Self::LanguageFixAll => held.fix_all_on_save = chosen(&FOLLOWS, option),
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
            Self::FetchInterval => {
                machine.git.fetch_minutes =
                    stepped(machine.git.fetch_minutes.into(), 5, option, 1, 240) as u32;
            }
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
            Self::PermissionMode => {
                machine.agents.permission_mode = chosen(&MODE_IDS, option).map(str::to_owned);
            }
            Self::Effort => {
                machine.agents.effort = chosen(&EFFORT_IDS, option).map(str::to_owned);
            }
            Self::SubagentBudget => {
                machine.agents.subagent_budget = chosen(&BUDGET_IDS, option).to_owned();
            }
            Self::PromptCache => machine.agents.cache = chosen(&CACHE_IDS, option).to_owned(),
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
            _ => {}
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
            Self::LightTheme | Self::DarkTheme => crate::theme::families()
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
            Self::AgentOrigins => "Add Origin",
            _ => "Add Pattern",
        }
    }

    /// What the box that a line of its list is written in says while it is empty, for a setting written as a list.
    pub(crate) fn add_placeholder(self) -> &'static str {
        match self {
            Self::VimBindings => "e.g. gh => Hover",
            Self::AgentOrigins => "e.g. https://example.com",
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
            Self::AgentOrigins => state.window.general.agent_origins.clone(),
            _ => Vec::new(),
        }
    }

    /// Has it hold `lines`, for a setting written as a list.
    pub(crate) fn set_lines(self, state: &mut WorkspaceState, lines: &[String]) {
        match self {
            Self::WorktreeInclude => state.machine.git.worktree_include = lines.join(", "),
            Self::VimBindings => state.editor.vim_bindings = lines.to_vec(),
            Self::AgentOrigins => state.window.general.agent_origins = lines.to_vec(),
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
        if let (Some(flag), Some(given)) = (self.flag(into), self.flag(&mut from.clone())) {
            *flag = *given;
            return;
        }
        let (editor, source) = (&mut into.editor, &from.editor);
        let (window, own) = (&mut into.window, &from.window);
        let (machine, kept) = (&mut into.machine, &from.machine);
        match self {
            Self::Launch => window.general.launch = own.general.launch,
            Self::AgentOrigins => window
                .general
                .agent_origins
                .clone_from(&own.general.agent_origins),
            Self::Channel => window.general.channel = own.general.channel,
            Self::SearchEngine => window.general.search_engine = own.general.search_engine,
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
            Self::LineLength => editor.preferred_line_length = source.preferred_line_length,
            Self::CursorShape => editor.cursor_shape = source.cursor_shape,
            Self::SoftWrap => editor.soft_wrap = source.soft_wrap,
            Self::TabSize => editor.tab_size = source.tab_size,
            Self::HardTabs => editor.hard_tabs = source.hard_tabs,
            Self::SemanticRangeLines => editor.semantic_range_lines = source.semantic_range_lines,
            Self::SemanticRangeMargin => {
                editor.semantic_range_margin = source.semantic_range_margin;
            }
            Self::WorkspaceFolders => machine
                .workspace
                .folders
                .clone_from(&kept.workspace.folders),
            Self::HoverDelay => editor.hover_delay = source.hover_delay,
            Self::Diagnostics => editor.diagnostics = source.diagnostics,
            Self::ScrollMargin => editor.vertical_scroll_margin = source.vertical_scroll_margin,
            Self::VimClipboard => editor.vim_clipboard = source.vim_clipboard,
            Self::VimBindings => editor.vim_bindings.clone_from(&source.vim_bindings),
            Self::AutoFindInSelection => {
                editor.auto_find_in_selection = source.auto_find_in_selection;
            }
            Self::Lightbulb => editor.lightbulb = source.lightbulb,
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
            Self::FetchInterval => machine.git.fetch_minutes = kept.git.fetch_minutes,
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
            Self::Effort => machine.agents.effort.clone_from(&kept.agents.effort),
            Self::SubagentBudget => {
                machine
                    .agents
                    .subagent_budget
                    .clone_from(&kept.agents.subagent_budget);
            }
            Self::PromptCache => machine.agents.cache.clone_from(&kept.agents.cache),
            Self::OnFinish => window.agents.on_finish = own.agents.on_finish,
            Self::Notification => window.agents.notification = own.agents.notification,
            Self::DefaultServer => {
                window
                    .servers
                    .default_server
                    .clone_from(&own.servers.default_server);
            }
            _ => {}
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
