//! The workspace page and its panes, screens and auxiliary windows.

mod debugging;
mod editing;
mod panes;
mod screen;
mod tasking;
mod workspace;

pub use screen::{Screen, picker_screen, screen, settings_screen, settings_window};
pub use workspace::{Outgoing, WorkspaceState};
