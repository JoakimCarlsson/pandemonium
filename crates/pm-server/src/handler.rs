//! Carrying out what a client asks.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;

use pm_harness::Agent;
use pm_protocol::{Attachment, Request, Response, ServerMessage};

use crate::conversation;
use crate::core::{Connection, Core, Session, Told, now};
use crate::debugs;
use crate::languages;
use crate::queue::Prompt;
use crate::sessions::{self, Meta};
use crate::tasks;
use crate::uploads;

/// The list kept under `name`, empty when none was ever saved.
fn stored_list<T: serde::de::DeserializeOwned>(name: &str) -> Result<Vec<T>, String> {
    match pm_store::load_text(name) {
        Some(text) => serde_json::from_str(&text).map_err(|error| error.to_string()),
        None => Ok(Vec::new()),
    }
}

/// The answer to saving `list` under `name`.
fn saved_list<T: serde::Serialize>(name: &str, list: &[T]) -> Response {
    let text = match serde_json::to_string(list) {
        Ok(text) => text,
        Err(error) => return failed(error.to_string()),
    };
    match pm_store::save_text(name, &text) {
        Ok(()) => Response::Done,
        Err(error) => failed(error.to_string()),
    }
}

/// The agent whose id is `id`, when the app offers one by that name.
pub(crate) fn agent_named(id: &str) -> Option<Agent> {
    pm_harness::agent(id)
}

/// The folder `path` names on this machine, with a leading `~` taken to be the reader's home folder.
pub(crate) fn place(path: &str) -> PathBuf {
    match (path.strip_prefix("~/"), std::env::home_dir()) {
        (Some(rest), Some(home)) => home.join(rest),
        _ => PathBuf::from(path),
    }
}

/// A response saying the request failed, for `message`.
pub(crate) fn failed(message: impl Into<String>) -> Response {
    Response::Failed {
        message: message.into(),
    }
}

/// `Done`, or a failure saying why `outcome` is an error.
fn done(outcome: Result<(), String>) -> Response {
    match outcome {
        Ok(()) => Response::Done,
        Err(reason) => failed(reason),
    }
}

/// `Directory` naming the folder a clone made, or a failure saying why it did not.
fn directory_made(made: Result<PathBuf, String>) -> Response {
    match made {
        Ok(path) => Response::Directory {
            path: path.to_string_lossy().into_owned(),
        },
        Err(reason) => failed(reason),
    }
}

/// `Done` when a request about a shell found it, and a failure saying there is none otherwise.
fn terminal_done(found: bool, terminal: u64) -> Response {
    if found {
        Response::Done
    } else {
        failed(format!("no terminal is numbered {terminal}"))
    }
}

/// `Terminal` for the shell a task was run in, which `connection` is sent what it prints, or a failure saying why it could not be run.
fn run_started(
    started: Result<u64, String>,
    connection: &Arc<Connection>,
    core: &Arc<Core>,
) -> Response {
    match started {
        Ok(terminal) => {
            core.attach_terminal(connection, terminal, 0);
            Response::Terminal { terminal }
        }
        Err(reason) => failed(format!("could not run the task: {reason}")),
    }
}

/// `Debug` for a debugging session that was started, or a failure saying why it was not.
fn debug_started(started: Result<(u64, pm_protocol::DebugView), String>) -> Response {
    match started {
        Ok((debug, view)) => Response::Debug { debug, view },
        Err(reason) => failed(format!("could not start debugging: {reason}")),
    }
}

/// `Done` when a request about a debugging session found it, and a failure saying there is none otherwise.
fn debug_done(found: bool, debug: u64) -> Response {
    if found {
        Response::Done
    } else {
        failed(format!("no debugging session is numbered {debug}"))
    }
}

/// Points every agent at the folder of the login in use for it, or at its own when none is.
fn apply_accounts(logins: &[pm_protocol::Login], in_use: &[(String, String)]) {
    for &agent in pm_harness::agents() {
        let home = in_use
            .iter()
            .find(|(owner, _)| owner == agent.id)
            .filter(|(owner, id)| {
                logins
                    .iter()
                    .any(|login| login.agent == *owner && login.id == *id)
            })
            .and_then(|(owner, id)| pm_store::account_directory(owner, id).ok());
        agent.use_home(home);
    }
}

/// The folder of the login `login` of `agent`, when the login is still kept; nothing for the agent's own folder.
pub(crate) fn login_folder(agent: &str, login: Option<&str>) -> Option<PathBuf> {
    let login = login?;
    let (logins, _) = pm_store::load_accounts();
    logins
        .iter()
        .any(|(owner, id, _)| owner == agent && id == login)
        .then(|| pm_store::account_directory(agent, login).ok())
        .flatten()
}

/// Keeps `key` as the API key of `agent` for its added account `login`, or for its own login when there is none, or forgets the key saved there when `key` is nothing.
fn save_key(agent: pm_harness::Agent, key: Option<&str>, login: Option<&str>) -> Response {
    let home = match login {
        Some(login) => match login_folder(agent.id, Some(login)) {
            Some(home) => Some(home),
            None => return failed(format!("no account {login} is kept for {}", agent.name)),
        },
        None => None,
    };
    match agent.save_key_in(key, home) {
        Ok(()) => Response::Done,
        Err(error) => failed(error),
    }
}

/// The display name of a named account on this server.
pub(crate) fn login_name(agent: &str, login: Option<&str>) -> Option<String> {
    let login = login?;
    pm_store::load_accounts()
        .0
        .into_iter()
        .find(|(owner, id, _)| owner == agent && id == login)
        .map(|(_, _, name)| name)
}

/// The id of the login new sessions of `agent` start as, or nothing for the agent's own.
fn login_for_new_work(agent: &str) -> Option<String> {
    let (_, in_use) = pm_store::load_accounts();
    in_use
        .into_iter()
        .find(|(owner, _)| owner == agent)
        .map(|(_, id)| id)
}

/// What the reader wrote for `session` and the files of `attachments`, stored where the agent can read them.
fn prompt(session: u64, text: &str, attachments: &[Attachment]) -> Prompt {
    Prompt {
        text: text.to_owned(),
        paths: uploads::store(session, attachments)
            .into_iter()
            .map(|path: PathBuf| path.to_string_lossy().into_owned())
            .collect(),
        names: attachments.iter().map(|file| file.name.clone()).collect(),
    }
}

/// Lets go of every queue `connection` holds, as the client went away, and hands the agents what then waits.
pub(crate) fn release_queues(core: &Core, connection: &Arc<Connection>) {
    let holder = Arc::as_ptr(connection) as usize;
    let mut told = Told::new();
    if let Ok(mut sessions) = core.sessions.lock() {
        for (number, session) in sessions.iter_mut() {
            told.extend(session.release_queue(*number, holder));
        }
    }
    core.tell_all(told);
}

/// Runs `action` on the session numbered `number`, tells the clients what it noted, or says there is no such session.
fn telling(core: &Core, number: u64, action: impl FnOnce(&mut Session) -> Told) -> Response {
    let mut told = Told::new();
    let response = with_session(core, number, |held| told = action(held));
    core.tell_all(told);
    response
}

/// Runs `action` on the session numbered `number` and tells the clients what it noted, or says why it could not be done.
fn trying(
    core: &Core,
    number: u64,
    action: impl FnOnce(&mut Session) -> Result<Told, String>,
) -> Response {
    let mut outcome = Ok(Told::new());
    let response = with_session(core, number, |held| outcome = action(held));
    match outcome {
        Ok(told) => {
            core.tell_all(told);
            response
        }
        Err(reason) => failed(reason),
    }
}

