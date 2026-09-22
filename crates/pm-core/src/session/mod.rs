//! A session: an agent's own worktree of a project, and the set of them.
//!
//! A project is the repository the window holds open — the checkout the
//! reader works in, and the truth every session is cut from. A session is a
//! worktree beside it, detached at the commit it was cut from, which an agent
//! is turned loose in: its files, its drift and its agent are the reader's to
//! browse the way the checkout is, and finishing it takes the worktree away
//! again.
//!
//! [`Sessions`] is the one seam a session is started and finished through —
//! the palette, a keybinding and a restored window all go through it, and
//! nothing else cuts or tears down a worktree. [`placement`] says where the
//! worktree goes; [`bootstrap`] says what it is given once it is there;
//! [`store`] is the set itself.

mod bootstrap;
mod placement;
mod store;

pub use bootstrap::Bootstrap;
pub use placement::slug;
pub use store::{Sessions, StartError, Started};

use std::path::{Path, PathBuf};

use crate::git::Summary;
use crate::project::ProjectId;

/// A session's identity for as long as the window holds it.
///
/// The id is handed out by [`Sessions`] and means nothing outside the window
/// that issued it, exactly as a [`ProjectId`] does: a pane, a tab and a
/// keybinding all name the same session without naming a path.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SessionId(u64);

impl SessionId {
    /// The id following this one, for the store handing them out.
    fn next(self) -> Self {
        Self(self.0 + 1)
    }
}

/// An agent's worktree of one project.
#[derive(Clone, Debug)]
pub struct Session {
    /// What this session is called for as long as it is open.
    id: SessionId,
    /// The project whose repository it was cut from.
    project: ProjectId,
    /// What the reader called it, which is what a row says.
    name: String,
    /// The worktree itself.
    root: PathBuf,
    /// The checkout it was cut from, which is where git is run to end it.
    origin: PathBuf,
    /// The commit it was cut from, shortened.
    base: String,
    /// The local port this session serves on, where one was free.
    port: Option<u16>,
    /// How far it has drifted from that commit, as git last said.
    summary: Summary,
}

impl Session {
    /// What this session is called for as long as it is open.
    pub fn id(&self) -> SessionId {
        self.id
    }

    /// The project whose repository it was cut from.
    pub fn project(&self) -> ProjectId {
        self.project
    }

    /// What the reader called it.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The worktree the agent works in.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// The commit it was cut from.
    pub fn base(&self) -> &str {
        &self.base
    }

    /// The local port this session serves on, where one was free.
    ///
    /// A port is the session's own so that two of them serving the same
    /// project can both be looked at: what hands it to a program is
    /// [`Bootstrap::env`], which names it.
    pub fn port(&self) -> Option<u16> {
        self.port
    }

    /// How far it has drifted from that commit, as git last said.
    pub fn summary(&self) -> Summary {
        self.summary
    }

    /// Asks git again how far the worktree has drifted.
    fn refresh(&mut self) {
        self.summary = crate::git::since(&self.root, &self.base);
    }
}
