//! The sidebar that lists what a project has changed.
//!
//! The list is the same shape every editor with git in it has settled on: a
//! message to commit with at the top, then what is staged, then what is not,
//! each file a row with the letter git marks it with. What a row can do is
//! drawn on the row rather than waiting for the pointer, because a control
//! that only exists while it is hovered is a control nobody finds.

use pm_core::{Changed, FileStatus, Head};
use pm_gfx::Rgba;
use pm_ui::{
    Div, IconName, IconSize, MenuItem, Styled, Theme, ToggleState, checkbox, h_flex, icon,
    icon_button, menu_entry, menu_separator, text, v_flex,
};

use crate::message::Message;
use crate::review::commit_editor;
use crate::review::store::{Group, Review};

/// How many lines of the commit message the sidebar has room for.
const MESSAGE_LINES: f32 = 3.0;

/// Builds the sidebar: the commit message, then the changes under it.
///
/// A window with no project open still draws the sidebar, because the
/// sidebar is a region of the window rather than a property of a project:
/// what it says instead is that there is nothing to review.
pub fn changes_sidebar(
    theme: &Theme,
    review: Option<&Review>,
    typing: bool,
    width: f32,
) -> Div<Message> {
    let Some(review) = review else {
        return empty(theme, width);
    };
    let (title, stopped) = review.committable();

    v_flex()
        .w_px(width)
        .flex_1()
        .overflow_hidden()
        .child(heading(theme, review))
        .child(branch_row(theme, review.head()))
        .child(message_field(theme, review, typing))
        .child(commit_button(theme, &title, stopped))
        .when_some(review.trouble(), |sidebar, said| {
            sidebar.child(trouble(theme, said))
        })
        .when(review.changed().is_empty(), |sidebar| {
            sidebar.child(
                text("No changes in this worktree")
                    .text_sm()
                    .font_light()
                    .color(theme.colors.text_subtle)
                    .px(1.5)
                    .py(1.5),
            )
        })
        .children(Group::ALL.into_iter().flat_map(|listed| {
            let rows = review.grouped(listed);
            let mut built = Vec::new();
            if rows.is_empty() {
                return built;
            }
            built.push(group(theme, review, listed));
            built.extend(
                rows.into_iter().filter_map(|index| {
                    Some(change_row(theme, review, index, review.change(index)?))
                }),
            );
            built
        }))
}

/// Builds what the sidebar says when the window has no project open.
fn empty(theme: &Theme, width: f32) -> Div<Message> {
    v_flex().w_px(width).flex_1().overflow_hidden().child(
        text("No project open")
            .text_sm()
            .font_light()
            .color(theme.colors.text_subtle)
            .px(3)
            .py(2),
    )
}

/// Builds the line naming the sidebar, and what can be done to all of it.
fn heading(theme: &Theme, review: &Review) -> Div<Message> {
    let reviewable = !review.changed().is_empty();

    h_flex()
        .w_full()
        .px(1.5)
        .pt(2)
        .pb(1)
        .items_center()
        .justify_between()
        .child(
            text("SOURCE CONTROL")
                .text_xs()
                .font_light()
                .color(theme.colors.text_subtle),
        )
        .child(
            h_flex()
                .gap(0.5)
                .items_center()
                .when(reviewable, |actions| {
                    actions.child(icon_button(
                        theme,
                        IconName::GitCompare,
                        Message::OpenReview,
                    ))
                })
                .child(icon_button(
                    theme,
                    IconName::Refresh,
                    Message::RefreshChanges,
                )),
        )
}

/// Builds the line saying which branch the worktree is on, and how it stands.
fn branch_row(theme: &Theme, head: &Head) -> Div<Message> {
    let name = head.name();
    let drift = match (head.ahead, head.behind) {
        (0, 0) => String::new(),
        (ahead, 0) => format!("↑{ahead}"),
        (0, behind) => format!("↓{behind}"),
        (ahead, behind) => format!("↑{ahead} ↓{behind}"),
    };

    h_flex()
        .w_full()
        .h_px(theme.size.row)
        .px(1.5)
        .gap(0.75)
        .items_center()
        .overflow_hidden()
        .child(
            icon(IconName::GitBranch)
                .size(IconSize::XSmall)
                .color(theme.colors.text_subtle),
        )
        .child(text(name).text_sm().font_light())
        .child(h_flex().flex_1())
        .when(!drift.is_empty(), |row| {
            row.child(
                text(drift)
                    .text_xs()
                    .font_light()
                    .color(theme.colors.text_muted),
            )
        })
}

