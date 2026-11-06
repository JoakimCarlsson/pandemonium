//! What an agent does, as the app sees it: the events of the harness' wire protocol and the ones the app adds as it keeps the conversation.

use serde_json::{Value, json};

pub use omc_wire::{
    Alternative, Background, Choice, Diff, Input, Inquiry, Kind, Merge, Part, Permission, Pick,
    PlanStep, Progress, Question, Reply, Setting, Skipped, Slash, Standing, Tool, ToolState,
    TrustRequest, Usage, Verdict, Worktree,
};

/// What an agent did, in the order it did it.
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum Event {
    /// What can be changed about the conversation, and how it stands now.
    Settings(Vec<Setting>),
    /// The commands the agent offers behind a slash.
    Commands(Vec<Slash>),
    /// How full the conversation's context is now.
    Used(Usage),
    /// A new block of the agent's reply starts, of the kind given; the `Said` or `Thought` chunks that follow belong to it until the next block starts.
    Started(Part),
    /// The agent said this much more of its reply.
    Said(String),
    /// The agent thought this much more.
    Thought(String),
    /// The agent started using a tool, or said more about one it was using.
    Tool(Tool),
    /// A command or task left running beyond its call started, or ended.
    Background(Background),
    /// The turn was stopped, so that nothing the agent had running goes on.
    Halted,
    /// The agent's plan, whole, as it stands now.
    Plan(Vec<PlanStep>),
    /// The agent waits for an answer to this.
    Ask(Permission),
    /// The agent waits for the answers to this form of questions.
    Inquire(Inquiry),
    /// The reader answered a form of questions: each question with what was answered, or nothing when it was skipped.
    Answered(Vec<(String, Option<String>)>),
    /// The turn ended, with the reason it failed where it did.
    Finished(Option<String>),
    /// The turn ended because the account needs signing in again.
    SignedOut {
        /// Why authentication failed.
        reason: String,
        /// The name of the session account, supplied by its server when named.
        account: Option<String>,
    },
    /// The session account was signed in again.
    SignedIn,
    /// The agent could not be started or stopped, for this reason.
    Failed(String),
    /// The dev container the agent runs in is being built or started, and this is the step it is on.
    Container(String),
    /// The agent named the conversation, or named it again.
    Titled(String),
    /// A reader handed the agent a turn: what they wrote and the names of the files they attached.
    Prompted {
        /// What the reader wrote.
        text: String,
        /// The names of the files attached.
        files: Vec<String>,
        /// Whether the files of the session's folder were kept as they were before the turn, so they can be put back.
        #[serde(default)]
        checkpoint: bool,
        /// The id the harness saves the turn under, and the name the files kept before it go by; nought for a turn from before turns had one, which goes by the number of this event in the log.
        #[serde(default)]
        turn: u64,
    },
    /// A reader wrote while the agent was on a turn, so their words wait for it, under `id`.
    Queued {
        /// The number the waiting prompt goes by within its session.
        id: u64,
        /// What the reader wrote.
        text: String,
        /// The names of the files attached.
        files: Vec<String>,
    },
    /// The waiting prompt numbered this stopped waiting: handed to the agent, or dropped.
    Unqueued(u64),
    /// The conversation went back to before the reader's turn `turn`, counted from nought, which is dropped with all that followed it.
    Rewound {
        /// The turn, by how many of the reader's turns came before it.
        turn: u64,
    },
    /// The files of the session's folder were put back as they were before the reader's turn `turn`.
    Restored {
        /// The turn, by how many of the reader's turns came before it.
        turn: u64,
        /// The files that changed, from the root of the repository.
        files: Vec<String>,
    },
    /// The harness left out what the project folder brings until the reader trusts the project, and waits for an answer under the request's ticket; the conversation goes on without those inputs meanwhile.
    Trust(TrustRequest),
    /// The trust request under `ticket` stopped waiting: the reader answered it with `verdict`, or it was dropped, with no verdict, because the conversation it belonged to ended.
    Trusted {
        /// The request, by the number the harness put it under.
        ticket: u64,
        /// What the reader decided, or nothing when the request was dropped unanswered.
        verdict: Option<Verdict>,
    },
}

/// What an agent says about itself when a conversation is opened.
#[derive(Clone, Debug, Default, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Discovery {
    /// What the reader can change about a conversation.
    pub settings: Vec<Setting>,
    /// The commands the agent offers behind a slash.
    pub commands: Vec<Slash>,
}

