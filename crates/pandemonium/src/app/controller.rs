//! The app as the views' [`Handler`]: what a control's press, click, wheel
//! or dropped file does, called by the interface while it handles the event.
//!
//! The interface runs these while the app is free, because the event loop
//! hands an event to the interface before it borrows the app for its own
//! handling of it (see [`Running`](super::Running)). What a handler finds is
//! kept in [`Claims`] for the rest of the app's handling of the same event,
//! which clears them when it is done.

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::{Rc, Weak};

use pm_components::events::Handler;
use pm_components::kit::FocusId;

use super::App;
use super::asides::Aside;

/// What the controls under the pointer said about the event being handled.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(super) struct Claims {
    /// The control the primary button went down on.
    pub(super) press: Option<FocusId>,
    /// Whether a control's view opened its menu for the secondary button.
    pub(super) context: bool,
    /// The control the primary button went down and came up on.
    pub(super) click: Option<FocusId>,
    /// The scrolling box the wheel turned over, and how far it can scroll.
    pub(super) wheel: Option<(FocusId, f32)>,
    /// Whether a handler changed the app, so a frame is due even for an
    /// event the app's own logic does not act on.
    pub(super) acted: bool,
}

/// The app as the views of one window see it.
pub(super) struct Controller {
    /// The app, while it runs.
    app: Weak<RefCell<App>>,
    /// The window beside the main one the views are in, or none for the main window.
    aside: Option<Aside>,
}

impl Controller {
    /// The handler of the views of the main window, or of `aside`'s, for `app`.
    pub(super) fn handler(app: &Rc<RefCell<App>>, aside: Option<Aside>) -> Rc<dyn Handler> {
        Rc::new(Self {
            app: Rc::downgrade(app),
            aside,
        })
    }

    /// Runs `act` on the app, in the window the views are in.
    ///
    /// # Panics
    ///
    /// When the app is already borrowed, which means the interface handled an
    /// event while the app was busy, against the order [`Running`](super::Running) keeps.
    fn with(&self, act: impl FnOnce(&mut App)) {
        let Some(app) = self.app.upgrade() else {
            return;
        };
        let mut app = app.borrow_mut();
        app.claimed.acted = true;
        match self.aside {
            Some(aside) => app.in_aside(aside, act),
            None => act(&mut app),
        }
    }
}

impl Handler for Controller {
    /// Takes the press, and keeps the control pressed so the app's handling
    /// of the press does not look for another.
    fn pressed(&self, id: FocusId) {
        self.with(|app| {
            app.claimed.press = Some(id);
            app.press(id);
        });
    }

    /// Carries out the click, and keeps the control clicked so the app's
    /// handling of the release does not look for another.
    fn clicked(&self, id: FocusId) {
        self.with(|app| {
            app.claimed.click = Some(id);
            app.click(id);
        });
    }

    /// Opens the control's menu, and keeps that it was opened so the app's
    /// handling of the press does not look for another.
    fn context_pressed(&self, id: FocusId) {
        self.with(|app| {
            app.claimed.context = true;
            app.context_press(Some(id));
        });
    }

    /// Keeps the box the wheel turned over for the app's handling of the wheel.
    fn scrolled(&self, id: FocusId, _: f32, room: f32) {
        self.with(|app| app.claimed.wheel = Some((id, room)));
    }

    /// Takes a file dropped on the window, unless the drag is the desktop's
    /// own one the app follows itself.
    fn dropped(&self, _: FocusId, path: PathBuf) {
        self.with(|app| {
            if !app.outside_offered() {
                app.file_dropped(path);
            }
        });
    }
}
