//! The workspace window: the sidebar of projects and sessions beside the page the reader is on.

use pm_components::areas::{BrowserEvent, MenuEvent, TerminalEvent, WindowEvent};
use pm_components::focus::request_focus_on;
use pm_components::target::Target;
use pm_components::theme;
use pm_components::theme::Role;
use rustle::Modifier;
use rustle::{Alignment, Arrangement, column_arranged, row_arranged};

use pm_components::arrange::{ProjectMenu, SessionMenu};
use pm_components::context_menu::{editor_menu, field_menu};
use pm_components::git_prompts;
use pm_components::message_menu::message_menu;
use pm_components::title_bar::TITLE_BAR_HEIGHT;
use pm_components::{Menu, TitleBar, Window};
use pm_components::{adder, history, plan, review, settings, sidebar};

pub use pm_components::state::{Outgoing, WorkspaceState};

use crate::observed::{follow_chrome, quiet_region, region};
use pm_components::sash::Sash;
use pm_components::state::Page;

use crate::panes;

/// Gives the keyboard, from effects the screen holds, to what was asked to take it: the
/// shell each time a press asked for it and the browser's address each time the browser
/// was opened or brought forward, as a Compose screen runs
/// `LaunchedEffect(asked) { focusRequester.requestFocus() }`. Held by the screen, a shell or
/// an address that shows again without being asked for keeps out of it. The prompt is
/// not asked for here: typing with nothing focused hands it the keyboard.
#[track_caller]
fn keyboard_asks(state: &WorkspaceState) {
    if state.shell_asked > 0 {
        request_focus_on(
            Target::Terminal(TerminalEvent::TerminalBody),
            &state.shell_asked,
        );
    }
    if let Some(asked) = state
        .browser
        .as_ref()
        .map(|browser| browser.asked)
        .filter(|asked| *asked > 0)
    {
        request_focus_on(Target::Browser(BrowserEvent::BrowserAddress), &asked);
    }
}

/// Emits the whole workspace screen for `state` in a window `width` logical pixels wide and `height` tall: the title bar over a row of the sidebar, the page taking the room the panels leave, the plan and the review panel, each panel as wide as it was dragged to with its handle lying in the row on its edge, and the menus and the dialogs laid over the window.
#[track_caller]
pub fn workspace(state: &WorkspaceState, width: f32, height: f32) {
    let page = height - TITLE_BAR_HEIGHT;
    let reviewing = state.review_session().is_some() && state.review;
    let sidebar_width = state.sidebar_room();
    let review_width = if reviewing {
        state.sizes.review_width.get()
    } else {
        0.0
    };
    let plan_width = state.plan_room(width);
    let paned = state.paned();
    let inner = state.inner_sizes();
    let scales = (theme::step_scale(), theme::text_scale());
    follow_chrome();
    let frame = state.code_lines.frame(Modifier::new().fill_max_size());
    keyboard_asks(state);
    rustle::stack_with(frame, || {
        state.places.provide(|| {
            column_arranged(
                Modifier::new()
                    .fill_max_size()
                    .background(theme::color(Role::Background)),
                Arrangement::Start,
                Alignment::Start,
                || {
                    let bar = TitleBar::of(state);
                    rustle::kept((bar.clone(), width, height, inner, scales), move || {
                        bar.build()
                    });
                    row_arranged(
                        Modifier::new().weight(1.0).fill_max_width(),
                        Arrangement::Start,
                        Alignment::Start,
                        || {
                            if state.sidebar_open {
                                let view = sidebar::SidebarState::of(state);
                                rustle::kept((view.clone(), scales), move || {
                                    sidebar::sidebar(&view);
                                });
                                Sash::column(Target::Window(WindowEvent::SidebarSash))
                                    .between()
                                    .build();
                            }
                            let rooms = (
                                width,
                                height,
                                sidebar_width,
                                review_width,
                                plan_width,
                                inner,
                            );
                            match state.page {
                                _ if paned => panes::main_area(state, (width, height)),
                                Page::History => {
                                    let view = history::HistoryState::of(state);
                                    rustle::kept((view.clone(), scales), move || {
                                        history::history(&view);
                                    });
                                }
                                Page::Extensions => {
                                    let view =
                                        pm_components::extensions::ExtensionsState::of(state);
                                    rustle::kept((view.clone(), scales), move || {
                                        pm_components::extensions::extensions(&view);
                                    });
                                }
                                _ => region(rooms, || panes::starting_page(state, page)),
                            }
                            if let Some(source) = state.plan_shown() {
                                Sash::column(Target::Window(WindowEvent::PlanSash))
                                    .between()
                                    .build();
                                let scroll = state.plan_scroll.clone();
                                let source = source.to_owned();
                                rustle::kept(
                                    (source.clone(), scroll.clone(), plan_width, height, scales),
                                    move || plan::panel(&scroll, &source, plan_width),
                                );
                            }
                            if let Some(session) = state.review_session().filter(|_| reviewing) {
                                Sash::column(Target::Window(WindowEvent::ReviewSash))
                                    .between()
                                    .build();
                                quiet_region((review_width, height, inner), || {
                                    review::review(state, session)
                                });
                            }
                        },
                    );
                },
            );
            if paned {
                panes::main_over(state);
            }
            over(state, height, reviewing);
        })
    });
}

