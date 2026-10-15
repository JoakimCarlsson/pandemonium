# The server protocol

How a client talks to `pandemonium-server`. The window uses it, and so will any
other client, such as a phone app. The types are in `crates/pm-protocol`; this
document says what they mean and how a client behaves.

- **Version:** `1` (`pm_protocol::VERSION`).
- **Transport:** a WebSocket on a TCP connection. Every message is one text
  frame holding one JSON object. Binary frames are not used.
- **Direction:** the client sends `ClientMessage`s, the server sends
  `ServerMessage`s.

## Connecting

A client reaches the server one of three ways. The protocol on the connection
is the same in each.

| Way    | What the client does                                                                                                                                                                                         |
| ------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Local  | Reads `server.json` from the data folder for the port and token, connects to `ws://127.0.0.1:<port>/`. If nothing answers, starts `pandemonium-server --listen 127.0.0.1:0` apart from itself and reads again. |
| Direct | Connects to `ws://<host>:<port>/` with a token it was given. The server must be started with `--listen <address> --token <token>`. The connection is not encrypted: use a trusted network such as Tailscale.    |
| ssh    | Reads the remote `server.json` with `ssh <host> cat …`, opens a local port, and for each connection runs `ssh -W 127.0.0.1:<port> <host>` and pipes the bytes through. The server must already be running.      |

A relay for phones, end-to-end encrypted, is planned and is not built.

The WebSocket path is not checked; use `/`.

### `server.json`

Written by the server when it starts, in the app's data folder (on Linux
`~/.local/share/pandemonium/server.json`), readable by its owner only:

```json
{
  "port": 34407,
  "token": "94cca7ed…",
  "pid": 4171030,
  "build": { "program": "/usr/bin/pandemonium", "modified": 1791968806502 }
}
```

`build` is the program file the server runs and when that file was last
written, in milliseconds since the epoch. A client treats the file as stale
when nothing listens on the port. A local client stops the server (sending
`pid` a termination signal) and starts its own when `build` is missing, or names
the client's own program file with another `modified`: the program was rebuilt
since the server started.

## Handshake

The first message of every connection is `hello`, sent within 5 seconds.

```json
{ "type": "hello", "version": 1, "token": "94cca7ed…" }
```

The server answers with one of:

```json
{ "type": "ready", "version": 1, "server": "0.12.0" }
{ "type": "denied", "reason": "the token is wrong" }
```

`denied` is sent for a protocol version that differs and for a wrong or missing
token, and the server then closes the connection. The server sends nothing
else, and answers no request, before `ready`.

## Requests and responses

A request carries an `id` chosen by the client. The server answers every
request with exactly one `response` carrying the same `id`.

```json
{ "type": "request", "id": 7, "request": { "request": "attach", "session": 1, "after": 4 } }
{ "type": "response", "id": 7, "response": { "response": "done" } }
```

- Requests are carried out concurrently, so responses can arrive in any order.
- A client should not reuse an `id` that has no response yet. The window uses
  `id` 0 for the `attach` it sends on its own, and ignores the response.
- Any request can be answered `failed` instead of its normal response:
  `{ "response": "failed", "message": "…" }`.
- A path that starts with `~/` is taken as the home folder of the user the
  server runs as.

### Requests

