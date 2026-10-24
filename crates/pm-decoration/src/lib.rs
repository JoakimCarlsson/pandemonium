//! Decoration styling: backgrounds, borders, shadows and effects.
//!
//! [`DecorationStyle`] is the plain data a painter reads to draw the box
//! around an element's content. Nothing here draws.

mod background;
mod borders;
mod effects;
mod presets;
mod rule;
mod shadows;
mod style;

pub use background::{Background, BackgroundRule, Gradient, GradientDirection};
pub use borders::{BorderRule, BorderStyle, Borders, LineKind, Outline};
pub use effects::{
    BlendMode, Cursor, EffectsRule, EffectsStyle, Interactivity, PointerEvents, UserSelect,
};
pub use presets::{Blur, DropShadow, InsetShadow, Radius, Shadow, TextShadow};
pub use rule::DecorationRule;
pub use shadows::{Ring, ShadowLayer, ShadowRule, ShadowStyle};
pub use style::DecorationStyle;
