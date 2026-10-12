//! The public styling API: a style is a list of small utility values.
//!
//! ```ignore
//! let styles = [
//!     flex(),
//!     flex_col(),
//!     gap(4),
//!     p(6),
//!     bg(Color::Blue500),
//!     rounded(8),
//!     bg(Color::Blue700).hover(),
//! ];
//! let style = Style::from(styles);
//! let computed = style.resolve(&Context::default());
//! ```
//!
//! Every utility is a [`Utility`]: one declaration from one of the style
//! crates, optionally gated by a [`Variants`] condition such as hover or a
//! viewport breakpoint. [`Style`] collects them, and [`Style::resolve`] turns
//! them into the [`ComputedStyle`] that layout, text shaping, painting and
//! animation read.
//!
//! Counts such as `p(4)` are steps of the spacing scale (four pixels each);
//! functions that take raw units say so in their docs.

mod computed;
mod style;
mod utilities;
mod utility;
mod variants;

pub use computed::ComputedStyle;
pub use pm_animation as animation;
pub use pm_animation::{Animation, CubicBezier, Ease, Origin, Transform, TransitionProperty};
pub use pm_color as color;
pub use pm_color::{Color, Oklch, Srgba};
pub use pm_decoration as decoration;
pub use pm_decoration::{
    BlendMode, Blur, BorderStyle, Cursor, DropShadow, GradientDirection, InsetShadow,
    PointerEvents, Radius, Shadow, TextShadow, UserSelect,
};
pub use pm_layout as layout;
pub use pm_layout::{Breakpoint, Container};
pub use pm_primitives as primitives;
pub use pm_primitives::{Axis, Length, Number, Sides};
pub use pm_typography as typography;
pub use pm_typography::{FontWeight, Leading, TextSize, Tracking};
pub use style::Style;
pub use utilities::*;
pub use utility::{Declaration, Utility};
pub use variants::{Context, State, StateSet, Variants};
