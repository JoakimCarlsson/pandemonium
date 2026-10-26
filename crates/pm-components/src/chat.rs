//! The conversation with an agent: its messages and the prompt box.

use pm_icon::IconName;
use pm_input::FocusId;
use pm_style::{
    Radius, Role, bg, border_b, border_color, cursor_pointer, flex, flex_1, flex_col, flex_none,
    font_mono, gap, h, items_center, leading_ratio, max_w_px, mb, min_h, min_w_px, mt, opacity,
    overflow_hidden, pb, px, py, rounded, rounded_full, self_end, self_start, size, text_color,
    text_px, w_full,
};
use pm_text::Shaper;
use pm_ui::{Node, node, text};

use crate::Button;
use crate::Markdown;
use crate::activity::activity;
use crate::button::Variant;
use crate::composer::composer;
use crate::input;
use crate::inquiry::{answered, inquiry};
use crate::parts::{clipped, glyph, line, wrap};
use crate::queued::queued;
use crate::sample::{Session, Status};
use crate::sidebar::status_icon;
use crate::size::Size;
use crate::state::{
    CHAT_SCROLL, CHAT_TEXT, COPY_BASE, DETAILS_BASE, FORK_BASE, KEEP_FILES, LINK_BASE,
    LINKS_PER_PIECE, LINKS_PER_REPLY, RESTORE_BASE, RESTORE_FILES, REWIND_BASE, TURN_BASE,
    WorkspaceState,
};
use crate::steps::{question, steps};

mod retained;

/// How wide the column holding the conversation is at most, in logical pixels.
const COLUMN: f32 = 760.0;

/// How many fold-outs one piece of a reply may have before the next piece's numbering begins.
const FOLDS_PER_PIECE: usize = 100;

/// The sideways room the column keeps free on each side, in logical pixels.
const GUTTER: f32 = 24.0;

/// The lines of `content` wrapped to `width`, one text line each, in `tone`, with a gap where it has a blank line.
///
/// A line the wrapping broke keeps the space it was broken at, so that text
/// selected across it is copied as one line.
fn paragraph(content: &str, tone: Role, width: f32) -> Node {
    node()
        .styles([flex(), flex_col()])
        .children(content.lines().flat_map(|source| {
            let pieces = wrap(source, 14, width);
            if pieces.is_empty() {
                return vec![node().styles([h(3)])];
            }
            let last = pieces.len() - 1;
            pieces
                .into_iter()
                .enumerate()
                .map(|(at, piece)| match at == last {
                    true => line(piece, 14, 1.57, tone),
                    false => line(piece + " ", 14, 1.57, tone),
                })
                .collect()
        }))
}

/// A button that shows under a message while the pointer is over it or it has focus, showing `icon`, saying what it does with `tooltip` and answering to `id`, switched off while `disabled`.
fn message_action(icon: IconName, tooltip: &str, id: u64, disabled: bool) -> Node {
    Button::icon(icon, FocusId(id))
        .disabled(disabled)
        .build()
        .tooltip(tooltip)
        .styles([
            text_color(Role::Subtle),
            opacity(0),
            opacity(100).group_hover(),
            opacity(100).focus(),
        ])
}

/// The offer to put the files back as they were before a message of the reader's, in place of the buttons under it.
fn restore_offer() -> Node {
    node()
        .styles([self_end(), flex(), items_center(), gap(2)])
        .child(line(
            "Restore files to before this turn?",
            12,
            1.4,
            Role::Muted,
        ))
        .child(
            Button::new("Restore", RESTORE_FILES)
                .variant(Variant::Destructive)
                .size(Size::Sm)
                .build(),
        )
        .child(
            Button::new("Cancel", KEEP_FILES)
                .variant(Variant::Ghost)
                .size(Size::Sm)
                .build(),
        )
}

/// A message the reader wrote, at `index` of `session`: a filled bubble at the right, with buttons under it to write it again and to put the files back as they were before it.
fn user_message(state: &WorkspaceState, session: &Session, index: usize, width: f32) -> Node {
    let message = &session.messages[index];
    let working = session.status == Status::Working;
    let actions = match state.restoring == Some(index) {
        true => restore_offer(),
        false => node()
            .styles([self_end(), flex(), items_center(), gap(1)])
            .children(message.checkpoint.then(|| {
                message_action(
                    IconName::History,
                    "Restore files to before this turn",
                    RESTORE_BASE + index as u64,
                    working,
                )
            }))
            .child(message_action(
                IconName::Undo,
                "Rewind conversation to before this message · edit and resend",
                REWIND_BASE + index as u64,
                working,
            )),
    };
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
    let (typed, elements) = crate::elements::split(&message.text);
    let bubble = match elements.is_empty() {
        true => bubble,
        false => bubble.child(crate::elements::sent_chips(&elements)),
    };
    let bubble = match typed.is_empty() {
        true => bubble,
        false => bubble.child(paragraph(typed, Role::Text, width * 0.85 - 32.0)),
    };
    node()
        .styles([flex(), flex_col(), gap(1)])
        .child(bubble)
        .child(actions)
        .group()
        .selectable(CHAT_TEXT)
}

