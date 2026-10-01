//! Typed ACP control for clients outside the desktop window.

use std::path::{Component, Path, PathBuf};

use pm_acp::{Agent, Setting, Way};
use pm_core::{Project, Session, SessionId};
use serde_json::{Value, json};

use crate::agent::{Talk, TalkId};
use crate::app::App;
use crate::control::AgentOperation;
use crate::message::Message;

impl App {
    /// Applies a typed ACP request to the running editor.
    pub(super) fn control_agent_operation(
        &mut self,
        operation: AgentOperation,
    ) -> Result<Value, String> {
        match operation {
            AgentOperation::Catalog => Ok(
                json!({ "agents": pm_acp::agents().iter().map(|agent| json!({
                "id": agent.id,
                "name": agent.name,
                "startable": agent.startable(),
                "installed": agent.installed(),
            })).collect::<Vec<_>>() }),
            ),
            AgentOperation::Start {
                project,
                session,
                agent,
            } => self.control_start_agent(project, session, &agent),
            AgentOperation::Detail(id) => self.control_agent_detail(id),
            AgentOperation::Send { id, text, files } => self.control_send_agent(id, &text, &files),
            AgentOperation::Cancel(id) => {
                let id = self.control_talk_id(id)?;
                let talk = self
                    .agents
                    .get(id)
                    .ok_or_else(|| "agent unavailable".to_owned())?;
                if !talk.is_busy() {
                    return Err("agent has no running turn".to_owned());
                }
                talk.cancel();
                Ok(json!({ "cancelled": true }))
            }
            AgentOperation::Answer {
                id,
                request,
                choice,
            } => self.control_answer_agent(id, request, choice.as_deref()),
            AgentOperation::SetMode { id, mode } => self.control_set_agent_mode(id, &mode),
            AgentOperation::SetKnob { id, knob, value } => {
                self.control_set_agent_knob(id, &knob, &value)
            }
            AgentOperation::ListHistory(id) => self.control_list_agent_history(id),
            AgentOperation::LoadHistory { id, saved } => {
                self.control_load_agent_history(id, &saved)
            }
            AgentOperation::Login { id, method } => self.control_login_agent(id, &method),
            AgentOperation::Terminal { id, terminal } => {
                let id = self.control_talk_id(id)?;
                let talk = self
                    .agents
                    .get(id)
                    .ok_or_else(|| "agent unavailable".to_owned())?;
                let tail = talk
                    .terminal_tail(&terminal)
                    .ok_or_else(|| "tool terminal unavailable".to_owned())?;
                Ok(json!({ "terminal": terminal, "tail": tail }))
            }
        }
    }

    /// Starts a new ACP conversation in a project checkout or worktree.
    fn control_start_agent(
        &mut self,
        project: u64,
        session: Option<u64>,
        agent: &str,
    ) -> Result<Value, String> {
        let project = self
            .open
            .iter()
            .find(|held| held.id().number() == project)
            .map(Project::id)
            .ok_or_else(|| "project unavailable".to_owned())?;
        let opened = self
            .open
            .get(project)
            .ok_or_else(|| "project unavailable".to_owned())?;
        if !opened.root().host.is_local() {
            return Err("agents on SSH projects are not available yet".to_owned());
        }
        let session = session
            .map(|id| {
                self.sessions
                    .iter()
                    .find(|held| held.id().number() == id)
                    .map(Session::id)
                    .ok_or_else(|| "session unavailable".to_owned())
            })
            .transpose()?;
        if session.is_some_and(|id| {
            self.sessions
                .get(id)
                .is_none_or(|held| held.project() != project)
        }) {
            return Err("session belongs to another project".to_owned());
        }
        let agent = Agent::named(agent).ok_or_else(|| "unknown agent".to_owned())?;
        if !agent.startable() {
            return Err("agent is not installed or startable".to_owned());
        }
        let root = match session {
            Some(id) => self.sessions.get(id).map(Session::root).map(PathBuf::from),
            None => self
                .open
                .get(project)
                .map(Project::root)
                .map(|root| root.path.clone()),
        }
        .ok_or_else(|| "worktree unavailable".to_owned())?;
        let before = self.agents.iter().map(Talk::id).collect::<Vec<_>>();
        self.open_agent(project, session, &root, agent);
        let opened = self
            .agents
            .iter()
            .find(|talk| !before.contains(&talk.id()))
            .ok_or_else(|| "agent could not start".to_owned())?;
        Ok(
            json!({ "id": opened.id().number(), "project_id": project.number(), "session_id": session.map(SessionId::number) }),
        )
    }

