//! Timing and keys for predictions in the focused document.

use std::time::Instant;

use winit::event::KeyEvent;
use winit::keyboard::{Key, NamedKey};

use crate::app::App;
use crate::keymap::{self, Action};

impl App {
    /// Whether the focused file may ask for predictions now.
    fn prediction_allowed(&self) -> bool {
        self.preferences.edit_predictions.enabled
            && self.writing.is_none()
            && self.picker.is_none()
            && !self.search_focused
            && !self.changes_focused
            && self.tree_edit.is_none()
            && self.focused_file().is_some_and(|file| {
                let document = file.borrow();
                !document.buffer().has_many_cursors()
                    && document.buffer().selection().is_empty()
                    && (!self.preferences.vim_mode || document.modal().mode().is_typing())
            })
    }

    /// The pending debounce deadline of the focused document.
    pub(super) fn next_prediction(&self) -> Option<Instant> {
        self.focused_file()?.borrow().next_prediction()
    }

    /// Asks the provider once the focused document's typing has settled.
    pub(super) fn ask_prediction(&mut self) {
        if let Some(file) = self.focused_file() {
            if self.prediction_allowed() {
                file.borrow_mut().ask_prediction(
                    self.preferences
                        .edit_predictions
                        .server
                        .map(|server| server.command),
                );
            } else {
                file.borrow_mut().dismiss_prediction();
            }
        }
    }

    /// Collects an answered request without waiting for the server.
    pub(super) fn collect_prediction(&mut self) -> bool {
        self.focused_file()
            .is_some_and(|file| file.borrow_mut().collect_prediction())
    }

    /// Gives Tab and Escape to a shown prediction before the editor sees them.
    pub(super) fn send_to_prediction(&mut self, event: &KeyEvent) -> bool {
        if !self.prediction_allowed() || self.completions.is_some() {
            return false;
        }
        let Some(file) = self.focused_file() else {
            return false;
        };
        if file.borrow().prediction().is_none() {
            return false;
        }
        match event.logical_key.as_ref() {
            Key::Named(NamedKey::Tab) if !self.modifiers.shift_key() => {
                if !self.resolver.pending().is_empty() {
                    return false;
                }
                if let Some(chord) = keymap::chord(event, self.modifiers)
                    && let Some(binding) = self
                        .resolver
                        .keymap()
                        .candidates(&[chord], &self.context())
                        .last()
                    && binding.action != Some(Action::Tab)
                {
                    return false;
                }
                file.borrow_mut().accept_prediction(false)
            }
            Key::Named(NamedKey::Escape) => {
                file.borrow_mut().dismiss_prediction();
                !self.preferences.vim_mode
            }
            _ => false,
        }
    }

    /// Accepts a shown prediction from the keymap or command palette.
    pub(super) fn accept_prediction(&mut self, word: bool) {
        if self.prediction_allowed()
            && self.completions.is_none()
            && let Some(file) = self.focused_file()
        {
            file.borrow_mut().accept_prediction(word);
        }
    }

    /// Hides the focused document's prediction and cancels its request.
    pub(super) fn dismiss_prediction(&mut self) {
        if let Some(file) = self.focused_file() {
            file.borrow_mut().dismiss_prediction();
        }
    }
}
