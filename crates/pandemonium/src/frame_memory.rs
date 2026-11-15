//! How big the main window was left and whether it was maximised, kept
//! between runs so it opens the way it was left.

use serde::{Deserialize, Serialize};

/// The file the main window's frame is kept in.
const FRAME: &str = "window-frame.json";

/// The size the main window first opens at, in logical pixels.
pub const FIRST_SIZE: (f32, f32) = (1280.0, 800.0);

/// The smallest the main window can be made, in logical pixels.
pub const MIN_SIZE: (f32, f32) = (720.0, 480.0);

/// The largest size a kept frame is opened at, in logical pixels, so a size
/// kept from a bigger monitor still fits.
const MOST_SIZE: (f32, f32) = (3840.0, 2160.0);

/// How the main window was left.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Frame {
    /// Its size while it floated, in logical pixels.
    pub size: (f32, f32),
    /// Whether it was maximised.
    pub maximised: bool,
}

impl Default for Frame {
    /// The frame the window first opens with.
    fn default() -> Self {
        Self {
            size: FIRST_SIZE,
            maximised: false,
        }
    }
}

impl Frame {
    /// The frame as it was last kept, its size held between the smallest and the largest the window opens at, or the first one.
    pub fn load() -> Self {
        let kept: Self = pm_store::load_text(FRAME)
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default();
        Self {
            size: (
                kept.size.0.clamp(MIN_SIZE.0, MOST_SIZE.0),
                kept.size.1.clamp(MIN_SIZE.1, MOST_SIZE.1),
            ),
            ..kept
        }
    }

    /// Has this frame written to its file, off the UI thread, when it changed.
    pub fn save(self) {
        if let Ok(text) = serde_json::to_string(&self) {
            crate::store_writer::save(FRAME, text);
        }
    }
}
