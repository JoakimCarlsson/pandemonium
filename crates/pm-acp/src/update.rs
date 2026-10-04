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

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::time::{Duration, Instant};

use serde_json::Value;

use crate::elicitation::Elicitation;
use crate::limits::Limits;
use crate::request::Request;

/// The tool calls of one session, keyed by the identity the agent gave each.
pub type Tools = BTreeMap<String, ToolCall>;

/// Something the agent has said or asked, on its way to the window.
#[derive(Clone, Debug)]
pub enum Event {
    /// A queued editor prompt is ready for its normal delivery and checkpoint seam.
    PromptReady(String),
    /// The session is open and will take prompts.
    Ready,
    /// Saved sessions returned by the agent, with whether more pages follow.
    Listed(Vec<History>, bool),
    /// Listing saved sessions failed.
    ListFailed(String),
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
    /// The title the agent has given the conversation, replacing the last.
    Titled(String),
    /// How much of the model's context the conversation fills, and what it
    /// has cost so far.
    Used(Usage),
    /// How much of the plan's rate limits has been used, as the agent's own
    /// source for them now reports it.
    Limited(Limits),
    /// A tool call the agent will not run until the reader allows it.
    Asked(Ask),
    /// Something the agent needs from the reader before it can go on.
    Elicited(Elicitation),
    /// The agent has what a link it sent the reader to was for.
    Concluded(String),
    /// A file or terminal request the window is to carry out and answer,
    /// under the ticket given.
    Requested(u64, Request),
    /// The agent could not take the conversation up again, so the one it
    /// opened is new.
    Fresh,
    /// The agent is logged out, and a conversation is being opened again.
    LoggedOut,
    /// The agent has forgotten the saved session of this name.
    Deleted(String),
    /// The turn is over, for the reason given.
    Stopped(Stop),
    /// The agent failed at something it was asked to do.
    Failed(String),
    /// The agent's process has gone.
    Ended,
}

/// A saved agent session that can be loaded again.
#[derive(Clone, Debug)]
pub struct History {
    /// The identity the agent loads the session by.
    pub id: String,
    /// The directory the session was working in.
    pub cwd: PathBuf,
    /// The title the agent gave it, where one is available.
    pub title: Option<String>,
    /// When the agent last updated it, where one is available.
    pub updated_at: Option<String>,
}

/// Reads one page of saved sessions from an agent's list response.
pub(crate) fn history(result: &Value) -> Vec<History> {
    result["sessions"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|session| {
            Some(History {
                id: session["sessionId"].as_str()?.to_owned(),
                cwd: PathBuf::from(session["cwd"].as_str()?),
                title: session["title"].as_str().map(str::to_owned),
                updated_at: session["updatedAt"].as_str().map(str::to_owned),
            })
        })
        .collect()
}

/// How much of the model's context a conversation fills.
#[derive(Clone, Debug, PartialEq)]
pub struct Usage {
    /// The tokens the context holds now.
    pub used: u64,
    /// The tokens it can hold.
    pub size: u64,
    /// What the conversation has cost so far, where the agent says.
    pub cost: Option<Cost>,
}

/// An amount of money a conversation has cost.
#[derive(Clone, Debug, PartialEq)]
pub struct Cost {
    /// How much.
    pub amount: f64,
    /// In what, as an ISO 4217 code.
    pub currency: String,
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
    /// How the login is carried out.
    pub way: Way,
}

/// How a login is carried out.
#[derive(Clone, Debug, PartialEq)]
pub enum Way {
    /// The agent does it when asked, by whatever flow it has of its own.
    Asked,
    /// The agent's own program is run in a terminal with these arguments and
    /// variables, for the reader to log in through; the agent is started
    /// again once that has finished.
    Terminal {
        /// What the program is run with, after the arguments it is always
        /// run with.
        args: Vec<String>,
        /// Variables it is run with.
        env: Vec<(String, String)>,
    },
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
    /// The one thing the tool was given to work on, where the agent passed
    /// the tool's input on: the command it ran, the pattern it searched for,
    /// the address it fetched.
    pub argument: Option<String>,
    /// What the tool gave back, where the agent passed that on as it was
    /// rather than as output of its own.
    pub returned: Option<String>,
    /// When the call first became pending or running.
    pub started: Option<Instant>,
    /// When the call first stopped running, however it stopped.
    pub finished: Option<Instant>,
    /// What the agent reported when the call failed, as it reported it.
    pub error: Option<String>,
    /// The call that launched the subagent making this call.
    pub parent: Option<String>,
    /// Whether this card represents a delegated agent's work.
    pub subagent: bool,
}

impl ToolCall {
    /// Whether the call is awaiting completion.
    pub fn is_running(&self) -> bool {
        matches!(self.status, Status::Pending | Status::Running)
    }

