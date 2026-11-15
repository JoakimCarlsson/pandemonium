//! The window's keys: chords pressed anywhere in the window looked up in the
//! keymap the settings make, and the actions that work everywhere done on
//! the link.

use pm_editor::{Action, Chord, Pressed};

use pm_protocol::Request;

use super::{Ask, ConversationCommand, Link, SessionCommand};
use crate::browser_action::BrowserAction;
use crate::composer::{ComposerAction, ComposerMenu, ConversationSetting};
use crate::conversation::ApprovalChoice;
use crate::dock_model::DebugCommand;
use crate::file_actions::FileAction;
use crate::git::{GitAction, GitSection};
use crate::model::{DockTab, RightTab};
use crate::open_files::{Pane, SessionTab, TabAction};
use crate::page::Page;
use crate::projects::{AddStep, ProjectAction};
use crate::quick::{Choosing, QuickAction, QuickOutcome};

/// The smallest the interface's text may be zoomed to, in points.
const ZOOM_SMALLEST: i32 = 10;

/// The largest the interface's text may be zoomed to, in points.
const ZOOM_LARGEST: i32 = 20;

/// Which answer to a permission a key gives.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Verdict {
    /// Allow this once.
    Allow,
    /// Allow this and the like from now on.
    Always,
    /// Refuse.
    Reject,
}

/// What pressing a chord in the window came to.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct KeyOutcome {
    /// Whether the keymap took the chord, so nothing else should.
    pub taken: bool,
    /// What the action asks of the window beyond the link.
    pub outcome: Option<QuickOutcome>,
}

impl Link {
    /// Takes the chord written as `written`, such as `ctrl+shift+p`, pressed anywhere in the window, or `escape`, which closes what is on top of the window: does the action the keymap binds it to, after the chords pressed before it, when that action works everywhere and the link can do it.
    ///
    /// A chord that starts a sequence of the window's is remembered, but is still left to the focused editor when it also starts or is one of the editor's own bindings, such as `ctrl+k` before `ctrl+k ctrl+i`, so the editor's sequences keep working.
    pub fn press_key(&mut self, written: &str) -> KeyOutcome {
        if written == "escape" {
            self.chords.clear();
            return KeyOutcome {
                taken: self.escape(),
                outcome: None,
            };
        }
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
        if let Some(outcome) = self.window_action(action) {
            return Some(outcome);
        }
        if let Some(outcome) = self.file_key(action) {
            return Some(outcome);
        }
        if self.session_action(action) || self.tab_action(action) || self.git_key(action) {
            return Some(None);
        }
        None
    }

