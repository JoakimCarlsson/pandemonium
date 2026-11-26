//! The history page: every session of every project, newest first, grouped by when it last changed and narrowed by a search.

use crate::areas::SidebarEvent;
use crate::parts;
use crate::target::Target;
use crate::theme;
use crate::theme::Frame;
use rustle::Modifier;
use rustle::TextOverflow;
use rustle::phosphor;
use rustle::{Alignment, Arrangement, row_arranged};

use crate::theme::{Radius, Role};

use crate::age::{self, Span};
use crate::agent_list::AgentLook;
use crate::input::{self, Input};
use crate::parts::{clipped, glyph};
use crate::sample::Session;
use crate::size::Size;
use crate::state::WorkspaceState;
use crate::tile::Tile;
use rustle::TextFieldState;

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

/// The row of one session as the history page shows it.
#[derive(Clone, Debug, PartialEq)]
struct HistoryRow {
    /// The session's position among every session.
    index: usize,
    /// When it last changed, as the group it falls in.
    span: Span,
    /// Its title.
    title: String,
    /// The agent that works in it.
    agent: AgentLook,
    /// The name of its project, or nothing.
    project: String,
    /// The machine it ran on.
    machine: String,
    /// Its branch.
    branch: String,
    /// How long ago it changed, in words.
    age: String,
}

/// What the history page shows, and nothing else, compared by value, so the page is
/// composed again only when it changed.
#[derive(Clone, Debug, PartialEq)]
pub struct HistoryState {
    /// The sessions that match the search, the one changed last first.
    rows: Vec<HistoryRow>,
    /// The search box.
    search: TextFieldState,
    /// The lazy list of the groups and rows: how far it is scrolled and how tall its items are.
    scroll: rustle::LazyListState,
}

impl HistoryState {
    /// What the history page of `state` shows.
    pub fn of(state: &WorkspaceState) -> Self {
        let now = age::now();
        let rows = matching(state)
            .into_iter()
            .map(|index| {
                let session: &Session = &state.sessions[index];
                HistoryRow {
                    index,
                    span: Span::of(session.updated_at, now),
                    title: session.title.clone(),
                    agent: state.agent_look(session.agent),
                    project: state
                        .project_of(session)
                        .map_or(String::new(), |project| project.name.clone()),
                    machine: state
                        .machine_of(session)
                        .flatten()
                        .unwrap_or_else(|| LOCAL_MACHINE.to_owned()),
                    branch: session.branch.clone(),
                    age: session.age(),
                }
            })
            .collect();
        Self {
            rows,
            search: state.history_search.clone(),
            scroll: state.history_scroll.clone(),
        }
    }
}

/// One item of the history page's lazy list.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Item {
    /// The heading of the group of `Span`, the first of the page when it is.
    Heading(Span, bool),
    /// The row at this place among the rows.
    Row(usize),
}

/// The items of the history page for `rows`, from the top: each group that has rows, its
/// heading over them.
fn items(rows: &[HistoryRow]) -> Vec<Item> {
    let mut items = Vec::new();
    for span in Span::ALL {
        let first = items.is_empty();
        let mut members = rows
            .iter()
            .enumerate()
            .filter(|(_, row)| row.span == span)
            .peekable();
        if members.peek().is_none() {
            continue;
        }
        items.push(Item::Heading(span, first));
        items.extend(members.map(|(place, _)| Item::Row(place)));
    }
    items
}

