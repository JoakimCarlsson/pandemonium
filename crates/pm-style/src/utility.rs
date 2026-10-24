//! The utility: one declaration and the condition it applies under.

use pm_animation::AnimationRule;
use pm_decoration::{BackgroundRule, BorderRule, DecorationRule, EffectsRule, ShadowRule};
use pm_layout::{
    Breakpoint, DisplayRule, FlexRule, GridRule, LayoutRule, PositioningRule, SizingRule,
    SpacingRule,
};
use pm_typography::TypographyRule;

use crate::{State, Variants};

/// One declaration, from whichever style crate it belongs to.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Declaration {
    /// Spacing, sizing, display, positioning, flex or grid.
    Layout(LayoutRule),
    /// Fonts and text.
    Typography(TypographyRule),
    /// Background, borders, shadows and effects.
    Decoration(DecorationRule),
    /// Transitions, transforms and animations.
    Animation(AnimationRule),
}

/// One declaration and the condition it applies under.
///
/// Utilities are small and `Copy`, so a list of them can be built fresh
/// for every frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Utility {
    /// When the declaration applies.
    pub variants: Variants,
    /// What it declares.
    pub declaration: Declaration,
}

impl Utility {
    /// A declaration that always applies.
    pub const fn new(declaration: Declaration) -> Self {
        Self {
            variants: Variants::ALWAYS,
            declaration,
        }
    }

    /// The same utility, applying only while `state` holds.
    pub const fn when(mut self, state: State) -> Self {
        self.variants.states = self.variants.states.with(state);
        self
    }

    /// The same utility, applying only from `breakpoint` upwards.
    pub const fn from_breakpoint(mut self, breakpoint: Breakpoint) -> Self {
        self.variants.breakpoint = Some(breakpoint);
        self
    }

    /// Applies only while the pointer is over the element.
    pub const fn hover(self) -> Self {
        self.when(State::Hover)
    }

    /// Applies only while the pointer is over the nearest enclosing group.
    pub const fn group_hover(self) -> Self {
        self.when(State::GroupHover)
    }

    /// Applies only while the element has keyboard focus.
    pub const fn focus(self) -> Self {
        self.when(State::Focus)
    }

    /// Applies only while the pointer is held down on the element.
    pub const fn active(self) -> Self {
        self.when(State::Active)
    }

    /// Applies only while the element is disabled.
    pub const fn disabled(self) -> Self {
        self.when(State::Disabled)
    }

    /// Applies only in the dark appearance.
    pub const fn dark(self) -> Self {
        self.when(State::Dark)
    }

    /// Applies only from the small breakpoint upwards.
    pub const fn sm(self) -> Self {
        self.from_breakpoint(Breakpoint::Sm)
    }

    /// Applies only from the medium breakpoint upwards.
    pub const fn md(self) -> Self {
        self.from_breakpoint(Breakpoint::Md)
    }

    /// Applies only from the large breakpoint upwards.
    pub const fn lg(self) -> Self {
        self.from_breakpoint(Breakpoint::Lg)
    }

    /// Applies only from the extra-large breakpoint upwards.
    pub const fn xl(self) -> Self {
        self.from_breakpoint(Breakpoint::Xl)
    }

    /// Applies only from the 2x-large breakpoint upwards.
    pub const fn xl2(self) -> Self {
        self.from_breakpoint(Breakpoint::Xl2)
    }
}

/// Implements `From<$rule> for Utility` through the declaration family and the
/// layout or decoration variant that wraps the rule.
macro_rules! utility_from {
    ($rule:ty => $family:ident, $outer:ident :: $inner:ident) => {
        impl From<$rule> for Utility {
            /// The rule as an unconditional utility.
            fn from(rule: $rule) -> Self {
                Self::new(Declaration::$family($outer::$inner(rule)))
            }
        }
    };
    ($rule:ty => $family:ident) => {
        impl From<$rule> for Utility {
            /// The rule as an unconditional utility.
            fn from(rule: $rule) -> Self {
                Self::new(Declaration::$family(rule))
            }
        }
    };
}

utility_from!(SpacingRule => Layout, LayoutRule::Spacing);
utility_from!(SizingRule => Layout, LayoutRule::Sizing);
utility_from!(DisplayRule => Layout, LayoutRule::Display);
utility_from!(PositioningRule => Layout, LayoutRule::Positioning);
utility_from!(FlexRule => Layout, LayoutRule::Flex);
utility_from!(GridRule => Layout, LayoutRule::Grid);
utility_from!(TypographyRule => Typography);
utility_from!(BackgroundRule => Decoration, DecorationRule::Background);
utility_from!(BorderRule => Decoration, DecorationRule::Border);
utility_from!(ShadowRule => Decoration, DecorationRule::Shadow);
utility_from!(EffectsRule => Decoration, DecorationRule::Effects);
utility_from!(AnimationRule => Animation);
