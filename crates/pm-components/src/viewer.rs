//! A file open in the window: its lines, numbered, as they are in the folder or in the last commit, edited in place.

use crate::kit::sp;
use rustle::{Flex, flex_column, flex_row};
use std::ops::Range;

use pm_protocol::{HintKind, Severity};
use pm_treesitter::{Highlight, Kind};
use rustle::{CursorIcon, Modifier};

use crate::Button;
use crate::annotations::lens_mark;
use crate::button::Variant;
use crate::color_picker::COLOR_SWATCH_BASE;
use crate::debug::{Dot, Gutter, breakpoint_mark};
use crate::file_icon::file_icon;
use crate::git_controls::VIEWER_SCROLL;
use crate::kit::{
    FocusId, Hovered, IconName, Interactive, Looks, Radius, Role, Scroll, Side, Styled, Tone, Type,
    frame, label, label_with, scroll_area,
};
use crate::marked::{CaretLook, CaretShape, Mark, Paint, marked_field_line};
use crate::open_files::{Contents, EDITOR_FIELD, EDITOR_SAVE, FILE_TAB_BASE, STRIP, Viewer};
use crate::panes::PEEK_SCROLL_BASE;
use crate::parts::{glyph, line, spacer};
use crate::preview::Rendering;
use crate::size::Size;
use crate::state::WorkspaceState;
use crate::tasks::run_mark;
use pm_editor::{Document, EditorSettings, Hint, Lens, LineChange, Swatch};

/// How wide a view of a file must be, in logical pixels, for a minimap to be drawn beside it.
const MINIMAP_ROOM: f32 = 420.0;

/// How far beyond the view, above and below, lines are still built, in logical pixels.
const OVERSCAN: f32 = 100.0;

/// Emits a box as tall as `lines` lines `row` logical pixels tall, standing in for those left out.
#[track_caller]
fn blank(lines: usize, row: f32) {
    rustle::spacer(Modifier::new().height(lines as f32 * row));
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
                tone: role.into(),
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
                    tone: mark.tone,
                    paint: Paint::Ink,
                });
            }
        }
    }
    kept.push(Mark {
        range,
        tone: role.into(),
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
                tone: if Some(place) == *selected {
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
                tone: role.into(),
                paint: Paint::Ink,
            })
        })
        .collect();
    marks.extend(clipped(&document.find_scope(), &line).map(|range| Mark {
        range,
        tone: Role::SurfaceHover.into(),
        paint: Paint::Fill,
    }));
    if settings.bracket_colors {
        for (range, depth) in document.brackets_on(index) {
            recolor(&mut marks, range, bracket_role(depth));
        }
    }
    if settings.redact_private_values {
        for stretch in document.redactions().iter() {
            if let Some(range) = clipped(&Some(stretch.clone()), &line) {
                recolor(&mut marks, range.clone(), Role::Muted);
                marks.push(Mark {
                    range,
                    tone: Role::Muted.into(),
                    paint: Paint::Fill,
                });
            }
        }
    }
    marks.extend(pair.iter().filter_map(|bracket| {
        clipped(&Some(bracket.clone()), &line).map(|range| Mark {
            range,
            tone: Role::SurfaceActive.into(),
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
                tone: Role::Focused.into(),
                paint: Paint::Selected,
            });
        }
        if (line.start..=line.end).contains(&caret) {
            let at = caret - line.start;
            marks.push(Mark {
                range: at..at,
                tone: Role::Text.into(),
                paint: Paint::Caret,
            });
        }
    }
    marks.extend(document.occurrences().iter().filter_map(|used| {
        clipped(&Some(used.clone()), &line).map(|range| Mark {
            range,
            tone: Role::SurfaceHover.into(),
            paint: Paint::Fill,
        })
    }));
    marks.extend(
        document
            .drop_mark()
            .filter(|at| (line.start..=line.end).contains(at))
            .map(|at| Mark {
                range: at - line.start..at - line.start,
                tone: Role::Accent.into(),
                paint: Paint::Drop,
            }),
    );
    marks.extend(clipped(&document.reach, &line).map(|range| Mark {
        range,
        tone: Role::SyntaxFunction.into(),
        paint: Paint::Underline,
    }));
    marks.extend(
        document
            .diagnostics_on(index)
            .into_iter()
            .filter(|(_, diagnostic)| settings.diagnostics.shows(diagnostic.severity))
            .map(|(range, diagnostic)| Mark {
                range,
                tone: severity_role(diagnostic.severity).into(),
                paint: Paint::Wave,
            }),
    );
    marks
}

