//! The screen the app holds, hoisted in one rustle state that every window's
//! root reads, and that the app's logic and its work change.
//!
//! Every change composes the roots again, and the block of the main window's root
//! that follows the screen runs the app's reaction to it as a side effect. The
//! title bar and the plan panel are composed again only when their own state,
//! compared by value, changed. The other regions of the screen take the whole
//! workspace, so the model still counts the changes, as the screen's revision,
//! except those that only resize panels, and those regions are keyed by the
//! revision and by their own size ([`pm_components::revision::region`]) so they
//! are composed again only when the screen or their room changed.

use std::cell::Cell;
use std::rc::Rc;

use pm_pages::{Screen, WorkspaceState};

/// The screen as a rustle state. Every clone is a handle to the same screen.
#[derive(Clone)]
pub struct Model {
    /// The screen, read by the root of every window, which a change composes again.
    state: rustle::State<Screen>,
    /// The screen's revision: how many changes there were that are not only of the
    /// sizes of panels.
    revision: Rc<Cell<u64>>,
}

impl Model {
    /// A model holding `screen`.
    pub fn new(screen: Screen) -> Self {
        Self {
            state: rustle::mutable_state(screen),
            revision: Rc::new(Cell::new(0)),
        }
    }

    /// Reads the screen through `read` and returns what it returns.
    pub fn read<R>(&self, read: impl FnOnce(&Screen) -> R) -> R {
        self.state.with(read)
    }

    /// Changes the screen in place through `change` and moves its revision on.
    pub fn update(&self, change: impl FnOnce(&mut Screen)) {
        self.state.update(change);
        self.revision.set(self.revision.get() + 1);
    }

    /// Replaces the workspace the screen shows with what `change` makes of it,
    /// as [`Model::update`] does.
    pub fn change(&self, change: impl FnOnce(WorkspaceState) -> WorkspaceState) {
        self.update(|screen| *screen = std::mem::take(screen).in_workspace(change));
    }

    /// Changes only the sizes of panels in the screen through `change`, leaving its
    /// revision, so only the regions whose room changed are
    /// composed again. `change` must change nothing but such sizes.
    pub fn resize(&self, change: impl FnOnce(&mut Screen)) {
        self.state.update(change);
    }

    /// The screen's revision: how many changes there were that are not only of the
    /// sizes of panels.
    pub fn revision(&self) -> u64 {
        self.revision.get()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A change through any handle is seen through every other.
    #[test]
    fn a_change_reaches_every_handle() {
        let model = Model::new(Screen::default());
        let other = model.clone();
        other.change(|state| state.with_fonts(vec!["Inter".to_owned()], Vec::new()));
        assert!(model.read(|screen| *screen != Screen::default()));
    }

    /// A change moves the revision on and a resize leaves it.
    #[test]
    fn only_a_resize_leaves_the_revision() {
        let model = Model::new(Screen::default());
        let before = model.revision();
        model.resize(|_| {});
        assert_eq!(model.revision(), before);
        model.update(|_| {});
        assert_eq!(model.revision(), before + 1);
    }
}