/// Builds the box the commit message is written in.
///
/// It is the editor, not a line: several lines, a cursor that moves, text
/// that selects — the same buffer the panes draw, in a box of its own.
fn message_field(theme: &Theme, review: &Review, typing: bool) -> Div<Message> {
    v_flex().w_full().px(1.5).py(1).child(
        v_flex()
            .w_full()
            .h_px(theme.size.control * MESSAGE_LINES)
            .px(0.5)
            .py(0.5)
            .overflow_hidden()
            .rounded(theme.radius.md)
            .bg(theme.colors.background)
            .border_1(match typing {
                true => theme.colors.border_focused,
                false => theme.colors.border,
            })
            .child(commit_editor(review, typing)),
    )
}

/// Builds the control that commits, and says what it would commit.
///
/// What stops it is written where the words would be, so a reader who cannot
/// commit is told why rather than left pressing a control that does nothing.
fn commit_button(theme: &Theme, title: &str, stopped: Option<&str>) -> Div<Message> {
    let committable = stopped.is_none();
    let color = match committable {
        true => theme.colors.text,
        false => theme.colors.text_subtle,
    };
    let label = stopped.unwrap_or(title).to_owned();

    v_flex().w_full().px(1.5).pb(1).child(
        h_flex()
            .w_full()
            .h_px(theme.size.control)
            .gap(0.75)
            .items_center()
            .justify_center()
            .rounded(theme.radius.md)
            .bg(theme.colors.surface_selected)
            .when(committable, |control| {
                control
                    .hover_bg(theme.colors.surface_hover)
                    .active_bg(theme.colors.surface_active)
                    .on_click(Message::Commit)
            })
            .child(
                icon(IconName::GitCommit)
                    .size(IconSize::XSmall)
                    .color(color),
            )
            .child(text(label).text_sm().font_medium().color(color)),
    )
}

/// Builds the line saying what git refused to do, until it is asked again.
fn trouble(theme: &Theme, said: &str) -> Div<Message> {
    v_flex().w_full().px(1.5).pb(1).child(
        text(said.to_owned())
            .text_xs()
            .font_light()
            .color(theme.colors.danger),
    )
}

/// Builds the heading above one group of changes, and the box that stages it.
///
/// The box says how much of the group is in the index — none of it, all of
/// it, or some — and clicking it puts the rest in or takes the lot back out.
fn group(theme: &Theme, review: &Review, listed: Group) -> Div<Message> {
    let (staged, count) = review.staged_in(listed);
    let name = listed.label();
    let all = Some(ToggleState::of(staged, count));

    h_flex()
        .w_full()
        .px(1.5)
        .pt(1.5)
        .pb(0.5)
        .items_center()
        .justify_between()
        .child(
            text(format!("{name} ({count})"))
                .text_xs()
                .font_light()
                .color(theme.colors.text_subtle),
        )
        .when_some(all, |heading, state| {
            heading.child(checkbox(theme, state, Message::ToggleGroupStaged(listed)))
        })
}