/// Emits the mark beside a line number saying how the line differs from the last commit: a bar for a line added or changed, and a short bar at its top where lines were taken out before it.
#[track_caller]
fn change_bar(change: Option<LineChange>, height: f32) {
    let holder = Modifier::new().width(3.0).height(height);
    let holder = match change {
        Some(LineChange::Added) => holder.fill(Role::Success),
        Some(LineChange::Modified) => holder.fill(Role::Warning),
        Some(LineChange::DeletedBefore) | None => holder,
    };
    flex_column(holder, Flex::new(), move || {
        if matches!(change, Some(LineChange::DeletedBefore)) {
            rustle::spacer(Modifier::new().width(3.0).height(4.0).fill(Role::Danger));
        }
    });
}

/// How tall the rows of a line broken into `segments` are together, in logical pixels.
pub(crate) fn document_row(settings: &EditorSettings, segments: &[Range<usize>]) -> f32 {
    settings.row_height() * segments.len().max(1) as f32
}

/// How wide the cell for the breakpoint of a line is, in logical pixels.
pub const BREAKPOINT_CELL: f32 = 16.0;

/// How wide the cell for the run button of a line is, in logical pixels, in a file that has places that can be run.
pub const RUN_CELL: f32 = 16.0;

/// Emits the cell before the breakpoint of a line that runs the place that starts on the line when pressed, and is empty on the lines where none starts.
#[track_caller]
fn run_cell(file: usize, index: usize, gutter: &Gutter, height: f32) {
    let runs = gutter.runs.contains(&index);
    let cell = Modifier::new().width(RUN_CELL).height(height);
    let cell = if runs {
        cell.cursor(CursorIcon::Pointer)
            .target(run_mark(file, index))
    } else {
        cell
    };
    flex_row(
        cell,
        Flex::new().items_center().justify_center(),
        move || {
            if runs {
                glyph(IconName::Play, 3.5, Role::Success);
            }
        },
    );
}

/// Emits the cells before a line number: the run button when the file has places that can be run, and the cell that sets or clears the breakpoint on the line when pressed, which shows the breakpoint, or the arrow where the program is paused.
#[track_caller]
pub(crate) fn breakpoint_cell(file: usize, index: usize, gutter: &Gutter, height: f32) {
    if gutter.runs.is_empty() {
        breakpoint_only(file, index, gutter, height);
        return;
    }
    flex_row(Modifier::new().height(height), Flex::new(), || {
        run_cell(file, index, gutter, height);
        breakpoint_only(file, index, gutter, height);
    });
}

/// Emits the cell before a line number that sets or clears the breakpoint on the line when pressed, and shows the breakpoint, or the arrow where the program is paused.
#[track_caller]
fn breakpoint_only(file: usize, index: usize, gutter: &Gutter, height: f32) {
    let cell = Modifier::new()
        .width(BREAKPOINT_CELL)
        .height(height)
        .cursor(CursorIcon::Pointer)
        .target(breakpoint_mark(file, index));
    let shown = (gutter.dot(index), gutter.paused == Some(index));
    flex_row(
        cell,
        Flex::new().items_center().justify_center(),
        move || match shown {
            (Some(Dot::Plain), _) => dot(Role::Danger),
            (Some(Dot::Conditional), _) => dot(Role::Warning),
            (Some(Dot::Unplaced), _) => dot(Role::Subtle),
            (None, true) => glyph(IconName::ArrowRight, 3.5, Role::Warning),
            (None, false) => {}
        },
    );
}

