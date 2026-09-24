//! The colours a terminal grid is drawn in.
//!
//! A terminal names its colours by number, not by meaning: what a program
//! calls "red" is whatever the terminal's palette has at index one. The
//! palette is therefore a token like any other.

use pm_gfx::Rgba;

/// The sixteen colours a program can name, and the cursor drawn over them.
#[derive(Clone, Copy, Debug, Default)]
pub struct Terminal {
    /// The eight ordinary colours, then the eight bright ones.
    pub ansi: [Rgba; 16],
    /// The block drawn where the next character will go.
    pub cursor: Rgba,
    /// A selected run of cells.
    pub selection: Rgba,
}
