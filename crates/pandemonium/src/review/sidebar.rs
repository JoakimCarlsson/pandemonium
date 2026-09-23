//! The sidebar that lists what a project has changed and its recent history.
//!
//! The list is the same shape every editor with git in it has settled on: a
//! message to commit with at the top, then what is staged, then what is not,
//! each file a row with the letter git marks it with. What a row can do is
//! drawn on the row rather than waiting for the pointer, because a control
//! that only exists while it is hovered is a control nobody finds.

use pm_core::{Changed, FileStatus};
use pm_gfx::Rgba;
use pm_ui::{
    Axis, Bounds, Div, IconName, IconSize, MenuItem, Styled, Theme, ToggleState, checkbox, h_flex,
    icon, icon_button, measured, menu_entry, menu_separator, sash, text, v_flex,
};

use crate::message::Message;
use crate::review::action::{primary_face, primary_message};
use crate::review::commit_editor;
use crate::review::graph::{graph_cell, lane_color};
use crate::review::store::{Group, Primary, Review};

/// Window controls and saved layout for the Source Control sidebar.
#[derive(Clone)]
pub struct SourceControlControls {
    /// Where the commit split button was drawn in the last frame.
    pub commit_bounds: Bounds,
    /// Where the history reference filter was drawn in the last frame.
    pub history_refs_bounds: Bounds,
    /// Bounds of the Graph panel from the last frame.
    pub history_graph_bounds: Bounds,
    /// Whether the graph includes every reference.
    pub history_all: bool,
    /// Height of the graph panel.
    pub history_graph_height: f32,
    /// Whether the graph panel is open.
    pub history_graph_open: bool,
    /// Whether the changes section is expanded.
    pub changes_section_open: bool,
}

/// Builds the sidebar: commit controls, changes, and recent history.
///
/// A window with no project open still draws the sidebar, because the
/// sidebar is a region of the window rather than a property of a project:
/// what it says instead is that there is nothing to review.
pub fn changes_sidebar(
    theme: &Theme,
    review: Option<&Review>,
    typing: bool,
    width: f32,
    controls: SourceControlControls,
) -> Div<Message> {
    let Some(review) = review else {
        return empty(theme, width);
    };
    let primary = review.primary();

    let groups = Group::ALL.into_iter().flat_map(|listed| {
        let rows = review.grouped(listed);
        let mut built = Vec::new();
        if rows.is_empty() {
            return built;
        }
        built.push(group(theme, review, listed));
        built.extend(
            rows.into_iter()
                .filter_map(|index| Some(change_row(theme, review, index, review.change(index)?))),
        );
        built
    });

    v_flex()
        .w_px(width)
        .flex_1()
        .overflow_hidden()
        .child(heading(theme, review))
        .child(section_heading(
            theme,
            review,
            controls.changes_section_open,
        ))
        .when(controls.changes_section_open, |sidebar| {
            sidebar
                .child(message_field(theme, review, typing))
                .child(measured(
                    controls.commit_bounds.clone(),
                    commit_button(theme, &primary),
                ))
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
                .child(v_flex().flex_1().overflow_hidden().children(groups))
        })
        .when(!controls.changes_section_open, |sidebar| {
            sidebar.child(v_flex().flex_1())
        })
        .when(controls.history_graph_open, |sidebar| {
            sidebar.child(sash(Axis::Vertical, Message::ResizeHistoryGraph))
        })
        .child(measured(
            controls.history_graph_bounds.clone(),
            history_graph(theme, review, controls),
        ))
}

