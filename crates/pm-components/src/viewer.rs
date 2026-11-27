//! A file open in the window: its lines, numbered, as they are in the folder or in the last commit, edited in place.

use crate::areas::TerminalEvent;
use crate::button;
use crate::events;
use crate::parts;
use crate::theme::Indicated;
use rustle::TextOverflow;

use crate::areas::EditorEvent;

use crate::events::{Event, send};
use crate::target::Target;
use crate::theme;
use crate::theme::sp;
use rustle::phosphor;
use rustle::{Alignment, Arrangement, Color, row_arranged};

use std::ops::Range;

use pm_protocol::{HintKind, Severity};
use pm_treesitter::{Highlight, Kind};
use rustle::{CursorIcon, Modifier};

use crate::button::Variant;

use crate::debug::{Dot, Gutter};
use crate::events::Interactive;
use crate::file_icon::file_icon;
use crate::marked::{CaretLook, CaretShape, Mark, Paint, marked_field_line};
use crate::open_files::{Contents, STRIP, Viewer};
use crate::parts::{glyph, line};
use crate::preview::Rendering;
use crate::state::WorkspaceState;
use crate::text_lines::TextLines;
use crate::theme::{Radius, Role};
use pm_editor::{Document, EditorSettings, Hint, Lens, LineChange, Swatch};

/// How wide a view of a file must be, in logical pixels, for a minimap to be drawn beside it.
const MINIMAP_ROOM: f32 = 420.0;

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
            (low < high).then(|| Mark::new(low - range.start..high - range.start, role, Paint::Ink))
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
                kept.push(Mark::new(piece, mark.tone, Paint::Ink));
            }
        }
    }
    kept.push(Mark::new(range, role, Paint::Ink));
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
    let on_line = |held: &Range<usize>| clipped(&Some(held.clone()), &line);
    let (matches, selected) = found;
    let mut found_marks: Vec<Mark> = matches
        .iter()
        .enumerate()
        .filter(|(_, held)| held.end >= line.start && held.start <= line.end)
        .filter_map(|(place, held)| {
            on_line(held).map(|range| match Some(place) == *selected {
                true => Mark::new(range, Role::Warning, Paint::Underline),
                false => Mark::new(range, Role::SurfaceActive, Paint::Fill),
            })
        })
        .collect();
    let mut marks: Vec<Mark> = spans
        .into_iter()
        .filter_map(|(range, kind)| Some(Mark::new(range, kind_role(kind)?, Paint::Ink)))
        .collect();
    marks.extend(
        clipped(&document.find_scope(), &line)
            .map(|range| Mark::new(range, Role::SurfaceHover, Paint::Fill)),
    );
    if settings.bracket_colors {
        for (range, depth) in document.brackets_on(index) {
            recolor(
                &mut marks,
                range,
                BRACKET_ROLES[depth % BRACKET_ROLES.len()],
            );
        }
    }
    if settings.redact_private_values {
        for range in document.redactions().iter().filter_map(on_line) {
            recolor(&mut marks, range.clone(), Role::Muted);
            marks.push(Mark::new(range, Role::Muted, Paint::Fill));
        }
    }
    marks.extend(
        pair.iter()
            .filter_map(on_line)
            .map(|range| Mark::new(range, Role::SurfaceActive, Paint::Fill)),
    );
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
            marks.push(Mark::new(range, Role::Focused, Paint::Selected));
        }
        if (line.start..=line.end).contains(&caret) {
            let at = caret - line.start;
            marks.push(Mark::new(at..at, Role::Text, Paint::Caret));
        }
    }
    marks.extend(
        document
            .occurrences()
            .iter()
            .filter_map(on_line)
            .map(|range| Mark::new(range, Role::SurfaceHover, Paint::Fill)),
    );
    marks.extend(
        document
            .drop_mark()
            .filter(|at| (line.start..=line.end).contains(at))
            .map(|at| Mark::new(at - line.start..at - line.start, Role::Accent, Paint::Drop)),
    );
    marks.extend(
        clipped(&document.reach, &line)
            .map(|range| Mark::new(range, Role::SyntaxFunction, Paint::Underline)),
    );
    marks.extend(
        document
            .diagnostics_on(index)
            .into_iter()
            .filter(|(_, diagnostic)| settings.diagnostics.shows(diagnostic.severity))
            .map(|(range, diagnostic)| {
                Mark::new(range, severity_role(diagnostic.severity), Paint::Wave)
            }),
    );
    marks
}

