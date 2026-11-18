//! The conversation with an agent: its messages and the prompt box.

use rustle::{BoxAlignment, CursorIcon, Modifier};

use crate::Button;
use crate::Markdown;
use crate::activity::activity;
use crate::button::Variant;
use crate::composer::composer;
use crate::exhibit::{
    EXHIBIT_COPY_BASE, EXHIBIT_OPEN_BASE, ExhibitCard, PREVIEW_OFFER_BASE, exhibit_button,
};
use crate::input;
use crate::inquiry::{answered, inquiry};
use crate::kit::{
    Flex, FocusId, Hovered, IconName, Interactive, Radius, Role, Scroll, Side as Edge, Styled,
    Tooltip, Type, block, column, gap_box, label, row, scroll_area, selectable, sp, wrap_lines,
};
use crate::parts::{clipped, glyph, line};
use crate::queued::queued;
use crate::sample::{Session, Side, Status};
use crate::sidebar::status_icon;
use crate::size::Size;
use crate::state::{
    CHAT_SCROLL, CHAT_TEXT, COPY_BASE, DETAILS_BASE, FORK_BASE, KEEP_FILES, LINK_BASE,
    LINKS_PER_PIECE, LINKS_PER_REPLY, RESTORE_BASE, RESTORE_FILES, REWIND_BASE, TURN_BASE,
    TURN_CHANGES_BASE, WorkspaceState,
};
use crate::steps::{question, steps};

mod retained;

/// How wide the column holding the conversation is at most, in logical pixels.
const COLUMN: f32 = 760.0;

/// How many fold-outs one piece of a reply may have before the next piece's numbering begins.
const FOLDS_PER_PIECE: usize = 100;

/// The sideways room the column keeps free on each side, in logical pixels.
const GUTTER: f32 = 24.0;

/// Emits the lines of `content` wrapped to `width` as conversation text, in `tone`, each a piece of the
/// selectable conversation text, with a gap where it has a blank line.
#[track_caller]
fn paragraph(content: &str, tone: Role, width: f32) {
    let kind = Type::new(14).ratio(1.57).tone(tone);
    let lines = wrap_lines(content, kind, width.max(32.0));
    column(Modifier::new(), Flex::new(), || {
        for part in lines {
            if part.end == part.start {
                gap_box(Modifier::new().h(3));
            } else {
                selectable(
                    Modifier::new(),
                    CHAT_TEXT,
                    &content[part.start..part.end],
                    kind,
                );
            }
        }
    });
}

/// Emits one line of `content` in `pixels` tall type in `tone` as a piece of the selectable conversation
/// text, cut short at the edge of the box it sits in.
#[track_caller]
fn chat_line(content: impl Into<String>, pixels: u32, tone: Role) {
    selectable(
        Modifier::new(),
        CHAT_TEXT,
        content,
        Type::new(pixels).ratio(1.4).tone(tone).truncate(),
    );
}

/// Emits a button that shows under a message while the pointer is over it or it has focus, showing `icon`, saying what it does with `tooltip` and answering to `id`, switched off while `disabled`.
#[track_caller]
fn message_action(icon: IconName, tooltip: &str, id: u64, disabled: bool) {
    Button::icon(icon, FocusId(id))
        .disabled(disabled)
        .tone(Role::Subtle)
        .modifier(
            Modifier::new()
                .tooltip(tooltip)
                .shown_on_group_hover(Some(FocusId(id))),
        )
        .build();
}

/// Emits the offer to put the files back as they were before a message of the reader's, in place of the buttons under it.
#[track_caller]
fn restore_offer() {
    row(
        Modifier::new().placed(BoxAlignment::BottomEnd),
        Flex::new().center().gap(2),
        || {
            chat_line("Restore files to before this turn?", 12, Role::Muted);
            Button::new("Restore", RESTORE_FILES)
                .variant(Variant::Destructive)
                .size(Size::Sm)
                .build();
            Button::new("Cancel", KEEP_FILES)
                .variant(Variant::Ghost)
                .size(Size::Sm)
                .build();
        },
    );
}

