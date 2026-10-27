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
mod debugs;
mod documents;
mod entries;
mod extensions;
mod fetch;
mod folders;
mod handler;
mod languages;
mod lenses;
mod library;
mod listener;
mod local_history;
mod machine;
mod queue;
mod releases;
mod rewind;
mod sessions;
mod tasks;
mod terminals;
mod uploads;

pub use cli::run_with_arguments;
pub use credential::FILE as CREDENTIAL_FILE;
pub use languages::render_with_module;
pub use listener::{Settings, run};
