//! Terminal commands applied to the same project, session, and ACP seams as the window.

use std::path::{Component, Path, PathBuf};

use pm_acp::Output;
use pm_core::{Project, ProjectId, Scope, Session, SessionId};
use serde_json::{Value, json};

use crate::agent::{Block, Talk, TalkId};
use crate::app::App;
use crate::control::Operation;
use crate::message::Message;

impl App {
    /// Runs every command received from the owner-only control socket.
    pub(super) fn take_control(&mut self) {
        let requests = self
            .control
            .as_ref()
            .map_or_else(Vec::new, |server| server.take());
        for request in requests {
            let result = match request.operation {
                Operation::Execute(line) => {
                    let result = self.control_command(&line);
                    if result.is_ok()
                        && let Some(control) = &self.control
                    {
                        control.changed();
                    }
                    result.map(|text| json!({ "text": text }))
                }
                Operation::Snapshot => Ok(self.control_snapshot()),
                Operation::Transcript(index) => self.control_transcript_json(index),
                Operation::TranscriptId(id) => {
                    let index = self
                        .agents
                        .iter()
                        .position(|talk| talk.id().number() == id)
                        .ok_or_else(|| "agent unavailable".to_owned());
                    index.and_then(|index| self.control_transcript_json(index))
                }
                Operation::Agent(operation) => {
                    let result = self.control_agent_operation(operation);
                    if result.is_ok()
                        && let Some(control) = &self.control
                    {
                        control.changed();
                    }
                    result
                }
            };
            let _ = request.answer.send(result);
        }
        self.request_redraw();
    }

    /// Returns the editor state as stable JSON fields for a mobile client.
    fn control_snapshot(&self) -> Value {
        let projects = self
            .open
            .iter()
            .enumerate()
            .map(|(index, project)| {
                json!({
                    "index": index,
                    "id": project.id().number(),
                    "name": project.name(),
                    "root": project.root().display().to_string(),
                    "active": self.open.active().is_some_and(|active| active.id() == project.id()),
                })
            })
            .collect::<Vec<_>>();
        let sessions = self
            .sessions
            .iter()
            .enumerate()
            .map(|(index, session)| {
                json!({
                    "index": index,
                    "id": session.id().number(),
                    "project_id": session.project().number(),
                    "project": self.control_project_index(session.project()),
                    "name": session.name(),
                    "root": session.root().display().to_string(),
                })
            })
            .collect::<Vec<_>>();
        let agents = self
            .agents
            .iter()
            .enumerate()
            .map(|(index, talk)| {
                json!({
                    "index": index,
                    "id": talk.id().number(),
                    "project_id": talk.scope().project().number(),
                    "session_id": talk.scope().session().map(pm_core::SessionId::number),
                    "project": self.control_project_index(talk.scope().project()),
                    "session": talk.scope().session().and_then(|id| self.control_session_index(id)),
                    "agent": talk.agent().id,
                    "title": talk.title(),
                    "standing": format!("{:?}", talk.standing()).to_lowercase(),
                    "transcript_revision": talk.transcript().revision(),
                    "requests": talk.asks().iter().enumerate().map(|(ask, request)| json!({
                        "index": ask,
                        "id": request.id,
                        "title": request.tool.title,
                        "choices": request.choices.iter().enumerate().map(|(choice, answer)| json!({
                            "index": choice,
                            "id": answer.id,
                            "name": answer.name,
                        })).collect::<Vec<_>>(),
                    })).collect::<Vec<_>>(),
                })
            })
            .collect::<Vec<_>>();
        json!({
            "revision": self.control.as_ref().map_or(0, |server| server.revision()),
            "projects": projects,
            "sessions": sessions,
            "agents": agents,
        })
    }

