//! The sidebar: ways to start and find work, and every project with its sessions.

use pm_icon::IconName;
use pm_input::FocusId;
use pm_style::{
    Radius, Role, absolute, animate_spin, bg, border, border_color, border_l, border_r, flex,
    flex_col, flex_none, gap, h, h_full, items_center, justify_center, left, mb, min_h, ml,
    opacity, overflow_hidden, p, pl, pr, pt, px, rounded, rounded_full, shadow_md, size, top, w_px,
    z,
};
use pm_ui::{Node, node};

use crate::Avatar;
use crate::Button;
use crate::Project;
use crate::Tile;
use crate::arrange::PROJECT_MORE_BASE;
use crate::parts::{clipped, glyph, line};
use crate::sample::{Session, Status};
use crate::size::Size;
use crate::state::{
    ADD_PROJECT, HISTORY, NEW_IN_BASE, NEW_IN_SUBMODULE_BASE, PROJECT_BASE, Page, SESSION_BASE,
    WorkspaceState,
};

/// How wide the sidebar is until it is dragged wider or narrower, in logical pixels.
pub const WIDTH: f32 = 264.0;

/// How narrow and how wide the sidebar can be dragged, in logical pixels.
pub const WIDTHS: (f32, f32) = (200.0, 520.0);

/// How tall a row of the sidebar is, in logical pixels.
const ROW: f32 = 28.0;

/// The room under a project's last row, in logical pixels.
const SECTION_GAP: f32 = 8.0;

/// How tall everything in the sidebar but its list of projects is, in a
/// window of any height: the three entries and the label above the list.
const CHROME: f32 = 12.0 + 8.0 + 3.0 * ROW + 2.0 * 2.0 + ROW;

/// How tall the part of the sidebar showing the list is, in a window `height` logical pixels tall, under the title bar.
pub fn list_viewport(height: f32) -> f32 {
    height - crate::title_bar::TITLE_BAR_HEIGHT - CHROME
}

/// How far, in steps, the line that nests submodules under what holds them is set in from the rows above it: under the middle of the avatar.
const SUBMODULE_RAIL: f32 = 4.5;

/// How tall the project at `index` is with its sessions and its submodules, in logical pixels.
pub fn project_height(state: &WorkspaceState, index: usize) -> f32 {
    let rows = state
        .sessions
        .iter()
        .filter(|session| session.project == index && !session.closed)
        .count()
        + 2
        + state.projects[index].submodules.len();
    rows as f32 * ROW + SECTION_GAP
}

/// Every submodule row of the sidebar, project by project in the order they were added: the project's position and the submodule's path. The place of a row in this list is what it answers to.
pub fn submodule_rows(state: &WorkspaceState) -> Vec<(usize, String)> {
    state
        .projects
        .iter()
        .enumerate()
        .flat_map(|(index, project)| {
            project
                .submodules
                .iter()
                .map(move |path| (index, path.clone()))
        })
        .collect()
}

/// The submodule of its project a session is listed under: the one it works in, when its project still lists it.
fn listed_under<'a>(state: &'a WorkspaceState, session: &Session) -> Option<&'a str> {
    let submodule = session.submodule.as_deref()?;
    state.projects[session.project]
        .submodules
        .iter()
        .find(|path| *path == submodule)
        .map(String::as_str)
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
        .height(7.0)
        .gap(2.0)
        .selected(selected)
        .build()
        .styles([px(2)])
        .child(glyph(icon, 4.0, tone))
        .child(line(label, 13, 1.4, tone))
}

/// The row of one session under its project, set in `indent` steps.
fn session_row(index: usize, session: &Session, selected: bool, indent: f32) -> Node {
    Tile::new(FocusId(SESSION_BASE + index as u64))
        .height(7.0)
        .gap(2.0)
        .selected(selected)
        .build()
        .styles([pl(indent), pr(2)])
        .child(status_icon(session.shown()))
        .child(clipped(&session.title, 13, Role::Text))
        .child(line(session.age(), 12, 1.4, Role::Subtle))
}

/// The submodule of `paths` that `path` sits directly inside, or nothing when it sits in the project itself.
fn holder<'a>(path: &str, paths: &'a [String]) -> Option<&'a str> {
    paths
        .iter()
        .filter(|held| {
            path.strip_prefix(held.as_str())
                .is_some_and(|rest| rest.starts_with('/'))
        })
        .max_by_key(|held| held.len())
        .map(String::as_str)
}

