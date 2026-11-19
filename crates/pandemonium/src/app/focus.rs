//! The control the app keeps keyboard focus on, and how it follows rustle's.
//!
//! Rustle owns focus and moves it (Tab, a press on a field, Escape); the app keeps the
//! [`Target`] of the control holding it as the domain key its logic asks about, and asks
//! for it to move by keeping another one. The views read what the app keeps through the
//! [`FocusKeys`] the windows are composed with, request rustle's focus for that control and
//! tell the app back when rustle moved it (see [`pm_components::focus`]). What rustle says
//! while the app is busy composing is queued and taken in once the frame is composed.

use pm_components::target::Target;
use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

use pm_components::focus::FocusKeys;

use super::App;

/// What rustle said about its focus while the app was busy: a control and whether focus
/// came to it.
type Moves = Rc<RefCell<Vec<(Target, bool)>>>;

/// The control the app keeps keyboard focus on, shared with the views it composes.
#[derive(Clone, Default)]
pub(crate) struct KeptFocus {
    /// The control the app keeps focus on.
    held: Rc<Cell<Option<Target>>>,
    /// The control rustle last said holds its focus.
    shown: Rc<Cell<Option<Target>>>,
    /// What rustle said while the app was busy, oldest first.
    moves: Moves,
    /// What the views call when rustle moves focus, and to ask for focus on a control not
    /// composed yet, once the app is shared with them.
    on_moved: Rc<RefCell<Option<(OnMoved, Request)>>>,
}

/// What the views call to ask for focus on a control not composed yet.
type Request = Rc<dyn Fn(Target)>;

/// What the views call with a control and whether rustle's focus came to it.
type OnMoved = Rc<dyn Fn(Target, bool)>;

impl std::fmt::Debug for KeptFocus {
    /// The control focus is kept on and the one rustle last said holds it.
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("KeptFocus")
            .field("held", &self.held.get())
            .field("shown", &self.shown.get())
            .finish()
    }
}

impl KeptFocus {
    /// Focus kept on nothing.
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// The control focus is kept on.
    pub(crate) fn current(&self) -> Option<Target> {
        self.held.get()
    }

    /// Keeps focus on `id`: its control asks rustle for focus when it is next composed, and
    /// focus is let go after that frame when nothing composed takes it.
    pub(crate) fn focus(&self, id: Target) {
        self.held.set(Some(id));
    }

    /// Keeps focus on nothing: whatever holds rustle's focus frees it.
    pub(crate) fn clear(&self) {
        self.held.set(None);
    }

    /// Whether rustle's focus is on `id`, as it last said.
    pub(crate) fn shown_on(&self, id: Target) -> bool {
        self.shown.get() == Some(id)
    }

    /// Follows rustle's focus coming to `id` (`focused`) or leaving it.
    pub(crate) fn moved(&self, id: Target, focused: bool) {
        if focused {
            self.shown.set(Some(id));
            self.held.set(Some(id));
        } else {
            if self.shown.get() == Some(id) {
                self.shown.set(None);
            }
            if self.held.get() == Some(id) {
                self.held.set(None);
            }
        }
    }

    /// Takes in what rustle said while the app was busy, then lets go of focus kept on a
    /// control of the main window rustle did not give focus to in the frame just composed,
    /// as nothing composed took it. A control of a window beside the main one keeps it until
    /// that window shows it, which can be a frame or more after it was composed.
    pub(crate) fn settle(&self, beside: impl Fn(Target) -> bool) {
        let moves = std::mem::take(&mut *self.moves.borrow_mut());
        for (id, focused) in moves {
            self.moved(id, focused);
        }
        if let Some(held) = self.held.get()
            && self.shown.get() != Some(held)
            && !beside(held)
        {
            self.held.set(None);
        }
    }

    /// Tells `app` when rustle moves focus from now on, or queues what rustle says while
    /// `app` is busy, to be taken in by [`settle`](Self::settle).
    pub(crate) fn attach(&self, app: &Rc<RefCell<App>>) {
        let app = Rc::downgrade(app);
        let moves = Rc::clone(&self.moves);
        let on_moved: OnMoved = Rc::new(move |id, focused| {
            let Some(app) = Weak::upgrade(&app) else {
                return;
            };
            match app.try_borrow_mut() {
                Ok(mut app) => {
                    app.focus.moved(id, focused);
                    app.claimed.acted = true;
                }
                Err(_) => moves.borrow_mut().push((id, focused)),
            }
        });
        let held = Rc::clone(&self.held);
        let request: Request = Rc::new(move |id| held.set(Some(id)));
        *self.on_moved.borrow_mut() = Some((on_moved, request));
    }

    /// The focus the views are composed with: the control it is kept on, and what they
    /// call when rustle moves it.
    pub(crate) fn keys(&self) -> FocusKeys {
        match self.on_moved.borrow().as_ref() {
            Some((on_moved, request)) => FocusKeys {
                held: self.current(),
                on_moved: Rc::clone(on_moved),
                request: Rc::clone(request),
            },
            None => FocusKeys::resting(self.current()),
        }
    }
}
