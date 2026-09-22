//! What an agent says, in the editor's own terms.
//!
//! Everything the agent sends during a turn arrives as one shape — a session
//! update carrying a tagged variant — and leaves this module as an [`Event`].
//! The protocol's spelling stops here: nothing above reads a JSON key.
//!
//! A tool call is the exception worth naming. The agent announces it once and
//! then amends it, field by field, as it runs; a reader wants the tool call as
//! it now stands, not a diff against what it was. [`Tools`] keeps the running
//! picture and every event carries the whole of it.

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde_json::Value;

/// The tool calls of one session, by the identity the agent gave each.
pub type Tools = BTreeMap<String, ToolCall>;

/// Something the agent has said or asked, on its way to the window.
#[derive(Clone, Debug)]
pub enum Event {
    /// The session is open and will take prompts.
    Ready,
    /// The agent will not open a session until it is logged in.
    Login(Vec<Method>),
    /// A run of text, of whichever voice [`Voice`] names.
    Said(Voice, String),
    /// A tool call, as it now stands.
    Ran(ToolCall),
    /// The plan the agent is working to, replacing the one before it.
    Planned(Vec<Step>),
    /// The commands this agent takes, as it now offers them.
    Offers(Vec<Command>),
    /// The mode the session has changed to.
    Mode(String),
    /// What the session can be set to, as the agent now offers it.
    Knobs(Vec<Knob>),
    /// A tool call the agent will not run until the reader allows it.
    Asked(Ask),
    /// The turn is over, for the reason given.
    Stopped(Stop),
    /// The agent failed at something it was asked to do.
    Failed(String),
    /// The agent's process has gone.
    Ended,
}

/// Who is speaking in a run of text.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Voice {
    /// The reader, echoed back by an agent that replays what it was told.
    Reader,
    /// The agent, in its answer.
    Agent,
    /// The agent, thinking out loud on the way to one.
    Thought,
}

/// A way of logging in that the agent offers.
#[derive(Clone, Debug)]
pub struct Method {
    /// What the agent calls this method when it is asked for.
    pub id: String,
    /// What a button offering it says.
    pub name: String,
    /// What it does, where the agent explains it.
    pub description: Option<String>,
}

/// A mode a session can be put into.
#[derive(Clone, Debug)]
pub struct Mode {
    /// What the agent calls this mode when it is set.
    pub id: String,
    /// What a menu offering it says.
    pub name: String,
    /// What it means, where the agent explains it.
    pub description: Option<String>,
}

/// Something about the session a reader can set.
///
/// Which model the agent is using, how hard it is made to think and whatever
/// switch it offers are one kind of thing to the protocol and one kind of
/// thing here: a named control, what it is set to, and what else it takes.
/// The mode is one of these too, for an agent that says so this way rather
/// than through [`Mode`].
#[derive(Clone, Debug)]
pub struct Knob {
    /// What the agent calls this knob when it is set.
    pub id: String,
    /// What a chip and a menu call it.
    pub name: String,
    /// What it does, where the agent explains it.
    pub description: Option<String>,
    /// What it is about, as far as the editor cares.
    pub about: About,
    /// What it is set to, and what else it can be set to.
    pub setting: Setting,
}

/// What a knob is about.
///
/// The agent says this so that a client can put the model where a reader
/// expects the model to be; a knob it says nothing about is still shown, and
/// is still set the same way.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum About {
    /// Which model the agent is talking to.
    Model,
    /// Which mode the session is in.
    Mode,
    /// How hard the model is made to think.
    Thinking,
    /// Something only the agent knows the meaning of.
    Other,
}

/// What a knob is set to.
#[derive(Clone, Debug)]
pub enum Setting {
    /// One value out of a list of them.
    Picked {
        /// What it is set to now.
        value: String,
        /// What it can be set to.
        picks: Vec<Pick>,
    },
    /// On or off.
    Switched(bool),
}

/// One value a knob can be set to.
#[derive(Clone, Debug)]
pub struct Pick {
    /// What the agent calls this value when it is set.
    pub id: String,
    /// What a menu row says.
    pub name: String,
    /// What it means, where the agent explains it.
    pub description: Option<String>,
}

