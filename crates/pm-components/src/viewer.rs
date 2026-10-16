//! A file open in the window: its lines, numbered, as they are in the folder or in the last commit, edited in place.

use std::ops::Range;

use pm_input::FocusId;
use pm_protocol::Severity;
use pm_style::{
    Radius, Role, bg, border_b, border_color, code_px, cursor_pointer, cursor_text, flex, flex_1,
    flex_col, flex_none, font_mono, gap, h, h_px, items_center, justify_end, leading_ratio, min_h,
    min_w_px, ml, overflow_hidden, pr, px, py, relative, rounded, text_color, w, w_px,
};
use pm_text::Shaper;
use pm_treesitter::{Highlight, Kind};
use pm_ui::{Mark, Node, Paint, node, text};

use crate::Button;
use crate::button::{Size, Variant};
use crate::file_icon::file_icon;
use crate::git_controls::VIEWER_SCROLL;
use crate::open_files::{Contents, EDITOR_FIELD, EDITOR_SAVE, STRIP, Viewer};
use crate::parts::{line, spacer};
use crate::state::WorkspaceState;
use pm_editor::{Document, EditorSettings, LineChange};

/// How far beyond the view, above and below, lines are still built, in logical pixels.
const OVERSCAN: f32 = 100.0;

/// A box as tall as `lines` lines `row` logical pixels tall, standing in for those left out.
fn blank(lines: usize, row: f32) -> Node {
    node().styles([h_px(lines as f32 * row), flex_none()])
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

/// The marks of the stretches of `highlights` that fall in `range` of the text they were found in, as offsets into `range`, in their colours; `highlights` are in order, so none past the end of `range` is looked at.
pub(crate) fn highlight_marks(highlights: &[Highlight], range: Range<usize>) -> Vec<Mark> {
    highlights
        .iter()
        .take_while(|highlight| highlight.range.start < range.end)
        .filter_map(|highlight| {
            let low = highlight.range.start.max(range.start);
            let high = highlight.range.end.min(range.end);
            let role = kind_role(highlight.kind)?;
            (low < high).then(|| Mark {
                range: low - range.start..high - range.start,
                color: role.into(),
                paint: Paint::Ink,
            })
        })
        .collect()
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
    settings: &EditorSettings,
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
            .filter(|(_, diagnostic)| settings.diagnostics.shows(diagnostic.severity))
            .map(|(range, diagnostic)| Mark {
                range,
                color: severity_role(diagnostic.severity).into(),
                paint: Paint::Wave,
            }),
    );
    marks
}

/// The mark beside a line number saying how the line differs from the last commit: a bar for a line added or changed, and a short bar at its top where lines were taken out before it.
fn change_bar(change: Option<LineChange>, height: f32) -> Node {
    let holder = node().styles([flex(), flex_col(), flex_none(), w_px(3.0), h_px(height)]);
    match change {
        Some(LineChange::Added) => holder.styles([bg(Role::Success)]),
        Some(LineChange::Modified) => holder.styles([bg(Role::Warning)]),
        Some(LineChange::DeletedBefore) => {
            holder.child(node().styles([w_px(3.0), h_px(4.0), bg(Role::Danger)]))
        }
        None => holder,
    }
}

/// How the carets of a file are drawn, as `settings` say.
fn caret_look(settings: &EditorSettings) -> pm_ui::CaretLook {
    pm_ui::CaretLook {
        shape: match settings.cursor_shape {
            pm_editor::CursorShape::Bar => pm_ui::CaretShape::Bar,
            pm_editor::CursorShape::Block => pm_ui::CaretShape::Block,
            pm_editor::CursorShape::Underline => pm_ui::CaretShape::Underline,
        },
        steady: !settings.cursor_blink,
    }
}

/// `marks` of a line that fall in the stretch `segment` of it, moved to count from the segment's start; the guides stand in the `first` row alone, and a mark at the very end of the line in the `last`.
fn marks_in(marks: &[Mark], segment: &Range<usize>, first: bool, last: bool) -> Vec<Mark> {
    marks
        .iter()
        .filter_map(|mark| {
            if mark.paint == Paint::Guide {
                return first.then(|| mark.clone());
            }
            let range = &mark.range;
            let kept = if range.is_empty() {
                (segment.start..segment.end).contains(&range.start)
                    || (last && range.start == segment.end)
            } else {
                range.start < segment.end && range.end > segment.start
            };
            kept.then(|| Mark {
                range: range.start.max(segment.start) - segment.start
                    ..range
                        .end
                        .min(segment.end)
                        .max(range.start.max(segment.start))
                        - segment.start,
                color: mark.color,
                paint: mark.paint,
            })
        })
        .collect()
}

