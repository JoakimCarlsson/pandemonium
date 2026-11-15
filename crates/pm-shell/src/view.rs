//! What of the window's layout is kept between runs: the page showing, which panels are open, their tabs and their sizes, so the window reopens the way it was left.

use serde::{Deserialize, Serialize};

use crate::browser_scope::SavedBrowser;
use crate::model::{DockTab, PanelSizes, RightTab, Workspace};
use crate::open_files::SavedFile;
use crate::page::Page;

/// A page as it is written down.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SavedPage {
    /// The session view with no session open.
    #[default]
    Home,
    /// The session view showing the session with this number.
    Session(u64),
    /// The history of every session.
    History,
    /// The extensions page.
    Extensions,
    /// The overview of the project at this address.
    Overview(String),
}

impl From<&Page> for SavedPage {
    /// `page` as it is written down.
    fn from(page: &Page) -> Self {
        match page {
            Page::Home => Self::Home,
            Page::Session(id) => Self::Session(*id),
            Page::History => Self::History,
            Page::Extensions => Self::Extensions,
            Page::Overview(path) => Self::Overview(path.clone()),
        }
    }
}

impl From<SavedPage> for Page {
    /// The page `saved` names.
    fn from(saved: SavedPage) -> Self {
        match saved {
            SavedPage::Home => Self::Home,
            SavedPage::Session(id) => Self::Session(id),
            SavedPage::History => Self::History,
            SavedPage::Extensions => Self::Extensions,
            SavedPage::Overview(path) => Self::Overview(path),
        }
    }
}

/// The layout of the window as it is kept between runs.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct View {
    /// The page showing.
    pub page: SavedPage,
    /// Whether the sidebar is shown.
    pub sidebar_open: bool,
    /// Whether the right panel is shown.
    pub right_open: bool,
    /// Whether the dock is shown.
    pub dock_open: bool,
    /// The tab chosen in the right panel.
    pub right_tab: RightTab,
    /// The tab chosen in the dock.
    pub dock_tab: DockTab,
    /// How big the resizable panels are.
    pub sizes: PanelSizes,
    /// The browser of each session, with its tabs.
    pub browsers: Vec<SavedBrowser>,
    /// The files open in the session view, with their unsaved text.
    pub files: Vec<SavedFile>,
    /// The session the open files belong to, when they belong to one.
    pub files_owner: Option<u64>,
    /// The files left open in the other sessions, by session.
    pub session_files: std::collections::BTreeMap<u64, Vec<SavedFile>>,
    /// The agent new work was last started with, by its id.
    pub last_agent: Option<String>,
    /// How far the reader has read each session on this machine: its number and the last event read.
    pub read_through: std::collections::BTreeMap<u64, u64>,
    /// The files opened lately from the quick input, the latest first.
    pub recent_files: Vec<String>,
    /// The titles of the commands run lately from the quick input, the latest first.
    pub recent_commands: Vec<String>,
    /// The addresses of the projects folded in the sidebar.
    pub folded: Vec<String>,
    /// The share of the split view's width the left pane takes, or 0 for half.
    pub split_share: f32,
}

impl Default for View {
    /// The layout the window first opens with.
    fn default() -> Self {
        Self {
            page: SavedPage::Home,
            sidebar_open: true,
            right_open: true,
            dock_open: true,
            right_tab: RightTab::default(),
            dock_tab: DockTab::default(),
            sizes: PanelSizes::default(),
            browsers: Vec::new(),
            files: Vec::new(),
            files_owner: None,
            session_files: Default::default(),
            last_agent: None,
            read_through: Default::default(),
            recent_files: Vec::new(),
            recent_commands: Vec::new(),
            folded: Vec::new(),
            split_share: 0.0,
        }
    }
}

impl View {
    /// The layout `workspace` shows now.
    pub fn of(workspace: &Workspace) -> Self {
        Self {
            page: SavedPage::from(&workspace.page),
            sidebar_open: workspace.sidebar_open,
            right_open: workspace.right_open,
            dock_open: workspace.dock_open,
            right_tab: workspace.right_tab,
            dock_tab: workspace.dock_tab,
            sizes: workspace.sizes,
            browsers: workspace.shelf.saved(&workspace.browser),
            files: workspace.open_files.saved(),
            files_owner: workspace.open_files.owner,
            session_files: workspace.open_files.others.clone(),
            last_agent: workspace.last_agent.clone(),
            read_through: workspace.read_through.clone(),
            split_share: workspace.open_files.share,
            ..Self::default()
        }
    }

    /// Lays `workspace` out the way this view says, leaving the page to be opened once the server is reached.
    pub fn lay_out(&self, workspace: &mut Workspace) {
        workspace.sidebar_open = self.sidebar_open;
        workspace.right_open = self.right_open;
        workspace.dock_open = self.dock_open;
        workspace.right_tab = self.right_tab;
        workspace.dock_tab = self.dock_tab;
        workspace.sizes = self.sizes.clamped();
        workspace.last_agent = self.last_agent.clone();
        workspace.read_through = self.read_through.clone();
        workspace.open_files.share = self.split_share.clamp(0.0, 0.9);
        workspace.open_files.owner = self.files_owner;
        workspace.open_files.others = self.session_files.clone();
        workspace
            .shelf
            .restore(&mut workspace.browser, &self.browsers);
    }

    /// The view written as `text`, or the first layout when it cannot be read.
    pub fn read(text: &str) -> Self {
        serde_json::from_str(text).unwrap_or_default()
    }

    /// The view written down.
    pub fn write(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A view written down reads back the same.
    #[test]
    fn reads_back_what_it_wrote() {
        let view = View {
            page: SavedPage::Overview("~/repos/void".to_owned()),
            dock_open: false,
            dock_tab: DockTab::Problems,
            sizes: PanelSizes {
                sidebar: 300.0,
                ..PanelSizes::default()
            },
            ..View::default()
        };
        assert_eq!(View::read(&view.write()), view);
    }

    /// Text that is not a view reads as the first layout.
    #[test]
    fn unreadable_text_is_the_first_layout() {
        assert_eq!(View::read("not json"), View::default());
    }
}
