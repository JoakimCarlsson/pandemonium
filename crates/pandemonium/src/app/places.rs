//! Where the window has been: what it jumped from, and what it closed.
//!
//! A place is named by its worktree and its path rather than by the id this
//! run gave an open file, because both of the things it is for outlive the
//! file being open: going back to where a definition was asked for, and
//! opening again the tab that was closed last.

use std::path::PathBuf;

use pm_core::{ProjectId, Scope};
use pm_text::Position;

/// How many places back the trail remembers.
const DEPTH: usize = 64;

/// One place in one worktree.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Place {
    /// The worktree the file belongs to.
    pub scope: Scope,
    /// Where the file lives.
    pub path: PathBuf,
    /// Where in it the cursor was.
    pub position: Position,
}

/// Where the window has been, in the order it was there.
#[derive(Default)]
pub struct Trail {
    /// The places jumped away from, oldest first.
    back: Vec<Place>,
    /// The places gone back from, oldest first.
    forward: Vec<Place>,
    /// The tabs closed, oldest first.
    closed: Vec<Place>,
}

impl Trail {
    /// Takes down `from` as a place a jump has just left.
    ///
    /// Jumping anywhere discards what going back had made available to go
    /// forward to, the way a browser's history behaves: the trail is a line
    /// and not a tree.
    pub fn jumped(&mut self, from: Place) {
        if self.back.last() == Some(&from) {
            return;
        }
        self.back.push(from);
        self.forward.clear();
        trim(&mut self.back);
    }

    /// The place to go back to, `from` being where the cursor is now.
    pub fn back(&mut self, from: Place) -> Option<Place> {
        let place = self.back.pop()?;
        self.forward.push(from);
        trim(&mut self.forward);
        Some(place)
    }

    /// The place to go forward to, `from` being where the cursor is now.
    pub fn forward(&mut self, from: Place) -> Option<Place> {
        let place = self.forward.pop()?;
        self.back.push(from);
        trim(&mut self.back);
        Some(place)
    }

    /// Takes down a tab that has just been closed.
    pub fn closed(&mut self, place: Place) {
        self.closed.push(place);
        trim(&mut self.closed);
    }

    /// The tab that was closed last, taken off the list.
    pub fn reopen(&mut self) -> Option<Place> {
        self.closed.pop()
    }

    /// Forgets everything about `project`, which is no longer open.
    pub fn close_project(&mut self, project: ProjectId) {
        for places in [&mut self.back, &mut self.forward, &mut self.closed] {
            places.retain(|place| place.scope.project() != project);
        }
    }
}

/// Drops the oldest places once there are more than the trail keeps.
fn trim(places: &mut Vec<Place>) {
    if places.len() > DEPTH {
        places.drain(..places.len() - DEPTH);
    }
}

/// The file in the worktree at `root` that `link` names, and the line in it
/// counted from nought, where it names a file that is there.
///
/// The link is the agent's to write, so a file it names outside the worktree
/// — by an absolute path, or by climbing out through `..` or a link — is not
/// opened: the conversation is about the worktree it was started in.
///
/// A line is read from the `#L12` an address in a browser would carry, or
/// from the `:12` or `:12:4` a compiler writes after a path.
pub(super) fn linked_file(
    root: impl Into<pm_host::Location>,
    link: &str,
) -> Option<(PathBuf, usize)> {
    let root = root.into();
    let path = match link.split_once("://") {
        Some(("file", path)) => path,
        Some(_) => return None,
        None => link,
    };
    let (path, line) = match path.split_once("#L") {
        Some((path, line)) => (
            path,
            line.split('-').next().and_then(|line| line.parse().ok()),
        ),
        None => after_colons(path),
    };
    let path = root.join(path.replace("%20", " "));
    let resolved = root.host.fs().canonicalize(&path).ok()?;
    let inside = root
        .host
        .fs()
        .canonicalize(&root)
        .is_ok_and(|root| resolved.starts_with(root));
    (inside && root.host.fs().is_file(&resolved))
        .then(|| (path, line.unwrap_or(1_usize).saturating_sub(1)))
}

/// `path` without the `:line` or `:line:column` written after it, and the
/// line, where one was.
fn after_colons(path: &str) -> (&str, Option<usize>) {
    match numbered(path) {
        Some((rest, last)) => match numbered(rest) {
            Some((file, line)) => (file, line.parse().ok()),
            None => (rest, last.parse().ok()),
        },
        None => (path, None),
    }
}

/// `path` split before the number written after its last colon, where a
/// number is what follows it.
fn numbered(path: &str) -> Option<(&str, &str)> {
    path.rsplit_once(':')
        .filter(|(_, number)| number.parse::<usize>().is_ok())
}