    /// Returns a conversation's offered ACP controls and current state.
    fn control_agent_detail(&self, id: u64) -> Result<Value, String> {
        let id = self.control_talk_id(id)?;
        let talk = self
            .agents
            .get(id)
            .ok_or_else(|| "agent unavailable".to_owned())?;
        let modes = talk
            .modes()
            .into_iter()
            .map(|mode| {
                json!({
                    "id": mode.id, "name": mode.name, "description": mode.description,
                })
            })
            .collect::<Vec<_>>();
        let knobs = talk
            .knobs()
            .into_iter()
            .map(|knob| {
                let setting = match knob.setting {
                    Setting::Picked { value, picks } => json!({
                        "kind": "picked", "value": value,
                        "picks": picks.into_iter().map(|pick| json!({
                            "id": pick.id, "name": pick.name, "description": pick.description,
                        })).collect::<Vec<_>>(),
                    }),
                    Setting::Switched(value) => json!({ "kind": "switched", "value": value }),
                };
                json!({
                    "id": knob.id, "name": knob.name, "description": knob.description,
                    "about": format!("{:?}", knob.about).to_lowercase(), "setting": setting,
                })
            })
            .collect::<Vec<_>>();
        let requests = talk
            .asks()
            .iter()
            .map(|ask| {
                json!({
                    "id": ask.id,
                    "tool": { "id": ask.tool.id, "title": ask.tool.title, "name": ask.tool.name },
                    "choices": ask.choices.iter().map(|choice| json!({
                        "id": choice.id, "name": choice.name,
                        "kind": format!("{:?}", choice.kind).to_lowercase(),
                    })).collect::<Vec<_>>(),
                })
            })
            .collect::<Vec<_>>();
        let logins = talk.logins().iter().map(|login| json!({
            "id": login.id, "name": login.name, "description": login.description,
            "way": match login.way { Way::Asked => "asked", Way::Terminal { .. } => "terminal" },
        })).collect::<Vec<_>>();
        Ok(json!({
            "id": id.number(), "agent": talk.agent().id, "title": talk.title(),
            "project_id": talk.scope().project().number(),
            "session_id": talk.scope().session().map(SessionId::number),
            "ready": talk.is_ready(), "busy": talk.is_busy(), "running": talk.is_running(),
            "standing": format!("{:?}", talk.standing()).to_lowercase(),
            "can_image": talk.can_image(), "resumable": talk.resumable(),
            "usage": talk.usage().map(|usage| json!({
                "used": usage.used, "size": usage.size,
                "cost": usage.cost.as_ref().map(|cost| json!({
                    "amount": cost.amount, "currency": cost.currency,
                })),
            })),
            "mode": talk.mode(), "modes": modes, "knobs": knobs,
            "requests": requests, "logins": logins,
            "can_list_history": talk.can_list(), "history_listing": talk.is_listing(),
            "history_error": talk.history_error(),
            "history": talk.history().iter().map(|saved| json!({
                "id": saved.id, "title": saved.title, "updated_at": saved.updated_at,
            })).collect::<Vec<_>>(),
            "transcript_revision": talk.transcript().revision(),
        }))
    }

    /// Sends exact text and optional files already present in the worktree.
    fn control_send_agent(
        &mut self,
        id: u64,
        text: &str,
        files: &[String],
    ) -> Result<Value, String> {
        let id = self.control_talk_id(id)?;
        let talk = self
            .agents
            .get(id)
            .ok_or_else(|| "agent unavailable".to_owned())?;
        if !talk.is_ready() || talk.is_busy() {
            return Err("agent is not ready for a prompt".to_owned());
        }
        if text.trim().is_empty() && files.is_empty() {
            return Err("prompt is empty".to_owned());
        }
        let root = talk
            .root()
            .canonicalize()
            .map_err(|error| error.to_string())?;
        let mut attachments = Vec::new();
        for file in files {
            let relative = Path::new(file);
            if !relative
                .components()
                .all(|part| matches!(part, Component::Normal(_) | Component::CurDir))
            {
                return Err("attached files must be relative to the worktree".to_owned());
            }
            let path = root
                .join(relative)
                .canonicalize()
                .map_err(|error| error.to_string())?;
            if !path.starts_with(&root) || !path.is_file() {
                return Err("attached file is outside the worktree or is not a file".to_owned());
            }
            attachments.push(path);
        }
        let scope = talk.scope();
        let talk = self
            .agents
            .get_mut(id)
            .ok_or_else(|| "agent unavailable".to_owned())?;
        talk.send_text_with_files(text, attachments);
        self.checks.reset(scope);
        self.follow_agents();
        Ok(json!({ "sent": true, "id": id.number() }))
    }

