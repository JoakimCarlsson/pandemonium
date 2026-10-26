//! The workspace window: the sidebar of projects and sessions beside the page the reader is on.

use pm_style::{Role, bg, flex, flex_1, flex_col, h_full, min_h, w_full};
use pm_text::Shaper;
use pm_ui::{Node, node};

use pm_components::arrange::{
    PROJECT_MENU_NEW, PROJECT_MENU_REMOVE, ProjectMenu, SESSION_MENU_CLOSE,
    SESSION_MENU_COPY_BRANCH, SESSION_MENU_COPY_PATH, SESSION_MENU_OPEN_FOLDER, SESSION_MENU_READ,
    SESSION_MENU_RENAME, SESSION_MENU_REVIEW_HOOKS, SessionMenu,
};
use pm_components::context_menu::{editor_menu, field_menu};
use pm_components::git_prompts;
use pm_components::message_menu::message_menu;
use pm_components::open_files::EDITOR_FIELD;
use pm_components::title_bar::TITLE_BAR_HEIGHT;
use pm_components::{Menu, TitleBar, Window};
use pm_components::{adder, history, plan, review, settings, sidebar, terminal, welcome};

pub use pm_components::state::{
    ADD_PROJECT, ATTACH, CHAT_SCROLL, DIFF_SCROLL, HISTORY, MENU_SCROLL, Outgoing, PLAN_SCROLL,
    PROMPT, SEND, STOP, TAB_DIFF, TERMINAL, TERMINAL_BODY, TREE_SCROLL, WorkspaceState,
};

use pm_components::sash::{PLAN_SASH, REVIEW_SASH, SIDEBAR_SASH, Sash, TERMINAL_SASH};
use pm_components::state::Page;

use crate::panes;

/// The whole workspace screen for `state` in a window `width` logical pixels wide and `height` tall: the title bar over the sidebar and the page.
pub fn workspace(state: &WorkspaceState, width: f32, height: f32, shaper: &mut Shaper) -> Node {
    let page = height - TITLE_BAR_HEIGHT;
    let planning = state.plan_shown().is_some();
    let reviewing = state.review_session().is_some() && state.review;
    let sidebar_width = state.sidebar_room();
    let review_width = if reviewing { state.review_width } else { 0.0 };
    let main_width = state.pane_area((width, height)).width;
    let plan_width = width - sidebar_width - review_width - main_width;
    let mut over = Vec::new();
    let main = match state.page {
        _ if state.projects.is_empty() => welcome::no_project(),
        _ if state.paned() => {
            let (main, laid_over) = panes::main_area(state, (width, height), shaper);
            over = laid_over;
            main
        }
        Page::History => history::history(state),
        Page::Extensions => pm_components::extensions::extensions(state),
        _ => panes::starting_page(state, main_width, page, shaper),
    };
    let mut body = node()
        .styles([flex(), flex_1(), w_full(), min_h(0)])
        .children(state.sidebar_open.then(|| sidebar::sidebar(state)))
        .child(main);
    if let Some(source) = state.plan_shown() {
        body = body.child(plan::panel(state, source, plan_width, shaper));
    }
    if let Some(session) = state.review_session().filter(|_| reviewing) {
        body = body.child(review::review(state, session, page, shaper));
    }
    let mut root = node()
        .styles([
            flex(),
            flex_col(),
            w_full(),
            h_full(),
            min_h(0),
            bg(Role::Background),
        ])
        .child(TitleBar::of(state).build())
        .child(body);
    if state.sidebar_open {
        root = root.child(
            Sash::column(SIDEBAR_SASH)
                .at((sidebar_width, TITLE_BAR_HEIGHT), page)
                .build(),
        );
    }
    if planning {
        root = root.child(
            Sash::column(PLAN_SASH)
                .at((sidebar_width + main_width, TITLE_BAR_HEIGHT), page)
                .build(),
        );
    }
    if reviewing {
        root = root.child(
            Sash::column(REVIEW_SASH)
                .at((width - review_width, TITLE_BAR_HEIGHT), page)
                .build(),
        );
    }
    if state.docking() {
        root = root.child(
            Sash::row(TERMINAL_SASH)
                .at(
                    (sidebar_width, height - terminal::height(state, page)),
                    main_width,
                )
                .build(),
        );
    }
    root.children(over)
        .children(state.menu.and_then(|menu| {
            if settings::is_settings(menu.field)
                || pm_components::folder_picker::is_picker(menu.field)
            {
                return None;
            }
            let field = state.field(menu.field)?;
            Some(if menu.field == EDITOR_FIELD {
                editor_menu(
                    field,
                    menu,
                    state.coding(),
                    state.review_is_local(),
                    &state.editor.keymap(),
                )
            } else {
                field_menu(field, menu)
            })
        }))
        .children(sidebar::drag_ghost(state))
        .children(state.project_menu.map(project_menu))
        .children(state.pane_menu.map(panes::pane_menu))
        .children(state.session_menu.map(|open| session_menu(state, open)))
        .children(state.message_menu.map(message_menu))
        .children(
            state.terminal_menu.map(|open| {
                pm_components::terminal_menu::terminal_menu(open, &state.editor.keymap())
            }),
        )
        .children(adder::adder(state))
        .children(state.git_menu_node().filter(|_| reviewing))
        .children(state.tab_menu.and_then(|open| {
            state.git.viewers.get(open.index).map(|viewer| {
                pm_components::tab_menu::tab_menu(
                    viewer,
                    open,
                    state.git.viewers.len(),
                    state.review_is_local(),
                )
            })
        }))
        .children(
            state
                .git
                .prompt
                .as_ref()
                .map(|open| git_prompts::prompt(state, open)),
        )
        .children(pm_components::assist::assist(state, height, shaper))
}

