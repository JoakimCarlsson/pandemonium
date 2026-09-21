//! An element that writes down where it was painted.
//!
//! Layout happens while the frame is built, so nothing outside the tree knows
//! where anything came out until it has been drawn — and a gesture that ends
//! somewhere else entirely, a tab carried across the window to another pane,
//! is answered by the caller, not by the element it started in. This is how
//! the caller is told: the element paints its child and leaves the child's
//! bounds in a cell it shares with the window.

use std::cell::Cell;
use std::rc::Rc;

use pm_gfx::{Rect, Size};

use crate::element::{Element, IntoElement, LayoutContext, PaintContext};
use crate::style::Style;

/// Where an element was last painted, shared with whoever wants to know.
pub type Bounds = Rc<Cell<Rect>>;

/// An element that reports its own bounds as it paints.
pub struct Measured<M> {
    /// Where the child was painted, as of the last frame.
    bounds: Bounds,
    /// What is drawn there.
    child: Box<dyn Element<M>>,
}

/// `child`, leaving where it was painted in `bounds`.
pub fn measured<M>(bounds: Bounds, child: impl IntoElement<M>) -> Measured<M> {
    Measured {
        bounds,
        child: child.into_element(),
    }
}

impl<M> Element<M> for Measured<M> {
    /// The child's own style; measuring changes nothing about layout.
    fn layout_style(&self) -> Style {
        self.child.layout_style()
    }

    /// Whatever the child measures to.
    fn measure(&mut self, available: Size, cx: &mut LayoutContext<'_>) -> Size {
        self.child.measure(available, cx)
    }

    /// Paints the child and writes down where that was.
    fn paint(&mut self, bounds: Rect, cx: &mut PaintContext<'_, '_, M>) {
        self.bounds.set(bounds);
        self.child.paint(bounds, cx);
    }
}
