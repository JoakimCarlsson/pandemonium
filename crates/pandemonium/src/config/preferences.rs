//! The preferences the editor draws and behaves by, and the messages that
//! change them.
//!
//! Onboarding and the settings pane are two editors of this one value: both
//! build their screens from it, both answer with the same [`Message`], and
//! [`Preferences::apply`] is the one place either of them changes it.

use std::collections::BTreeMap;
use std::ops::RangeInclusive;
use std::path::PathBuf;

use pm_core::Bootstrap;
use pm_text::Indent;
use pm_ui::Appearance;
use serde::{Deserialize, Serialize};

use crate::config::fonts::Fonts;
use crate::config::overrides::ThemeOverrides;
use crate::editor::Display;
use crate::keymap::{self, Action, Changes, DEFAULT_KEYMAP, Keymap, Sequence};
use crate::message::Message;
use crate::theme::{DEFAULT_FAMILY, families};

/// The sizes a font can be set at, in logical pixels.
const FONT_SIZES: RangeInclusive<f32> = 8.0..=40.0;

/// The sizes the interface can be set at, in logical pixels.
const INTERFACE_SIZES: RangeInclusive<f32> = 10.0..=24.0;

/// The weights code can be set in.
const WEIGHTS: RangeInclusive<f32> = 100.0..=900.0;

/// The distances between lines of code, as multiples of its size.
const LINE_HEIGHTS: RangeInclusive<f32> = 1.0..=2.5;

/// How many lines of scrollback a terminal can be asked to keep.
const SCROLLBACKS: RangeInclusive<f32> = 1_000.0..=100_000.0;

/// How wide a step of indentation can be.
const TAB_SIZES: RangeInclusive<f32> = 1.0..=16.0;

/// How far the wheel can be made to scroll, against its usual distance.
const SENSITIVITIES: RangeInclusive<f32> = 0.25..=4.0;

/// Which theme the editor draws in.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum ThemeMode {
    /// Always the light theme.
    Light,
    /// Always the dark theme.
    Dark,
    /// Whichever one the desktop is set to.
    System,
}

impl ThemeMode {
    /// Every mode, in the order a toggle offers them.
    pub const ALL: [Self; 3] = [Self::Light, Self::Dark, Self::System];

    /// The mode's label in the theme toggle.
    pub fn label(self) -> &'static str {
        match self {
            Self::Light => "Light",
            Self::Dark => "Dark",
            Self::System => "System",
        }
    }

    /// The appearance to draw in, given what the system asks for.
    pub fn resolve(self, system: Appearance) -> Appearance {
        match self {
            Self::Light => Appearance::Light,
            Self::Dark => Appearance::Dark,
            Self::System => system,
        }
    }
}

/// How the editor handles a missing installable language server.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum InstallLanguageServers {
    /// Ask before installing.
    #[default]
    Ask,
    /// Install when a language first needs the server.
    Always,
    /// Never offer an automatic install.
    Never,
}

impl InstallLanguageServers {
    /// Choices shown in the settings pane.
    pub const ALL: [Self; 3] = [Self::Ask, Self::Always, Self::Never];

    /// The short label shown in the settings pane.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Ask => "Ask",
            Self::Always => "Always",
            Self::Never => "Never",
        }
    }
}

