//! The menu that opens over a message of the conversation: copying what is
//! selected, the message as it reads or as the markdown it was written in,
//! and selecting the whole conversation.

use crate::Menu;
use crate::areas::MenuEvent;
use crate::state::{Outgoing, WorkspaceState};
use crate::target::Target;

/// The menu open over a message of the open session, and where.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MessageMenu {
    /// The message, by position in the open session.
    pub message: usize,
    /// Where it opened, in logical pixels.
    pub at: (f32, f32),
    /// Whether any of the conversation's text is selected, so there is something to copy.
    pub selected: bool,
}

/// Whether `id` names a row of the menu over a message.
pub fn is_message_menu_item(id: Target) -> bool {
    [
        Target::Menu(MenuEvent::MessageMenuCopy),
        Target::Menu(MenuEvent::MessageMenuCopyReply),
        Target::Menu(MenuEvent::MessageMenuCopyMarkdown),
        Target::Menu(MenuEvent::MessageMenuSelectAll),
    ]
    .contains(&id)
}

/// Emits the menu opened at `open`.
#[track_caller]
pub fn message_menu(open: MessageMenu) {
    let operations = crate::events::operations();
    Menu::new(open.at.0, open.at.1)
        .on_pick(move |row| operations.message_menu_picked(row))
        .item(
            "Copy",
            Some("Ctrl+C"),
            Target::Menu(MenuEvent::MessageMenuCopy),
            !open.selected,
        )
        .separator()
        .item(
            "Copy reply",
            None,
            Target::Menu(MenuEvent::MessageMenuCopyReply),
            false,
        )
        .item(
            "Copy reply as Markdown",
            None,
            Target::Menu(MenuEvent::MessageMenuCopyMarkdown),
            false,
        )
        .separator()
        .item(
            "Select all",
            Some("Ctrl+A"),
            Target::Menu(MenuEvent::MessageMenuSelectAll),
            false,
        )
        .build();
}

impl WorkspaceState {
    /// The state with the menu over the message at `message` of the open
    /// session open at `at`, offering to copy the selection when `selected`.
    pub fn with_message_menu(mut self, message: usize, at: (f32, f32), selected: bool) -> Self {
        let exists = self
            .open()
            .is_some_and(|session| message < session.messages.len());
        if exists {
            self.menu = None;
            self.project_menu = None;
            self.session_menu = None;
            self.message_menu = Some(MessageMenu {
                message,
                at,
                selected,
            });
        }
        self
    }

    /// The state after the row `row` of the open menu over a message was
    /// pressed: the menu closes, and the message is asked to be put on the
    /// clipboard when that is what the row does.
    pub(crate) fn message_menu_pressed(mut self, row: Target) -> Self {
        let Some(open) = self.message_menu.take() else {
            return self;
        };
        let Some(message) = self
            .open()
            .and_then(|session| session.messages.get(open.message))
        else {
            return self;
        };
        let text = match row {
            Target::Menu(MenuEvent::MessageMenuCopyReply) => message.plain_text(),
            Target::Menu(MenuEvent::MessageMenuCopyMarkdown) => message.markdown_text(),
            _ => return self,
        };
        self.outbox.push(Outgoing::Copy { text });
        self
    }
}
