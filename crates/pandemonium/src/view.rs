//! Where the window was when it was closed, kept on this machine so that the next launch opens on the same page.

use pm_components::open_files::{Contents, EDITOR_FIELD, Left};
use pm_components::state::{PROMPT, Page, SESSION_BASE, Tab, WorkspaceState, prompt_field};
use pm_input::FocusId;
use serde::{Deserialize, Serialize};

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

/// What the reader had on screen: the page, the panels and the prompt being written.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct View {
    /// The page the main area showed.
    page: Saved,
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
}

impl View {
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
            Page::History => Saved::History,
            Page::Extensions => Saved::Extensions,
        };
        let with_files = matches!(page, Saved::Session { .. })
            || matches!((&page, state.page), (Saved::New(_), Page::New(_)));
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
            page,
            files,
            shown: state.git.shown.filter(|_| with_files),
            file_tab: with_files && state.tab == Tab::File,
            diff: state.tab == Tab::Diff,
            review: state.review,
            terminal: state.terminal,
            scroll: state.sidebar_scroll,
            agent: state.agent,
            prompt: state.prompt.text().to_owned(),
            typing,
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
            Saved::History => state.page = Page::History,
            Saved::Extensions => state.page = Page::Extensions,
            Saved::Session { .. } | Saved::First => {}
        }
        state.review = self.review;
        state.terminal = self.terminal;
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
            (None, Page::New(_)) if matches!(self.page, Saved::New(_)) => {
                let mut opened = state;
                opened.sync_context();
                self.reopen_files(opened)
            }
            _ => state,
        };
        state.sidebar_scroll = self.scroll;
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
