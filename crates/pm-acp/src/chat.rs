//! A conversation with one agent: what the reader says goes in, what the agent does comes out.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::time::{Duration, Instant};

use serde_json::{Value, json};

use crate::agent::Agent;
use crate::client::{Answer, Client, Heard, LOGIN_REQUIRED, answer};

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

/// What an agent did, in the order it did it.
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum Event {
    /// What can be changed about the conversation, and how it stands now.
    Settings(Vec<Setting>),
    /// The commands the agent offers behind a slash.
    Commands(Vec<Slash>),
    /// The agent said this much more of its reply.
    Said(String),
    /// The agent thought this much more.
    Thought(String),
    /// The agent started using a tool, or said more about one it was using.
    Tool(Tool),
    /// The agent's plan, whole, as it stands now.
    Plan(Vec<PlanStep>),
    /// The agent waits for an answer to this.
    Ask(Permission),
    /// The turn ended, with the reason it failed where it did.
    Finished(Option<String>),
    /// The agent could not be started or stopped, for this reason.
    Failed(String),
    /// The agent named the conversation, or named it again.
    Titled(String),
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
}

impl Chat {
    /// Starts `agent` in the folder `directory` and opens a conversation with it.
    pub fn start(agent: Agent, directory: PathBuf) -> Self {
        let (commands, received) = mpsc::channel();
        let (sender, events) = mpsc::channel();
        std::thread::spawn(move || {
            if let Err(reason) = drive(agent, &directory, &received, &sender) {
                let _ = sender.send(Event::Failed(reason));
            }
        });
        Self { commands, events }
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

/// The skills `agent` can be asked for by name, found in the folders it keeps them in, when it has skills.
pub fn skills(agent: Agent, directory: &Path) -> Vec<Slash> {
    if agent.id != "codex" {
        return Vec::new();
    }
    let home = std::env::home_dir();
    let codex_home = std::env::var_os("CODEX_HOME")
        .map(PathBuf::from)
        .or_else(|| home.as_ref().map(|home| home.join(".codex")));
    let folders = [
        home.as_ref().map(|home| home.join(".agents/skills")),
        codex_home.map(|home| home.join("skills")),
        Some(directory.join(".codex/skills")),
        Some(directory.join(".agents/skills")),
    ];
    let mut found = BTreeMap::new();
    for folder in folders.into_iter().flatten() {
        let Ok(entries) = std::fs::read_dir(folder) else {
            continue;
        };
        for entry in entries.flatten() {
            let Ok(held) = std::fs::read_to_string(entry.path().join("SKILL.md")) else {
                continue;
            };
            let field = |key: &str| {
                held.lines()
                    .find_map(|line| line.strip_prefix(key))
                    .map(|value| value.trim().to_owned())
            };
            if let Some(name) = field("name: ").filter(|name| !name.is_empty()) {
                let description = field("description: ").unwrap_or_default();
                found.insert(name.clone(), Slash { name, description });
            }
        }
    }
    found.into_values().collect()
}

/// Starts the agent, opens a conversation and carries it until the chat is
/// dropped or the agent stops.
fn drive(
    agent: Agent,
    directory: &std::path::Path,
    commands: &Receiver<Command>,
    events: &Sender<Event>,
) -> Result<(), String> {
    if !agent.startable() {
        return Err(format!("{} is not installed", agent.name));
    }
    if !directory.is_dir() {
        return Err(format!("{} is not a folder", directory.display()));
    }
    let patience = if agent.fetches() { FETCH } else { QUICK };
    let mut client = Client::start_in(agent.command(&[]), directory)?;
    let shaken = client.initialize(patience)?;
    let prompts = shaken["agentCapabilities"]["promptCapabilities"].clone();
    let opened = match client.ask(
        "session/new",
        &json!({ "cwd": directory, "mcpServers": [] }),
        patience,
    )? {
        Answer::Result(result) => result,
        Answer::Error(LOGIN_REQUIRED, _) => {
            return Err(format!("{} needs you to sign in first", agent.name));
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
    /// The ticket the next question is put under.
    tickets: u64,
    /// Whether the agent takes pictures inside a prompt.
    images: bool,
    /// Whether the agent takes the contents of files inside a prompt.
    embeds: bool,
}

/// Passes commands to the agent and what it says back, until the chat is
/// dropped or the agent stops.
fn converse(
    client: &mut Client,
    session: &str,
    configurable: bool,
    prompts: &Value,
    commands: &Receiver<Command>,
    events: &Sender<Event>,
) -> Result<(), String> {
    let mut waiting = Waiting {
        images: prompts["image"] == json!(true),
        embeds: prompts["embeddedContext"] == json!(true),
        turn: None,
        sets: HashSet::new(),
        asked: HashMap::new(),
        tickets: 0,
    };
    loop {
        loop {
            match commands.try_recv() {
                Ok(command) => command_agent(client, session, configurable, &mut waiting, command)?,
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => return Ok(()),
            }
        }
        let message = match client.receive(LISTEN) {
            Heard::Message(message) => message,
            Heard::Silence => continue,
            Heard::Closed => return Err("the agent stopped".to_owned()),
        };
        hear_agent(client, &mut waiting, &message, events)?;
    }
}

/// Carries out one of the reader's commands.
fn command_agent(
    client: &mut Client,
    session: &str,
    configurable: bool,
    waiting: &mut Waiting,
    command: Command,
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
        Command::Cancel => {
            for (_, id) in waiting.asked.drain() {
                client.respond(&id, &outcome(None))?;
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
    waiting: &mut Waiting,
    message: &Value,
    events: &Sender<Event>,
) -> Result<(), String> {
    match (message["method"].as_str(), message.get("id")) {
        (Some("session/request_permission"), Some(id)) => {
            waiting.tickets += 1;
            waiting.asked.insert(waiting.tickets, id.clone());
            let _ = events.send(Event::Ask(permission(waiting.tickets, &message["params"])));
        }
        (Some(_), Some(id)) => client.refuse(id, NO_SUCH_METHOD, "not supported")?,
        (Some("session/update"), None) => {
            for event in update(&message["params"]["update"]) {
                let _ = events.send(event);
            }
        }
        (Some(_), None) => {}
        (None, Some(id)) => {
            let id = id.as_u64();
            if id.is_some() && id == waiting.turn {
                waiting.turn = None;
                let error = match answer(message) {
                    Answer::Result(_) => None,
                    Answer::Error(_, message) => Some(message),
                };
                let _ = events.send(Event::Finished(error));
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
        "config_option_update" => update["configOptions"]
            .is_array()
            .then(|| Event::Settings(settings(update))),
        _ => None,
    };
    event.into_iter().collect()
}

/// The tool a `tool_call` or `tool_call_update` describes.
fn tool(update: &Value) -> Option<Tool> {
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
        .map(str::to_owned);
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
    })
}
