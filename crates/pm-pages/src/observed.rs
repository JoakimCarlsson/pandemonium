//! The screen's observable state, provided to the regions of the workspace that take the
//! whole workspace, so each region reads the state it shows itself: it is composed again
//! when the screen changed or the room it is given did, and a drag of a panel's handle,
//! which changes only sizes held apart from the screen, leaves the regions whose room did
//! not change as they were.
//!
//! The screen's changes are counted by two generations: one for every change but the words
//! an agent streams into a conversation, and one for those. The regions that can show a
//! conversation follow both; the rest of the window, keyed on the first, is left as it was
//! while an agent writes.

use std::rc::Rc;

use rustle::CompositionLocal;

/// Handles to the generations that count the screen's changes. Handles are equal when they
/// are to the same states.
#[derive(Clone)]
pub struct ScreenState(Rc<Generations>);

/// The generations that count the screen's changes.
struct Generations {
    /// The generation of every change of the screen but the words streamed into conversations.
    screen: rustle::State<u64>,
    /// The generation of the words streamed into conversations.
    conversation: rustle::State<u64>,
}

impl ScreenState {
    /// Handles to the generations `screen`, of the screen's changes, and `conversation`, of
    /// what is streamed into its conversations.
    pub fn new(screen: rustle::State<u64>, conversation: rustle::State<u64>) -> Self {
        Self(Rc::new(Generations {
            screen,
            conversation,
        }))
    }
}

impl PartialEq for ScreenState {
    /// Whether both handles are to the same states.
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

/// The generation of the screen being composed, which moves on with every change of it but
/// the words streamed into its conversations, when one that changes is provided; nothing for
/// a screen shown once.
pub(crate) fn screen_generation() -> Option<u64> {
    LOCAL_SCREEN
        .current()
        .map(|screen| screen.0.screen.with(|generation| *generation))
}

/// Composes `content`, a region of the screen given the room `room` (the sizes it lays
/// out with, such as its own width and the window's), as a block that reads the screen's
/// state itself: it runs again when the screen changed, conversations included, when `room`
/// changed or when other state it read did, and otherwise emits what it emitted last time. A
/// screen shown once, with no state provided, is composed once for each room.
#[track_caller]
pub(crate) fn region<D: PartialEq + 'static>(room: D, content: impl FnOnce()) {
    let screen = LOCAL_SCREEN.current();
    rustle::kept(room, move || {
        if let Some(screen) = &screen {
            screen.0.screen.with(|_| ());
            screen.0.conversation.with(|_| ());
        }
        content();
    });
}

/// Composes `content` as [`region`] does, for a region that shows no conversation: it does
/// not run again for the words streamed into one.
#[track_caller]
pub(crate) fn quiet_region<D: PartialEq + 'static>(room: D, content: impl FnOnce()) {
    let screen = LOCAL_SCREEN.current();
    rustle::kept(room, move || {
        if let Some(screen) = &screen {
            screen.0.screen.with(|_| ());
        }
        content();
    });
}
