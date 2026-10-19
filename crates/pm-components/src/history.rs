//! The history page: every session of every project, newest first, grouped by when it last changed and narrowed by a search.

use pm_icon::IconName;
use pm_input::FocusId;
use pm_style::{
    Radius, Role, bg, border, border_b, border_color, flex, flex_1, flex_col, flex_none,
    font_medium, gap, h, items_center, min_h, min_w_px, overflow_hidden, pb, pt, px, rounded,
    text_px, truncate, w_px,
};
use pm_ui::{Node, node};

use crate::age::{self, Span};
use crate::input::{self, Input};
use crate::parts::{clipped, glyph, line};
use crate::sample::Session;
use crate::size::Size;
use crate::state::{HISTORY_SEARCH, SESSION_BASE, WorkspaceState};
use crate::tile::Tile;

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

/// The history page for `state`.
pub fn history(state: &WorkspaceState) -> Node {
    let now = age::now();
    let rows = matching(state);
    node()
        .styles([flex(), flex_col(), flex_1(), min_w_px(0), min_h(0)])
        .child(heading())
        .child(search(state))
        .child(
            node()
                .styles([
                    flex(),
                    flex_col(),
                    flex_1(),
                    min_h(0),
                    overflow_hidden(),
                    px(5),
                ])
                .children(
                    Span::ALL
                        .into_iter()
                        .filter_map(|span| group(state, span, &rows, now)),
                ),
        )
}

/// The bar across the top naming the page.
fn heading() -> Node {
    node()
        .styles([
            flex(),
            items_center(),
            h(12),
            px(5),
            flex_none(),
            border_b(),
            border_color(Role::Border),
        ])
        .child(line("History", 14, 1.4, Role::Text).styles([font_medium(), text_px(14)]))
}

/// The box the reader types into to narrow the list.
fn search(state: &WorkspaceState) -> Node {
    node().styles([px(5), pt(5), pb(2), flex_none()]).child(
        node()
            .styles([
                flex(),
                items_center(),
                gap(2),
                w_px(SEARCH_WIDTH),
                h(10),
                px(3),
                rounded(Radius::Lg),
                bg(Role::Surface),
                border(),
                border_color(Role::Border),
            ])
            .child(glyph(IconName::Search, 4.0, Role::Muted))
            .child(
                Input::new(&state.history_search, HISTORY_SEARCH)
                    .variant(input::Variant::Ghost)
                    .size(Size::Sm)
                    .placeholder("Search history")
                    .build(),
            ),
    )
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
    let project = &state.projects[session.project];
    format!(
        "{} {} {} {} {}",
        session.title,
        state.agent_look(session.agent).name,
        project.name,
        project.host().unwrap_or_default(),
        session.branch,
    )
    .to_lowercase()
}

/// The heading and rows of `span`, when a session in `rows` falls in it.
fn group(state: &WorkspaceState, span: Span, rows: &[usize], now: u64) -> Option<Node> {
    let members: Vec<usize> = rows
        .iter()
        .copied()
        .filter(|&index| Span::of(state.sessions[index].updated_at, now) == span)
        .collect();
    if members.is_empty() {
        return None;
    }
    Some(
        node()
            .styles([flex(), flex_col(), gap(0.5), flex_none(), pt(5)])
            .child(
                node()
                    .styles([flex(), items_center(), h(8), px(3)])
                    .child(line(span.title(), 14, 1.4, Role::Text).styles([font_medium()])),
            )
            .children(members.into_iter().map(|index| row(state, index))),
    )
}

/// The row of the session at `index`: its title and agent, then where it ran and when.
fn row(state: &WorkspaceState, index: usize) -> Node {
    let session: &Session = &state.sessions[index];
    let project = &state.projects[session.project];
    let agent = state.agent_look(session.agent);
    Tile::new(FocusId(SESSION_BASE + index as u64))
        .height(12.0)
        .build()
        .styles([px(3)])
        .child(
            node()
                .styles([flex(), items_center(), gap(2), flex_1(), min_w_px(0)])
                .child(
                    node()
                        .styles([flex_none(), overflow_hidden(), pm_style::max_w_px(560.0)])
                        .child(line(&session.title, 14, 1.4, Role::Text).styles([truncate()])),
                )
                .child(glyph(IconName::ChevronRight, 4.0, Role::Subtle))
                .child(glyph(agent.icon, 4.0, Role::Muted))
                .child(line(agent.name, 14, 1.4, Role::Muted).styles([flex_none()])),
        )
        .child(column(&project.name, PROJECT_WIDTH))
        .child(column(
            &project.host().unwrap_or_else(|| LOCAL_MACHINE.to_owned()),
            MACHINE_WIDTH,
        ))
        .child(column(&session.branch, BRANCH_WIDTH))
        .child(column(&session.age(), AGE_WIDTH).styles([pm_style::justify_end()]))
}

/// A cell `width` pixels wide holding `content`, cut short at its edge.
fn column(content: &str, width: f32) -> Node {
    node()
        .styles([flex(), flex_none(), w_px(width), overflow_hidden()])
        .child(clipped(content, 14, Role::Muted))
}
