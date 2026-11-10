//! Asking the agent's provider how much of the account's allowance is used, without holding the conversation up.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::{Duration, Instant};

use crate::agent::Agent;
use crate::event::{Allowance, Event};

/// How long the allowance is left alone before it is asked after again for no reason but time.
const REFRESH: Duration = Duration::from_secs(300);

/// How soon after the last time it is asked after again when a turn has just ended; providers rate limit these questions.
const AFTER_TURN: Duration = Duration::from_secs(45);

/// Asks the provider of one conversation's agent how much of the account's allowance is used, on threads of their own, and holds the answers until they are collected.
pub(crate) struct Watch {
    /// The provider asked.
    provider: &'static str,
    /// The login folder of the account asked about, or the harness's own when there is none.
    home: Option<PathBuf>,
    /// Where the threads that ask leave what they learned.
    answers: Sender<Event>,
    /// What the threads that ask left.
    heard: Receiver<Event>,
    /// Whether a thread is asking now.
    asking: Arc<AtomicBool>,
    /// When the provider was last asked.
    asked: Option<Instant>,
}

impl Watch {
    /// A watch on the allowance of `agent`'s account under the login folder `home`; nothing when the agent's provider meters no allowance that can be read.
    pub(crate) fn of(agent: Agent, home: Option<PathBuf>) -> Option<Self> {
        omc_session::limits_providers()
            .iter()
            .any(|id| id == agent.provider)
            .then(|| {
                let (answers, heard) = mpsc::channel();
                Self {
                    provider: agent.provider,
                    home,
                    answers,
                    heard,
                    asking: Arc::new(AtomicBool::new(false)),
                    asked: None,
                }
            })
    }

    /// Asks again when the provider was never asked or `patience` has passed since it was, and no question is out.
    pub(crate) fn ask(&mut self, patience: Duration) {
        if self.asked.is_some_and(|asked| asked.elapsed() < patience)
            || self.asking.swap(true, Ordering::AcqRel)
        {
            return;
        }
        self.asked = Some(Instant::now());
        let (provider, home) = (self.provider, self.home.clone());
        let (answers, asking) = (self.answers.clone(), Arc::clone(&self.asking));
        let spawned = std::thread::Builder::new()
            .name("harness-limits".to_owned())
            .spawn(move || {
                if let Some(limits) = fetch(provider, home) {
                    let _ = answers.send(Event::Limits(Allowance(limits)));
                }
                asking.store(false, Ordering::Release);
            });
        if spawned.is_err() {
            self.asking.store(false, Ordering::Release);
        }
    }

    /// Asks again for no reason but that time has passed.
    pub(crate) fn tick(&mut self) {
        self.ask(REFRESH);
    }

    /// Asks again because a turn has just ended, which is when the allowance has moved.
    pub(crate) fn turn_ended(&mut self) {
        self.ask(AFTER_TURN);
    }

    /// What the provider said since the last call.
    pub(crate) fn heard(&self) -> Vec<Event> {
        self.heard.try_iter().collect()
    }
}

/// What the account of `agent` under the login folder `home` has used of its allowance, asked now and waited for; nothing when the agent's provider meters none that can be read, or does not say.
pub fn allowance(agent: Agent, home: Option<PathBuf>) -> Option<Allowance> {
    omc_session::limits_providers()
        .iter()
        .any(|id| id == agent.provider)
        .then(|| fetch(agent.provider, home))
        .flatten()
        .map(Allowance)
}

/// What the account of `provider` under `home` has used of its allowance, or nothing when the provider cannot be asked just now or says nothing; a failed question costs nothing but the wait for the next one.
fn fetch(provider: &str, home: Option<PathBuf>) -> Option<omc_wire::Limits> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .ok()?;
    runtime
        .block_on(omc_session::limits(provider, home))
        .ok()
        .flatten()
}
