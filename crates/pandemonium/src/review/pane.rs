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

use std::path::Path;

use pm_core::{Changed, Hunk, Line, LineKind};
use pm_text::Highlight;
use pm_ui::{
    Div, IconName, IconSize, Styled, Theme, checkbox, h_flex, icon, icon_button, text, v_flex,
};

use crate::editor::tint;
use crate::message::Message;
use crate::review::action::{primary_face, primary_message};
use crate::review::commit_editor;
use crate::review::sidebar::{staged_state, status_color};
use crate::review::store::{ChangeId, Review};

/// How many rows are built at once, however far the review runs on.
///
/// The pane clips whatever will not fit, and a change of a thousand files is
/// not a thousand rows of layout: what is built is the part the reader is
/// looking at and enough beyond it to fill any pane.
const DRAWN: usize = 400;

/// How wide the column of line numbers is drawn.
const NUMBERS: f32 = 76.0;

/// Builds the review of everything one project has changed.
///
/// `typing` says the commit message at the foot of it has the keyboard, so
/// that the caret is drawn in the field the reader is actually in — the same
/// message the sidebar shows, because there is one message and two places it
/// can be written.
pub fn review_pane(theme: &Theme, review: &Review, typing: bool) -> Div<Message> {
    let empty = review.changed().is_empty();

    v_flex()
        .w_full()
        .h_full()
        .overflow_hidden()
        .bg(theme.colors.background)
        .child(toolbar(theme, review))
        .child(
            v_flex()
                .w_full()
                .flex_1()
                .overflow_hidden()
                .when(empty, |pane| {
                    pane.child(nothing(theme, "No uncommitted changes"))
                })
                .children(drawn(theme, review, None)),
        )
        .when(!empty, |pane| pane.child(commit_bar(theme, review, typing)))
}

/// Builds the diff of the one changed file `id` names.
///
/// A file that has stopped differing — committed, or put back the way it was
/// — keeps its pane and says so, rather than the tab closing itself under a
/// reader who was in the middle of it.
pub fn change_pane(theme: &Theme, review: &Review, id: ChangeId) -> Div<Message> {
    let name = review
        .path_of(id)
        .map(|path| relative(review, path))
        .unwrap_or_default();
    let place = review.place_of(id);

    v_flex()
        .w_full()
        .h_full()
        .overflow_hidden()
        .bg(theme.colors.background)
        .child(
            bar(theme)
                .child(text(name).text_xs().font_mono())
                .child(h_flex().flex_1()),
        )
        .when(place.is_none(), |pane| {
            pane.child(nothing(theme, "This file no longer differs from the index"))
        })
        .children(drawn(theme, review, Some(id)))
}

/// The rows of the pane showing `shown`, from where it is scrolled to.
fn drawn(theme: &Theme, review: &Review, shown: Option<ChangeId>) -> Vec<Div<Message>> {
    rows(review, shown)
        .into_iter()
        .skip(review.scroll(shown))
        .take(DRAWN)
        .map(|row| self::row(theme, review, row))
        .collect()
}

/// Where in the review the file `id` names begins.
///
/// This is what moving the review to a file comes to: the row its heading is
/// on is the row the pane is scrolled to, so the file the reader asked for is
/// the first thing under the toolbar.
pub fn row_of(review: &Review, id: ChangeId) -> Option<usize> {
    let wanted = review.place_of(id)?;
    rows(review, None).into_iter().position(|row| match row {
        Row::File(index, _) => index == wanted,
        _ => false,
    })
}

