//! Chats: conversations with an agent that belong to no project and work in a scratch folder their server makes.

use std::path::PathBuf;

use pm_input::FocusId;

use crate::Project;
use crate::sample::Session;
use crate::state::{Page, WorkspaceState};

/// Shows the page that starts a chat.
pub const NEW_CHAT: FocusId = FocusId(302);

impl WorkspaceState {
    /// The project `session` belongs to: nothing for a chat, or when the project is gone.
    pub fn project_of(&self, session: &Session) -> Option<&Project> {
        self.projects.get(session.project?)
    }

    /// The machine `session` is on, by destination with nothing standing for this one: its project's, or for a chat the one its scratch folder was made on. Nothing when its project is gone.
    pub fn machine_of(&self, session: &Session) -> Option<Option<String>> {
        match session.project {
            Some(project) => self.projects.get(project).map(Project::host),
            None => Some(session.machine.clone()),
        }
    }

    /// Whether `session` is on the machine `host`.
    pub fn is_on(&self, session: &Session, host: &Option<String>) -> bool {
        self.machine_of(session)
            .is_some_and(|machine| machine == *host)
    }

    /// The folder paths in the conversation of `session` are shown from: its project's, or for a chat its scratch folder.
    pub fn root_of(&self, session: &Session) -> PathBuf {
        match self.project_of(session) {
            Some(project) => project.directory(),
            None => PathBuf::from(&session.directory),
        }
    }

    /// The machine of what the page shows, by destination: the open session's, the project's, or the one the chat being started goes to. Nothing for this machine, and for a page that shows no project.
    pub fn shown_machine(&self) -> Option<String> {
        match self.page {
            Page::Session(index) => self
                .sessions
                .get(index)
                .and_then(|session| self.machine_of(session))
                .flatten(),
            Page::New(project) | Page::Project(project) => {
                self.projects.get(project).and_then(Project::host)
            }
            Page::Chat => self.chat_on.clone(),
            Page::History | Page::Extensions => None,
        }
    }

    /// Shows the page that starts a chat, which will live on the machine of the project shown now, or on this machine when none is shown.
    pub(crate) fn start_chat(&mut self) {
        self.start_chat_on(self.shown_machine());
    }

    /// Shows the page that starts a chat on `machine`, by destination, or on this machine when there is none.
    pub(crate) fn start_chat_on(&mut self, machine: Option<String>) {
        self.chat_on = machine;
        self.page = Page::Chat;
        self.new_in = None;
    }

    /// The state open on the page that starts a chat.
    pub fn with_new_chat(mut self) -> Self {
        self.start_chat();
        self.sync_context();
        self
    }

    /// Shows the page for starting work in the first project, or the page that starts a chat when there is no project, as when what the page showed is gone.
    pub(crate) fn start_anew(&mut self) {
        if self.projects.is_empty() {
            self.start_chat_on(None);
        } else {
            self.start_in(0, None);
        }
    }

    /// The chats the sidebar lists, by position, in the order they were started: those the reader has not closed.
    pub fn open_chats(&self) -> Vec<usize> {
        self.sessions
            .iter()
            .enumerate()
            .filter(|(_, session)| session.is_chat() && !session.closed && !session.home)
            .map(|(position, _)| position)
            .collect()
    }

    /// Whether the review panel is about a chat, which has no branch, no worktree and no changes to show.
    pub fn reviewing_chat(&self) -> bool {
        self.review_session().is_some_and(Session::is_chat)
    }
}

#[cfg(test)]
mod tests;
