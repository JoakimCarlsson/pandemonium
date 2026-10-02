//! Where the settings pane is: which page or section it shows, how far down
//! it, and which pages the sidebar has opened out.
//!
//! The sidebar is a tree, after Zed's: a page is a root, and a page with
//! more than one section lists them as its children. Choosing a page shows
//! every section of it; choosing a section shows that section alone.

use pm_ui::{Appearance, Axis, ResizeEvent, ResizePhase, Scroll, Scrolled};

use crate::config::Preference;
use crate::keymap::{Action, Chord, Sequence};

/// The most chords a binding recorded in the keymap screen is pressed as.
const LONGEST_RECORDING: usize = 4;

/// One page of the settings pane, as its sidebar lists them.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum SettingsPage {
    /// How the window is painted and what its text is set in.
    #[default]
    Appearance,
    /// How text is edited, drawn and written down.
    Editor,
    /// Installed languages, extension discovery and server configuration.
    Languages,
    /// The keymap the editor starts from, modal editing, and every binding.
    Keymap,
    /// How a terminal is drawn and how much it remembers.
    Terminal,
    /// How a session's worktree is made and treated.
    Sessions,
    /// What agents are started with.
    Agents,
}

impl SettingsPage {
    /// Every page, in the order the sidebar lists them.
    pub const ALL: [Self; 7] = [
        Self::Appearance,
        Self::Editor,
        Self::Languages,
        Self::Keymap,
        Self::Terminal,
        Self::Sessions,
        Self::Agents,
    ];

    /// What the sidebar and the page's heading call it.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Appearance => "Appearance",
            Self::Editor => "Editor",
            Self::Languages => "Languages",
            Self::Keymap => "Keymap",
            Self::Terminal => "Terminal",
            Self::Sessions => "Sessions",
            Self::Agents => "Agents",
        }
    }

    /// The sections of the page, top to bottom.
    pub const fn sections(self) -> &'static [SettingsSection] {
        match self {
            Self::Appearance => &[
                SettingsSection::Theme,
                SettingsSection::ThemeColors,
                SettingsSection::Fonts,
                SettingsSection::Cursor,
            ],
            Self::Editor => &[
                SettingsSection::Indentation,
                SettingsSection::Gutter,
                SettingsSection::Highlighting,
                SettingsSection::Display,
                SettingsSection::Saving,
            ],
            Self::Languages => &[
                SettingsSection::Languages,
                SettingsSection::LanguageSettings,
            ],
            Self::Keymap => &[SettingsSection::Keymap, SettingsSection::Keybindings],
            Self::Terminal => &[SettingsSection::Terminal],
            Self::Sessions => &[SettingsSection::Sessions],
            Self::Agents => &[SettingsSection::AgentServers, SettingsSection::McpServers],
        }
    }

    /// Whether the sidebar lists the page's sections under it: a page of
    /// one section is that section, and has nothing to list, unless it is a
    /// page that more sections are still to join.
    pub const fn has_sections(self) -> bool {
        self.sections().len() > 1 || matches!(self, Self::Agents)
    }
}

/// One section of a page: a heading over rows, and on a page of several, a
/// child in the sidebar that shows the section alone.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SettingsSection {
    /// Language extensions to find, install and remove.
    Languages,
    /// How files of one language are indented, saved and served.
    LanguageSettings,
    /// The theme mode and family.
    Theme,
    /// Every colour of the theme, repaintable one at a time, and the
    /// reader's own themes.
    ThemeColors,
    /// The families and sizes of the interface and of code.
    Fonts,
    /// The caret's shape and blink.
    Cursor,
    /// How wide a step of indentation is, and what it is written as.
    Indentation,
    /// What the gutter numbers the lines with.
    Gutter,
    /// What is washed and lined behind the text.
    Highlighting,
    /// What is drawn around the text and into it, and how it scrolls.
    Display,
    /// What happens to a file as it is written.
    Saving,
    /// The keymap the editor starts from, and vim mode.
    Keymap,
    /// Every action, the chords it is pressed as, and the way to change them.
    Keybindings,
    /// How a terminal is set and how much it remembers.
    Terminal,
    /// Whether a session's worktree is trusted, and what a new one is given.
    Sessions,
    /// The agents the editor can start, and the ones the reader added.
    AgentServers,
    /// The tool servers every agent is started with.
    McpServers,
}