    /// Returns one ACP conversation in machine-readable blocks.
    fn control_transcript_json(&self, index: usize) -> Result<Value, String> {
        let talk = self
            .agents
            .iter()
            .nth(index)
            .ok_or_else(|| "agent unavailable".to_owned())?;
        let blocks = talk.transcript().blocks().iter().map(|block| match block {
            Block::Said(voice, text) => json!({ "kind": "said", "voice": format!("{voice:?}").to_lowercase(), "text": text }),
            Block::Picture(_) => json!({ "kind": "picture" }),
            Block::Ran(call) => json!({
                "kind": "tool", "id": call.id, "title": call.title,
                "name": call.name, "tool_kind": format!("{:?}", call.kind).to_lowercase(),
                "status": format!("{:?}", call.status).to_lowercase(),
                "argument": call.argument, "returned": call.returned,
                "locations": call.locations.iter().map(|location| json!({
                    "path": location.path.display().to_string(), "line": location.line,
                })).collect::<Vec<_>>(),
                "output": call.output.iter().map(|output| match output {
                    Output::Said(text) => json!({ "kind": "said", "text": text }),
                    Output::Changed { path, before, after } => json!({
                        "kind": "changed", "path": path.display().to_string(),
                        "before": before, "after": after,
                    }),
                    Output::Terminal(id) => json!({ "kind": "terminal", "id": id }),
                }).collect::<Vec<_>>(),
            }),
            Block::Planned(steps) => json!({
                "kind": "plan",
                "steps": steps.iter().map(|step| json!({
                    "text": step.text, "status": format!("{:?}", step.status).to_lowercase(),
                })).collect::<Vec<_>>(),
            }),
            Block::Note(text) => json!({ "kind": "note", "text": text }),
            Block::Failure(text, compact) => json!({ "kind": "failure", "text": text, "compact": compact }),
        }).collect::<Vec<_>>();
        Ok(
            json!({ "agent": index, "id": talk.id().number(), "revision": talk.transcript().revision(), "blocks": blocks }),
        )
    }

    /// Routes a terminal command through the window's existing state seams.
    fn control_command(&mut self, line: &str) -> Result<String, String> {
        let mut words = line.split_whitespace();
        let command = words.next().unwrap_or("help");
        match command {
            "help" => Ok("status | project open PATH | project use N | project close N | session new PROJECT NAME | session use N | session checkout PROJECT | session finish N confirm | file read PROJECT SESSION|checkout PATH | file open PROJECT SESSION|checkout PATH | agent list | agent start PROJECT SESSION|checkout AGENT | agent show N | agent send N PROMPT | agent stop N | agent answer N ASK CHOICE".to_owned()),
            "status" => Ok(self.control_status()),
            "project" => self.control_project(words.collect()),
            "session" => self.control_session(words.collect()),
            "file" => self.control_file(words.collect()),
            "agent" => self.control_agent(words.collect()),
            _ => Err("unknown command; type help".to_owned()),
        }
    }

    /// Reads or opens a file in the named project worktree.
    fn control_file(&mut self, words: Vec<&str>) -> Result<String, String> {
        let [action, project, session, path @ ..] = words.as_slice() else {
            return Err("usage: file read|open PROJECT SESSION|checkout PATH".to_owned());
        };
        if path.is_empty() || !matches!(*action, "read" | "open") {
            return Err("usage: file read|open PROJECT SESSION|checkout PATH".to_owned());
        }
        let project = self.control_project_id(project)?;
        let scope = if *session == "checkout" {
            Scope::checkout(project)
        } else {
            let session = self.control_session_id(session)?;
            if self
                .sessions
                .get(session)
                .is_none_or(|held| held.project() != project)
            {
                return Err("session belongs to another project".to_owned());
            }
            Scope::of(project, session)
        };
        let relative = Path::new(path[0]);
        let relative = if path.len() == 1 {
            relative.to_path_buf()
        } else {
            PathBuf::from(path.join(" "))
        };
        if relative.as_os_str().is_empty()
            || !relative
                .components()
                .all(|part| matches!(part, Component::Normal(_) | Component::CurDir))
        {
            return Err("path must be relative to the worktree".to_owned());
        }
        let root = self
            .root_of(scope)
            .ok_or_else(|| "worktree unavailable".to_owned())?;
        let path = root.path.join(relative);
        let canonical_root = root
            .host
            .fs()
            .canonicalize(&root.path)
            .map_err(|error| error.to_string())?;
        let path = root
            .host
            .fs()
            .canonicalize(path)
            .map_err(|error| error.to_string())?;
        if !path.starts_with(canonical_root) {
            return Err("file is outside the worktree".to_owned());
        }
        let file = self
            .editor
            .open(scope, &root, &path, false)
            .ok_or_else(|| "file unavailable".to_owned())?;
        if *action == "read" {
            let opened = self
                .editor
                .get(file)
                .ok_or_else(|| "file unavailable".to_owned())?;
            Ok(opened.borrow().buffer().contents())
        } else {
            self.point_at(scope);
            self.show_file(self.panes.focus(), file, false);
            Ok("File opened".to_owned())
        }
    }