/// A session made ready for `start_session`, before its agent is started.
pub(crate) struct Seat {
    /// The number it is held under.
    pub(crate) number: u64,
    /// The session, with no agent running.
    pub(crate) session: Session,
    /// The folder of its worktree, its project or its scratch folder.
    pub(crate) folder: PathBuf,
    /// The submodules and files that could not be brought into its worktree, each with why.
    pub(crate) skipped: Vec<(String, String)>,
}

/// Makes ready the session of `agent` that a `start_session` asks for, without starting its agent: it works in a scratch folder made for it when `scratch` is set, which makes it a chat that belongs to no project, in a worktree made from `base` when one is given, and in `directory` otherwise.
///
/// # Errors
///
/// Says why when the scratch folder or the worktree cannot be made, `directory` is not a folder, or the folder has no dev container to run in.
#[allow(clippy::too_many_arguments)]
pub(crate) fn seat(
    core: &Core,
    agent: Agent,
    repository: &str,
    directory: &str,
    title: &str,
    base: Option<&str>,
    submodule: Option<&str>,
    container: bool,
    scratch: bool,
) -> Result<Seat, String> {
    let (repository, base, submodule, container) = if scratch {
        ("", None, None, false)
    } else {
        (repository, base, submodule, container)
    };
    let reserved = scratch.then(|| core.next_session());
    let mut skipped = Vec::new();
    let (folder, head) = match (reserved, base) {
        (Some(number), _) => {
            let folder = crate::chats::create(number)
                .map_err(|reason| format!("could not make a folder for the chat: {reason}"))?;
            (folder, None)
        }
        (None, Some(_)) if pm_git::layout(&place(repository)) == pm_git::Layout::Neither => {
            return Err(format!(
                "{repository} holds no git repository, so it has no worktrees"
            ));
        }
        (None, Some(base)) => match pm_git::add_worktree(
            &place(repository),
            title,
            base,
            &crate::machine::placement_for(container),
        ) {
            Ok(made) => {
                skipped = made.skipped;
                (made.path, Some((made.head, made.branch)))
            }
            Err(reason) => return Err(format!("could not make a worktree: {reason}")),
        },
        (None, None) => {
            let folder = place(directory);
            if !folder.is_dir() {
                return Err(format!("{directory} is not a folder on this machine"));
            }
            (folder, None)
        }
    };
    if container && !pm_transport::configured(&folder) {
        return Err(format!(
            "{} has no devcontainer.json to run the agent in",
            folder.display()
        ));
    }
    let (head, branch) = head.map_or((None, None), |(head, branch)| (Some(head), branch));
    let meta = Meta {
        agent: agent.id.to_owned(),
        repository: repository.to_owned(),
        directory: folder.to_string_lossy().into_owned(),
        branch,
        head,
        base: base.map(str::to_owned),
        title: title.to_owned(),
        closed: false,
        updated: now(),
        login: login_for_new_work(agent.id),
        submodule: submodule.map(str::to_owned),
        container,
        chat: scratch,
    };
    let session = Session::new(agent, meta.agent_folder(), Vec::new(), meta);
    Ok(Seat {
        number: reserved.unwrap_or_else(|| core.next_session()),
        session,
        folder,
        skipped,
    })
}

/// Starts a session, its agent in `submodule` of its folder when one is given, or, when `scratch` is set, a chat in a scratch folder of its own, and hands the agent the first prompt.
#[allow(clippy::too_many_arguments)]
fn start_session(
    core: &Arc<Core>,
    connection: &Arc<Connection>,
    agent: &str,
    repository: &str,
    directory: &str,
    title: &str,
    base: Option<&str>,
    settings: &[(String, String)],
    text: &str,
    attachments: &[Attachment],
    submodule: Option<&str>,
    container: bool,
    scratch: bool,
) -> Response {
    let Some(agent) = agent_named(agent) else {
        return failed(format!("no agent is called {agent}"));
    };
    let Seat {
        number,
        mut session,
        folder,
        skipped,
    } = match seat(
        core, agent, repository, directory, title, base, submodule, container, scratch,
    ) {
        Ok(seat) => seat,
        Err(reason) => return failed(reason),
    };
    let (branch, head) = (session.meta.branch.clone(), session.meta.head.clone());
    session.start_agent(number, &crate::machine::with_mode(settings));
    hold_session_as(core, connection, number, session);
    let first = prompt(number, text, attachments);
    telling(core, number, |held| held.submit(number, first, false));
    announce(core, connection, number);
    Response::Started {
        session: number,
        directory: folder.to_string_lossy().into_owned(),
        branch,
        head,
        skipped,
    }
}

/// Stops the agent of session `number` and waits for it to exit, ends the shells that work in its worktree, removes the worktree it made when no other open session works in it, deletes the branch it made there when nothing would be lost, and marks it closed.
///
/// A worktree with changes that are not committed is kept and the request answered with `Uncommitted`, unless `discard` is set. The scratch folder of a chat is kept, so that the chat can be opened again, unless `discard` is set: then the shells in it are ended and it is removed with everything in it, when no other open session works in it.
pub(crate) fn close_session(core: &Core, number: u64, discard: bool) -> Response {
    let (chat, removal, enclosed, scratch) = {
        let Ok(mut sessions) = core.sessions.lock() else {
            return failed("the server is shutting down");
        };
        let Some(held) = sessions.get(&number) else {
            return failed(format!("no session is numbered {number}"));
        };
        let folder = place(&held.meta.directory);
        let shared = sessions.iter().any(|(other, session)| {
            *other != number
                && !session.meta.closed
                && session.meta.directory == held.meta.directory
        });
        let worktree = held.meta.head.is_some() && !shared;
        let container = held.meta.container && !shared;
        let scratch = held.meta.chat && discard && !shared;
        if worktree && !discard && !pm_git::changes(&folder).is_empty() {
            return Response::Uncommitted;
        }
        let held = sessions
            .get_mut(&number)
            .expect("the session was just found");
        held.busy = false;
        let removal = worktree.then(|| {
            (
                place(&held.meta.repository),
                folder,
                held.meta.branch.clone(),
                held.meta.base.clone(),
                held.meta.directory.clone(),
            )
        });
        (
            held.chat.take(),
            removal,
            container.then(|| held.meta.directory.clone()),
            scratch.then(|| held.meta.directory.clone()),
        )
    };
    if let Some(chat) = chat {
        chat.stop();
    }
    if let Some(folder) = enclosed
        && let Err(reason) = pm_transport::remove_session_container(&folder)
    {
        return failed(format!("could not remove the dev container: {reason}"));
    }
    if let Some((repository, folder, branch, base, directory)) = removal {
        if !discard && !pm_git::changes(&folder).is_empty() {
            return Response::Uncommitted;
        }
        core.close_terminals_in(&directory);
        if let Err(reason) =
            pm_git::remove_worktree(&repository, &folder, branch.as_deref(), base.as_deref())
        {
            return failed(format!("could not remove the worktree: {reason}"));
        }
    }
    if let Some(directory) = scratch {
        core.close_terminals_in(&directory);
        if let Err(reason) = crate::chats::remove(&place(&directory)) {
            return failed(format!("could not remove the chat's folder: {reason}"));
        }
    }
    let told = {
        let Ok(mut sessions) = core.sessions.lock() else {
            return failed("the server is shutting down");
        };
        let Some(held) = sessions.get_mut(&number) else {
            return failed(format!("no session is numbered {number}"));
        };
        held.meta.closed = true;
        sessions::save_meta(number, &held.meta);
        held.drop_queue(number)
    };
    core.tell_all(told);
    Response::Done
}

