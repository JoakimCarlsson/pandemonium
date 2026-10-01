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
{"version":1,"id":1,"result":{"protocol":1,"methods":["system.hello","control.execute","state.snapshot","state.watch","agent.transcript"]}}
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
returns its current grammar. It covers project open/select/close, worktree
session create/select/finish, file open/read, agent list/start/show/send/stop,
and ACP permission answers. Commands use the numbered indices shown by
`status` or `state.snapshot`. A session finish command requires the literal
`confirm` argument because it removes a worktree.

The command parser splits words on whitespace. Text arguments such as prompts
are joined with single spaces, so clients needing exact multiline text should
wait for a future typed action method. `control.execute` returns plain text;
it is the compatibility surface for the terminal client.

### `state.snapshot`

No parameters. Returns a JSON object with `revision`, `projects`, `sessions`,
and `agents`:

```json
{
  "revision": 12,
  "projects": [{"index": 0, "name": "pandemonium", "root": "/home/me/pandemonium", "active": true}],
  "sessions": [{"index": 0, "project": 0, "name": "review", "root": "/home/me/.pandemonium/worktrees/review"}],
  "agents": [{"index": 0, "project": 0, "session": 0, "agent": "codex", "title": null, "standing": "working", "transcript_revision": 7, "requests": []}]
}
```

An agent's `session` is `null` when it runs in the project's checkout.
`standing` is one of `stopped`, `waiting`, `working`, `done`, or `idle`.
Each pending permission request has an `index`, `title`, and `choices` array;
each choice has an `index` and `name`. Project, session, agent, request, and
choice indices are positions in the current snapshot, not persistent IDs.
Refresh the snapshot before acting on an index because another window action
may have changed the list. `transcript_revision` changes when an agent's
conversation changes.

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

Parameters: `{"agent":0}`. Returns the current conversation for the agent
at that snapshot index:

```json
{"agent":0,"revision":7,"blocks":[{"kind":"said","voice":"reader","text":"Review the diff"},{"kind":"said","voice":"agent","text":"I will review it."}]}
```

Block kinds are `said` (`voice`, `text`), `picture`, `tool` (`title`, `name`,
`status`), `plan` (`steps`, a count), `note` (`text`), and `failure` (`text`,
`compact`). A `said` block's `voice` is `reader`, `agent`, or `thought`; a
tool's `status` is `pending`, `running`, `done`, or `failed`. The returned
revision matches the agent's
`transcript_revision` in a snapshot if the conversation has not changed
between calls. Images and complete tool outputs are not included in v1.

## Current scope

Version 1 provides structured state and conversation reads, a change wait,
and the terminal command surface for actions. It does not expose every desktop
pane, editor operation, or ACP setting as a typed method. A native phone app
can build project, session, and conversation screens against this contract;
additional typed methods can be added within v1 without changing existing
fields or methods.
