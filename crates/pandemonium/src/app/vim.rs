//! What a keypress does to a file while modal editing is on: the key goes to vim's engine before the keymap sees it, because in normal mode a plain letter is a command rather than a character, and whatever the engine does not take goes on as it would without modal editing, which is how insert mode types.

use pm_editor::Action;
use pm_input::KeyEvent;
use pm_vim::Command;

use pm_components::git_controls::VIEWER_SCROLL;
use pm_components::open_files::{EDITOR_FIELD, STRIP};
use pm_pages::Screen;

use super::App;

/// The system clipboard, as modal editing reaches it.
struct Desktop<'a>(&'a mut Option<arboard::Clipboard>);

impl pm_vim::Clipboard for Desktop<'_> {
    /// What is on the clipboard, if it holds text.
    fn read(&mut self) -> Option<String> {
        self.0.as_mut()?.get_text().ok()
    }

    /// Puts `text` on the clipboard.
    fn write(&mut self, text: String) {
        if let Some(clipboard) = self.0.as_mut() {
            let _ = clipboard.set_text(text);
        }
    }
}

/// The window's own action a command of modal editing stands for, when there is one.
fn action_for(command: Command) -> Option<Action> {
    Some(match command {
        Command::Save | Command::SaveAll => Action::Save,
        Command::Close => Action::CloseTab,
        Command::SplitRight => Action::SplitRight,
        Command::SplitDown => Action::SplitDown,
        Command::FocusLeft | Command::FocusUp => Action::FocusPreviousPane,
        Command::FocusRight | Command::FocusDown => Action::FocusNextPane,
        Command::GoBack => Action::GoBack,
        Command::GoForward => Action::GoForward,
        Command::GoToDefinition => Action::GoToDefinition,
        Command::GoToTypeDefinition => Action::GoToTypeDefinition,
        Command::GoToImplementation => Action::GoToImplementation,
        Command::GoToDeclaration => Action::GoToDeclaration,
        Command::FindReferences => Action::FindReferences,
        Command::Hover => Action::Hover,
        Command::Rename => Action::Rename,
        Command::CodeActions => Action::CodeActions,
        Command::ShowCompletions => Action::Complete,
        Command::ShowSignature => Action::SignatureHelp,
        Command::NextDiagnostic => Action::NextProblem,
        Command::PreviousDiagnostic => Action::PreviousProblem,
        Command::ToggleFold => Action::Fold,
        Command::FoldAll => Action::FoldAll,
        Command::UnfoldAll => Action::UnfoldAll,
        Command::ShowSymbols => Action::Outline,
        Command::ShowWorkspaceSymbols => Action::ProjectSymbols,
        Command::ShowFiles => Action::FileFinder,
        Command::SearchProject => Action::SearchProject,
        Command::MoveLineUp => Action::MoveLineUp,
        Command::MoveLineDown => Action::MoveLineDown,
        Command::SelectAllMatches => Action::SelectAllOccurrences,
        Command::SelectNext => Action::AddNextOccurrence,
        Command::NextTab
        | Command::PreviousTab
        | Command::ReopenTab
        | Command::NextHunk
        | Command::PreviousHunk
        | Command::Cancel => return None,
    })
}

impl App {
    /// Sends a keypress to modal editing, when it is on and the file shown has the keyboard, and says whether it was taken.
    pub(super) fn vim_key(&mut self, event: KeyEvent) -> bool {
        if self.focus.current() != Some(EDITOR_FIELD) || !self.model.read(Screen::vim_on) {
            return false;
        }
        let Some(key) = pm_components::vim::keystroke(&event) else {
            return false;
        };
        let height = self
            .output
            .regions
            .iter()
            .find(|region| region.id == VIEWER_SCROLL)
            .map_or_else(|| self.window_height(), |region| region.rect.height + STRIP);
        self.clipboard();
        let clipboard = &mut self.clipboard;
        let mut commands = None;
        self.model.update(|screen| {
            commands = screen.vim_key(key, height, &mut Desktop(clipboard));
        });
        let Some(commands) = commands else {
            return false;
        };
        self.show_editor_caret();
        for command in commands {
            if let Some(action) = action_for(command) {
                self.perform(action);
            }
        }
        true
    }
}
