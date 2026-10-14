//! The agent protocol client and the events an agent produces.
//!
//! An agent is a program that speaks the Agent Client Protocol on its
//! standard input and output. This crate knows the agents the app ships,
//! finds out where each one stands on this machine, signs one in and
//! installs one that is missing.

mod account;
mod agent;
mod attachment;
mod background;
mod chat;
mod client;
mod inquiry;
mod install;
mod login;
mod profile;
mod standing;
mod subagent;

pub use account::{account, default_account, skill_names};
pub use agent::{AGENTS, Agent, Source};
pub use chat::{
    Background, Chat, Choice, Diff, Discovery, Event, Kind, Permission, Pick, PlanStep, Progress,
    Setting, Slash, Tool, ToolState, Verdict, discover, skills,
};
pub use inquiry::{Alternative, Inquiry, Question, Reply};
pub use standing::{Login, Standing, Way};
