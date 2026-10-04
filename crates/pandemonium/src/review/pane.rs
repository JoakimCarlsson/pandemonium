//! The review: every uncommitted change of a project, in one pane.
//!
//! This is Zed's project diff, and it is the point of the whole flow. There
//! is one of these per project and it holds the lot: every changed file, one
//! under the next, each a heading with its own controls and its lines
//! beneath it. Clicking a row of the sidebar does not open another pane —
//! it brings this one forward and moves it to that file, because what a
//! reader wants after reading one file is the next one, not a tab for each.
//!
//! Above the changes is what to do with them — how much has changed, the way
//! through the hunks, staging — and below them is the message the whole lot
//! is about to be committed with. A reader can read every change and commit
//! it without leaving the pane.
//!
//! One file's diff on its own is the same rows with the rest left out, which
//! is what the menu's "Open File Diff" opens.
//!
//! Either pane reads a hunk one of two ways, the reader's to choose: as one
//! column, a line taken out above the line put in its place, or as two, the
//! old side set beside the new so a line and what became of it are read
//! across rather than down.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use pm_core::{Changed, Hunk, Line, LineKind};
use pm_gfx::Rgba;
use pm_text::Highlight;
use pm_ui::{
    Div, IconName, IconSize, Side, Styled, Theme, checkbox, h_flex, icon, icon_button, measured,
    text, turning_icon_button, v_flex,
};

use crate::config::Preference;
use crate::editor::tint;
use crate::message::Message;
use crate::review::action::{primary_face, primary_message};
use crate::review::comment::{Anchor, Comment, Composing, Side as CommentSide};
use crate::review::commit_editor;
use crate::review::gutter::revealing;
use crate::review::remark::{
    Delivery, block_rows, comment_block, composer_block, composer_rows, pending_bar,
};
use crate::review::sidebar::{staged_state, status_color};
use crate::review::store::{ChangeId, Patch, Review};

/// How many rows are built at once, however far the review runs on.
///
/// The pane clips whatever will not fit, and a change of a thousand files is
/// not a thousand rows of layout: what is built is the part the reader is
/// looking at and enough beyond it to fill any pane.
const DRAWN: usize = 400;

/// How wide the column of line numbers is drawn.
const NUMBERS: f32 = 76.0;

/// How wide the one number of a side of a split diff is drawn.
const SIDE_NUMBER: f32 = 48.0;

/// How wide the column a line's `+` or `−` is written in is drawn.
const MARK: f32 = 20.0;

/// How many times deeper a changed line's gutter is washed than its text.
const GUTTER_DEPTH: f32 = 2.5;

/// Space separating one changed file from the next.
const FILE_GAP: f32 = 24.0;

/// Builds the review of everything one project has changed.
///
/// `typing` says the commit message at the foot of it has the keyboard, so
/// that the caret is drawn in the field the reader is actually in — the same
/// message the sidebar shows, because there is one message and two places it
/// can be written. `split` sets each hunk's two sides beside each other.
pub fn review_pane(
    theme: &Theme,
    review: &Review,
    typing: bool,
    solid: bool,
    split: bool,
    remarking: Remarking,
) -> Div<Message> {
    let empty = review.changed().is_empty();
    let pending = review.comments().pending();

    v_flex()
        .w_full()
        .h_full()
        .overflow_hidden()
        .bg(theme.colors.background)
        .child(toolbar(theme, review, split))
        .when(pending > 0, |pane| {
            pane.child(pending_bar(theme, pending, remarking.delivery))
        })
        .child(measured(
            review.area(None),
            v_flex()
                .w_full()
                .flex_1()
                .overflow_hidden()
                .when(empty, |pane| {
                    pane.child(nothing(theme, "No uncommitted changes"))
                })
                .children(drawn(theme, review, None, split, remarking)),
        ))
        .when(!empty, |pane| {
            pane.child(commit_bar(theme, review, typing, solid))
        })
}

/// What a pane needs to know of the comments beyond the comments themselves.
#[derive(Clone, Copy, Debug)]
pub struct Remarking {
    /// Whether the keyboard is in the box a comment is being written in.
    pub focused: bool,
    /// Whether the caret is in the visible half of its blink.
    pub solid: bool,
    /// Whether the review can be sent to the agent.
    pub delivery: Delivery,
}

/// Builds the diff of the one changed file `id` names.
///
/// A file that has stopped differing — committed, or put back the way it was
/// — keeps its pane and says so, rather than the tab closing itself under a
/// reader who was in the middle of it.
pub fn change_pane(
    theme: &Theme,
    review: &Review,
    id: ChangeId,
    split: bool,
    remarking: Remarking,
) -> Div<Message> {
    let name = review
        .path_of(id)
        .map(|path| relative(review, path))
        .unwrap_or_default();
    let place = review.place_of(id);
    let conflicted = place
        .and_then(|index| review.change(index))
        .is_some_and(Changed::is_conflicted);

    v_flex()
        .w_full()
        .h_full()
        .overflow_hidden()
        .bg(theme.colors.background)
        .child(
            bar(theme)
                .child(text(name).text_xs().font_mono())
                .child(h_flex().flex_1())
                .when_some(place.filter(|_| conflicted), |bar, index| {
                    bar.child(worded(
                        theme,
                        "Open Result",
                        true,
                        Message::OpenChangeFile(index),
                    ))
                })
                .when(!conflicted, |bar| bar.child(layout_toggle(theme, split))),
        )
        .when(place.is_none(), |pane| {
            pane.child(nothing(theme, "This file no longer differs from the index"))
        })
        .child(measured(
            review.area(Some(id)),
            v_flex().w_full().flex_1().overflow_hidden().children(drawn(
                theme,
                review,
                Some(id),
                split,
                remarking,
            )),
        ))
}

/// The rows of the pane showing `shown`, from where it is scrolled to.
fn drawn(
    theme: &Theme,
    review: &Review,
    shown: Option<ChangeId>,
    split: bool,
    remarking: Remarking,
) -> Vec<Div<Message>> {
    let rows = rows(review, shown, split);
    let first = review.scroll(shown).min(rows.len().saturating_sub(1));
    let picked = review
        .comments()
        .picked()
        .or_else(|| review.comments().composing().map(|draft| draft.anchor));
    rows.into_iter()
        .skip(first)
        .take(DRAWN)
        .map(|row| {
            let outlined = shown.is_none() && !matches!(row, Row::FileGap | Row::FileEnd);
            self::row(theme, review, row, (shown, &picked), remarking).when(outlined, |row| {
                row.border_side(Side::Left, 1.0, theme.colors.border)
                    .border_side(Side::Right, 1.0, theme.colors.border)
            })
        })
        .collect()
}

