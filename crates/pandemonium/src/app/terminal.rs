//! The terminal dock in the window: sizing it to the shell, typing into it, marking text and scrolling back.

use pm_buffer::Reach;
use pm_components::areas::{ComposerEvent, TerminalEvent};
use pm_components::pointing::{Stroke, TerminalPointer};
use pm_components::target::Target;
use pm_components::terminal::{cell_size, fitting};
use pm_components::terminal_keys;
use pm_components::terminal_menu::TerminalCommand;
use pm_editor::{Action, Pressed};

use rustle::Size;
use rustle::{Key, KeyEvent};

use super::App;
use super::selecting::{Hold, Selected};
use crate::keys::{self, Clip};

impl App {
    /// How tall the type of the shells is, in logical pixels, as the settings say.
    fn terminal_font(&self) -> f32 {
        self.model
            .read(|state| Some(pm_components::terminal::Look::of(state).font))
            .unwrap_or(13.0)
    }

    /// How wide and how tall one cell of a shell's window is, in logical pixels.
    fn terminal_cell_size(&mut self) -> (f32, f32) {
        let font = self.terminal_font();
        cell_size(font)
    }

    /// The size the window of the shell showing was laid out at, as it told it with `onSizeChanged`, if it is shown.
    fn terminal_size(&self) -> Option<Size> {
        self.model.read(|screen| {
            screen
                .places()
                .size(Target::Terminal(TerminalEvent::TerminalBody))
        })
    }