/// Emits the mark beside a line number saying how the line differs from the last commit: a bar for a line added or changed, and a short bar at its top where lines were taken out before it.
fn change_bar(change: Option<LineChange>, height: f32) {
    let holder = Modifier::new().width(3.0).height(height);
    let holder = match change {
        Some(LineChange::Added) => holder.background(theme::color(Role::Success)),
        Some(LineChange::Modified) => holder.background(theme::color(Role::Warning)),
        Some(LineChange::DeletedBefore) | None => holder,
    };
    rustle::column_arranged(holder, Arrangement::Start, Alignment::Start, move || {
        if matches!(change, Some(LineChange::DeletedBefore)) {
            rustle::spacer(
                Modifier::new()
                    .width(3.0)
                    .height(4.0)
                    .background(theme::color(Role::Danger)),
            );
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

/// Emits `content` centred in a cell of the gutter laid out by `cell`.
#[track_caller]
fn centred(cell: Modifier, content: impl FnOnce()) {
    row_arranged(cell, Arrangement::Center, Alignment::Center, content);
}

/// Emits the cell before the breakpoint of a line that runs the place that starts on the line when pressed, and is empty on the lines where none starts.
fn run_cell(file: usize, index: usize, gutter: &Gutter, height: f32) {
    let runs = gutter.runs.contains(&index);
    let cell = Modifier::new().width(RUN_CELL).height(height);
    let target = Target::Editor(EditorEvent::Run { file, line: index });
    centred(
        if runs {
            events::pressed(cell, target)
        } else {
            cell
        },
        move || {
            if runs {
                glyph(phosphor::PLAY, 3.5, Role::Success);
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
    rustle::row_arranged(
        Modifier::new().height(height),
        Arrangement::Start,
        Alignment::Start,
        || {
            run_cell(file, index, gutter, height);
            breakpoint_only(file, index, gutter, height);
        },
    );
}

/// Emits the cell before a line number that sets or clears the breakpoint on the line when pressed, and shows the breakpoint, or the arrow where the program is paused.
#[track_caller]
fn breakpoint_only(file: usize, index: usize, gutter: &Gutter, height: f32) {
    let cell = Modifier::new().width(BREAKPOINT_CELL).height(height);
    let target = Target::Editor(EditorEvent::Breakpoint { file, line: index });
    let shown = (gutter.dot(index), gutter.paused == Some(index));
    centred(events::pressed(cell, target), move || match shown {
        (Some(Dot::Plain), _) => dot(Role::Danger),
        (Some(Dot::Conditional), _) => dot(Role::Warning),
        (Some(Dot::Unplaced), _) => dot(Role::Subtle),
        (None, true) => glyph(phosphor::ARROW_RIGHT, 3.5, Role::Warning),
        (None, false) => {}
    });
}

/// Emits the round dot of a breakpoint in `role`.
#[track_caller]
fn dot(role: Role) {
    rustle::spacer(
        Modifier::new()
            .size(9.0, 9.0)
            .background_rounded(theme::color(role), Radius::Full.px()),
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

/// Emits one numbered line of the file, broken into the rows `segments` of it hold, with the caret and selection placed in it for the field `owner`, coloured by `marks`, each row telling the lines holder `drawn` where it was drawn.
#[track_caller]
pub(crate) fn file_line(
    document: &Document,
    index: usize,
    marks: Vec<Mark>,
    segments: Vec<Range<usize>>,
    beside: Beside<impl FnOnce()>,
    settings: &EditorSettings,
    (owner, drawn): (Target, &TextLines),
) {
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
    let own = Modifier::new().fill_max_width().height(total);
    let own = if beside.paused {
        own.background(theme::color(Role::SurfaceActive))
    } else if settings.current_line_highlight
        && index == caret_line
        && document.field.selection().is_none()
    {
        own.background(theme::color(Role::Surface))
    } else {
        own
    };
    let number_tone = if index == caret_line {
        Role::Muted
    } else {
        Role::Subtle
    };
    let number_box = Modifier::new().width(theme::sp(if settings.line_numbers { 13 } else { 2 }));
    rustle::row_arranged(own.clip(), Arrangement::Start, Alignment::Start, || {
        (beside.breakpoint)();
        change_bar(
            document.line_change(index).filter(|_| settings.git_gutter),
            total,
        );
        rustle::column_arranged(
            number_box.padding_each(0.0, 0.0, theme::sp(3), 0.0),
            Arrangement::Start,
            Alignment::Start,
            || {
                if lens_row {
                    rustle::spacer(Modifier::new().height(height));
                }
                row_arranged(
                    Modifier::new().fill_max_width().height(height),
                    Arrangement::End,
                    Alignment::Center,
                    || {
                        if beside.bulb && settings.line_numbers {
                            bulb_button(height);
                        }
                        if settings.line_numbers {
                            rustle::basic_text(
                                Modifier::new(),
                                number.to_string(),
                                theme::code_style(font, 1.4)
                                    .color(theme::color(worst.map_or(number_tone, severity_role))),
                                TextOverflow::Ellipsis,
                            );
                        }
                    },
                );
            },
        );
        rustle::column_arranged(
            Modifier::new()
                .weight(1.0)
                .clip()
                .padding_xy(theme::sp(2), 0.0),
            Arrangement::Start,
            Alignment::Start,
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
                    segment_row(
                        document,
                        &at,
                        &hints,
                        &marks,
                        settings,
                        (owner, drawn),
                        || {
                            if place < last {
                                return;
                            }
                            if document.is_folded_at(index) {
                                fold_chip(index);
                            }
                            if let Some(said) = beside.blamed.clone() {
                                rustle::basic_text(
                                    Modifier::new()
                                        .padding_each(theme::sp(8), 0.0, 0.0, 0.0)
                                        .weight(1.0),
                                    said,
                                    theme::line(font, Role::Subtle),
                                    TextOverflow::Ellipsis,
                                );
                            }
                        },
                    );
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

/// Emits one row of a line, `at` saying where it lies: its text cut into pieces at the inlay `hints` that stand in it, each piece coloured by the `marks` of the line and holding the caret and selection of the field `owner` while that field has focus and telling the lines holder `drawn` where it was drawn, with the hint after each piece, then what `tail` emits.
fn segment_row(
    document: &Document,
    at: &Segment,
    hints: &[&Hint],
    marks: &[Mark],
    settings: &EditorSettings,
    (owner, drawn): (Target, &TextLines),
    tail: impl FnOnce(),
) {
    let (line, segment, first, last) = (at.line, at.segment, at.first, at.last);
    let text_of = &document.field.text()[line.clone()];
    let caret = document.field.caret();
    let font = settings.font_size;
    let height = settings.row_height();
    let focused = crate::focus::is_focused(owner);
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
    rustle::row_arranged(
        Modifier::new().fill_max_width().height(height),
        Arrangement::Start,
        Alignment::Center,
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
                    drawn,
                    (owner, piece_absolute.start),
                    &text_of[piece.clone()],
                    theme::code_style(font, 1.4).color(theme::color(Role::Text)),
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
fn run_lens_row(lenses: &[(usize, String)], given: &[Lens], height: f32, font: u32) {
    rustle::row_arranged(
        Modifier::new().height(height),
        Arrangement::Start,
        Alignment::Center,
        || {
            for (place, (index, label)) in lenses.iter().enumerate() {
                let target = Target::Editor(EditorEvent::RunLens(*index));
                lens_chip(label, Role::Muted, font, place == 0, Some(target));
            }
            for (place, lens) in given.iter().enumerate() {
                let target = Target::Editor(EditorEvent::Lens(lens.index));
                let first = lenses.is_empty() && place == 0;
                lens_chip(
                    &lens.title,
                    Role::Subtle,
                    font,
                    first,
                    lens.pressable.then_some(target),
                );
            }
        },
    );
}

/// Emits the marker that says the language server offers changes on the line, which opens the list of them when pressed.
fn bulb_button(height: f32) {
    centred(
        Modifier::new()
            .width(16.0)
            .height(height)
            .cursor(CursorIcon::Pointer)
            .control(Target::Editor(EditorEvent::Lightbulb), {
                let operations = crate::events::operations();
                move || operations.show_code_actions()
            }),
        || glyph(phosphor::LIGHTBULB, 3.5, Role::Warning),
    );
}

/// Emits the text of `hint` written inside a line in a smaller, quieter hand than the code around it, or the swatch of the colour it stands for, pressed to pick another.
fn hint_chip(hint: &Hint, font: u32) {
    if let Some(Swatch {
        color: [red, green, blue, alpha],
        index,
    }) = hint.swatch
    {
        let side = (font as f32 * 0.8).max(8.0);
        rustle::row_arranged(
            Modifier::new()
                .cursor(CursorIcon::Pointer)
                .control(Target::Editor(EditorEvent::ColorSwatch(index)), {
                    let operations = crate::events::operations();
                    move || operations.open_color_picker(index)
                })
                .padding_each(0.0, 0.0, theme::sp(1.0), 0.0),
            Arrangement::Start,
            Alignment::Center,
            || {
                rustle::spacer(Modifier::new().size(side, side).background_rounded(
                    theme::color(Color::rgba(red, green, blue, alpha)),
                    Radius::Sm.px(),
                ));
            },
        );
        return;
    }
    let tone = match hint.kind {
        HintKind::Type | HintKind::Value => Role::Subtle,
        HintKind::Parameter | HintKind::Other => Role::Muted,
    };
    let left = if hint.pad_left { theme::sp(1) } else { 0.0 };
    rustle::row_arranged(
        Modifier::new()
            .padding_each(left, 0.0, 0.0, 0.0)
            .background_rounded(theme::color(Role::Surface), Radius::Sm.px())
            .padding_xy(theme::sp(1.0), 0.0),
        Arrangement::Start,
        Alignment::Center,
        || {
            rustle::basic_text(
                Modifier::new(),
                &hint.label,
                theme::mono_style((font - 1).max(8), 1.2).color(theme::color(tone)),
                TextOverflow::Ellipsis,
            );
        },
    );
}

/// The look of a pressable chip in or above a line: a small rounded fill under the pointer.
fn chip_tones() -> theme::InteractionTones {
    theme::InteractionTones::new()
        .hover(Role::SurfaceHover)
        .radius(Radius::Sm)
}

/// Emits `label` in `role` on the row above a line, as a chip with no room at its left when it is the `first`, which sends the press of `press` when it is given one.
#[track_caller]
fn lens_chip(label: &str, role: Role, font: u32, first: bool, press: Option<Target>) {
    let own = Modifier::new().height(theme::sp(4));
    let own = match press {
        Some(id) => events::pressed(own.indicated(chip_tones(), theme::Focused::Rustle), id),
        None => own,
    };
    let left = theme::sp(if first { 0.0 } else { 1.5 });
    rustle::row_arranged(
        own.padding_each(left, 0.0, theme::sp(1.5), 0.0),
        Arrangement::Start,
        Alignment::Center,
        || {
            rustle::basic_text(
                Modifier::new(),
                label,
                theme::text_style((font - 1).max(8), 1.2).color(theme::color(role)),
                TextOverflow::Ellipsis,
            );
        },
    );
}

/// Emits the mark after the line at `index` standing for the lines folded away after it, which unfolds them when pressed.
fn fold_chip(index: usize) {
    let own = Modifier::new()
        .padding_each(theme::sp(2), 0.0, 0.0, 0.0)
        .height(theme::sp(4))
        .indicated(
            chip_tones().idle(Role::SurfaceActive),
            theme::Focused::Rustle,
        );
    rustle::row_arranged(
        events::pressed(own, Target::Editor(EditorEvent::CodeFold(index)))
            .padding_xy(theme::sp(1.5), 0.0),
        Arrangement::Start,
        Alignment::Center,
        || line("⋯", 11, 1.2, Role::Muted),
    );
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
    let at = |index: usize| indent_columns(document, index);
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
    owner: Target,
    /// Where the lines tell they were drawn.
    drawn: &'a TextLines,
    /// The open file the lines are of, which names its marks.
    file: usize,
    /// The breakpoints and the paused line of the file.
    gutter: &'a Gutter,
    /// The line the marker for the changes the language server offers stands beside.
    bulb: Option<usize>,
}

/// Emits the line at `index` of `document` as an item of the list of its lines, drawn as `look` says, with the brackets `pair` paired at the caret behind a box.
fn shown_line(document: &Document, index: usize, pair: &[Range<usize>], look: &Look) {
    let settings = look.settings;
    let spans = document
        .highlights(index..index + 1)
        .pop()
        .unwrap_or_default();
    let mut standing = if settings.indent_guides {
        guides(document, index..index + 1).pop().unwrap_or_default()
    } else {
        Vec::new()
    };
    if settings.wrap_guide {
        standing.push(settings.preferred_line_length);
    }
    let mut marks = line_marks(document, index, spans, pair, look.found, settings);
    marks.extend(
        standing
            .into_iter()
            .map(|column| Mark::new(column..column, Role::Border, Paint::Guide)),
    );
    let segments = document.line_segments(index, look.columns);
    let cell_height = document_row(settings, &segments);
    file_line(
        document,
        index,
        marks,
        segments,
        Beside {
            blamed: (look.blamed)(index),
            breakpoint: || breakpoint_cell(look.file, index, look.gutter, cell_height),
            bulb: look.bulb == Some(index),
            paused: look.gutter.paused == Some(index),
        },
        settings,
        (look.owner, look.drawn),
    );
}

/// The space above the first line of a file and below its last, in logical pixels.
fn line_padding() -> f32 {
    theme::sp(2)
}

/// The row of the list of a file's lines at `offset` along it, rows `row` logical pixels tall,
/// counting the part of it scrolled out: worked out from the offset alone, so it is exact
/// wherever the list is scrolled, whether or not the lines there are composed yet.
fn list_row(offset: f32, row: f32) -> f32 {
    ((offset - line_padding()) / row.max(1.0)).max(0.0)
}

/// How often the work on a text under way away from the thread that draws is looked in on.
const SETTLE_POLL: std::time::Duration = std::time::Duration::from_millis(8);

/// Composes the caller again once the work on `document` under way away from the thread that draws, parsing it again and comparing it with the last commit, is done, so that what it found is drawn; what was known before is drawn meanwhile.
#[track_caller]
fn settle(document: &Document) {
    let pending = document.pending();
    let settled = rustle::remember(|| 0_u64);
    let bumped = settled.clone();
    rustle::launched_effect(
        pending.as_ref().map(pm_editor::Pending::id),
        move |scope| async move {
            let Some(pending) = pending else {
                return;
            };
            while !pending.is_settled() {
                scope.delay(SETTLE_POLL).await;
            }
            bumped.set(bumped.get().wrapping_add(1));
        },
    );
    settled.get();
}

/// How many rows the note that the rest of a file cut short is not shown takes, below its last line.
const NOTE_ROWS: usize = 2;

/// The rows the items of the list of a file's lines take: each line that shows takes the rows it wraps into, with the row of links above it, as the wrap index of the document counts them, and the note of a file cut short takes [`NOTE_ROWS`] after them.
struct DocumentRows<'a> {
    /// The file the lines are of.
    document: &'a Document,
    /// How many lines show.
    shown: usize,
    /// How many rows come before each line, with one more for the end of the text, when lines do not take one row each.
    starts: Option<std::sync::Arc<Vec<usize>>>,
    /// Whether the list ends with the note of a file cut short.
    note: bool,
}

impl<'a> DocumentRows<'a> {
    /// The rows of the lines of `document` when rows hold `columns` characters as lines wrap.
    fn new(document: &'a Document, columns: Option<usize>) -> Self {
        Self {
            document,
            shown: document.rows(),
            starts: document.row_starts(columns),
            note: document.truncated,
        }
    }

    /// How many rows the lines that show take.
    fn text_rows(&self) -> usize {
        self.starts
            .as_ref()
            .and_then(|starts| starts.last().copied())
            .unwrap_or(self.shown)
    }
}

impl rustle::UniformRows for DocumentRows<'_> {
    /// The lines that show and the note of a file cut short.
    fn items(&self) -> usize {
        self.shown + usize::from(self.note)
    }

    /// The rows of the lines that show and of the note.
    fn rows(&self) -> usize {
        self.text_rows() + if self.note { NOTE_ROWS } else { 0 }
    }

    /// The first row of the line that shows at `item`, or of the note after them.
    fn first_row(&self, item: usize) -> usize {
        if item >= self.items() {
            return self.rows();
        }
        if item >= self.shown {
            return self.text_rows();
        }
        match (&self.starts, self.document.shown_line(item)) {
            (Some(starts), Some(line)) => starts.get(line).copied().unwrap_or(self.text_rows()),
            _ => item,
        }
    }

    /// The item of the line drawn in `row`, or of the note.
    fn item_at_row(&self, row: usize) -> usize {
        if row >= self.rows() {
            return self.items();
        }
        if row >= self.text_rows() {
            return self.shown;
        }
        match &self.starts {
            Some(starts) => {
                let lines = starts.len().saturating_sub(1);
                let line = starts[1..=lines].partition_point(|end| *end <= row);
                self.document.row_of_line(line).min(self.shown)
            }
            None => row,
        }
    }

    /// Whether every line takes one row and no note follows them.
    fn one_per_row(&self) -> bool {
        self.starts.is_none() && !self.note
    }
}

/// Emits the lines of `document` that show as a lazy list with `modifier`, scrolled by `scroll` in a view `height` tall, each drawn as `look` says, followed by a note when the file was cut short.
///
/// Every row is as tall as the settings say, so the list finds the lines in view by arithmetic on the rows each line takes, without measuring a line.
#[track_caller]
fn lines(
    document: &Document,
    scroll: &rustle::LazyListState,
    modifier: Modifier,
    look: &Look,
    height: f32,
) {
    let settings = look.settings;
    settle(document);
    let pair: Vec<Range<usize>> = document
        .syntax
        .as_ref()
        .and_then(|syntax| syntax.bracket_pair(document.field.text(), document.field.caret()))
        .map(|(open, close)| vec![open, close])
        .unwrap_or_default();
    let beyond = if settings.scroll_beyond_last_line {
        (height - settings.row_height() * 2.0).max(0.0)
    } else {
        0.0
    };
    rustle::uniform_list_with(
        modifier,
        scroll,
        rustle::LazyContent::new().padding_each(line_padding(), line_padding() + beyond),
        settings.row_height(),
        &DocumentRows::new(document, look.columns),
        |item| document.shown_line(item).unwrap_or(usize::MAX),
        |item| match document.shown_line(item) {
            Some(index) => shown_line(document, index, &pair, look),
            None => parts::note(4, 2, "The rest of the file is not shown.", 12, Role::Subtle),
        },
    );
}

/// Scrolls the list `scroll` of the lines of `document` so that `target`, a row counted from its first, is at the top of its view, when lines hold `columns` characters as they wrap and rows are `row` logical pixels tall; a row before the first scrolls to the very top.
pub(crate) fn scroll_document_to_row(
    scroll: &rustle::LazyListState,
    document: &Document,
    target: f32,
    columns: Option<usize>,
    row: f32,
) {
    if target <= 0.0 {
        scroll.scroll_to_item_offset(0, -line_padding());
        return;
    }
    let whole = target.floor() as usize;
    let (shown, start) = document.wrapped_lines(whole, 1, columns);
    match shown.first() {
        Some(&line) => {
            scroll.scroll_to_item_offset(document.row_of_line(line), (target - start as f32) * row)
        }
        None => scroll.scroll_to_end(),
    }
}

impl Viewer {
    /// The row of the file at the top of its view, counting the part of it scrolled out, when its lines hold `columns` characters as they wrap and rows are `row` logical pixels tall; for a file not shown as text, how far it is scrolled in rows.
    pub fn top_row(&self, columns: Option<usize>, row: f32) -> f32 {
        let row = row.max(1.0);
        let (Some(document), false) = (self.document(), shows_rendering(self)) else {
            return self.scroll.scroll().offset() / row;
        };
        let rows = DocumentRows::new(document, columns);
        let along = list_row(self.scroll.scroll().offset(), row);
        let item = rustle::UniformRows::item_at_row(&rows, along.floor() as usize);
        match document.shown_lines(item, 1).first() {
            Some(&line) => {
                document.wrapped_row_of_line(line, columns) as f32 + along
                    - rustle::UniformRows::first_row(&rows, item) as f32
            }
            None => document.wrapped_rows(columns) as f32,
        }
    }

    /// Scrolls the file so that `target`, a row counted from its first, is at the top of its view, when its lines hold `columns` characters as they wrap and rows are `row` logical pixels tall; a row before the first scrolls to the very top. A file not read yet takes `target` as a line; one shown rendered scrolls `target` rows of pixels.
    pub fn scroll_to_row(&self, target: f32, columns: Option<usize>, row: f32) {
        let whole = target.max(0.0).floor() as usize;
        let within = (target.max(0.0) - whole as f32) * row;
        let document = match &self.contents {
            Contents::Text(document) if !shows_rendering(self) => document,
            Contents::Loading => {
                self.scroll.scroll_to_item_offset(whole, within);
                return;
            }
            _ => {
                self.scroll.scroll().jump_to(target.max(0.0) * row);
                return;
            }
        };
        scroll_document_to_row(&self.scroll, document, target, columns, row);
    }
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
    rustle::row_arranged(
        events::pressed(
            Modifier::new(),
            Target::Terminal(TerminalEvent::BottomProblemsCount),
        ),
        Arrangement::SpacedBy(theme::sp(2)),
        Alignment::Center,
        move || {
            for (said, role) in words {
                line(said, 11, 1.4, role);
            }
        },
    );
}

/// Emits `words` in `style`, as wide as they need when there is room for them and given up from the end first when there is not.
#[track_caller]
fn yielding_text(words: &str, style: rustle::TextStyle) {
    let wanted = rustle::layout_text(words, &style, None).size().width;
    rustle::basic_text(
        Modifier::new().weight_with(wanted, false),
        words,
        style,
        TextOverflow::Ellipsis,
    );
}

/// Emits the strip above the file: its icon and name, and, while it is the file shown, `live`, the switch between its source and its rendering and the button that writes it when it has changes to write.
#[track_caller]
fn strip(viewer: &Viewer, live: bool) {
    let crumbs = match &viewer.contents {
        Contents::Text(document) => breadcrumbs(document),
        _ => None,
    };
    parts::bar(10, 4, 2, || {
        rustle::row_arranged(
            Modifier::new().weight(1.0),
            Arrangement::SpacedBy(theme::sp(2)),
            Alignment::Center,
            || {
                rustle::stack_with(Modifier::new().weight_with(sp(4), false).clip(), || {
                    file_icon(&viewer.path, 4.0)
                });
                yielding_text(&viewer.path, theme::code(13, Role::Text));
                if viewer.head {
                    line("HEAD", 11, 1.4, Role::Muted);
                }
                if let Contents::Text(document) = &viewer.contents
                    && document.field.is_read_only()
                    && !viewer.head
                {
                    line("Read only", 11, 1.4, Role::Muted);
                }
                if let Some(crumbs) = crumbs {
                    yielding_text(&crumbs, theme::code(12, Role::Muted));
                }
            },
        );
        rustle::spacer(Modifier::new());
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
            button::action(
                "Save",
                Target::Editor(EditorEvent::EditorSave),
                Variant::Secondary,
            );
        }
    });
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

/// The row of `viewer`'s file at the top of its view, as [`Viewer::top_row`] says, read so that
/// the view composes again only when the scroll crosses a row, not on every pixel it moves:
/// the sticky lines and the minimap follow it by rows.
#[track_caller]
fn top_in_rows(viewer: &Viewer, columns: Option<usize>, row: f32) -> f32 {
    let scroll = viewer.scroll.scroll().clone();
    let rows = row.max(1.0);
    rustle::watched_state_of(move || list_row(scroll.offset(), rows).floor() as usize).get();
    rustle::untracked(|| viewer.top_row(columns, row))
}

/// Emits the open file at `index`, in a view `width` by `height` logical pixels: the file shown, edited in place, or one shown in a pane other than the one being worked in, which a press on makes the file shown.
#[track_caller]
pub fn file_view(state: &WorkspaceState, index: usize, width: f32, height: f32) {
    let Some(viewer) = state.git.viewers.get(index) else {
        return;
    };
    let live = state.shown_viewer().is_some() && state.git.shown == Some(index);
    let owner = if live {
        Target::Editor(EditorEvent::EditorField)
    } else {
        Target::Editor(EditorEvent::FileTab(index))
    };
    let found = if live {
        state.find_marks()
    } else {
        (Vec::new(), None)
    };
    let lines_height = (height - STRIP).max(0.0);
    let source = !shows_rendering(viewer);
    let shown_text = viewer.document().filter(|_| source);
    let (columns, row) = (state.wrap_columns(), state.editor.row_height());
    let top = top_in_rows(viewer, columns, row);
    let settings = pm_editor::EditorSettings {
        git_gutter: state.shows_gutter(),
        cursor_shape: shown_text.map_or(state.editor.cursor_shape, |document| {
            modal_shape(state, document, live)
        }),
        ..state.editor.clone()
    };
    let gutter = state.gutter_of(index);
    let blamed = |line: usize| state.blame_of_line(line).filter(|_| live);
    let look = Look {
        found: &found,
        blamed: &blamed,
        settings: &settings,
        columns,
        owner,
        drawn: &state.code_lines,
        file: index,
        gutter: &gutter,
        bulb: state.lightbulb_line().filter(|_| live),
    };
    let drawn = &state.code_lines;
    let reaching = live && shown_text.is_some_and(|document| document.reach.is_some());
    let own = Modifier::new()
        .cursor(if reaching {
            CursorIcon::Pointer
        } else {
            CursorIcon::Text
        })
        .chain(if owner == Target::Editor(EditorEvent::EditorField) {
            Modifier::new()
        } else {
            crate::focus::focus_target(owner)
        });
    let on_context = {
        let operations = crate::events::operations();
        let drawn = drawn.clone();
        move |at| {
            if live {
                operations.open_field_menu(owner, at, drawn.offset_at_window(owner, at));
            } else {
                operations.open_tab_menu(index, at);
            }
        }
    };
    let pressed = if live {
        let on_press = crate::code_pointing::on_press(owner, drawn);
        let carrying = match owner == Target::Editor(EditorEvent::EditorField) {
            true => crate::code_pointing::text_drag(owner, drawn),
            false => Modifier::new(),
        };
        own.chain(crate::code_pointing::hover(owner, drawn))
            .chain(crate::code_pointing::held(owner, drawn))
            .pressable_pointing(
                owner,
                send(Event::pressing(owner)),
                on_context,
                move |event| on_press(event),
            )
            .chain(carrying)
    } else {
        own.pressable_menu(owner, send(Event::pressing(owner)), on_context)
    };
    let body = || match &viewer.contents {
        _ if !source => {
            crate::preview::rendered(viewer, &state.figures, (width, height));
        }
        Contents::Loading => crate::preview::note("Reading the file."),
        Contents::Message(message) => crate::preview::note(message),
        Contents::Picture(_) | Contents::Text(_) => {}
    };
    rustle::column_arranged(
        Modifier::new()
            .weight(1.0)
            .background(theme::color(Role::Background)),
        Arrangement::Start,
        Alignment::Start,
        || {
            strip(viewer, live);
            crate::language_offer::offer(state, viewer);
            rustle::row_arranged(
                Modifier::new().fill_max_width().weight(1.0).clip(),
                Arrangement::Start,
                Alignment::Start,
                || {
                    rustle::stack_with(
                        Modifier::new().fill_max_height().weight(1.0).clip(),
                        || {
                            let area = Modifier::new().fill_max_size();
                            let area = match live {
                                true => state.code_size.track(
                                    state
                                        .code_bounds
                                        .track(area.chain(crate::focus::keyboard_focus(owner))),
                                ),
                                false => area,
                            };
                            match shown_text {
                                Some(document) => lines(
                                    document,
                                    &viewer.scroll,
                                    area.chain(pressed.clone()),
                                    &look,
                                    lines_height,
                                ),
                                None => rustle::column_arranged(
                                    area.vertical_scroll(viewer.scroll.scroll())
                                        .fill_max_width(),
                                    Arrangement::Start,
                                    Alignment::Start,
                                    body,
                                ),
                            }
                            if let Some(document) = shown_text {
                                crate::scroll_marks::scroll_marks(
                                    document,
                                    &found.0,
                                    &state.editor,
                                );
                                crate::sticky::sticky(document, top, columns, &state.editor);
                                if let Some(finding) = state.finding.as_ref().filter(|_| live) {
                                    crate::find::find_bar(
                                        finding,
                                        document,
                                        state.editor.smartcase,
                                    );
                                }
                            }
                        },
                    );
                    if let Some(document) = shown_text
                        && state.editor.minimap
                        && width >= MINIMAP_ROOM
                    {
                        crate::minimap::minimap(
                            document,
                            top,
                            lines_height,
                            row,
                            columns,
                            lines_height,
                        );
                    }
                },
            );
            if let Some(status) = state.vim_status().filter(|_| live && shown_text.is_some()) {
                vim_bar(&status);
            }
        },
    );
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
fn vim_bar(status: &crate::vim::VimStatus) {
    let kind = theme::mono_style(12, 1.4);
    rustle::column_arranged(
        Modifier::new()
            .fill_max_width()
            .height(theme::sp(6))
            .background(theme::color(Role::Surface)),
        Arrangement::Start,
        Alignment::Start,
        || {
            rustle::horizontal_divider(Modifier::new(), 1.0, theme::color(Role::Border));
            rustle::row_arranged(
                Modifier::new()
                    .fill_max_width()
                    .weight(1.0)
                    .padding_xy(theme::sp(3), 0.0),
                Arrangement::SpacedBy(theme::sp(3)),
                Alignment::Center,
                || {
                    rustle::basic_text(
                        Modifier::new(),
                        status.mode,
                        kind.color(theme::color(Role::Text)),
                        TextOverflow::Ellipsis,
                    );
                    if let Some(pending) = &status.pending {
                        rustle::basic_text(
                            Modifier::new().weight(1.0),
                            pending,
                            theme::code(12, Role::Muted),
                            TextOverflow::Ellipsis,
                        );
                    }
                    if let Some(register) = status.recording {
                        let said = format!("recording @{register}");
                        rustle::basic_text(
                            Modifier::new(),
                            said,
                            kind.color(theme::color(Role::Danger)),
                            TextOverflow::Ellipsis,
                        );
                    }
                },
            );
        },
    );
}
