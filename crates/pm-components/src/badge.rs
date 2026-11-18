//! The badge: a small label in a pill, filled, washed in its colour or quietly neutral.

use rustle::Modifier;

use crate::kit::{Flex, Radius, Role, Styled, Tone, row};

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
/// emit it with [`Badge::build`].
#[derive(Clone, Debug)]
pub struct Badge {
    /// The words on the badge.
    label: String,
    /// The colour of its fill or its words, by variant.
    tone: Tone,
    /// How it looks.
    variant: Variant,
    /// Whether its corners are fully round.
    round: bool,
    /// What the caller puts around its box, outside its own.
    modifier: Modifier,
    /// The padding the caller gave in place of the badge's own.
    inner: Option<Modifier>,
}

impl Badge {
    /// A neutral badge saying `label` in the muted colour, with fully round corners.
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            tone: Tone::from(Role::Muted),
            variant: Variant::default(),
            round: true,
            modifier: Modifier::new(),
            inner: None,
        }
    }

    /// The badge in `tone`.
    pub fn tone(mut self, tone: impl Into<Tone>) -> Self {
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

    /// The badge with `modifier` put around its box, outside its own: sizes, weight, placement.
    pub fn modifier(mut self, modifier: Modifier) -> Self {
        self.modifier = self.modifier.chain(modifier);
        self
    }

    /// The badge with `modifier`, padding, in place of its own padding, inside its fill.
    pub fn inner(mut self, modifier: Modifier) -> Self {
        self.inner = Some(modifier);
        self
    }

    /// Emits the badge: its words in a pill.
    #[track_caller]
    pub fn build(self) {
        let (fill, words) = match self.variant {
            Variant::Solid => (self.tone, Tone::from(Role::Background)),
            Variant::Soft => (self.tone.alpha(0.18), self.tone),
            Variant::Subtle => (Tone::from(Role::SurfaceSelected), self.tone),
        };
        let radius = if self.round { Radius::Full } else { Radius::Sm };
        let own = Modifier::new()
            .h(4.5)
            .fill_rounded(fill, radius)
            .chain(self.inner.unwrap_or_else(|| Modifier::new().pad_x(2)));
        let label = self.label;
        row(self.modifier.chain(own), Flex::new().center(), move || {
            line(label, 11, 1.2, words);
        });
    }
}

impl From<Badge> for rustle::Node {
    /// The drawn badge, held as a value.
    fn from(badge: Badge) -> Self {
        rustle::compose(move || badge.build())
    }
}
