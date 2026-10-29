//! The places the reader has been in the workspace, walked back and forward like the history of a browser: every page, every view of a session, and every file shown with the place of its caret.

use pm_protocol::Position;

use crate::open_files::{Contents, jump};
use crate::state::{Page, Tab, WorkspaceState};

/// A file shown in a place, and where its caret was when the reader left it.
#[derive(Clone, Debug, Eq, PartialEq)]
struct Showing {
    /// The file.
    path: String,
    /// Whether it is the file as it is in the last commit.
    head: bool,
    /// Where the caret was when the place was left by a jump or a step; nothing while the place is the one showing, or when it was left some other way, in which case the file is shown as it is.
    caret: Option<Position>,
}

/// One place in the workspace: the page and, on a session, the view of it and the file it shows.
#[derive(Clone, Debug, Eq, PartialEq)]
struct Place {
    /// The page.
    page: Page,
    /// The view of the session.
    tab: Tab,
    /// The file the view shows, when it is the view of a file.
    file: Option<Showing>,
}

impl Place {
    /// Whether `other` is the same page, view and file, wherever the caret is.
    fn is(&self, other: &Place) -> bool {
        self.page == other.page
            && self.tab == other.tab
            && self.file.as_ref().map(|file| (&file.path, file.head))
                == other.file.as_ref().map(|file| (&file.path, file.head))
    }
}

/// The places visited in order, and which of them is showing.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Trail {
    /// The places, oldest first.
    places: Vec<Place>,
    /// The position in `places` of the place showing.
    at: usize,
    /// Whether the caret is about to jump within the place showing, which makes the place it jumps to a place of its own.
    forked: bool,
}

impl Trail {
    /// Whether `place` differs from the one showing, or the one showing is about to be left by a jump, so that a place must be recorded.
    fn differs(&self, place: &Place) -> bool {
        self.forked || self.places.get(self.at).is_none_or(|held| !held.is(place))
    }

    /// Records `place` as the newest, dropping the places that were ahead of the one showing.
    fn visit(&mut self, place: Place) {
        if !self.places.is_empty() {
            self.places.truncate(self.at + 1);
        }
        self.places.push(place);
        self.at = self.places.len() - 1;
        self.forked = false;
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

    /// Notes where the caret of the file in the place showing is.
    fn note_caret(&mut self, caret: Option<Position>) {
        if let Some(file) = self
            .places
            .get_mut(self.at)
            .and_then(|place| place.file.as_mut())
        {
            file.caret = caret;
        }
    }
}

impl WorkspaceState {
    /// The place showing now.
    fn place_now(&self) -> Place {
        let file = self
            .git
            .shown
            .and_then(|index| self.git.viewers.get(index))
            .filter(|_| self.tab == Tab::File)
            .map(|viewer| Showing {
                path: viewer.path.clone(),
                head: viewer.head,
                caret: None,
            });
        Place {
            page: self.page,
            tab: self.tab,
            file,
        }
    }

    /// Where the caret of the file shown is, when a file with text is shown.
    fn caret_now(&self) -> Option<Position> {
        let document = self.shown_document().filter(|_| self.tab == Tab::File)?;
        Some(document.position_of(document.field.caret()))
    }

    /// Whether the place showing is not the one the trail ends at, so that [`WorkspaceState::track`] has something to record.
    pub fn untracked(&self) -> bool {
        self.trail.differs(&self.place_now())
    }

    /// Records the place showing in the trail when it is a new one.
    pub fn track(&mut self) {
        let place = self.place_now();
        if self.trail.differs(&place) {
            self.trail.visit(place);
        }
    }

    /// Remembers where the caret is before a jump, so that going back returns there; the place the caret jumps to is recorded as a place of its own.
    pub(crate) fn remember_spot(&mut self) {
        self.track();
        let caret = self.caret_now();
        self.trail.note_caret(caret);
        self.trail.forked = true;
    }

    /// Whether `place` can be shown: a session still there, a project still there, and a file that is open or can be opened again.
    fn exists(&self, place: &Place) -> bool {
        let page = match place.page {
            Page::Session(index) => index < self.sessions.len(),
            Page::New(project) | Page::Project(project) => project < self.projects.len(),
            Page::History | Page::Extensions => true,
        };
        page && place.file.as_ref().is_none_or(|file| {
            let open = || {
                self.review_of(place.page) == self.files_owner
                    && self
                        .git
                        .viewers
                        .iter()
                        .any(|viewer| viewer.path == file.path && viewer.head == file.head)
            };
            (!file.head && !file.path.starts_with('/')) || open()
        })
    }

    /// Shows the file `path`, with its caret at `caret` when that is known: the file open already, or the file of the project opened again.
    fn show_file(&mut self, path: &str, head: bool, caret: Option<Position>) {
        let row = self.editor.row_height();
        let columns = self.wrap_columns();
        let held = self
            .git
            .viewers
            .iter()
            .position(|viewer| viewer.path == path && viewer.head == head);
        let Some(index) = held else {
            self.open_viewer(path.to_owned(), head);
            if let Some(viewer) = self
                .git
                .shown
                .and_then(|index| self.git.viewers.get_mut(index))
            {
                viewer.jump = caret;
            }
            return;
        };
        self.git.shown = Some(index);
        self.tab = Tab::File;
        if let Some(viewer) = self.git.viewers.get_mut(index) {
            viewer.hidden = false;
            match (&mut viewer.contents, caret) {
                (Contents::Text(document), Some(position)) => {
                    viewer.scroll = jump(document, position, row, columns);
                }
                (Contents::Text(_), None) => {}
                (_, position) => viewer.jump = position,
            }
        }
    }

    /// The workspace moved `delta` places along the trail, towards the past when negative, staying put when there is no place that way.
    pub fn navigated(mut self, delta: isize) -> Self {
        self.track();
        let caret = self.caret_now();
        self.trail.note_caret(caret);
        let target = self.trail.nearest(delta, |place| self.exists(place));
        if let Some(position) = target {
            let place = self.trail.places[position].clone();
            self.trail.at = position;
            self.trail.forked = false;
            self.page = place.page;
            self.tab = place.tab;
            self.sync_context();
            self.refresh_files();
            if let Some(file) = &place.file {
                self.show_file(&file.path, file.head, file.caret);
            }
        }
        self
    }
}