| `request`      | Fields                                                                                           | Normal response |
| -------------- | ------------------------------------------------------------------------------------------------ | --------------- |
| `discover`     | `agent`                                                                                          | `discovery`     |
| `skills`       | `agent`, `directory`                                                                             | `skills`        |
| `files`        | `directory`, `limit`                                                                             | `files`         |
| `branches`     | `directory`                                                                                      | `branches`      |
| `identity`     | `agent`                                                                                          | `identity`      |
| `set_accounts` | `logins`: `[{agent, id, name}]`, `in_use`: `[[agent, id]]`                                       | `done`          |
| `sign_in`      | `agent`                                                                                          | `done`          |
| `start_session`| `agent`, `repository`, `directory`, `title`, `base`, `settings`, `text`, `attachments`           | `started`       |
| `prompt`       | `session`, `text`, `attachments`, `now` (optional, `false` by default)                           | `done`          |
| `hurry`        | `session`, `queued`                                                                              | `done`          |
| `rewind`       | `session`, `turn`                                                                                | `done`          |
| `restore`      | `session`, `turn`                                                                                | `done`          |
| `fork_session` | `session`, `turn`                                                                                | `sessions`      |
| `choose`       | `session`, `ticket`, `option`                                                                    | `done`          |
| `answer`       | `session`, `ticket`, `replies`                                                                   | `done`          |
| `cancel`       | `session`                                                                                        | `done`          |
| `close_session`| `session`                                                                                        | `done`          |
| `set`          | `session`, `setting`, `value`                                                                    | `done`          |
| `attach`       | `session`, `after`                                                                               | `done`          |
| `inspect`      | `directory`, `base`                                                                              | `inspected`     |
| `projects`     | none                                                                                             | `projects`      |
| `save_projects`| `paths`                                                                                          | `done`          |
| `choices`      | none                                                                                             | `choices`       |
| `save_choices` | `choices`: `[[agent, setting, value]]`                                                           | `done`          |
| `agent_servers`| none                                                                                             | `agent_servers` |
| `save_agent_servers`| `servers`: `[{id, name, command, env}]`, `env`: `[[name, value]]`                                | `done`          |
| `mcp_servers`  | none                                                                                             | `mcp_servers`   |
| `save_mcp_servers`| `servers`: `[{name, target, env, description, website, enabled}]`, `env`: `[[name, value]]`                                     | `done`          |
| `registry_agents`| none                                                                                             | `registry_agents`|
| `search_tools` | `query`                                                                                          | `tool_listings` |
| `accounts`     | none                                                                                             | `accounts`      |
| `sessions`     | none                                                                                             | `sessions`      |
| `stage`        | `directory`, `paths`, `stage`                                                                    | `done`          |
| `commit`       | `directory`, `message`, `tracked` (optional), `amend` (optional)                                 | `done`          |
| `log`          | `directory`, `limit`, `all`                                                                      | `log`           |
| `sync`         | `directory`, `operation`                                                                         | `done`          |
| `cherry_pick`  | `directory`, `commit`                                                                            | `done`          |
| `stashes`      | `directory`                                                                                      | `stashes`       |
| `stash`        | `directory`, `action`, `index`, `message`                                                        | `done`          |
| `step`         | `directory`, `action`, `message` (optional)                                                      | `done`          |
| `discard`      | `directory`, `paths`                                                                             | `done`          |
| `ignore`       | `directory`, `paths`                                                                             | `done`          |
| `checkout`     | `directory`, `branch`                                                                            | `done`          |
| `create_branch`| `directory`, `name`                                                                              | `done`          |
| `read_file`    | `directory`, `path`, `head`                                                                      | `file`          |
| `open_terminal`| `directory`, `columns`, `rows`                                                                   | `terminal`      |
| `terminal_input`| `terminal`, `data` (base64)                                                                     | `done`          |
| `resize_terminal`| `terminal`, `columns`, `rows`                                                                  | `done`          |
| `close_terminal`| `terminal`                                                                                      | `done`          |
| `attach_terminal`| `terminal`, `after`                                                                            | `done`          |
| `terminals`    | none                                                                                             | `terminals`     |
| `owners`       | none                                                                                             | `owners`        |
| `repositories` | `owner`, `organization`                                                                          | `repositories`  |
| `folders`      | `path`                                                                                           | `folders`       |
| `clone`        | `url`, `under` (optional)                                                                        | `directory`     |

`agent` is an agent's id: `claude-code`, `codex` or `grok`.

- **`discover`:** starts the agent briefly and returns what it lets the reader
  change (models, modes, effort) and the commands it offers behind a slash.
  It can take several seconds.
