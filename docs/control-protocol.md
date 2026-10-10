# Editor control protocol v1

This is the protocol for controlling a running Pandemonium desktop editor from
another process. It is separate from the `pandemonium-server --stdio` protocol
used to read a project on a remote development host.

## Transport and access

The editor listens on a Unix domain socket at
`$PANDEMONIUM_HOME/control/editor.sock`, or
`~/.pandemonium/control/editor.sock` when `PANDEMONIUM_HOME` is unset. The
directory is private to its owner and the socket has mode `0600`. The editor
does not listen on a network port. A client must run as the same OS user as the
editor. For a phone app, SSH into the desktop machine as that user and run
`pandemonium control --stdio`; this command relays protocol lines between its
standard input/output and the socket. Use a second SSH exec channel when one
channel is waiting for a state change.

Each request and response is one UTF-8 JSON object followed by a newline.
The socket accepts multiple requests on one connection and replies in request
order. Requests are limited to 65,536 bytes including the newline. Responses
may be larger, particularly file contents or conversation transcripts. The
SSH standard-IO relay accepts multiple JSON lines until standard input closes.

For example:

```sh
printf '%s\n' '{"version":1,"id":1,"method":"state.snapshot","params":{}}' \
  | ssh my-desktop 'pandemonium control --stdio'
```

## Envelope and compatibility

Every request has `version: 1`, a client-chosen unsigned integer `id`, a
`method` string, and a `params` object. `params` may be omitted when the
method takes no arguments. The response echoes the `id` and contains exactly
one of `result` or `error`:

```json
{"version":1,"id":1,"result":{"protocol":1,"methods":["system.hello","control.execute","state.snapshot","state.watch","agent.transcript","agent.catalog","agent.start","agent.detail","agent.send","agent.image.upload","agent.cancel","agent.answer","agent.mode.set","agent.knob.set","agent.history.list","agent.history.load","agent.login","agent.login.read","agent.login.write","agent.terminal"]}}
```

```json
{"version":1,"id":1,"error":{"code":"invalid_params","message":"agent must be an unsigned integer"}}
```

The response `id` is `null` when the request cannot be decoded. A version
other than `1` receives `unsupported_version`; clients should call
`system.hello` with version `1` before using other methods. Unknown request
fields are ignored, and future v1 responses may add fields. Clients should
ignore fields they do not recognize. A change that alters existing field
meanings requires a new protocol version. The wire version is independent of
the Pandemonium release version.

Errors use these codes: `invalid_request` for malformed or oversized input,
`unsupported_version`, `unknown_method`, `invalid_params`, `operation_failed`
for a rejected editor action, and `unavailable` if the editor stops answering.
The `message` is for display; clients should branch on `code`.

## Methods

### `system.hello`

No parameters. Returns the protocol version and supported method names.

### `control.execute`

Parameters: `{"line":"status"}`. Returns `{"text":"..."}`. This is the
same command surface used by `pandemonium control` in a terminal. `help`
returns its current grammar. It covers local and SSH project open/select/reconnect/close, worktree
session create/select/finish, file open/read, agent list/start/show/send/stop,
and ACP permission answers. Commands use the numbered indices shown by
`status` or `state.snapshot`. A session finish command requires the literal
`confirm` argument because it removes a worktree.

The command parser splits words on whitespace. Text arguments such as prompts
are joined with single spaces. Native clients should use the typed `agent.*`
methods for ACP controls, including exact multiline prompts. `control.execute`
is the compatibility surface for the terminal client.

### `state.snapshot`

No parameters. Returns a JSON object with `revision`, `projects`, `sessions`,
and `agents`:

```json
{
  "revision": 12,
  "projects": [{"index": 0, "id": 0, "name": "pandemonium", "root": "/home/me/pandemonium", "active": true}],
  "sessions": [{"index": 0, "id": 0, "project": 0, "project_id": 0, "name": "review", "root": "/home/me/.pandemonium/worktrees/review"}],
  "agents": [{"index": 0, "id": 0, "project": 0, "project_id": 0, "session": null, "session_id": null, "agent": "codex", "title": null, "standing": "working", "transcript_revision": 7, "requests": []}]
}
```

Each project also reports `host` (the SSH alias, or `null` for local),
`connected`, and `connecting`. A root path belongs to that host.

An agent's `session` is `null` when it runs in the project's checkout.
`standing` is one of `stopped`, `waiting`, `working`, `done`, or `idle`.
Each pending permission request has an `index`, numeric `id`, `title`, and
`choices` array; each choice has an `index`, string `id`, and `name`. Indices
are positions that can change when the window changes. IDs remain stable for
the life of this editor process. Refresh the snapshot after reconnecting to a
restarted editor. Typed ACP methods take IDs; terminal commands still use
indices. `transcript_revision` changes when an agent's conversation changes.

### `state.watch`

Parameters: `{"after_revision":12,"timeout_ms":30000}`. Waits until the
editor state revision exceeds `after_revision`, then returns the same object
as `state.snapshot`. If no change arrives before the timeout, returns
`{"changed":false,"revision":12}`. `timeout_ms` defaults to 30,000 and is
capped at 30,000. An `after_revision` ahead of the editor is rejected. After
each reply, repeat `state.watch` using the returned revision. Editor commands
and background events, including ACP updates, advance the revision; a changed
revision does not necessarily mean every array changed.

### `agent.transcript`

