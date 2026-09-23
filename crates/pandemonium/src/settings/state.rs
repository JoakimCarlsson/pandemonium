//! Where the settings pane is: which page or section it shows, how far down
//! it, and which pages the sidebar has opened out.
//!
//! The sidebar is a tree, after Zed's: a page is a root, and a page with
//! more than one section lists them as its children. Choosing a page shows
//! every section of it; choosing a section shows that section alone.

use pm_ui::{Appearance, Scroll, Scrolled};

use crate::config::Preference;

/// One page of the settings pane, as its sidebar lists them.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum SettingsPage {
    /// How the window is painted and what its text is set in.
    #[default]
    Appearance,
    /// How text is edited, drawn and written down.
    Editor,
    /// The bindings the editor starts from, and modal editing.
    Keymap,
    /// How a terminal is drawn and how much it remembers.
    Terminal,
    /// How a session's worktree is made and treated.
    Sessions,
}

impl SettingsPage {
    /// Every page, in the order the sidebar lists them.
    pub const ALL: [Self; 5] = [
        Self::Appearance,
        Self::Editor,
        Self::Keymap,
        Self::Terminal,
        Self::Sessions,
    ];

    /// What the sidebar and the page's heading call it.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Appearance => "Appearance",
            Self::Editor => "Editor",
            Self::Keymap => "Keymap",
            Self::Terminal => "Terminal",
            Self::Sessions => "Sessions",
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
            Self::Keymap => &[SettingsSection::Keymap],
            Self::Terminal => &[SettingsSection::Terminal],
            Self::Sessions => &[SettingsSection::Sessions],
        }
    }

    /// Whether the sidebar lists the page's sections under it: a page of
    /// one section is that section, and has nothing to list.
    pub const fn has_sections(self) -> bool {
        self.sections().len() > 1
    }
}

/// One section of a page: a heading over rows, and on a page of several, a
/// child in the sidebar that shows the section alone.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SettingsSection {
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
    /// The keymap the editor starts from.
    Keymap,
    /// How a terminal is set and how much it remembers.
    Terminal,
    /// Whether a session's worktree is trusted, and what a new one is given.
    Sessions,
}

impl SettingsSection {
    /// What the section's heading and its entry in the sidebar call it.
    pub const fn label(self) -> &'static str {
        match self {
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
            Self::Terminal => "Terminal",
            Self::Sessions => "Sessions",
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
                Preference::WrapGuide,
            ],
            Self::Display => &[
                Preference::StickyScroll,
                Preference::Scrollbars,
                Preference::InlayHints,
                Preference::ScrollSensitivity,
            ],
            Self::Saving => &[
                Preference::FormatOnSave,
                Preference::TrimWhitespace,
                Preference::FinalNewline,
            ],
            Self::Keymap => &[Preference::Keymap],
            Self::Terminal => &[Preference::TerminalFontSize, Preference::TerminalScrollback],
            Self::Sessions => &[
                Preference::TrustWorktrees,
                Preference::WorktreeLink,
                Preference::WorktreeCopy,
                Preference::WorktreePort,
            ],
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
    /// The pages the sidebar has opened out to list their sections.
    expanded: Vec<SettingsPage>,
}

impl Default for Settings {
    /// The first page, at its top, opened out in the sidebar.
    fn default() -> Self {
        let page = SettingsPage::default();
        Self {
            view: SettingsView::Page(page),
            scroll: Scrolled::default(),
            expanded: vec![page],
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

    /// Scrolls the view by `delta` logical pixels, positive being towards
    /// the top.
    pub fn scroll_by(&mut self, delta: f32) {
        let mut scroll = self.scroll.get();
        scroll.by(delta);
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
