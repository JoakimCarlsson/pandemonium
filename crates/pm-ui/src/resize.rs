//! Resizable panel state and the sash that drives it.

use std::sync::Arc;

use pm_gfx::{Point, Quad, Rect, Size};

use crate::{Axis, Element, Interaction, LayoutContext, PaintContext, Style};

/// Logical pixels occupied by and accepting input on the sash.
const SASH_SIZE: f32 = 12.0;

/// Thickness of the line through the middle of the sash.
const LINE_SIZE: f32 = 1.0;

/// Length of the grip along the sash.
const GRIP_LENGTH: f32 = 16.0;

/// Width of the compact grip across the sash.
const GRIP_WIDTH: f32 = 12.0;

/// Diameter of each grip dot.
const DOT_SIZE: f32 = 1.5;

/// The stage of a resize gesture.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResizePhase {
    /// The pointer was pressed on the sash.
    Started,
    /// The captured pointer moved.
    Moved,
    /// The captured pointer was released.
    Ended,
}

/// One pointer event in a captured resize gesture.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ResizeEvent {
    /// The stage of the gesture.
    pub phase: ResizePhase,
    /// Where the gesture began.
    pub start: Point,
    /// Where the pointer is now.
    pub current: Point,
}

impl ResizeEvent {
    /// Returns pointer travel along `axis` since the gesture began.
    pub fn delta(self, axis: Axis) -> f32 {
        match axis {
            Axis::Horizontal => self.current.x - self.start.x,
            Axis::Vertical => self.current.y - self.start.y,
        }
    }
}

/// Which edge of a panel its sash moves.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResizeEdge {
    /// The sash moves the panel's left or top edge.
    Start,
    /// The sash moves the panel's right or bottom edge.
    End,
}

/// Persistent size and constraints for one resizable panel.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ResizeState {
    /// The panel's current extent along its resize axis.
    extent: f32,
    /// The smallest permitted extent.
    min: f32,
    /// The largest permitted extent.
    max: f32,
    /// The extent captured when the current gesture began.
    drag_extent: Option<f32>,
}

impl Default for ResizeState {
    /// Creates a practical sidebar-sized panel with broad constraints.
    fn default() -> Self {
        Self::new(240.0, 120.0, 800.0)
    }
}

impl ResizeState {
    /// Creates panel state clamped between `min` and `max`.
    pub fn new(extent: f32, min: f32, max: f32) -> Self {
        let min = min.max(0.0).min(max.max(0.0));
        let max = max.max(min);
        Self {
            extent: extent.clamp(min, max),
            min,
            max,
            drag_extent: None,
        }
    }

    /// Returns the panel's current extent.
    pub fn extent(self) -> f32 {
        self.extent
    }

    /// Applies `event` from the sash on `edge` along `axis`.
    pub fn resize(&mut self, event: ResizeEvent, axis: Axis, edge: ResizeEdge) {
        if event.phase == ResizePhase::Started {
            self.drag_extent = Some(self.extent);
        }
        let Some(start_extent) = self.drag_extent else {
            return;
        };
        let direction = match edge {
            ResizeEdge::Start => -1.0,
            ResizeEdge::End => 1.0,
        };
        self.extent = (start_extent + event.delta(axis) * direction).clamp(self.min, self.max);
        if event.phase == ResizePhase::Ended {
            self.drag_extent = None;
        }
    }
}

/// A thin divider with a forgiving hit area that captures pointer drags.
pub struct Sash<M> {
    /// The dimension changed by dragging the sash.
    axis: Axis,
    /// Builds the caller's message for each resize event.
    on_resize: Arc<dyn Fn(ResizeEvent) -> M>,
}

/// Creates a sash changing the extent along `axis`.
pub fn sash<M>(axis: Axis, on_resize: impl Fn(ResizeEvent) -> M + 'static) -> Sash<M> {
    Sash {
        axis,
        on_resize: Arc::new(on_resize),
    }
}

