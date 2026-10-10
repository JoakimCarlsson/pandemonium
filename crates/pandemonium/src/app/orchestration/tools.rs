//! Project authorization, bounded inspection and prompt delivery.

use super::links::open_link;
use crate::agent::{Block, Standing, Talk};
use crate::app::App;
use crate::orchestration::Call;
use pm_acp::{About, Agent, Setting, Voice};
use pm_core::{ProjectId, Session, SessionId};
use serde_json::{Value, json};
use std::path::Path;

impl App {
    /// Resolves an explicit project and checks a reader-configured grant before access.
    pub(super) fn orchestration_project(&self, call: &Call) -> Result<ProjectId, String> {
        let talk = self
            .agents
            .get(call.caller)
            .filter(|talk| talk.is_running())
            .ok_or("The calling conversation is closed; reconnect its agent pane")?;
        let source = self
            .open
            .get(talk.scope().project())
            .ok_or("The calling project is closed")?;
        let Some(target) = call.arguments.get("project") else {
            return Ok(source.id());
        };
        let path = target
            .as_str()
            .ok_or("project must be an exact project root string")?;
        let target = self
            .open
            .iter()
            .find(|project| project.root().stored() == Path::new(path))
            .ok_or("The target project is not open; use an exact open project root")?;
        if source.id() != target.id()
            && !self
                .preferences
                .orchestration
                .project_grants
                .get(&source.root().stored())
                .is_some_and(|roots| roots.iter().any(|root| *root == target.root().stored()))
        {
            return Err("Cross-project access is unauthorized. The reader must explicitly add this source/target pair to orchestration.project_grants in settings.yaml.".to_owned());
        }
        Ok(target.id())
    }

    /// Returns a project-scoped session without treating a path as implicit authorization.
    fn orchestration_target(&self, call: &Call, project: ProjectId) -> Result<SessionId, String> {
        let identifier = text_argument(&call.arguments, "session", 4096)?;
        let session = self
            .sessions
            .of(project)
            .find(|session| session.root() == Path::new(&identifier))
            .ok_or(
                "The target session is missing from the authorized project; refresh list_sessions",
            )?;
        if self.session_finishing(session.id()) || !session.root().is_dir() {
            return Err(
                "The target worktree is being removed or is missing; refresh list_sessions"
                    .to_owned(),
            );
        }
        Ok(session.id())
    }

