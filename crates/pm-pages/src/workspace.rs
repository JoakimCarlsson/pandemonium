//! The workspace window: the sidebar of projects and sessions beside the page the reader is on.

use pm_icon::IconName;
use pm_input::FocusId;
use pm_style::{
    Role, bg, border_b, border_color, flex, flex_1, flex_col, flex_none, font_medium, gap, h,
    h_full, items_center, items_end, mb, min_h, min_w_px, px, text_color, w_full,
};
use pm_text::Shaper;
use pm_ui::{Node, node};

use pm_components::Menu;
use pm_components::arrange::{
    PROJECT_MENU_NEW, PROJECT_MENU_REMOVE, ProjectMenu, SESSION_MENU_CLOSE,
    SESSION_MENU_COPY_BRANCH, SESSION_MENU_COPY_PATH, SESSION_MENU_OPEN_FOLDER,
    SESSION_MENU_RENAME, SessionMenu,
};
use pm_components::context_menu::{editor_menu, field_menu};
use pm_components::file_icon::file_icon;
use pm_components::git_prompts;
use pm_components::message_menu::message_menu;
use pm_components::open_files::EDITOR_FIELD;
use pm_components::open_files::{FILE_CLOSE_BASE, FILE_TAB_BASE, Viewer};
use pm_components::parts::{glyph, line, spacer};
use pm_components::{
    Button, adder, chat, diff, history, input, overview, plan, review, settings, sidebar, terminal,
    viewer, welcome,
};

pub use pm_components::state::{
    ADD_PROJECT, ATTACH, CHAT_SCROLL, DIFF_SCROLL, HISTORY, MENU_SCROLL, Outgoing, PLAN_SCROLL,
    PROMPT, SEND, STOP, TAB_DIFF, TERMINAL, TERMINAL_BODY, TREE_SCROLL, WorkspaceState,
};

use pm_components::sample::Session;
use pm_components::sash::{PLAN_SASH, REVIEW_SASH, SIDEBAR_SASH, Sash, TERMINAL_SASH};
use pm_components::state::{Page, TAB_CHAT, Tab};

/// How much of a window's height the conversation page keeps for something other than the room above the prompt box, in logical pixels.
const CHAT_CHROME: f32 = 230.0;

/// How tall the page for starting work is, in logical pixels.
const WELCOME_HEIGHT: f32 = 265.0;

/// How far down the page for starting work its prompt box starts, from the top of the page's own content, in logical pixels.
const WELCOME_ABOVE: f32 = 127.0;

/// One tab of the strip above a session: a glyph and a label, filled while it is the one shown.
fn tab(icon: IconName, label: &str, id: FocusId, shown: bool) -> Node {
    let tone = if shown { Role::Text } else { Role::Muted };
    let mut styles = vec![
        flex(),
        items_center(),
        gap(2),
        h(10),
        px(3),
        pm_style::rounded_t(8),
        pm_style::cursor_pointer(),
        text_color(tone),
    ];
    if shown {
        styles.push(bg(Role::Surface));
    } else {
        styles.push(bg(Role::SurfaceHover).hover());
    }
    node()
        .styles(styles)
        .child(glyph(icon, 3.5, tone))
        .child(line(label, 13, 1.4, tone))
        .focusable(id)
}

/// The tab of the open file at `index`: its icon and name, set slanted while the file is only being looked at, a dot while it has changes that are not saved, and a button that closes it.
fn viewer_tab(viewer: &Viewer, index: usize, shown: bool) -> Node {
    let tone = if shown { Role::Text } else { Role::Muted };
    let name = viewer.path.rsplit('/').next().unwrap_or(&viewer.path);
    let label = if viewer.head {
        format!("{name} (HEAD)")
    } else {
        name.to_owned()
    };
    let mut styles = vec![
        flex(),
        items_center(),
        gap(2),
        h(10),
        px(3),
        pm_style::rounded_t(8),
        pm_style::cursor_pointer(),
        text_color(tone),
    ];
    if shown {
        styles.push(bg(Role::Surface));
    } else {
        styles.push(bg(Role::SurfaceHover).hover());
    }
    let mut name_line = line(&label, 13, 1.4, tone);
    if viewer.preview {
        name_line = name_line.styles([pm_style::italic()]);
    }
    let mut tab = node()
        .styles(styles)
        .child(file_icon(&viewer.path, 3.5))
        .child(name_line);
    if viewer.is_dirty() {
        tab = tab.child(node().styles([
            pm_style::size(2.0),
            pm_style::rounded(pm_style::Radius::Xs),
            bg(Role::Muted),
        ]));
    }
    tab.child(
        Button::icon(
            if viewer.pinned {
                IconName::Pin
            } else {
                IconName::Close
            },
            FocusId(FILE_CLOSE_BASE + index as u64),
        )
        .size(pm_components::button::Size::Icon)
        .build(),
    )
    .focusable(FocusId(FILE_TAB_BASE + index as u64))
}