/// Emits the round dot of a breakpoint in `role`.
#[track_caller]
fn dot(role: Role) {
    rustle::spacer(
        Modifier::new()
            .width(9.0)
            .height(9.0)
            .fill_rounded(role, Radius::Full),
    );
}

/// How the carets of a file are drawn, as `settings` say.
fn caret_look(settings: &EditorSettings) -> CaretLook {
    CaretLook {
        shape: match settings.cursor_shape {
            pm_editor::CursorShape::Bar => CaretShape::Bar,
            pm_editor::CursorShape::Block => CaretShape::Block,
            pm_editor::CursorShape::Underline => CaretShape::Underline,
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
                tone: mark.tone,
                paint: mark.paint,
            })
        })
        .collect()
}

/// What stands beside a line: who last changed it, and the cell that sets its breakpoint.
pub(crate) struct Beside<F: FnOnce()> {
    /// Who last changed the line, when it is said.
    pub(crate) blamed: Option<String>,
    /// Emits the cell before the line number.
    pub(crate) breakpoint: F,
    /// Whether the marker for the changes the language server offers stands beside the line number.
    pub(crate) bulb: bool,
    /// Whether a program being debugged is paused on the line.
    pub(crate) paused: bool,
}

/// Emits one numbered line of the file, broken into the rows `segments` of it hold, with the caret and selection placed in it for the field `owner`, coloured by `marks`.
#[track_caller]
pub(crate) fn file_line(
    document: &Document,
    index: usize,
    marks: Vec<Mark>,
    segments: Vec<Range<usize>>,
    beside: Beside<impl FnOnce()>,
    settings: &EditorSettings,
    owner: FocusId,
) {
    let Beside {
        blamed,
        breakpoint,
        bulb,
        paused,
    } = beside;
    let line = document.line(index);
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
    let lens_row = document.lens_row_at(index);
    let text_segments = &segments[usize::from(lens_row)..];
    let last = text_segments.len().saturating_sub(1);
    let swatches = document.colors_on(index);
    let mut hints: Vec<&Hint> = if settings.inlay_hints {
        document.hints_on(index).iter().collect()
    } else {
        Vec::new()
    }
    .into_iter()
    .chain(document.values_on(index))
    .chain(swatches.iter())
    .collect();
    hints.sort_by_key(|hint| hint.offset);
    let run_lenses: Vec<(usize, String)> = document
        .run_lens_items()
        .into_iter()
        .enumerate()
        .filter(|(_, item)| item.line == index)
        .map(|(place, item)| (place, item.label))
        .collect();
    let lenses = if settings.code_lens {
        document.lenses_on(index)
    } else {
        &[]
    };
    let lens_row = lens_row && (!run_lenses.is_empty() || !lenses.is_empty());
    let total = height * segments.len().max(1) as f32;
    let own = Modifier::new().height(total);
    let own = if paused {
        own.fill(Role::SurfaceActive)
    } else if settings.current_line_highlight
        && index == caret_line
        && document.field.selection().is_none()
    {
        own.fill(Role::Surface)
    } else {
        own
    };
    let number_tone = if index == caret_line {
        Role::Muted
    } else {
        Role::Subtle
    };
    let number_box = if settings.line_numbers {
        Modifier::new().w(13)
    } else {
        Modifier::new().w(2)
    };
    flex_row(own.clip_to(Radius::None), Flex::new(), || {
        breakpoint();
        change_bar(
            document.line_change(index).filter(|_| settings.git_gutter),
            total,
        );
        flex_column(number_box.pad_side(Side::Right, 3), Flex::new(), || {
            if lens_row {
                rustle::spacer(Modifier::new().height(height));
            }
            flex_row(
                Modifier::new().height(height),
                Flex::new().items_center().justify_end(),
                || {
                    if bulb && settings.line_numbers {
                        bulb_button(height);
                    }
                    if settings.line_numbers {
                        label(
                            number.to_string(),
                            Type::code(font)
                                .ratio(1.4)
                                .tone(worst.map_or(number_tone, severity_role)),
                        );
                    }
                },
            );
        });
        flex_column(
            Modifier::new().grow().clip_to(Radius::None).pad_x(2),
            Flex::new(),
            || {
                if lens_row {
                    run_lens_row(&run_lenses, lenses, height, font);
                }
                for (place, segment) in text_segments.iter().enumerate() {
                    let at = Segment {
                        line: &line,
                        segment,
                        first: place == 0,
                        last: place == last,
                    };
                    if place < last {
                        segment_row(document, &at, &hints, &marks, settings, owner, || {});
                        continue;
                    }
                    segment_row(document, &at, &hints, &marks, settings, owner, || {
                        if document.is_folded_at(index) {
                            fold_chip(index);
                        }
                        if let Some(said) = blamed.clone() {
                            label_with(
                                Modifier::new().pad_side(Side::Left, 8).grow(),
                                said,
                                Type::new(font).ratio(1.4).tone(Role::Subtle).truncate(),
                            );
                        }
                    });
                }
            },
        );
    });
}

