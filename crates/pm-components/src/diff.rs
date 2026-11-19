//! The diff view: every changed file stacked, the old and the new side by side, with notes on lines for the agent.

use crate::events::reduce;
use crate::theme;
use crate::theme::sp;
use rustle::{Alignment, Arrangement, column_arranged, row_arranged};
use rustle::{CursorIcon, Modifier};

use crate::events::Interactive;
use crate::focus::FocusId;
use crate::icons::IconName;
use crate::icons::icon;
use crate::marked::{CaretLook, Mark, marked_line};
use crate::select::styled_text;
use crate::theme::{Radius, Role, Tone};

use crate::button::Variant;
use crate::diff_syntax::{DiffColours, shown};
use crate::file_icon::{file_icon, file_language};
use crate::parts::{line, spacer, wrap};
use crate::sample::{Change, Comment, DiffLine, Kind, Range, Session};
use crate::size::Size;
use crate::state::{
    COMMENT_ADD, COMMENT_CANCEL, COMMENT_FIELD, COMMENT_FILE_BASE, COMMENT_REMOVE_BASE,
    CommentTarget, DIFF_FOLD_BASE, SEND_COMMENTS, VIEWED_BASE, WorkspaceState,
};
use crate::{Button, Input, Tally};

/// How tall a line of a diff is, in logical pixels.
const ROW: f32 = 20.0;

/// How many pixels one logical pixel of the sizes below comes to: the rows, headers and notes are set in steps that follow the interface size, while the view and the scroll are counted in pixels.
fn pixels() -> f32 {
    crate::theme::step_scale()
}

/// The width of what is beside a pair of columns of code: the margins and rules of a file, in logical pixels.
const SECTION_SIDES: f32 = 34.0;

/// The width of a line number and the padding around the code beside it, in logical pixels.
const GUTTER_AND_PADDING: f32 = 44.0 + 24.0 + 1.0;

/// How tall the type of code is, in logical pixels.
const CODE_PIXELS: f32 = 12.0;

/// How wide a character of code is as a share of how tall the type is, on the wide side so that a line wraps before it would be cut.
const CODE_CHARACTER: f32 = 0.6;

/// How tall the header of a file is, in logical pixels.
const HEADER: f32 = 40.0;

/// The space above each file and the rules around it, in logical pixels.
const SECTION_CHROME: f32 = 16.0 + 2.0;

/// How tall what is above and below the diff in its column is at most: the strip of tabs and the summary, in logical pixels.
const CHROME: f32 = 88.0;

/// How far beyond the view, above and below, rows are still built, so that scrolling shows no gaps, in logical pixels.
const OVERSCAN: f32 = 100.0;

/// About how tall a note is before its lines of text, in logical pixels.
const NOTE_CHROME: f32 = 16.0 + 2.0 + 33.0 + 20.0;

/// How tall a line of text of a note is, in logical pixels.
const NOTE_LINE: f32 = 20.0;

/// About how tall the box for writing a note is, in logical pixels.
const NOTE_BOX: f32 = 160.0;

/// The first of the numbers of lines, which no other button or row has; each is named by the file, the side and the line.
const GUTTER_BASE: u64 = 10_000_000_000;

/// How many lines of a file a side of it can number.
const LINES: u64 = 500_000;

/// How many changes can have their lines numbered, keeping the numbers clear of the names past them.
const CHANGES: u64 = 10_000;

/// The name of the number of line `line` of the change at `change`, on the new side when `new`.
pub fn gutter_id(change: usize, new: bool, line: usize) -> FocusId {
    FocusId(GUTTER_BASE + change as u64 * 2 * LINES + u64::from(new) * LINES + line as u64)
}

/// Which change, side and line the number named `id` is, if it is one.
pub fn gutter_of(id: FocusId) -> Option<(usize, bool, usize)> {
    let offset =
        id.0.checked_sub(GUTTER_BASE)
            .filter(|offset| *offset < CHANGES * 2 * LINES)?;
    let side = offset / LINES;
    Some((
        (side / 2) as usize,
        side % 2 == 1,
        (offset % LINES) as usize,
    ))
}

/// A line of a diff with the number it has before the change and after it.
#[derive(Clone, Copy)]
struct Numbered<'a> {
    /// The line.
    line: &'a DiffLine,
    /// Where it is in the diff.
    at: usize,
    /// Its number in the old file, if it is in it.
    old: Option<usize>,
    /// Its number in the new file, if it is in it.
    new: Option<usize>,
}

/// The numbers `header`, a hunk header, says its first line has before and after the change.
fn starts(header: &str) -> Option<(usize, usize)> {
    let mut parts = header.split_whitespace().skip(1);
    let number = |part: &str| part[1..].split(',').next()?.parse().ok();
    Some((number(parts.next()?)?, number(parts.next()?)?))
}

