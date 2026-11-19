//! The frame of the main window, drawn by the window itself: moving it by its title bar, maximizing it with a second press there, resizing it by the edges its views lay over it, and the buttons that minimize, maximize and close it.

use pm_components::SHADOW_MARGIN;
use pm_components::title_bar::FRAMELESS;
use pm_pages::Screen;
use rustle::ResizeEdge;
use winit::window::ResizeDirection;

use super::App;

/// The edge winit resizes a window by for the edge rustle names.
fn direction_of(edge: ResizeEdge) -> ResizeDirection {
    match edge {
        ResizeEdge::North => ResizeDirection::North,
        ResizeEdge::South => ResizeDirection::South,
        ResizeEdge::West => ResizeDirection::West,
        ResizeEdge::East => ResizeDirection::East,
        ResizeEdge::NorthWest => ResizeDirection::NorthWest,
        ResizeEdge::NorthEast => ResizeDirection::NorthEast,
        ResizeEdge::SouthWest => ResizeDirection::SouthWest,
        ResizeEdge::SouthEast => ResizeDirection::SouthEast,
    }
}

/// How much wider and taller a window is than its frame, in logical pixels: the band on both sides that holds its shadow, when the window draws its own frame and its shadow is not shown outside it.
fn band() -> f32 {
    if FRAMELESS && !pm_components::shadow_outside() {
        2.0 * SHADOW_MARGIN
    } else {
        0.0
    }
}

/// How large a window is whose frame is `size` logical pixels across.
pub(super) fn with_band(size: (f32, f32)) -> (f32, f32) {
    (size.0 + band(), size.1 + band())
}

/// How large the frame is of a window `size` logical pixels across.
pub(super) fn without_band(size: (f32, f32)) -> (f32, f32) {
    (size.0 - band(), size.1 - band())
}

impl App {
    /// Tells the windows built next whether they float, with a band around them for their shadow, composing them again when that changed: they float unless the window is maximized or full screen.
    pub(super) fn sync_floating(&mut self) {
        let floating = self.window.as_ref().is_some_and(|window| window.floats());
        if floating != pm_components::framed() && FRAMELESS {
            self.recompose = true;
        }
        pm_components::set_floating(floating);
    }

    /// Asks the desktop to resize the window by its edge or corner `edge`, pressed on the
    /// strips [`resize_edges`](pm_components::resize_edges) lays over the frame.
    pub(super) fn edge_pressed(&mut self, edge: ResizeEdge) {
        if let Some(window) = self.window.clone() {
            window.drag_resize(direction_of(edge));
        }
    }

    /// Moves the window when the primary button goes down on its bare title bar, and
    /// maximizes or restores it when that is the second press there in a row. Says whether
    /// the press was taken: there is a window.
    pub(super) fn title_bar_pressed(&mut self) -> bool {
        let Some(window) = self.window.clone() else {
            return false;
        };
        if self.press_clicks == 2 {
            window.set_maximized(!window.is_maximized());
        } else {
            window.drag_window();
        }
        true
    }

    /// Makes the window an icon on the desktop's bar, as its button for that does.
    pub(super) fn minimize_window(&mut self) {
        if let Some(window) = self.window.clone() {
            window.set_minimized(true);
        }
    }

    /// Fills the screen with the window, or gives it back its size, as its button for that does.
    pub(super) fn toggle_maximized(&mut self) {
        if let Some(window) = self.window.clone() {
            window.set_maximized(!window.is_maximized());
        }
    }

    /// Carries out the window's close button: a window beside the main one closes what it
    /// shows, and the main window asks to close.
    pub(super) fn close_button_pressed(&mut self) {
        if self.window.is_none() {
            return;
        }
        match self.on_aside {
            Some(aside) => self
                .model
                .update(|state| *state = aside.closed(std::mem::take(state))),
            None => self.closing = self.close_window(),
        }
    }

    /// Asks the window to close, as its close button or the desktop does, and says whether it is to close now: once nothing is left to ask first.
    pub(super) fn close_window(&mut self) -> bool {
        self.model
            .update(|screen| *screen = std::mem::take(screen).close_asked());
        self.model.read(Screen::quitting)
    }
}
