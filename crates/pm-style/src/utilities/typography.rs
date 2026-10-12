//! Fonts and text.

use pm_color::Color;
use pm_primitives::Number;
use pm_typography::{
    FontFamily, FontSlant, FontWeight, Leading, LineStyle, TextAlign, TextLine, TextOverflow,
    TextSize, TextTransform, TextWrap, Tracking, TypographyRule, WhiteSpace,
};

use crate::Utility;

/// The text colour.
pub fn text_color(color: impl Into<Color>) -> Utility {
    TypographyRule::Color(color.into()).into()
}

/// A step of the type scale: size and matching line height.
pub fn text_size(size: TextSize) -> Utility {
    TypographyRule::Size(size).into()
}

/// A font size in raw logical pixels.
pub fn text_px(pixels: impl Number) -> Utility {
    TypographyRule::SizePx(pixels.value()).into()
}

/// A named font weight.
pub fn font_weight(weight: FontWeight) -> Utility {
    TypographyRule::Weight(weight).into()
}

/// A numeric font weight.
pub fn font_weight_value(weight: u16) -> Utility {
    TypographyRule::WeightValue(weight).into()
}

/// A named letter spacing.
pub fn tracking(tracking: Tracking) -> Utility {
    TypographyRule::Tracking(tracking).into()
}

/// A named line height.
pub fn leading(leading: Leading) -> Utility {
    TypographyRule::Leading(leading).into()
}

/// A line height in steps.
pub fn leading_steps(steps: impl Number) -> Utility {
    TypographyRule::LeadingPx(steps.step_pixels()).into()
}

/// A line height as a multiple of the font size.
pub fn leading_ratio(ratio: impl Number) -> Utility {
    TypographyRule::LeadingRatio(ratio.value()).into()
}

/// The thickness of text decoration lines, in raw logical pixels.
pub fn decoration_px(pixels: impl Number) -> Utility {
    TypographyRule::LineThickness(pixels.value()).into()
}

/// The gap between text and underline, in raw logical pixels.
pub fn underline_offset(pixels: impl Number) -> Utility {
    TypographyRule::UnderlineOffset(pixels.value()).into()
}

/// The colour of text decoration lines.
pub fn decoration_color(color: impl Into<Color>) -> Utility {
    TypographyRule::LineColor(color.into()).into()
}

/// Cut the text after `lines` lines, marking the cut with an ellipsis.
pub fn line_clamp(lines: u16) -> Utility {
    TypographyRule::LineClamp(Some(lines)).into()
}

/// Indent the first line, in steps.
pub fn indent(steps: impl Number) -> Utility {
    TypographyRule::Indent(steps.step_pixels()).into()
}

