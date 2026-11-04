//! The harness running inside this program: one conversation on a thread of its own.
//!
//! The harness is a library of async code, and this program has no async
//! runtime of its own, so each conversation gets a thread that holds a small
//! runtime and runs the conversation on it. Commands and events cross to and
//! from that thread over channels, so a panic or a stuck turn in one
//! conversation ends that conversation alone.

use std::any::Any;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

use omc_wire::{Command, Event};
use tokio::sync::mpsc::{UnboundedSender, unbounded_channel};

/// What listening to a conversation for a moment came to.
#[derive(Debug)]
pub(crate) enum Heard {
    /// The conversation reported this.
    Event(Event),
    /// Nothing came in the time given.
    Silence,
    /// The conversation has ended, with the reason when it ended badly.
    Closed(String),
}

/// A way to end a conversation from another thread.
#[derive(Clone)]
pub(crate) struct Stopper {
    /// Where the conversation's commands go.
    commands: UnboundedSender<Command>,
}

impl Stopper {
    /// Asks the conversation to end, which stops the turn that is running and what it started.
    pub(crate) fn stop(&self) {
        let _ = self.commands.send(Command::Close);
    }
}

/// One running conversation with the harness.
pub(crate) struct Harness {
    /// Where commands go.
    commands: UnboundedSender<Command>,
    /// What the conversation reported.
    events: Receiver<Event>,
    /// What the thread said when it ended badly.
    failure: Arc<Mutex<String>>,
    /// The thread the conversation runs on.
    thread: Option<JoinHandle<()>>,
}

impl Harness {
    /// Starts a conversation on a thread of its own.
    ///
    /// # Errors
    ///
    /// Fails with the reason when the thread cannot be started.
    pub(crate) fn start() -> Result<Self, String> {
        let (commands, received) = unbounded_channel();
        let (sender, events) = mpsc::channel();
        let failure = Arc::new(Mutex::new(String::new()));
        let said = Arc::clone(&failure);
        let thread = std::thread::Builder::new()
            .name("harness".to_owned())
            .spawn(move || {
                let outcome = catch_unwind(AssertUnwindSafe(|| hold(received, &sender)));
                let reason = match outcome {
                    Ok(Ok(())) => return,
                    Ok(Err(reason)) => reason,
                    Err(panic) => format!("the agent crashed: {}", describe(panic.as_ref())),
                };
                if let Ok(mut said) = said.lock() {
                    *said = reason;
                }
            })
            .map_err(|error| format!("could not start the agent: {error}"))?;
        Ok(Self {
            commands,
            events,
            failure,
            thread: Some(thread),
        })
    }

    /// Sends `command` to the conversation.
    ///
    /// # Errors
    ///
    /// Fails with the reason when the conversation has ended.
    pub(crate) fn send(&mut self, command: &Command) -> Result<(), String> {
        self.commands
            .send(command.clone())
            .map_err(|_| self.stopped())
    }

    /// The next event, waiting up to `patience` for one.
    pub(crate) fn listen(&self, patience: Duration) -> Heard {
        match self.events.recv_timeout(patience) {
            Ok(event) => Heard::Event(event),
            Err(RecvTimeoutError::Timeout) => Heard::Silence,
            Err(RecvTimeoutError::Disconnected) => Heard::Closed(self.stopped()),
        }
    }

    /// Why the conversation ended.
    fn stopped(&self) -> String {
        match self.failure.lock().map(|said| said.clone()) {
            Ok(said) if !said.is_empty() => said,
            _ => "stopped".to_owned(),
        }
    }

    /// A way to end the conversation from another thread.
    pub(crate) fn stopper(&self) -> Stopper {
        Stopper {
            commands: self.commands.clone(),
        }
    }
}

impl Drop for Harness {
    /// Ends the conversation and waits until its thread is gone, so nothing it started is left running.
    fn drop(&mut self) {
        let _ = self.commands.send(Command::Close);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// Runs the conversation that reads `received` on a runtime of its own, passing what it reports to `sender`, until it ends.
///
/// # Errors
///
/// Fails with the reason when the runtime cannot be made.
fn hold(
    received: tokio::sync::mpsc::UnboundedReceiver<Command>,
    sender: &mpsc::Sender<Event>,
) -> Result<(), String> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| format!("could not start the agent's runtime: {error}"))?;
    runtime.block_on(async {
        let (out, mut reported) = unbounded_channel();
        let session = omc_session::run(received, out);
        tokio::pin!(session);
        loop {
            tokio::select! {
                () = &mut session => break,
                Some(event) = reported.recv() => {
                    if sender.send(event).is_err() {
                        return;
                    }
                }
            }
        }
        while let Ok(event) = reported.try_recv() {
            if sender.send(event).is_err() {
                return;
            }
        }
    });
    Ok(())
}

/// What a panic said, when it said anything readable.
fn describe(panic: &(dyn Any + Send)) -> String {
    panic
        .downcast_ref::<&str>()
        .map(|text| (*text).to_owned())
        .or_else(|| panic.downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "it panicked".to_owned())
}
