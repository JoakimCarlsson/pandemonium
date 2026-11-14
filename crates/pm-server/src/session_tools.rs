//! Tools that let an agent start sessions of its own and see how they are doing, offered to agents as an MCP server on the loopback address.
//!
//! A session an agent starts is an ordinary session: it shows in every window,
//! has its own agent, conversation and folder, and goes on when the session that
//! started it stops. By default it works in a worktree of its own, made from what
//! the starting session has checked out, with the changes it has not committed
//! carried over. The server runs the tools itself, so they work with no window
//! connected.

use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock, OnceLock};

use pm_harness::Event;
use pm_protocol::Response;
use serde_json::{Value, json};

use crate::conversation;
use crate::core::Core;
use crate::handler::{agent_named, has_worktrees, place, seat, start_seated};
use crate::mcp_endpoint::{Endpoint, failure, success, tool};
use crate::sessions::Meta;

/// The most sessions one session can have open that its agent started.
const MOST_OPEN: usize = 8;

/// The most characters of a session's final message that the status gives, counted from its end.
const FINAL_TAIL: usize = 4000;

/// The longest a title made from a prompt is, in characters.
const TITLE_LENGTH: usize = 60;

/// The server's sessions, once the server has started.
static CORE: OnceLock<Arc<Core>> = OnceLock::new();

/// The endpoint agents reach the tools at.
static ENDPOINT: LazyLock<Endpoint> =
    LazyLock::new(|| Endpoint::new("pandemonium-sessions", definitions, call));

/// Starts offering the tools, which work on the sessions `core` holds.
pub(crate) fn start(core: Arc<Core>) {
    let _ = CORE.set(core);
    ENDPOINT.start();
}

/// The address of the tools and the token of `session`, to be given to the agent of that session when it starts; nothing until the tools are offered.
pub(crate) fn offered(session: u64) -> Option<(String, String)> {
    ENDPOINT.offered(session)
}

/// Where a session an agent starts works.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Place {
    /// In a worktree of its own, on a branch of its own.
    Worktree,
    /// In the project's folder itself.
    Local,
    /// In a scratch folder of its own that belongs to no project.
    Chat,
}

impl Place {
    /// The place `name` names, a worktree when no name is given.
    ///
    /// # Errors
    ///
    /// Says which names there are when `name` is none of them.
    fn named(name: Option<&str>) -> Result<Self, String> {
        match name.unwrap_or("worktree") {
            "worktree" => Ok(Self::Worktree),
            "local" => Ok(Self::Local),
            "chat" => Ok(Self::Chat),
            other => Err(format!("`where` is worktree, local or chat, not {other:?}")),
        }
    }
}

/// What an agent asked `session_start` for.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct Asked {
    /// The first prompt of the new session.
    pub(crate) prompt: String,
    /// What the session is called, when the agent named it.
    pub(crate) title: Option<String>,
    /// Where it works, by name.
    pub(crate) place: Option<String>,
    /// The branch or commit its worktree is made from, when the agent named one.
    pub(crate) base: Option<String>,
    /// The project it works in, by folder or folder name, when the agent named one.
    pub(crate) project: Option<String>,
    /// Whether its agent runs in the project's dev container, when the agent said.
    pub(crate) container: Option<bool>,
    /// The submodule its agent works in, when the agent named one.
    pub(crate) submodule: Option<String>,
    /// The model its agent uses, when the agent named one.
    pub(crate) model: Option<String>,
    /// Whether the changes not committed where its worktree is made from are carried into it.
    pub(crate) carry: bool,
}

/// What `session_start` was called with, read from `arguments`.
///
/// # Errors
///
/// Says so when there is no prompt.
pub(crate) fn asked(arguments: &Value) -> Result<Asked, String> {
    let text = |key: &str| {
        arguments[key]
            .as_str()
            .map(str::trim)
            .filter(|text| !text.is_empty())
            .map(str::to_owned)
    };
    let prompt = text("prompt").ok_or("`prompt` is needed: what the new session is to do")?;
    Ok(Asked {
        prompt,
        title: text("title"),
        place: text("where"),
        base: text("base_branch"),
        project: text("project"),
        container: arguments["container"].as_bool(),
        submodule: text("submodule"),
        model: text("model"),
        carry: arguments["carry_changes"].as_bool().unwrap_or(true),
    })
}

