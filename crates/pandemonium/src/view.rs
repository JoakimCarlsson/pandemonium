//! Where the window was when it was closed, kept on this machine so that the next launch opens on the same page.

use pm_components::browser_tabs::SavedTab;
use pm_components::open_files::{Contents, EDITOR_FIELD, Left};
use pm_components::panes::{Layout, Panes};
use pm_components::state::{
    PROMPT, Page, ReviewTab, SESSION_BASE, Tab, WorkspaceState, prompt_field,
};
use pm_input::FocusId;
use serde::{Deserialize, Serialize};

mod writer;

pub use writer::Writer;

/// The name the view is kept under in the data folder.
const FILE: &str = "view";

/// The page the main area showed.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
enum Saved {
    /// A session, by the machine it is on and the server's number for it.
    Session {
        /// The machine, by destination; nothing for this one.
        host: Option<String>,
        /// The server's number for the session.
        number: u64,
    },
    /// The page for starting work in the project at this path.
    New(String),
    /// Every session.
    History,
    /// The overview of the project at this path.
    Project(String),
    /// The extensions there are to install.
    Extensions,
    /// The page for starting work in the first project.
    #[default]
    First,
}

/// A file that was open in a tab of the session.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct SavedFile {
    /// The file, from the root of the repository.
    path: String,
    /// Whether it was read from the last commit.
    head: bool,
    /// Whether its tab was only a preview.
    preview: bool,
    /// How far down it was scrolled, in logical pixels.
    #[serde(default)]
    scroll: f32,
    /// The caret, as a byte offset into the text.
    #[serde(default)]
    caret: usize,
    /// The text that was not written to the folder, when the file had changes.
    #[serde(default)]
    unsaved: Option<String>,
}

/// The field the reader was typing in.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
enum Typing {
    /// The prompt to the agent.
    Prompt,
    /// The file shown.
    Editor,
}

/// A session's read position, keyed by its machine and server identity.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct ReadReceipt {
    /// The machine hosting the conversation.
    pub host: Option<String>,
    /// The server's session number.
    pub number: u64,
    /// The latest response update that was read.
    pub through: u64,
    /// Whether the reader left an explicit unread reminder.
    pub marked: bool,
}

/// What the reader had on screen: the page, the panels and the prompt being written.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct View {
    /// The page the main area showed.
    page: Saved,
    /// Read positions for conversations on all machines.
    #[serde(default)]
    receipts: Vec<ReadReceipt>,
    /// Whether the files the agent changed showed instead of the conversation.
    diff: bool,
    /// The files open in the session's tabs, in order.
    #[serde(default)]
    files: Vec<SavedFile>,
    /// The tab of the file shown, if one was.
    #[serde(default)]
    shown: Option<usize>,
    /// Whether a file was what the main area showed.
    #[serde(default)]
    file_tab: bool,
    /// Whether the review panel was open.
    review: bool,
    /// Whether the review panel listed every file of the project instead of the changes.
    #[serde(default)]
    files_tab: bool,
    /// The folders of the project's files that were unfolded.
    #[serde(default)]
    open_tree: Vec<String>,
    /// The folders of the changed files that were unfolded.
    #[serde(default)]
    closed_folders: Vec<String>,
    /// How far down the review panel's list was scrolled, in logical pixels.
    #[serde(default)]
    tree_scroll: f32,
    /// How far down the diff was scrolled, in logical pixels.
    #[serde(default)]
    diff_scroll: f32,
    /// Whether the list of changes was open in the review panel.
    #[serde(default = "open")]
    changes_open: bool,
    /// Whether the graph of commits was open in the review panel.
    #[serde(default)]
    graph_open: bool,
    /// Whether the graph showed every branch.
    #[serde(default)]
    graph_all: bool,
    /// How tall the graph was, in logical pixels.
    #[serde(default)]
    graph_height: f32,
    /// Whether the terminal was taller than usual.
    #[serde(default)]
    terminal_maximized: bool,
    /// How tall the terminal was while it was not maximized, in logical pixels.
    #[serde(default)]
    terminal_height: f32,
    /// How wide the sidebar was, in logical pixels.
    #[serde(default)]
    sidebar_width: f32,
    /// The commands last carried out from the quick input, by the name each is kept under, the latest first.
    #[serde(default)]
    recent_commands: Vec<String>,
    /// How wide the review panel was, in logical pixels.
    #[serde(default)]
    review_width: f32,
    /// How wide the plan panel was dragged to be, in logical pixels, or zero while it took the width it would by itself.
    #[serde(default)]
    plan_width: f32,
    /// Whether the terminal was open.
    terminal: bool,
    /// How far the sidebar's list was scrolled, in logical pixels.
    scroll: f32,
    /// The agent chosen for new work, by position.
    agent: usize,
    /// The prompt being written.
    prompt: String,
    /// The field the reader was typing in, if one had the keyboard.
    #[serde(default)]
    typing: Option<Typing>,
    /// The browser, when it was open.
    #[serde(default)]
    browser: Option<SavedBrowser>,
    /// How the main area was cut into panes, and the pane worked in.
    #[serde(default)]
    panes: Option<SavedPanes>,
}

