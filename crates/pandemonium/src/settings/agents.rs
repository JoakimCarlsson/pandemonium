//! The top of the Agents page: what can be set about agents, a card for each.
//!
//! The page is laid out after VS Code's customizations overview: a line on
//! what the page is for, and under it a card for every group of settings
//! the page has, each saying what it is for and how much is set in it.
//! Pressing a card opens its section.

use pm_ui::{Div, Styled, Theme, h_flex, text, v_flex};

use crate::message::Message;
use crate::settings::mcp::McpPage;
use crate::settings::state::SettingsSection;

/// Builds the overview of the Agents page.
pub fn overview(theme: &Theme, mcp: &McpPage<'_>) -> Div<Message> {
    let servers = mcp.servers.len();
    v_flex()
        .w_full()
        .gap(4)
        .child(
            text("Tailor how agents work in your projects: the tools they are given, and what they are told.")
                .color(theme.colors.text_muted),
        )
        .child(
            v_flex()
                .w_full()
                .gap(2)
                .child(text("Explore").text_lg().font_semibold())
                .child(card(
                    theme,
                    SettingsSection::McpServers,
                    "Connect agents to external tools and data through MCP servers. Manage the servers available to your agents.",
                    &format!("{servers} installed"),
                )),
        )
}

/// One card: what the section is called and for, how much is set in it, and
/// the press that opens it.
fn card(theme: &Theme, section: SettingsSection, description: &str, count: &str) -> Div<Message> {
    v_flex()
        .w_full()
        .p(3)
        .gap(1)
        .rounded(theme.radius.md)
        .border_1(theme.colors.border_variant)
        .bg(theme.colors.surface)
        .hover_bg(theme.colors.surface_hover)
        .on_click(Message::ShowSettingsSection(section))
        .child(
            h_flex()
                .w_full()
                .items_center()
                .justify_between()
                .child(text(section.label()).font_medium())
                .child(
                    text(count.to_owned())
                        .text_xs()
                        .color(theme.colors.text_muted),
                ),
        )
        .child(
            text(description.to_owned())
                .text_sm()
                .color(theme.colors.text_muted),
        )
}
