//! One cell of the grid: a character and how it is drawn.

/// A colour a cell asks for.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Hash)]
pub enum Color {
    /// The terminal's own foreground or background.
    #[default]
    Default,
    /// An entry of the 256-colour palette.
    Indexed(u8),
    /// A true colour.
    Rgb(u8, u8, u8),
}

/// How a cell's character is drawn, as a set of bits.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Hash)]
pub struct Flags(u8);

impl Flags {
    /// Heavier type.
    pub const BOLD: Self = Self(1);
    /// Fainter colour.
    pub const DIM: Self = Self(2);
    /// Leaning type.
    pub const ITALIC: Self = Self(4);
    /// A line under the character.
    pub const UNDERLINE: Self = Self(8);
    /// Foreground and background swapped.
    pub const INVERSE: Self = Self(16);
    /// A line through the character.
    pub const STRIKE: Self = Self(32);
    /// The character is not drawn.
    pub const HIDDEN: Self = Self(64);

    /// Whether every bit of `other` is set.
    pub fn has(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    /// These flags with the bits of `other` set.
    pub fn with(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    /// These flags with the bits of `other` cleared.
    pub fn without(self, other: Self) -> Self {
        Self(self.0 & !other.0)
    }
}

/// One character of the grid, with its colours and flags.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct Cell {
    /// The character; a space for an empty cell.
    pub ch: char,
    /// The foreground.
    pub fg: Color,
    /// The background.
    pub bg: Color,
    /// How it is drawn.
    pub flags: Flags,
    /// The hyperlink a program put on it, counted from one; nought for none.
    pub link: u16,
}

impl Default for Cell {
    /// An empty cell in the default colours.
    fn default() -> Self {
        Self {
            ch: ' ',
            fg: Color::Default,
            bg: Color::Default,
            flags: Flags::default(),
            link: 0,
        }
    }
}

impl Cell {
    /// Whether this cell looks the same as an empty one.
    pub fn is_blank(&self) -> bool {
        self.ch == ' ' && self.bg == Color::Default && !self.flags.has(Flags::INVERSE)
    }
}
