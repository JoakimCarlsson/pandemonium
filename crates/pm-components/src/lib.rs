//! The components the app is built from.
//!
//! A component is a builder. It is made with `new`, given a variant for how
//! it looks and a size for how big it is, and turned into a [`pm_ui::Node`]
//! with `build`; the caller restyles the node with `.styles([...])` and never
//! reaches into it. Each component lives in a module named for what it is,
//! which holds its `Variant` and `Size`, and is re-exported here.

pub mod avatar;
pub mod button;
mod context_menu;
pub mod input;
pub mod menu;
mod onboarding;
mod parts;
mod project;
mod screen;
mod workspace;

pub use avatar::Avatar;
pub use button::Button;
pub use context_menu::{
    ContextMenu, MENU_COPY, MENU_CUT, MENU_PASTE, MENU_REDO, MENU_SELECT_ALL, MENU_UNDO,
    is_menu_item,
};
pub use input::Input;
pub use menu::Menu;
pub use onboarding::{
    ADD, AgentStatus, BACK, BROWSE, CONTINUE, FINISH, OnboardingState, PATH, SKIP, Step,
};
pub use project::Project;
pub use screen::{Screen, screen};
pub use workspace::{ADD_PROJECT, HISTORY, NEW, TAB_DIFF, TERMINAL, WorkspaceState};
