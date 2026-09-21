//! One cell of the grid: the character in it and how it is drawn.

/// A colour a cell is drawn in.
///
/// The emulator never resolves a colour to pixels: indices 0..16 are the
/// palette the theme names, 16..256 the cube and greyscale ramp every
/// terminal agrees on, and [`Color::Default`] is whatever the surface drawing
/// the grid calls foreground and background.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Color {
    /// The terminal's own foreground or background.
    Default,
    /// One of the 256 palette entries.
    Indexed(u8),
    /// A direct 24-bit colour.
    Rgb(u8, u8, u8),
}

/// How a cell is drawn, apart from the character in it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Attrs {
    /// The colour the character is drawn in.
    pub foreground: Color,
    /// The colour behind it.
    pub background: Color,
    /// Drawn at a heavier weight.
    pub bold: bool,
    /// Drawn at a lighter weight.
    pub dim: bool,
    /// Drawn slanted.
    pub italic: bool,
    /// Drawn with a line under it.
    pub underline: bool,
    /// Drawn with a line through it.
    pub strikethrough: bool,
    /// Foreground and background swapped.
    pub inverse: bool,
    /// Not drawn at all, though it still occupies its cell.
    pub hidden: bool,
}

impl Attrs {
    /// The attributes a cell has before anything has styled it.
    pub const DEFAULT: Self = Self {
        foreground: Color::Default,
        background: Color::Default,
        bold: false,
        dim: false,
        italic: false,
        underline: false,
        strikethrough: false,
        inverse: false,
        hidden: false,
    };

    /// The pair of colours this cell is actually drawn with.
    ///
    /// Inverse is resolved here rather than at the call site, so a surface
    /// asking for the colours never has to know the attribute exists.
    pub fn colors(&self) -> (Color, Color) {
        if self.inverse {
            (self.background, self.foreground)
        } else {
            (self.foreground, self.background)
        }
    }
}

impl Default for Attrs {
    /// The attributes a cell has before anything has styled it.
    fn default() -> Self {
        Self::DEFAULT
    }
}

/// One character on the screen, with the style it was written in.
///
/// A double-width character occupies two cells: the character itself, of
/// [`Cell::width`] two, and a spacer of width zero holding no character. The
/// spacer keeps the columns of the grid and the columns of the screen the
/// same thing.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Cell {
    /// The character written here.
    pub ch: char,
    /// How it is drawn.
    pub attrs: Attrs,
    /// How many columns it occupies: zero for the spacer of a wide character.
    pub width: u8,
}

impl Cell {
    /// An empty cell styled with `attrs`.
    pub fn blank(attrs: Attrs) -> Self {
        Self {
            ch: ' ',
            attrs,
            width: 1,
        }
    }

    /// Whether this cell is the trailing half of a double-width character.
    pub fn is_spacer(&self) -> bool {
        self.width == 0
    }

    /// Whether this cell would put no glyph on the screen.
    pub fn is_blank(&self) -> bool {
        self.ch == ' ' || self.ch == '\0' || self.attrs.hidden
    }
}

impl Default for Cell {
    /// An empty, unstyled cell.
    fn default() -> Self {
        Self::blank(Attrs::DEFAULT)
    }
}
