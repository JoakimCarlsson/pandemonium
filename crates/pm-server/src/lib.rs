//! The headless server that owns the agents, their worktrees and their sessions.
//!
//! Clients, the window among them, connect over a WebSocket, ask for things
//! with [`pm_protocol::Request`] and are told what the agents do through
//! numbered events. An agent keeps running when every client has gone.

mod cli;
mod connection;
mod conversation;
mod core;
mod credential;
mod documents;
mod entries;
mod fetch;
mod folders;
mod handler;
mod languages;
mod listener;
mod machine;
mod queue;
mod releases;
mod rewind;
mod sessions;
mod terminals;
mod uploads;

pub use cli::run_with_arguments;
pub use credential::FILE as CREDENTIAL_FILE;
pub use listener::{Settings, run};
