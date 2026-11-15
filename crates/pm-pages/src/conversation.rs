//! The conversation of the open session: its messages kept scrolled to the
//! end, selectable as one document, each with its blocks of markdown,
//! thoughts, tool steps and notices, and a menu of what can be done from it.

use std::rc::Rc;

use rustle::prelude::*;

use pm_components::{
    Card, CardVariant, Label, LabelVariant, Markdown, MessageMenu, Notice, NoticeVariant, Size,
    Step, StepVariant, Thought,
};
use pm_shell::{
    Block, ConversationCommand, Message, NoticeKind, Speaker, StepState, ToolKind, ToolStep,
};

use crate::actions::Actions;
use crate::session::READING_WIDTH;

/// Emits `messages` in a list kept scrolled to its end, inside a selection
/// container so their text can be selected and copied across messages.
pub fn conversation(messages: &[Message], actions: &Actions) {
    let list = remember_lazy_list_state();
    list.set_stick_to_end(true);
    let messages = Rc::new(messages.to_vec());
    let actions = actions.clone();
    selection_container(Modifier::new().fill_max_width().weight(1.0), move || {
        let count = messages.len();
        lazy_list_keyed(
            Modifier::new().fill_max_size(),
            &list,
            count,
            |index| index,
            move |index| {
                let message = messages[index].clone();
                let last = index + 1 == messages.len();
                let actions = actions.clone();
                row_arranged(
                    Modifier::new().fill_max_width().padding_each(
                        0.0,
                        if index == 0 { 16.0 } else { 0.0 },
                        0.0,
                        18.0,
                    ),
                    Arrangement::Center,
                    Alignment::Start,
                    move || {
                        column_with(Modifier::new().width(READING_WIDTH), || {
                            message_view(&message, last, &actions)
                        });
                    },
                );
            },
        );
    });
}

/// Emits one message: a bubble on the right for the person, the agent's
/// blocks in a column otherwise, then its footer and menu.
fn message_view(message: &Message, last: bool, actions: &Actions) {
    match message.speaker {
        Speaker::User => {
            row_arranged(
                Modifier::new().fill_max_width(),
                Arrangement::SpacedBy(6.0),
                Alignment::Start,
                || {
                    spacer(Modifier::new().weight(1.0));
                    menu(message, last, actions).show();
                    Card::new()
                        .variant(CardVariant::Filled)
                        .size(Size::Md)
                        .modifier(Modifier::new().width_in(0.0, READING_WIDTH * 0.6))
                        .show(|| blocks(&message.blocks));
                },
            );
        }
        Speaker::Agent => {
            blocks(&message.blocks);
            row_arranged(
                Modifier::new()
                    .fill_max_width()
                    .padding_each(0.0, 6.0, 0.0, 0.0),
                Arrangement::SpacedBy(6.0),
                Alignment::Center,
                || {
                    if let Some(footer) = &message.footer {
                        icon(phosphor::CLOCK, 11.0, color_scheme().on_surface_variant);
                        Label::new(footer.clone())
                            .variant(LabelVariant::Subtle)
                            .size(Size::Xs)
                            .show();
                    }
                    menu(message, last, actions).show();
                },
            );
        }
    }
}

/// Emits `list` in a column, each block as its kind shows.
fn blocks(list: &[Block]) {
    column_arranged(
        Modifier::new().fill_max_width(),
        Arrangement::SpacedBy(8.0),
        Alignment::Start,
        || {
            for block in list {
                match block {
                    Block::Markdown { blocks, .. } => Markdown::new(blocks.clone()).show(),
                    Block::Thought(text) => Thought::new(text.clone()).show(),
                    Block::Tool(step) => tool_step(step),
                    Block::Notice(notice) => {
                        let (variant, glyph) = match notice.kind {
                            NoticeKind::Fallback => {
                                (NoticeVariant::Warning, phosphor::ARROWS_CLOCKWISE)
                            }
                            NoticeKind::Aside => (NoticeVariant::Accent, phosphor::CHAT_CIRCLE),
                            NoticeKind::HandedOff => {
                                (NoticeVariant::Info, phosphor::ARROW_BEND_DOWN_RIGHT)
                            }
                            NoticeKind::Failure => (NoticeVariant::Danger, phosphor::WARNING),
                        };
                        Notice::new(notice.title.clone())
                            .detail(notice.detail.clone())
                            .variant(variant)
                            .icon(glyph)
                            .show();
                    }
                }
            }
        },
    );
}

/// Emits the row of the tool call `step`.
fn tool_step(step: &ToolStep) {
    let glyph = match step.kind {
        ToolKind::Read => phosphor::FILE_TEXT,
        ToolKind::Edit => phosphor::PENCIL_SIMPLE,
        ToolKind::Search => phosphor::MAGNIFYING_GLASS,
        ToolKind::Execute => phosphor::TERMINAL,
        ToolKind::Fetch => phosphor::GLOBE,
        ToolKind::Other => phosphor::WRENCH,
    };
    let variant = match step.state {
        StepState::Running => StepVariant::Running,
        StepState::Done => StepVariant::Done,
        StepState::Failed => StepVariant::Failed,
    };
    Step::new(step.title.clone())
        .icon(glyph)
        .variant(variant)
        .lines(step.lines)
        .output(step.output.clone())
        .show();
}

/// The menu of `message`: copying it, and the conversation commands that act
/// from it; retrying only from the `last` message.
fn menu(message: &Message, last: bool, actions: &Actions) -> MessageMenu {
    let command = |order: ConversationCommand| {
        let run = actions.conversation.clone();
        move || run(order.clone())
    };
    let mut menu = MessageMenu::new().copy("Copy", message.plain_text());
    match (message.speaker, message.turn) {
        (Speaker::User, Some(turn)) => {
            menu = menu
                .item("Rewind to here", command(ConversationCommand::Rewind(turn)))
                .item("Branch off here", command(ConversationCommand::Fork(turn)));
        }
        _ => {
            if last {
                menu = menu.item("Retry", command(ConversationCommand::Retry));
            }
            menu = menu
                .item("Start afresh", command(ConversationCommand::Afresh))
                .item("Hand over", command(ConversationCommand::HandOver))
                .item(
                    "Ask on the side",
                    command(ConversationCommand::SideQuestion),
                );
        }
    }
    menu
}