/// The lines of `diff` with the numbers they have on each side, read out of the hunk headers.
fn numbered(diff: &[DiffLine]) -> Vec<Numbered<'_>> {
    let (mut old, mut new) = (1, 1);
    diff.iter()
        .enumerate()
        .map(|(at, line)| match line.kind {
            Kind::Hunk => {
                (old, new) = starts(&line.text).unwrap_or((old, new));
                Numbered {
                    line,
                    at,
                    old: None,
                    new: None,
                }
            }
            Kind::Add => {
                new += 1;
                Numbered {
                    line,
                    at,
                    old: None,
                    new: Some(new - 1),
                }
            }
            Kind::Remove => {
                old += 1;
                Numbered {
                    line,
                    at,
                    old: Some(old - 1),
                    new: None,
                }
            }
            Kind::Context => {
                old += 1;
                new += 1;
                Numbered {
                    line,
                    at,
                    old: Some(old - 1),
                    new: Some(new - 1),
                }
            }
        })
        .collect()
}

/// One row of a diff as it is shown.
enum Row<'a> {
    /// The header of a stretch of changes.
    Hunk(&'a str),
    /// The old line and the new line that stand side by side, either of which may be missing.
    Pair(Option<Numbered<'a>>, Option<Numbered<'a>>),
}

/// Moves the runs of removed and added lines into `rows`, the first of each beside the first of the other, leaving a blank where one run is shorter.
fn pair_runs<'a>(
    rows: &mut Vec<Row<'a>>,
    removed: &mut Vec<Numbered<'a>>,
    added: &mut Vec<Numbered<'a>>,
) {
    let mut old = std::mem::take(removed).into_iter();
    let mut new = std::mem::take(added).into_iter();
    loop {
        match (old.next(), new.next()) {
            (None, None) => break,
            (old, new) => rows.push(Row::Pair(old, new)),
        }
    }
}

/// The rows of `change`: the removed and added runs lined up beside each other, context on both sides.
fn rows(change: &Change) -> Vec<Row<'_>> {
    let mut rows = Vec::new();
    let mut removed: Vec<Numbered> = Vec::new();
    let mut added: Vec<Numbered> = Vec::new();
    for entry in numbered(&change.diff) {
        match entry.line.kind {
            Kind::Hunk => {
                pair_runs(&mut rows, &mut removed, &mut added);
                rows.push(Row::Hunk(&entry.line.text));
            }
            Kind::Remove => removed.push(entry),
            Kind::Add => added.push(entry),
            Kind::Context => {
                pair_runs(&mut rows, &mut removed, &mut added);
                rows.push(Row::Pair(Some(entry), Some(entry)));
            }
        }
    }
    pair_runs(&mut rows, &mut removed, &mut added);
    rows
}

/// How many characters of code one side of a row holds on a line, in a view `width` wide.
fn columns(width: f32) -> usize {
    let side = (width - SECTION_SIDES * pixels()) / 2.0 - GUTTER_AND_PADDING * pixels();
    let character = CODE_PIXELS * crate::theme::text_scale() * CODE_CHARACTER;
    ((side / character) as usize).max(8)
}

/// How many lines `content` takes when each holds `columns` characters, a tab counting four.
fn lines_of(content: &str, columns: usize) -> usize {
    let width: usize = content
        .chars()
        .map(|character| if character == '\t' { 4 } else { 1 })
        .sum();
    width.div_ceil(columns).max(1)
}

/// The stretches of `shown`, text as it is shown, that each fill one line of `columns` characters.
fn pieces(shown: &str, columns: usize) -> Vec<std::ops::Range<usize>> {
    let mut ranges = Vec::new();
    let (mut start, mut count) = (0, 0);
    for (at, _) in shown.char_indices() {
        if count == columns {
            ranges.push(start..at);
            start = at;
            count = 0;
        }
        count += 1;
    }
    ranges.push(start..shown.len());
    ranges
}

/// How many lines the rows made of the runs of removed and added lines, whose lines each take the numbers of lines in `removed` and `added`, come to when they stand side by side.
fn paired(removed: &[usize], added: &[usize]) -> usize {
    (0..removed.len().max(added.len()))
        .map(|at| {
            let side = |run: &[usize]| run.get(at).copied().unwrap_or(1);
            side(removed).max(side(added))
        })
        .sum()
}

/// How many lines of `columns` characters the rows of `change` come to, counted without building them.
fn row_lines(change: &Change, columns: usize) -> usize {
    if !change.loaded {
        return change.expected_rows();
    }
    let (mut total, mut removed, mut added) = (0, Vec::new(), Vec::new());
    for line in &change.diff {
        match line.kind {
            Kind::Remove => removed.push(lines_of(&line.text, columns)),
            Kind::Add => added.push(lines_of(&line.text, columns)),
            Kind::Hunk | Kind::Context => {
                total += paired(&removed, &added);
                removed.clear();
                added.clear();
                total += if line.kind == Kind::Hunk {
                    1
                } else {
                    lines_of(&line.text, columns)
                };
            }
        }
    }
    total + paired(&removed, &added)
}

