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
}

/// `child`, painted at `origin` over whatever was painted before it.
pub fn overlay<M>(origin: Point, child: impl IntoElement<M>) -> Overlay<M> {
    Overlay {
        origin,
        child: child.into_element(),
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
        let y = self
            .origin
            .y
            .min(window.bottom() - size.height - MARGIN)
            .max(window.top() + MARGIN);

        cx.push_layer();
        self.child
            .paint(Rect::from_xywh(x, y, size.width, size.height), cx);
        cx.pop_layer();
    }
}
