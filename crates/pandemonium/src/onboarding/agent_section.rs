//! The agent grid: one card per agent CLI a session can run.

use pm_ui::{Div, Styled, Theme, h_flex};

use crate::onboarding::agent_card::agent_card;
use crate::onboarding::section::section;
use crate::onboarding::setup::{AGENTS, Message, Setup};

/// The agent grid: one card per agent CLI a session can run.
pub(super) fn agent_section(theme: &Theme, setup: &Setup) -> Div<Message> {
    let cards = AGENTS
        .into_iter()
        .enumerate()
        .map(|(index, agent)| agent_card(theme, agent, setup.agents[index], index));

    section(
        theme,
        "Agent Setup",
        Some("Install the agents your sessions will be conducted with"),
        h_flex().w_full().gap_2().children(cards),
    )
}
