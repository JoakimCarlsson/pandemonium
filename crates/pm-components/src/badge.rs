//! The badge: a small label in a pill, filled, washed in its colour or quietly neutral.

use crate::theme;
use rustle::{Alignment, Arrangement};
use rustle::{Color, Modifier};

use crate::theme::{Radius, Role};

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

    /// Emits the badge: its words in a pill.
    #[track_caller]
    pub fn build(self) {
        let (fill, words) = match self.variant {
            Variant::Solid => (self.tone, Color::from(Role::Background)),
            Variant::Soft => (self.tone.copy(self.tone.alpha() * 0.18), self.tone),
            Variant::Subtle => (Color::from(Role::SurfaceSelected), self.tone),
        };
        let radius = if self.round { Radius::Full } else { Radius::Sm };
        let inner = Modifier::new().padding_xy(theme::sp(2), 0.0);
        let label = self.label;
        rustle::surface_custom(
            Modifier::new().height(theme::sp(4.5)),
            rustle::Shape::rounded(radius.px()),
            theme::color(fill),
            theme::color(words),
            None,
            move || {
                rustle::row_arranged(inner, Arrangement::Start, Alignment::Center, move || {
                    line(label, 11, 1.2, words);
                });
            },
        );
    }
}

impl From<Badge> for rustle::Node {
    /// The drawn badge, held as a value.
    fn from(badge: Badge) -> Self {
        rustle::compose(move || badge.build())
    }
}
