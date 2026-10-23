//! A file open in the window: its lines, numbered, as they are in the folder or in the last commit, edited in place.

use std::ops::Range;

use pm_icon::IconName;
use pm_input::FocusId;
use pm_protocol::{HintKind, Severity};
use pm_style::{
    Radius, Role, bg, border_b, border_color, code_px, cursor_pointer, cursor_text, flex, flex_1,
    flex_col, flex_none, font_mono, gap, h, h_px, items_center, justify_center, justify_end,
    leading_ratio, min_h, min_w_px, ml, overflow_hidden, pr, px, py, relative, rounded,
    rounded_full, text_color, w, w_px,
};
use pm_text::Shaper;
use pm_treesitter::{Highlight, Kind};
use pm_ui::{Mark, Node, Paint, node, text};

use crate::Button;
use crate::annotations::lens_mark;
use crate::button::Variant;
use crate::debug::{Dot, Gutter, breakpoint_mark};
use crate::file_icon::file_icon;
use crate::git_controls::VIEWER_SCROLL;
use crate::open_files::{Contents, EDITOR_FIELD, EDITOR_SAVE, FILE_TAB_BASE, STRIP, Viewer};
use crate::panes::PEEK_SCROLL_BASE;
use crate::parts::{glyph, line, spacer};
use crate::size::Size;
use crate::state::WorkspaceState;
use crate::tasks::run_mark;
use pm_editor::{Document, EditorSettings, Hint, Lens, LineChange};

/// How wide a view of a file must be, in logical pixels, for a minimap to be drawn beside it.
const MINIMAP_ROOM: f32 = 420.0;

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

/// The colours brackets are drawn in, one for each level of nesting, going round again after the last.
const BRACKET_ROLES: [Role; 4] = [
    Role::SyntaxType,
    Role::SyntaxKeyword,
    Role::SyntaxFunction,
    Role::SyntaxString,
];

/// The colour a bracket enclosed by `depth` pairs is drawn in.
fn bracket_role(depth: usize) -> Role {
    BRACKET_ROLES[depth % BRACKET_ROLES.len()]
}

/// Puts `role` over the stretch `range` of the coloured letters in `marks`, cutting the stretches it falls on short and leaving the rest of them as they were.
fn recolor(marks: &mut Vec<Mark>, range: Range<usize>, role: Role) {
    let mut kept = Vec::with_capacity(marks.len() + 1);
    for mark in marks.drain(..) {
        let overlaps = mark.paint == Paint::Ink
            && mark.range.start < range.end
            && range.start < mark.range.end;
        if !overlaps {
            kept.push(mark);
            continue;
        }
        for piece in [
            mark.range.start..range.start.max(mark.range.start),
            range.end.min(mark.range.end)..mark.range.end,
        ] {
            if piece.start < piece.end {
                kept.push(Mark {
                    range: piece,
                    color: mark.color,
                    paint: Paint::Ink,
                });
            }
        }
    }
    kept.push(Mark {
        range,
        color: role.into(),
        paint: Paint::Ink,
    });
    *marks = kept;
}

