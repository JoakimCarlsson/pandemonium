//! The screen's observable state, provided to the regions of the workspace that take the
//! whole workspace, so each region reads the state it shows itself: it is composed again
//! when the screen changed or the room it is given did, and a drag of a panel's handle,
//! which changes only sizes held apart from the screen, leaves the regions whose room did
//! not change as they were.
//!
//! The screen's changes are counted by three generations: one for every change but the words
//! an agent streams into a conversation and whether they were read, one for those words, and
//! one for what they change of the window's chrome, the sidebar and the title bar. The regions
//! that can show a conversation follow the first two, the chrome follows the first and the
//! last, and the rest of the window, keyed on the first, is left as it was while an agent
//! writes.

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
    /// The generation of what streamed words and reading them change of the chrome.
    chrome: rustle::State<u64>,
}

impl ScreenState {
    /// Handles to the generations `screen`, of the screen's changes, `conversation`, of what
    /// is streamed into its conversations, and `chrome`, of what that and reading it change
    /// of the sidebar and the title bar.
    pub fn new(
        screen: rustle::State<u64>,
        conversation: rustle::State<u64>,
        chrome: rustle::State<u64>,
    ) -> Self {
        Self(Rc::new(Generations {
            screen,
            conversation,
            chrome,
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

/// Follows the generation of the screen being composed and that of its chrome, which moves
/// on when streamed words or reading them change the sidebar or the title bar, so the caller
/// composes again when either changes; nothing for a screen shown once.
pub(crate) fn follow_chrome() {
    if let Some(screen) = LOCAL_SCREEN.current() {
        screen.0.screen.with(|_| ());
        screen.0.chrome.with(|_| ());
    }
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