/// Emits a message the reader wrote, at `index` of `session`: a filled bubble at the right, with buttons under it to show what the turn changed, to put the files back as they were before it and to write it again.
#[track_caller]
fn user_message(state: &WorkspaceState, session: &Session, index: usize, width: f32) {
    let message = &session.messages[index];
    let working = session.status == Status::Working;
    let restoring = state.restoring == Some(index);
    let (typed, elements) = crate::elements::split(&message.text);
    column(
        Modifier::new().group().selectable_area(CHAT_TEXT),
        Flex::new().gap(1),
        || {
            column(
                Modifier::new()
                    .placed(BoxAlignment::BottomEnd)
                    .fill_rounded(Role::SurfaceHover, Radius::Xl2)
                    .pad_x(4)
                    .pad_y(2.5),
                Flex::new().gap(2),
                || {
                    if !message.attachments.is_empty() {
                        crate::composer::sent_files(state, &message.attachments);
                    }
                    if !elements.is_empty() {
                        crate::elements::sent_chips(&elements);
                    }
                    if !typed.is_empty() {
                        paragraph(typed, Role::Text, width * 0.85 - 32.0);
                    }
                },
            );
            if restoring {
                restore_offer();
            } else {
                row(
                    Modifier::new().placed(BoxAlignment::BottomEnd),
                    Flex::new().center().gap(1),
                    || {
                        if message.checkpoint {
                            message_action(
                                IconName::GitCompare,
                                "Show what this turn changed",
                                TURN_CHANGES_BASE + index as u64,
                                false,
                            );
                            message_action(
                                IconName::History,
                                "Restore files to before this turn",
                                RESTORE_BASE + index as u64,
                                working,
                            );
                        }
                        message_action(
                            IconName::Undo,
                            "Rewind conversation to before this message · edit and resend",
                            REWIND_BASE + index as u64,
                            working,
                        );
                    },
                );
            }
        },
    );
}

/// Emits a note beside the conversation that says `content`: an answer to a question asked on the side, framed apart from the turns since it is not part of the conversation, or the mark where the conversation was handed over to a new one, with the prompt the new one starts with.
#[track_caller]
fn side_note(side: &Side, content: &str, width: f32) {
    let (icon, heading, asked) = match side {
        Side::Aside { question, failed } => (
            IconName::Message,
            if *failed {
                "Asked on the side · no answer"
            } else {
                "Asked on the side · not part of the conversation"
            },
            Some(question.as_str()).filter(|question| !question.is_empty()),
        ),
        Side::HandedOff => (
            IconName::ArrowRight,
            "Handed over to a new conversation, which starts with",
            None,
        ),
        Side::Fallback { .. } => (IconName::Zap, "Switched to a backup model", None),
    };
    let inner = width - 34.0;
    column(
        Modifier::new()
            .group()
            .selectable_area(CHAT_TEXT)
            .stroke(Role::Border, Radius::Xl)
            .pad_x(4)
            .pad_y(3),
        Flex::new().gap(2),
        || {
            row(Modifier::new(), Flex::new().center().gap(2), || {
                glyph(icon, 3.5, Role::Subtle);
                chat_line(heading, 12, Role::Subtle);
            });
            if let Some(question) = asked {
                paragraph(question, Role::Muted, inner);
            }
            match side {
                Side::Aside { failed: true, .. } => paragraph(content, Role::Danger, inner),
                Side::Aside { .. } => Markdown::new(content, inner).build(),
                Side::HandedOff | Side::Fallback { .. } => paragraph(content, Role::Muted, inner),
            }
        },
    );
}

/// Emits the card of `exhibit`, which the agent showed as the one numbered `number` of the message at `index`, `width` logical pixels wide, with the buttons that open it in the browser pane and copy its source, and the one that installs or turns on the embedded browser while its picture waits for it.
#[track_caller]
fn exhibit_card(
    state: &WorkspaceState,
    exhibit: &pm_harness::Exhibit,
    (index, number): (usize, usize),
    width: f32,
) {
    let offer = state.preview_offer();
    ExhibitCard::new(exhibit, &state.figures, width)
        .open(exhibit_button(EXHIBIT_OPEN_BASE, index, number))
        .copy(exhibit_button(EXHIBIT_COPY_BASE, index, number))
        .offer(&offer, exhibit_button(PREVIEW_OFFER_BASE, index, number))
        .build();
}

