//! The sidebar: ways to start and find work, and every project with its sessions.

use pm_icon::IconName;
use pm_input::FocusId;
use pm_style::{
    Radius, Role, animate_pulse, bg, border_color, border_r, border_t, cursor_pointer, flex,
    flex_col, flex_none, font_mono, gap, h, h_full, items_center, justify_center, leading_ratio,
    mb, min_h, overflow_hidden, p, pl, pr, pt, px, rounded, rounded_full, size, text_color,
    text_px, w_px,
};
use pm_ui::{Node, node, text};

use super::sample::{Session, Status};
use super::state::{
    ADD_PROJECT, HISTORY, NEW, NEW_IN_BASE, PROJECT_BASE, Page, SEARCH, SESSION_BASE, SETTINGS,
    WorkspaceState,
};
use crate::Avatar;
use crate::Button;
use crate::parts::{clipped, glyph, line, spacer};

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
pub(super) fn list_viewport(height: f32) -> f32 {
    height - CHROME
}

/// How tall the project at `index` is with its sessions, in logical pixels.
pub(super) fn project_height(state: &WorkspaceState, index: usize) -> f32 {
    let rows = if state.folded[index] {
        1
    } else {
        state
            .sessions
            .iter()
            .filter(|session| session.project == index)
            .count()
            + 2
    };
    rows as f32 * ROW + SECTION_GAP
}

/// A row of the sidebar: `selected` fills it, and it answers to `id`.
fn tile(id: FocusId, selected: bool) -> Node {
    let mut styles = vec![
        flex(),
        items_center(),
        gap(3),
        h(9),
        rounded(Radius::Md),
        cursor_pointer(),
        bg(Role::SurfaceHover).hover(),
        bg(Role::SurfaceActive).active(),
    ];
    if selected {
        styles.push(bg(Role::SurfaceSelected));
    }
    node().styles(styles).focusable(id)
}

/// The mark of where a session stands: a turning ring, a tick or a dot.
pub(super) fn status_icon(status: Status) -> Node {
    let slot = node().styles([
        flex(),
        items_center(),
        justify_center(),
        size(4),
        flex_none(),
    ]);
    slot.child(match status {
        Status::Working => glyph(IconName::Loading, 3.5, Role::Muted).styles([animate_pulse()]),
        Status::Review => glyph(IconName::CheckCircle, 4.0, Role::Success),
        Status::Done => node().styles([size(1.5), rounded_full(), bg(Role::Subtle)]),
    })
}

/// One of the three ways in at the top: a glyph and a label.
fn entry(icon: IconName, label: &str, id: FocusId, selected: bool) -> Node {
    let tone = if selected { Role::Text } else { Role::Muted };
    tile(id, selected)
        .styles([px(2)])
        .child(glyph(icon, 4.0, tone))
        .child(line(label, 14, 1.4, tone))
}

/// The row of one session under its project.
fn session_row(index: usize, session: &Session, selected: bool) -> Node {
    tile(FocusId(SESSION_BASE + index as u64), selected)
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
        .filter(|(_, session)| session.project == index)
        .collect();
    let folded = state.folded[index];
    let mut header = tile(FocusId(PROJECT_BASE + index as u64), false)
        .styles([px(2)])
        .child(Avatar::of(project).build())
        .child(clipped(&project.name, 14, Role::Text));
    if folded && !members.is_empty() {
        header = header.child(text(members.len().to_string()).styles([
            text_px(11),
            leading_ratio(1.4),
            font_mono(),
            text_color(Role::Subtle),
        ]));
    }
    let mut section = node().styles([flex(), flex_col(), mb(3)]).child(header);
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
            tile(FocusId(NEW_IN_BASE + index as u64), starting)
                .styles([pl(3)])
                .child(glyph(
                    IconName::Plus,
                    4.0,
                    if starting { Role::Text } else { Role::Muted },
                ))
                .child(line(
                    "New workspace",
                    14,
                    1.4,
                    if starting { Role::Text } else { Role::Muted },
                )),
        );
    }
    section
}

/// The whole sidebar for `state`.
pub(super) fn sidebar(state: &WorkspaceState) -> Node {
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
                    entry(IconName::Plus, "New workspace", NEW, false),
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
                .children((0..state.projects.len()).map(|index| project(state, index)))
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
                .child(Button::icon(IconName::Plus, ADD_PROJECT).build())
                .child(spacer())
                .child(Button::icon(IconName::Settings, SETTINGS).build()),
        )
}
