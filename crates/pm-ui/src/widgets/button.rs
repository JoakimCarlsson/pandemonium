//! The button: a label in a box that sends one message when it is pressed.

use pm_gfx::{Point, Quad, Rect, Rgba, Size};

use crate::element::{Element, Interaction, LayoutContext, PaintContext};
use crate::style::{Length, Style, Styled, space};
use crate::theme::{Font, TextSize, Theme};

/// The type a button's label is set in.
const LABEL: Font = Font::new(TextSize::Base).weight(500);

/// How much of the theme's accent a button carries.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ButtonVariant {
    /// The one action on a screen: filled with the accent.
    Filled,
    /// A secondary action: a surface with a border.
    Outlined,
    /// A tertiary action: nothing until the pointer is over it.
    Ghost,
}

/// A label in a box that sends `message` when it is clicked or activated.
pub struct Button<M> {
    /// The text in the box.
    label: String,
    /// What a press sends.
    message: M,
    /// How much of the accent the box carries.
    variant: ButtonVariant,
    /// How the box is sized and padded.
    style: Style,
}

/// An outlined button labelled `label` that sends `message` when pressed.
pub fn button<M>(label: impl Into<String>, message: M) -> Button<M> {
    Button {
        label: label.into(),
        message,
        variant: ButtonVariant::Outlined,
        style: Style::default(),
    }
}

impl<M> Button<M> {
    /// Returns this button filled with the accent.
    pub fn filled(mut self) -> Self {
        self.variant = ButtonVariant::Filled;
        self
    }

    /// Returns this button as a surface with a border.
    pub fn outlined(mut self) -> Self {
        self.variant = ButtonVariant::Outlined;
        self
    }

    /// Returns this button without a box until it is hovered.
    pub fn ghost(mut self) -> Self {
        self.variant = ButtonVariant::Ghost;
        self
    }

    /// The fill for this variant in `interaction`.
    fn background(&self, theme: &Theme, interaction: Interaction) -> Rgba {
        match (self.variant, interaction.pressed, interaction.hovered) {
            (ButtonVariant::Filled, true, _) => theme.colors.accent_active,
            (ButtonVariant::Filled, _, true) => theme.colors.accent_hover,
            (ButtonVariant::Filled, _, _) => theme.colors.accent,
            (ButtonVariant::Outlined, true, _) => theme.colors.surface_active,
            (ButtonVariant::Outlined, _, true) => theme.colors.surface_hover,
            (ButtonVariant::Outlined, _, _) => theme.colors.surface,
            (ButtonVariant::Ghost, true, _) => theme.colors.surface_active,
            (ButtonVariant::Ghost, _, true) => theme.colors.surface_hover,
            (ButtonVariant::Ghost, _, _) => Rgba::TRANSPARENT,
        }
    }

    /// The label colour for this variant.
    fn foreground(&self, theme: &Theme) -> Rgba {
        match self.variant {
            ButtonVariant::Filled => theme.colors.text_on_accent,
            ButtonVariant::Outlined => theme.colors.text,
            ButtonVariant::Ghost => theme.colors.text_muted,
        }
    }

    /// The border for this variant when it holds focus or does not.
    fn border(&self, theme: &Theme, interaction: Interaction) -> (f32, Rgba) {
        if interaction.focused {
            return (1.0, theme.colors.border_focused);
        }
        match self.variant {
            ButtonVariant::Outlined => (1.0, theme.colors.border),
            _ => (0.0, Rgba::TRANSPARENT),
        }
    }
}

impl<M> Styled for Button<M> {
    /// How the box is sized and padded.
    fn style(&mut self) -> &mut Style {
        &mut self.style
    }
}

impl<M: Clone> Element<M> for Button<M> {
    /// How the box is sized and padded.
    fn layout_style(&self) -> Style {
        self.style
    }

    /// Sizes the box around its label.
    fn measure(&mut self, available: Size, cx: &mut LayoutContext<'_>) -> Size {
        let font = LABEL.resolve(&cx.theme.text);
        let label = cx.measure(&self.label, font);
        let width = match self.style.width {
            Length::Px(pixels) => pixels,
            Length::Full => available.width,
            Length::Auto => label.width + space(3.0) * 2.0,
        };
        let height = match self.style.height {
            Length::Px(pixels) => pixels,
            Length::Full => available.height,
            Length::Auto => cx.theme.size.control,
        };

        Size::new(width, height)
    }

    /// Registers the press target, then paints the box and its label.
    fn paint(&mut self, bounds: Rect, cx: &mut PaintContext<'_, '_, M>) {
        let interaction = cx.interactive(bounds, self.message.clone());
        let theme = *cx.theme();
        let (border_width, border_color) = self.border(&theme, interaction);

        cx.quad(
            Quad::filled(bounds, self.background(&theme, interaction))
                .corner_radius(theme.radius.md)
                .border(border_width, border_color),
        );

        let run = cx.shape(&self.label, LABEL.resolve(&theme.text));
        let origin = Point::new(
            (bounds.left() + (bounds.size.width - run.width) / 2.0).round(),
            (bounds.top() + (bounds.size.height - run.height) / 2.0).round(),
        );
        cx.text(origin, run, self.foreground(&theme));
    }
}
