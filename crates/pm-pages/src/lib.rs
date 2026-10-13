//! The pages of the window: what the reader sees on first launch, and the workspace they work in.

mod onboarding;
mod screen;
mod workspace;

pub use onboarding::{
    ADD, AgentStatus, BACK, BROWSE, CONTINUE, FINISH, OnboardingState, PATH, SKIP, Step,
};
pub use screen::{Screen, screen};
pub use workspace::{
    ADD_PROJECT, ATTACH, CHAT_SCROLL, HISTORY, MENU_SCROLL, NEW, Outgoing, PROMPT, SEND, STOP,
    TAB_DIFF, TERMINAL, WorkspaceState,
};
