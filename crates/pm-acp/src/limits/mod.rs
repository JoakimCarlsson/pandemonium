//! How much of a plan's rate limits an agent has left.
//!
//! The protocol carries a conversation's context and cost and nothing of the
//! plan behind it, so every agent says this its own way, or not at all: one
//! answers a request of its own for them, and the rest leave them in a file
//! or behind a service the editor reads beside them.
//! Each of those ways is a submodule here, and every one of them comes out as
//! [`Limits`]; above this module nothing knows which agent said what.
//!
//! Every source fails quiet. A missing file, a login that is not there, an
//! answer in a shape not seen before or a network that is down is no limits
//! for that agent, never an error: the window draws what it is told and says
//! nothing of what it was not.

mod claude;
mod clock;
mod codex;
mod copilot;
mod cursor;
mod grok;
mod web;

use std::time::SystemTime;

use serde_json::Value;

use crate::agent::Agent;

/// How much of each of a plan's rate-limit windows has been used.
#[derive(Clone, Debug, PartialEq)]
pub struct Limits {
    /// The plan the windows belong to, where the agent names it.
    pub plan: Option<String>,
    /// The windows, in the order the agent's own view of them lists them.
    pub windows: Vec<Window>,
}

/// One window of a plan's rate limits: a span of time and how much of it is
/// gone.
#[derive(Clone, Debug, PartialEq)]
pub struct Window {
    /// What the window is called, as `5-hour` or `Weekly`.
    pub label: String,
    /// How much of it has been used, as a percentage.
    pub used: f64,
    /// When it starts over, where the agent says.
    pub resets: Option<SystemTime>,
}

impl Limits {
    /// The limits `windows` come to under the plan `plan` names, or none
    /// where there is neither a plan nor a window to show.
    fn of(plan: Option<&str>, windows: Vec<Window>) -> Option<Self> {
        let plan = plan.filter(|plan| !plan.is_empty()).map(capitalized);
        (plan.is_some() || !windows.is_empty()).then_some(Self { plan, windows })
    }
}

/// `word` with its first letter in capitals, as a plan is named in a header.
fn capitalized(word: &str) -> String {
    let mut letters = word.chars();
    letters
        .next()
        .map(|first| first.to_uppercase().chain(letters).collect())
        .unwrap_or_default()
}

/// A read of an agent's limits made off its pipe, which may block on a disk
/// or a network for as long as it takes.
pub(crate) type Read = fn() -> Option<Limits>;

/// Where one session's limits come from, which is down to the agent.
pub(crate) enum Meter {
    /// Nowhere: the agent says nothing of its limits.
    Unmetered,
    /// A request of the agent's own, sent over its pipe under `method` and
    /// read by `answer`.
    Asked {
        /// The method the request goes under.
        method: &'static str,
        /// What the agent's answer comes to.
        answer: fn(&Value) -> Option<Limits>,
    },
    /// Something beside the agent, read off its pipe.
    Read(Read),
}

impl Meter {
    /// Where `agent`'s limits come from.
    pub(crate) fn of(agent: Agent) -> Self {
        match agent.id {
            "claude-code" => Self::Read(claude::read),
            "codex" => Self::Read(codex::read),
            "grok" => Self::Asked {
                method: grok::METHOD,
                answer: grok::billing,
            },
            "cursor" => Self::Read(cursor::read),
            "copilot" => Self::Read(copilot::read),
            _ => Self::Unmetered,
        }
    }

    /// The method the agent is asked its limits under, where it is asked.
    pub(crate) fn asks(&self) -> Option<&'static str> {
        match self {
            Self::Asked { method, .. } => Some(method),
            _ => None,
        }
    }

    /// The limits the agent's answer to [`Self::asks`] comes to.
    pub(crate) fn answered(&self, result: &Value) -> Option<Limits> {
        match self {
            Self::Asked { answer, .. } => answer(result),
            _ => None,
        }
    }

    /// The read made beside the agent, where its limits are read that way.
    pub(crate) fn reads(&self) -> Option<Read> {
        match self {
            Self::Read(read) => Some(*read),
            _ => None,
        }
    }
}