/// One preference, named so it can be put back to its default on its own.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Preference {
    /// Which theme the editor draws in.
    ThemeMode,
    /// The theme family the editor draws in.
    ThemeFamily,
    /// One colour repainted over an appearance, by index into the tokens.
    ThemeColor(Appearance, usize),
    /// Every colour repainted over an appearance.
    ThemeOverrides(Appearance),
    /// The family prose and labels are set in.
    InterfaceFont,
    /// The body size of the interface.
    InterfaceFontSize,
    /// The family code is set in.
    BufferFont,
    /// The size code is set in.
    BufferFontSize,
    /// The weight code is set in.
    BufferFontWeight,
    /// The distance between two lines of code.
    BufferLineHeight,
    /// The keymap the editor starts from.
    Keymap,
    /// Every binding the reader changed over that keymap.
    Keybindings,
    /// What the reader changed about one action's bindings.
    Binding(Action),
    /// Whether editing starts in vim mode.
    VimMode,
    /// How much vim's unnamed register shares with the system clipboard.
    VimClipboard,
    /// How wide a step of indentation is where a file does not say.
    TabSize,
    /// Whether a step of indentation is a tab where a file does not say.
    HardTabs,
    /// Whether the gutter numbers the lines.
    LineNumbers,
    /// Whether the numbers count away from the cursor's line.
    RelativeLineNumbers,
    /// Whether the cursor's line is washed.
    CurrentLine,
    /// Whether the other places the word at the cursor appears are washed.
    Occurrences,
    /// Whether a line is drawn at every step of indentation.
    IndentGuides,
    /// Whether the lines the view is inside stay pinned above it.
    StickyScroll,
    /// Whether the scrollbars are drawn.
    Scrollbars,
    /// Whether the whole file is drawn in miniature beside the text.
    Minimap,
    /// Whether the file's path and the blocks the cursor is in are named
    /// above the text.
    Breadcrumbs,
    /// Whether a diff sets the old side and the new side beside each other.
    SplitDiff,
    /// The column a guide is drawn down.
    WrapGuide,
    /// Whether a language server's hints are written into the lines.
    InlayHints,
    /// Whether a language server's notes are written after declarations.
    CodeLens,
    /// How the caret is drawn.
    CursorShape,
    /// Whether the caret blinks.
    CursorBlink,
    /// How far a notch of the wheel scrolls.
    ScrollSensitivity,
    /// Whether a file is laid out by its formatter when it is saved.
    FormatOnSave,
    /// Whether the space at the ends of lines goes when a file is saved.
    TrimWhitespace,
    /// Whether a saved file always ends in a line break.
    FinalNewline,
    /// The size a terminal is set in.
    TerminalFontSize,
    /// How many lines of scrollback a terminal keeps.
    TerminalScrollback,
    /// Whether a new session's worktree is trusted without being asked about.
    TrustWorktrees,
    /// How a missing language server is installed.
    InstallLanguageServers,
    /// The paths symlinked into a new session's worktree.
    WorktreeLink,
    /// The paths copied into it.
    WorktreeCopy,
    /// The variable a session's own port is handed to its programs in.
    WorktreePort,
}

/// Which way a number is stepped.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Step {
    /// Towards the smaller end of its range.
    Down,
    /// Towards the larger end.
    Up,
}

/// Which of a new worktree's two lists of paths is meant.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorktreePaths {
    /// The paths symlinked in from the repository.
    Linked,
    /// The paths copied in from it.
    Copied,
}

/// One of the reader's own vim bindings: keys written the way Zed's vim
/// keymap writes them, the action they do and when.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VimBinding {
    /// The keystrokes, apart by spaces: `g h`, `ctrl-w v`.
    pub keys: String,
    /// The action, as Zed names it: `Hover`, `NextWordStart`.
    pub action: String,
    /// When it applies: `normal`, `visual || operator`, `op=d`.
    pub when: String,
}

/// A value the reader chose for one agent knob.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(untagged)]
pub enum KnobValue {
    /// An id from the knob's offered values.
    Picked(String),
    /// Whether a switch is on.
    Switched(bool),
}

/// The last options the reader chose for one agent CLI.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(default)]
pub struct AgentOptions {
    /// The last chosen protocol mode.
    pub mode: Option<String>,
    /// The last chosen value of each knob, by knob id.
    pub knobs: BTreeMap<String, KnobValue>,
}

/// Everything the reader decides about how the editor draws and behaves.
#[derive(Clone, Debug, PartialEq)]
pub struct Preferences {
    /// The last options chosen for each agent CLI, by agent id.
    pub agent_options: BTreeMap<String, AgentOptions>,
    /// Which theme the editor draws in.
    pub theme_mode: ThemeMode,
    /// Index into `pm_ui::families` of the theme family the editor draws in.
    pub theme_family: usize,
    /// The colours repainted over whichever family is chosen.
    pub theme_overrides: ThemeOverrides,
    /// The faces and sizes the window's text is set in.
    pub fonts: Fonts,
    /// Index into `keymap::keymaps` of the keymap the editor starts from.
    pub keymap: usize,
    /// The reader's own bindings, over that keymap.
    pub keybindings: Changes,
    /// Whether editing starts in vim mode.
    pub vim_mode: bool,
    /// How much vim's unnamed register shares with the system clipboard.
    pub vim_clipboard: pm_vim::ClipboardUse,
    /// The reader's own vim bindings, laid over Zed's.
    pub vim_bindings: Vec<VimBinding>,
    /// How wide a step of indentation is where a file does not say.
    pub tab_size: usize,
    /// Whether a step of indentation is a tab where a file does not say.
    pub hard_tabs: bool,
    /// What a pane of text draws around and over its text.
    pub display: Display,
    /// Whether a diff sets the old side and the new side beside each other.
    pub split_diff: bool,
    /// Whether a language server's hints are written into the lines.
    pub inlay_hints: bool,
    /// Whether a language server's notes are written after declarations.
    pub code_lens: bool,
    /// Whether the caret blinks.
    pub cursor_blink: bool,
    /// How far a notch of the wheel scrolls, against its usual distance.
    pub scroll_sensitivity: f32,
    /// Whether a file is laid out the way its formatter would when it is saved.
    pub format_on_save: bool,
    /// Whether the space at the ends of lines goes when a file is saved.
    pub trim_whitespace: bool,
    /// Whether a saved file always ends in a line break.
    pub final_newline: bool,
    /// How many lines of scrollback a terminal keeps.
    pub terminal_scrollback: usize,
    /// Whether a new session's worktree is trusted without being asked about.
    pub trust_worktrees: bool,
    /// How missing language servers are installed.
    pub install_language_servers: InstallLanguageServers,
    /// What a session's fresh worktree is given, git having left it out.
    pub bootstrap: Bootstrap,
}

