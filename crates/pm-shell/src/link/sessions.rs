//! The link's part in what the reader does to whole sessions: their menu in
//! the sidebar, closing one (asking first when its worktree holds changes not
//! committed), renaming one, copying its folder or branch, showing its folder,
//! the sessions branched off a turn, and the status bar's dismissal.

use pm_protocol::{Request, Response, SessionInfo};

use super::{Ask, Link};
use crate::model::Renaming;
use crate::page::Page;
use crate::quick::QuickOutcome;
use crate::thread::Thread;

/// Something the reader does to a whole session, or to the bars and dialogs about one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SessionCommand {
    /// Opens the menu of the session the window numbers so in the sidebar, or closes it when nothing.
    Menu(Option<u64>),
    /// Closes the session, asking first when its worktree has changes not committed.
    Close(u64),
    /// Closes the session the question is about, throwing its changes not committed away.
    Discard,
    /// Leaves the session the question is about open.
    KeepOpen,
    /// Opens the dialog renaming the session.
    Rename(u64),
    /// Takes the title typed in the rename dialog so far.
    Retitle(String),
    /// Gives the session the title typed in the rename dialog.
    ConfirmRename,
    /// Closes the rename dialog without renaming.
    CancelRename,
    /// Puts the session's folder on the clipboard.
    CopyPath(u64),
    /// Puts the session's branch on the clipboard.
    CopyBranch(u64),
    /// Shows the session's folder in the desktop's file manager.
    OpenFolder(u64),
    /// Takes the status bar's message away.
    DismissStatus,
}

impl Link {
    /// Does `command`, giving back what it asks of the window beyond the link.
    pub fn session_command(&mut self, command: SessionCommand) -> Option<QuickOutcome> {
        self.workspace.session_menu = None;
        match command {
            SessionCommand::Menu(session) => self.workspace.session_menu = session,
            SessionCommand::Close(session) => self.close_session(session, false),
            SessionCommand::Discard => {
                if let Some(session) = self.workspace.closing.take() {
                    self.close_session(session, true);
                }
            }
            SessionCommand::KeepOpen => self.workspace.closing = None,
            SessionCommand::Rename(session) => {
                let title = self
                    .session_info(session)
                    .map(|info| info.title)
                    .unwrap_or_default();
                self.workspace.renaming = Some(Renaming { session, title });
            }
            SessionCommand::Retitle(text) => {
                if let Some(renaming) = &mut self.workspace.renaming {
                    renaming.title = text;
                }
            }
            SessionCommand::ConfirmRename => self.rename_session(),
            SessionCommand::CancelRename => self.workspace.renaming = None,
            SessionCommand::CopyPath(session) => {
                return self
                    .session_info(session)
                    .map(|info| QuickOutcome::Copy(info.directory));
            }
            SessionCommand::CopyBranch(session) => {
                return self
                    .session_info(session)
                    .and_then(|info| info.branch)
                    .map(QuickOutcome::Copy);
            }
            SessionCommand::OpenFolder(session) => {
                if self.routes.remote(session).is_some() {
                    self.workspace.status =
                        Some("This session's folder is on another machine.".to_owned());
                    return None;
                }
                return self
                    .session_info(session)
                    .map(|info| QuickOutcome::OpenFolder(info.directory));
            }
            SessionCommand::DismissStatus => self.workspace.status = None,
        }
        None
    }

    /// Whether the window may close now: when the settings ask to confirm closing while agents work and some do, the question shows instead and the window stays, until the reader confirms.
    pub fn may_close(&mut self) -> bool {
        let working = self.working_agents();
        if self.workspace.quit_confirmed
            || working == 0
            || !self.settings.window.general.confirm_close
        {
            return true;
        }
        self.workspace.quitting = Some(working);
        false
    }

    /// Answers the question before the window closes on working agents: closing anyway when `close`, staying open otherwise.
    pub fn confirm_close(&mut self, close: bool) {
        self.workspace.quitting = None;
        self.workspace.quit_confirmed = close;
    }

    /// How many sessions in the sidebar have an agent working.
    fn working_agents(&self) -> usize {
        working_in(&self.workspace.projects)
            + self
                .workspace
                .chats
                .iter()
                .filter(|chat| chat.status == crate::model::SessionStatus::Working)
                .count()
    }

    /// Asks the server of its machine to close the session the window numbers `session`, throwing away changes its worktree has not committed when `discard`.
    fn close_session(&mut self, session: u64, discard: bool) {
        self.ask(
            Request::CloseSession { session, discard },
            Ask::Close(session),
        );
    }

    /// Gives the session of the rename dialog the title typed, unless it is blank, and closes the dialog.
    fn rename_session(&mut self) {
        let Some(renaming) = self.workspace.renaming.take() else {
            return;
        };
        let title = renaming.title.trim().to_owned();
        if title.is_empty() {
            return;
        }
        let id = renaming.session;
        let number = self.routes.remote(id).map(|(_, number)| number);
        let listed = self
            .sessions
            .iter_mut()
            .chain(self.remote_sessions.values_mut().flatten())
            .find(|info| info.session == number.unwrap_or(id));
        if let Some(info) = listed {
            info.title = title.clone();
        }
        if self.watched == Some(id) {
            self.workspace.title = title.clone();
        }
        self.ask(Request::RenameSession { session: id, title }, Ask::Nothing);
        self.rebuild();
    }