/// One tool call, as the agent has described it so far.
#[derive(Clone, Debug)]
pub struct ToolCall {
    /// What the agent calls this call, for as long as it runs.
    pub id: String,
    /// The one line a row for it shows.
    pub title: String,
    /// What the agent calls the tool itself, where it says.
    pub name: Option<String>,
    /// What kind of work it is, which is what an icon is chosen by.
    pub kind: Kind,
    /// How far along it is.
    pub status: Status,
    /// What it has produced.
    pub output: Vec<Output>,
    /// The files it is working on.
    pub locations: Vec<Location>,
}

/// What kind of work a tool call does.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Kind {
    /// Reading a file.
    Read,
    /// Changing one.
    Edit,
    /// Deleting one.
    Delete,
    /// Moving one.
    Move,
    /// Searching.
    Search,
    /// Running a command.
    Execute,
    /// Thinking.
    Think,
    /// Fetching something from elsewhere.
    Fetch,
    /// Changing the session's mode.
    SwitchMode,
    /// Anything the protocol has no name for.
    Other,
}

/// How far along a tool call is.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Status {
    /// Announced, not started.
    Pending,
    /// Running.
    Running,
    /// Finished.
    Done,
    /// Finished badly.
    Failed,
}

/// Something a tool call has produced.
#[derive(Clone, Debug)]
pub enum Output {
    /// Text the call wrote.
    Said(String),
    /// A change it proposes or has made to one file.
    Changed {
        /// The file the change is to.
        path: PathBuf,
        /// What the file held before, where the agent said.
        before: Option<String>,
        /// What it holds after.
        after: String,
    },
}

/// A file a tool call names, and where in it.
#[derive(Clone, Debug)]
pub struct Location {
    /// The file.
    pub path: PathBuf,
    /// The line, where the call named one.
    pub line: Option<u32>,
}

/// One step of the agent's plan.
#[derive(Clone, Debug)]
pub struct Step {
    /// What the step is.
    pub text: String,
    /// How far along it is.
    pub status: Status,
}

/// A command the agent takes, of the kind a reader types with a slash.
#[derive(Clone, Debug)]
pub struct Command {
    /// What it is called, without the slash.
    pub name: String,
    /// What it does.
    pub description: String,
}

/// A tool call the agent is waiting to be allowed to run.
#[derive(Clone, Debug)]
pub struct Ask {
    /// The ticket the answer is given under.
    pub id: u64,
    /// The call being asked about, as far as the agent has described it.
    pub tool: ToolCall,
    /// The answers the agent will accept.
    pub choices: Vec<Choice>,
}

/// One answer to a permission request.
#[derive(Clone, Debug)]
pub struct Choice {
    /// What the agent calls this answer when it is given.
    pub id: String,
    /// What the button says.
    pub name: String,
    /// What choosing it means, which is what a button is styled by.
    pub kind: Weight,
}

/// What an answer to a permission request amounts to.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Weight {
    /// Allowed, this once.
    AllowOnce,
    /// Allowed, and do not ask again.
    AllowAlways,
    /// Refused, this once.
    RejectOnce,
    /// Refused, and do not ask again.
    RejectAlways,
}

/// Why a turn ended.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Stop {
    /// The agent had said what it had to say.
    EndTurn,
    /// It ran out of tokens.
    MaxTokens,
    /// It ran out of turns.
    MaxRequests,
    /// It refused to go on.
    Refusal,
    /// The reader stopped it.
    Cancelled,
}

impl Stop {
    /// The stop reason `reason` names, or the ordinary one.
    pub(crate) fn read(reason: &Value) -> Self {
        match reason.as_str() {
            Some("max_tokens") => Self::MaxTokens,
            Some("max_turn_requests") => Self::MaxRequests,
            Some("refusal") => Self::Refusal,
            Some("cancelled") => Self::Cancelled,
            _ => Self::EndTurn,
        }
    }
}

