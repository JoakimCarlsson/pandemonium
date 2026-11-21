//! The diff view: every changed file stacked, the old and the new side by side, with notes on lines for the agent.

use crate::areas::DiffEvent;
use crate::button;
use crate::events;
use crate::icons;
use crate::parts;
use crate::theme::Frame;
use crate::theme::Indicated;
use rustle::TextOverflow;

use crate::target::Target;
use crate::theme;
use crate::theme::sp;
use rustle::Modifier;
use rustle::phosphor;
use rustle::{Alignment, Arrangement, Color, column_arranged, row_arranged};

use crate::icons::icon;
use crate::marked::{CaretLook, Mark, marked_line};
use crate::theme::{Radius, Role};

use crate::button::Variant;
use crate::diff_syntax::{DiffColours, shown};
use crate::file_icon::{file_icon, file_language};
use crate::parts::{line, spacer, wrapping};
use crate::sample::{Change, Comment, DiffLine, Kind, Range, Session};
use crate::size::Size;
use crate::state::{CommentTarget, WorkspaceState};
use crate::{Button, Input, Tally};

/// How tall a line of a diff is, in logical pixels.
const ROW: f32 = 20.0;

/// The width of what is beside a pair of columns of code: the margins and rules of a file, in logical pixels.
const SECTION_SIDES: f32 = 34.0;

/// The width of a line number and the padding around the code beside it, in logical pixels.
const GUTTER_AND_PADDING: f32 = 44.0 + 24.0 + 1.0;

/// How tall the type of code is, in logical pixels.
const CODE_PIXELS: f32 = 12.0;

