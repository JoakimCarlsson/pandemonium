//! Where the settings pane is: which page it shows, and how far down it.

use pm_ui::Scrolled;

use crate::config::Preference;

/// One page of the settings pane, as its sidebar lists them.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum SettingsPage {
    /// The theme and how it follows the desktop.
    #[default]
    Appearance,
    /// The bindings the editor starts from, and modal editing.
    Keymap,
    /// How text is edited and written down.
    Editor,
    /// How a session's worktree is made and treated.
    Sessions,
}

impl SettingsPage {
    /// Every page, in the order the sidebar lists them.
    pub const ALL: [Self; 4] = [Self::Appearance, Self::Keymap, Self::Editor, Self::Sessions];

    /// What the sidebar and the page's heading call it.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Appearance => "Appearance",
            Self::Keymap => "Keymap",
            Self::Editor => "Editor",
            Self::Sessions => "Sessions",
        }
    }

    /// The preferences the page edits, for the sidebar to mark the pages
    /// that have one set away from its default.
    pub const fn preferences(self) -> &'static [Preference] {
        match self {
            Self::Appearance => &[Preference::ThemeMode, Preference::ThemeFamily],
            Self::Keymap => &[Preference::Keymap, Preference::VimMode],
            Self::Editor => &[Preference::FormatOnSave],
            Self::Sessions => &[
                Preference::TrustWorktrees,
                Preference::WorktreeLink,
                Preference::WorktreeCopy,
                Preference::WorktreePort,
            ],
        }
    }
}

/// What the settings pane remembers between frames.
///
/// The preferences themselves are not here: the pane is an editor of
/// [`crate::config::Preferences`], and only where the reader is in it is its
/// own.
#[derive(Debug, Default)]
pub struct Settings {
    /// The page being shown.
    page: SettingsPage,
    /// How far down it the pane is scrolled.
    scroll: Scrolled,
}

impl Settings {
    /// The page being shown.
    pub fn page(&self) -> SettingsPage {
        self.page
    }

    /// The scroll the page is drawn at, shared with the element drawing it.
    pub fn scroll(&self) -> Scrolled {
        self.scroll.clone()
    }

    /// Shows `page`, from its top.
    pub fn show(&mut self, page: SettingsPage) {
        if self.page != page {
            self.page = page;
            self.scroll.set(pm_ui::Scroll::default());
        }
    }

    /// Scrolls the page by `delta` logical pixels, positive being towards
    /// the top.
    pub fn scroll_by(&mut self, delta: f32) {
        let mut scroll = self.scroll.get();
        scroll.by(delta);
        self.scroll.set(scroll);
    }
}