    /// Serves read, message and cancellation operations in the authorized project.
    pub(super) fn session_tool(&mut self, call: &Call) -> Result<Value, String> {
        validate_arguments(call)?;
        let project = self.orchestration_project(call)?;
        if call.name == "list_sessions" {
            let sessions = self
                .sessions
                .of(project)
                .take(128)
                .map(|session| self.session_metadata(session))
                .collect::<Vec<_>>();
            let agents = pm_acp::agents()
                .iter()
                .filter(|agent| agent.startable())
                .take(64)
                .map(|agent| {
                    let models = self.advertised_models(*agent);
                    json!({"id": agent.id, "name": agent.name, "models": models})
                })
                .collect::<Vec<_>>();
            return Ok(
                json!({"project": self.open.get(project).map(|project| project.root().stored()), "sessions": sessions,
                "truncated": self.sessions.count(project) > sessions.len(), "agents": agents,
                "limits": {
                    "max_depth": self.preferences.orchestration.max_depth,
                    "max_children": self.preferences.orchestration.max_children,
                    "max_sessions": self.preferences.orchestration.max_sessions,
                    "max_messages": self.preferences.orchestration.max_messages,
                    "max_pending_messages": self.preferences.orchestration.max_pending_messages,
                    "max_context_chars": self.preferences.orchestration.max_context_chars.min(16384)
                }}),
            );
        }
        let session = self.orchestration_target(call, project)?;
        let id = self
            .agents
            .of_session(session)
            .ok_or("The target has no open agent pane; open the session and start an agent")?;
        match call.name.as_str() {
            "get_session_context" => {
                let max = self.preferences.orchestration.max_context_chars.min(16384);
                if max == 0 {
                    return Err(
                        "Context access is disabled by orchestration.max_context_chars".to_owned(),
                    );
                }
                let max = number_argument(&call.arguments, "max_chars", max, max)?;
                let turns = number_argument(&call.arguments, "turns", 5, 20)?;
                let talk = self
                    .agents
                    .get(id)
                    .ok_or("The target conversation disappeared")?;
                let (summary, truncated) = conversation_summary(talk, turns, max);
                Ok(
                    json!({"session": self.sessions.get(session).map(|session| session.root()), "summary": summary, "truncated": truncated, "status": standing(talk.standing())}),
                )
            }
            "send_message" => {
                if id == call.caller {
                    return Err(
                        "A caller cannot message itself; continue its current turn instead"
                            .to_owned(),
                    );
                }
                let message = text_argument(&call.arguments, "message", 8192)?;
                let used = self
                    .orchestration
                    .messages
                    .get(&call.caller)
                    .copied()
                    .unwrap_or_default();
                if used >= self.preferences.orchestration.max_messages {
                    return Err(
                        "The caller's orchestration.max_messages limit was reached".to_owned()
                    );
                }
                let source = self
                    .agents
                    .get(call.caller)
                    .ok_or("The caller disappeared")?;
                let message = format!(
                    "Message from {} in {}:\n{message}",
                    source.agent().name,
                    source.root().display()
                );
                let queued = self
                    .agents
                    .get_mut(id)
                    .ok_or("The target disappeared")?
                    .message(
                        &message,
                        self.preferences.orchestration.max_pending_messages,
                    )?;
                self.orchestration.messages.insert(call.caller, used + 1);
                Ok(
                    json!({"session": self.sessions.get(session).map(|session| session.root()), "delivery": if queued { "queued_after_turn" } else { "accepted" }}),
                )
            }
            "cancel_session" => {
                let caller = self
                    .agents
                    .get(call.caller)
                    .ok_or("The caller disappeared")?;
                let caller_root = caller.root().to_path_buf();
                if !self.is_descendant(session, &caller_root) {
                    return Err(
                        "Cancellation is limited to the caller's delegated descendants".to_owned(),
                    );
                }
                self.agents
                    .get_mut(id)
                    .ok_or("The target disappeared")?
                    .cancel();
                Ok(json!({"cancel_requested": true, "worktree_retained": true}))
            }
            _ => Err("Unknown editor session tool".to_owned()),
        }
    }

    /// Returns compact metadata with stable worktree identity and pane-opening links.
    pub(super) fn session_metadata(&self, session: &Session) -> Value {
        let talk = self
            .agents
            .of_session(session.id())
            .and_then(|id| self.agents.get(id));
        json!({
            "session": session.root(), "name": session.name(),
            "project": self.open.get(session.project()).map(|project| project.root().stored()),
            "open_link": open_link(session.root()), "files": session.root(),
            "files_link": format!("{}?view=files", open_link(session.root())),
            "review_link": format!("{}?view=review", open_link(session.root())),
            "status": talk.map_or("no_agent", |talk| standing(talk.standing())),
            "agent": talk.map(|talk| talk.agent().id),
            "parent": session.delegation(),
            "changes": {"files": session.summary().files, "added": session.summary().added, "removed": session.summary().removed}
        })
    }

    /// Model ids actually advertised by open conversations of this adapter.
    pub(super) fn advertised_models(&self, agent: Agent) -> Vec<String> {
        let mut models = self
            .agents
            .iter()
            .filter(|talk| talk.agent() == agent && talk.is_ready())
            .flat_map(|talk| talk.knobs())
            .filter(|knob| knob.about == About::Model)
            .flat_map(|knob| match knob.setting {
                Setting::Picked { picks, .. } => picks.into_iter().map(|pick| pick.id).collect(),
                Setting::Switched(_) => Vec::new(),
            })
            .collect::<Vec<String>>();
        models.sort();
        models.dedup();
        models.truncate(64);
        models
    }

