//! A conversation with one agent: what the reader says goes in, what the agent does comes out.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::process::Child;
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use serde_json::{Value, json};

use crate::agent::Agent;
use crate::background::{self, Shells};
use crate::client::{self, Answer, Client, Heard, LOGIN_REQUIRED, answer};
use crate::contained::{Containment, Terminals};
use crate::inquiry::{self, Inquiry, Question, Reply};
use crate::subagent::Subagents;

/// How long an agent that is already installed has to start and open a conversation.
const QUICK: Duration = Duration::from_secs(60);

/// How long an agent that has to be fetched first has to start and open a conversation.
const FETCH: Duration = Duration::from_secs(180);

/// How long the driver waits for the agent before looking at its commands again.
const LISTEN: Duration = Duration::from_millis(20);

/// How long to wait, after a conversation is opened, for the agent to say what commands it offers.
const COMMANDS_WAIT: Duration = Duration::from_secs(3);

/// The error an agent is answered with when it asks for something the app does not do.
const NO_SUCH_METHOD: i64 = -32601;

/// How far a tool the agent is using has got.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum ToolState {
    /// It is being run.
    Running,
    /// It finished.
    Done,
    /// It did not work.
    Failed,
}

/// What a tool does, as far as the agent says.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum Kind {
    /// It reads a file.
    Read,
    /// It changes a file.
    Edit,
    /// It looks for something.
    Search,
    /// It runs a command.
    Execute,
    /// It fetches something from the network.
    Fetch,
    /// It is something else.
    Other,
}

/// How a file changed in one edit.
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Diff {
    /// The file.
    pub path: String,
    /// What the file held before, when it existed.
    pub old: Option<String>,
    /// What it holds after.
    pub new: String,
}

/// A tool the agent is using, as far as it has said.
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Tool {
    /// What the agent calls this use of the tool.
    pub id: String,
    /// What it is doing, when the agent has said.
    pub title: Option<String>,
    /// What kind of tool it is, when the agent has said.
    pub kind: Option<Kind>,
    /// How far it has got, when the agent has said.
    pub state: Option<ToolState>,
    /// The file, command or query it works on, when the agent has said.
    pub target: Option<String>,
    /// What it changed in a file, when it did.
    pub diff: Option<Diff>,
    /// What it printed, when it did.
    pub output: Option<String>,
    /// The call of the subagent whose work this is, when it is a subagent's.
    #[serde(default)]
    pub parent: Option<String>,
    /// The kind of subagent this call starts, when it starts one.
    #[serde(default)]
    pub agent: Option<String>,
}

/// A command or task that goes on running beyond the call that started it.
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Background {
    /// The handle the agent knows it by.
    pub id: String,
    /// The command or task that was started.
    pub label: String,
    /// Whether it still runs.
    pub running: bool,
    /// The call that started it, when the agent said so.
    #[serde(default)]
    pub call: Option<String>,
}

/// What a permission choice amounts to.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum Verdict {
    /// The agent may go ahead this once.
    Allow,
    /// The agent may go ahead, now and whenever it asks again.
    Always,
    /// The agent may not.
    Reject,
}

/// One answer the reader can give to a permission request.
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Choice {
    /// What the agent calls the answer.
    pub id: String,
    /// What it is called to the reader.
    pub name: String,
    /// What the answer lets the agent do.
    pub verdict: Verdict,
}

/// A question the agent puts to the reader before it goes on.
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Permission {
    /// Names this question in the answer.
    pub ticket: u64,
    /// What the agent wants to do.
    pub title: String,
    /// The command it wants to run, when it is one.
    pub command: Option<String>,
    /// The plan it wants approved before it starts, when it is in plan mode.
    #[serde(default)]
    pub plan: Option<String>,
    /// The answers on offer.
    pub choices: Vec<Choice>,
}

/// One value a setting can take.
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Pick {
    /// What the agent calls the value.
    pub value: String,
    /// What it is called to the reader.
    pub name: String,
    /// What choosing it means, when the agent says.
    pub description: String,
}

/// Something about the conversation the reader can change: its model, mode or effort.
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Setting {
    /// What the agent calls the setting.
    pub id: String,
    /// What the setting is for: `model`, `mode`, `thought_level` or nothing in particular.
    pub category: String,
    /// What it is called to the reader.
    pub name: String,
    /// The value it has now.
    pub current: String,
    /// The values it can take.
    pub picks: Vec<Pick>,
}

impl Setting {
    /// The name the reader knows the current value by.
    pub fn current_name(&self) -> Option<&str> {
        self.picks
            .iter()
            .find(|pick| pick.value == self.current)
            .map(|pick| pick.name.as_str())
    }
}

/// How far along the agent's plan one step is.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum Progress {
    /// Not begun.
    Pending,
    /// Under way.
    Doing,
    /// Finished.
    Done,
}

