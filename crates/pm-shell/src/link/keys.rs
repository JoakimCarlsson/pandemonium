//! The window's keys: chords pressed anywhere in the window looked up in the
//! keymap the settings make, and the actions that work everywhere done on
//! the link.

use pm_editor::{Action, Chord, Pressed};

use super::{Ask, ConversationCommand, Link};
use crate::dock_model::DebugCommand;
use crate::git::GitAction;
use crate::model::{DockTab, RightTab};
use crate::page::Page;
use crate::quick::{Choosing, QuickAction, QuickOutcome};

/// What pressing a chord in the window came to.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct KeyOutcome {
    /// Whether the keymap took the chord, so nothing else should.
    pub taken: bool,
    /// What the action asks of the window beyond the link.
    pub outcome: Option<QuickOutcome>,
}

impl Link {
    /// Takes the chord written as `written`, such as `ctrl+shift+p`, pressed anywhere in the window: does the action the keymap binds it to, after the chords pressed before it, when that action works everywhere and the link can do it.
    ///
    /// A chord that starts a sequence of the window's is remembered, but is still left to the focused editor when it also starts or is one of the editor's own bindings, such as `ctrl+k` before `ctrl+k ctrl+i`, so the editor's sequences keep working.
    pub fn press_key(&mut self, written: &str) -> KeyOutcome {
        let Some(chord) = Chord::parse(written) else {
            return KeyOutcome::default();
        };
        let keymap = self.settings.editor.keymap();
        let before = std::mem::take(&mut self.chords);
        match keymap.press_among(&before, &chord, Action::everywhere) {
            Pressed::Waiting => {
                let shared = keymap.press_among(&before, &chord, |action| !action.everywhere())
                    != Pressed::Nothing;
                self.chords = before;
                self.chords.push(chord);
                KeyOutcome {
                    taken: !shared,
                    outcome: None,
                }
            }
            Pressed::Action(action) => match self.run_action(action) {
                Some(outcome) => KeyOutcome {
                    taken: true,
                    outcome,
                },
                None => KeyOutcome::default(),
            },
            Pressed::Nothing => KeyOutcome::default(),
        }
    }

    /// Does `action`, giving back what it asks of the window beyond the link, or `None` when the link cannot do it.
    pub fn run_action(&mut self, action: Action) -> Option<Option<QuickOutcome>> {
        let quick = |text: &str| QuickAction::Open(text.to_owned());
        match action {
            Action::CommandPalette => return Some(self.quick(quick(">"))),
            Action::FileFinder => return Some(self.quick(quick(""))),
            Action::ProjectSymbols => return Some(self.quick(quick("#"))),
            Action::Outline => return Some(self.quick(quick("@"))),
            Action::GoToSession => return Some(self.quick(QuickAction::Choose(Choosing::Session))),
            Action::ChangeTheme => return Some(self.quick(QuickAction::Choose(Choosing::Theme))),
            Action::ChangeKeymap => return Some(self.quick(QuickAction::Choose(Choosing::Keymap))),
            Action::Problems => return Some(self.quick(QuickAction::Choose(Choosing::Problems))),
            Action::CheckoutBranch => {
                return Some(self.quick(QuickAction::Choose(Choosing::Branch)));
            }
            Action::RewindConversation => {
                return Some(self.quick(QuickAction::Choose(Choosing::Rewind)));
            }
            Action::OpenSettings => return Some(Some(QuickOutcome::Settings)),
            Action::NewSession => {
                let at = self
                    .workspace
                    .open
                    .as_ref()
                    .and_then(|open| open.first().copied())
                    .unwrap_or_default();
                self.new_session(vec![at]);
            }
            Action::NewChat => self.new_chat(),
            Action::ShowHistory => self.navigate(Page::History),
            Action::ShowExtensions => self.navigate(Page::Extensions),
            Action::WorkspaceBack => self.back(),
            Action::WorkspaceForward => self.forward(),
            Action::ToggleTerminal => self.workspace.dock_open = !self.workspace.dock_open,
            Action::NewTerminal => {
                self.workspace.dock_open = true;
                self.workspace.dock_tab = DockTab::Terminal;
                self.new_terminal();
            }
            Action::RunTask => return Some(self.quick(QuickAction::Choose(Choosing::Task))),
            Action::StartDebugging => {
                return Some(self.quick(QuickAction::Choose(Choosing::Debug)));
            }
            Action::SelectToolchain => {
                return Some(self.quick(QuickAction::Choose(Choosing::Toolchain)));
            }
            Action::RerunTask => self.show_dock(DockTab::Tasks),
            Action::AttachDebugger => self.show_dock(DockTab::Debug),
            Action::StopDebugging => self.debug_command(DebugCommand::Stop),
            Action::PauseDebugging => self.debug_command(DebugCommand::Pause),
            Action::StepOver => self.debug_command(DebugCommand::StepOver),
            Action::StepInto => self.debug_command(DebugCommand::StepIn),
            Action::StepOut => self.debug_command(DebugCommand::StepOut),
            Action::ToggleSourceControl => self.show_right(RightTab::Git),
            Action::SearchProject => self.show_right(RightTab::Search),
            Action::OpenBrowser => self.open_browser(None),
            Action::StopAgent => {
                let session = self.watched?;
                self.ask(pm_protocol::Request::Cancel { session }, Ask::Nothing);
            }
            Action::RetryReply => self.conversation_command(ConversationCommand::Retry),
            Action::StartFresh => self.conversation_command(ConversationCommand::Afresh),
            Action::HandOffConversation => self.conversation_command(ConversationCommand::HandOver),
            Action::AskOnTheSide => self.conversation_command(ConversationCommand::SideQuestion),
            Action::Commit => self.git(GitAction::Commit { push: false }),
            Action::CommitAndPush => self.git(GitAction::Commit { push: true }),
            Action::RefreshSourceControl => self.git(GitAction::Refresh),
            Action::StageAll => self.git(GitAction::StageAll(true)),
            Action::UnstageAll => self.git(GitAction::StageAll(false)),
            Action::Fetch => self.git(GitAction::Sync("fetch".to_owned())),
            Action::Pull => self.git(GitAction::Sync("pull".to_owned())),
            Action::PullRebase => self.git(GitAction::Sync("pull_rebase".to_owned())),
            Action::Push => self.git(GitAction::Sync("push".to_owned())),
            Action::ForcePush => self.git(GitAction::Sync("force_push".to_owned())),
            Action::Sync => self.git(GitAction::Sync("sync".to_owned())),
            _ => return None,
        }
        Some(None)
    }

