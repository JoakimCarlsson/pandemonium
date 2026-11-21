//! The sidebar: ways to start and find work, the chats that belong to no project, and every project with its sessions.

use crate::areas::{SettingsEvent, SidebarEvent};
use crate::pointing::Over;
use crate::target::Target;
use crate::theme;
use crate::theme::Frame;
use crate::theme::ShadowLayers;
use rustle::Modifier;
use rustle::TextOverflow;
use rustle::{Alignment, Arrangement, IntrinsicSize, row_arranged};
use rustle::{Icon, phosphor};
use std::rc::Rc;

use crate::icons::spinner;
use crate::theme::{Radius, Role, sp};

use crate::Avatar;
use crate::Button;
use crate::Project;
use crate::Tile;
use crate::parts::{clipped, glyph, line};
use crate::sample::{Session, Status};
use crate::size::Size;
use crate::state::{Page, WorkspaceState};

/// How wide the sidebar is until it is dragged wider or narrower, in logical pixels.
pub const WIDTH: f32 = 280.0;

/// How narrow and how wide the sidebar can be dragged, in logical pixels.
pub const WIDTHS: (f32, f32) = (200.0, 520.0);

/// How tall a row of the sidebar is, in logical pixels.
const ROW: f32 = 36.0;

/// How far, in steps, the line that nests submodules under what holds them is set in from the rows above it: under the middle of the avatar.
const SUBMODULE_RAIL: f32 = 4.5;

/// Emits the heading of a group of the list, saying `words`.
#[track_caller]
fn heading(words: &str) {
    let banded = Modifier::new()
        .height(theme::sp(9))
        .padding_xy(theme::sp(2), 0.0);
    rustle::row_arranged(banded, Arrangement::Start, Alignment::Center, || {
        line(words, 12, 1.4, Role::Subtle)
    });
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
    let dot = Modifier::new().size(theme::sp(2), theme::sp(2));
    crate::tooltip::with_tooltip(meaning, || {
        row_arranged(
            Modifier::new().size(theme::sp(4), theme::sp(4)),
            Arrangement::Center,
            Alignment::Center,
            move || match status {
                Status::Working => spinner(3.5, Role::Muted),
                Status::Unread => rustle::spacer(
                    dot.background_rounded(theme::color(Role::Accent), Radius::Full.px()),
                ),
                Status::Error => glyph(phosphor::X, 3.5, Role::Danger),
                Status::NeedsInput => glyph(phosphor::CHAT_CIRCLE, 3.5, Role::Warning),
                Status::Unreachable => glyph(phosphor::PLUG, 3.5, Role::Warning),
                Status::Idle => rustle::spacer(
                    dot.border(1.0, theme::color(Role::Subtle), Radius::Full.px())
                        .padding(1.0),
                ),
            },
        );
    });
}

/// The session the page shows, by position, if it shows one.
fn open_session(state: &WorkspaceState) -> Option<usize> {
    match state.page {
        Page::Session(open) => Some(open),
        _ => None,
    }
}

/// A row of the sidebar across its width answering to `id`, filled while `selected`, its content padded by `inner`.
fn tile(id: Target, selected: bool, inner: Modifier) -> Tile {
    Tile::new(id)
        .modifier(Modifier::new().fill_max_width())
        .selected(selected)
        .inner(inner)
}

/// The modifier of a row of the project `index`, its header, its row for new work or one of
/// its sessions, as a place a carried project header can be let go over.
fn project_place(index: usize) -> Modifier {
    crate::events::carried_over(false, move |_, _| Over::Project(index))
}

/// Emits one of the ways in at the top: a glyph and a label.
#[track_caller]
fn entry(icon: Icon, label: &str, id: Target, selected: bool) {
    let inner = Modifier::new().padding_xy(theme::sp(2), 0.0);
    glyph_row(tile(id, selected, inner), icon, label, selected);
}