/// The browser as it was left.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct SavedBrowser {
    /// The address in the address line of the tab that was shown, which older versions kept as the only tab.
    address: String,
    /// The machine the page of that tab was on, when it was not this one.
    host: Option<String>,
    /// Every tab, in the order of the strip; empty when saved by a version without tabs.
    #[serde(default)]
    tabs: Vec<SavedBrowserTab>,
    /// The index of the tab that was shown.
    #[serde(default)]
    active: usize,
    /// Whether the developer tools were open.
    tools: bool,
    /// How wide the developer tools were, in logical pixels.
    #[serde(default)]
    tools_width: f32,
    /// Whether the browser was what the main area showed.
    shown: bool,
}

/// One tab of the browser as it was left.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct SavedBrowserTab {
    /// The address in its address line.
    address: String,
    /// The machine the page was on, when it was not this one.
    #[serde(default)]
    host: Option<String>,
    /// Whether an agent opened it.
    #[serde(default)]
    agent: bool,
    /// Whether it was locked against agents.
    #[serde(default)]
    locked: bool,
}

/// The panes of the main area as they were left.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct SavedPanes {
    /// How the area was cut.
    layout: Layout,
    /// The pane last worked in, by position.
    focused: usize,
}

/// A flag that was set before it was kept.
fn open() -> bool {
    true
}

impl View {
    /// The saved acknowledgements for conversations, including remote ones.
    pub(crate) fn receipts(&self) -> &[ReadReceipt] {
        &self.receipts
    }

    /// The view saved by the last run, if there is one.
    pub fn load() -> Option<Self> {
        serde_json::from_str(&pm_store::load_text(FILE)?).ok()
    }

    /// Keeps the view for the next launch, readable by the reader alone since it holds the text of files that were not saved.
    pub fn save(&self) {
        if let Ok(text) = serde_json::to_string(self) {
            let _ = pm_store::save_private(FILE, &text);
        }
    }

    /// What `state` shows now, with `focused` the field that has the keyboard and `server_number` telling each session's machine and number on its server.
    pub fn capture(
        state: &WorkspaceState,
        focused: Option<FocusId>,
        server_number: impl Fn(usize) -> Option<(Option<String>, u64)>,
    ) -> Self {
        let path = |project: usize| {
            state
                .projects
                .get(project)
                .map(|held| held.path.clone())
                .unwrap_or_default()
        };
        let page = match state.page {
            Page::Session(position) => match server_number(position) {
                Some((host, number)) => Saved::Session { host, number },
                None => Saved::New(path(
                    state
                        .sessions
                        .get(position)
                        .map_or(0, |session| session.project),
                )),
            },
            Page::New(project) => Saved::New(path(project)),
            Page::Project(project) => Saved::Project(path(project)),
            Page::History => Saved::History,
            Page::Extensions => Saved::Extensions,
        };
        let with_files = matches!(page, Saved::Session { .. })
            || matches!(
                (&page, state.page),
                (Saved::New(_), Page::New(_)) | (Saved::Project(_), Page::Project(_))
            );
        let files = if with_files {
            state
                .git
                .viewers
                .iter()
                .map(|viewer| {
                    let document = match &viewer.contents {
                        Contents::Text(document) => Some(document),
                        _ => None,
                    };
                    SavedFile {
                        path: viewer.path.clone(),
                        head: viewer.head,
                        preview: viewer.preview,
                        scroll: viewer.scroll,
                        caret: document.map_or(0, |document| document.field.caret()),
                        unsaved: document
                            .filter(|document| document.is_dirty())
                            .map(|document| document.field.text().to_owned()),
                    }
                })
                .collect()
        } else {
            Vec::new()
        };
        let typing = match focused {
            Some(PROMPT) => Some(Typing::Prompt),
            Some(EDITOR_FIELD) if with_files => Some(Typing::Editor),
            _ => None,
        };
        Self {
            receipts: state
                .sessions
                .iter()
                .enumerate()
                .filter_map(|(index, session)| {
                    let (host, number) = server_number(index)?;
                    Some(ReadReceipt {
                        host,
                        number,
                        through: session.read_through,
                        marked: session.marked_unread,
                    })
                })
                .collect(),
            page,
            files,
            shown: state.git.shown.filter(|_| with_files),
            file_tab: with_files && state.tab == Tab::File,
            diff: state.tab == Tab::Diff,
            review: state.review,
            files_tab: state.review_tab == ReviewTab::Files,
            open_tree: sorted(&state.open_tree),
            closed_folders: sorted(&state.closed_folders),
            tree_scroll: state.tree_scroll,
            diff_scroll: state.diff_scroll,
            changes_open: state.git.changes_open,
            graph_open: state.git.graph_open,
            graph_all: state.git.graph_all,
            graph_height: state.git.graph_height,
            terminal_maximized: state.terminal_maximized,
            terminal_height: state.terminal_height,
            sidebar_width: state.sidebar_width,
            recent_commands: state
                .recent_commands
                .iter()
                .map(|action| action.key().to_owned())
                .collect(),
            review_width: state.review_width,
            plan_width: state.plan_width,
            terminal: state.terminal,
            scroll: state.sidebar_scroll,
            agent: state.agent,
            prompt: state.prompt.text().to_owned(),
            typing,
            browser: state
                .browser_saved()
                .map(|(tabs, active, tools, tools_width)| SavedBrowser {
                    address: tabs
                        .get(active)
                        .map(|tab| tab.address.clone())
                        .unwrap_or_default(),
                    host: tabs.get(active).and_then(|tab| tab.host.clone()),
                    tabs: tabs
                        .into_iter()
                        .map(|tab| SavedBrowserTab {
                            address: tab.address,
                            host: tab.host,
                            agent: tab.agent,
                            locked: tab.locked,
                        })
                        .collect(),
                    active,
                    tools,
                    tools_width,
                    shown: state.tab == Tab::Browser,
                }),
            panes: Some(SavedPanes {
                layout: state.panes.layout.clone(),
                focused: state.panes.focused,
            }),
        }
    }

