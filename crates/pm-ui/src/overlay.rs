//! Something drawn over the screen rather than laid out inside it.
//!
//! An overlay takes no room: it is measured as nothing, so the screen around
//! it is laid out as though it were not there, and then painted at a point
//! of its own, in a layer of its own. The layer is what makes it an overlay
//! rather than a late sibling: within a layer every quad is drawn before
//! every glyph, so a panel painted late without one would still sit under
//! the text of the screen it covers. Being painted last also makes it the
//! topmost region under the pointer, so it is clicked before what it hides.

use pm_gfx::{Point, Rect, Size};

use crate::element::{Element, IntoElement, LayoutContext, PaintContext};
use crate::style::Style;

/// How close to the edge of the window an overlay may come.
const MARGIN: f32 = 4.0;

/// One element painted at a point of the window's own choosing.
pub struct Overlay<M> {
    /// Where the overlay would like its top left corner to be.
    origin: Point,
    /// What is drawn there.
    child: Box<dyn Element<M>>,
    /// Whether `origin` is the bottom left corner instead, so the overlay
    /// grows upward from it.
    rises: bool,
}

/// `child`, painted at `origin` over whatever was painted before it.
pub fn overlay<M>(origin: Point, child: impl IntoElement<M>) -> Overlay<M> {
    Overlay {
        origin,
        child: child.into_element(),
        rises: false,
    }
}

/// `child`, painted with its bottom left corner at `origin`: the overlay a
/// control along the bottom of the window opens, which belongs above it.
pub fn overlay_above<M>(origin: Point, child: impl IntoElement<M>) -> Overlay<M> {
    Overlay {
        origin,
        child: child.into_element(),
        rises: true,
    }
}

impl<M> Element<M> for Overlay<M> {
    /// An overlay is laid out as nothing; it is placed, not stacked.
    fn layout_style(&self) -> Style {
        Style::default()
    }

    /// Takes no room from the screen it is drawn over.
    fn measure(&mut self, _available: Size, _cx: &mut LayoutContext<'_>) -> Size {
        Size::zero()
    }

    /// Paints the child at its point, moved to keep it inside the window.
    fn paint(&mut self, _bounds: Rect, cx: &mut PaintContext<'_, '_, M>) {
        let window = cx.viewport();
        let size = self.child.measure(window.size, &mut cx.layout);
        let x = self
            .origin
            .x
            .min(window.right() - size.width - MARGIN)
            .max(window.left() + MARGIN);
        let top = match self.rises {
            true => self.origin.y - size.height,
            false => self.origin.y,
        };
        let y = top
            .min(window.bottom() - size.height - MARGIN)
            .max(window.top() + MARGIN);

        cx.push_layer();
        self.child
            .paint(Rect::from_xywh(x, y, size.width, size.height), cx);
        cx.pop_layer();
    }
}

/// One element painted beside another, in a layer of its own.
///
/// This is the overlay that has somewhere to be: a submenu belongs against
/// the row that opened it, and the row only knows where it is once it has
/// been laid out. The panel takes no room either way — the screen is laid
/// out as though only the anchor were there.
pub struct Beside<M> {
    /// The element the panel is placed against.
    anchor: Box<dyn Element<M>>,
    /// What is drawn beside it.
    panel: Box<dyn Element<M>>,
}

/// `panel`, painted against the right edge of `anchor`.
pub fn beside<M>(anchor: impl IntoElement<M>, panel: impl IntoElement<M>) -> Beside<M> {
    Beside {
        anchor: anchor.into_element(),
        panel: panel.into_element(),
    }
}

impl<M> Element<M> for Beside<M> {
    /// Lays out as the anchor does; the panel is placed, not stacked.
    fn layout_style(&self) -> Style {
        self.anchor.layout_style()
    }

    /// Asks for what the anchor asks for, the panel taking no room.
    fn measure(&mut self, available: Size, cx: &mut LayoutContext<'_>) -> Size {
        self.anchor.measure(available, cx)
    }

    /// Paints the anchor where it belongs, then the panel against its edge.
    fn paint(&mut self, bounds: Rect, cx: &mut PaintContext<'_, '_, M>) {
        self.anchor.paint(bounds, cx);

        let window = cx.viewport();
        let size = self.panel.measure(window.size, &mut cx.layout);
        let x = bounds
            .right()
            .min(window.right() - size.width - MARGIN)
            .max(window.left() + MARGIN);
        let y = bounds
            .top()
            .min(window.bottom() - size.height - MARGIN)
            .max(window.top() + MARGIN);

        cx.push_layer();
        self.panel
            .paint(Rect::from_xywh(x, y, size.width, size.height), cx);
        cx.pop_layer();
    }
}

/// One element painted on top of another, in a layer of its own.
///
/// This is the overlay a control along the bottom of a pane opens over the
/// content above it: a list of completions belongs against the box being
/// typed in, as wide as it, and must not push what is above out of the way.
/// The panel takes no room — the screen is laid out as though only the
/// anchor were there.
pub struct Above<M> {
    /// The element the panel is placed on top of.
    anchor: Box<dyn Element<M>>,
    /// What is drawn above it.
    panel: Box<dyn Element<M>>,
}

/// `panel`, painted as wide as `anchor` with its bottom against the anchor's
/// top edge.
pub fn above<M>(anchor: impl IntoElement<M>, panel: impl IntoElement<M>) -> Above<M> {
    Above {
        anchor: anchor.into_element(),
        panel: panel.into_element(),
    }
}

impl<M> Element<M> for Above<M> {
    /// Lays out as the anchor does; the panel is placed, not stacked.
    fn layout_style(&self) -> Style {
        self.anchor.layout_style()
    }

    /// Asks for what the anchor asks for, the panel taking no room.
    fn measure(&mut self, available: Size, cx: &mut LayoutContext<'_>) -> Size {
        self.anchor.measure(available, cx)
    }

    /// Paints the anchor where it belongs, then the panel on top of it.
    fn paint(&mut self, bounds: Rect, cx: &mut PaintContext<'_, '_, M>) {
        self.anchor.paint(bounds, cx);

        let window = cx.viewport();
        let size = self.panel.measure(
            Size::new(bounds.size.width, window.size.height),
            &mut cx.layout,
        );
        let y = (bounds.top() - size.height).max(window.top() + MARGIN);

        cx.push_layer();
        self.panel.paint(
            Rect::from_xywh(bounds.left(), y, bounds.size.width, size.height),
            cx,
        );
        cx.pop_layer();
    }
}