/// One step of the agent's plan.
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PlanStep {
    /// What the step is.
    pub text: String,
    /// How far along it is.
    pub progress: Progress,
}

/// A command the agent offers behind a slash.
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Slash {
    /// What is typed after the slash.
    pub name: String,
    /// What it does.
    pub description: String,
}

/// How full the agent's context is: what a conversation holds of what it can.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Usage {
    /// How many tokens the conversation holds.
    pub used: u64,
    /// How many tokens the context window takes in all.
    pub size: u64,
}

/// What an agent did, in the order it did it.
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum Event {
    /// What can be changed about the conversation, and how it stands now.
    Settings(Vec<Setting>),
    /// The commands the agent offers behind a slash.
    Commands(Vec<Slash>),
    /// How full the conversation's context is now.
    Used(Usage),
    /// The agent said this much more of its reply.
    Said(String),
    /// The agent thought this much more.
    Thought(String),
    /// The agent started using a tool, or said more about one it was using.
    Tool(Tool),
    /// A command or task left running beyond its call started, or ended.
    Background(Background),
    /// The turn was stopped, so that nothing the agent had running goes on.
    Halted,
    /// The agent's plan, whole, as it stands now.
    Plan(Vec<PlanStep>),
    /// The agent waits for an answer to this.
    Ask(Permission),
    /// The agent waits for the answers to this form of questions.
    Inquire(Inquiry),
    /// The reader answered a form of questions: each question with what was answered, or nothing when it was skipped.
    Answered(Vec<(String, Option<String>)>),
    /// The turn ended, with the reason it failed where it did.
    Finished(Option<String>),
    /// The turn ended because the account needs signing in again.
    SignedOut {
        /// Why authentication failed.
        reason: String,
        /// The name of the session account, supplied by its server when named.
        account: Option<String>,
    },
    /// The session account was signed in again.
    SignedIn,
    /// The agent could not be started or stopped, for this reason.
    Failed(String),
    /// The dev container the agent runs in is being built or started, and this is the step it is on.
    Container(String),
    /// The agent named the conversation, or named it again.
    Titled(String),
    /// The hooks the agent's Codex holds back until the reader trusts them, all of them as they stand now; empty when none is held back.
    Hooks(Vec<crate::hooks::Hook>),
    /// A reader handed the agent a turn: what they wrote and the names of the files they attached.
    Prompted {
        /// What the reader wrote.
        text: String,
        /// The names of the files attached.
        files: Vec<String>,
        /// Whether the files of the session's folder were kept as they were before the turn, so they can be put back.
        #[serde(default)]
        checkpoint: bool,
    },
    /// A reader wrote while the agent was on a turn, so their words wait for it, under `id`.
    Queued {
        /// The number the waiting prompt goes by within its session.
        id: u64,
        /// What the reader wrote.
        text: String,
        /// The names of the files attached.
        files: Vec<String>,
    },
    /// The waiting prompt numbered this stopped waiting: handed to the agent, or dropped.
    Unqueued(u64),
    /// The conversation went back to before the reader's turn `turn`, counted from nought, which is dropped with all that followed it.
    Rewound {
        /// The turn, by how many of the reader's turns came before it.
        turn: u64,
    },
    /// The files of the session's folder were put back as they were before the reader's turn `turn`.
    Restored {
        /// The turn, by how many of the reader's turns came before it.
        turn: u64,
        /// The files that changed, from the root of the repository.
        files: Vec<String>,
    },
}

