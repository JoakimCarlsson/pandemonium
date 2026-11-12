//! Files dragged onto the window from other programs: over the project's files they are brought into the folder they are let go on, and anywhere else they are attached to the prompt.
//!
//! winit reports the files let go but not where, and on Wayland nothing at all. Where winit reports them, the place is taken from the pointer as soon as it is heard of again; on Wayland a data device of our own follows the drag.

#[cfg(target_os = "linux")]
mod offered;

use std::path::PathBuf;
use std::time::{Duration, Instant};

use super::App;

/// How long files let go wait for the pointer to say where, before the place it was last seen at is taken.
const SETTLE: Duration = Duration::from_millis(150);

/// What is known of files dragged in from outside the window.
#[derive(Default)]
pub(super) struct Outside {
    /// The files winit said were let go, until it is known where.
    dropped: Vec<String>,
    /// When to stop waiting for the pointer to say where they were let go.
    due: Option<Instant>,
    /// Whether listening on Wayland has been tried.
    tried: bool,
    /// What follows drags over the window on Wayland, when the window is on it.
    #[cfg(target_os = "linux")]
    offered: Option<offered::Offered>,
}

impl App {
    /// Keeps the file at `path`, which winit said was let go over the window, until the pointer says where.
    pub(super) fn file_dropped(&mut self, path: PathBuf) {
        self.outside
            .dropped
            .push(path.to_string_lossy().into_owned());
        self.outside.due = Some(Instant::now() + SETTLE);
    }

    /// Puts the files winit said were let go where the pointer is, now that it is known.
    pub(super) fn settle_drop(&mut self) {
        self.outside.due = None;
        let paths = std::mem::take(&mut self.outside.dropped);
        if !paths.is_empty() {
            self.files_dropped(paths, self.pointer.position);
        }
    }

    /// When the files let go stop waiting for the pointer, if any wait; they are put where it was last seen once that has passed.
    pub(super) fn drop_deadline(&mut self) -> Option<Instant> {
        let due = self.outside.due?;
        if Instant::now() < due {
            return Some(due);
        }
        self.settle_drop();
        if let Some(window) = self.window.as_ref() {
            window.request_redraw();
        }
        None
    }

    /// Hears what happened to files dragged over the window on Wayland, listening from the first time the window is there.
    pub(super) fn pump_drops(&mut self) {
        #[cfg(target_os = "linux")]
        {
            use winit::raw_window_handle::{
                HasDisplayHandle, HasWindowHandle, RawDisplayHandle, RawWindowHandle,
            };

            if !self.outside.tried
                && let Some(window) = self.window.as_ref()
            {
                self.outside.tried = true;
                let display = window.display_handle().map(|handle| handle.as_raw());
                let surface = window.window_handle().map(|handle| handle.as_raw());
                if let (
                    Ok(RawDisplayHandle::Wayland(display)),
                    Ok(RawWindowHandle::Wayland(surface)),
                ) = (display, surface)
                {
                    self.outside.offered = offered::Offered::open(display.display, surface.surface);
                }
            }
            let offers = self
                .outside
                .offered
                .as_mut()
                .map(offered::Offered::take)
                .unwrap_or_default();
            if offers.is_empty() {
                return;
            }
            let band = pm_components::shadow_margin();
            for offer in offers {
                match offer {
                    offered::Offer::Over(x, y) => self.files_hovered(Some((x - band, y - band))),
                    offered::Offer::Left => self.files_hovered(None),
                    offered::Offer::Dropped(paths, (x, y)) => {
                        self.files_dropped(paths, Some((x - band, y - band)));
                    }
                }
            }
            if let Some(window) = self.window.as_ref() {
                window.request_redraw();
            }
        }
    }
}