/// How many lines of `columns` characters `row` takes.
fn row_height(row: &Row, columns: usize) -> usize {
    match row {
        Row::Hunk(_) => 1,
        Row::Pair(old, new) => [old, new]
            .into_iter()
            .flatten()
            .map(|entry| lines_of(&entry.line.text, columns))
            .max()
            .unwrap_or(1),
    }
}

/// About how tall the notes of `session` on the file at `index` are, in logical pixels, for a column `column` wide; the box for writing one counts as well.
fn notes_height(state: &WorkspaceState, session: &Session, index: usize, column: f32) -> f32 {
    let path = &session.changes[index].path;
    let kept: f32 = session
        .comments
        .iter()
        .filter(|comment| &comment.path == path)
        .map(|comment| NOTE_CHROME + wrap(&comment.text, 13, column).len() as f32 * NOTE_LINE)
        .sum();
    let open = state
        .comment_target
        .is_some_and(|target| target.change == index && !target.dragging);
    kept + if open { NOTE_BOX } else { 0.0 }
}

/// About how tall the file at `index` is with its header, its rows and its notes, in logical pixels, folded away or not.
pub(crate) fn section_height(
    state: &WorkspaceState,
    session: &Session,
    index: usize,
    width: f32,
) -> f32 {
    let change = &session.changes[index];
    if state.closed_diffs.contains(&change.path) {
        return (SECTION_CHROME + HEADER) * pixels();
    }
    let column = ((width - 34.0) / 2.0 - 46.0).max(80.0);
    (SECTION_CHROME
        + HEADER
        + row_lines(change, columns(width)) as f32 * ROW
        + notes_height(state, session, index, column))
        * pixels()
}

/// How far down the diff the file at `index` starts, in logical pixels, for the files folded away in `state` and a view `width` wide.
pub fn offset_of(state: &WorkspaceState, session: &Session, index: usize, width: f32) -> f32 {
    (0..index)
        .map(|before| section_height(state, session, before, width))
        .sum::<f32>()
}

/// The type the code of a diff is set in: 12 pixels tall, a line height of 1.4 times that, in the code font, in `tone`.
fn code_type(tone: impl Into<Tone>) -> rustle::TextStyle {
    theme::mono_style(12, 1.4).color(theme::color(tone))
}

/// Emits one line of a diff: tinted by what it does, written in the code font, its code coloured by `marks`.
#[track_caller]
pub fn diff_line(kind: Kind, content: &str, marks: Vec<Mark>) {
    let (marker, fill, tone) = match kind {
        Kind::Hunk => (" ", Some(Tone::from(Role::Surface)), Role::Subtle),
        Kind::Context => (" ", None, Role::Muted),
        Kind::Add => ("+", Some(Tone::from(Role::Success).alpha(0.12)), Role::Text),
        Kind::Remove => ("-", Some(Tone::from(Role::Danger).alpha(0.12)), Role::Text),
    };
    let marker = if kind == Kind::Hunk { "" } else { marker };
    let mut own = Modifier::new().fill_max_width().height(theme::sp(5));
    if let Some(fill) = fill {
        own = own.background(theme::color(fill));
    }
    let shift = marker.len();
    let marks = marks
        .into_iter()
        .map(|mark| Mark {
            range: mark.range.start + shift..mark.range.end + shift,
            ..mark
        })
        .collect::<Vec<_>>();
    let content = format!("{marker}{}", shown(content));
    row_arranged(
        own.padding_xy(theme::sp(3), 0.0),
        Arrangement::Start,
        Alignment::Center,
        move || {
            marked_line(
                Modifier::new(),
                &content,
                code_type(tone),
                marks,
                CaretLook::default(),
            );
        },
    );
}

/// The tint behind a line, the stronger one behind its number, and the colour of the number.
fn tints(kind: Option<Kind>) -> (Option<Tone>, Option<Tone>, Role) {
    match kind {
        Some(Kind::Add) => (
            Some(Tone::from(Role::Success).alpha(0.12)),
            Some(Tone::from(Role::Success).alpha(0.25)),
            Role::Success,
        ),
        Some(Kind::Remove) => (
            Some(Tone::from(Role::Danger).alpha(0.12)),
            Some(Tone::from(Role::Danger).alpha(0.25)),
            Role::Danger,
        ),
        Some(_) => (None, None, Role::Subtle),
        None => (
            Some(Tone::from(Role::Surface).alpha(0.6)),
            Some(Tone::from(Role::Surface).alpha(0.6)),
            Role::Subtle,
        ),
    }
}

