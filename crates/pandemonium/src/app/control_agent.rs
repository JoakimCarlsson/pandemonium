//! Typed ACP control for clients outside the desktop window.

use std::path::{Component, Path, PathBuf};

use base64::Engine;
use pm_acp::{Agent, Attachment, Setting, Way};
use pm_core::{Project, Session, SessionId};
use serde_json::{Value, json};

use crate::agent::{Talk, TalkId};
use crate::app::App;
use crate::control::AgentOperation;
use crate::message::Message;

/// An image being transferred through bounded control requests.
pub(super) struct ControlImage {
    /// Image MIME type advertised by the client.
    mime_type: String,
    /// Optional display name for the attachment.
    name: Option<String>,
    /// Decoded image content accumulated from chunks.
    data: Vec<u8>,
    /// Whether the last chunk has been received and validated.
    complete: bool,
}

/// Maximum decoded bytes kept for one uploaded image.
const MAX_IMAGE_DATA: usize = 12 * 1024 * 1024;

/// Maximum decoded image bytes retained across all pending uploads.
const MAX_PENDING_IMAGE_DATA: usize = 48 * 1024 * 1024;

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
            AgentOperation::Send {
                id,
                text,
                files,
                images,
            } => self.control_send_agent(id, &text, &files, &images),
            AgentOperation::UploadImage {
                id,
                image,
                mime_type,
                name,
                data,
                finish,
            } => self.control_upload_image(id, image, &mime_type, name, &data, finish),
            AgentOperation::Cancel(id) => {
                let id = self.control_talk_id(id)?;
                let talk = self
                    .agents
                    .get_mut(id)
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
            AgentOperation::LoginRead(id) => self.control_read_login(id),
            AgentOperation::LoginWrite { id, input } => self.control_write_login(id, &input),
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
        self.open_agent(project, session, &root, agent, None, false);
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
        images: &[u64],
    ) -> Result<Value, String> {
        let id = self.control_talk_id(id)?;
        let talk = self
            .agents
            .get(id)
            .ok_or_else(|| "agent unavailable".to_owned())?;
        if !talk.is_ready() || talk.is_busy() {
            return Err("agent is not ready for a prompt".to_owned());
        }
        if text.trim().is_empty() && files.is_empty() && images.is_empty() {
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
            attachments.push(Attachment::File(path));
        }
        if !images.is_empty() && !talk.can_image() {
            return Err("agent does not accept image prompts".to_owned());
        }
        for image in images {
            let uploaded = self
                .control_images
                .get(&(id, *image))
                .ok_or_else(|| "image upload unavailable".to_owned())?;
            if !uploaded.complete {
                return Err("image upload is incomplete".to_owned());
            }
            attachments.push(Attachment::Image {
                data: base64::engine::general_purpose::STANDARD.encode(&uploaded.data),
                mime_type: uploaded.mime_type.clone(),
                name: uploaded.name.clone(),
            });
        }
        let scope = talk.scope();
        let talk = self
            .agents
            .get_mut(id)
            .ok_or_else(|| "agent unavailable".to_owned())?;
        talk.send_text_with_attachments(text, attachments);
        for image in images {
            self.control_images.remove(&(id, *image));
        }
        self.checks.reset(scope);
        self.follow_agents();
        Ok(json!({ "sent": true, "id": id.number() }))
    }

    /// Appends a bounded base64 chunk to a conversation's pending image.
    fn control_upload_image(
        &mut self,
        id: u64,
        image: u64,
        mime_type: &str,
        name: Option<String>,
        data: &str,
        finish: bool,
    ) -> Result<Value, String> {
        let id = self.control_talk_id(id)?;
        let talk = self
            .agents
            .get(id)
            .ok_or_else(|| "agent unavailable".to_owned())?;
        if !talk.can_image() {
            return Err("agent does not accept image prompts".to_owned());
        }
        let format = match mime_type {
            "image/png" => image::ImageFormat::Png,
            "image/jpeg" => image::ImageFormat::Jpeg,
            "image/gif" => image::ImageFormat::Gif,
            "image/webp" => image::ImageFormat::WebP,
            _ => return Err("unsupported image MIME type".to_owned()),
        };
        if data.len() > 60_000 || !data.len().is_multiple_of(4) {
            return Err("image chunk must be base64 and fit within 60000 bytes".to_owned());
        }
        let decoded = base64::engine::general_purpose::STANDARD
            .decode(data)
            .map_err(|_| "invalid base64 image chunk".to_owned())?;
        if decoded.is_empty() {
            return Err("image chunk is empty".to_owned());
        }
        let key = (id, image);
        if !self.control_images.contains_key(&key)
            && self
                .control_images
                .keys()
                .filter(|(held, _)| *held == id)
                .count()
                >= 4
        {
            return Err("too many pending image uploads".to_owned());
        }
        if self
            .control_images
            .values()
            .map(|held| held.data.len())
            .sum::<usize>()
            .saturating_add(decoded.len())
            > MAX_PENDING_IMAGE_DATA
        {
            return Err("pending image uploads exceed 48 MiB".to_owned());
        }
        let uploaded = self
            .control_images
            .entry(key)
            .or_insert_with(|| ControlImage {
                mime_type: mime_type.to_owned(),
                name: name.clone(),
                data: Vec::new(),
                complete: false,
            });
        if uploaded.complete || uploaded.mime_type != mime_type || uploaded.name != name {
            return Err("image upload metadata changed or upload is complete".to_owned());
        }
        if uploaded.data.len().saturating_add(decoded.len()) > MAX_IMAGE_DATA {
            self.control_images.remove(&key);
            return Err("image exceeds 12 MiB limit".to_owned());
        }
        uploaded.data.extend_from_slice(&decoded);
        if finish {
            if image::guess_format(&uploaded.data).map_err(|_| "unrecognized image".to_owned())?
                != format
            {
                return Err("image bytes do not match MIME type".to_owned());
            }
            uploaded.complete = true;
        }
        Ok(json!({ "image": image, "complete": uploaded.complete, "bytes": uploaded.data.len() }))
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
        self.open_agent_history(id, saved);
        let opened = self
            .agents
            .get(id)
            .filter(|talk| talk.resumable().as_deref() == Some(saved))
            .map(Talk::id)
            .ok_or_else(|| "saved conversation could not load".to_owned())?;
        Ok(json!({ "id": opened.number() }))
    }

    /// Starts an offered login method through the existing desktop seam.
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
        let terminal = matches!(offered.way, Way::Terminal { .. });
        let place = talk
            .logins()
            .iter()
            .position(|login| login.id == method)
            .ok_or_else(|| "login method unavailable".to_owned())?;
        self.log_in_agent(id, place);
        if terminal && !self.logins.iter().any(|(held, _, _)| *held == id) {
            return Err("terminal login could not start".to_owned());
        }
        Ok(json!({ "started": true, "terminal": terminal }))
    }

    /// Reads the current screen and scrollback of a conversation's login.
    fn control_read_login(&self, id: u64) -> Result<Value, String> {
        let id = self.control_talk_id(id)?;
        let (_, scope, shell) = self
            .logins
            .iter()
            .find(|(held, _, _)| *held == id)
            .ok_or_else(|| "login terminal unavailable".to_owned())?;
        let shell = self
            .terminals
            .get(*scope, *shell)
            .ok_or_else(|| "login terminal unavailable".to_owned())?;
        let mut shell = shell.borrow_mut();
        let text = shell.text();
        let mut start = text.len().saturating_sub(32_000);
        while !text.is_char_boundary(start) {
            start += 1;
        }
        Ok(json!({ "text": &text[start..], "running": shell.is_running() }))
    }

    /// Writes exact UTF-8 bytes to a conversation's running login terminal.
    fn control_write_login(&mut self, id: u64, input: &str) -> Result<Value, String> {
        let id = self.control_talk_id(id)?;
        let (_, scope, shell) = self
            .logins
            .iter()
            .find(|(held, _, _)| *held == id)
            .ok_or_else(|| "login terminal unavailable".to_owned())?;
        let shell = self
            .terminals
            .get(*scope, *shell)
            .ok_or_else(|| "login terminal unavailable".to_owned())?;
        let mut shell = shell.borrow_mut();
        if !shell.is_running() {
            return Err("login terminal has exited".to_owned());
        }
        shell.send(input.as_bytes());
        Ok(json!({ "written": true }))
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
