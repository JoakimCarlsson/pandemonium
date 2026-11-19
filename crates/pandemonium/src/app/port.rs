//! What the app asks of the window around it, behind a trait so the same
//! logic runs against a native window, and against a stand-in that records
//! what was asked when no window is open.

use std::sync::Arc;

use rustle::{ResizeEdge, WindowHandle};
use winit::dpi::{LogicalPosition, LogicalSize};
use winit::window::{ImeRequest, ImeRequestData, UserAttentionType, Window};

/// A window of the app, as its logic sees it: how large it is, whether it
/// floats, and what the app asks of it.
pub(crate) trait Port {
    /// The window's native window, when it has one.
    fn native(&self) -> Option<Arc<dyn Window>>;

    /// How many physical pixels a logical pixel is.
    fn scale(&self) -> f32;

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

    /// Tells the input method that the caret is at `area`, as x, y, width and
    /// height in logical pixels of the window's surface.
    fn set_ime_area(&self, area: (f32, f32, f32, f32));
}

/// A native window opened by rustle, with the handle rustle controls it by.
pub(crate) struct NativePort {
    /// The window.
    window: Arc<dyn Window>,
    /// The handle rustle moves and resizes it by.
    handle: WindowHandle,
}

impl NativePort {
    /// The port of `window`, which rustle controls by `handle`.
    pub(crate) fn new(window: Arc<dyn Window>, handle: WindowHandle) -> Self {
        Self { window, handle }
    }
}

impl Port for NativePort {
    /// The window itself.
    fn native(&self) -> Option<Arc<dyn Window>> {
        Some(Arc::clone(&self.window))
    }

    /// The window's scale factor.
    fn scale(&self) -> f32 {
        self.window.scale_factor() as f32
    }

    /// The window's surface in logical pixels.
    fn logical_size(&self) -> (f32, f32) {
        let size = self
            .window
            .surface_size()
            .to_logical::<f32>(self.window.scale_factor());
        (size.width, size.height)
    }

    /// Whether the window is maximized.
    fn is_maximized(&self) -> bool {
        self.window.is_maximized()
    }

    /// Whether the window is neither maximized nor full screen.
    fn floats(&self) -> bool {
        !self.window.is_maximized() && self.window.fullscreen().is_none()
    }

    /// Whether the window has the keyboard.
    fn has_focus(&self) -> bool {
        self.window.has_focus()
    }

    /// Maximizes or restores the window.
    fn set_maximized(&self, maximized: bool) {
        self.window.set_maximized(maximized);
    }

    /// Minimizes or brings back the window.
    fn set_minimized(&self, minimized: bool) {
        self.window.set_minimized(minimized);
    }

    /// Brings the window forward.
    fn focus_window(&self) {
        self.window.focus_window();
    }

    /// Asks for the reader's attention, informationally.
    fn request_attention(&self) {
        self.window
            .request_user_attention(Some(UserAttentionType::Informational));
    }

    /// Hands the window to the desktop to move.
    fn drag_window(&self) {
        self.handle.drag_window();
    }

    /// Hands the window to the desktop to resize.
    fn drag_resize(&self, edge: ResizeEdge) {
        self.handle.drag_resize_window(edge);
    }

    /// Updates the input method's caret area.
    fn set_ime_area(&self, area: (f32, f32, f32, f32)) {
        let data = ImeRequestData::default().with_cursor_area(
            LogicalPosition::new(area.0, area.1).into(),
            LogicalSize::new(area.2, area.3).into(),
        );
        let _ = self.window.request_ime_update(ImeRequest::Update(data));
    }
}