/// Builds `tile` holding a glyph and a label, in the text role while `selected` and muted otherwise.
#[track_caller]
fn glyph_row(tile: Tile, icon: Icon, label: &str, selected: bool) {
    let tone = if selected { Role::Text } else { Role::Muted };
    tile.build_with(|| {
        glyph(icon, 4.0, tone);
        line(label, 14, 1.4, tone);
    });
}

/// Emits the row of one session under its project, set in `indent` steps.
#[track_caller]
fn session_row(index: usize, session: &Session, selected: bool, indent: f32) {
    let operations = crate::events::operations();
    tile(
        Target::Sidebar(SidebarEvent::Session(index)),
        selected,
        Modifier::new().padding_each(theme::sp(indent), 0.0, theme::sp(2), 0.0),
    )
    .modifier(session.project.map_or_else(Modifier::new, project_place))
    .on_context(move |at| operations.open_session_menu(index, at))
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
fn submodule_row(project: &Project, relative: &str, position: usize, starting: bool) {
    let (folder, name) = relative
        .rsplit_once('/')
        .map_or(("", relative), |(folder, name)| (folder, name));
    tile(
        Target::Sidebar(SidebarEvent::NewInSubmodule(position)),
        starting,
        Modifier::new().padding_each(theme::sp(2), 0.0, theme::sp(2), 0.0),
    )
    .build_with(|| {
        Avatar::new(name, project.color()).size(Size::Xs).build();
        rustle::row_arranged(
            Modifier::new().weight(1.0).clip(),
            Arrangement::SpacedBy(theme::sp(1.5)),
            Alignment::Center,
            || {
                line(name, 13, 1.4, Role::Text);
                if !folder.is_empty() {
                    rustle::basic_text(
                        Modifier::new().weight(1.0),
                        format!("{folder}/"),
                        theme::line(12, Role::Subtle),
                        TextOverflow::Ellipsis,
                    );
                }
            },
        );
        let plus = if starting { Role::Text } else { Role::Subtle };
        glyph(phosphor::PLUS, 3.5, plus);
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
    let open = open_session(state);
    let blocks: Vec<(usize, &String)> = project
        .submodules
        .iter()
        .enumerate()
        .filter(|(_, path)| holder(path, &project.submodules) == parent)
        .collect();
    if blocks.is_empty() {
        return;
    }
    rustle::row_arranged(
        Modifier::new()
            .fill_max_width()
            .padding_each(theme::sp(SUBMODULE_RAIL), 0.0, 0.0, 0.0)
            .height_intrinsic(IntrinsicSize::Min),
        Arrangement::Start,
        Alignment::Start,
        || {
            rustle::vertical_divider(Modifier::new(), 1.0, theme::color(project.color()));
            rustle::column_arranged(
                Modifier::new().weight(1.0),
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
                            rustle::column_arranged(
                                Modifier::new().fill_max_width(),
                                Arrangement::Start,
                                Alignment::Start,
                                || {
                                    submodule_row(project, name, first + offset, starting);
                                    for &(position, session) in
                                        members.iter().filter(|(_, session)| {
                                            listed_under(state, session) == Some(path.as_str())
                                        })
                                    {
                                        session_row(position, session, open == Some(position), 3.5);
                                    }
                                    submodule_tree(state, index, Some(path), members, first);
                                },
                            );
                        });
                    }
                },
            );
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
    let open = open_session(state);
    let starting = state.page == Page::New(index) && state.new_in.is_none();
    let section = Modifier::new()
        .fill_max_width()
        .padding_each(0.0, 0.0, 0.0, theme::sp(3));
    let section = if dragged {
        section.alpha(0.35)
    } else {
        section
    };
    rustle::column_arranged(section, Arrangement::Start, Alignment::Start, || {
        let operations = crate::events::operations();
        let opens = Rc::clone(&operations);
        tile(
            Target::Sidebar(SidebarEvent::Project(index)),
            state.page == Page::Project(index),
            Modifier::new().padding_xy(theme::sp(2), 0.0),
        )
        .modifier(project_place(index))
        .on_context(move |at| opens.open_project_menu(index, Some(at)))
        .build_with(|| {
            Avatar::of(project).build();
            clipped(&project.name, 14, Role::Text);
            Button::icon(
                phosphor::DOTS_THREE,
                Target::Sidebar(SidebarEvent::ProjectMore(index)),
            )
            .on_click(move || operations.open_project_menu(index, None))
            .build();
        });
        for &(position, session) in members
            .iter()
            .filter(|(_, session)| listed_under(state, session).is_none())
        {
            session_row(position, session, open == Some(position), 3.5);
        }
        submodule_tree(state, index, None, &members, first_submodule);
        let inner = Modifier::new().padding_each(theme::sp(3), 0.0, 0.0, 0.0);
        let new_in = tile(Target::Sidebar(SidebarEvent::NewIn(index)), starting, inner)
            .modifier(project_place(index));
        glyph_row(new_in, phosphor::PLUS, "New session", starting);
    });
}

/// Emits the chats that belong to no project, under a heading of their own, or nothing while no chat is open.
#[track_caller]
fn chats(state: &WorkspaceState) {
    let members = state.open_chats();
    if members.is_empty() {
        return;
    }
    let open = open_session(state);
    rustle::column_arranged(
        Modifier::new()
            .fill_max_width()
            .padding_each(0.0, 0.0, 0.0, theme::sp(3)),
        Arrangement::Start,
        Alignment::Start,
        || {
            heading("Chats");
            for position in members {
                let session = &state.sessions[position];
                session_row(position, session, open == Some(position), 2.0);
            }
        },
    );
}

/// Emits the whole sidebar for `state`.
#[track_caller]
pub fn sidebar(state: &WorkspaceState) {
    state.sidebar_scroll.set_scrollbar(rustle::ScrollbarStyle {
        visibility: rustle::ScrollbarVisibility::Hidden,
        ..theme::scrollbar()
    });
    rustle::column_arranged(
        Modifier::new()
            .width(state.sidebar_width)
            .fill_max_height()
            .background(theme::color(Role::Sidebar))
            .border_right_hairline(theme::color(Role::Border))
            .padding_each(0.0, 0.0, 1.0, 0.0),
        Arrangement::Start,
        Alignment::Start,
        || {
            rustle::column_arranged(
                Modifier::new().fill_max_width().padding_each(
                    theme::sp(2),
                    theme::sp(3),
                    theme::sp(2),
                    theme::sp(2),
                ),
                Arrangement::SpacedBy(theme::sp(0.5)),
                Alignment::Start,
                || {
                    entry(
                        phosphor::CHAT_CIRCLE,
                        "New chat",
                        Target::Sidebar(SidebarEvent::NewChat),
                        state.page == Page::Chat,
                    );
                    entry(
                        phosphor::PLUS,
                        "New workspace",
                        Target::Sidebar(SidebarEvent::AddProject),
                        false,
                    );
                    entry(
                        phosphor::CLOCK_COUNTER_CLOCKWISE,
                        "History",
                        Target::Sidebar(SidebarEvent::History),
                        state.page == Page::History,
                    );
                    entry(
                        phosphor::CODE,
                        "Extensions",
                        Target::Settings(SettingsEvent::Extensions),
                        state.page == Page::Extensions,
                    );
                },
            );
            rustle::column_arranged(
                Modifier::new()
                    .weight(1.0)
                    .fill_max_width()
                    .vertical_scroll(&state.sidebar_scroll)
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
        rustle::row_arranged(
            Modifier::new()
                .height(theme::sp(9))
                .width(state.sidebar_width - 16.0)
                .shadow_layers(&theme::MEDIUM_SHADOW, Radius::Md.px())
                .panel(Radius::Md)
                .padding_xy(theme::sp(2), 0.0),
            Arrangement::SpacedBy(theme::sp(3)),
            Alignment::Center,
            || {
                Avatar::of(project).build();
                clipped(&project.name, 14, Role::Text);
            },
        );
    });
}
