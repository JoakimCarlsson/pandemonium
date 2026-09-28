//! A session: an agent's own worktree of a project, and the set of them.
//!
//! A project is the folder the window holds open — the checkout the reader
//! works in, and the truth every session is cut from. A session is a copy of
//! it beside it: one worktree per repository the project holds, detached at
//! the commit each was cut from and laid out the way the project lays them
//! out, which an agent is turned loose in. Its files, its drift and its agent
//! are the reader's to browse the way the checkout is, and finishing it takes
//! the worktrees away again.
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
pub use store::{Cutting, Found, Sessions, StartError, Started};

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
///
/// A project that is one repository is cut as one worktree, and the session
/// is that worktree. A folder of several is cut as a folder of worktrees, one
/// where each repository sits, with what lies outside every repository copied
/// in beside them.
#[derive(Clone, Debug)]
pub struct Session {
    /// What this session is called for as long as it is open.
    id: SessionId,
    /// The project whose repositories it was cut from.
    project: ProjectId,
    /// What the reader called it, which is what a row says.
    name: String,
    /// The folder the agent works in: the one worktree, or the folder of them.
    root: PathBuf,
    /// The worktree cut of each repository, the outermost first.
    cuts: Vec<Cut>,
    /// The local port this session serves on, where one was free.
    port: Option<u16>,
    /// How far it has drifted from what it was cut from, as git last said.
    summary: Summary,
}

/// One repository's worktree within a session.
#[derive(Clone, Debug)]
struct Cut {
    /// The checkout it was cut from, which is where git is run to end it.
    origin: PathBuf,
    /// The worktree itself.
    root: PathBuf,
    /// The commit it was cut from, shortened.
    base: String,
}

impl Session {
    /// What this session is called for as long as it is open.
    pub fn id(&self) -> SessionId {
        self.id
    }

    /// The project whose repositories it was cut from.
    pub fn project(&self) -> ProjectId {
        self.project
    }

    /// What the reader called it.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The folder the agent works in.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// The root of every worktree cut for this session.
    pub fn roots(&self) -> impl Iterator<Item = &Path> {
        self.cuts.iter().map(|cut| cut.root.as_path())
    }

    /// The commit it was cut from, or the first of them for a session cut
    /// from several repositories.
    pub fn base(&self) -> &str {
        self.cuts.first().map_or("", |cut| cut.base.as_str())
    }

    /// The local port this session serves on, where one was free.
    ///
    /// A port is the session's own so that two of them serving the same
    /// project can both be looked at: what hands it to a program is
    /// [`Bootstrap::env`], which names it.
    pub fn port(&self) -> Option<u16> {
        self.port
    }

    /// How far it has drifted from what it was cut from, as git last said.
    pub fn summary(&self) -> Summary {
        self.summary
    }
}

/// How far `cuts` have drifted between them from what each was cut from.
fn drift(cuts: &[Cut]) -> Summary {
    cuts.iter()
        .map(|cut| crate::git::since(&cut.root, &cut.base))
        .fold(Summary::default(), |total, one| Summary {
            files: total.files + one.files,
            added: total.added + one.added,
            removed: total.removed + one.removed,
        })
}