impl<M> Sash<M> {
    /// Returns the one-pixel line centred inside `bounds`.
    fn line_bounds(&self, bounds: Rect) -> Rect {
        match self.axis {
            Axis::Horizontal => Rect::from_xywh(
                bounds.left() + (bounds.size.width - LINE_SIZE) / 2.0,
                bounds.top(),
                LINE_SIZE,
                bounds.size.height,
            ),
            Axis::Vertical => Rect::from_xywh(
                bounds.left(),
                bounds.top() + (bounds.size.height - LINE_SIZE) / 2.0,
                bounds.size.width,
                LINE_SIZE,
            ),
        }
    }

    /// Returns the centred grip background inside `bounds`.
    fn grip_bounds(&self, bounds: Rect) -> Rect {
        match self.axis {
            Axis::Horizontal => Rect::from_xywh(
                bounds.left() + (bounds.size.width - GRIP_WIDTH) / 2.0,
                bounds.top() + (bounds.size.height - GRIP_LENGTH) / 2.0,
                GRIP_WIDTH,
                GRIP_LENGTH,
            ),
            Axis::Vertical => Rect::from_xywh(
                bounds.left() + (bounds.size.width - GRIP_LENGTH) / 2.0,
                bounds.top() + (bounds.size.height - GRIP_WIDTH) / 2.0,
                GRIP_LENGTH,
                GRIP_WIDTH,
            ),
        }
    }

    /// Chooses the visible divider colour for `interaction`.
    fn line_color<M2>(interaction: Interaction, cx: &PaintContext<'_, '_, M2>) -> pm_gfx::Rgba {
        if interaction.pressed || interaction.hovered {
            cx.theme().colors.border_focused
        } else {
            cx.theme().colors.border
        }
    }

    /// Paints the shadcn-style six-dot grip centered on the sash.
    fn paint_grip(&self, bounds: Rect, interaction: Interaction, cx: &mut PaintContext<'_, '_, M>) {
        let grip = self.grip_bounds(bounds);
        let fill = Self::line_color(interaction, cx);
        cx.quad(Quad::filled(grip, fill).corner_radius(cx.theme().radius.sm));
        for primary in [-3.0, 0.0, 3.0] {
            for secondary in [-2.0, 2.0] {
                let dot = match self.axis {
                    Axis::Horizontal => Rect::from_xywh(
                        grip.left() + grip.size.width / 2.0 + secondary - DOT_SIZE / 2.0,
                        grip.top() + grip.size.height / 2.0 + primary - DOT_SIZE / 2.0,
                        DOT_SIZE,
                        DOT_SIZE,
                    ),
                    Axis::Vertical => Rect::from_xywh(
                        grip.left() + grip.size.width / 2.0 + primary - DOT_SIZE / 2.0,
                        grip.top() + grip.size.height / 2.0 + secondary - DOT_SIZE / 2.0,
                        DOT_SIZE,
                        DOT_SIZE,
                    ),
                };
                cx.quad(Quad::filled(dot, cx.theme().colors.surface).corner_radius(DOT_SIZE / 2.0));
            }
        }
    }
}

impl<M> Element<M> for Sash<M> {
    /// Sizes the sash to contain its grip while keeping the divider itself thin.
    fn layout_style(&self) -> Style {
        let mut style = Style::default();
        match self.axis {
            Axis::Horizontal => {
                style.width = crate::Length::Px(SASH_SIZE);
                style.height = crate::Length::Full;
            }
            Axis::Vertical => {
                style.width = crate::Length::Full;
                style.height = crate::Length::Px(SASH_SIZE);
            }
        }
        style
    }

    /// Takes the offered cross-axis space and the grip width on the resize axis.
    fn measure(&mut self, available: Size, _cx: &mut LayoutContext<'_>) -> Size {
        match self.axis {
            Axis::Horizontal => Size::new(SASH_SIZE, available.height),
            Axis::Vertical => Size::new(available.width, SASH_SIZE),
        }
    }

    /// Registers the hit area and paints the divider.
    fn paint(&mut self, bounds: Rect, cx: &mut PaintContext<'_, '_, M>) {
        let interaction = cx.resizable(bounds, self.axis, self.on_resize.clone());
        cx.quad(Quad::filled(bounds, cx.theme().colors.surface));
        cx.quad(Quad::filled(
            self.line_bounds(bounds),
            Self::line_color(interaction, cx),
        ));
        self.paint_grip(bounds, interaction, cx);
    }
}