    /// Lists the open projects, their worktrees, and ACP conversations.
    fn control_status(&self) -> String {
        let mut rows = vec!["Projects:".to_owned()];
        for (index, project) in self.open.iter().enumerate() {
            let selected = if self
                .open
                .active()
                .is_some_and(|active| active.id() == project.id())
            {
                "*"
            } else {
                " "
            };
            rows.push(format!(
                "  {index}{selected} {}  {}",
                project.name(),
                project.root().display()
            ));
        }
        rows.push("Sessions:".to_owned());
        for (index, session) in self.sessions.iter().enumerate() {
            let project = self
                .control_project_index(session.project())
                .unwrap_or_default();
            rows.push(format!(
                "  {index}  project {project}  {}  {}",
                session.name(),
                session.root().display()
            ));
        }
        rows.push("Agents:".to_owned());
        for (index, talk) in self.agents.iter().enumerate() {
            let scope = talk.scope();
            let project = self
                .control_project_index(scope.project())
                .unwrap_or_default();
            let session = scope
                .session()
                .and_then(|id| self.control_session_index(id))
                .map_or("checkout".to_owned(), |index| index.to_string());
            rows.push(format!(
                "  {index}  project {project}  session {session}  {}  {:?}",
                talk.agent().id,
                talk.standing()
            ));
        }
        rows.join("\n")
    }

    /// Applies a project command from the terminal.
    fn control_project(&mut self, words: Vec<&str>) -> Result<String, String> {
        match words.as_slice() {
            ["open", path @ ..] if !path.is_empty() => {
                let path = PathBuf::from(path.join(" "));
                let id = self
                    .open
                    .find_or_open(path)
                    .map_err(|error| error.to_string())?;
                self.read_new_worktrees();
                self.store();
                Ok(format!(
                    "Opened project {}",
                    self.control_project_index(id).unwrap_or_default()
                ))
            }
            ["use", index] => {
                let id = self.control_project_id(index)?;
                self.point_at(Scope::checkout(id));
                Ok("Project selected".to_owned())
            }
            ["close", index] => {
                let id = self.control_project_id(index)?;
                self.apply(Message::CloseProject(id));
                Ok("Project closed".to_owned())
            }
            _ => Err("usage: project open PATH | project use N | project close N".to_owned()),
        }
    }

    /// Applies a worktree session command from the terminal.
    fn control_session(&mut self, words: Vec<&str>) -> Result<String, String> {
        match words.as_slice() {
            ["new", project, name @ ..] if !name.is_empty() => {
                let id = self.control_project_id(project)?;
                let opened = self.open.get(id).ok_or_else(|| "project unavailable".to_owned())?;
                if !opened.root().host.is_local() {
                    return Err("sessions on SSH projects are not available yet".to_owned());
                }
                if opened.repositories().is_empty() {
                    return Err("project has no repository to cut a session from".to_owned());
                }
                let roots = opened.repositories().iter().map(|repository| repository.root().to_path_buf()).collect();
                self.open.activate(id);
                self.session_base = None;
                self.session_name = name.join(" ");
                self.session_picks = roots;
                self.cut_session();
                Ok("Session creation started".to_owned())
            }
            ["use", index] => {
                let id = self.control_session_id(index)?;
                self.select_session(id);
                Ok("Session selected".to_owned())
            }
            ["checkout", project] => {
                let id = self.control_project_id(project)?;
                self.point_at(Scope::checkout(id));
                Ok("Checkout selected".to_owned())
            }
            ["finish", index, "confirm"] => {
                let id = self.control_session_id(index)?;
                self.end_session(id);
                Ok("Session removal started".to_owned())
            }
            ["finish", _, ..] => Err("finishing removes the worktree; use session finish N confirm".to_owned()),
            _ => Err("usage: session new PROJECT NAME | session use N | session checkout PROJECT | session finish N confirm".to_owned()),
        }
    }

