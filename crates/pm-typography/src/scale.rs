//! The type scale, font weights, letter spacing and line spacing.

/// A step of the type scale.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum TextSize {
    /// 12 logical pixels with a line 1.333 times as tall.
    Xs,
    /// 14 logical pixels with a line 1.429 times as tall.
    Sm,
    /// 16 logical pixels with a line 1.500 times as tall.
    Base,
    /// 18 logical pixels with a line 1.556 times as tall.
    Lg,
    /// 20 logical pixels with a line 1.400 times as tall.
    Xl,
    /// 24 logical pixels with a line 1.333 times as tall.
    Xl2,
    /// 30 logical pixels with a line 1.200 times as tall.
    Xl3,
    /// 36 logical pixels with a line 1.111 times as tall.
    Xl4,
    /// 48 logical pixels with a line 1.000 times as tall.
    Xl5,
    /// 60 logical pixels with a line 1.000 times as tall.
    Xl6,
    /// 72 logical pixels with a line 1.000 times as tall.
    Xl7,
    /// 96 logical pixels with a line 1.000 times as tall.
    Xl8,
    /// 128 logical pixels with a line 1.000 times as tall.
    Xl9,
}

impl TextSize {
    /// The font size in logical pixels.
    pub const fn pixels(self) -> f32 {
        match self {
            Self::Xs => 12.0,
            Self::Sm => 14.0,
            Self::Base => 16.0,
            Self::Lg => 18.0,
            Self::Xl => 20.0,
            Self::Xl2 => 24.0,
            Self::Xl3 => 30.0,
            Self::Xl4 => 36.0,
            Self::Xl5 => 48.0,
            Self::Xl6 => 60.0,
            Self::Xl7 => 72.0,
            Self::Xl8 => 96.0,
            Self::Xl9 => 128.0,
        }
    }
}

impl TextSize {
    /// The line height as a multiple of the font size.
    pub const fn line_height_ratio(self) -> f32 {
        match self {
            Self::Xs => 1.333333,
            Self::Sm => 1.428571,
            Self::Base => 1.5,
            Self::Lg => 1.555556,
            Self::Xl => 1.4,
            Self::Xl2 => 1.333333,
            Self::Xl3 => 1.2,
            Self::Xl4 => 1.111111,
            Self::Xl5 => 1.0,
            Self::Xl6 => 1.0,
            Self::Xl7 => 1.0,
            Self::Xl8 => 1.0,
            Self::Xl9 => 1.0,
        }
    }
}

impl TextSize {
    /// Every step, smallest first.
    pub const ALL: [Self; 13] = [
        Self::Xs,
        Self::Sm,
        Self::Base,
        Self::Lg,
        Self::Xl,
        Self::Xl2,
        Self::Xl3,
        Self::Xl4,
        Self::Xl5,
        Self::Xl6,
        Self::Xl7,
        Self::Xl8,
        Self::Xl9,
    ];
}

/// A named font weight.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum FontWeight {
    /// Weight 100.
    Thin,
    /// Weight 200.
    Extralight,
    /// Weight 300.
    Light,
    /// Weight 400.
    Normal,
    /// Weight 500.
    Medium,
    /// Weight 600.
    Semibold,
    /// Weight 700.
    Bold,
    /// Weight 800.
    Extrabold,
    /// Weight 900.
    Black,
}

impl FontWeight {
    /// The numeric weight, 100 to 900.
    pub const fn value(self) -> u16 {
        match self {
            Self::Thin => 100,
            Self::Extralight => 200,
            Self::Light => 300,
            Self::Normal => 400,
            Self::Medium => 500,
            Self::Semibold => 600,
            Self::Bold => 700,
            Self::Extrabold => 800,
            Self::Black => 900,
        }
    }
}

impl FontWeight {
    /// Every weight, lightest first.
    pub const ALL: [Self; 9] = [
        Self::Thin,
        Self::Extralight,
        Self::Light,
        Self::Normal,
        Self::Medium,
        Self::Semibold,
        Self::Bold,
        Self::Extrabold,
        Self::Black,
    ];
}

/// A named letter spacing.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Tracking {
    /// -0.05 em between letters.
    Tighter,
    /// -0.025 em between letters.
    Tight,
    /// 0 em between letters.
    Normal,
    /// 0.025 em between letters.
    Wide,
    /// 0.05 em between letters.
    Wider,
    /// 0.1 em between letters.
    Widest,
}

impl Tracking {
    /// The letter spacing in multiples of the font size.
    pub const fn em(self) -> f32 {
        match self {
            Self::Tighter => -0.05,
            Self::Tight => -0.025,
            Self::Normal => 0.0,
            Self::Wide => 0.025,
            Self::Wider => 0.05,
            Self::Widest => 0.1,
        }
    }
}

/// A named line height multiplier.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Leading {
    /// 1.25 times the font size.
    Tight,
    /// 1.375 times the font size.
    Snug,
    /// 1.5 times the font size.
    Normal,
    /// 1.625 times the font size.
    Relaxed,
    /// 2 times the font size.
    Loose,
}

impl Leading {
    /// The line height as a multiple of the font size.
    pub const fn ratio(self) -> f32 {
        match self {
            Self::Tight => 1.25,
            Self::Snug => 1.375,
            Self::Normal => 1.5,
            Self::Relaxed => 1.625,
            Self::Loose => 2.0,
        }
    }
}
