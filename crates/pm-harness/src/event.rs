//! What an agent does, as the app sees it: the events of the harness' wire protocol and the ones the app adds as it keeps the conversation.

use serde_json::{Value, json};

pub use omc_wire::{
    Alternative, Background, Choice, Diff, Goal, GoalState, Input, Inquiry, Kind, Limit, LimitUnit,
    Limits, Merge, ModelSpend, Part, Permission, Pick, PlanStep, Progress, Question, Reply,
    Setting, Skipped, Slash, Spent, Standing, Subagent, SubagentState, Tool, ToolState,
    TrustRequest, Usage, Verdict, Worktree, dollars,
};

/// What an account has used of its allowance, as the event that carries it holds it.
///
/// The shares and counts in it are finite numbers, since the harness reads none that is not, so that two of them are equal exactly when every figure is, which is what [`Eq`] asks.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(transparent)]
pub struct Allowance(pub Limits);

impl Eq for Allowance {}

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
    /// A subagent started, or ended; while it runs it can be stopped by the id this gives it.
    Subagent(Subagent),
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
        /// The id the harness saves the turn under when the harness named the turn itself, as it does for a turn it starts toward the goal, and so the id is not `turn`; the files kept around the turn go by it, and a rewind goes back to before the turn by it.
        #[serde(default)]
        saved: Option<String>,
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
    /// What the reader's turn `turn` changed was taken back out of the files of the session's folder, what was done to them since left in.
    Undone {
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
    /// A tool server the agent uses wants the reader to sign in to it, at this web address, which a client opens in the reader's browser; the server's tools come with the next prompt once they have.
    ToolSignIn(String),
    /// How much of the account's allowance the provider says is used, as it stood when it was asked.
    Limits(Allowance),
    /// The conversation's goal was set, or how it stands changed; a cleared goal says so with its state.
    Goal(Goal),
    /// What the conversation has cost so far, whole, for each model it asked.
    Spent(Spent),
    /// The agent answered a question the reader asked on the side with `/btw`, which is not part of the conversation and is never told to the agent again; or, when `failed` is set, could not, and `answer` says why.
    Aside {
        /// What the reader asked, or nothing when the question could not be answered.
        question: String,
        /// What the agent answered, or why it could not.
        answer: String,
        /// Whether the question could not be answered.
        #[serde(default)]
        failed: bool,
    },
    /// The agent handed the work over to a new conversation, which starts afresh with `prompt` as its first turn; that turn's events follow.
    HandedOff {
        /// The prompt the agent wrote to hand the conversation over.
        prompt: String,
        /// The id the harness saved the new conversation's first turn under.
        turn: String,
    },
    /// The model slots of the conversation as they stand: each slot's model, its backups, the model it resolves to and where that comes from. Not part of any turn.
    Slots(Vec<omc_wire::SlotInfo>),
    /// A request to the model of `slot` kept failing in a way another model may not, so the rest of the turn asks the slot's backup `to` instead of `from`; `reason` is the first line of the failure.
    ModelFallback {
        /// The slot whose backup is used, such as `main`.
        slot: String,
        /// The model that failed.
        from: String,
        /// The model asked for the rest of the turn.
        to: String,
        /// Why the request failed, as the provider said.
        reason: String,
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

/// Whether `reason` is the harness saying that the subagent it was told to stop is not running: the subagent ended just before the reader stopped it, which leaves the turn as it is and is nothing to tell.
fn stop_came_late(reason: &str) -> bool {
    reason.starts_with("no subagent ") && reason.ends_with(" is running")
}

/// Whether `reason` is the harness refusing `/retry`, `/fresh` or `/handoff`, or saying it had nothing to answer again: the agent goes on as it was, so the turn the command was sent as ends with the reason rather than the agent being taken for gone.
fn refused_restart(reason: &str) -> bool {
    ["/retry ", "/fresh ", "/handoff "]
        .iter()
        .any(|command| reason.starts_with(command))
}

impl Event {
    /// The event the app keeps for what the harness `reported`, or nothing when the app has no use for it: a refusal to stop a subagent that had ended already, and a `/goal` command the agent could not carry out while it worked, are dropped, since the app takes a failure to mean the agent is gone; the goal's own events say how it stands. A turn the harness started itself toward the goal comes as the prompt it started with.
    ///
    /// The answer to a question asked on the side becomes an aside, and a refusal to answer one a failed aside rather than a failure of the agent; a refused `/retry`, `/fresh` or `/handoff` ends the turn it was sent as.
    ///
    /// The harness's answers about the files of a turn are not kept here: [`crate::FileNews`] reads them.
    pub fn from_harness(reported: omc_wire::Event) -> Option<Self> {
        use omc_wire::Event as Wire;
        match reported {
            Wire::Settings(settings) => Some(Self::Settings(settings)),
            Wire::Commands(commands) => Some(Self::Commands(commands)),
            Wire::Used(usage) => Some(Self::Used(usage)),
            Wire::Spent(spent) => Some(Self::Spent(spent)),
            Wire::Started(part) => Some(Self::Started(part)),
            Wire::Said(text) => Some(Self::Said(text)),
            Wire::Thought(text) => Some(Self::Thought(text)),
            Wire::Tool(tool) => Some(Self::Tool(tool)),
            Wire::Background(background) => Some(Self::Background(background)),
            Wire::Subagent(subagent) => Some(Self::Subagent(subagent)),
            Wire::Halted => Some(Self::Halted),
            Wire::Plan(plan) => Some(Self::Plan(plan)),
            Wire::Ask(permission) => Some(Self::Ask(permission)),
            Wire::Inquire(inquiry) => Some(Self::Inquire(inquiry)),
            Wire::Finished(failure) => Some(Self::Finished(failure)),
            Wire::SignedOut { reason, account } => Some(Self::SignedOut { reason, account }),
            Wire::SignedIn => Some(Self::SignedIn),
            Wire::LoginUrl(address) => Some(Self::ToolSignIn(address)),
            Wire::LoginCode { .. } | Wire::Resumed(_) | Wire::Language(_) | Wire::Queued(_) => None,
            Wire::Goal(goal) => Some(Self::Goal(goal)),
            Wire::Continued { prompt, turn } => Some(Self::Prompted {
                text: prompt,
                files: Vec::new(),
                checkpoint: false,
                turn: 0,
                saved: Some(turn),
            }),
            Wire::Trust(request) => Some(Self::Trust(request)),
            Wire::Failed(reason) if stop_came_late(&reason) || reason.starts_with("/goal:") => None,
            Wire::Failed(reason) if reason.starts_with("/btw ") => Some(Self::Aside {
                question: String::new(),
                answer: reason,
                failed: true,
            }),
            Wire::Failed(reason) if refused_restart(&reason) => Some(Self::Finished(Some(reason))),
            Wire::Failed(reason) => Some(Self::Failed(reason)),
            Wire::Aside { question, answer } => Some(Self::Aside {
                question,
                answer,
                failed: false,
            }),
            Wire::HandedOff { prompt, turn } => Some(Self::HandedOff { prompt, turn }),
            Wire::Titled(title) => Some(Self::Titled(title)),
            Wire::Changes { .. } | Wire::Restored { .. } | Wire::Undone { .. } => None,
            Wire::Servers(_) | Wire::FoundModels { .. } | Wire::Presets(_) => None,
            Wire::Slots(slots) => Some(Self::Slots(slots)),
            Wire::ModelFallback {
                slot,
                from,
                to,
                reason,
            } => Some(Self::ModelFallback {
                slot,
                from,
                to,
                reason,
            }),
        }
    }
}

#[cfg(test)]
/// Tests for how the harness's events become the app's.
mod tests {
    use std::path::PathBuf;

    use super::*;

    /// A turn the harness starts itself toward the goal comes as the prompt it started with, the goal as the app's goal event, and a `/goal` command it could not carry out is dropped.
    #[test]
    fn goal_events_become_the_apps() {
        assert_eq!(
            Event::from_harness(omc_wire::Event::Continued {
                prompt: "keep going".into(),
                turn: "goal-7".into(),
            }),
            Some(Event::Prompted {
                text: "keep going".into(),
                files: Vec::new(),
                checkpoint: false,
                turn: 0,
                saved: Some("goal-7".into()),
            })
        );
        let goal = Goal {
            objective: "ship".into(),
            state: GoalState::Active,
            budget: None,
            used: 0,
            note: None,
        };
        assert_eq!(
            Event::from_harness(omc_wire::Event::Goal(goal.clone())),
            Some(Event::Goal(goal))
        );
        assert_eq!(
            Event::from_harness(omc_wire::Event::Failed(
                "/goal: the goal is not active".into()
            )),
            None
        );
    }

    /// An answer on the side becomes the app's aside and a question that could not be answered a failed one; a hand-over is kept with its prompt, and a refused `/retry` ends the turn instead of taking the agent for gone; each survives the session log's encoding.
    #[test]
    fn asides_and_hand_overs_become_the_apps() {
        let aside = Event::from_harness(omc_wire::Event::Aside {
            question: "why?".into(),
            answer: "because".into(),
        });
        assert_eq!(
            aside,
            Some(Event::Aside {
                question: "why?".into(),
                answer: "because".into(),
                failed: false,
            })
        );
        assert_eq!(
            Event::from_harness(omc_wire::Event::Failed(
                "/btw could not be answered: offline".into()
            )),
            Some(Event::Aside {
                question: String::new(),
                answer: "/btw could not be answered: offline".into(),
                failed: true,
            })
        );
        let handed = Event::from_harness(omc_wire::Event::HandedOff {
            prompt: "carry on".into(),
            turn: "handoff-1".into(),
        });
        assert_eq!(
            handed,
            Some(Event::HandedOff {
                prompt: "carry on".into(),
                turn: "handoff-1".into(),
            })
        );
        assert_eq!(
            Event::from_harness(omc_wire::Event::Failed(
                "/retry has no prompt to answer again".into()
            )),
            Some(Event::Finished(Some(
                "/retry has no prompt to answer again".into()
            )))
        );
        for kept in [aside, handed] {
            let line = serde_json::to_string(&kept).unwrap();
            assert_eq!(serde_json::from_str::<Option<Event>>(&line).unwrap(), kept);
        }
    }

    /// The sign-in page a tool server wants opened, which the harness sends during a conversation as a line of its wire protocol, becomes the app's sign-in event, and survives the trip through the session log's encoding.
    #[test]
    fn a_tool_sign_in_is_kept() {
        let address = "http://127.0.0.1:4567/authorize?state=x".to_owned();
        let line = omc_wire::encode(&omc_wire::Event::LoginUrl(address.clone())).unwrap();
        let heard = Event::from_harness(omc_wire::decode(&line).unwrap());
        assert_eq!(heard, Some(Event::ToolSignIn(address.clone())));
        let kept = serde_json::to_string(&heard).unwrap();
        assert_eq!(
            serde_json::from_str::<Option<Event>>(&kept).unwrap(),
            Some(Event::ToolSignIn(address))
        );
    }

    /// A subagent starting is kept with the id it is stopped by; the harness refusing to stop one that had ended already is dropped, and any other failure is kept.
    #[test]
    fn a_subagent_is_kept_and_a_late_stop_is_dropped() {
        let subagent = Subagent {
            id: "c1/2".to_owned(),
            call: "c1".to_owned(),
            agent: "scout".to_owned(),
            label: "Find the parser".to_owned(),
            model: None,
            background: false,
            state: SubagentState::Running,
        };
        assert_eq!(
            Event::from_harness(omc_wire::Event::Subagent(subagent.clone())),
            Some(Event::Subagent(subagent))
        );
        assert_eq!(
            Event::from_harness(omc_wire::Event::Failed(
                "no subagent c1/2 is running".to_owned()
            )),
            None
        );
        assert_eq!(
            Event::from_harness(omc_wire::Event::Failed("out of memory".to_owned())),
            Some(Event::Failed("out of memory".to_owned()))
        );
    }

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
