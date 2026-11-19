//! The history page: every session of every project, newest first, grouped by when it last changed and narrowed by a search.

use crate::theme;
use crate::theme::sp;
use rustle::Modifier;
use rustle::{Alignment, Arrangement, column_arranged, row_arranged};

use crate::focus::FocusId;
use crate::icons::IconName;
use crate::theme::{Radius, Role};

use crate::age::{self, Span};
use crate::input::{self, Input};
use crate::parts::{clipped, glyph};
use crate::sample::Session;
use crate::size::Size;
use crate::state::{HISTORY_SEARCH, SESSION_BASE, WorkspaceState};
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
    column_arranged(
        Modifier::new().weight(1.0),
        Arrangement::Start,
        Alignment::Start,
        || {
            heading();
            search(state);
            column_arranged(
                Modifier::new()
                    .fill_max_width()
                    .weight(1.0)
                    .vertical_scroll(&state.history_scroll)
                    .fill_max_width()
                    .padding_xy(theme::sp(5), 0.0),
                Arrangement::Start,
                Alignment::Start,
                || {
                    for span in Span::ALL {
                        group(state, span, &rows, now);
                    }
                },
            );
        },
    );
}

/// Emits the bar across the top naming the page.
#[track_caller]
fn heading() {
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
        Arrangement::Start,
        Alignment::Center,
        || {
            rustle::basic_text(
                Modifier::new(),
                "History",
                theme::text_style(14, 1.4)
                    .font_weight(rustle::FontWeight::MEDIUM)
                    .color(theme::color(Role::Text))
                    .max_lines(1),
                rustle::TextOverflow::Ellipsis,
            );
        },
    );
}

/// Emits the box the reader types into to narrow the list.
#[track_caller]
fn search(state: &WorkspaceState) {
    column_arranged(
        Modifier::new()
            .padding_xy(theme::sp(5), 0.0)
            .padding_each(0.0, theme::sp(5), 0.0, 0.0)
            .padding_each(0.0, 0.0, 0.0, theme::sp(2)),
        Arrangement::Start,
        Alignment::Start,
        || {
            row_arranged(
                Modifier::new()
                    .width(SEARCH_WIDTH)
                    .height(theme::sp(10))
                    .background_rounded(theme::color(Role::Surface), Radius::Lg.px())
                    .border(1.0, theme::color(Role::Border), Radius::Lg.px())
                    .padding(1.0)
                    .padding_xy(theme::sp(3), 0.0),
                Arrangement::SpacedBy(sp(2)),
                Alignment::Center,
                || {
                    glyph(IconName::Search, 4.0, Role::Muted);
                    Input::new(&state.history_search, HISTORY_SEARCH)
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
    column_arranged(
        Modifier::new()
            .fill_max_width()
            .padding_each(0.0, theme::sp(5), 0.0, 0.0),
        Arrangement::SpacedBy(sp(0.5)),
        Alignment::Start,
        || {
            row_arranged(
                Modifier::new()
                    .height(theme::sp(8))
                    .padding_xy(theme::sp(3), 0.0),
                Arrangement::Start,
                Alignment::Center,
                || {
                    rustle::basic_text(
                        Modifier::new(),
                        span.title(),
                        theme::text_style(14, 1.4)
                            .font_weight(rustle::FontWeight::MEDIUM)
                            .color(theme::color(Role::Text))
                            .max_lines(1),
                        rustle::TextOverflow::Ellipsis,
                    );
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
    Tile::new(FocusId(SESSION_BASE + index as u64))
        .on_context(move || operations.open_session_menu(index))
        .height(12.0)
        .modifier(Modifier::new().fill_max_width())
        .inner(Modifier::new().padding_xy(theme::sp(3), 0.0))
        .build_with(|| {
            row_arranged(
                Modifier::new().weight(1.0),
                Arrangement::SpacedBy(sp(2)),
                Alignment::Center,
                || {
                    rustle::basic_text(
                        Modifier::new().width_in(0.0, 560.0),
                        &session.title,
                        theme::text_style(14, 1.4)
                            .color(theme::color(Role::Text))
                            .max_lines(1),
                        rustle::TextOverflow::Ellipsis,
                    );
                    glyph(IconName::ChevronRight, 4.0, Role::Subtle);
                    glyph(agent.icon, 4.0, Role::Muted);
                    rustle::basic_text(
                        Modifier::new(),
                        agent.name,
                        theme::text_style(14, 1.4)
                            .color(theme::color(Role::Muted))
                            .max_lines(1),
                        rustle::TextOverflow::Ellipsis,
                    );
                },
            );
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