/// The event one session update comes to, against the tool calls so far.
///
/// An update the editor has no use for is not an event: the agent is free to
/// tell a client more than it draws.
pub(crate) fn event(update: &Value, tools: &mut Tools) -> Option<Event> {
    match update["sessionUpdate"].as_str()? {
        "user_message_chunk" => Some(Event::Said(Voice::Reader, text(&update["content"])?)),
        "agent_message_chunk" => Some(Event::Said(Voice::Agent, text(&update["content"])?)),
        "agent_thought_chunk" => Some(Event::Said(Voice::Thought, text(&update["content"])?)),
        "tool_call" | "tool_call_update" => Some(Event::Ran(merge(update, tools)?)),
        "plan" => Some(Event::Planned(steps(&update["entries"]))),
        "available_commands_update" => Some(Event::Offers(commands(&update["availableCommands"]))),
        "current_mode_update" => Some(Event::Mode(update["currentModeId"].as_str()?.to_owned())),
        "config_option_update" => Some(Event::Knobs(knobs(&update["configOptions"]))),
        _ => None,
    }
}

/// The login methods an agent answered its handshake with.
pub(crate) fn methods(methods: &Value) -> Vec<Method> {
    methods
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|method| {
            Some(Method {
                id: method["id"].as_str()?.to_owned(),
                name: method["name"].as_str().unwrap_or("Log in").to_owned(),
                description: method["description"].as_str().map(str::to_owned),
            })
        })
        .collect()
}

/// The modes an agent opened a conversation with.
pub(crate) fn modes(modes: &Value) -> Vec<Mode> {
    modes["availableModes"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|mode| {
            Some(Mode {
                id: mode["id"].as_str()?.to_owned(),
                name: mode["name"].as_str().unwrap_or_default().to_owned(),
                description: mode["description"].as_str().map(str::to_owned),
            })
        })
        .collect()
}

/// The knobs `options` lists, in the order the agent put them in.
///
/// A knob of a kind the editor cannot draw is dropped rather than shown
/// broken: the protocol grows a new one whenever a client learns to set a new
/// kind of thing, and an agent is free to offer more than the window takes.
pub(crate) fn knobs(options: &Value) -> Vec<Knob> {
    options
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|option| {
            Some(Knob {
                id: option["id"].as_str()?.to_owned(),
                name: option["name"].as_str().unwrap_or_default().to_owned(),
                description: option["description"].as_str().map(str::to_owned),
                about: about(option["category"].as_str()),
                setting: setting(option)?,
            })
        })
        .collect()
}

/// What `category` says a knob is about.
fn about(category: Option<&str>) -> About {
    match category {
        Some("model") => About::Model,
        Some("mode") => About::Mode,
        Some("thought_level") => About::Thinking,
        _ => About::Other,
    }
}

/// What `option` says a knob is set to, where it says anything the editor
/// can set.
fn setting(option: &Value) -> Option<Setting> {
    match option["type"].as_str()? {
        "select" => Some(Setting::Picked {
            value: option["currentValue"].as_str()?.to_owned(),
            picks: picks(&option["options"]),
        }),
        "boolean" => Some(Setting::Switched(option["currentValue"].as_bool()?)),
        _ => None,
    }
}

/// The values `options` lists, whether or not they come in groups.
///
/// A group is a heading over the values it holds; the window shows one flat
/// list, so the headings are dropped and the values are kept in the order the
/// groups put them in.
fn picks(options: &Value) -> Vec<Pick> {
    options
        .as_array()
        .into_iter()
        .flatten()
        .flat_map(|option| match option["options"].as_array() {
            Some(grouped) => grouped.iter().filter_map(pick).collect::<Vec<_>>(),
            None => pick(option).into_iter().collect(),
        })
        .collect()
}

/// The one value `option` describes.
fn pick(option: &Value) -> Option<Pick> {
    Some(Pick {
        id: option["value"].as_str()?.to_owned(),
        name: option["name"].as_str().unwrap_or_default().to_owned(),
        description: option["description"].as_str().map(str::to_owned),
    })
}