    /// What the review panel listed.
    fn review_tab(&self) -> ReviewTab {
        if self.files_tab {
            ReviewTab::Files
        } else {
            ReviewTab::Changes
        }
    }

    /// The field the reader was typing in, to give the keyboard back to once the view is finished.
    pub fn typing(&self) -> Option<FocusId> {
        match self.typing? {
            Typing::Prompt => Some(PROMPT),
            Typing::Editor => Some(EDITOR_FIELD),
        }
    }

    /// The session that was open, by machine and the server's number, which can only be opened once the server has listed it.
    pub fn session(&self) -> Option<(Option<String>, u64)> {
        match &self.page {
            Saved::Session { host, number } => Some((host.clone(), *number)),
            _ => None,
        }
    }

    /// The machine whose server has to have listed its sessions before the view can be finished.
    pub fn home(&self) -> Option<String> {
        self.session().and_then(|(host, _)| host)
    }

    /// `state` with what does not wait for the server put back: the page for starting work or the history, the panels, the agent and the prompt.
    pub fn apply(&self, mut state: WorkspaceState) -> WorkspaceState {
        let project_at = |path: &String| state.projects.iter().position(|held| held.path == *path);
        match &self.page {
            Saved::New(path) => state.page = Page::New(project_at(path).unwrap_or(0)),
            Saved::Project(path) => state.page = Page::Project(project_at(path).unwrap_or(0)),
            Saved::History => state.page = Page::History,
            Saved::Extensions => state.page = Page::Extensions,
            Saved::Session { .. } | Saved::First => {}
        }
        state.review = self.review;
        state.review_tab = self.review_tab();
        state.open_tree = self.open_tree.iter().cloned().collect();
        state.closed_folders = self.closed_folders.iter().cloned().collect();
        state.terminal = self.terminal;
        state.terminal_maximized = self.terminal_maximized;
        if self.terminal_height > 0.0 {
            state.terminal_height = self.terminal_height;
        }
        if self.sidebar_width > 0.0 {
            state.sidebar_width = self.sidebar_width;
        }
        state.recent_commands = self
            .recent_commands
            .iter()
            .filter_map(|key| pm_editor::Action::of_key(key))
            .collect();
        if self.review_width > 0.0 {
            state.review_width = self.review_width;
        }
        state.plan_width = self.plan_width;
        state.agent = self.agent.min(state.agent_slots().saturating_sub(1));
        state.prompt = prompt_field(&self.prompt);
        state
    }

