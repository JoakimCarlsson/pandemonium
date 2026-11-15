//! The strip beside a code editor's text: a minimap of the file in small
//! coloured bars with the stretch in view shaded, and marks along its edge
//! for the matches found, the changed lines, the breakpoints and the caret.

use crate::app_theme::ExtendedColors;
use std::cell::RefCell;
use std::ops::Range;
use std::rc::Rc;

use pm_editor::{Document, LineChange};
use rustle::prelude::*;
use rustle::{PointerButton, PointerPhase};

use crate::code_editor::Handle;
use crate::code_text::{column_of, kind_colour, runs};
use rustle::ColorScheme;

/// How wide the minimap is, marks included.
pub(crate) const OVERVIEW_WIDTH: f32 = 96.0;

/// How tall one line of the file is drawn in the minimap.
const PITCH: f32 = 2.0;

/// How wide one column of the file is drawn in the minimap.
const COLUMN: f32 = 0.7;

/// How wide the strip of marks on the minimap's right edge is.
const MARKS_WIDTH: f32 = 6.0;

/// What the marks along the minimap's edge show, by line counted from 0.
#[derive(Clone, Default)]
pub(crate) struct Marks {
    /// Lines with a match of the find bar's query.
    pub(crate) found: Vec<usize>,
    /// Lines with a breakpoint.
    pub(crate) breakpoints: Vec<usize>,
    /// The line the caret is on.
    pub(crate) caret: usize,
}

/// The first line of the file the minimap draws at its top, for a minimap
/// `height` tall over `count` lines whose first line in view is `top` with
/// `rows` in view: the whole file when it fits, otherwise a stretch that
/// moves with the view so the view's lines are always in it.
fn first_drawn(count: usize, top: f32, rows: f32, height: f32) -> f32 {
    let fits = height / PITCH;
    if count as f32 <= fits {
        return 0.0;
    }
    let travel = (count as f32 - rows).max(1.0);
    (top / travel).clamp(0.0, 1.0) * (count as f32 - fits)
}

/// The colour of the mark for a line changed as `change`.
fn change_colour(change: LineChange, scheme: &ColorScheme, extended: &ExtendedColors) -> Color {
    match change {
        LineChange::Added => extended.success,
        LineChange::Modified => scheme.primary,
        LineChange::DeletedBefore => scheme.error,
    }
}

/// Emits the minimap with its marks; a press or a drag on it scrolls the
/// text so the line under the pointer is in the middle of the view.
pub(crate) fn overview(
    handle: &Handle,
    marks: Marks,
    scheme: ColorScheme,
    extended: ExtendedColors,
) {
    let document = handle.document.clone();
    let list = handle.list.clone();
    let line_height = handle.line_height;
    let pointer = {
        let handle = handle.clone();
        move |event: &rustle::PointerEvent| {
            if matches!(event.phase, PointerPhase::Press | PointerPhase::Drag) {
                handle.scroll_minimap_to(event.position.y, PITCH, first_drawn);
            }
        }
    };
    canvas(
        Modifier::new()
            .width(OVERVIEW_WIDTH)
            .fill_max_height()
            .background(scheme.background)
            .on_pointer_button(PointerButton::Primary, pointer)
            .cursor(CursorIcon::Default),
        move |canvas| {
            let scroll = list.scroll();
            let height = canvas.height();
            let top = scroll.offset() / line_height;
            let rows = scroll.viewport_length() / line_height;
            paint(
                canvas,
                &document,
                &marks,
                (&scheme, &extended),
                (top, rows, height),
            );
        },
    );
}

/// Paints the minimap of `document` onto `canvas` with `marks`, given the
/// first line in view, the lines in view and the minimap's height in `view`,
/// in the Material and extended `colours`.
fn paint(
    canvas: &mut Canvas,
    document: &Rc<RefCell<Document>>,
    marks: &Marks,
    colours: (&ColorScheme, &ExtendedColors),
    view: (f32, f32, f32),
) {
    let (scheme, extended) = colours;
    let (top, rows, height) = view;
    let document = document.borrow();
    let count = document.lines().max(1);
    let first = first_drawn(count, top, rows, height);
    let start = first.floor() as usize;
    let end = (start + (height / PITCH).ceil() as usize + 1).min(document.lines());
    let shade = (top - first) * PITCH;
    canvas.rect(
        0.0,
        shade,
        OVERVIEW_WIDTH,
        (rows * PITCH).max(PITCH),
        Color {
            a: 26,
            ..scheme.on_surface
        },
    );
    let lines: Range<usize> = start..end.max(start);
    let highlights = document.highlights(lines.clone());
    for (offset, index) in lines.enumerate() {
        let y = (index as f32 - first) * PITCH;
        let range = document.line(index);
        let text = &document.field.text()[range];
        let spans = highlights.get(offset).cloned().unwrap_or_default();
        for (run, kind) in runs(text.len(), &spans) {
            let piece = &text[run.clone()];
            let trimmed = piece.trim_start();
            if trimmed.trim_end().is_empty() {
                continue;
            }
            let column = column_of(text, run.start + piece.len() - trimmed.len());
            let width = trimmed.trim_end().chars().count() as f32 * COLUMN;
            let colour = kind.map_or(scheme.on_surface_variant, |kind| {
                kind_colour(kind, scheme, extended)
            });
            canvas.rect(
                4.0 + column as f32 * COLUMN,
                y,
                width,
                PITCH * 0.7,
                Color { a: 170, ..colour },
            );
        }
    }
    canvas.rect(0.0, 0.0, 1.0, height, scheme.outline_variant);
    let along = |line: usize| (line as f32 / count as f32) * height;
    let left = OVERVIEW_WIDTH - MARKS_WIDTH;
    for line in 0..document.lines() {
        if let Some(change) = document.line_change(line) {
            canvas.rect(
                left,
                along(line),
                2.0,
                (height / count as f32).max(2.0),
                change_colour(change, scheme, extended),
            );
        }
    }
    for line in &marks.found {
        canvas.rect(
            left + 2.0,
            along(*line),
            MARKS_WIDTH - 2.0,
            2.0,
            extended.warning,
        );
    }
    for line in &marks.breakpoints {
        canvas.rect(
            left + 2.0,
            along(*line),
            MARKS_WIDTH - 2.0,
            3.0,
            scheme.error,
        );
    }
    canvas.rect(
        left,
        along(marks.caret),
        MARKS_WIDTH,
        2.0,
        scheme.on_surface,
    );
}

/// The colour of the bar beside a line number for a line changed as `change`.
pub(crate) fn gutter_colour(
    change: LineChange,
    scheme: &ColorScheme,
    extended: &ExtendedColors,
) -> Color {
    change_colour(change, scheme, extended)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A file that fits is drawn from its first line; a longer one moves
    /// from its first line to its last as the view does.
    #[test]
    fn drawn_stretch_follows_the_view() {
        assert_eq!(first_drawn(100, 50.0, 40.0, 400.0), 0.0);
        assert_eq!(first_drawn(1000, 0.0, 40.0, 400.0), 0.0);
        assert_eq!(first_drawn(1000, 960.0, 40.0, 400.0), 800.0);
    }
}