/// Where in the review the file `id` names begins.
///
/// This is what moving the review to a file comes to: the row its heading is
/// on is the row the pane is scrolled to, so the file the reader asked for is
/// the first thing under the toolbar.
pub fn row_of(review: &Review, id: ChangeId, split: bool) -> Option<usize> {
    let wanted = review.place_of(id)?;
    rows(review, None, split)
        .into_iter()
        .position(|row| match row {
            Row::File(index, _) => index == wanted,
            _ => false,
        })
}

/// The row of the hunk before or after the one the pane is scrolled to.
///
/// The headings are what the eye lands on, so they are what the arrows move
/// between: a file's own heading counts as one, because the top of a file is
/// somewhere to stop as much as the first change in it is.
pub fn hunk_row(review: &Review, forward: bool, split: bool) -> Option<usize> {
    let at = review.scroll(None);
    let stops = rows(review, None, split)
        .into_iter()
        .enumerate()
        .filter(|(_, row)| matches!(row, Row::File(..) | Row::Heading(..)))
        .map(|(index, _)| index)
        .collect::<Vec<_>>();

    match forward {
        true => stops.into_iter().find(|stop| *stop > at),
        false => stops.into_iter().rev().find(|stop| *stop < at),
    }
}

/// Builds the bar above the changes: how much, the way through, and staging.
///
/// This is Zed's project-diff toolbar: what has changed as a reading, the two
/// arrows through the hunks, then staging — worded, because "Stage" says what
/// it does and a symbol does not.
fn toolbar(theme: &Theme, review: &Review, split: bool) -> Div<Message> {
    let (added, removed) = totals(review);
    let files = review.changed().len();
    let counted = match files {
        1 => "1 file changed".to_owned(),
        files => format!("{files} files changed"),
    };
    let staged = review.staged();
    let acting = review.acting_on().len();
    let idle = !review.is_working();

    bar(theme)
        .child(
            text(review.head().map(pm_core::Head::name).unwrap_or_default())
                .text_xs()
                .font_mono()
                .color(theme.colors.text_muted),
        )
        .when_some(
            review.head().and_then(|head| head.operation.as_ref()),
            |bar, operation| {
                bar.child(
                    text(operation.label())
                        .text_xs()
                        .color(theme.colors.text_muted),
                )
            },
        )
        .child(
            text(counted)
                .text_xs()
                .font_light()
                .color(theme.colors.text_muted),
        )
        .child(diff_stat(theme, added, removed))
        .child(h_flex().flex_1())
        .when(review.comments().any_sent(), |bar| {
            bar.child(worded(
                theme,
                match review.comments().shows_sent() {
                    true => "Hide sent comments",
                    false => "Show sent comments",
                },
                true,
                Message::ToggleSentComments,
            ))
        })
        .child(layout_toggle(theme, split))
        .child(worded(theme, "Edit", files > 0, Message::OpenExcerpts))
        .child(icon_button(theme, IconName::ArrowUp, Message::PreviousHunk))
        .child(icon_button(theme, IconName::ArrowDown, Message::NextHunk))
        .child(worded(
            theme,
            "Stage",
            idle && acting > 0,
            Message::StageSelection,
        ))
        .child(worded(
            theme,
            "Unstage",
            idle && staged > 0,
            Message::UnstageSelection,
        ))
        .child(match staged == files && files > 0 {
            true => worded(theme, "Unstage All", idle, Message::UnstageAll),
            false => worded(theme, "Stage All", idle && files > 0, Message::StageAll),
        })
        .child(turning_icon_button(
            theme,
            IconName::Refresh,
            review.refresh_turn(),
            Message::RefreshChanges,
        ))
}

/// Builds the reading of how much a diff adds and takes out.
fn diff_stat(theme: &Theme, added: usize, removed: usize) -> Div<Message> {
    h_flex()
        .gap(0.5)
        .items_center()
        .child(
            text(format!("+\u{2009}{added}"))
                .text_xs()
                .font_mono()
                .color(theme.colors.success),
        )
        .child(
            text(format!("\u{2012}\u{2009}{removed}"))
                .text_xs()
                .font_mono()
                .color(theme.colors.danger),
        )
}

/// Builds the control that sets a diff's two sides beside each other, or
/// back into one column.
///
/// It says what pressing it does rather than what the diff is now, the way
/// the staging controls beside it do.
fn layout_toggle(theme: &Theme, split: bool) -> Div<Message> {
    worded(
        theme,
        match split {
            true => "Unified",
            false => "Side by Side",
        },
        true,
        Message::TogglePreference(Preference::SplitDiff),
    )
}

/// Builds one of the worded controls a bar of them is made of.
pub(super) fn worded(theme: &Theme, label: &str, enabled: bool, message: Message) -> Div<Message> {
    let color = match enabled {
        true => theme.colors.text_muted,
        false => theme.colors.text_subtle,
    };

    h_flex()
        .h_px(theme.size.icon_control)
        .px(1)
        .items_center()
        .rounded(theme.radius.md)
        .when(enabled, |control| {
            control
                .hover_bg(theme.colors.surface_hover)
                .active_bg(theme.colors.surface_active)
                .on_click(message)
        })
        .child(text(label.to_owned()).text_xs().font_light().color(color))
}

/// Builds the bar below the changes: the message, and what it commits or
/// syncs.
fn commit_bar(theme: &Theme, review: &Review, typing: bool, solid: bool) -> Div<Message> {
    let active = review.active();
    let primary = review.primary(active);
    let pressed = primary_message(active, &primary);

    h_flex()
        .w_full()
        .px(1.5)
        .py(1)
        .gap(1)
        .items_center()
        .bg(theme.colors.surface)
        .child(
            h_flex().flex_1().children(
                review
                    .repository(active)
                    .map(|held| commit_editor(theme, active, held, typing, solid)),
            ),
        )
        .child(
            h_flex()
                .h_px(theme.size.control)
                .px(1.5)
                .items_center()
                .rounded(theme.radius.md)
                .bg(theme.colors.surface_selected)
                .when_some(pressed, |control, message| {
                    control
                        .hover_bg(theme.colors.surface_hover)
                        .active_bg(theme.colors.surface_active)
                        .on_click(message)
                })
                .child(primary_face(theme, &primary, IconName::GitCommit)),
        )
}