    /// Answers a permission request by its ticket and offered choice identity.
    fn control_answer_agent(
        &mut self,
        id: u64,
        request: u64,
        choice: Option<&str>,
    ) -> Result<Value, String> {
        let id = self.control_talk_id(id)?;
        let talk = self
            .agents
            .get(id)
            .ok_or_else(|| "agent unavailable".to_owned())?;
        let ask = talk
            .asks()
            .iter()
            .find(|ask| ask.id == request)
            .ok_or_else(|| "permission request unavailable".to_owned())?;
        let place = match choice {
            Some(choice) => ask
                .choices
                .iter()
                .position(|offered| offered.id == choice)
                .ok_or_else(|| "permission choice unavailable".to_owned())?,
            None => ask.choices.len(),
        };
        self.apply(Message::AnswerAgent(id, request, place));
        Ok(json!({ "answered": true }))
    }

    /// Selects an offered ACP mode through the desktop's preference seam.
    fn control_set_agent_mode(&mut self, id: u64, mode: &str) -> Result<Value, String> {
        let id = self.control_talk_id(id)?;
        let talk = self
            .agents
            .get(id)
            .ok_or_else(|| "agent unavailable".to_owned())?;
        if !talk.modes().iter().any(|offered| offered.id == mode) {
            return Err("mode unavailable".to_owned());
        }
        self.set_agent_mode(id, mode);
        Ok(json!({ "selected": mode }))
    }

    /// Sets an offered ACP knob through the desktop's preference seam.
    fn control_set_agent_knob(
        &mut self,
        id: u64,
        knob: &str,
        value: &Value,
    ) -> Result<Value, String> {
        let id = self.control_talk_id(id)?;
        let talk = self
            .agents
            .get(id)
            .ok_or_else(|| "agent unavailable".to_owned())?;
        let setting = talk
            .knob(knob)
            .ok_or_else(|| "knob unavailable".to_owned())?
            .setting;
        match setting {
            Setting::Picked { picks, .. } => {
                let wanted = value
                    .as_str()
                    .ok_or_else(|| "picked knob needs a string value".to_owned())?;
                if !picks.iter().any(|pick| pick.id == wanted) {
                    return Err("knob value unavailable".to_owned());
                }
                self.set_knob(id, knob, wanted);
            }
            Setting::Switched(current) => {
                let wanted = value
                    .as_bool()
                    .ok_or_else(|| "switched knob needs a boolean value".to_owned())?;
                if wanted != current {
                    self.toggle_agent_knob(id, knob);
                }
            }
        }
        Ok(json!({ "set": true }))
    }

    /// Starts listing saved conversations and returns those already available.
    fn control_list_agent_history(&mut self, id: u64) -> Result<Value, String> {
        let id = self.control_talk_id(id)?;
        let talk = self
            .agents
            .get_mut(id)
            .ok_or_else(|| "agent unavailable".to_owned())?;
        if !talk.can_list() {
            return Err("agent cannot list saved conversations".to_owned());
        }
        talk.list_history();
        Ok(json!({ "listing": true }))
    }

    /// Loads a saved conversation through the desktop's history seam.
    fn control_load_agent_history(&mut self, id: u64, saved: &str) -> Result<Value, String> {
        let id = self.control_talk_id(id)?;
        let talk = self
            .agents
            .get(id)
            .ok_or_else(|| "agent unavailable".to_owned())?;
        if !talk.history().iter().any(|entry| entry.id == saved) {
            return Err("saved conversation unavailable".to_owned());
        }
        let (scope, agent) = (talk.scope(), talk.agent());
        self.open_agent_history(id, saved);
        let opened = self
            .agents
            .find_saved(scope, agent, saved)
            .ok_or_else(|| "saved conversation could not load".to_owned())?;
        Ok(json!({ "id": opened.number() }))
    }

    /// Starts an offered direct login method for an agent.
    fn control_login_agent(&mut self, id: u64, method: &str) -> Result<Value, String> {
        let id = self.control_talk_id(id)?;
        let talk = self
            .agents
            .get(id)
            .ok_or_else(|| "agent unavailable".to_owned())?;
        let offered = talk
            .logins()
            .iter()
            .find(|login| login.id == method)
            .ok_or_else(|| "login method unavailable".to_owned())?;
        if !matches!(offered.way, Way::Asked) {
            return Err("terminal login requires the desktop terminal".to_owned());
        }
        let place = talk
            .logins()
            .iter()
            .position(|login| login.id == method)
            .ok_or_else(|| "login method unavailable".to_owned())?;
        self.log_in_agent(id, place);
        Ok(json!({ "started": true }))
    }

    /// Finds a running conversation by its identity within this window.
    fn control_talk_id(&self, id: u64) -> Result<TalkId, String> {
        self.agents
            .iter()
            .find(|talk| talk.id().number() == id)
            .map(Talk::id)
            .ok_or_else(|| "agent unavailable".to_owned())
    }
}
