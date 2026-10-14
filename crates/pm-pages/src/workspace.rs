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
    PROJECT_MENU_NEW, PROJECT_MENU_REMOVE, ProjectMenu, SESSION_MENU_CLOSE, SessionMenu,
};
use pm_components::context_menu::field_menu;
use pm_components::message_menu::message_menu;
use pm_components::parts::{glyph, line, spacer};
use pm_components::{adder, chat, diff, history, input, plan, review, sidebar, terminal, welcome};

pub use pm_components::sidebar::WIDTH as SIDEBAR_WIDTH;
pub use pm_components::state::{
    ADD_PROJECT, ATTACH, CHAT_SCROLL, DIFF_SCROLL, HISTORY, MENU_SCROLL, Outgoing, PLAN_SCROLL,
    PROMPT, SEND, STOP, TAB_DIFF, TERMINAL, TERMINAL_BODY, TREE_SCROLL, WorkspaceState,
};

use pm_components::sample::{AGENTS, Session};
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

/// The strip above a session: its views, and the button for the terminal.
fn tabs(state: &WorkspaceState, session: &Session) -> Node {
    let agent = &AGENTS[session.agent.min(AGENTS.len() - 1)];
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
            agent.name,
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
        Tab::Diff if !session.changes.is_empty() => diff::diff(state, session, width),
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
    let reviewing = open.is_some() && state.review;
    let review_width = if reviewing { review::WIDTH } else { 0.0 };
    let plan_width = if planning {
        plan::width_in(width - sidebar::WIDTH - review_width)
    } else {
        0.0
    };
    let main_width = width - sidebar::WIDTH - review_width - plan_width;
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
        (Page::History, _) => history::history(state),
        _ => welcome::welcome(
            state,
            ((height - WELCOME_HEIGHT) / 2.0 + WELCOME_ABOVE).max(0.0),
            input::inner_width((main_width - 48.0).min(680.0)),
            shaper,
        ),
    };
    let mut root = node()
        .styles([flex(), w_full(), h_full(), min_h(0), bg(Role::Background)])
        .child(sidebar::sidebar(state))
        .child(main);
    if let Some(source) = state.plan_shown() {
        root = root.child(plan::panel(state, source, plan_width, shaper));
    }
    if let Some(session) = open.filter(|_| reviewing) {
        root = root.child(review::review(state, session));
    }
    root.children(
        state
            .menu
            .and_then(|menu| state.field(menu.field).map(|field| field_menu(field, menu))),
    )
    .children(sidebar::drag_ghost(state))
    .children(state.project_menu.map(project_menu))
    .children(state.session_menu.map(session_menu))
    .children(state.message_menu.map(message_menu))
    .children(adder::adder(state))
}

/// The menu of a project: start a session in it, or take it out of the window.
fn project_menu(open: ProjectMenu) -> Node {
    Menu::new(open.at.0, open.at.1)
        .item("New session", None, PROJECT_MENU_NEW, false)
        .separator()
        .item("Remove workspace", None, PROJECT_MENU_REMOVE, false)
        .build()
}

/// The menu of a session: close it.
fn session_menu(open: SessionMenu) -> Node {
    Menu::new(open.at.0, open.at.1)
        .item("Close session", None, SESSION_MENU_CLOSE, false)
        .build()
}