/// The marks of line `index` of `document`: its highlighted stretches in their colours, the brackets `pair` paired at the caret behind a box, and what the language server finds wrong under a wavy line.
pub(crate) fn line_marks(
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
    if settings.bracket_colors {
        for (range, depth) in document.brackets_on(index) {
            recolor(&mut marks, range, bracket_role(depth));
        }
    }
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

/// How tall the rows of a line broken into `segments` are together, in logical pixels.
pub(crate) fn document_row(settings: &EditorSettings, segments: &[Range<usize>]) -> f32 {
    settings.row_height() * segments.len().max(1) as f32
}

/// How wide the cell for the breakpoint of a line is, in logical pixels.
pub const BREAKPOINT_CELL: f32 = 16.0;

/// How wide the cell for the run button of a line is, in logical pixels, in a file that has places that can be run.
pub const RUN_CELL: f32 = 16.0;

/// The cell before the breakpoint of a line that runs the place that starts on the line when pressed, and is empty on the lines where none starts.
fn run_cell(file: usize, index: usize, gutter: &Gutter, height: f32) -> Node {
    let cell = node().styles([
        flex(),
        items_center(),
        justify_center(),
        flex_none(),
        w_px(RUN_CELL),
        h_px(height),
    ]);
    if !gutter.runs.contains(&index) {
        return cell;
    }
    cell.styles([cursor_pointer()])
        .focusable(run_mark(file, index))
        .child(glyph(IconName::Play, 3.5, Role::Success))
}

/// The cells before a line number: the run button when the file has places that can be run, and the cell that sets or clears the breakpoint on the line when pressed, which shows the breakpoint, or the arrow where the program is paused.
pub(crate) fn breakpoint_cell(file: usize, index: usize, gutter: &Gutter, height: f32) -> Node {
    let cell = breakpoint_only(file, index, gutter, height);
    if gutter.runs.is_empty() {
        return cell;
    }
    node()
        .styles([flex(), flex_none(), h_px(height)])
        .child(run_cell(file, index, gutter, height))
        .child(cell)
}

/// The cell before a line number that sets or clears the breakpoint on the line when pressed, and shows the breakpoint, or the arrow where the program is paused.
fn breakpoint_only(file: usize, index: usize, gutter: &Gutter, height: f32) -> Node {
    let cell = node()
        .styles([
            flex(),
            items_center(),
            justify_center(),
            flex_none(),
            w_px(BREAKPOINT_CELL),
            h_px(height),
            cursor_pointer(),
        ])
        .focusable(breakpoint_mark(file, index));
    let dot =
        |role: Role| node().styles([w_px(9.0), h_px(9.0), flex_none(), rounded_full(), bg(role)]);
    match (gutter.dot(index), gutter.paused == Some(index)) {
        (Some(Dot::Plain), _) => cell.child(dot(Role::Danger)),
        (Some(Dot::Conditional), _) => cell.child(dot(Role::Warning)),
        (Some(Dot::Unplaced), _) => cell.child(dot(Role::Subtle)),
        (None, true) => cell.child(glyph(IconName::ArrowRight, 3.5, Role::Warning)),
        (None, false) => cell,
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

/// What stands beside a line: who last changed it, and the cell that sets its breakpoint.
pub(crate) struct Beside {
    /// Who last changed the line, when it is said.
    pub(crate) blamed: Option<String>,
    /// The cell before the line number.
    pub(crate) breakpoint: Node,
    /// Whether a program being debugged is paused on the line.
    pub(crate) paused: bool,
}

/// One numbered line of the file, broken into the rows `segments` of it hold, with the caret and selection placed in it for the field `owner`, coloured by `marks`.
pub(crate) fn file_line(
    document: &Document,
    index: usize,
    marks: Vec<Mark>,
    segments: Vec<Range<usize>>,
    beside: Beside,
    settings: &EditorSettings,
    owner: FocusId,
) -> Node {
    let Beside {
        blamed,
        breakpoint,
        paused,
    } = beside;
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
    let hints: Vec<&Hint> = if settings.inlay_hints {
        document.hints_on(index).iter().collect()
    } else {
        Vec::new()
    }
    .into_iter()
    .chain(document.values_on(index))
    .collect();
    let lenses = if settings.code_lens {
        document.lenses_on(index)
    } else {
        &[]
    };
    let rows = segments.iter().enumerate().map(|(place, segment)| {
        let absolute = line.start + segment.start..line.start + segment.end;
        let standing: Vec<&Hint> = hints
            .iter()
            .filter(|hint| {
                (absolute.start <= hint.offset && hint.offset < absolute.end)
                    || (place == last && hint.offset == absolute.end)
            })
            .copied()
            .collect();
        let mut cuts: Vec<usize> = vec![segment.start];
        cuts.extend(standing.iter().map(|hint| hint.offset - line.start));
        cuts.push(segment.end);
        let pieces = cuts.len() - 1;
        let mut held = node().styles([
            flex(),
            items_center(),
            h_px(height),
            flex_none(),
            min_w_px(0),
        ]);
        for (step, bounds) in cuts.windows(2).enumerate() {
            let piece = bounds[0]..bounds[1];
            let piece_absolute = line.start + piece.start..line.start + piece.end;
            let ends_segment = step + 1 == pieces;
            let mut content = text(&text_of[piece.clone()])
                .styles([
                    code_px(font),
                    leading_ratio(1.4),
                    font_mono(),
                    text_color(Role::Text),
                    flex_none(),
                ])
                .field_line(owner, piece_absolute.start)
                .marks(marks_in(
                    &marks,
                    &piece,
                    place == 0 && step == 0,
                    place == last && ends_segment,
                ))
                .caret_look(caret_look(settings));
            let after_start = if step == 0 {
                piece_absolute.start <= caret
            } else {
                piece_absolute.start < caret
            };
            let before_end = if ends_segment {
                caret < piece_absolute.end || (place == last && caret == piece_absolute.end)
            } else {
                caret <= piece_absolute.end
            };
            let holds_caret = after_start && before_end;
            if holds_caret {
                content = content.caret(caret - piece_absolute.start, owner);
            }
            if let Some(range) = clipped(&document.field.selection(), &piece_absolute) {
                content = content.selection(range, owner);
            }
            if let Some(range) = clipped(&document.field.marked(), &piece_absolute) {
                content = content.marked(range, owner);
            }
            held = held.child(content);
            if let Some(hint) = standing.get(step) {
                held = held.child(hint_chip(hint, font));
            }
        }
        if place < last {
            return held;
        }
        held.children(document.is_folded_at(index).then(|| fold_chip(index)))
            .children(lenses.iter().map(|lens| lens_chip(lens, font)))
            .children(blamed.clone().map(|said| {
                node()
                    .styles([flex_1(), ml(8), min_w_px(0), overflow_hidden()])
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
        .child(breakpoint)
        .child(change_bar(
            document.line_change(index).filter(|_| settings.git_gutter),
            height * segments.len().max(1) as f32,
        ));
    if paused {
        row = row.styles([bg(Role::SurfaceActive)]);
    } else if settings.current_line_highlight
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

/// The text of `hint` written inside a line in a smaller, quieter hand than the code around it.
fn hint_chip(hint: &Hint, font: u32) -> Node {
    let tone = match hint.kind {
        HintKind::Type | HintKind::Value => Role::Subtle,
        HintKind::Parameter | HintKind::Other => Role::Muted,
    };
    let mut chip = node().styles([
        flex(),
        items_center(),
        flex_none(),
        px(1.0),
        rounded(Radius::Sm),
        bg(Role::Surface),
    ]);
    if hint.pad_left {
        chip = chip.styles([ml(1)]);
    }
    chip.child(line(&hint.label, (font - 1).max(8), 1.2, tone).styles([font_mono(), flex_none()]))
}

/// The text of `lens` at the end of its line, pressed to do what the server says when it can be.
fn lens_chip(lens: &Lens, font: u32) -> Node {
    let chip = node().styles([
        flex(),
        items_center(),
        flex_none(),
        ml(4),
        px(1.5),
        h(4),
        rounded(Radius::Sm),
    ]);
    let words = line(&lens.title, (font - 1).max(8), 1.2, Role::Subtle).styles([flex_none()]);
    if lens.pressable {
        chip.styles([cursor_pointer(), bg(Role::SurfaceHover).hover()])
            .focusable(lens_mark(lens.index))
            .child(words)
    } else {
        chip.child(words)
    }
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

/// How the lines of a file are drawn.
struct Look<'a> {
    /// The places find found, and the one chosen.
    found: &'a (Vec<Range<usize>>, Option<usize>),
    /// Who last changed a line, by its index.
    blamed: &'a dyn Fn(usize) -> Option<String>,
    /// How the editor draws.
    settings: &'a EditorSettings,
    /// How many characters a row holds when lines wrap.
    columns: Option<usize>,
    /// The field whose caret and selection the lines hold.
    owner: FocusId,
    /// The open file the lines are of, which names its marks.
    file: usize,
    /// The breakpoints and the paused line of the file.
    gutter: &'a Gutter,
}

/// The lines of `document` that are in or near a view `height` tall scrolled `scroll` down, drawn as `look` says, with a blank for those above and below.
fn visible(document: &Document, scroll: f32, height: f32, look: &Look) -> Vec<Node> {
    let Look {
        found,
        blamed,
        settings,
        columns,
        owner,
        file,
        gutter,
    } = *look;
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
                    let cell =
                        breakpoint_cell(file, index, gutter, document_row(settings, &segments));
                    file_line(
                        document,
                        index,
                        marks,
                        segments,
                        Beside {
                            blamed: blamed(index),
                            breakpoint: cell,
                            paused: gutter.paused == Some(index),
                        },
                        settings,
                        owner,
                    )
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

/// How many errors and warnings the language server finds in `document`, as short words in their colours, pressed to show the problems of the project; nothing when it finds none.
fn tallies(document: &Document) -> Option<Node> {
    let count = |wanted: Severity| {
        document
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.severity == wanted)
            .count()
    };
    let words: Vec<Node> = [(Severity::Error, "error"), (Severity::Warning, "warning")]
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
        .collect();
    (!words.is_empty()).then(|| {
        node()
            .styles([flex(), items_center(), gap(2), cursor_pointer()])
            .children(words)
            .focusable(crate::state::BOTTOM_PROBLEMS_COUNT)
    })
}

/// The strip above the file: its icon and name, and, while it is the file shown, `live`, the switch between its source and its rendering and the button that writes it when it has changes to write.
fn strip(viewer: &Viewer, live: bool) -> Node {
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
    if !live {
        return strip;
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
    match state.git.shown.filter(|_| state.shown_viewer().is_some()) {
        Some(index) => file_view(state, index, width, height, shaper),
        None => node(),
    }
}

/// The open file at `index`, in a view `width` by `height` logical pixels: the file shown, edited in place, or one shown in a pane other than the one being worked in, which a press on makes the file shown.
pub fn file_view(
    state: &WorkspaceState,
    index: usize,
    width: f32,
    height: f32,
    shaper: &mut Shaper,
) -> Node {
    let Some(viewer) = state.git.viewers.get(index) else {
        return node();
    };
    let live = state.shown_viewer().is_some() && state.git.shown == Some(index);
    let owner = if live {
        EDITOR_FIELD
    } else {
        FocusId(FILE_TAB_BASE + index as u64)
    };
    let scroller = if live {
        VIEWER_SCROLL
    } else {
        FocusId(PEEK_SCROLL_BASE + index as u64)
    };
    let found = if live {
        state.find_marks()
    } else {
        (Vec::new(), None)
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
                .focusable(owner)
                .children(visible(
                    document,
                    viewer.scroll,
                    lines_height,
                    &Look {
                        found: &found,
                        blamed: &|line| state.blame_of_line(line).filter(|_| live),
                        settings: &pm_editor::EditorSettings {
                            git_gutter: state.shows_gutter(),
                            ..state.editor.clone()
                        },
                        columns: state.wrap_columns(),
                        owner,
                        file: index,
                        gutter: &state.gutter_of(index),
                    },
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
        .child(strip(viewer, live))
        .children(crate::language_offer::offer(state, viewer))
        .child(
            node()
                .styles([flex(), flex_1(), min_h(0), overflow_hidden()])
                .child(
                    node()
                        .styles([
                            flex(),
                            flex_1(),
                            min_w_px(0),
                            min_h(0),
                            overflow_hidden(),
                            relative(),
                        ])
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
                                .scroll_from_start(scroller, viewer.scroll)
                                .child(body),
                        )
                        .children(match &viewer.contents {
                            Contents::Text(document) if source => {
                                crate::scroll_marks::scroll_marks(document, &found.0, &state.editor)
                            }
                            _ => None,
                        })
                        .children(match &viewer.contents {
                            Contents::Text(document) if source => crate::sticky::sticky(
                                document,
                                viewer.scroll,
                                state.wrap_columns(),
                                &state.editor,
                            ),
                            _ => None,
                        })
                        .children(state.finding.as_ref().filter(|_| live).and_then(|finding| {
                            match &viewer.contents {
                                Contents::Text(document) if source => Some(crate::find::find_bar(
                                    finding,
                                    document,
                                    state.editor.smartcase,
                                )),
                                _ => None,
                            }
                        })),
                )
                .children(match &viewer.contents {
                    Contents::Text(document)
                        if source && state.editor.minimap && width >= MINIMAP_ROOM =>
                    {
                        Some(crate::minimap::minimap(
                            document,
                            viewer.scroll,
                            lines_height,
                            state.editor.row_height(),
                            state.wrap_columns(),
                            lines_height,
                        ))
                    }
                    _ => None,
                }),
        )
}
