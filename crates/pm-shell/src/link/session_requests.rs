//! What the menu of a session or chat in the sidebar does: renaming it,
//! closing it, marking it read or unread, and opening its folder.

use pm_protocol::{Request, Response};

use super::{Ask, Link};
use crate::page::Page;

/// What the reader does to a session or chat from its row in the sidebar.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SessionAction {
    /// Open the menu of the session with this number, or close the one open.
    Menu(Option<u64>),
    /// Start renaming the session with this number in its row.
    StartRename(u64),
    /// The title typed so far while renaming.
    RenameText(String),
    /// Give the session being renamed the title typed.
    CommitRename,
    /// Stop renaming without changing the title.
    CancelRename,
    /// Close the session with this number, keeping its log; a worktree with uncommitted changes is kept.
    Close(u64),
    /// Remove the chat with this number, its scratch folder with it.
    Remove(u64),
    /// Mark the session with this number unread, or read when it already is.
    ToggleRead(u64),
    /// Show the folder of the session with this number in the file manager, when it is on this machine.
    OpenFolder(u64),
}

impl Link {
    /// Does what the reader asked of a session or chat from its row in the sidebar.
    pub fn session_action(&mut self, action: SessionAction) {
        match action {
            SessionAction::Menu(session) => self.workspace.session_menu = session,
            SessionAction::StartRename(session) => {
                self.workspace.session_menu = None;
                let title = self
                    .session_info(session)
                    .map(|info| info.title)
                    .unwrap_or_default();
                self.workspace.renaming = Some((session, title));
            }
            SessionAction::RenameText(text) => {
                if let Some((_, title)) = &mut self.workspace.renaming {
                    *title = text;
                }
            }
            SessionAction::CommitRename => self.commit_rename(),
            SessionAction::CancelRename => self.workspace.renaming = None,
            SessionAction::Close(session) => self.close_session(session, false),
            SessionAction::Remove(session) => self.close_session(session, true),
            SessionAction::ToggleRead(session) => {
                self.workspace.session_menu = None;
                let thread = self.threads.entry(session).or_default();
                if thread.is_unread() {
                    thread.mark_read();
                    let seq = thread.seq;
                    if self.routes.remote(session).is_none() {
                        self.workspace.read_through.insert(session, seq);
                    }
                } else {
                    thread.mark_unread();
                }
                self.rebuild();
            }
            SessionAction::OpenFolder(session) => {
                self.workspace.session_menu = None;
                let folder = self
                    .session_info(session)
                    .map(|info| info.directory)
                    .filter(|folder| !folder.is_empty() && !folder.starts_with("ssh://"));
                if let Some(folder) = folder {
                    show_in_file_manager(&folder);
                }
            }
        }
    }

    /// Gives the session being renamed the title typed, unless it is blank or unchanged.
    fn commit_rename(&mut self) {
        let Some((session, title)) = self.workspace.renaming.take() else {
            return;
        };
        let title = title.trim().to_owned();
        let known = self.session_info(session).map(|info| info.title);
        if title.is_empty() || known.as_deref() == Some(title.as_str()) {
            return;
        }
        if let Some(info) = self
            .sessions
            .iter_mut()
            .find(|info| info.session == session)
        {
            info.title = title.clone();
        }
        if let Some(thread) = self.threads.get_mut(&session) {
            thread.title = Some(title.clone());
        }
        if self.watched == Some(session) {
            self.workspace.title = title.clone();
        }
        self.ask(Request::RenameSession { session, title }, Ask::Nothing);
        self.rebuild();
    }

    /// Asks the server to close `session`, removing a chat's scratch folder when `discard`.
    fn close_session(&mut self, session: u64, discard: bool) {
        self.workspace.session_menu = None;
        self.ask(
            Request::CloseSession { session, discard },
            Ask::Closed(session),
        );
    }

    /// Takes the server's answer to closing `session`: drops it from the sidebar, leaving it if it was open, or says why it could not be closed.
    pub(super) fn closed(&mut self, session: u64, response: Response) {
        if let Response::Failed { message } = response {
            self.workspace.status = Some(message);
            return;
        }
        if let Some(info) = self
            .sessions
            .iter_mut()
            .find(|info| info.session == session)
        {
            info.closed = true;
        }
        self.reopened.remove(&session);
        if self.watched == Some(session) {
            self.navigate(Page::Home);
        }
        self.rebuild();
    }
}

/// Shows `folder` in the desktop's file manager; a file manager that cannot be started is ignored.
fn show_in_file_manager(folder: &str) {
    let opener = if cfg!(target_os = "macos") {
        "open"
    } else if cfg!(target_os = "windows") {
        "explorer"
    } else {
        "xdg-open"
    };
    let folder = super::expand_home(folder);
    if let Err(error) = std::process::Command::new(opener)
        .arg(folder)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
    {
        eprintln!("could not open the folder: {error}");
    }
}
