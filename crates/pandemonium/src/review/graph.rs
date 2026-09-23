//! The lanes of the Source Control graph, drawn as lines and dots.
//!
//! Each row of the graph paints its own slice of the lanes: the lines that
//! cross its upper and lower halves and the dot of its commit. A line that
//! changes lane runs straight down its own lane, bends through a quarter
//! circle and runs across to the commit it meets, so rows stacked on each
//! other join into one continuous picture without any row knowing about its
//! neighbours.

use pm_core::{Edge, Half, Lanes};
use pm_gfx::{Point, Quad, Rect, Rgba, Size};
use pm_ui::{Element, LayoutContext, PaintContext, Theme};

/// How far apart two lanes sit, centre to centre.
const LANE: f32 = 12.0;
/// How thick a lane's line is drawn.
const STROKE: f32 = 1.5;
/// The radius of a commit's dot.
const DOT: f32 = 4.0;

/// The terminal palette entries branches are coloured from, in the order
/// they are handed out.
const PALETTE: [usize; 6] = [12, 10, 13, 11, 14, 9];

/// One commit's slice of the graph.
pub struct GraphCell {
    /// How the row is drawn.
    lanes: Lanes,
    /// How many lanes wide the column is across every visible row.
    columns: usize,
    /// The height of one row.
    height: f32,
}

/// A slice of the graph for `lanes`, in a column `columns` lanes wide.
pub fn graph_cell(theme: &Theme, lanes: Lanes, columns: usize) -> GraphCell {
    GraphCell {
        lanes,
        columns,
        height: theme.size.row,
    }
}

/// The colour a branch with colour index `color` is drawn in.
pub fn lane_color(theme: &Theme, color: usize) -> Rgba {
    theme.terminal.ansi[PALETTE[color % PALETTE.len()]]
}

impl<M> Element<M> for GraphCell {
    /// As wide as the column's lanes and as tall as one row.
    fn measure(&mut self, _available: Size, _cx: &mut LayoutContext<'_>) -> Size {
        Size::new(self.columns.max(1) as f32 * LANE, self.height)
    }

    /// Paints every line crossing the row, then the commit's dot over them.
    fn paint(&mut self, bounds: Rect, cx: &mut PaintContext<'_, '_, M>) {
        let theme = *cx.theme();
        for edge in &self.lanes.edges {
            paint_edge(bounds, edge, lane_color(&theme, edge.color), cx);
        }
        let centre = Point::new(lane_x(bounds, self.lanes.lane), middle(bounds));
        cx.quad(
            Quad::filled(
                Rect::from_xywh(centre.x - DOT, centre.y - DOT, DOT * 2.0, DOT * 2.0),
                lane_color(&theme, self.lanes.color),
            )
            .corner_radius(DOT),
        );
    }
}

/// The x coordinate of the centre of `lane`.
fn lane_x(bounds: Rect, lane: usize) -> f32 {
    bounds.left() + LANE / 2.0 + lane as f32 * LANE
}

/// The y coordinate the row's commit sits at.
fn middle(bounds: Rect) -> f32 {
    bounds.top() + bounds.size.height / 2.0
}

/// Paints one line through half of the row.
///
/// An upper line bends where it reaches the commit's height and runs across
/// into the commit; a lower one leaves the commit across and bends down into
/// the lane it continues in.
fn paint_edge<M>(bounds: Rect, edge: &Edge, color: Rgba, cx: &mut PaintContext<'_, '_, M>) {
    let from = lane_x(bounds, edge.from);
    let to = lane_x(bounds, edge.to);
    let middle = middle(bounds);
    let (top, bottom) = match edge.half {
        Half::Upper => (bounds.top(), middle),
        Half::Lower => (middle, bounds.bottom()),
    };
    if edge.from == edge.to {
        vertical(from, top, bottom, color, cx);
        return;
    }
    let radius = (bottom - top).min((to - from).abs());
    match edge.half {
        Half::Upper => elbow(Point::new(from, middle), top, to, radius, color, cx),
        Half::Lower => elbow(Point::new(to, middle), bottom, from, radius, color, cx),
    }
}

/// Paints a line along `x` from `top` to `bottom`.
fn vertical<M>(x: f32, top: f32, bottom: f32, color: Rgba, cx: &mut PaintContext<'_, '_, M>) {
    cx.quad(Quad::filled(
        Rect::from_xywh(x - STROKE / 2.0, top, STROKE, (bottom - top).max(0.0)),
        color,
    ));
}

/// Paints a line along `y` from `left` to `right`.
fn horizontal<M>(y: f32, left: f32, right: f32, color: Rgba, cx: &mut PaintContext<'_, '_, M>) {
    cx.quad(Quad::filled(
        Rect::from_xywh(left, y - STROKE / 2.0, (right - left).max(0.0), STROKE),
        color,
    ));
}

/// Paints a bend at `corner` joining a vertical line that ends at `vertical_end`
/// to a horizontal one that ends at `horizontal_end`, rounded by `radius`.
///
/// The quarter circle is the matching corner of a ring, clipped to that one
/// quadrant, which keeps the bend a quad like every other stroke.
fn elbow<M>(
    corner: Point,
    vertical_end: f32,
    horizontal_end: f32,
    radius: f32,
    color: Rgba,
    cx: &mut PaintContext<'_, '_, M>,
) {
    let across = (horizontal_end - corner.x).signum();
    let along = (vertical_end - corner.y).signum();
    let centre = Point::new(corner.x + across * radius, corner.y + along * radius);

    vertical(
        corner.x,
        centre.y.min(vertical_end),
        centre.y.max(vertical_end),
        color,
        cx,
    );
    horizontal(
        corner.y,
        centre.x.min(horizontal_end),
        centre.x.max(horizontal_end),
        color,
        cx,
    );

    let outer = radius + STROKE / 2.0;
    let quadrant_x = centre.x - across * outer;
    let quadrant_y = centre.y - along * outer;
    cx.push_clip(Rect::from_xywh(
        centre.x.min(quadrant_x),
        centre.y.min(quadrant_y),
        outer,
        outer,
    ));
    cx.quad(
        Quad::filled(
            Rect::from_xywh(centre.x - outer, centre.y - outer, outer * 2.0, outer * 2.0),
            Rgba::TRANSPARENT,
        )
        .corner_radius(outer)
        .border(STROKE, color),
    );
    cx.pop_clip();
}