/// Starts a session that branches off the conversation of session `source` after the reader's turn `turn`, counted from nought, and its reply: it works in the same folder, its agent is not started until it is handed a prompt, and that agent is then told the conversation so far.
fn fork_session(core: &Core, connection: &Arc<Connection>, source: u64, turn: u64) -> Response {
    let branch = {
        let Ok(sessions) = core.sessions.lock() else {
            return failed("the server is shutting down");
        };
        let Some(held) = sessions.get(&source) else {
            return failed(format!("no session is numbered {source}"));
        };
        let replying = held.busy && conversation::turns(&held.log).len() as u64 == turn + 1;
        if replying {
            return failed("the agent is still writing that reply");
        }
        let Some(log) = conversation::through(&held.log, turn) else {
            return failed(format!("session {source} has no turn {turn}"));
        };
        let meta = Meta {
            title: format!("{} (branch)", held.meta.title),
            closed: false,
            updated: now(),
            ..held.meta.clone()
        };
        let mut branch = Session::new(held.agent, held.directory.clone(), log, meta);
        branch.resume_before = conversation::turns(&held.log)
            .get(turn as usize + 1)
            .map(|next| next.id.to_string());
        branch
    };
    let number = core.next_session();
    crate::sessions::copy_history(source, number);
    hold_session_as(core, connection, number, branch);
    announce(core, connection, number);
    match core.info(number) {
        Some(info) => Response::Sessions {
            sessions: vec![info],
        },
        None => failed(format!("session {number} went away as it was made")),
    }
}

/// Holds `session` under `number`, which the caller took from the core, keeps it and its log on disk and has `connection` watch it.
fn hold_session_as(core: &Core, connection: &Arc<Connection>, number: u64, session: Session) {
    sessions::save_meta(number, &session.meta);
    for (seq, event) in &session.log {
        sessions::append(number, *seq, event);
    }
    if let Ok(mut held) = core.sessions.lock() {
        held.insert(number, session);
    }
    if let Ok(mut attached) = connection.attached.lock() {
        attached.insert(number);
    }
}

/// Tells every client but `connection` of the session numbered `number`.
fn announce(core: &Core, connection: &Arc<Connection>, number: u64) {
    if let Some(info) = core.info(number) {
        core.broadcast(&ServerMessage::SessionAdded { info }, Some(connection));
    }
}

/// Runs `action` on the session numbered `number`, or says there is none.
fn with_session(core: &Core, number: u64, action: impl FnOnce(&mut Session)) -> Response {
    let Ok(mut sessions) = core.sessions.lock() else {
        return failed("the server is shutting down");
    };
    match sessions.get_mut(&number) {
        Some(session) => {
            action(session);
            Response::Done
        }
        None => failed(format!("no session is numbered {number}")),
    }
}

/// Subscribes `connection` to a session and replays what it missed after `after`.
fn attach(core: &Core, connection: &Arc<Connection>, number: u64, after: u64) -> Response {
    let Ok(sessions) = core.sessions.lock() else {
        return failed("the server is shutting down");
    };
    let Some(session) = sessions.get(&number) else {
        return failed(format!("no session is numbered {number}"));
    };
    if let Ok(mut attached) = connection.attached.lock() {
        attached.insert(number);
    }
    for (seq, event) in session.log.iter().filter(|(seq, _)| *seq > after) {
        let _ = connection.outbox.send(ServerMessage::Event {
            session: number,
            seq: *seq,
            event: event.clone(),
        });
    }
    Response::Done
}