/// How many lines the whole review adds and takes out.
fn totals(review: &Review) -> (usize, usize) {
    review
        .changed()
        .iter()
        .map(|changed| counts(review, changed))
        .fold((0, 0), |(added, removed), (one, two)| {
            (added + one, removed + two)
        })
}

/// How far down the pane showing `shown` the scrolling can reach.
pub fn row_count(review: &Review, shown: Option<ChangeId>, split: bool) -> usize {
    rows(review, shown, split).len()
}

/// Builds the bar along the top of either pane, around what it holds.
fn bar(theme: &Theme) -> Div<Message> {
    h_flex()
        .w_full()
        .h_px(theme.size.bar)
        .px(1.5)
        .gap(0.75)
        .items_center()
        .overflow_hidden()
        .bg(theme.colors.surface)
        .child(
            icon(IconName::GitBranch)
                .size(IconSize::XSmall)
                .color(theme.colors.text_subtle),
        )
}

/// Builds what a pane with no diff to draw says instead.
fn nothing(theme: &Theme, said: &str) -> Div<Message> {
    v_flex().w_full().px(2).py(2).child(
        text(said.to_owned())
            .text_sm()
            .font_light()
            .color(theme.colors.text_subtle),
    )
}

/// One row of a review, in the order they are drawn.
enum Row<'a> {
    /// The space before another changed file begins.
    FileGap,
    /// The bottom edge of one changed file.
    FileEnd,
    /// A file: its name, its counts and what can be done to it.
    File(usize, &'a Changed),
    /// Which side of the index the hunks under it are on.
    Side(&'static str),
    /// A hunk's own heading: what it is inside of, and whether it is staged.
    ///
    /// Which file it belongs to and which side of the index it is on travel
    /// with it, because staging one hunk is done from its own heading.
    Heading(usize, bool, usize, &'a Hunk),
    /// One line of a hunk, with the place of the file it is in in the list
    /// and the side of the index its hunk is on, which together say what
    /// colour it is.
    Line(usize, &'a Path, bool, &'a Line),
    /// One row of a hunk set out on two sides: the old line on the left and
    /// the new on the right, either missing where the other side has more.
    Pair(usize, &'a Path, bool, Option<&'a Line>, Option<&'a Line>),
    /// A comment left on lines above it, or on lines that are not drawn.
    Comment(Comment),
    /// The box a comment is being written in.
    Composer(Composing),
    /// The heading of a conflict compared across its two versions.
    CompareHeading(usize, usize),
    /// A current and incoming line displayed side by side.
    ComparePair(Option<&'a str>, Option<&'a str>),
}

/// Every row of the pane showing `shown`, file by file and hunk by hunk.
///
/// A file whose lines are folded away is one row; a file that has no diff to
/// show — one git will not read, a file too large for it — is one row as
/// well, because the heading is what says it changed at all. A pane showing
/// one file draws no heading for it, because its own bar already names it,
/// and folding it away there would leave a pane with nothing in it.
fn rows(review: &Review, shown: Option<ChangeId>, split: bool) -> Vec<Row<'_>> {
    let Some(id) = shown else {
        return review
            .changed()
            .iter()
            .enumerate()
            .flat_map(|(index, changed)| {
                let folded = review.is_collapsed(&changed.path);
                let mut rows = Vec::new();
                if index > 0 {
                    rows.push(Row::FileGap);
                }
                rows.push(Row::File(index, changed));
                if !folded {
                    rows.extend(lines(review, index, changed, split));
                }
                rows.push(Row::FileEnd);
                rows
            })
            .collect();
    };
    let Some(index) = review.place_of(id) else {
        return Vec::new();
    };
    review.change(index).map_or_else(Vec::new, |changed| {
        if changed.is_conflicted() {
            compared(review, changed)
        } else {
            lines(review, index, changed, split)
        }
    })
}

/// Lays out each conflict's two versions in matching rows for comparison.
fn compared<'a>(review: &'a Review, changed: &'a Changed) -> Vec<Row<'a>> {
    let conflicts = review.conflicts(&changed.path);
    let mut rows = Vec::new();
    for (at, block) in conflicts.iter().enumerate() {
        rows.push(Row::CompareHeading(at + 1, conflicts.len()));
        let current = block.current.lines().collect::<Vec<_>>();
        let incoming = block.incoming.lines().collect::<Vec<_>>();
        for line in 0..current.len().max(incoming.len()) {
            rows.push(Row::ComparePair(
                current.get(line).copied(),
                incoming.get(line).copied(),
            ));
        }
    }
    rows
}

/// The rows of one file's diff: each side of the index, hunk by hunk, with
/// the comments left on its lines under them.
///
/// A comment whose lines are not among the ones drawn — the file was
/// commented on from a pane that shows more of it, or the lines are gone —
/// is drawn at the top instead, so it is never a comment nobody can see.
fn lines<'a>(review: &'a Review, index: usize, changed: &'a Changed, split: bool) -> Vec<Row<'a>> {
    if changed.is_conflicted() {
        return vec![Row::Side(
            "Open this file in the editor to resolve conflicts",
        )];
    }
    let Some(patch) = review.patch(&changed.path) else {
        return Vec::new();
    };
    let sides = [
        ("Staged", true, &patch.staged),
        ("Not staged", false, &patch.unstaged),
    ];
    let both = !patch.staged.is_empty() && !patch.unstaged.is_empty();
    let relative = review.relative(&changed.path);
    let comments = review.comments();
    let composing = comments
        .composing()
        .filter(|composing| composing.anchor.path == relative);
    let mut placed = BTreeSet::new();
    let mut composed = false;
    let mut rows = Vec::new();

    let mut attach =
        |rows: &mut Vec<Row<'a>>, staged: bool, old: Option<usize>, new: Option<usize>| {
            let (old_ok, new_ok) = numbering(patch, staged);
            for (side, number, ok) in [
                (CommentSide::Old, old, old_ok),
                (CommentSide::New, new, new_ok),
            ] {
                let Some(number) = number.filter(|_| ok) else {
                    continue;
                };
                for comment in comments.ending_at(&relative, side, number) {
                    placed.insert(comment.id);
                    rows.push(Row::Comment(comment));
                }
                if let Some(composing) = composing.as_ref().filter(|composing| {
                    composing.anchor.side == side && composing.anchor.last == number
                }) {
                    composed = true;
                    rows.push(Row::Composer(composing.clone()));
                }
            }
        };

    for (name, staged, hunks) in sides {
        if hunks.is_empty() {
            continue;
        }
        if both {
            rows.push(Row::Side(name));
        }
        for (at, hunk) in hunks.iter().enumerate() {
            rows.push(Row::Heading(index, staged, at, hunk));
            match split {
                true => {
                    for row in paired(index, &changed.path, staged, hunk) {
                        let numbers = match &row {
                            Row::Pair(_, _, _, old, new) => {
                                (old.and_then(|line| line.old), new.and_then(|line| line.new))
                            }
                            _ => (None, None),
                        };
                        rows.push(row);
                        attach(&mut rows, staged, numbers.0, numbers.1);
                    }
                }
                false => {
                    for line in &hunk.lines {
                        rows.push(Row::Line(index, &changed.path, staged, line));
                        attach(&mut rows, staged, line.old, line.new);
                    }
                }
            }
        }
    }

    let mut first = comments
        .in_file(&relative)
        .into_iter()
        .filter(|comment| !placed.contains(&comment.id))
        .map(Row::Comment)
        .collect::<Vec<_>>();
    if let Some(composing) = composing.filter(|_| !composed) {
        first.push(Row::Composer(composing));
    }
    first.append(&mut rows);
    first
}

/// Which sides of `patch`'s hunks on the `staged` side of the index are
/// numbered the way the worktree and the last commit number their lines:
/// whether a comment can be anchored by their old numbers, and by their new.
///
/// A staged hunk counts its new lines by the index, and an unstaged one its
/// old lines by the index, so once a file is changed on both sides of the
/// index those numbers are of a file that is neither of the two a comment
/// can be about.
fn numbering(patch: &Patch, staged: bool) -> (bool, bool) {
    (
        staged || patch.staged.is_empty(),
        !staged || patch.unstaged.is_empty(),
    )
}

/// The rows of `hunk` set out on two sides.
///
/// A line on both sides is one row, drawn on both. A run of lines taken out
/// and the run put in their place are read against each other, the first
/// taken out beside the first put in, and whichever run is longer goes on
/// alone below the other.
fn paired<'a>(index: usize, path: &'a Path, staged: bool, hunk: &'a Hunk) -> Vec<Row<'a>> {
    let mut rows = Vec::new();
    let mut removed: Vec<&Line> = Vec::new();
    let mut added: Vec<&Line> = Vec::new();
    let flush =
        |rows: &mut Vec<Row<'a>>, removed: &mut Vec<&'a Line>, added: &mut Vec<&'a Line>| {
            let length = removed.len().max(added.len());
            for at in 0..length {
                rows.push(Row::Pair(
                    index,
                    path,
                    staged,
                    removed.get(at).copied(),
                    added.get(at).copied(),
                ));
            }
            removed.clear();
            added.clear();
        };

    for line in &hunk.lines {
        match line.kind {
            LineKind::Removed if !added.is_empty() => {
                flush(&mut rows, &mut removed, &mut added);
                removed.push(line);
            }
            LineKind::Removed => removed.push(line),
            LineKind::Added => added.push(line),
            LineKind::Context => {
                flush(&mut rows, &mut removed, &mut added);
                rows.push(Row::Pair(index, path, staged, Some(line), Some(line)));
            }
        }
    }
    flush(&mut rows, &mut removed, &mut added);
    rows
}

