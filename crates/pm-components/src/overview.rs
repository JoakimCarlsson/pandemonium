//! A project's overview: its sessions grouped by where they stand, and how its own repository is.

use rustle::{CursorIcon, Modifier};

use crate::extensions::group;
use crate::kit::{
    Flex, FocusId, Hovered, IconName, Interactive, Looks, Radius, Role, Scroll, Side, Styled,
    Tooltip, Type, column, label, label_with, row, scroll_area,
};
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
const GROUPS: [(Status, &str); 6] = [
    (Status::NeedsInput, "Needs input"),
    (Status::Unreachable, "Server out of reach"),
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

/// Emits the overview of the project at `project`.
#[track_caller]
pub fn overview(state: &WorkspaceState, project: usize) {
    column(Modifier::new().grow(), Flex::new(), || {
        heading(state, project);
        scroll_area(
            Modifier::new().grow(),
            Scroll::from_start(OVERVIEW_SCROLL, state.overview_scroll),
            || {
                column(
                    Modifier::new().fill_max_width(),
                    Flex::new().center(),
                    || {
                        column(
                            Modifier::new()
                                .fill_max_width()
                                .max_w_px(PAGE_WIDTH)
                                .pad_x(5)
                                .pad_side(Side::Top, 5)
                                .pad_side(Side::Bottom, 8),
                            Flex::new().gap(6),
                            || {
                                sessions(state, project);
                                repository(state);
                                commits(state);
                            },
                        );
                    },
                );
            },
        );
    });
}

/// Emits the bar across the top: the project, where it lives, and the button that starts a session in it.
#[track_caller]
fn heading(state: &WorkspaceState, project: usize) {
    row(
        Modifier::new()
            .h(12)
            .stroke_side(Side::Bottom, 1.0, Role::Border)
            .pad_x(5),
        Flex::new().center().gap(3),
        || {
            if let Some(held) = state.projects.get(project) {
                Avatar::of(held).build();
                label(
                    &held.name,
                    Type::new(14)
                        .ratio(1.4)
                        .tone(Role::Text)
                        .truncate()
                        .medium(),
                );
                clipped(&held.path, 12, Role::Subtle);
            }
            spacer();
            Button::new("New session", FocusId(NEW_IN_BASE + project as u64))
                .with_icon(IconName::Plus)
                .size(Size::Sm)
                .build();
        },
    );
}

/// Emits the project's open sessions, a group for each way they stand, or a line saying there are none.
#[track_caller]
fn sessions(state: &WorkspaceState, project: usize) {
    let mut members: Vec<usize> = (0..state.sessions.len())
        .filter(|&index| {
            let session = &state.sessions[index];
            session.project == Some(project) && !session.closed && !session.home
        })
        .collect();
    members.sort_by_key(|&index| std::cmp::Reverse(state.sessions[index].updated_at));
    if members.is_empty() {
        line("No sessions yet.", 13, 1.4, Role::Subtle);
        return;
    }
    for (status, title) in GROUPS {
        group(title, || {
            for &index in members
                .iter()
                .filter(|&&index| state.sessions[index].shown() == status)
            {
                session_row(state, index);
            }
        });
    }
}

/// Emits the row of the session at `index`: where it stands, its title, its agent's mark, what it changed, its branch and when it last changed.
#[track_caller]
fn session_row(state: &WorkspaceState, index: usize) {
    let session: &Session = &state.sessions[index];
    let agent = state.agent_look(session.agent);
    let id = FocusId(SESSION_BASE + index as u64);
    let own = Modifier::new()
        .looks_of(Looks::new().hover(Role::SurfaceHover), id)
        .cursor(CursorIcon::Pointer)
        .target(id);
    card_row(own, || {
        status_icon(session.shown());
        clipped(&session.title, 14, Role::Text);
        row(Modifier::new().tooltip(agent.name), Flex::new(), || {
            glyph(agent.icon, 3.5, Role::Muted);
        });
        if !session.changes.is_empty() {
            tally(session);
        }
        row(
            Modifier::new().width(BRANCH_WIDTH).clip_to(Radius::None),
            Flex::new(),
            || {
                label_with(
                    Modifier::new().grow(),
                    &session.branch,
                    Type::new(12).ratio(1.4).tone(Role::Muted).truncate(),
                );
            },
        );
        row(
            Modifier::new().width(AGE_WIDTH),
            Flex::new().trailing(),
            || line(session.age(), 13, 1.4, Role::Subtle),
        );
    });
}

/// Emits the lines `session` adds and takes away, in their colours.
#[track_caller]
fn tally(session: &Session) {
    row(Modifier::new(), Flex::new().center().gap(1.5), || {
        label(
            format!("+{}", session.added()),
            Type::new(12)
                .ratio(1.4)
                .tone(Role::Success)
                .truncate()
                .mono(),
        );
        label(
            format!("−{}", session.removed()),
            Type::new(12)
                .ratio(1.4)
                .tone(Role::Danger)
                .truncate()
                .mono(),
        );
    });
}

/// Emits the project's own folder: the branch it is on and how far it is from what it follows, and what is not committed.
#[track_caller]
fn repository(state: &WorkspaceState) {
    let Some(home) = state.review_session() else {
        return;
    };
    if home.branch.is_empty() {
        group("Repository", || {
            card_row(Modifier::new(), || {
                glyph(IconName::Folder, 4.0, Role::Muted);
                line(
                    if home.unknown {
                        "Reading the folder…"
                    } else {
                        "Not a Git repository"
                    },
                    13,
                    1.4,
                    Role::Muted,
                );
            });
        });
        return;
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
    group("Repository", || {
        card_row(Modifier::new(), || {
            glyph(IconName::GitFork, 4.0, Role::Muted);
            label_with(
                Modifier::new().grow(),
                &home.branch,
                Type::new(13).ratio(1.4).tone(Role::Text).truncate(),
            );
            line(standing, 13, 1.4, Role::Muted);
        });
        card_row(Modifier::new(), || {
            glyph(IconName::GitCompare, 4.0, Role::Muted);
            match home.changes.len() {
                0 => clipped("No uncommitted changes", 13, Role::Muted),
                count => {
                    clipped(
                        format!(
                            "{count} uncommitted {}",
                            if count == 1 { "file" } else { "files" }
                        ),
                        13,
                        Role::Text,
                    );
                    tally(home);
                }
            }
        });
    });
}

/// Emits the latest commits of the project's own repository, once the server has listed them.
#[track_caller]
fn commits(state: &WorkspaceState) {
    if state
        .review_session()
        .filter(|home| !home.branch.is_empty())
        .is_none()
    {
        return;
    }
    group("Recent commits", || {
        for commit in state.git.history.iter().take(COMMITS_SHOWN) {
            card_row(Modifier::new(), || {
                glyph(IconName::GitCommit, 4.0, Role::Subtle);
                label(
                    commit.id.chars().take(SHORT_ID).collect::<String>(),
                    Type::new(12)
                        .ratio(1.4)
                        .tone(Role::Subtle)
                        .truncate()
                        .mono(),
                );
                clipped(&commit.summary, 13, Role::Text);
            });
        }
    });
}

/// Emits a row of one of the overview's cards, its contents in a line, with `modifier` around its
/// box before its padding, holding what `content` emits.
#[track_caller]
fn card_row(modifier: Modifier, content: impl FnOnce()) {
    row(
        Modifier::new().h(11).chain(modifier).pad_x(4),
        Flex::new().center().gap(3),
        content,
    );
}
