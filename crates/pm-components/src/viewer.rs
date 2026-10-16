//! A file open in the window: its lines, numbered, as they are in the folder or in the last commit, edited in place.

use std::ops::Range;

use pm_input::FocusId;
use pm_protocol::Severity;
use pm_style::{
    Radius, Role, bg, border_b, border_color, cursor_pointer, cursor_text, flex, flex_1, flex_col,
    flex_none, font_mono, gap, h, h_px, items_center, justify_end, leading_ratio, min_h, min_w_px,
    ml, overflow_hidden, pr, px, py, relative, rounded, text_color, text_px, w, w_px,
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
use pm_editor::{Document, LineChange};

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
    found: &(Vec<Range<usize>>, Option<usize>),
) -> Vec<Mark> {
    let line = document.line(index);
    let (matches, selected) = found;
    let mut found_marks: Vec<Mark> = matches
        .iter()
        .enumerate()
        .filter(|(_, held)| held.end >= line.start && held.start <= line.end)
        .filter_map(|(place, held)| {
            clipped(&Some(held.clone()), &line).map(|range| Mark {
                range,
                color: if Some(place) == *selected {
                    Role::Warning.into()
                } else {
                    Role::SurfaceActive.into()
                },
                paint: if Some(place) == *selected {
                    Paint::Underline
                } else {
                    Paint::Fill
                },
            })
        })
        .collect();
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
    marks.append(&mut found_marks);
    let primary = document.field.caret();
    for (anchor, caret) in document
        .field
        .selections()
        .into_iter()
        .filter(|(_, caret)| *caret != primary)
    {
        let (start, end) = (anchor.min(caret), anchor.max(caret));
        if start < end
            && let Some(range) = clipped(&Some(start..end), &line)
        {
            marks.push(Mark {
                range,
                color: Role::Focused.into(),
                paint: Paint::Selected,
            });
        }
        if (line.start..=line.end).contains(&caret) {
            let at = caret - line.start;
            marks.push(Mark {
                range: at..at,
                color: Role::Text.into(),
                paint: Paint::Caret,
            });
        }
    }
    marks.extend(document.occurrences().iter().filter_map(|used| {
        clipped(&Some(used.clone()), &line).map(|range| Mark {
            range,
            color: Role::SurfaceHover.into(),
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

/// The mark beside a line number saying how the line differs from the last commit: a bar for a line added or changed, and a short bar at its top where lines were taken out before it.
fn change_bar(change: Option<LineChange>) -> Node {
    let holder = node().styles([flex(), flex_col(), flex_none(), w_px(3.0), h(5)]);
    match change {
        Some(LineChange::Added) => holder.styles([bg(Role::Success)]),
        Some(LineChange::Modified) => holder.styles([bg(Role::Warning)]),
        Some(LineChange::DeletedBefore) => {
            holder.child(node().styles([w_px(3.0), h_px(4.0), bg(Role::Danger)]))
        }
        None => holder,
    }
}

/// One numbered line of the file, with the caret and selection placed in it, coloured by `marks`.
fn file_line(document: &Document, index: usize, marks: Vec<Mark>, blamed: Option<String>) -> Node {
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
        .child(change_bar(document.line_change(index)))
        .child(
            node()
                .styles([
                    flex(),
                    items_center(),
                    justify_end(),
                    flex_none(),
                    w(13),
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
                .styles([
                    flex(),
                    items_center(),
                    flex_1(),
                    min_w_px(0),
                    overflow_hidden(),
                    px(2),
                ])
                .child(content)
                .children(document.is_folded_at(index).then(|| fold_chip(index)))
                .children(blamed.map(|said| {
                    node()
                        .styles([flex_none(), ml(8), min_w_px(0), overflow_hidden()])
                        .child(crate::parts::line(said, 12, 1.4, Role::Subtle))
                })),
        )
}

/// The first of the marks standing for folded lines, each following it by the index of the line it is on.
pub const FOLD_BASE: u64 = 7_500_000;

/// One past the last mark standing for folded lines.
pub const FOLD_END: u64 = 8_500_000;

/// The mark after the line at `index` standing for the lines folded away after it, which unfolds them when pressed.
fn fold_chip(index: usize) -> Node {
    node()
        .styles([
            flex(),
            items_center(),
            flex_none(),
            ml(2),
            px(1.5),
            h(4),
            rounded(Radius::Sm),
            bg(Role::SurfaceActive),
            bg(Role::SurfaceHover).hover(),
            cursor_pointer(),
        ])
        .focusable(FocusId(FOLD_BASE + index as u64))
        .child(line("⋯", 11, 1.2, Role::Muted))
}

/// The line whose folded lines the mark `id` stands for.
pub fn fold_of(id: FocusId) -> Option<usize> {
    (FOLD_BASE..FOLD_END)
        .contains(&id.0)
        .then(|| (id.0 - FOLD_BASE) as usize)
}

/// How many lines on either side of those shown are looked through for the indentation a blank line takes.
const GUIDE_REACH: usize = 200;

/// How many columns the indentation of the line at `index` of `document` takes, or nothing for a blank line.
fn indent_columns(document: &Document, index: usize) -> Option<usize> {
    let line = document.line(index);
    let held = &document.field.text()[line];
    if held.trim().is_empty() {
        return None;
    }
    Some(held.len() - held.trim_start_matches(' ').len())
}

/// The columns the indent guides of each of the lines `rows` of `document` stand at: one at each level of indentation the line, or for a blank line the lines around it, is in; none in a file indented with tabs.
fn guides(document: &Document, rows: Range<usize>) -> Vec<Vec<usize>> {
    let unit = document.editing.indent.max(1);
    if document.editing.tabs {
        return vec![Vec::new(); rows.len()];
    }
    let total = document.lines();
    let reach_back = rows.start.saturating_sub(GUIDE_REACH);
    let reach_on = (rows.end + GUIDE_REACH).min(total);
    let columns: Vec<Option<usize>> = (reach_back..reach_on)
        .map(|index| indent_columns(document, index))
        .collect();
    let at = |index: usize| columns[index - reach_back];
    rows.map(|index| {
        let depth = at(index).unwrap_or_else(|| {
            let before = (reach_back..index).rev().find_map(at).unwrap_or(0);
            let after = (index + 1..reach_on).find_map(at).unwrap_or(0);
            before.min(after)
        });
        (0..depth).step_by(unit).collect()
    })
    .collect()
}

/// The lines of `document` that are in or near a view `height` tall scrolled `scroll` down, with a blank for those above and below.
fn visible(
    document: &Document,
    scroll: f32,
    height: f32,
    found: &(Vec<Range<usize>>, Option<usize>),
    blamed: &dyn Fn(usize) -> Option<String>,
) -> Vec<Node> {
    let total = document.rows();
    let first = (((scroll - OVERSCAN) / ROW).floor().max(0.0) as usize).min(total);
    let last = ((((scroll + height + OVERSCAN) / ROW).ceil()) as usize).min(total);
    let shown = document.shown_lines(first, last - first);
    let pair: Vec<Range<usize>> = document
        .syntax
        .as_ref()
        .and_then(|syntax| syntax.bracket_pair(document.field.text(), document.field.caret()))
        .map(|(open, close)| vec![open, close])
        .unwrap_or_default();
    let mut rows = vec![blank(first)];
    for run in runs(&shown) {
        let painted = document.highlights(run.clone());
        let standing = guides(document, run.clone());
        rows.extend(
            run.zip(painted)
                .zip(standing)
                .map(|((index, spans), columns)| {
                    let mut marks = line_marks(document, index, spans, &pair, found);
                    marks.extend(columns.into_iter().map(|column| Mark {
                        range: column..column,
                        color: Role::Border.into(),
                        paint: Paint::Guide,
                    }));
                    file_line(document, index, marks, blamed(index))
                }),
        );
    }
    rows.push(blank(total - last));
    rows
}

/// The lines `shown`, in order, as runs of lines that follow one another.
fn runs(shown: &[usize]) -> Vec<Range<usize>> {
    let mut found: Vec<Range<usize>> = Vec::new();
    for index in shown {
        match found.last_mut() {
            Some(run) if run.end == *index => run.end += 1,
            _ => found.push(*index..*index + 1),
        }
    }
    found
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
                .children(visible(
                    document,
                    viewer.scroll,
                    lines_height,
                    &state.find_marks(),
                    &|index| state.blame_of_line(index),
                ));
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
                .styles([flex(), flex_1(), min_h(0), overflow_hidden(), relative()])
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
                )
                .children(
                    state
                        .finding
                        .as_ref()
                        .and_then(|finding| match &viewer.contents {
                            Contents::Text(document) => {
                                Some(crate::find::find_bar(finding, document))
                            }
                            _ => None,
                        }),
                ),
        )
}