/// Builds one row of a review, whichever kind of row it is.
fn row(
    theme: &Theme,
    review: &Review,
    row: Row<'_>,
    (shown, picked): (Option<ChangeId>, &Option<Anchor>),
    remarking: Remarking,
) -> Div<Message> {
    match row {
        Row::FileGap => v_flex().w_full().h_px(FILE_GAP),
        Row::FileEnd => v_flex().w_full().h_px(1.0).bg(theme.colors.border),
        Row::File(index, changed) => file_row(theme, review, index, changed),
        Row::Side(name) => side_row(theme, name),
        Row::Heading(index, staged, at, hunk) => {
            heading_row(theme, review, index, staged, at, hunk)
        }
        Row::Line(index, path, staged, line) => {
            let spot = Spot::of(review, shown, index, path, staged, picked);
            line_row(theme, line, review.shade(path, staged, line), &spot)
        }
        Row::Pair(index, path, staged, old, new) => {
            let spot = Spot::of(review, shown, index, path, staged, picked);
            h_flex()
                .w_full()
                .h_px(theme.size.row)
                .items_stretch()
                .overflow_hidden()
                .child(half(theme, review, path, staged, old, false, &spot))
                .child(v_flex().w_px(1.0).bg(theme.colors.border))
                .child(half(theme, review, path, staged, new, true, &spot))
        }
        Row::Comment(comment) => comment_block(
            theme,
            &comment,
            theme.size.row,
            NUMBERS + MARK,
            review.comments().moving(),
        ),
        Row::Composer(composing) => composer_block(
            theme,
            &composing,
            theme.size.row,
            NUMBERS + MARK,
            remarking.focused,
            remarking.solid,
        ),
        Row::CompareHeading(at, total) => compare_heading(theme, at, total),
        Row::ComparePair(current, incoming) => compare_pair(theme, current, incoming),
    }
}

/// Builds a heading above one side-by-side conflict comparison.
fn compare_heading(theme: &Theme, at: usize, total: usize) -> Div<Message> {
    h_flex()
        .w_full()
        .h_px(theme.size.row)
        .bg(theme.colors.surface)
        .child(
            h_flex().flex_1().px(1.5).items_center().child(
                text(format!("Current · Conflict {at} of {total}"))
                    .text_xs()
                    .color(theme.colors.accent),
            ),
        )
        .child(v_flex().w_px(1.0).bg(theme.colors.border))
        .child(
            h_flex()
                .flex_1()
                .px(1.5)
                .items_center()
                .child(text("Incoming").text_xs().color(theme.colors.success)),
        )
}

/// Builds the current and incoming text of one comparison row.
fn compare_pair(theme: &Theme, current: Option<&str>, incoming: Option<&str>) -> Div<Message> {
    h_flex()
        .w_full()
        .h_px(theme.size.row)
        .items_stretch()
        .child(compare_half(theme, current, true))
        .child(v_flex().w_px(1.0).bg(theme.colors.border))
        .child(compare_half(theme, incoming, false))
}

