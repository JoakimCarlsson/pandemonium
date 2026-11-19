//! The conversation with an agent: its messages and the prompt box.

use crate::events::reduce;
use crate::theme;
use rustle::{Alignment, Arrangement, column_arranged, row_arranged};
use rustle::{BoxAlignment, CursorIcon, Modifier};
use rustle::{Icon, phosphor};

use crate::Button;
use crate::Markdown;
use crate::activity::activity;
use crate::button::Variant;
use crate::composer::composer;
use crate::events::Interactive;
use crate::exhibit::{
    EXHIBIT_COPY_BASE, EXHIBIT_OPEN_BASE, ExhibitCard, PREVIEW_OFFER_BASE, exhibit_button,
};
use crate::focus::FocusId;
use crate::inquiry::{answered, inquiry};
use crate::parts::{clipped, glyph, line};
use crate::queued::queued;
use crate::sample::{Session, Side, Status};
use crate::sidebar::status_icon;
use crate::size::Size;
use crate::state::{
    COPY_BASE, DETAILS_BASE, FORK_BASE, KEEP_FILES, LINK_BASE, LINKS_PER_PIECE, LINKS_PER_REPLY,
    RESTORE_BASE, RESTORE_FILES, REWIND_BASE, TURN_BASE, TURN_CHANGES_BASE, WorkspaceState,
};
use crate::steps::{question, steps};
use crate::theme::{Radius, Role, sp};

/// How wide the column holding the conversation is at most, in logical pixels.
const COLUMN: f32 = 760.0;

/// How many fold-outs one piece of a reply may have before the next piece's numbering begins.
const FOLDS_PER_PIECE: usize = 100;

/// The sideways room the column keeps free on each side, in logical pixels.
const GUTTER: f32 = 24.0;

/// Emits `content` as conversation text in `tone`, wrapping at `width`: each run of lines
/// between blank lines one text, with a gap where a blank line was.
#[track_caller]
fn paragraph(content: &str, tone: Role, width: f32) {
    let kind = theme::text_style(14, 1.57).color(theme::color(tone));
    let room = width.max(32.0);
    let lines = rustle::wrap_at_spaces(content, &kind, room);
    column_arranged(
        Modifier::new(),
        Arrangement::Start,
        Alignment::Start,
        || {
            for run in lines.split(|part| part.end == part.start) {
                match (run.first(), run.last()) {
                    (Some(first), Some(last)) => rustle::text_styled(
                        Modifier::new().width_in(0.0, room),
                        content[first.start..last.end].trim_end_matches('\n'),
                        kind,
                    ),
                    _ => rustle::spacer(Modifier::new().height(theme::sp(3))),
                }
            }
        },
    );
}

/// Emits one line of `content` in `pixels` tall type in `tone`, cut short at the edge of the box it
/// sits in.
#[track_caller]
fn chat_line(content: impl Into<String>, pixels: u32, tone: Role) {
    rustle::text_styled(
        Modifier::new(),
        content,
        theme::text_style(pixels, 1.4)
            .color(theme::color(tone))
            .max_lines(1),
    );
}

/// Emits a button that shows under a message while the pointer is over it or it has focus, showing `icon`, saying what it does with `tooltip` and answering to `id`, switched off while `disabled`.
#[track_caller]
fn message_action(icon: Icon, tooltip: &str, id: u64, disabled: bool) {
    Button::icon(icon, FocusId(id))
        .disabled(disabled)
        .tone(Role::Subtle)
        .modifier(
            Modifier::new()
                .interaction_focused(crate::focus::is_held(FocusId(id)))
                .draw_with_content_interactive(shown_on_group_hover),
        )
        .tooltip(tooltip)
        .build();
}

