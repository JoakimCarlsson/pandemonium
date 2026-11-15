//! The history page's rows: every session the server holds, closed or not,
//! with where it ran and when it last changed.

use pm_protocol::SessionInfo;

use crate::link::age;

/// What the machine column says for a project on this machine.
const LOCAL_MACHINE: &str = "local";

/// What the project column says for a chat that belongs to no project.
const CHAT_PROJECT: &str = "Chat";

/// The stretch of days a session last changed in, which groups the history.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub enum Span {
    /// The same day as now.
    #[default]
    Today,
    /// The day before.
    Yesterday,
    /// Any day before that.
    Earlier,
}

impl Span {
    /// Every span, newest first.
    pub const ALL: [Span; 3] = [Span::Today, Span::Yesterday, Span::Earlier];

    /// The span a change at `then` falls in when it is `now`, both in
    /// milliseconds since the epoch, by local calendar day.
    pub fn of(then: u64, now: u64) -> Self {
        match local_day(now).saturating_sub(local_day(then)) {
            0 => Self::Today,
            1 => Self::Yesterday,
            _ => Self::Earlier,
        }
    }

    /// What the heading over the span says.
    pub fn title(self) -> &'static str {
        match self {
            Self::Today => "Today",
            Self::Yesterday => "Yesterday",
            Self::Earlier => "Earlier",
        }
    }
}

/// The number of the local calendar day `millis` since the epoch falls on.
fn local_day(millis: u64) -> i64 {
    use chrono::Datelike;
    chrono::DateTime::from_timestamp_millis(millis as i64).map_or(0, |moment| {
        i64::from(
            moment
                .with_timezone(&chrono::Local)
                .date_naive()
                .num_days_from_ce(),
        )
    })
}

/// A session as the history page lists it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PastSession {
    /// The server's number for it.
    pub id: u64,
    /// Its title.
    pub title: String,
    /// The agent that runs it, by its id.
    pub agent: String,
    /// The name of the project it belongs to.
    pub project: String,
    /// The machine it runs on, `local` for this one.
    pub machine: String,
    /// The branch its worktree is on, or empty.
    pub branch: String,
    /// How long ago it last changed, such as `4d ago`.
    pub age: String,
    /// The stretch of days it last changed in.
    pub span: Span,
    /// Whether the reader closed it.
    pub closed: bool,
}

impl PastSession {
    /// The row for the session `info` describes when it is `now`, in
    /// milliseconds since the epoch.
    pub fn of(info: &SessionInfo, now: u64) -> Self {
        let project = if info.chat {
            CHAT_PROJECT.to_owned()
        } else {
            info.repository
                .trim_end_matches('/')
                .rsplit('/')
                .next()
                .unwrap_or_default()
                .to_owned()
        };
        Self {
            id: info.session,
            title: info.title.clone(),
            agent: pm_harness::agents()
                .iter()
                .find(|agent| agent.id == info.agent)
                .map_or_else(|| info.agent.clone(), |agent| agent.name.to_owned()),
            project,
            machine: pm_protocol::host_of(&info.repository)
                .map(|(host, _)| host.to_owned())
                .unwrap_or_else(|| LOCAL_MACHINE.to_owned()),
            branch: info.branch.clone().unwrap_or_default(),
            age: age(now.saturating_sub(info.updated)),
            span: Span::of(info.updated, now),
            closed: info.closed,
        }
    }

    /// Whether the row matches `needle`, already in lower case, by its
    /// title, agent, project, machine or branch.
    pub fn matches(&self, needle: &str) -> bool {
        needle.is_empty()
            || [
                &self.title,
                &self.agent,
                &self.project,
                &self.machine,
                &self.branch,
            ]
            .iter()
            .any(|field| field.to_lowercase().contains(needle))
    }
}

#[cfg(test)]
/// Tests for the history's rows.
mod tests {
    use chrono::TimeZone;

    use super::*;

    /// One day, in milliseconds.
    const DAY_MS: u64 = 86_400_000;

    /// Changes fall in today, yesterday or earlier by local calendar day.
    #[test]
    fn spans_follow_calendar_days() {
        let noon = chrono::Local::now()
            .date_naive()
            .and_hms_opt(12, 0, 0)
            .expect("noon is a time");
        let now = chrono::Local
            .from_local_datetime(&noon)
            .earliest()
            .expect("noon is on the local clock")
            .timestamp_millis() as u64;
        assert_eq!(Span::of(now - 500, now), Span::Today);
        assert_eq!(Span::of(now - DAY_MS, now), Span::Yesterday);
        assert_eq!(Span::of(now - 3 * DAY_MS, now), Span::Earlier);
    }
}
