//! Lines drawn along text.

use pm_color::Color;

/// Where a line is drawn relative to the text.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TextLine {
    /// Below the baseline.
    Underline,
    /// Above the text.
    Overline,
    /// Through the middle of the x-height.
    LineThrough,
}

/// The pattern of a text decoration line.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum LineStyle {
    /// One continuous line.
    #[default]
    Solid,
    /// Two parallel lines.
    Double,
    /// A row of dots.
    Dotted,
    /// A row of dashes.
    Dashed,
    /// A wavy line.
    Wavy,
}

/// The lines drawn on text and how they look.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TextDecoration {
    /// Draw an underline.
    pub underline: bool,
    /// Draw an overline.
    pub overline: bool,
    /// Draw a line through the text.
    pub line_through: bool,
    /// The line pattern.
    pub style: LineStyle,
    /// The line colour; the text colour when unset.
    pub color: Option<Color>,
    /// The line thickness in logical pixels; the font's own when unset.
    pub thickness: Option<f32>,
    /// The gap between text and underline in logical pixels; the font's own when unset.
    pub underline_offset: Option<f32>,
}

impl TextDecoration {
    /// No lines at all.
    pub const NONE: Self = Self {
        underline: false,
        overline: false,
        line_through: false,
        style: LineStyle::Solid,
        color: None,
        thickness: None,
        underline_offset: None,
    };

    /// Turns on the given line.
    pub fn enable(&mut self, line: TextLine) {
        match line {
            TextLine::Underline => self.underline = true,
            TextLine::Overline => self.overline = true,
            TextLine::LineThrough => self.line_through = true,
        }
    }

    /// Turns off every line.
    pub fn clear(&mut self) {
        self.underline = false;
        self.overline = false;
        self.line_through = false;
    }
}