Parameters: `{"id":0}` for a stable conversation ID, or `{"agent":0}` for
the old snapshot index. Returns the current conversation:

```json
{"agent":0,"id":0,"revision":7,"blocks":[{"kind":"said","voice":"reader","text":"Review the diff"},{"kind":"said","voice":"agent","text":"I will review it."}]}
```

Block kinds are `said` (`voice`, `text`), `picture`, `tool` (its identity,
kind, status, argument, return value, locations, and output), `plan` (a list
of steps), `note` (`text`), and `failure` (`text`, `compact`). Tool output is
`said`, `changed` (path and before/after text), or `terminal` (an ID to use
with `agent.terminal`). A `said` block's `voice` is `reader`, `agent`, or
`thought`; a tool or plan step's `status` is `pending`, `running`, `done`, or
`failed`. The returned revision matches the agent's `transcript_revision` in
a snapshot if the conversation has not changed between calls. Picture bytes
are not included in v1.

## Typed ACP methods

An **agent conversation** can run in a project's checkout without a worktree
session. `agent.start` creates the conversation and returns its stable `id`.
Set `session_id` to `null` or omit it to chat in the checkout:

```json
{"version":1,"id":2,"method":"agent.start","params":{"project_id":0,"session_id":null,"agent":"codex"}}
```

The other ACP methods use that conversation ID. They return the following
result fields; failures use the standard error envelope.

| Method | Parameters | Result |
| --- | --- | --- |
| `agent.catalog` | none | `agents`: offered IDs, names, install and start status |
| `agent.start` | `project_id`, optional `session_id`, `agent` ID | `id`, `project_id`, `session_id` |
| `agent.detail` | `id` | readiness, standing, pending permission requests, modes, knobs, login methods, history, usage |
| `agent.send` | `id`, exact `text`, optional `files` paths and uploaded `images` IDs | `sent`, `id` |
| `agent.image.upload` | `id`, client-chosen numeric `image`, `mime_type`, optional `name`, base64 chunk `data`, boolean `finish` | `image`, `complete`, decoded `bytes` received |
| `agent.cancel` | `id` | `cancelled` |
| `agent.answer` | `id`, numeric `request_id`, string `choice_id` or `null` to refuse | `answered` |
| `agent.mode.set` | `id`, offered `mode` ID | `selected` |
| `agent.knob.set` | `id`, offered `knob` ID, `value` (pick ID string or switch boolean) | `set` |
| `agent.history.list` | `id` | `listing`; watch state, then read `agent.detail` for entries |
| `agent.history.load` | `id`, offered saved conversation `saved` ID | conversation `id` |
| `agent.login` | `id`, offered login `method` ID | `started`, `terminal` boolean |
| `agent.login.read` | `id` | login terminal `text` and `running` boolean |
| `agent.login.write` | `id`, exact UTF-8 `input` | `written` |
| `agent.terminal` | `id`, ACP tool `terminal` ID | `tail` text |

`agent.send` preserves text, including line breaks, and leaves any desktop
prompt draft alone. Its `files` are paths relative to the conversation's
worktree that already exist on the desktop machine; paths outside that
worktree are rejected. An empty `text` is allowed when files are present.
An empty `text` is also allowed when uploaded images are present. Images require
`can_image: true` in `agent.detail`; the agent receives them as ACP image
content blocks. Image IDs are scoped to one conversation and consumed by a
successful `agent.send`.
After sending, use `state.watch`, then `agent.transcript` when its revision
changes. `agent.answer` uses the request and choice IDs from `agent.detail`;
omitting `choice_id` or passing `null` refuses the request.

Upload each image in base64 chunks of at most 60,000 characters. Each chunk is
encoded separately; repeat `id`, `image`, `mime_type`, and `name` on every
request. Set `finish: true` on the last chunk. Supported MIME types are
`image/png`, `image/jpeg`, `image/gif`, and `image/webp`; the bytes must match.
An image may contain at most 12 MiB of decoded data. At most four pending
images per conversation and 48 MiB across the editor are retained. For example:

```json
{"version":1,"id":3,"method":"agent.image.upload","params":{"id":0,"image":1,"mime_type":"image/png","name":"photo.png","data":"<base64 chunk>","finish":true}}
{"version":1,"id":4,"method":"agent.send","params":{"id":0,"text":"What is in this picture?","images":[1]}}
```

`agent.detail` reports each knob as either `{"kind":"picked","value":"...",
"picks":[...]}` or `{"kind":"switched","value":true}`. Mode and knob
changes follow the desktop's saved agent preferences. Saved conversation
listing is asynchronous. For a login method marked `terminal` in `agent.detail`,
call `agent.login`, then `agent.login.read` to display its terminal output and
`agent.login.write` to send input. Input is written as exact UTF-8 bytes; append
`\r` to press Enter. Repeat `agent.login.read` after `state.watch` reports a
change. The read result contains the last 32,000 bytes of terminal screen and
scrollback text. Once the login process exits, the editor restarts the agent
after a successful exit or records a failure in its transcript. Terminal login
may also require opening a URL or confirming a browser flow outside the app.
The mobile client should avoid keeping login input in its own logs.

## Current scope

Version 1 provides typed ACP conversation controls, structured state and
transcripts, a change wait, and the terminal command surface for project and
worktree actions. It does not expose every desktop pane or editor operation.
Native clients can build a mobile conversation UI without creating worktree
sessions, including image prompts and interactive terminal login.
