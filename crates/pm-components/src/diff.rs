//! The diff view: every changed file stacked, the old and the new side by side, with notes on lines for the agent.

use pm_icon::IconName;
use pm_input::FocusId;
use pm_style::{
    Color, Radius, Role, bg, border, border_b, border_color, border_l, cursor_pointer, flex,
    flex_1, flex_col, flex_none, font_medium, font_mono, gap, h, items_center, justify_center,
    justify_end, leading_ratio, min_h, min_w_px, mt, mx, overflow_hidden, p, pb, pr, px, py,
    rounded, rounded_full, size, text_color, text_px, truncate, w, w_full,
};
use pm_ui::{Node, node, text};

use crate::button::{Size, Variant};
use crate::file_icon::file_icon;
use crate::parts::{glyph, line, spacer, wrap};
use crate::sample::{Change, Comment, DiffLine, Kind, Range, Session};
use crate::state::{
    COMMENT_ADD, COMMENT_CANCEL, COMMENT_FIELD, COMMENT_FILE_BASE, COMMENT_REMOVE_BASE,
    CommentTarget, DIFF_FOLD_BASE, DIFF_SCROLL, SEND_COMMENTS, VIEWED_BASE, WorkspaceState,
};
use crate::{Button, Input, Tally};

/// How tall a line of a diff is, in logical pixels.
const ROW: f32 = 20.0;

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

/// The name of the number of line `line` of the change at `change`, on the new side when `new`.
pub fn gutter_id(change: usize, new: bool, line: usize) -> FocusId {
    FocusId(GUTTER_BASE + change as u64 * 2 * LINES + u64::from(new) * LINES + line as u64)
}