/// Emits the cell that holds a line number, on the stronger tint of the line it numbers; pressing it, or dragging from it, starts a comment on the lines.
#[track_caller]
fn gutter(number: Option<usize>, kind: Option<Kind>, id: Option<FocusId>) {
    let (_, tint, tone) = tints(kind);
    let mut own = Modifier::new().fill_max_height().width(theme::sp(11));
    own = match id.filter(|_| number.is_some()) {
        Some(id) => {
            let looks = theme::InteractionTones::new().hover(Role::Accent);
            let looks = match tint {
                Some(tint) => looks.idle(tint),
                None => looks,
            };
            own.interaction_background(looks.colors())
                .cursor(CursorIcon::Pointer)
                .control(id, reduce(id))
                .chain(crate::events::comment_drag(id))
        }
        None => match tint {
            Some(tint) => own.background(theme::color(tint)),
            None => own,
        },
    };
    let shown_number = number.map(|number| number.to_string()).unwrap_or_default();
    row_arranged(
        own.padding_each(0.0, 0.0, theme::sp(2), 0.0),
        Arrangement::End,
        Alignment::Start,
        move || {
            row_arranged(
                Modifier::new().height(theme::sp(5)),
                Arrangement::Start,
                Alignment::Center,
                move || {
                    styled_text(Modifier::new(), shown_number, code_type(tone));
                },
            );
        },
    );
}

/// Emits the cell that holds the text of a line, coloured by `marks` and broken into lines of `columns` characters, each on a row of its own.
#[track_caller]
fn code(content: &str, marks: Vec<Mark>, columns: usize) {
    let content = shown(content);
    column_arranged(
        Modifier::new()
            .weight(1.0)
            .clip()
            .padding_xy(theme::sp(3), 0.0),
        Arrangement::Start,
        Alignment::Start,
        || {
            for piece in pieces(&content, columns) {
                let within: Vec<Mark> = marks
                    .iter()
                    .filter(|mark| mark.range.start < piece.end && mark.range.end > piece.start)
                    .map(|mark| Mark {
                        range: mark.range.start.max(piece.start) - piece.start
                            ..mark.range.end.min(piece.end) - piece.start,
                        ..mark.clone()
                    })
                    .collect();
                let text = &content[piece];
                row_arranged(
                    Modifier::new().fill_max_width().height(theme::sp(5)),
                    Arrangement::Start,
                    Alignment::Center,
                    move || {
                        marked_line(
                            Modifier::new().weight(1.0),
                            text,
                            code_type(Role::Text).max_lines(1),
                            within,
                            CaretLook::default(),
                        );
                    },
                );
            }
        },
    );
}

/// Emits one half of a row: the line that is on that side, coloured as `colours` says, or a blank where there is none, lit while it is among the lines being commented on.
#[track_caller]
fn half(
    entry: Option<&Numbered>,
    colours: &DiffColours,
    change: usize,
    new: bool,
    selected: Option<Range>,
    columns: usize,
    lines: usize,
) {
    let (number, kind, content, marks) = match entry {
        Some(entry) => (
            if new { entry.new } else { entry.old },
            Some(entry.line.kind),
            entry.line.text.as_str(),
            colours.line(entry.at),
        ),
        None => (None, None, "", Vec::new()),
    };
    let lit = selected.is_some_and(|range| {
        range.new == new && number.is_some_and(|n| (range.start..=range.end).contains(&n))
    });
    let (fill, _, _) = tints(kind);
    let fill = if lit {
        Some(Tone::from(Role::Accent).alpha(0.25))
    } else {
        fill
    };
    let mut own = Modifier::new()
        .weight(1.0)
        .height(theme::sp(5.0 * lines as f32));
    if let Some(fill) = fill {
        own = own.background(theme::color(fill));
    }
    if new {
        own = own
            .draw_behind(theme::edge(
                theme::Side::Left,
                1.0,
                theme::color(Role::Border),
            ))
            .padding_each(1.0, 0.0, 0.0, 0.0);
    }
    row_arranged(
        own.clip(),
        Arrangement::Start,
        Alignment::Start,
        move || {
            gutter(
                number,
                kind,
                number.map(|number| gutter_id(change, new, number)),
            );
            code(content, marks, columns);
        },
    );
}

/// Emits the row of a hunk header.
#[track_caller]
fn hunk_row(header: &str) {
    row_arranged(
        Modifier::new()
            .fill_max_width()
            .height(theme::sp(5))
            .background(theme::color(Role::Surface))
            .padding_xy(theme::sp(3), 0.0),
        Arrangement::Start,
        Alignment::Center,
        || {
            styled_text(
                Modifier::new().weight(1.0),
                header,
                code_type(Role::Subtle).max_lines(1),
            );
        },
    );
}

/// Emits a diff row of the change at `change`, its code coloured as `colours` says.
#[track_caller]
fn diff_row(
    row_of: &Row,
    colours: &DiffColours,
    change: usize,
    selected: Option<Range>,
    columns: usize,
) {
    let lines = row_height(row_of, columns);
    match row_of {
        Row::Hunk(header) => hunk_row(header),
        Row::Pair(old, new) => row_arranged(
            Modifier::new()
                .fill_max_width()
                .height(theme::sp(5.0 * lines as f32)),
            Arrangement::Start,
            Alignment::Start,
            || {
                half(
                    old.as_ref(),
                    colours,
                    change,
                    false,
                    selected,
                    columns,
                    lines,
                );
                half(
                    new.as_ref(),
                    colours,
                    change,
                    true,
                    selected,
                    columns,
                    lines,
                );
            },
        ),
    }
}

