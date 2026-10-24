//! The marks along the scrollbar of a file: where its problems, the lines changed since the last commit and the matches of a search are, each at its place in the whole file.

use std::ops::Range;

use pm_editor::{Document, EditorSettings, LineChange};
use pm_style::{
    Length, Role, Sides, absolute, bg, flex_none, h_px, inset_length, right, top, w_px,
};
use pm_ui::{Node, node};

use crate::viewer::severity_role;

/// How wide the strip of marks is, in logical pixels.
const STRIP_WIDTH: f32 = 6.0;

/// How tall one mark is, in logical pixels.
const MARK_HEIGHT: f32 = 3.0;

/// The most lines a file has for its changed lines to be marked, so a long file is not walked on every frame.
const MOST_LINES: usize = 50_000;

/// A mark of the strip: where it is as a share of the file's rows, and its colour.
type Placed = (f32, Role);

/// The marks of `document`'s problems, as bad as `settings` show, at the shares of its rows they are on.
fn problems(document: &Document, settings: &EditorSettings, rows: f32) -> Vec<Placed> {
    document
        .diagnostics
        .iter()
        .filter(|diagnostic| settings.diagnostics.shows(diagnostic.severity))
        .map(|diagnostic| {
            let line = document.line_of(document.range_of(diagnostic.span).start);
            (
                document.row_of_line(line) as f32 / rows,
                severity_role(diagnostic.severity),
            )
        })
        .collect()
}

/// The marks of the lines of `document` changed since the last commit, at the shares of its rows they are on.
fn changes(document: &Document, rows: f32) -> Vec<Placed> {
    if document.lines() > MOST_LINES {
        return Vec::new();
    }
    (0..document.lines())
        .filter_map(|line| {
            let tone = match document.line_change(line)? {
                LineChange::Added => Role::Success,
                LineChange::Modified => Role::Accent,
                LineChange::DeletedBefore => Role::Danger,
            };
            Some((document.row_of_line(line) as f32 / rows, tone))
        })
        .collect()
}

/// The marks of the matches `found` of a search in `document`, at the shares of its rows they are on.
fn matches(document: &Document, found: &[Range<usize>], rows: f32) -> Vec<Placed> {
    found
        .iter()
        .map(|range| {
            let line = document.line_of(range.start);
            (document.row_of_line(line) as f32 / rows, Role::Warning)
        })
        .collect()
}

/// The strip of marks along the right edge of the view of `document`, with the matches `found` of a search, when `settings` want it and there is anything to mark.
pub(crate) fn scroll_marks(
    document: &Document,
    found: &[Range<usize>],
    settings: &EditorSettings,
) -> Option<Node> {
    if !settings.scrollbar_marks {
        return None;
    }
    let rows = document.rows().max(1) as f32;
    let mut placed = changes(document, rows);
    placed.extend(matches(document, found, rows));
    placed.extend(problems(document, settings, rows));
    if placed.is_empty() {
        return None;
    }
    placed.dedup_by(|later, earlier| (later.0 - earlier.0).abs() < 0.002 && later.1 == earlier.1);
    Some(
        node()
            .styles([absolute(), top(0), right(0), w_px(STRIP_WIDTH), flex_none()])
            .styles([inset_length(Sides::Bottom, Length::Px(0.0))])
            .children(placed.into_iter().map(|(share, tone)| {
                node().styles([
                    absolute(),
                    right(0),
                    inset_length(Sides::Top, Length::Fraction(share.clamp(0.0, 1.0))),
                    w_px(STRIP_WIDTH),
                    h_px(MARK_HEIGHT),
                    bg(tone),
                ])
            })),
    )
}
