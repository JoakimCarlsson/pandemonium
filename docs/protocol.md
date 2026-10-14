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
{ "port": 34407, "token": "94cca7ed…", "pid": 4171030 }
```

A client treats the file as stale when nothing listens on the port.

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
| `prompt`       | `session`, `text`, `attachments`                                                                 | `done`          |
| `choose`       | `session`, `ticket`, `option`                                                                    | `done`          |
| `cancel`       | `session`                                                                                        | `done`          |
| `set`          | `session`, `setting`, `value`                                                                    | `done`          |
| `attach`       | `session`, `after`                                                                               | `done`          |
| `inspect`      | `directory`, `base`                                                                              | `inspected`     |
| `projects`     | none                                                                                             | `projects`      |
| `save_projects`| `paths`                                                                                          | `done`          |
| `choices`      | none                                                                                             | `choices`       |
| `save_choices` | `choices`: `[[agent, setting, value]]`                                                           | `done`          |
| `accounts`     | none                                                                                             | `accounts`      |
| `sessions`     | none                                                                                             | `sessions`      |
| `stage`        | `directory`, `paths`, `stage`                                                                    | `done`          |
| `commit`       | `directory`, `message`                                                                           | `done`          |

`agent` is an agent's id: `claude-code`, `codex` or `grok`.

- **`discover`:** starts the agent briefly and returns what it lets the reader
  change (models, modes, effort) and the commands it offers behind a slash.
  It can take several seconds.
- **`start_session`:** if `base` is a branch name, the server makes a new
  branch and a git worktree off it under `<repository>/.pandemonium/worktrees`
  and the agent works there. Otherwise the agent works in `directory`.
  `settings` is a list of `[setting, value]` pairs applied before the first
  prompt. The response gives the session number and the folder and branch the
  agent works in. The connection is attached to the session already.
- **`prompt`:** hands the agent its next turn. If the agent has stopped (after
  a `failed` event) the server starts it again first. A prompt sent while a
  turn is running is ignored.
- **`choose`:** answers an `ask` event. `ticket` is the ticket of the question
  and `option` is the `id` of one of its choices.
- **`attach`:** subscribes the connection to a session and replays the events
  numbered after `after`. See [Events](#events).
- **`inspect`:** the files that differ from the last commit, with their diffs,
  and, when `base` is given, the messages of the commits the branch has that
  `base` has not.
- **`set_accounts`:** keeps the logins and the login in use for each agent on
  the server's machine, and points the agents at them. `accounts` reads them
  back.
- **`projects`, `choices`:** what the reader set up, kept by the server so that
  every client sees the same. `projects` says `saved: false` when the list was
  never saved, which means the app has not been set up yet. A project is an
  address of a folder on the server's machine, such as `~/code/app`.
- **`sessions`:** every session the server holds, running or not, with enough
  to list it. To open one, send `attach` with `after: 0`: the log replays the
  whole conversation.
- **`stage`:** stages the `paths` (from the repository root) or, when `stage`
  is `false`, unstages them.

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
| `branches`   | `names`: local branches, the current one first            |
| `identity`   | `default`, `selected`: who the agent is signed in as, or null |
| `started`    | `session`, `directory`, `branch` (null without a worktree) |
| `inspected`  | `changes`: `[{path, diff, staged}]`, `commits`: `[string]` or null |
| `projects`   | `paths`, `saved`                                          |
| `choices`    | `choices`: `[[agent, setting, value]]`                    |
| `accounts`   | `logins`: `[{agent, id, name}]`, `in_use`: `[[agent, id]]` |
| `sessions`   | `sessions`: `[SessionInfo]`                               |

## Changes other clients make

Two messages tell a client about changes it did not make itself. Neither is
sent to the client that made the change.

```json
{ "type": "session_added", "info": { "session": 2, "agent": "codex", … } }
{ "type": "projects_changed", "paths": ["~/code/app", "~/code/lib"] }
```

- `session_added`: another client started a session. `info` is a
  `SessionInfo`. The client lists it and sends `attach` with `after: 0` to open
  it.
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

A **SessionInfo** is `{session, agent, repository, directory, branch, base,
title, updated, last_seq, running}`. `updated` is milliseconds since the epoch,
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
| `{"Plan": [PlanStep]}`        | The agent's plan, whole. Replaces the earlier one.                                                        |
| `{"Ask": Permission}`         | The agent waits for an answer: send `choose` with the `ticket` and the `id` of a choice.                  |
| `{"Finished": null}`          | The turn ended. A string instead of null means it failed with that reason.                                |
| `{"Failed": "reason"}`        | The agent could not be started or stopped. The next `prompt` starts it again.                             |
| `{"Prompted": {text, files}}` | A client handed the agent a turn. Sent to every client watching the session, the one that sent it included. `files` are names. |

Shapes used above:

- **Setting:** `{id, category, name, current, picks: [{value, name, description}]}`.
  `category` is `model`, `mode`, `thought_level` or something else. Change one
  with `set` and a `value` from `picks`.
- **Tool:** `{id, title, kind, state, target, diff, output}`, each but `id`
  possibly null. `kind` is `Read`, `Edit`, `Search`, `Execute`, `Fetch` or
  `Other`. `state` is `Running`, `Done` or `Failed`. `diff` is
  `{path, old, new}`.
- **PlanStep:** `{text, progress}` with `progress` one of `Pending`, `Doing`,
  `Done`.
- **Permission:** `{ticket, title, command, choices: [{id, name, verdict}]}`
  with `verdict` one of `Allow`, `Always`, `Reject`.

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
  next `prompt` starts the agent again as a new conversation, without what was
  said before.
- The log of events is never trimmed.
- There are no requests for terminals, file contents, file watching,
  checkpoints or deleting a session, and no message says that a project was
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