/// The strip above a session: its views, and the button for the terminal.
fn tabs(state: &WorkspaceState, session: &Session) -> Node {
    let agent = state.agent_look(session.agent);
    let mut strip = node()
        .styles([
            flex(),
            items_end(),
            gap(1),
            h(12),
            px(3),
            flex_none(),
            border_b(),
            border_color(Role::Border),
        ])
        .child(tab(
            IconName::Sparkles,
            &agent.name,
            TAB_CHAT,
            state.tab == Tab::Chat,
        ));
    if !session.changes.is_empty() {
        strip = strip.child(tab(
            IconName::GitCompare,
            "Diff",
            TAB_DIFF,
            state.tab == Tab::Diff,
        ));
    }
    for (index, viewer) in state.git.viewers.iter().enumerate() {
        let shown = state.tab == Tab::File && state.git.shown == Some(index);
        strip = strip.child(viewer_tab(viewer, index, shown));
    }
    let on = state.terminal;
    let mut toggle = vec![
        flex(),
        items_center(),
        gap(2),
        h(8),
        px(3),
        mb(1),
        pm_style::rounded(pm_style::Radius::Lg),
        pm_style::cursor_pointer(),
        bg(Role::SurfaceHover).hover(),
    ];
    if on {
        toggle.push(bg(Role::SurfaceSelected));
    }
    let tone = if on { Role::Text } else { Role::Muted };
    strip.child(spacer()).child(
        node()
            .styles(toggle)
            .child(glyph(IconName::Terminal, 3.5, tone))
            .child(line("Terminal", 13, 1.4, tone).styles([font_medium()]))
            .focusable(TERMINAL),
    )
}

/// What fills the main area on a page of a project while a file of the project is open: the tabs, the first of them, `first`, showing `page`, and the file under the others.
fn home_page(state: &WorkspaceState, first: (IconName, &str), page: Node, height: f32) -> Node {
    let mut strip = node()
        .styles([
            flex(),
            items_end(),
            gap(1),
            h(12),
            px(3),
            flex_none(),
            border_b(),
            border_color(Role::Border),
        ])
        .child(tab(first.0, first.1, TAB_CHAT, state.tab != Tab::File));
    for (index, viewer) in state.git.viewers.iter().enumerate() {
        let shown = state.tab == Tab::File && state.git.shown == Some(index);
        strip = strip.child(viewer_tab(viewer, index, shown));
    }
    node()
        .styles([flex(), flex_col(), flex_1(), min_w_px(0), h_full()])
        .child(strip)
        .child(
            if state.tab == Tab::File && state.shown_viewer().is_some() {
                viewer::viewer(state, width, height, shaper)
            } else {
                page
            },
        )
}

/// The page for starting work, its prompt box `width` logical pixels across in a window `height` tall.
fn starting_page(state: &WorkspaceState, width: f32, height: f32, shaper: &mut Shaper) -> Node {
    welcome::welcome(
        state,
        ((height - WELCOME_HEIGHT) / 2.0 + WELCOME_ABOVE).max(0.0),
        input::inner_width((width - 48.0).min(680.0)),
        shaper,
    )
}

/// What fills the main area while a session is open.
fn session_page(
    state: &WorkspaceState,
    session: &Session,
    width: f32,
    above: f32,
    height: f32,
    shaper: &mut Shaper,
) -> Node {
    let view = match state.tab {
        Tab::Diff if !session.changes.is_empty() => diff::diff(state, session, width, height),
        Tab::File if state.shown_viewer().is_some() => viewer::viewer(state, width, height, shaper),
        _ => chat::chat(state, session, width, above, shaper),
    };
    let mut page = node()
        .styles([flex(), flex_col(), flex_1(), min_w_px(0), h_full()])
        .child(tabs(state, session))
        .child(view);
    if state.terminal {
        page = page.child(terminal::terminal(state, session, width, height, shaper));
    }
    page
}

