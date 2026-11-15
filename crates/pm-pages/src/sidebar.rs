//! The sidebar: navigation, the made-up projects' notice before real ones
//! exist, every project with its nested submodules, its sessions and a
//! "New session" row, each project with its menu and dragged to reorder,
//! the adder that adds a project, and the chats that belong to none; each
//! session and chat with a menu on right click and renamed in its row.

use std::rc::Rc;

use rustle::prelude::*;

use pm_components::{
    Avatar, Button, ButtonVariant, IconButton, Label, LabelVariant, Leading, Menu, Notice, Size,
    TextInput, Tile, TileVariant, project_colour,
};
use pm_shell::{
    AddStep, Page, Project, ProjectAction, ProjectReach, Session, SessionAction, SessionStatus,
    Workspace,
};

use crate::actions::{Actions, Destination};
use crate::adder::adder;

/// Indent of each nesting level.
const NEST: f32 = 12.0;

/// Emits the sidebar for `workspace`.
pub fn sidebar(workspace: &Workspace, actions: &Actions) {
    let scheme = color_scheme();
    let scroll = remember_scroll_state();
    column_with(
        Modifier::new()
            .width(workspace.sizes.sidebar)
            .fill_max_height()
            .background(scheme.surface_container_low)
            .vertical_scroll(&scroll)
            .padding_each(8.0, 10.0, 8.0, 10.0),
        || {
            for (destination, label, glyph) in [
                (Destination::NewChat, "New chat", phosphor::CHAT_CIRCLE),
                (Destination::NewWorkspace, "New workspace", phosphor::PLUS),
                (
                    Destination::History,
                    "History",
                    phosphor::CLOCK_COUNTER_CLOCKWISE,
                ),
                (Destination::Extensions, "Extensions", phosphor::CODE),
            ] {
                key(label, || {
                    let navigate = actions.navigate.clone();
                    let selected = match destination {
                        Destination::History => workspace.page == Page::History,
                        Destination::Extensions => workspace.page == Page::Extensions,
                        Destination::NewChat => workspace
                            .starting
                            .as_ref()
                            .is_some_and(|start| start.at.is_none()),
                        _ => false,
                    };
                    Tile::new(label)
                        .leading(Leading::Icon(glyph))
                        .selected(selected)
                        .on_click(move || navigate(destination))
                        .show();
                });
            }
            spacer(Modifier::new().height(pm_components::spacing(14.0)));
            if workspace.sample {
                sample_notice(actions);
            }
            workspaces_heading(workspace, actions);
            for (index, project) in workspace.projects.iter().enumerate() {
                key(project.path.clone(), || {
                    let reorder = actions.projects.clone();
                    column_with(
                        Modifier::new()
                            .fill_max_width()
                            .drag_source(index)
                            .drop_target(move |payload| {
                                if let Some(from) = payload.downcast_ref::<usize>() {
                                    reorder(ProjectAction::Reorder(*from, index));
                                }
                            }),
                        || project_rows(project, vec![index], workspace, actions),
                    );
                    spacer(Modifier::new().height(pm_components::spacing(8.0)));
                });
            }
            chats(workspace, actions);
        },
    );
}

/// The notice over the made-up projects, which stand in until the reader adds a real one.
fn sample_notice(actions: &Actions) {
    let add = actions.projects.clone();
    column_with(
        Modifier::new().fill_max_width().padding_xy(4.0, 4.0),
        || {
            Notice::new("These projects are examples")
                .detail("Add a folder or clone a repository to start working in your own.")
                .icon(phosphor::SPARKLE)
                .size(Size::Sm)
                .show();
            spacer(Modifier::new().height(pm_components::spacing(6.0)));
            Button::new("Add project")
                .icon(phosphor::FOLDER_PLUS)
                .variant(ButtonVariant::Primary)
                .size(Size::Sm)
                .on_click(move || add(ProjectAction::Adding(Some(AddStep::Choose))))
                .show();
        },
    );
    spacer(Modifier::new().height(pm_components::spacing(10.0)));
}

/// The "Workspaces" heading with the button that opens the adder, and the adder hung under it while open.
fn workspaces_heading(workspace: &Workspace, actions: &Actions) {
    let open = actions.projects.clone();
    let opened = workspace.adder.is_some();
    row_arranged(
        Modifier::new()
            .fill_max_width()
            .padding_each(8.0, 0.0, 2.0, 0.0),
        Arrangement::SpaceBetween,
        Alignment::Center,
        move || {
            Label::new("Workspaces")
                .variant(LabelVariant::Subtle)
                .size(Size::Xs)
                .show();
            IconButton::new(phosphor::PLUS)
                .size(Size::Xs)
                .active(opened)
                .on_click(move || open(ProjectAction::Adding((!opened).then_some(AddStep::Choose))))
                .show();
        },
    );
    if let Some(state) = &workspace.adder {
        adder(state, actions);
    }
}

