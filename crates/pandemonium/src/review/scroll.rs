//! Scrolling and pointer geometry for diff rows that wrap to several screen lines.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use pm_gfx::{Rect, Size};
use pm_ui::{Element, IntoElement, LayoutContext, PaintContext, Style, Styled, v_flex};

use crate::message::Message;

/// The position and measured geometry of one diff pane.
#[derive(Clone, Default)]
pub(crate) struct DiffScroll {
    /// Measurements shared with the elements built for this pane.
    state: Arc<Mutex<State>>,
}

/// A logical row position with an offset into its wrapped screen lines.
#[derive(Default)]
struct State {
    /// The first logical row to build.
    row: usize,
    /// Pixels hidden above the viewport within the first row.
    offset: f32,
    /// Height of a single screen line.
    line_height: f32,
    /// Width used for the cached row heights.
    width: f32,
    /// Measured heights of logical rows.
    heights: BTreeMap<usize, f32>,
    /// Logical row bounds from the last painted frame.
    bounds: BTreeMap<usize, Rect>,
}

impl DiffScroll {
    /// Returns the first logical row displayed.
    pub(crate) fn row(&self) -> usize {
        self.state.lock().unwrap().row
    }

    /// Moves directly to the beginning of a logical row.
    pub(crate) fn to(&self, row: usize) {
        let mut state = self.state.lock().unwrap();
        state.row = row;
        state.offset = 0.0;
    }

    /// Moves by screen lines, allowing every continuation of a long row to be read.
    pub(crate) fn by(&self, lines: isize, total: usize) {
        let mut state = self.state.lock().unwrap();
        let line_height = state.line_height.max(1.0);
        state.row = state.row.min(total.saturating_sub(1));
        let mut offset = state.offset + lines as f32 * line_height;
        while offset < 0.0 && state.row > 0 {
            state.row -= 1;
            offset += state
                .heights
                .get(&state.row)
                .copied()
                .unwrap_or(line_height)
                .max(1.0);
        }
        while state.row + 1 < total {
            let height = state
                .heights
                .get(&state.row)
                .copied()
                .unwrap_or(line_height)
                .max(1.0);
            if offset < height {
                break;
            }
            offset -= height;
            state.row += 1;
        }
        let height = state
            .heights
            .get(&state.row)
            .copied()
            .unwrap_or(line_height);
        state.offset = offset.clamp(0.0, (height - line_height).max(0.0));
    }

    /// Returns the painted logical row at a vertical pointer position.
    pub(super) fn at(&self, y: f32) -> Option<usize> {
        let state = self.state.lock().unwrap();
        state
            .bounds
            .iter()
            .find(|(_, bounds)| y < bounds.bottom())
            .map(|(row, _)| *row)
            .or_else(|| state.bounds.last_key_value().map(|(row, _)| *row))
    }

    /// Wraps one logical row to record its measured height and painted bounds.
    pub(super) fn measured(&self, row: usize, child: impl IntoElement<Message>) -> DiffRow {
        DiffRow {
            scroll: self.clone(),
            row,
            child: child.into_element(),
        }
    }

    /// Clips visible logical rows and shifts the first one's hidden continuation.
    pub(super) fn viewport(&self, child: impl IntoElement<Message>) -> DiffViewport {
        DiffViewport {
            scroll: self.clone(),
            child: child.into_element(),
        }
    }
}

/// A row whose height and bounds follow the actual wrapping layout.
pub(super) struct DiffRow {
    /// The pane's scroll and geometry.
    scroll: DiffScroll,
    /// The logical row this element represents.
    row: usize,
    /// The row's content and gutter.
    child: Box<dyn Element<Message>>,
}

impl Element<Message> for DiffRow {
    /// Keeps the row's own layout settings.
    fn layout_style(&self) -> Style {
        self.child.layout_style()
    }

    /// Records the height after the text has wrapped to the offered width.
    fn measure(&mut self, available: Size, cx: &mut LayoutContext<'_>) -> Size {
        let size = self.child.measure(available, cx);
        self.scroll
            .state
            .lock()
            .unwrap()
            .heights
            .insert(self.row, size.height);
        size
    }

    /// Records actual bounds for comment gestures before drawing the row.
    fn paint(&mut self, bounds: Rect, cx: &mut PaintContext<'_, '_, Message>) {
        self.scroll
            .state
            .lock()
            .unwrap()
            .bounds
            .insert(self.row, bounds);
        self.child.paint(bounds, cx);
    }
}

/// A clipped pane containing a bounded set of logical rows.
pub(super) struct DiffViewport {
    /// The first row and its continuation offset.
    scroll: DiffScroll,
    /// The rows built for the current position.
    child: Box<dyn Element<Message>>,
}

impl Element<Message> for DiffViewport {
    /// Fills the remaining reading area of the diff pane.
    fn layout_style(&self) -> Style {
        v_flex::<Message>().w_full().flex_1().layout_style()
    }

    /// Accepts the viewport while invalidating heights when its width changes.
    fn measure(&mut self, available: Size, cx: &mut LayoutContext<'_>) -> Size {
        let mut state = self.scroll.state.lock().unwrap();
        if state.width != available.width || state.line_height != cx.theme.size.row {
            state.heights.clear();
        }
        state.width = available.width;
        state.line_height = cx.theme.size.row;
        available
    }

    /// Paints wrapped rows at the continuation offset, clipped to the reading area.
    fn paint(&mut self, bounds: Rect, cx: &mut PaintContext<'_, '_, Message>) {
        let content = self.child.measure(bounds.size, &mut cx.layout);
        let offset = {
            let mut state = self.scroll.state.lock().unwrap();
            state.bounds.clear();
            let height = state
                .heights
                .get(&state.row)
                .copied()
                .unwrap_or(state.line_height);
            state.offset = state.offset.min((height - state.line_height).max(0.0));
            state.offset
        };
        let first = cx.region_count();
        cx.push_clip(bounds);
        self.child.paint(
            Rect::from_xywh(
                bounds.left(),
                bounds.top() - offset,
                bounds.size.width,
                content.height,
            ),
            cx,
        );
        cx.pop_clip();
        cx.clip_regions(first, bounds);
    }
}
