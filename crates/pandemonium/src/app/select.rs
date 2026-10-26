//! Selecting the text of the conversation with the pointer, copying it, and
//! the menu over a message that copies it or the message.

use pm_components::assist::HOVER_CARD;
use pm_components::message_menu::{
    MESSAGE_MENU_COPY, MESSAGE_MENU_SELECT_ALL, is_message_menu_item,
};
use pm_components::panes::pane_of;
use pm_components::state::CHAT_TEXT;
use pm_input::{FocusId, Key, KeyEvent};
use pm_pages::Screen;
use pm_text::{Edit, Reach};

use super::gesture::ChatText;
use super::{App, DOUBLE_CLICK};
use crate::keys::{self, Clip};

impl App {
    /// Begins a selection where the primary button went down, when it went
    /// down on selectable text and on nothing that can be pressed but the
    /// pane holding it, and lets go of the selection there was unless the
    /// press is on a row of the menu over a message, which may act on it. A
    /// second press soon after the first takes the word under it and a third
    /// the paragraph; a press with Shift held runs the selection there was
    /// to it.
    pub(super) fn text_press(&mut self) {
        if self.pressed_on.is_some_and(is_message_menu_item) {
            return;
        }
        let before = self.text_selection.take();
        self.text_gesture.release();
        if self
            .pressed_on
            .is_some_and(|id| id != HOVER_CARD && pane_of(id).is_none())
        {
            return;
        }
        let Some((id, point)) = self
            .pointer
            .position
            .and_then(|at| self.output.text_under(at))
        else {
            return;
        };
        if self.modifiers.shift
            && let Some(before) = before.filter(|held| held.id == id)
        {
            let mut text = ChatText {
                output: &self.output,
                selection: &mut self.text_selection,
                id,
            };
            self.text_gesture
                .press_from(&mut text, before.anchor, point);
            return;
        }
        let reach = Reach::of_clicks(self.clicks.press(id, DOUBLE_CLICK));
        let mut text = ChatText {
            output: &self.output,
            selection: &mut self.text_selection,
            id,
        };
        self.text_gesture.press(&mut text, point, reach);
    }

    /// Extends the selection to the point `at` while the button is held, a
    /// word or a paragraph at a time when the press took one.
    pub(super) fn text_drag(&mut self, at: (f32, f32)) {
        let Some(selection) = self.text_selection else {
            return;
        };
        if let Some(point) = self.output.text_point(selection.id, at) {
            let mut text = ChatText {
                output: &self.output,
                selection: &mut self.text_selection,
                id: selection.id,
            };
            self.text_gesture.drag(&mut text, point);
        }
    }

    /// Ends a selection when the button comes up, letting go of it when it holds nothing.
    pub(super) fn text_release(&mut self) {
        self.text_gesture.release();
        if self
            .text_selection
            .is_some_and(|selection| selection.is_empty())
        {
            self.text_selection = None;
        }
    }

    /// Selects the whole conversation, copies the selected text or lets go
    /// of it when a key asks for that outside a text field, and says whether
    /// it did.
    pub(super) fn text_key(&mut self, event: KeyEvent) -> bool {
        if self.focus.current().is_some_and(Screen::is_field) {
            return false;
        }
        if matches!(keys::edit(&event), Some(Edit::SelectAll)) {
            self.select_all_text();
            return self.text_selection.is_some();
        }
        if self.text_selection.is_none() {
            return false;
        }
        if event.key == Key::Escape {
            self.text_selection = None;
            return true;
        }
        if keys::clip(&event) != Some(Clip::Copy) {
            return false;
        }
        self.copy_selection();
        true
    }

    /// Selects the text of the whole conversation, when one is drawn.
    fn select_all_text(&mut self) {
        self.text_selection = self.output.select_all(CHAT_TEXT);
        self.text_gesture.release();
    }

    /// Puts the selected text on the clipboard, when some is selected.
    pub(super) fn copy_selection(&mut self) {
        let Some(selection) = self.text_selection.filter(|held| !held.is_empty()) else {
            return;
        };
        let text = self.output.selected_text(&selection);
        if let Some(clipboard) = self.clipboard() {
            let _ = clipboard.set_text(text);
        }
    }

    /// Opens the menu over the message under the pointer, when it is over
    /// one, and closes any other menu.
    pub(super) fn open_message_menu(&mut self) {
        let Some(at) = self.pointer.position else {
            return;
        };
        let message = self
            .output
            .selectable_at(at)
            .filter(|(id, _)| *id == CHAT_TEXT)
            .map(|(_, message)| message);
        let selected = self.text_selection.is_some_and(|held| !held.is_empty());
        self.model.update(|state| {
            let closed = std::mem::take(state).without_menu();
            *state = match message {
                Some(message) => closed.with_message_menu(message, at, selected),
                None => closed,
            };
        });
    }

    /// Carries out the row `row` of the menu over a message, and closes the menu.
    pub(super) fn message_menu_action(&mut self, row: FocusId) {
        match row {
            MESSAGE_MENU_COPY => self.copy_selection(),
            MESSAGE_MENU_SELECT_ALL => self.select_all_text(),
            _ => {}
        }
        self.model
            .update(|state| *state = std::mem::take(state).after(row));
        self.deliver();
    }
}