/// Builds one half of a conflict comparison row.
fn compare_half(theme: &Theme, line: Option<&str>, current: bool) -> Div<Message> {
    let color = match current {
        true => theme.colors.accent,
        false => theme.colors.success,
    };
    h_flex()
        .flex_1()
        .px(1.5)
        .items_center()
        .overflow_hidden()
        .bg(color.alpha(theme.emphasis.change))
        .child(
            text(line.unwrap_or_default().to_owned())
                .text_sm()
                .font_mono(),
        )
}

/// Builds the heading of one file: its path, its counts and its controls.
///
/// The heading is a row of the same list the sidebar draws and is lit only
/// when marked; a click anywhere on it folds its lines away or back out, and
/// the chevron shows which way it is.
fn file_row(theme: &Theme, review: &Review, index: usize, changed: &Changed) -> Div<Message> {
    let collapsed = review.is_collapsed(&changed.path);
    let (added, removed) = counts(review, changed);
    let marked = review.id_of(index).is_some_and(|id| review.is_marked(id));
    let path = relative(review, &changed.path);
    let name = Path::new(&path)
        .file_name()
        .map_or(path.as_str(), |name| name.to_str().unwrap_or(&path));
    let directory = path.strip_suffix(name).unwrap_or_default();

    h_flex()
        .w_full()
        .h_px(theme.size.field)
        .px(1.5)
        .gap(0.75)
        .items_center()
        .overflow_hidden()
        .bg(theme.colors.surface)
        .border_side(Side::Top, 1.0, theme.colors.border)
        .border_side(Side::Bottom, 1.0, theme.colors.border_variant)
        .when(marked, |row| row.bg(theme.colors.surface_selected))
        .hover_bg(theme.colors.surface_hover)
        .on_click(match changed.is_conflicted() {
            true => Message::OpenChangeFile(index),
            false => Message::ExpandChange(index),
        })
        .on_secondary_click(Message::ShowChangeMenu(index))
        .child(
            v_flex()
                .size_px(theme.size.icon_control)
                .items_center()
                .justify_center()
                .child(
                    icon(match (changed.is_conflicted(), collapsed) {
                        (true, _) | (false, true) => IconName::ChevronRight,
                        (false, false) => IconName::ChevronDown,
                    })
                    .size(IconSize::XSmall)
                    .color(theme.colors.text_subtle),
                ),
        )
        .child(
            h_flex()
                .items_center()
                .overflow_hidden()
                .child(
                    text(directory.to_owned())
                        .text_sm()
                        .font_mono()
                        .color(theme.colors.text_muted),
                )
                .child(text(name.to_owned()).text_sm().font_mono().font_semibold()),
        )
        .child(
            text(changed.mark().letter())
                .text_xs()
                .font_mono()
                .color(status_color(theme, changed.mark())),
        )
        .child(h_flex().flex_1())
        .when(added > 0, |row| {
            row.child(
                text(format!("+{added}"))
                    .text_xs()
                    .font_mono()
                    .color(theme.colors.success),
            )
        })
        .when(removed > 0, |row| {
            row.child(
                text(format!("−{removed}"))
                    .text_xs()
                    .font_mono()
                    .color(theme.colors.danger),
            )
        })
        .child(icon_button(
            theme,
            IconName::ArrowRight,
            Message::OpenChangeFile(index),
        ))
        .child(checkbox(
            theme,
            staged_state(changed),
            Message::ToggleChangeStaged(index),
        ))
}

/// Builds the line saying which side of the index the hunks under it are on.
fn side_row(theme: &Theme, name: &str) -> Div<Message> {
    h_flex()
        .w_full()
        .h_px(theme.size.row)
        .px(1.5)
        .items_center()
        .overflow_hidden()
        .child(
            text(name.to_owned())
                .text_xs()
                .font_light()
                .color(theme.colors.text_subtle),
        )
}

/// Builds a hunk's own heading: where it is, and the box that stages it.
///
/// One hunk is staged on its own the way a whole file is — by the box on its
/// row — because a reader who has read one change and wants that change in
/// the next commit should not have to take the rest of the file with it. A
/// file git has never been told about has no index entry to write part of,
/// so its hunks are staged by staging the file.
fn heading_row(
    theme: &Theme,
    review: &Review,
    index: usize,
    staged: bool,
    at: usize,
    hunk: &Hunk,
) -> Div<Message> {
    let separate = review
        .change(index)
        .is_some_and(|changed| !changed.is_untracked() && !changed.is_conflicted());

    h_flex()
        .w_full()
        .h_px(theme.size.row)
        .pr(1.5)
        .gap(1)
        .items_center()
        .overflow_hidden()
        .bg(theme.colors.accent.alpha(theme.emphasis.change))
        .child(
            h_flex().w_px(NUMBERS).h_full().bg(theme
                .colors
                .accent
                .alpha(theme.emphasis.change * GUTTER_DEPTH)),
        )
        .child(
            text(range_of(hunk))
                .text_xs()
                .font_mono()
                .color(theme.colors.text_muted),
        )
        .child(
            text(hunk.heading.clone())
                .text_xs()
                .font_mono()
                .color(theme.colors.text_muted),
        )
        .when_some(
            review
                .change(index)
                .and_then(|changed| review.patch(&changed.path))
                .and_then(|patch| patch.attribution.get(&(staged, at))),
            |row, step| row.child(provenance(theme, review.checkpoint_scope, step)),
        )
        .child(h_flex().flex_1())
        .when(hunk_anchor(review, index, staged, hunk).is_some(), |row| {
            row.child(worded(
                theme,
                "Comment on hunk",
                true,
                Message::CommentOnHunk(index, staged, at),
            ))
        })
        .when(separate, |row| {
            row.child(worded(
                theme,
                match staged {
                    true => "Unstage",
                    false => "Stage",
                },
                true,
                Message::ToggleHunkStaged(index, staged, at),
            ))
            .child(worded(
                theme,
                "Restore",
                true,
                Message::RestoreHunk(index, staged, at),
            ))
        })
}

/// The range a hunk covers, written the way git heads it: `@@ -22,6 +22,7 @@`.
fn range_of(hunk: &Hunk) -> String {
    format!(
        "@@ -{},{} +{},{} @@",
        hunk.old_start, hunk.old_count, hunk.start, hunk.new_count
    )
}

/// What the number column of a row of a hunk needs to be a way to comment:
/// which change the row is of, which sides of it can be commented on, and
/// the lines being selected or commented on.
struct Spot {
    /// Which file the pane is of, when it is of one alone.
    shown: Option<ChangeId>,
    /// Where the file is in the list of changes.
    index: usize,
    /// Whether the old numbers of the row's hunk are the last commit's.
    old: bool,
    /// Whether the new numbers of the row's hunk are the worktree's.
    new: bool,
    /// Whether a comment is waiting for a line to be put on.
    moving: bool,
    /// The selected or drafted lines, when they are in this file.
    picked: Option<Anchor>,
}

