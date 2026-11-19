//! A project's overview: its sessions grouped by where they stand, and how its own repository is.

use crate::areas::SidebarEvent;
use crate::button::Variant;
use crate::events::{Event, send};
use crate::kit::{self, Frame};
use crate::target::Target;
use crate::theme;
use crate::theme::Indicated;
use rustle::phosphor;
use rustle::{Alignment, Arrangement, column_arranged, row_arranged};
use rustle::{CursorIcon, Modifier};

use crate::Avatar;
use crate::events::Interactive;
use crate::extensions::group_of_fixed_rows as group;
use crate::parts::{clipped, glyph, line, spacer};
use crate::sample::{Session, Status};
use crate::sidebar::status_icon;
use crate::state::WorkspaceState;
use crate::theme::Role;

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

impl WorkspaceState {}

/// Emits the overview of the project at `project`.
#[track_caller]
pub fn overview(state: &WorkspaceState, project: usize) {
    kit::column(Modifier::new().weight(1.0), 0, || {
        heading(state, project);
        column_arranged(
            Modifier::new()
                .fill_max_width()
                .weight(1.0)
                .vertical_scroll(&state.overview_scroll)
                .fill_max_width(),
            Arrangement::Start,
            Alignment::Center,
            || {
                kit::column(
                    Modifier::new()
                        .fill_max_width()
                        .width_in(0.0, PAGE_WIDTH)
                        .padding_xy(theme::sp(5), 0.0)
                        .padding_each(0.0, theme::sp(5), 0.0, 0.0)
                        .padding_each(0.0, 0.0, 0.0, theme::sp(8)),
                    6,
                    || {
                        sessions(state, project);
                        repository(state);
                        commits(state);
                    },
                );
            },
        );
    });
}

/// Emits the bar across the top: the project, where it lives, and the button that starts a session in it.
#[track_caller]
fn heading(state: &WorkspaceState, project: usize) {
    kit::row(
        Modifier::new()
            .fill_max_width()
            .height(theme::sp(12))
            .ruled(theme::Side::Bottom)
            .padding_xy(theme::sp(5), 0.0),
        3,
        || {
            if let Some(held) = state.projects.get(project) {
                Avatar::of(held).build();
                kit::text(Modifier::new(), &held.name, kit::strong(14, Role::Text));
                clipped(&held.path, 12, Role::Subtle);
            }
            spacer();
            kit::small_button(
                "New session",
                Target::Sidebar(SidebarEvent::NewIn(project)),
                Variant::Default,
            )
            .with_icon(phosphor::PLUS)
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
    let id = Target::Sidebar(SidebarEvent::Session(index));
    let own = Modifier::new()
        .indicated(
            theme::InteractionTones::new().hover(Role::SurfaceHover),
            theme::Focused::Held(crate::focus::is_held(id)),
        )
        .cursor(CursorIcon::Pointer)
        .control_menu(
            id,
            {
                let pressed = send(Event::pressing(id));
                move |_| pressed()
            },
            {
                let operations = crate::events::operations();
                move || operations.open_session_menu(index)
            },
        );
    card_row(own, || {
        status_icon(session.shown());
        clipped(&session.title, 14, Role::Text);
        crate::tooltip::with_tooltip(agent.name, || {
            kit::row_top(Modifier::new(), 0, || {
                glyph(agent.icon, 3.5, Role::Muted);
            });
        });
        if !session.changes.is_empty() {
            tally(session);
        }
        kit::row_top(Modifier::new().width(BRANCH_WIDTH).clip(), 0, || {
            kit::text(
                Modifier::new().weight(1.0),
                &session.branch,
                kit::line(12, Role::Muted),
            );
        });
        row_arranged(
            Modifier::new().width(AGE_WIDTH),
            Arrangement::End,
            Alignment::Start,
            || line(session.age(), 13, 1.4, Role::Subtle),
        );
    });
}

/// Emits the lines `session` adds and takes away, in their colours.
#[track_caller]
fn tally(session: &Session) {
    kit::row(Modifier::new(), 1.5, || {
        kit::text(
            Modifier::new(),
            format!("+{}", session.added()),
            kit::code(12, Role::Success),
        );
        kit::text(
            Modifier::new(),
            format!("−{}", session.removed()),
            kit::code(12, Role::Danger),
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
                glyph(phosphor::FOLDER, 4.0, Role::Muted);
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
            glyph(phosphor::GIT_FORK, 4.0, Role::Muted);
            kit::text(
                Modifier::new().weight(1.0),
                &home.branch,
                kit::line(13, Role::Text),
            );
            line(standing, 13, 1.4, Role::Muted);
        });
        card_row(Modifier::new(), || {
            glyph(phosphor::GIT_DIFF, 4.0, Role::Muted);
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
                glyph(phosphor::GIT_COMMIT, 4.0, Role::Subtle);
                kit::text(
                    Modifier::new(),
                    commit.id.chars().take(SHORT_ID).collect::<String>(),
                    kit::code(12, Role::Subtle),
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
    kit::row(
        Modifier::new()
            .fill_max_width()
            .height(theme::sp(11))
            .chain(modifier)
            .padding_xy(theme::sp(4), 0.0),
        3,
        content,
    );
}
