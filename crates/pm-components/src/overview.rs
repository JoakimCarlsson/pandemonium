//! A project's overview: its sessions grouped by where they stand, and how its own repository is.

use pm_icon::IconName;
use pm_input::FocusId;
use pm_style::{
    Role, bg, border_b, border_color, cursor_pointer, flex, flex_1, flex_col, flex_none,
    font_medium, font_mono, gap, h, items_center, max_w_px, min_h, min_w_px, mx_auto,
    overflow_hidden, pb, pt, px, text_px, w_full, w_px,
};
use pm_ui::{Node, node};

use crate::extensions::group;
use crate::parts::{clipped, glyph, line, spacer};
use crate::sample::{Session, Status};
use crate::sidebar::status_icon;
use crate::size::Size;
use crate::state::{NEW_IN_BASE, SESSION_BASE, WorkspaceState};
use crate::{Avatar, Button};

/// The scrolling body of the overview.
pub const OVERVIEW_SCROLL: FocusId = FocusId(9_200_000);

/// How wide the overview's column is at most, in logical pixels.
const PAGE_WIDTH: f32 = 860.0;

/// How wide the column naming a session's branch is, in logical pixels.
const BRANCH_WIDTH: f32 = 140.0;

/// How wide the column saying how long ago a session changed is, in logical pixels.
const AGE_WIDTH: f32 = 72.0;

/// How many of the repository's latest commits the overview lists.
const COMMITS_SHOWN: usize = 8;

/// How many characters of a commit's name the overview shows.
const SHORT_ID: usize = 7;

/// The groups the sessions fall in, in the order the overview shows them, each with its heading.
const GROUPS: [(Status, &str); 5] = [
    (Status::NeedsInput, "Needs input"),
    (Status::Error, "Error"),
    (Status::Unread, "Unread"),
    (Status::Working, "Working"),
    (Status::Idle, "Idle"),
];

impl WorkspaceState {
    /// The state after the overview is scrolled by `delta` logical pixels, downwards when positive, with `room` pixels to scroll.
    pub fn scrolled_overview(mut self, delta: f32, room: f32) -> Self {
        self.overview_scroll = (self.overview_scroll + delta).clamp(0.0, room);
        self
    }
}

/// The overview of the project at `project`.
pub fn overview(state: &WorkspaceState, project: usize) -> Node {
    let mut column = node().styles([
        flex(),
        flex_col(),
        gap(6),
        w_full(),
        max_w_px(PAGE_WIDTH),
        mx_auto(),
        px(5),
        pt(5),
        pb(8),
        flex_none(),
    ]);
    column = column.children(sessions(state, project));
    column = column.children(repository(state));
    column = column.children(commits(state));
    node()
        .styles([flex(), flex_col(), flex_1(), min_w_px(0), min_h(0)])
        .child(heading(state, project))
        .child(
            node()
                .styles([flex(), flex_col(), flex_1(), min_h(0), overflow_hidden()])
                .scroll_from_start(OVERVIEW_SCROLL, state.overview_scroll)
                .child(column),
        )
}

/// The bar across the top: the project, where it lives, and the button that starts a session in it.
fn heading(state: &WorkspaceState, project: usize) -> Node {
    let mut bar = node().styles([
        flex(),
        items_center(),
        gap(3),
        h(12),
        px(5),
        flex_none(),
        border_b(),
        border_color(Role::Border),
    ]);
    if let Some(held) = state.projects.get(project) {
        bar = bar
            .child(Avatar::of(held).build())
            .child(line(&held.name, 14, 1.4, Role::Text).styles([
                font_medium(),
                text_px(14),
                flex_none(),
            ]))
            .child(clipped(&held.path, 12, Role::Subtle));
    }
    bar.child(spacer()).child(
        Button::new("New session", FocusId(NEW_IN_BASE + project as u64))
            .with_icon(IconName::Plus)
            .size(Size::Sm)
            .build(),
    )
}

/// The project's open sessions, a group for each way they stand, or a line saying there are none.
fn sessions(state: &WorkspaceState, project: usize) -> Vec<Node> {
    let mut members: Vec<usize> = (0..state.sessions.len())
        .filter(|&index| {
            let session = &state.sessions[index];
            session.project == project && !session.closed && !session.home
        })
        .collect();
    members.sort_by_key(|&index| std::cmp::Reverse(state.sessions[index].updated_at));
    if members.is_empty() {
        return vec![line("No sessions yet.", 13, 1.4, Role::Subtle)];
    }
    GROUPS
        .into_iter()
        .filter_map(|(status, title)| {
            group(
                title,
                members
                    .iter()
                    .filter(|&&index| state.sessions[index].shown() == status)
                    .map(|&index| session_row(state, index))
                    .collect(),
            )
        })
        .collect()
}

