//! The chat landing pane, agent choices and conversations in the selected worktree.

use pm_ui::{
    Div, IconName, IconSize, Scrolled, Styled, Theme, button, h_flex, icon, scroll_area, text,
    v_flex,
};

use crate::agent::{Talk, standing_color};
use crate::message::Message;
use crate::picker::{Choice, Row};

/// Builds the chat launcher with agents available directly in the selected worktree.
pub fn chat_pane(
    theme: &Theme,
    context: Option<String>,
    agents: &[Row],
    talks: &[&Talk],
    scroll: Scrolled,
) -> Div<Message> {
    let available = context.is_some();
    let empty = talks.is_empty();
    let introduction = v_flex()
        .w_full()
        .items_center()
        .gap(2)
        .child(
            icon(IconName::Sparkle)
                .size(IconSize::Medium)
                .color(theme.colors.text_muted),
        )
        .child(text("Start a chat").text_lg().color(theme.colors.text))
        .child(
            text(if available {
                "Choose an agent below."
            } else {
                "Open a project first."
            })
            .text_sm()
            .color(theme.colors.text_muted),
        )
        .when_some(context, |column, context| {
            column.child(text(context).text_xs().color(theme.colors.text_subtle))
        });
    let conversations = talks.iter().map(|talk| {
        v_flex()
            .w_full()
            .p(2)
            .gap(1)
            .rounded(theme.radius.md)
            .bg(theme.colors.surface)
            .hover_bg(theme.colors.surface_hover)
            .on_click(Message::ShowAgent(talk.id()))
            .child(
                text(talk.title().unwrap_or("New conversation").to_owned())
                    .text_sm()
                    .color(theme.colors.text),
            )
            .child(
                h_flex()
                    .gap(1)
                    .items_center()
                    .child(
                        text("●")
                            .text_xs()
                            .color(standing_color(theme, talk.standing())),
                    )
                    .child(
                        text(match talk.profile_name() {
                            Some(name) => format!("{} · {name}", talk.agent().name),
                            None => talk.agent().name.to_owned(),
                        })
                        .text_xs()
                        .color(theme.colors.text_muted),
                    ),
            )
    });
    v_flex()
        .w_full()
        .h_full()
        .bg(theme.colors.background)
        .child(
            scroll_area(
                scroll,
                v_flex()
                    .w_full()
                    .items_center()
                    .justify_center()
                    .p(3)
                    .child(
                        v_flex()
                            .w_full()
                            .max_w_px(480.0)
                            .gap(3)
                            .child(introduction)
                            .when(available, |column| {
                                column.child(v_flex().w_full().gap(1).children(
                                    agents.iter().map(|agent| agent_choice(theme, agent)),
                                ))
                            })
                            .child(h_flex().w_full().justify_center().child(if available {
                                button("Manage agents…", Message::ManageAgentServers)
                            } else {
                                button("Open project…", Message::OpenProject).filled()
                            }))
                            .when(!empty, |column| {
                                column.child(
                                    text("Conversations")
                                        .text_sm()
                                        .color(theme.colors.text_subtle),
                                )
                            })
                            .children(conversations),
                    ),
            )
            .w_full()
            .flex_1(),
        )
}

/// Builds one agent choice, retaining the picker's availability and installation hint.
fn agent_choice(theme: &Theme, row: &Row) -> Div<Message> {
    h_flex()
        .w_full()
        .p(2)
        .gap(2)
        .items_center()
        .rounded(theme.radius.md)
        .border_1(theme.colors.border_variant)
        .bg(theme.colors.surface)
        .tooltip(row.detail.clone())
        .child(text(row.label.clone()).text_sm().color(if row.enabled {
            theme.colors.text
        } else {
            theme.colors.text_subtle
        }))
        .child(h_flex().flex_1())
        .when(row.enabled, |choice| {
            choice
                .hover_bg(theme.colors.surface_hover)
                .child(
                    icon(IconName::ArrowRight)
                        .size(IconSize::Small)
                        .color(theme.colors.text_muted),
                )
                .when_some(
                    match row.choice {
                        Choice::Agent(agent) => Some(Message::StartAgent(agent)),
                        _ => None,
                    },
                    |choice, message| choice.on_click(message),
                )
        })
        .when(!row.enabled, |choice| {
            choice.child(
                text("Install to use")
                    .text_xs()
                    .color(theme.colors.text_subtle),
            )
        })
}