    /// Applies an ACP conversation command from the terminal.
    fn control_agent(&mut self, words: Vec<&str>) -> Result<String, String> {
        match words.as_slice() {
            ["list"] => Ok(pm_acp::agents()
                .iter()
                .map(|agent| format!("{}  {}", agent.id, agent.name))
                .collect::<Vec<_>>()
                .join("\n")),
            ["start", project, session, agent] => {
                let project = self.control_project_id(project)?;
                if self.open.get(project).is_some_and(|held| !held.root().host.is_local()) {
                    return Err("agents on SSH projects are not available yet".to_owned());
                }
                let session = if *session == "checkout" { None } else { Some(self.control_session_id(session)?) };
                if session.is_some_and(|id| self.sessions.get(id).is_none_or(|held| held.project() != project)) {
                    return Err("session belongs to another project".to_owned());
                }
                let agent = pm_acp::Agent::named(agent).ok_or_else(|| "unknown agent".to_owned())?;
                let root = match session {
                    Some(id) => self.sessions.get(id).map(Session::root).map(PathBuf::from),
                    None => self.open.get(project).map(Project::root).map(|root| root.path.clone()),
                }.ok_or_else(|| "worktree unavailable".to_owned())?;
                let before = self.agents.iter().count();
                self.open_agent(project, session, &root, agent);
                if self.agents.iter().count() == before { Err("agent could not start".to_owned()) } else { Ok("Agent started".to_owned()) }
            }
            ["show", index] => {
                let id = self.control_agent_id(index)?;
                let talk = self.agents.get(id).ok_or_else(|| "agent unavailable".to_owned())?;
                Ok(Self::control_transcript(talk))
            }
            ["send", index, prompt @ ..] if !prompt.is_empty() => {
                let id = self.control_agent_id(index)?;
                let talk = self.agents.get_mut(id).ok_or_else(|| "agent unavailable".to_owned())?;
                if !talk.is_ready() || talk.is_busy() { return Err("agent is not ready for a prompt".to_owned()); }
                let scope = talk.scope();
                talk.send_text(&prompt.join(" "));
                self.checks.reset(scope);
                self.follow_agents();
                Ok("Prompt sent".to_owned())
            }
            ["stop", index] => {
                let id = self.control_agent_id(index)?;
                self.apply(Message::StopAgentTurn(id));
                Ok("Stop requested".to_owned())
            }
            ["answer", index, ask, choice] => {
                let id = self.control_agent_id(index)?;
                let ask = ask.parse::<usize>().map_err(|_| "invalid request index".to_owned())?;
                let choice = choice.parse::<usize>().map_err(|_| "invalid choice index".to_owned())?;
                let talk = self.agents.get(id).ok_or_else(|| "agent unavailable".to_owned())?;
                let request = talk.asks().get(ask).ok_or_else(|| "request unavailable".to_owned())?;
                if choice >= request.choices.len() { return Err("choice unavailable".to_owned()); }
                self.apply(Message::AnswerAgent(id, request.id, choice));
                Ok("Answer sent".to_owned())
            }
            _ => Err("usage: agent list | agent start PROJECT SESSION|checkout AGENT | agent show N | agent send N PROMPT | agent stop N | agent answer N ASK CHOICE".to_owned()),
        }
    }

    /// Formats one ACP conversation and its pending permission requests.
    fn control_transcript(talk: &Talk) -> String {
        let mut lines = Vec::new();
        for block in talk.transcript().blocks() {
            match block {
                Block::Said(voice, text) => lines.push(format!("{voice:?}: {text}")),
                Block::Picture(_) => lines.push("[picture]".to_owned()),
                Block::Ran(call) => lines.push(format!(
                    "Tool {:?}: {} ({:?})",
                    call.name, call.title, call.status
                )),
                Block::Planned(steps) => lines.push(format!("Plan: {} steps", steps.len())),
                Block::Note(text) => lines.push(format!("Note: {text}")),
                Block::Failure(text, _) => lines.push(format!("Failure: {text}")),
            }
        }
        for (index, request) in talk.asks().iter().enumerate() {
            lines.push(format!("Request {index}: {}", request.tool.title));
            for (choice, answer) in request.choices.iter().enumerate() {
                lines.push(format!("  {choice}: {}", answer.name));
            }
        }
        if lines.is_empty() {
            "Conversation is empty".to_owned()
        } else {
            lines.join("\n\n")
        }
    }

    /// Resolves the current project list index to an identity.
    fn control_project_id(&self, index: &str) -> Result<ProjectId, String> {
        let index = index
            .parse::<usize>()
            .map_err(|_| "invalid project index".to_owned())?;
        self.open
            .iter()
            .nth(index)
            .map(Project::id)
            .ok_or_else(|| "project unavailable".to_owned())
    }

    /// Resolves the current worktree list index to an identity.
    fn control_session_id(&self, index: &str) -> Result<SessionId, String> {
        let index = index
            .parse::<usize>()
            .map_err(|_| "invalid session index".to_owned())?;
        self.sessions
            .iter()
            .nth(index)
            .map(Session::id)
            .ok_or_else(|| "session unavailable".to_owned())
    }

    /// Resolves the current ACP conversation list index to an identity.
    fn control_agent_id(&self, index: &str) -> Result<TalkId, String> {
        let index = index
            .parse::<usize>()
            .map_err(|_| "invalid agent index".to_owned())?;
        self.agents
            .iter()
            .nth(index)
            .map(Talk::id)
            .ok_or_else(|| "agent unavailable".to_owned())
    }

    /// Finds an open project's index for status display.
    fn control_project_index(&self, id: ProjectId) -> Option<usize> {
        self.open.iter().position(|project| project.id() == id)
    }

    /// Finds an open worktree's index for status display.
    fn control_session_index(&self, id: SessionId) -> Option<usize> {
        self.sessions.iter().position(|session| session.id() == id)
    }
}
