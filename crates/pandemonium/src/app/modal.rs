//! What a keypress does to a file while modal editing is on.
//!
//! The keys go to [`pm_vim::Vim`] before the keymap sees them, because in
//! normal mode a plain letter is a command rather than a character, and the
//! window's own chords would otherwise take the ones vim gives a meaning.
//! Whatever vim does not take goes on to the keymap and the editor as it
//! would without modal editing, which is how insert mode types; whatever
//! vim asks of the window comes back as an effect and is carried out here.

use pm_text::Position;
use pm_vim::{Command, Effect, Mode, Placement, View};
use winit::event::KeyEvent;

use crate::app::App;
use crate::desktop;
use crate::editor;
use crate::keymap::Action;

/// The column `gq` wraps at when the reader has set no guide.
const WRAP_COLUMN: usize = 80;

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
        if !self.preferences.vim_mode
            || self.search_focused
            || self.writing.is_some()
            || self.picker.is_some()
            || self.tree_edit.is_some()
        {
            return false;
        }
        if self.is_window_chord() || !self.resolver.pending().is_empty() {
            return false;
        }
        let Some(file) = self.focused_file() else {
            return false;
        };
        let Some(key) = editor::keystroke(event, self.modifiers) else {
            return false;
        };
        let (top, rows, folds) = {
            let document = file.borrow();
            (
                document.scroll(),
                document.rows(),
                document.folds().to_vec(),
            )
        };
        let view = View {
            top,
            rows,
            margin: editor::SCROLL_MARGIN,
            wrap: self.preferences.display.wrap_guide.unwrap_or(WRAP_COLUMN),
            folds: &folds,
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

    /// Puts the reader's own vim bindings on top of Zed's, as the
    /// preferences write them; one that cannot be read is left out.
    pub(super) fn bind_vim_keys(&mut self) {
        self.vim.reset_bindings();
        for binding in &self.preferences.vim_bindings {
            let _ = self.vim.bind(&binding.keys, &binding.action, &binding.when);
        }
    }

    /// The matches of the search modal editing lights in `file` on `lines`.
    pub(super) fn vim_matches(
        &self,
        file: &editor::OpenFile,
        lines: std::ops::Range<usize>,
    ) -> Vec<std::ops::Range<Position>> {
        if !self.preferences.vim_mode {
            return Vec::new();
        }
        let document = file.borrow();
        self.vim.matches(document.modal(), document.buffer(), lines)
    }

    /// Does what modal editing asked of the window for `file`.
    fn carry_out(&mut self, file: &editor::OpenFile, effect: Effect) {
        match effect {
            Effect::Command(command) => self.act(action_for(command)),
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
            Effect::ScrollColumns(columns) => file.borrow_mut().scroll_columns(columns),
            Effect::Jumped(position) => {
                if let Some(mut from) = self.here() {
                    from.position = position;
                    self.trail.jumped(from);
                }
            }
        }
    }
}

/// The window's action a command modal editing asks for stands for.
fn action_for(command: Command) -> Action {
    match command {
        Command::Save => Action::Save,
        Command::SaveAll => Action::SaveAll,
        Command::Close => Action::ClosePane,
        Command::SplitRight => Action::SplitRight,
        Command::SplitDown => Action::SplitDown,
        Command::FocusLeft => Action::FocusLeft,
        Command::FocusRight => Action::FocusRight,
        Command::FocusUp => Action::FocusUp,
        Command::FocusDown => Action::FocusDown,
        Command::NextTab => Action::NextTab,
        Command::PreviousTab => Action::PreviousTab,
        Command::ReopenTab => Action::ReopenTab,
        Command::GoBack => Action::GoBack,
        Command::GoForward => Action::GoForward,
        Command::GoToDefinition => Action::GoToDefinition,
        Command::GoToTypeDefinition => Action::GoToTypeDefinition,
        Command::GoToImplementation => Action::GoToImplementation,
        Command::GoToDeclaration => Action::GoToDeclaration,
        Command::FindReferences => Action::FindReferences,
        Command::Hover => Action::ShowHover,
        Command::Rename => Action::Rename,
        Command::CodeActions => Action::ShowCodeActions,
        Command::ShowCompletions => Action::ShowCompletions,
        Command::ShowSignature => Action::ShowSignature,
        Command::NextDiagnostic => Action::NextDiagnostic,
        Command::PreviousDiagnostic => Action::PreviousDiagnostic,
        Command::NextHunk => Action::NextChange,
        Command::PreviousHunk => Action::PreviousChange,
        Command::ToggleFold => Action::ToggleFold,
        Command::FoldAll => Action::FoldAll,
        Command::UnfoldAll => Action::UnfoldAll,
        Command::ShowSymbols => Action::ShowSymbols,
        Command::ShowWorkspaceSymbols => Action::ShowWorkspaceSymbols,
        Command::ShowFiles => Action::ShowFiles,
        Command::SearchProject => Action::SearchProject,
        Command::MoveLineUp => Action::MoveLineUp,
        Command::MoveLineDown => Action::MoveLineDown,
        Command::SelectAllMatches => Action::SelectAllMatches,
        Command::SelectNext => Action::AddNextMatch,
        Command::Cancel => Action::Cancel,
    }
}