/// Where one row of a line broken into rows lies in the line and in the file.
struct Segment<'a> {
    /// Where the line lies in the file's text.
    line: &'a Range<usize>,
    /// Where the row lies in the line.
    segment: &'a Range<usize>,
    /// Whether it is the line's first row of text.
    first: bool,
    /// Whether it is the line's last row.
    last: bool,
}

/// Emits one row of a line, `at` saying where it lies: its text cut into pieces at the inlay `hints` that stand in it, each piece coloured by the `marks` of the line and holding the caret and selection of the field `owner` while that field has focus, with the hint after each piece, then what `tail` emits.
#[track_caller]
fn segment_row(
    document: &Document,
    at: &Segment,
    hints: &[&Hint],
    marks: &[Mark],
    settings: &EditorSettings,
    owner: FocusId,
    tail: impl FnOnce(),
) {
    let Segment {
        line,
        segment,
        first,
        last,
    } = *at;
    let text_of = &document.field.text()[line.clone()];
    let caret = document.field.caret();
    let font = settings.font_size;
    let height = settings.row_height();
    let focused = frame::is_focused(owner);
    let absolute = line.start + segment.start..line.start + segment.end;
    let standing: Vec<&Hint> = hints
        .iter()
        .filter(|hint| {
            (absolute.start <= hint.offset && hint.offset < absolute.end)
                || (last && hint.offset == absolute.end)
        })
        .copied()
        .collect();
    let mut cuts: Vec<usize> = vec![segment.start];
    cuts.extend(standing.iter().map(|hint| hint.offset - line.start));
    cuts.push(segment.end);
    let pieces = cuts.len() - 1;
    flex_row(
        Modifier::new().height(height),
        Flex::new().items_center(),
        || {
            for (step, bounds) in cuts.windows(2).enumerate() {
                let piece = bounds[0]..bounds[1];
                let piece_absolute = line.start + piece.start..line.start + piece.end;
                let ends_segment = step + 1 == pieces;
                let mut piece_marks =
                    marks_in(marks, &piece, first && step == 0, last && ends_segment);
                let after_start = if step == 0 {
                    piece_absolute.start <= caret
                } else {
                    piece_absolute.start < caret
                };
                let before_end = if ends_segment {
                    caret < piece_absolute.end || (last && caret == piece_absolute.end)
                } else {
                    caret <= piece_absolute.end
                };
                let holds_caret = after_start && before_end;
                if focused {
                    if holds_caret {
                        let at = caret - piece_absolute.start;
                        piece_marks.push(Mark::new(at..at, Role::Text, Paint::Caret));
                    }
                    if let Some(range) = clipped(&document.field.selection(), &piece_absolute) {
                        piece_marks.push(Mark::new(range, Role::Text, Paint::Selected));
                    }
                    if let Some(range) = clipped(&document.field.marked(), &piece_absolute) {
                        piece_marks.push(Mark::new(range, Role::Text, Paint::Underline));
                    }
                }
                marked_field_line(
                    Modifier::new(),
                    owner,
                    piece_absolute.start,
                    &text_of[piece.clone()],
                    Type::code(font).ratio(1.4).tone(Role::Text),
                    piece_marks,
                    caret_look(settings),
                );
                if let Some(hint) = standing.get(step) {
                    hint_chip(hint, font);
                }
            }
            tail();
        },
    );
}

