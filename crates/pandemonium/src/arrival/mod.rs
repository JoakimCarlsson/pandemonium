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
use winit::event_loop::OwnedDisplayHandle;

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

/// Owns platform drag handling until the application stops it.
pub struct Listening {
    /// The guest backend's thread and borrowed display owner.
    #[cfg(target_os = "linux")]
    _wayland: wayland::Listening,
}

/// Follows files carried onto the display where winit does not, putting what
/// happens on `arrivals` and calling `wake` after each.
///
/// Returns a guard that stops and joins the listener before releasing its display.
pub fn listen(
    display: OwnedDisplayHandle,
    arrivals: Arrivals,
    wake: Arc<dyn Fn() + Send + Sync>,
) -> Option<Listening> {
    #[cfg(target_os = "linux")]
    {
        wayland::listen(display, move |arrival| {
            if let Ok(mut arrivals) = arrivals.lock() {
                arrivals.push(arrival);
            }
            wake();
        })
        .ok()
        .map(|listener| Listening { _wayland: listener })
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = (display, arrivals, wake);
        None
    }
}
