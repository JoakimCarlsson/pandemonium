//! The history page: every session of every project, newest first, grouped by when it last changed and narrowed by a search.

use crate::areas::SidebarEvent;
use crate::kit::{self, Frame};
use crate::target::Target;
use crate::theme;
use rustle::Modifier;
use rustle::phosphor;
use rustle::{Alignment, Arrangement, row_arranged};

use crate::theme::{Radius, Role};

use crate::age::{self, Span};
use crate::input::{self, Input};
use crate::parts::{clipped, glyph};
use crate::sample::Session;
use crate::size::Size;
use crate::state::WorkspaceState;
use crate::tile::Tile;

impl WorkspaceState {}

/// How wide the search box is.
const SEARCH_WIDTH: f32 = 500.0;

/// How wide the column naming the project is.
const PROJECT_WIDTH: f32 = 144.0;

/// How wide the column naming the machine is.
const MACHINE_WIDTH: f32 = 112.0;

/// How wide the column naming the branch is.
const BRANCH_WIDTH: f32 = 176.0;

/// How wide the column saying how long ago it was is.
const AGE_WIDTH: f32 = 80.0;

/// What the machine column says for a project on this machine.
const LOCAL_MACHINE: &str = "local";

/// Emits the history page for `state`.
#[track_caller]
pub fn history(state: &WorkspaceState) {
    let now = age::now();
    let rows = matching(state);
    kit::column(Modifier::new().weight(1.0), 0, || {
        heading();
        search(state);
        kit::column(
            Modifier::new()
                .fill_max_width()
                .weight(1.0)
                .vertical_scroll(&state.history_scroll)
                .fill_max_width()
                .padding_xy(theme::sp(5), 0.0),
            0,
            || {
                for span in Span::ALL {
                    group(state, span, &rows, now);
                }
            },
        );
    });
}

/// Emits the bar across the top naming the page.
#[track_caller]
fn heading() {
    kit::row(
        Modifier::new()
            .fill_max_width()
            .height(theme::sp(12))
            .ruled(theme::Side::Bottom)
            .padding_xy(theme::sp(5), 0.0),
        0,
        || {
            kit::text(Modifier::new(), "History", kit::strong(14, Role::Text));
        },
    );
}

/// Emits the box the reader types into to narrow the list.
#[track_caller]
fn search(state: &WorkspaceState) {
    kit::column(
        Modifier::new()
            .padding_xy(theme::sp(5), 0.0)
            .padding_each(0.0, theme::sp(5), 0.0, 0.0)
            .padding_each(0.0, 0.0, 0.0, theme::sp(2)),
        0,
        || {
            kit::row(
                Modifier::new()
                    .width(SEARCH_WIDTH)
                    .height(theme::sp(10))
                    .panel(Radius::Lg)
                    .padding_xy(theme::sp(3), 0.0),
                2,
                || {
                    glyph(phosphor::MAGNIFYING_GLASS, 4.0, Role::Muted);
                    Input::new(
                        &state.history_search,
                        Target::Sidebar(SidebarEvent::HistorySearch),
                    )
                    .variant(input::Variant::Ghost)
                    .size(Size::Sm)
                    .placeholder("Search history")
                    .modifier(Modifier::new().weight(1.0))
                    .build();
                },
            );
        },
    );
}

/// The sessions that match the search, by position, the one changed last first.
fn matching(state: &WorkspaceState) -> Vec<usize> {
    let needle = state.history_search.text().to_lowercase();
    let mut found: Vec<usize> = (0..state.sessions.len())
        .filter(|&index| !state.sessions[index].home)
        .filter(|&index| needle.is_empty() || searchable(state, index).contains(&needle))
        .collect();
    found.sort_by_key(|&index| std::cmp::Reverse(state.sessions[index].updated_at));
    found
}

/// Everything the search looks in for the session at `index`, in lower case.
fn searchable(state: &WorkspaceState, index: usize) -> String {
    let session = &state.sessions[index];
    format!(
        "{} {} {} {} {}",
        session.title,
        state.agent_look(session.agent).name,
        state
            .project_of(session)
            .map_or("", |project| project.name.as_str()),
        state.machine_of(session).flatten().unwrap_or_default(),
        session.branch,
    )
    .to_lowercase()
}

/// Emits the heading and rows of `span`, when a session in `rows` falls in it.
#[track_caller]
fn group(state: &WorkspaceState, span: Span, rows: &[usize], now: u64) {
    let members: Vec<usize> = rows
        .iter()
        .copied()
        .filter(|&index| Span::of(state.sessions[index].updated_at, now) == span)
        .collect();
    if members.is_empty() {
        return;
    }
    kit::column(
        Modifier::new()
            .fill_max_width()
            .padding_each(0.0, theme::sp(5), 0.0, 0.0),
        0.5,
        || {
            kit::row(
                Modifier::new()
                    .height(theme::sp(8))
                    .padding_xy(theme::sp(3), 0.0),
                0,
                || {
                    kit::text(Modifier::new(), span.title(), kit::strong(14, Role::Text));
                },
            );
            for index in members {
                rustle::key(index, || row_of(state, index));
            }
        },
    );
}

/// Emits the row of the session at `index`: its title and agent, then where it ran and when.
#[track_caller]
fn row_of(state: &WorkspaceState, index: usize) {
    let session: &Session = &state.sessions[index];
    let project = state
        .project_of(session)
        .map_or("", |project| project.name.as_str());
    let machine = state
        .machine_of(session)
        .flatten()
        .unwrap_or_else(|| LOCAL_MACHINE.to_owned());
    let agent = state.agent_look(session.agent);
    let operations = crate::events::operations();
    Tile::new(Target::Sidebar(SidebarEvent::Session(index)))
        .on_context(move || operations.open_session_menu(index))
        .height(12.0)
        .modifier(Modifier::new().fill_max_width())
        .inner(Modifier::new().padding_xy(theme::sp(3), 0.0))
        .build_with(|| {
            kit::row(Modifier::new().weight(1.0), 2, || {
                kit::text(
                    Modifier::new().width_in(0.0, 560.0),
                    &session.title,
                    kit::line(14, Role::Text),
                );
                glyph(phosphor::CARET_RIGHT, 4.0, Role::Subtle);
                glyph(agent.icon, 4.0, Role::Muted);
                kit::text(Modifier::new(), agent.name, kit::line(14, Role::Muted));
            });
            cell(project, PROJECT_WIDTH, false);
            cell(&machine, MACHINE_WIDTH, false);
            cell(&session.branch, BRANCH_WIDTH, false);
            cell(&session.age(), AGE_WIDTH, true);
        });
}

/// Emits a cell `width` pixels wide holding `content`, cut short at its edge, against its end when `end`.
#[track_caller]
fn cell(content: &str, width: f32, end: bool) {
    let arrangement = if end {
        Arrangement::End
    } else {
        Arrangement::Start
    };
    row_arranged(
        Modifier::new().width(width).clip(),
        arrangement,
        Alignment::Start,
        || clipped(content, 14, Role::Muted),
    );
}
