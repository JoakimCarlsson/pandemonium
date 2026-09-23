//! What a keypress does to a file while modal editing is on.
//!
//! The keys go to [`pm_vim::Vim`] before the keymap sees them, because in
//! normal mode a plain letter is a command rather than a character, and the
//! window's own chords would otherwise take the ones vim gives a meaning.
//! Whatever vim does not take goes on to the keymap and the editor as it
//! would without modal editing, which is how insert mode types.

use pm_vim::{Effect, Mode, Placement, View, Window};
use winit::event::KeyEvent;

use crate::app::App;
use crate::desktop;
use crate::editor;
use crate::keymap::Action;

/// The system clipboard, as modal editing reaches it.
struct Desktop;

impl pm_vim::Clipboard for Desktop {
    /// What is on the clipboard, if it holds text.
    fn read(&mut self) -> Option<String> {
        desktop::paste()
    }

    /// Puts `text` on the clipboard.
    fn write(&mut self, text: String) {
        desktop::copy(text);
    }
}

impl App {
    /// Sends a keypress to modal editing, when it is on and a file has the
    /// keyboard, saying whether it was taken.
    pub(super) fn send_to_vim(&mut self, event: &KeyEvent) -> bool {
        if !self.preferences.vim_mode || self.search_focused || self.writing.is_some() {
            return false;
        }
        if self.is_window_chord() {
            return false;
        }
        let Some(file) = self.focused_file() else {
            return false;
        };
        let Some(key) = editor::keystroke(&event.logical_key, self.modifiers) else {
            return false;
        };
        let view = {
            let document = file.borrow();
            View {
                top: document.scroll(),
                rows: document.rows(),
            }
        };
        let vim = &mut self.vim;
        let outcome = file
            .borrow_mut()
            .edit_modal(|buffer, state| vim.press(state, buffer, view, &mut Desktop, key));
        if !outcome.handled {
            return false;
        }
        if file.borrow().modal().mode() != Mode::Insert {
            self.dismiss_popup();
        }
        for effect in outcome.effects {
            self.carry_out(&file, effect);
        }
        true
    }

    /// Does what modal editing asked of the window for `file`.
    fn carry_out(&mut self, file: &editor::OpenFile, effect: Effect) {
        match effect {
            Effect::Save => self.act(Action::Save),
            Effect::SaveAll => self.act(Action::SaveAll),
            Effect::Close => self.act(Action::ClosePane),
            Effect::Window(window) => self.act(match window {
                Window::FocusLeft => Action::FocusLeft,
                Window::FocusRight => Action::FocusRight,
                Window::FocusUp => Action::FocusUp,
                Window::FocusDown => Action::FocusDown,
                Window::SplitRight => Action::SplitRight,
                Window::SplitDown => Action::SplitDown,
                Window::Close => Action::ClosePane,
            }),
            Effect::Scroll(placement) => {
                let mut document = file.borrow_mut();
                let rows = document.rows();
                document.scroll_cursor_to(match placement {
                    Placement::Top => 0,
                    Placement::Center => rows / 2,
                    Placement::Bottom => rows.saturating_sub(1),
                });
            }
            Effect::ScrollBy(lines) => file.borrow_mut().scroll_by(lines),
        }
    }
}