/// The permission request `params` asks, read against the tool calls so far.
pub(crate) fn ask(id: u64, params: &Value, tools: &mut Tools) -> Option<Ask> {
    Some(Ask {
        id,
        tool: merge(&params["toolCall"], tools)?,
        choices: params["options"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|choice| {
                Some(Choice {
                    id: choice["optionId"].as_str()?.to_owned(),
                    name: choice["name"].as_str().unwrap_or("Allow").to_owned(),
                    kind: match choice["kind"].as_str() {
                        Some("allow_always") => Weight::AllowAlways,
                        Some("reject_once") => Weight::RejectOnce,
                        Some("reject_always") => Weight::RejectAlways,
                        _ => Weight::AllowOnce,
                    },
                })
            })
            .collect(),
    })
}

/// The tool call `update` describes, folded into the one it amends.
///
/// The protocol sends a field it has nothing new to say about as nothing at
/// all, so an absent field is the old value and never a cleared one.
fn merge(update: &Value, tools: &mut Tools) -> Option<ToolCall> {
    let id = update["toolCallId"].as_str()?.to_owned();
    let call = tools.entry(id.clone()).or_insert_with(|| ToolCall {
        id,
        title: String::new(),
        name: None,
        kind: Kind::Other,
        status: Status::Pending,
        output: Vec::new(),
        locations: Vec::new(),
    });

    if let Some(title) = update["title"].as_str() {
        call.title = title.to_owned();
    }
    if let Some(name) = update["name"].as_str() {
        call.name = Some(name.to_owned());
    }
    if let Some(kind) = update["kind"].as_str() {
        call.kind = self::kind(kind);
    }
    if let Some(status) = update["status"].as_str() {
        call.status = self::status(status);
    }
    if let Some(output) = update["content"].as_array() {
        call.output = output.iter().filter_map(self::output).collect();
    }
    if let Some(locations) = update["locations"].as_array() {
        call.locations = locations.iter().filter_map(location).collect();
    }
    Some(call.clone())
}

/// The plan steps `entries` lists.
fn steps(entries: &Value) -> Vec<Step> {
    entries
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|entry| {
            Some(Step {
                text: entry["content"].as_str()?.to_owned(),
                status: status(entry["status"].as_str().unwrap_or_default()),
            })
        })
        .collect()
}

/// The commands `available` lists.
fn commands(available: &Value) -> Vec<Command> {
    available
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|command| {
            Some(Command {
                name: command["name"].as_str()?.to_owned(),
                description: command["description"]
                    .as_str()
                    .unwrap_or_default()
                    .to_owned(),
            })
        })
        .collect()
}

/// What one piece of a tool call's output comes to.
fn output(output: &Value) -> Option<Output> {
    match output["type"].as_str()? {
        "content" => Some(Output::Said(text(&output["content"])?)),
        "diff" => Some(Output::Changed {
            path: PathBuf::from(output["path"].as_str()?),
            before: output["oldText"].as_str().map(str::to_owned),
            after: output["newText"].as_str().unwrap_or_default().to_owned(),
        }),
        _ => None,
    }
}

/// The file `location` names, and where in it.
fn location(location: &Value) -> Option<Location> {
    Some(Location {
        path: PathBuf::from(location["path"].as_str()?),
        line: location["line"].as_u64().map(|line| line as u32),
    })
}

/// What one content block reads as, where it reads as anything.
///
/// A picture and a sound have no text in them, and the block that carries
/// one is dropped rather than announced: a transcript says what was said.
fn text(content: &Value) -> Option<String> {
    match content["type"].as_str()? {
        "text" => content["text"].as_str().map(str::to_owned),
        "resource" => content["resource"]["text"].as_str().map(str::to_owned),
        "resource_link" => content["uri"].as_str().map(str::to_owned),
        _ => None,
    }
}

/// The kind of work `kind` names.
fn kind(kind: &str) -> Kind {
    match kind {
        "read" => Kind::Read,
        "edit" => Kind::Edit,
        "delete" => Kind::Delete,
        "move" => Kind::Move,
        "search" => Kind::Search,
        "execute" => Kind::Execute,
        "think" => Kind::Think,
        "fetch" => Kind::Fetch,
        "switch_mode" => Kind::SwitchMode,
        _ => Kind::Other,
    }
}

/// The progress `status` names.
fn status(status: &str) -> Status {
    match status {
        "in_progress" => Status::Running,
        "completed" => Status::Done,
        "failed" => Status::Failed,
        _ => Status::Pending,
    }
}
