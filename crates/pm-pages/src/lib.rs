//! The workspace page and its panes, screens and auxiliary windows.

mod debugging;
mod editing;
mod panes;
mod screen;
mod tasking;
mod workspace;

pub use screen::{Screen, picker_screen, screen, settings_screen, settings_window};
pub use workspace::{
    ADD_PROJECT, ATTACH, HISTORY, Outgoing, PROMPT, SEND, STOP, TAB_DIFF, TERMINAL, TERMINAL_BODY,
    WorkspaceState,
};
