//! The sidebar: ways to start and find work, the chats that belong to no project, and every project with its sessions.

use crate::theme;
use rustle::Modifier;
use rustle::{Alignment, Arrangement, BoxAlignment, column_arranged, row_arranged};
use rustle::{Icon, phosphor};
use std::rc::Rc;

use crate::focus::FocusId;
use crate::icons::spinner;
use crate::theme::{Radius, Role, sp};

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
pub const WIDTH: f32 = 280.0;

/// How narrow and how wide the sidebar can be dragged, in logical pixels.
pub const WIDTHS: (f32, f32) = (200.0, 520.0);

/// How tall a row of the sidebar is, in logical pixels.
const ROW: f32 = 36.0;

/// The room under a project's last row, in logical pixels.
const SECTION_GAP: f32 = 12.0;

/// How tall everything in the sidebar but its list of chats and projects is, in a
/// window of any height: the four entries.
const CHROME: f32 = 12.0 + 8.0 + 4.0 * ROW + 3.0 * 2.0;

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
        .filter(|session| session.project == Some(index) && !session.closed)
        .count()
        + 2
        + state.projects[index].submodules.len();
    rows as f32 * ROW + SECTION_GAP
}

/// How tall the group of chats is with its heading, in logical pixels: nothing while no chat is open.
pub fn chats_height(state: &WorkspaceState) -> f32 {
    match state.open_chats().len() {
        0 => 0.0,
        chats => (chats + 1) as f32 * ROW + SECTION_GAP,
    }
}

/// How tall the heading above the projects is, in logical pixels.
pub const HEADING_HEIGHT: f32 = ROW;

/// Emits the heading of a group of the list, saying `words`.
#[track_caller]
fn heading(words: &str) {
    row_arranged(
        Modifier::new()
            .height(theme::sp(9))
            .padding_xy(theme::sp(2), 0.0),
        Arrangement::Start,
        Alignment::Center,
        || {
            line(words, 12, 1.4, Role::Subtle);
        },
    );
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
    state
        .project_of(session)?
        .submodules
        .iter()
        .find(|path| *path == submodule)
        .map(String::as_str)
}

/// Emits the session activity or attention indicator, with its meaning on hover.
#[track_caller]
pub fn status_icon(status: Status) {
    let meaning = match status {
        Status::Working => "Working",
        Status::Unread => "Unread",
        Status::Error => "Unread error",
        Status::NeedsInput => "Needs input",
        Status::Idle => "Idle",
        Status::Unreachable => "Server out of reach",
    };
    crate::tooltip::with_tooltip(meaning, || {
        row_arranged(
            Modifier::new().size(theme::sp(4), theme::sp(4)),
            Arrangement::Center,
            Alignment::Center,
            move || match status {
                Status::Working => spinner(3.5, Role::Muted),
                Status::Unread => {
                    rustle::spacer(
                        Modifier::new()
                            .size(theme::sp(2), theme::sp(2))
                            .background_rounded(theme::color(Role::Accent), Radius::Full.px()),
                    );
                }
                Status::Error => glyph(phosphor::X, 3.5, Role::Danger),
                Status::NeedsInput => glyph(phosphor::CHAT_CIRCLE, 3.5, Role::Warning),
                Status::Unreachable => glyph(phosphor::PLUG, 3.5, Role::Warning),
                Status::Idle => {
                    rustle::spacer(
                        Modifier::new()
                            .size(theme::sp(2), theme::sp(2))
                            .border(1.0, theme::color(Role::Subtle), Radius::Full.px())
                            .padding(1.0),
                    );
                }
            },
        );
    });
}

/// Emits one of the ways in at the top: a glyph and a label.
#[track_caller]
fn entry(icon: Icon, label: &str, id: FocusId, selected: bool) {
    let tone = if selected { Role::Text } else { Role::Muted };
    Tile::new(id)
        .modifier(Modifier::new().fill_max_width())
        .selected(selected)
        .inner(Modifier::new().padding_xy(theme::sp(2), 0.0))
        .build_with(|| {
            glyph(icon, 4.0, tone);
            line(label, 14, 1.4, tone);
        });
}

