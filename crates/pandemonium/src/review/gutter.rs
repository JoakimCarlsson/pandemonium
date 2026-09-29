//! The column of line numbers, which shows a button while the pointer is on it.
//!
//! A diff line's numbers are what the eye reads; the button that comments on
//! the line is only wanted by the reader who is pointing at it. The two are
//! the same size and drawn in the same place, and which of them is drawn is
//! whether the pointer is over that place.

use pm_gfx::{Rect, Size};
use pm_ui::{Element, IntoElement, LayoutContext, PaintContext, Style};

/// Two looks for one place: the one it has, and the one it has under the
/// pointer.
pub struct Reveal<M> {
    /// What is drawn while the pointer is elsewhere, and what is measured.
    idle: Box<dyn Element<M>>,
    /// What is drawn while the pointer is over it.
    hovered: Box<dyn Element<M>>,
}

/// `idle`, drawn as `hovered` while the pointer is over it.
pub fn revealing<M>(idle: impl IntoElement<M>, hovered: impl IntoElement<M>) -> Reveal<M> {
    Reveal {
        idle: idle.into_element(),
        hovered: hovered.into_element(),
    }
}

impl<M> Element<M> for Reveal<M> {
    /// The idle look's own style: the place is as big as it is.
    fn layout_style(&self) -> Style {
        self.idle.layout_style()
    }

    /// Whatever the idle look measures to.
    fn measure(&mut self, available: Size, cx: &mut LayoutContext<'_>) -> Size {
        self.idle.measure(available, cx)
    }

    /// Draws the look the pointer's place calls for.
    fn paint(&mut self, bounds: Rect, cx: &mut PaintContext<'_, '_, M>) {
        match cx.input().is_over(bounds) {
            true => {
                self.hovered.measure(bounds.size, &mut cx.layout);
                self.hovered.paint(bounds, cx);
            }
            false => self.idle.paint(bounds, cx),
        }
    }
}
