//! How big the main window was left and whether it was maximised, kept
//! between runs so it opens the way it was left.

/// The file the main window's frame is kept in.
const FRAME: &str = "window-frame.txt";

/// The size the main window first opens at, in logical pixels.
pub const FIRST_SIZE: (f32, f32) = (1280.0, 800.0);

/// The smallest the main window can be made, in logical pixels.
pub const MIN_SIZE: (f32, f32) = (720.0, 480.0);

/// The largest size a kept frame is opened at, in logical pixels, so a size
/// kept from a bigger monitor still fits.
const MOST_SIZE: (f32, f32) = (3840.0, 2160.0);

/// How the main window was left.
#[derive(Clone, Copy, Debug, PartialEq)]
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
        let kept = pm_store::load_text(FRAME)
            .and_then(|text| Self::read(&text))
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
        crate::store_writer::save(FRAME, self.write());
    }

    /// The frame written as `text`: its width, its height and `maximised` or `floating`, apart by spaces; nothing when it cannot be read.
    fn read(text: &str) -> Option<Self> {
        let mut words = text.split_whitespace();
        let width = words.next()?.parse().ok()?;
        let height = words.next()?.parse().ok()?;
        let maximised = words.next() == Some("maximised");
        Some(Self {
            size: (width, height),
            maximised,
        })
    }

    /// The frame written down, as [`Frame::read`] reads it.
    fn write(self) -> String {
        let shape = if self.maximised {
            "maximised"
        } else {
            "floating"
        };
        format!("{} {} {shape}\n", self.size.0, self.size.1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A frame written down reads back the same.
    #[test]
    fn frames_read_back() {
        let frame = Frame {
            size: (1024.5, 700.0),
            maximised: true,
        };
        assert_eq!(Frame::read(&frame.write()), Some(frame));
        assert_eq!(Frame::read("nonsense"), None);
    }
}