impl Spot {
    /// The spot of a row in the `index`-th change's `staged` hunks.
    fn of(
        review: &Review,
        shown: Option<ChangeId>,
        index: usize,
        path: &Path,
        staged: bool,
        picked: &Option<Anchor>,
    ) -> Self {
        let relative = review.relative(path);
        let (old, new) = review
            .patch(path)
            .map_or((false, false), |patch| numbering(patch, staged));
        Self {
            shown,
            index,
            picked: picked.clone().filter(|picked| picked.path == relative),
            old,
            new,
            moving: review.comments().moving().is_some(),
        }
    }

    /// Whether `number` on `side` can be commented on.
    fn allows(&self, side: CommentSide) -> bool {
        match side {
            CommentSide::Old => self.old,
            CommentSide::New => self.new,
        }
    }

    /// Whether `number` on `side` is selected or being commented on.
    fn sweeps(&self, side: CommentSide, number: usize) -> bool {
        self.picked.as_ref().is_some_and(|picked| {
            picked.side == side && (picked.first..=picked.last).contains(&number)
        })
    }
}

/// Builds the column of numbers `numbers` is, as a place to comment on
/// `number` on `side`: pressing it comments on that line, dragging down it
/// comments on the run of lines, and while the pointer is on it the numbers
/// are a button that says so.
fn commentable(
    theme: &Theme,
    spot: &Spot,
    side: CommentSide,
    number: usize,
    width: f32,
    numbers: Div<Message>,
) -> Div<Message> {
    let (shown, index) = (spot.shown, spot.index);
    let label = match spot.moving {
        true => "Move here",
        false => "+ Comment",
    };
    let button = h_flex()
        .w_px(width)
        .items_center()
        .justify_center()
        .bg(theme
            .colors
            .accent
            .alpha(theme.emphasis.change * GUTTER_DEPTH))
        .child(text(label).text_xs().color(theme.colors.accent));
    h_flex()
        .w_px(width)
        .items_stretch()
        .on_drag(move |event| Message::DragComment(shown, index, side, number, event))
        .child(revealing(numbers, button))
}

/// Builds one line of a hunk: its numbers on each side, its mark, then the
/// line itself, coloured by `shade` the way the file it came from is.
///
/// This is GitHub's unified diff: the numbers sit in a gutter washed deeper
/// than the line beside it, so the eye finds where a change is by the edge
/// before it reads what the change is.
fn line_row(
    theme: &Theme,
    line: &Line,
    shade: Option<&[Option<Highlight>]>,
    spot: &Spot,
) -> Div<Message> {
    let side = match line.kind {
        LineKind::Removed => CommentSide::Old,
        LineKind::Added | LineKind::Context => CommentSide::New,
    };
    let number_here = match side {
        CommentSide::Old => line.old,
        CommentSide::New => line.new,
    }
    .filter(|_| spot.allows(side));
    let swept = number_here.is_some_and(|number| spot.sweeps(side, number));
    let (gutter, wash) = comment_washes(theme, Some(line.kind), swept);
    let numbers = h_flex()
        .w_px(NUMBERS)
        .px(0.5)
        .gap(0.5)
        .items_center()
        .justify_end()
        .when_some(gutter, Div::bg)
        .child(number(theme, line.old))
        .child(number(theme, line.new));
    let column = match number_here {
        Some(number) => commentable(theme, spot, side, number, NUMBERS, numbers),
        None => numbers,
    };

    h_flex()
        .w_full()
        .h_px(theme.size.row)
        .items_stretch()
        .overflow_hidden()
        .when_some(wash, Div::bg)
        .child(column)
        .child(marked(theme, Some(line)))
        .child(shaded(theme, &line.text, shade.unwrap_or_default()))
}

/// Builds one side of a row of a split diff: the line's number on that
/// side, its mark, then the line, or a filler where that side has no line.
///
/// A line on both sides is washed on neither, since nothing happened to it.
/// The side that has nothing across from a line taken out or put in is
/// filled in the quiet colour of the bars, the way GitHub greys it, so the
/// gap reads as a gap rather than as an unchanged blank line.
fn half(
    theme: &Theme,
    review: &Review,
    path: &Path,
    staged: bool,
    line: Option<&Line>,
    new: bool,
    spot: &Spot,
) -> Div<Message> {
    let number = line.and_then(|line| match new {
        true => line.new,
        false => line.old,
    });
    let shade = line.and_then(|line| review.shade(path, staged, line));
    let side = match new {
        true => CommentSide::New,
        false => CommentSide::Old,
    };
    let commenting = number.filter(|_| spot.allows(side));
    let swept = commenting.is_some_and(|number| spot.sweeps(side, number));
    let (gutter, wash) = comment_washes(theme, line.map(|line| line.kind), swept);
    let numbers = h_flex()
        .w_px(SIDE_NUMBER)
        .px(0.5)
        .items_center()
        .justify_end()
        .when_some(gutter, Div::bg)
        .when_some(number, |slot, number| {
            slot.child(
                text(number.to_string())
                    .text_xs()
                    .font_mono()
                    .color(theme.colors.text_subtle),
            )
        });
    let column = match commenting {
        Some(number) => commentable(theme, spot, side, number, SIDE_NUMBER, numbers),
        None => numbers,
    };

    h_flex()
        .flex_1()
        .items_stretch()
        .overflow_hidden()
        .when_some(wash, Div::bg)
        .child(column)
        .child(marked(theme, line))
        .child(match line {
            Some(line) => shaded(theme, &line.text, shade.unwrap_or_default()),
            None => h_flex().child(text(" ").text_sm().font_mono()),
        })
}

/// Highlights the full selected comment range, retaining the diff wash elsewhere.
fn comment_washes(
    theme: &Theme,
    kind: Option<LineKind>,
    selected: bool,
) -> (Option<Rgba>, Option<Rgba>) {
    match selected {
        true => (
            Some(theme.colors.accent.alpha(theme.emphasis.selection)),
            Some(theme.colors.selection.alpha(theme.emphasis.selection)),
        ),
        false => washes(theme, kind),
    }
}

