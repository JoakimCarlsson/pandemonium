//! The frame of the main window, drawn by the window itself: moving it by its title bar, maximizing it with a second press there, resizing it by its edges, and the buttons that minimize, maximize and close it.

use std::time::Instant;

use pm_components::SHADOW_MARGIN;
use pm_components::title_bar::{
    FRAMELESS, TITLE_BAR, WINDOW_CLOSE, WINDOW_MAXIMIZE, WINDOW_MINIMIZE, is_window_control,
};
use pm_input::FocusId;
use pm_pages::Screen;
use winit::dpi::LogicalSize;
use winit::window::{CursorIcon, ResizeDirection, Window};

use super::shadow::Shadow;
use super::{App, DOUBLE_CLICK};

/// How far in from the frame of the window a press resizes it, in logical pixels; so does a press anywhere in the band around the frame that holds its shadow.
const EDGE: f32 = 4.0;

/// The edge or corner of a window whose frame is `size` logical pixels wide and tall that a press at `at` grabs, measured from the corner of the frame, when it is near enough to one or outside the frame.
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

/// How much wider and taller a window is than its frame, in logical pixels: the band on both sides that holds its shadow, when the window draws its own frame and its shadow is not shown outside it.
fn band() -> f32 {
    if FRAMELESS && !pm_components::shadow_outside() {
        2.0 * SHADOW_MARGIN
    } else {
        0.0
    }
}

/// Whether `window` floats, with room around it for a shadow: it does unless it is maximized or full screen.
pub(super) fn floats(window: &Window) -> bool {
    !window.is_maximized() && window.fullscreen().is_none()
}

/// How large a window is whose frame is `size` logical pixels across.
pub(super) fn with_band(size: LogicalSize<f32>) -> LogicalSize<f32> {
    LogicalSize::new(size.width + band(), size.height + band())
}

/// How large the frame is of a window `size` logical pixels across.
pub(super) fn without_band(size: LogicalSize<f32>) -> LogicalSize<f32> {
    LogicalSize::new(size.width - band(), size.height - band())
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
    /// Tells the windows built next whether they float, with a band around them for their shadow, has the renderer leave that band, and fits the shadow cast outside the window to it: they float unless the window is maximized or full screen.
    pub(super) fn sync_floating(&mut self) {
        let floating = self.window.as_deref().is_some_and(floats);
        pm_components::set_floating(floating);
        if let Some(renderer) = self.renderer.as_mut() {
            renderer.set_band(pm_components::shadow_margin());
        }
        if let Some(window) = self.window.as_deref() {
            self.shadow
                .get_or_insert_with(|| Shadow::open(window))
                .fit(window, FRAMELESS && floating);
        }
    }

    /// The edge or corner of the window the pointer is on, when the window draws its own frame and is not maximized.
    pub(super) fn frame_edge(&self) -> Option<ResizeDirection> {
        let window = self.window.as_ref()?;
        if !FRAMELESS || !floats(window) {
            return None;
        }
        edge_at(self.pointer.position?, self.window_size())
    }

    /// Resizes the window when the left button goes down on its edge, moves it when it goes down on the title bar and maximizes or restores it when that is the second press there in a row. Says whether the press was taken.
    pub(super) fn frame_press(&mut self, window: &Window) -> bool {
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

    /// Carries out the control of the window `id` names, and says whether it was one. Closing a window beside the main one closes what it shows.
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
            WINDOW_CLOSE if let Some(aside) = self.on_aside => self
                .model
                .update(|state| *state = aside.closed(std::mem::take(state))),
            WINDOW_CLOSE => self.closing = self.close_window(),
            _ => {}
        }
        true
    }

    /// Asks the window to close, as its close button or the desktop does, and says whether it is to close now: once nothing is left to ask first.
    pub(super) fn close_window(&mut self) -> bool {
        self.model
            .update(|screen| *screen = std::mem::take(screen).close_asked());
        self.model.read(Screen::quitting)
    }
}