    /// The elapsed time, frozen at the first completion update.
    pub fn elapsed(&self) -> Duration {
        self.started.map_or(Duration::ZERO, |started| {
            self.finished
                .unwrap_or_else(Instant::now)
                .saturating_duration_since(started)
        })
    }
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
    /// Stopped before it finished, by the reader or on their behalf.
    Cancelled,
    /// Cut off with its agent, so how it ended is not known.
    Disconnected,
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
    /// A terminal the agent started, showing what its command writes as it
    /// runs.
    Terminal(String),
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
        "tool_call" | "tool_call_update" => {
            let call = merge(update, tools)?;
            (!call.title.is_empty()
                || call.name.as_ref().is_some_and(|name| !name.is_empty())
                || call.kind != Kind::Other
                || call.argument.is_some()
                || !call.locations.is_empty()
                || !call.output.is_empty()
                || call.returned.is_some())
            .then_some(Event::Ran(call))
        }
        "plan" => Some(Event::Planned(steps(&update["entries"]))),
        "available_commands_update" => Some(Event::Offers(commands(&update["availableCommands"]))),
        "current_mode_update" => Some(Event::Mode(update["currentModeId"].as_str()?.to_owned())),
        "config_option_update" => Some(Event::Knobs(knobs(&update["configOptions"]))),
        "session_info_update" => Some(Event::Titled(update["title"].as_str()?.to_owned())),
        "usage_update" => Some(Event::Used(usage(update)?)),
        _ => None,
    }
}

/// The login methods an agent answered its handshake with.
///
/// A method of a kind the editor cannot carry out — a key it would have to
/// ask the reader for, say — is left out rather than offered and failed.
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
                way: way(method)?,
            })
        })
        .collect()
}

/// How the login `method` describes is carried out, where the editor can.
fn way(method: &Value) -> Option<Way> {
    match method["type"].as_str() {
        None | Some("agent") => Some(Way::Asked),
        Some("terminal") => Some(Way::Terminal {
            args: method["args"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|arg| arg.as_str().map(str::to_owned))
                .collect(),
            env: variables(&method["env"]),
        }),
        Some(_) => None,
    }
}

/// The variables `env` sets, written either as a map or as a list of names
/// and values.
fn variables(env: &Value) -> Vec<(String, String)> {
    match env {
        Value::Object(map) => map
            .iter()
            .filter_map(|(name, value)| Some((name.clone(), value.as_str()?.to_owned())))
            .collect(),
        Value::Array(list) => list
            .iter()
            .filter_map(|variable| {
                Some((
                    variable["name"].as_str()?.to_owned(),
                    variable["value"].as_str().unwrap_or_default().to_owned(),
                ))
            })
            .collect(),
        _ => Vec::new(),
    }
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
        argument: None,
        returned: None,
        started: None,
        finished: None,
        error: None,
        parent: None,
        subagent: false,
    });

    if let Some(title) = update["title"].as_str() {
        call.title = title.to_owned();
    }
    if let Some(name) = update["name"].as_str() {
        call.name = Some(name.to_owned());
    }
    call.subagent |= matches!(call.name.as_deref(), Some("Task" | "Agent"))
        || update["rawInput"]["subagent_type"].is_string();
    if let Some(kind) = update["kind"].as_str() {
        call.kind = self::kind(kind);
    }
    if let Some(status) = update["status"].as_str() {
        settle(
            call,
            match self::status(status) {
                Status::Failed if cancelled(&update["_meta"]) => Status::Cancelled,
                status => status,
            },
        );
        if call.status != Status::Failed {
            call.error = None;
        } else if let Some(error) = error(update) {
            call.error = Some(error);
        }
    }
    if let Some(output) = update["content"].as_array() {
        call.output = output.iter().filter_map(self::output).collect();
    }
    if let Some(locations) = update["locations"].as_array() {
        call.locations = locations.iter().filter_map(location).collect();
    }
    if let Some(argument) = argument(&update["rawInput"]) {
        call.argument = Some(argument);
    }
    if let Some(returned) = returned(&update["rawOutput"]) {
        call.returned = Some(returned);
    }
    if let Some(parent) = update["_meta"]["claudeCode"]["parentToolUseId"].as_str() {
        call.parent = Some(parent.to_owned());
    }
    Some(call.clone())
}

/// Puts `call` at `status`, starting its clock when it starts and stopping
/// it the first time it stops.
fn settle(call: &mut ToolCall, status: Status) {
    call.status = status;
    match status {
        Status::Pending | Status::Running => {
            call.started.get_or_insert_with(Instant::now);
        }
        Status::Done | Status::Failed | Status::Cancelled | Status::Disconnected => {
            call.finished.get_or_insert_with(Instant::now);
        }
    }
}

