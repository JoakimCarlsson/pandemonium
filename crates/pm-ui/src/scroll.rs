//! How far a scrollable area is scrolled, and what that does to its layout.
//!
//! Scrolling is a property of the area being scrolled, not of the window: the
//! content is laid out in a space as tall as it needs and painted at a
//! negative offset, and the offset is clamped against what the last frame
//! actually painted. One of these belongs to each area that scrolls, so two
//! of them side by side scroll independently.
//!
//! The window scrolls its whole page this way; a part of a screen scrolls
//! inside a [`ScrollArea`], which shares its [`Scroll`] with the caller so a
//! wheel turned over it can move the offset the next frame is drawn at.

use std::cell::Cell;
use std::rc::Rc;

use pm_gfx::{Point, Rect, Size};

use crate::element::{Element, IntoElement, LayoutContext, PaintContext};
use crate::resize::ResizeEvent;
use crate::style::{Style, Styled};
use crate::widgets::{OnScroll, SCROLLBAR_GUTTER, paint_scrollbar};
use std::sync::Arc;

/// The scroll offset of one area, and the extents it is clamped against.
#[derive(Clone, Copy, Debug, Default)]
pub struct Scroll {
    /// How far the content is shifted up, in logical pixels.
    offset: f32,
    /// The height the last frame painted, in logical pixels.
    content_height: f32,
    /// The space the content is seen through, in logical pixels.
    viewport: Size,
}

impl Scroll {
    /// A scroll `offset` logical pixels down, held against the content's
    /// extents once a frame has painted it.
    pub fn at(offset: f32) -> Self {
        Self {
            offset,
            ..Self::default()
        }
    }

    /// Records the space the content is seen through.
    pub fn set_viewport(&mut self, viewport: Size) {
        self.viewport = viewport;
        self.clamp();
    }

    /// Records how tall the content turned out, once a frame has painted it.
    pub fn set_content_height(&mut self, height: f32) {
        self.content_height = height;
        self.clamp();
    }

    /// Records the space the content is seen through and how tall it turned
    /// out together, holding the offset against both at once.
    ///
    /// Recording one and then the other holds the offset against a pair of
    /// extents that never existed, and a scroll made with [`Scroll::at`],
    /// which knows neither yet, would be held against no content at all.
    pub fn set_extents(&mut self, viewport: Size, height: f32) {
        self.viewport = viewport;
        self.content_height = height;
        self.clamp();
    }

    /// Scrolls by `delta` logical pixels, positive being towards the top.
    pub fn by(&mut self, delta: f32) {
        self.offset -= delta;
        self.clamp();
    }

    /// How far down the content the viewport begins.
    pub fn offset(&self) -> f32 {
        self.offset
    }

    /// Height of the whole content from the last frame.
    pub fn content_height(&self) -> f32 {
        self.content_height
    }

    /// Height of the visible content area from the last frame.
    pub fn viewport_height(&self) -> f32 {
        self.viewport.height
    }

    /// The space to lay the content out in: as tall as the offset reaches.
    pub fn content_space(&self) -> Size {
        Size::new(self.viewport.width, self.viewport.height + self.offset)
    }

    /// Where the content's top-left corner lands at this offset.
    pub fn origin(&self) -> Point {
        Point::new(0.0, -self.offset)
    }

    /// Holds the offset between the top and the end of the content.
    fn clamp(&mut self) {
        let limit = (self.content_height - self.viewport.height).max(0.0);
        self.offset = self.offset.clamp(0.0, limit);
    }
}

/// The scroll of one area, shared between the caller and the element drawing it.
pub type Scrolled = Rc<Cell<Scroll>>;

/// An area that shows its child through a window, scrolled by a [`Scroll`].
///
/// The child is laid out as tall as it wants and painted shifted up by the
/// offset, clipped to the area; the area writes back the extents it found,
/// so the offset the caller moves is clamped against what was really there.
pub struct ScrollArea<M> {
    /// The scroll the area is drawn at, and records its extents in.
    scroll: Scrolled,
    /// How the area itself is sized.
    style: Style,
    /// What is scrolled.
    child: Box<dyn Element<M>>,
    /// Optional visible thumb and its drag handler.
    on_scroll: Option<OnScroll<M>>,
    /// Whether overflowing content leaves room for the visible scrollbar.
    scrollbar_gutter: bool,
    /// Whether placed text in this area can be selected.
    selectable: bool,
}

/// `child`, scrolled by `scroll` inside whatever room the area is given.
pub fn scroll_area<M>(scroll: Scrolled, child: impl IntoElement<M>) -> ScrollArea<M> {
    ScrollArea {
        scroll,
        style: Style::default(),
        child: child.into_element(),
        on_scroll: None,
        scrollbar_gutter: false,
        selectable: false,
    }
}

impl<M> ScrollArea<M> {
    /// Enables pointer selection and copying of the area’s placed text.
    pub fn selectable(mut self) -> Self {
        self.selectable = true;
        self
    }

    /// Shows a scrollbar when content overflows, reporting thumb drags to the caller.
    pub fn with_scrollbar(mut self, on_scroll: impl Fn(ResizeEvent, f32) -> M + 'static) -> Self {
        self.on_scroll = Some(Arc::new(on_scroll));
        self
    }

    /// Reserves room beside the content only while its scrollbar is visible.
    pub fn reserve_scrollbar_gutter(mut self) -> Self {
        self.scrollbar_gutter = true;
        self
    }
}

impl<M> Styled for ScrollArea<M> {
    /// How the area is sized in its parent.
    fn style(&mut self) -> &mut Style {
        &mut self.style
    }
}

impl<M: 'static> Element<M> for ScrollArea<M> {
    /// How the area is sized in its parent.
    fn layout_style(&self) -> Style {
        self.style
    }

    /// The whole of what it is offered: the content scrolls rather than grows.
    fn measure(&mut self, available: Size, _cx: &mut LayoutContext<'_>) -> Size {
        available
    }

    /// Paints the child at the scrolled offset, clipped to the area.
    fn paint(&mut self, bounds: Rect, cx: &mut PaintContext<'_, '_, M>) {
        let mut available = bounds.size;
        let mut content = self.child.measure(available, &mut cx.layout);
        if self.scrollbar_gutter && self.on_scroll.is_some() && content.height > bounds.size.height
        {
            available.width = (available.width - SCROLLBAR_GUTTER).max(0.0);
            content = self.child.measure(available, &mut cx.layout);
        }
        let mut scroll = self.scroll.get();
        scroll.set_extents(bounds.size, content.height);
        self.scroll.set(scroll);

        let previous = cx.selection.take();
        if self.selectable {
            let surface = cx.selections.borrow_mut().surface(&self.scroll);
            {
                let mut state = surface.borrow_mut();
                state.bounds = bounds;
                state.visible = true;
                state.rows.clear();
                state.placements.clear();
                state.starts.clear();
                state.last_bounds = None;
            }
            cx.selection = Some(surface);
            cx.clickable(bounds, None, None);
        }
        let first = cx.region_count();
        cx.push_clip(bounds);
        self.child.paint(
            Rect::from_xywh(
                bounds.left(),
                bounds.top() + scroll.origin().y,
                available.width,
                content.height.max(bounds.size.height),
            ),
            cx,
        );
        cx.selection = previous;
        cx.pop_clip();
        cx.clip_regions(first, bounds);
        if let Some(on_scroll) = &self.on_scroll {
            paint_scrollbar(
                bounds,
                content.height,
                scroll.offset(),
                on_scroll.clone(),
                cx,
            );
        }
    }
}