/// What the reader hands an agent.
enum Command {
    /// Starts a turn with these words and these files.
    Prompt {
        /// What the reader wrote.
        text: String,
        /// The files attached, by path.
        files: Vec<PathBuf>,
    },
    /// Answers the question under `ticket` with the choice `option`, or declines it.
    Choose {
        /// The question being answered.
        ticket: u64,
        /// The choice made, or nothing to decline.
        option: Option<String>,
    },
    /// Answers the form under `ticket` with `replies`, one for each of its questions, or abandons it.
    Answer {
        /// The form being answered.
        ticket: u64,
        /// What the reader answered, or nothing to abandon the form.
        replies: Option<Vec<Reply>>,
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

/// The agent's process once it is started, and whether the chat was stopped, shared by a chat and its driver thread.
#[derive(Default)]
struct Ending {
    /// The process, once it is started.
    process: Option<Arc<Mutex<Child>>>,
    /// Whether the chat was stopped, so a process started after that is ended at once.
    stopped: bool,
}

/// What a [`Chat`] calls each time its agent has something for [`Chat::events`], so the owner looks at once instead of on a timer.
pub type Notify = Arc<dyn Fn() + Send + Sync>;

/// Where the threads of a chat report what the agent did: the queue [`Chat::events`] reads, and the [`Notify`] that says it has grown.
#[derive(Clone)]
pub(crate) struct Events {
    /// The queue [`Chat::events`] reads.
    sender: Sender<Event>,
    /// Called after each event is queued.
    notify: Notify,
}

impl Events {
    /// Queues `event` for the owner of the chat and notifies it, and says whether the owner still listens.
    pub(crate) fn send(&self, event: Event) -> bool {
        let sent = self.sender.send(event).is_ok();
        (self.notify)();
        sent
    }
}

/// How a command is made to run inside the dev container of a session.
pub type Enclose = Arc<dyn Fn(std::process::Command) -> std::process::Command + Send + Sync>;

/// Brings the dev container of a session up, reporting each step to the closure it is given, and returns the folder the session works in inside it and how commands run there.
pub type Enclosure =
    Box<dyn FnOnce(&dyn Fn(&str)) -> Result<(PathBuf, Enclose), String> + Send + 'static>;

/// A running agent and the conversation held with it.
///
/// Everything the agent does happens on a thread of its own, so nothing it
/// takes time over holds a frame up; [`Chat::events`] collects what it has
/// said since the last look. Dropping the chat ends the agent.
pub struct Chat {
    /// Where the reader's commands go to the driver thread.
    commands: Sender<Command>,
    /// What the driver thread reports.
    events: Receiver<Event>,
    /// The agent's process, for ending it.
    ending: Arc<Mutex<Ending>>,
    /// The driver thread.
    driver: JoinHandle<()>,
    /// Kept while the chat lives, so that the thread watching the agent's hooks ends with it.
    _hooks: Sender<()>,
}

impl Chat {
    /// Starts `agent` in the folder `directory`, keeping its config and login in the folder `home` or in its own when there is none, and opens a conversation with it that has the tool servers `tools`, leaving out what `launch` says to; `notify` is called whenever the agent has something for [`Chat::events`]. With an `enclosure` the commands the agent runs go to the dev container it brings up, while the agent itself stays on this machine with its login.
    #[allow(clippy::too_many_arguments)]
    pub fn start(
        agent: Agent,
        directory: PathBuf,
        home: Option<PathBuf>,
        tools: Vec<serde_json::Value>,
        launch: pm_agent_library::Launch,
        enclosure: Option<Enclosure>,
        notify: Notify,
    ) -> Self {
        let (commands, received) = mpsc::channel();
        let (queue, events) = mpsc::channel();
        let sender = Events {
            sender: queue,
            notify,
        };
        let ending = Arc::new(Mutex::new(Ending::default()));
        let shared = Arc::clone(&ending);
        let (hooks, unwatched) = mpsc::channel();
        if agent.product() == Some(crate::product::Product::Codex) {
            let (directory, home, sender) = (directory.clone(), home.clone(), sender.clone());
            std::thread::spawn(move || {
                crate::hooks::watch(agent, directory, home, sender, unwatched)
            });
        }
        let driver = std::thread::spawn(move || {
            if let Err(reason) = drive(
                agent, &directory, home, &tools, &launch, enclosure, &shared, &received, &sender,
            ) {
                let _ = sender.send(Event::Failed(reason));
            }
        });
        Self {
            commands,
            events,
            ending,
            driver,
            _hooks: hooks,
        }
    }

    /// Ends the agent's process and waits until it and the driver thread are gone, so nothing the agent runs is left writing into its folder.
    pub fn stop(self) {
        if let Ok(mut ending) = self.ending.lock() {
            ending.stopped = true;
            if let Some(process) = &ending.process {
                client::end(process);
            }
        }
        drop(self.commands);
        let _ = self.driver.join();
    }

    /// Hands the agent `text` and the files at `files` as the next turn.
    pub fn prompt(&self, text: &str, files: &[String]) {
        let _ = self.commands.send(Command::Prompt {
            text: text.to_owned(),
            files: files.iter().map(PathBuf::from).collect(),
        });
    }

    /// Answers the question under `ticket` with the choice `option`, or declines it.
    pub fn choose(&self, ticket: u64, option: Option<String>) {
        let _ = self.commands.send(Command::Choose { ticket, option });
    }

    /// Answers the form of questions under `ticket` with `replies`, or abandons it.
    pub fn answer(&self, ticket: u64, replies: Option<Vec<Reply>>) {
        let _ = self.commands.send(Command::Answer { ticket, replies });
    }

    /// Stops the turn that is running.
    pub fn cancel(&self) {
        let _ = self.commands.send(Command::Cancel);
    }

    /// Gives the setting `setting` the value `value`.
    pub fn set(&self, setting: &str, value: &str) {
        let _ = self.commands.send(Command::Set {
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
                Err(TryRecvError::Empty) => return found,
                Err(TryRecvError::Disconnected) => return found,
            }
        }
    }
}

/// What an agent says about itself when a conversation is opened.
#[derive(Clone, Debug, Default, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Discovery {
    /// What the reader can change about a conversation.
    pub settings: Vec<Setting>,
    /// The commands the agent offers behind a slash.
    pub commands: Vec<Slash>,
}

impl Discovery {
    /// What this comes to as text that [`Discovery::from_text`] reads back.
    pub fn to_text(&self) -> String {
        let settings: Vec<Value> = self
            .settings
            .iter()
            .map(|setting| {
                json!({
                    "id": setting.id,
                    "category": setting.category,
                    "name": setting.name,
                    "current": setting.current,
                    "picks": setting.picks.iter().map(|pick| json!({
                        "value": pick.value,
                        "name": pick.name,
                        "description": pick.description,
                    })).collect::<Vec<_>>(),
                })
            })
            .collect();
        let commands: Vec<Value> = self
            .commands
            .iter()
            .map(|command| json!({ "name": command.name, "description": command.description }))
            .collect();
        json!({ "settings": settings, "commands": commands }).to_string()
    }

