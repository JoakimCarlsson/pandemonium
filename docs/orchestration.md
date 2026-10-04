# Delegated agent sessions

An adapter advertising ACP MCP HTTP support receives the editor's local
`pandemonium-sessions` server. Each conversation has its own bearer credential;
tools resolve the caller's project and session from that credential. An adapter
without HTTP support shows an explicit unsupported state in its transcript and
MCP menu. Configured external MCP servers continue using their existing transport
negotiation.

The server offers `list_sessions`, `create_session`, `get_session_context`,
`send_message` and `cancel_session`. Session identifiers are exact worktree roots
returned by the editor. Agent choices come from the editor's available adapters;
model choices come only from their advertised ACP options. A requested model is
validated again and confirmed by the child before its initial prompt is sent.

Children use the same worktree creation and agent startup seams as reader-started
sessions. Creation returns the identifier and links for the agent, files and
review panes once the adapter is ready. Parent names and depth appear in the
Projects pane. Ancestry is stored beside the child's worktree git metadata and
survives restart, even when the parent has been finished. Opening a reference to
a finished or missing session reports an error.

Context reads return recent public messages and tool titles, excluding hidden
thinking and raw tool inputs/outputs. Responses cap turns and Unicode characters;
long messages are clipped to compact snippets and `truncated` reports omitted
content. Messages to busy agents enter the existing ACP prompt queue, return
`queued_after_turn`, and pass through the normal prompt and filesystem checkpoint
seams after a successful turn. Cancellation or failure discards queued follow-ups.
Restart restores the conversations through existing ACP load/resume support;
pending follow-ups are not persisted or replayed.

`cancel_session` can stop only descendants of the caller. It keeps their files
available for review. Finishing a child remains the existing reader-controlled
session teardown operation. Delegation never authorizes committing, pushing or
landing changes.

Limits are configured in the editor's `settings.yaml` and applied on restart:

```yaml
orchestration:
  max_depth: 3
  max_children: 4
  max_sessions: 16
  max_messages: 50
  max_pending_messages: 8
  max_context_chars: 8192
  project_grants: {}
```

Depth starts at zero for reader-started conversations. Child and window limits
count retained delegated worktrees and pending creations, so a completed child's
files remain counted until reviewed and finished. Message fan-out is bounded per
calling conversation per editor launch. Context cannot exceed 16384 characters or
20 turns even when a larger configuration value is supplied. Zero limits disable
the corresponding operation.

Every tool defaults to the calling project. Cross-project access requires both an
explicit `project` argument naming an open project root and a reader-configured
source/target grant:

```yaml
orchestration:
  project_grants:
    /srv/projects/first:
    - /srv/projects/second
```

These grants authorize editor-serviced operations; they do not sandbox the
adapter's own filesystem access. Failed, cancelled or unsupported child startups
stop the child process and remove its worktree through the existing teardown
seam before returning an error. A cleanup failure retains a browsable session and
reports its path and git error. Startup has a 45-second deadline. The local MCP
transport binds only to loopback, rejects browser origins, requires authentication
and caps request size, queued requests and concurrent connections.