/// Emits the history page `view` describes, its groups and rows a lazy list that composes
/// only the items in view.
#[track_caller]
pub fn history(view: &HistoryState) {
    rustle::column_arranged(
        Modifier::new().weight(1.0),
        Arrangement::Start,
        Alignment::Start,
        || {
            heading();
            search(&view.search);
            let items = items(&view.rows);
            rustle::lazy_list_with(
                Modifier::new().fill_max_width().weight(1.0),
                &view.scroll,
                rustle::LazyContent::new().spacing(theme::sp(0.5)),
                items.len(),
                |index| match items[index] {
                    Item::Heading(span, _) => (0, span.title().to_owned()),
                    Item::Row(place) => (1, view.rows[place].index.to_string()),
                },
                |index| {
                    rustle::column_arranged(
                        Modifier::new()
                            .fill_max_width()
                            .padding_xy(theme::sp(5), 0.0),
                        Arrangement::Start,
                        Alignment::Start,
                        || match items[index] {
                            Item::Heading(span, first) => group_heading(span, first),
                            Item::Row(place) => row_of(&view.rows[place]),
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
    parts::bar(12, 5, 0, || {
        rustle::basic_text(
            Modifier::new(),
            "History",
            theme::strong(14, Role::Text),
            TextOverflow::Ellipsis,
        );
    });
}

/// Emits the box the reader types into to narrow the list, `field`.
#[track_caller]
fn search(field: &TextFieldState) {
    rustle::column_arranged(
        Modifier::new().padding_each(theme::sp(5), theme::sp(5), theme::sp(5), theme::sp(2)),
        Arrangement::Start,
        Alignment::Start,
        || {
            rustle::row_arranged(
                Modifier::new()
                    .width(SEARCH_WIDTH)
                    .height(theme::sp(10))
                    .panel(Radius::Lg)
                    .padding_xy(theme::sp(3), 0.0),
                Arrangement::SpacedBy(theme::sp(2)),
                Alignment::Center,
                || {
                    glyph(phosphor::MAGNIFYING_GLASS, 4.0, Role::Muted);
                    Input::new(field, Target::Sidebar(SidebarEvent::HistorySearch))
                        .variant(input::Variant::Ghost)
                        .size(Size::Sm)
                        .placeholder("Search history")
                        .modifier(Modifier::new().weight(1.0))
                        .submits(Target::Sidebar(SidebarEvent::History))
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

/// Emits the heading of the group of `span`, set off from the group above it, or from the
/// top of the list when it is the `first`.
#[track_caller]
fn group_heading(span: Span, first: bool) {
    let above = if first { 5.0 } else { 4.5 };
    rustle::row_arranged(
        Modifier::new()
            .padding_each(0.0, theme::sp(above), 0.0, 0.0)
            .height(theme::sp(8))
            .padding_xy(theme::sp(3), 0.0),
        Arrangement::Start,
        Alignment::Center,
        || {
            rustle::basic_text(
                Modifier::new(),
                span.title(),
                theme::strong(14, Role::Text),
                TextOverflow::Ellipsis,
            );
        },
    );
}

/// Emits the row `row` of a session: its title and agent, then where it ran and when.
#[track_caller]
fn row_of(row: &HistoryRow) {
    let index = row.index;
    let operations = crate::events::operations();
    Tile::new(Target::Sidebar(SidebarEvent::Session(index)))
        .on_context(move |at| operations.open_session_menu(index, at))
        .height(12.0)
        .modifier(Modifier::new().fill_max_width())
        .inner(Modifier::new().padding_xy(theme::sp(3), 0.0))
        .build_with(|| {
            rustle::row_arranged(
                Modifier::new().weight(1.0),
                Arrangement::SpacedBy(theme::sp(2)),
                Alignment::Center,
                || {
                    rustle::basic_text(
                        Modifier::new().width_in(0.0, 560.0),
                        &row.title,
                        theme::line(14, Role::Text),
                        TextOverflow::Ellipsis,
                    );
                    glyph(phosphor::CARET_RIGHT, 4.0, Role::Subtle);
                    glyph(row.agent.icon.clone(), 4.0, Role::Muted);
                    rustle::basic_text(
                        Modifier::new(),
                        &row.agent.name,
                        theme::line(14, Role::Muted),
                        TextOverflow::Ellipsis,
                    );
                },
            );
            cell(&row.project, PROJECT_WIDTH, false);
            cell(&row.machine, MACHINE_WIDTH, false);
            cell(&row.branch, BRANCH_WIDTH, false);
            cell(&row.age, AGE_WIDTH, true);
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