/// The header of a submodule at `relative`, its path from what holds it, numbered `position` among [`submodule_rows`]: its own avatar in its project's colour, its name, the folder it is in and a button that starts work in it.
fn submodule_row(project: &Project, relative: &str, position: usize, starting: bool) -> Node {
    let (folder, name) = relative
        .rsplit_once('/')
        .map_or(("", relative), |(folder, name)| (folder, name));
    Tile::new(FocusId(NEW_IN_SUBMODULE_BASE + position as u64))
        .height(7.0)
        .gap(2.0)
        .selected(starting)
        .build()
        .styles([pl(2), pr(2)])
        .child(Avatar::new(name, project.color()).size(Size::Xs).build())
        .child(
            node()
                .styles([
                    flex(),
                    items_center(),
                    gap(1.5),
                    pm_style::flex_1(),
                    pm_style::min_w_px(0),
                    overflow_hidden(),
                ])
                .child(line(name, 13, 1.4, Role::Text).styles([flex_none()]))
                .children((!folder.is_empty()).then(|| {
                    line(format!("{folder}/"), 12, 1.4, Role::Subtle).styles([pm_style::truncate()])
                })),
        )
        .child(glyph(
            IconName::Plus,
            3.5,
            if starting { Role::Text } else { Role::Subtle },
        ))
}

/// The submodules of the project at `index` that sit directly in `parent`, each with the sessions working in it and the submodules inside it, behind a line that ties them to what holds them.
fn submodule_tree(
    state: &WorkspaceState,
    index: usize,
    parent: Option<&str>,
    members: &[(usize, &Session)],
    first: usize,
) -> Option<Node> {
    let project = &state.projects[index];
    let open = match state.page {
        Page::Session(open) => Some(open),
        _ => None,
    };
    let blocks: Vec<Node> = project
        .submodules
        .iter()
        .enumerate()
        .filter(|(_, path)| holder(path, &project.submodules) == parent)
        .map(|(offset, path)| {
            let name = parent
                .and_then(|parent| path.strip_prefix(parent))
                .map_or(path.as_str(), |rest| rest.trim_start_matches('/'));
            let starting =
                state.page == Page::New(index) && state.new_in.as_deref() == Some(path.as_str());
            node()
                .styles([flex(), flex_col()])
                .child(submodule_row(project, name, first + offset, starting))
                .children(
                    members
                        .iter()
                        .filter(|(_, session)| listed_under(state, session) == Some(path.as_str()))
                        .map(|&(position, session)| {
                            session_row(position, session, open == Some(position), 3.5)
                        }),
                )
                .children(submodule_tree(state, index, Some(path), members, first))
        })
        .collect();
    (!blocks.is_empty()).then(|| {
        node()
            .styles([
                flex(),
                flex_col(),
                ml(SUBMODULE_RAIL),
                border_l(),
                border_color(project.color()),
            ])
            .children(blocks)
    })
}

/// One project: its header, its sessions and a row to start new work, then each submodule with the sessions working in it.
fn project(state: &WorkspaceState, index: usize) -> Node {
    let project = &state.projects[index];
    let members: Vec<(usize, &Session)> = state
        .sessions
        .iter()
        .enumerate()
        .filter(|(_, session)| session.project == index && !session.closed)
        .collect();
    let first_submodule = submodule_rows(state)
        .iter()
        .position(|(held, _)| *held == index)
        .unwrap_or(0);
    let dragged = state.project_drag.is_some_and(|drag| drag.from == index);
    let mut header = Tile::new(FocusId(PROJECT_BASE + index as u64))
        .height(7.0)
        .gap(2.0)
        .selected(state.page == Page::Project(index))
        .build()
        .styles([px(2)])
        .child(Avatar::of(project).build())
        .child(clipped(&project.name, 13, Role::Text));
    header = header
        .child(Button::icon(IconName::More, FocusId(PROJECT_MORE_BASE + index as u64)).build());
    let mut section = node().styles([flex(), flex_col(), mb(2)]).child(header);
    if dragged {
        section = section.styles([opacity(35)]);
    }
    let open = match state.page {
        Page::Session(open) => Some(open),
        _ => None,
    };
    section = section.children(
        members
            .iter()
            .filter(|(_, session)| listed_under(state, session).is_none())
            .map(|&(position, session)| {
                session_row(position, session, open == Some(position), 3.5)
            }),
    );
    section = section.children(submodule_tree(
        state,
        index,
        None,
        &members,
        first_submodule,
    ));
    let starting = state.page == Page::New(index) && state.new_in.is_none();
    section = section.child(
        Tile::new(FocusId(NEW_IN_BASE + index as u64))
            .height(7.0)
            .gap(2.0)
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
                13,
                1.4,
                if starting { Role::Text } else { Role::Muted },
            )),
    );
    section
}

/// The whole sidebar for `state`.
pub fn sidebar(state: &WorkspaceState) -> Node {
    node()
        .styles([
            flex(),
            flex_col(),
            flex_none(),
            w_px(state.sidebar_width),
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
                    entry(
                        IconName::FileCode,
                        "Extensions",
                        crate::extensions::EXTENSIONS,
                        state.page == Page::Extensions,
                    ),
                ]),
        )
        .child(
            node()
                .styles([flex(), items_center(), h(7), px(4)])
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
                h(7),
                w_px(state.sidebar_width - 16.0),
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