/// Emits a card of a note, drawn by `card`, under the column it is about when it is about lines and across the file when it is not; the blank beside it and the card share the room left after the card's side margins.
#[track_caller]
fn under(range: Option<Range>, card: impl FnOnce()) {
    let blank = || rustle::spacer(Modifier::new().weight(1.0));
    row_arranged(
        Modifier::new().fill_max_width(),
        Arrangement::Start,
        Alignment::Start,
        move || match range {
            Some(range) if range.new => {
                blank();
                margined(card);
            }
            Some(_) => {
                margined(card);
                blank();
            }
            None => margined(card),
        },
    );
}

/// Emits the card `card` draws in a weighted share of its row, with two steps of margin on every side, the side margins kept out of the share.
#[track_caller]
fn margined(card: impl FnOnce()) {
    rustle::spacer(Modifier::new().width(theme::sp(2)));
    column_arranged(
        Modifier::new().weight(1.0).padding_xy(0.0, theme::sp(2)),
        Arrangement::Start,
        Alignment::Start,
        card,
    );
    rustle::spacer(Modifier::new().width(theme::sp(2)));
}

/// Emits the frame of a note: a rounded box on the panel colour, padded when `padded`, spreading what `content` emits by `arrangement`.
#[track_caller]
fn card(padded: bool, arrangement: Arrangement, content: impl FnOnce()) {
    let own = Modifier::new()
        .fill_max_width()
        .background_rounded(theme::color(Role::Surface), Radius::Lg.px())
        .border(1.0, theme::color(Role::Border), Radius::Lg.px())
        .padding(1.0);
    let own = if padded {
        own.padding(theme::sp(3))
    } else {
        own
    };
    column_arranged(own, arrangement, Alignment::Start, content);
}

/// Emits a note waiting to be sent: which lines it is about, that it is unsent, a button to take it back, and what it says.
#[track_caller]
fn comment_card(index: usize, comment: &Comment, width: f32) {
    let about = comment
        .range
        .map_or_else(|| "Whole file".to_owned(), |range| range.label());
    let body = wrap(&comment.text, 13, width);
    under(comment.range, || {
        card(false, Arrangement::Start, || {
            row_arranged(
                Modifier::new()
                    .fill_max_width()
                    .height(theme::sp(8))
                    .draw_behind(theme::edge(
                        theme::Side::Bottom,
                        1.0,
                        theme::color(Role::Border),
                    ))
                    .padding_each(0.0, 0.0, 0.0, 1.0)
                    .padding_xy(theme::sp(3), 0.0),
                Arrangement::SpacedBy(sp(2)),
                Alignment::Center,
                || {
                    line(about, 12, 1.4, Role::Subtle);
                    column_arranged(
                        Modifier::new()
                            .border(1.0, theme::color(Role::Border), Radius::Full.px())
                            .padding(1.0)
                            .padding_xy(theme::sp(2), 0.0),
                        Arrangement::Start,
                        Alignment::Start,
                        || line("Not sent", 11, 1.4, Role::Muted),
                    );
                    spacer();
                    Button::icon(IconName::Close, FocusId(COMMENT_REMOVE_BASE + index as u64))
                        .build();
                },
            );
            column_arranged(
                Modifier::new()
                    .padding_xy(theme::sp(3), 0.0)
                    .padding_xy(0.0, theme::sp(2.5)),
                Arrangement::Start,
                Alignment::Start,
                || {
                    for text in &body {
                        line(text, 13, 1.54, Role::Text);
                    }
                },
            );
        });
    });
}

/// Emits the form for a comment on `target`: what it is about, a box to write it in, and the buttons to keep it or drop it.
#[track_caller]
fn comment_box(state: &WorkspaceState, target: CommentTarget) {
    let about = target
        .range
        .map_or_else(|| "this file".to_owned(), |range| range.label());
    let ready = !state.comment_text.text().trim().is_empty();
    under(target.range, || {
        card(true, Arrangement::SpacedBy(sp(2.5)), || {
            styled_text(
                Modifier::new(),
                format!("Comment on {about}"),
                theme::text_style(13, 1.4)
                    .font_weight(rustle::FontWeight::MEDIUM)
                    .color(theme::color(Role::Text))
                    .max_lines(1),
            );
            Input::new(&state.comment_text, COMMENT_FIELD)
                .placeholder("Tell the agent what to change")
                .build();
            row_arranged(
                Modifier::new().fill_max_width(),
                Arrangement::SpacedBy(sp(2)),
                Alignment::Center,
                || {
                    spacer();
                    Button::new("Cancel", COMMENT_CANCEL)
                        .variant(Variant::Ghost)
                        .size(Size::Sm)
                        .build();
                    Button::new("Add comment", COMMENT_ADD)
                        .size(Size::Sm)
                        .disabled(!ready)
                        .build();
                },
            );
        });
    });
}

