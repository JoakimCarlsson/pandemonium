//! One option of a toggle group: the part that paints and answers to a click.

use pm_gfx::{Point, Quad, Rect, Size};

use crate::element::{Element, LayoutContext, PaintContext};
use crate::style::{Length, Style, space};
use crate::theme::{Font, TextSize};

/// The type an option's label is set in.
const LABEL: Font = Font::new(TextSize::Base);

/// One option of a toggle group.
pub(crate) struct ToggleOption<M> {
    /// The text in the option.
    pub(crate) label: String,
    /// What selecting this option sends.
    pub(crate) message: M,
    /// Whether this is the group's current option.
    pub(crate) selected: bool,
    /// How the option is sized; options share their row equally.
    pub(crate) style: Style,
}

impl<M: Clone> Element<M> for ToggleOption<M> {
    /// Options grow to share their row equally.
    fn layout_style(&self) -> Style {
        Style {
            flex_grow: 1.0,
            ..self.style
        }
    }

    /// Sizes the option around its label.
    fn measure(&mut self, available: Size, cx: &mut LayoutContext<'_>) -> Size {
        let font = LABEL.resolve(&cx.theme.text);
        let label = cx.measure(&self.label, font);
        let width = match self.style.width {
            Length::Px(pixels) => pixels,
            Length::Full => available.width,
            Length::Auto => label.width + space(3.0) * 2.0,
        };

        Size::new(width, cx.theme.size.field)
    }

    /// Registers the press target, then paints the option and its label.
    fn paint(&mut self, bounds: Rect, cx: &mut PaintContext<'_, '_, M>) {
        let interaction = cx.interactive(bounds, self.message.clone());
        let theme = *cx.theme();

        let background = match (self.selected, interaction.pressed, interaction.hovered) {
            (true, _, _) => theme.colors.surface_selected,
            (false, true, _) => theme.colors.surface_active,
            (false, _, true) => theme.colors.surface_hover,
            (false, _, _) => theme.colors.surface,
        };
        let border = if interaction.focused {
            theme.colors.border_focused
        } else if self.selected {
            theme.colors.border_selected
        } else {
            theme.colors.border
        };

        cx.quad(
            Quad::filled(bounds, background)
                .corner_radius(theme.radius.md)
                .border(1.0, border),
        );

        let run = cx.shape(&self.label, LABEL.resolve(&theme.text));
        let color = if self.selected {
            theme.colors.text
        } else {
            theme.colors.text_muted
        };
        let origin = Point::new(
            (bounds.left() + (bounds.size.width - run.width) / 2.0).round(),
            (bounds.top() + (bounds.size.height - run.height) / 2.0).round(),
        );
        cx.text(origin, run, color);
    }
}