/// Emits the row above a line that holds the links which run the line's place, each the `index`th of the links of the file, then the lenses `given` by the language server.
#[track_caller]
fn run_lens_row(lenses: &[(usize, String)], given: &[pm_editor::Lens], height: f32, font: u32) {
    flex_row(
        Modifier::new().height(height),
        Flex::new().items_center(),
        || {
            for (place, (index, label)) in lenses.iter().enumerate() {
                run_lens_chip(*index, label, font, place == 0);
            }
            for (place, lens) in given.iter().enumerate() {
                lens_chip(lens, font, lenses.is_empty() && place == 0);
            }
        },
    );
}

/// Emits the marker that says the language server offers changes on the line, which opens the list of them when pressed.
#[track_caller]
fn bulb_button(height: f32) {
    flex_row(
        Modifier::new()
            .width(16.0)
            .height(height)
            .cursor(CursorIcon::Pointer)
            .target(crate::lightbulb::LIGHTBULB),
        Flex::new().items_center().justify_center(),
        || glyph(IconName::Lightbulb, 3.5, Role::Warning),
    );
}

/// Emits the text of `hint` written inside a line in a smaller, quieter hand than the code around it, or the swatch of the colour it stands for, pressed to pick another.
#[track_caller]
fn hint_chip(hint: &Hint, font: u32) {
    if let Some(Swatch {
        color: [red, green, blue, alpha],
        index,
    }) = hint.swatch
    {
        let side = (font as f32 * 0.8).max(8.0);
        flex_row(
            Modifier::new()
                .cursor(CursorIcon::Pointer)
                .target(FocusId(COLOR_SWATCH_BASE + index as u64))
                .pad_side(Side::Right, 1.0),
            Flex::new().items_center(),
            || {
                rustle::spacer(Modifier::new().width(side).height(side).fill_rounded(
                    Tone::rgb(red, green, blue).alpha(f32::from(alpha) / 255.0),
                    Radius::Sm,
                ));
            },
        );
        return;
    }
    let tone = match hint.kind {
        HintKind::Type | HintKind::Value => Role::Subtle,
        HintKind::Parameter | HintKind::Other => Role::Muted,
    };
    let own = if hint.pad_left {
        Modifier::new().pad_side(Side::Left, 1)
    } else {
        Modifier::new()
    };
    flex_row(
        own.fill_rounded(Role::Surface, Radius::Sm).pad_x(1.0),
        Flex::new().items_center(),
        || {
            label(
                &hint.label,
                Type::new((font - 1).max(8)).ratio(1.2).tone(tone).mono(),
            );
        },
    );
}

/// The padding inside a chip of the row above a line: none at the left of the `first`, a step and a half otherwise and at the right.
fn chip_padding(own: Modifier, first: bool) -> Modifier {
    own.pad_side(Side::Left, if first { 0.0 } else { 1.5 })
        .pad_side(Side::Right, 1.5)
}

/// Emits the link at the end of its line that runs a place with the task `label`, the `index`th of the links of the file.
#[track_caller]
fn run_lens_chip(index: usize, label: &str, font: u32, first: bool) {
    let own = Modifier::new()
        .h(4)
        .looks(Looks::new().hover(Role::SurfaceHover).radius(Radius::Sm))
        .cursor(CursorIcon::Pointer)
        .target(crate::tasks::run_lens_mark(index));
    flex_row(chip_padding(own, first), Flex::new().items_center(), || {
        crate::kit::label(
            label,
            Type::new((font - 1).max(8)).ratio(1.2).tone(Role::Muted),
        );
    });
}