/// A reply from the agent: the steps it took, what it says, the points it lists and how long it took.
fn agent_message(
    state: &WorkspaceState,
    session: &Session,
    index: usize,
    width: f32,
    shaper: &mut Shaper,
) -> Node {
    let message = &session.messages[index];
    let mut reply = node().styles([flex(), flex_col(), gap(3)]);
    let open = !message.steps.is_empty()
        && message
            .open
            .unwrap_or(session.latest_steps() == Some(index));
    let root = state.projects[session.project].directory();
    let root = root.to_string_lossy();
    let held = state.opened.clone();
    let page = state.open_index();
    let run = |from: usize, to: usize| {
        let label = match message.duration.as_deref() {
            _ if to < message.steps.len() => "Worked".to_owned(),
            Some(took) if !took.is_empty() => took.to_owned(),
            Some(_) => "Worked".to_owned(),
            None => "Working".to_owned(),
        };
        let count = (from..to)
            .filter(|position| !crate::steps::is_inside_card(&message.steps, *position))
            .count();
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
                    from..to,
                    index,
                    |position| page.is_some_and(|open| held.contains(&(open, index, position))),
                    &root,
                    width,
                )
            }))
    };
    let mut shown = 0;
    let mut recorded = 0;
    for (number, (before, piece)) in message.chunks().into_iter().enumerate() {
        let before = before.min(message.steps.len());
        if before > shown {
            reply = reply.child(run(shown, before));
        }
        shown = shown.max(before);
        while let Some((_, answers)) = message
            .answers
            .get(recorded)
            .filter(|(at, _)| *at <= before)
        {
            reply = reply.child(answered(answers));
            recorded += 1;
        }
        let base = number * FOLDS_PER_PIECE;
        let unfolded = state
            .unfolded_in(index)
            .into_iter()
            .filter(|place| (base..base + FOLDS_PER_PIECE).contains(place))
            .map(|place| place - base)
            .collect();
        reply = reply.child(
            Markdown::new(piece, width)
                .details(DETAILS_BASE + (index * 1000 + base) as u64, unfolded)
                .links(LINK_BASE + index as u64 * LINKS_PER_REPLY + number as u64 * LINKS_PER_PIECE)
                .figures(&state.figures, None)
                .build(shaper),
        );
    }
    if shown < message.steps.len() {
        reply = reply.child(run(shown, message.steps.len()));
    }
    for (_, answers) in &message.answers[recorded..] {
        reply = reply.child(answered(answers));
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
    let copy = (!message.text.is_empty())
        .then(|| message_action(IconName::Copy, "Copy", COPY_BASE + index as u64, false));
    let writing = session.status == Status::Working && index + 1 == session.messages.len();
    let fork = (session.turn_of(index) > 0).then(|| {
        message_action(
            IconName::GitFork,
            "Branch from this reply · new session · shared current files",
            FORK_BASE + index as u64,
            writing,
        )
    });
    if copy.is_some() || fork.is_some() {
        reply = reply.child(
            node()
                .styles([self_start(), flex(), items_center(), gap(1), mt(-1.0)])
                .children(copy)
                .children(fork),
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
    reply.group().selectable(CHAT_TEXT)
}

/// The strip above the conversation: its status, title, branch and agent.
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
        .child(status_icon(session.shown()))
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
    let mut agent = state.agent_look(session.agent).name;
    if let Some(account) = state.account_as(session.agent, session.login.as_deref()) {
        agent = format!("{agent} ({account})");
    }
    let model = session
        .settings
        .iter()
        .find(|setting| setting.category == "model")
        .and_then(|setting| setting.current_name());
    strip = strip.child(clipped(
        match model {
            Some(model) => format!("{agent} · {model}"),
            None => agent.clone(),
        },
        12,
        Role::Subtle,
    ));
    strip
}

/// Whether the frame just drawn left messages far back unbuilt for a later frame, so another frame is wanted; asking clears the answer.
pub fn history_pending() -> bool {
    retained::pending()
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
        .children(retained::messages(state, session, column, shaper));
    let asking = session.ask.as_ref();
    if let Some(ask) = asking.filter(|ask| ask.plan.is_none()) {
        messages = messages.child(question(ask, column, shaper));
    } else if asking.is_none() && session.inquiry.is_none() && session.status == Status::Working {
        let label = if session.echoes > 0 {
            "Sending prompt…"
        } else {
            "Thinking…"
        };
        messages = messages.child(
            node()
                .styles([flex(), items_center(), gap(2)])
                .child(status_icon(session.shown()))
                .child(line(label, 13, 1.4, Role::Muted)),
        );
    }
    messages = messages.children(crate::hooks_card::hooks_card(session, column));
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
                        .children(queued(session))
                        .children(session.inquiry.as_ref().map(|draft| {
                            node()
                                .styles([w_full(), mb(2)])
                                .child(inquiry(draft, column, shaper))
                        }))
                        .children(activity(state, session, column, shaper))
                        .child(composer(
                            state,
                            "Reply to the agent, or type / for commands",
                            Some(session),
                            2,
                            above,
                            input::inner_width(column),
                            shaper,
                        )),
                ),
        )
}
