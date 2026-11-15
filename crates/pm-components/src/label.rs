//! A run of text in one of the interface's tones.

use rustle::prelude::*;

use crate::size::Size;
use rustle::color_scheme;

/// How strongly a label reads.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LabelVariant {
    /// Body text.
    #[default]
    Body,
    /// Secondary text.
    Muted,
    /// Faint text.
    Subtle,
    /// A heading, semi-bold.
    Heading,
    /// Code, in the monospace face.
    Code,
}

/// A single line or paragraph of text.
#[derive(Clone)]
pub struct Label {
    /// The text.
    content: String,
    /// Its tone.
    variant: LabelVariant,
    /// Its size.
    size: Size,
    /// A colour replacing the variant's.
    colour: Option<Color>,
    /// Whether it wraps instead of ending in an ellipsis.
    wrap: bool,
    /// The modifier it is laid out with.
    modifier: Modifier,
}

impl Label {
    /// A body label reading `content`.
    pub fn new(content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
            variant: LabelVariant::Body,
            size: Size::Sm,
            colour: None,
            wrap: false,
            modifier: Modifier::new(),
        }
    }

    /// The label in `variant`.
    pub fn variant(mut self, variant: LabelVariant) -> Self {
        self.variant = variant;
        self
    }

    /// The label at `size`.
    pub fn size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }

    /// The label painted `colour`.
    pub fn colour(mut self, colour: Color) -> Self {
        self.colour = Some(colour);
        self
    }

    /// The label wrapping onto more lines.
    pub fn wrap(mut self) -> Self {
        self.wrap = true;
        self
    }

    /// The label laid out with `modifier`.
    pub fn modifier(mut self, modifier: Modifier) -> Self {
        self.modifier = modifier;
        self
    }

    /// The text style the label is set in.
    pub fn style(&self) -> TextStyle {
        let scheme = color_scheme();
        let interface = crate::interface_font();
        let code = crate::code_font();
        let drawn = match (self.variant, self.size) {
            (LabelVariant::Heading, Size::Xs) => 13.0,
            (LabelVariant::Heading, Size::Sm) => 15.0,
            (LabelVariant::Heading, Size::Md) => 17.0,
            (LabelVariant::Heading, Size::Lg) => 30.0,
            (_, Size::Xs) => 11.0,
            (_, Size::Sm) => 13.0,
            (_, Size::Md) => 14.0,
            (_, Size::Lg) => 16.0,
        };
        let font_size = if self.variant == LabelVariant::Code {
            code.sized(drawn)
        } else {
            interface.sized(drawn)
        };
        let colour = self.colour.unwrap_or(match self.variant {
            LabelVariant::Body | LabelVariant::Heading | LabelVariant::Code => scheme.on_surface,
            LabelVariant::Muted => scheme.on_surface_variant,
            LabelVariant::Subtle => scheme.on_surface_variant,
        });
        TextStyle {
            color: colour,
            font_size,
            line_height: font_size * 1.5,
            font_weight: if self.variant == LabelVariant::Heading {
                FontWeight::SEMI_BOLD
            } else {
                FontWeight::NORMAL
            },
            font_family: if self.variant == LabelVariant::Code {
                code.family
            } else {
                interface.family
            },
            max_lines: (!self.wrap).then_some(1),
            ..TextStyle::default()
        }
    }

    /// Emits the label.
    pub fn show(self) {
        let style = self.style();
        text_styled(self.modifier, self.content, style);
    }
}