impl Discovery {
    /// What this comes to as text that [`Discovery::from_text`] reads back.
    pub fn to_text(&self) -> String {
        let settings: Vec<Value> = self
            .settings
            .iter()
            .map(|setting| {
                json!({
                    "id": setting.id,
                    "category": setting.category,
                    "name": setting.name,
                    "current": setting.current,
                    "picks": setting.picks.iter().map(|pick| json!({
                        "value": pick.value,
                        "name": pick.name,
                        "description": pick.description,
                    })).collect::<Vec<_>>(),
                })
            })
            .collect();
        let commands: Vec<Value> = self
            .commands
            .iter()
            .map(|command| json!({ "name": command.name, "description": command.description }))
            .collect();
        json!({ "settings": settings, "commands": commands }).to_string()
    }

    /// The discovery that `text` from [`Discovery::to_text`] holds, if it holds one.
    pub fn from_text(text: &str) -> Option<Self> {
        let held: Value = serde_json::from_str(text).ok()?;
        let string = |value: &Value, key: &str| value[key].as_str().map(str::to_owned);
        let settings = held["settings"]
            .as_array()?
            .iter()
            .filter_map(|setting| {
                Some(Setting {
                    id: string(setting, "id")?,
                    category: string(setting, "category")?,
                    name: string(setting, "name")?,
                    current: string(setting, "current")?,
                    picks: setting["picks"]
                        .as_array()?
                        .iter()
                        .filter_map(|pick| {
                            Some(Pick {
                                value: string(pick, "value")?,
                                name: string(pick, "name")?,
                                description: string(pick, "description")?,
                            })
                        })
                        .collect(),
                })
            })
            .collect();
        let commands = held["commands"]
            .as_array()?
            .iter()
            .filter_map(|command| {
                Some(Slash {
                    name: string(command, "name")?,
                    description: string(command, "description")?,
                })
            })
            .collect();
        Some(Self { settings, commands })
    }
}

impl Event {
    /// The event the app keeps for what the harness `reported`, or nothing when the app has no use for it.
    pub fn from_harness(reported: omc_wire::Event) -> Option<Self> {
        use omc_wire::Event as Wire;
        match reported {
            Wire::Settings(settings) => Some(Self::Settings(settings)),
            Wire::Commands(commands) => Some(Self::Commands(commands)),
            Wire::Used(usage) => Some(Self::Used(usage)),
            Wire::Started(part) => Some(Self::Started(part)),
            Wire::Said(text) => Some(Self::Said(text)),
            Wire::Thought(text) => Some(Self::Thought(text)),
            Wire::Tool(tool) => Some(Self::Tool(tool)),
            Wire::Background(background) => Some(Self::Background(background)),
            Wire::Halted => Some(Self::Halted),
            Wire::Plan(plan) => Some(Self::Plan(plan)),
            Wire::Ask(permission) => Some(Self::Ask(permission)),
            Wire::Inquire(inquiry) => Some(Self::Inquire(inquiry)),
            Wire::Finished(failure) => Some(Self::Finished(failure)),
            Wire::SignedOut { reason, account } => Some(Self::SignedOut { reason, account }),
            Wire::SignedIn => Some(Self::SignedIn),
            Wire::LoginUrl(_)
            | Wire::LoginCode { .. }
            | Wire::Resumed(_)
            | Wire::Language(_)
            | Wire::Queued(_) => None,
            Wire::Trust(request) => Some(Self::Trust(request)),
            Wire::Failed(reason) => Some(Self::Failed(reason)),
            Wire::Titled(title) => Some(Self::Titled(title)),
        }
    }
}

#[cfg(test)]
/// Tests for how the harness's events become the app's.
mod tests {
    use std::path::PathBuf;

    use super::*;

    /// A trust request the harness sends as a line of its wire protocol becomes the app's trust event whole, and survives the trip through the session log's encoding.
    #[test]
    fn a_trust_request_is_kept() {
        let request = TrustRequest {
            ticket: 7,
            root: PathBuf::from("/w/repo"),
            standing: Standing::Changed,
            skipped: vec![Skipped {
                kind: Input::Hook,
                name: "PreToolUse bash: ./guard.sh".to_owned(),
                path: PathBuf::from("/w/repo/.claude/settings.json"),
            }],
        };
        let line = omc_wire::encode(&omc_wire::Event::Trust(request.clone())).unwrap();
        let heard = Event::from_harness(omc_wire::decode(&line).unwrap());
        assert_eq!(heard, Some(Event::Trust(request.clone())));
        let kept = serde_json::to_string(&heard).unwrap();
        assert_eq!(
            serde_json::from_str::<Option<Event>>(&kept).unwrap(),
            Some(Event::Trust(request))
        );
    }
}
