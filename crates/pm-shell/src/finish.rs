//! What the window tells the reader when an agent finishes its turn, decided
//! from the settings without telling anyone, so it can be tested.

use pm_config::{AgentSettings, Finish, Notification};

/// How and what the window tells the reader about a finished turn.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FinishNotice {
    /// How the reader is told: a sound or a notification of the desktop's own.
    pub how: Notification,
    /// The notification's title: the session's.
    pub title: String,
    /// The line under the title.
    pub body: String,
}

/// What the window tells the reader once the agent of the session titled
/// `title` finished its turn, the way `settings` say; `None` when they say
/// to tell nothing.
pub fn finish_notice(settings: &AgentSettings, title: &str) -> Option<FinishNotice> {
    if settings.on_finish == Finish::Nothing || settings.notification == Notification::None {
        return None;
    }
    Some(FinishNotice {
        how: settings.notification,
        title: title.to_owned(),
        body: "The agent finished its turn.".to_owned(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The settings as given, with `on_finish` and `notification` chosen.
    fn settings(on_finish: Finish, notification: Notification) -> AgentSettings {
        AgentSettings {
            on_finish,
            notification,
        }
    }

    /// A finished turn is told the way the notification setting says while finishing notifies.
    #[test]
    fn finished_turns_notify_as_set() {
        for how in [Notification::Desktop, Notification::Sound] {
            for finish in [Finish::Notify, Finish::OpenReview] {
                let notice = finish_notice(&settings(finish, how), "Fix the parser");
                assert_eq!(
                    notice,
                    Some(FinishNotice {
                        how,
                        title: "Fix the parser".to_owned(),
                        body: "The agent finished its turn.".to_owned(),
                    })
                );
            }
        }
    }

    /// Nothing is told when finishing does nothing or the notification is turned off.
    #[test]
    fn quiet_settings_tell_nothing() {
        assert_eq!(
            finish_notice(&settings(Finish::Nothing, Notification::Desktop), "a"),
            None
        );
        assert_eq!(
            finish_notice(&settings(Finish::Notify, Notification::None), "a"),
            None
        );
    }
}
