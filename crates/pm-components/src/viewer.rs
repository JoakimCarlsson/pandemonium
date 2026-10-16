//! A file open in the window: its lines, numbered, as they are in the folder or in the last commit, edited in place.

use std::ops::Range;

use pm_protocol::Severity;
use pm_style::{
    Role, bg, border_b, border_color, cursor_text, flex, flex_1, flex_col, flex_none, font_mono,
    gap, h, items_center, justify_end, leading_ratio, min_h, min_w_px, overflow_hidden, pr, px, py,
    text_color, text_px, w,
};
use pm_treesitter::Kind;
use pm_ui::{Mark, Node, Paint, node, text};

use crate::Button;
use crate::button::{Size, Variant};
use crate::file_icon::file_icon;
use crate::git_controls::VIEWER_SCROLL;
use crate::open_files::{Contents, EDITOR_FIELD, EDITOR_SAVE, ROW, STRIP, Viewer};
use crate::parts::{line, spacer};
use crate::state::WorkspaceState;
use pm_editor::Document;

/// How far beyond the view, above and below, lines are still built, in logical pixels.
const OVERSCAN: f32 = 100.0;

/// A box as tall as `lines` lines, standing in for those left out.
fn blank(lines: usize) -> Node {
    node().styles([h(lines as f32 * ROW / 4.0), flex_none()])
}

/// The part of `range` that falls on the line at `line`, as offsets into the line, or nothing when none of it does.
fn clipped(range: &Option<Range<usize>>, line: &Range<usize>) -> Option<Range<usize>> {
    let range = range.as_ref()?;
    let (low, high) = (range.start.max(line.start), range.end.min(line.end));
    (low < high).then(|| low - line.start..high - line.start)
}

/// The colour a highlighted stretch of `kind` is drawn in, or nothing for plain text.
pub(crate) fn kind_role(kind: Kind) -> Option<Role> {
    match kind {
        Kind::Keyword | Kind::Markup => Some(Role::SyntaxKeyword),
        Kind::Function | Kind::Link => Some(Role::SyntaxFunction),
        Kind::Type => Some(Role::SyntaxType),
        Kind::String => Some(Role::SyntaxString),
        Kind::Constant => Some(Role::SyntaxConstant),
        Kind::Comment => Some(Role::SyntaxComment),
        Kind::Property => Some(Role::SyntaxProperty),
        Kind::Operator | Kind::Punctuation => Some(Role::SyntaxPunctuation),
        Kind::Variable => None,
    }
}

/// The colour a diagnostic of `severity` is drawn in.
pub(crate) fn severity_role(severity: Severity) -> Role {
    match severity {
        Severity::Error => Role::Danger,
        Severity::Warning => Role::Warning,
        Severity::Information | Severity::Hint => Role::Muted,
    }
}

/// The marks of line `index` of `document`: its highlighted stretches in their colours, the brackets `pair` paired at the caret behind a box, and what the language server finds wrong under a wavy line.
fn line_marks(
    document: &Document,
    index: usize,
    spans: Vec<(Range<usize>, Kind)>,
    pair: &[Range<usize>],
) -> Vec<Mark> {
    let line = document.line(index);
    let mut marks: Vec<Mark> = spans
        .into_iter()
        .filter_map(|(range, kind)| {
            kind_role(kind).map(|role| Mark {
                range,
                color: role.into(),
                paint: Paint::Ink,
            })
        })
        .collect();
    marks.extend(pair.iter().filter_map(|bracket| {
        clipped(&Some(bracket.clone()), &line).map(|range| Mark {
            range,
            color: Role::SurfaceActive.into(),
            paint: Paint::Fill,
        })
    }));
    marks.extend(clipped(&document.reach, &line).map(|range| Mark {
        range,
        color: Role::SyntaxFunction.into(),
        paint: Paint::Underline,
    }));
    marks.extend(
        document
            .diagnostics_on(index)
            .into_iter()
            .map(|(range, diagnostic)| Mark {
                range,
                color: severity_role(diagnostic.severity).into(),
                paint: Paint::Wave,
            }),
    );
    marks
}

/// One numbered line of the file, with the caret and selection placed in it, coloured by `marks`.
fn file_line(document: &Document, index: usize, marks: Vec<Mark>) -> Node {
    let line = document.line(index);
    let text_of = &document.field.text()[line.clone()];
    let caret = document.field.caret();
    let worst = document
        .diagnostics_on(index)
        .iter()
        .map(|(_, diagnostic)| diagnostic.severity)
        .min();
    let mut content = text(text_of)
        .styles([
            text_px(12),
            leading_ratio(1.4),
            font_mono(),
            text_color(Role::Text),
            flex_none(),
        ])
        .field_line(EDITOR_FIELD, line.start)
        .marks(marks);
    if (line.start..=line.end).contains(&caret) {
        content = content.caret(caret - line.start, EDITOR_FIELD);
    }
    if let Some(range) = clipped(&document.field.selection(), &line) {
        content = content.selection(range, EDITOR_FIELD);
    }
    if let Some(range) = clipped(&document.field.marked(), &line) {
        content = content.marked(range, EDITOR_FIELD);
    }
    node()
        .styles([flex(), h(5), flex_none(), overflow_hidden()])
        .child(
            node()
                .styles([
                    flex(),
                    items_center(),
                    justify_end(),
                    flex_none(),
                    w(14),
                    pr(3),
                ])
                .child(text((index + 1).to_string()).styles([
                    text_px(12),
                    leading_ratio(1.4),
                    font_mono(),
                    text_color(worst.map_or(Role::Subtle, severity_role)),
                ])),
        )
        .child(
            node()
                .styles([flex_1(), min_w_px(0), overflow_hidden(), px(2)])
                .child(content),
        )
}

