//! The chat landing pane and conversations in the selected worktree.

use pm_ui::{Div, Scrolled, Styled, Theme, button, h_flex, scroll_area, text, v_flex};

use crate::agent::{Talk, standing_color};
use crate::message::Message;

/// Builds the chat launcher, with an empty state before any conversation exists.
pub fn chat_pane(
    theme: &Theme,
    context: Option<String>,
    talks: &[&Talk],
    scroll: Scrolled,
) -> Div<Message> {
    let available = context.is_some();
    let empty = talks.is_empty();
    let introduction = v_flex()
        .w_full()
        .gap(2)
        .child(text("Chat").text_lg().color(theme.colors.text))
        .when_some(context, |column, context| {
            column.child(text(context).text_sm().color(theme.colors.text_muted))
        })
        .when(empty, |column| {
            column.child(
                text(if available {
                    "Start a conversation with an agent to work on this project."
                } else {
                    "Open a project to start a conversation with an agent."
                })
                .text_sm()
                .color(theme.colors.text_muted),
            )
        })
        .child(
            h_flex().child(
                button(
                    if available {
                        "New conversation"
                    } else {
                        "Open project…"
                    },
                    if available {
                        Message::NewAgentSession
                    } else {
                        Message::OpenProject
                    },
                )
                .filled(),
            ),
        );
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
                        text(talk.agent().name.to_owned())
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
                v_flex().w_full().items_center().p(3).child(
                    v_flex()
                        .w_full()
                        .max_w_px(760.0)
                        .gap(3)
                        .child(introduction)
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
