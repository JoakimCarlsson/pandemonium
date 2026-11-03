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
mod login;
mod mcp;
mod product;
mod profile;
mod tool_registry;
mod web;

pub use account::{account, account_in_folder, default_account};
pub use agent::{AGENTS, Agent, harness_program};
pub use chat::{Chat, Given, Notify};
pub use discover::discover;
pub use event::{
    Alternative, Background, Choice, Diff, Discovery, Event, Inquiry, Kind, Permission, Pick,
    PlanStep, Progress, Question, Reply, Setting, Slash, Tool, ToolState, Usage, Verdict,
};
pub use mcp::{command_line, command_words, mcp_servers};
pub use omc_wire::{Reach, ToolServer};
pub use product::Product;
pub use tool_registry::{Listing, search as search_tools};
pub use web::{is_web_address, open as open_address};
