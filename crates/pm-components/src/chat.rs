//! The conversation with an agent: its messages, a summary of what it changed and the prompt box.

use pm_icon::IconName;
use pm_input::FocusId;
use pm_style::{
    Color, Radius, Role, bg, border, border_b, border_color, cursor_pointer, flex, flex_1,
    flex_col, flex_none, font_mono, gap, h, items_center, leading_ratio, max_w_px, min_h, min_w_px,
    mt, overflow_hidden, pb, px, py, rounded, rounded_full, self_end, self_start, size, text_color,
    text_px, w_full,
};
use pm_text::Shaper;
use pm_ui::{Node, node, text};

use crate::Button;
use crate::Tally;
use crate::activity::activity;
use crate::button::Size;
use crate::composer::composer;
use crate::parts::{clipped, glyph, line, spacer, wrap};
use crate::prompt;
use crate::sample::{AGENTS, Message, Session, Status};
use crate::sidebar::status_icon;
use crate::state::{CHAT_SCROLL, COPY_BASE, OPEN_DIFF, REVIEW, TURN_BASE, WorkspaceState};
use crate::steps::{question, steps};

/// How wide the column holding the conversation is at most, in logical pixels.
const COLUMN: f32 = 760.0;

/// The sideways room the column keeps free on each side, in logical pixels.
const GUTTER: f32 = 24.0;

/// The lines of `content` wrapped to `width`, one text line each, in `tone`, with a gap where it has a blank line.
fn paragraph(content: &str, tone: Role, width: f32) -> Node {
    node()
        .styles([flex(), flex_col()])
        .children(content.lines().flat_map(|source| {
            let pieces = wrap(source, 14, width);
            if pieces.is_empty() {
                vec![node().styles([h(3)])]
            } else {
                pieces
                    .into_iter()
                    .map(|piece| line(piece, 14, 1.57, tone))
                    .collect()
            }
        }))
}

/// A message the reader wrote: a filled bubble at the right.
fn user_message(state: &WorkspaceState, message: &Message, width: f32) -> Node {
    let bubble = node()
        .styles([
            self_end(),
            px(4),
            py(2.5),
            rounded(Radius::Xl2),
            bg(Role::SurfaceHover),
        ])
        .styles([flex(), flex_col(), gap(2)]);
    let bubble = match message.attachments.is_empty() {
        true => bubble,
        false => bubble.child(crate::composer::sent_files(state, &message.attachments)),
    };
    match message.text.is_empty() {
        true => bubble,
        false => bubble.child(paragraph(&message.text, Role::Text, width * 0.85 - 32.0)),
    }
}

/// A reply from the agent: the steps it took, what it says, the points it lists and how long it took.
fn agent_message(state: &WorkspaceState, session: &Session, index: usize, width: f32) -> Node {
    let message = &session.messages[index];
    let mut reply = node().styles([flex(), flex_col(), gap(3)]);
    if !message.steps.is_empty() {
        let open = message
            .open
            .unwrap_or(session.latest_steps() == Some(index));
        let label = match message.duration.as_deref() {
            Some(took) if !took.is_empty() => took.to_owned(),
            Some(_) => "Worked".to_owned(),
            None => "Working".to_owned(),
        };
        let root = state.projects[session.project].directory();
        let root = root.to_string_lossy();
        let count = message.steps.len();
        let held = state.opened.clone();
        let page = state.open_index();
        reply = reply.child(
            node()
                .styles([flex(), flex_col(), gap(1)])
                .child(
                    node()
                        .styles([
                            flex(),
                            items_center(),
                            gap(1.5),
                            h(7),
                            cursor_pointer(),
                            self_start(),
                        ])
                        .child(glyph(
                            if open {
                                IconName::ChevronDown
                            } else {
                                IconName::ChevronRight
                            },
                            3.5,
                            Role::Subtle,
                        ))
                        .child(line(
                            format!(
                                "{label} · {count} step{}",
                                if count == 1 { "" } else { "s" }
                            ),
                            12,
                            1.4,
                            Role::Subtle,
                        ))
                        .focusable(FocusId(TURN_BASE + index as u64)),
                )
                .children(open.then(|| {
                    steps(
                        &message.steps,
                        index,
                        |position| page.is_some_and(|open| held.contains(&(open, index, position))),
                        &root,
                        width,
                    )
                })),
        );
    }
    if !message.text.is_empty() {
        reply = reply.child(paragraph(&message.text, Role::Text, width));
    }
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
    if !message.text.is_empty() {
        reply = reply.child(
            node().styles([self_start(), mt(-1.0)]).child(
                Button::icon(IconName::Copy, FocusId(COPY_BASE + index as u64))
                    .build()
                    .styles([text_color(Role::Subtle)]),
            ),
        );
    }
    if message.steps.is_empty()
        && let Some(duration) = message.duration.as_deref().filter(|took| !took.is_empty())
    {
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
        .child(
            Tally::new(session.added(), session.removed())
                .size(12)
                .build(),
        )
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
        .child(clipped(&session.title, 14, Role::Text).styles([flex_1()]));
    if !session.branch.is_empty() {
        strip = strip.child(
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
        );
    }
    let agent = AGENTS[session.agent.min(AGENTS.len() - 1)].name;
    let model = session
        .settings
        .iter()
        .find(|setting| setting.category == "model")
        .and_then(|setting| setting.current_name());
    strip = strip
        .child(
            line(
                match model {
                    Some(model) => format!("{agent} · {model}"),
                    None => agent.to_owned(),
                },
                12,
                1.4,
                Role::Subtle,
            )
            .styles([flex_none()]),
        )
        .child(spacer());
    if !session.changes.is_empty() {
        strip = strip.child(
            Button::new(
                format!(
                    "Review changes  +{} −{}",
                    session.added(),
                    session.removed()
                ),
                OPEN_DIFF,
            )
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
pub fn chat(
    state: &WorkspaceState,
    session: &Session,
    width: f32,
    above: f32,
    shaper: &mut Shaper,
) -> Node {
    let column = width.min(COLUMN) - GUTTER * 2.0;
    let mut messages = node()
        .styles([
            flex(),
            flex_col(),
            flex_none(),
            gap(6),
            w_full(),
            max_w_px(COLUMN),
            px(6),
            py(8),
        ])
        .children((0..session.messages.len()).map(|index| {
            let message = &session.messages[index];
            if message.from_user {
                user_message(state, message, column)
            } else {
                agent_message(state, session, index, column)
            }
        }));
    if let Some(ask) = &session.ask {
        messages = messages.child(question(ask));
    } else if session.status == Status::Working {
        messages = messages.child(
            node()
                .styles([flex(), items_center(), gap(2)])
                .child(status_icon(session.status))
                .child(line("Thinking…", 13, 1.4, Role::Muted)),
        );
    }
    if session.status == Status::Review && !session.changes.is_empty() {
        messages = messages.child(summary(session));
    }
    node()
        .styles([flex(), flex_col(), flex_1(), min_w_px(0), min_h(0)])
        .child(header(state, session))
        .child(
            node()
                .styles([
                    flex(),
                    flex_col(),
                    items_center(),
                    flex_1(),
                    min_h(0),
                    overflow_hidden(),
                ])
                .scroll_from_end(CHAT_SCROLL, state.chat_back)
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
                        .children(activity(state, session))
                        .child(composer(
                            state,
                            "Reply to the agent, or type / for commands",
                            Some(session),
                            2,
                            above,
                            prompt::inner_width(column),
                            shaper,
                        )),
                ),
        )
}
