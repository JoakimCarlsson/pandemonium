//! What an agent says about itself before a conversation: found by opening one and closing it again.

use std::time::{Duration, Instant};

use omc_wire::{Command, Event, Opening};

use crate::inprocess::{Harness, Heard};

use crate::agent::Agent;
use crate::event::Discovery;

/// How long the harness has to open a conversation and say what it offers.
const PATIENCE: Duration = Duration::from_secs(60);

/// What `agent` says about itself, its settings and the commands its skills give, found by opening a conversation and closing it again.
///
/// # Errors
///
/// Fails with the reason when the conversation cannot be started, or the agent cannot be reached.
pub fn discover(agent: Agent) -> Result<Discovery, String> {
    let mut harness = Harness::start()?;
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
        conversation: None,
        history: None,
        keep: None,
        before: None,
        defaults: Vec::new(),
        languages: false,
        navigation: false,
    }))?;
    let deadline = Instant::now() + PATIENCE;
    let mut settings = None;
    let mut commands = None;
    while let Some(left) = deadline.checked_duration_since(Instant::now()) {
        match harness.listen(left) {
            Heard::Event(Event::Settings(said)) => settings = Some(said),
            Heard::Event(Event::Commands(said)) => commands = Some(said),
            Heard::Event(Event::SignedOut { reason, .. } | Event::Failed(reason)) => {
                return Err(reason);
            }
            Heard::Event(_) | Heard::Silence => {}
            Heard::Closed(reason) => return Err(reason),
        }
        if let (Some(settings), Some(commands)) = (&settings, &commands) {
            return Ok(Discovery {
                settings: settings.clone(),
                commands: commands.clone(),
            });
        }
    }
    Err("did not answer in time".to_owned())
}
