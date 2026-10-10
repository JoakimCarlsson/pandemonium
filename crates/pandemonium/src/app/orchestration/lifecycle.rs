//! Creation reservations, negotiated startup and cancellation rollback.

use super::Pending;
use super::tools::{text_argument, validate_arguments};
use crate::agent::{Block, Talk};
use crate::app::{App, Wake};
use crate::config;
use crate::orchestration::{Call, Server};
use pm_acp::{About, Agent, Setting};
use pm_core::{Cutting, Delegation, Session, SessionId, StartError};
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

/// Maximum time an adapter may take to open a delegated conversation.
const OPEN_TIMEOUT: Duration = Duration::from_secs(45);

impl App {
    /// Starts the local server before restoring or opening agent conversations.
    pub(in crate::app) fn start_orchestration(&mut self) {
        if self.orchestration.server.is_some() {
            return;
        }
        match Server::start(self.waker(Wake::Orchestration)) {
            Ok(server) => {
                self.agents.set_mcp(server.factory());
                self.orchestration.server = Some(server);
            }
            Err(error) => self.notices.trouble(
                format!("Session orchestration is unavailable: {error}"),
                None,
            ),
        }
    }

    /// Drains authenticated requests and checks cancellations and startup timeouts.
    pub(in crate::app) fn serve_orchestration(&mut self) {
        let calls = self
            .orchestration
            .server
            .as_ref()
            .map(Server::drain)
            .unwrap_or_default();
        for call in calls {
            if call.name == "notifications/cancelled" {
                for pending in self.orchestration.pending.values_mut() {
                    if pending.call.caller == call.caller && pending.call.id == call.id {
                        pending.call.abandoned.store(true, Ordering::Release);
                    }
                }
                continue;
            }
            if call.abandoned.load(Ordering::Acquire) {
                continue;
            }
            if call.name == "create_session" {
                if let Err(error) = self.delegate(&call) {
                    call.answer(Err(error));
                }
            } else {
                let result = self.session_tool(&call);
                call.answer(result);
            }
        }
        self.finish_delegations();
        self.request_redraw();
    }

    /// Reserves delegation limits and launches the existing background worktree cut.
    fn delegate(&mut self, call: &Call) -> Result<(), String> {
        validate_arguments(call)?;
        let project = self.orchestration_project(call)?;
        let name = text_argument(&call.arguments, "name", 100)?;
        if !name
            .chars()
            .any(|character| character.is_ascii_alphanumeric())
        {
            return Err("The session name needs at least one letter or number".to_owned());
        }
        let prompt = text_argument(&call.arguments, "prompt", 8192)?;
        let source = self
            .agents
            .get(call.caller)
            .ok_or("The caller disappeared")?;
        let own_project = self
            .open
            .get(source.scope().project())
            .ok_or("The calling project is closed")?;
        let depth = source
            .scope()
            .session()
            .and_then(|id| self.sessions.get(id))
            .and_then(Session::delegation)
            .map_or(1, |delegation| delegation.depth.saturating_add(1));
        let parent = Delegation {
            project: own_project.root().stored(),
            parent: source.root().stored(),
            conversation: source.resumable(),
            name: source
                .scope()
                .session()
                .and_then(|id| self.sessions.get(id))
                .map_or_else(
                    || own_project.name().to_owned(),
                    |session| session.name().to_owned(),
                ),
            depth,
        };
        let limits = &self.preferences.orchestration;
        let children = self
            .open
            .iter()
            .flat_map(|project| self.sessions.of(project.id()))
            .filter(|session| {
                session
                    .delegation()
                    .is_some_and(|held| held.parent == parent.parent)
            })
            .count();
        let pending_children = self
            .orchestration
            .pending
            .values()
            .filter(|pending| pending.parent.parent == parent.parent && pending.session.is_none())
            .count();
        let total = self
            .open
            .iter()
            .flat_map(|project| self.sessions.of(project.id()))
            .filter(|session| session.delegation().is_some())
            .count()
            + self
                .orchestration
                .pending
                .values()
                .filter(|pending| pending.session.is_none())
                .count();
        if depth > limits.max_depth {
            return Err("Delegation depth exceeds orchestration.max_depth".to_owned());
        }
        if children + pending_children >= limits.max_children {
            return Err("Parent child limit reached (orchestration.max_children); finish reviewed children before spawning more".to_owned());
        }
        if total >= limits.max_sessions {
            return Err("Window delegation limit reached (orchestration.max_sessions)".to_owned());
        }
        let agent = match call.arguments.get("agent") {
            Some(value) => Agent::named(
                value
                    .as_str()
                    .ok_or("agent must be an advertised id string")?,
            )
            .filter(|agent| {
                self.open
                    .get(project)
                    .is_some_and(|project| agent.startable_on(&project.root().host))
            })
            .ok_or("The selected agent is not advertised or cannot be started")?,
            None => source.agent(),
        };
        let model = call
            .arguments
            .get("model")
            .map(|_| text_argument(&call.arguments, "model", 256))
            .transpose()?;
        if model
            .as_ref()
            .is_some_and(|model| !self.advertised_models(agent).contains(model))
        {
            return Err("The selected model is not advertised for this agent; omit model or use list_sessions choices".to_owned());
        }
        let project = self
            .open
            .get(project)
            .cloned()
            .ok_or("The target project closed")?;
        let under = config::worktrees().ok_or("The editor has no worktree directory")?;
        let chosen = project
            .repositories()
            .iter()
            .map(|repository| repository.root().to_path_buf())
            .collect::<Vec<_>>();
        let reply = call
            .reply
            .clone()
            .ok_or("Creation requires a reply channel")?;
        let ticket = self.orchestration.next;
        self.orchestration.next += 1;
        self.orchestration.pending.insert(
            ticket,
            Pending {
                call: Call {
                    caller: call.caller,
                    id: call.id.clone(),
                    name: call.name.clone(),
                    arguments: call.arguments.clone(),
                    reply: Some(reply),
                    abandoned: call.abandoned.clone(),
                },
                project: project.id(),
                parent,
                agent,
                prompt,
                model,
                model_requested: false,
                session: None,
                talk: None,
                started: Instant::now(),
            },
        );
        self.cut_session_later(&project, &name, "", &chosen, &under, Some(ticket));
        let wake = self.waker(Wake::Orchestration);
        std::thread::spawn(move || {
            std::thread::sleep(OPEN_TIMEOUT);
            wake();
        });
        Ok(())
    }

