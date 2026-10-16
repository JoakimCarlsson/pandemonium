//! The terminal dock in the window: sizing it to the shell, typing into it, marking text and scrolling back.

use pm_components::terminal::{cell_at, cell_size, fitting};
use pm_components::terminal_keys;
use pm_components::terminal_menu::{TERMINAL_MENU_HEIGHT, TerminalCommand};
use pm_editor::{Action, Pressed};
use pm_input::{Key, KeyEvent};
use pm_pages::{PROMPT, TERMINAL_BODY};
use pm_renderer::Rect;

use super::App;
use crate::keys::{self, Clip};

impl App {
    /// How tall the type of the shells is, in logical pixels, as the settings say.
    fn terminal_font(&self) -> f32 {
        self.model
            .read(|screen| match screen {
                pm_pages::Screen::Workspace(state) => {
                    Some(pm_components::terminal::Look::of(state).font)
                }
                pm_pages::Screen::Onboarding(_) => None,
            })
            .unwrap_or(13.0)
    }

    /// How wide and how tall one cell of a shell's window is, in logical pixels.
    fn terminal_cell_size(&mut self) -> (f32, f32) {
        let font = self.terminal_font();
        cell_size(&mut self.shaper, font)
    }

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
        let cell = self.terminal_cell_size();
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
        let cell = self.terminal_cell_size();
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
        if self.terminal_keymap_key(event) {
            return true;
        }
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

    /// Takes a press of keys the keymap binds to one of the shell's actions, or that starts such a binding, and says whether it took it.
    fn terminal_keymap_key(&mut self, event: KeyEvent) -> bool {
        let modifiers = event.modifiers;
        if !(modifiers.control || modifiers.logo || modifiers.alt) && self.pending.is_empty() {
            return false;
        }
        let Some(chord) = Self::chord_of(&event) else {
            return false;
        };
        match self
            .keymap()
            .press_among(&self.pending, &chord, Action::in_terminal)
        {
            Pressed::Waiting => {
                self.pending.push(chord);
                true
            }
            Pressed::Action(action) => {
                self.pending.clear();
                if let Some(command) = TerminalCommand::of_action(action) {
                    self.terminal_command(command);
                }
                true
            }
            Pressed::Nothing => !std::mem::take(&mut self.pending).is_empty(),
        }
    }

    /// Carries out `command` on the shell showing: the clipboard's part of pasting here, the rest by the state, giving the prompt the keyboard when the marked text went into it.
    pub(super) fn terminal_command(&mut self, command: TerminalCommand) {
        if command == TerminalCommand::Paste {
            let bracketed = self
                .model
                .read(|screen| screen.terminal_modes())
                .is_some_and(|(_, bracketed)| bracketed);
            self.terminal_clip(Clip::Paste, bracketed);
        }
        self.model
            .update(|screen| *screen = std::mem::take(screen).terminal_command(command));
        if command == TerminalCommand::AddToChat {
            self.focus.focus(PROMPT);
        }
        self.deliver();
    }

    /// Opens the menu over the shell's window just under the pointer and inside the window.
    pub(super) fn open_terminal_menu(&mut self) {
        let Some((x, y)) = self.pointer.position else {
            return;
        };
        let (width, height) = self.window_size();
        let at = (
            (x - 8.0).min(width - 264.0).max(0.0),
            (y + 4.0).min(height - TERMINAL_MENU_HEIGHT - 8.0).max(0.0),
        );
        self.model
            .update(|screen| *screen = std::mem::take(screen).with_terminal_menu(at));
    }

    /// Copies what is marked in the shell showing, when the settings say to copy as soon as text is selected.
    pub(super) fn copy_terminal_selection(&mut self) {
        if !self.saved_window.terminal.copy_on_select {
            return;
        }
        if let Some(text) = self.model.read(|screen| screen.terminal_selection())
            && !text.is_empty()
            && let Some(clipboard) = self.clipboard()
        {
            let _ = clipboard.set_text(text);
        }
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

    /// The cell of the shell's window under the pointer, when the pointer is over the window.
    fn terminal_cell_under_pointer(&mut self) -> Option<(usize, usize)> {
        let at = self.pointer.position?;
        (self.output.hit(at) == Some(TERMINAL_BODY)).then_some(())?;
        self.terminal_region()
            .is_some_and(|body| body.contains(at.0, at.1))
            .then(|| self.terminal_cell(at))
            .flatten()
    }

    /// Underlines the web address under the pointer in the shell's window while Ctrl is held, and takes the underline away otherwise.
    pub(super) fn reach_terminal_link(&mut self) {
        let at = self
            .modifiers
            .control
            .then(|| self.terminal_cell_under_pointer())
            .flatten();
        let reaching = self.model.read(|screen| screen.reaching_terminal_link());
        if at.is_some() || reaching {
            self.model.update(|screen| screen.reach_terminal_link(at));
        }
    }

    /// Whether a web address of the shell is underlined as what a press opens.
    pub(super) fn reaching_terminal_link(&self) -> bool {
        self.model.read(|screen| screen.reaching_terminal_link())
    }

    /// Opens the web address under the pointer when Ctrl is held as the primary button goes down in the shell's window, or else begins marking text there.
    pub(super) fn terminal_press(&mut self) {
        self.terminal_anchor = None;
        let Some(cell) = self.terminal_cell_under_pointer() else {
            return;
        };
        let mut opened = false;
        if self.modifiers.control {
            self.model
                .update(|screen| opened = screen.open_terminal_link(cell));
        }
        if opened {
            self.deliver();
            return;
        }
        self.terminal_anchor = Some(cell);
        self.model
            .update(|screen| screen.clear_terminal_selection());
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
        let height = self.terminal_cell_size().1;
        self.terminal_scroll_rest -= pixels / height;
        let lines = self.terminal_scroll_rest.trunc();
        self.terminal_scroll_rest -= lines;
        self.model
            .update(|screen| screen.scroll_terminal(lines as isize));
        true
    }
}
