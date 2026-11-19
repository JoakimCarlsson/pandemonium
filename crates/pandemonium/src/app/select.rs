//! What the app does with the selections of the conversation's text and of the
//! card about a place, which their selection containers make as the pointer
//! presses and drags: letting go of them on a press elsewhere, the keys that
//! select all and copy outside a text field, and the menu over a message that
//! copies or selects all.

use pm_components::areas::{EditorEvent, MenuEvent};
use pm_components::events::Event;
use pm_components::message_menu::is_message_menu_item;
use pm_components::target::Target;
use pm_field::Edit;
use pm_pages::Screen;
use rustle::{Key, KeyEvent, SelectionPoint};

use super::App;
use crate::keys::{self, Clip};

impl App {
    /// Lets go of the selections a primary press lands outside of, as a press elsewhere
    /// takes focus from a selection container: the conversation's unless the press is in
    /// the conversation, the card's unless it is on the card, and neither when the press is
    /// on a row of the menu over a message, which may act on the selection.
    pub(super) fn text_press(&mut self) {
        if self.pressed_on.is_some_and(is_message_menu_item) {
            return;
        }
        let in_conversation = self.pointer.position.is_some_and(|(x, y)| {
            self.model
                .read(Screen::conversation_area)
                .map(|area| self.framed(area))
                .is_some_and(|area| {
                    x >= area.origin.x
                        && y >= area.origin.y
                        && x < area.origin.x + area.size.width
                        && y < area.origin.y + area.size.height
                })
        });
        if !in_conversation {
            self.chat_selection().clear();
        }
        if self.pressed_on != Some(Target::Editor(EditorEvent::HoverCard)) {
            self.model.read(Screen::hover_selection).clear();
        }
    }

    /// Selects the whole conversation, copies the selected text or lets go
    /// of it when a key asks for that outside a text field, and says whether
    /// it did.
    pub(super) fn text_key(&mut self, event: &KeyEvent) -> bool {
        if self.focus.current().is_some_and(Screen::is_field) {
            return false;
        }
        if matches!(keys::edit(event), Some(Edit::SelectAll)) {
            self.select_all_text();
            return self.text_selection().is_some();
        }
        if self.text_selection().is_none() {
            return false;
        }
        if event.key == Key::Escape {
            self.chat_selection().clear();
            return true;
        }
        if keys::clip(event) != Some(Clip::Copy) {
            return false;
        }
        self.copy_selection();
        true
    }

    /// Selects the text of the whole conversation, when one is drawn.
    fn select_all_text(&mut self) {
        self.chat_selection().select_all();
    }

    /// The selection of the conversation's text.
    pub(super) fn chat_selection(&self) -> rustle::SelectionState {
        self.model.read(Screen::chat_selection)
    }

    /// The two ends of the selected stretch of the conversation's text, if any is selected.
    pub(super) fn text_selection(&self) -> Option<(SelectionPoint, SelectionPoint)> {
        self.chat_selection().selection()
    }

    /// The selected text of the conversation, as it is copied, or nothing when none is selected.
    #[cfg(test)]
    pub(super) fn selected_text(&self) -> String {
        self.chat_selection().selected_text().unwrap_or_default()
    }

    /// Puts the selected text of the conversation on the clipboard, when some is selected.
    pub(super) fn copy_selection(&mut self) {
        let Some(text) = self.chat_selection().selected_text() else {
            return;
        };
        if let Some(clipboard) = self.clipboard() {
            clipboard.set_text(&text);
        }
    }

    /// Opens the menu over the message under the pointer, when it is over
    /// one, and closes any other menu.
    pub(super) fn open_message_menu(&mut self) {
        let Some(at) = self.pointer.position else {
            return;
        };
        let (left, top) = self.framed_point((0.0, 0.0));
        let message = self
            .model
            .read(|screen| screen.message_at((at.0 - left, at.1 - top)));
        let selected = self.text_selection().is_some();
        self.model.update(|state| {
            let closed = std::mem::take(state).without_menu();
            *state = match message {
                Some(message) => closed.with_message_menu(message, at, selected),
                None => closed,
            };
        });
    }

    /// Carries out the row `row` of the menu over a message, and closes the menu.
    pub(super) fn message_menu_action(&mut self, row: Target) {
        match row {
            Target::Menu(MenuEvent::MessageMenuCopy) => self.copy_selection(),
            Target::Menu(MenuEvent::MessageMenuSelectAll) => self.select_all_text(),
            _ => {}
        }
        self.model
            .update(|state| *state = std::mem::take(state).handle(Event::pressing(row)));
        self.deliver();
    }
}
