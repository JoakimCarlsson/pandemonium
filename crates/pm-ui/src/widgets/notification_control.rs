//! Shared notification icon controls with tooltips and visible keyboard focus.

use pm_gfx::{Quad, Rect, Rgba, Size};

use crate::{Element, Icon, IconName, IconSize, LayoutContext, PaintContext, Style, icon};

/// A notification icon button whose keyboard focus is visibly outlined.
pub struct NotificationControl<M> {
    /// The shared icon artwork.
    icon: Icon,
    /// The accessible tooltip naming the action.
    label: String,
    /// The message sent by activating the control.
    message: M,
}

/// Builds a labelled icon control for an explicit notification action.
pub fn notification_control<M>(
    _theme: &crate::Theme,
    name: IconName,
    label: &str,
    message: M,
) -> NotificationControl<M> {
    NotificationControl {
        icon: icon(name).size(IconSize::Medium),
        label: label.to_owned(),
        message,
    }
}

impl<M: Clone> Element<M> for NotificationControl<M> {
    /// Keeps the control at its intrinsic size in a flex row.
    fn layout_style(&self) -> Style {
        Style::default()
    }

    /// Measures a square control using the theme's shared icon-button size.
    fn measure(&mut self, _available: Size, cx: &mut LayoutContext<'_>) -> Size {
        Size::new(cx.theme.size.icon_control, cx.theme.size.icon_control)
    }

    /// Registers one focus target, paints its feedback and centers the shared icon.
    fn paint(&mut self, bounds: Rect, cx: &mut PaintContext<'_, '_, M>) {
        let interaction = cx.interactive(bounds, self.message.clone());
        let theme = *cx.theme();
        let background = match (interaction.pressed, interaction.hovered) {
            (true, _) => theme.colors.surface_active,
            (_, true) => theme.colors.surface_hover,
            _ => Rgba::TRANSPARENT,
        };
        cx.quad(
            Quad::filled(bounds, background)
                .corner_radius(theme.radius.md)
                .border(
                    if interaction.focused { 1.0 } else { 0.0 },
                    theme.colors.border_focused,
                ),
        );
        if interaction.hovered || interaction.focused {
            cx.tooltip(bounds, self.label.clone());
        }
        let size = <Icon as Element<M>>::measure(&mut self.icon, bounds.size, &mut cx.layout);
        self.icon.paint(
            Rect::from_xywh(
                bounds.left() + (bounds.size.width - size.width) * 0.5,
                bounds.top() + (bounds.size.height - size.height) * 0.5,
                size.width,
                size.height,
            ),
            cx,
        );
    }
}