/// What starting a session needs to know of the session whose agent asks.
#[derive(Clone, Debug, Default)]
pub(crate) struct Caller {
    /// What the session is, apart from what its agent did.
    pub(crate) meta: Meta,
    /// The settings its agent last said it had.
    pub(crate) settings: Vec<(String, String)>,
    /// The models its agent can use, as the agent calls them and as the reader knows them.
    pub(crate) models: Vec<(String, String)>,
}

/// What a session is made from: everything [`seat`] and [`start_seated`] are handed, and the folder whose changes are carried into it.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct Plan {
    /// Where it works.
    pub(crate) place: Option<Place>,
    /// The folder of its project, empty for a chat.
    pub(crate) repository: String,
    /// The folder it works in, when it works in the project's own.
    pub(crate) directory: String,
    /// What it is called.
    pub(crate) title: String,
    /// The branch or commit its worktree is made from.
    pub(crate) base: Option<String>,
    /// The folder whose changes not yet committed are carried into its worktree.
    pub(crate) carry_from: Option<PathBuf>,
    /// The submodule its agent works in.
    pub(crate) submodule: Option<String>,
    /// Whether its agent runs in the project's dev container.
    pub(crate) container: bool,
    /// The settings its agent starts with.
    pub(crate) settings: Vec<(String, String)>,
}

/// A title for a session made from its first prompt: the first line, cut at a word to at most [`TITLE_LENGTH`] characters.
pub(crate) fn title_from(prompt: &str) -> String {
    let line = prompt.lines().map(str::trim).find(|line| !line.is_empty());
    let line = line.unwrap_or("Session");
    if line.chars().count() <= TITLE_LENGTH {
        return line.to_owned();
    }
    let cut: String = line.chars().take(TITLE_LENGTH).collect();
    match cut.rfind(' ') {
        Some(space) if space > 0 => cut[..space].to_owned(),
        _ => cut,
    }
}