/// Emits the text of `lens` on the row above its line, pressed to do what the server says when it can be.
#[track_caller]
fn lens_chip(lens: &Lens, font: u32, first: bool) {
    let own = Modifier::new().h(4);
    let own = if lens.pressable {
        own.looks(Looks::new().hover(Role::SurfaceHover).radius(Radius::Sm))
            .cursor(CursorIcon::Pointer)
            .target(lens_mark(lens.index))
    } else {
        own
    };
    flex_row(chip_padding(own, first), Flex::new().items_center(), || {
        label(
            &lens.title,
            Type::new((font - 1).max(8)).ratio(1.2).tone(Role::Subtle),
        );
    });
}

/// The first of the marks standing for folded lines, each following it by the index of the line it is on.
pub const FOLD_BASE: u64 = 7_500_000;

/// One past the last mark standing for folded lines.
pub const FOLD_END: u64 = 8_500_000;

/// Emits the mark after the line at `index` standing for the lines folded away after it, which unfolds them when pressed.
#[track_caller]
fn fold_chip(index: usize) {
    flex_row(
        Modifier::new()
            .pad_side(Side::Left, 2)
            .h(4)
            .looks(
                Looks::new()
                    .idle(Role::SurfaceActive)
                    .hover(Role::SurfaceHover)
                    .radius(Radius::Sm),
            )
            .cursor(CursorIcon::Pointer)
            .target(FocusId(FOLD_BASE + index as u64))
            .pad_x(1.5),
        Flex::new().items_center(),
        || line("⋯", 11, 1.2, Role::Muted),
    );
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
    /// The line the marker for the changes the language server offers stands beside.
    bulb: Option<usize>,
}

/// Emits the lines of `document` that are in or near a view `height` tall scrolled `scroll` down, drawn as `look` says, with a blank for those above and below.
#[track_caller]
fn visible(document: &Document, scroll: f32, height: f32, look: &Look) {
    let Look {
        found,
        blamed,
        settings,
        columns,
        owner,
        file,
        gutter,
        bulb,
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
    blank(starts, row);
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
        for ((index, spans), guides_at) in run.zip(painted).zip(standing) {
            let mut marks = line_marks(document, index, spans, &pair, found, settings);
            marks.extend(
                guides_at
                    .into_iter()
                    .map(|column| Mark::new(column..column, Role::Border, Paint::Guide)),
            );
            let segments = document.line_segments(index, columns);
            drawn += segments.len();
            let cell_height = document_row(settings, &segments);
            rustle::key(index, || {
                file_line(
                    document,
                    index,
                    marks,
                    segments,
                    Beside {
                        blamed: blamed(index),
                        breakpoint: || breakpoint_cell(file, index, gutter, cell_height),
                        bulb: bulb == Some(index),
                        paused: gutter.paused == Some(index),
                    },
                    settings,
                    owner,
                );
            });
        }
    }
    blank(total.saturating_sub(drawn), row);
    if settings.scroll_beyond_last_line {
        rustle::spacer(Modifier::new().height((height - row * 2.0).max(0.0)));
    }
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

/// Emits how many errors and warnings the language server finds in `document`, as short words in their colours, pressed to show the problems of the project; emits nothing when it finds none.
#[track_caller]
fn tallies(document: &Document) {
    let count = |wanted: Severity| {
        document
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.severity == wanted)
            .count()
    };
    let words: Vec<(String, Role)> = [(Severity::Error, "error"), (Severity::Warning, "warning")]
        .into_iter()
        .filter_map(|(severity, word)| {
            let found = count(severity);
            (found > 0).then(|| {
                let plural = if found == 1 { "" } else { "s" };
                (format!("{found} {word}{plural}"), severity_role(severity))
            })
        })
        .collect();
    if words.is_empty() {
        return;
    }
    flex_row(
        Modifier::new()
            .cursor(CursorIcon::Pointer)
            .target(crate::state::BOTTOM_PROBLEMS_COUNT),
        Flex::new().items_center().gap(sp(2)),
        move || {
            for (said, role) in words {
                line(said, 11, 1.4, role);
            }
        },
    );
}