constants! {
    /// The interface typeface.
    font_sans = TypographyRule::Family(FontFamily::Sans);
    /// A serif typeface.
    font_serif = TypographyRule::Family(FontFamily::Serif);
    /// A fixed-width typeface.
    font_mono = TypographyRule::Family(FontFamily::Mono);
    /// Extra-small text.
    text_xs = TypographyRule::Size(TextSize::Xs);
    /// Small text.
    text_sm = TypographyRule::Size(TextSize::Sm);
    /// Base-size text.
    text_base = TypographyRule::Size(TextSize::Base);
    /// Large text.
    text_lg = TypographyRule::Size(TextSize::Lg);
    /// Extra-large text.
    text_xl = TypographyRule::Size(TextSize::Xl);
    /// 2x-large text.
    text_2xl = TypographyRule::Size(TextSize::Xl2);
    /// 3x-large text.
    text_3xl = TypographyRule::Size(TextSize::Xl3);
    /// 4x-large text.
    text_4xl = TypographyRule::Size(TextSize::Xl4);
    /// 5x-large text.
    text_5xl = TypographyRule::Size(TextSize::Xl5);
    /// Weight 100.
    font_thin = TypographyRule::Weight(FontWeight::Thin);
    /// Weight 200.
    font_extralight = TypographyRule::Weight(FontWeight::Extralight);
    /// Weight 300.
    font_light = TypographyRule::Weight(FontWeight::Light);
    /// Weight 400.
    font_normal = TypographyRule::Weight(FontWeight::Normal);
    /// Weight 500.
    font_medium = TypographyRule::Weight(FontWeight::Medium);
    /// Weight 600.
    font_semibold = TypographyRule::Weight(FontWeight::Semibold);
    /// Weight 700.
    font_bold = TypographyRule::Weight(FontWeight::Bold);
    /// Weight 800.
    font_extrabold = TypographyRule::Weight(FontWeight::Extrabold);
    /// Weight 900.
    font_black = TypographyRule::Weight(FontWeight::Black);
    /// Leaning glyphs.
    italic = TypographyRule::Slant(FontSlant::Italic);
    /// Upright glyphs.
    not_italic = TypographyRule::Slant(FontSlant::Normal);
    /// Tightest letter spacing.
    tracking_tighter = TypographyRule::Tracking(Tracking::Tighter);
    /// Tight letter spacing.
    tracking_tight = TypographyRule::Tracking(Tracking::Tight);
    /// Normal letter spacing.
    tracking_normal = TypographyRule::Tracking(Tracking::Normal);
    /// Wide letter spacing.
    tracking_wide = TypographyRule::Tracking(Tracking::Wide);
    /// Wider letter spacing.
    tracking_wider = TypographyRule::Tracking(Tracking::Wider);
    /// Widest letter spacing.
    tracking_widest = TypographyRule::Tracking(Tracking::Widest);
    /// Line height equal to the font size.
    leading_none = TypographyRule::LeadingRatio(1.0);
    /// Tight line height.
    leading_tight = TypographyRule::Leading(Leading::Tight);
    /// Snug line height.
    leading_snug = TypographyRule::Leading(Leading::Snug);
    /// Normal line height.
    leading_normal = TypographyRule::Leading(Leading::Normal);
    /// Relaxed line height.
    leading_relaxed = TypographyRule::Leading(Leading::Relaxed);
    /// Loose line height.
    leading_loose = TypographyRule::Leading(Leading::Loose);
    /// Lines aligned to the start edge.
    text_start = TypographyRule::Align(TextAlign::Start);
    /// Lines aligned to the end edge.
    text_end = TypographyRule::Align(TextAlign::End);
    /// Lines aligned left.
    text_left = TypographyRule::Align(TextAlign::Left);
    /// Lines centred.
    text_center = TypographyRule::Align(TextAlign::Center);
    /// Lines aligned right.
    text_right = TypographyRule::Align(TextAlign::Right);
    /// Lines justified.
    text_justify = TypographyRule::Align(TextAlign::Justify);
    /// An underline.
    underline = TypographyRule::Line(TextLine::Underline);
    /// A line above the text.
    overline = TypographyRule::Line(TextLine::Overline);
    /// A line through the text.
    line_through = TypographyRule::Line(TextLine::LineThrough);
    /// No decoration lines.
    no_underline = TypographyRule::NoLine;
    /// Solid decoration lines.
    decoration_solid = TypographyRule::LineStyle(LineStyle::Solid);
    /// Double decoration lines.
    decoration_double = TypographyRule::LineStyle(LineStyle::Double);
    /// Dotted decoration lines.
    decoration_dotted = TypographyRule::LineStyle(LineStyle::Dotted);
    /// Dashed decoration lines.
    decoration_dashed = TypographyRule::LineStyle(LineStyle::Dashed);
    /// Wavy decoration lines.
    decoration_wavy = TypographyRule::LineStyle(LineStyle::Wavy);
    /// Upper case.
    uppercase = TypographyRule::Transform(TextTransform::Uppercase);
    /// Lower case.
    lowercase = TypographyRule::Transform(TextTransform::Lowercase);
    /// Capitalised words.
    capitalize = TypographyRule::Transform(TextTransform::Capitalize);
    /// The text as written.
    normal_case = TypographyRule::Transform(TextTransform::None);
    /// Collapse white space and wrap.
    whitespace_normal = TypographyRule::WhiteSpace(WhiteSpace::Normal);
    /// Collapse white space, never wrap.
    whitespace_nowrap = TypographyRule::WhiteSpace(WhiteSpace::Nowrap);
    /// Keep white space, never wrap.
    whitespace_pre = TypographyRule::WhiteSpace(WhiteSpace::Pre);
    /// Collapse white space but keep line breaks.
    whitespace_pre_line = TypographyRule::WhiteSpace(WhiteSpace::PreLine);
    /// Keep white space and wrap.
    whitespace_pre_wrap = TypographyRule::WhiteSpace(WhiteSpace::PreWrap);
    /// Keep white space, wrap, and break at every space.
    whitespace_break_spaces = TypographyRule::WhiteSpace(WhiteSpace::BreakSpaces);
    /// One line, cut with an ellipsis.
    truncate = TypographyRule::Truncate;
    /// Overflowing text ends in an ellipsis.
    text_ellipsis = TypographyRule::Overflow(TextOverflow::Ellipsis);
    /// Overflowing text is cut.
    text_clip = TypographyRule::Overflow(TextOverflow::Clip);
    /// Remove the line limit.
    line_clamp_none = TypographyRule::LineClamp(None);
    /// Break greedily.
    text_wrap = TypographyRule::Wrap(TextWrap::Wrap);
    /// Never break.
    text_nowrap = TypographyRule::Wrap(TextWrap::NoWrap);
    /// Even out line lengths.
    text_balance = TypographyRule::Wrap(TextWrap::Balance);
    /// Avoid a short last line.
    text_pretty = TypographyRule::Wrap(TextWrap::Pretty);
}
