//! The typography rule: one declaration about text.

use pm_color::Color;
use pm_primitives::Rule;

use crate::{
    FontFamily, FontSlant, FontWeight, Leading, LineHeight, LineStyle, TextAlign, TextLine,
    TextOverflow, TextSize, TextTransform, TextWrap, Tracking, TypographyStyle, WhiteSpace,
};

/// One declaration about typography.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TypographyRule {
    /// The typeface family.
    Family(FontFamily),
    /// A step of the type scale: sets the size and its matching line height.
    Size(TextSize),
    /// An arbitrary font size in logical pixels.
    SizePx(f32),
    /// A named weight.
    Weight(FontWeight),
    /// An arbitrary weight from 1 to 1000.
    WeightValue(u16),
    /// Upright or leaning.
    Slant(FontSlant),
    /// A named letter spacing.
    Tracking(Tracking),
    /// A named line height.
    Leading(Leading),
    /// A line height as a multiple of the font size.
    LeadingRatio(f32),
    /// A line height in logical pixels.
    LeadingPx(f32),
    /// Horizontal alignment.
    Align(TextAlign),
    /// The text colour.
    Color(Color),
    /// Draws a line along the text.
    Line(TextLine),
    /// Removes every line.
    NoLine,
    /// The line pattern.
    LineStyle(LineStyle),
    /// The line colour.
    LineColor(Color),
    /// The line thickness in logical pixels.
    LineThickness(f32),
    /// The gap between text and underline in logical pixels.
    UnderlineOffset(f32),
    /// Case mapping.
    Transform(TextTransform),
    /// White-space handling.
    WhiteSpace(WhiteSpace),
    /// Overflow marker.
    Overflow(TextOverflow),
    /// Line breaking strategy.
    Wrap(TextWrap),
    /// Cut after this many lines, with an ellipsis; `None` removes the limit.
    LineClamp(Option<u16>),
    /// Indent of the first line in logical pixels.
    Indent(f32),
    /// One line cut with an ellipsis.
    Truncate,
}

impl Rule for TypographyRule {
    type Target = TypographyStyle;

    /// Writes the declaration into `target`.
    fn apply(self, target: &mut TypographyStyle) {
        match self {
            Self::Family(family) => target.family = family,
            Self::Size(size) => {
                target.size = size.pixels();
                target.line_height = LineHeight::Ratio(size.line_height_ratio());
            }
            Self::SizePx(pixels) => target.size = pixels,
            Self::Weight(weight) => target.weight = weight.value(),
            Self::WeightValue(weight) => target.weight = weight,
            Self::Slant(slant) => target.slant = slant,
            Self::Tracking(tracking) => target.tracking = tracking.em(),
            Self::Leading(leading) => target.line_height = LineHeight::Ratio(leading.ratio()),
            Self::LeadingRatio(ratio) => target.line_height = LineHeight::Ratio(ratio),
            Self::LeadingPx(pixels) => target.line_height = LineHeight::Px(pixels),
            Self::Align(align) => target.align = align,
            Self::Color(color) => target.color = Some(color),
            Self::Line(line) => target.decoration.enable(line),
            Self::NoLine => target.decoration.clear(),
            Self::LineStyle(style) => target.decoration.style = style,
            Self::LineColor(color) => target.decoration.color = Some(color),
            Self::LineThickness(pixels) => target.decoration.thickness = Some(pixels),
            Self::UnderlineOffset(pixels) => target.decoration.underline_offset = Some(pixels),
            Self::Transform(transform) => target.transform = transform,
            Self::WhiteSpace(white_space) => target.white_space = white_space,
            Self::Overflow(overflow) => target.overflow = overflow,
            Self::Wrap(wrap) => target.wrap = wrap,
            Self::LineClamp(lines) => {
                target.line_clamp = lines;
                target.overflow = if lines.is_some() {
                    TextOverflow::Ellipsis
                } else {
                    TextOverflow::Clip
                };
            }
            Self::Indent(pixels) => target.indent = pixels,
            Self::Truncate => {
                target.overflow = TextOverflow::Ellipsis;
                target.white_space = WhiteSpace::Nowrap;
            }
        }
    }
}
