//! A project's overview: its sessions grouped by where they stand, and how its own repository is.

use crate::events::reduce;
use crate::theme;
use crate::theme::sp;
use rustle::{Alignment, Arrangement, column_arranged, row_arranged};
use rustle::{CursorIcon, Modifier};

use crate::events::Interactive;
use crate::extensions::group_of_fixed_rows as group;
use crate::focus::FocusId;
use crate::icons::IconName;
use crate::parts::{clipped, glyph, line, spacer};
use crate::sample::{Session, Status};
use crate::sidebar::status_icon;
use crate::size::Size;
use crate::state::{NEW_IN_BASE, SESSION_BASE, WorkspaceState};
use crate::theme::Role;
use crate::{Avatar, Button};

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
    column_arranged(
        Modifier::new().weight(1.0),
        Arrangement::Start,
        Alignment::Start,
        || {
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
                    column_arranged(
                        Modifier::new()
                            .fill_max_width()
                            .width_in(0.0, PAGE_WIDTH)
                            .padding_xy(theme::sp(5), 0.0)
                            .padding_each(0.0, theme::sp(5), 0.0, 0.0)
                            .padding_each(0.0, 0.0, 0.0, theme::sp(8)),
                        Arrangement::SpacedBy(sp(6)),
                        Alignment::Start,
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
}

/// Emits the bar across the top: the project, where it lives, and the button that starts a session in it.
#[track_caller]
fn heading(state: &WorkspaceState, project: usize) {
    row_arranged(
        Modifier::new()
            .fill_max_width()
            .height(theme::sp(12))
            .draw_behind(theme::edge(
                theme::Side::Bottom,
                1.0,
                theme::color(Role::Border),
            ))
            .padding_each(0.0, 0.0, 0.0, 1.0)
            .padding_xy(theme::sp(5), 0.0),
        Arrangement::SpacedBy(sp(3)),
        Alignment::Center,
        || {
            if let Some(held) = state.projects.get(project) {
                Avatar::of(held).build();
                rustle::basic_text(
                    Modifier::new(),
                    &held.name,
                    theme::text_style(14, 1.4)
                        .font_weight(rustle::FontWeight::MEDIUM)
                        .color(theme::color(Role::Text))
                        .max_lines(1),
                    rustle::TextOverflow::Ellipsis,
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
        .interaction_focused(crate::focus::is_held(id))
        .interaction_background((theme::InteractionTones::new().hover(Role::SurfaceHover)).colors())
        .cursor(CursorIcon::Pointer)
        .control_menu(
            id,
            {
                let pressed = reduce(id);
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
            row_arranged(
                Modifier::new(),
                Arrangement::Start,
                Alignment::Start,
                || {
                    glyph(agent.icon, 3.5, Role::Muted);
                },
            );
        });
        if !session.changes.is_empty() {
            tally(session);
        }
        row_arranged(
            Modifier::new().width(BRANCH_WIDTH).clip(),
            Arrangement::Start,
            Alignment::Start,
            || {
                rustle::basic_text(
                    Modifier::new().weight(1.0),
                    &session.branch,
                    theme::text_style(12, 1.4)
                        .color(theme::color(Role::Muted))
                        .max_lines(1),
                    rustle::TextOverflow::Ellipsis,
                );
            },
        );
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
    row_arranged(
        Modifier::new(),
        Arrangement::SpacedBy(sp(1.5)),
        Alignment::Center,
        || {
            rustle::basic_text(
                Modifier::new(),
                format!("+{}", session.added()),
                theme::mono_style(12, 1.4)
                    .color(theme::color(Role::Success))
                    .max_lines(1),
                rustle::TextOverflow::Ellipsis,
            );
            rustle::basic_text(
                Modifier::new(),
                format!("−{}", session.removed()),
                theme::mono_style(12, 1.4)
                    .color(theme::color(Role::Danger))
                    .max_lines(1),
                rustle::TextOverflow::Ellipsis,
            );
        },
    );
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
            rustle::basic_text(
                Modifier::new().weight(1.0),
                &home.branch,
                theme::text_style(13, 1.4)
                    .color(theme::color(Role::Text))
                    .max_lines(1),
                rustle::TextOverflow::Ellipsis,
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
                rustle::basic_text(
                    Modifier::new(),
                    commit.id.chars().take(SHORT_ID).collect::<String>(),
                    theme::mono_style(12, 1.4)
                        .color(theme::color(Role::Subtle))
                        .max_lines(1),
                    rustle::TextOverflow::Ellipsis,
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
    row_arranged(
        Modifier::new()
            .fill_max_width()
            .height(theme::sp(11))
            .chain(modifier)
            .padding_xy(theme::sp(4), 0.0),
        Arrangement::SpacedBy(sp(3)),
        Alignment::Center,
        content,
    );
}
