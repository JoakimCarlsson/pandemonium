//! The screen's observable state, provided to the regions of the workspace that take the
//! whole workspace, so each region reads the state it shows itself: it is composed again
//! when the screen changed or the room it is given did, and a drag of a panel's handle,
//! which changes only sizes held apart from the screen, leaves the regions whose room did
//! not change as they were.

use std::rc::Rc;

use rustle::CompositionLocal;

use crate::Screen;

/// A handle to the observable state that holds the screen. Handles are equal when they
/// are to the same state.
#[derive(Clone)]
pub struct ScreenState(Rc<rustle::State<Screen>>);

impl ScreenState {
    /// A handle to `state`.
    pub fn new(state: rustle::State<Screen>) -> Self {
        Self(Rc::new(state))
    }
}

impl PartialEq for ScreenState {
    /// Whether both handles are to the same state.
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

/// The screen's state the views being composed show, when they show one that changes.
static LOCAL_SCREEN: CompositionLocal<Option<ScreenState>> = CompositionLocal::new(|| None);

/// Composes `content` showing the screen held in `screen`.
pub fn provide<R>(screen: ScreenState, content: impl FnOnce() -> R) -> R {
    LOCAL_SCREEN.provide(Some(screen), content)
}

/// Composes `content`, a region of the screen given the room `room` (the sizes it lays
/// out with, such as its own width and the window's), as a block that reads the screen's
/// state itself: it runs again when the screen changed, when `room` changed or when other
/// state it read did, and otherwise emits what it emitted last time. A screen shown once,
/// with no state provided, is composed once for each room.
#[track_caller]
pub(crate) fn region<D: PartialEq + 'static>(room: D, content: impl FnOnce()) {
    let screen = LOCAL_SCREEN.current();
    rustle::kept(room, move || {
        if let Some(screen) = &screen {
            screen.0.with(|_| ());
        }
        content();
    });
}
