//! Moving between the tabs of the pane worked in, and bringing back the file tab closed last.

use crate::open_files::{Contents, Left, Viewer};
use crate::state::WorkspaceState;

/// How many closed tabs are remembered to bring back.
const REMEMBERED: usize = 20;

/// A file tab that was closed, with what is needed to put the tab back as it was.
#[derive(Clone, Debug, PartialEq)]
pub struct ClosedTab {
    /// The position of the session the tab was open in.
    pub session: usize,
    /// The file, from the root of the repository.
    pub path: String,
    /// Whether it was read from the last commit.
    pub head: bool,
    /// The caret in the text, as a byte offset, when it held text.
    pub caret: Option<usize>,
    /// How far down it was scrolled, in logical pixels.
    pub scroll: f32,
}

impl WorkspaceState {
    /// Remembers the tab of `viewer`, closed in the session at `session`, to bring back.
    pub(crate) fn remember_closed_tab(&mut self, session: usize, viewer: &Viewer) {
        if viewer.hidden {
            return;
        }
        let caret = match &viewer.contents {
            Contents::Text(document) => Some(document.field.caret()),
            _ => None,
        };
        self.closed_tabs
            .retain(|held| !(held.session == session && held.path == viewer.path));
        self.closed_tabs.push(ClosedTab {
            session,
            path: viewer.path.clone(),
            head: viewer.head,
            caret,
            scroll: viewer.scroll,
        });
        let surplus = self.closed_tabs.len().saturating_sub(REMEMBERED);
        self.closed_tabs.drain(..surplus);
    }

    /// Forgets the closed tabs of the file at `path` or of files in it, which no longer exist.
    pub(crate) fn forget_closed_tabs(&mut self, path: &str) {
        self.closed_tabs.retain(|held| {
            held.path != path
                && !held
                    .path
                    .strip_prefix(path)
                    .is_some_and(|rest| rest.starts_with('/'))
        });
    }

    /// The state showing the tab `delta` tabs along from the one shown in the pane worked in, going round the ends.
    pub fn tab_stepped(mut self, delta: isize) -> Self {
        let panes = self.panes();
        let Some(group) = panes.layout.groups().get(panes.focused).copied().cloned() else {
            return self;
        };
        let count = group.items.len();
        if count < 2 {
            return self;
        }
        let current = self.current_item();
        let at = group
            .items
            .iter()
            .position(|item| *item == current)
            .unwrap_or(group.active);
        let next = (at as isize + delta).rem_euclid(count as isize) as usize;
        self.show_item(&group.items[next]);
        self.settle_panes();
        self
    }

    /// The state with the file tab closed last in this session open again, with its caret and scroll, or as it is when there is none to bring back.
    pub fn tab_reopened(mut self) -> Self {
        let Some(session) = self.review_index() else {
            return self;
        };
        let Some(at) = self
            .closed_tabs
            .iter()
            .rposition(|held| held.session == session)
        else {
            return self;
        };
        let closed = self.closed_tabs.remove(at);
        self.open_viewer(closed.path.clone(), closed.head);
        if let Some(viewer) = self
            .git
            .viewers
            .iter_mut()
            .find(|viewer| viewer.path == closed.path && viewer.head == closed.head)
        {
            viewer.preview = false;
            if matches!(viewer.contents, Contents::Loading) {
                viewer.scroll = closed.scroll;
                viewer.left = closed.caret.map(|caret| Left {
                    unsaved: None,
                    caret,
                });
            }
        }
        self
    }
}
