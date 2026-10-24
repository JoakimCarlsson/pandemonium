//! The resolved style of one element.

use pm_animation::AnimationStyle;
use pm_decoration::DecorationStyle;
use pm_layout::LayoutStyle;
use pm_primitives::Rule;
use pm_typography::TypographyStyle;

use crate::Declaration;

/// Every style crate's slice, with all utilities applied: the data layout,
/// text shaping, painting and animation read.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ComputedStyle {
    /// What the layout pass reads.
    pub layout: LayoutStyle,
    /// What the text shaper reads.
    pub typography: TypographyStyle,
    /// What the painter reads.
    pub decoration: DecorationStyle,
    /// What the animation driver reads.
    pub animation: AnimationStyle,
}

impl ComputedStyle {
    /// Writes one declaration into the slice it belongs to.
    pub fn apply(&mut self, declaration: Declaration) {
        match declaration {
            Declaration::Layout(rule) => rule.apply(&mut self.layout),
            Declaration::Typography(rule) => rule.apply(&mut self.typography),
            Declaration::Decoration(rule) => rule.apply(&mut self.decoration),
            Declaration::Animation(rule) => rule.apply(&mut self.animation),
        }
    }
}