/// The reasons Claude's adapter stamps on a failed call that never ran
/// because the reader stopped or refused it, rather than because it broke.
const CANCELLED: [&str; 3] = ["interrupted", "cancelled", "user-rejected"];

/// Whether `meta` says a failed call was stopped rather than broken.
fn cancelled(meta: &Value) -> bool {
    meta["claudeCode"]["nonExecutionKind"]
        .as_str()
        .is_some_and(|kind| CANCELLED.contains(&kind))
}

/// The error the update failing a call carried: the text it was given as
/// content, or else what the tool gave back.
fn error(update: &Value) -> Option<String> {
    let said = update["content"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(output)
        .filter_map(|output| match output {
            Output::Said(text) => Some(text),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n");
    (!said.trim().is_empty())
        .then_some(said)
        .or_else(|| returned(&update["rawOutput"]))
}

/// Stops every call still running at `status`, or, given a `root`, that
/// call and every running call made beneath it, with an event for each.
///
/// This is for an ending the agent has confirmed for the calls as a whole: a
/// turn it says was cancelled, a subagent it says was, or a process that has
/// gone. A call beneath `root` that has already stopped keeps how it
/// stopped; `root` itself takes the ending it was given.
pub(crate) fn halt(tools: &mut Tools, root: Option<&str>, status: Status) -> Vec<Event> {
    let within = root.map(|root| beneath(tools, root));
    tools
        .values_mut()
        .filter(|call| {
            within
                .as_ref()
                .is_none_or(|within| within.contains(&call.id))
                && (call.is_running() || root == Some(call.id.as_str()))
        })
        .map(|call| {
            settle(call, status);
            Event::Ran(call.clone())
        })
        .collect()
}

/// The identities of `root` and of every call made beneath it, at whatever
/// depth.
fn beneath(tools: &Tools, root: &str) -> BTreeSet<String> {
    let mut found = BTreeSet::from([root.to_owned()]);
    loop {
        let more = tools
            .values()
            .filter(|call| {
                !found.contains(&call.id)
                    && call
                        .parent
                        .as_ref()
                        .is_some_and(|parent| found.contains(parent))
            })
            .map(|call| call.id.clone())
            .collect::<Vec<_>>();
        if more.is_empty() {
            return found;
        }
        found.extend(more);
    }
}

/// The input fields that name what a tool works on, most telling first.
const ARGUMENTS: [&str; 7] = [
    "command",
    "pattern",
    "query",
    "url",
    "description",
    "file_path",
    "path",
];

/// The one thing `input` gave a tool to work on, where it names one.
///
/// A tool's input is the tool's own shape, not the protocol's, so this reads
/// the few fields every agent's shell, search and fetch tools agree on. A
/// command given as a list of words is those words, as a shell would read
/// them back.
fn argument(input: &Value) -> Option<String> {
    if let Some(argument) = input.as_str() {
        return Some(argument.to_owned());
    }
    ARGUMENTS.iter().find_map(|field| match &input[*field] {
        Value::String(argument) => Some(argument.clone()),
        Value::Array(words) => {
            let words = words.iter().filter_map(Value::as_str).collect::<Vec<_>>();
            (!words.is_empty()).then(|| words.join(" "))
        }
        _ => None,
    })
}

/// What `output` comes to as text, where a tool gave anything back.
///
/// A tool that returned text is that text; one that returned a structure is
/// the output fields shells report, or the structure written out whole.
fn returned(output: &Value) -> Option<String> {
    match output {
        Value::Null => None,
        Value::String(text) => Some(text.clone()),
        Value::Object(fields)
            if ![
                "exitCode",
                "exit_code",
                "exitStatus",
                "exit_status",
                "error",
                "matches",
                "files",
                "numMatches",
                "matchCount",
                "match_count",
                "count",
            ]
            .iter()
            .any(|key| fields.contains_key(*key)) =>
        {
            ["output", "stdout", "result"]
                .iter()
                .find_map(|field| fields.get(*field)?.as_str().map(str::to_owned))
                .or_else(|| Some(output.to_string()))
        }
        output => Some(output.to_string()),
    }
}

/// The context usage `update` reports, where it reports a size to fill.
fn usage(update: &Value) -> Option<Usage> {
    Some(Usage {
        used: update["used"].as_u64()?,
        size: update["size"].as_u64().filter(|size| *size > 0)?,
        cost: cost(&update["cost"]),
    })
}

/// The cost `cost` reports, where it reports one whole.
fn cost(cost: &Value) -> Option<Cost> {
    Some(Cost {
        amount: cost["amount"].as_f64()?,
        currency: cost["currency"].as_str()?.to_owned(),
    })
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
        "terminal" => Some(Output::Terminal(output["terminalId"].as_str()?.to_owned())),
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