    /// Gives the shell the size of the room the dock has for it, when that differs from the size it has.
    pub(super) fn fit_terminal(&mut self) {
        let Some(body) = self.terminal_size() else {
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
        if self.focus.focused() != Some(Target::Terminal(TerminalEvent::TerminalBody)) {
            return false;
        }
        self.type_into_terminal(text.as_bytes().to_vec());
        true
    }

    /// Acts on a key when the shell holds the keyboard, and says whether it did.
    pub(super) fn terminal_key(&mut self, event: &KeyEvent) -> bool {
        if self.focus.focused() != Some(Target::Terminal(TerminalEvent::TerminalBody)) {
            return false;
        }
        let Some((application, bracketed)) = self.model.read(|screen| screen.terminal_modes())
        else {
            return false;
        };
        if self.terminal_keymap_key(event) {
            return true;
        }
        if let Some(action) = keys::terminal_clip(event) {
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
        } else if let Some(data) = terminal_keys::bytes(event, application) {
            self.type_into_terminal(data);
        }
        true
    }

    /// Takes a press of keys the keymap binds to one of the shell's actions, or that starts such a binding, and says whether it took it.
    fn terminal_keymap_key(&mut self, event: &KeyEvent) -> bool {
        let modifiers = event.modifiers;
        if !(modifiers.ctrl || modifiers.meta || modifiers.alt) && self.pending.is_empty() {
            return false;
        }
        let Some(chord) = Self::chord_of(event) else {
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
        self.model.change(|state| state.terminal_command(command));
        if command == TerminalCommand::AddToChat {
            self.focus
                .request_focus(Target::Composer(ComposerEvent::Prompt));
        } else {
            self.focus
                .request_focus(Target::Terminal(TerminalEvent::TerminalBody));
        }
        self.deliver();
    }

    /// Opens the menu over the shell's window where the pointer was, at `at` in the window's
    /// frame.
    pub(super) fn open_terminal_menu(&mut self, at: (f32, f32)) {
        self.model.change(|state| state.with_terminal_menu(at));
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
            clipboard.set_text(&text);
        }
    }

    /// Copies the text marked in the shell, or pastes the clipboard into it.
    fn terminal_clip(&mut self, action: Clip, bracketed: bool) {
        match action {
            Clip::Paste => {
                let pasted = self.clipboard().and_then(|clipboard| clipboard.text());
                if let Some(text) = pasted {
                    self.type_into_terminal(terminal_keys::paste(&text, bracketed));
                }
            }
            Clip::Copy | Clip::Cut => {
                if let Some(text) = self.model.read(|screen| screen.terminal_selection())
                    && let Some(clipboard) = self.clipboard()
                {
                    clipboard.set_text(&text);
                }
            }
        }
    }

    /// Notes the cell of the shell's window the pointer is over, or that it left the window,
    /// as the window tells, and follows the link Ctrl underlines there.
    pub(super) fn terminal_hovered(&mut self, cell: Option<(usize, usize)>) {
        self.terminal_hover = cell;
        self.reach_terminal_link();
    }

    /// Underlines the web address under the pointer in the shell's window while Ctrl is held, and takes the underline away otherwise.
    pub(super) fn reach_terminal_link(&mut self) {
        let at = self.terminal_hover.filter(|_| self.modifiers.ctrl);
        let reaching = self.model.read(|screen| screen.reaching_terminal_link());
        if at.is_some() || reaching {
            self.model.update(|screen| screen.reach_terminal_link(at));
        }
    }

    /// Carries out what the primary button did on the shell's window, as `pointer` tells: a
    /// press with Ctrl held opens the web address there, and any other begins marking text,
    /// a point, a word or a line as the presses in a row say; a move with the button held
    /// stretches what is marked; the release copies it when the settings say to.
    pub(super) fn terminal_pointer(&mut self, pointer: TerminalPointer) {
        match pointer.stroke {
            Stroke::Down { clicks, modifiers } => {
                self.terminal_hold = None;
                let mut opened = false;
                if modifiers.ctrl {
                    self.model
                        .update(|screen| opened = screen.open_terminal_link(pointer.cell));
                }
                if opened {
                    self.deliver();
                    return;
                }
                let Some(cell) = self.absolute_terminal_cell(pointer.cell) else {
                    return;
                };
                let (hold, selected) =
                    Hold::press(cell, clicks, |reach, at| self.terminal_reach(reach, at));
                self.mark_terminal(selected);
                self.terminal_hold = Some(hold);
            }
            Stroke::Move => {
                let Some(hold) = self.terminal_hold else {
                    return;
                };
                if let Some(cell) = self.absolute_terminal_cell(pointer.cell) {
                    let selected = hold.drag(cell, |reach, at| self.terminal_reach(reach, at));
                    self.mark_terminal(selected);
                }
            }
            Stroke::Up => {
                if self.terminal_hold.take().is_some() {
                    self.copy_terminal_selection();
                }
            }
        }
    }

    /// The word or line `reach` takes around the cell `at`, by line counted from the oldest kept and column.
    fn terminal_reach(&self, reach: Reach, at: (usize, usize)) -> ((usize, usize), (usize, usize)) {
        self.model
            .read(|screen| screen.terminal_reach(reach, at))
            .unwrap_or((at, at))
    }

    /// Marks what `selected` says of the shell's text, both ends included, or unmarks it.
    fn mark_terminal(&mut self, selected: Selected<(usize, usize)>) {
        match selected {
            Selected::Point(_) => self
                .model
                .update(|screen| screen.clear_terminal_selection()),
            Selected::Span(anchor, head) => self
                .model
                .update(|screen| screen.select_terminal_span(anchor, head)),
        }
    }

    /// The cell `cell` of the shell's window as a line counted from the oldest kept and a column.
    fn absolute_terminal_cell(&self, cell: (usize, usize)) -> Option<(usize, usize)> {
        let first = self.model.read(|screen| screen.terminal_first_line())?;
        Some((first + cell.0, cell.1))
    }

    /// Scrolls the shell back or forward by `pixels` of wheel turned over its window, downwards when positive.
    pub(super) fn terminal_wheel(&mut self, pixels: f32) {
        let height = self.terminal_cell_size().1;
        self.terminal_scroll_rest -= pixels / height;
        let lines = self.terminal_scroll_rest.trunc();
        self.terminal_scroll_rest -= lines;
        self.model
            .update(|screen| screen.scroll_terminal(lines as isize));
    }
}
