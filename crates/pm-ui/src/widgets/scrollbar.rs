//! The scrollbar: a thumb down the right of what it scrolls, saying how much
//! there is, how much of it shows and how far in the view has reached.

use std::sync::Arc;

use pm_gfx::{Quad, Rect, Size};

use crate::element::{Element, IntoElement, LayoutContext, PaintContext};
use crate::resize::ResizeEvent;
use crate::style::Style;
use crate::ui::PointerCursor;

/// Width of the thumb.
const WIDTH: f32 = 6.0;

/// How far the thumb's track sits from the edges of what it scrolls.
const PADDING: f32 = 4.0;

/// Shortest the thumb is drawn, however long the content runs.
const MIN_THUMB: f32 = 25.0;

/// How much of the right of what it scrolls the thumb lies over, with room
/// either side of it: content kept this far in never runs under the thumb.
pub const SCROLLBAR_GUTTER: f32 = WIDTH + PADDING * 2.0;

/// What dragging the thumb sends, given the drag and how far the content
/// moves for each pixel the pointer travels.
pub(crate) type OnScroll<M> = Arc<dyn Fn(ResizeEvent, f32) -> M>;

/// `child`, with a thumb drawn over its right edge.
pub struct Scrollbar<M> {
    /// What is scrolled.
    child: Box<dyn Element<M>>,
    /// How tall the content is, in logical pixels.
    total: f32,
    /// How far down the content the view begins, in logical pixels.
    at: f32,
    /// What dragging the thumb sends.
    on_scroll: OnScroll<M>,
}

/// `child`, scrolled `at` logical pixels down content `total` tall, with a
/// thumb whose drags are reported through `on_scroll`.
///
/// How much of the content shows is the height `child` is painted at.
pub fn scrollbar<M>(
    child: impl IntoElement<M>,
    total: f32,
    at: f32,
    on_scroll: impl Fn(ResizeEvent, f32) -> M + 'static,
) -> Scrollbar<M> {
    Scrollbar {
        child: child.into_element(),
        total,
        at,
        on_scroll: Arc::new(on_scroll),
    }
}

impl<M: 'static> Element<M> for Scrollbar<M> {
    /// The child's own style; the thumb lies over it and takes no room.
    fn layout_style(&self) -> Style {
        self.child.layout_style()
    }

    /// Whatever the child measures to.
    fn measure(&mut self, available: Size, cx: &mut LayoutContext<'_>) -> Size {
        self.child.measure(available, cx)
    }

    /// Paints the child, then the thumb over it while the content runs past
    /// the view: a thumb spanning its whole track says nothing.
    fn paint(&mut self, bounds: Rect, cx: &mut PaintContext<'_, '_, M>) {
        self.child.paint(bounds, cx);
        paint_scrollbar(bounds, self.total, self.at, self.on_scroll.clone(), cx);
    }
}

/// Draws and registers a thumb for the current content and viewport extents.
pub(crate) fn paint_scrollbar<M: 'static>(
    bounds: Rect,
    total: f32,
    at: f32,
    on_scroll: OnScroll<M>,
    cx: &mut PaintContext<'_, '_, M>,
) {
    let showing = bounds.size.height;
    let hidden = total - showing;
    if hidden <= 0.0 {
        return;
    }

    let track = Rect::from_xywh(
        bounds.right() - PADDING - WIDTH,
        bounds.top() + PADDING,
        WIDTH,
        (showing - PADDING * 2.0).max(0.0),
    );
    let extent = (track.size.height * showing / total).max(MIN_THUMB);
    let travel = (track.size.height - extent).max(0.0);
    let reached = (at / hidden).clamp(0.0, 1.0);
    let thumb = Rect::from_xywh(
        track.left(),
        track.top() + travel * reached,
        track.size.width,
        extent,
    );

    let step = if travel > 0.0 { hidden / travel } else { 1.0 };
    let interaction = cx.draggable(
        thumb,
        PointerCursor::Default,
        Arc::new(move |event| on_scroll(event, step)),
        None,
    );

    let theme = *cx.theme();
    let strength = if interaction.hovered || interaction.pressed {
        theme.emphasis.scrollbar_active
    } else {
        theme.emphasis.scrollbar
    };
    cx.quad(
        Quad::filled(thumb, theme.colors.text_subtle.alpha(strength))
            .corner_radius(theme.radius.full),
    );
}