/// The project folder `named` names among `projects`: one whose folder is `named`, or whose last folder name is `named`, in any case.
///
/// # Errors
///
/// Says which projects there are when none matches, and which match when more than one does.
pub(crate) fn project_named(named: &str, projects: &[String]) -> Result<String, String> {
    let wanted = place(named);
    let found: Vec<&String> = projects
        .iter()
        .filter(|project| {
            let folder = place(project);
            folder == wanted
                || folder
                    .file_name()
                    .is_some_and(|name| name.to_string_lossy().eq_ignore_ascii_case(named))
        })
        .collect();
    match found.as_slice() {
        [one] => Ok((*one).clone()),
        [] => Err(format!(
            "no project is called {named}; the projects are: {}",
            projects.join(", ")
        )),
        many => Err(format!(
            "{named} names more than one project: {}",
            many.iter()
                .map(|project| project.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        )),
    }
}

/// The settings the caller's agent has, with the model `asked` names in place of its own when one is named.
///
/// # Errors
///
/// Says which models there are when the name matches none of them or more than one.
pub(crate) fn settings_for(
    asked: &Asked,
    caller: &Caller,
) -> Result<Vec<(String, String)>, String> {
    let mut settings = caller.settings.clone();
    let Some(model) = &asked.model else {
        return Ok(settings);
    };
    let value = if caller.models.is_empty() {
        model.clone()
    } else {
        model_named(model, &caller.models)?
    };
    settings.retain(|(id, _)| id != "model");
    settings.push(("model".to_owned(), value));
    Ok(settings)
}

/// The model `named` names among `models`: one the agent or the reader calls exactly that, or else the only one whose names hold it, in any case.
///
/// # Errors
///
/// Says which models there are when none matches, and which match when more than one does.
fn model_named(named: &str, models: &[(String, String)]) -> Result<String, String> {
    if let Some((value, _)) = models
        .iter()
        .find(|(value, name)| value == named || name == named)
    {
        return Ok(value.clone());
    }
    let lower = named.to_lowercase();
    let found: Vec<&String> = models
        .iter()
        .filter(|(value, name)| {
            value.to_lowercase().contains(&lower) || name.to_lowercase().contains(&lower)
        })
        .map(|(value, _)| value)
        .collect();
    match found.as_slice() {
        [one] => Ok((*one).clone()),
        [] => Err(format!(
            "no model is called {named}; the models are: {}",
            models
                .iter()
                .map(|(value, _)| value.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        )),
        many => Err(format!(
            "{named} names more than one model: {}",
            many.iter()
                .map(|value| value.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        )),
    }
}

/// The branch `folder` has checked out, or the commit when it is on none; nothing when it is no repository.
fn checked_out(folder: &Path) -> Option<String> {
    pm_git::current_branch(folder).or_else(|| pm_git::resolve(folder, "HEAD"))
}

/// How the session `asked` describes is made, for the session the caller describes, among the reader's `projects`.
///
/// A worktree is made from `base_branch` when one is named; otherwise from what is checked out in the folder the caller works in, when it works in that project, or in the project's folder, and the changes not committed there are carried into it unless the agent asked not to.
///
/// # Errors
///
/// Says why when `where` or the model names nothing, the project cannot be found, a worktree cannot be made of it, or what to make it from cannot be told.
pub(crate) fn plan(asked: &Asked, caller: &Caller, projects: &[String]) -> Result<Plan, String> {
    let wanted = Place::named(asked.place.as_deref())?;
    let meta = &caller.meta;
    let settings = settings_for(asked, caller)?;
    let title = asked
        .title
        .clone()
        .unwrap_or_else(|| title_from(&asked.prompt));
    if wanted == Place::Chat {
        return Ok(Plan {
            place: Some(wanted),
            title,
            settings,
            ..Plan::default()
        });
    }
    let repository = match &asked.project {
        Some(named) => project_named(named, projects)?,
        None if !meta.repository.is_empty() => meta.repository.clone(),
        None => {
            return Err(format!(
                "this session belongs to no project, so name one with `project`; the projects are: {}",
                projects.join(", ")
            ));
        }
    };
    let container = asked.container.unwrap_or(meta.container);
    let submodule = asked.submodule.clone();
    if wanted == Place::Local {
        return Ok(Plan {
            place: Some(wanted),
            directory: repository.clone(),
            repository,
            title,
            submodule,
            container,
            settings,
            ..Plan::default()
        });
    }
    has_worktrees(&repository)?;
    let source = if repository == meta.repository && !meta.directory.is_empty() {
        place(&meta.directory)
    } else {
        place(&repository)
    };
    let single = pm_git::layout(&source) == pm_git::Layout::Repository;
    let (base, carry_from) = match &asked.base {
        Some(base) => (base.clone(), None),
        None if single => (
            checked_out(&source).ok_or_else(|| {
                format!(
                    "could not tell what {} has checked out; name `base_branch`",
                    source.display()
                )
            })?,
            asked.carry.then(|| source.clone()),
        ),
        None => {
            return Err(format!(
                "{repository} holds several repositories, so name `base_branch`, one reference or `repository=reference` entries separated by commas"
            ));
        }
    };
    Ok(Plan {
        place: Some(wanted),
        directory: repository.clone(),
        repository,
        title,
        base: Some(base),
        carry_from,
        submodule,
        container,
        settings,
    })
}

/// Runs `tool`, which the agent of session `session` called with `arguments`, and describes what came of it.
fn call(tool: &str, arguments: &Value, session: u64) -> Value {
    let Some(core) = CORE.get() else {
        return failure("the server is still starting");
    };
    let outcome = match tool {
        "session_start" => start_for(core, session, arguments),
        "session_status" => Ok(status(core, session, arguments)),
        other => Err(format!("there is no tool {other}")),
    };
    match outcome {
        Ok(text) => success(&text),
        Err(reason) => failure(&reason),
    }
}

/// What starting a session needs to know of the session numbered `number`.
///
/// # Errors
///
/// Says so when there is no such session.
fn caller(core: &Core, number: u64) -> Result<Caller, String> {
    let sessions = core
        .sessions
        .lock()
        .map_err(|_| "the server is shutting down".to_owned())?;
    let held = sessions
        .get(&number)
        .ok_or_else(|| format!("no session is numbered {number}"))?;
    let models = held
        .log
        .iter()
        .rev()
        .find_map(|(_, event)| match event {
            Event::Settings(settings) => Some(settings.clone()),
            _ => None,
        })
        .and_then(|settings| {
            settings
                .into_iter()
                .find(|setting| setting.category == "model")
        })
        .map(|setting| {
            setting
                .picks
                .into_iter()
                .map(|pick| (pick.value, pick.name))
                .collect()
        })
        .unwrap_or_default();
    Ok(Caller {
        meta: held.meta.clone(),
        settings: conversation::settings(&held.log),
        models,
    })
}

/// How many sessions the agent of session `number` started that are still open.
fn open_started_by(core: &Core, number: u64) -> usize {
    core.sessions.lock().map_or(0, |sessions| {
        sessions
            .values()
            .filter(|session| session.meta.started_by == Some(number) && !session.meta.closed)
            .count()
    })
}

/// The open session whose agent is running in `directory`, when there is one.
fn running_in(core: &Core, directory: &str) -> Option<u64> {
    let sessions = core.sessions.lock().ok()?;
    sessions
        .iter()
        .find(|(_, session)| {
            !session.meta.closed
                && place(&session.meta.directory) == place(directory)
                && (session.chat.is_some() || session.busy)
        })
        .map(|(number, _)| *number)
}

/// Starts the session the agent of session `caller` asked for with `arguments` and describes it.
///
/// # Errors
///
/// Says why when the session could not be started.
fn start_for(core: &Arc<Core>, number: u64, arguments: &Value) -> Result<String, String> {
    let asked = asked(arguments)?;
    let caller = caller(core, number)?;
    let meta = caller.meta.clone();
    if let Some(starter) = meta.started_by {
        return Err(format!(
            "this session was started by the agent of session {starter}, and a session an agent started cannot start sessions of its own"
        ));
    }
    if open_started_by(core, number) >= MOST_OPEN {
        return Err(format!(
            "this session already has {MOST_OPEN} sessions open that it started; wait for some to finish and be closed"
        ));
    }
    let projects = pm_store::load_projects().unwrap_or_default();
    let plan = plan(&asked, &caller, &projects)?;
    if plan.place == Some(Place::Local)
        && let Some(other) = running_in(core, &plan.directory)
    {
        return Err(format!(
            "session {other} has an agent running in {}; two agents in one folder overwrite each other's work, so start it in a worktree instead",
            plan.directory
        ));
    }
    let agent =
        agent_named(&meta.agent).ok_or_else(|| format!("no agent is called {}", meta.agent))?;
    let mut seated = seat(
        core,
        agent,
        &plan.repository,
        &plan.directory,
        &plan.title,
        plan.base.as_deref(),
        plan.submodule.as_deref(),
        plan.container,
        plan.place == Some(Place::Chat),
    )?;
    seated.session.meta.started_by = Some(number);
    if let Some(from) = &plan.carry_from {
        let carried = pm_git::carry_changes(from, &seated.folder);
        seated.skipped.extend(carried);
    }
    match start_seated(core, None, seated, &plan.settings, &asked.prompt, &[]) {
        Response::Started {
            session,
            directory,
            branch,
            skipped,
            ..
        } => Ok(started(
            &plan,
            session,
            &directory,
            branch.as_deref(),
            &skipped,
        )),
        Response::Failed { message } => Err(message),
        _ => Err("the session could not be started".to_owned()),
    }
}

/// What the agent is told of the session numbered `session` it started as `plan` says, working in `directory` on `branch`, with what could not be brought into its folder.
fn started(
    plan: &Plan,
    session: u64,
    directory: &str,
    branch: Option<&str>,
    skipped: &[(String, String)],
) -> String {
    let mut text = format!("Started session {session}, \"{}\", ", plan.title);
    text.push_str(&match plan.place {
        Some(Place::Chat) => format!("as a chat in {directory}."),
        Some(Place::Local) => format!("in the project's own folder {directory}."),
        _ => format!(
            "in a worktree at {directory}{} made from {}{}.",
            branch.map_or_else(String::new, |branch| format!(" on branch {branch}")),
            plan.base.as_deref().unwrap_or("the project"),
            if plan.carry_from.is_some() {
                ", with the changes not yet committed carried over"
            } else {
                ""
            }
        ),
    });
    for (what, why) in skipped {
        text.push_str(&format!("\nNot brought over: {what}: {why}"));
    }
    text.push_str("\nIt is working on its prompt now. Call session_status to see how it is doing.");
    text
}

/// The state of a session as `session_status` tells it: whether its agent is on a turn, whether it runs at all, and its log.
pub(crate) fn state(busy: bool, running: bool, log: &[(u64, Event)]) -> String {
    if busy {
        return if waits_for_reader(log) {
            "waiting for you: its agent asked something".to_owned()
        } else {
            "working".to_owned()
        };
    }
    let ended = log.iter().rev().find_map(|(_, event)| match event {
        Event::Finished(None) => Some("finished".to_owned()),
        Event::Finished(Some(reason)) => Some(format!("stopped: {reason}")),
        Event::Failed(reason) => Some(format!("failed: {reason}")),
        Event::SignedOut { .. } => {
            Some("signed out: its account needs signing in again".to_owned())
        }
        _ => None,
    });
    ended.unwrap_or_else(|| {
        if running {
            "starting".to_owned()
        } else {
            "not started".to_owned()
        }
    })
}

/// Whether the turn `log` ends on waits for the reader: the last thing its agent did was ask a question, ask leave or ask for trust, with nothing after it.
fn waits_for_reader(log: &[(u64, Event)]) -> bool {
    log.iter()
        .rev()
        .take_while(|(_, event)| !matches!(event, Event::Prompted { .. }))
        .find_map(|(_, event)| match event {
            Event::Ask(_) | Event::Inquire(_) | Event::Trust(_) => Some(true),
            Event::Said(_)
            | Event::Tool(_)
            | Event::Answered(_)
            | Event::Trusted { .. }
            | Event::Subagent(_) => Some(false),
            _ => None,
        })
        .unwrap_or(false)
}

/// The last `most` characters of `text`, with a mark in front when some were left out.
pub(crate) fn tail(text: &str, most: usize) -> String {
    let count = text.chars().count();
    if count <= most {
        return text.to_owned();
    }
    let kept: String = text.chars().skip(count - most).collect();
    format!("…{kept}")
}

/// What `session_status` reads of one session while the sessions are locked.
struct Seen {
    /// The session's number.
    number: u64,
    /// What it is, apart from what its agent did.
    meta: Meta,
    /// How it stands.
    state: String,
    /// Its agent's final message, when it is not on a turn and wrote one.
    reply: Option<String>,
}

/// The sessions `arguments` names, or else those the agent of session `caller` started that are still open, as `session_status` describes them.
fn status(core: &Core, caller: u64, arguments: &Value) -> String {
    let named: Option<Vec<u64>> = arguments["sessions"]
        .as_array()
        .map(|numbers| numbers.iter().filter_map(Value::as_u64).collect());
    let seen: Vec<Seen> = {
        let Ok(sessions) = core.sessions.lock() else {
            return "the server is shutting down".to_owned();
        };
        let mut seen: Vec<Seen> = sessions
            .iter()
            .filter(|(number, session)| match &named {
                Some(numbers) => numbers.contains(number),
                None => session.meta.started_by == Some(caller) && !session.meta.closed,
            })
            .map(|(number, session)| Seen {
                number: *number,
                meta: session.meta.clone(),
                state: state(session.busy, session.chat.is_some(), &session.log),
                reply: (!session.busy)
                    .then(|| conversation::final_reply(&session.log).ok())
                    .flatten(),
            })
            .collect();
        seen.sort_by_key(|seen| seen.number);
        seen
    };
    if seen.is_empty() {
        return match named {
            Some(_) => "No session has those numbers.".to_owned(),
            None => "This session has started no sessions that are still open.".to_owned(),
        };
    }
    seen.iter().map(described).collect::<Vec<_>>().join("\n\n")
}

/// What `session_status` says of the session `seen`, with what git and GitHub say of its folder.
fn described(seen: &Seen) -> String {
    let meta = &seen.meta;
    let folder = place(&meta.directory);
    let mut lines = vec![
        format!("## Session {}: {}", seen.number, meta.title),
        format!("state: {}", seen.state),
    ];
    if meta.closed {
        lines.push("closed by the reader".to_owned());
    }
    lines.push(match (&meta.branch, &meta.head, meta.chat) {
        (_, _, true) => format!("where: a chat, in {}", meta.directory),
        (Some(branch), _, _) => format!(
            "where: a worktree at {} on branch {branch}, made from {}",
            meta.directory,
            meta.base.as_deref().unwrap_or("?")
        ),
        (None, Some(head), _) => format!(
            "where: a worktree at {} on a detached head made from {head}",
            meta.directory
        ),
        (None, None, false) => format!("where: the project's own folder {}", meta.directory),
    });
    if !meta.chat && folder.is_dir() {
        let uncommitted = pm_git::changes(&folder).len();
        let commits = meta
            .base
            .as_deref()
            .map_or(0, |base| pm_git::commits_since(&folder, base).len());
        lines.push(match &meta.base {
            Some(base) => format!(
                "changes: {commits} commits since {base}, {uncommitted} files not committed"
            ),
            None => format!("changes: {uncommitted} files not committed"),
        });
    }
    if let Some(branch) = &meta.branch
        && let Some(request) = pm_git::pull_request(&folder, branch)
    {
        lines.push(format!(
            "pull request: #{} {}{} {}",
            request.number,
            request.state.to_lowercase(),
            if request.draft { " draft" } else { "" },
            request.url
        ));
    }
    if let Some(reply) = &seen.reply {
        lines.push(format!("final message:\n{}", tail(reply, FINAL_TAIL)));
    }
    lines.join("\n")
}

/// The tools the agents are offered.
fn definitions() -> Vec<Value> {
    vec![
        tool(
            "session_start",
            "Start a new session in Pandemonium: an agent of its own with its own conversation, which the reader sees in the sidebar and can open and talk to, and which goes on when this session stops. Use it for independent pieces of work that should each be reviewed and shipped on their own, such as separate features or fixes, one call per piece; several calls in one reply start them side by side. Only split work into sessions when the reader asked for that, or suggest it to them first. Work that depends on another session's work belongs in the same session, or in one started later from that session's branch with `base_branch`.\n\nThe call returns as soon as the session has its prompt, not when its work is done; check on it with session_status. Write `prompt` so it stands on its own: the new agent has seen none of this conversation, so say what to do, why, what is known, and when it is done.\n\n`where` is `worktree` by default: a worktree and branch of its own, made from what this session has checked out, with the changes this session has not committed carried over (set `carry_changes` to false to start from the commit alone). `base_branch` makes it from that branch or commit instead, with nothing carried. `local` works in the project's own folder and is refused while any agent runs there, since two agents in one folder overwrite each other's work. `chat` works in an empty scratch folder outside any project, for research and questions that change no code.",
            &json!({
                "prompt": { "type": "string", "description": "The new session's first prompt, written to stand on its own." },
                "title": { "type": "string", "description": "What the session is called; it names its worktree and branch. Made from the prompt when left out." },
                "where": { "type": "string", "enum": ["worktree", "local", "chat"], "description": "Where it works; worktree when left out." },
                "base_branch": { "type": "string", "description": "The branch or commit its worktree is made from, instead of what this session has checked out. In a project of several repositories, `repository=reference` entries separated by commas, with at most one bare reference for the rest." },
                "carry_changes": { "type": "boolean", "description": "Whether the changes not committed where the worktree is made from are carried into it; true when left out. Has no effect with base_branch." },
                "project": { "type": "string", "description": "The project it works in, by folder or folder name; this session's project when left out." },
                "container": { "type": "boolean", "description": "Whether its agent runs in the project's dev container; as this session when left out." },
                "submodule": { "type": "string", "description": "The submodule its agent works in, as a path from the project's root." },
                "model": { "type": "string", "description": "The model its agent uses, by name; this session's model when left out." },
            }),
            &["prompt"],
        ),
        tool(
            "session_status",
            "See how sessions stand: for each, whether its agent is working, waits for the reader, finished or failed; where it works; its commits and files not committed; its pull request; and the end of its final message. Without `sessions`, it tells of the sessions this session started that are still open.",
            &json!({
                "sessions": {
                    "type": "array",
                    "items": { "type": "integer" },
                    "description": "The sessions to tell of, by number; those this session started when left out.",
                },
            }),
            &[],
        ),
    ]
}

#[cfg(test)]
mod tests;
