//! Transitions: easing a change of value over a duration.

use crate::CubicBezier;

/// Which properties a transition covers.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum TransitionProperty {
    /// No property changes gradually.
    #[default]
    None,
    /// Every animatable property.
    All,
    /// Colours: background, border, text, fill and decoration.
    Colors,
    /// Opacity.
    Opacity,
    /// Box shadow and ring.
    Shadow,
    /// Translate, scale, rotate and skew.
    Transform,
    /// Colours, opacity, shadow, transform and filters.
    Common,
}

/// How a change of style is spread over time.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Transition {
    /// The properties that ease.
    pub property: TransitionProperty,
    /// How long the change takes, in milliseconds.
    pub duration_ms: f32,
    /// The timing curve.
    pub ease: CubicBezier,
    /// How long to wait before starting, in milliseconds.
    pub delay_ms: f32,
}

impl Transition {
    /// The eased progress, from 0.0 to 1.0, `elapsed_ms` after the change.
    pub fn progress(self, elapsed_ms: f32) -> f32 {
        if self.duration_ms <= 0.0 {
            return 1.0;
        }
        self.ease
            .progress((elapsed_ms - self.delay_ms) / self.duration_ms)
    }

    /// Whether the change has finished `elapsed_ms` after it began.
    pub fn finished(self, elapsed_ms: f32) -> bool {
        elapsed_ms >= self.delay_ms + self.duration_ms
    }
}

impl Default for Transition {
    /// Nothing eases; when one does, it takes 150 milliseconds on the standard curve.
    fn default() -> Self {
        Self {
            property: TransitionProperty::None,
            duration_ms: 150.0,
            ease: CubicBezier::new(0.4, 0.0, 0.2, 1.0),
            delay_ms: 0.0,
        }
    }
}
