//! The marks along the scrollbar of a file: where its problems, the lines changed since the last commit and the matches of a search are, each at its place in the whole file.

use std::ops::Range;

use pm_editor::{Document, EditorSettings, LineChange};
use rustle::Modifier;

use crate::theme::{Role, color};

use crate::viewer::severity_role;

/// How wide the strip of marks is, in logical pixels.
const STRIP_WIDTH: f32 = 6.0;

/// How tall one mark is, in logical pixels.
const MARK_HEIGHT: f32 = 3.0;

/// The most lines a file has for its changed lines to be marked, so a long file is not walked on every frame.
const MOST_LINES: usize = 50_000;

/// A mark of the strip: where it is as a share of the file's rows, and its colour.
type Placed = (f32, Role);

/// How far down the `rows` rows of `document` the row of the byte `at` is, as a share of them.
fn share(document: &Document, at: usize, rows: f32) -> f32 {
    document.row_of_line(document.line_of(at)) as f32 / rows
}

/// The marks of `document`'s problems, as bad as `settings` show, at the shares of its rows they are on.
fn problems(document: &Document, settings: &EditorSettings, rows: f32) -> Vec<Placed> {
    document
        .diagnostics
        .iter()
        .filter(|diagnostic| settings.diagnostics.shows(diagnostic.severity))
        .map(|diagnostic| {
            let at = document.range_of(diagnostic.span).start;
            (
                share(document, at, rows),
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
        .map(|range| (share(document, range.start, rows), Role::Warning))
        .collect()
}

/// Emits the strip of marks along the right edge of the view of `document`, with the matches `found` of a search, when `settings` want it and there is anything to mark.
///
/// The strip covers the layer it is emitted in, which the view sizes, and paints each mark at its share of that layer's height.
#[track_caller]
pub(crate) fn scroll_marks(document: &Document, found: &[Range<usize>], settings: &EditorSettings) {
    if !settings.scrollbar_marks {
        return;
    }
    let rows = document.rows().max(1) as f32;
    let mut placed = changes(document, rows);
    placed.extend(matches(document, found, rows));
    placed.extend(problems(document, settings, rows));
    if placed.is_empty() {
        return;
    }
    placed.dedup_by(|later, earlier| (later.0 - earlier.0).abs() < 0.002 && later.1 == earlier.1);
    let painted: Vec<(f32, rustle::Color)> = placed
        .into_iter()
        .map(|(share, tone)| (share.clamp(0.0, 1.0), color(tone)))
        .collect();
    rustle::canvas(Modifier::new().match_parent_size(), move |canvas| {
        let left = canvas.width() - STRIP_WIDTH;
        let height = canvas.height();
        for (share, paint) in &painted {
            canvas.rect(left, height * share, STRIP_WIDTH, MARK_HEIGHT, *paint);
        }
    });
}
