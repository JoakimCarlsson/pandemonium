//! The frame of the main window, drawn by the window itself: moving it by its title bar, maximizing it with a second press there, resizing it by its edges, and the buttons that minimize, maximize and close it.

use std::time::Instant;

use pm_components::title_bar::{
    FRAMELESS, TITLE_BAR, WINDOW_CLOSE, WINDOW_MAXIMIZE, WINDOW_MINIMIZE, is_window_control,
};
use pm_input::FocusId;
use pm_pages::Screen;
use winit::window::{CursorIcon, ResizeDirection, Window};

use super::{App, DOUBLE_CLICK};

/// How far in from the edge of the window a press resizes it, in logical pixels.
const EDGE: f32 = 6.0;

/// The edge or corner of a window `size` logical pixels wide and tall that a press at `at` grabs, when it is near enough to one.
fn edge_at(at: (f32, f32), size: (f32, f32)) -> Option<ResizeDirection> {
    let (x, y) = at;
    let (width, height) = size;
    let west = x < EDGE;
    let east = x > width - EDGE;
    let north = y < EDGE;
    let south = y > height - EDGE;
    match (north, south, west, east) {
        (true, _, true, _) => Some(ResizeDirection::NorthWest),
        (true, _, _, true) => Some(ResizeDirection::NorthEast),
        (_, true, true, _) => Some(ResizeDirection::SouthWest),
        (_, true, _, true) => Some(ResizeDirection::SouthEast),
        (true, ..) => Some(ResizeDirection::North),
        (_, true, ..) => Some(ResizeDirection::South),
        (_, _, true, _) => Some(ResizeDirection::West),
        (_, _, _, true) => Some(ResizeDirection::East),
        _ => None,
    }
}

/// The pointer shape that says the edge or corner `direction` resizes the window.
pub(super) fn edge_icon(direction: ResizeDirection) -> CursorIcon {
    match direction {
        ResizeDirection::North => CursorIcon::NResize,
        ResizeDirection::South => CursorIcon::SResize,
        ResizeDirection::West => CursorIcon::WResize,
        ResizeDirection::East => CursorIcon::EResize,
        ResizeDirection::NorthWest => CursorIcon::NwResize,
        ResizeDirection::NorthEast => CursorIcon::NeResize,
        ResizeDirection::SouthWest => CursorIcon::SwResize,
        ResizeDirection::SouthEast => CursorIcon::SeResize,
    }
}

impl App {
    /// The edge or corner of the main window the pointer is on, when the window draws its own frame and is not maximized.
    pub(super) fn frame_edge(&self) -> Option<ResizeDirection> {
        let window = self.window.as_ref()?;
        if !FRAMELESS || self.on_settings || window.is_maximized() {
            return None;
        }
        edge_at(self.pointer.position?, self.window_size())
    }

    /// Resizes the main window when the left button goes down on its edge, moves it when it goes down on the title bar and maximizes or restores it when that is the second press there in a row. Says whether the press was taken.
    pub(super) fn frame_press(&mut self, window: &Window) -> bool {
        if self.on_settings {
            return false;
        }
        if let Some(direction) = self.frame_edge() {
            let _ = window.drag_resize_window(direction);
            return true;
        }
        let on_bar = self
            .pointer
            .position
            .and_then(|at| self.output.hit(at))
            .is_some_and(|id| id == TITLE_BAR);
        if !on_bar {
            return false;
        }
        let again = self
            .title_press
            .is_some_and(|at| at.elapsed() < DOUBLE_CLICK);
        if again {
            self.title_press = None;
            window.set_maximized(!window.is_maximized());
        } else {
            self.title_press = Some(Instant::now());
            let _ = window.drag_window();
        }
        true
    }

    /// Carries out the control of the window `id` names, and says whether it was one.
    pub(super) fn window_control(&mut self, id: FocusId) -> bool {
        if !is_window_control(id) {
            return false;
        }
        let Some(window) = self.window.clone() else {
            return true;
        };
        match id {
            WINDOW_MINIMIZE => window.set_minimized(true),
            WINDOW_MAXIMIZE => window.set_maximized(!window.is_maximized()),
            WINDOW_CLOSE => self.closing = self.close_window(),
            _ => {}
        }
        true
    }

    /// Asks the window to close, as its close button or the desktop does, and says whether it is to close now: at once while onboarding, and otherwise once nothing is left to ask first.
    pub(super) fn close_window(&mut self) -> bool {
        self.model
            .update(|screen| *screen = std::mem::take(screen).close_asked());
        let onboarding = self
            .model
            .read(|screen| matches!(screen, Screen::Onboarding(_)));
        onboarding || self.model.read(Screen::quitting)
    }
}
