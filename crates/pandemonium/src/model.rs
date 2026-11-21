//! The screen the app holds, hoisted in one rustle state that every window's
//! root reads, and that the app's logic and its work change.
//!
//! Every change composes the roots again. The model counts the changes, as the
//! screen's revision, except those that only resize panels, so the regions of the
//! screen keyed by the revision and by their own size
//! ([`pm_components::revision::region`]) are composed again only when what they
//! show or their room changed.

use std::cell::Cell;
use std::rc::Rc;

use pm_pages::{Screen, WorkspaceState};

/// The screen as a rustle state. Every clone is a handle to the same screen.
#[derive(Clone)]
pub struct Model {
    /// The screen, read by the root of every window, which a change composes again.
    state: rustle::State<Screen>,
    /// The screen's revision: how many changes there were that are not only of the
    /// sizes of panels, and how often the windows were composed again for something
    /// outside the screen.
    revision: Rc<Cell<u64>>,
    /// How many changes of any kind there were, resizes of panels included.
    generation: Rc<Cell<u64>>,
}

impl Model {
    /// A model holding `screen`.
    pub fn new(screen: Screen) -> Self {
        Self {
            state: rustle::mutable_state(screen),
            revision: Rc::new(Cell::new(0)),
            generation: Rc::new(Cell::new(0)),
        }
    }

    /// Reads the screen through `read` and returns what it returns.
    pub fn read<R>(&self, read: impl FnOnce(&Screen) -> R) -> R {
        self.state.with(read)
    }

    /// Changes the screen in place through `change` and moves its revision on.
    pub fn update(&self, change: impl FnOnce(&mut Screen)) {
        self.state.update(change);
        self.generation.set(self.generation.get() + 1);
        self.revise();
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
        self.generation.set(self.generation.get() + 1);
    }

    /// Moves the screen's revision on, for a change of something the views read
    /// besides the screen, so every region is composed again.
    pub fn revise(&self) {
        self.revision.set(self.revision.get() + 1);
    }

    /// The screen's revision; see [`Model::revise`].
    pub fn revision(&self) -> u64 {
        self.revision.get()
    }

    /// How many changes of the screen there were, of any kind, which effects that follow
    /// every change are keyed on.
    pub fn generation(&self) -> u64 {
        self.generation.get()
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

    /// Every change, a resize included, moves the generation on.
    #[test]
    fn every_change_moves_the_generation() {
        let model = Model::new(Screen::default());
        let before = model.generation();
        model.resize(|_| {});
        model.update(|_| {});
        assert_eq!(model.generation(), before + 2);
    }
}
