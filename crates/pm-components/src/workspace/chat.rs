//! The conversation with an agent: its messages, a summary of what it changed and the prompt box.

use pm_icon::IconName;
use pm_style::{
    Color, Radius, Role, bg, border, border_b, border_color, cursor_pointer, flex, flex_1,
    flex_col, flex_none, font_mono, gap, h, items_center, justify_end, leading_ratio, max_w_px,
    min_h, min_w_px, mt, overflow_hidden, pb, px, py, rounded, rounded_full, self_end, size,
    text_color, text_px, w_full,
};
use pm_ui::{Node, node, text};

use super::composer::composer;
use super::sample::{AGENTS, Message, Session};
use super::sidebar::status_icon;
use super::state::{OPEN_DIFF, REVIEW, WorkspaceState};
use crate::Button;
use crate::button::Size;
use crate::parts::{clipped, glyph, line, spacer, wrap};

/// How wide the column holding the conversation is at most, in logical pixels.
const COLUMN: f32 = 760.0;

/// The sideways room the column keeps free on each side, in logical pixels.
const GUTTER: f32 = 24.0;

/// `+added −removed`, the first in the success colour and the second in the danger colour.
pub(super) fn tally(added: usize, removed: usize, pixels: u32) -> Node {
    node()
        .styles([flex(), items_center(), gap(1.5), flex_none()])
        .child(text(format!("+{added}")).styles([
            text_px(pixels),
            leading_ratio(1.4),
            font_mono(),
            text_color(Role::Success),
        ]))
        .child(text(format!("−{removed}")).styles([
            text_px(pixels),
            leading_ratio(1.4),
            font_mono(),
            text_color(Role::Danger),
        ]))
}

/// The lines of `content` wrapped to `width`, one text line each, in `tone`.
fn paragraph(content: &str, tone: Role, width: f32) -> Node {
    node().styles([flex(), flex_col()]).children(
        wrap(content, 14, width)
            .into_iter()
            .map(|piece| line(piece, 14, 1.57, tone)),
    )
}

/// A message the reader wrote: a filled bubble at the right.
fn user_message(message: &Message, width: f32) -> Node {
    node()
        .styles([
            self_end(),
            px(4),
            py(2.5),
            rounded(Radius::Xl2),
            bg(Role::SurfaceHover),
        ])
        .child(paragraph(&message.text, Role::Text, width * 0.85 - 32.0))
}

/// A reply from the agent: how long it took, what it says and the points it lists.
fn agent_message(message: &Message, width: f32) -> Node {
    let mut reply = node().styles([flex(), flex_col(), gap(3)]).child(paragraph(
        &message.text,
        Role::Text,
        width,
    ));
    if !message.bullets.is_empty() {
        reply = reply.child(node().styles([flex(), flex_col(), gap(1.5)]).children(
            message.bullets.iter().map(|bullet| {
                node()
                    .styles([flex(), gap(2.5)])
                    .child(node().styles([
                        size(1),
                        mt(2.5),
                        flex_none(),
                        rounded_full(),
                        bg(Role::Subtle),
                    ]))
                    .child(paragraph(bullet, Role::Muted, width - 14.0))
            }),
        ));
    }
    if let Some(duration) = &message.duration {
        reply = reply.child(
            node()
                .styles([flex(), items_center(), gap(2)])
                .child(glyph(IconName::Progress, 3.5, Role::Subtle))
                .child(line(duration, 12, 1.4, Role::Subtle)),
        );
    }
    reply
}

/// The card that sums up the changes and opens them for review.
fn summary(session: &Session) -> Node {
    let count = session.changes.len();
    node()
        .styles([
            flex(),
            items_center(),
            gap(3),
            px(4),
            py(3),
            rounded(Radius::Xl),
            border(),
            border_color(Role::Border),
            bg(Role::Surface),
            bg(Role::SurfaceHover).hover(),
            cursor_pointer(),
        ])
        .child(glyph(IconName::GitCompare, 4.0, Role::Accent))
        .child(node().styles([flex_1()]).child(line(
            format!("{count} file{} changed", if count == 1 { "" } else { "s" }),
            13,
            1.4,
            Role::Text,
        )))
        .child(tally(session.added(), session.removed(), 12))
        .child(line("Review", 12, 1.4, Role::Accent))
        .focusable(OPEN_DIFF)
}

/// The strip above the conversation: where the session stands, what it is and how to review it.
fn header(state: &WorkspaceState, session: &Session) -> Node {
    let mut strip = node()
        .styles([
            flex(),
            items_center(),
            gap(3),
            h(10),
            px(5),
            flex_none(),
            border_b(),
            border_color(Role::Border),
        ])
        .child(status_icon(session.status))
        .child(clipped(&session.title, 14, Role::Text).styles([flex_1()]))
        .child(
            node()
                .styles([
                    flex(),
                    items_center(),
                    gap(1),
                    px(2),
                    rounded(Radius::Md),
                    bg(Role::Surface),
                    flex_none(),
                ])
                .child(glyph(IconName::GitFork, 3.0, Role::Muted))
                .child(text(&session.branch).styles([
                    text_px(11),
                    leading_ratio(1.6),
                    font_mono(),
                    text_color(Role::Muted),
                ])),
        )
        .child(spacer());
    if !session.changes.is_empty() {
        strip = strip.child(
            Button::new("Review changes", OPEN_DIFF)
                .size(Size::Sm)
                .build(),
        );
    }
    strip.child(
        Button::icon(IconName::PanelRight, REVIEW)
            .build()
            .styles([text_color(if state.review {
                Color::from(Role::Text)
            } else {
                Color::from(Role::Muted)
            })]),
    )
}

/// The conversation of `session` in a column `width` logical pixels wide, with the prompt box below.
pub(super) fn chat(state: &WorkspaceState, session: &Session, width: f32) -> Node {
    let column = width.min(COLUMN) - GUTTER * 2.0;
    let mut messages = node()
        .styles([
            flex(),
            flex_col(),
            gap(6),
            w_full(),
            max_w_px(COLUMN),
            px(6),
            py(8),
        ])
        .children(session.messages.iter().map(|message| {
            if message.from_user {
                user_message(message, column)
            } else {
                agent_message(message, column)
            }
        }));
    if session.status == super::sample::Status::Working
        && session
            .messages
            .last()
            .is_some_and(|message| message.from_user)
    {
        messages = messages.child(
            node()
                .styles([flex(), items_center(), gap(2)])
                .child(status_icon(session.status))
                .child(line("Thinking…", 13, 1.4, Role::Muted)),
        );
    }
    if session.status == super::sample::Status::Review && !session.changes.is_empty() {
        messages = messages.child(summary(session));
    }
    let agent = &AGENTS[session.agent.min(AGENTS.len() - 1)];
    node()
        .styles([flex(), flex_col(), flex_1(), min_w_px(0), min_h(0)])
        .child(header(state, session))
        .child(
            node()
                .styles([
                    flex(),
                    flex_col(),
                    items_center(),
                    justify_end(),
                    flex_1(),
                    min_h(0),
                    overflow_hidden(),
                ])
                .child(messages),
        )
        .child(
            node()
                .styles([
                    flex(),
                    flex_col(),
                    items_center(),
                    w_full(),
                    flex_none(),
                    pb(5),
                ])
                .child(
                    node()
                        .styles([w_full(), max_w_px(COLUMN), px(6)])
                        .child(composer(
                            state,
                            "Reply to the agent, or type / for commands",
                            agent,
                        )),
                ),
        )
}
