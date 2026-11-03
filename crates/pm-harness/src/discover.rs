//! What an agent says about itself before a conversation: found by opening one and closing it again.

use std::time::{Duration, Instant};

use omc_client::{Command, Event, Harness, Heard, Opening};

use crate::agent::Agent;
use crate::event::Discovery;

/// How long the harness has to open a conversation and say what it offers.
const PATIENCE: Duration = Duration::from_secs(60);

/// What `agent` says about itself, found by opening a conversation and closing it again.
///
/// # Errors
///
/// Fails with the reason when the harness is not installed, cannot be started, or the agent cannot be reached.
pub fn discover(agent: Agent) -> Result<Discovery, String> {
    let command = agent.command().ok_or("the harness is not installed")?;
    let mut harness = Harness::start(command)?;
    let home = std::env::home_dir().unwrap_or_default();
    harness.send(&Command::Open(Opening {
        directory: home,
        home: agent.home(),
        provider: agent.provider.to_owned(),
        model: None,
        yolo: false,
        tools: Vec::new(),
        denied: Vec::new(),
        disabled: Vec::new(),
    }))?;
    let deadline = Instant::now() + PATIENCE;
    while let Some(left) = deadline.checked_duration_since(Instant::now()) {
        match harness.listen(left) {
            Heard::Event(Event::Settings(settings)) => {
                return Ok(Discovery {
                    settings,
                    commands: Vec::new(),
                });
            }
            Heard::Event(Event::SignedOut { reason, .. } | Event::Failed(reason)) => {
                return Err(reason);
            }
            Heard::Event(_) | Heard::Silence => {}
            Heard::Closed(reason) => return Err(reason),
        }
    }
    Err("did not answer in time".to_owned())
}
