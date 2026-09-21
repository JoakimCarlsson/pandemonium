//! The colours a terminal grid is drawn in.
//!
//! A terminal names its colours by number, not by meaning: what a program
//! calls "red" is whatever the terminal's palette has at index one. The
//! palette is therefore a token like any other, and a theme that wants its
//! own only has to say so.

use pm_gfx::Rgba;

/// The sixteen colours a program can name, and the cursor drawn over them.
#[derive(Clone, Copy, Debug)]
pub struct Terminal {
    /// The eight ordinary colours, then the eight bright ones.
    pub ansi: [Rgba; 16],
    /// The block drawn where the next character will go.
    pub cursor: Rgba,
    /// A selected run of cells.
    pub selection: Rgba,
}

impl Terminal {
    /// The palette dark themes use unless they name their own.
    pub const DARK: Self = Self {
        ansi: [
            Rgba::hex(0x2a2d31),
            Rgba::hex(0xe06c62),
            Rgba::hex(0x7fb97f),
            Rgba::hex(0xd9a343),
            Rgba::hex(0x74a0c8),
            Rgba::hex(0xb98dc4),
            Rgba::hex(0x63b8b8),
            Rgba::hex(0xc8cacd),
            Rgba::hex(0x5a5e63),
            Rgba::hex(0xf08a80),
            Rgba::hex(0x9ad39a),
            Rgba::hex(0xecc06a),
            Rgba::hex(0x93bbe0),
            Rgba::hex(0xcfa8d8),
            Rgba::hex(0x86d2d2),
            Rgba::hex(0xf2f3f5),
        ],
        cursor: Rgba::hex(0xc8cacd),
        selection: Rgba::hexa(0x74a0c840),
    };

    /// The palette light themes use unless they name their own.
    pub const LIGHT: Self = Self {
        ansi: [
            Rgba::hex(0x30333a),
            Rgba::hex(0xb3332b),
            Rgba::hex(0x1f7a45),
            Rgba::hex(0x9a6b0f),
            Rgba::hex(0x2f5d8c),
            Rgba::hex(0x81419b),
            Rgba::hex(0x15707a),
            Rgba::hex(0x6a6a6a),
            Rgba::hex(0x8a8a8a),
            Rgba::hex(0xcc4136),
            Rgba::hex(0x2a9459),
            Rgba::hex(0xb5851b),
            Rgba::hex(0x3d76ad),
            Rgba::hex(0x9c56b8),
            Rgba::hex(0x1d8b96),
            Rgba::hex(0x1b1c1e),
        ],
        cursor: Rgba::hex(0x3b3b3b),
        selection: Rgba::hexa(0x2f5d8c33),
    };
}