/// Emits the strip above the file: its icon and name, and, while it is the file shown, `live`, the switch between its source and its rendering and the button that writes it when it has changes to write.
#[track_caller]
fn strip(viewer: &Viewer, live: bool) {
    flex_row(
        Modifier::new()
            .h(10)
            .stroke_side(Side::Bottom, 1.0, Role::Border)
            .pad_x(4),
        Flex::new().items_center().gap(sp(2)),
        || {
            file_icon(&viewer.path, 4.0);
            label(
                &viewer.path,
                Type::new(13).ratio(1.4).tone(Role::Text).mono().truncate(),
            );
            if viewer.head {
                line("HEAD", 11, 1.4, Role::Muted);
            }
            if let Contents::Text(document) = &viewer.contents
                && document.field.is_read_only()
                && !viewer.head
            {
                line("Read only", 11, 1.4, Role::Muted);
            }
            if let Contents::Text(document) = &viewer.contents
                && let Some(crumbs) = breadcrumbs(document)
            {
                label(
                    crumbs,
                    Type::new(12).ratio(1.4).tone(Role::Muted).mono().truncate(),
                );
            }
            spacer();
            crate::preview::dimensions(viewer);
            if let Contents::Text(document) = &viewer.contents
                && viewer.rendering().is_none()
            {
                tallies(document);
                if let Some(problem) = &document.server_problem {
                    let short: String = problem.chars().take(60).collect();
                    line(short, 11, 1.4, Role::Subtle);
                }
            }
            if !live {
                return;
            }
            crate::preview::switch(viewer);
            if viewer.is_dirty() {
                Button::new("Save", EDITOR_SAVE)
                    .variant(Variant::Secondary)
                    .size(Size::Sm)
                    .build();
            }
        },
    );
}

/// Whether `viewer` is shown rendered rather than as its source: the cases in which [`crate::preview::rendered`] draws it.
fn shows_rendering(viewer: &Viewer) -> bool {
    matches!(
        (viewer.rendering(), &viewer.contents),
        (
            Some(Rendering::Markdown | Rendering::Vector),
            Contents::Text(_)
        ) | (Some(Rendering::Picture), Contents::Picture(_))
    )
}

/// Emits the file shown, in a view `width` by `height` logical pixels: a strip naming it, then the file rendered when it is shown so, or else its lines, which are edited in place unless the file is read only.
#[track_caller]
pub fn viewer(state: &WorkspaceState, width: f32, height: f32) {
    if let Some(index) = state.git.shown.filter(|_| state.shown_viewer().is_some()) {
        file_view(state, index, width, height);
    }
}