/// Emits the button that marks a file as read, or takes the mark off.
#[track_caller]
fn viewed_button(viewed: bool, id: FocusId) {
    Button::icon(IconName::Check, id)
        .size(Size::Xs)
        .round()
        .tone(if viewed { Role::Success } else { Role::Subtle })
        .selected(viewed)
        .build();
}

/// Something shown under a row of a diff: the form for a comment, or a note kept.
enum Block<'a> {
    /// The form for a comment on the target.
    Form(CommentTarget),
    /// The note at this position among the session's notes.
    Note(usize, &'a Comment),
}

/// How wide, in steps, the chevron and the file icon of a file header are.
const HEADER_MARKS: [f32; 2] = [3.5, 4.0];

/// Emits the parts of a file header that answer to its fold, in `room` logical pixels across: the chevron, open when `open`, the icon of the file `name` and `name` itself, then its `directory`.
///
/// When they do not fit, the chevron and the icon give up their width, in proportion to it, as far as the overflow goes or until they have none; the name then takes what it needs of what is left, cut short at its edge, and the folder the rest, cut off at the header's edge.
#[track_caller]
fn heading_parts(name: &str, directory: &str, open: bool, room: f32) {
    let name_kind = theme::text_style(13, 1.4)
        .font_weight(rustle::FontWeight::MEDIUM)
        .color(theme::color(Role::Text))
        .max_lines(1);
    let directory_kind = theme::mono_style(11, 1.4).color(theme::color(Role::Subtle));
    let name_width = rustle::layout_text(name, &name_kind, None)
        .size()
        .width
        .ceil();
    let directory_width = rustle::layout_text(directory, &directory_kind, None)
        .size()
        .width
        .ceil();
    let marks = sp(HEADER_MARKS[0] + HEADER_MARKS[1]);
    let gaps = sp(2) * 3.0;
    let over = (marks + name_width + directory_width + gaps - room).max(0.0);
    let kept = (1.0 - over / marks).clamp(0.0, 1.0);
    row_arranged(
        Modifier::new().width_in(0.0, room),
        Arrangement::SpacedBy(sp(2)),
        Alignment::Center,
        || {
            column_arranged(
                Modifier::new().width(sp(HEADER_MARKS[0]) * kept).clip(),
                Arrangement::Start,
                Alignment::Start,
                || {
                    icon(
                        if open {
                            IconName::ChevronDown
                        } else {
                            IconName::ChevronRight
                        },
                        HEADER_MARKS[0],
                        Role::Muted,
                    );
                },
            );
            column_arranged(
                Modifier::new().width(sp(HEADER_MARKS[1]) * kept).clip(),
                Arrangement::Start,
                Alignment::Start,
                || file_icon(name, HEADER_MARKS[1]),
            );
            styled_text(Modifier::new(), name, name_kind);
            column_arranged(
                Modifier::new().clip(),
                Arrangement::Start,
                Alignment::Start,
                || styled_text(Modifier::new(), directory, directory_kind),
            );
        },
    );
}

/// Emits the header of `change`, the change at `index`, unfolded when `open`, with the count of `notes` on it; when the header is too narrow the chevron and the file icon give up their room first, then the folder, which is cut off at the edge.
#[track_caller]
fn file_header(change: &Change, index: usize, open: bool, notes: usize) {
    let mut own = Modifier::new()
        .fill_max_width()
        .height(theme::sp(10))
        .background(theme::color(Role::Surface));
    if open {
        own = own
            .draw_behind(theme::edge(
                theme::Side::Bottom,
                1.0,
                theme::color(Role::Border),
            ))
            .padding_each(0.0, 0.0, 0.0, 1.0);
    }
    row_arranged(
        own.padding_xy(theme::sp(3), 0.0),
        Arrangement::SpacedBy(sp(2)),
        Alignment::Center,
        || {
            row_arranged(
                Modifier::new()
                    .weight(1.0)
                    .clip()
                    .cursor(CursorIcon::Pointer)
                    .control(
                        FocusId(DIFF_FOLD_BASE + index as u64),
                        reduce(FocusId(DIFF_FOLD_BASE + index as u64)),
                    ),
                Arrangement::SpacedBy(sp(2)),
                Alignment::Center,
                || {
                    let name = change.name().to_owned();
                    let directory = change.directory().to_owned();
                    rustle::with_constraints(Modifier::new(), move |room| {
                        heading_parts(&name, &directory, open, room.max_width);
                    });
                },
            );
            if notes > 0 {
                row_arranged(
                    Modifier::new(),
                    Arrangement::SpacedBy(sp(1)),
                    Alignment::Center,
                    || {
                        icon(IconName::Message, 3.5, Role::Muted);
                        line(notes.to_string(), 11, 1.4, Role::Muted);
                    },
                );
            }
            Tally::new(change.added(), change.removed())
                .size(11)
                .build();
            viewed_button(change.viewed, FocusId(VIEWED_BASE + index as u64));
            Button::icon(IconName::Message, FocusId(COMMENT_FILE_BASE + index as u64)).build();
        },
    );
}

