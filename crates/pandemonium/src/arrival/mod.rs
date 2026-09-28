//! Files dragged onto the window from outside it: a file manager, a desktop,
//! another program's list of files.
//!
//! winit says where such files are carried and let go on X11, macOS and
//! Windows, but not on Wayland, where the window follows the compositor's
//! data device itself. Either way what reaches the window is an [`Arrival`].

#[cfg(target_os = "linux")]
mod wayland;

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use pm_gfx::Point;
use winit::window::Window;

/// What files carried in from outside the window are doing.
#[derive(Clone, Debug)]
pub enum Arrival {
    /// Files are being carried over the window, at the point when it is known.
    Hovering(Option<Point>),
    /// The files carried over the window have left it, or the drag was
    /// called off.
    Left,
    /// Files were let go over the window.
    Dropped {
        /// Where they were let go, when it is known.
        at: Option<Point>,
        /// The files and directories let go.
        paths: Vec<PathBuf>,
    },
}

/// Arrivals that have happened away from the window and have not been taken
/// in yet.
pub type Arrivals = Arc<Mutex<Vec<Arrival>>>;

/// Follows files carried onto `window` where winit does not, putting what
/// happens on `arrivals` and calling `wake` after each.
pub fn listen(window: &Window, arrivals: Arrivals, wake: Arc<dyn Fn() + Send + Sync>) {
    #[cfg(target_os = "linux")]
    {
        use winit::raw_window_handle::{HasDisplayHandle, RawDisplayHandle};

        if let Ok(handle) = window.display_handle()
            && let RawDisplayHandle::Wayland(display) = handle.as_raw()
        {
            wayland::listen(display.display, move |arrival| {
                if let Ok(mut arrivals) = arrivals.lock() {
                    arrivals.push(arrival);
                }
                wake();
            });
        }
    }
    #[cfg(not(target_os = "linux"))]
    let _ = (window, arrivals, wake);
}