/// The menu of a project: start a session in it, or take it out of the window.
fn project_menu(open: ProjectMenu) -> Node {
    Menu::new(open.at.0, open.at.1)
        .item("New session", None, PROJECT_MENU_NEW, false)
        .separator()
        .item("Remove workspace", None, PROJECT_MENU_REMOVE, false)
        .build()
}

/// The session menu: its checkout, title, hooks, read status and archive action.
fn session_menu(state: &WorkspaceState, open: SessionMenu) -> Node {
    let held = state.sessions.get(open.session);
    let on_branch = held.is_some_and(|held| held.on_branch);
    let hooks_held = held.is_some_and(|held| !held.held_hooks.is_empty());
    let menu = Menu::new(open.at.0, open.at.1)
        .item("Copy path", None, SESSION_MENU_COPY_PATH, false)
        .item(
            "Copy branch name",
            None,
            SESSION_MENU_COPY_BRANCH,
            !on_branch,
        )
        .item("Rename session", None, SESSION_MENU_RENAME, false)
        .item(
            "Open in file manager",
            None,
            SESSION_MENU_OPEN_FOLDER,
            state.is_remote_session(open.session),
        );
    let menu = match hooks_held {
        true => menu.item("Review hooks", None, SESSION_MENU_REVIEW_HOOKS, false),
        false => menu,
    };
    menu.item(
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
    .item("Archive", None, SESSION_MENU_CLOSE, false)
    .build()
}

/// What the settings window, `width` logical pixels across, shows while settings are open: the title bar that closes it, the sections and the one chosen, the menu of a server and the menu of a field of the window.
pub(crate) fn settings_window(state: &WorkspaceState, width: f32) -> Option<Node> {
    let page = settings::settings(state, width)?;
    Some(
        Window::new()
            .body(page)
            .over(pm_components::settings_servers::server_menu(state))
            .over(
                state
                    .menu
                    .filter(|menu| settings::is_settings(menu.field))
                    .and_then(|menu| Some(field_menu(state.field(menu.field)?, menu))),
            )
            .build(),
    )
}
