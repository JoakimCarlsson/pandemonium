//! The sidebar: ways to start and find work, and every project with its sessions.

use pm_icon::IconName;
use pm_input::FocusId;
use pm_style::{
    Radius, Role, absolute, animate_spin, bg, border, border_color, border_r, border_t, flex,
    flex_col, flex_none, font_mono, gap, h, h_full, items_center, justify_center, leading_ratio,
    left, mb, min_h, opacity, overflow_hidden, p, pl, pr, pt, px, rounded, rounded_full, shadow_md,
    size, text_color, text_px, top, w_px, z,
};
use pm_ui::{Node, node, text};

use crate::Avatar;
use crate::Button;
use crate::Tile;
use crate::arrange::PROJECT_MORE_BASE;
use crate::parts::{clipped, glyph, line, spacer};
use crate::sample::{Session, Status};
use crate::state::{
    ADD_PROJECT, HISTORY, NEW_IN_BASE, PROJECT_BASE, Page, SEARCH, SESSION_BASE, SETTINGS,
    WorkspaceState,
};

/// How wide the sidebar is, in logical pixels.
pub const WIDTH: f32 = 280.0;

/// How tall a row of the sidebar is, in logical pixels.
const ROW: f32 = 36.0;

/// The room under a project's last row, in logical pixels.
const SECTION_GAP: f32 = 12.0;

/// How tall everything in the sidebar but its list of projects is, in a
/// window of any height: the three entries, the label above the list and the footer.
const CHROME: f32 = 12.0 + 8.0 + 3.0 * ROW + 2.0 * 2.0 + ROW + 45.0;

/// How tall the part of the sidebar showing the list is, in a window `height` logical pixels tall.
pub fn list_viewport(height: f32) -> f32 {
    height - CHROME
}

/// How tall the project at `index` is with its sessions, in logical pixels.
pub fn project_height(state: &WorkspaceState, index: usize) -> f32 {
    let rows = if state.folded[index] {
        1
    } else {
        state
            .sessions
            .iter()
            .filter(|session| session.project == index && !session.closed)
            .count()
            + 2
    };
    rows as f32 * ROW + SECTION_GAP
}

/// The mark of where a session stands: a turning ring, a tick or a dot.
pub fn status_icon(status: Status) -> Node {
    let slot = node().styles([
        flex(),
        items_center(),
        justify_center(),
        size(4),
        flex_none(),
    ]);
    slot.child(match status {
        Status::Working => glyph(IconName::Loading, 3.5, Role::Muted).styles([animate_spin()]),
        Status::Review => glyph(IconName::CheckCircle, 4.0, Role::Success),
        Status::Done => node().styles([size(1.5), rounded_full(), bg(Role::Subtle)]),
    })
}

/// One of the three ways in at the top: a glyph and a label.
fn entry(icon: IconName, label: &str, id: FocusId, selected: bool) -> Node {
    let tone = if selected { Role::Text } else { Role::Muted };
    Tile::new(id)
        .selected(selected)
        .build()
        .styles([px(2)])
        .child(glyph(icon, 4.0, tone))
        .child(line(label, 14, 1.4, tone))
}

/// The row of one session under its project.
fn session_row(index: usize, session: &Session, selected: bool) -> Node {
    Tile::new(FocusId(SESSION_BASE + index as u64))
        .selected(selected)
        .build()
        .styles([pl(3.5), pr(2)])
        .child(status_icon(session.status))
        .child(clipped(&session.title, 14, Role::Text))
        .child(line(&session.updated, 12, 1.4, Role::Subtle))
}