/// What the request `request` of `connection` comes to.
pub(crate) fn handle(core: &Arc<Core>, connection: &Arc<Connection>, request: Request) -> Response {
    match request {
        Request::Discover { agent } => match agent_named(&agent) {
            Some(agent) => match pm_harness::discover(agent) {
                Ok(found) => Response::Discovery(found),
                Err(reason) => failed(reason),
            },
            None => failed(format!("no agent is called {agent}")),
        },
        Request::Allowance { agent, login } => match agent_named(&agent) {
            Some(agent) => {
                let home = login_folder(agent.id, login.as_deref());
                Response::Allowance {
                    limits: pm_harness::allowance(agent, home),
                }
            }
            None => failed(format!("no agent is called {agent}")),
        },
        Request::Library { agent, directory } => match agent_named(&agent) {
            Some(agent) => Response::Library {
                product: agent.product(),
                items: crate::library::items(&place(&directory)),
                withheld: crate::library::withheld(),
            },
            None => failed(format!("no agent is called {agent}")),
        },
        Request::ToolSets { agent } => match agent_named(&agent) {
            Some(agent) => Response::ToolSets {
                product: agent.product(),
                sets: crate::library::tool_sets(agent),
                withheld: crate::library::withheld(),
            },
            None => failed(format!("no agent is called {agent}")),
        },
        Request::SaveWithheld { keys } => saved_list(crate::library::WITHHELD, &keys),
        Request::CreatePromptFile {
            file,
            scope,
            directory,
        } => match pm_agent_library::create_prompt_file(file, scope, &place(&directory)) {
            Ok(path) => Response::Created {
                path: path.to_string_lossy().into_owned(),
            },
            Err(reason) => failed(reason),
        },
        Request::CreateCustomization {
            kind,
            scope,
            name,
            description,
            directory,
        } => {
            let draft = pm_agent_library::Draft {
                kind,
                scope,
                name,
                description,
            };
            match pm_agent_library::create(&draft, &place(&directory)) {
                Ok(path) => Response::Created {
                    path: path.to_string_lossy().into_owned(),
                },
                Err(reason) => failed(reason),
            }
        }
        Request::Files {
            directory,
            limit,
            expanded,
        } => {
            let place = place(&directory);
            let mut paths = pm_git::files(&place, limit);
            let mut folders = pm_git::untracked_folders(&place, limit);
            let mut ignored = Vec::new();
            let mut listable = std::collections::HashSet::new();
            for entry in pm_git::ignored_entries(&place, limit) {
                match entry.strip_suffix('/') {
                    Some(folder) => {
                        listable.insert(folder.to_owned());
                        folders.push(folder.to_owned());
                        ignored.push(folder.to_owned());
                    }
                    None => {
                        paths.push(entry.clone());
                        ignored.push(entry);
                    }
                }
            }
            for folder in crate::entries::unfolded(expanded) {
                if !listable.contains(&folder) {
                    continue;
                }
                for (path, is_folder) in crate::entries::children(&place, &folder) {
                    ignored.push(path.clone());
                    if is_folder {
                        listable.insert(path.clone());
                        folders.push(path);
                    } else {
                        paths.push(path);
                    }
                }
            }
            Response::Files {
                paths,
                folders,
                ignored,
            }
        }
        Request::Branches { directory } => Response::Branches {
            names: pm_git::branches(&place(&directory)),
        },
        Request::Submodules { directory } => Response::Submodules {
            paths: pm_git::submodules(&place(&directory)),
        },
        Request::DevContainer { directory } => Response::DevContainer {
            configured: pm_transport::configured(&place(&directory)),
        },
        Request::SubmoduleHeads { directory } => Response::SubmoduleHeads {
            heads: pm_git::submodule_heads(&place(&directory)),
        },
        Request::Identity { agent, login } => match agent_named(&agent) {
            Some(agent) => Response::Identity {
                default: pm_harness::default_account(agent),
                selected: match login {
                    Some(login) => login_folder(agent.id, Some(&login))
                        .or_else(|| pm_store::account_directory(agent.id, &login).ok())
                        .and_then(|home| pm_harness::account_in_folder(agent, home)),
                    None => pm_harness::account(agent),
                },
            },
            None => failed(format!("no agent is called {agent}")),
        },
        Request::SetAccounts { logins, in_use } => {
            apply_accounts(&logins, &in_use);
            let accounts: Vec<pm_store::Account> = logins
                .iter()
                .map(|login| (login.agent.clone(), login.id.clone(), login.name.clone()))
                .collect();
            let _ = pm_store::save_accounts(&accounts, &in_use);
            Response::Done
        }
        Request::Accounts => {
            let (accounts, in_use) = pm_store::load_accounts();
            Response::Accounts {
                logins: accounts
                    .into_iter()
                    .map(|(agent, id, name)| pm_protocol::Login { agent, id, name })
                    .collect(),
                in_use,
            }
        }
        Request::Projects => match pm_store::load_projects() {
            Some(paths) => Response::Projects { paths, saved: true },
            None => Response::Projects {
                paths: Vec::new(),
                saved: false,
            },
        },
        Request::SaveProjects { paths } => match pm_store::save_projects(&paths) {
            Ok(()) => {
                core.broadcast(&ServerMessage::ProjectsChanged { paths }, Some(connection));
                Response::Done
            }
            Err(error) => failed(error.to_string()),
        },
        Request::Owners => match pm_git::owners() {
            Ok(owners) => Response::Owners { owners },
            Err(reason) => failed(reason),
        },
        Request::Repositories {
            owner,
            organization,
        } => match pm_git::repositories(&pm_git::Owner {
            login: owner,
            organization,
        }) {
            Ok(repositories) => Response::Repositories { repositories },
            Err(reason) => failed(reason),
        },
        Request::Folders { path } => match pm_git::folders(&path) {
            Ok((path, names)) => Response::Folders {
                path: path.to_string_lossy().into_owned(),
                names,
            },
            Err(reason) => failed(reason),
        },
        Request::Clone { url, under } => {
            directory_made(pm_git::clone(&url, under.as_deref().map(place).as_deref()))
        }
        Request::Choices => Response::Choices {
            choices: pm_store::load_choices(),
        },
        Request::SaveChoices { choices } => match pm_store::save_choices(&choices) {
            Ok(()) => Response::Done,
            Err(error) => failed(error.to_string()),
        },
        Request::SearchTools { query } => match pm_harness::search_tools(&query) {
            Ok(listings) => Response::ToolListings {
                listings: listings
                    .into_iter()
                    .map(|listing| pm_protocol::ToolListing {
                        id: listing.id,
                        title: listing.title,
                        description: listing.description,
                        name: listing.name,
                        target: listing.target,
                        env: listing.env,
                        inputs: listing.inputs,
                        asks: listing.asks,
                        website: listing.website,
                    })
                    .collect(),
            },
            Err(reason) => failed(reason),
        },
        Request::McpServers => match stored_list::<pm_protocol::McpServer>("mcp-servers") {
            Ok(mut servers) => {
                servers.retain(|server| server.extension.is_none());
                servers.extend(crate::extensions::mcp_servers());
                Response::McpServers { servers }
            }
            Err(reason) => failed(reason),
        },
        Request::SaveMcpServers { mut servers } => {
            servers.retain(|server| server.extension.is_none());
            saved_list("mcp-servers", &servers)
        }
        Request::WatchFolders { directories } => {
            core.folders.watch(connection, directories);
            Response::Done
        }
        Request::MachineSettings => Response::MachineSettings {
            settings: crate::machine::settings(),
        },
        Request::SaveMachineSettings { mut settings } => {
            settings.extensions = crate::machine::settings().extensions;
            let answer = crate::machine::save(&settings);
            core.documents.settings_changed(&settings);
            answer
        }
        Request::LatestRelease { channel } => crate::releases::latest(&channel),
        Request::DownloadUpdate { channel, version } => {
            crate::releases::download(connection, &channel, &version)
        }
        Request::CancelUpdate => {
            crate::releases::cancel();
            Response::Done
        }
        Request::Sessions => Response::Sessions {
            sessions: core.infos(),
        },
        Request::SetSessionAccount { session, login } => with_session(core, session, |held| {
            held.meta.login = login;
            sessions::save_meta(session, &held.meta);
        }),
        Request::SignInSession { session } => sign_in_session(core, connection, session),
        Request::SaveKey { agent, key, login } => match agent_named(&agent) {
            Some(agent) => save_key(agent, key.as_deref(), login.as_deref()),
            None => failed(format!("no agent is called {agent}")),
        },
        Request::SignIn { agent, login } => match agent_named(&agent) {
            Some(agent) => {
                match sign_in_login(agent, login.as_deref(), &code_teller(connection, agent)) {
                    Ok(()) => Response::Done,
                    Err(reason) => failed(reason),
                }
            }
            None => failed(format!("no agent is called {agent}")),
        },
        Request::StartSession {
            agent,
            repository,
            directory,
            title,
            base,
            settings,
            text,
            attachments,
            submodule,
            container,
            scratch,
        } => start_session(
            core,
            connection,
            &agent,
            &repository,
            &directory,
            &title,
            base.as_deref(),
            &settings,
            &text,
            &attachments,
            submodule.as_deref(),
            container,
            scratch,
        ),
        Request::CloseSession { session, discard } => {
            let closed = close_session(core, session, discard);
            if matches!(closed, Response::Done) {
                core.broadcast(&ServerMessage::SessionClosed { session }, Some(connection));
            }
            closed
        }
        Request::RenameSession { session, title } => {
            let title = title.trim().to_owned();
            trying(core, session, |held| {
                if title.is_empty() {
                    return Err("a session needs a title".to_owned());
                }
                if let Some(chat) = &held.chat {
                    chat.rename(&title);
                    return Ok(Vec::new());
                }
                if let Some(folder) = crate::sessions::history_folder(session) {
                    pm_harness::rename_saved(crate::sessions::HISTORY, &folder, &title)?;
                }
                let event = pm_harness::Event::Titled(title);
                Ok(vec![(session, held.note(session, &event), event)])
            })
        }
        Request::Prompt {
            session,
            text,
            attachments,
            now,
        } => {
            let next = prompt(session, &text, &attachments);
            trying(core, session, |held| {
                if held.meta.closed && !place(&held.meta.directory).is_dir() {
                    return Err(format!(
                        "the folder of {} was removed when it was closed",
                        held.meta.title
                    ));
                }
                Ok(held.submit(session, next, now))
            })
        }
        Request::RemoveQueued { session, queued } => {
            telling(core, session, |held| held.remove_queued(session, queued))
        }
        Request::HoldQueue { session, held } => {
            let holder = Arc::as_ptr(connection) as usize;
            telling(core, session, |found| {
                found.hold_queue(session, holder, held)
            })
        }
        Request::EditQueued {
            session,
            queued,
            text,
        } => {
            let holder = Arc::as_ptr(connection) as usize;
            telling(core, session, |held| {
                held.edit_queued(session, holder, queued, text)
            })
        }
        Request::Hurry { session, queued } => {
            let mut found = true;
            let response = telling(core, session, |held| {
                let told = held.hurry(session, queued);
                found = told.is_some();
                told.unwrap_or_default()
            });
            match response {
                Response::Done if !found => failed(format!(
                    "no prompt waits under {queued} in session {session}"
                )),
                response => response,
            }
        }
        Request::Rewind { session, turn } => {
            trying(core, session, |held| held.rewind(session, turn))
        }
        Request::ForkSession { session, turn } => fork_session(core, connection, session, turn),
        Request::Restore { session, turn } => {
            trying(core, session, |held| held.restore(session, turn))
        }
        Request::Choose {
            session,
            ticket,
            option,
        } => with_session(core, session, |held| {
            if let Some(chat) = &held.chat {
                chat.choose(ticket, Some(option));
            }
        }),
        Request::Answer {
            session,
            ticket,
            replies,
        } => with_session(core, session, |held| {
            if let Some(chat) = &held.chat {
                chat.answer(ticket, replies);
            }
        }),
        Request::Trust {
            session,
            ticket,
            verdict,
        } => {
            let answered = match core.sessions.lock() {
                Ok(mut sessions) => crate::trust::answer(&mut sessions, session, ticket, verdict),
                Err(_) => Err("the server is shutting down".to_owned()),
            };
            match answered {
                Ok(told) => {
                    core.tell_all(told);
                    Response::Done
                }
                Err(reason) => failed(reason),
            }
        }
        Request::Cancel { session } => telling(core, session, |held| {
            let mut told = match &held.chat {
                Some(chat) => {
                    chat.cancel();
                    Told::new()
                }
                None => held.end_open_turn(session, crate::core::NOT_RUNNING),
            };
            told.extend(held.drop_queue(session));
            told
        }),
        Request::Set {
            session,
            setting,
            value,
        } => with_session(core, session, |held| {
            if let Some(chat) = &held.chat {
                chat.set(&setting, &value);
            }
        }),
        Request::Attach { session, after } => attach(core, connection, session, after),
        Request::Inspect { directory, base } => {
            let directory = place(&directory);
            let directory = directory.as_path();
            if pm_git::layout(directory) == pm_git::Layout::Neither {
                return Response::Inspected {
                    changes: Vec::new(),
                    commits: None,
                    branch: None,
                    head: None,
                    tracking: pm_git::Tracking::default(),
                    operation: None,
                    repositories: Vec::new(),
                };
            }
            Response::Inspected {
                changes: pm_git::changes(directory),
                commits: base.map(|base| pm_git::commits_since(directory, &base)),
                branch: pm_git::current_branch(directory),
                head: pm_git::resolve(directory, "HEAD"),
                tracking: pm_git::tracking(directory),
                operation: pm_git::operation(directory),
                repositories: pm_git::standings(directory),
            }
        }
        Request::Standing { directory } => {
            let directory = place(&directory);
            let directory = directory.as_path();
            Response::Standing {
                branch: pm_git::current_branch(directory),
                head: pm_git::resolve(directory, "HEAD"),
                tracking: pm_git::tracking(directory),
                operation: pm_git::operation(directory),
                repositories: pm_git::standings(directory),
                repository: pm_git::layout(directory) != pm_git::Layout::Neither,
            }
        }
        Request::Diffs { directory, paths } => Response::Diffs {
            files: pm_git::diffs(&place(&directory), &paths),
        },
        Request::Stage {
            directory,
            paths,
            stage,
        } => {
            let directory = place(&directory);
            let directory = directory.as_path();
            done(if stage {
                pm_git::stage(directory, &paths)
            } else {
                pm_git::unstage(directory, &paths)
            })
        }
        Request::CreateEntry {
            directory,
            path,
            folder,
        } => done(crate::entries::create(&place(&directory), &path, folder)),
        Request::RenameEntry {
            directory,
            from,
            to,
        } => {
            let root = place(&directory);
            let hub = &core.documents.hub;
            let files = hub.will_rename(&root, &from, &to).unwrap_or_default();
            match crate::entries::rename(&root, &from, &to) {
                Ok(()) => {
                    hub.did_rename(&root, &from, &to);
                    if files.is_empty() {
                        Response::Done
                    } else {
                        Response::Edited { files }
                    }
                }
                Err(reason) => failed(reason),
            }
        }
        Request::CopyEntry {
            directory,
            from,
            into,
        } => done(crate::entries::copy(&place(&directory), &from, &into)),
        Request::RemoveEntry { directory, path } => {
            done(crate::entries::remove(&place(&directory), &path))
        }
        Request::ImportFile {
            directory,
            path,
            data,
        } => done(crate::entries::import(&place(&directory), &path, &data)),
        Request::Discard { directory, paths } => done(pm_git::discard(&place(&directory), &paths)),
        Request::Ignore { directory, paths } => done(pm_git::ignore(&place(&directory), &paths)),
        Request::Checkout { directory, branch } => {
            done(pm_git::checkout(&place(&directory), &branch))
        }
        Request::CreateBranch { directory, name } => {
            done(pm_git::create_branch(&place(&directory), &name))
        }
        Request::FileHistory { directory, path } => {
            match pm_git::file_history(&place(&directory), &path) {
                Ok(commits) => Response::FileHistory { commits },
                Err(reason) => failed(reason),
            }
        }
        Request::Permalink {
            directory,
            path,
            line,
            end_line,
        } => match pm_git::permalink(
            &place(&directory),
            &path,
            line.map(|first| (first, end_line.unwrap_or(first))),
            &crate::machine::settings().git.hosts,
        ) {
            Ok(url) => Response::Address { url },
            Err(reason) => failed(reason),
        },
        Request::ReadFile {
            directory,
            path,
            head,
            revision,
        } => match pm_git::read_at(
            &place(&directory),
            &path,
            revision.as_deref().or(head.then_some("HEAD")),
        ) {
            Ok(pm_git::Contents::Text { text, truncated }) => Response::File {
                language: pm_lsp::language_of_text(&path, &text),
                text: Some(text),
                truncated,
            },
            Ok(pm_git::Contents::Binary) => Response::File {
                text: None,
                truncated: false,
                language: None,
            },
            Err(reason) => failed(reason),
        },
        Request::ReadBytes {
            directory,
            path,
            head,
        } => match pm_git::read_bytes(&place(&directory), &path, head) {
            Ok((bytes, truncated)) => Response::Bytes { bytes, truncated },
            Err(reason) => failed(reason),
        },
        Request::FetchBytes { url } => match crate::fetch::bytes(&url) {
            Ok((bytes, truncated)) => Response::Bytes { bytes, truncated },
            Err(reason) => failed(reason),
        },
        Request::WriteFile {
            directory,
            path,
            text,
        } => {
            let folder = place(&directory);
            let written = crate::entries::write(&folder, &path, &text);
            if written.is_ok() {
                let _ = crate::local_history::record(
                    &folder,
                    &path,
                    &text,
                    &crate::machine::settings().history,
                );
            }
            done(written)
        }
        Request::LocalHistory { directory, path } => match crate::local_history::versions(
            &place(&directory),
            &path,
            &crate::machine::settings().history,
        ) {
            Ok(versions) => Response::LocalHistory { versions },
            Err(reason) => failed(reason),
        },
        Request::ReadVersion {
            directory,
            path,
            version,
        } => match crate::local_history::read(&place(&directory), &path, version) {
            Ok(text) => Response::File {
                language: pm_lsp::language_of_text(&path, &text),
                text: Some(text),
                truncated: false,
            },
            Err(reason) => failed(reason),
        },
        Request::ReadConflict { directory, path } => {
            let folder = place(&directory);
            match (
                pm_git::sides(&folder, &path),
                pm_git::read_at(&folder, &path, None),
            ) {
                (
                    Ok(_),
                    Ok(pm_git::Contents::Text {
                        truncated: true, ..
                    }),
                ) => failed(format!("{path} is too large to resolve here")),
                (Ok(sides), Ok(pm_git::Contents::Text { text, .. })) => Response::Conflict {
                    files: pm_protocol::ConflictFiles {
                        ours: sides.ours,
                        theirs: sides.theirs,
                        base: sides.base,
                        merged: text,
                        language: pm_lsp::language_of(&path),
                    },
                },
                (Err(reason), _) => failed(reason),
                (_, Ok(pm_git::Contents::Binary)) => failed(format!("{path} is not text")),
                (_, Err(reason)) => failed(reason),
            }
        }
        Request::ResolveConflict {
            directory,
            path,
            text,
        } => {
            let folder = place(&directory);
            done(
                crate::entries::write(&folder, &path, &text)
                    .and_then(|()| pm_git::stage(&folder, &[path])),
            )
        }
        Request::RevisionChanges {
            directory,
            from,
            to,
        } => match pm_git::revision_changes(&place(&directory), from.as_deref(), &to) {
            Ok(changes) => Response::RevisionChanges { changes },
            Err(reason) => failed(reason),
        },
        Request::BrowserAvailable { available } => {
            crate::browser_tools::set_available(connection, available);
            Response::Done
        }
        Request::BrowserResult {
            call,
            ok,
            text,
            image,
        } => {
            crate::browser_tools::answer(call, crate::browser_tools::Reply { ok, text, image });
            Response::Done
        }
        Request::Extensions => Response::Extensions {
            extensions: core.extension_infos(true),
        },
        Request::InstallExtensions { ids } => {
            core.install_extensions(ids);
            Response::Done
        }
        Request::RemoveExtension { id } => {
            if pm_plugin::named(&id).is_some() {
                done(core.remove_package(&id))
            } else {
                done(core.remove_extension(&id))
            }
        }
        Request::InstallExtension { path, grants } => {
            let folder = place(&path);
            if folder.join(pm_plugin::MANIFEST).is_file() {
                core.install_package(&path, grants)
            } else {
                match pm_lsp::add_extension(&folder) {
                    Ok(id) => {
                        core.install_extensions(vec![id]);
                        Response::Done
                    }
                    Err(reason) => failed(reason),
                }
            }
        }
        Request::InspectExtension { path } => match pm_plugin::inspect(&place(&path)) {
            Ok(package) => Response::ExtensionPreview {
                extension: crate::extensions::describe(&package, false),
            },
            Err(reason) => failed(reason),
        },
        Request::GrantExtension {
            id,
            capability,
            granted,
        } => done(core.grant(&id, &capability, granted)),
        Request::ExtensionAssets => Response::ExtensionAssets(crate::extensions::assets()),
        Request::RunExtensionCommand {
            extension,
            command,
            argument,
            directory,
        } => core.run_extension_command(&extension, &command, &argument, directory.as_deref()),
        Request::Grammar { language } => match languages::grammar(&language) {
            Ok(bundle) => Response::Grammar(bundle),
            Err(reason) => failed(reason),
        },
        Request::OpenDocument {
            directory,
            path,
            language,
            version,
            text,
        } => {
            let root = place(&directory);
            core.documents
                .opened(connection, root.clone(), &directory, &path);
            match core
                .documents
                .hub
                .open(&root, &path, &language, version, &text)
            {
                Ok(triggers) => Response::Opened {
                    triggers: triggers.completion,
                    signature: triggers.signature,
                    format: triggers.format,
                    formats_ranges: triggers.formats_ranges,
                    folds: triggers.folds,
                    links: triggers.links,
                    semantic_tokens: triggers.semantic_tokens,
                    inlay_hints: triggers.inlay_hints,
                    code_lens: triggers.code_lens || crate::dependencies::applies(&language, &path),
                    inline_values: triggers.inline_values,
                    document_links: triggers.document_links,
                    colors: triggers.colors,
                    semantic_ranges: triggers.semantic_ranges,
                },
                Err(reason) => failed(reason),
            }
        }
        Request::ChangeDocument {
            directory,
            path,
            version,
            changes,
        } => done(
            core.documents
                .hub
                .change(&place(&directory), &path, version, changes),
        ),
        Request::CloseDocument { directory, path } => {
            core.documents.closed(connection, place(&directory), &path);
            Response::Done
        }
        Request::Completion {
            directory,
            path,
            version,
            position,
            trigger,
            scope,
        } => match core.documents.hub.completion(
            &place(&directory),
            &path,
            version,
            position,
            trigger.as_deref(),
            scope.as_deref(),
        ) {
            Ok(items) => Response::Completions { items },
            Err(reason) => failed(reason),
        },
        Request::ResolveCompletion {
            directory,
            path,
            item,
        } => match core
            .documents
            .hub
            .resolve_completion(&place(&directory), &path, &item)
        {
            Ok(item) => Response::CompletionResolved { item },
            Err(reason) => failed(reason),
        },
        Request::Format {
            directory,
            path,
            version,
            range,
        } => match core
            .documents
            .hub
            .format(&place(&directory), &path, version, range)
        {
            Ok(edits) => Response::Formatted { edits },
            Err(reason) => failed(reason),
        },
        Request::FormatOnType {
            directory,
            path,
            version,
            position,
            character,
        } => match core.documents.hub.format_on_type(
            &place(&directory),
            &path,
            version,
            position,
            &character,
        ) {
            Ok(edits) => Response::Formatted { edits },
            Err(reason) => failed(reason),
        },
        Request::FoldingRanges {
            directory,
            path,
            version,
        } => match core
            .documents
            .hub
            .folding_ranges(&place(&directory), &path, version)
        {
            Ok(ranges) => Response::FoldingRanges { ranges },
            Err(reason) => failed(reason),
        },
        Request::LinkedRanges {
            directory,
            path,
            version,
            position,
        } => match core
            .documents
            .hub
            .linked_ranges(&place(&directory), &path, version, position)
        {
            Ok(spans) => Response::LinkedRanges { spans },
            Err(reason) => failed(reason),
        },
        Request::Definition {
            directory,
            path,
            version,
            position,
            seek,
        } => match core
            .documents
            .hub
            .locate(&place(&directory), &path, version, position, seek)
        {
            Ok(locations) => Response::Definitions { locations },
            Err(reason) => failed(reason),
        },
        Request::Blame {
            directory,
            path,
            line,
        } => match pm_git::blame(&place(&directory), &path, line) {
            Ok(blame) => Response::Blamed { blame },
            Err(reason) => failed(reason),
        },
        Request::Problems { directory } => Response::Problems {
            problems: core.documents.hub.problems(&place(&directory)),
        },
        Request::SearchFiles {
            directory,
            query,
            regex,
            case,
        } => match pm_git::search(&place(&directory), &query, regex, case) {
            Ok(found) => Response::Definitions {
                locations: found
                    .into_iter()
                    .map(|held| {
                        let start = held.column.saturating_sub(1).min(held.text.len());
                        let character = held
                            .text
                            .get(..start)
                            .map_or(0, |before| before.encode_utf16().count())
                            as u32;
                        let line = held.line.saturating_sub(1) as u32;
                        pm_lsp::Location {
                            path: held.path,
                            span: pm_protocol::Span::new(
                                pm_protocol::Position::new(line, character),
                                pm_protocol::Position::new(line, character),
                            ),
                            inside: true,
                            line: Some(held.text),
                            ..pm_lsp::Location::default()
                        }
                    })
                    .collect(),
            },
            Err(reason) => failed(reason),
        },
        Request::WorkspaceSymbols {
            directory,
            path,
            query,
        } => match core
            .documents
            .hub
            .workspace_symbols(&place(&directory), &path, &query)
        {
            Ok(locations) => Response::Definitions { locations },
            Err(reason) => failed(reason),
        },
        Request::Highlights {
            directory,
            path,
            version,
            position,
        } => match core
            .documents
            .hub
            .highlights(&place(&directory), &path, version, position)
        {
            Ok(spans) => Response::Highlights { spans },
            Err(reason) => failed(reason),
        },
        Request::Signature {
            directory,
            path,
            version,
            position,
        } => match core
            .documents
            .hub
            .signature(&place(&directory), &path, version, position)
        {
            Ok(signature) => Response::Signature { signature },
            Err(reason) => failed(reason),
        },
        Request::PrepareRename {
            directory,
            path,
            version,
            position,
        } => match core
            .documents
            .hub
            .prepare_rename(&place(&directory), &path, version, position)
        {
            Ok(target) => Response::RenameTarget { target },
            Err(reason) => failed(reason),
        },
        Request::Rename {
            directory,
            path,
            version,
            position,
            name,
        } => match core
            .documents
            .hub
            .rename(&place(&directory), &path, version, position, &name)
        {
            Ok(files) => Response::Edited { files },
            Err(reason) => failed(reason),
        },
        Request::CodeActions {
            directory,
            path,
            version,
            span,
            only,
        } => match core.documents.hub.code_actions(
            &place(&directory),
            &path,
            version,
            span,
            &only.unwrap_or_default(),
        ) {
            Ok(actions) => Response::CodeActions { actions },
            Err(reason) => failed(reason),
        },
        Request::SemanticTokens {
            directory,
            path,
            version,
            rules,
            range,
        } => match core.documents.hub.semantic_tokens(
            &place(&directory),
            &path,
            version,
            &rules,
            range,
        ) {
            Ok((tokens, partial)) => Response::SemanticTokens { tokens, partial },
            Err(reason) => failed(reason),
        },
        Request::InlayHints {
            directory,
            path,
            version,
            span,
        } => match core
            .documents
            .hub
            .inlay_hints(&place(&directory), &path, version, span)
        {
            Ok(hints) => Response::InlayHints { hints },
            Err(reason) => failed(reason),
        },
        Request::DocumentLinks {
            directory,
            path,
            version,
        } => match core
            .documents
            .hub
            .document_links(&place(&directory), &path, version)
        {
            Ok(links) => Response::DocumentLinks { links },
            Err(reason) => failed(reason),
        },
        Request::ResolveLink {
            directory,
            path,
            link,
        } => match core
            .documents
            .hub
            .resolve_link(&place(&directory), &path, &link)
        {
            Ok(target) => Response::LinkResolved { target },
            Err(reason) => failed(reason),
        },
        Request::ColorPresentations {
            directory,
            path,
            version,
            span,
            color,
        } => match core.documents.hub.color_presentations(
            &place(&directory),
            &path,
            version,
            span,
            color,
        ) {
            Ok(presentations) => Response::ColorPresentations { presentations },
            Err(reason) => failed(reason),
        },
        Request::Colors {
            directory,
            path,
            version,
        } => match core
            .documents
            .hub
            .colors(&place(&directory), &path, version)
        {
            Ok(colors) => Response::Colors { colors },
            Err(reason) => failed(reason),
        },
        Request::InlineValues {
            directory,
            path,
            version,
            span,
            frame,
            stopped,
        } => match core.documents.hub.inline_values(
            &place(&directory),
            &path,
            version,
            span,
            frame,
            stopped,
        ) {
            Ok(values) => Response::InlineValues { values },
            Err(reason) => failed(reason),
        },
        Request::CodeLenses {
            directory,
            path,
            version,
            dependencies,
        } => {
            let root = place(&directory);
            match core.documents.hub.code_lenses(&root, &path, version) {
                Ok(mut lenses) => {
                    let after = lenses.len();
                    crate::dependencies::forget(&root, &path);
                    let listed = dependencies
                        .then(|| {
                            core.documents
                                .hub
                                .read_open(&root, &path, version, |language, buffer| {
                                    (
                                        language.to_owned(),
                                        crate::dependencies::listed(language, &path, buffer),
                                    )
                                })
                                .ok()
                        })
                        .flatten();
                    if let Some((language, found)) = listed {
                        lenses.extend(crate::dependencies::lenses(
                            &root, &path, &language, found, after,
                        ));
                    }
                    Response::CodeLenses { lenses }
                }
                Err(reason) => failed(reason),
            }
        }
        Request::PressCodeLens {
            directory,
            path,
            index,
        } => {
            let root = place(&directory);
            let own = crate::dependencies::press(&root, &path, index, |span| {
                core.documents
                    .hub
                    .read_open(&root, &path, 0, |_, buffer| {
                        let range = buffer.offset_of(span.start)..buffer.offset_of(span.end);
                        buffer.text().get(range).map(str::to_owned)
                    })
                    .ok()
                    .flatten()
            });
            match own {
                Some(Ok(edit)) => Response::LensPressed {
                    places: Vec::new(),
                    files: vec![edit],
                },
                Some(Err(reason)) => failed(reason),
                None => match core.documents.hub.press_lens(&root, &path, index) {
                    Ok((places, files)) => Response::LensPressed { places, files },
                    Err(reason) => failed(reason),
                },
            }
        }
        Request::TakeCodeAction {
            directory,
            path,
            index,
        } => match core
            .documents
            .hub
            .take_action(&place(&directory), &path, index)
        {
            Ok((files, shown)) if shown.is_empty() => Response::Edited { files },
            Ok((files, shown)) => Response::ActionTaken { files, shown },
            Err(reason) => failed(reason),
        },
        Request::Hover {
            directory,
            path,
            version,
            position,
        } => match core
            .documents
            .hub
            .hover(&place(&directory), &path, version, position)
        {
            Ok(hover) => Response::Hovered { hover },
            Err(reason) => failed(reason),
        },
        Request::OpenTerminal {
            directory,
            columns,
            rows,
        } => {
            let folder = place(&directory);
            if !folder.is_dir() {
                return failed(format!("{directory} is not a folder on this machine"));
            }
            match core.open_terminal(&folder, &directory, columns, rows) {
                Ok(terminal) => {
                    core.attach_terminal(connection, terminal, 0);
                    Response::Terminal { terminal }
                }
                Err(reason) => failed(format!("could not start a shell: {reason}")),
            }
        }
        Request::TerminalInput { terminal, data } => {
            terminal_done(core.type_into_terminal(terminal, &data), terminal)
        }
        Request::ResizeTerminal {
            terminal,
            columns,
            rows,
        } => terminal_done(core.resize_terminal(terminal, columns, rows), terminal),
        Request::CloseTerminal { terminal } => {
            terminal_done(core.close_terminal(terminal), terminal)
        }
        Request::AttachTerminal { terminal, after } => {
            terminal_done(core.attach_terminal(connection, terminal, after), terminal)
        }
        Request::Terminals => Response::Terminals {
            terminals: core.terminal_infos(),
        },
        Request::Tasks { directory } => tasks::listed(&place(&directory)),
        Request::RunTask {
            directory,
            project,
            task,
            columns,
            rows,
        } => run_started(
            core.run_task(
                &place(&directory),
                &directory,
                project.as_deref(),
                &task,
                (columns, rows),
            ),
            connection,
            core,
        ),
        Request::RunRunnable {
            directory,
            project,
            path,
            kind,
            name,
            tags,
            variables,
            position,
            task,
            columns,
            rows,
        } => {
            let root = place(&directory);
            let found = tasks::Place {
                tags: if tags.is_empty() { vec![kind] } else { tags },
                variables: if variables.is_empty() && !name.is_empty() {
                    BTreeMap::from([("name".to_owned(), name)])
                } else {
                    variables
                },
                position,
            };
            match tasks::runnable(&root, &path, &found, task.as_deref()) {
                Ok(task) => run_started(
                    core.run_task(
                        &root,
                        &directory,
                        project.as_deref(),
                        &task,
                        (columns, rows),
                    ),
                    connection,
                    core,
                ),
                Err(reason) => failed(reason),
            }
        }
        Request::RunnableTasks {
            directory,
            path,
            tags,
            variables,
            position,
            ..
        } => {
            let found = tasks::Place {
                tags,
                variables,
                position,
            };
            Response::RunnableTasks {
                tasks: tasks::runnable_tasks(&place(&directory), &path, &found)
                    .into_iter()
                    .map(|task| pm_protocol::RunnableTask {
                        name: task.name,
                        command: task.command,
                    })
                    .collect(),
            }
        }
        Request::Toolchains { directory, project } => {
            tasks::toolchains(&place(&directory), project.as_deref())
        }
        Request::SelectToolchain {
            project,
            language,
            toolchain,
        } => done(tasks::select(&project, &language, toolchain.as_deref())),
        Request::DebugScenarios { directory, file } => Response::DebugScenarios {
            scenarios: debugs::scenarios(&place(&directory), file.as_deref()),
        },
        Request::DebugProcesses => {
            let (adapters, processes) = debugs::processes();
            Response::DebugProcesses {
                adapters,
                processes,
            }
        }
        Request::DebugStart {
            directory,
            file,
            scenario,
            files,
            project,
        } => debug_started(core.debug_scenario(
            connection,
            &directory,
            project.as_deref(),
            file.as_deref(),
            scenario,
            files,
        )),
        Request::DebugAttach {
            directory,
            adapter,
            pid,
            files,
        } => debug_started(core.debug_attach(
            connection,
            &directory,
            &place(&directory),
            &adapter,
            pid,
            files,
        )),
        Request::DebugDo { debug, action } => debug_done(core.debug_do(debug, action), debug),
        Request::DebugWatch { debug } => match core.watch_debug(connection, debug) {
            Some(view) => Response::Debug { debug, view },
            None => failed(format!("no debugging session is numbered {debug}")),
        },
        Request::Debugs => Response::Debugs {
            debugs: core.debug_infos(),
        },
        Request::Commit {
            directory,
            message,
            tracked,
            amend,
        } => done(pm_git::commit(&place(&directory), &message, tracked, amend)),
        Request::CherryPick { directory, commit } => {
            done(pm_git::cherry_pick(&place(&directory), &commit))
        }
        Request::Stashes { directory } => Response::Stashes {
            stashes: pm_git::stashes(&place(&directory)),
        },
        Request::Stash {
            directory,
            action,
            index,
            message,
        } => done(pm_git::stash(
            &place(&directory),
            &action,
            index,
            message.as_deref(),
        )),
        Request::Step {
            directory,
            action,
            message,
        } => done(pm_git::step(
            &place(&directory),
            &action,
            message.as_deref(),
        )),
        Request::Log {
            directory,
            limit,
            all,
        } => Response::Log {
            commits: pm_git::history(&place(&directory), limit, all),
        },
        Request::Sync {
            directory,
            operation,
        } => done(pm_git::remote(&place(&directory), &operation)),
        Request::BranchInfo { directory, branch } => {
            let info = pm_git::branch_info(&place(&directory), branch.as_deref());
            Response::BranchInfo {
                remotes: info.remotes,
                remote_branches: info.remote_branches,
                unmerged: info.unmerged,
                unpushed: info.unpushed,
                elsewhere: info.elsewhere,
                follows: info.follows,
            }
        }
        Request::BranchOperation {
            directory,
            action,
            branch,
            target,
            remote,
            with_remote,
        } => done(pm_git::branch_operation(
            &place(&directory),
            &action,
            &branch,
            target.as_deref(),
            remote.as_deref(),
            with_remote,
        )),
    }
}

