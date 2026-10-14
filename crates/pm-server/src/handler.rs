//! Carrying out what a client asks.

use std::path::PathBuf;
use std::sync::Arc;

use pm_acp::{AGENTS, Agent};
use pm_protocol::{Attachment, Request, Response, ServerMessage};

use crate::conversation;
use crate::core::{Connection, Core, Session, Told, now};
use crate::queue::Prompt;
use crate::sessions::{self, Meta};
use crate::uploads;

/// The agent whose id is `id`.
fn agent_named(id: &str) -> Option<Agent> {
    AGENTS.iter().copied().find(|agent| agent.id == id)
}

/// The folder `path` names on this machine, with a leading `~` taken to be the reader's home folder.
fn place(path: &str) -> PathBuf {
    match (path.strip_prefix("~/"), std::env::home_dir()) {
        (Some(rest), Some(home)) => home.join(rest),
        _ => PathBuf::from(path),
    }
}

/// A response saying the request failed, for `message`.
fn failed(message: impl Into<String>) -> Response {
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

/// Points every agent at the folder of the login in use for it, or at its own when none is.
fn apply_accounts(logins: &[pm_protocol::Login], in_use: &[(String, String)]) {
    for agent in AGENTS {
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

/// Starts a session and hands its agent the first prompt.
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
) -> Response {
    let Some(agent) = agent_named(agent) else {
        return failed(format!("no agent is called {agent}"));
    };
    let (folder, head) = match base {
        Some(_) if !pm_git::is_repository(&place(repository)) => {
            return failed(format!(
                "{repository} is not a git repository, so it has no worktrees"
            ));
        }
        Some(base) => match pm_git::add_worktree(&place(repository), title, base) {
            Ok(made) => (made.path, Some(made.head)),
            Err(reason) => return failed(format!("could not make a worktree: {reason}")),
        },
        None => {
            let folder = place(directory);
            if !folder.is_dir() {
                return failed(format!("{directory} is not a folder on this machine"));
            }
            (folder, None)
        }
    };
    let meta = Meta {
        agent: agent.id.to_owned(),
        repository: repository.to_owned(),
        directory: folder.to_string_lossy().into_owned(),
        branch: None,
        head: head.clone(),
        base: base.map(str::to_owned),
        title: title.to_owned(),
        closed: false,
        updated: now(),
    };
    let mut session = Session::new(agent, folder.clone(), Vec::new(), meta);
    session.start_agent(settings);
    let number = hold_session(core, connection, session);
    let first = prompt(number, text, attachments);
    telling(core, number, |held| held.submit(number, first, false));
    announce(core, connection, number);
    Response::Started {
        session: number,
        directory: folder.to_string_lossy().into_owned(),
        branch: None,
        head,
    }
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
        Session::new(held.agent, held.directory.clone(), log, meta)
    };
    let number = hold_session(core, connection, branch);
    announce(core, connection, number);
    match core.info(number) {
        Some(info) => Response::Sessions {
            sessions: vec![info],
        },
        None => failed(format!("session {number} went away as it was made")),
    }
}

/// Holds `session` under a new number, keeps it and its log on disk, has `connection` watch it and returns the number.
fn hold_session(core: &Core, connection: &Arc<Connection>, session: Session) -> u64 {
    let number = core.next_session();
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
    number
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
            Some(agent) => match pm_acp::discover(agent) {
                Ok(found) => Response::Discovery(found),
                Err(reason) => failed(reason),
            },
            None => failed(format!("no agent is called {agent}")),
        },
        Request::Skills { agent, directory } => match agent_named(&agent) {
            Some(agent) => Response::Skills {
                skills: pm_acp::skills(agent, &place(&directory)),
                names: pm_acp::skill_names(agent, &place(&directory))
                    .into_iter()
                    .collect(),
            },
            None => failed(format!("no agent is called {agent}")),
        },
        Request::Files { directory, limit } => Response::Files {
            paths: pm_git::files(&place(&directory), limit),
        },
        Request::Branches { directory } => Response::Branches {
            names: pm_git::branches(&place(&directory)),
        },
        Request::Identity { agent } => match agent_named(&agent) {
            Some(agent) => Response::Identity {
                default: pm_acp::default_account(agent),
                selected: pm_acp::account(agent),
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
        Request::Sessions => Response::Sessions {
            sessions: core.infos(),
        },
        Request::SignIn { agent } => match agent_named(&agent) {
            Some(agent) => match agent.sign_in() {
                Ok(()) => Response::Done,
                Err(reason) => failed(reason),
            },
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
        ),
        Request::CloseSession { session } => {
            let closed = telling(core, session, |held| {
                if let Some(chat) = held.chat.take() {
                    chat.cancel();
                }
                held.busy = false;
                held.meta.closed = true;
                sessions::save_meta(session, &held.meta);
                held.drop_queue(session)
            });
            if matches!(closed, Response::Done) {
                core.broadcast(&ServerMessage::SessionClosed { session }, Some(connection));
            }
            closed
        }
        Request::Prompt {
            session,
            text,
            attachments,
            now,
        } => {
            let next = prompt(session, &text, &attachments);
            telling(core, session, |held| held.submit(session, next, now))
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
        Request::Cancel { session } => telling(core, session, |held| {
            if let Some(chat) = &held.chat {
                chat.cancel();
            }
            held.drop_queue(session)
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
            if !pm_git::is_repository(directory) {
                return Response::Inspected {
                    changes: Vec::new(),
                    commits: None,
                    branch: None,
                    head: None,
                    tracking: pm_git::Tracking::default(),
                    operation: None,
                };
            }
            Response::Inspected {
                changes: pm_git::changes(directory),
                commits: base.map(|base| pm_git::commits_since(directory, &base)),
                branch: pm_git::current_branch(directory),
                head: pm_git::resolve(directory, "HEAD"),
                tracking: pm_git::tracking(directory),
                operation: pm_git::operation(directory),
            }
        }
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
        Request::Discard { directory, paths } => done(pm_git::discard(&place(&directory), &paths)),
        Request::Ignore { directory, paths } => done(pm_git::ignore(&place(&directory), &paths)),
        Request::Checkout { directory, branch } => {
            done(pm_git::checkout(&place(&directory), &branch))
        }
        Request::CreateBranch { directory, name } => {
            done(pm_git::create_branch(&place(&directory), &name))
        }
        Request::ReadFile {
            directory,
            path,
            head,
        } => match pm_git::read(&place(&directory), &path, head) {
            Ok(pm_git::Contents::Text { text, truncated }) => Response::File {
                text: Some(text),
                truncated,
            },
            Ok(pm_git::Contents::Binary) => Response::File {
                text: None,
                truncated: false,
            },
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
    }
}
