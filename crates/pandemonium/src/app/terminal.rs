//! The terminal dock in the window: sizing it to the shell, typing into it, marking text and scrolling back.

use pm_components::terminal::{cell_at, cell_size, fitting};
use pm_components::terminal_keys;
use pm_input::{Key, KeyEvent};
use pm_pages::TERMINAL_BODY;
use pm_renderer::Rect;

use super::App;
use crate::keys::{self, Clip};

impl App {
    /// Where the window of the shell showing was drawn, if it was.
    fn terminal_region(&self) -> Option<Rect> {
        self.output
            .regions
            .iter()
            .find(|region| region.id == TERMINAL_BODY)
            .map(|region| region.rect)
    }

    /// The cell of the shell's window that the point `at` is over, and the room it has.
    fn terminal_cell(&mut self, at: (f32, f32)) -> Option<(usize, usize)> {
        let body = self.terminal_region()?;
        let (columns, rows, _) = self.model.read(|screen| screen.terminal_window())?;
        let cell = cell_size(&mut self.shaper);
        Some(cell_at(
            (at.0 - body.x, at.1 - body.y),
            cell,
            (columns, rows),
        ))
    }

    /// Gives the shell the size of the room the dock has for it, when that differs from the size it has.
    pub(super) fn fit_terminal(&mut self) {
        let Some(body) = self.terminal_region() else {
            return;
        };
        let cell = cell_size(&mut self.shaper);
        let (columns, rows) = fitting(body.width, body.height, cell);
        let differs = self
            .model
            .read(|screen| screen.terminal_window())
            .is_some_and(|(held_columns, held_rows, _)| {
                (held_columns, held_rows) != (columns as usize, rows as usize)
            });
        if differs {
            self.model
                .update(|screen| screen.fit_terminal(columns, rows));
            self.deliver();
        }
    }

    /// Sends `data` to the shell showing.
    fn type_into_terminal(&mut self, data: Vec<u8>) {
        self.model.update(|screen| screen.type_into_terminal(data));
        self.deliver();
    }

    /// Types the text an input method committed into the shell, when it holds the keyboard.
    pub(super) fn commit_to_terminal(&mut self, text: &str) -> bool {
        if self.focus.current() != Some(TERMINAL_BODY) {
            return false;
        }
        self.type_into_terminal(text.as_bytes().to_vec());
        true
    }

    /// Acts on a key when the shell holds the keyboard, and says whether it did.
    pub(super) fn terminal_key(&mut self, event: KeyEvent) -> bool {
        if self.focus.current() != Some(TERMINAL_BODY) {
            return false;
        }
        let Some((application, bracketed)) = self.model.read(|screen| screen.terminal_modes())
        else {
            return false;
        };
        if let Some(action) = keys::terminal_clip(&event) {
            self.terminal_clip(action, bracketed);
        } else if event.modifiers.shift && matches!(event.key, Key::PageUp | Key::PageDown) {
            let page = self
                .model
                .read(|screen| screen.terminal_window())
                .map_or(1, |(_, rows, _)| rows.saturating_sub(1).max(1))
                as isize;
            let lines = if event.key == Key::PageUp {
                page
            } else {
                -page
            };
            self.model.update(|screen| screen.scroll_terminal(lines));
        } else if let Some(data) = terminal_keys::bytes(&event, application) {
            self.type_into_terminal(data);
        }
        true
    }

    /// Copies the text marked in the shell, or pastes the clipboard into it.
    fn terminal_clip(&mut self, action: Clip, bracketed: bool) {
        match action {
            Clip::Paste => {
                let pasted = self
                    .clipboard()
                    .and_then(|clipboard| clipboard.get_text().ok());
                if let Some(text) = pasted {
                    self.type_into_terminal(terminal_keys::paste(&text, bracketed));
                }
            }
            Clip::Copy | Clip::Cut => {
                if let Some(text) = self.model.read(|screen| screen.terminal_selection())
                    && let Some(clipboard) = self.clipboard()
                {
                    let _ = clipboard.set_text(text);
                }
            }
        }
    }

    /// Begins marking text when the primary button went down in the shell's window.
    pub(super) fn terminal_press(&mut self) {
        self.terminal_anchor = None;
        let Some(at) = self.pointer.position else {
            return;
        };
        if self
            .terminal_region()
            .is_some_and(|body| body.contains(at.0, at.1))
        {
            self.terminal_anchor = self.terminal_cell(at);
            self.model
                .update(|screen| screen.clear_terminal_selection());
        }
    }

    /// Extends the marked text to the point `at` while the button is held.
    pub(super) fn terminal_drag(&mut self, at: (f32, f32)) {
        let Some(anchor) = self.terminal_anchor else {
            return;
        };
        if let Some(head) = self.terminal_cell(at)
            && head != anchor
        {
            self.model
                .update(|screen| screen.select_in_terminal(anchor, head));
        }
    }

    /// Scrolls the shell back or forward when the wheel turned over its window, and says whether it did.
    pub(super) fn terminal_wheel(&mut self, pixels: f32) -> bool {
        let Some(at) = self.pointer.position else {
            return false;
        };
        if !self
            .terminal_region()
            .is_some_and(|body| body.contains(at.0, at.1))
        {
            return false;
        }
        let height = cell_size(&mut self.shaper).1;
        self.terminal_scroll_rest -= pixels / height;
        let lines = self.terminal_scroll_rest.trunc();
        self.terminal_scroll_rest -= lines;
        self.model
            .update(|screen| screen.scroll_terminal(lines as isize));
        true
    }
}