/// Emits what is laid over the workspace window of `height` logical pixels for `state`: the open menus, drag ghosts, the project adder, the git menus and prompts while `reviewing`, and the dialogs.
#[track_caller]
fn over(state: &WorkspaceState, height: f32, reviewing: bool) {
    if let Some(menu) = state.menu
        && !settings::is_settings(menu.field)
        && !pm_components::folder_picker::is_picker(menu.field)
    {
        if let Some(field) = state.field(menu.field) {
            editor_menu(
                field,
                menu,
                state.coding(),
                state.review_is_local(),
                &state.editor.keymap(),
            );
        } else if let Some(field) = state.text_field(menu.field) {
            field_menu(field, menu);
        }
    }
    sidebar::drag_ghost(state);
    pm_components::file_tree::drag_ghost(state);
    if let Some(open) = state.project_menu {
        project_menu(open);
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
        pm_components::browser_menu::browser_menu(
            open,
            state
                .browser
                .as_ref()
                .and_then(|browser| browser.chrome.menu.as_ref()),
        );
    }
    if let Some(open) = state.browser_tab_menu {
        pm_components::browser_tab_menu::browser_tab_menu(state, open);
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
    pm_components::bug_report::report_dialog(state);
    pm_components::assist::assist(state, height);
}

/// Emits the menu of a project opened at a point; one that hangs from the project's button is
/// emitted there.
#[track_caller]
fn project_menu(open: ProjectMenu) {
    if let Some((x, y)) = open.at {
        pm_components::arrange::project_menu(Menu::new(x, y).offset(-8.0, 12.0)).build();
    }
}

/// Emits the session menu: its checkout, title, read status and archive action. A chat has no branch to copy, and can also be removed for good, with its scratch folder.
#[track_caller]
fn session_menu(state: &WorkspaceState, open: SessionMenu) {
    let held = state.sessions.get(open.session);
    let on_branch = held.is_some_and(|held| held.on_branch);
    let chat = held.is_some_and(|held| held.is_chat());
    let mut menu = Menu::new(open.at.0, open.at.1).offset(-8.0, 12.0).item(
        "Copy path",
        None,
        Target::Menu(MenuEvent::SessionMenuCopyPath),
        false,
    );
    if !chat {
        menu = menu.item(
            "Copy branch name",
            None,
            Target::Menu(MenuEvent::SessionMenuCopyBranch),
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
            Target::Menu(MenuEvent::SessionMenuRename),
            false,
        )
        .item(
            "Open in file manager",
            None,
            Target::Menu(MenuEvent::SessionMenuOpenFolder),
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
            Target::Menu(MenuEvent::SessionMenuRead),
            false,
        )
        .separator()
        .item(
            "Archive",
            None,
            Target::Menu(MenuEvent::SessionMenuClose),
            false,
        );
    if chat {
        menu.item(
            "Remove",
            None,
            Target::Menu(MenuEvent::SessionMenuRemove),
            false,
        )
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
            if let Some(menu) = state.menu.filter(|menu| settings::is_settings(menu.field))
                && let Some(field) = state.text_field(menu.field)
            {
                field_menu(field, menu);
            }
        })
        .build();
}