impl SettingsSection {
    /// What the section's heading and its entry in the sidebar call it.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Languages => "Extensions",
            Self::LanguageSettings => "Language Settings",
            Self::Theme => "Theme",
            Self::ThemeColors => "Theme Colors",
            Self::Fonts => "Fonts",
            Self::Cursor => "Cursor",
            Self::Indentation => "Indentation",
            Self::Gutter => "Gutter",
            Self::Highlighting => "Highlighting",
            Self::Display => "Display",
            Self::Saving => "Saving",
            Self::Keymap => "Keymap",
            Self::Keybindings => "Keybindings",
            Self::Terminal => "Terminal",
            Self::Sessions => "Sessions",
            Self::AgentServers => "Agent Servers",
            Self::McpServers => "MCP Servers",
        }
    }

    /// The page the section is on.
    pub fn page(self) -> SettingsPage {
        SettingsPage::ALL
            .into_iter()
            .find(|page| page.sections().contains(&self))
            .unwrap_or_default()
    }

    /// The preferences the section edits, for the sidebar to mark what has
    /// one set away from its default.
    pub const fn preferences(self) -> &'static [Preference] {
        match self {
            Self::Theme => &[Preference::ThemeMode, Preference::ThemeFamily],
            Self::ThemeColors => &[
                Preference::ThemeOverrides(Appearance::Dark),
                Preference::ThemeOverrides(Appearance::Light),
            ],
            Self::Fonts => &[
                Preference::InterfaceFont,
                Preference::InterfaceFontSize,
                Preference::BufferFont,
                Preference::BufferFontSize,
                Preference::BufferFontWeight,
                Preference::BufferLineHeight,
            ],
            Self::Cursor => &[Preference::CursorShape, Preference::CursorBlink],
            Self::Indentation => &[Preference::TabSize, Preference::HardTabs],
            Self::Gutter => &[Preference::LineNumbers, Preference::RelativeLineNumbers],
            Self::Highlighting => &[
                Preference::CurrentLine,
                Preference::Occurrences,
                Preference::IndentGuides,
                Preference::BracketColors,
                Preference::WrapGuide,
            ],
            Self::Display => &[
                Preference::StickyScroll,
                Preference::Scrollbars,
                Preference::Minimap,
                Preference::Breadcrumbs,
                Preference::SplitDiff,
                Preference::InlayHints,
                Preference::CodeLens,
                Preference::EditPredictions,
                Preference::ScrollSensitivity,
            ],
            Self::Saving => &[
                Preference::FormatOnSave,
                Preference::OrganizeImportsOnSave,
                Preference::FixOnSave,
                Preference::TrimWhitespace,
                Preference::FinalNewline,
                Preference::InstallLanguageServers,
            ],
            Self::Keymap => &[
                Preference::Keymap,
                Preference::VimMode,
                Preference::VimClipboard,
            ],
            Self::Keybindings => &[Preference::Keybindings],
            Self::Terminal => &[Preference::TerminalFontSize, Preference::TerminalScrollback],
            Self::Sessions => &[
                Preference::TrustWorktrees,
                Preference::HealthFeedback,
                Preference::HealthRetries,
                Preference::WorktreeLink,
                Preference::WorktreeCopy,
                Preference::WorktreePort,
            ],
            Self::AgentServers | Self::McpServers | Self::Languages | Self::LanguageSettings => &[],
        }
    }
}

/// What the settings pane is showing.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SettingsView {
    /// Every section of a page.
    Page(SettingsPage),
    /// One section alone.
    Section(SettingsSection),
}

impl SettingsView {
    /// The page what is shown is on.
    pub fn page(self) -> SettingsPage {
        match self {
            Self::Page(page) => page,
            Self::Section(section) => section.page(),
        }
    }

    /// The sections shown, top to bottom.
    pub fn sections(self) -> &'static [SettingsSection] {
        match self {
            Self::Page(page) => page.sections(),
            Self::Section(section) => {
                let sections = section.page().sections();
                let at = sections
                    .iter()
                    .position(|shown| *shown == section)
                    .unwrap_or(0);
                &sections[at..=at]
            }
        }
    }
}

/// What the settings pane remembers between frames.
///
/// The preferences themselves are not here: the pane is an editor of
/// [`crate::config::Preferences`], and only where the reader is in it is its
/// own.
#[derive(Debug)]
pub struct Settings {
    /// What is being shown.
    view: SettingsView,
    /// How far down it the pane is scrolled.
    scroll: Scrolled,
    /// Scroll offset at the start of the current thumb drag.
    scroll_origin: Option<f32>,
    /// The pages the sidebar has opened out to list their sections.
    expanded: Vec<SettingsPage>,
    /// The binding being recorded, while one is.
    recording: Option<Recording>,
    /// Whether the list of installed MCP servers is open.
    installed_open: bool,
    /// Whether the list of MCP servers on offer is open.
    available_open: bool,
    /// Whether the list of installed agents is open.
    agents_installed_open: bool,
    /// Whether the list of agents on offer is open.
    agents_available_open: bool,
}

/// The chords pressed so far for an action being bound.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Recording {
    /// The action the chords are for.
    pub action: Action,
    /// The chords pressed, in order.
    pub chords: Vec<Chord>,
}

impl Recording {
    /// The chords pressed, as the sequence they bind, once there is one.
    pub fn sequence(&self) -> Option<Sequence> {
        self.chords
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(" ")
            .parse()
            .ok()
    }
}

