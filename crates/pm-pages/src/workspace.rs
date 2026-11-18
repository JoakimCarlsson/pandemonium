//! The workspace window: the sidebar of projects and sessions beside the page the reader is on.

use pm_components::kit::{Flex, Role, Styled, column, layer, row};
use rustle::Modifier;

use pm_components::arrange::{
    PROJECT_MENU_NEW, PROJECT_MENU_REMOVE, ProjectMenu, SESSION_MENU_CLOSE,
    SESSION_MENU_COPY_BRANCH, SESSION_MENU_COPY_PATH, SESSION_MENU_OPEN_FOLDER, SESSION_MENU_READ,
    SESSION_MENU_REMOVE, SESSION_MENU_RENAME, SessionMenu,
};
use pm_components::context_menu::{editor_menu, field_menu};
use pm_components::git_prompts;
use pm_components::message_menu::message_menu;
use pm_components::open_files::EDITOR_FIELD;
use pm_components::title_bar::TITLE_BAR_HEIGHT;
use pm_components::{Menu, TitleBar, Window};
use pm_components::{adder, history, plan, review, settings, sidebar, terminal};

pub use pm_components::state::{
    ADD_PROJECT, ATTACH, CHAT_SCROLL, DIFF_SCROLL, HISTORY, MENU_SCROLL, Outgoing, PLAN_SCROLL,
    PROMPT, SEND, STOP, TAB_DIFF, TERMINAL, TERMINAL_BODY, TREE_SCROLL, WorkspaceState,
};

use pm_components::sash::{PLAN_SASH, REVIEW_SASH, SIDEBAR_SASH, Sash, TERMINAL_SASH};
use pm_components::state::Page;

use crate::panes;

/// Emits the whole workspace screen for `state` in a window `width` logical pixels wide and `height` tall: the title bar over the sidebar and the page, with the handles between them, the menus and the dialogs laid over the window.
#[track_caller]
pub fn workspace(state: &WorkspaceState, width: f32, height: f32) {
    let page = height - TITLE_BAR_HEIGHT;
    let planning = state.plan_shown().is_some();
    let reviewing = state.review_session().is_some() && state.review;
    let sidebar_width = state.sidebar_room();
    let review_width = if reviewing { state.review_width } else { 0.0 };
    let main_width = state.pane_area((width, height)).width;
    let plan_width = width - sidebar_width - review_width - main_width;
    let paned = state.paned();
    layer(Modifier::new().fill_max_size(), || {
        column(
            Modifier::new().fill_max_size().fill(Role::Background),
            Flex::new(),
            || {
                TitleBar::of(state).build();
                row(Modifier::new().grow().fill_max_width(), Flex::new(), || {
                    if state.sidebar_open {
                        sidebar::sidebar(state);
                    }
                    match state.page {
                        _ if paned => panes::main_area(state, (width, height)),
                        Page::History => history::history(state),
                        Page::Extensions => pm_components::extensions::extensions(state),
                        _ => panes::starting_page(state, main_width, page),
                    }
                    if let Some(source) = state.plan_shown() {
                        plan::panel(state, source, plan_width);
                    }
                    if let Some(session) = state.review_session().filter(|_| reviewing) {
                        review::review(state, session, page);
                    }
                });
            },
        );
        if state.sidebar_open {
            Sash::column(SIDEBAR_SASH)
                .at((sidebar_width, TITLE_BAR_HEIGHT), page)
                .build();
        }
        if planning {
            Sash::column(PLAN_SASH)
                .at((sidebar_width + main_width, TITLE_BAR_HEIGHT), page)
                .build();
        }
        if reviewing {
            Sash::column(REVIEW_SASH)
                .at((width - review_width, TITLE_BAR_HEIGHT), page)
                .build();
        }
        if state.docking() {
            Sash::row(TERMINAL_SASH)
                .at(
                    (sidebar_width, height - terminal::height(state, page)),
                    main_width,
                )
                .build();
        }
        if paned {
            panes::main_over(state, (width, height));
        }
        over(state, height, reviewing);
    });
}

