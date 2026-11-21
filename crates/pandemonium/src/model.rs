//! The screen the app holds, hoisted in one rustle state that every window's
//! root reads, and that the app's logic and its work change.
//!
//! Every change composes the roots again, and the block of the main window's root
//! that follows the screen runs the app's reaction to it as a side effect. The
//! title bar, the sidebar, the plan panel, the history page and the extensions page
//! are composed again only when their own state, compared by value, changed. The
//! other regions of the screen take the whole workspace and read the screen's state
//! themselves ([`pm_pages::observed`]), so they are composed again when the screen
//! or their room changed. The sizes of panels are held apart from the screen as
//! observable state, so dragging a handle writes no screen and leaves every region
//! whose room did not change as it was.

use pm_pages::observed::ScreenState;
use pm_pages::{Screen, WorkspaceState};

/// The screen as a rustle state. Every clone is a handle to the same screen.
#[derive(Clone)]
pub struct Model {
    /// The screen, read by the root of every window and by each region that shows it, which
    /// a change composes again.
    state: rustle::State<Screen>,
    /// A handle to the same state that the views are given, so the regions of the screen
    /// read it themselves.
    observed: ScreenState,
}

impl Model {
    /// A model holding `screen`.
    pub fn new(screen: Screen) -> Self {
        let state = rustle::mutable_state(screen);
        Self {
            observed: ScreenState::new(state.clone()),
            state,
        }
    }

    /// Reads the screen through `read` and returns what it returns.
    pub fn read<R>(&self, read: impl FnOnce(&Screen) -> R) -> R {
        self.state.with(read)
    }

    /// Changes the screen in place through `change`, which composes again what read it.
    pub fn update(&self, change: impl FnOnce(&mut Screen)) {
        self.state.update(change);
    }

    /// Replaces the workspace the screen shows with what `change` makes of it,
    /// as [`Model::update`] does.
    pub fn change(&self, change: impl FnOnce(WorkspaceState) -> WorkspaceState) {
        self.update(|screen| *screen = std::mem::take(screen).in_workspace(change));
    }

    /// The handle to the screen's state the views are given.
    pub fn observed(&self) -> ScreenState {
        self.observed.clone()
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

    /// A block that reads the screen runs again for a change of the screen, and not for a
    /// panel resized through the size it holds apart from the screen.
    #[test]
    fn only_a_held_resize_leaves_the_screen() {
        use pm_components::areas::WindowEvent;
        use pm_components::target::Target;
        use std::cell::Cell;
        use std::rc::Rc;
        let model = Model::new(Screen::default());
        let runs = Rc::new(Cell::new(0));
        let (reading, counted) = (model.clone(), Rc::clone(&runs));
        let mut ui = rustle::Embedded::without_renderer(
            move || {
                let (reading, counted) = (reading.clone(), Rc::clone(&counted));
                rustle::skippable((), move || {
                    reading.read(|_| ());
                    counted.set(counted.get() + 1);
                });
            },
            || {},
        );
        ui.resize((100, 100), 1.0);
        ui.update();
        let before = runs.get();
        let sash = Target::Window(WindowEvent::SidebarSash);
        assert!(model.read(|screen| screen.hold_sash_size(sash, 300.0, (1280.0, 800.0))));
        ui.update();
        assert_eq!(runs.get(), before);
        assert_eq!(model.read(|screen| screen.sizes.sidebar_width.get()), 300.0);
        model.update(|_| {});
        ui.update();
        assert_eq!(runs.get(), before + 1);
    }
}