/// One numbered line of the file, broken into the rows `segments` of it hold, with the caret and selection placed in it, coloured by `marks`.
fn file_line(
    document: &Document,
    index: usize,
    marks: Vec<Mark>,
    segments: Vec<Range<usize>>,
    blamed: Option<String>,
    settings: &EditorSettings,
) -> Node {
    let line = document.line(index);
    let text_of = &document.field.text()[line.clone()];
    let caret = document.field.caret();
    let caret_line = document.line_of(caret);
    let worst = document
        .diagnostics_on(index)
        .iter()
        .map(|(_, diagnostic)| diagnostic.severity)
        .filter(|severity| settings.diagnostics.shows(*severity))
        .min();
    let font = settings.font_size;
    let height = settings.row_height();
    let number = if settings.relative_line_numbers && index != caret_line {
        index.abs_diff(caret_line)
    } else {
        index + 1
    };
    let last = segments.len().saturating_sub(1);
    let rows = segments.iter().enumerate().map(|(place, segment)| {
        let absolute = line.start + segment.start..line.start + segment.end;
        let mut content = text(&text_of[segment.clone()])
            .styles([
                code_px(font),
                leading_ratio(1.4),
                font_mono(),
                text_color(Role::Text),
                flex_none(),
            ])
            .field_line(EDITOR_FIELD, absolute.start)
            .marks(marks_in(&marks, segment, place == 0, place == last))
            .caret_look(caret_look(settings));
        let holds_caret = (absolute.start..absolute.end).contains(&caret)
            || (place == last && caret == absolute.end);
        if holds_caret {
            content = content.caret(caret - absolute.start, EDITOR_FIELD);
        }
        if let Some(range) = clipped(&document.field.selection(), &absolute) {
            content = content.selection(range, EDITOR_FIELD);
        }
        if let Some(range) = clipped(&document.field.marked(), &absolute) {
            content = content.marked(range, EDITOR_FIELD);
        }
        let held = node()
            .styles([
                flex(),
                items_center(),
                h_px(height),
                flex_none(),
                min_w_px(0),
            ])
            .child(content);
        if place < last {
            return held;
        }
        held.children(document.is_folded_at(index).then(|| fold_chip(index)))
            .children(blamed.clone().map(|said| {
                node()
                    .styles([flex_none(), ml(8), min_w_px(0), overflow_hidden()])
                    .child(crate::parts::line(said, font, 1.4, Role::Subtle))
            }))
    });
    let mut row = node()
        .styles([
            flex(),
            h_px(height * segments.len().max(1) as f32),
            flex_none(),
            overflow_hidden(),
        ])
        .child(change_bar(
            document.line_change(index).filter(|_| settings.git_gutter),
            height * segments.len().max(1) as f32,
        ));
    if settings.current_line_highlight
        && index == caret_line
        && document.field.selection().is_none()
    {
        row = row.styles([bg(Role::Surface)]);
    }
    let number_tone = if index == caret_line {
        Role::Muted
    } else {
        Role::Subtle
    };
    row.child(
        node()
            .styles([flex(), flex_col(), flex_none(), pr(3)])
            .styles([if settings.line_numbers { w(13) } else { w(2) }])
            .child(
                node()
                    .styles([flex(), items_center(), justify_end(), h_px(height)])
                    .children(settings.line_numbers.then(|| {
                        text(number.to_string()).styles([
                            code_px(font),
                            leading_ratio(1.4),
                            font_mono(),
                            text_color(worst.map_or(number_tone, severity_role)),
                        ])
                    })),
            ),
    )
    .child(
        node()
            .styles([
                flex(),
                flex_col(),
                flex_1(),
                min_w_px(0),
                overflow_hidden(),
                px(2),
            ])
            .children(rows),
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

/// The lines of `document` that are in or near a view `height` tall scrolled `scroll` down, broken into rows of `columns` characters when lines wrap, with a blank for those above and below.
fn visible(
    document: &Document,
    scroll: f32,
    height: f32,
    found: &(Vec<Range<usize>>, Option<usize>),
    blamed: &dyn Fn(usize) -> Option<String>,
    settings: &EditorSettings,
    columns: Option<usize>,
) -> Vec<Node> {
    let row = settings.row_height();
    let total = document.wrapped_rows(columns);
    let first = (((scroll - OVERSCAN) / row).floor().max(0.0) as usize).min(total);
    let last = ((((scroll + height + OVERSCAN) / row).ceil()) as usize).min(total);
    let (shown, starts) = document.wrapped_lines(first, last - first, columns);
    let pair: Vec<Range<usize>> = document
        .syntax
        .as_ref()
        .and_then(|syntax| syntax.bracket_pair(document.field.text(), document.field.caret()))
        .map(|(open, close)| vec![open, close])
        .unwrap_or_default();
    let mut rows = vec![blank(starts, row)];
    let mut drawn = starts;
    for run in runs(&shown) {
        let painted = document.highlights(run.clone());
        let mut standing = if settings.indent_guides {
            guides(document, run.clone())
        } else {
            vec![Vec::new(); run.len()]
        };
        if settings.wrap_guide {
            for columns in &mut standing {
                columns.push(settings.preferred_line_length);
            }
        }
        rows.extend(
            run.zip(painted)
                .zip(standing)
                .map(|((index, spans), guides_at)| {
                    let mut marks = line_marks(document, index, spans, &pair, found, settings);
                    marks.extend(guides_at.into_iter().map(|column| Mark {
                        range: column..column,
                        color: Role::Border.into(),
                        paint: Paint::Guide,
                    }));
                    let segments = document.line_segments(index, columns);
                    drawn += segments.len();
                    file_line(document, index, marks, segments, blamed(index), settings)
                }),
        );
    }
    rows.push(blank(total.saturating_sub(drawn), row));
    if settings.scroll_beyond_last_line {
        rows.push(node().styles([h_px((height - row * 2.0).max(0.0)), flex_none()]));
    }
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
    strip = strip.children(crate::preview::dimensions(viewer));
    if let Contents::Text(document) = &viewer.contents
        && viewer.rendering().is_none()
    {
        strip = strip.children(tallies(document));
        if let Some(problem) = &document.server_problem {
            let short: String = problem.chars().take(60).collect();
            strip = strip
                .child(line(short, 11, 1.4, Role::Subtle).styles([min_w_px(0), overflow_hidden()]));
        }
    }
    strip = strip.children(crate::preview::switch(viewer));
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

/// The file shown, in a view `width` by `height` logical pixels: a strip naming it, then the file rendered when it is shown so, or else its lines, which are edited in place unless the file is read only.
pub fn viewer(state: &WorkspaceState, width: f32, height: f32, shaper: &mut Shaper) -> Node {
    let Some(viewer) = state.shown_viewer() else {
        return node();
    };
    let lines_height = (height - STRIP).max(0.0);
    let rendered = crate::preview::rendered(viewer, &state.figures, (width, height), shaper);
    let source = rendered.is_none();
    let body = match (rendered, &viewer.contents) {
        (Some(rendered), _) => rendered,
        (None, Contents::Loading) => {
            node()
                .styles([px(4), py(4)])
                .child(line("Reading the file.", 13, 1.4, Role::Subtle))
        }
        (None, Contents::Message(message)) => {
            node()
                .styles([px(4), py(4)])
                .child(line(message, 13, 1.4, Role::Subtle))
        }
        (None, Contents::Picture(_)) => node(),
        (None, Contents::Text(document)) => {
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
                    &pm_editor::EditorSettings {
                        git_gutter: state.shows_gutter(),
                        ..state.editor.clone()
                    },
                    state.wrap_columns(),
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
                .children(match &viewer.contents {
                    Contents::Text(document) if source => crate::scroll_marks::scroll_marks(
                        document,
                        &state.find_marks().0,
                        &state.editor,
                    ),
                    _ => None,
                })
                .children(
                    state
                        .finding
                        .as_ref()
                        .and_then(|finding| match &viewer.contents {
                            Contents::Text(document) if source => Some(crate::find::find_bar(
                                finding,
                                document,
                                state.editor.smartcase,
                            )),
                            _ => None,
                        }),
                ),
        )
}
