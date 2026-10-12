//! Opacity, filters, blending and pointer behaviour.

use pm_decoration::{BlendMode, Cursor, EffectsRule, PointerEvents, UserSelect};
use pm_primitives::Number;

use crate::Utility;

/// Opacity of the element and its content, as a percentage from 0 to 100.
pub fn opacity(percent: impl Number) -> Utility {
    EffectsRule::Opacity(percent.value() / 100.0).into()
}

/// Blur radius of the element, in raw logical pixels or as a named [`Blur`](crate::Blur).
pub fn blur(radius: impl Number) -> Utility {
    EffectsRule::Blur(radius.value()).into()
}

/// Blur radius of what is behind the element.
pub fn backdrop_blur(radius: impl Number) -> Utility {
    EffectsRule::BackdropBlur(radius.value()).into()
}

/// Brightness as a percentage; 100 leaves it unchanged.
pub fn brightness(percent: impl Number) -> Utility {
    EffectsRule::Brightness(percent.value() / 100.0).into()
}

/// Contrast as a percentage; 100 leaves it unchanged.
pub fn contrast(percent: impl Number) -> Utility {
    EffectsRule::Contrast(percent.value() / 100.0).into()
}

/// Saturation as a percentage; 100 leaves it unchanged.
pub fn saturate(percent: impl Number) -> Utility {
    EffectsRule::Saturate(percent.value() / 100.0).into()
}

/// Hue rotation in degrees.
pub fn hue_rotate(degrees: impl Number) -> Utility {
    EffectsRule::HueRotate(degrees.value()).into()
}

constants! {
    /// Fully desaturated.
    grayscale = EffectsRule::Grayscale(1.0);
    /// Blends by multiplying.
    blend_multiply = EffectsRule::Blend(BlendMode::Multiply);
    /// Blends by screening.
    blend_screen = EffectsRule::Blend(BlendMode::Screen);
    /// Blends by overlaying.
    blend_overlay = EffectsRule::Blend(BlendMode::Overlay);
    /// Blends keeping the darker channel.
    blend_darken = EffectsRule::Blend(BlendMode::Darken);
    /// Blends keeping the lighter channel.
    blend_lighten = EffectsRule::Blend(BlendMode::Lighten);
    /// Blends by difference.
    blend_difference = EffectsRule::Blend(BlendMode::Difference);
    /// Covers what is beneath.
    blend_normal = EffectsRule::Blend(BlendMode::Normal);
    /// The default pointer.
    cursor_default = EffectsRule::Cursor(Cursor::Default);
    /// A pointing hand.
    cursor_pointer = EffectsRule::Cursor(Cursor::Pointer);
    /// An I-beam.
    cursor_text = EffectsRule::Cursor(Cursor::Text);
    /// Four arrows.
    cursor_move = EffectsRule::Cursor(Cursor::Move);
    /// A crossed-out circle.
    cursor_not_allowed = EffectsRule::Cursor(Cursor::NotAllowed);
    /// An open hand.
    cursor_grab = EffectsRule::Cursor(Cursor::Grab);
    /// A closed hand.
    cursor_grabbing = EffectsRule::Cursor(Cursor::Grabbing);
    /// A busy indicator.
    cursor_wait = EffectsRule::Cursor(Cursor::Wait);
    /// A crosshair.
    cursor_crosshair = EffectsRule::Cursor(Cursor::Crosshair);
    /// A horizontal resize arrow.
    cursor_col_resize = EffectsRule::Cursor(Cursor::ColResize);
    /// A vertical resize arrow.
    cursor_row_resize = EffectsRule::Cursor(Cursor::RowResize);
    /// Takes pointer input.
    pointer_events_auto = EffectsRule::PointerEvents(PointerEvents::Auto);
    /// Lets the pointer through.
    pointer_events_none = EffectsRule::PointerEvents(PointerEvents::None);
    /// Text is not selectable.
    select_none = EffectsRule::UserSelect(UserSelect::None);
    /// Text is selectable.
    select_text = EffectsRule::UserSelect(UserSelect::Text);
    /// One click selects everything.
    select_all = EffectsRule::UserSelect(UserSelect::All);
    /// The default for the element's kind.
    select_auto = EffectsRule::UserSelect(UserSelect::Auto);
}