/// Builds one row: the file's name, where it lives, and what it is marked.
///
/// A click selects the row and opens its diff; the secondary modifier marks
/// it alongside whatever else is marked, and shift marks everything between.
/// The lit row is the one the keyboard is on, and the marked ones are lit
/// more quietly beside it.
fn change_row(theme: &Theme, review: &Review, index: usize, changed: &Changed) -> Div<Message> {
    let mark = changed.mark();
    let id = review.id_of(index);
    let selected = id.is_some_and(|id| review.selected() == Some(id));
    let marked = id.is_some_and(|id| review.is_marked(id));

    h_flex()
        .w_full()
        .h_px(theme.size.row)
        .px(1.5)
        .gap(0.5)
        .items_center()
        .overflow_hidden()
        .when(marked, |row| row.bg(theme.colors.surface_selected))
        .when(selected, |row| {
            row.bg(theme.colors.surface_active)
                .border_1(theme.colors.border_selected)
        })
        .hover_bg(theme.colors.surface_hover)
        .on_click(Message::SelectChange(index))
        .on_secondary_click(Message::ShowChangeMenu(index))
        .child(
            icon(IconName::File)
                .size(IconSize::XSmall)
                .color(theme.colors.text_subtle),
        )
        .child(text(changed.name()).text_sm().font_light())
        .child(
            text(directory(review, changed))
                .text_xs()
                .font_light()
                .font_mono()
                .color(theme.colors.text_subtle),
        )
        .child(h_flex().flex_1())
        .child(
            text(mark.letter())
                .text_xs()
                .font_mono()
                .color(status_color(theme, mark)),
        )
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

/// What the box on a file's row says: how much of it is in the index.
pub fn staged_state(changed: &Changed) -> ToggleState {
    match (changed.is_staged(), changed.is_unstaged()) {
        (true, true) => ToggleState::Mixed,
        (true, false) => ToggleState::On,
        _ => ToggleState::Off,
    }
}

/// The things that can be done to what the list is acting on.
///
/// The menu is about the selection rather than about the row it was opened
/// from: a reader who has marked four files and asks the fourth what can be
/// done to it is asking about the four. What each entry says counts them, so
/// there is no way to discard four files while believing you asked about one.
pub fn change_menu(review: &Review) -> Vec<MenuItem<Message>> {
    let acting = review.acting_on();
    let changes = acting
        .iter()
        .filter_map(|id| review.change(review.place_of(*id)?))
        .collect::<Vec<_>>();
    if changes.is_empty() {
        return Vec::new();
    }

    let count = changes.len();
    let staged = changes.iter().all(|changed| changed.is_staged());
    let created = changes.iter().all(|changed| changed.is_created());
    let stage = match (count, staged) {
        (1, true) => "Unstage File".to_owned(),
        (1, false) => "Stage File".to_owned(),
        (count, true) => format!("Unstage {count} Files"),
        (count, false) => format!("Stage {count} Files"),
    };
    let discard = match (count, created) {
        (1, true) => "Delete File".to_owned(),
        (1, false) => "Discard Changes".to_owned(),
        (count, true) => format!("Delete {count} Files"),
        (count, false) => format!("Discard Changes to {count} Files"),
    };
    let alone = (count == 1)
        .then(|| review.place_of(*acting.first()?))
        .flatten();

    vec![
        menu_entry("Open Changes", alone.map(Message::OpenChange)),
        menu_entry("Open File Diff", alone.map(Message::OpenChangeDiff)),
        menu_entry("Open File", alone.map(Message::OpenChangeFile)),
        menu_separator(),
        menu_entry(
            stage,
            Some(match staged {
                true => Message::UnstageSelection,
                false => Message::StageSelection,
            }),
        ),
        menu_entry(discard, Some(Message::DiscardSelection)),
        menu_separator(),
        menu_entry("Stage All Changes", Some(Message::StageAll)),
        menu_entry("Unstage All Changes", Some(Message::UnstageAll)),
        menu_separator(),
        menu_entry("Copy Path", alone.map(Message::CopyChangePath)),
        menu_entry(
            "Copy Relative Path",
            alone.map(Message::CopyChangeRelativePath),
        ),
        menu_separator(),
        menu_entry("Reveal in File Manager", alone.map(Message::RevealChange)),
    ]
}

/// Where the file sits in the worktree, for the row that names it.
fn directory(review: &Review, changed: &Changed) -> String {
    let relative = changed
        .path
        .strip_prefix(review.root())
        .unwrap_or(&changed.path);
    relative
        .parent()
        .map(|directory| directory.display().to_string())
        .unwrap_or_default()
}

/// The colour a status is written in.
pub fn status_color(theme: &Theme, status: FileStatus) -> Rgba {
    match status {
        FileStatus::Modified | FileStatus::Renamed => theme.colors.warning,
        FileStatus::Added | FileStatus::Untracked => theme.colors.success,
        FileStatus::Deleted => theme.colors.text_subtle,
        FileStatus::Conflicted => theme.colors.danger,
    }
}
