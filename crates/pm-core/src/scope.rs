//! The worktree everything else is scoped to.

use crate::project::ProjectId;
use crate::session::SessionId;

/// One worktree of one project: its checkout, or one of its sessions.
///
/// A project is a repository the window holds open, and it is read through
/// one worktree at a time — the checkout the reader works in, or a session
/// cut beside it. Everything that belongs to a worktree rather than to a
/// repository is scoped by this: the tabs a pane draws, the shells running,
/// the file tree, the changes. A project id alone answers "which repository";
/// this answers "which copy of it", which is what a reader is looking at.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Scope {
    /// The project the worktree belongs to.
    project: ProjectId,
    /// The session it is, or nothing at all for the project's own checkout.
    session: Option<SessionId>,
}

impl Scope {
    /// The project's own checkout.
    pub fn checkout(project: ProjectId) -> Self {
        Self {
            project,
            session: None,
        }
    }

    /// The worktree of `session`, which is a worktree of `project`.
    pub fn of(project: ProjectId, session: SessionId) -> Self {
        Self {
            project,
            session: Some(session),
        }
    }

    /// The project the worktree belongs to.
    pub fn project(self) -> ProjectId {
        self.project
    }

    /// The session it is, where it is one rather than the checkout.
    pub fn session(self) -> Option<SessionId> {
        self.session
    }

    /// Whether this is the project's own checkout.
    pub fn is_checkout(self) -> bool {
        self.session.is_none()
    }
}
