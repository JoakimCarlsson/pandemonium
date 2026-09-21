//! The page setup leaves behind: what was chosen, and the way back.

use pm_ui::{Div, Styled, Theme, text, v_flex};

use crate::onboarding::setup::{AGENTS, KEYMAPS, Message, Setup};

/// The page setup leaves behind: what was chosen, and the way back.
pub(super) fn ready(theme: &Theme, setup: &Setup) -> Div<Message> {
    let agents: Vec<&str> = AGENTS
        .into_iter()
        .enumerate()
        .filter(|(index, _)| setup.agents[*index])
        .map(|(_, agent)| agent)
        .collect();
    let agents = if agents.is_empty() {
        "no agents yet".to_owned()
    } else {
        agents.join(", ")
    };

    v_flex()
        .w_full()
        .gap_2()
        .p_4()
        .rounded(theme.radius.lg)
        .bg(theme.colors.surface)
        .border_1(theme.colors.border)
        .child(text("Setup finished").font_medium())
        .child(
            text(format!(
                "{} keymap · {} · {}",
                KEYMAPS[setup.keymap],
                if setup.vim_mode {
                    "vim mode on"
                } else {
                    "vim mode off"
                },
                agents,
            ))
            .text_sm()
            .color(theme.colors.text_muted),
        )
        .child(
            text("Projects and sessions land here next.")
                .text_sm()
                .color(theme.colors.text_subtle),
        )
}
