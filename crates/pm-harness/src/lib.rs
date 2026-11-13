//! The app's side of the harness: the agents it ships, the conversation held with one, and what its files say about it.
//!
//! The harness is a program of its own that runs the agent loop and the
//! tools. This crate starts it, speaks its protocol, and turns what it
//! reports into the events the app keeps; it knows the agents the app ships,
//! finds out what one offers, signs one in and reads the customizations that
//! belong to it.

mod account;
mod agent;
mod chat;
mod discover;
mod event;
mod history;
mod inprocess;
mod language;
mod limits;
mod listing;
mod login;
mod mcp;
mod product;
mod profile;
mod tool_registry;
mod web;

pub use account::{SAVED_KEY, UNNAMED, account, account_in_folder, default_account};
pub use agent::{Agent, agent, agents};
pub use chat::{Chat, Given, Notify};
pub use discover::discover;
pub use event::{
    Allowance, Alternative, Background, Choice, Diff, Discovery, Event, Goal, GoalState, Input,
    Inquiry, Kind, Limit, LimitUnit, Limits, Merge, Part, Permission, Pick, PlanStep, Progress,
    Question, Reply, Setting, Skipped, Slash, Standing, Subagent, SubagentState, Tool, ToolState,
    TrustRequest, Usage, Verdict, Worktree,
};
pub use history::{import_saved, rename_saved, saved_conversation};
pub use language::LanguageServers;
pub use limits::allowance;
pub use listing::harness_tools;
pub use mcp::{command_line, command_words, mcp_servers};
pub use omc_wire::Conversation;
pub use omc_wire::Event as WireEvent;
pub use omc_wire::PastTurn;
pub use omc_wire::{Imported, ImportedTurn};
pub use omc_wire::{Reach, SignIn, ToolServer};
pub use product::Product;
pub use tool_registry::{Listing, search as search_tools};
pub use web::{is_web_address, open as open_address};