/// Emits one changed file: its header, and, unless it is folded away, its lines with the notes on them.
#[track_caller]
fn file(
    state: &WorkspaceState,
    session: &Session,
    index: usize,
    width: f32,
    top: f32,
    window: (f32, f32),
) {
    let height = section_height(state, session, index, width);
    if top + height < window.0 || top > window.1 {
        rustle::spacer(Modifier::new().height(theme::sp(height / (4.0 * pixels()))));
        return;
    }
    let change = &session.changes[index];
    let open = !state.closed_diffs.contains(&change.path);
    let column_width = ((width - 34.0) / 2.0 - 46.0).max(80.0);
    let mine: Vec<(usize, &Comment)> = session
        .comments
        .iter()
        .enumerate()
        .filter(|(_, comment)| comment.path == change.path)
        .collect();
    let target = state.comment_target.filter(|target| target.change == index);
    column_arranged(
        Modifier::new()
            .fill_max_width()
            .padding_xy(theme::sp(4), 0.0)
            .padding_each(0.0, theme::sp(4), 0.0, 0.0)
            .border(1.0, theme::color(Role::Border), Radius::Lg.px())
            .padding(1.0)
            .clip(),
        Arrangement::Start,
        Alignment::Start,
        || {
            file_header(change, index, open, mine.len());
            if !open {
                return;
            }
            if !change.loaded {
                row_arranged(
                    Modifier::new()
                        .height(theme::sp(change.expected_rows() as f32 * ROW / 4.0))
                        .padding_xy(theme::sp(3), 0.0)
                        .padding_xy(0.0, theme::sp(2)),
                    Arrangement::Start,
                    Alignment::Start,
                    || line("Reading the diff…", 12, 1.4, Role::Subtle),
                );
                return;
            }
            if let Some(target) = target.filter(|target| target.range.is_none() && !target.dragging)
            {
                rustle::key("file form", || comment_box(state, target));
            }
            for (at, comment) in mine.iter().filter(|(_, comment)| comment.range.is_none()) {
                rustle::key(("file note", *at), || comment_card(*at, comment, width));
            }
            let selected = target.and_then(|target| target.range);
            let mut colours = DiffColours::of(&change.diff, file_language(&change.path).as_deref());
            if state.window.git.word_diff {
                colours = colours.with_words(&change.diff, &change.words);
            }
            let scale = pixels();
            let columns = columns(width);
            let mut at = top + (SECTION_CHROME + HEADER) * scale;
            let mut skipped = 0;
            for (position, row_of) in rows(change).iter().enumerate() {
                let mut blocks: Vec<(Block, f32)> = Vec::new();
                if let Row::Pair(old, new) = row_of {
                    for (side_new, entry) in [(false, old), (true, new)] {
                        let number = entry
                            .as_ref()
                            .and_then(|entry| if side_new { entry.new } else { entry.old });
                        let Some(number) = number else {
                            continue;
                        };
                        let ends_here = |range: Option<Range>| {
                            range.is_some_and(|range| range.new == side_new && range.end == number)
                        };
                        if let Some(target) =
                            target.filter(|target| !target.dragging && ends_here(target.range))
                        {
                            blocks.push((Block::Form(target), NOTE_BOX));
                        }
                        for (nth, comment) in
                            mine.iter().filter(|(_, comment)| ends_here(comment.range))
                        {
                            blocks.push((
                                Block::Note(*nth, comment),
                                NOTE_CHROME
                                    + wrap(&comment.text, 13, column_width).len() as f32
                                        * NOTE_LINE,
                            ));
                        }
                    }
                }
                let lines = row_height(row_of, columns);
                let near = at + lines as f32 * ROW * scale >= window.0 && at <= window.1;
                if blocks.is_empty() && !near {
                    skipped += lines;
                    at += lines as f32 * ROW * scale;
                    continue;
                }
                skip(&mut skipped);
                rustle::key(("row", position), || {
                    diff_row(row_of, &colours, index, selected, columns);
                });
                at += lines as f32 * ROW * scale;
                for (block, tall) in blocks {
                    match block {
                        Block::Form(target) => {
                            rustle::key(("form", position), || comment_box(state, target));
                        }
                        Block::Note(nth, comment) => {
                            rustle::key(("note", nth), || {
                                comment_card(nth, comment, column_width);
                            });
                        }
                    }
                    at += tall * scale;
                }
            }
            skip(&mut skipped);
        },
    );
}

