//! The history page: every session of every project, newest first, grouped by when it last changed and narrowed by a search.

use crate::kit::sp;
use rustle::Modifier;
use rustle::{Alignment, Arrangement, column_arranged, row_arranged};

use crate::kit::{
    FocusId, IconName, Radius, Role, Scroll, Side, Styled, Type, label, label_with, scroll_area,
};

use crate::age::{self, Span};
use crate::input::{self, Input};
use crate::parts::{clipped, glyph};
use crate::sample::Session;
use crate::size::Size;
use crate::state::{HISTORY_SEARCH, SESSION_BASE, WorkspaceState};
use crate::tile::Tile;

/// The scrolling list of session groups on the history page.
pub const HISTORY_SCROLL: FocusId = FocusId(9_300_000);

impl WorkspaceState {
    /// The state after the history list is scrolled by `delta` logical pixels, kept within `room` pixels of overflow.
    pub fn scrolled_history(mut self, delta: f32, room: f32) -> Self {
        self.history_scroll = (self.history_scroll + delta).clamp(0.0, room);
        self
    }
}

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
            scroll_area(
                Modifier::new().fill_max_width().weight(1.0),
                Scroll::from_start(HISTORY_SCROLL, state.history_scroll),
                || {
                    column_arranged(
                        Modifier::new().fill_max_width().pad_x(5),
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
        },
    );
}

/// Emits the bar across the top naming the page.
#[track_caller]
fn heading() {
    row_arranged(
        Modifier::new()
            .fill_max_width()
            .h(12)
            .stroke_side(Side::Bottom, 1.0, Role::Border)
            .pad_x(5),
        Arrangement::Start,
        Alignment::Center,
        || {
            label(
                "History",
                Type::new(14)
                    .ratio(1.4)
                    .tone(Role::Text)
                    .truncate()
                    .medium(),
            );
        },
    );
}

/// Emits the box the reader types into to narrow the list.
#[track_caller]
fn search(state: &WorkspaceState) {
    column_arranged(
        Modifier::new()
            .pad_x(5)
            .pad_side(Side::Top, 5)
            .pad_side(Side::Bottom, 2),
        Arrangement::Start,
        Alignment::Start,
        || {
            row_arranged(
                Modifier::new()
                    .width(SEARCH_WIDTH)
                    .h(10)
                    .fill_rounded(Role::Surface, Radius::Lg)
                    .stroke(Role::Border, Radius::Lg)
                    .pad_x(3),
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
        Modifier::new().fill_max_width().pad_side(Side::Top, 5),
        Arrangement::SpacedBy(sp(0.5)),
        Alignment::Start,
        || {
            row_arranged(
                Modifier::new().h(8).pad_x(3),
                Arrangement::Start,
                Alignment::Center,
                || {
                    label(
                        span.title(),
                        Type::new(14)
                            .ratio(1.4)
                            .tone(Role::Text)
                            .truncate()
                            .medium(),
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
    Tile::new(FocusId(SESSION_BASE + index as u64))
        .height(12.0)
        .modifier(Modifier::new().fill_max_width())
        .inner(Modifier::new().pad_x(3))
        .build_with(|| {
            row_arranged(
                Modifier::new().weight(1.0),
                Arrangement::SpacedBy(sp(2)),
                Alignment::Center,
                || {
                    label_with(
                        Modifier::new().max_w_px(560.0),
                        &session.title,
                        Type::new(14).ratio(1.4).tone(Role::Text).truncate(),
                    );
                    glyph(IconName::ChevronRight, 4.0, Role::Subtle);
                    glyph(agent.icon, 4.0, Role::Muted);
                    label(
                        agent.name,
                        Type::new(14).ratio(1.4).tone(Role::Muted).truncate(),
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
        Modifier::new().width(width).clip_to(Radius::None),
        arrangement,
        Alignment::Start,
        || clipped(content, 14, Role::Muted),
    );
}