impl Default for Preferences {
    /// The preferences a first launch starts from.
    fn default() -> Self {
        Self {
            agent_options: BTreeMap::new(),
            theme_mode: ThemeMode::System,
            theme_family: DEFAULT_FAMILY,
            theme_overrides: ThemeOverrides::default(),
            fonts: Fonts::default(),
            keymap: DEFAULT_KEYMAP,
            keybindings: Changes::default(),
            vim_mode: false,
            vim_clipboard: pm_vim::ClipboardUse::default(),
            vim_bindings: Vec::new(),
            tab_size: Indent::default().width,
            hard_tabs: false,
            display: Display::default(),
            split_diff: false,
            inlay_hints: true,
            code_lens: true,
            cursor_blink: true,
            scroll_sensitivity: 1.0,
            format_on_save: false,
            trim_whitespace: false,
            final_newline: false,
            terminal_scrollback: pm_vt::SCROLLBACK,
            trust_worktrees: false,
            install_language_servers: InstallLanguageServers::Ask,
            bootstrap: Bootstrap::default(),
        }
    }
}

/// Declares [`Preferences::flag`] and [`Preferences::flag_mut`] from the
/// preferences that are a switch, and the field each one is.
macro_rules! flags {
    ($($preference:ident => $($field:ident).+),* $(,)?) => {
        impl Preferences {
            /// Whether `preference` is on, when it is a switch.
            pub fn flag(&self, preference: Preference) -> Option<bool> {
                match preference {
                    $(Preference::$preference => Some(self.$($field).+),)*
                    _ => None,
                }
            }

            /// The switch `preference` is, to flip.
            fn flag_mut(&mut self, preference: Preference) -> Option<&mut bool> {
                match preference {
                    $(Preference::$preference => Some(&mut self.$($field).+),)*
                    _ => None,
                }
            }
        }
    };
}

flags! {
    VimMode => vim_mode,
    HardTabs => hard_tabs,
    LineNumbers => display.line_numbers,
    RelativeLineNumbers => display.relative_line_numbers,
    CurrentLine => display.current_line,
    Occurrences => display.occurrences,
    IndentGuides => display.indent_guides,
    StickyScroll => display.sticky_scroll,
    Scrollbars => display.scrollbars,
    Minimap => display.minimap,
    Breadcrumbs => display.breadcrumbs,
    SplitDiff => split_diff,
    InlayHints => inlay_hints,
    CodeLens => code_lens,
    CursorBlink => cursor_blink,
    FormatOnSave => format_on_save,
    TrimWhitespace => trim_whitespace,
    FinalNewline => final_newline,
    TrustWorktrees => trust_worktrees,
}

/// Declares the part of [`Preferences::reset`] that puts a field back as it
/// is, from the preferences that are one field each.
macro_rules! fields {
    ($($preference:ident => $($field:ident).+),* $(,)?) => {
        impl Preferences {
            /// Puts `preference` back to `defaults`' value when it is one
            /// field, saying whether it was.
            fn reset_field(&mut self, preference: Preference, defaults: Self) -> bool {
                match preference {
                    $(Preference::$preference => self.$($field).+ = defaults.$($field).+,)*
                    _ => return false,
                }
                true
            }
        }
    };
}

