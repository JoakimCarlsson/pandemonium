//! A row or a column of panes, each keeping its share of the space.
//!
//! A split is the one container whose children are sized by proportion
//! rather than by content: the window is divided between them, and the
//! divider between two neighbours moves that division. It places its
//! children itself, so a split of splits is a tree of panes and nothing
//! else — no nested flex boxes, no pixel widths to keep in step with the
//! window as it resizes.

use std::sync::Arc;

use pm_gfx::{Rect, Size};

use crate::element::{Element, IntoElement, LayoutContext, PaintContext};
use crate::resize::{ResizeEvent, SASH_SIZE, divider};
use crate::style::{Axis, Length, Style, Styled};

/// What dragging a divider sends: which one, the drag, and its scale.
type OnResize<M> = Arc<dyn Fn(usize, ResizeEvent, f32) -> M>;

/// A container dividing the space it is given between its children.
pub struct Split<M> {
    /// The axis the children are divided along.
    axis: Axis,
    /// The children, in order along that axis.
    children: Vec<Box<dyn Element<M>>>,
    /// Each child's share of the space, in the same order.
    shares: Vec<f32>,
    /// What dragging a divider sends, when the caller listens for it.
    on_resize: Option<OnResize<M>>,
    /// How the split is sized within its parent.
    style: Style,
}

/// An empty split dividing its space along `axis`.
pub fn split<M>(axis: Axis) -> Split<M> {
    Split {
        axis,
        children: Vec::new(),
        shares: Vec::new(),
        on_resize: None,
        style: Style::default(),
    }
    .w_full()
    .h_full()
}

impl<M> Split<M> {
    /// Appends one child taking `share` of the space.
    ///
    /// Shares are weights rather than fractions: what a child is given is
    /// its share of their total, so a split whose children read 1 and 1 is
    /// the same as one whose children read 0.5 and 0.5.
    pub fn child(mut self, share: f32, child: impl IntoElement<M>) -> Self {
        self.children.push(child.into_element());
        self.shares.push(share.max(0.0));
        self
    }

    /// Returns this split reporting divider drags through `on_resize`.
    ///
    /// The handler is given which divider was dragged, the drag itself and
    /// how much of the split one pixel of travel is worth, because only the
    /// painted split knows how wide it came out.
    pub fn on_resize(mut self, on_resize: impl Fn(usize, ResizeEvent, f32) -> M + 'static) -> Self {
        self.on_resize = Some(Arc::new(on_resize));
        self
    }

    /// The extent left for the panes once the dividers have taken theirs.
    fn pane_extent(&self, bounds: Rect) -> f32 {
        let dividers = self.children.len().saturating_sub(1) as f32 * SASH_SIZE;
        (self.axis.main_of(bounds.size) - dividers).max(0.0)
    }

    /// Each child's extent along the axis, in the order they are painted.
    fn extents(&self, bounds: Rect) -> Vec<f32> {
        let extent = self.pane_extent(bounds);
        let total: f32 = self.shares.iter().sum();
        if total <= 0.0 {
            let each = extent / self.children.len().max(1) as f32;
            return vec![each; self.children.len()];
        }
        self.shares
            .iter()
            .map(|share| extent * share / total)
            .collect()
    }

    /// The rectangle a child of `extent` occupies at `offset` along the axis.
    fn slot(&self, bounds: Rect, offset: f32, extent: f32) -> Rect {
        match self.axis {
            Axis::Horizontal => Rect::from_xywh(
                bounds.left() + offset,
                bounds.top(),
                extent,
                bounds.size.height,
            ),
            Axis::Vertical => Rect::from_xywh(
                bounds.left(),
                bounds.top() + offset,
                bounds.size.width,
                extent,
            ),
        }
    }
}

impl<M> Styled for Split<M> {
    /// How the split is sized within its parent.
    fn style(&mut self) -> &mut Style {
        &mut self.style
    }
}

impl<M: 'static> Element<M> for Split<M> {
    /// How the split is sized within its parent.
    fn layout_style(&self) -> Style {
        self.style
    }

    /// Takes everything it is offered; a split is as large as its region.
    fn measure(&mut self, available: Size, _cx: &mut LayoutContext<'_>) -> Size {
        let width = match self.style.width {
            Length::Px(pixels) => pixels,
            _ => available.width,
        };
        let height = match self.style.height {
            Length::Px(pixels) => pixels,
            _ => available.height,
        };
        Size::new(width, height)
    }

    /// Places every child in its share of the bounds, dividers between them.
    fn paint(&mut self, bounds: Rect, cx: &mut PaintContext<'_, '_, M>) {
        let extents = self.extents(bounds);
        let scale = 1.0 / self.pane_extent(bounds).max(1.0);
        let axis = self.axis;
        let last = self.children.len().saturating_sub(1);
        let on_resize = self.on_resize.clone();
        let mut slots = Vec::with_capacity(self.children.len());
        let mut offset = 0.0;

        for extent in &extents {
            slots.push(self.slot(bounds, offset, *extent));
            offset += extent + SASH_SIZE;
        }

        for (index, (child, slot)) in self.children.iter_mut().zip(&slots).enumerate() {
            child.paint(*slot, cx);
            if index == last {
                continue;
            }
            let Some(on_resize) = on_resize.clone() else {
                continue;
            };
            let line = line_slot(axis, bounds, *slot);
            divider(
                cx,
                line,
                axis,
                Arc::new(move |event| on_resize(index, event, scale)),
            );
        }
    }
}

/// The divider drawn straight after the child occupying `slot`.
fn line_slot(axis: Axis, bounds: Rect, slot: Rect) -> Rect {
    match axis {
        Axis::Horizontal => {
            Rect::from_xywh(slot.right(), bounds.top(), SASH_SIZE, bounds.size.height)
        }
        Axis::Vertical => {
            Rect::from_xywh(bounds.left(), slot.bottom(), bounds.size.width, SASH_SIZE)
        }
    }
}
