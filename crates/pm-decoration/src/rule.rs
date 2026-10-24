//! The decoration rule: one declaration of any decoration concern.

use pm_primitives::Rule;

use crate::{BackgroundRule, BorderRule, DecorationStyle, EffectsRule, ShadowRule};

/// One declaration about decoration, whichever concern it belongs to.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DecorationRule {
    /// Fill or gradient.
    Background(BackgroundRule),
    /// Border, radius or outline.
    Border(BorderRule),
    /// Shadow or ring.
    Shadow(ShadowRule),
    /// Opacity, filter, blend or pointer behaviour.
    Effects(EffectsRule),
}

impl Rule for DecorationRule {
    type Target = DecorationStyle;

    /// Writes the declaration into `target`.
    fn apply(self, target: &mut DecorationStyle) {
        match self {
            Self::Background(rule) => rule.apply(&mut target.background),
            Self::Border(rule) => rule.apply(&mut target.borders),
            Self::Shadow(rule) => rule.apply(&mut target.shadows),
            Self::Effects(rule) => rule.apply(&mut target.effects),
        }
    }
}