/// One project: its header, and, unless folded, its sessions and a row to start new work.
fn project(state: &WorkspaceState, index: usize) -> Node {
    let project = &state.projects[index];
    let members: Vec<(usize, &Session)> = state
        .sessions
        .iter()
        .enumerate()
        .filter(|(_, session)| session.project == index && !session.closed)
        .collect();
    let folded = state.folded[index];
    let dragged = state.project_drag.is_some_and(|drag| drag.from == index);
    let mut header = Tile::new(FocusId(PROJECT_BASE + index as u64))
        .selected(false)
        .build()
        .styles([px(2)])
        .child(Avatar::of(project).build())
        .child(clipped(&project.name, 14, Role::Text));
    if folded {
        header = header.child(text(members.len().to_string()).styles([
            text_px(11),
            leading_ratio(1.4),
            font_mono(),
            text_color(Role::Subtle),
        ]));
    }
    header = header
        .child(Button::icon(IconName::More, FocusId(PROJECT_MORE_BASE + index as u64)).build());
    let mut section = node().styles([flex(), flex_col(), mb(3)]).child(header);
    if dragged {
        section = section.styles([opacity(35)]);
    }
    if !folded {
        let open = match state.page {
            Page::Session(open) => Some(open),
            _ => None,
        };
        section = section.children(
            members
                .into_iter()
                .map(|(position, session)| session_row(position, session, open == Some(position))),
        );
        let starting = state.page == Page::New(index);
        section = section.child(
            Tile::new(FocusId(NEW_IN_BASE + index as u64))
                .selected(starting)
                .build()
                .styles([pl(3)])
                .child(glyph(
                    IconName::Plus,
                    4.0,
                    if starting { Role::Text } else { Role::Muted },
                ))
                .child(line(
                    "New session",
                    14,
                    1.4,
                    if starting { Role::Text } else { Role::Muted },
                )),
        );
    }
    section
}

/// The whole sidebar for `state`.
pub fn sidebar(state: &WorkspaceState) -> Node {
    node()
        .styles([
            flex(),
            flex_col(),
            flex_none(),
            w_px(WIDTH),
            h_full(),
            bg(Role::Sidebar),
            border_r(),
            border_color(Role::Border),
        ])
        .child(
            node()
                .styles([flex(), flex_col(), gap(0.5), p(2), pt(3)])
                .children([
                    entry(IconName::Plus, "New workspace", ADD_PROJECT, false),
                    entry(
                        IconName::History,
                        "History",
                        HISTORY,
                        state.page == Page::History,
                    ),
                    entry(IconName::Search, "Search", SEARCH, false),
                ]),
        )
        .child(
            node()
                .styles([flex(), items_center(), h(9), px(4)])
                .child(line("Workspaces", 12, 1.4, Role::Subtle)),
        )
        .child(
            node()
                .styles([
                    flex(),
                    flex_col(),
                    px(2),
                    min_h(0),
                    overflow_hidden(),
                    pm_style::flex_1(),
                ])
                .children(
                    state
                        .display_order()
                        .into_iter()
                        .map(|index| project(state, index)),
                )
                .scroll_y(state.sidebar_scroll),
        )
        .child(
            node()
                .styles([
                    flex(),
                    items_center(),
                    gap(1),
                    p(2),
                    border_t(),
                    border_color(Role::Border),
                ])
                .child(spacer())
                .child(Button::icon(IconName::Settings, SETTINGS).build()),
        )
}

/// The header of the project being dragged, floating where the pointer is.
pub fn drag_ghost(state: &WorkspaceState) -> Option<Node> {
    let drag = state.project_drag?;
    let project = state.projects.get(drag.from)?;
    Some(
        node()
            .styles([
                absolute(),
                left(2),
                top((drag.at.1 - ROW / 2.0) / 4.0),
                z(60),
                flex(),
                items_center(),
                gap(3),
                h(9),
                w_px(WIDTH - 16.0),
                px(2),
                rounded(Radius::Md),
                bg(Role::Surface),
                border(),
                border_color(Role::Border),
                shadow_md(),
            ])
            .child(Avatar::of(project).build())
            .child(clipped(&project.name, 14, Role::Text)),
    )
}