    /// Whether following durable parent roots reaches the caller without crossing a cycle.
    fn is_descendant(&self, session: SessionId, root: &Path) -> bool {
        let mut current = self.sessions.get(session);
        for _ in 0..64 {
            let Some(parent) = current.and_then(Session::delegation) else {
                return false;
            };
            if parent.parent == root {
                return true;
            }
            current = self
                .open
                .iter()
                .flat_map(|project| self.sessions.of(project.id()))
                .find(|session| session.root() == parent.parent);
        }
        false
    }
}

/// Reads a nonempty bounded string while rejecting incorrect JSON types.
pub(super) fn text_argument(arguments: &Value, name: &str, max: usize) -> Result<String, String> {
    let text = arguments[name]
        .as_str()
        .ok_or_else(|| format!("{name} must be a string"))?
        .trim();
    if text.is_empty() || text.chars().count() > max {
        return Err(format!(
            "{name} must contain between 1 and {max} characters"
        ));
    }
    Ok(text.to_owned())
}

/// Reads an optional positive integer capped by the configured limit.
fn number_argument(
    arguments: &Value,
    name: &str,
    default: usize,
    max: usize,
) -> Result<usize, String> {
    match arguments.get(name) {
        None => Ok(default),
        Some(value) => value
            .as_u64()
            .and_then(|value| usize::try_from(value).ok())
            .filter(|value| *value > 0 && *value <= max)
            .ok_or_else(|| format!("{name} must be an integer between 1 and {max}")),
    }
}

/// Rejects unknown tools and fields before any side effects or target resolution.
pub(super) fn validate_arguments(call: &Call) -> Result<(), String> {
    let fields: &[&str] = match call.name.as_str() {
        "list_sessions" => &["project"],
        "create_session" => &["project", "name", "prompt", "agent", "model"],
        "get_session_context" => &["project", "session", "max_chars", "turns"],
        "send_message" => &["project", "session", "message"],
        "cancel_session" => &["project", "session"],
        _ => return Err("Unknown editor session tool".to_owned()),
    };
    if call
        .arguments
        .as_object()
        .is_none_or(|arguments| arguments.keys().any(|key| !fields.contains(&key.as_str())))
    {
        return Err(
            "Arguments must be an object containing only this tool's advertised fields".to_owned(),
        );
    }
    Ok(())
}

/// The status vocabulary returned to agents by session inspection.
fn standing(value: Standing) -> &'static str {
    match value {
        Standing::Stopped => "stopped",
        Standing::Waiting => "input_needed",
        Standing::Working => "working",
        Standing::Done => "done",
        Standing::Idle => "idle",
    }
}

/// Summarizes recent public turns within a hard Unicode character budget.
fn conversation_summary(talk: &Talk, turns: usize, max: usize) -> (String, bool) {
    let blocks = talk.transcript().blocks();
    let start = blocks
        .iter()
        .enumerate()
        .rev()
        .filter(|(_, block)| matches!(block, Block::Said(Voice::Reader, _)))
        .nth(turns.saturating_sub(1))
        .map_or(0, |(at, _)| at);
    let mut text = String::new();
    let mut remaining = max;
    let mut truncated = start > 0;
    for block in &blocks[start..] {
        let (label, passage) = match block {
            Block::Said(Voice::Reader, text) => ("Reader", text.as_str()),
            Block::Said(Voice::Agent, text) => ("Agent", text.as_str()),
            Block::Ran(call) => ("Tool", call.title.as_str()),
            Block::Note(text) | Block::Failure(text, _) => ("Editor", text.as_str()),
            _ => continue,
        };
        let clipped = passage.chars().take(1024).collect::<String>();
        truncated |= passage.chars().take(1025).count() > 1024;
        let passage = format!("{label}: {clipped}\n");
        let count = passage.chars().count();
        text.extend(passage.chars().take(remaining));
        truncated |= count > remaining;
        remaining = remaining.saturating_sub(count);
        if remaining == 0 {
            truncated = true;
            break;
        }
    }
    (text, truncated)
}
