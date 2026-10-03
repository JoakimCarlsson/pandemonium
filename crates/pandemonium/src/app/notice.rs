//! What the window tells the reader about: turning what happened into
//! notices, and carrying out what a notice is clicked for.

use pm_core::{Said, Scope};

use crate::agent::TalkId;
use crate::app::{App, RemoteOperation};
use crate::message::Message;
use crate::panel::PanelView;
use crate::terminal::Exited;
use crate::workspace::SidebarView;

impl App {
    /// Carries out the messages a notice sends.
    ///
    /// The answer says whether the message was one of them, so that the
    /// window can go on trying the rest.
    pub(super) fn notice_command(&mut self, message: Message) -> bool {
        match message {
            Message::ScrollNotification(id, event, step) => {
                self.notices.scroll_installation(id, event, step)
            }
            Message::ActOnNotification(id, action) => {
                use crate::notice::{InstallationStage, NotificationAction};
                let Some(card) = self.notices.installation_at(id).cloned() else {
                    return true;
                };
                match action {
                    NotificationAction::Install => {
                        if matches!(
                            card.stage,
                            InstallationStage::Offer | InstallationStage::Failed
                        ) {
                            self.start_server_install(card.command, true);
                        }
                    }
                    NotificationAction::LanguageSettings => {
                        if let Some(language) = card.language {
                            self.select_settings_language(language);
                        }
                        self.settings
                            .show_section(crate::settings::SettingsSection::LanguageSettings);
                        self.open_settings();
                    }
                    NotificationAction::Preference => {
                        self.settings
                            .show_section(crate::settings::SettingsSection::Saving);
                        self.open_settings();
                    }
                    NotificationAction::Close => self.notices.dismiss_installation(id),
                    NotificationAction::Previous => self.notices.step_installation(true),
                    NotificationAction::Next => self.notices.step_installation(false),
                }
                if matches!(
                    action,
                    NotificationAction::Close
                        | NotificationAction::LanguageSettings
                        | NotificationAction::Preference
                ) && let Some(ui) = self.ui.as_mut()
                {
                    ui.clear_focus();
                }
            }
            Message::FollowNotice(id) => {
                if let Some(action) = self.notices.dismiss(id) {
                    self.apply(action);
                }
            }
            Message::DismissNotice(id) => {
                self.notices.dismiss(id);
            }
            _ => return false,
        }
        true
    }

    /// Says which agents went away on their own.
    pub(super) fn hear_ended_agents(&mut self) {
        for talk in self.agents.take_ended() {
            let Some(held) = self.agents.get(talk) else {
                continue;
            };
            let text = format!(
                "{} stopped in {}",
                held.agent().name,
                self.worktree_name(held.scope())
            );
            self.notices.trouble(text, Some(Message::ShowAgent(talk)));
        }
    }

    /// Says which shells exited badly.
    pub(super) fn hear_failed_shells(&mut self) {
        for Exited { scope, name, code } in self.terminals.take_failed() {
            let text = format!("{name} exited with {code} in {}", self.worktree_name(scope));
            self.notices
                .trouble(text, Some(Message::ShowPanelView(PanelView::Terminal)));
        }
    }

    /// Says how a remote operation in `scope` came out.
    pub(super) fn hear_remote(&mut self, scope: Scope, kind: RemoteOperation, said: &Said) {
        let changes = Some(Message::SetSidebarView(SidebarView::Changes));
        let place = self.worktree_name(scope);
        match said {
            Ok(_) => self
                .notices
                .done(format!("{} {place}", kind.done()), changes),
            Err(trouble) => {
                let first = trouble.lines().next().unwrap_or_default();
                self.notices
                    .trouble(format!("{} {place}: {first}", kind.failed()), changes);
            }
        }
    }

    /// Opens the pane the conversation `talk` is read in, wherever it is.
    pub(super) fn show_agent(&mut self, talk: TalkId) {
        let Some(scope) = self.agents.get(talk).map(crate::agent::Talk::scope) else {
            return;
        };
        self.show_item(
            self.panes.focus(),
            scope,
            crate::panes::Item::Agent(scope, talk),
            false,
        );
    }

    /// What a notice calls the worktree `scope` names: the session's name,
    /// or the project's for its own checkout.
    pub(super) fn worktree_name(&self, scope: Scope) -> String {
        scope
            .session()
            .and_then(|session| self.sessions.get(session))
            .map(|session| session.name().to_owned())
            .or_else(|| {
                self.open
                    .get(scope.project())
                    .map(|project| project.name().to_owned())
            })
            .unwrap_or_default()
    }
}