    /// The discovery that `text` from [`Discovery::to_text`] holds, if it holds one.
    pub fn from_text(text: &str) -> Option<Self> {
        let held: Value = serde_json::from_str(text).ok()?;
        let string = |value: &Value, key: &str| value[key].as_str().map(str::to_owned);
        let settings = held["settings"]
            .as_array()?
            .iter()
            .filter_map(|setting| {
                Some(Setting {
                    id: string(setting, "id")?,
                    category: string(setting, "category")?,
                    name: string(setting, "name")?,
                    current: string(setting, "current")?,
                    picks: setting["picks"]
                        .as_array()?
                        .iter()
                        .filter_map(|pick| {
                            Some(Pick {
                                value: string(pick, "value")?,
                                name: string(pick, "name")?,
                                description: string(pick, "description")?,
                            })
                        })
                        .collect(),
                })
            })
            .collect();
        let commands = held["commands"]
            .as_array()?
            .iter()
            .filter_map(|command| {
                Some(Slash {
                    name: string(command, "name")?,
                    description: string(command, "description")?,
                })
            })
            .collect();
        Some(Self { settings, commands })
    }
}

/// What `agent` says about itself, found by opening a conversation and closing it again.
pub fn discover(agent: Agent) -> Result<Discovery, String> {
    if !agent.startable() {
        return Err(format!("{} is not installed", agent.name));
    }
    let patience = if agent.fetches() { FETCH } else { QUICK };
    let mut client = Client::start(agent.command(&[]))?;
    client.initialize(patience)?;
    let home = std::env::home_dir().unwrap_or_default();
    let opened = match client.ask(
        "session/new",
        &json!({ "cwd": home, "mcpServers": [] }),
        patience,
    )? {
        Answer::Result(result) => result,
        Answer::Error(_, message) => return Err(message),
    };
    let mut found = Discovery {
        settings: settings(&opened),
        commands: Vec::new(),
    };
    let deadline = Instant::now() + COMMANDS_WAIT;
    while found.commands.is_empty() && Instant::now() < deadline {
        if let Heard::Message(message) = client.receive(LISTEN)
            && message["method"].as_str() == Some("session/update")
        {
            for event in update(&message["params"]["update"]) {
                if let Event::Commands(commands) = event {
                    found.commands = commands;
                }
            }
        }
    }
    Ok(found)
}

/// The skills `agent` can be asked for by name, found in the folders it keeps them in, less the ones `withheld` turns off, when it has skills.
pub fn skills(agent: Agent, directory: &Path, withheld: &[String]) -> Vec<Slash> {
    if agent.product() != Some(crate::product::Product::Codex) {
        return Vec::new();
    }
    crate::account::given_skills(agent, directory, withheld)
        .into_iter()
        .map(|skill| {
            (
                skill.name.clone(),
                Slash {
                    name: skill.name,
                    description: skill.description,
                },
            )
        })
        .collect::<BTreeMap<_, _>>()
        .into_values()
        .collect()
}

/// Starts the agent with its config and login in `home`, or in its own folder when there is none, opens a conversation and carries it until the chat is
/// dropped or the agent stops; `launch` says what it is started without.
#[allow(clippy::too_many_arguments)]
fn drive(
    agent: Agent,
    directory: &std::path::Path,
    home: Option<PathBuf>,
    tools: &[serde_json::Value],
    launch: &pm_agent_library::Launch,
    enclosure: Option<Enclosure>,
    ending: &Mutex<Ending>,
    commands: &Receiver<Command>,
    events: &Events,
) -> Result<(), String> {
    if !agent.startable() {
        return Err(format!("{} is not installed", agent.name));
    }
    if !directory.is_dir() {
        return Err(format!("{} is not a folder", directory.display()));
    }
    let patience = if agent.fetches() { FETCH } else { QUICK };
    let enclosed = match enclosure {
        Some(enclosure) => Some(enclosure(&|step| {
            let _ = events.send(Event::Container(step.to_owned()));
        })?),
        None => None,
    };
    if let Some(home) = &home {
        crate::profile::share_setup(agent, home);
    }
    let mut command = agent.command_in(&[], home);
    command.envs(launch.env.iter().map(|(name, value)| (name, value)));
    let containment = enclosed.map(|(workspace, enclose)| Containment {
        host: directory.to_path_buf(),
        workspace,
        enclose,
    });
    let working = directory.to_path_buf();
    let mut client = Client::start_in(command, directory)?;
    {
        let mut ending = ending.lock().map_err(|_| "the chat was dropped")?;
        if ending.stopped {
            return Ok(());
        }
        ending.process = Some(client.process());
    }
    let shaken = client.initialize(patience)?;
    let prompts = shaken["agentCapabilities"]["promptCapabilities"].clone();
    let mut opening = json!({ "cwd": working, "mcpServers": tools });
    if let Some(meta) = &launch.meta {
        opening["_meta"] = meta.clone();
    }
    let opened = match client.ask("session/new", &opening, patience)? {
        Answer::Result(result) => result,
        Answer::Error(code, message) if signed_out(code, &message) => {
            let _ = events.send(Event::SignedOut {
                reason: message,
                account: None,
            });
            return Ok(());
        }
        Answer::Error(_, message) => return Err(message),
    };
    let session = opened["sessionId"]
        .as_str()
        .ok_or("the agent opened no conversation")?
        .to_owned();
    let settings = settings(&opened);
    let configurable = opened["configOptions"].is_array();
    let _ = events.send(Event::Settings(settings));
    converse(
        &mut client,
        &session,
        configurable,
        &prompts,
        commands,
        events,
        containment,
    )
}

/// The settings a `session/new` result or a configuration update offers: its
/// `configOptions` where it has them, and otherwise its modes.
fn settings(result: &Value) -> Vec<Setting> {
    if let Some(options) = result["configOptions"].as_array() {
        return options.iter().filter_map(setting).collect();
    }
    let modes = &result["modes"];
    let picks: Vec<Pick> = modes["availableModes"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|mode| {
            Some(Pick {
                value: mode["id"].as_str()?.to_owned(),
                name: mode["name"].as_str().unwrap_or_default().to_owned(),
                description: mode["description"].as_str().unwrap_or_default().to_owned(),
            })
        })
        .collect();
    match modes["currentModeId"].as_str() {
        Some(current) if !picks.is_empty() => vec![Setting {
            id: "mode".to_owned(),
            category: "mode".to_owned(),
            name: "Mode".to_owned(),
            current: current.to_owned(),
            picks,
        }],
        _ => Vec::new(),
    }
}

/// The setting a `configOptions` entry describes, when it is a choice from a list.
fn setting(option: &Value) -> Option<Setting> {
    if option["type"].as_str() != Some("select") {
        return None;
    }
    let picks = option["options"]
        .as_array()?
        .iter()
        .filter_map(|pick| {
            Some(Pick {
                value: pick["value"].as_str()?.to_owned(),
                name: pick["name"].as_str().unwrap_or_default().to_owned(),
                description: pick["description"].as_str().unwrap_or_default().to_owned(),
            })
        })
        .collect();
    Some(Setting {
        id: option["id"].as_str()?.to_owned(),
        category: option["category"].as_str().unwrap_or_default().to_owned(),
        name: option["name"].as_str().unwrap_or_default().to_owned(),
        current: option["currentValue"].as_str()?.to_owned(),
        picks,
    })
}

/// What the reader's side of a conversation is waiting on.
struct Waiting {
    /// The id of the turn that is running, if one is.
    turn: Option<u64>,
    /// The ids of the requests that set a setting, whose answers carry the new settings.
    sets: HashSet<u64>,
    /// The questions put to the reader, by ticket, against the id the agent asked under.
    asked: HashMap<u64, Value>,
    /// The forms of questions put to the reader, by ticket, with the questions each asked.
    inquired: HashMap<u64, Vec<Question>>,
    /// The ticket the next question is put under.
    tickets: u64,
    /// Whether the agent takes pictures inside a prompt.
    images: bool,
    /// Whether the agent takes the contents of files inside a prompt.
    embeds: bool,
    /// The commands and tasks running beyond the turn, by handle.
    shells: Shells,
    /// The commands in the turn that have not ended, by call.
    running: Shells,
    /// The subagents the agent announced.
    subagents: Subagents,
    /// Whether the reader stopped the turn that is running.
    cancelled: bool,
    /// The commands the agent has the app run in the container of its session.
    terminals: Terminals,
}

/// Passes commands to the agent and what it says back, until the chat is
/// dropped or the agent stops.
fn converse(
    client: &mut Client,
    session: &str,
    configurable: bool,
    prompts: &Value,
    commands: &Receiver<Command>,
    events: &Events,
    containment: Option<Containment>,
) -> Result<(), String> {
    let mut waiting = Waiting {
        terminals: Terminals::new(containment),
        images: prompts["image"] == json!(true),
        embeds: prompts["embeddedContext"] == json!(true),
        turn: None,
        sets: HashSet::new(),
        asked: HashMap::new(),
        inquired: HashMap::new(),
        tickets: 0,
        shells: Shells::new(),
        running: Shells::new(),
        subagents: Subagents::default(),
        cancelled: false,
    };
    loop {
        loop {
            match commands.try_recv() {
                Ok(command) => {
                    command_agent(client, session, configurable, &mut waiting, command, events)?
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => return Ok(()),
            }
        }
        waiting.terminals.poll(client)?;
        let message = match client.receive(LISTEN) {
            Heard::Message(message) => message,
            Heard::Silence => continue,
            Heard::Closed => return Err("the agent stopped".to_owned()),
        };
        hear_agent(client, session, &mut waiting, &message, events)?;
    }
}

/// Carries out one of the reader's commands.
fn command_agent(
    client: &mut Client,
    session: &str,
    configurable: bool,
    waiting: &mut Waiting,
    command: Command,
    events: &Events,
) -> Result<(), String> {
    match command {
        Command::Prompt { text, files } if waiting.turn.is_none() => {
            let mut content = Vec::new();
            if !text.is_empty() {
                content.push(json!({ "type": "text", "text": text }));
            }
            content.extend(files.iter().filter_map(|path| {
                crate::attachment::content(path, waiting.images, waiting.embeds)
            }));
            let params = json!({ "sessionId": session, "prompt": content });
            waiting.turn = Some(client.request("session/prompt", &params)?);
        }
        Command::Prompt { .. } => {}
        Command::Choose { ticket, option } => {
            if let Some(id) = waiting.asked.remove(&ticket) {
                client.respond(&id, &outcome(option))?;
            }
        }
        Command::Answer { ticket, replies } => {
            if let (Some(id), Some(questions)) = (
                waiting.asked.remove(&ticket),
                waiting.inquired.remove(&ticket),
            ) {
                if let Some(replies) = &replies {
                    let _ = events.send(Event::Answered(inquiry::summary(&questions, replies)));
                }
                client.respond(&id, &inquiry::response(&questions, replies))?;
            }
        }
        Command::Cancel => {
            waiting.cancelled = true;
            for (ticket, id) in waiting.asked.drain() {
                let outcome = if waiting.inquired.remove(&ticket).is_some() {
                    inquiry::response(&[], None)
                } else {
                    outcome(None)
                };
                client.respond(&id, &outcome)?;
            }
            client.notify("session/cancel", &json!({ "sessionId": session }))?;
        }
        Command::Set { setting, value } => {
            let id = if configurable {
                client.request(
                    "session/set_config_option",
                    &json!({ "sessionId": session, "configId": setting, "value": value }),
                )?
            } else {
                client.request(
                    "session/set_mode",
                    &json!({ "sessionId": session, "modeId": value }),
                )?
            };
            waiting.sets.insert(id);
        }
    }
    Ok(())
}

/// Takes in one message the agent wrote.
fn hear_agent(
    client: &mut Client,
    session: &str,
    waiting: &mut Waiting,
    message: &Value,
    events: &Events,
) -> Result<(), String> {
    match (message["method"].as_str(), message.get("id")) {
        (Some("session/request_permission"), Some(id)) => {
            waiting.tickets += 1;
            waiting.asked.insert(waiting.tickets, id.clone());
            let _ = events.send(Event::Ask(permission(waiting.tickets, &message["params"])));
        }
        (Some("elicitation/create"), Some(id)) => match inquiry::questions(&message["params"]) {
            Some(questions) => {
                waiting.tickets += 1;
                waiting.asked.insert(waiting.tickets, id.clone());
                let _ = events.send(Event::Inquire(Inquiry {
                    ticket: waiting.tickets,
                    questions: questions.clone(),
                }));
                waiting.inquired.insert(waiting.tickets, questions);
            }
            None => client.respond(id, &json!({ "action": "decline" }))?,
        },
        (Some(method), Some(id))
            if method.starts_with("terminal/") && waiting.terminals.serves() =>
        {
            waiting
                .terminals
                .serve(client, id, method, &message["params"])?
        }
        (Some(_), Some(id)) => client.refuse(id, NO_SUCH_METHOD, "not supported")?,
        (Some("session/update"), None) => {
            for event in updated(session, waiting, &message["params"]) {
                let _ = events.send(event);
            }
        }
        (Some(_), None) => {}
        (None, Some(id)) => {
            let id = id.as_u64();
            if id.is_some() && id == waiting.turn {
                waiting.turn = None;
                let (finished, stopped) = match answer(message) {
                    Answer::Result(result) => (
                        Event::Finished(None),
                        result["stopReason"].as_str() == Some("cancelled"),
                    ),
                    Answer::Error(code, message) if signed_out(code, &message) => (
                        Event::SignedOut {
                            reason: message,
                            account: None,
                        },
                        true,
                    ),
                    Answer::Error(_, message) => (Event::Finished(Some(message)), true),
                };
                for event in turn_ended(waiting, stopped || waiting.cancelled) {
                    let _ = events.send(event);
                }
                waiting.cancelled = false;
                let _ = events.send(finished);
            } else if id.is_some_and(|id| waiting.sets.remove(&id))
                && message["result"]["configOptions"].is_array()
            {
                let _ = events.send(Event::Settings(settings(&message["result"])));
            }
        }
        (None, None) => {}
    }
    Ok(())
}

/// What an update of the conversation amounts to, the subagents' and the background work's included.
fn updated(session: &str, waiting: &mut Waiting, params: &Value) -> Vec<Event> {
    let from = params["sessionId"].as_str().unwrap_or(session);
    let update = &params["update"];
    let mut found = waiting.subagents.events(session, from, update);
    if found.is_empty() && !waiting.subagents.is_child(from) {
        found = self::update(update);
    }
    if from == session {
        track_commands(waiting, update);
        found.extend(background::background(update, &mut waiting.shells));
    }
    found
}

/// Keeps count of the commands of the turn that are yet to end.
fn track_commands(waiting: &mut Waiting, update: &Value) {
    if !matches!(
        update["sessionUpdate"].as_str(),
        Some("tool_call" | "tool_call_update")
    ) {
        return;
    }
    let Some(id) = update["toolCallId"].as_str() else {
        return;
    };
    if matches!(
        update["status"].as_str(),
        Some("completed" | "failed" | "cancelled")
    ) {
        waiting.running.remove(id);
    } else if update["kind"].as_str() == Some("execute") {
        let label = background::command(&update["rawInput"])
            .or_else(|| update["title"].as_str().map(str::to_owned))
            .unwrap_or_else(|| "Command".to_owned());
        waiting.running.insert(id.to_owned(), label);
    }
}

/// What the end of a turn means for the work it left running: all of it ends when the turn was `stopped`, and what is a command still going carries on otherwise.
fn turn_ended(waiting: &mut Waiting, stopped: bool) -> Vec<Event> {
    let running = std::mem::take(&mut waiting.running);
    if stopped {
        let mut ended = background::abandon(&mut waiting.shells);
        ended.push(Event::Halted);
        return ended;
    }
    background::outliving(&running, &mut waiting.shells)
}

/// The answer to a permission request that picks `option`, or declines when there is none.
fn outcome(option: Option<String>) -> Value {
    match option {
        Some(id) => json!({ "outcome": { "outcome": "selected", "optionId": id } }),
        None => json!({ "outcome": { "outcome": "cancelled" } }),
    }
}

/// The question a permission request's `params` put, under `ticket`.
fn permission(ticket: u64, params: &Value) -> Permission {
    let choices = params["options"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|option| {
            let verdict = match option["kind"].as_str().unwrap_or_default() {
                "allow_always" => Verdict::Always,
                kind if kind.starts_with("reject") => Verdict::Reject,
                _ => Verdict::Allow,
            };
            Some(Choice {
                id: option["optionId"].as_str()?.to_owned(),
                name: option["name"].as_str().unwrap_or("Choose").to_owned(),
                verdict,
            })
        })
        .collect();
    let call = &params["toolCall"];
    Permission {
        ticket,
        title: call["title"].as_str().unwrap_or("Use a tool").to_owned(),
        command: call["rawInput"]["command"].as_str().map(str::to_owned),
        plan: call["rawInput"]["plan"]
            .as_str()
            .filter(|_| call["rawInput"]["command"].is_null())
            .map(str::to_owned),
        choices,
    }
}

/// What the agent's `update` notification amounts to, if it is something to show.
fn update(update: &Value) -> Vec<Event> {
    let event = match update["sessionUpdate"].as_str().unwrap_or_default() {
        "agent_message_chunk" => update["content"]["text"]
            .as_str()
            .map(|text| Event::Said(text.to_owned())),
        "agent_thought_chunk" => update["content"]["text"]
            .as_str()
            .map(|text| Event::Thought(text.to_owned())),
        "tool_call" | "tool_call_update" => tool(update).map(Event::Tool),
        "plan" => Some(Event::Plan(
            update["entries"]
                .as_array()
                .into_iter()
                .flatten()
                .map(|entry| PlanStep {
                    text: entry["content"].as_str().unwrap_or_default().to_owned(),
                    progress: match entry["status"].as_str() {
                        Some("completed") => Progress::Done,
                        Some("in_progress") => Progress::Doing,
                        _ => Progress::Pending,
                    },
                })
                .collect(),
        )),
        "available_commands_update" => Some(Event::Commands(
            update["availableCommands"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|command| {
                    Some(Slash {
                        name: command["name"].as_str()?.to_owned(),
                        description: command["description"]
                            .as_str()
                            .unwrap_or_default()
                            .to_owned(),
                    })
                })
                .collect(),
        )),
        "session_info_update" => update["title"]
            .as_str()
            .filter(|title| !title.trim().is_empty())
            .map(|title| Event::Titled(title.trim().to_owned())),
        "usage_update" => update["used"]
            .as_u64()
            .zip(update["size"].as_u64())
            .map(|(used, size)| Event::Used(Usage { used, size })),
        "config_option_update" => update["configOptions"]
            .is_array()
            .then(|| Event::Settings(settings(update))),
        _ => None,
    };
    event.into_iter().collect()
}

/// The tool a `tool_call` or `tool_call_update` describes.
pub(crate) fn tool(update: &Value) -> Option<Tool> {
    let content = update["content"].as_array();
    let diff = content
        .into_iter()
        .flatten()
        .find(|item| item["type"].as_str() == Some("diff"))
        .and_then(|item| {
            Some(Diff {
                path: item["path"].as_str()?.to_owned(),
                old: item["oldText"].as_str().map(str::to_owned),
                new: item["newText"].as_str().unwrap_or_default().to_owned(),
            })
        });
    let printed: Vec<&str> = content
        .into_iter()
        .flatten()
        .filter(|item| item["type"].as_str() == Some("content"))
        .filter_map(|item| item["content"]["text"].as_str())
        .collect();
    let input = &update["rawInput"];
    let target = update["locations"][0]["path"]
        .as_str()
        .or_else(|| input["command"].as_str())
        .or_else(|| input["file_path"].as_str())
        .or_else(|| input["pattern"].as_str())
        .or_else(|| input["query"].as_str())
        .or_else(|| input["url"].as_str())
        .or_else(|| input["description"].as_str())
        .map(str::to_owned);
    let agent = subagent_kind(update);
    Some(Tool {
        id: update["toolCallId"].as_str()?.to_owned(),
        title: update["title"].as_str().map(str::to_owned),
        kind: update["kind"].as_str().map(|kind| match kind {
            "read" => Kind::Read,
            "edit" | "delete" | "move" => Kind::Edit,
            "search" => Kind::Search,
            "execute" => Kind::Execute,
            "fetch" => Kind::Fetch,
            _ => Kind::Other,
        }),
        state: update["status"].as_str().map(|status| match status {
            "completed" => ToolState::Done,
            "failed" => ToolState::Failed,
            _ => ToolState::Running,
        }),
        target,
        diff,
        output: (!printed.is_empty()).then(|| printed.join("\n")),
        parent: update["_meta"]["claudeCode"]["parentToolUseId"]
            .as_str()
            .map(str::to_owned),
        agent,
    })
}

/// The kind of subagent a call starts, when it is one that does.
fn subagent_kind(update: &Value) -> Option<String> {
    let names = [
        update["name"].as_str(),
        update["_meta"]["claudeCode"]["toolName"].as_str(),
    ];
    let named = names.contains(&Some("Task")) || names.contains(&Some("Agent"));
    let typed = update["rawInput"]["subagent_type"].as_str();
    (named || typed.is_some()).then(|| typed.unwrap_or("agent").to_owned())
}

/// Whether a prompt failure means the account needs signing in again.
fn signed_out(code: i64, message: &str) -> bool {
    let message = message.to_ascii_lowercase();
    code == LOGIN_REQUIRED
        || message.contains("failed to authenticate")
        || message.contains("authentication required")
        || message.contains("not authenticated")
        || message.contains("not logged in")
        || message.contains("oauth session expired")
}

#[cfg(test)]
/// Tests for authentication failures returned by prompts.
mod tests {
    use super::signed_out;

    /// Auth failures are distinct from ordinary prompt failures.
    #[test]
    fn prompt_authentication_errors() {
        assert!(signed_out(-32000, ""));
        assert!(signed_out(
            -32603,
            "Failed to authenticate: OAuth session expired and could not be refreshed"
        ));
        assert!(signed_out(-32603, "Not logged in"));
        assert!(!signed_out(-32603, ""));
        assert!(!signed_out(-32603, "Request timed out"));
    }
}
