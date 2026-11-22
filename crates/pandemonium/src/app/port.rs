//! What the app asks of the window around it, behind a trait so the same
//! logic runs against a window rustle opened, and against a stand-in that
//! records what was asked when no window is open.

use rustle::{ResizeEdge, WindowHandle, WindowPlacement};

/// A window of the app, as its logic sees it: how large it is, whether it
/// floats, and what the app asks of it.
pub(crate) trait Port {
    /// How wide and tall the window's surface is, in logical pixels.
    fn logical_size(&self) -> (f32, f32);

    /// Whether the window is maximized.
    fn is_maximized(&self) -> bool;

    /// Whether the window floats, with room around it for a shadow: it does
    /// unless it is maximized or full screen.
    fn floats(&self) -> bool;

    /// Whether the window has the keyboard.
    fn has_focus(&self) -> bool;

    /// Maximizes the window, or restores it when `maximized` is false.
    fn set_maximized(&self, maximized: bool);

    /// Minimizes the window, or brings it back when `minimized` is false.
    fn set_minimized(&self, minimized: bool);

    /// Brings the window to the front and gives it the keyboard.
    fn focus_window(&self);

    /// Asks the desktop to draw the reader's attention to the window.
    fn request_attention(&self);

    /// Lets the pointer move the window, from a press on its title bar.
    fn drag_window(&self);

    /// Lets the pointer resize the window by its edge or corner `edge`.
    fn drag_resize(&self, edge: ResizeEdge);

    /// Makes the window fill the screen, or gives it back its place when `fullscreen` is false; a window that cannot does nothing.
    fn set_fullscreen(&self, fullscreen: bool) {
        let _ = fullscreen;
    }
}

/// A window rustle opened, controlled through its handle.
pub(crate) struct NativePort {
    /// The handle rustle controls the window by.
    handle: WindowHandle,
}

impl NativePort {
    /// The port of the window rustle controls by `handle`.
    pub(crate) fn new(handle: WindowHandle) -> Self {
        Self { handle }
    }

    /// Whether the window is open, so the platform can be asked about it.
    pub(crate) fn is_open(&self) -> bool {
        self.handle.size().is_some()
    }
}

impl Port for NativePort {
    /// The window's content in logical pixels, or nothing while it is not open.
    fn logical_size(&self) -> (f32, f32) {
        self.handle
            .size()
            .map_or((0.0, 0.0), |size| (size.width, size.height))
    }

    /// Whether the window is maximized.
    fn is_maximized(&self) -> bool {
        self.handle.placement() == WindowPlacement::Maximized
    }

    /// Whether the window is neither maximized nor full screen.
    fn floats(&self) -> bool {
        self.handle.placement() == WindowPlacement::Floating
    }

    /// Whether the window has the keyboard.
    fn has_focus(&self) -> bool {
        self.handle.is_focused()
    }

    /// Maximizes or restores the window.
    fn set_maximized(&self, maximized: bool) {
        self.handle.set_maximized(maximized);
    }

    /// Minimizes or brings back the window.
    fn set_minimized(&self, minimized: bool) {
        self.handle.set_minimized(minimized);
    }

    /// Makes the window fill the screen, or gives it back its place.
    fn set_fullscreen(&self, fullscreen: bool) {
        self.handle.set_fullscreen(fullscreen);
    }

    /// Brings the window in front of the others.
    fn focus_window(&self) {
        self.handle.to_front();
    }

    /// Asks for the reader's attention, without insisting.
    fn request_attention(&self) {
        self.handle.request_user_attention(false);
    }

    /// Hands the window to the desktop to move.
    fn drag_window(&self) {
        self.handle.drag_window();
    }

    /// Hands the window to the desktop to resize.
    fn drag_resize(&self, edge: ResizeEdge) {
        self.handle.drag_resize_window(edge);
    }
}