fields! {
    ThemeMode => theme_mode,
    ThemeFamily => theme_family,
    InterfaceFont => fonts.interface_family,
    InterfaceFontSize => fonts.interface_size,
    BufferFont => fonts.buffer_family,
    BufferFontSize => fonts.buffer_size,
    BufferFontWeight => fonts.buffer_weight,
    BufferLineHeight => fonts.buffer_line_height,
    Keymap => keymap,
    VimMode => vim_mode,
    VimClipboard => vim_clipboard,
    TabSize => tab_size,
    HardTabs => hard_tabs,
    LineNumbers => display.line_numbers,
    RelativeLineNumbers => display.relative_line_numbers,
    CurrentLine => display.current_line,
    Occurrences => display.occurrences,
    IndentGuides => display.indent_guides,
    StickyScroll => display.sticky_scroll,
    Scrollbars => display.scrollbars,
    Minimap => display.minimap,
    Breadcrumbs => display.breadcrumbs,
    SplitDiff => split_diff,
    WrapGuide => display.wrap_guide,
    InlayHints => inlay_hints,
    CodeLens => code_lens,
    CursorShape => display.cursor_shape,
    CursorBlink => cursor_blink,
    ScrollSensitivity => scroll_sensitivity,
    FormatOnSave => format_on_save,
    TrimWhitespace => trim_whitespace,
    FinalNewline => final_newline,
    TerminalFontSize => fonts.terminal_size,
    TerminalScrollback => terminal_scrollback,
    TrustWorktrees => trust_worktrees,
    InstallLanguageServers => install_language_servers,
    WorktreeLink => bootstrap.link,
    WorktreeCopy => bootstrap.copy,
    WorktreePort => bootstrap.port,
}

impl Preferences {
    /// Folds one message into the preferences, saying whether it was one of
    /// theirs.
    pub fn apply(&mut self, message: Message) -> bool {
        match message {
            Message::SetThemeMode(mode) => self.theme_mode = mode,
            Message::SetThemeFamily(index) => {
                self.theme_family = index.min(families().len().saturating_sub(1));
            }
            Message::SetKeymap(index) => {
                self.keymap = index.min(keymap::keymaps().len().saturating_sub(1));
            }
            Message::TogglePreference(preference) => {
                if let Some(flag) = self.flag_mut(preference) {
                    *flag = !*flag;
                }
            }
            Message::StepPreference(preference, step) => self.step(preference, step),
            Message::SetCursorShape(shape) => self.display.cursor_shape = shape,
            Message::SetVimClipboard(sharing) => self.vim_clipboard = sharing,
            Message::SetInstallLanguageServers(mode) => self.install_language_servers = mode,
            Message::SetWrapGuide(column) => self.display.wrap_guide = column,
            Message::RemoveWorktreePath(list, index) => {
                let paths = self.paths_mut(list);
                if index < paths.len() {
                    paths.remove(index);
                }
            }
            Message::ResetPreference(preference) => self.reset(preference),
            _ => return false,
        }
        true
    }

    /// Moves the number `preference` is one step along its range.
    fn step(&mut self, preference: Preference, step: Step) {
        let fonts = &mut self.fonts;
        match preference {
            Preference::InterfaceFontSize => {
                fonts.interface_size = stepped(fonts.interface_size, step, 1.0, INTERFACE_SIZES);
            }
            Preference::BufferFontSize => {
                fonts.buffer_size = stepped(fonts.buffer_size, step, 1.0, FONT_SIZES);
            }
            Preference::BufferFontWeight => {
                let weight = stepped(f32::from(fonts.buffer_weight), step, 100.0, WEIGHTS);
                fonts.buffer_weight = weight as u16;
            }
            Preference::BufferLineHeight => {
                fonts.buffer_line_height =
                    stepped(fonts.buffer_line_height, step, 0.1, LINE_HEIGHTS);
            }
            Preference::TerminalFontSize => {
                fonts.terminal_size = stepped(fonts.terminal_size, step, 1.0, FONT_SIZES);
            }
            Preference::TerminalScrollback => {
                let lines = stepped(self.terminal_scrollback as f32, step, 1_000.0, SCROLLBACKS);
                self.terminal_scrollback = lines as usize;
            }
            Preference::TabSize => {
                self.tab_size = stepped(self.tab_size as f32, step, 1.0, TAB_SIZES) as usize;
            }
            Preference::ScrollSensitivity => {
                self.scroll_sensitivity =
                    stepped(self.scroll_sensitivity, step, 0.25, SENSITIVITIES);
            }
            _ => {}
        }
    }