    /// Takes a delegated cut up, records ancestry, and opens its ordinary agent pane.
    pub(in crate::app) fn take_delegated_cut(
        &mut self,
        ticket: u64,
        cut: Result<Cutting, StartError>,
    ) {
        let Some(mut pending) = self.orchestration.pending.remove(&ticket) else {
            return;
        };
        let cutting = match cut {
            Ok(cutting) => cutting,
            Err(error) => {
                pending.call.answer(Err(format!(
                    "The delegated worktree could not be cut: {error}"
                )));
                return;
            }
        };
        let started = self.sessions.took(cutting);
        pending.session = Some(started.id);
        if let Err(error) = self.sessions.delegated(started.id, pending.parent.clone()) {
            self.rollback_delegation(
                pending,
                &format!("Could not persist child ancestry: {error}"),
            );
            return;
        }
        if let Some(error) = self.delegation_unavailable(&pending) {
            self.rollback_delegation(pending, &error);
            return;
        }
        let Some(root) = self
            .sessions
            .get(started.id)
            .map(|session| session.root().clone())
        else {
            return;
        };
        let profile = self
            .agents
            .get(pending.call.caller)
            .filter(|source| source.agent() == pending.agent)
            .and_then(Talk::profile)
            .cloned();
        let Some(talk) = self.open_agent(
            pending.project,
            Some(started.id),
            &root,
            pending.agent,
            profile.as_ref(),
            false,
        ) else {
            self.rollback_delegation(
                pending,
                "The delegated agent process could not start; check its installed command",
            );
            return;
        };
        pending.talk = Some(talk);
        self.orchestration.pending.insert(ticket, pending);
        self.say_bootstrap_trouble(&started.trouble);
        self.finish_delegations();
        self.store();
    }

