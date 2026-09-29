//! What a review has git do to its worktree, carried out away from the window.
//!
//! Staging, throwing away and committing are each a subprocess or more, and a
//! commit runs the repository's hooks, which can take minutes. So the review
//! only says what is to be done, as a [`Work`], and the window carries it out
//! on a thread of its own, one at a time per worktree, and hands back what
//! git said as a [`Done`] for the review to take in.

use std::path::PathBuf;

/// What git said in each repository a piece of work was carried out in.
type Heard = Vec<(PathBuf, pm_core::Said)>;

/// Something to have git do to one worktree, not done yet.
pub struct Work {
    /// The repositories it is carried out in, whose buttons show it.
    roots: Vec<PathBuf>,
    /// What those buttons say meanwhile: "Staging…".
    doing: &'static str,
    /// Whether the message of a repository it went through in is cleared.
    commits: bool,
    /// Whether the branch is pushed once git has done it.
    pushes: bool,
    /// Carries it out, answering what git said in each repository.
    run: Box<dyn FnOnce() -> Heard + Send>,
}

impl Work {
    /// Work in the repositories at `roots`, shown on their buttons as
    /// `doing`, carried out by `run`.
    pub(super) fn new(
        roots: Vec<PathBuf>,
        doing: &'static str,
        run: impl FnOnce() -> Heard + Send + 'static,
    ) -> Self {
        Self {
            roots,
            doing,
            commits: false,
            pushes: false,
            run: Box::new(run),
        }
    }

    /// The same work, as a commit whose message goes once it is made.
    pub(super) fn committing(mut self) -> Self {
        self.commits = true;
        self
    }

    /// The same work, followed by a push of the branch once it went through.
    pub fn then_push(mut self) -> Self {
        self.pushes = true;
        self
    }

    /// Whether it is carried out in the repository at `root`.
    pub(super) fn is_in(&self, root: &std::path::Path) -> bool {
        self.roots.iter().any(|held| held == root)
    }

    /// What the buttons of its repositories say meanwhile.
    pub(super) fn doing(&self) -> &'static str {
        self.doing
    }

    /// Carries the work out, on whichever thread it is called on.
    pub fn run(self) -> Done {
        Done {
            commits: self.commits,
            pushes: self.pushes,
            heard: (self.run)(),
        }
    }
}

/// What git said once a piece of work had been carried out.
pub struct Done {
    /// Whether it was a commit, whose message goes once it is made.
    pub(super) commits: bool,
    /// Whether the branch is to be pushed now that it has gone through.
    pushes: bool,
    /// What git said in each repository it was carried out in.
    pub(super) heard: Heard,
}

impl Done {
    /// Git's words for any failed work, before the review takes this result.
    pub fn troubles(&self) -> Vec<String> {
        self.heard
            .iter()
            .filter_map(|(_, said)| said.as_ref().err().cloned())
            .filter(|words| !words.is_empty())
            .collect()
    }

    /// Whether git did everything it was asked to.
    pub(super) fn went_through(&self) -> bool {
        self.heard.iter().all(|(_, said)| said.is_ok())
    }

    /// Whether the branch is to be pushed now: it was asked for, and what
    /// came before it went through.
    pub fn wants_push(&self) -> bool {
        self.pushes && self.went_through()
    }
}