/// The chats, conversations that belong to no project, under their own heading.
fn chats(workspace: &Workspace, actions: &Actions) {
    if workspace.chats.is_empty() {
        return;
    }
    spacer(Modifier::new().height(pm_components::spacing(6.0)));
    Label::new("Chats")
        .variant(LabelVariant::Subtle)
        .size(Size::Xs)
        .modifier(Modifier::new().padding_xy(8.0, 4.0))
        .show();
    for chat in &workspace.chats {
        key(chat.id, || {
            let open = actions.reopen_session.clone();
            let id = chat.id;
            let selected = workspace.page == Page::Session(id) && workspace.starting.is_none();
            session_tile(chat, 0.0, selected, true, workspace, actions, move || {
                open(id)
            });
        });
    }
}

/// The menu of the top-level project at `path`, at place `index` of `count`, hung under its header.
fn project_menu(project: &Project, at: &[usize], count: usize, actions: &Actions) {
    let index = at[0];
    let act = |action: ProjectAction| {
        let projects = actions.projects.clone();
        move || projects(action.clone())
    };
    let path = project.path.clone();
    let start = actions.new_session.clone();
    let overview = actions.open_overview.clone();
    let close = actions.projects.clone();
    let start_at = at.to_vec();
    let overview_path = path.clone();
    let mut menu = Menu::new()
        .offset(8.0, 0.0)
        .width(200.0)
        .on_dismiss(move || close(ProjectAction::Menu(None)))
        .icon_item(phosphor::PLUS, "New session", move || {
            start(start_at.clone())
        })
        .icon_item(phosphor::INFO, "Overview", move || {
            overview(overview_path.clone())
        })
        .icon_item(
            if project.collapsed {
                phosphor::CARET_DOWN
            } else {
                phosphor::CARET_UP
            },
            if project.collapsed { "Unfold" } else { "Fold" },
            act(ProjectAction::Fold(path.clone())),
        )
        .separator();
    menu = if index > 0 {
        menu.icon_item(
            phosphor::ARROW_UP,
            "Move up",
            act(ProjectAction::Move(path.clone(), -1)),
        )
    } else {
        menu.disabled_item(phosphor::ARROW_UP, "Move up")
    };
    menu = if index + 1 < count {
        menu.icon_item(
            phosphor::ARROW_DOWN,
            "Move down",
            act(ProjectAction::Move(path.clone(), 1)),
        )
    } else {
        menu.disabled_item(phosphor::ARROW_DOWN, "Move down")
    };
    menu.separator()
        .destructive_item(
            phosphor::TRASH,
            "Remove from sidebar",
            act(ProjectAction::Remove(path.clone())),
        )
        .show();
}

/// Emits the rows of `project` at index path `at`: its header, its nested
/// projects, its sessions and, at the top level, a "New session" row.
fn project_rows(project: &Project, at: Vec<usize>, workspace: &Workspace, actions: &Actions) {
    let depth = at.len() - 1;
    let tint = project_colour(&project.path);
    let name = project.name.clone();
    let avatar_size = if depth == 0 { Size::Sm } else { Size::Xs };
    let start = actions.new_session.clone();
    let start_at = at.clone();
    let mut header = Tile::new(project.name.clone())
        .leading(Leading::Custom(Rc::new(move || {
            Avatar::new(&name, tint).size(avatar_size).show()
        })))
        .indent(depth as f32 * NEST)
        .hover_icon(if depth == 0 {
            phosphor::DOTS_THREE
        } else {
            phosphor::PLUS
        });
    if depth == 0 {
        let overview = actions.open_overview.clone();
        let path = project.path.clone();
        let menu = actions.projects.clone();
        let menu_path = project.path.clone();
        header = header
            .variant(TileVariant::Header)
            .selected(workspace.page == Page::Overview(project.path.clone()))
            .on_hover_icon(move || menu(ProjectAction::Menu(Some(menu_path.clone()))))
            .on_click(move || overview(path.clone()));
    } else {
        header = header
            .size(Size::Xs)
            .on_click(move || start(start_at.clone()));
    }
    header = match &project.reach {
        Some(ProjectReach::Reaching) => header.trailing("reaching…"),
        Some(ProjectReach::Unreachable(_)) => header.trailing("offline"),
        _ => header,
    };
    header.show();
    if let Some(ProjectReach::Unreachable(reason)) = &project.reach {
        Label::new(reason.clone())
            .variant(LabelVariant::Subtle)
            .size(Size::Xs)
            .modifier(Modifier::new().padding_each(36.0, 0.0, 8.0, 4.0))
            .show();
    }
    if depth == 0 && workspace.project_menu.as_deref() == Some(project.path.as_str()) {
        project_menu(project, &at, workspace.projects.len(), actions);
    }
    if project.collapsed {
        return;
    }
    for (index, child) in project.children.iter().enumerate() {
        key(child.path.clone(), || {
            let mut path = at.clone();
            path.push(index);
            project_rows(child, path, workspace, actions);
        });
    }
    for (index, session) in project.sessions.iter().enumerate() {
        key(session.id, || {
            let mut path = at.clone();
            path.push(index);
            session_row(session, path, depth, workspace, actions);
        });
    }
    if depth == 0 {
        let start = actions.new_session.clone();
        let start_at = at.clone();
        Tile::new("New session")
            .leading(Leading::Icon(phosphor::PLUS))
            .variant(TileVariant::Action)
            .size(Size::Xs)
            .on_click(move || start(start_at.clone()))
            .show();
    }
}