/// The washes a line of `kind` is drawn on: the deeper one of its gutter,
/// then the one under its text.
///
/// A line both sides share is washed on neither; a side with no line at all
/// is filled in the colour of the bars on both.
fn washes(theme: &Theme, kind: Option<LineKind>) -> (Option<Rgba>, Option<Rgba>) {
    let changed = |color: Rgba| {
        (
            Some(color.alpha(theme.emphasis.change * GUTTER_DEPTH)),
            Some(color.alpha(theme.emphasis.change)),
        )
    };
    match kind {
        Some(LineKind::Added) => changed(theme.colors.success),
        Some(LineKind::Removed) => changed(theme.colors.danger),
        Some(LineKind::Context) => (None, None),
        None => (Some(theme.colors.surface), Some(theme.colors.surface)),
    }
}

/// Builds the column a line's `+` or `−` is written in, blank for a line
/// both sides share or a side with no line.
fn marked(theme: &Theme, line: Option<&Line>) -> Div<Message> {
    let mark = match line.map(|line| line.kind) {
        Some(LineKind::Added) => "+",
        Some(LineKind::Removed) => "−",
        _ => " ",
    };

    h_flex().w_px(MARK).items_center().justify_center().child(
        text(mark)
            .text_sm()
            .font_mono()
            .color(theme.colors.text_muted),
    )
}

/// Builds `said` as runs of one colour each, as `shade` colours it.
///
/// A character `shade` says nothing about is drawn in the body colour, the
/// way the editor draws one its grammar says nothing about.
fn shaded(theme: &Theme, said: &str, shade: &[Option<Highlight>]) -> Div<Message> {
    let mut runs: Vec<(Option<Highlight>, String)> = Vec::new();
    for (column, ch) in said.chars().enumerate() {
        let highlight = shade.get(column).copied().flatten();
        match runs.last_mut() {
            Some((last, run)) if *last == highlight => run.push(ch),
            _ => runs.push((highlight, ch.to_string())),
        }
    }

    h_flex()
        .items_center()
        .children(runs.into_iter().map(|(highlight, run)| {
            let color = highlight.map_or(theme.colors.text, |highlight| tint(highlight, theme));
            text(run).text_sm().font_mono().color(color)
        }))
}

/// Builds one of a line's numbers, or the blank where it has none.
fn number(theme: &Theme, line: Option<usize>) -> Div<Message> {
    h_flex()
        .w_px(NUMBERS / 2.0 - 6.0)
        .justify_end()
        .when_some(line, |slot, line| {
            slot.child(
                text(line.to_string())
                    .text_xs()
                    .font_mono()
                    .color(theme.colors.text_subtle),
            )
        })
}

/// Where `path` sits in the worktree being reviewed.
fn relative(review: &Review, path: &std::path::Path) -> String {
    path.strip_prefix(review.root())
        .unwrap_or(path)
        .display()
        .to_string()
}

/// How many lines one file's change adds and takes out, both sides counted.
fn counts(review: &Review, changed: &Changed) -> (usize, usize) {
    let Some(patch) = review.patch(&changed.path) else {
        return (0, 0);
    };
    let hunks = patch.staged.iter().chain(&patch.unstaged);
    hunks.fold((0, 0), |(added, removed), hunk| {
        (added + hunk.added(), removed + hunk.removed())
    })
}

/// The run of lines a comment on `hunk` is anchored to: the lines it puts in
/// the worktree, or the lines it removes when it puts none.
///
/// Nothing is answered for a hunk whose lines are numbered by the index on
/// both counts, which no comment can be about.
pub fn hunk_anchor(
    review: &Review,
    index: usize,
    staged: bool,
    hunk: &Hunk,
) -> Option<(CommentSide, usize, usize)> {
    let path = &review.change(index)?.path;
    let (old_ok, new_ok) = numbering(review.patch(path)?, staged);
    match () {
        () if new_ok && hunk.new_count > 0 => Some((
            CommentSide::New,
            hunk.start,
            hunk.start + hunk.new_count - 1,
        )),
        () if old_ok && hunk.old_count > 0 => Some((
            CommentSide::Old,
            hunk.old_start,
            hunk.old_start + hunk.old_count - 1,
        )),
        () => None,
    }
}

/// How tall `row` is drawn.
///
/// Rows have no height of their own to ask for once they are drawn, so the
/// heights are written down here beside the rows: a gesture that ends
/// somewhere down the pane is turned into a line by adding them up.
fn height(theme: &Theme, row: &Row<'_>) -> f32 {
    match row {
        Row::FileGap => FILE_GAP,
        Row::FileEnd => 1.0,
        Row::File(..) => theme.size.field,
        Row::Comment(comment) => block_rows(comment) as f32 * theme.size.row,
        Row::Composer(_) => composer_rows() as f32 * theme.size.row,
        _ => theme.size.row,
    }
}

/// The line, counted on `side` in the `index`-th change, that `row` is.
fn numbered(row: &Row<'_>, index: usize, side: CommentSide) -> Option<usize> {
    let of = |line: &Line| match side {
        CommentSide::Old => line.old,
        CommentSide::New => line.new,
    };
    match row {
        Row::Line(place, _, _, line) if *place == index => of(line),
        Row::Pair(place, _, _, old, new) if *place == index => match side {
            CommentSide::Old => old.and_then(of),
            CommentSide::New => new.and_then(of),
        },
        _ => None,
    }
}

/// The line of the `index`-th change, counted on `side`, that a pointer
/// `y` pixels down the window has reached in the pane showing `shown`.
///
/// A pointer over something that is not a line of that file and side — a
/// comment, a heading, the space past the end — is taken to be at the line
/// nearest to it.
pub fn line_at(
    theme: &Theme,
    review: &Review,
    (shown, split): (Option<ChangeId>, bool),
    (index, side): (usize, CommentSide),
    y: f32,
) -> Option<usize> {
    let rows = rows(review, shown, split);
    let first = review.scroll(shown).min(rows.len().saturating_sub(1));
    let mut top = review.area(shown).get().top();
    let mut reached = rows.len().saturating_sub(1);
    for (at, row) in rows.iter().enumerate().skip(first) {
        let bottom = top + height(theme, row);
        if y < bottom {
            reached = at;
            break;
        }
        top = bottom;
    }
    (0..rows.len()).find_map(|distance| {
        let below = rows
            .get(reached + distance)
            .and_then(|row| numbered(row, index, side));
        let above = reached
            .checked_sub(distance)
            .and_then(|at| rows.get(at))
            .and_then(|row| numbered(row, index, side));
        below.or(above)
    })
}