    /// The number `preference` is set to, as the settings pane shows it,
    /// when it is a number.
    pub fn number(&self, preference: Preference) -> Option<String> {
        let fonts = &self.fonts;
        Some(match preference {
            Preference::InterfaceFontSize => format!("{}", fonts.interface_size),
            Preference::BufferFontSize => format!("{}", fonts.buffer_size),
            Preference::BufferFontWeight => format!("{}", fonts.buffer_weight),
            Preference::BufferLineHeight => format!("{:.1}", fonts.buffer_line_height),
            Preference::TerminalFontSize => format!("{}", fonts.terminal_size),
            Preference::TerminalScrollback => format!("{}", self.terminal_scrollback),
            Preference::TabSize => format!("{}", self.tab_size),
            Preference::ScrollSensitivity => format!("{:.2}×", self.scroll_sensitivity),
            _ => return None,
        })
    }

    /// How a file that does not say is indented, and how wide a tab is.
    pub fn indent(&self) -> Indent {
        Indent {
            width: self.tab_size,
            tabs: self.hard_tabs,
        }
    }

    /// Adds `typed` to the `list` of paths a new worktree is given.
    ///
    /// The path is the repository's, so it is kept as it was typed, less the
    /// space and slashes at either end; one already listed is not listed
    /// twice.
    pub fn add_worktree_path(&mut self, list: WorktreePaths, typed: &str) {
        let trimmed = typed.trim().trim_matches('/');
        if trimmed.is_empty() {
            return;
        }
        let path = PathBuf::from(trimmed);
        let paths = self.paths_mut(list);
        if !paths.contains(&path) {
            paths.push(path);
        }
    }

    /// Names the variable a session's port is handed in, or none for
    /// nothing typed.
    pub fn set_worktree_port(&mut self, typed: &str) {
        let name = typed.trim();
        self.bootstrap.port = (!name.is_empty()).then(|| name.to_owned());
    }

    /// The `list` of paths a new worktree is given.
    pub fn worktree_paths(&self, list: WorktreePaths) -> &[PathBuf] {
        match list {
            WorktreePaths::Linked => &self.bootstrap.link,
            WorktreePaths::Copied => &self.bootstrap.copy,
        }
    }

    /// The `list` of paths a new worktree is given, to change.
    fn paths_mut(&mut self, list: WorktreePaths) -> &mut Vec<PathBuf> {
        match list {
            WorktreePaths::Linked => &mut self.bootstrap.link,
            WorktreePaths::Copied => &mut self.bootstrap.copy,
        }
    }

    /// The keymap in force: the chosen one, with the reader's own bindings
    /// over it.
    pub fn keymap_in_force(&self) -> Keymap {
        keymap::resolve(self.keymap, &self.keybindings)
    }

    /// Binds `action` to `sequence` alone, over the chosen keymap.
    pub fn rebind(&mut self, action: Action, sequence: Sequence) {
        let beneath = keymap::resolve(self.keymap, &Changes::default());
        self.keybindings.rebind(&beneath, action, sequence);
    }

    /// Takes every chord of `action` away, over the chosen keymap.
    pub fn unbind(&mut self, action: Action) {
        let beneath = keymap::resolve(self.keymap, &Changes::default());
        self.keybindings.clear(&beneath, action);
    }

    /// Whether `preference` is set to something other than its default.
    pub fn is_modified(&self, preference: Preference) -> bool {
        let mut reset = self.clone();
        reset.reset(preference);
        reset != *self
    }

    /// Puts `preference` back to what a first launch starts from.
    fn reset(&mut self, preference: Preference) {
        if self.reset_field(preference, Self::default()) {
            return;
        }
        match preference {
            Preference::ThemeColor(appearance, token) => {
                self.theme_overrides.clear(appearance, token);
            }
            Preference::ThemeOverrides(appearance) => {
                self.theme_overrides.clear_all(appearance);
            }
            Preference::Keybindings => self.keybindings = Changes::default(),
            Preference::Binding(action) => self.keybindings.reset(action),
            _ => {}
        }
    }
}

/// `value` moved one `by` in the direction of `step`, on the grid `by`
/// divides the range into and within the range.
fn stepped(value: f32, step: Step, by: f32, range: RangeInclusive<f32>) -> f32 {
    let moved = match step {
        Step::Down => value - by,
        Step::Up => value + by,
    };
    ((moved / by).round() * by).clamp(*range.start(), *range.end())
}