/// Emits the row of one session under its project, set in `indent` steps.
#[track_caller]
fn session_row(index: usize, session: &Session, selected: bool, indent: f32) {
    let operations = crate::events::operations();
    Tile::new(FocusId(SESSION_BASE + index as u64))
        .on_context(move || operations.open_session_menu(index))
        .modifier(Modifier::new().fill_max_width())
        .selected(selected)
        .inner(
            Modifier::new()
                .padding_each(theme::sp(indent), 0.0, 0.0, 0.0)
                .padding_each(0.0, 0.0, theme::sp(2), 0.0),
        )
        .build_with(|| {
            status_icon(session.shown());
            clipped(&session.title, 14, Role::Text);
            line(session.age(), 12, 1.4, Role::Subtle);
        });
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

/// Emits the header of a submodule at `relative`, its path from what holds it, numbered `position` among [`submodule_rows`]: its own avatar in its project's colour, its name, the folder it is in and a button that starts work in it.
#[track_caller]
fn submodule_row(project: &Project, relative: &str, position: usize, starting: bool) {
    let (folder, name) = relative
        .rsplit_once('/')
        .map_or(("", relative), |(folder, name)| (folder, name));
    Tile::new(FocusId(NEW_IN_SUBMODULE_BASE + position as u64))
        .modifier(Modifier::new().fill_max_width())
        .selected(starting)
        .inner(
            Modifier::new()
                .padding_each(theme::sp(2), 0.0, 0.0, 0.0)
                .padding_each(0.0, 0.0, theme::sp(2), 0.0),
        )
        .build_with(|| {
            Avatar::new(name, project.color()).size(Size::Xs).build();
            row_arranged(
                Modifier::new().weight(1.0).clip(),
                Arrangement::SpacedBy(sp(1.5)),
                Alignment::Center,
                || {
                    line(name, 13, 1.4, Role::Text);
                    if !folder.is_empty() {
                        rustle::basic_text(
                            Modifier::new().weight(1.0),
                            format!("{folder}/"),
                            theme::text_style(12, 1.4)
                                .color(theme::color(Role::Subtle))
                                .max_lines(1),
                            rustle::TextOverflow::Ellipsis,
                        );
                    }
                },
            );
            glyph(
                phosphor::PLUS,
                3.5,
                if starting { Role::Text } else { Role::Subtle },
            );
        });
}

/// Emits the submodules of the project at `index` that sit directly in `parent`, each with the sessions working in it and the submodules inside it, behind a line that ties them to what holds them; nothing when there are none.
#[track_caller]
fn submodule_tree(
    state: &WorkspaceState,
    index: usize,
    parent: Option<&str>,
    members: &[(usize, &Session)],
    first: usize,
) {
    let project = &state.projects[index];
    let open = match state.page {
        Page::Session(open) => Some(open),
        _ => None,
    };
    let blocks: Vec<(usize, &String)> = project
        .submodules
        .iter()
        .enumerate()
        .filter(|(_, path)| holder(path, &project.submodules) == parent)
        .collect();
    if blocks.is_empty() {
        return;
    }
    column_arranged(
        Modifier::new()
            .fill_max_width()
            .padding_each(theme::sp(SUBMODULE_RAIL), 0.0, 0.0, 0.0)
            .draw_behind(theme::edge(
                theme::Side::Left,
                1.0,
                theme::color(project.color()),
            ))
            .padding_each(1.0, 0.0, 0.0, 0.0),
        Arrangement::Start,
        Alignment::Start,
        || {
            for (offset, path) in blocks {
                let name = parent
                    .and_then(|parent| path.strip_prefix(parent))
                    .map_or(path.as_str(), |rest| rest.trim_start_matches('/'));
                let starting = state.page == Page::New(index)
                    && state.new_in.as_deref() == Some(path.as_str());
                rustle::key(first + offset, || {
                    column_arranged(
                        Modifier::new().fill_max_width(),
                        Arrangement::Start,
                        Alignment::Start,
                        || {
                            submodule_row(project, name, first + offset, starting);
                            for &(position, session) in members.iter().filter(|(_, session)| {
                                listed_under(state, session) == Some(path.as_str())
                            }) {
                                session_row(position, session, open == Some(position), 3.5);
                            }
                            submodule_tree(state, index, Some(path), members, first);
                        },
                    );
                });
            }
        },
    );
}

/// Emits one project: its header, its sessions and a row to start new work, then each submodule with the sessions working in it.
#[track_caller]
fn project(state: &WorkspaceState, index: usize) {
    let project = &state.projects[index];
    let members: Vec<(usize, &Session)> = state
        .sessions
        .iter()
        .enumerate()
        .filter(|(_, session)| session.project == Some(index) && !session.closed)
        .collect();
    let first_submodule = submodule_rows(state)
        .iter()
        .position(|(held, _)| *held == index)
        .unwrap_or(0);
    let dragged = state.project_drag.is_some_and(|drag| drag.from == index);
    let open = match state.page {
        Page::Session(open) => Some(open),
        _ => None,
    };
    let starting = state.page == Page::New(index) && state.new_in.is_none();
    let section = Modifier::new()
        .fill_max_width()
        .padding_each(0.0, 0.0, 0.0, theme::sp(3));
    let section = if dragged {
        section.alpha(0.35)
    } else {
        section
    };
    column_arranged(section, Arrangement::Start, Alignment::Start, || {
        let operations = crate::events::operations();
        let opens = Rc::clone(&operations);
        Tile::new(FocusId(PROJECT_BASE + index as u64))
            .on_context(move || opens.open_project_menu(index))
            .modifier(Modifier::new().fill_max_width())
            .selected(state.page == Page::Project(index))
            .inner(Modifier::new().padding_xy(theme::sp(2), 0.0))
            .build_with(|| {
                Avatar::of(project).build();
                clipped(&project.name, 14, Role::Text);
                Button::icon(
                    phosphor::DOTS_THREE,
                    FocusId(PROJECT_MORE_BASE + index as u64),
                )
                .on_click(move || operations.open_project_menu(index))
                .build();
            });
        for &(position, session) in members
            .iter()
            .filter(|(_, session)| listed_under(state, session).is_none())
        {
            session_row(position, session, open == Some(position), 3.5);
        }
        submodule_tree(state, index, None, &members, first_submodule);
        let tone = if starting { Role::Text } else { Role::Muted };
        Tile::new(FocusId(NEW_IN_BASE + index as u64))
            .modifier(Modifier::new().fill_max_width())
            .selected(starting)
            .inner(Modifier::new().padding_each(theme::sp(3), 0.0, 0.0, 0.0))
            .build_with(|| {
                glyph(phosphor::PLUS, 4.0, tone);
                line("New session", 14, 1.4, tone);
            });
    });
}

/// Emits the chats that belong to no project, under a heading of their own, or nothing while no chat is open.
#[track_caller]
fn chats(state: &WorkspaceState) {
    let members = state.open_chats();
    if members.is_empty() {
        return;
    }
    let open = match state.page {
        Page::Session(open) => Some(open),
        _ => None,
    };
    column_arranged(
        Modifier::new()
            .fill_max_width()
            .padding_each(0.0, 0.0, 0.0, theme::sp(3)),
        Arrangement::Start,
        Alignment::Start,
        || {
            heading("Chats");
            for position in members {
                session_row(
                    position,
                    &state.sessions[position],
                    open == Some(position),
                    2.0,
                );
            }
        },
    );
}

/// Emits the whole sidebar for `state`.
#[track_caller]
pub fn sidebar(state: &WorkspaceState) {
    column_arranged(
        Modifier::new()
            .width(state.sidebar_width)
            .fill_max_height()
            .background(theme::color(Role::Sidebar))
            .draw_behind(theme::edge(
                theme::Side::Right,
                1.0,
                theme::color(Role::Border),
            ))
            .padding_each(0.0, 0.0, 1.0, 0.0),
        Arrangement::Start,
        Alignment::Start,
        || {
            column_arranged(
                Modifier::new()
                    .fill_max_width()
                    .padding_xy(theme::sp(2), 0.0)
                    .padding_each(0.0, theme::sp(3), 0.0, 0.0)
                    .padding_each(0.0, 0.0, 0.0, theme::sp(2)),
                Arrangement::SpacedBy(sp(0.5)),
                Alignment::Start,
                || {
                    entry(
                        phosphor::CHAT_CIRCLE,
                        "New chat",
                        crate::chats::NEW_CHAT,
                        state.page == Page::Chat,
                    );
                    entry(phosphor::PLUS, "New workspace", ADD_PROJECT, false);
                    entry(
                        phosphor::CLOCK_COUNTER_CLOCKWISE,
                        "History",
                        HISTORY,
                        state.page == Page::History,
                    );
                    entry(
                        phosphor::CODE,
                        "Extensions",
                        crate::extensions::EXTENSIONS,
                        state.page == Page::Extensions,
                    );
                },
            );
            rustle::stack_with(Modifier::new().weight(1.0).fill_max_width().clip(), || {
                column_arranged(
                    Modifier::new()
                        .offset(0.0, -state.sidebar_scroll)
                        .wrap_content_height(BoxAlignment::TopStart, true)
                        .fill_max_width()
                        .padding_xy(theme::sp(2), 0.0),
                    Arrangement::Start,
                    Alignment::Start,
                    || {
                        chats(state);
                        heading("Workspaces");
                        for index in state.display_order() {
                            rustle::key(index, || project(state, index));
                        }
                    },
                );
            });
        },
    );
}

/// Emits the header of the project being dragged, floating where the pointer is; nothing while no project is dragged.
#[track_caller]
pub fn drag_ghost(state: &WorkspaceState) {
    let Some(drag) = state.project_drag else {
        return;
    };
    let Some(project) = state.projects.get(drag.from) else {
        return;
    };
    crate::popups::at_window(sp(2.0), drag.at.1 - ROW / 2.0, false, || {
        row_arranged(
            Modifier::new()
                .height(theme::sp(9))
                .width(state.sidebar_width - 16.0)
                .draw_behind(theme::shadows(&theme::MEDIUM_SHADOW, Radius::Md))
                .background_rounded(theme::color(Role::Surface), Radius::Md.px())
                .border(1.0, theme::color(Role::Border), Radius::Md.px())
                .padding(1.0)
                .padding_xy(theme::sp(2), 0.0),
            Arrangement::SpacedBy(sp(3)),
            Alignment::Center,
            || {
                Avatar::of(project).build();
                clipped(&project.name, 14, Role::Text);
            },
        );
    });
}