    /// Does `action` when it is about the window, its pages and panels, giving back what it asks of the window beyond the link, or `None` when it is about something else.
    fn window_action(&mut self, action: Action) -> Option<Option<QuickOutcome>> {
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
            Action::CheckoutBranch | Action::CreateBranch => {
                return Some(self.quick(QuickAction::Choose(Choosing::Branch)));
            }
            Action::OpenSettings => return Some(Some(QuickOutcome::Settings)),
            Action::Quit => return Some(Some(QuickOutcome::Quit)),
            Action::ToggleTheme => return Some(Some(QuickOutcome::ToggleTheme)),
            Action::ZoomIn => return Some(self.zoom(Some(1))),
            Action::ZoomOut => return Some(self.zoom(Some(-1))),
            Action::ResetZoom => return Some(self.zoom(None)),
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
            Action::AddProject => self.project_action(ProjectAction::Adding(Some(AddStep::Choose))),
            Action::CloneRepository => {
                self.project_action(ProjectAction::Adding(Some(AddStep::Clone)))
            }
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
            Action::ToggleBrowserTools => {
                self.browser(BrowserAction::ToggleTools);
            }
            Action::CopySessionPath => {
                let session = self.watched?;
                return Some(self.session_command(SessionCommand::CopyPath(session)));
            }
            Action::CopyBranchName => {
                let session = self.watched?;
                return Some(self.session_command(SessionCommand::CopyBranch(session)));
            }
            Action::OpenSessionFolder => {
                let session = self.watched?;
                return Some(self.session_command(SessionCommand::OpenFolder(session)));
            }
            Action::CopyPath | Action::CopyRelativePath => {
                let file = self.workspace.open_files.shown()?;
                let path = if action == Action::CopyPath {
                    format!("{}/{}", file.directory.trim_end_matches('/'), file.path)
                } else {
                    file.path.clone()
                };
                return Some(Some(QuickOutcome::Copy(path)));
            }
            _ => return None,
        }
        Some(None)
    }

    /// Does `action` when it is about the open session, its agent or its conversation, giving back whether it was.
    fn session_action(&mut self, action: Action) -> bool {
        let session = self.watched;
        match action {
            Action::StopAgent => {
                if let Some(session) = session {
                    self.ask(pm_protocol::Request::Cancel { session }, Ask::Nothing);
                }
            }
            Action::RetryReply => self.conversation_command(ConversationCommand::Retry),
            Action::StartFresh => self.conversation_command(ConversationCommand::Afresh),
            Action::HandOffConversation => {
                self.conversation_command(ConversationCommand::HandOver);
                self.focus_prompt();
            }
            Action::AskOnTheSide => {
                self.conversation_command(ConversationCommand::SideQuestion);
                self.focus_prompt();
            }
            Action::RewindConversation => {
                let _ = self.quick(QuickAction::Choose(Choosing::Rewind));
            }
            Action::ForkConversation => {
                let _ = self.quick(QuickAction::Choose(Choosing::Fork));
            }
            Action::RestoreFiles => {
                let _ = self.quick(QuickAction::Choose(Choosing::Restore));
            }
            Action::FocusPrompt => self.focus_prompt(),
            Action::ShowChat => self.file_tab(TabAction::Select(SessionTab::Chat)),
            Action::ShowDiff => self.file_tab(TabAction::Select(SessionTab::Diff)),
            Action::ChangeModel => self.open_setting_menu(|setting| setting.category == "model"),
            Action::ChangeMode => self.open_setting_menu(|setting| setting.category == "mode"),
            Action::ChangeAgentSetting => self.open_setting_menu(|_| true),
            Action::CycleMode => self.cycle_mode(),
            Action::AllowRequest => self.answer_approval(Verdict::Allow),
            Action::AlwaysAllowRequest => self.answer_approval(Verdict::Always),
            Action::RejectRequest => self.answer_approval(Verdict::Reject),
            Action::RenameSession => {
                if let Some(session) = session {
                    self.session_command(SessionCommand::Rename(session));
                }
            }
            Action::ArchiveSession => {
                if let Some(session) = session {
                    self.session_command(SessionCommand::Close(session));
                }
            }
            _ => return false,
        }
        true
    }

    /// Does `action` when it is about the tabs and panes of the session view, giving back whether it was.
    fn tab_action(&mut self, action: Action) -> bool {
        let shown = match self.workspace.open_files.tab {
            SessionTab::File(index) => Some(index),
            _ => None,
        };
        match action {
            Action::CloseTab => match shown {
                Some(index) => self.file_tab(TabAction::Close(index)),
                None => self.file_tab(TabAction::Dismiss(self.workspace.open_files.tab)),
            },
            Action::CloseOtherTabs => {
                if let Some(index) = shown {
                    self.file_tab(TabAction::CloseOthers(index));
                }
            }
            Action::CloseSavedTabs => {
                let saved: Vec<usize> = (0..self.workspace.open_files.files.len())
                    .rev()
                    .filter(|index| !self.workspace.open_files.files[*index].is_dirty())
                    .collect();
                for index in saved {
                    self.file_tab(TabAction::Close(index));
                }
            }
            Action::CloseAllTabs => self.file_tab(TabAction::CloseAll),
            Action::NextTab => self.file_tab(TabAction::Next),
            Action::PreviousTab => self.file_tab(TabAction::Previous),
            Action::ReopenTab => self.file_tab(TabAction::Reopen),
            Action::PinTab => {
                if let Some(index) = shown {
                    self.file_tab(TabAction::Keep(index));
                }
            }
            Action::RevealInProject => {
                if let Some(index) = shown {
                    self.file_action(index, FileAction::Reveal);
                }
            }
            Action::SplitRight => self.file_tab(TabAction::Split),
            Action::SplitLeft => self.file_tab(TabAction::SplitLeft),
            Action::SplitDown => self.file_tab(TabAction::SplitBelow),
            Action::SplitUp => self.file_tab(TabAction::SplitAbove),
            Action::FocusNextPane | Action::FocusPreviousPane => {
                let other = match self.workspace.open_files.focus {
                    Pane::Left => Pane::Right,
                    Pane::Right => Pane::Left,
                };
                if self.workspace.open_files.split.is_some() {
                    self.file_tab(TabAction::Focus(other));
                }
            }
            _ => return false,
        }
        true
    }

    /// Does `action` when it is about the open session's repository, giving back whether it was.
    fn git_key(&mut self, action: Action) -> bool {
        let directory = self.workspace.git.folder.clone();
        let step = |action: &str| Request::Step {
            directory: directory.clone(),
            action: action.to_owned(),
            message: None,
        };
        let stash = |action: &str, index: Option<usize>| Request::Stash {
            directory: directory.clone(),
            action: action.to_owned(),
            index,
            message: None,
        };
        match action {
            Action::Commit => self.git(GitAction::Commit { push: false }),
            Action::CommitAndPush => self.git(GitAction::Commit { push: true }),
            Action::CommitAmend => self.git_commit(true, false),
            Action::RefreshSourceControl => self.git(GitAction::Refresh),
            Action::StageAll => self.git(GitAction::StageAll(true)),
            Action::UnstageAll => self.git(GitAction::StageAll(false)),
            Action::Fetch => self.git(GitAction::Sync("fetch".to_owned())),
            Action::Pull => self.git(GitAction::Sync("pull".to_owned())),
            Action::PullRebase => self.git(GitAction::Sync("pull_rebase".to_owned())),
            Action::Push => self.git(GitAction::Sync("push".to_owned())),
            Action::ForcePush => self.git(GitAction::Sync("force_push".to_owned())),
            Action::Sync => self.git(GitAction::Sync("sync".to_owned())),
            Action::ContinueOperation => self.git_act(step("continue"), None),
            Action::SkipCommit => self.git_act(step("skip"), None),
            Action::AbortOperation => self.git_act(step("abort"), None),
            Action::StashChanges => self.git_act(stash("push", None), None),
            Action::StashApply => self.open_choices(Choosing::StashApply),
            Action::StashPop => self.open_choices(Choosing::StashPop),
            Action::StashDrop => self.open_choices(Choosing::StashDrop),
            Action::ToggleGraph => {
                self.show_right(RightTab::Git);
                self.git(GitAction::Fold(GitSection::Graph));
            }
            _ => return false,
        }
        true
    }

    /// Makes the interface's text `steps` points bigger, smaller when negative, or its usual size when nothing, within the sizes the settings allow, and asks the window to save the settings.
    fn zoom(&mut self, steps: Option<i32>) -> Option<QuickOutcome> {
        let appearance = &mut self.settings.window.appearance;
        appearance.interface_size = match steps {
            Some(steps) => {
                (appearance.interface_size as i32 + steps).clamp(ZOOM_SMALLEST, ZOOM_LARGEST) as u32
            }
            None => pm_config::AppearanceSettings::default().interface_size,
        };
        Some(QuickOutcome::SettingsChanged)
    }

    /// Asks the composer of the open session to take the keyboard.
    fn focus_prompt(&mut self) {
        self.workspace.extras.composer.menu = None;
        self.workspace.extras.composer.asked += 1;
    }

    /// Opens the composer's picker of the first agent setting `wanted` accepts.
    fn open_setting_menu(&mut self, wanted: impl Fn(&ConversationSetting) -> bool) {
        let composer = &mut self.workspace.extras.composer;
        let Some(id) = composer
            .settings
            .iter()
            .find(|setting| wanted(setting))
            .map(|setting| setting.id.clone())
        else {
            return;
        };
        composer.menu = Some(ComposerMenu::Setting(id));
        composer.asked += 1;
    }

    /// Gives the agent's mode the value after the one it has, round to the first.
    fn cycle_mode(&mut self) {
        let Some(setting) = self
            .workspace
            .extras
            .composer
            .settings
            .iter()
            .find(|setting| setting.category == "mode")
        else {
            return;
        };
        let place = setting
            .picks
            .iter()
            .position(|pick| pick.value == setting.current);
        let next = place.map_or(0, |place| (place + 1) % setting.picks.len().max(1));
        let Some(pick) = setting.picks.get(next) else {
            return;
        };
        let change = ComposerAction::Set(setting.id.clone(), pick.value.clone());
        self.composer_action(change);
    }

    /// Answers the permission the open session's agent waits on with the choice that gives `verdict`.
    fn answer_approval(&mut self, verdict: Verdict) {
        let Some(approval) = &self.workspace.extras.approval else {
            return;
        };
        let always = |choice: &&ApprovalChoice| {
            let words = format!("{} {}", choice.id, choice.name).to_lowercase();
            words.contains("always")
        };
        let choice = match verdict {
            Verdict::Allow => approval
                .choices
                .iter()
                .find(|choice| !choice.refuses && !always(choice))
                .or_else(|| approval.choices.iter().find(|choice| !choice.refuses)),
            Verdict::Always => approval
                .choices
                .iter()
                .find(|choice| !choice.refuses && always(choice)),
            Verdict::Reject => approval.choices.iter().find(|choice| choice.refuses),
        };
        if let Some(choice) = choice {
            let (ticket, option) = (approval.ticket, choice.id.clone());
            self.choose(ticket, option);
        }
    }

    /// Closes the dialog, question or menu on top of the window, giving back whether one was open.
    pub fn escape(&mut self) -> bool {
        let workspace = &mut self.workspace;
        if workspace.renaming.take().is_some()
            || workspace.closing.take().is_some()
            || workspace.session_menu.take().is_some()
            || workspace.tree.removing.take().is_some()
            || workspace.quick.take().is_some()
            || workspace.project_menu.take().is_some()
            || workspace.tree.menu.take().is_some()
            || workspace.adder.take().is_some()
            || workspace.open_files.menu.take().is_some()
            || workspace.git.menu.take().is_some()
            || workspace.git.prompt.take().is_some()
        {
            return true;
        }
        if workspace.sign_in.is_some() {
            self.hide_sign_in();
            return true;
        }
        if workspace.update.as_ref().is_some_and(|offer| offer.open) {
            self.show_update(false);
            return true;
        }
        false
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
        for chord in ["ctrl+space", "f12", "f2", "ctrl+.", "ctrl+shift+space"] {
            assert!(!link.press_key(chord).taken, "{chord} was taken");
        }
    }

    /// The actions the window does beyond the link come back to it, the zoom changes the interface's size, and Escape closes the dialog on top.
    #[test]
    fn window_actions_are_done() {
        let mut link = Link::built(None, None, Arc::new(|| {}));
        assert_eq!(
            link.run_action(Action::Quit),
            Some(Some(QuickOutcome::Quit))
        );
        assert_eq!(
            link.run_action(Action::ToggleTheme),
            Some(Some(QuickOutcome::ToggleTheme))
        );
        let before = link.settings.window.appearance.interface_size;
        assert_eq!(
            link.run_action(Action::ZoomIn),
            Some(Some(QuickOutcome::SettingsChanged))
        );
        assert_eq!(link.settings.window.appearance.interface_size, before + 1);
        link.run_action(Action::ResetZoom);
        assert_eq!(link.settings.window.appearance.interface_size, before);
        assert!(link.run_action(Action::StashChanges).is_some());
        assert!(link.run_action(Action::CloseAllTabs).is_some());
        link.workspace.renaming = Some(crate::model::Renaming::default());
        assert!(link.press_key("escape").taken);
        assert_eq!(link.workspace.renaming, None);
        assert!(!link.press_key("escape").taken);
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