/// The whole workspace screen for `state` in a window `width` logical pixels wide.
pub fn workspace(state: &WorkspaceState, width: f32, height: f32, shaper: &mut Shaper) -> Node {
    let open = state.open();
    let planning = state.plan_shown().is_some();
    let reviewing = state.review_session().is_some() && state.review;
    let sidebar_width = state.sidebar_width;
    let review_width = if reviewing { state.review_width } else { 0.0 };
    let plan_width = if planning {
        plan::width_in(state, width - sidebar_width - review_width)
    } else {
        0.0
    };
    let main_width = width - sidebar_width - review_width - plan_width;
    let main = match (state.page, open) {
        _ if state.projects.is_empty() => welcome::no_project(),
        (Page::Session(_), Some(session)) => session_page(
            state,
            session,
            main_width,
            (height - CHAT_CHROME).max(0.0),
            height,
            shaper,
        ),
        (Page::New(_), _) if !state.git.viewers.is_empty() => home_page(
            state,
            (IconName::Plus, "New Session"),
            starting_page(state, main_width, height, shaper),
            height,
        ),
        (Page::Project(project), _) if !state.git.viewers.is_empty() => home_page(
            state,
            (IconName::Folder, "Overview"),
            overview::overview(state, project),
            height,
        ),
        (Page::Project(project), _) => overview::overview(state, project),
        (Page::History, _) => history::history(state),
        (Page::Extensions, _) => pm_components::extensions::extensions(state),
        _ => starting_page(state, main_width, height, shaper),
    };
    let mut root = node()
        .styles([flex(), w_full(), h_full(), min_h(0), bg(Role::Background)])
        .child(sidebar::sidebar(state))
        .child(main);
    if let Some(source) = state.plan_shown() {
        root = root.child(plan::panel(state, source, plan_width, shaper));
    }
    if let Some(session) = state.review_session().filter(|_| reviewing) {
        root = root.child(review::review(state, session, shaper));
    }
    root = root.child(
        Sash::column(SIDEBAR_SASH)
            .at((sidebar_width, 0.0), height)
            .build(),
    );
    if planning {
        root = root.child(
            Sash::column(PLAN_SASH)
                .at((sidebar_width + main_width, 0.0), height)
                .build(),
        );
    }
    if reviewing {
        root = root.child(
            Sash::column(REVIEW_SASH)
                .at((width - review_width, 0.0), height)
                .build(),
        );
    }
    if state.terminal
        && !state.projects.is_empty()
        && matches!((state.page, open), (Page::Session(_), Some(_)))
    {
        root = root.child(
            Sash::row(TERMINAL_SASH)
                .at(
                    (sidebar_width, height - terminal::height(state, height)),
                    main_width,
                )
                .build(),
        );
    }
    root.children(state.menu.and_then(|menu| {
        if settings::is_settings(menu.field) {
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
    .children(state.session_menu.map(|open| session_menu(state, open)))
    .children(state.message_menu.map(message_menu))
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

/// The menu of a session: copy where it works, rename it, show its folder, or archive it.
fn session_menu(state: &WorkspaceState, open: SessionMenu) -> Node {
    let on_branch = state
        .sessions
        .get(open.session)
        .is_some_and(|held| held.on_branch);
    Menu::new(open.at.0, open.at.1)
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
        )
        .item("Archive", None, SESSION_MENU_CLOSE, false)
        .build()
}

/// What the settings window, `width` logical pixels across, shows while settings are open: the sections and the one chosen, the menu of a server and the menu of a field of the window.
pub(crate) fn settings_window(state: &WorkspaceState, width: f32) -> Option<Node> {
    Some(
        settings::settings(state, width)?
            .children(pm_components::settings_servers::server_menu(state))
            .children(
                state
                    .menu
                    .filter(|menu| settings::is_settings(menu.field))
                    .and_then(|menu| Some(field_menu(state.field(menu.field)?, menu))),
            ),
    )
}
