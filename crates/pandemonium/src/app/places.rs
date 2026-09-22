//! Where the window has been: what it jumped from, and what it closed.
//!
//! A place is named by its worktree and its path rather than by the id this
//! run gave an open file, because both of the things it is for outlive the
//! file being open: going back to where a definition was asked for, and
//! opening again the tab that was closed last.

use std::path::PathBuf;

use pm_core::ProjectId;
use pm_text::Position;

/// How many places back the trail remembers.
const DEPTH: usize = 64;

/// One place in one project's worktree.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Place {
    /// The project whose worktree the file belongs to.
    pub project: ProjectId,
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
            places.retain(|place| place.project != project);
        }
    }
}

/// Drops the oldest places once there are more than the trail keeps.
fn trim(places: &mut Vec<Place>) {
    if places.len() > DEPTH {
        places.drain(..places.len() - DEPTH);
    }
}