/// The lines of `document` that are in or near a view `height` tall scrolled `scroll` down, with a blank for those above and below.
fn visible(document: &Document, scroll: f32, height: f32) -> Vec<Node> {
    let total = document.lines();
    let first = (((scroll - OVERSCAN) / ROW).floor().max(0.0) as usize).min(total);
    let last = ((((scroll + height + OVERSCAN) / ROW).ceil()) as usize).min(total);
    let painted = document.highlights(first..last);
    let pair: Vec<Range<usize>> = document
        .syntax
        .as_ref()
        .and_then(|syntax| syntax.bracket_pair(document.field.text(), document.field.caret()))
        .map(|(open, close)| vec![open, close])
        .unwrap_or_default();
    let mut rows = vec![blank(first)];
    rows.extend((first..last).zip(painted).map(|(index, spans)| {
        file_line(document, index, line_marks(document, index, spans, &pair))
    }));
    rows.push(blank(total - last));
    rows
}

/// The items of the outline that hold the caret of `document`, outermost first, as `fn main › impl Thing`.
fn breadcrumbs(document: &Document) -> Option<String> {
    let syntax = document.syntax.as_ref()?;
    let items = syntax.enclosing(document.field.text(), document.field.caret());
    let words: Vec<String> = items
        .iter()
        .rev()
        .take(3)
        .rev()
        .map(|item| {
            let name: String = item.name.chars().take(40).collect();
            if item.context.is_empty() {
                name
            } else {
                format!("{} {name}", item.context)
            }
        })
        .collect();
    (!words.is_empty()).then(|| words.join("  ›  "))
}

/// How many errors and warnings the language server finds in `document`, as short words in their colours.
fn tallies(document: &Document) -> Vec<Node> {
    let count = |wanted: Severity| {
        document
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.severity == wanted)
            .count()
    };
    [(Severity::Error, "error"), (Severity::Warning, "warning")]
        .into_iter()
        .filter_map(|(severity, word)| {
            let found = count(severity);
            (found > 0).then(|| {
                let plural = if found == 1 { "" } else { "s" };
                line(
                    format!("{found} {word}{plural}"),
                    11,
                    1.4,
                    severity_role(severity),
                )
            })
        })
        .collect()
}

/// The strip above the file: its icon and name, and the button that writes it when it has changes to write.
fn strip(viewer: &Viewer) -> Node {
    let mut strip = node()
        .styles([
            flex(),
            items_center(),
            gap(2),
            h(10),
            px(4),
            flex_none(),
            border_b(),
            border_color(Role::Border),
        ])
        .child(file_icon(&viewer.path, 4.0))
        .child(line(&viewer.path, 13, 1.4, Role::Text).styles([font_mono()]));
    if viewer.head {
        strip = strip.child(line("HEAD", 11, 1.4, Role::Muted));
    }
    if let Contents::Text(document) = &viewer.contents
        && document.field.is_read_only()
        && !viewer.head
    {
        strip = strip.child(line("Read only", 11, 1.4, Role::Muted));
    }
    if let Contents::Text(document) = &viewer.contents
        && let Some(crumbs) = breadcrumbs(document)
    {
        strip = strip.child(line(crumbs, 12, 1.4, Role::Muted).styles([
            font_mono(),
            min_w_px(0),
            overflow_hidden(),
        ]));
    }
    strip = strip.child(spacer());
    if let Contents::Text(document) = &viewer.contents {
        strip = strip.children(tallies(document));
        if let Some(problem) = &document.server_problem {
            let short: String = problem.chars().take(60).collect();
            strip = strip
                .child(line(short, 11, 1.4, Role::Subtle).styles([min_w_px(0), overflow_hidden()]));
        }
    }
    if viewer.is_dirty() {
        strip = strip.child(
            Button::new("Save", EDITOR_SAVE)
                .variant(Variant::Secondary)
                .size(Size::Sm)
                .build(),
        );
    }
    strip
}

/// The file shown, in a view `height` logical pixels tall: a strip naming it, then its lines, which are edited in place unless the file is read only.
pub fn viewer(state: &WorkspaceState, height: f32) -> Node {
    let Some(viewer) = state.shown_viewer() else {
        return node();
    };
    let lines_height = (height - STRIP).max(0.0);
    let body = match &viewer.contents {
        Contents::Loading => {
            node()
                .styles([px(4), py(4)])
                .child(line("Reading the file.", 13, 1.4, Role::Subtle))
        }
        Contents::Message(message) => {
            node()
                .styles([px(4), py(4)])
                .child(line(message, 13, 1.4, Role::Subtle))
        }
        Contents::Text(document) => {
            let mut rows = node()
                .styles([
                    flex(),
                    flex_col(),
                    flex_none(),
                    py(2),
                    min_h(lines_height / 4.0),
                    cursor_text(),
                ])
                .focusable(EDITOR_FIELD)
                .children(visible(document, viewer.scroll, lines_height));
            if document.truncated {
                rows = rows.child(node().styles([px(4), py(2)]).child(line(
                    "The rest of the file is not shown.",
                    12,
                    1.4,
                    Role::Subtle,
                )));
            }
            rows
        }
    };
    node()
        .styles([
            flex(),
            flex_col(),
            flex_1(),
            min_w_px(0),
            min_h(0),
            bg(Role::Background),
        ])
        .child(strip(viewer))
        .child(
            node()
                .styles([flex(), flex_1(), min_h(0), overflow_hidden()])
                .child(
                    node()
                        .styles([
                            flex(),
                            flex_col(),
                            flex_1(),
                            min_w_px(0),
                            min_h(0),
                            overflow_hidden(),
                        ])
                        .scroll_from_start(VIEWER_SCROLL, viewer.scroll)
                        .child(body),
                ),
        )
}
