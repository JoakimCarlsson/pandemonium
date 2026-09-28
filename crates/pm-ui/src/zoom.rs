//! How far a picture is zoomed into and moved about, and the area that shows it.
//!
//! A zoomed picture keeps the room it had at rest: the area is as large as
//! the picture fitted to what it is offered, and zooming magnifies the
//! picture inside it, clipped, rather than growing the area and pushing the
//! screen around it. One of these belongs to each picture that zooms, shared
//! with the caller the way a [`Scrolled`](crate::Scrolled) is, so a wheel
//! turned or a button pressed can move it before the next frame is drawn.

use std::cell::Cell;
use std::rc::Rc;

use pm_gfx::{Image, Point, Rect, Size};

use crate::element::{Element, LayoutContext, PaintContext};
use crate::resize::{ResizeEvent, ResizePhase};
use crate::style::{Style, Styled};

/// The most a picture is magnified, against its size at rest.
pub const MAX_ZOOM: f32 = 8.0;

/// The zoom and pan of one picture, and the bounds it was last painted in.
#[derive(Clone, Copy, Debug)]
pub struct Zoom {
    /// How many times larger than at rest the picture is drawn, never below one.
    factor: f32,
    /// Where the magnified picture's top-left corner sits against the area's,
    /// never right of it nor below it.
    offset: Point,
    /// Where the area was last painted, in logical pixels.
    bounds: Rect,
    /// The offset the pan under way started from.
    pan_from: Option<Point>,
}

impl Default for Zoom {
    /// At rest: the whole picture, fitted.
    fn default() -> Self {
        Self {
            factor: 1.0,
            offset: Point::new(0.0, 0.0),
            bounds: Rect::from_xywh(0.0, 0.0, 0.0, 0.0),
            pan_from: None,
        }
    }
}

impl Zoom {
    /// How many times larger than at rest the picture is drawn.
    pub fn factor(&self) -> f32 {
        self.factor
    }

    /// Whether the picture is magnified at all.
    pub fn is_zoomed(&self) -> bool {
        self.factor > 1.0
    }

    /// Whether `point` fell inside the area the last time it was painted.
    pub fn contains(&self, point: Point) -> bool {
        self.bounds.contains(point)
    }

    /// Multiplies the zoom by `multiplier`, keeping the part of the picture
    /// under `around` where it is.
    pub fn zoom_at(&mut self, multiplier: f32, around: Point) {
        let factor = (self.factor * multiplier).clamp(1.0, MAX_ZOOM);
        let (x, y) = (around.x - self.bounds.left(), around.y - self.bounds.top());
        let grown = factor / self.factor;
        self.offset = Point::new(
            x - (x - self.offset.x) * grown,
            y - (y - self.offset.y) * grown,
        );
        self.factor = factor;
        self.clamp();
    }

    /// Multiplies the zoom by `multiplier`, keeping the middle of the area
    /// where it is.
    pub fn zoom_centred(&mut self, multiplier: f32) {
        let middle = Point::new(
            self.bounds.left() + self.bounds.size.width / 2.0,
            self.bounds.top() + self.bounds.size.height / 2.0,
        );
        self.zoom_at(multiplier, middle);
    }

    /// Returns the picture to rest, whole and fitted.
    pub fn reset(&mut self) {
        self.factor = 1.0;
        self.offset = Point::new(0.0, 0.0);
        self.pan_from = None;
    }

    /// Moves the magnified picture along with the pointer dragging it.
    pub fn pan(&mut self, event: ResizeEvent) {
        if event.phase == ResizePhase::Started {
            self.pan_from = Some(self.offset);
        }
        let Some(from) = self.pan_from else {
            return;
        };
        self.offset = Point::new(
            from.x + event.current.x - event.start.x,
            from.y + event.current.y - event.start.y,
        );
        self.clamp();
        if event.phase == ResizePhase::Ended {
            self.pan_from = None;
        }
    }

    /// Records where the area was painted, holding the offset against it.
    fn set_bounds(&mut self, bounds: Rect) {
        self.bounds = bounds;
        self.clamp();
    }

    /// Holds the magnified picture over the whole of the area.
    fn clamp(&mut self) {
        let spare = |extent: f32| extent * (1.0 - self.factor);
        self.offset = Point::new(
            self.offset.x.clamp(spare(self.bounds.size.width), 0.0),
            self.offset.y.clamp(spare(self.bounds.size.height), 0.0),
        );
    }
}

/// The zoom of one picture, shared between the caller and the area drawing it.
pub type Zoomed = Rc<Cell<Zoom>>;

/// A picture drawn at its [`Zoom`], in the room it takes at rest.
pub struct ZoomArea {
    /// The zoom the picture is drawn at, and records its bounds in.
    zoomed: Zoomed,
    /// The picture, at whatever resolution the caller has for this zoom.
    image: Image,
    /// The size the picture is at rest, in logical pixels, however many
    /// pixels `image` holds.
    natural: Size,
    /// How the area is sized in its parent.
    style: Style,
}

/// `image`, shown `natural` logical pixels large at rest and zoomed by `zoomed`.
pub fn zoom_area(zoomed: Zoomed, image: Image, natural: Size) -> ZoomArea {
    ZoomArea {
        zoomed,
        image,
        natural,
        style: Style::default(),
    }
}

impl Styled for ZoomArea {
    /// How the area is sized in its parent.
    fn style(&mut self) -> &mut Style {
        &mut self.style
    }
}

impl<M> Element<M> for ZoomArea {
    /// How the area is sized in its parent.
    fn layout_style(&self) -> Style {
        self.style
    }

    /// The picture at rest, narrowed to fit the width it is offered.
    fn measure(&mut self, available: Size, _cx: &mut LayoutContext<'_>) -> Size {
        let width = self.natural.width.max(1.0);
        let fit = (available.width / width).clamp(0.0, 1.0);
        Size::new(self.natural.width * fit, self.natural.height * fit)
    }

    /// Paints the picture magnified and moved by the zoom, clipped to the area.
    fn paint(&mut self, bounds: Rect, cx: &mut PaintContext<'_, '_, M>) {
        let mut zoom = self.zoomed.get();
        zoom.set_bounds(bounds);
        self.zoomed.set(zoom);
        cx.push_clip(bounds);
        cx.image(
            Rect::from_xywh(
                bounds.left() + zoom.offset.x,
                bounds.top() + zoom.offset.y,
                bounds.size.width * zoom.factor,
                bounds.size.height * zoom.factor,
            ),
            self.image.clone(),
        );
        cx.pop_clip();
    }
}