/// What tells `connection` the page and code a sign-in of `agent` waits for the reader to enter.
fn code_teller(connection: &Arc<Connection>, agent: Agent) -> impl Fn(&str, &str) {
    let connection = Arc::clone(connection);
    move |url: &str, code: &str| {
        let _ = connection.outbox.send(ServerMessage::SignInCode {
            agent: agent.id.to_owned(),
            url: url.to_owned(),
            code: code.to_owned(),
        });
    }
}

/// Signs `agent` in under the folder of the login `login`, so that the sign-in does not depend on which folder other requests have set; the agent's own default folder when there is no login. `code` is told the page and code to show when the provider signs in with one.
fn sign_in_login(
    agent: Agent,
    login: Option<&str>,
    code: &dyn Fn(&str, &str),
) -> Result<(), String> {
    match login {
        Some(login) => {
            let home = pm_store::account_directory(agent.id, login).map_err(|e| e.to_string())?;
            agent.sign_in_folder(home, code)
        }
        None => agent.sign_in_in(None, code),
    }
}

/// Signs in on this server using the session's account, then tells every attached client; `connection`, which asked, is shown the code when the provider signs in with one.
fn sign_in_session(core: &Core, connection: &Arc<Connection>, number: u64) -> Response {
    let found = core.sessions.lock().ok().and_then(|sessions| {
        let held = sessions.get(&number)?;
        Some((held.agent, held.meta.agent.clone(), held.meta.login.clone()))
    });
    let Some((agent, id, login)) = found else {
        return failed(format!("no session is numbered {number}"));
    };
    let home = login_folder(&id, login.as_deref());
    let result = if login.is_some() && home.is_none() {
        Err("the session's account no longer exists".to_owned())
    } else {
        agent.sign_in_in(home, &code_teller(connection, agent))
    };
    let event = match &result {
        Ok(()) => pm_harness::Event::SignedIn,
        Err(reason) => pm_harness::Event::SignedOut {
            reason: reason.clone(),
            account: login_name(&id, login.as_deref()),
        },
    };
    let mut told = Vec::new();
    if let Ok(mut sessions) = core.sessions.lock()
        && let Some(held) = sessions.get_mut(&number)
        && held.meta.login == login
    {
        told.push((number, held.note(number, &event), event));
    }
    core.tell_all(told);
    match result {
        Ok(()) => Response::Done,
        Err(reason) => failed(reason),
    }
}

#[cfg(test)]
mod tests;
