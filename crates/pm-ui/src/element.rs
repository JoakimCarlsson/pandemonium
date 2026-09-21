//! The element tree: what an element is, and the two passes it goes through.
//!
//! The tree is rebuilt every frame. An element is a value the caller assembles,
//! measured once and painted once, then dropped; nothing of it survives to the
//! next frame. What does survive lives in the caller's own state and in the
//! shaping cache behind [`LayoutContext::measure`], which is why rebuilding is
//! cheap enough to do at the refresh rate.

use std::sync::Arc;

use pm_gfx::{DrawList, FontStyle, Point, Quad, Rect, Rgba, ShapedRun, Size, TextSystem};

use crate::style::Style;
use crate::theme::Theme;

/// What the pointer and keyboard are doing, as of the last event.
#[derive(Clone, Copy, Debug, Default)]
pub struct Input {
    /// Where the pointer is, if it is over the window at all.
    pub pointer: Option<Point>,
    /// Where the pointer went down, while it is still held.
    pub pressed_at: Option<Point>,
}

impl Input {
    /// Whether the pointer is inside `bounds`.
    pub fn is_over(&self, bounds: Rect) -> bool {
        self.pointer.is_some_and(|pointer| bounds.contains(pointer))
    }

    /// Whether the pointer went down inside `bounds` and is still held there.
    pub fn is_pressing(&self, bounds: Rect) -> bool {
        self.pressed_at
            .is_some_and(|pressed_at| bounds.contains(pressed_at))
            && self.is_over(bounds)
    }
}

/// A painted region that answers to the pointer and to the keyboard.
///
/// Regions are recorded in paint order, which is therefore also tab order, and
/// the last one containing a point is the one on top of it.
pub struct Region<M> {
    /// Where the region is.
    pub bounds: Rect,
    /// What it sends when it is clicked or activated.
    pub message: M,
}

/// What an interactive element needs to know to paint itself.
#[derive(Clone, Copy, Debug, Default)]
pub struct Interaction {
    /// The pointer is over the element.
    pub hovered: bool,
    /// The pointer is held down on the element.
    pub pressed: bool,
    /// The element holds keyboard focus.
    pub focused: bool,
}

/// Measurement: the theme to size against and the text system to measure with.
pub struct LayoutContext<'a> {
    /// The tokens this frame is drawn from.
    pub theme: &'a Theme,
    /// Shaping and measurement, cached across frames.
    text: &'a mut TextSystem,
}

impl<'a> LayoutContext<'a> {
    /// Creates a context measuring against `theme` with `text`.
    pub fn new(theme: &'a Theme, text: &'a mut TextSystem) -> Self {
        Self { theme, text }
    }

    /// Shapes `content` in `font`, reusing the cached run when there is one.
    pub fn shape(&mut self, content: &str, font: FontStyle) -> Arc<ShapedRun> {
        self.text.shape(content, font)
    }

    /// The extent `content` occupies in `font`.
    pub fn measure(&mut self, content: &str, font: FontStyle) -> Size {
        self.text.measure(content, font)
    }
}

/// Painting: everything measurement has, plus the draw list and the input.
pub struct PaintContext<'a, 'b, M> {
    /// Measurement, which painting needs as much as layout does.
    pub layout: LayoutContext<'b>,
    /// The list this frame's primitives go into.
    list: &'a mut DrawList,
    /// What the pointer is doing.
    input: Input,
    /// The region holding keyboard focus, as an index into `regions`.
    focused: Option<usize>,
    /// The regions painted so far this frame.
    regions: &'a mut Vec<Region<M>>,
}

impl<'a, 'b, M> PaintContext<'a, 'b, M> {
    /// Creates a paint context writing into `list` and `regions`.
    pub fn new(
        layout: LayoutContext<'b>,
        list: &'a mut DrawList,
        input: Input,
        focused: Option<usize>,
        regions: &'a mut Vec<Region<M>>,
    ) -> Self {
        Self {
            layout,
            list,
            input,
            focused,
            regions,
        }
    }

    /// The tokens this frame is drawn from.
    pub fn theme(&self) -> &Theme {
        self.layout.theme
    }

    /// Shapes `content` in `font`, reusing the cached run when there is one.
    pub fn shape(&mut self, content: &str, font: FontStyle) -> Arc<ShapedRun> {
        self.layout.shape(content, font)
    }

    /// The extent `content` occupies in `font`.
    pub fn measure(&mut self, content: &str, font: FontStyle) -> Size {
        self.layout.measure(content, font)
    }

    /// Adds a quad to the frame.
    pub fn quad(&mut self, quad: Quad) {
        self.list.quad(quad);
    }

    /// Draws a shaped run with its line box starting at `origin`.
    pub fn text(&mut self, origin: Point, run: Arc<ShapedRun>, color: Rgba) {
        self.list.text(origin, run, color);
    }

    /// Confines later primitives to `bounds` as well as the current clip.
    pub fn push_clip(&mut self, bounds: Rect) {
        self.list.push_clip(bounds);
    }

    /// Restores the clip in force before the matching [`Self::push_clip`].
    pub fn pop_clip(&mut self) {
        self.list.pop_clip();
    }

    /// The offset the pointer would be at, for an element that shifts content.
    pub fn input(&self) -> Input {
        self.input
    }

    /// Registers `bounds` as a click and tab target sending `message`.
    ///
    /// The returned state is what the element paints itself from: hover and
    /// press come from the current pointer position, focus from the tab index
    /// this registration takes.
    pub fn interactive(&mut self, bounds: Rect, message: M) -> Interaction {
        let index = self.regions.len();
        self.regions.push(Region { bounds, message });

        Interaction {
            hovered: self.input.is_over(bounds),
            pressed: self.input.is_pressing(bounds),
            focused: self.focused == Some(index),
        }
    }
}

/// One node of the tree: measured against an offer, then painted into bounds.
///
/// `M` is the caller's message type. An element never mutates the caller's
/// state; it registers interest through [`PaintContext::interactive`] and the
/// window turns a click or a keypress into one message the caller applies.
pub trait Element<M> {
    /// The style the parent lays this element out with.
    fn layout_style(&self) -> Style {
        Style::default()
    }

    /// The extent this element wants, given the extent it is offered.
    fn measure(&mut self, available: Size, cx: &mut LayoutContext<'_>) -> Size;

    /// Paints this element into `bounds`.
    fn paint(&mut self, bounds: Rect, cx: &mut PaintContext<'_, '_, M>);
}

/// Anything that can become a child of a container.
///
/// Elements convert to themselves; the trait exists so containers can accept a
/// boxed child, an element by value, or a widget that wraps one, all with the
/// same `child` call.
pub trait IntoElement<M> {
    /// Boxes this value as a child element.
    fn into_element(self) -> Box<dyn Element<M>>;
}

impl<M, E> IntoElement<M> for E
where
    E: Element<M> + 'static,
{
    /// Boxes the element itself.
    fn into_element(self) -> Box<dyn Element<M>> {
        Box::new(self)
    }
}

impl<M> Element<M> for Box<dyn Element<M>> {
    /// Defers to the boxed element.
    fn layout_style(&self) -> Style {
        (**self).layout_style()
    }

    /// Defers to the boxed element.
    fn measure(&mut self, available: Size, cx: &mut LayoutContext<'_>) -> Size {
        (**self).measure(available, cx)
    }

    /// Defers to the boxed element.
    fn paint(&mut self, bounds: Rect, cx: &mut PaintContext<'_, '_, M>) {
        (**self).paint(bounds, cx);
    }
}
