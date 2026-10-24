//! The badge: a small label in a pill, filled, washed in its colour or quietly neutral.

use pm_style::{
    Color, Radius, Role, bg, flex, flex_none, h, items_center, px, rounded, rounded_full,
};
use pm_ui::{Node, node};

use crate::parts::line;

/// How a badge looks.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Variant {
    /// A fill in its colour with the words cut out of it.
    Solid,
    /// A faint wash of its colour with the words in it.
    Soft,
    /// A neutral wash with the words in its colour.
    #[default]
    Subtle,
}

/// A badge being described: what it says, in which colour and how it looks.
///
/// Build it with [`Badge::new`], change it with the methods that follow, and
/// turn it into a node with [`Badge::build`].
#[derive(Clone, Debug)]
pub struct Badge {
    /// The words on the badge.
    label: String,
    /// The colour of its fill or its words, by variant.
    tone: Color,
    /// How it looks.
    variant: Variant,
    /// Whether its corners are fully round.
    round: bool,
}

impl Badge {
    /// A neutral badge saying `label` in the muted colour, with fully round corners.
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            tone: Color::from(Role::Muted),
            variant: Variant::default(),
            round: true,
        }
    }

    /// The badge in `tone`.
    pub fn tone(mut self, tone: impl Into<Color>) -> Self {
        self.tone = tone.into();
        self
    }

    /// The badge looking like `variant`.
    pub fn variant(mut self, variant: Variant) -> Self {
        self.variant = variant;
        self
    }

    /// The badge with small rounded corners instead of fully round ones.
    pub fn square(mut self) -> Self {
        self.round = false;
        self
    }

    /// The node that draws the badge.
    pub fn build(self) -> Node {
        let (fill, words) = match self.variant {
            Variant::Solid => (self.tone, Color::from(Role::Background)),
            Variant::Soft => (self.tone.alpha(0.18), self.tone),
            Variant::Subtle => (Color::from(Role::SurfaceSelected), self.tone),
        };
        let pill = node().styles([flex(), items_center(), flex_none(), h(4.5), px(2), bg(fill)]);
        let pill = if self.round {
            pill.styles([rounded_full()])
        } else {
            pill.styles([rounded(Radius::Sm)])
        };
        pill.child(line(self.label, 11, 1.2, words))
    }
}

impl From<Badge> for Node {
    /// The node that draws the badge.
    fn from(badge: Badge) -> Self {
        badge.build()
    }
}