/// The row of the session at `index`: where it stands, its title, its agent's mark, what it changed, its branch and when it last changed.
fn session_row(state: &WorkspaceState, index: usize) -> Node {
    let session: &Session = &state.sessions[index];
    let agent = state.agent_look(session.agent);
    let mut row = row()
        .styles([cursor_pointer(), bg(Role::SurfaceHover).hover()])
        .child(status_icon(session.shown()))
        .child(clipped(&session.title, 14, Role::Text))
        .child(
            node()
                .styles([flex(), flex_none()])
                .child(glyph(agent.icon, 3.5, Role::Muted))
                .tooltip(agent.name),
        );
    if !session.changes.is_empty() {
        row = row.child(tally(session));
    }
    row.child(
        node()
            .styles([flex(), flex_none(), w_px(BRANCH_WIDTH), overflow_hidden()])
            .child(clipped(&session.branch, 12, Role::Muted).styles([font_mono()])),
    )
    .child(
        node()
            .styles([
                flex(),
                flex_none(),
                w_px(AGE_WIDTH),
                pm_style::justify_end(),
            ])
            .child(line(session.age(), 13, 1.4, Role::Subtle)),
    )
    .focusable(FocusId(SESSION_BASE + index as u64))
}

/// The lines `session` adds and takes away, in their colours.
fn tally(session: &Session) -> Node {
    node()
        .styles([flex(), items_center(), gap(1.5), flex_none()])
        .child(line(format!("+{}", session.added()), 12, 1.4, Role::Success).styles([font_mono()]))
        .child(line(format!("−{}", session.removed()), 12, 1.4, Role::Danger).styles([font_mono()]))
}

/// The project's own folder: the branch it is on and how far it is from what it follows, and what is not committed.
fn repository(state: &WorkspaceState) -> Option<Node> {
    let home = state.review_session()?;
    if home.branch.is_empty() {
        return group(
            "Repository",
            vec![
                row()
                    .child(glyph(IconName::Folder, 4.0, Role::Muted))
                    .child(line("Not a Git repository", 13, 1.4, Role::Muted)),
            ],
        );
    }
    let tracking = &home.tracking;
    let standing = match &tracking.upstream {
        None => "No upstream".to_owned(),
        Some(upstream) if tracking.ahead == 0 && tracking.behind == 0 => {
            format!("Up to date with {upstream}")
        }
        Some(upstream) => format!(
            "{} ahead, {} behind {upstream}",
            tracking.ahead, tracking.behind
        ),
    };
    let branch = row()
        .child(glyph(IconName::GitFork, 4.0, Role::Muted))
        .child(clipped(&home.branch, 13, Role::Text).styles([font_mono()]))
        .child(line(standing, 13, 1.4, Role::Muted).styles([flex_none()]));
    let mut changes = row().child(glyph(IconName::GitCompare, 4.0, Role::Muted));
    changes = match home.changes.len() {
        0 => changes.child(clipped("No uncommitted changes", 13, Role::Muted)),
        count => changes
            .child(clipped(
                format!(
                    "{count} uncommitted {}",
                    if count == 1 { "file" } else { "files" }
                ),
                13,
                Role::Text,
            ))
            .child(tally(home)),
    };
    group("Repository", vec![branch, changes])
}

/// The latest commits of the project's own repository, once the server has listed them.
fn commits(state: &WorkspaceState) -> Option<Node> {
    state
        .review_session()
        .filter(|home| !home.branch.is_empty())?;
    group(
        "Recent commits",
        state
            .git
            .history
            .iter()
            .take(COMMITS_SHOWN)
            .map(|commit| {
                row()
                    .child(glyph(IconName::GitCommit, 4.0, Role::Subtle))
                    .child(
                        line(
                            commit.id.chars().take(SHORT_ID).collect::<String>(),
                            12,
                            1.4,
                            Role::Subtle,
                        )
                        .styles([font_mono(), flex_none()]),
                    )
                    .child(clipped(&commit.summary, 13, Role::Text))
            })
            .collect(),
    )
}

/// A row of one of the overview's cards, its contents in a line.
fn row() -> Node {
    node().styles([flex(), items_center(), gap(3), h(11), px(4)])
}