/// Emits the offer to put the files back as they were before a message of the reader's, in place of the buttons under it.
#[track_caller]
fn restore_offer() {
    row_arranged(
        Modifier::new().align(BoxAlignment::BottomEnd),
        Arrangement::SpacedBy(sp(2)),
        Alignment::Center,
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
    column_arranged(
        Modifier::new()
            .fill_max_width()
            .interaction_group()
            .message_area(index),
        Arrangement::SpacedBy(sp(1)),
        Alignment::Start,
        || {
            column_arranged(
                Modifier::new()
                    .align(BoxAlignment::BottomEnd)
                    .background_rounded(theme::color(Role::SurfaceHover), Radius::Xl2.px())
                    .padding_xy(theme::sp(4), 0.0)
                    .padding_xy(0.0, theme::sp(2.5)),
                Arrangement::SpacedBy(sp(2)),
                Alignment::Start,
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
                row_arranged(
                    Modifier::new().align(BoxAlignment::BottomEnd),
                    Arrangement::SpacedBy(sp(1)),
                    Alignment::Center,
                    || {
                        if message.checkpoint {
                            message_action(
                                phosphor::GIT_DIFF,
                                "Show what this turn changed",
                                TURN_CHANGES_BASE + index as u64,
                                false,
                            );
                            message_action(
                                phosphor::CLOCK_COUNTER_CLOCKWISE,
                                "Restore files to before this turn",
                                RESTORE_BASE + index as u64,
                                working,
                            );
                        }
                        message_action(
                            phosphor::ARROW_COUNTER_CLOCKWISE,
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

/// Emits a note beside the conversation, message `index`, that says `content`: an answer to a question asked on the side, framed apart from the turns since it is not part of the conversation, or the mark where the conversation was handed over to a new one, with the prompt the new one starts with.
#[track_caller]
fn side_note(side: &Side, content: &str, index: usize, width: f32) {
    let (icon, heading, asked) = match side {
        Side::Aside { question, failed } => (
            phosphor::CHAT_CIRCLE,
            if *failed {
                "Asked on the side · no answer"
            } else {
                "Asked on the side · not part of the conversation"
            },
            Some(question.as_str()).filter(|question| !question.is_empty()),
        ),
        Side::HandedOff => (
            phosphor::ARROW_RIGHT,
            "Handed over to a new conversation, which starts with",
            None,
        ),
        Side::Fallback { .. } => (phosphor::LIGHTNING, "Switched to a backup model", None),
    };
    let inner = width - 34.0;
    column_arranged(
        Modifier::new()
            .fill_max_width()
            .interaction_group()
            .message_area(index)
            .border(1.0, theme::color(Role::Border), Radius::Xl.px())
            .padding(1.0)
            .padding_xy(theme::sp(4), 0.0)
            .padding_xy(0.0, theme::sp(3)),
        Arrangement::SpacedBy(sp(2)),
        Alignment::Start,
        || {
            row_arranged(
                Modifier::new(),
                Arrangement::SpacedBy(sp(2)),
                Alignment::Center,
                || {
                    glyph(icon, 3.5, Role::Subtle);
                    chat_line(heading, 12, Role::Subtle);
                },
            );
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
        side_note(side, &message.text, index, width);
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
        column_arranged(
            Modifier::new(),
            Arrangement::SpacedBy(sp(1)),
            Alignment::Start,
            || {
                row_arranged(
                    Modifier::new()
                        .align(BoxAlignment::TopStart)
                        .height(theme::sp(7))
                        .cursor(CursorIcon::Pointer)
                        .control(
                            FocusId(TURN_BASE + index as u64),
                            reduce(FocusId(TURN_BASE + index as u64)),
                        ),
                    Arrangement::SpacedBy(sp(1.5)),
                    Alignment::Center,
                    || {
                        glyph(
                            if open {
                                phosphor::CARET_DOWN
                            } else {
                                phosphor::CARET_RIGHT
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
            },
        );
    };
    column_arranged(
        Modifier::new()
            .fill_max_width()
            .interaction_group()
            .message_area(index),
        Arrangement::SpacedBy(sp(3)),
        Alignment::Start,
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
                column_arranged(
                    Modifier::new(),
                    Arrangement::SpacedBy(sp(1.5)),
                    Alignment::Start,
                    || {
                        for bullet in &message.bullets {
                            row_arranged(
                                Modifier::new(),
                                Arrangement::SpacedBy(sp(2.5)),
                                Alignment::Start,
                                || {
                                    rustle::spacer(
                                        Modifier::new()
                                            .padding_each(0.0, theme::sp(2.5), 0.0, 0.0)
                                            .size(theme::sp(1), theme::sp(1))
                                            .background_rounded(
                                                theme::color(Role::Subtle),
                                                Radius::Full.px(),
                                            ),
                                    );
                                    paragraph(bullet, Role::Muted, width - 14.0);
                                },
                            );
                        }
                    },
                );
            }
            let copy = !message.text.is_empty();
            let writing = session.status == Status::Working && index + 1 == session.messages.len();
            let fork = session.turn_of(index) > 0;
            if copy || fork {
                row_arranged(
                    Modifier::new().align(BoxAlignment::TopStart).padding_each(
                        0.0,
                        -sp(1.0),
                        0.0,
                        0.0,
                    ),
                    Arrangement::SpacedBy(sp(1)),
                    Alignment::Center,
                    || {
                        if copy {
                            message_action(phosphor::COPY, "Copy", COPY_BASE + index as u64, false);
                        }
                        if fork {
                            message_action(
                                phosphor::GIT_FORK,
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
                row_arranged(
                    Modifier::new(),
                    Arrangement::SpacedBy(sp(2)),
                    Alignment::Center,
                    || {
                        glyph(phosphor::TIMER, 3.5, Role::Subtle);
                        chat_line(duration, 12, Role::Subtle);
                    },
                );
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
    row_arranged(
        Modifier::new()
            .fill_max_width()
            .height(theme::sp(10))
            .draw_behind(theme::edge(
                theme::Side::Bottom,
                1.0,
                theme::color(Role::Border),
            ))
            .padding_each(0.0, 0.0, 0.0, 1.0)
            .padding_xy(theme::sp(5), 0.0),
        Arrangement::SpacedBy(sp(3)),
        Alignment::Center,
        || {
            status_icon(session.shown());
            clipped(&session.title, 14, Role::Text);
            if !session.branch.is_empty() {
                row_arranged(
                    Modifier::new()
                        .background_rounded(theme::color(Role::Surface), Radius::Md.px())
                        .padding_xy(theme::sp(2), 0.0),
                    Arrangement::SpacedBy(sp(1)),
                    Alignment::Center,
                    || {
                        glyph(phosphor::GIT_FORK, 3.0, Role::Muted);
                        rustle::basic_text(
                            Modifier::new(),
                            &session.branch,
                            theme::mono_style(11, 1.6).color(theme::color(Role::Muted)),
                            rustle::TextOverflow::Ellipsis,
                        );
                    },
                );
            }
            clipped(who, 12, Role::Subtle);
            crate::spend::cost_of(session);
        },
    );
}

/// Emits the conversation of `session` in a column `width` logical pixels wide, with the prompt box below, which keeps its own height and runs past the bottom when the column is too short for it.
#[track_caller]
pub fn chat(state: &WorkspaceState, session: &Session, width: f32, above: f32) {
    let text_width = width.min(COLUMN) - GUTTER * 2.0;
    column_arranged(
        Modifier::new().weight(1.0),
        Arrangement::Start,
        Alignment::Start,
        || {
            header(state, session);
            crate::pull_request_bar::bar(session);
            conversation_text(state, || {
                conversation(state, session, Modifier::new().fill_max_size(), text_width)
            });
            column_arranged(
                Modifier::new()
                    .fill_max_width()
                    .wrap_content_height(BoxAlignment::TopStart, true)
                    .padding_each(0.0, 0.0, 0.0, theme::sp(5)),
                Arrangement::Start,
                Alignment::Center,
                || {
                    column_arranged(
                        Modifier::new()
                            .width_in(0.0, COLUMN)
                            .fill_max_width()
                            .padding_xy(theme::sp(6), 0.0),
                        Arrangement::Start,
                        Alignment::Start,
                        || {
                            queued(state, session, text_width);
                            if let Some(draft) = session.inquiry.as_ref() {
                                column_arranged(
                                    Modifier::new()
                                        .padding_each(0.0, 0.0, 0.0, theme::sp(2))
                                        .fill_max_width(),
                                    Arrangement::Start,
                                    Alignment::Start,
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
                            );
                        },
                    );
                },
            );
        },
    );
}

/// Emits `content`, the conversation's list, as the selectable text of the conversation: a
/// selection container holding the workspace's selection of it, filling the rest of the column.
#[track_caller]
fn conversation_text(state: &WorkspaceState, content: impl FnOnce()) {
    rustle::selection_container_with(
        state
            .chat_bounds
            .track(Modifier::new().fill_max_width().weight(1.0)),
        &state.chat_selection,
        content,
    );
}

/// One entry of the conversation's list, in the order it reads from the top.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum Entry {
    /// The message at this position of the session.
    Message(usize),
    /// The request to trust the session's folder.
    Trust,
    /// The question the agent asks.
    Question,
    /// What the agent is doing while it works and has nothing else to show.
    Working,
}

/// The entries of the conversation of `session`, from the top: its messages, then the
/// request to trust its folder, the question the agent asks, or what it is doing while it works.
fn entries(session: &Session) -> Vec<Entry> {
    let mut entries: Vec<Entry> = (0..session.messages.len()).map(Entry::Message).collect();
    if session.trust.is_some() {
        entries.push(Entry::Trust);
    }
    let asking = session.ask.as_ref();
    if asking.is_some_and(|ask| ask.plan.is_none()) {
        entries.push(Entry::Question);
    } else if asking.is_none() && session.inquiry.is_none() && session.status == Status::Working {
        entries.push(Entry::Working);
    }
    entries
}

/// Emits the conversation of `session` as a lazy list in `modifier`, `width` logical pixels of
/// text wide: only the entries near what is shown are composed. The list is laid out from its
/// end, newest entry first, in the list state the workspace holds, so it stays at its end while
/// the agent writes, and keeps how far it is scrolled back from its end when older messages are
/// measured, the pane is resized or the session is shown again.
#[track_caller]
fn conversation(state: &WorkspaceState, session: &Session, modifier: Modifier, width: f32) {
    let entries = entries(session);
    let page = state.open_index();
    let newest_first = |index: usize| entries[entries.len() - 1 - index];
    rustle::lazy_list_with(
        modifier.fill_max_width(),
        &state.chat_list,
        rustle::LazyContent::new()
            .padding(theme::sp(8))
            .spacing(sp(6))
            .reverse_layout(true),
        entries.len(),
        |index| (page, newest_first(index)),
        |index| {
            rustle::stack_with(Modifier::new().fill_max_width(), || {
                column_arranged(
                    Modifier::new()
                        .align(BoxAlignment::TopCenter)
                        .width_in(0.0, COLUMN)
                        .fill_max_width()
                        .padding_xy(theme::sp(6), 0.0),
                    Arrangement::Start,
                    Alignment::Start,
                    || entry(state, session, newest_first(index), width),
                );
            });
        },
    );
}

/// Emits `entry` of the conversation of `session`, `width` logical pixels of text wide.
#[track_caller]
fn entry(state: &WorkspaceState, session: &Session, entry: Entry, width: f32) {
    match entry {
        Entry::Message(index) => message(state, session, index, width),
        Entry::Trust => {
            if let Some(request) = session.trust.as_ref() {
                crate::trust::card(request, width);
            }
        }
        Entry::Question => {
            if let Some(ask) = session.ask.as_ref() {
                question(ask, width);
            }
        }
        Entry::Working => working(session),
    }
}

/// Emits the line saying what the agent of `session` is doing while it works.
#[track_caller]
fn working(session: &Session) {
    let label = if session.out_of_reach {
        "The server is out of reach. Waiting for the connection to come back…"
    } else if let Some(step) = &session.container {
        step.as_str()
    } else if session.echoes > 0 {
        "Sending prompt…"
    } else {
        "Thinking…"
    };
    row_arranged(
        Modifier::new(),
        Arrangement::SpacedBy(sp(2)),
        Alignment::Center,
        || {
            status_icon(session.shown());
            line(label, 13, 1.4, Role::Muted);
        },
    );
}

/// Everything outside a message that what it emits depends on, so a message is composed
/// again only when one of them changes, and typing in the prompt box leaves it as it was.
#[derive(PartialEq)]
struct MessageInputs {
    /// The message itself.
    message: crate::sample::Message,
    /// Its position, which the identities of its controls and links count from.
    index: usize,
    /// The place of its session in the workspace.
    page: Option<usize>,
    /// The folder the targets of its tools are shown relative to.
    project: String,
    /// Whether its session is working.
    working: bool,
    /// The reply whose steps are open unless the reader chose.
    latest_steps: Option<usize>,
    /// Whether it can be branched from.
    fork: bool,
    /// Whether it is the last message.
    last: bool,
    /// The message whose offer to restore files is open.
    restoring: Option<usize>,
    /// The tool cards open.
    opened: Vec<(usize, usize, usize)>,
    /// The fold-outs of replies open.
    unfolded: Vec<(usize, usize, usize)>,
    /// The figures drawn so far.
    figures: crate::figures::Figures,
    /// What its exhibits offer while the embedded browser is missing, when it has exhibits.
    offer: Option<crate::exhibit::Offer>,
    /// The thumbnails of attached files.
    previews: std::collections::HashMap<String, rustle::ImageData>,
    /// The fonts, width, density, text scale and, while a reply runs, the second its timer shows.
    geometry: (u64, u32, u32, u32, u64),
}

/// Emits the message at `index` of `session`, `width` logical pixels of text wide, skipped while
/// nothing it shows has changed.
#[track_caller]
fn message(state: &WorkspaceState, session: &Session, index: usize, width: f32) {
    let message = &session.messages[index];
    let working = session.status == Status::Working;
    let second = if working && !message.steps.is_empty() {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |time| time.as_secs())
    } else {
        0
    };
    let inputs = MessageInputs {
        message: message.clone(),
        index,
        page: state.open_index(),
        project: state
            .project_of(session)
            .map_or(session.directory.as_str(), |project| project.path.as_str())
            .to_owned(),
        working,
        latest_steps: session.latest_steps(),
        fork: session.turn_of(index) > 0,
        last: index + 1 == session.messages.len(),
        restoring: state.restoring,
        opened: state.opened.clone(),
        unfolded: state.unfolded.clone(),
        figures: state.figures.clone(),
        offer: (!message.exhibits.is_empty()).then(|| state.preview_offer()),
        previews: state.previews.clone(),
        geometry: (
            theme::font_key(),
            width.to_bits(),
            theme::step_scale().to_bits(),
            theme::text_scale().to_bits(),
            second,
        ),
    };
    rustle::skippable(inputs, || {
        if message.from_user {
            user_message(state, session, index, width)
        } else {
            agent_message(state, session, index, width)
        }
    });
}

/// Draws the content only while the group around it is hovered or it counts as focused.
fn shown_on_group_hover(canvas: &mut rustle::Canvas, interaction: rustle::Interaction) {
    if interaction.group_hovered || interaction.focused {
        canvas.draw_content();
    }
}

/// Tests of the conversation's list: what it composes, where it stays as messages grow and
/// arrive, and that messages skipped while unchanged show what a fresh list shows.
#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::rc::Rc;

    use rustle::Rect;

    use crate::bounds::Places;
    use crate::message_menu::message_of_area;
    use crate::state::WorkspaceState;
    use crate::tagged::{interface, tagged_in};

    use super::*;

    /// The conversation of the first session of the state in `held` laid out 600 by 1000 logical
    /// pixels and settled until the list has measured what it shows, with where its messages went.
    struct Shown {
        /// The interface holding the conversation.
        ui: rustle::Embedded,
        /// Where each message composed is drawn.
        places: Places,
    }

    impl Shown {
        /// Lays out the conversation of the first session of the state in `held`.
        fn new(held: &Rc<RefCell<WorkspaceState>>) -> Self {
            let root = held.clone();
            let places = Places::default();
            let provided = places.clone();
            let ui = interface(600.0, 1000.0, None, move || {
                let state = root.borrow();
                provided.provide(|| {
                    conversation(
                        &state,
                        &state.sessions[0],
                        Modifier::new().fill_max_size(),
                        552.0,
                    )
                });
            });
            let mut shown = Self { ui, places };
            shown.settle();
            shown
        }

        /// Composes and lays the conversation out again until nothing changes.
        fn settle(&mut self) {
            self.ui.invalidate();
            for _ in 0..40 {
                if !self.ui.update() {
                    return;
                }
            }
        }

        /// The messages composed and where the visible part of each is drawn, by position.
        fn messages(&self) -> Vec<(usize, Rect)> {
            let mut shown: Vec<(usize, Rect)> = self
                .places
                .shown()
                .into_iter()
                .filter_map(|(id, rect)| Some((message_of_area(id)?, rect)))
                .collect();
            shown.sort_by_key(|(index, _)| *index);
            shown
        }

        /// Where the visible part of message `index` is drawn, if it is composed and in view.
        fn message(&self, index: usize) -> Option<Rect> {
            self.messages()
                .into_iter()
                .find(|(at, rect)| *at == index && rect.size.height > 0.0)
                .map(|(_, rect)| rect)
        }
    }

    /// The sample workspace with its first session holding `count` replies, each saying its number.
    fn long_conversation(count: usize) -> Rc<RefCell<WorkspaceState>> {
        let mut state = WorkspaceState::sample();
        let reply = state.sessions[0]
            .messages
            .iter()
            .find(|message| !message.from_user && message.side.is_none())
            .unwrap()
            .clone();
        state.sessions[0].messages = (0..count)
            .map(|number| {
                let mut reply = reply.clone();
                reply.text = format!("Reply number {number}.");
                reply.said.clear();
                reply
            })
            .collect();
        state.sessions[0].status = Status::Idle;
        Rc::new(RefCell::new(state))
    }

    /// Appends two paragraphs to message `index` of the first session in `held`, as the agent
    /// writing it does.
    fn write_on(held: &Rc<RefCell<WorkspaceState>>, index: usize) {
        let mut state = held.borrow_mut();
        let reply = &mut state.sessions[0].messages[index];
        reply
            .text
            .push_str("\n\nA new paragraph.\n\nAnd another one, written as the agent goes on.");
        reply.said.clear();
    }

    /// A long conversation opens at its end and composes only the messages near it.
    #[test]
    fn a_long_conversation_opens_at_its_end_and_composes_only_what_is_near() {
        let held = long_conversation(400);
        let shown = Shown::new(&held);
        let composed = shown.messages();
        assert!(composed.len() < 40, "{} messages composed", composed.len());
        let last = shown.message(399).expect("the newest message is in view");
        assert!(last.origin.y + last.size.height <= 1000.0);
        assert!(shown.message(0).is_none());
        assert_eq!(held.borrow().chat_list.scroll().offset(), 0.0);
    }

    /// A conversation restored scrolled back to its start shows its first message.
    #[test]
    fn a_conversation_scrolled_back_to_its_start_shows_its_first_message() {
        let held = long_conversation(60);
        held.borrow().chat_list.scroll().jump_to(1.0e6);
        let shown = Shown::new(&held);
        assert!(shown.message(0).is_some());
        assert!(shown.message(59).is_none());
    }

    /// While the reader is at the end, the reply being written keeps its end in view as it grows.
    #[test]
    fn the_conversation_stays_at_its_end_while_the_agent_writes() {
        let held = long_conversation(30);
        let mut shown = Shown::new(&held);
        let before = shown.message(29).unwrap();
        write_on(&held, 29);
        shown.settle();
        let after = shown.message(29).unwrap();
        assert!(after.size.height > before.size.height);
        let bottom = |rect: Rect| rect.origin.y + rect.size.height;
        assert!((bottom(after) - bottom(before)).abs() < 1.0);
        assert_eq!(held.borrow().chat_list.scroll().offset(), 0.0);
    }

    /// While the reader is scrolled back, what they read stays where it is as the reply being
    /// written grows below it.
    #[test]
    fn a_reader_scrolled_back_keeps_their_place_while_the_agent_writes() {
        let held = long_conversation(60);
        held.borrow().chat_list.scroll().jump_to(2000.0);
        let mut shown = Shown::new(&held);
        let (index, before) = shown
            .messages()
            .into_iter()
            .find(|(_, rect)| (100.0..800.0).contains(&rect.origin.y))
            .expect("a message in view");
        write_on(&held, 59);
        shown.settle();
        let after = shown.message(index).unwrap();
        assert!((after.origin.y - before.origin.y).abs() < 1.0);
    }

    /// While the reader is scrolled back, a shorter pane keeps what sits at the bottom of the
    /// view where it was against the bottom, as the list is laid out from its end, and taking
    /// the height back shows the same place again.
    #[test]
    fn a_reader_scrolled_back_keeps_their_place_when_the_pane_is_resized() {
        let held = long_conversation(60);
        held.borrow().chat_list.scroll().jump_to(2000.0);
        let mut shown = Shown::new(&held);
        let (index, before) = shown
            .messages()
            .into_iter()
            .find(|(_, rect)| (500.0..900.0).contains(&rect.origin.y))
            .expect("a message in view");
        let from_bottom = |rect: Rect, height: f32| height - rect.origin.y;
        shown.ui.resize((600, 700), 1.0);
        shown.settle();
        let shorter = shown.message(index).expect("still in view");
        assert!((from_bottom(shorter, 700.0) - from_bottom(before, 1000.0)).abs() < 1.0);
        shown.ui.resize((600, 1000), 1.0);
        shown.settle();
        let again = shown.message(index).expect("in view again");
        assert!((again.origin.y - before.origin.y).abs() < 1.0);
    }

    /// Typing in the prompt keeps messages as they were, while streamed text and opened
    /// fold-outs show at once, as a list composed afresh shows them.
    #[test]
    fn skipped_messages_show_what_a_fresh_list_shows() {
        let held = Rc::new(RefCell::new(WorkspaceState::sample()));
        let index = held.borrow().sessions[0]
            .messages
            .iter()
            .position(|message| !message.from_user)
            .unwrap();
        let mut shown = Shown::new(&held);
        for change in 0..3 {
            match change {
                0 => held.borrow_mut().prompt.edit(|buffer| buffer.append("x")),
                1 => write_on(&held, index),
                _ => {
                    let mut state = held.borrow_mut();
                    let page = state.open_index().unwrap();
                    state.unfolded.push((page, index, 0));
                }
            }
            shown.settle();
            let mut fresh = held.borrow().clone();
            fresh.chat_list = rustle::LazyListState::default();
            let fresh = Shown::new(&Rc::new(RefCell::new(fresh)));
            let laid = |shown: &Shown| {
                let tagged: Vec<_> = tagged_in(&shown.ui, 0.0, 0.0)
                    .into_iter()
                    .map(|tagged| (tagged.id, tagged.bounds))
                    .collect();
                (tagged, shown.messages())
            };
            assert_eq!(laid(&shown), laid(&fresh), "after change {change}");
        }
    }
}
