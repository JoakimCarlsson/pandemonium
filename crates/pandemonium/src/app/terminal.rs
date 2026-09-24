//! What the pointer and the clipboard do to the bottom panel's shell.
//!
//! The pane reports presses and drags as cells of its grid; this is where a
//! press becomes a selection, a double press a word, a triple press a line
//! and a press with the link key held an address opened in the browser.

use pm_ui::ResizePhase;
use pm_vt::{Place, Unit};

use crate::app::App;
use crate::desktop;
use crate::keymap::Action;

impl App {
    /// Gives the keyboard to the bottom panel's shell.
    pub(super) fn focus_terminal(&mut self) {
        self.terminal_focused = true;
        self.editor_focused = false;
    }

    /// Selects, or follows a link, for a press or a drag over the grid.
    ///
    /// Only the press begins a gesture: it decides what the drag grows by,
    /// and a single press lets go of what was picked out before, so a click
    /// that goes nowhere leaves nothing selected. Control held on a link
    /// opens it instead, which is how every name in the editor is followed.
    pub(super) fn point_terminal(&mut self, phase: ResizePhase, anchor: Place, head: Place) {
        self.focus_terminal();
        let Some(shell) = self.focused_shell() else {
            return;
        };
        let mut shell = shell.borrow_mut();

        if phase == ResizePhase::Started {
            if self.modifiers.control_key()
                && let Some(link) = shell.link_at(head)
            {
                self.screen_clicks.clear();
                return desktop::browse(&link.target);
            }
            self.screen_unit = match self.screen_clicks.press(head) {
                2 => Unit::Word,
                3 => Unit::Line,
                _ => Unit::Cell,
            };
            return match self.screen_unit {
                Unit::Cell => shell.clear_selection(),
                unit => shell.select(anchor, head, unit),
            };
        }
        if anchor == head {
            return;
        }
        self.screen_clicks.clear();
        shell.select(anchor, head, self.screen_unit);
    }

    /// Carries a clipboard command out on the focused terminal, if one is focused.
    ///
    /// A copy lets go of the selection once it is on the clipboard, so the
    /// next Control-C is the interrupt again rather than a second copy.
    pub(super) fn act_on_terminal(&mut self, action: Action) -> bool {
        let Some(shell) = self.focused_shell() else {
            return false;
        };
        let mut shell = shell.borrow_mut();
        match action {
            Action::Copy => {
                if let Some(text) = shell.selected_text() {
                    desktop::copy(text);
                }
                shell.clear_selection();
            }
            Action::Paste => {
                if let Some(text) = desktop::paste() {
                    shell.paste(&text);
                }
            }
            Action::SelectAll => shell.select_all(),
            _ => return false,
        }
        true
    }
}
