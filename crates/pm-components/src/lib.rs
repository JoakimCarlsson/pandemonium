//! The components the app is built from.
//!
//! A component is a builder. It is made with `new`, given a variant for how
//! it looks and a size for how big it is, and turned into a [`pm_ui::Node`]
//! with `build`; the caller restyles the node with `.styles([...])` and never
//! reaches into it. Each component lives in a module named for what it is,
//! which holds its `Variant` and `Size`, and is re-exported here.

pub mod button;
pub mod input;
pub mod menu;
mod onboarding;

pub use button::Button;
pub use input::Input;
pub use menu::Menu;
pub use onboarding::{
    ADD, AgentStatus, BACK, BROWSE, CONTINUE, FINISH, MENU_COPY, MENU_CUT, MENU_PASTE, MENU_REDO,
    MENU_SELECT_ALL, MENU_UNDO, OnboardingState, PATH, SKIP, Step, onboarding,
};
