//! The menu that opens over the window of a shell: putting what is marked into the chat, copying, pasting, marking everything, clearing and ending the shell, each with the keys the keymap gives it.

use crate::areas::{ComposerEvent, MenuEvent};
use crate::target::Target;
use pm_editor::{Action, Keymap};

use crate::Menu;
use crate::state::{Outgoing, WorkspaceState};

/// What a row of the menu over a shell does.
#[derive(
    Clone, Copy, Debug, serde::Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, serde::Serialize,
)]
pub enum TerminalCommand {
    /// Puts the marked text into the prompt of the chat.
    AddToChat,
    /// Copies the marked text.
    Copy,
    /// Pastes the clipboard into the shell.
    Paste,
    /// Marks every line of the shell.
    SelectAll,
    /// Clears the window and its history.
    Clear,
    /// Ends the shell.
    Kill,
}

impl TerminalCommand {
    /// Every command, in the order of their rows' names.
    const ALL: [Self; 6] = [
        Self::AddToChat,
        Self::Copy,
        Self::Paste,
        Self::SelectAll,
        Self::Clear,
        Self::Kill,
    ];

    /// The name of the command's row.
    pub fn id(self) -> Target {
        Target::Menu(MenuEvent::TerminalMenu(self))
    }

    /// The command whose row `id` names.
    pub fn of(id: Target) -> Option<Self> {
        match id {
            Target::Menu(MenuEvent::TerminalMenu(command)) => Some(command),
            _ => None,
        }
    }

    /// The action of the keymap that does the same.
    pub fn action(self) -> Action {
        match self {
            Self::AddToChat => Action::TerminalAddToChat,
            Self::Copy => Action::TerminalCopy,
            Self::Paste => Action::TerminalPaste,
            Self::SelectAll => Action::TerminalSelectAll,
            Self::Clear => Action::TerminalClear,
            Self::Kill => Action::KillTerminal,
        }
    }

    /// The command the keymap's `action` asks for, when it is one of the shell's.
    pub fn of_action(action: Action) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|command| command.action() == action)
    }
}

/// The menu open over the window of the shell showing, and where.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TerminalMenu {
    /// Where it opened, in logical pixels.
    pub at: (f32, f32),
    /// Whether any of the shell's text is marked, so there is something to copy.
    pub selected: bool,
}

impl WorkspaceState {
    /// The state with the menu over the shell showing open at `at`, and the other menus closed.
    pub fn with_terminal_menu(self, at: (f32, f32)) -> Self {
        if self.active_terminal().is_none() {
            return self;
        }
        let selected = self
            .terminal_selection()
            .is_some_and(|text| !text.is_empty());
        let mut state = self.without_menu();
        state.terminal_menu = Some(TerminalMenu { at, selected });
        state
    }

    /// Carries out `command` on the shell showing and closes the menu over it; pasting is the caller's, which holds the clipboard.
    pub fn terminal_command(mut self, command: TerminalCommand) -> Self {
        self.terminal_menu = None;
        match command {
            TerminalCommand::AddToChat => {
                if let Some(text) = self.terminal_selection().filter(|text| !text.is_empty()) {
                    return self.typed(
                        Target::Composer(ComposerEvent::Prompt),
                        &format!("```\n{text}\n```\n"),
                    );
                }
            }
            TerminalCommand::Copy => {
                if let Some(text) = self.terminal_selection().filter(|text| !text.is_empty()) {
                    self.outbox.push(Outgoing::Copy { text });
                }
            }
            TerminalCommand::Paste => {}
            TerminalCommand::SelectAll => {
                if let Some(tab) = self.active_terminal() {
                    tab.emulator.write().select_all();
                }
            }
            TerminalCommand::Clear => {
                if let Some(tab) = self.active_terminal_mut() {
                    tab.emulator.write().clear();
                    tab.back = 0;
                }
            }
            TerminalCommand::Kill => {
                if let Some(index) = self.docked().map(|session| session.terminal_active) {
                    self.close_terminal_tab(index);
                }
            }
        }
        self
    }
}

/// Emits the menu over a shell opened at `open`, its rows that act on marked text enabled only when there is some, each showing the keys `keymap` gives it.
#[track_caller]
pub fn terminal_menu(open: TerminalMenu, keymap: &Keymap) {
    let row = |menu: Menu, command: TerminalCommand, on: bool| {
        let keys = keymap.label(command.action());
        menu.item(command.action().title(), keys.as_deref(), command.id(), !on)
    };
    let operations = crate::events::operations();
    let menu = Menu::new(open.at.0, open.at.1)
        .offset(-8.0, 4.0)
        .width(64.0)
        .row_height(6.5)
        .on_pick(move |row| {
            if let Some(command) = TerminalCommand::of(row) {
                operations.terminal_command(command);
            }
        });
    let menu = row(menu, TerminalCommand::AddToChat, open.selected).separator();
    let menu = row(menu, TerminalCommand::Copy, open.selected);
    let menu = row(menu, TerminalCommand::Paste, true);
    let menu = row(menu, TerminalCommand::SelectAll, true).separator();
    let menu = row(menu, TerminalCommand::Clear, true).separator();
    row(menu, TerminalCommand::Kill, true).build();
}