- **`start_session`:** if `base` is given, the server looks it up in the
  repository at that moment, fails with `<base> does not exist` when it is
  gone, and otherwise makes a git worktree under
  `<repository>/.pandemonium/worktrees` with that commit checked out on a
  detached head, so no branch is made. The agent works there. Without `base`
  the agent works in `directory` itself.
  `settings` is a list of `[setting, value]` pairs applied before the first
  prompt. The response gives the session number, the folder the agent works in
  and, for a worktree, the abbreviated commit it has checked out (`head`). The connection is attached to the session already.
- **`close_session`:** stops the session's agent, drops its queued prompts and marks it `closed`. The
  folder and the log are kept, so the session still shows in `sessions` (with
  `closed: true`) and can be opened with `attach`. A later `prompt` opens it
  again. Other clients are told with `session_closed`.
- **`prompt`:** hands the agent its next turn. If the agent has stopped (after
  a `failed` event) the server starts it again first. A prompt sent while a
  turn is running waits in the session's queue, which the server keeps, and is
  announced with a `Queued` event. When the turn ends, the prompt that has
  waited longest is handed over: an `Unqueued` event, then `Prompted`. With
  `now: true` the prompt goes to the front of the queue and the running turn is
  stopped, so it is handed over as soon as the agent ends the turn.
- **`hurry`:** moves the queued prompt whose `Queued` event gave it the id
  `queued` to the front of the queue and stops the running turn, as `now`
  does for a new prompt. Fails when no prompt waits under that id.
- **`rewind`:** takes the conversation back to before the reader's turn
  `turn`, counted from 0 among the `Prompted` events a rewind has not
  dropped. The turn and everything after it are dropped, announced with a
  `Rewound` event, and the agent stops. The next `prompt` starts it again as a
  new conversation and hands it what came before the turn, as text ahead of
  the prompt, with the settings it last reported. Files are left as they are.
  Fails while a turn is running.
- **`restore`:** puts the files of the session's folder back as they were
  before the reader's turn `turn`, counted as for `rewind`, and announces it
  with a `Restored` event. Before every turn in a git repository the server
  keeps the files, tracked or not but not ignored, under
  `refs/worktree/pandemonium/sessions/<session>/turns/<seq>`, where `seq` is
  the number of the turn's `Prompted` event; the files as they were before a
  restore are kept under `…/sessions/<session>/before-restore`. Neither the
  branch, the head nor the index changes. The agent's next prompt says that
  the files were put back. Fails while a turn is running, or when no files
  were kept before the turn.
- **`fork_session`:** starts a session that branches off the conversation
  of `session` after the reader's turn `turn`, counted as for `rewind`, and
  the agent's reply to it. The new session works in the same folder, so the
  files are shared, and is titled after the source with ` (branch)`. Its log
  starts with the source's latest `Settings` and `Commands` and the events of
  those turns, numbered from 1, with no files kept before them. Its agent is
  not started until the first `prompt`, which hands it the conversation so far
  as text ahead of the prompt. The response is `sessions` holding the new
  session; the connection is attached to it, and other clients are told with
  `session_added`. Fails while the agent is still writing that reply.
- **`cancel`:** stops the running turn and drops the prompts queued behind it,
  each with an `Unqueued` event. A question the agent was waiting on is
  declined.
- **`choose`:** answers an `ask` event. `ticket` is the ticket of the question
  and `option` is the `id` of one of its choices.
- **`answer`:** answers an `Inquire` event. `ticket` is the ticket of the form
  and `replies` holds one `{picks, other}` for each question, in order: `picks`
  are the `label`s of the alternatives chosen and `other` is what the reader
  typed, both possibly empty to skip the question. A null `replies` abandons the
  form, which aborts the agent's use of the tool.