/// How wide a character of code is as a share of how tall the type is, on the wide side so that a line wraps before it would be cut.
const CODE_CHARACTER: f32 = 0.6;

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
        .map(|(at, line)| {
            let (old_number, new_number) = match line.kind {
                Kind::Hunk => {
                    (old, new) = starts(&line.text).unwrap_or((old, new));
                    (None, None)
                }
                Kind::Add => {
                    new += 1;
                    (None, Some(new - 1))
                }
                Kind::Remove => {
                    old += 1;
                    (Some(old - 1), None)
                }
                Kind::Context => {
                    old += 1;
                    new += 1;
                    (Some(old - 1), Some(new - 1))
                }
            };
            Numbered {
                line,
                at,
                old: old_number,
                new: new_number,
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
    let scale = theme::step_scale();
    let side = (width - SECTION_SIDES * scale) / 2.0 - GUTTER_AND_PADDING * scale;
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

/// The type the code of a diff is set in: 12 pixels tall, a line height of 1.4 times that, in the code font, in `tone`.
fn code_type(tone: impl Into<Color>) -> rustle::TextStyle {
    theme::mono_style(12, 1.4).color(theme::color(tone))
}

/// Emits one line of a diff: tinted by what it does, written in the code font, its code coloured by `marks`.
#[track_caller]
pub fn diff_line(kind: Kind, content: &str, marks: Vec<Mark>) {
    let (marker, fill, tone) = match kind {
        Kind::Hunk => ("", Some(Color::from(Role::Surface)), Role::Subtle),
        Kind::Context => (" ", None, Role::Muted),
        Kind::Add => ("+", Some(Role::Success.alpha(0.12)), Role::Text),
        Kind::Remove => ("-", Some(Role::Danger.alpha(0.12)), Role::Text),
    };
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
    rustle::row_arranged(
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
fn tints(kind: Option<Kind>) -> (Option<Color>, Option<Color>, Role) {
    match kind {
        Some(Kind::Add) => (
            Some(Role::Success.alpha(0.12)),
            Some(Role::Success.alpha(0.25)),
            Role::Success,
        ),
        Some(Kind::Remove) => (
            Some(Role::Danger.alpha(0.12)),
            Some(Role::Danger.alpha(0.25)),
            Role::Danger,
        ),
        Some(_) => (None, None, Role::Subtle),
        None => (
            Some(Role::Surface.alpha(0.6)),
            Some(Role::Surface.alpha(0.6)),
            Role::Subtle,
        ),
    }
}

/// Emits the cell that holds a line number, on the stronger tint of the line it numbers; pressing it, or dragging from it, starts a comment on the lines.
#[track_caller]
fn gutter(number: Option<usize>, kind: Option<Kind>, id: Option<Target>) {
    let (_, tint, tone) = tints(kind);
    let mut own = Modifier::new().fill_max_height().width(theme::sp(11));
    own = match id.filter(|_| number.is_some()) {
        Some(id) => {
            let looks = theme::InteractionTones::new().hover(Role::Accent);
            let looks = match tint {
                Some(tint) => looks.idle(tint),
                None => looks,
            };
            events::pressed(own.indicated(looks, theme::Focused::Rustle), id)
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
            rustle::row_arranged(
                Modifier::new().height(theme::sp(5)),
                Arrangement::Start,
                Alignment::Center,
                move || {
                    rustle::basic_text(
                        Modifier::new(),
                        shown_number,
                        code_type(tone),
                        TextOverflow::Ellipsis,
                    );
                },
            );
        },
    );
}

/// Emits the cell that holds the text of a line, coloured by `marks` and broken into lines of `columns` characters, each on a row of its own.
#[track_caller]
fn code(content: &str, marks: Vec<Mark>, columns: usize) {
    let content = shown(content);
    rustle::column_arranged(
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
                rustle::row_arranged(
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
        Some(Role::Accent.alpha(0.25))
    } else {
        fill
    };
    let mut own = Modifier::new()
        .weight(1.0)
        .height(theme::sp(5.0 * lines as f32));
    if let Some(fill) = fill {
        own = own.background(theme::color(fill));
    }
    rustle::row_arranged(own, Arrangement::Start, Alignment::Start, move || {
        if new {
            rustle::vertical_divider(Modifier::new(), 1.0, theme::color(Role::Border));
        }
        rustle::row_arranged(
            Modifier::new().weight(1.0).fill_max_height().clip(),
            Arrangement::Start,
            Alignment::Start,
            move || {
                gutter(
                    number,
                    kind,
                    number.map(|line| Target::Diff(DiffEvent::Gutter { change, new, line })),
                );
                code(content, marks, columns);
            },
        );
    });
}

/// Emits the row of a hunk header.
fn hunk_row(header: &str) {
    rustle::row_arranged(
        Modifier::new()
            .fill_max_width()
            .height(theme::sp(5))
            .background(theme::color(Role::Surface))
            .padding_xy(theme::sp(3), 0.0),
        Arrangement::Start,
        Alignment::Center,
        || {
            rustle::basic_text(
                Modifier::new().weight(1.0),
                header,
                code_type(Role::Subtle).max_lines(1),
                TextOverflow::Ellipsis,
            );
        },
    );
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

/// Emits a diff row of the change at `change`, its code coloured as `colours` says.
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
        Row::Pair(old, new) => rustle::row_arranged(
            Modifier::new()
                .fill_max_width()
                .height(theme::sp(5.0 * lines as f32)),
            Arrangement::Start,
            Alignment::Start,
            || {
                for (entry, new) in [(old, false), (new, true)] {
                    half(
                        entry.as_ref(),
                        colours,
                        change,
                        new,
                        selected,
                        columns,
                        lines,
                    );
                }
            },
        ),
    }
}

/// Emits a card of a note, drawn by `card`, under the column it is about when it is about lines and across the file when it is not; the blank beside it and the card share the room left after the card's side margins.
#[track_caller]
fn under(range: Option<Range>, card: impl FnOnce()) {
    let blank = || rustle::spacer(Modifier::new().weight(1.0));
    rustle::row_arranged(
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
    rustle::column_arranged(
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
    let own = Modifier::new().fill_max_width().panel(Radius::Lg);
    let own = if padded {
        own.padding(theme::sp(3))
    } else {
        own
    };
    column_arranged(own, arrangement, Alignment::Start, content);
}

/// Emits a note waiting to be sent: which lines it is about, that it is unsent, a button to take it back, and what it says.
#[track_caller]
fn comment_card(index: usize, comment: &Comment) {
    let about = comment
        .range
        .map_or_else(|| "Whole file".to_owned(), |range| range.label());
    under(comment.range, || {
        card(false, Arrangement::Start, || {
            parts::bar(8, 3, 2, || {
                line(about, 12, 1.4, Role::Subtle);
                rustle::column_arranged(
                    Modifier::new()
                        .outlined(Radius::Full)
                        .padding_xy(theme::sp(2), 0.0),
                    Arrangement::Start,
                    Alignment::Start,
                    || line("Not sent", 11, 1.4, Role::Muted),
                );
                spacer();
                Button::icon(phosphor::X, Target::Diff(DiffEvent::CommentRemove(index))).build();
            });
            rustle::column_arranged(
                Modifier::new().padding_xy(theme::sp(3), theme::sp(2.5)),
                Arrangement::Start,
                Alignment::Start,
                || {
                    wrapping(Modifier::new(), &comment.text, 13, 1.54, Role::Text);
                },
            );
        });
    });
}

/// Emits the form for a comment on `target`: what it is about, a box to write it in, and the buttons to keep it or drop it.
fn comment_box(state: &WorkspaceState, target: CommentTarget) {
    let about = target
        .range
        .map_or_else(|| "this file".to_owned(), |range| range.label());
    let ready = !state.comment_text.text().trim().is_empty();
    under(target.range, || {
        card(true, Arrangement::SpacedBy(sp(2.5)), || {
            rustle::basic_text(
                Modifier::new(),
                format!("Comment on {about}"),
                theme::strong(13, Role::Text),
                TextOverflow::Ellipsis,
            );
            crate::focus::request_focus_on(Target::Diff(DiffEvent::CommentField), &target);
            Input::new(&state.comment_text, Target::Diff(DiffEvent::CommentField))
                .placeholder("Tell the agent what to change")
                .submits(Target::Diff(DiffEvent::CommentAdd))
                .build();
            rustle::row_arranged(
                Modifier::new().fill_max_width(),
                Arrangement::SpacedBy(theme::sp(2)),
                Alignment::Center,
                || {
                    spacer();
                    button::action(
                        "Cancel",
                        Target::Diff(DiffEvent::CommentCancel),
                        Variant::Ghost,
                    );
                    button::small_button(
                        "Add comment",
                        Target::Diff(DiffEvent::CommentAdd),
                        Variant::Default,
                    )
                    .disabled(!ready)
                    .build();
                },
            );
        });
    });
}

/// Emits the button that marks a file as read, or takes the mark off.
fn viewed_button(viewed: bool, id: Target) {
    Button::icon(phosphor::CHECK, id)
        .size(Size::Xs)
        .round()
        .tone(if viewed { Role::Success } else { Role::Subtle })
        .selected(viewed)
        .build();
}

/// How wide, in steps, the chevron and the file icon of a file header are.
const HEADER_MARKS: [f32; 2] = [3.5, 4.0];

/// Emits the parts of a file header that answer to its fold, in `room` logical pixels across: the chevron, open when `open`, the icon of the file `name` and `name` itself, then its `directory`.
///
/// When they do not fit, the chevron and the icon give up their width, in proportion to it, as far as the overflow goes or until they have none; the name then takes what it needs of what is left, cut short at its edge, and the folder the rest, cut off at the header's edge.
fn heading_parts(name: &str, directory: &str, open: bool, room: f32) {
    let name_kind = theme::strong(13, Role::Text);
    let directory_kind = theme::mono_style(11, 1.4).color(theme::color(Role::Subtle));
    let width = |text: &str, kind| rustle::layout_text(text, kind, None).size().width.ceil();
    let (name_width, directory_width) =
        (width(name, &name_kind), width(directory, &directory_kind));
    let marks = sp(HEADER_MARKS[0] + HEADER_MARKS[1]);
    let gaps = sp(2) * 3.0;
    let over = (marks + name_width + directory_width + gaps - room).max(0.0);
    let kept = (1.0 - over / marks).clamp(0.0, 1.0);
    rustle::row_arranged(
        Modifier::new().width_in(0.0, room),
        Arrangement::SpacedBy(theme::sp(2)),
        Alignment::Center,
        || {
            rustle::column_arranged(
                Modifier::new().width(sp(HEADER_MARKS[0]) * kept).clip(),
                Arrangement::Start,
                Alignment::Start,
                || {
                    icon(icons::chevron(open), HEADER_MARKS[0], Role::Muted);
                },
            );
            rustle::column_arranged(
                Modifier::new().width(sp(HEADER_MARKS[1]) * kept).clip(),
                Arrangement::Start,
                Alignment::Start,
                || file_icon(name, HEADER_MARKS[1]),
            );
            rustle::basic_text(Modifier::new(), name, name_kind, TextOverflow::Ellipsis);
            rustle::column_arranged(
                Modifier::new().clip(),
                Arrangement::Start,
                Alignment::Start,
                || {
                    rustle::basic_text(
                        Modifier::new(),
                        directory,
                        directory_kind,
                        TextOverflow::Ellipsis,
                    )
                },
            );
        },
    );
}

/// Emits the header of `change`, the change at `index`, unfolded when `open`, with the count of `notes` on it; when the header is too narrow the chevron and the file icon give up their room first, then the folder, which is cut off at the edge.
fn file_header(change: &Change, index: usize, open: bool, notes: usize) {
    let mut own = Modifier::new()
        .fill_max_width()
        .height(theme::sp(10))
        .background(theme::color(Role::Surface));
    if open {
        own = own
            .border_bottom(1.0, theme::color(Role::Border))
            .padding_each(0.0, 0.0, 0.0, 1.0);
    }
    rustle::row_arranged(
        own.padding_xy(theme::sp(3), 0.0),
        Arrangement::SpacedBy(theme::sp(2)),
        Alignment::Center,
        || {
            rustle::row_arranged(
                events::pressed(
                    Modifier::new().weight(1.0).clip(),
                    Target::Diff(DiffEvent::DiffFold(index)),
                ),
                Arrangement::SpacedBy(theme::sp(2)),
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
                rustle::row_arranged(
                    Modifier::new(),
                    Arrangement::SpacedBy(theme::sp(1)),
                    Alignment::Center,
                    || {
                        icon(phosphor::CHAT_CIRCLE, 3.5, Role::Muted);
                        line(notes.to_string(), 11, 1.4, Role::Muted);
                    },
                );
            }
            Tally::new(change.added(), change.removed())
                .size(11)
                .build();
            viewed_button(change.viewed, Target::Diff(DiffEvent::Viewed(index)));
            Button::icon(
                phosphor::CHAT_CIRCLE,
                Target::Diff(DiffEvent::CommentFile(index)),
            )
            .build();
        },
    );
}

/// One item of the diff's lazy list: a part of a changed file, named by the file's index among the session's changes.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
enum Item {
    /// The header of the file.
    Header(usize),
    /// The line that says the file's diff is being read.
    Reading(usize),
    /// The form for a comment on the whole file.
    FileForm(usize),
    /// The note on the whole file at this position among the session's notes.
    FileNote(usize, usize),
    /// The row of the file at this position among its rows.
    Row(usize, usize),
    /// The form for a comment on the lines that end at the row at this position.
    Form(usize, usize),
    /// The note on lines of the file at this position among the session's notes.
    Note(usize, usize),
}

impl Item {
    /// The index of the file the item is part of.
    fn file(self) -> usize {
        match self {
            Self::Header(file)
            | Self::Reading(file)
            | Self::FileForm(file)
            | Self::FileNote(file, _)
            | Self::Row(file, _)
            | Self::Form(file, _)
            | Self::Note(file, _) => file,
        }
    }
}

/// The notes of `session` on the file `change`, each with its position among the session's notes.
fn notes_on<'a>(session: &'a Session, change: &Change) -> Vec<(usize, &'a Comment)> {
    session
        .comments
        .iter()
        .enumerate()
        .filter(|(_, comment)| comment.path == change.path)
        .collect()
}

/// The rows of each file of `session` that is shown unfolded with its diff read; none for the others.
fn shown_rows<'a>(state: &WorkspaceState, session: &'a Session) -> Vec<Vec<Row<'a>>> {
    session
        .changes
        .iter()
        .map(|change| {
            if change.loaded && !state.closed_diffs.contains(&change.path) {
                rows(change)
            } else {
                Vec::new()
            }
        })
        .collect()
}

/// Appends the items of the file at `index` of `session`, whose shown rows are `rows`: its header and, unless it is folded away, the line saying it is being read or its notes, forms and rows.
fn file_items(
    state: &WorkspaceState,
    session: &Session,
    index: usize,
    rows: &[Row],
    items: &mut Vec<Item>,
) {
    items.push(Item::Header(index));
    let change = &session.changes[index];
    if state.closed_diffs.contains(&change.path) {
        return;
    }
    if !change.loaded {
        items.push(Item::Reading(index));
        return;
    }
    let mine = notes_on(session, change);
    let target = state
        .comment_target
        .filter(|target| target.change == index && !target.dragging);
    if target.is_some_and(|target| target.range.is_none()) {
        items.push(Item::FileForm(index));
    }
    for (nth, _) in mine.iter().filter(|(_, comment)| comment.range.is_none()) {
        items.push(Item::FileNote(index, *nth));
    }
    for (position, row_of) in rows.iter().enumerate() {
        items.push(Item::Row(index, position));
        let Row::Pair(old, new) = row_of else {
            continue;
        };
        for (side_new, entry) in [(false, old), (true, new)] {
            let Some(number) = entry
                .as_ref()
                .and_then(|entry| if side_new { entry.new } else { entry.old })
            else {
                continue;
            };
            let ends_here = |range: Option<Range>| {
                range.is_some_and(|range| range.new == side_new && range.end == number)
            };
            if target.is_some_and(|target| ends_here(target.range)) {
                items.push(Item::Form(index, position));
            }
            for (nth, _) in mine.iter().filter(|(_, comment)| ends_here(comment.range)) {
                items.push(Item::Note(index, *nth));
            }
        }
    }
}

/// The items of the diff of `session`, file after file, whose shown rows are `rows`.
fn items(state: &WorkspaceState, session: &Session, rows: &[Vec<Row>]) -> Vec<Item> {
    let mut items = Vec::new();
    for (index, rows) in rows.iter().enumerate() {
        file_items(state, session, index, rows, &mut items);
    }
    items
}

/// The index, in the diff's lazy list, of the header of the file at `index` of `session`, as `state` shows it.
pub fn header_item(state: &WorkspaceState, session: &Session, index: usize) -> usize {
    let rows = shown_rows(state, session);
    items(state, session, &rows[..index.min(rows.len())]).len()
}

/// The index of the file of each item in the diff's lazy list of `session`, as `state` shows it.
pub(crate) fn item_files(state: &WorkspaceState, session: &Session) -> Vec<usize> {
    items(state, session, &shown_rows(state, session))
        .into_iter()
        .map(Item::file)
        .collect()
}

/// What the items of the diff are drawn from while the diff composes.
struct Shown<'a> {
    /// The workspace.
    state: &'a WorkspaceState,
    /// The session whose diff it is.
    session: &'a Session,
    /// The items of the list.
    items: Vec<Item>,
    /// The shown rows of each file.
    rows: Vec<Vec<Row<'a>>>,
    /// The colours of the code of each file whose rows were drawn, worked out once per composition.
    colours: std::cell::RefCell<std::collections::HashMap<usize, DiffColours>>,
    /// How many characters of code one side of a row holds on a line.
    columns: usize,
}

impl Shown<'_> {
    /// Whether the item at `at` is the last of its file.
    fn last_of_file(&self, at: usize) -> bool {
        self.items
            .get(at + 1)
            .is_none_or(|next| next.file() != self.items[at].file())
    }

    /// Emits the item at `at` in the frame of its file.
    #[track_caller]
    fn item(&self, at: usize) {
        let item = self.items[at];
        framed(
            matches!(item, Item::Header(_)),
            self.last_of_file(at),
            || self.content(item),
        );
    }

    /// Emits what `item` shows.
    #[track_caller]
    fn content(&self, item: Item) {
        let index = item.file();
        let change = &self.session.changes[index];
        let comment = |nth: usize| &self.session.comments[nth];
        let target = self
            .state
            .comment_target
            .filter(|target| target.change == index);
        match item {
            Item::Header(_) => file_header(
                change,
                index,
                !self.state.closed_diffs.contains(&change.path),
                notes_on(self.session, change).len(),
            ),
            Item::Reading(_) => rustle::row_arranged(
                Modifier::new()
                    .height(theme::sp(change.expected_rows() as f32 * ROW / 4.0))
                    .padding_xy(theme::sp(3), theme::sp(2)),
                Arrangement::Start,
                Alignment::Start,
                || line("Reading the diff…", 12, 1.4, Role::Subtle),
            ),
            Item::FileForm(_) | Item::Form(..) => {
                if let Some(target) = target {
                    comment_box(self.state, target);
                }
            }
            Item::FileNote(_, nth) => comment_card(nth, comment(nth)),
            Item::Note(_, nth) => comment_card(nth, comment(nth)),
            Item::Row(_, position) => {
                let mut cache = self.colours.borrow_mut();
                let colours = cache.entry(index).or_insert_with(|| {
                    let colours =
                        DiffColours::of(&change.diff, file_language(&change.path).as_deref());
                    if self.state.window.git.word_diff {
                        colours.with_words(&change.diff, &change.words)
                    } else {
                        colours
                    }
                });
                diff_row(
                    &self.rows[index][position],
                    colours,
                    index,
                    target.and_then(|target| target.range),
                    self.columns,
                );
            }
        }
    }
}

/// Emits `content` as a part of a file's frame, a rounded rule around the whole file a step and a half in from the sides: its top with the space above the file when `first`, its bottom when `last`, and its sides between.
#[track_caller]
fn framed(first: bool, last: bool, content: impl FnOnce()) {
    let colour = theme::color(Role::Border);
    let radius = Radius::Lg.px();
    let reach = radius * 2.0 + 2.0;
    let edge = |at: bool| if at { 1.0 } else { 0.0 };
    rustle::column_arranged(
        Modifier::new().fill_max_width().padding_each(
            theme::sp(4),
            if first { theme::sp(4) } else { 0.0 },
            theme::sp(4),
            0.0,
        ),
        Arrangement::Start,
        Alignment::Start,
        || {
            rustle::column_arranged(
                Modifier::new()
                    .fill_max_width()
                    .draw_with_content(move |canvas| {
                        canvas.draw_content();
                        let (width, height) = (canvas.width(), canvas.height());
                        let top = if first { 0.0 } else { -reach };
                        let bottom = if last { height } else { height + reach };
                        canvas.push_clip(0.0, 0.0, width, height);
                        canvas.stroke_rect(0.0, top, width, bottom - top, radius, 1.0, colour);
                        canvas.pop_clip();
                    })
                    .padding_each(1.0, edge(first), 1.0, edge(last))
                    .clip(),
                Arrangement::Start,
                Alignment::Start,
                content,
            );
        },
    );
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
            .border_bottom(1.0, theme::color(Role::Border))
            .padding_each(0.0, 0.0, 0.0, 1.0)
            .padding_xy(theme::sp(4), 0.0),
        crate::graph::SplitLayout {
            gap: sp(3),
            split: 2,
            fixed: &[1, 3],
        },
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
            button::small_button(
                if waiting > 0 {
                    format!("Send to agent · {waiting}")
                } else {
                    "Send to agent".to_owned()
                },
                Target::Diff(DiffEvent::SendComments),
                Variant::Default,
            )
            .disabled(waiting == 0)
            .build();
        },
    );
}

/// Emits the diff of `session` in a view `width` logical pixels wide: a summary strip, then a lazy list of the parts of each changed file, of which only those in or near the view are composed.
#[track_caller]
pub fn diff(state: &WorkspaceState, session: &Session, width: f32) {
    let rows = shown_rows(state, session);
    let shown = Shown {
        state,
        session,
        items: items(state, session, &rows),
        rows,
        colours: std::cell::RefCell::default(),
        columns: columns(width),
    };
    rustle::column_arranged(
        Modifier::new().weight(1.0),
        Arrangement::Start,
        Alignment::Start,
        || {
            summary(session);
            rustle::lazy_list_with(
                state
                    .diff_bounds
                    .track(Modifier::new().fill_max_width().weight(1.0))
                    .fill_max_width(),
                &state.diff_list,
                rustle::LazyContent::new().padding_each(0.0, theme::sp(4)),
                shown.items.len(),
                |at| {
                    let item = shown.items[at];
                    (session.changes[item.file()].path.as_str(), item)
                },
                |at| shown.item(at),
            );
        },
    );
}