    /// Opens the dock on `tab`.
    fn show_dock(&mut self, tab: DockTab) {
        self.workspace.dock_open = true;
        self.workspace.dock_tab = tab;
    }

    /// Opens the right panel on `tab`.
    fn show_right(&mut self, tab: RightTab) {
        self.workspace.right_open = true;
        self.choose_right_tab(tab);
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;

    /// The keymap's chords for actions that work everywhere do them, and a chord bound to nothing is left to the rest of the window.
    #[test]
    fn keymap_chords_run_their_actions() {
        let mut link = Link::built(None, None, Arc::new(|| {}));
        link.workspace.dock_open = false;
        let toggled = link.press_key("ctrl+`");
        assert!(toggled.taken);
        assert!(link.workspace.dock_open);
        let palette = link.press_key("ctrl+shift+p");
        assert!(palette.taken);
        assert!(link.workspace.quick.is_some());
        assert!(!link.press_key("ctrl+alt+shift+f9").taken);
    }

    /// The editor's own keys are left to the editor: a chord that starts both a window sequence and an editor sequence is not taken, the editor's sequence is not finished by the window, and editor actions the link cannot do pass through.
    #[test]
    fn editor_keys_pass_through() {
        let mut link = Link::built(None, None, Arc::new(|| {}));
        assert!(!link.press_key("ctrl+k").taken);
        assert!(!link.press_key("ctrl+i").taken);
        for chord in [
            "ctrl+space",
            "f12",
            "f2",
            "ctrl+\\",
            "ctrl+.",
            "ctrl+shift+space",
        ] {
            assert!(!link.press_key(chord).taken, "{chord} was taken");
        }
    }

    /// A chord the reader bound an action to in the settings does it instead of the keymap's own.
    #[test]
    fn own_bindings_win() {
        let mut link = Link::built(None, None, Arc::new(|| {}));
        link.settings
            .editor
            .keys
            .insert("toggle_terminal".to_owned(), vec!["ctrl+alt+j".to_owned()]);
        link.workspace.dock_open = false;
        assert!(!link.press_key("ctrl+`").taken);
        assert!(link.press_key("ctrl+alt+j").taken);
        assert!(link.workspace.dock_open);
    }
}
