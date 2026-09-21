//! One agent card: the agent's name and whether it is installed.

use pm_ui::{Div, Styled, Theme, button, text, v_flex};

use crate::onboarding::setup::Message;

/// One agent card: the agent's name and whether it is installed.
pub(super) fn agent_card(
    theme: &Theme,
    agent: &str,
    installed: bool,
    index: usize,
) -> Div<Message> {
    let action = if installed {
        button("Installed", Message::ToggleAgent(index))
            .ghost()
            .w_full()
    } else {
        button("Install", Message::ToggleAgent(index))
            .outlined()
            .w_full()
    };

    v_flex()
        .flex_1()
        .gap_2()
        .p_3()
        .rounded(theme.radius.lg)
        .bg(theme.colors.surface)
        .border_1(if installed {
            theme.colors.border_selected
        } else {
            theme.colors.border
        })
        .child(text(agent).text_sm().font_medium())
        .child(action)
}