/// The row of the hunk before or after the one the pane is scrolled to.
///
/// The headings are what the eye lands on, so they are what the arrows move
/// between: a file's own heading counts as one, because the top of a file is
/// somewhere to stop as much as the first change in it is.
pub fn hunk_row(review: &Review, forward: bool) -> Option<usize> {
    let at = review.scroll(None);
    let stops = rows(review, None)
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
fn toolbar(theme: &Theme, review: &Review) -> Div<Message> {
    let (added, removed) = totals(review);
    let files = review.changed().len();
    let counted = match files {
        1 => "1 file changed".to_owned(),
        files => format!("{files} files changed"),
    };
    let staged = review.staged();
    let acting = review.acting_on().len();

    bar(theme)
        .child(
            text(review.head().name())
                .text_xs()
                .font_mono()
                .color(theme.colors.text_muted),
        )
        .child(
            text(counted)
                .text_xs()
                .font_light()
                .color(theme.colors.text_muted),
        )
        .child(diff_stat(theme, added, removed))
        .child(h_flex().flex_1())
        .child(icon_button(theme, IconName::ArrowUp, Message::PreviousHunk))
        .child(icon_button(theme, IconName::ArrowDown, Message::NextHunk))
        .child(worded(theme, "Stage", acting > 0, Message::StageSelection))
        .child(worded(
            theme,
            "Unstage",
            staged > 0,
            Message::UnstageSelection,
        ))
        .child(match staged == files && files > 0 {
            true => worded(theme, "Unstage All", true, Message::UnstageAll),
            false => worded(theme, "Stage All", files > 0, Message::StageAll),
        })
        .child(icon_button(
            theme,
            IconName::Refresh,
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

/// Builds one of the worded controls a bar of them is made of.
fn worded(theme: &Theme, label: &str, enabled: bool, message: Message) -> Div<Message> {
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
fn commit_bar(theme: &Theme, review: &Review, typing: bool) -> Div<Message> {
    let primary = review.primary();
    let pressed = primary_message(&primary);

    h_flex()
        .w_full()
        .px(1.5)
        .py(1)
        .gap(1)
        .items_center()
        .bg(theme.colors.surface)
        .child(
            h_flex()
                .flex_1()
                .child(commit_editor(theme, review, typing)),
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
pub fn row_count(review: &Review, shown: Option<ChangeId>) -> usize {
    rows(review, shown).len()
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
    /// A file: its name, its counts and what can be done to it.
    File(usize, &'a Changed),
    /// Which side of the index the hunks under it are on.
    Side(&'static str),
    /// A hunk's own heading: what it is inside of, and whether it is staged.
    ///
    /// Which file it belongs to and which side of the index it is on travel
    /// with it, because staging one hunk is done from its own heading.
    Heading(usize, bool, usize, &'a Hunk),
    /// One line of a hunk, with the file it is in and the side of the
    /// index its hunk is on, which together say what colour it is.
    Line(&'a Path, bool, &'a Line),
}

/// Every row of the pane showing `shown`, file by file and hunk by hunk.
///
/// A file whose lines are folded away is one row; a file that has no diff to
/// show — one git will not read, a file too large for it — is one row as
/// well, because the heading is what says it changed at all. A pane showing
/// one file draws no heading for it, because its own bar already names it,
/// and folding it away there would leave a pane with nothing in it.
fn rows(review: &Review, shown: Option<ChangeId>) -> Vec<Row<'_>> {
    let Some(id) = shown else {
        return review
            .changed()
            .iter()
            .enumerate()
            .flat_map(|(index, changed)| {
                let folded = review.is_collapsed(&changed.path);
                let mut rows = vec![Row::File(index, changed)];
                if !folded {
                    rows.extend(lines(review, index, changed));
                }
                rows
            })
            .collect();
    };
    let Some(index) = review.place_of(id) else {
        return Vec::new();
    };
    review
        .change(index)
        .map(|changed| lines(review, index, changed))
        .unwrap_or_default()
}

/// The rows of one file's diff: each side of the index, hunk by hunk.
fn lines<'a>(review: &'a Review, index: usize, changed: &'a Changed) -> Vec<Row<'a>> {
    let Some(patch) = review.patch(&changed.path) else {
        return Vec::new();
    };
    let sides = [
        ("Staged", true, &patch.staged),
        ("Not staged", false, &patch.unstaged),
    ];
    let both = !patch.staged.is_empty() && !patch.unstaged.is_empty();
    let mut rows = Vec::new();

    for (name, staged, hunks) in sides {
        if hunks.is_empty() {
            continue;
        }
        if both {
            rows.push(Row::Side(name));
        }
        for (at, hunk) in hunks.iter().enumerate() {
            rows.push(Row::Heading(index, staged, at, hunk));
            rows.extend(
                hunk.lines
                    .iter()
                    .map(|line| Row::Line(&changed.path, staged, line)),
            );
        }
    }
    rows
}

/// Builds one row of a review, whichever kind of row it is.
fn row(theme: &Theme, review: &Review, row: Row<'_>) -> Div<Message> {
    match row {
        Row::File(index, changed) => file_row(theme, review, index, changed),
        Row::Side(name) => side_row(theme, name),
        Row::Heading(index, staged, at, hunk) => {
            heading_row(theme, review, index, staged, at, hunk)
        }
        Row::Line(path, staged, line) => line_row(theme, line, review.shade(path, staged, line)),
    }
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

    h_flex()
        .w_full()
        .h_px(theme.size.row)
        .px(1.5)
        .gap(0.5)
        .items_center()
        .overflow_hidden()
        .bg(theme.colors.surface)
        .when(marked, |row| row.bg(theme.colors.surface_selected))
        .hover_bg(theme.colors.surface_hover)
        .on_click(Message::ExpandChange(index))
        .on_secondary_click(Message::ShowChangeMenu(index))
        .child(
            v_flex()
                .size_px(theme.size.icon_control)
                .items_center()
                .justify_center()
                .child(
                    icon(match collapsed {
                        true => IconName::ChevronRight,
                        false => IconName::ChevronDown,
                    })
                    .size(IconSize::XSmall)
                    .color(theme.colors.text_subtle),
                ),
        )
        .child(text(relative(review, &changed.path)).text_sm().font_mono())
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
        .px(1.5)
        .gap(1)
        .items_center()
        .overflow_hidden()
        .bg(theme.colors.surface)
        .child(
            text(format!("@@ {}", hunk.start))
                .text_xs()
                .font_mono()
                .color(theme.colors.text_subtle),
        )
        .child(
            text(hunk.heading.clone())
                .text_xs()
                .font_mono()
                .color(theme.colors.text_subtle),
        )
        .child(h_flex().flex_1())
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

/// Builds one line of a hunk: its numbers on each side, then the line itself,
/// coloured by `shade` the way the file it came from is.
fn line_row(theme: &Theme, line: &Line, shade: Option<&[Option<Highlight>]>) -> Div<Message> {
    let wash = match line.kind {
        LineKind::Added => Some(theme.colors.success.alpha(theme.emphasis.change)),
        LineKind::Removed => Some(theme.colors.danger.alpha(theme.emphasis.change)),
        LineKind::Context => None,
    };
    let mark = match line.kind {
        LineKind::Added => "+",
        LineKind::Removed => "−",
        LineKind::Context => " ",
    };

    h_flex()
        .w_full()
        .px(1.5)
        .gap(0.5)
        .items_center()
        .overflow_hidden()
        .when_some(wash, Div::bg)
        .child(
            h_flex()
                .w_px(NUMBERS)
                .gap(0.5)
                .justify_end()
                .child(number(theme, line.old))
                .child(number(theme, line.new)),
        )
        .child(
            text(mark)
                .text_sm()
                .font_mono()
                .color(theme.colors.text_subtle),
        )
        .child(shaded(theme, &line.text, shade.unwrap_or_default()))
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

    h_flex().children(runs.into_iter().map(|(highlight, run)| {
        let color = highlight.map_or(theme.colors.text, |highlight| tint(highlight, theme));
        text(run).text_sm().font_mono().color(color)
    }))
}

/// Builds one of a line's numbers, or the blank where it has none.
fn number(theme: &Theme, line: Option<usize>) -> Div<Message> {
    h_flex()
        .w_px(NUMBERS / 2.0 - 4.0)
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