/// Emits a reply from the agent: the steps it took, what it says, the points it lists and how long it took; a note beside the conversation shows as one.
#[track_caller]
fn agent_message(state: &WorkspaceState, session: &Session, index: usize, width: f32) {
    let message = &session.messages[index];
    if let Some(Side::Fallback { from, to, .. }) = &message.side {
        crate::fallback::fallback_notice(from, to, &message.text);
        return;
    }
    if let Some(side) = &message.side {
        side_note(side, &message.text, width);
        return;
    }
    let open = !message.steps.is_empty()
        && message
            .open
            .unwrap_or(session.latest_steps() == Some(index));
    let root = state.root_of(session);
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
        column(Modifier::new(), Flex::new().gap(1), || {
            row(
                Modifier::new()
                    .placed(BoxAlignment::TopStart)
                    .h(7)
                    .cursor(CursorIcon::Pointer)
                    .target(FocusId(TURN_BASE + index as u64)),
                Flex::new().center().gap(1.5),
                || {
                    glyph(
                        if open {
                            IconName::ChevronDown
                        } else {
                            IconName::ChevronRight
                        },
                        3.5,
                        Role::Subtle,
                    );
                    line(
                        format!(
                            "{label} · {count} step{}",
                            if count == 1 { "" } else { "s" }
                        ),
                        12,
                        1.4,
                        Role::Subtle,
                    );
                },
            );
            if open {
                steps(
                    &message.steps,
                    from..to,
                    index,
                    |position| page.is_some_and(|open| held.contains(&(open, index, position))),
                    &root,
                    width,
                );
            }
        });
    };
    column(
        Modifier::new().group().selectable_area(CHAT_TEXT),
        Flex::new().gap(3),
        || {
            let mut shown = 0;
            let mut recorded = 0;
            let mut exhibited = 0;
            for (number, (before, piece)) in message.chunks().into_iter().enumerate() {
                let before = before.min(message.steps.len());
                while let Some((at, exhibit)) = message
                    .exhibits
                    .get(exhibited)
                    .filter(|(at, _)| *at <= before)
                {
                    let at = (*at).min(message.steps.len());
                    if at > shown {
                        run(shown, at);
                        shown = at;
                    }
                    exhibit_card(state, exhibit, (index, exhibited), width);
                    exhibited += 1;
                }
                if before > shown {
                    run(shown, before);
                }
                shown = shown.max(before);
                while let Some((_, answers)) = message
                    .answers
                    .get(recorded)
                    .filter(|(at, _)| *at <= before)
                {
                    answered(answers, width);
                    recorded += 1;
                }
                let base = number * FOLDS_PER_PIECE;
                let unfolded = state
                    .unfolded_in(index)
                    .into_iter()
                    .filter(|place| (base..base + FOLDS_PER_PIECE).contains(place))
                    .map(|place| place - base)
                    .collect();
                Markdown::new(piece, width)
                    .details(DETAILS_BASE + (index * 1000 + base) as u64, unfolded)
                    .links(
                        LINK_BASE
                            + index as u64 * LINKS_PER_REPLY
                            + number as u64 * LINKS_PER_PIECE,
                    )
                    .figures(&state.figures, None)
                    .build();
            }
            for (number, (at, exhibit)) in message.exhibits.iter().enumerate().skip(exhibited) {
                let at = (*at).min(message.steps.len());
                if at > shown {
                    run(shown, at);
                    shown = at;
                }
                exhibit_card(state, exhibit, (index, number), width);
            }
            if shown < message.steps.len() {
                run(shown, message.steps.len());
            }
            for (_, answers) in &message.answers[recorded..] {
                answered(answers, width);
            }
            if !message.bullets.is_empty() {
                column(Modifier::new(), Flex::new().gap(1.5), || {
                    for bullet in &message.bullets {
                        row(Modifier::new(), Flex::new().gap(2.5), || {
                            gap_box(
                                Modifier::new()
                                    .pad_side(Edge::Top, 2.5)
                                    .square(1)
                                    .fill_rounded(Role::Subtle, Radius::Full),
                            );
                            paragraph(bullet, Role::Muted, width - 14.0);
                        });
                    }
                });
            }
            let copy = !message.text.is_empty();
            let writing = session.status == Status::Working && index + 1 == session.messages.len();
            let fork = session.turn_of(index) > 0;
            if copy || fork {
                row(
                    Modifier::new()
                        .placed(BoxAlignment::TopStart)
                        .offset(0.0, -sp(1.0)),
                    Flex::new().center().gap(1),
                    || {
                        if copy {
                            message_action(IconName::Copy, "Copy", COPY_BASE + index as u64, false);
                        }
                        if fork {
                            message_action(
                                IconName::GitFork,
                                "Branch from this reply · new session · shared current files",
                                FORK_BASE + index as u64,
                                writing,
                            );
                        }
                    },
                );
            }
            if message.steps.is_empty()
                && let Some(duration) = message.duration.as_deref().filter(|took| !took.is_empty())
            {
                row(Modifier::new(), Flex::new().center().gap(2), || {
                    glyph(IconName::Progress, 3.5, Role::Subtle);
                    chat_line(duration, 12, Role::Subtle);
                });
            }
        },
    );
}

