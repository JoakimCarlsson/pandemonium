//! The workspace page and its panes, screens and auxiliary windows.

mod debugging;
mod editing;
mod panes;
mod screen;
mod tasking;
mod workspace;

pub use screen::{Screen, picker_screen, screen, settings_screen};
pub use workspace::{
    ADD_PROJECT, ATTACH, CHAT_SCROLL, DIFF_SCROLL, HISTORY, MENU_SCROLL, Outgoing, PLAN_SCROLL,
    PROMPT, SEND, STOP, TAB_DIFF, TERMINAL, TERMINAL_BODY, TREE_SCROLL, WorkspaceState,
};