    /// Takes the server's answer to closing the session the window numbers `session`: leaves its page once it closed, and asks the reader before throwing away changes not committed.
    pub(super) fn answer_close(&mut self, session: u64, response: Response) {
        match response {
            Response::Uncommitted => self.workspace.closing = Some(session),
            Response::Failed { message } => {
                self.workspace.status = Some(format!("Could not close the session: {message}"));
            }
            _ => {
                if self.routes.remote(session).is_none() {
                    self.sessions.retain(|info| info.session != session);
                }
                if self.workspace.page == Page::Session(session) {
                    self.navigate(Page::Home);
                }
                self.rebuild();
            }
        }
    }

    /// Takes the server's answer to branching a session off a turn: lists the new session in the sidebar and opens it.
    pub(super) fn answer_fork(&mut self, response: Response) {
        let sessions = match response {
            Response::Sessions { sessions } => sessions,
            Response::Failed { message } => {
                self.workspace.status = Some(format!("Could not branch from this turn: {message}"));
                return;
            }
            _ => return,
        };
        let host = self
            .watched
            .and_then(|id| self.routes.remote(id))
            .map(|(host, _)| host.to_owned());
        let Some(first) = sessions.first().map(|info| info.session) else {
            return;
        };
        let opened = match host {
            Some(host) => {
                let id = self.routes.local(&host, "session", first);
                self.ask_remote(&host, Request::Sessions, Ask::RemoteSessions(host.clone()));
                id
            }
            None => {
                self.adopt(sessions);
                first
            }
        };
        self.rebuild();
        self.navigate(Page::Session(opened));
    }

    /// Lists `sessions` of this machine the window did not know of and follows their events.
    fn adopt(&mut self, sessions: Vec<SessionInfo>) {
        for info in sessions {
            if self
                .sessions
                .iter()
                .any(|known| known.session == info.session)
            {
                continue;
            }
            if let std::collections::hash_map::Entry::Vacant(entry) =
                self.threads.entry(info.session)
            {
                entry.insert(Thread::default());
                self.watch(info.session);
            }
            self.sessions.push(info);
        }
    }
}

/// How many sessions of `projects` and the projects nested in them have an agent working.
fn working_in(projects: &[crate::model::Project]) -> usize {
    projects
        .iter()
        .map(|project| {
            project
                .sessions
                .iter()
                .filter(|session| session.status == crate::model::SessionStatus::Working)
                .count()
                + working_in(&project.children)
        })
        .sum()
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;

    /// A session of this machine numbered `session` and titled `title`, with nothing else known of it.
    fn listed(session: u64, title: &str) -> SessionInfo {
        SessionInfo {
            session,
            agent: String::new(),
            repository: String::new(),
            directory: String::new(),
            branch: None,
            head: None,
            base: None,
            title: title.to_owned(),
            updated: 0,
            last_seq: 0,
            closed: false,
            running: false,
            login: None,
            submodule: None,
            chat: false,
        }
    }

    /// A failed request with nothing waiting on its answer shows in the status bar, and dismissing it takes it away.
    #[test]
    fn failures_reach_the_status_bar() {
        let mut link = Link::built(None, None, Arc::new(|| {}));
        link.answer(
            Ask::Nothing,
            Response::Failed {
                message: "no such session".to_owned(),
            },
        );
        assert_eq!(link.workspace.status.as_deref(), Some("no such session"));
        link.session_command(SessionCommand::DismissStatus);
        assert_eq!(link.workspace.status, None);
    }

    /// Closing a session whose worktree has changes asks first, and confirming closes it.
    #[test]
    fn closing_with_changes_asks_first() {
        let mut link = Link::built(None, None, Arc::new(|| {}));
        link.answer(Ask::Close(7), Response::Uncommitted);
        assert_eq!(link.workspace.closing, Some(7));
        link.session_command(SessionCommand::KeepOpen);
        assert_eq!(link.workspace.closing, None);
    }

    /// A session branched off a turn is listed and opened at once.
    #[test]
    fn forks_open_at_once() {
        let mut link = Link::built(None, None, Arc::new(|| {}));
        let info = listed(42, "Fork");
        link.answer(
            Ask::Fork,
            Response::Sessions {
                sessions: vec![info],
            },
        );
        assert!(link.sessions.iter().any(|info| info.session == 42));
        assert_eq!(link.workspace.page, Page::Session(42));
    }

    /// Closing the window while an agent works asks first when the settings say so, and closes once confirmed.
    #[test]
    fn closing_on_working_agents_asks_first() {
        let mut link = Link::built(None, None, Arc::new(|| {}));
        link.workspace.projects = vec![crate::model::Project {
            sessions: vec![crate::model::Session {
                status: crate::model::SessionStatus::Working,
                ..crate::model::Session::default()
            }],
            ..crate::model::Project::default()
        }];
        assert!(!link.may_close());
        assert_eq!(link.workspace.quitting, Some(1));
        link.confirm_close(true);
        assert!(link.may_close());
    }

    /// Renaming a session changes its title in the window and clears the dialog.
    #[test]
    fn renaming_changes_the_title() {
        let mut link = Link::built(None, None, Arc::new(|| {}));
        link.sessions.push(listed(3, "Old"));
        link.session_command(SessionCommand::Rename(3));
        assert_eq!(
            link.workspace.renaming.as_ref().map(|r| r.title.as_str()),
            Some("Old")
        );
        link.session_command(SessionCommand::Retitle("New".to_owned()));
        link.session_command(SessionCommand::ConfirmRename);
        assert_eq!(link.workspace.renaming, None);
        assert_eq!(link.sessions[0].title, "New");
    }
}