/// Emits the row of `session`, found at index path `at` under a project `depth` levels deep.
fn session_row(
    session: &Session,
    at: Vec<usize>,
    depth: usize,
    workspace: &Workspace,
    actions: &Actions,
) {
    let open = actions.open_session.clone();
    let selected = workspace.open.as_ref() == Some(&at);
    session_tile(
        session,
        (depth + 1) as f32 * NEST,
        selected,
        false,
        workspace,
        actions,
        move || open(at.clone()),
    );
}

/// Emits the row of `session`, indented `indent`, a chat when `chat`,
/// calling `on_open` when clicked; or the field renaming it, and its menu
/// while open.
fn session_tile(
    session: &Session,
    indent: f32,
    selected: bool,
    chat: bool,
    workspace: &Workspace,
    actions: &Actions,
    on_open: impl Fn() + 'static,
) {
    let id = session.id;
    if let Some((_, title)) = workspace
        .renaming
        .as_ref()
        .filter(|(renaming, _)| *renaming == id)
    {
        rename_field(title, indent, actions);
        return;
    }
    let menu = actions.session.clone();
    Tile::new(session.title.clone())
        .leading(status_mark(session.status))
        .trailing(session.age.clone())
        .indent(indent)
        .size(Size::Xs)
        .selected(selected)
        .on_click(on_open)
        .on_menu(move || menu(SessionAction::Menu(Some(id))))
        .show();
    if workspace.session_menu == Some(id) {
        session_menu(session, indent, chat, actions);
    }
}

/// Emits the field renaming a session, holding `title`, indented `indent`.
fn rename_field(title: &str, indent: f32, actions: &Actions) {
    let (change, submit, cancel) = (
        actions.session.clone(),
        actions.session.clone(),
        actions.session.clone(),
    );
    TextInput::new(title)
        .autofocus()
        .size(Size::Sm)
        .modifier(
            Modifier::new()
                .fill_max_width()
                .padding_each(indent, 2.0, 0.0, 2.0)
                .on_key_event(move |event| {
                    let escape = event.pressed && event.key == rustle::Key::Escape;
                    if escape {
                        cancel(SessionAction::CancelRename);
                    }
                    escape
                }),
        )
        .on_change(move |text| change(SessionAction::RenameText(text)))
        .on_submit(move |_| submit(SessionAction::CommitRename))
        .show();
}

/// The menu of `session`, a chat when `chat`, hung under its row indented `indent`.
fn session_menu(session: &Session, indent: f32, chat: bool, actions: &Actions) {
    let id = session.id;
    let act = |action: SessionAction| {
        let run = actions.session.clone();
        move || run(action.clone())
    };
    let copy = |text: String| {
        let close = actions.session.clone();
        move || {
            if let Some(clipboard) = rustle::clipboard() {
                clipboard.set_text(&text);
            }
            close(SessionAction::Menu(None));
        }
    };
    let close = actions.session.clone();
    let mut menu = Menu::new()
        .offset(indent + 8.0, 0.0)
        .width(220.0)
        .on_dismiss(move || close(SessionAction::Menu(None)))
        .icon_item(
            phosphor::PENCIL_SIMPLE,
            "Rename",
            act(SessionAction::StartRename(id)),
        )
        .icon_item(
            phosphor::ENVELOPE_SIMPLE,
            if session.status == SessionStatus::Unread {
                "Mark as read"
            } else {
                "Mark as unread"
            },
            act(SessionAction::ToggleRead(id)),
        );
    if !session.directory.is_empty() {
        menu = menu.icon_item(phosphor::COPY, "Copy path", copy(session.directory.clone()));
        if !session.directory.starts_with("ssh://") {
            menu = menu.icon_item(
                phosphor::FOLDER_OPEN,
                "Open folder",
                act(SessionAction::OpenFolder(id)),
            );
        }
    }
    if !chat && !session.branch.is_empty() {
        menu = menu.icon_item(
            phosphor::GIT_BRANCH,
            "Copy branch name",
            copy(session.branch.clone()),
        );
    }
    menu = menu.separator();
    menu = if chat {
        menu.destructive_item(
            phosphor::TRASH,
            "Remove chat",
            act(SessionAction::Remove(id)),
        )
    } else {
        menu.icon_item(
            phosphor::ARCHIVE,
            "Close session",
            act(SessionAction::Close(id)),
        )
    };
    menu.show();
}

/// The mark a session's row leads with for `status`.
fn status_mark(status: SessionStatus) -> Leading {
    match status {
        SessionStatus::Idle => Leading::Dot { filled: false },
        SessionStatus::Unread => Leading::Dot { filled: true },
        SessionStatus::Working => Leading::Spinner,
        SessionStatus::NeedsInput => Leading::Attention,
        SessionStatus::Error => Leading::Failed,
    }
}