/// Emits the open file at `index`, in a view `width` by `height` logical pixels: the file shown, edited in place, or one shown in a pane other than the one being worked in, which a press on makes the file shown.
#[track_caller]
pub fn file_view(state: &WorkspaceState, index: usize, width: f32, height: f32) {
    let Some(viewer) = state.git.viewers.get(index) else {
        return;
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
    let source = !shows_rendering(viewer);
    let shown_text = match &viewer.contents {
        Contents::Text(document) if source => Some(document),
        _ => None,
    };
    let body = || match &viewer.contents {
        _ if !source => {
            crate::preview::rendered(viewer, &state.figures, (width, height));
        }
        Contents::Loading => note("Reading the file."),
        Contents::Message(message) => note(message),
        Contents::Picture(_) => {}
        Contents::Text(document) => {
            flex_column(
                Modifier::new()
                    .min_h(lines_height / 4.0)
                    .cursor(CursorIcon::Text)
                    .target(owner)
                    .pad_y(2),
                Flex::new(),
                || {
                    visible(
                        document,
                        viewer.scroll,
                        lines_height,
                        &Look {
                            found: &found,
                            blamed: &|line| state.blame_of_line(line).filter(|_| live),
                            settings: &pm_editor::EditorSettings {
                                git_gutter: state.shows_gutter(),
                                cursor_shape: modal_shape(state, document, live),
                                ..state.editor.clone()
                            },
                            columns: state.wrap_columns(),
                            owner,
                            file: index,
                            gutter: &state.gutter_of(index),
                            bulb: state.lightbulb_line().filter(|_| live),
                        },
                    );
                    if document.truncated {
                        flex_column(Modifier::new().pad_x(4).pad_y(2), Flex::new(), || {
                            line("The rest of the file is not shown.", 12, 1.4, Role::Subtle);
                        });
                    }
                },
            );
        }
    };
    flex_column(
        Modifier::new().grow().fill(Role::Background),
        Flex::new(),
        || {
            strip(viewer, live);
            crate::language_offer::offer(state, viewer);
            flex_row(
                Modifier::new().grow().clip_to(Radius::None),
                Flex::new(),
                || {
                    rustle::stack_with(Modifier::new().grow().clip_to(Radius::None), || {
                        scroll_area(
                            Modifier::new().fill_max_size(),
                            Scroll::from_start(scroller, viewer.scroll),
                            || flex_column(Modifier::new().fill_max_width(), Flex::new(), body),
                        );
                        if let Some(document) = shown_text {
                            crate::scroll_marks::scroll_marks(document, &found.0, &state.editor);
                            crate::sticky::sticky(
                                document,
                                viewer.scroll,
                                state.wrap_columns(),
                                &state.editor,
                            );
                            if let Some(finding) = state.finding.as_ref().filter(|_| live) {
                                crate::find::find_bar(finding, document, state.editor.smartcase);
                            }
                        }
                    });
                    if let Some(document) = shown_text
                        && state.editor.minimap
                        && width >= MINIMAP_ROOM
                    {
                        crate::minimap::minimap(
                            document,
                            viewer.scroll,
                            lines_height,
                            state.editor.row_height(),
                            state.wrap_columns(),
                            lines_height,
                        );
                    }
                },
            );
            if let Some(status) = state
                .vim_status()
                .filter(|_| live && source && matches!(viewer.contents, Contents::Text(_)))
            {
                vim_bar(&status);
            }
        },
    );
}

/// Emits a line saying `message` where the file's lines would be.
#[track_caller]
fn note(message: &str) {
    flex_column(Modifier::new().pad_x(4).pad_y(4), Flex::new(), || {
        line(message, 13, 1.4, Role::Subtle);
    });
}

/// How the caret of `document` is drawn: as the mode of modal editing says while it is on and the file is the one being edited, and as the reader chose otherwise.
fn modal_shape(state: &WorkspaceState, document: &Document, live: bool) -> pm_editor::CursorShape {
    use pm_editor::CursorShape;
    if !(live && state.vim_on()) {
        return state.editor.cursor_shape;
    }
    match document.vim_shape() {
        pm_vim::Shape::Block => CursorShape::Block,
        pm_vim::Shape::Underline => CursorShape::Underline,
        pm_vim::Shape::Typing => state.editor.cursor_shape,
    }
}

/// Emits the bar under the file that says which mode modal editing is in, the keys typed towards a command or the line being typed after `:` or `/`, and the register being recorded.
#[track_caller]
fn vim_bar(status: &crate::vim::VimStatus) {
    let kind = Type::new(12).ratio(1.4).mono();
    flex_row(
        Modifier::new()
            .h(6)
            .fill(Role::Surface)
            .stroke_side(Side::Top, 1.0, Role::Border)
            .pad_x(3),
        Flex::new().items_center().gap(sp(3)),
        || {
            label(status.mode, kind.tone(Role::Text));
            if let Some(pending) = &status.pending {
                label_with(
                    Modifier::new().grow(),
                    pending,
                    kind.tone(Role::Muted).truncate(),
                );
            }
            if let Some(register) = status.recording {
                label(format!("recording @{register}"), kind.tone(Role::Danger));
            }
        },
    );
}
