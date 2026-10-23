//! The places the reader has been in the workspace, walked back and forward.

use crate::state::{Page, Tab, WorkspaceState};

/// One place in the workspace: the page and, on a session, the view of it.
type Place = (Page, Tab);

/// The places visited in order, and which of them is showing.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Trail {
    /// The places, oldest first.
    places: Vec<Place>,
    /// The position in `places` of the place showing.
    at: usize,
}

impl Trail {
    /// Whether `place` differs from the one showing, so that it must be recorded.
    fn differs(&self, place: Place) -> bool {
        self.places.get(self.at) != Some(&place)
    }

    /// Records `place` as the newest, dropping the places that were ahead of the one showing.
    fn visit(&mut self, place: Place) {
        if !self.places.is_empty() {
            self.places.truncate(self.at + 1);
        }
        self.places.push(place);
        self.at = self.places.len() - 1;
    }

    /// The position of the nearest place from the one showing in the direction of `delta` that `valid` accepts.
    fn nearest(&self, delta: isize, valid: impl Fn(&Place) -> bool) -> Option<usize> {
        let mut position = self.at as isize + delta;
        while let Some(place) = usize::try_from(position)
            .ok()
            .and_then(|index| self.places.get(index))
        {
            if valid(place) {
                return Some(position as usize);
            }
            position += delta;
        }
        None
    }
}

impl WorkspaceState {
    /// Whether the place showing is not the one the trail ends at, so that [`WorkspaceState::track`] has something to record.
    pub fn untracked(&self) -> bool {
        self.trail.differs((self.page, self.tab))
    }

    /// Records the place showing in the trail when it is a new one.
    pub fn track(&mut self) {
        let place = (self.page, self.tab);
        if self.trail.differs(place) {
            self.trail.visit(place);
        }
    }

    /// Whether `place` can be shown: a session still there, a project still there.
    fn exists(&self, place: &Place) -> bool {
        match place.0 {
            Page::Session(index) => index < self.sessions.len(),
            Page::New(project) | Page::Project(project) => project < self.projects.len(),
            Page::History | Page::Extensions => true,
        }
    }

    /// The workspace moved `delta` places along the trail, towards the past when negative, staying put when there is no place that way.
    pub fn navigated(mut self, delta: isize) -> Self {
        self.track();
        let target = self.trail.nearest(delta, |place| self.exists(place));
        if let Some(position) = target {
            let (page, tab) = self.trail.places[position];
            self.trail.at = position;
            self.page = page;
            self.tab = tab;
            self.sync_context();
            self.fill_terminal_dock();
            self.refresh_files();
        }
        self
    }
}