/// Builds the compact commit graph pinned below the change list.
fn history_graph(theme: &Theme, review: &Review, controls: SourceControlControls) -> Div<Message> {
    let row_height = theme.size.row;
    let visible = ((controls.history_graph_height - row_height) / row_height)
        .floor()
        .max(1.0) as usize;
    let commits = review
        .history(controls.history_all)
        .iter()
        .skip(review.history_scroll(controls.history_all, visible))
        .take(visible)
        .cloned()
        .collect::<Vec<_>>();
    let columns = commits
        .iter()
        .map(|commit| commit.lanes.width)
        .max()
        .unwrap_or(1);
    let open = controls.history_graph_open;
    v_flex()
        .w_full()
        .h_px(if open {
            controls.history_graph_height
        } else {
            theme.size.row
        })
        .overflow_hidden()
        .border_1(theme.colors.border)
        .child(
            h_flex()
                .w_full()
                .h_px(theme.size.row)
                .px(1.5)
                .items_center()
                .child(
                    icon_button(
                        theme,
                        match open {
                            true => IconName::ChevronDown,
                            false => IconName::ChevronRight,
                        },
                        Message::ToggleHistoryGraph,
                    )
                    .tooltip(if open {
                        "Collapse Graph"
                    } else {
                        "Expand Graph"
                    }),
                )
                .child(text("Graph".to_owned()).text_sm().font_light())
                .child(h_flex().flex_1())
                .child(measured(
                    controls.history_refs_bounds.clone(),
                    history_ref_picker(theme, controls.history_all)
                        .tooltip("Choose history references"),
                ))
                .child(
                    icon_button(theme, IconName::Target, Message::RevealCurrentHistoryItem)
                        .tooltip("Go to current history item"),
                )
                .child(
                    icon_button(theme, IconName::GitFetch, Message::Fetch)
                        .tooltip("Fetch from all remotes"),
                )
                .child(icon_button(theme, IconName::GitPull, Message::Pull).tooltip("Pull"))
                .child(icon_button(theme, IconName::GitPush, Message::PushBranch).tooltip("Push"))
                .child(
                    icon_button(theme, IconName::Refresh, Message::RefreshChanges)
                        .tooltip("Refresh"),
                ),
        )
        .when(open, |graph| {
            graph.children(
                commits
                    .into_iter()
                    .map(|commit| history_row(theme, commit, columns)),
            )
        })
}

/// How many branch and tag pills a commit row draws before counting the rest.
const SHOWN_REFS: usize = 2;

/// Builds one commit of the graph: its lanes, its references and its summary.
fn history_row(theme: &Theme, commit: pm_core::Commit, columns: usize) -> Div<Message> {
    let color = lane_color(theme, commit.lanes.color);
    h_flex()
        .w_full()
        .h_px(theme.size.row)
        .px(1.5)
        .gap(0.75)
        .items_center()
        .overflow_hidden()
        .child(graph_cell(theme, commit.lanes, columns))
        .child(
            text(commit.id)
                .text_xs()
                .font_mono()
                .color(theme.colors.text_muted),
        )
        .children(
            commit
                .refs
                .iter()
                .take(SHOWN_REFS)
                .map(|name| ref_badge(theme, name, color)),
        )
        .when(commit.refs.len() > SHOWN_REFS, |row| {
            row.child(
                text(format!("+{}", commit.refs.len() - SHOWN_REFS))
                    .text_xs()
                    .color(theme.colors.text_subtle),
            )
        })
        .child(
            text(commit.summary)
                .text_xs()
                .font_light()
                .color(theme.colors.text),
        )
}

/// Builds the pill naming a branch or tag that points at a commit.
///
/// Git decorates the checked out branch as `HEAD -> name` and a tag as
/// `tag: name`; the pill says the name, and the branch HEAD is on is the
/// one drawn solid.
fn ref_badge(theme: &Theme, decorated: &str, color: Rgba) -> Div<Message> {
    let (name, current) = match decorated.strip_prefix("HEAD -> ") {
        Some(branch) => (branch, true),
        None => (decorated.strip_prefix("tag: ").unwrap_or(decorated), false),
    };
    h_flex()
        .h_px(theme.size.row - 8.0)
        .px(1)
        .items_center()
        .rounded(theme.radius.sm)
        .bg(if current { color } else { color.alpha(0.18) })
        .child(text(name.to_owned()).text_xs().color(if current {
            theme.colors.background
        } else {
            color
        }))
}

