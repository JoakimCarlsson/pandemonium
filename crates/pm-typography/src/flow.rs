//! How text lines up, breaks and is cased.

/// Horizontal alignment of lines inside their box.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum TextAlign {
    /// The start edge of the writing direction.
    #[default]
    Start,
    /// The end edge of the writing direction.
    End,
    /// The left edge.
    Left,
    /// The middle.
    Center,
    /// The right edge.
    Right,
    /// Both edges, with word spacing stretched.
    Justify,
}

/// A case mapping applied while shaping.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum TextTransform {
    /// The text as written.
    #[default]
    None,
    /// Every letter upper case.
    Uppercase,
    /// Every letter lower case.
    Lowercase,
    /// The first letter of each word upper case.
    Capitalize,
}

/// How runs of white space and line breaks in the text are treated.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum WhiteSpace {
    /// Collapsed, and wrapped at the box edge.
    #[default]
    Normal,
    /// Collapsed, never wrapped.
    Nowrap,
    /// Kept, never wrapped.
    Pre,
    /// Collapsed, but line breaks are kept.
    PreLine,
    /// Kept, and wrapped at the box edge.
    PreWrap,
    /// Kept, wrapped, and every space is a break opportunity.
    BreakSpaces,
}

/// What stands in for text that does not fit on its line.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum TextOverflow {
    /// The text is cut at the box edge.
    #[default]
    Clip,
    /// The text is cut and an ellipsis marks the cut.
    Ellipsis,
}

/// How the line breaker balances lines.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum TextWrap {
    /// Breaks greedily.
    #[default]
    Wrap,
    /// Never breaks.
    NoWrap,
    /// Evens out the line lengths.
    Balance,
    /// Avoids a short last line.
    Pretty,
}
