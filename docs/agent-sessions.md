# Sessions agents start

An agent can start other sessions. Ask one session to "implement these three
features as separate sessions" and its agent starts three sessions, each with
its own agent, conversation and worktree. They show in the sidebar like any
other session: you can open one, prompt it and close it. A started session goes
on when the session that started it stops.

## How agents reach it

`pm-server` offers two tools as an MCP server on the loopback address, named
`pandemonium-sessions`, in the same way it offers the browser's tools
(`crates/pm-server/src/mcp_endpoint.rs` serves both). Every agent the server
starts is given the address and a token of its own session, so a call says
which session made it. The server runs the tools itself, so they work while no
window is connected. The harness asks you before each call, as it does for any
tool server, unless the session runs in a mode that does not ask.

## `session_start`

Starts a session with `prompt` as its first prompt and returns once the new
agent has it, not once the work is done. The new session runs the same agent
with the same settings as the session that started it; `model` changes the
model, by the name the agent or you know it by, or by a part only one model's
name has.

`where` says where it works:

| `where` | Folder | Notes |
| --- | --- | --- |
| `worktree` (the default) | a worktree and branch of its own | Made from what the starting session has checked out, its branch or commit, with the changes it has not committed carried over: changes to tracked files, staged or not, become unstaged changes, and files git neither tracks nor ignores are copied. `carry_changes: false` starts from the commit alone. `base_branch` makes it from that branch or commit instead and carries nothing. A project that holds several repositories needs `base_branch`. |
| `local` | the project's own folder | Refused while any session's agent runs in that folder, the starting session's included, since two agents in one folder overwrite each other's work. |
| `chat` | a scratch folder of its own | Belongs to no project, like a chat started from the window. For research and questions that change no code. |

`project` names another of your projects, by its folder or its folder's name;
a session that belongs to no project must name one for `worktree` and `local`.
`container` runs the agent in the project's dev container and follows the
starting session when left out. `submodule` and `title` are as for a session
you start yourself; the title is made from the prompt's first line when left
out, and it names the worktree and its branch.

Two limits keep it from running away: a session an agent started cannot start
sessions itself, and one session can have at most eight open sessions that it
started.

## `session_status`

Tells how sessions stand, by number with `sessions`, or else the open sessions
the calling session started. For each it gives:

- its state: working, waiting for you (its agent asked something), finished,
  stopped, failed or not started;
- where it works: its worktree and branch and what it was made from, the
  project's folder, or its scratch folder;
- its commits since the branch it was made from and its files not committed;
- the newest pull request of its branch, as `gh` lists it, when there is one;
- the last 4000 characters of its agent's final message, once it is not on a
  turn.

A started session does not report back by itself. The starting agent calls
`session_status` when you ask how the sessions are doing; the window tells you
when each one finishes, as for any session.
