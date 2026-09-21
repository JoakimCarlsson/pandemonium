//! Controls representing the editor's primary layout regions.

use pm_gfx::{Quad, Rect, Rgba, Size};

use crate::{Element, LayoutContext, PaintContext, Style};

/// Width of the outline the icon is drawn as.
const GLYPH_WIDTH: f32 = 16.0;

/// Height of that outline.
const GLYPH_HEIGHT: f32 = 12.0;

/// Space between the outline and the edge of the control.
const PADDING: f32 = 7.0;

/// Which edge an editor layout icon emphasizes.
#[derive(Clone, Copy)]
pub enum LayoutIcon {
    /// The primary sidebar on the left.
    PrimarySidebar,
    /// The panel along the bottom.
    BottomPanel,
    /// The secondary sidebar on the right.
    SecondarySidebar,
}

/// A compact editor-layout toggle drawn from theme-coloured lines.
pub struct LayoutIconButton<M> {
    /// Region represented by the icon.
    icon: LayoutIcon,
    /// Message sent when pressed.
    message: M,
    /// Whether the represented region is visible.
    active: bool,
    /// Fixed control dimensions.
    style: Style,
}

/// Builds one editor-layout toggle with its selected region filled.
pub fn layout_icon_button<M>(icon: LayoutIcon, active: bool, message: M) -> LayoutIconButton<M> {
    let mut style = Style::default();
    style.set_width(30.0);
    style.set_height(28.0);
    LayoutIconButton {
        icon,
        message,
        active,
        style,
    }
}

impl<M: Clone> Element<M> for LayoutIconButton<M> {
    /// Returns the control's fixed dimensions.
    fn layout_style(&self) -> Style {
        self.style
    }

    /// Takes the fixed size established by the control style.
    fn measure(&mut self, _available: Size, cx: &mut LayoutContext<'_>) -> Size {
        Size::new(GLYPH_WIDTH + PADDING * 2.0, cx.theme.size.control)
    }

    /// Paints the interaction state and the editor-layout outline.
    fn paint(&mut self, bounds: Rect, cx: &mut PaintContext<'_, '_, M>) {
        let interaction = cx.interactive(bounds, self.message.clone());
        let theme = *cx.theme();
        let background = if interaction.pressed {
            theme.colors.surface_active
        } else if interaction.hovered {
            theme.colors.surface_hover
        } else {
            Rgba::TRANSPARENT
        };
        cx.quad(Quad::filled(bounds, background).corner_radius(theme.radius.md));

        let icon = Rect::from_xywh(
            bounds.left() + (bounds.size.width - GLYPH_WIDTH) / 2.0,
            bounds.top() + (bounds.size.height - GLYPH_HEIGHT) / 2.0,
            GLYPH_WIDTH,
            GLYPH_HEIGHT,
        );
        let color = theme.colors.text_subtle;
        cx.quad(
            Quad::filled(icon, Rgba::TRANSPARENT)
                .corner_radius(theme.radius.sm)
                .border(1.0, color),
        );

        let (divider, region) = self.parts(icon);
        cx.quad(Quad::filled(divider, color));
        if self.active {
            cx.quad(Quad::filled(region, color));
        }
    }
}

impl<M> LayoutIconButton<M> {
    /// Returns the divider and fill region inside `icon`.
    fn parts(&self, icon: Rect) -> (Rect, Rect) {
        match self.icon {
            LayoutIcon::PrimarySidebar => (
                Rect::from_xywh(icon.left() + 5.0, icon.top() + 1.0, 1.0, 10.0),
                Rect::from_xywh(icon.left() + 1.0, icon.top() + 1.0, 4.0, 10.0),
            ),
            LayoutIcon::BottomPanel => (
                Rect::from_xywh(icon.left() + 1.0, icon.bottom() - 5.0, 14.0, 1.0),
                Rect::from_xywh(icon.left() + 1.0, icon.bottom() - 4.0, 14.0, 3.0),
            ),
            LayoutIcon::SecondarySidebar => (
                Rect::from_xywh(icon.right() - 6.0, icon.top() + 1.0, 1.0, 10.0),
                Rect::from_xywh(icon.right() - 5.0, icon.top() + 1.0, 4.0, 10.0),
            ),
        }
    }
}