/// Emits what is laid over the workspace window of `height` logical pixels for `state`: the open menus, drag ghosts, the project adder, the git menus and prompts while `reviewing`, and the dialogs.
#[track_caller]
fn over(state: &WorkspaceState, height: f32, reviewing: bool) {
    if let Some(menu) = state.menu
        && !settings::is_settings(menu.field)
        && !pm_components::folder_picker::is_picker(menu.field)
        && let Some(field) = state.field(menu.field)
    {
        if menu.field == EDITOR_FIELD {
            editor_menu(
                field,
                menu,
                state.coding(),
                state.review_is_local(),
                &state.editor.keymap(),
            );
        } else {
            field_menu(field, menu);
        }
    }
    sidebar::drag_ghost(state);
    pm_components::file_tree::drag_ghost(state);
    if let Some(open) = state.project_menu {
        project_menu(open);
    }
    if let Some(open) = state.pane_menu {
        panes::pane_menu(open);
    }
    if let Some(open) = state.session_menu {
        session_menu(state, open);
    }
    if let Some(open) = state.message_menu {
        message_menu(open);
    }
    if let Some(open) = state.terminal_menu {
        pm_components::terminal_menu::terminal_menu(open, &state.editor.keymap());
    }
    if let Some(open) = state.browser_menu {
        pm_components::browser_menu::browser_menu(open);
    }
    adder::adder(state);
    if reviewing {
        state.git_menu_node();
    }
    if let Some(open) = state.tab_menu
        && let Some(viewer) = state.git.viewers.get(open.index)
    {
        pm_components::tab_menu::tab_menu(
            viewer,
            open,
            state.git.viewers.len(),
            state.review_is_local(),
        );
    }
    if let Some(open) = state.git.prompt.as_ref() {
        git_prompts::prompt(state, open);
    }
    pm_components::browser_clear::clear_question(state);
    pm_components::sign_in_code::code_dialog(state);
    pm_components::assist::assist(state, height);
}

/// Emits the menu of a project: start a session in it, or take it out of the window.
#[track_caller]
fn project_menu(open: ProjectMenu) {
    Menu::new(open.at.0, open.at.1)
        .item("New session", None, PROJECT_MENU_NEW, false)
        .separator()
        .item("Remove workspace", None, PROJECT_MENU_REMOVE, false)
        .build();
}

/// Emits the session menu: its checkout, title, read status and archive action. A chat has no branch to copy, and can also be removed for good, with its scratch folder.
#[track_caller]
fn session_menu(state: &WorkspaceState, open: SessionMenu) {
    let held = state.sessions.get(open.session);
    let on_branch = held.is_some_and(|held| held.on_branch);
    let chat = held.is_some_and(|held| held.is_chat());
    let mut menu =
        Menu::new(open.at.0, open.at.1).item("Copy path", None, SESSION_MENU_COPY_PATH, false);
    if !chat {
        menu = menu.item(
            "Copy branch name",
            None,
            SESSION_MENU_COPY_BRANCH,
            !on_branch,
        );
    }
    let menu = menu
        .item(
            if chat {
                "Rename chat"
            } else {
                "Rename session"
            },
            None,
            SESSION_MENU_RENAME,
            false,
        )
        .item(
            "Open in file manager",
            None,
            SESSION_MENU_OPEN_FOLDER,
            state.is_remote_session(open.session),
        );
    let menu = menu
        .item(
            if held.is_some_and(|held| held.unread()) {
                "Mark as read"
            } else {
                "Mark as unread"
            },
            None,
            SESSION_MENU_READ,
            false,
        )
        .separator()
        .item("Archive", None, SESSION_MENU_CLOSE, false);
    if chat {
        menu.item("Remove", None, SESSION_MENU_REMOVE, false)
            .build();
    } else {
        menu.build();
    }
}

/// Emits what the settings window, `width` logical pixels across, shows while settings are open: the title bar that closes it, the sections and the one chosen, the menu of a server and the menu of a field of the window; emits nothing while settings are closed.
#[track_caller]
pub(crate) fn settings_window(state: &WorkspaceState, width: f32) {
    if state.settings.is_none() {
        return;
    }
    Window::new()
        .body(move || {
            settings::settings(state, width);
        })
        .over(move || {
            pm_components::settings_servers::server_menu(state);
        })
        .over(move || {
            if let Some(menu) = state.menu.filter(|menu| settings::is_settings(menu.field))
                && let Some(field) = state.field(menu.field)
            {
                field_menu(field, menu);
            }
        })
        .build();
}