/// The immutable content and scrolling of a persisted turn comparison pane.
#[derive(Default)]
pub(crate) struct TurnDiff {
    /// Files, hunks and their optional conservative provenance.
    pub(crate) files: BTreeMap<PathBuf, Vec<(Hunk, Option<pm_core::CheckpointStep>)>>,
    /// The first visible row.
    pub(crate) scroll: usize,
}

/// A compact, stable identifier for a checkpoint heading's transcript target.
pub fn step_prefix(step: &pm_core::CheckpointStep) -> u64 {
    u64::from_str_radix(step.commit.get(..16).unwrap_or_default(), 16).unwrap_or_default()
}

/// Draws a provenance label and enables navigation only for an identified call.
fn provenance(
    theme: &Theme,
    scope: Option<pm_core::Scope>,
    step: &pm_core::CheckpointStep,
) -> Div<Message> {
    h_flex()
        .items_center()
        .child(
            text(format!("{} · turn {}", step.title, step.turn))
                .text_xs()
                .color(theme.colors.text_muted),
        )
        .when_some(scope.filter(|_| step.tool.is_some()), |label, scope| {
            label.on_click(Message::ShowCheckpointStep(scope, step_prefix(step)))
        })
}

/// Builds a read-only turn comparison with the review's hunk and line primitives.
pub(crate) fn turns_pane(
    theme: &Theme,
    scope: pm_core::Scope,
    span: crate::panes::TurnSpan,
    diff: &TurnDiff,
    split: bool,
) -> Div<Message> {
    let rows = turn_rows(diff, split);
    let empty = rows.is_empty();
    let visible = rows
        .into_iter()
        .skip(diff.scroll)
        .take(DRAWN)
        .map(|row| turn_row(theme, scope, row));
    v_flex()
        .w_full()
        .h_full()
        .overflow_hidden()
        .bg(theme.colors.background)
        .child(
            h_flex()
                .w_full()
                .items_center()
                .gap(1)
                .child(text(format!("Turn {} → {}", span.from, span.to)).text_sm())
                .child(layout_toggle(theme, split))
                .child(worded(
                    theme,
                    "Rewind to first turn",
                    true,
                    Message::RequestRewind(scope, span.from),
                ))
                .child(worded(
                    theme,
                    "Rewind to second turn",
                    true,
                    Message::RequestRewind(scope, span.to),
                )),
        )
        .child(
            v_flex()
                .w_full()
                .flex_1()
                .overflow_hidden()
                .when(empty, |pane| {
                    pane.child(nothing(theme, "No changes between these turns"))
                })
                .children(visible),
        )
}

/// One lightweight row of a turn comparison, laid out only when visible.
enum TurnRow<'a> {
    /// A changed file's heading.
    File(&'a Path),
    /// A binary or metadata-only file change.
    Metadata,
    /// A hunk's ranges and conservative provenance.
    Heading(&'a Hunk, Option<&'a pm_core::CheckpointStep>),
    /// A unified diff line.
    Line(&'a Line),
    /// The old and new lines in a split comparison.
    Pair(Option<&'a Line>, Option<&'a Line>),
}

/// Lists comparison rows without constructing off-screen UI elements.
fn turn_rows(diff: &TurnDiff, split: bool) -> Vec<TurnRow<'_>> {
    let mut rows = Vec::new();
    for (path, hunks) in &diff.files {
        rows.push(TurnRow::File(path));
        if hunks.is_empty() {
            rows.push(TurnRow::Metadata);
        }
        for (hunk, step) in hunks {
            rows.push(TurnRow::Heading(hunk, step.as_ref()));
            if split {
                rows.extend(
                    paired(0, path, false, hunk)
                        .into_iter()
                        .filter_map(|row| match row {
                            Row::Pair(_, _, _, old, new) => Some(TurnRow::Pair(old, new)),
                            _ => None,
                        }),
                );
            } else {
                rows.extend(hunk.lines.iter().map(TurnRow::Line));
            }
        }
    }
    rows
}

impl TurnDiff {
    /// Counts rows in the currently selected diff layout.
    pub(crate) fn row_count(&self, split: bool) -> usize {
        turn_rows(self, split).len()
    }
}

/// Draws one visible turn row with the review's existing line and hunk primitives.
fn turn_row(theme: &Theme, scope: pm_core::Scope, row: TurnRow<'_>) -> Div<Message> {
    match row {
        TurnRow::File(path) => h_flex().h_px(theme.size.row).child(
            text(path.display().to_string())
                .text_sm()
                .font_mono()
                .color(theme.colors.text),
        ),
        TurnRow::Metadata => h_flex()
            .h_px(theme.size.row)
            .child(text("Binary content or file metadata changed").text_xs()),
        TurnRow::Heading(hunk, step) => h_flex()
            .w_full()
            .h_px(theme.size.row)
            .items_center()
            .gap(1)
            .bg(theme.colors.accent.alpha(theme.emphasis.change))
            .child(text(range_of(hunk)).text_xs().font_mono())
            .child(text(hunk.heading.clone()).text_xs())
            .when_some(step, |row, step| {
                row.child(provenance(theme, Some(scope), step))
            }),
        TurnRow::Line(line) => line_row(
            theme,
            line,
            None,
            &Spot {
                shown: None,
                index: 0,
                old: false,
                new: false,
                moving: false,
                picked: None,
            },
        ),
        TurnRow::Pair(old, new) => readonly_pair(theme, old, new),
    }
}

/// Draws one read-only split row using the review's numbers, marks and text styles.
fn readonly_pair(theme: &Theme, old: Option<&Line>, new: Option<&Line>) -> Div<Message> {
    h_flex()
        .w_full()
        .h_px(theme.size.row)
        .items_stretch()
        .child(readonly_half(theme, old, false))
        .child(readonly_half(theme, new, true))
}

/// Draws one half of a checkpoint row with the review's line washes.
fn readonly_half(theme: &Theme, line: Option<&Line>, new: bool) -> Div<Message> {
    let (gutter, wash) = washes(theme, line.map(|line| line.kind));
    h_flex()
        .flex_1()
        .overflow_hidden()
        .when_some(wash, Div::bg)
        .child(
            h_flex()
                .w_px(SIDE_NUMBER)
                .justify_end()
                .when_some(gutter, Div::bg)
                .child(number(
                    theme,
                    line.and_then(|line| if new { line.new } else { line.old }),
                )),
        )
        .child(marked(theme, line))
        .child(shaded(
            theme,
            line.map_or(" ", |line| line.text.as_str()),
            &[],
        ))
}
