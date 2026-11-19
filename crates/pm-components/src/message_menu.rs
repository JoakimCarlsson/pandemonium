//! The menu that opens over a message of the conversation: copying what is
//! selected, the message as it reads or as the markdown it was written in,
//! and selecting the whole conversation.

use crate::Menu;
use crate::focus::FocusId;
use crate::state::{Outgoing, WorkspaceState};

/// Copies the selected text of the conversation.
pub const MESSAGE_MENU_COPY: FocusId = FocusId(206);

/// Copies the message the menu was opened on, as it reads.
pub const MESSAGE_MENU_COPY_REPLY: FocusId = FocusId(207);

/// Copies the message the menu was opened on, as the markdown it was written in.
pub const MESSAGE_MENU_COPY_MARKDOWN: FocusId = FocusId(208);

/// Selects the text of the whole conversation.
pub const MESSAGE_MENU_SELECT_ALL: FocusId = FocusId(209);

/// The first of the names of the messages of the conversation, by which the app finds the
/// message under a point; the rest follow it.
pub const MESSAGE_AREA_BASE: u64 = 9_800_000_000_000;

/// How many messages [`MESSAGE_AREA_BASE`] leaves room for.
const MESSAGE_AREAS: u64 = 100_000_000_000;

/// The name of the area of message `index` of the conversation.
pub fn message_area_id(index: usize) -> FocusId {
    FocusId(MESSAGE_AREA_BASE + index as u64)
}

/// The message whose area is named `id`, if it names one.
pub fn message_of_area(id: FocusId) -> Option<usize> {
    let index = id.0.checked_sub(MESSAGE_AREA_BASE)?;
    (index < MESSAGE_AREAS).then_some(index as usize)
}

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
pub fn is_message_menu_item(id: FocusId) -> bool {
    [
        MESSAGE_MENU_COPY,
        MESSAGE_MENU_COPY_REPLY,
        MESSAGE_MENU_COPY_MARKDOWN,
        MESSAGE_MENU_SELECT_ALL,
    ]
    .contains(&id)
}

/// Emits the menu opened at `open`.
#[track_caller]
pub fn message_menu(open: MessageMenu) {
    let operations = crate::events::operations();
    Menu::new(open.at.0, open.at.1)
        .on_pick(move |row| operations.message_menu_picked(row))
        .item("Copy", Some("Ctrl+C"), MESSAGE_MENU_COPY, !open.selected)
        .separator()
        .item("Copy reply", None, MESSAGE_MENU_COPY_REPLY, false)
        .item(
            "Copy reply as Markdown",
            None,
            MESSAGE_MENU_COPY_MARKDOWN,
            false,
        )
        .separator()
        .item("Select all", Some("Ctrl+A"), MESSAGE_MENU_SELECT_ALL, false)
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
    pub(crate) fn message_menu_pressed(mut self, row: FocusId) -> Self {
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
            MESSAGE_MENU_COPY_REPLY => message.plain_text(),
            MESSAGE_MENU_COPY_MARKDOWN => message.markdown_text(),
            _ => return self,
        };
        self.outbox.push(Outgoing::Copy { text });
        self
    }
}
