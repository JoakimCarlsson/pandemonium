//! Typography styling: families, sizes, weights, spacing, alignment, colour
//! and decoration of text.
//!
//! [`TypographyStyle`] is the plain data a text shaper reads; the rules write
//! into it. Nothing here shapes or draws a glyph.

mod decoration;
mod family;
mod flow;
mod rule;
mod scale;
mod style;

pub use decoration::{LineStyle, TextDecoration, TextLine};
pub use family::{FontFamily, FontSlant};
pub use flow::{TextAlign, TextOverflow, TextTransform, TextWrap, WhiteSpace};
pub use rule::TypographyRule;
pub use scale::{FontWeight, Leading, TextSize, Tracking};
pub use style::{LineHeight, TypographyStyle};