impl Default for Settings {
    /// The first page, at its top, opened out in the sidebar.
    fn default() -> Self {
        let page = SettingsPage::default();
        Self {
            view: SettingsView::Page(page),
            scroll: Scrolled::default(),
            scroll_origin: None,
            expanded: vec![page],
            recording: None,
            installed_open: true,
            available_open: true,
            agents_installed_open: true,
            agents_available_open: true,
        }
    }
}

impl Settings {
    /// What is being shown.
    pub fn view(&self) -> SettingsView {
        self.view
    }

    /// The scroll the view is drawn at, shared with the element drawing it.
    pub fn scroll(&self) -> Scrolled {
        self.scroll.clone()
    }

    /// Whether the sidebar lists the sections of `page`.
    pub fn is_expanded(&self, page: SettingsPage) -> bool {
        self.expanded.contains(&page)
    }

    /// Shows every section of `page`, opening it out in the sidebar.
    pub fn show(&mut self, page: SettingsPage) {
        self.open(SettingsView::Page(page));
    }

    /// Shows `section` alone, opening its page out in the sidebar.
    pub fn show_section(&mut self, section: SettingsSection) {
        self.open(SettingsView::Section(section));
    }

    /// Opens the sidebar's list of `page`'s sections, or folds it away.
    pub fn toggle(&mut self, page: SettingsPage) {
        match self.expanded.iter().position(|open| *open == page) {
            Some(index) => {
                self.expanded.remove(index);
            }
            None => self.expanded.push(page),
        }
    }

    /// Whether the list of installed MCP servers is open.
    pub fn installed_open(&self) -> bool {
        self.installed_open
    }

    /// Whether the list of MCP servers on offer is open.
    pub fn available_open(&self) -> bool {
        self.available_open
    }

    /// Whether the list of installed agents is open.
    pub fn agents_installed_open(&self) -> bool {
        self.agents_installed_open
    }

    /// Whether the list of agents on offer is open.
    pub fn agents_available_open(&self) -> bool {
        self.agents_available_open
    }

    /// Opens the list of installed agents, or folds it.
    pub fn toggle_agents_installed(&mut self) {
        self.agents_installed_open = !self.agents_installed_open;
    }

    /// Opens the list of agents on offer, or folds it.
    pub fn toggle_agents_available(&mut self) {
        self.agents_available_open = !self.agents_available_open;
    }

    /// Opens the list of installed MCP servers, or folds it.
    pub fn toggle_installed(&mut self) {
        self.installed_open = !self.installed_open;
    }

    /// Opens the list of installed MCP servers, if it was folded away.
    pub fn open_installed(&mut self) {
        self.installed_open = true;
    }

    /// Opens the list of MCP servers on offer, or folds it.
    pub fn toggle_available(&mut self) {
        self.available_open = !self.available_open;
    }

    /// The binding being recorded, while one is.
    pub fn recording(&self) -> Option<&Recording> {
        self.recording.as_ref()
    }

    /// Starts listening for the chords to bind `action` to.
    pub fn record(&mut self, action: Action) {
        self.recording = Some(Recording {
            action,
            chords: Vec::new(),
        });
    }

    /// Adds `chord` to the binding being recorded, up to the longest a
    /// binding is let be.
    pub fn press(&mut self, chord: Chord) {
        if let Some(recording) = self.recording.as_mut()
            && recording.chords.len() < LONGEST_RECORDING
        {
            recording.chords.push(chord);
        }
    }

    /// Takes the last chord off the binding being recorded.
    pub fn erase(&mut self) {
        if let Some(recording) = self.recording.as_mut() {
            recording.chords.pop();
        }
    }

    /// Stops listening, handing back what was recorded.
    pub fn stop_recording(&mut self) -> Option<Recording> {
        self.recording.take()
    }

    /// Scrolls the view by `delta` logical pixels, positive being towards
    /// the top.
    pub fn scroll_by(&mut self, delta: f32) {
        let mut scroll = self.scroll.get();
        scroll.by(delta);
        self.scroll.set(scroll);
    }

    /// Moves the viewport with a thumb drag measured from its initial offset.
    pub fn drag_scroll(&mut self, event: ResizeEvent, step: f32) {
        let mut scroll = self.scroll.get();
        let base = match event.phase {
            ResizePhase::Started => scroll.offset(),
            _ => self.scroll_origin.unwrap_or(scroll.offset()),
        };
        self.scroll_origin = match event.phase {
            ResizePhase::Ended => None,
            _ => Some(base),
        };
        scroll.by(scroll.offset() - base - event.delta(Axis::Vertical) * step);
        self.scroll.set(scroll);
    }

    /// Shows `view` from its top, and opens its page out in the sidebar.
    fn open(&mut self, view: SettingsView) {
        if self.view != view {
            self.view = view;
            self.scroll.set(Scroll::default());
        }
        let page = view.page();
        if page.has_sections() && !self.is_expanded(page) {
            self.expanded.push(page);
        }
    }
}
