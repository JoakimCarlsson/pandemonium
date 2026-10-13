//! The agent protocol client and the events an agent produces.
//!
//! An agent is a program that speaks the Agent Client Protocol on its
//! standard input and output. This crate knows the agents the app ships,
//! finds out where each one stands on this machine, signs one in and
//! installs one that is missing.

mod agent;
mod client;
mod install;
mod login;
mod standing;

pub use agent::{AGENTS, Agent, Source};
pub use standing::{Login, Standing, Way};
