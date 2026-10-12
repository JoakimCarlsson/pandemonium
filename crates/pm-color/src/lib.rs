//! Colours, the named palette and opacity.
//!
//! [`Color`] is what a style names: a palette entry such as
//! `Color::Blue500`, or an explicit value. It resolves to [`Srgba`], the
//! gamma-encoded form a renderer takes, through [`Oklch`], the perceptual form
//! the palette is defined in, so mixing and lightening behave the way the eye
//! expects.

mod color;
mod oklch;
mod palette;
mod scheme;
mod srgba;

pub use color::Color;
pub use oklch::{Oklab, Oklch};
pub use scheme::{Interface, Syntax, Terminal};
pub use srgba::Srgba;
