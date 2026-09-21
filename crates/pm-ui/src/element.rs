//! The element tree: what an element is, and the two passes it goes through.
//!
//! The tree is rebuilt every frame. An element is a value the caller assembles,
//! measured once and painted once, then dropped; nothing of it survives to the
//! next frame. What does survive lives in the caller's own state and in the
//! shaping cache behind [`LayoutContext::measure`], which is why rebuilding is
//! cheap enough to do at the refresh rate.

use std::sync::Arc;

use pm_gfx::{DrawList, FontStyle, Point, Quad, Rect, Rgba, ShapedRun, Size, Svg, TextSystem};

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
    pub action: RegionAction<M>,
    /// What it sends when it is clicked with the secondary button.
    pub secondary: Option<M>,
}

/// What an interactive region does with pointer input.
pub enum RegionAction<M> {
    /// Answers to the pointer without sending anything.
    Inert,
    /// Sends one message when a press and release both land in the region.
    Click(M),
    /// Sends messages throughout a captured pointer drag.
    Drag {
        /// The shape the pointer takes over the region.
        cursor: crate::PointerCursor,
        /// Builds the caller's message for each captured pointer event.
        handler: Arc<dyn Fn(crate::resize::ResizeEvent) -> M>,
    },
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

    /// Draws `svg` inside `bounds`, tinted `color`.
    pub fn icon(&mut self, bounds: Rect, svg: Svg, color: Rgba) {
        self.list.icon(bounds, svg, color);
    }

    /// Confines later primitives to `bounds` as well as the current clip.
    pub fn push_clip(&mut self, bounds: Rect) {
        self.list.push_clip(bounds);
    }

    /// Restores the clip in force before the matching [`Self::push_clip`].
    pub fn pop_clip(&mut self) {
        self.list.pop_clip();
    }

    /// Draws later primitives over everything drawn so far.
    pub fn push_layer(&mut self) {
        self.list.push_layer();
    }

    /// Returns to the layer in force before the matching [`Self::push_layer`].
    pub fn pop_layer(&mut self) {
        self.list.pop_layer();
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
        self.clickable(bounds, Some(message), None)
    }

    /// Registers `bounds` as a target for either mouse button.
    ///
    /// A region with only a secondary message is still a region: a tab that
    /// opens a menu on the right button and does nothing on the left is a
    /// thing the pointer can be over.
    pub fn clickable(
        &mut self,
        bounds: Rect,
        on_click: Option<M>,
        on_secondary: Option<M>,
    ) -> Interaction {
        let index = self.regions.len();
        self.regions.push(Region {
            bounds,
            action: match on_click {
                Some(message) => RegionAction::Click(message),
                None => RegionAction::Inert,
            },
            secondary: on_secondary,
        });

        Interaction {
            hovered: self.input.is_over(bounds),
            pressed: self.input.is_pressing(bounds),
            focused: self.focused == Some(index),
        }
    }

    /// The window this frame is being drawn for.
    pub fn viewport(&self) -> Rect {
        self.list.viewport()
    }

    /// Registers `bounds` as an edge dragged along `axis` by `on_resize`.
    pub fn resizable(
        &mut self,
        bounds: Rect,
        axis: crate::Axis,
        on_resize: Arc<dyn Fn(crate::resize::ResizeEvent) -> M>,
    ) -> Interaction {
        let cursor = match axis {
            crate::Axis::Horizontal => crate::PointerCursor::ResizeHorizontal,
            crate::Axis::Vertical => crate::PointerCursor::ResizeVertical,
        };
        self.draggable(bounds, cursor, on_resize)
    }

    /// Registers `bounds` as a pointer-drag target handled by `on_drag`.
    ///
    /// A drag is how a region hears where the pointer is rather than only
    /// that it was clicked, so a pane that places a cursor and a sash that
    /// resizes a sidebar are the same kind of region, under two shapes of
    /// pointer.
    pub fn draggable(
        &mut self,
        bounds: Rect,
        cursor: crate::PointerCursor,
        on_drag: Arc<dyn Fn(crate::resize::ResizeEvent) -> M>,
    ) -> Interaction {
        let index = self.regions.len();
        self.regions.push(Region {
            bounds,
            action: RegionAction::Drag {
                cursor,
                handler: on_drag,
            },
            secondary: None,
        });

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
