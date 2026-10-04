//! The bounded editor session tools advertised over MCP.

use serde_json::{Value, json};

/// The editor's session tools and their argument schemas.
pub(super) fn definitions() -> Value {
    let target = json!({"type": "string", "description": "Exact session identifier returned by list_sessions or create_session."});
    let project = json!({"type": "string", "description": "Optional exact project root. Other projects require a reader-configured grant."});
    json!([
        tool(
            "list_sessions",
            "List project-scoped sessions, status, ancestry, open links, and advertised agent/model choices.",
            json!({"project": project}),
            &[],
            true
        ),
        tool(
            "create_session",
            "Delegate a bounded task in a new independent worktree. Returns an openable session after the agent is ready. This grants no permission to commit, push or land work.",
            json!({
                "project": project,
                "name": {"type": "string", "minLength": 1, "maxLength": 100},
                "prompt": {"type": "string", "minLength": 1, "maxLength": 8192},
                "agent": {"type": "string", "description": "An advertised agent id; defaults to the caller's agent."},
                "model": {"type": "string", "description": "An advertised model id for that agent; unsupported selections fail."}
            }),
            &["name", "prompt"],
            false
        ),
        tool(
            "get_session_context",
            "Read a bounded summary of recent public messages. Hidden thinking and raw tool outputs are excluded.",
            json!({
                "session": target, "project": project,
                "max_chars": {"type": "integer", "minimum": 1, "maximum": 16384},
                "turns": {"type": "integer", "minimum": 1, "maximum": 20}
            }),
            &["session"],
            true
        ),
        tool(
            "send_message",
            "Send a bounded follow-up. Busy targets queue it after their current turn; no mid-turn steering is promised.",
            json!({
                "session": target, "project": project,
                "message": {"type": "string", "minLength": 1, "maxLength": 8192}
            }),
            &["session", "message"],
            false
        ),
        tool(
            "cancel_session",
            "Cancel a delegated descendant's active turn and pending messages, retaining its worktree for review.",
            json!({"session": target, "project": project}),
            &["session"],
            false
        )
    ])
}

/// One tool definition with a closed argument object.
fn tool(
    name: &str,
    description: &str,
    properties: Value,
    required: &[&str],
    read_only: bool,
) -> Value {
    json!({
        "name": name, "description": description,
        "inputSchema": {"type": "object", "properties": properties, "required": required, "additionalProperties": false},
        "annotations": {"readOnlyHint": read_only, "destructiveHint": false, "openWorldHint": false}
    })
}