    /// `state` open on the session at `position`, on the tab it was left on, unless it was closed since, or on the page for starting work with the files that were open there, with the sidebar scrolled as it was.
    pub fn reopen(&self, state: WorkspaceState, position: Option<usize>) -> WorkspaceState {
        let open = position.filter(|at| state.sessions.get(*at).is_some_and(|held| !held.closed));
        let mut state = match (open, state.page) {
            (Some(position), _) => {
                let mut opened = state.after(FocusId(SESSION_BASE + position as u64));
                opened.tab = if self.diff { Tab::Diff } else { Tab::Chat };
                opened.sync_context();
                self.reopen_files(opened)
            }
            (None, Page::New(_) | Page::Project(_))
                if matches!(self.page, Saved::New(_) | Saved::Project(_)) =>
            {
                let mut opened = state;
                opened.sync_context();
                self.reopen_files(opened)
            }
            _ => state,
        };
        state.git.changes_open = self.changes_open;
        state.git.graph_open = self.graph_open;
        state.git.graph_all = self.graph_all;
        if self.graph_height > 0.0 {
            state.git.graph_height = self.graph_height;
        }
        state.review_tab = self.review_tab();
        state.sidebar_scroll = self.scroll;
        state.tree_scroll = self.tree_scroll;
        state.diff_scroll = self.diff_scroll;
        state.refresh_files();
        if let Some(saved) = &self.panes {
            state.panes = Panes {
                layout: saved.layout.clone(),
                focused: saved.focused,
                parked: None,
            };
        }
        if let Some(browser) = &self.browser {
            let tabs: Vec<SavedTab> = if browser.tabs.is_empty() {
                vec![SavedTab {
                    address: browser.address.clone(),
                    host: browser.host.clone(),
                    agent: false,
                    locked: false,
                }]
            } else {
                browser
                    .tabs
                    .iter()
                    .map(|tab| SavedTab {
                        address: tab.address.clone(),
                        host: tab.host.clone(),
                        agent: tab.agent,
                        locked: tab.locked,
                    })
                    .collect()
            };
            state.restore_browser(
                &tabs,
                browser.active,
                browser.tools,
                browser.tools_width,
                browser.shown,
            );
        }
        state
    }

    /// `opened` with the files that were open put back in their tabs, each scrolled, with its caret and its unsaved text as it was left, on the tab that was showing.
    fn reopen_files(&self, mut opened: WorkspaceState) -> WorkspaceState {
        for file in &self.files {
            opened.open_viewer(file.path.clone(), file.head);
            if let Some(viewer) = opened.git.viewers.last_mut() {
                viewer.preview = false;
            }
        }
        for file in &self.files {
            if let Some(viewer) = opened
                .git
                .viewers
                .iter_mut()
                .find(|viewer| viewer.path == file.path && viewer.head == file.head)
            {
                viewer.preview = file.preview;
                viewer.scroll = file.scroll;
                viewer.left = Some(Left {
                    unsaved: file.unsaved.clone(),
                    caret: file.caret,
                });
            }
        }
        opened.git.shown = self.shown.filter(|shown| *shown < opened.git.viewers.len());
        opened.tab = match (self.file_tab, opened.git.shown) {
            (true, Some(_)) => Tab::File,
            _ if self.diff => Tab::Diff,
            _ => Tab::Chat,
        };
        opened
    }
}

/// The members of `set`, in order, so that the same set is always kept the same way.
fn sorted(set: &std::collections::HashSet<String>) -> Vec<String> {
    let mut members: Vec<String> = set.iter().cloned().collect();
    members.sort();
    members
}

/// Coverage for persistent read acknowledgements and compatibility with older saved views.
#[cfg(test)]
mod tests {
    use super::*;

    /// Equal session numbers on different machines retain independent read positions and reminders.
    #[test]
    fn read_receipts_round_trip_by_machine_and_session() {
        let mut state = WorkspaceState::sample();
        state.sessions[0].updates = 12;
        state.sessions[0].mark_read();
        state.sessions[1].marked_unread = true;
        let view = View::capture(&state, None, |index| match index {
            0 => Some((None, 7)),
            1 => Some((Some("ssh://build".into()), 7)),
            _ => None,
        });
        let text = serde_json::to_string(&view).unwrap();
        let restored: View = serde_json::from_str(&text).unwrap();
        assert_eq!(restored.receipts(), view.receipts());
        assert_eq!(restored.receipts()[0].through, 12);
        assert_eq!(restored.receipts()[0].host, None);
        assert!(restored.receipts()[1].marked);
        assert_eq!(restored.receipts()[1].host.as_deref(), Some("ssh://build"));
        let mut legacy = serde_json::to_value(&view).unwrap();
        legacy.as_object_mut().unwrap().remove("receipts");
        let restored: View = serde_json::from_value(legacy).unwrap();
        assert!(restored.receipts().is_empty());
    }
}