/// Emits the strip above the conversation: its status, title, branch, agent and what it has cost.
#[track_caller]
fn header(state: &WorkspaceState, session: &Session) {
    let mut agent = state.agent_look(session.agent).name;
    if let Some(account) = state.account_as(session.agent, session.login.as_deref()) {
        agent = format!("{agent} ({account})");
    }
    let model = session
        .settings
        .iter()
        .find(|setting| setting.category == "model")
        .and_then(|setting| setting.current_name());
    let who = match model {
        Some(model) => format!("{agent} · {model}"),
        None => agent.clone(),
    };
    row(
        Modifier::new()
            .h(10)
            .stroke_side(Edge::Bottom, 1.0, Role::Border)
            .pad_x(5),
        Flex::new().center().gap(3),
        || {
            status_icon(session.shown());
            clipped(&session.title, 14, Role::Text);
            if !session.branch.is_empty() {
                row(
                    Modifier::new()
                        .fill_rounded(Role::Surface, Radius::Md)
                        .pad_x(2),
                    Flex::new().center().gap(1),
                    || {
                        glyph(IconName::GitFork, 3.0, Role::Muted);
                        label(
                            &session.branch,
                            Type::new(11).ratio(1.6).mono().tone(Role::Muted),
                        );
                    },
                );
            }
            clipped(who, 12, Role::Subtle);
            crate::spend::cost_of(session);
        },
    );
}

/// Whether the frame just drawn left messages far back unbuilt for a later frame, so another frame is wanted; asking clears the answer.
pub fn history_pending() -> bool {
    retained::pending()
}

/// Emits the conversation of `session` in a column `width` logical pixels wide, with the prompt box below.
#[track_caller]
pub fn chat(state: &WorkspaceState, session: &Session, width: f32, above: f32) {
    let text_width = width.min(COLUMN) - GUTTER * 2.0;
    column(Modifier::new().grow(), Flex::new(), || {
        header(state, session);
        crate::pull_request_bar::bar(session);
        scroll_area(
            Modifier::new().grow(),
            Scroll::from_end(CHAT_SCROLL, state.chat_back),
            || {
                column(
                    Modifier::new().fill_max_width(),
                    Flex::new().center(),
                    || {
                        messages(state, session, text_width);
                    },
                );
            },
        );
        column(
            Modifier::new().fill_max_width().pad_side(Edge::Bottom, 5),
            Flex::new().center(),
            || {
                block(
                    Modifier::new().max_w_px(COLUMN).fill_max_width().pad_x(6),
                    || {
                        queued(state, session, text_width);
                        if let Some(draft) = session.inquiry.as_ref() {
                            block(
                                Modifier::new().pad_side(Edge::Bottom, 2).fill_max_width(),
                                || {
                                    inquiry(
                                        draft,
                                        &state.figures,
                                        &state.preview_offer(),
                                        text_width,
                                    )
                                },
                            );
                        }
                        activity(state, session, text_width);
                        composer(
                            state,
                            "Reply to the agent, or type / for commands",
                            Some(session),
                            2,
                            above,
                            input::inner_width(text_width),
                        );
                    },
                );
            },
        );
    });
}

/// Emits the column of the conversation's messages, `width` logical pixels of text wide, then the request to
/// trust its folder, the question the agent asks, or what it is doing while it works.
#[track_caller]
fn messages(state: &WorkspaceState, session: &Session, width: f32) {
    column(
        Modifier::new()
            .max_w_px(COLUMN)
            .fill_max_width()
            .pad_x(6)
            .pad_y(8),
        Flex::new().gap(6),
        || {
            retained::messages(state, session, width);
            if let Some(request) = session.trust.as_ref() {
                crate::trust::card(request, width);
            }
            let asking = session.ask.as_ref();
            if let Some(ask) = asking.filter(|ask| ask.plan.is_none()) {
                question(ask, width);
            } else if asking.is_none()
                && session.inquiry.is_none()
                && session.status == Status::Working
            {
                let label = if session.out_of_reach {
                    "The server is out of reach. Waiting for the connection to come back…"
                } else if let Some(step) = &session.container {
                    step.as_str()
                } else if session.echoes > 0 {
                    "Sending prompt…"
                } else {
                    "Thinking…"
                };
                row(Modifier::new(), Flex::new().center().gap(2), || {
                    status_icon(session.shown());
                    line(label, 13, 1.4, Role::Muted);
                });
            }
        },
    );
}