    /// Completes ready children and rolls back cancelled, missing or unsupported startups.
    pub(in crate::app) fn finish_delegations(&mut self) {
        let ids = self.agents.iter().map(Talk::id).collect::<Vec<_>>();
        if let Some(server) = self.orchestration.server.as_ref() {
            server.retain(&ids);
        }
        let tickets = self
            .orchestration
            .pending
            .keys()
            .copied()
            .collect::<Vec<_>>();
        for ticket in tickets {
            let Some(pending) = self.orchestration.pending.get(&ticket) else {
                continue;
            };
            if pending.session.is_none() {
                continue;
            }
            if let Some(error) = self.delegation_unavailable(pending) {
                let pending = self.orchestration.pending.remove(&ticket).unwrap();
                self.rollback_delegation(pending, &error);
                continue;
            }
            let Some(talk) = pending.talk.and_then(|id| self.agents.get(id)) else {
                continue;
            };
            if !talk.is_ready() || talk.is_configuring() {
                continue;
            }
            let available = talk
                .mcp_servers()
                .iter()
                .any(|server| server.name == "pandemonium-sessions" && server.given);
            if !available {
                let pending = self.orchestration.pending.remove(&ticket).unwrap();
                self.rollback_delegation(pending, "This agent does not advertise MCP HTTP support; select an HTTP-capable adapter");
                continue;
            }
            if let Some(model) = &pending.model {
                let knob = talk.knobs().into_iter().find(|knob| knob.about == About::Model && matches!(&knob.setting, Setting::Picked { picks, .. } if picks.iter().any(|pick| pick.id == *model)));
                let Some(knob) = knob else {
                    let pending = self.orchestration.pending.remove(&ticket).unwrap();
                    self.rollback_delegation(pending, "The child did not advertise the requested model; omit model or choose a supported one");
                    continue;
                };
                if !pending.model_requested {
                    talk.set_knob(&knob.id, model);
                    self.orchestration
                        .pending
                        .get_mut(&ticket)
                        .unwrap()
                        .model_requested = true;
                    continue;
                }
                if !matches!(&knob.setting, Setting::Picked { value, .. } if value == model) {
                    let pending = self.orchestration.pending.remove(&ticket).unwrap();
                    self.rollback_delegation(pending, "The adapter did not accept the requested model; choose an advertised model");
                    continue;
                }
            }
            let pending = self.orchestration.pending.remove(&ticket).unwrap();
            let Some(session) = pending.session.and_then(|id| self.sessions.get(id)) else {
                continue;
            };
            let metadata = self.session_metadata(session);
            let prompt = format!(
                "Delegated task from {} in {}. This task grants no permission to commit, push or land changes. Keep the worktree available for review.\n\n{}",
                pending.parent.name,
                pending.parent.parent.display(),
                pending.prompt
            );
            if let Some(talk) = pending.talk.and_then(|id| self.agents.get_mut(id)) {
                talk.send_text(&prompt);
            }
            pending.call.answer(Ok(metadata));
            self.store();
        }
    }

    /// The reason a pending creation can no longer become a usable child.
    fn delegation_unavailable(&self, pending: &Pending) -> Option<String> {
        if pending.call.abandoned.load(Ordering::Acquire) {
            return Some(
                "Delegation was cancelled or timed out; its worktree is being removed".to_owned(),
            );
        }
        if pending.started.elapsed() >= OPEN_TIMEOUT {
            return Some(
                "Delegated agent startup timed out; verify its login and MCP HTTP support"
                    .to_owned(),
            );
        }
        if self.open.get(pending.project).is_none()
            || self
                .agents
                .get(pending.call.caller)
                .is_none_or(|talk| !talk.is_running())
        {
            return Some(
                "The calling conversation or target project closed during creation".to_owned(),
            );
        }
        if pending.session.is_some_and(|id| self.session_finishing(id)) {
            return Some("The child was finished during creation".to_owned());
        }
        if let Some(id) = pending.talk {
            let Some(talk) = self.agents.get(id) else {
                return Some("The child agent pane closed during creation".to_owned());
            };
            if !talk.is_running()
                || talk
                    .transcript()
                    .blocks()
                    .iter()
                    .any(|block| matches!(block, Block::Failure(_, _)))
            {
                return Some("The child agent failed to open; verify its command, credentials and ACP support".to_owned());
            }
            if talk.pending().is_some() {
                return Some("The child requires interactive login; authenticate this agent before delegating".to_owned());
            }
        }
        None
    }

    /// Stops a failed startup and removes its worktree through the existing teardown seam.
    fn rollback_delegation(&mut self, pending: Pending, error: &str) {
        if let Some(id) = pending.talk {
            let held = self
                .agents
                .iter()
                .map(Talk::id)
                .filter(|held| *held != id)
                .collect();
            self.agents.retain(&held);
        }
        if let Some(session) = pending.session {
            self.orchestration
                .rollback
                .insert(session, (pending.call, error.to_owned()));
            self.finish_session_later(session);
        } else {
            pending.call.answer(Err(error.to_owned()));
        }
    }

    /// Answers a rejected creation after teardown has either completed or reported its failure.
    pub(in crate::app) fn delegation_removed(
        &mut self,
        session: SessionId,
        finished: &Result<(), StartError>,
    ) {
        if let Some((call, mut error)) = self.orchestration.rollback.remove(&session) {
            if let Err(trouble) = finished {
                let path = self
                    .sessions
                    .get(session)
                    .map(|session| session.root().display().to_string())
                    .unwrap_or_default();
                error.push_str(&format!(". Worktree cleanup failed at {path}: {trouble}. It remains in the session list; finish it after resolving the git error."));
            }
            call.answer(Err(error));
        }
    }
}