/// Which change, side and line the number named `id` is, if it is one.
pub fn gutter_of(id: FocusId) -> Option<(usize, bool, usize)> {
    let offset = id.0.checked_sub(GUTTER_BASE)?;
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
        .map(|line| match line.kind {
            Kind::Hunk => {
                (old, new) = starts(&line.text).unwrap_or((old, new));
                Numbered {
                    line,
                    old: None,
                    new: None,
                }
            }
            Kind::Add => {
                new += 1;
                Numbered {
                    line,
                    old: None,
                    new: Some(new - 1),
                }
            }
            Kind::Remove => {
                old += 1;
                Numbered {
                    line,
                    old: Some(old - 1),
                    new: None,
                }
            }
            Kind::Context => {
                old += 1;
                new += 1;
                Numbered {
                    line,
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

/// How many rows the diff of `change` shows, counted without building them.
fn row_count(change: &Change) -> usize {
    let (mut rows, mut removed, mut added) = (0, 0, 0);
    for line in &change.diff {
        match line.kind {
            Kind::Remove => removed += 1,
            Kind::Add => added += 1,
            Kind::Hunk | Kind::Context => {
                rows += removed.max(added) + 1;
                removed = 0;
                added = 0;
            }
        }
    }
    rows + removed.max(added)
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
fn section_height(state: &WorkspaceState, session: &Session, index: usize, width: f32) -> f32 {
    let change = &session.changes[index];
    if state.closed_diffs.contains(&change.path) {
        return SECTION_CHROME + HEADER;
    }
    let column = ((width - 34.0) / 2.0 - 46.0).max(80.0);
    SECTION_CHROME
        + HEADER
        + row_count(change) as f32 * ROW
        + notes_height(state, session, index, column)
}

/// How far down the diff the file at `index` starts, in logical pixels, for the files folded away in `state` and a view `width` wide.
pub fn offset_of(state: &WorkspaceState, session: &Session, index: usize, width: f32) -> f32 {
    (0..index)
        .map(|before| section_height(state, session, before, width))
        .sum::<f32>()
}

/// One line of a diff: tinted by what it does, written in the code font.
pub fn diff_line(kind: Kind, content: &str) -> Node {
    let (marker, fill, tone) = match kind {
        Kind::Hunk => (" ", Some(Color::from(Role::Surface)), Role::Subtle),
        Kind::Context => (" ", None, Role::Muted),
        Kind::Add => (
            "+",
            Some(Color::from(Role::Success).alpha(0.12)),
            Role::Text,
        ),
        Kind::Remove => ("-", Some(Color::from(Role::Danger).alpha(0.12)), Role::Text),
    };
    let marker = if kind == Kind::Hunk { "" } else { marker };
    let mut row = node().styles([flex(), w_full(), h(5), px(3), items_center()]);
    if let Some(fill) = fill {
        row = row.styles([bg(fill)]);
    }
    row.child(
        text(format!("{marker}{}", content.replace('\t', "    "))).styles([
            text_px(12),
            leading_ratio(1.4),
            font_mono(),
            text_color(tone),
        ]),
    )
}

/// The tint behind a line, the stronger one behind its number, and the colour of the number.
fn tints(kind: Option<Kind>) -> (Option<Color>, Option<Color>, Role) {
    match kind {
        Some(Kind::Add) => (
            Some(Color::from(Role::Success).alpha(0.12)),
            Some(Color::from(Role::Success).alpha(0.25)),
            Role::Success,
        ),
        Some(Kind::Remove) => (
            Some(Color::from(Role::Danger).alpha(0.12)),
            Some(Color::from(Role::Danger).alpha(0.25)),
            Role::Danger,
        ),
        Some(_) => (None, None, Role::Subtle),
        None => (
            Some(Color::from(Role::Surface).alpha(0.6)),
            Some(Color::from(Role::Surface).alpha(0.6)),
            Role::Subtle,
        ),
    }
}

/// The cell that holds a line number, on the stronger tint of the line it numbers; pressing it, or dragging from it, starts a comment on the lines.
fn gutter(number: Option<usize>, kind: Option<Kind>, id: Option<FocusId>) -> Node {
    let (_, tint, tone) = tints(kind);
    let mut cell = node().styles([
        flex(),
        items_center(),
        justify_end(),
        flex_none(),
        w(11),
        pr(2),
    ]);
    if let Some(tint) = tint {
        cell = cell.styles([bg(tint)]);
    }
    if let Some(id) = id.filter(|_| number.is_some()) {
        cell = cell
            .styles([cursor_pointer(), bg(Role::Accent).hover()])
            .focusable(id);
    }
    cell.child(
        text(number.map(|number| number.to_string()).unwrap_or_default()).styles([
            text_px(12),
            leading_ratio(1.4),
            font_mono(),
            text_color(tone),
        ]),
    )
}

/// The cell that holds the text of a line, cut short at its edge.
fn code(content: &str) -> Node {
    node()
        .styles([flex_1(), min_w_px(0), overflow_hidden(), px(3)])
        .child(text(content.replace('\t', "    ")).styles([
            text_px(12),
            leading_ratio(1.4),
            font_mono(),
            text_color(Role::Text),
            truncate(),
        ]))
}

/// One half of a row: the line that is on that side, or a blank where there is none, lit while it is among the lines being commented on.
fn half(entry: Option<&Numbered>, change: usize, new: bool, selected: Option<Range>) -> Node {
    let (number, kind, content) = match entry {
        Some(entry) => (
            if new { entry.new } else { entry.old },
            Some(entry.line.kind),
            entry.line.text.as_str(),
        ),
        None => (None, None, ""),
    };
    let lit = selected.is_some_and(|range| {
        range.new == new && number.is_some_and(|n| (range.start..=range.end).contains(&n))
    });
    let (fill, _, _) = tints(kind);
    let mut side = node()
        .styles([flex(), flex_1(), min_w_px(0), h(5), overflow_hidden()])
        .child(gutter(
            number,
            kind,
            number.map(|number| gutter_id(change, new, number)),
        ));
    if let Some(fill) = fill {
        side = side.styles([bg(fill)]);
    }
    if lit {
        side = side.styles([bg(Color::from(Role::Accent).alpha(0.25))]);
    }
    if new {
        side = side.styles([border_l(), border_color(Role::Border)]);
    }
    side.child(code(content))
}

/// The row of a hunk header.
fn hunk_row(header: &str) -> Node {
    node()
        .styles([
            flex(),
            w_full(),
            h(5),
            flex_none(),
            items_center(),
            px(3),
            bg(Role::Surface),
        ])
        .child(text(header.to_owned()).styles([
            text_px(12),
            leading_ratio(1.4),
            font_mono(),
            text_color(Role::Subtle),
            truncate(),
        ]))
}

/// A diff row of the change at `change` as a node.
fn row_node(row: &Row, change: usize, selected: Option<Range>) -> Node {
    match row {
        Row::Hunk(header) => hunk_row(header),
        Row::Pair(old, new) => node()
            .styles([flex(), w_full(), h(5), flex_none()])
            .child(half(old.as_ref(), change, false, selected))
            .child(half(new.as_ref(), change, true, selected)),
    }
}

/// A card of a note, under the column it is about when it is about lines and across the file when it is not.
fn under(range: Option<Range>, card: Node) -> Node {
    let row = node().styles([flex(), w_full(), flex_none()]);
    let gap = || node().styles([flex_1(), min_w_px(0)]);
    let card = node().styles([flex_1(), min_w_px(0), p(2)]).child(card);
    match range {
        Some(range) if range.new => row.child(gap()).child(card),
        Some(_) => row.child(card).child(gap()),
        None => row.child(card),
    }
}

/// The frame of a note: a rounded box on the panel colour.
fn card() -> Node {
    node().styles([
        flex(),
        flex_col(),
        rounded(Radius::Lg),
        border(),
        border_color(Role::Border),
        bg(Role::Surface),
    ])
}

/// A note waiting to be sent: which lines it is about, that it is unsent, a button to take it back, and what it says.
fn comment_card(index: usize, comment: &Comment, width: f32) -> Node {
    let label = comment
        .range
        .map_or_else(|| "Whole file".to_owned(), |range| range.label());
    let body = wrap(&comment.text, 13, width);
    under(
        comment.range,
        card()
            .child(
                node()
                    .styles([
                        flex(),
                        items_center(),
                        gap(2),
                        h(8),
                        px(3),
                        border_b(),
                        border_color(Role::Border),
                    ])
                    .child(line(label, 12, 1.4, Role::Subtle))
                    .child(
                        node()
                            .styles([px(2), rounded_full(), border(), border_color(Role::Border)])
                            .child(line("Not sent", 11, 1.4, Role::Muted)),
                    )
                    .child(spacer())
                    .child(
                        Button::icon(IconName::Close, FocusId(COMMENT_REMOVE_BASE + index as u64))
                            .build(),
                    ),
            )
            .child(
                node()
                    .styles([flex(), flex_col(), px(3), py(2.5)])
                    .children(body.iter().map(|text| line(text, 13, 1.54, Role::Text))),
            ),
    )
}

/// The form for a comment on `target`: what it is about, a box to write it in, and the buttons to keep it or drop it.
fn comment_box(state: &WorkspaceState, target: CommentTarget) -> Node {
    let about = target
        .range
        .map_or_else(|| "this file".to_owned(), |range| range.label());
    let ready = !state.comment_text.text().trim().is_empty();
    under(
        target.range,
        card()
            .styles([p(3), gap(2.5)])
            .child(line(format!("Comment on {about}"), 13, 1.4, Role::Text).styles([font_medium()]))
            .child(
                Input::new(&state.comment_text, COMMENT_FIELD)
                    .placeholder("Tell the agent what to change")
                    .build(),
            )
            .child(
                node()
                    .styles([flex(), items_center(), gap(2)])
                    .child(spacer())
                    .child(
                        Button::new("Cancel", COMMENT_CANCEL)
                            .variant(Variant::Ghost)
                            .size(Size::Sm)
                            .build(),
                    )
                    .child(
                        Button::new("Add comment", COMMENT_ADD)
                            .size(Size::Sm)
                            .disabled(!ready)
                            .build(),
                    ),
            ),
    )
}

/// The button that marks a file as read, or takes the mark off.
fn viewed_button(viewed: bool, id: FocusId) -> Node {
    let mut button = node()
        .styles([
            flex(),
            items_center(),
            justify_center(),
            size(6),
            flex_none(),
            rounded_full(),
            cursor_pointer(),
            bg(Role::SurfaceHover).hover(),
        ])
        .child(glyph(
            IconName::Check,
            3.5,
            if viewed { Role::Success } else { Role::Subtle },
        ))
        .focusable(id);
    if viewed {
        button = button.styles([bg(Color::from(Role::Success).alpha(0.15))]);
    }
    button
}

/// One changed file: its header, and, unless it is folded away, its lines with the notes on them.
fn file(
    state: &WorkspaceState,
    session: &Session,
    index: usize,
    width: f32,
    top: f32,
    window: (f32, f32),
) -> Node {
    let height = section_height(state, session, index, width);
    if top + height < window.0 || top > window.1 {
        return node().styles([h(height / 4.0), flex_none()]);
    }
    let change = &session.changes[index];
    let open = !state.closed_diffs.contains(&change.path);
    let column = ((width - 34.0) / 2.0 - 46.0).max(80.0);
    let mine: Vec<(usize, &Comment)> = session
        .comments
        .iter()
        .enumerate()
        .filter(|(_, comment)| comment.path == change.path)
        .collect();
    let target = state.comment_target.filter(|target| target.change == index);
    let mut header = node()
        .styles([
            flex(),
            items_center(),
            gap(2),
            h(10),
            px(3),
            flex_none(),
            bg(Role::Surface),
        ])
        .child(
            node()
                .styles([
                    flex(),
                    items_center(),
                    gap(2),
                    flex_1(),
                    min_w_px(0),
                    overflow_hidden(),
                    cursor_pointer(),
                ])
                .focusable(FocusId(DIFF_FOLD_BASE + index as u64))
                .child(glyph(
                    if open {
                        IconName::ChevronDown
                    } else {
                        IconName::ChevronRight
                    },
                    3.5,
                    Role::Muted,
                ))
                .child(file_icon(change.name(), 4.0))
                .child(
                    line(change.name(), 13, 1.4, Role::Text).styles([font_medium(), flex_none()]),
                )
                .child(text(change.directory().to_owned()).styles([
                    text_px(11),
                    leading_ratio(1.4),
                    font_mono(),
                    text_color(Role::Subtle),
                    truncate(),
                ])),
        );
    if !mine.is_empty() {
        header = header.child(
            node()
                .styles([flex(), items_center(), gap(1), flex_none()])
                .child(glyph(IconName::Message, 3.5, Role::Muted))
                .child(line(mine.len().to_string(), 11, 1.4, Role::Muted)),
        );
    }
    header = header
        .child(
            Tally::new(change.added(), change.removed())
                .size(11)
                .build(),
        )
        .child(viewed_button(
            change.viewed,
            FocusId(VIEWED_BASE + index as u64),
        ))
        .child(Button::icon(IconName::Message, FocusId(COMMENT_FILE_BASE + index as u64)).build());
    if open {
        header = header.styles([border_b(), border_color(Role::Border)]);
    }
    let mut section = node()
        .styles([
            flex(),
            flex_col(),
            flex_none(),
            mx(4),
            mt(4),
            rounded(Radius::Lg),
            border(),
            border_color(Role::Border),
            overflow_hidden(),
        ])
        .child(header);
    if !open {
        return section;
    }
    if let Some(target) = target.filter(|target| target.range.is_none() && !target.dragging) {
        section = section.child(comment_box(state, target));
    }
    for (at, comment) in mine.iter().filter(|(_, comment)| comment.range.is_none()) {
        section = section.child(comment_card(*at, comment, width));
    }
    let selected = target.and_then(|target| target.range);
    let mut at = top + SECTION_CHROME + HEADER;
    let mut skipped = 0;
    for row in rows(change) {
        let mut blocks = Vec::new();
        if let Row::Pair(old, new) = &row {
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
                    blocks.push((comment_box(state, target), NOTE_BOX));
                }
                for (nth, comment) in mine.iter().filter(|(_, comment)| ends_here(comment.range)) {
                    blocks.push((
                        comment_card(*nth, comment, column),
                        NOTE_CHROME + wrap(&comment.text, 13, column).len() as f32 * NOTE_LINE,
                    ));
                }
            }
        }
        let near = at + ROW >= window.0 && at <= window.1;
        if blocks.is_empty() && !near {
            skipped += 1;
            at += ROW;
            continue;
        }
        section = skip(section, &mut skipped);
        section = section.child(row_node(&row, index, selected));
        at += ROW;
        for (block, tall) in blocks {
            section = section.child(block);
            at += tall;
        }
    }
    skip(section, &mut skipped)
}

/// `section` with a blank as tall as the `rows` rows left out since the last one built, which are taken as counted.
fn skip(section: Node, rows: &mut usize) -> Node {
    let blank = std::mem::take(rows);
    if blank == 0 {
        return section;
    }
    section.child(node().styles([h(blank as f32 * ROW / 4.0), flex_none()]))
}

/// The strip above the files: how many changed, how many are read, and the button that sends the notes to the agent.
fn summary(session: &Session) -> Node {
    let viewed = session
        .changes
        .iter()
        .filter(|change| change.viewed)
        .count();
    let count = session.changes.len();
    let waiting = session.comments.len();
    node()
        .styles([
            flex(),
            items_center(),
            gap(3),
            h(10),
            px(4),
            flex_none(),
            border_b(),
            border_color(Role::Border),
        ])
        .child(line(
            format!("{count} file{} changed", if count == 1 { "" } else { "s" }),
            12,
            1.4,
            Role::Muted,
        ))
        .child(
            Tally::new(session.added(), session.removed())
                .size(12)
                .build(),
        )
        .child(spacer())
        .child(line(
            format!("{viewed} of {count} viewed"),
            12,
            1.4,
            Role::Subtle,
        ))
        .child(
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
            .build(),
        )
}

/// The diff of `session` in a view `width` by `height` logical pixels: a summary strip, then each changed file, of which only those in or near the view are built.
pub fn diff(state: &WorkspaceState, session: &Session, width: f32, height: f32) -> Node {
    let heights: Vec<f32> = (0..session.changes.len())
        .map(|index| section_height(state, session, index, width))
        .collect();
    let total: f32 = heights.iter().sum();
    let view = (height - CHROME).max(0.0);
    let window = (
        state.diff_scroll.min(total - view) - OVERSCAN,
        state.diff_scroll.min(total) + view + OVERSCAN,
    );
    let mut top = 0.0;
    let files: Vec<Node> = heights
        .iter()
        .enumerate()
        .map(|(index, tall)| {
            let node = file(state, session, index, width, top, window);
            top += tall;
            node
        })
        .collect();
    let body = node()
        .styles([flex(), flex_col(), flex_1(), min_h(0), overflow_hidden()])
        .scroll_from_start(DIFF_SCROLL, state.diff_scroll)
        .child(
            node()
                .styles([flex(), flex_col(), flex_none(), pb(4)])
                .children(files),
        );
    node()
        .styles([flex(), flex_col(), flex_1(), min_w_px(0), min_h(0)])
        .child(summary(session))
        .child(body)
}
