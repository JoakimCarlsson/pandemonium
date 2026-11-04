//! A conversation with one agent: what the reader says goes in, what the agent does comes out.

use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

use omc_wire::{Command, Opening, ToolServer};

use crate::inprocess::{Harness, Heard, Stopper};

use crate::agent::Agent;
use crate::event::{Event, Reply};

/// How long the driver waits for the harness before looking at its commands again.
const LISTEN: Duration = Duration::from_millis(20);

/// What the reader hands an agent, before it is sent to the harness.
enum Request {
    /// Starts a turn with these words and these files.
    Prompt {
        /// What the reader wrote.
        text: String,
        /// The files attached, by path.
        files: Vec<PathBuf>,
        /// The id the turn is saved under.
        turn: Option<String>,
        /// Words the agent is told before the reader's, which are not saved as theirs.
        context: Option<String>,
    },
    /// Answers the question under `ticket` with the choice `option`, or declines it.
    Choose {
        /// The question being answered.
        ticket: u64,
        /// The choice made, or nothing to decline.
        option: Option<String>,
    },
    /// Answers the form under `ticket` with `replies`, or abandons it.
    Answer {
        /// The form being answered.
        ticket: u64,
        /// What the reader answered, or nothing to abandon the form.
        replies: Option<Vec<Reply>>,
    },
    /// Names the conversation.
    Rename {
        /// The new name.
        title: String,
    },
    /// Stops the turn that is running.
    Cancel,
    /// Gives the setting `setting` the value `value`.
    Set {
        /// The setting.
        setting: String,
        /// The value.
        value: String,
    },
}

impl Request {
    /// The command that carries out the request.
    fn command(self) -> Command {
        match self {
            Self::Prompt {
                text,
                files,
                turn,
                context,
            } => Command::Prompt {
                text,
                files,
                turn,
                context,
            },
            Self::Choose { ticket, option } => Command::Choose { ticket, option },
            Self::Answer { ticket, replies } => Command::Answer { ticket, replies },
            Self::Rename { title } => Command::Rename { title },
            Self::Cancel => Command::Cancel,
            Self::Set { setting, value } => Command::Set { setting, value },
        }
    }
}

/// The way to end the conversation once it is started, and whether the chat was stopped, shared by a chat and its driver thread.
#[derive(Default)]
struct Ending {
    /// The way to end the conversation, once it is started.
    stopper: Option<Stopper>,
    /// Whether the chat was stopped, so a conversation started after that is ended at once.
    stopped: bool,
}

/// What a [`Chat`] calls each time its agent has something for [`Chat::events`], so the owner looks at once instead of on a timer.
pub type Notify = Arc<dyn Fn() + Send + Sync>;

/// What an agent is given besides its own tools, and what it is kept from.
#[derive(Clone, Debug, Default)]
pub struct Given {
    /// The tool servers whose tools it may call.
    pub tools: Vec<ToolServer>,
    /// The names of the tools it is not given, whoever offers them.
    pub denied: Vec<String>,
    /// The instruction files (by path), skills and agent definitions (by name) the reader turned off.
    pub disabled: Vec<String>,
    /// Why the agent must not be started, when what the reader turned off cannot be kept off; the chat then fails with this instead of starting it.
    pub blocked: Option<String>,
    /// The id the harness keeps the conversation's saved history under, and loads it from when the conversation is opened again; nothing is saved without one.
    pub conversation: Option<String>,
    /// The folder the harness keeps saved histories in, or its own when there is none.
    pub history: Option<PathBuf>,
    /// The id of the turn to go back to before when the saved history is loaded, or all of it when there is none.
    pub before: Option<String>,
    /// The values to give settings the saved conversation has no choice for.
    pub defaults: Vec<(String, String)>,
}

/// A running agent and the conversation held with it.
///
/// Everything the agent does happens on a thread of its own, so nothing it
/// takes time over holds a frame up; [`Chat::events`] collects what it has
/// said since the last look. Dropping the chat ends the agent.
pub struct Chat {
    /// Where the reader's requests go to the driver thread.
    requests: Sender<Request>,
    /// What the driver thread reports.
    events: Receiver<Event>,
    /// The way to end the conversation.
    ending: Arc<Mutex<Ending>>,
    /// The driver thread.
    driver: JoinHandle<()>,
}

impl Chat {
    /// Starts `agent` in the folder `directory`, keeping its login in the folder `home` or in the harness's own when there is none, and opens a conversation with it in which it is `given` its tools; `notify` is called whenever the agent has something for [`Chat::events`].
    pub fn start(
        agent: Agent,
        directory: PathBuf,
        home: Option<PathBuf>,
        given: Given,
        notify: Notify,
    ) -> Self {
        let (requests, received) = mpsc::channel();
        let (queue, events) = mpsc::channel();
        let ending = Arc::new(Mutex::new(Ending::default()));
        let shared = Arc::clone(&ending);
        let driver = std::thread::spawn(move || {
            let report = |event: Event| {
                let sent = queue.send(event).is_ok();
                notify();
                sent
            };
            if let Err(reason) = drive(agent, directory, home, given, &shared, &received, &report) {
                report(Event::Failed(reason));
            }
        });
        Self {
            requests,
            events,
            ending,
            driver,
        }
    }

