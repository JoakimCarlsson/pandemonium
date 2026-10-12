//! A list of utilities, ready to be resolved.

use crate::{ComputedStyle, Context, Utility};

/// An element's utilities, split once into what always applies and what
/// depends on state or viewport.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Style {
    base: ComputedStyle,
    conditional: Vec<Utility>,
}

impl Style {
    /// A style from utilities, applied in order.
    pub fn new(utilities: impl IntoIterator<Item = Utility>) -> Self {
        let mut style = Self::default();
        style.extend(utilities);
        style
    }

    /// Applies more utilities after the ones already held.
    pub fn extend(&mut self, utilities: impl IntoIterator<Item = Utility>) {
        for utility in utilities {
            if utility.variants.is_always() {
                self.base.apply(utility.declaration);
            } else {
                self.conditional.push(utility);
            }
        }
        self.conditional.sort_by(|left, right| {
            left.variants
                .min_width()
                .total_cmp(&right.variants.min_width())
        });
    }

    /// What always applies, before any state or breakpoint is considered.
    pub fn base(&self) -> &ComputedStyle {
        &self.base
    }

    /// The style in `context`: the base, then every conditional utility that
    /// matches, narrower breakpoints first and in written order within one.
    pub fn resolve(&self, context: &Context) -> ComputedStyle {
        let mut computed = self.base;
        for utility in &self.conditional {
            if utility.variants.matches(context) {
                computed.apply(utility.declaration);
            }
        }
        computed
    }
}

impl FromIterator<Utility> for Style {
    /// Collects utilities into a style.
    fn from_iter<I: IntoIterator<Item = Utility>>(utilities: I) -> Self {
        Self::new(utilities)
    }
}

impl<const COUNT: usize> From<[Utility; COUNT]> for Style {
    /// A style from an array of utilities.
    fn from(utilities: [Utility; COUNT]) -> Self {
        Self::new(utilities)
    }
}

impl From<&[Utility]> for Style {
    /// A style from a slice of utilities.
    fn from(utilities: &[Utility]) -> Self {
        Self::new(utilities.iter().copied())
    }
}