/// Builds the current history-reference filter beside the Graph actions.
fn history_ref_picker(theme: &Theme, all: bool) -> Div<Message> {
    h_flex()
        .h_px(theme.size.icon_control)
        .px(0.5)
        .gap(0.5)
        .items_center()
        .rounded(theme.radius.sm)
        .hover_bg(theme.colors.surface_hover)
        .on_click(Message::ShowHistoryRefsMenu)
        .child(
            icon(IconName::GitBranch)
                .size(IconSize::XSmall)
                .color(theme.colors.text_subtle),
        )
        .child(text(if all { "All" } else { "Auto" }).text_xs())
        .child(
            icon(IconName::ChevronDown)
                .size(IconSize::XSmall)
                .color(theme.colors.text_subtle),
        )
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
            text("Source Control".to_owned())
                .text_sm()
                .font_light()
                .color(theme.colors.text),
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
                }),
        )
}

/// Builds the label for the change list below the commit controls.
fn section_heading(theme: &Theme, review: &Review, open: bool) -> Div<Message> {
    let (_, stopped) = review.committable();
    h_flex()
        .w_full()
        .h_px(theme.size.row)
        .px(1.5)
        .gap(0.5)
        .items_center()
        .on_click(Message::ToggleChangesSection)
        .child(
            icon(match open {
                true => IconName::ChevronDown,
                false => IconName::ChevronRight,
            })
            .size(IconSize::XSmall)
            .color(theme.colors.text_subtle),
        )
        .child(text("Changes".to_owned()).text_sm().font_light())
        .child(h_flex().flex_1())
        .child(toolbar_action(
            theme,
            IconName::GitCommit,
            stopped.is_none(),
            Message::Commit,
        ))
        .child(icon_button(
            theme,
            IconName::Refresh,
            Message::RefreshChanges,
        ))
        .child(icon_button(
            theme,
            IconName::More,
            Message::ShowSourceControlMenu,
        ))
}

/// Builds one compact toolbar action, disabling it when `enabled` is false.
fn toolbar_action(
    theme: &Theme,
    symbol: IconName,
    enabled: bool,
    message: Message,
) -> Div<Message> {
    h_flex()
        .size_px(theme.size.icon_control)
        .items_center()
        .justify_center()
        .rounded(theme.radius.md)
        .when(enabled, |action| {
            action
                .hover_bg(theme.colors.surface_hover)
                .active_bg(theme.colors.surface_active)
                .on_click(message)
        })
        .child(icon(symbol).size(IconSize::Medium).color(match enabled {
            true => theme.colors.text_subtle,
            false => theme.colors.text_subtle.alpha(0.5),
        }))
}
/// Builds the box the commit message is written in.
///
/// It is the editor, not a line: several lines, a cursor that moves about and
/// text that selects — the same buffer the panes draw, in a box of its own.
fn message_field(theme: &Theme, review: &Review, typing: bool) -> Div<Message> {
    v_flex()
        .w_full()
        .px(1.5)
        .py(1)
        .child(commit_editor(theme, review, typing))
}

/// Builds the control that commits, or syncs once there is nothing to commit.
///
/// What stops it is written where the words would be, so a reader who cannot
/// commit is told why rather than left pressing a control that does nothing.
fn commit_button(theme: &Theme, primary: &Primary) -> Div<Message> {
    let pressed = primary_message(primary);
    let enabled = pressed.is_some();
    let color = match enabled {
        true => theme.colors.text,
        false => theme.colors.text_subtle,
    };

    v_flex().w_full().px(1.5).pb(1).child(
        h_flex()
            .w_full()
            .h_px(theme.size.control)
            .items_center()
            .rounded(theme.radius.md)
            .bg(theme.colors.surface_selected)
            .when(enabled, |control| {
                control
                    .hover_bg(theme.colors.surface_hover)
                    .active_bg(theme.colors.surface_active)
            })
            .child(
                h_flex()
                    .h_full()
                    .flex_1()
                    .items_center()
                    .justify_center()
                    .when_some(pressed, |control, message| control.on_click(message))
                    .child(primary_face(theme, primary, IconName::Check)),
            )
            .child(
                h_flex()
                    .h_full()
                    .px(1)
                    .items_center()
                    .border_1(theme.colors.border)
                    .when(enabled, |control| control.on_click(Message::ShowCommitMenu))
                    .child(
                        icon(IconName::ChevronDown)
                            .size(IconSize::XSmall)
                            .color(color),
                    ),
            ),
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