    /// Ends the conversation and waits until it and the driver thread are gone, so nothing the agent runs is left writing into its folder.
    pub fn stop(self) {
        if let Ok(mut ending) = self.ending.lock() {
            ending.stopped = true;
            if let Some(stopper) = &ending.stopper {
                stopper.stop();
            }
        }
        drop(self.requests);
        let _ = self.driver.join();
    }

    /// Hands the agent `text` and the files at `files` as the next turn, which is saved under the id `turn`; `context` is what the agent is told first and is not saved as the reader's words.
    pub fn prompt(&self, text: &str, files: &[String], turn: u64, context: &str) {
        let _ = self.requests.send(Request::Prompt {
            text: text.to_owned(),
            files: files.iter().map(PathBuf::from).collect(),
            turn: Some(turn.to_string()),
            context: Some(context.to_owned()).filter(|context| !context.is_empty()),
        });
    }

    /// Answers the question under `ticket` with the choice `option`, or declines it.
    pub fn choose(&self, ticket: u64, option: Option<String>) {
        let _ = self.requests.send(Request::Choose { ticket, option });
    }

    /// Answers the form of questions under `ticket` with `replies`, or abandons it.
    pub fn answer(&self, ticket: u64, replies: Option<Vec<Reply>>) {
        let _ = self.requests.send(Request::Answer { ticket, replies });
    }

    /// Names the conversation, which the harness saves with it and tells back as a title.
    pub fn rename(&self, title: &str) {
        let _ = self.requests.send(Request::Rename {
            title: title.to_owned(),
        });
    }

    /// Stops the turn that is running.
    pub fn cancel(&self) {
        let _ = self.requests.send(Request::Cancel);
    }

    /// Gives the setting `setting` the value `value`.
    pub fn set(&self, setting: &str, value: &str) {
        let _ = self.requests.send(Request::Set {
            setting: setting.to_owned(),
            value: value.to_owned(),
        });
    }

    /// What the agent has done since the last call.
    pub fn events(&self) -> Vec<Event> {
        let mut found = Vec::new();
        loop {
            match self.events.try_recv() {
                Ok(event) => found.push(event),
                Err(TryRecvError::Empty | TryRecvError::Disconnected) => return found,
            }
        }
    }
}

/// Starts a conversation with the harness, opens it and carries it until the chat is dropped or the harness stops.
///
/// # Errors
///
/// Fails with the reason when the conversation cannot be started, or ends badly.
fn drive(
    agent: Agent,
    directory: PathBuf,
    home: Option<PathBuf>,
    given: Given,
    ending: &Mutex<Ending>,
    requests: &Receiver<Request>,
    report: &dyn Fn(Event) -> bool,
) -> Result<(), String> {
    if !directory.is_dir() {
        return Err(format!("{} is not a folder", directory.display()));
    }
    if let Some(reason) = given.blocked {
        return Err(reason);
    }
    let mut harness = Harness::start()?;
    {
        let mut ending = ending.lock().map_err(|_| "the chat was dropped")?;
        if ending.stopped {
            return Ok(());
        }
        ending.stopper = Some(harness.stopper());
    }
    harness.send(&Command::Open(Opening {
        directory,
        home,
        provider: agent.provider.to_owned(),
        model: None,
        yolo: false,
        tools: given.tools,
        denied: given.denied,
        disabled: given.disabled,
        conversation: given.conversation,
        history: given.history,
        keep: None,
        before: given.before,
        defaults: given.defaults,
    }))?;
    loop {
        loop {
            match requests.try_recv() {
                Ok(request) => harness.send(&request.command())?,
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => return Ok(()),
            }
        }
        match harness.listen(LISTEN) {
            Heard::Event(event) => {
                if let Some(event) = Event::from_harness(event)
                    && !report(event)
                {
                    return Ok(());
                }
            }
            Heard::Silence => {}
            Heard::Closed(reason) => return Err(reason),
        }
    }
}

#[cfg(test)]
/// Tests that hold a conversation with the real harness.
mod tests {
    use super::*;
    use crate::agent::AGENTS;

    /// A prompt that needs a tool is answered with a question, and the answer lets the turn finish.
    #[test]
    #[ignore = "needs the harness installed and a login"]
    fn live_turn_asks_before_running_a_command() {
        let folder = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../build/pm-harness-live");
        let _ = std::fs::create_dir_all(&folder);
        let chat = Chat::start(
            AGENTS[0],
            folder.clone(),
            None,
            Given::default(),
            Arc::new(|| {}),
        );
        chat.set("model", "claude-sonnet-5-5");
        chat.prompt(
            "Run the bash command `echo hi`, then reply with just: done",
            &[],
            1,
            "",
        );
        let mut seen = Vec::new();
        let deadline = std::time::Instant::now() + Duration::from_secs(90);
        let mut finished = false;
        while !finished && std::time::Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(100));
            for event in chat.events() {
                if let Event::Ask(question) = &event {
                    chat.choose(question.ticket, Some("allow".to_owned()));
                }
                finished |= matches!(event, Event::Finished(_));
                seen.push(event);
            }
        }
        chat.stop();
        let _ = std::fs::remove_dir_all(&folder);
        assert!(
            seen.iter().any(|event| matches!(event, Event::Ask(_))),
            "{seen:?}"
        );
        assert!(
            seen.iter()
                .any(|event| matches!(event, Event::Finished(None))),
            "{seen:?}"
        );
    }
}