/// Emits a blank as tall as the `rows` lines left out since the last row built, which are taken as counted.
#[track_caller]
fn skip(rows: &mut usize) {
    let blank = std::mem::take(rows);
    if blank == 0 {
        return;
    }
    rustle::spacer(Modifier::new().height(theme::sp(blank as f32 * ROW / 4.0)));
}

/// Emits the strip above the files: how many changed, how many are read, and the button that sends the notes to the agent.
#[track_caller]
fn summary(session: &Session) {
    let viewed = session
        .changes
        .iter()
        .filter(|change| change.viewed)
        .count();
    let count = session.changes.len();
    let waiting = session.comments.len();
    rustle::layout(
        Modifier::new()
            .fill_max_width()
            .height(theme::sp(10))
            .draw_behind(theme::edge(
                theme::Side::Bottom,
                1.0,
                theme::color(Role::Border),
            ))
            .padding_each(0.0, 0.0, 0.0, 1.0)
            .padding_xy(theme::sp(4), 0.0),
        SummaryLayout { gap: sp(3) },
        || {
            line(
                format!("{count} file{} changed", if count == 1 { "" } else { "s" }),
                12,
                1.4,
                Role::Muted,
            );
            Tally::new(session.added(), session.removed())
                .size(12)
                .build();
            line(format!("{viewed} of {count} viewed"), 12, 1.4, Role::Subtle);
            Button::new(
                if waiting > 0 {
                    format!("Send to agent · {waiting}")
                } else {
                    "Send to agent".to_owned()
                },
                SEND_COMMENTS,
            )
            .size(Size::Sm)
            .disabled(waiting == 0)
            .build();
        },
    );
}

/// How the summary strip lays out its four children: the count of files and the tally at the
/// start, the count of files read and the send button at the end, `gap` apart and at least two
/// gaps between the two groups.
///
/// When they do not fit, the two counts, which cut themselves short, give up the overflow in
/// proportion to their widths; the tally and the button keep theirs.
struct SummaryLayout {
    /// The least room between neighbours, in logical pixels.
    gap: f32,
}

impl rustle::Layout for SummaryLayout {
    /// Measures the tally and the button at their own widths, the counts at their own widths or
    /// their shares of what is left, and centres everything down the strip.
    fn measure(
        &self,
        children: &mut rustle::Children,
        constraints: rustle::Constraints,
    ) -> rustle::LayoutResult {
        let height = constraints.max_height;
        let room = constraints.max_width;
        let natural: Vec<f32> = (0..children.len())
            .map(|index| children.max_intrinsic_width(index, height))
            .collect();
        let fixed = natural[1] + natural[3] + 4.0 * self.gap;
        let wanted = natural[0] + natural[2];
        let scale = if wanted > 0.0 {
            ((room - fixed).max(0.0) / wanted).min(1.0)
        } else {
            1.0
        };
        let widths = [
            natural[0] * scale,
            natural[1],
            natural[2] * scale,
            natural[3],
        ];
        let lefts = [
            0.0,
            widths[0] + self.gap,
            room - widths[3] - self.gap - widths[2],
            room - widths[3],
        ];
        let positions = widths
            .iter()
            .zip(lefts)
            .enumerate()
            .map(|(index, (width, left))| {
                let size = children.measure(
                    index,
                    rustle::Constraints {
                        min_width: 0.0,
                        max_width: *width,
                        min_height: 0.0,
                        max_height: height,
                    },
                );
                rustle::Offset::new(left, (height - size.height) / 2.0)
            })
            .collect();
        rustle::LayoutResult {
            size: rustle::Size::new(room, height),
            positions,
        }
    }
}

/// Emits the diff of `session` in a view `width` by `height` logical pixels: a summary strip, then each changed file, of which only those in or near the view are built.
#[track_caller]
pub fn diff(state: &WorkspaceState, session: &Session, width: f32, height: f32) {
    let heights: Vec<f32> = (0..session.changes.len())
        .map(|index| section_height(state, session, index, width))
        .collect();
    let total: f32 = heights.iter().sum();
    let view = (height - CHROME * pixels()).max(0.0);
    let scrolled = state.diff_scroll.offset();
    let window = (
        scrolled.min(total - view) - OVERSCAN * pixels(),
        scrolled.min(total) + view + OVERSCAN * pixels(),
    );
    column_arranged(
        Modifier::new().weight(1.0),
        Arrangement::Start,
        Alignment::Start,
        || {
            summary(session);
            column_arranged(
                state
                    .diff_bounds
                    .track(Modifier::new().fill_max_width().weight(1.0))
                    .vertical_scroll(&state.diff_scroll)
                    .fill_max_width()
                    .padding_each(0.0, 0.0, 0.0, theme::sp(4)),
                Arrangement::Start,
                Alignment::Start,
                || {
                    let mut top = 0.0;
                    for (index, tall) in heights.iter().enumerate() {
                        rustle::key(index, || file(state, session, index, width, top, window));
                        top += tall;
                    }
                },
            );
        },
    );
}
