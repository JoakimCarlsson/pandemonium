//! What a page paints.

use std::sync::Arc;

/// The pixels a page painted, at its device resolution.
#[derive(Clone, Debug)]
pub struct Frame {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// How many paints the page has made when this one was taken; a larger number is a newer picture.
    pub generation: u64,
    /// Tightly packed RGBA bytes, row by row from the top.
    pub rgba: Arc<Vec<u8>>,
}
