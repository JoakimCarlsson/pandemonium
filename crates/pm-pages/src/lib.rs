//! The pages of the window: what the reader sees on first launch, and the workspace they work in.

mod debugging;
mod editing;
mod onboarding;
mod panes;
mod picking;
mod screen;
mod tasking;
mod workspace;

pub use onboarding::{
    ADD, AgentStatus, BACK, BROWSE, CONTINUE, FINISH, OnboardingState, PATH, SKIP, Step,
};
pub use screen::{Screen, picker_screen, screen, settings_screen};
pub use workspace::{
    ADD_PROJECT, ATTACH, CHAT_SCROLL, DIFF_SCROLL, HISTORY, MENU_SCROLL, Outgoing, PLAN_SCROLL,
    PROMPT, SEND, STOP, TAB_DIFF, TERMINAL, TERMINAL_BODY, TREE_SCROLL, WorkspaceState,
};
