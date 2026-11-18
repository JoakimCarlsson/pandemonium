//! The screen the app holds, hoisted in one rustle state that every window's
//! root reads, and that the app's logic and its work change.

use std::cell::Cell;
use std::rc::Rc;

use pm_pages::Screen;

/// The screen as a rustle state, with a mark the app clears once it composed
/// for a change. Every clone is a handle to the same screen.
#[derive(Clone)]
pub struct Model {
    /// The screen, read by the root of every window, which a change composes again.
    state: rustle::State<Screen>,
    /// Whether the screen changed since the app last composed for it.
    changed: Rc<Cell<bool>>,
}

impl Model {
    /// A model holding `screen`, not marked as changed.
    pub fn new(screen: Screen) -> Self {
        Self {
            state: rustle::mutable_state(screen),
            changed: Rc::new(Cell::new(false)),
        }
    }

    /// Reads the screen through `read` and returns what it returns.
    pub fn read<R>(&self, read: impl FnOnce(&Screen) -> R) -> R {
        self.state.with(read)
    }

    /// Changes the screen in place through `change`, and marks it changed.
    pub fn update(&self, change: impl FnOnce(&mut Screen)) {
        self.state.update(change);
        self.changed.set(true);
    }

    /// A copy of the screen.
    pub fn get(&self) -> Screen {
        self.state.get()
    }

    /// The mark set by every change, for the app to clear once it composed.
    pub fn changes(&self) -> Rc<Cell<bool>> {
        Rc::clone(&self.changed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A change through any handle is seen through every other and marks the model changed.
    #[test]
    fn a_change_reaches_every_handle_and_marks_the_model() {
        let model = Model::new(Screen::default());
        let other = model.clone();
        let changes = model.changes();
        assert!(!changes.get());
        other.update(|screen| {
            *screen = std::mem::take(screen).with_fonts(vec!["Inter".to_owned()], Vec::new());
        });
        assert!(changes.get());
        assert!(model.read(|screen| *screen != Screen::default()));
    }
}
