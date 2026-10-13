//! The app's icons: Phosphor artwork, named and sized.
//!
//! Each icon is one piece of vector artwork shipped with the app, drawn at
//! whatever size the caller asks for and tinted from the theme. Artwork is
//! never coloured in the file: an icon in a disabled row and the same icon in
//! a selected one are one drawing, tinted twice. The crate holds the artwork
//! and its names; placing and painting an icon belongs to the renderer.

mod name;
mod size;
mod svg;

pub use name::IconName;
pub use size::IconSize;
pub use svg::{Mask, Svg};