- **`attach`:** subscribes the connection to a session and replays the events
  numbered after `after`. See [Events](#events).
- **`inspect`:** the files that differ from the last commit, with their diffs,
  and, when `base` is given, the messages of the commits the branch has that
  `base` has not, plus the branch and abbreviated head commit the folder is on.
- **`set_accounts`:** keeps the logins and the login in use for each agent on
  the server's machine, and points the agents at them. `accounts` reads them
  back.
- **`projects`, `choices`:** what the reader set up, kept by the server so that
  every client sees the same. `projects` says `saved: false` when the list was
  never saved, which means the app has not been set up yet. A project is an
  address of a folder on the server's machine, such as `~/code/app`.
- **`agent_servers`, `mcp_servers`:** the agents the reader added by hand and
  the tool servers every agent is started with, kept by the server and replaced
  whole by the matching `save_` request. A tool server's `target` is an address
  when it begins with `http://` or `https://`, and `env` is then the headers it
  is sent; otherwise it is the command that starts it, and `env` its environment.
  Every new session is started with the tool servers saved at that moment that are `enabled`; `description` and `website` are optional and `enabled` is true when missing.
- **`registry_agents`, `search_tools`:** what the public registries of agents and
  of tool servers offer, read by the server so that a client needs no access to
  the internet. Only agents that are a package a runner fetches are listed.
  `search_tools` with a blank `query` lists the well known servers; `asks` says
  that installing a listing has to ask for something first.
- **`sessions`:** every session the server holds, running or not, with enough
  to list it. To open one, send `attach` with `after: 0`: the log replays the
  whole conversation.
- **`open_terminal`:** starts the user's shell (`$SHELL`, with `TERM=xterm-256color`)
  in a pseudo-terminal in `directory`, with a window of `columns` by `rows`. The
  connection is attached to the shell already. The shell keeps running when no
  client is connected, until it exits or is closed. See [Terminals](#terminals).
- **`owners`:** the GitHub account `gh` is signed in to on the server's
  machine, then the organizations it belongs to, as `{login, organization}`.
  Fails with what `gh` complained of when it is not installed or signed in.
- **`repositories`:** the repositories of `owner` the account can reach, by
  name, each with the address to clone it from (over ssh when `gh` is set to
  use ssh).
- **`folders`:** the folders inside `path` on the server's machine, for choosing
  one: a leading `~` is the home folder, and an empty `path` is the home folder
  itself. The response gives the folder as an absolute path and the names of
  the folders inside it, hidden ones left out. Fails when `path` is not a
  folder that can be read.
- **`clone`:** fetches the repository at `url` into a folder named after it, in
  `under` (a leading `~` is the home folder; made when it is not there) or in
  the first of `~/Documents/repos`, `~/repos`, `~/code` and `~/projects` that
  exists, or `~/projects`. The response names the folder. Fails when
  that folder is taken.
- **`stage`:** stages the `paths` (from the repository root) or, when `stage`
  is `false`, unstages them.
- **`commit`:** commits what is staged under `message`. With nothing staged and
  `tracked` set, it takes the changes of every tracked file instead. Fails on an
  empty message.
- **`log`:** the most recent `limit` commits, newest first, as `{id, refs,
  summary, lanes}`. `refs` are the branches and tags pointing at the commit as
  git decorates them (`HEAD -> main`, `tag: v1`). `lanes` lays the commit out in
  a graph: its `lane` and `color`, the `edges` crossing its row (`half`
  `Upper` or `Lower`, `from` and `to` lanes, `color`) and the `width` in lanes.
  Without `all` it lists what the checked out branch reaches; with it, every
  reference.
- **`sync`:** `operation` is `fetch` (all remotes, pruning), `pull`,
  `pull_rebase`, `push` (publishing the branch when it follows nothing),
  `force_push` (`--force-with-lease`) or `sync` (pull, then push).
- **`cherry_pick`:** applies `commit` to the checked out branch.
- **`stashes`:** the stashes, newest first, as `{index, message, when}`.
- **`stash`:** `action` is `push` (untracked files included, named `message`
  when given), or `apply`, `pop` or `drop` for the stash at `index`.
- **`step`:** takes the next step of the merge, rebase or cherry-pick the folder
  is stopped in: `action` is `continue` (a merge or cherry-pick finishes under
  `message` when given), `skip` or `abort`. Fails when nothing is stopped.
- **`commit` with `amend`:** rewrites the last commit in place of making another.
- **`discard`:** puts the `paths` back as they are in the last commit, in the
  index and in the files; a path that is not in the last commit is deleted.
- **`ignore`:** adds each of the `paths` to the `.gitignore` at the root of the
  repository, anchored with a leading slash, unless it is in it already.
- **`checkout`:** checks out the local branch `branch`; a remote branch such as
  `origin/main` gets a local branch that follows it. Fails when git refuses,
  for instance while the branch is checked out in another worktree.
- **`create_branch`:** makes the branch `name` at the commit the folder is at
  and checks it out.
- **`read_file`:** reads `path` from the working folder or, when `head`, from
  the last commit. The response holds no text for a file that is not text.

An **attachment** is `{ "name": "a.txt", "data": "<base64>" }`. The file is
sent whole, so the server needs no access to the client's disk. The server
keeps it under its data folder. A picture goes to the agent as bytes when the
agent takes images, a small text file as its contents when the agent takes
embedded context, and anything else as a link to the stored file.

### Responses

| `response`   | Fields                                                    |
| ------------ | --------------------------------------------------------- |
| `done`       | none                                                      |
| `failed`     | `message`                                                 |
| `discovery`  | `settings`, `commands`                                    |
| `skills`     | `skills` (`[{name, description}]`), `names` (`[string]`)  |
| `files`      | `paths`: repository-relative, tracked or not ignored      |
| `branches`   | `names`: local branches, the current one first, then remote ones such as `origin/main`; read from git at each request |
| `stashes`    | `stashes`: `[{index, message, when}]`, newest first |
| `log`        | `commits`: `[{id, refs, summary, lanes}]`, newest first |
| `file`       | `text`: string or null for a file that is not text, `truncated`: whether it was cut short |
| `identity`   | `default`, `selected`: who the agent is signed in as, or null |
| `started`    | `session`, `directory`, `branch` (always null for now), `head` (null without a worktree) |
| `inspected`  | `changes`: `[{path, diff, staged, untracked, index, worktree, conflicted, from}]`, `commits`: `[string]` or null, `branch`: string or null, `head`: string or null, `tracking`: `{upstream, ahead, behind}`, `operation`: `Merge`, `Rebase` or `CherryPick` with what git prepared, or null |
| `projects`   | `paths`, `saved`                                          |
| `choices`    | `choices`: `[[agent, setting, value]]`                    |
| `agent_servers` | `servers`: `[{id, name, command, env}]`                |
| `mcp_servers` | `servers`: `[{name, target, env, description, website, enabled}]`                     |
| `registry_agents` | `agents`: `[{id, name, version, description, website, command, env}]` |
| `tool_listings` | `listings`: `[{id, title, description, name, target, env, inputs, asks, website}]` |
| `accounts`   | `logins`: `[{agent, id, name}]`, `in_use`: `[[agent, id]]` |
| `sessions`   | `sessions`: `[SessionInfo]`                               |
| `terminal`   | `terminal`: the number of the shell that was started      |
| `terminals`  | `terminals`: `[{terminal, directory, columns, rows}]`, oldest first; `directory` is the address `open_terminal` was given |
| `owners`     | `owners`: `[{login, organization}]`                       |
| `repositories` | `repositories`: `[{name, description, private, url}]`   |
| `folders`    | `path`: absolute, `names`: the folders inside it           |
| `directory`  | `path`: a `clone` made   |

## Changes other clients make

Two messages tell a client about changes it did not make itself. Neither is
sent to the client that made the change.

```json
{ "type": "session_added", "info": { "session": 2, "agent": "codex", … } }
{ "type": "session_closed", "session": 2 }
{ "type": "projects_changed", "paths": ["~/code/app", "~/code/lib"] }
```

- `session_added`: another client started or branched a session. `info` is a
  `SessionInfo`. The client lists it and sends `attach` with `after: 0` to open
  it.
- `session_closed`: another client closed a session. The client takes it out of
  what is going on.
- `projects_changed`: another client saved the projects. The client adds the
  ones it does not have. Removing a project is not sent yet.

A client that was disconnected misses these, so after every connect it asks
`sessions` and `projects` again.

## Events

What an agent does arrives as an `event`, not as a response:

```json
{ "type": "event", "session": 1, "seq": 3, "event": { "Said": "Hello" } }
```

- `session` is the number `started` gave.
- `seq` counts the events of one session from 1, with no gaps.
- The server keeps every event of a session in memory and on disk (under its
  data folder, in `sessions/<number>/`), so sessions and their logs survive a
  restart of the server. After `start_session`
  the connection receives that session's events. A client that connects later,
  or reconnects, sends `attach` with the `seq` of the last event it has, and the
  server sends the ones after it, then keeps sending new ones. `attach` with
  `after: 0` replays the whole session.
- Events can arrive before the `started` response of the request that made the
  session. A client should hold them until it knows the session.
- A client drops an event whose `seq` it already has, so replays are safe.

A **SessionInfo** is `{session, agent, repository, directory, branch, head,
base, title, closed, updated, last_seq, running}`. `updated` is milliseconds since the epoch,
`last_seq` is the number of the session's latest event, and `running` says
whether its agent is running now.

### Event types

Events use serde's default form, one key naming the type.

| Event                         | Meaning                                                                                                   |
| ----------------------------- | --------------------------------------------------------------------------------------------------------- |
| `{"Settings": [Setting]}`     | What can be changed and how it stands now. Replaces the earlier list.                                     |
| `{"Commands": [Slash]}`       | The commands offered behind a slash. Replaces the earlier list.                                           |
| `{"Said": "text"}`            | More of the agent's reply. Append it to the reply so far.                                                 |
| `{"Thought": "text"}`         | More of the agent's reasoning. Append it.                                                                 |
| `{"Tool": Tool}`              | A tool the agent is using. Later events with the same `id` update it.                                     |
| `{"Background": {id, label, running}}` | A command or task went on running beyond its call (`running` true), or ended (false). `id` is the handle the agent knows it by; for a command left running by a turn it is the `id` of its `Tool`. The agent is still at work while any runs, though the turn is over. |
| `{"Halted": null}`            | The turn was stopped or failed: nothing the agent had running goes on. A `Finished` follows.              |
| `{"Plan": [PlanStep]}`        | The agent's plan, whole. Replaces the earlier one.                                                        |
| `{"Ask": Permission}`         | The agent waits for an answer: send `choose` with the `ticket` and the `id` of a choice.                  |
| `{"Inquire": Inquiry}`      | The agent waits for answers to a form of questions: send `answer` with the `ticket` and a reply for each question. |
| `{"Answered": [[question, answer]]}` | A client answered a form of questions. `answer` is null for a question that was skipped. |
| `{"Finished": null}`          | The turn ended. A string instead of null means it failed with that reason.                                |
| `{"Failed": "reason"}`        | The agent could not be started or stopped. The next `prompt` starts it again.                             |
| `{"Titled": "text"}`          | The agent named the session, or named it again. The server keeps it as the session's `title`.            |
| `{"Prompted": {text, files, checkpoint}}` | A client handed the agent a turn. Sent to every client watching the session, the one that sent it included. `files` are names. `checkpoint` (absent means `false`) says the files were kept before the turn, so `restore` can put them back. |
| `{"Queued": {id, text, files}}` | A client wrote while a turn was running, so the prompt waits under `id`, counted per session. `files` are names. |
| `{"Unqueued": id}`            | The prompt queued under `id` stopped waiting: it is handed over (a `Prompted` event follows) or dropped.  |
| `{"Rewound": {turn}}`         | The conversation went back to before the reader's turn `turn`: drop that turn and everything after it.   |
| `{"Restored": {turn, files}}` | The files were put back as they were before the reader's turn `turn`. `files` are the paths that changed, from the root of the repository. |

Shapes used above:

- **Setting:** `{id, category, name, current, picks: [{value, name, description}]}`.
  `category` is `model`, `mode`, `thought_level` or something else. Change one
  with `set` and a `value` from `picks`.
- **Tool:** `{id, title, kind, state, target, diff, output, parent, agent}`,
  each but `id` possibly null. `agent`, when set, says the call starts a
  subagent of that kind; `parent` is the `id` of the subagent call whose work
  this is. Both may be absent in older logs. `kind` is `Read`, `Edit`, `Search`, `Execute`, `Fetch` or
  `Other`. `state` is `Running`, `Done` or `Failed`. `diff` is
  `{path, old, new}`.
- **PlanStep:** `{text, progress}` with `progress` one of `Pending`, `Doing`,
  `Done`.
- **Permission:** `{ticket, title, command, plan?, choices: [{id, name, verdict}]}`
  with `verdict` one of `Allow`, `Always`, `Reject`.

- **Inquiry:** `{ticket, questions: [{header, text, multiple, alternatives: [{label, description}]}]}`.
  Send the answers to the questions with `answer`.

## Terminals

A shell is a process in a pseudo-terminal that the server runs. The server
does not interpret what it prints: it passes the bytes on, and each client
keeps its own emulator for them.

```json
{ "type": "terminal_output", "terminal": 1, "offset": 0, "data": "<base64>" }
{ "type": "terminal_exited", "terminal": 1 }
```

- `terminal_output`: bytes the shell printed. `offset` is how many bytes it had
  printed before these, so a client drops what it already has and notices a gap.
  A connection gets output from `open_terminal`, and from `attach_terminal`
  until it ends. Output is not sent to a connection that has not attached.
- `attach_terminal` sends what the shell printed after the byte `after`. The
  server keeps the latest 256 KiB; a client that is further behind gets the
  oldest it still has, from its `offset`. A client attaches again after every
  reconnect with the number of bytes it has, and after a restart of the window
  with `after: 0` for each shell `terminals` lists.
- `terminal_exited` is sent to every client when the shell's process ends, and
  the shell is forgotten.
- `terminal_input` carries what a terminal sends the shell for a key or a paste
  (`"\r"` for Enter, `"\u001b[A"` for the up arrow), and the answers an
  emulator gives to the shell's queries.
- A shell started in a session's folder belongs to that session in the window.

## Compatibility

- The protocol is append-only. Add fields (optional on read) and new variants.
  Never remove, rename or repurpose one.
- A client ignores a message or an event variant it does not know. A server
  that gets a request it does not know closes nothing and sends no response
  for it, so a client should time out a request.
- A change that older clients cannot live with raises `VERSION`. A server and
  a client with different versions refuse each other at the handshake.

## Not in the protocol yet

- After a restart the server holds the sessions but not their agents. The
  next `prompt` starts the agent again as a new conversation and hands it what
  was said before as text ahead of the prompt. Queued prompts are not kept: the server drops them when it
  starts again, with an `Unqueued` event for each.
- The log of events is never trimmed.
- Shells do not survive a restart of the server.
- There are no requests for file contents, file watching,
  listing checkpoints or deleting a session, and no message says that a project was
  removed or a session renamed or deleted.
- A client that reconnects to a server restarted on another port has to read
  `server.json` again itself.
- The relay and TLS are not built. Direct connections send the token in the
  clear.

## What a phone client needs

1. A WebSocket client and JSON.
2. The `hello` handshake and the token, from a pairing link or a setting.
3. A table of pending request ids, and one place that takes in `response`s.
4. A store of events per session with the last `seq`, and `attach` with that
   `seq` after every reconnect. At first connect, `projects` and `sessions`
   say what there is, and `attach` with `after: 0` opens a session.
5. Drawing of the event types above.
6. Reaching the server: direct (Tailscale), an ssh tunnel through an ssh
   library, or the relay once it exists.
