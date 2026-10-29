# The server protocol

How a client talks to `pandemonium-server`. The window uses it, and so will any
other client, such as a phone app. The types are in `crates/pm-protocol`; this
document says what they mean and how a client behaves.

- **Version:** `1` (`pm_protocol::VERSION`).
- **Transport:** a WebSocket on a TCP connection. Every message is one text
  message holding one JSON object. Server messages larger than 16 MiB use
  continuation frames, each carrying at most 16 MiB. Clients reassemble them
  before decoding JSON, with the existing 64 MiB message limit. Binary frames
  are not used.
- **Direction:** the client sends `ClientMessage`s, the server sends
  `ServerMessage`s.

## Connecting

A client reaches the server one of six ways. The protocol on the connection
is the same in each: only the way to the server's port differs.

| Way    | What the client does                                                                                                                                                                                         |
| ------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Local  | Reads `server.json` from the data folder for the port and token, connects to `ws://127.0.0.1:<port>/`. If nothing answers, starts `pandemonium-server --listen 127.0.0.1:0` apart from itself and reads again. |
| Direct | Connects to `ws://<host>:<port>/` with a token it was given. The server must be started with `--listen <address> --token <token>`. The connection is not encrypted: use a trusted network such as Tailscale.    |
| ssh    | Reads the remote `server.json` with `ssh <host> cat …`, opens a local port, and for each connection runs `ssh -W 127.0.0.1:<port> <host>` and pipes the bytes through. The server must already be running.      |
| WSL    | On Windows, runs `wsl.exe -d <distro> --exec sh -c …` to read the distribution's `server.json`. When no server answers there, installs `pandemonium-server` with the installer if it is missing, starts it as a `wsl.exe` process of its own with `--listen 127.0.0.1:0`, so that it and the distribution outlive the client, and connects to `ws://127.0.0.1:<port>/` through WSL's localhost forwarding. |
| Dev container | Reads the folder's `devcontainer.json` (`image`, `build`, `workspaceFolder`, `containerEnv`, `mounts`, `runArgs`, `postCreateCommand`), builds or pulls the image, and runs one container per folder, found again by its `pandemonium.folder` label, with the folder mounted at the workspace folder and port 6868 published on `127.0.0.1`. Installs and starts the server in it with `docker exec -d … --listen 0.0.0.0:6868`, reads its `server.json` with `docker exec`, and connects to the published port. Progress is reported step by step while this happens. |
| Session container | A session started with `container` runs its agent and terminals through `docker exec` in the container of its worktree, made as above. The `devcontainer.json` is refused unless `runArgs` holds only `--init`, `--hostname` and `--env`, the workspace folder is an absolute path without `,`, `=`, quotes or control characters, a Dockerfile build reads only paths inside the worktree, and every entry of `mounts` is a volume, a tmpfs or a bind of a path inside the worktree after links are resolved, with no key besides `type`, `source`, `src`, `target`, `dst`, `destination` and `readonly`. The container is not a security boundary. |
| Tunnel | Opens a local port and, for each connection, runs `cloudflared access tcp --hostname <hostname>` and pipes the bytes through it. The server's machine runs a named tunnel that points at the port the server listens on, so it only connects outward and opens no port. The token is the one the server was started with, kept by the client. |

A project on another machine is addressed `ssh://<destination>/<folder>` for
a server reached over ssh, `tcp://<host>:<port>/<folder>` for one reached
directly, `wsl://<distro>/<folder>` for one in a WSL distribution,
`devcontainer://<host folder>/<folder>` for one in the dev container of a folder
on this machine, and `tunnel://<hostname>/<folder>` for one behind a tunnel. The
host folder of a dev container is written with `/` as `%2F`, `%` as `%25` and
whitespace as `%XX`, so that it holds no slash; `<folder>` is the folder inside
the container. The window keeps the token of each direct or tunnelled server in
its own settings.

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
| `files`        | `directory`, `limit`, `expanded` (optional)                                                      | `files`         |
| `branches`     | `directory`                                                                                      | `branches`      |
| `submodules`   | `directory`                                                                                      | `submodules`    |
| `dev_container` | `directory`                                                                                    | `dev_container` |
| `submodule_heads` | `directory`                                                                                   | `submodule_heads` |
| `identity`     | `agent`, `login` (optional): a login id whose folder answers `selected`, instead of the one in use | `identity`      |
| `set_accounts` | `logins`: `[{agent, id, name}]`, `in_use`: `[[agent, id]]`                                       | `done`          |
| `set_session_account` | `session`, `login` (a login id, or `null` for the agent's own)                            | `done`          |
| `sign_in`      | `agent`, `login` (optional): a login id, signed in under its own folder and never the agent's default one | `done`          |
| `sign_in_session` | `session` | `done` |
| `start_session`| `agent`, `repository`, `directory`, `title`, `base`, `settings`, `text`, `attachments`, `submodule`, `container` | `started`       |
| `prompt`       | `session`, `text`, `attachments`, `now` (optional, `false` by default)                           | `done`          |
| `remove_queued` | `session`, `queued`                                                                            | `done`          |
| `hurry`        | `session`, `queued`                                                                              | `done`          |
| `rewind`       | `session`, `turn`                                                                                | `done`          |
| `restore`      | `session`, `turn`                                                                                | `done`          |
| `fork_session` | `session`, `turn`                                                                                | `sessions`      |
| `choose`       | `session`, `ticket`, `option`                                                                    | `done`          |
| `answer`       | `session`, `ticket`, `replies`                                                                   | `done`          |
| `trust_hooks`  | `session`, `hooks`: `[Hook]`                                                                     | `done`          |
| `cancel`       | `session`                                                                                        | `done`          |
| `close_session`| `session`, `discard`?                                                                           | `done`, or `uncommitted` |
| `rename_session`| `session`, `title`                                                                              | `done`          |
| `set`          | `session`, `setting`, `value`                                                                    | `done`          |
| `attach`       | `session`, `after`                                                                               | `done`          |
| `inspect`      | `directory`, `base`                                                                              | `inspected`     |
| `standing`     | `directory`                                                                                      | `standing`      |
| `diffs`        | `directory`, `paths`                                                                             | `diffs`         |
| `projects`     | none                                                                                             | `projects`      |
| `save_projects`| `paths`                                                                                          | `done`          |
| `choices`      | none                                                                                             | `choices`       |
| `save_choices` | `choices`: `[[agent, setting, value]]`                                                           | `done`          |
| `agent_servers`| none                                                                                             | `agent_servers` |
| `save_agent_servers`| `servers`: `[{id, name, command, env, package, installer, home}]`, `env`: `[[name, value]]`                                | `done`          |
| `mcp_servers`  | none                                                                                             | `mcp_servers`   |
| `save_mcp_servers`| `servers`: `[{name, target, env, description, website, enabled, extension}]`, `env`: `[[name, value]]`                                     | `done`          |
| `library`      | `agent`, `directory`                                                                             | `library`       |
| `tool_sets`    | `agent`                                                                                          | `tool_sets`     |
| `save_withheld`| `keys`: `[string]`                                                                               | `done`          |
| `watch_folders`| `directories`: `[string]`                                                                       | `done`          |
| `machine_settings`| none                                                                                          | `machine_settings` |
| `save_machine_settings`| `settings`: `{git, history, agents, shell, languages, workspace, extensions, servers}`              | `done`          |
| `latest_release`| `channel`: `stable` or `preview`                                                                 | `latest_release` |
| `download_update`| `channel`, `version`                                                                            | `update_staged` |
| `cancel_update`| none                                                                                              | `done`          |
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
| `create_entry` | `directory`, `path`, `folder`                                                                    | `done`          |
| `rename_entry` | `directory`, `from`, `to`                                                                        | `done`, or `edited` |
| `copy_entry`   | `directory`, `from`, `into`                                                                      | `done`          |
| `remove_entry` | `directory`, `path`                                                                              | `done`          |
| `write_file`   | `directory`, `path`, `text`                                                                      | `done`          |
| `read_file`    | `directory`, `path`, `head`, `revision` (optional)                                               | `file`          |
| `read_bytes`   | `directory`, `path`, `head`                                                                      | `bytes`         |
| `fetch_bytes`  | `url`: an `http` or `https` address                                                              | `bytes`         |
| `open_terminal`| `directory`, `columns`, `rows`                                                                   | `terminal`      |
| `terminal_input`| `terminal`, `data` (base64)                                                                     | `done`          |
| `resize_terminal`| `terminal`, `columns`, `rows`                                                                  | `done`          |
| `close_terminal`| `terminal`                                                                                      | `done`          |
| `attach_terminal`| `terminal`, `after`                                                                            | `done`          |
| `terminals`    | none                                                                                             | `terminals`     |
| `tasks`        | `directory`                                                                                      | `tasks`         |
| `run_task`     | `directory`, `project` (optional), `task`: a [task](#tasks), `columns`, `rows`                   | `terminal`      |
| `runnable_tasks` | `directory`, `project` (optional), `path`, `tags`, `variables`, `position` (optional)       | `runnable_tasks` |
| `run_runnable` | `directory`, `project` (optional), `path`, `kind` and `name` (optional, for clients that predate tags), `tags`, `variables`, `position` (optional), `task` (optional), `columns`, `rows` | `terminal`      |
| `toolchains`   | `directory`, `project` (optional)                                                                | `toolchains`    |
| `select_toolchain`| `project`, `language`, `toolchain` (optional)                                                 | `done`          |
| `debug_scenarios`| `directory`, `file` (optional)                                                                 | `debug_scenarios` |
| `debug_processes`| none                                                                                           | `debug_processes` |
| `debug_start`  | `directory`, `file` (optional), `scenario`, `files`: `[{path, breakpoints}]`, `project` (optional) | `debug`      |
| `debug_attach` | `directory`, `adapter`, `pid`, `files`                                                           | `debug`         |
| `debug_do`     | `debug`, `action`: `{action, ...}`                                                               | `done`          |
| `debug_watch`  | `debug`                                                                                          | `debug`         |
| `debugs`       | none                                                                                             | `debugs`        |
| `owners`       | none                                                                                             | `owners`        |
| `repositories` | `owner`, `organization`                                                                          | `repositories`  |
| `folders`      | `path`                                                                                           | `folders`       |
| `clone`        | `url`, `under` (optional)                                                                        | `directory`     |
| `extensions`   | none                                                                                             | `extensions`    |
| `install_extensions` | `ids`                                                                                       | `done`          |
| `remove_extension` | `id`                                                                                          | `done`          |
| `install_extension` | `path`, `grants` (optional)                                                                 | `done`          |
| `inspect_extension` | `path`                                                                                      | `extension_preview` |
| `grant_extension` | `id`, `capability`, `granted`                                                                 | `done`          |
| `extension_assets` | none                                                                                         | `extension_assets` |
| `run_extension_command` | `extension`, `command`, `argument` (optional), `directory` (optional)                   | `extension_command` |
| `grammar`      | `language`                                                                                       | `grammar`       |
| `open_document`| `directory`, `path`, `language`, `version`, `text`                                               | `opened`        |
| `change_document` | `directory`, `path`, `version`, `changes`: `[{span, text}]`                                   | `done`          |
| `close_document` | `directory`, `path`                                                                            | `done`          |
| `completion`   | `directory`, `path`, `version`, `position`, `trigger` (optional), `scope` (optional)             | `completions`   |
| `resolve_completion` | `directory`, `path`, `item`: a completion's `resolve`                                      | `completion_resolved` |
| `hover`        | `directory`, `path`, `version`, `position`                                                       | `hovered`       |
| `format`       | `directory`, `path`, `version`, `range` (optional)                                               | `formatted`     |
| `format_on_type` | `directory`, `path`, `version`, `position`, `character`                                        | `formatted`     |
| `folding_ranges` | `directory`, `path`, `version`                                                                 | `folding_ranges` |
| `linked_ranges` | `directory`, `path`, `version`, `position`                                                      | `linked_ranges` |
| `semantic_tokens` | `directory`, `path`, `version`, `rules`, `range` (optional)                                          | `semantic_tokens` |
| `inlay_hints`  | `directory`, `path`, `version`, `span`                                                           | `inlay_hints`   |
| `document_links` | `directory`, `path`, `version`                                                                 | `document_links` |
| `colors`       | `directory`, `path`, `version`                                                                   | `colors`        |
| `resolve_link` | `directory`, `path`, `link`: a link's `unresolved` target                                       | `link_resolved` |
| `color_presentations` | `directory`, `path`, `version`, `span`, `color`: `[red, green, blue, alpha]`, each 0 to 255 | `color_presentations` |
| `inline_values` | `directory`, `path`, `version`, `span`, `frame`, `stopped`                                      | `inline_values` |
| `code_lenses`  | `directory`, `path`, `version`                                                                   | `code_lenses`   |
| `press_code_lens` | `directory`, `path`, `index`                                                                  | `lens_pressed`  |
| `definition`   | `directory`, `path`, `version`, `position`, `seek` (optional)                                    | `definitions`   |
| `workspace_symbols` | `directory`, `path`, `query`                                                                | `definitions`   |
| `search_files` | `directory`, `query`, `regex` (optional), `case` (optional)                                      | `definitions`   |
| `problems`     | `directory`                                                                                      | `problems`      |
| `blame`        | `directory`, `path`, `line`                                                                      | `blamed`        |
| `highlights`   | `directory`, `path`, `version`, `position`                                                       | `highlights`    |
| `signature`    | `directory`, `path`, `version`, `position`                                                       | `signature`     |
| `prepare_rename` | `directory`, `path`, `version`, `position`                                                     | `rename_target` |
| `rename`       | `directory`, `path`, `version`, `position`, `name`                                               | `edited`        |
| `code_actions` | `directory`, `path`, `version`, `span`, `only` (optional)                                        | `code_actions`  |
| `take_code_action` | `directory`, `path`, `index`                                                                 | `edited`        |
| `file_history` | `directory`, `path`                                                                              | `file_history`  |
| `local_history`| `directory`, `path`                                                                              | `local_history` |
| `read_version` | `directory`, `path`, `version`                                                                   | `file`          |
| `read_conflict`| `directory`, `path`                                                                              | `conflict`      |
| `resolve_conflict`| `directory`, `path`, `text`                                                                   | `done`          |
| `revision_changes`| `directory`, `from` (optional), `to`                                                          | `revision_changes` |
| `permalink`    | `directory`, `path`, `line` (optional), `end_line` (optional)                                    | `address`       |

`agent` is an agent's id: `claude-code`, `codex` or `grok`.

`files` lists tracked, untracked and ignored files together, up to `limit` for
each listing. Ignored paths are marked in the `ignored` response field so the
tree can dim them. The optional `expanded` array is accepted for older clients
and has no effect. Clients list again when notified of changes.

A `directory` that is no repository but has repositories as direct children
(each with a `.git` entry, hidden and bulky folders such as `node_modules`,
`target`, `build` and `dist` not looked into) is a **folder of repositories**,
and every request that works on a repository works across them. Paths stay
relative to `directory`, so `api/src/main.rs` is `src/main.rs` of the child
`api`. A path is acted on by the child that holds it: `stage`, `discard`,
`ignore`, `read` of the last commit, `file_history`, `blame`, `permalink` and
`read_conflict` run there, and fail for a path in none of them (a file that
sits directly in the folder). `files` and `search_files` list the children's
files under their paths from `directory`. `branches` lists the branches every
child has before those only some have; `checkout`, `create_branch`, `commit`
(only in children with something to commit, as staged or, with `tracked`,
changed) and every `sync` operation run in each child, all of them even when
one fails, and the `failed` message names each child that did, as `name: why`.
`log` and `stashes` list nothing, and `stash`, `cherry_pick` and
`revision_changes` fail, for such a folder: they need one repository. Such a folder is one even when it sits inside another
repository; a folder with a `.git` entry of its own is a repository. A child that has submodules keeps
everything said of them, its submodules' paths written from `directory`.

- **`discover`:** starts the agent briefly and returns what it lets the reader
  change (models, modes, effort) and the commands it offers behind a slash.
  It can take several seconds.
- **`start_session`:** if `base` is given, the server looks it up in the
  repository at that moment, fails with `<base> does not exist` when it is
  gone, and otherwise makes a git worktree under
  `<repository>/.pandemonium/worktrees` with that commit checked out on a
  detached head, so no branch is made. The agent works there. Without `base`
  the agent works in `directory` itself. The machine settings change where and
  how: with `git.worktree_root` the worktree goes in
  `<worktree_root>/<repository name>` instead, and with `git.branch_template`
  it is checked out on a new branch named by the template, `{title}` standing
  for the title made into a name (a template without it gets `/<title>` after
  it), numbered on when that branch is taken; `started` then gives the branch.
  `git.worktree_include` lists ignored files and folders of the repository, as
  paths from its root separated by commas or lines (`*` stands for part of one
  name; `.env, .env.*, target/, node_modules/, .build/, build/, dist/, .venv/` by
  default), that are copied into the new worktree, sharing blocks where the
  filesystem allows; one that could not be copied is named in `skipped` with
  why, and does not fail the request.
  `settings` is a list of `[setting, value]` pairs applied before the first
  prompt; when it names no `mode` and `agents.permission_mode` is set, that mode
  is asked for too. The response gives the session number, the folder the agent works in
  and, for a worktree, the abbreviated commit it has checked out (`head`). The connection is attached to the session already.
  A worktree has every submodule checked out at the commit it records, nested
  ones included; a submodule that cannot be fetched is left out and named in
  `skipped` with why, and the session starts all the same. With `submodule`, a
  path from the root of the project, the agent works in that submodule's folder
  of the worktree (or of `directory`). When the worktree has a branch, every
  submodule it checked out is put on a branch of the same name; one that cannot
  be is named in `skipped` too.
  With `container`, set only for a folder that holds a `devcontainer.json`
  (`dev_container` answers `configured`), the agent and the session's
  terminals run in a dev container of their own for that session, started
  before the agent: the same container path as a dev container machine, but
  owned by the session's folder instead of the project folder, so two such
  sessions of one project have two containers. The session's worktree gets a
  copy of the git-ignored `.devcontainer` configuration, is mounted at the
  workspace folder, and the container is removed with the session. Build and
  start steps arrive as `Container` events; the agent keeps running when the
  client disconnects. A folder without a `devcontainer.json` fails the request.
  For a folder of repositories the worktree is a folder holding one worktree
  per child, laid out under the child's name (`<worktree>/api`,
  `<worktree>/web`), all checked out from `base` and, when the settings give a
  branch, all on the same new branch; the agent works in the folder, and files
  that sit directly in the project are not copied. `head` is the first child's.
  When a child cannot be made the request fails naming it (`web: <why>`) and
  the worktrees made before it are removed again, or named when they cannot
  be; `skipped` names a child's submodules and files as `<child>/<path>`.
- **`close_session`:** stops the session's agent and waits for its process to exit, drops its queued prompts and marks it `closed`. The
  log is kept, so the session still shows in `sessions` (with `closed: true`)
  and can be opened with `attach`. A session that has a worktree of its own
  has the worktree removed, unless another open session works in the same
  folder; while the worktree has changes that are not committed the request
  is answered `uncommitted` and the session stays open, unless `discard` is `true`.
  The branch the worktree was made on is deleted with it when every commit on
  it is also on the session's base or a remote, and kept otherwise. For a
  folder of repositories every child's worktree and its branch are removed,
  whatever commits the branch has, a child that fails does not stop the rest and is named in the
  failure, and the folder goes when they are all removed. A later `prompt` opens a closed session
  again, and fails when its folder was removed. Other clients are told with `session_closed`.
- **`rename_session`:** keeps `title` as the session's title, trimmed, and fails when it is empty.
  Every client watching the session is told with a `Titled` event, the one that asked included. The
  agent may name the session again later.
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
- **`remove_queued`:** discards the waiting prompt identified by `queued` and
  deletes its uploaded files. Announces `Unqueued` so connected and reconnecting
  clients remove the row. Other waiting prompts keep their order. If the prompt
  has already left the queue, succeeds without doing anything.
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
  declined. When the agent is no longer running but its log leaves a turn open,
  the turn is ended with `Halted` and a `Finished` that says why.
- A turn the server was working on when it stopped is ended the same way the
  next time it loads its sessions, so no session stays working after a restart.
- **`choose`:** answers an `ask` event. `ticket` is the ticket of the question
  and `option` is the `id` of one of its choices.
- **`answer`:** answers an `Inquire` event. `ticket` is the ticket of the form
  and `replies` holds one `{picks, other}` for each question, in order: `picks`
  are the `label`s of the alternatives chosen and `other` is what the reader
  typed, both possibly empty to skip the question. A null `replies` abandons the
  form, which aborts the agent's use of the tool.
- **`trust_hooks`:** trusts hooks a `Hooks` event of the session listed, on the
  machine the session runs on, the way Codex records trust itself (asked of
  `codex app-server`, so `codex` in a terminal sees the same state). A hook is
  trusted only when its `key` and `hash` still match what Codex lists, so one
  that changed since it was shown stays untrusted. A `Hooks` event follows with
  what is still held back, and the session's next turn starts its agent again
  so that it runs the hooks trusted. Nothing is trusted without this request.
- **`attach`:** subscribes the connection to a session and replays the events
  numbered after `after`. See [Events](#events).
- **`standing`:** where a folder stands, answered from git's refs alone and so
  at once however many files differ: the branch, the abbreviated head, what the
  branch follows, the operation it is stopped in, and whether the folder is a
  repository or holds some. A client asks for it beside `inspect` and shows the
  branch from it without waiting for the files.
- **`diffs`:** the diffs of the files `paths` names, each as `inspect` named it,
  for a client to show only the files in view. Each answer is `{path, diff,
  words}`, a file's lines from its first hunk on, at most 2000 of them, and for a
  file git does not track all its lines as added.
- **`inspect`:** the files that differ from the last commit, listed with the
  lines they change but without their diffs (`lazy`), and, when `base` is
  given, the messages of the commits the branch has that `base` has not, plus the branch and abbreviated head commit the folder is on.
  Each change's `words` is `[{line, ranges}]`: for a removed or added line of its
  diff, counted from 0 among the lines of `diff`, the `[start, end]` bytes of the
  line, without its marker, that differ from the line paired with it in the same
  run of changes.
- **`set_accounts`:** keeps the logins and the login in use for each agent on
  the server's machine, and points the agents at them. `accounts` reads them
  back. The login in use for an agent is the one new sessions of it start
  with; a session keeps its own login after that.
- **`set_session_account`:** has the session run as `login`, or as the agent's
  own login when it is `null`, without touching any other session or the login
  new sessions start with. The server keeps it with the session; a running
  agent is started again under the new login before the session's next turn,
  and told the conversation so far.
- **`projects`, `choices`:** what the reader set up, kept by the server so that
  every client sees the same. `projects` says `saved: false` when the list was
  never saved, which means the app has not been set up yet. A project is an
  address of a folder on the server's machine, such as `~/code/app`.
- **`mcp_servers`, extensions:** `mcp_servers` also lists the tool servers the extensions installed on the machine bring, each with `extension` set to the extension's folder name; `save_mcp_servers` leaves those out of what it keeps, and they go when the extension does. A tool server started as a program is `enabled: false` while its extension is not allowed to run programs, and one reached by address while it is not allowed to download.
- **`agent_servers`, `mcp_servers`:** the agents the server can start and
  the tool servers every agent is started with, kept by the server and replaced
  whole by the matching `save_` request. The agents the app ships are in the list the first time it is asked for, and are changed or removed like any other; `package`, `installer` and `home` are optional. A tool server's `target` is an address
  when it begins with `http://` or `https://`, and `env` is then the headers it
  is sent; otherwise it is the command that starts it, and `env` its environment.
  Every new session is started with the tool servers saved at that moment that are `enabled`; `description` and `website` are optional and `enabled` is true when missing.
- **`library`, `tool_sets`, `save_withheld`:** what an agent is given in a
  project, read on the server's machine from the folders the agent's product
  keeps it in, so a remote project lists the remote machine's files. `library`
  answers with `product` (`claude`, `codex`, `grok`, `copilot`, or null), the
  `items` found (`[{kind, scope, name, description, path, applies, model, tools,
  command}]`, `kind` one of `skill`, `instruction`, `agent`, `hook` and `scope`
  one of `workspace`, `user`, `built_in`) and `withheld`. `tool_sets` answers
  with `product`, `sets` (`[{id, name, source, plugin, tools: [{name,
  description}], error}]`, `source` one of `built_in`, `server`, `plugin`): the
  agent's own tools under the id `built-in`, then each tool server's, listed by
  starting it, under the server's name. `withheld` is every key turned off: an
  item's `<kind>/<scope>/<name>` (the scope written `built-in`), a whole set's
  `tools/<id>` and one tool's `tool/<id>/<tool>`; `save_withheld` replaces it
  whole and the server keeps it. A session started afterwards is not given what
  is turned off where its product can leave it out: a tool server turned off
  whole is left out of `mcpServers`, Claude Code is started without the skills,
  agent definitions and tools turned off and without the instruction files, and
  Codex without the skills; `skills` leaves out the skills turned off. The
  files on disk are never changed.
- **`registry_agents`, `search_tools`:** what the public registries of agents and
  of tool servers offer, read by the server so that a client needs no access to
  the internet. Only agents that are a package a runner fetches are listed.
  `search_tools` with a blank `query` lists the well known servers; `asks` says
  that installing a listing has to ask for something first.
- **`sessions`:** every session the server holds, running or not, with enough
  to list it. To open one, send `attach` with `after: 0`: the log replays the
  whole conversation.
- **`watch_folders`:** the server tells this connection with `folder_changed`
  whenever a file in one of `directories` changes, or what git keeps about it
  (the index, `HEAD`, the references, also where a worktree keeps them outside
  its folder). It replaces the folders the connection watched before; an empty
  list watches nothing, and the watches end with the connection. Changes inside
  `.pandemonium`, `node_modules`, `target`, `.git/objects` and `.git/logs` are
  left out, and changes coming close together are told once, at most every
  1.5 seconds while the folder keeps changing.
- **`machine_settings`, `save_machine_settings`:** what the reader set up for
  the work the server does on its machine, kept by the server, replaced whole by
  the save and put to work at once. `git` is `{auto_fetch, fetch_minutes,
  branch_template, worktree_root, worktree_include, hosts}`: `hosts` maps the host name of a
  self-hosted git server to the layout its web addresses use (`github`,
  `gitlab`, `bitbucket`, `gitea`, `azure`, `sourcehut` or `gitee`), which
  `permalink` follows for a host whose name does not give it away; while `auto_fetch` is set the server runs
  `git fetch --all --prune` every `fetch_minutes` in the repositories of its
  open sessions and of the projects saved on its machine. `history` is
  `{enabled, days, megabytes}`: whether the server keeps a state of a file each
  time it is written with `write_file`, how many days it keeps it and how many
  megabytes of states one project may take up (30 days and 256 megabytes when
  not set). `agents` is
  `{default_agent, permission_mode}`, each optional: the agent a client offers
  first for new work, by id, and the mode new sessions start in. `shell` is
  `{program, arguments}`, the shell `open_terminal` starts when `program` is not
  empty. `extensions` maps an extension's folder name to `{granted}`, the
  capabilities it was allowed; `save_machine_settings` does not change it, only
  `install_extension` and `grant_extension` do. `languages` maps a language's id to `{tab_size, hard_tabs,
  format_on_save, server_off, server_command, servers}`: the first three are for
  clients to edit with and are optional; `server_off` keeps the language server
  from reading the language, and a `server_command` is started in place of the
  one the language comes with. `servers` lists the language servers that read
  the language, by id and in order: the first is the language's server and the
  rest run beside it, `...` stands for the servers that run when nothing is
  chosen, and a server that is opt-in for the language is started only when it
  is named. `servers` at the top maps a server's id to what is set for it,
  whichever language it reads: `{binary, arguments, env, initialization_options,
  workspace_configuration}`, each over what the language says. Files already
  open move to the new servers at once, and a server whose settings changed is
  stopped and started again.
  `organize_imports_on_save` and `fix_all_on_save` are optional too, for
  clients to decide whether saving first asks the language server for the
  `source.organizeImports` and `source.fixAll` changes.
  `workspace` is `{attach_repositories, folders}`: the folders besides a
  project's own that its language servers work on. The server sends a language
  server all of them as `workspaceFolders` when it starts it, the project's
  folder first, and tells a running server which came and went with
  `workspace/didChangeWorkspaceFolders` when the settings change, for a server
  that says it follows them. `folders` are written `folder` for every project
  or `project=folder` for one, a project named by its folder or that folder's
  name, a folder that is not a full path taken from the project's root.
  `attach_repositories` adds the folders inside the project that are
  repositories of their own and the submodules of its repository. All of it is
  resolved on the machine that holds the project, so a project on another
  machine takes the folders of that machine's settings. A definition in an
  attached folder comes as a file outside the project, with its text, and
  `workspace_symbols` lists up to 25 symbols of attached folders in the same
  way after those inside the project.
  The window sends the settings the server on its own machine keeps to every
  other server it reaches, as it reaches it and whenever they change, so a
  project on another machine works by the same settings.
- **`latest_release`:** the newest release of the app on `channel`, looked up
  on the project's releases: the latest made for everyone for `stable`, the
  newest pre-release for `preview` (`nightly`, from a client older than the rename, is read as `preview`).
- **`download_update`:** downloads the build of `version` made for the
  server's platform from the release on `channel`, checks it against the
  release's `SHA256SUMS` and unpacks it into the server's cache folder.
  Progress is told to the asking client as `update_progress` messages
  (`received` and `total` bytes, `total` zero when unknown); the answer is
  `update_staged`, or `done` when `cancel_update` stopped it.
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
  is `false`, unstages them. A path inside a submodule is staged in the
  submodule; a submodule's own path stages the commit it is at in the
  repository around it.
- **`commit`:** commits what is staged under `message`. With nothing staged and
  `tracked` set, it takes the changes of every tracked file instead. It commits
  in the repository `directory` is, so a submodule's folder commits in the
  submodule. Fails on an empty message.
- **`log`:** the most recent `limit` commits, newest first, as `{id, refs,
  summary, lanes}`. `refs` are the branches and tags pointing at the commit as
  git decorates them (`HEAD -> main`, `tag: v1`). `lanes` lays the commit out in
  a graph: its `lane` and `color`, the `edges` crossing its row (`half`
  `Upper` or `Lower`, `from` and `to` lanes, `color`) and the `width` in lanes.
  Without `all` it lists what the checked out branch reaches; with it, every
  reference.
- **`sync`:** `operation` is `fetch` (all remotes, pruning), `pull`,
  `pull_rebase`, `push` (publishing the branch when it follows nothing, after
  pushing each submodule on a branch with commits no remote has),
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
  index and in the files; a path that is not in the last commit is deleted. A
  path inside a submodule is put back in the submodule; a submodule's own path
  is checked out again at the commit recorded.
- **`ignore`:** adds each of the `paths` to the `.gitignore` at the root of the
  repository, anchored with a leading slash, unless it is in it already.
- **`checkout`:** checks out the local branch `branch`; a remote branch such as
  `origin/main` gets a local branch that follows it. Fails when git refuses,
  for instance while the branch is checked out in another worktree.
- **`create_branch`:** makes the branch `name` at the commit the folder is at
  and checks it out.
- **`create_entry`:** makes an empty file, or a folder when `folder`, at
  `path` (from the root of `directory`) and the folders above it; a name that
  is taken is refused.
- **`rename_entry`:** renames or moves `from` to `to`, both from the root of
  `directory`; a name that is taken is refused. Language servers running for
  the folder are asked first for the edits the rename calls for, such as the
  imports that name the file; the answer is `edited` when some of them are to
  files a client has open, and `done` otherwise.
- **`copy_entry`:** copies `from` into the folder `into` (empty for the root of
  `directory`), under `name copy.ext`, `name copy 2.ext` and so on when the name
  is taken.
- **`remove_entry`:** takes `path` and everything in it off the disk. Every
  path of these four must stay inside `directory`.
- **`write_file`:** replaces the contents of `path` (from the root of
  `directory`, which must stay inside it) with `text`, making the file when it is
  not there. A folder is refused. When the write succeeds and `directory` is in a
  repository, the server keeps `text` as one more state of the file (see
  `local_history`).
- **`local_history`, `read_version`:** every `write_file` keeps the text it wrote
  in the git folder of the worktree `directory` is in, beside the worktree, so
  the states outlive the client that saved them and any other client reaches
  them. `local_history` lists the states kept of `path`, newest first, after
  removing those older than `history.days` of the machine settings;
  `read_version` answers `file` with the state `version` names, which is the
  `id` the list gave. A folder that is in no repository keeps no states and
  answers `failed`. Past `history.megabytes`, or `history.days`, the server
  removes the oldest states of the project, when a state is kept, when one is
  listed and every ten minutes; with `history.enabled` off it keeps none.
- **`read_conflict`, `resolve_conflict`:** while a merge, a rebase or a
  cherry-pick is stopped on a conflicted `path`, `read_conflict` answers
  `conflict` with the versions the index keeps, which are the common ancestor
  (`base`), the branch being worked on (`ours`) and the incoming side
  (`theirs`), and with `merged`, what the working folder holds. It fails for a
  file that is not in conflict, not text or larger than a client reads whole.
  `resolve_conflict` writes `text` as the file, as `write_file` does, and
  stages it, which is what concludes the conflict for that file.
- **`revision_changes`:** the files a commit changed with their diffs, when
  `from` is not given: `to` against its first parent, or against nothing for a
  commit that has none. With `from`, the files `to` changed since it left
  `from`, as `git diff from...to` says; `to` and `from` are names of commits or
  branches.
- **`read_file`:** reads `path` from the working folder or, when `head`, from
  the last commit, or from the commit `revision` names by its object name. The response holds no text for a file that is not text, and
  names the installed language the file is written in, by the endings, names
  and name patterns of files its manifest lists.
- **`extensions`:** every extension the list offers, then those
  installed on the server's machine that it does not, each as `{id, name,
  kind, version, description, icon, color, hidden, suffixes, names, patterns,
  installed, busy, problem, server, local}`. `kind` is what the extension is; `language` is the
  only kind so far. `icon` is the SVG an installed language's files are shown
  with and `color` its tint as `#rrggbb`; a client shows a file of no installed
  language as a plain file. `hidden` languages are
  installed only as part of another and left out of what a reader picks from;
  `busy` says what installing it is doing now, `problem` why installing it or
  its language server last failed, and `server` what the machine has of its
  language server, in words. `update` says what is newer than what is
  installed, the language as the list now has it or its language server, and
  installing the language again, which a failed download leaves as it was,
  brings it up to date. An extension package's `status` is what its
  module last said it was doing, such as the release of a language server it
  fetched, and its `problem` includes why its module was refused something. The
  list is `languages.json` read from
  `https://raw.githubusercontent.com/JoakimCarlsson/pandemonium/<tag>/languages`,
  where the tag is the server's own release (`v<version>`); a debug
  build, and the preview channel, read the `languages` folder of their checkout instead, and
  `PANDEMONIUM_LANGUAGES` names another folder or address for either. It is fetched again only
  when it changed, and the copy kept is used when it cannot be fetched. The
  language folders in `local-languages` in the server's data folder are listed
  as well, with `local: true`, each in place of a listed language with the same
  name; their files are not checked against `languages.json`, and a folder may
  hold its own `grammar.wasm` in place of a pinned `grammar_file`.
- **`install_extensions`:** installs each extension of `ids`, the extensions it
  `requires` first, and their language servers, on a thread of the server's
  own, and answers at once. Every file of a language's folder is checked against
  the checksum `languages.json` gives it, and the grammar and a downloaded
  language server against the checksums the language's `config.toml` gives
  them. A language server is looked for in the project's own folders and on the
  machine's path first; one that is missing is installed into the data folder
  under its version, by download, `go install` or `npm ci` as the manifest
  says, and a version installed before is used when that fails. `npm ci` runs
  from the `package-lock.json` in the language's folder, which pins the
  checksum of every package, and `go install` runs with Go's checksum database
  enforced. Every client is
  told how it goes with `extensions_changed`.
- **`install_extension`:** installs the folder at `path` on the server's
  machine. A folder holding an `extension.toml` is an extension package, which
  [`docs/extensions.md`](extensions.md) describes: its folder is kept under
  `extensions` in the server's data folder, each language it carries is
  installed through the language index as `<extension>-<folder>`, and `grants`,
  the names of the capabilities it asks for that the reader allows
  (`process`, `download`, `read-project`), are saved in the machine settings.
  Everything else a package carries is read from the kept folder when it is
  used. A language that has to download its grammar or its language server is
  not installed unless `download` is granted, and the reason comes back as the
  extension's `problem`. A folder holding only a `config.toml` that reads as a
  language is copied into `local-languages` under the folder's name, in place of
  one of the same name, and installed as `install_extensions` does.
- **`inspect_extension`:** reads the folder at `path` on the server's machine
  without installing it, to show the reader what it `provides` and what it
  `requests` before they allow it anything. A language folder answers with no
  requests.
- **`grant_extension`:** allows the installed extension `id` the capability
  `capability`, or takes it back when `granted` is false; it must be one the
  extension's manifest asks for. Every client is told with
  `extensions_changed`. Allowing `download` installs what the extension could
  not before.
- **`extension_assets`:** what the installed extensions bring that a client uses
  itself: themes, icon themes and commands. A theme is a colour as `#rrggbb` or
  `#rrggbbaa` for each role it names, by the role's name such as
  `surface-hover`, in a dark and a light variant; roles it leaves out keep the
  default theme's. An icon theme gives an SVG, and a tint, for the endings of
  file names.
- **`run_extension_command`:** runs the command `command` of the extension
  `extension` in the folder `directory` when there is one, and answers what it
  came to: a `prompt` for the client to put in its prompt box and a `message` for
  the reader. A command whose code asks to do something its extension was not
  allowed is not stopped from finishing: the thing is refused, and the refusal
  is what the code does with it.
- **`remove_extension`:** takes the extension off the machine, with the
  languages it installed and what was allowed it; a language's server stays, for
  other languages that use it.
- **`grammar`:** the grammar and queries of an installed language, found by its
  folder name or a name a code fence calls it, as `{id, symbol, aliases, wasm,
  highlights, indents, outline, injections, brackets, runnables, overrides,
  redactions, textobjects, debugger, indent, tabs, line_comment, pairs,
  scopes, tags, word_characters, bracket_pairs, increase_indent_pattern,
  decrease_indent_pattern, decrease_indent_patterns}`. A language that builds on
  another (`inherits`) is sent with the queries and grammar it takes from it.
  `wasm` is the tree-sitter grammar compiled to
  WebAssembly, in base64; a client parses and highlights with it itself.
  `pairs` lists what the editor closes as it is opened, each the opening and
  the closing character, such as `"()"`. `scopes` names parts of a file that are
  edited differently, each as `{nodes, line_comment, block_comment, emmet}`:
  `nodes` are the grammar's node kinds the scope is made of, the caret is in the
  scope of the innermost of them it is strictly inside, `line_comment` replaces
  the language's (an empty one means there are none inside), `block_comment` is
  the `[open, close]` marks used when there is no line comment, and `emmet`
  says whether abbreviations expand there; a language that sets `emmet` in any
  scope expands them only in those that set it true; a scope the language's
  `overrides` query captures is one without `nodes`. `word_characters` are the
  characters besides letters, digits and the underscore that are part of a
  word, `bracket_pairs` are `{start, end, close, not_in}` for the pairs that
  are not closed in some scopes, and the three `*_indent_pattern*` keys are the
  language's line patterns for indentation. `tags` names the node kinds
  of `element`, `open`, `close` and tag `names`, with which the editor writes
  the closing tag when the `>` of an opening tag is typed.
- **`open_document`:** opens `path` of the folder `directory` for the language
  server of `language`, holding `text` at `version`, and starts that server for
  the folder when it is not running, which can take a while. The response gives
  the characters after which the server offers completions. The connection is
  told what the server finds wrong with the file with `diagnostics`.
- **`change_document`:** edits an open file the way the client did. `changes`
  take the text from the version before to `version`, each a `span` (absent for
  the whole text) and the `text` that replaces it. Requests are carried out
  concurrently, so changes that arrive ahead of the ones before them wait for
  those, and ones already taken in are ignored.
- **`close_document`:** closes the file for this connection; the server closes
  it for its language server once no connection has it open, and stops the
  server once no open file of the folder uses it. A connection that goes away
  closes what it had open.
- **`completion`, `hover`:** wait until the open file reaches `version`, then
  ask its language server, and the companion servers the language names for the
  project, whose answers are added to its own: a language's `[[companion]]` is a
  second server that reads the same file, started only where its `detect` finds
  a dependency in the root `package.json` or a file at the root, such as the
  Tailwind CSS server for a project with `tailwindcss`. `scope` is the name of
  the scope of the language the place is in; a companion that names `scopes` is
  asked for completions only inside those. Companion servers' diagnostics are
  not published. `completions` holds `[{label, detail, kind, text,
  span, filter, sort, documentation}]`, the best match for the word before
  `position` first, with snippets made plain text; `span`, when given, is what
  choosing it replaces, and `documentation` is markdown when the server sent
  it with the list. Each item also holds `additional_edits`, `[{span, text}]`
  that choosing it makes besides writing its text, such as the line that
  imports it, and `resolve`, the item as its own server wrote it, set while
  that server has more to say about it and never on a companion's items.
  `resolve_completion` hands `resolve` back to the server that listed the item
  and answers `completion_resolved` with the item completed: its
  documentation, detail and `additional_edits`. `hovered` holds `{text, span}`
  in markdown, or null.
- **`format`:** the edits that format the open file at `version`, as its
  language server gives them; only the stretch `range` of it when that is
  given, and none when the server formats whole files only.
- **`format_on_type`:** the edits that format what was just typed, `character`
  having been typed to reach `position`, as `formatted`; none when the server
  does not format as one types.
- **`folding_ranges`:** the blocks of the open file at `version` that can be
  folded, as the language server gives them; none when it does not say.
- **`linked_ranges`:** the stretches of the open file at `version` that are
  edited together with the one at `position`, the one at `position` among
  them, as the language server gives them; none when it knows of none.
- **`semantic_tokens`:** what each name of the open file at `version` is, as
  the language server says, one token to a stretch of a line; none when it
  does not say. `rules` are the reader's own rules for what the types of tokens
  look like, `{token_type, token_modifiers, style}`, which come before those of
  the language's `semantic_token_rules.json`.
  With a `range` (a span of whole lines) the server asks the
  language server only for the names within it when the language server says
  it can (`opened` tells `semantic_ranges`), and the answer has `partial` set;
  otherwise, or without a `range`, the answer holds every name of the file and
  `partial` is not set. A client with a long file asks for the lines in view
  first and for the whole file after, and asks for a further `range` while the
  whole file has not arrived and the view moves past the lines it holds.
- **`document_links`:** which stretches of the open file at `version` lead
  somewhere when pressed, as the language server says: web addresses, and
  files inside the folder with the place they name. Links to anywhere else are
  left out, and so are links the server gives no address for.
- **`resolve_link`:** where a link the server gave without an address leads,
  asked of the server that listed it when the link is pressed. `link_resolved`
  holds `target`, a link target as in `document_links`, or null.
- **`color_presentations`:** the ways the language server offers to write
  `color` in place of `span`, such as `#rrggbb` or `rgb(…)`; `span` is the
  stretch the colour is written in.
- **`colors`:** which stretches of the open file at `version` spell a colour, as
  the language server says; none when it does not say.
- **`inlay_hints`:** what to show inside the lines of the open file at
  `version` within `span`, as the language server says; none when it does not
  say.
- **`inline_values`:** which stretches of the open file at `version` within
  `span` to show the values of while the program is paused at `stopped`, in
  the debugger's `frame`, as the language server says; none when it does not
  say.
- **`code_lenses`:** what to show with stretches of the open file at
  `version`, as the language server says, each already filled in by the
  server. The server keeps the list as given, and `press_code_lens` takes one
  by its place in it. With `dependencies` set, the lenses of the language
  server are followed by one for each dependency the file lists, when its
  language says how and the registry names a newer release; pressing one
  answers with the change that writes the version, and the lookups are
  cached on the server. A client that does not send it gets none of these.
- **`press_code_lens`:** presses one of the lenses last given for the open
  file: the places it points to when it counts uses, and otherwise the
  changes the language server made carrying out its command, as for
  `take_code_action`.
- **`definition`:** the places `seek` looks for from `position`: `definition`
  (the default), `declaration`, `type_definition`, `implementation`,
  `references`, `incoming_calls`, `outgoing_calls`, `supertypes` or `subtypes`. A file inside the folder
  is named by its path from the root; one outside it by its absolute path, with
  the whole text it holds for the first four, to be shown read only.
- **`prepare_rename`:** asks the language server what can be renamed at
  `position`. `rename_target` holds `target`: `{span, placeholder}`, each null
  when the server leaves it to the word at the place, or null when nothing
  there can be renamed.
- **`rename`, `take_code_action`:** carry out an edit across the folder. Files
  no connection has open are changed on disk by the server; the changes to the
  open ones come back in `edited` for the client to make, each a stretch of
  the file as it is before any of them. `code_actions` lists what the language
  server offers for `span`, and `take_code_action` takes one by its place in
  the latest list for that file. `only` limits the list to the kinds of change
  named, such as `source.organizeImports` or `source.fixAll`, which the
  language server is asked for in `context.only`; every kind when it is empty
  or not given. A client organizes the imports of a file by asking for that
  kind over the whole file and taking the one the server prefers.
- **`search_files`:** the lines of the files of the folder's repository,
  tracked or not but not ignored, that hold `query`, as typed or as a regular
  expression when `regex`, matching case only when `case`; as `definitions`
  with `line` holding each line, at most 500 of them.
- **`workspace_symbols`:** the symbols of the folder whose names match
  `query`, as the language server of the open file `path` finds them, those
  defined inside the folder, or in a folder attached to its language server's
  workspace (see `machine_settings`), as `definitions` with `name` naming each.

A **position** is `{line, character}`, counted from 0, `character` in UTF-16
code units as language servers count; a **span** is `{start, end}`.

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
| `files`      | `paths`: repository-relative tracked, untracked and ignored files, `folders` (optional): untracked and ignored folders that may hold no file yet, `ignored` (optional): which paths and folders git ignores, listed so a tree can dim them |
| `branches`   | `names`: local branches, the current one first, then remote ones such as `origin/main`; read from git at each request. Of a folder of repositories, those every child has, then the rest |
| `submodules` | `paths`: every submodule's path from the root of the repository, nested ones included, initialised or not; in a folder of repositories, each child followed by its submodules, from the folder, so a child is listed like a submodule |
| `submodule_heads` | `heads`: `[[path, head]]` for each checked out submodule, nested ones included (in a folder of repositories each child first, then its submodules): its branch, or `@ <commit>` on a detached head. `branches`, `checkout` and `create_branch` on a submodule's or a child's folder act on that repository alone |
| `stashes`    | `stashes`: `[{index, message, when}]`, newest first |
| `log`        | `commits`: `[{id, refs, summary, lanes}]`, newest first |
| `file`       | `text`: string or null for a file that is not text, `truncated`: whether it was cut short, `language` (optional): the installed language it is written in |
| `bytes`      | `bytes` (base64): what a file holds, `truncated`: whether it was cut short |
| `identity`   | `default`, `selected`: who the agent is signed in as, or null |
| `started`    | `session`, `directory`, `branch` (always null for now), `head` (null without a worktree), `skipped`: `[[path, reason]]` for submodules the worktree could not check out |
| `standing`   | `branch`: string or null, `head`: string or null, `tracking`: `{upstream, ahead, behind}`, `operation`: as in `inspected`, `repositories`: `[{path, branch, head, tracking}]`, `repository`: whether the folder is a repository or holds some |
| `diffs`      | `files`: `[{path, diff, words}]`, one for each of the files asked for that a repository of the folder holds |
| `inspected`  | `changes`: `[{path, diff, words, staged, untracked, index, worktree, conflicted, from, submodule, repository, moved, added, removed, lazy}]`, `added` and `removed` the lines the file puts in and takes out, counted over the whole file, and `lazy` true when `diff` and `words` are left out because the file's diff is asked for with `diffs` (an older server leaves `lazy` false and sends them), files inside submodules listed under their paths from the root with `submodule` the submodule's path, files of a folder of repositories listed under their paths from the folder with `repository` the child's folder name and null for one repository, and a submodule checked out at another commit than the one recorded as its own path with `moved`: `{from, to, commits, back}` (abbreviated commits, the subjects between them oldest first, `back` when it went back past them) and an empty `diff`, `commits`: `[string]` or null, `branch`: string or null, `head`: string or null, `tracking`: `{upstream, ahead, behind}`, `operation`: `Merge`, `Rebase` or `CherryPick` with what git prepared, or null, `repositories` (optional): `[{path, branch, head, tracking}]` for each child of a folder of repositories, empty for one repository; `branch`, `head` and `tracking` then sum up the children: the one branch they are all on or the branches joined by ` · `, the first child's commit, and the commits ahead and behind added up, following something only when every child does |
| `projects`   | `paths`, `saved`                                          |
| `choices`    | `choices`: `[[agent, setting, value]]`                    |
| `agent_servers` | `servers`: `[{id, name, command, env, package, installer, home}]`                |
| `mcp_servers` | `servers`: `[{name, target, env, description, website, enabled, extension}]`          |
| `library`    | `product`, `items`, `withheld`, as described under `library` above |
| `tool_sets`  | `product`, `sets`, `withheld`, as described under `library` above |
| `machine_settings` | `settings`: `{git, history, agents, shell, languages, extensions, servers}`, as `save_machine_settings` takes them |
| `latest_release` | `version`: the version, or null when none was found; `url`: its page; `name`, `published`, `body` (markdown notes): each null when absent; `assets`: `[{name, url, size}]` |
| `update_staged` | `path`: where the checked and unpacked update is, to install from |
| `registry_agents` | `agents`: `[{id, name, version, description, website, command, env}]` |
| `tool_listings` | `listings`: `[{id, title, description, name, target, env, inputs, asks, website}]` |
| `accounts`   | `logins`: `[{agent, id, name}]`, `in_use`: `[[agent, id]]` |
| `sessions`   | `sessions`: `[SessionInfo]`                               |
| `terminal`   | `terminal`: the number of the shell that was started      |
| `terminals`  | `terminals`: `[{terminal, directory, columns, rows}]`, oldest first; `directory` is the address `open_terminal` was given |
| `tasks`      | `tasks`: `[Task]`, those the project writes first; `problem`: why its own file could not be read, when it could not |
| `runnable_tasks` | `tasks`: `[{name, command}]` that can run the place, the one `run_runnable` runs by default first |
| `toolchains` | `toolchains`: `[{language, id, name, detail, selected, language_name}]`, grouped by language; `language_name` is what the manifest of a language of the family calls it, such as `Node.js` |
| `debug_scenarios` | `scenarios`: `[{label, adapter, installed, attach, source, task}]`, the order `debug_start` chooses by; `task` is the task the scenario names to run first |
| `debug_processes` | `adapters`: `[{id, name, attach_names}]` installed, where `attach_names` are the starts of the names of the programs an adapter attaches to, an empty one standing for any, `processes`: `[{pid, name, command}]`, newest first |
| `debug`      | `debug`: the number of the session, `view`: the [view](#debugging) of it now |
| `debugs`     | `debugs`: `[{debug, directory, label}]`, oldest first      |
| `owners`     | `owners`: `[{login, organization}]`                       |
| `repositories` | `repositories`: `[{name, description, private, url}]`   |
| `folders`    | `path`: absolute, `names`: the folders inside it           |
| `directory`  | `path`: a `clone` made   |
| `extensions` | `extensions`: `[{id, name, kind, version, description, icon, color, hidden, suffixes, names, patterns, installed, busy, problem, server, local, owner, provides, requests, granted, status}]` |
| `extension_preview` | `extension`: one entry as `extensions` lists them, not installed, with what it `provides` and `requests` |
| `extension_assets` | `themes`: `[{extension, name, dark, light}]`, `icon_themes`: `[{extension, name, icons: [{suffix, svg, color}]}]`, `commands`: `[{extension, id, title, description}]` |
| `extension_command` | `prompt`: the text to put in the prompt box, `message`: a line to tell the reader; each is optional |
| `grammar`    | `{id, symbol, aliases, wasm, highlights, indents, outline, injections, brackets, runnables, overrides, redactions, textobjects, debugger, indent, tabs, line_comment, pairs, scopes, tags, word_characters, bracket_pairs, increase_indent_pattern, decrease_indent_pattern, decrease_indent_patterns}` |
| `opened`     | `triggers`: the characters after which the server offers completions; `signature`: those after which it tells what the call the caret is in looks like; `format`: those after which it formats what was typed; `formats_ranges`, `folds`, `links`, `semantic_tokens`, `inlay_hints`, `code_lens`, `inline_values`, `document_links`, `colors`, `semantic_ranges`: whether it formats a stretch, says what can be folded, says what is edited together, says what each name is, says what to show inside lines, says what to show with stretches and says which names to show the values of while a program is paused, says which stretches lead somewhere when pressed and says which stretches spell a colour and says what the names of a stretch are |
| `completion_resolved` | `item`: a completion, as in `completions`, with what its server added when asked |
| `completions`| `items`: `[{label, detail, kind, text, span, filter, sort, documentation, additional_edits, resolve}]`, `kind` one of `function`, `variable`, `type`, `module`, `keyword`, `constant`, `snippet`, `other` |
| `hovered`    | `hover`: `{text, span}` or null |
| `formatted`  | `edits`: `[{span, text}]` |
| `folding_ranges` | `ranges`: `[{start, end}]`, the first and last line of each block, counted from 0 |
| `linked_ranges` | `spans`: the stretches edited together |
| `semantic_tokens` | `tokens`: `[{line, start, length, kind}]`, `start` and `length` in UTF-16 code units, `kind` one of `keyword`, `function`, `type`, `string`, `constant`, `comment`, `property`, `operator`, `variable` Also `partial`, set when the tokens are those of the requested `range` alone. |
| `document_links` | `links`: `[{span, target}]`, `target` one of `{kind: "url", address}`, `{kind: "file", path, position}` with `position` null when the link names none, or `{kind: "unresolved", link}` for a link the server says where it leads only when asked |
| `link_resolved` | `target`: a link target as in `document_links`, or null |
| `color_presentations` | `presentations`: `[{label, span, text, additional_edits}]`, `additional_edits` as in `completions` |
| `colors` | `colors`: `[{span, red, green, blue, alpha}]`, each channel from 0 to 255 |
| `inlay_hints` | `hints`: `[{position, label, kind, pad_left, pad_right}]`, `kind` one of `type`, `parameter`, `other`, `value` |
| `inline_values` | `values`: `[{span, kind, text}]`, `kind` one of `text` (`text` is the text), `variable` (`text` the name, or none for the one the span spells), `expression` (`text` the expression, or none for the one the span spells) |
| `code_lenses` | `lenses`: `[{span, title, pressable}]` |
| `lens_pressed` | `places`: as `locations` of `definitions`; `files`: as of `edited` |
| `definitions`| `locations`: `[{path, span, inside, text, language, name, line}]`; `name` names a calling or called function, `line` is the line the span starts on |
| `code_actions` | `actions`: `[{title, kind, preferred, disabled}]` |
| `highlights` | `spans`: the stretches of the file where the thing at `position` is used |
| `blamed`     | `blame`: `{id, author, when, summary}`, who last changed the line as the file is on disk; `id` is empty for a line not committed, `when` seconds since the Unix epoch |
| `problems`   | `problems`: `[{path, diagnostic}]`, what the language servers last said is wrong with each file of the folder, open or not, the worst first |
| `signature`  | `signature`: `{label, active, documentation, others}` or null; `active` is the byte range of `label` naming the parameter the place is at, `others` how many more signatures there are |
| `edited`     | `files`: `[{path, changes: [{span, text}]}]` |
| `file_history` | `commits`: `[{object, id, author, when, time, summary, path}]`, newest first, following the file back through moves; `path` is what it was called in that commit and `time` when it was written, in seconds since the Unix epoch |
| `local_history` | `versions`: `[{id, size}]`, the states the file was kept in each time it was saved, newest first; `id` is when it was kept, in milliseconds since the Unix epoch, and `size` how many bytes it held |
| `conflict`   | `files`: `{ours, theirs, base, merged, language}`: what each side of a conflicted file holds, each text or null for a side that has no such file, `merged` the file as the working folder holds it with the markers git wrote, `language` as of `file` |
| `revision_changes` | `changes`: `[{path, diff, staged, untracked, index, worktree, conflicted, from}]`, as of `inspected`, for each file a commit or a branch changed |
| `address`    | `url`: the web address of the file, at the line when one is given, on the host of the repository's remote, at the commit checked out |

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
base, title, closed, updated, last_seq, running, login, submodule}`. `updated` is milliseconds since the epoch,
`last_seq` is the number of the session's latest event, `running` says
whether its agent is running now, and `login` is the id of the login the
session runs as, `null` for the agent's own. `submodule` is the path of the
submodule its agent works in, `null` for the project itself; `directory` is
still the folder of the whole worktree.

### Signing in again during a session

`sign_in_session { session }` runs the agent's existing login flow on the server
that owns the session, using that session's account folder rather than the
account selected for new work. It answers `done` or `failed`.

`{"SignedOut": {"reason", "account"}}` ends a turn that failed with ACP `LOGIN_REQUIRED`
(`-32000`) or an authentication failure. `account` names the session's account
on its server, or is null for the default account. It replaces `Finished` for that turn;
clients show a sign-in notice and action, not an assistant message. The server
stops the expired agent and drops queued prompts without sending them.
`"SignedIn"` clears the notice after a successful `sign_in_session`; login
failure sends `SignedOut` with its reason instead. Both events are numbered,
persisted, and replayed like other session events. The next prompt uses the
same session and its conversation history with refreshed credentials.

### Event types

Events use serde's default form, one key naming the type.

| Event                         | Meaning                                                                                                   |
| ----------------------------- | --------------------------------------------------------------------------------------------------------- |
| `{"Settings": [Setting]}`     | What can be changed and how it stands now. Replaces the earlier list.                                     |
| `{"Commands": [Slash]}`       | The commands offered behind a slash. Replaces the earlier list.                                           |
| `{"Used": {"used", "size"}}`  | How many tokens the context holds and how many it takes in all. Replaces the earlier count.               |
| `{"Said": "text"}`            | More of the agent's reply. Append it to the reply so far.                                                 |
| `{"Thought": "text"}`         | More of the agent's reasoning. Append it.                                                                 |
| `{"Tool": Tool}`              | A tool the agent is using. Later events with the same `id` update it.                                     |
| `{"Background": {id, label, running}}` | A command or task went on running beyond its call (`running` true), or ended (false). `id` is the handle the agent knows it by; for a command left running by a turn it is the `id` of its `Tool`. The agent is still at work while any runs, though the turn is over. |
| `{"Halted": null}`            | The turn was stopped or failed: nothing the agent had running goes on. A `Finished` or `SignedOut` follows.              |
| `{"Plan": [PlanStep]}`        | The agent's plan, whole. Replaces the earlier one.                                                        |
| `{"Ask": Permission}`         | The agent waits for an answer: send `choose` with the `ticket` and the `id` of a choice.                  |
| `{"Inquire": Inquiry}`      | The agent waits for answers to a form of questions: send `answer` with the `ticket` and a reply for each question. |
| `{"Answered": [[question, answer]]}` | A client answered a form of questions. `answer` is null for a question that was skipped. |
| `{"Finished": null}`          | The turn ended. A string instead of null means it failed with that reason.                                |
| `{"SignedOut": {"reason", "account"}}` | The turn ended because the session account needs signing in again. Send `sign_in_session`; do not append the reason as an assistant reply. |
| `"SignedIn"` | The session account was signed in again. Clear its sign-in notice. |
| `{"Failed": "reason"}`        | The agent could not be started or stopped. The next `prompt` starts it again.                             |
| `{"Container": "step"}`       | The dev container of the session is coming up: what it is doing now. Cleared when the agent starts.      |
| `{"Titled": "text"}`          | The agent named the session, or named it again. The server keeps it as the session's `title`.            |
| `{"Hooks": [Hook]}`           | The hooks the session's Codex holds back until the reader trusts them, all of them as they stand now. Replaces the earlier list; empty when none is held back. Told when a Codex session starts and again when a file the hooks are read from changes. Trust them with `trust_hooks`. |
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

- **Hook:** `{key, event, matcher, command, source, hash, changed}`. `key` is
  what Codex keeps the hook's trust under, `event` the event it runs on as Codex
  names it (`preToolUse`), `matcher` what the event must match (possibly null),
  `command` what it runs, `source` the file it is read from, `hash` what Codex
  makes of it as it stands, and `changed` whether it was trusted before and has
  changed since rather than new.
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

## Tasks

A task is what a project can be told to do: build, test, run. It is a value, so
that the command palette, the gutter and the debugger share one way of running
one.

```json
{ "name": "cargo test", "command": "cargo test", "folder": "", "env": {}, "group": "test", "language": "rust", "source": "Cargo.toml" }
```

- `name` is what the task is called, and what a debug scenario names the task it
  needs first by. `command` is typed into a shell. `folder` is where it runs,
  from the root of the project, and may not leave it. `env` is added to the
  shell's environment. `group` is `build`, `test`, `run` or empty. `language` is
  the family of the toolchain it runs with (`rust`, `javascript`, `python`,
  `go`) or empty. `source` is where it was found. Only `name` and `command` are
  required when a client sends one. `tags` are the tags of the places of a file
  the task is run from; a task of the project with a tag is run from a place
  with it in place of the tasks the languages ship for the tag.
- `tasks` lists them: those written in `.pandemonium.tasks.json` in the root of
  the folder, `{"tasks": [{"name", "command", …}]}`, then those its manifests
  imply, which the `tasks.json` of each installed language makes of them:
  Cargo (`build`, `check`, `test`, `clippy`, `run`), the scripts of a
  `package.json` run with the package manager its lockfile names (a Next.js
  app lists its `dev`, `build`, `start` and `lint` scripts), the targets of
  a Makefile, Go modules and Python projects. A detected task a written one has
  the name of is left out, and so is a written task that reads a variable of a
  place in a file.
- `run_task` starts the user's shell in the task's folder with the task's
  environment and the toolchain chosen for its language, types the command into
  it, and answers `terminal` as `open_terminal` does: the connection is attached
  and the shell is one the server keeps, listed by `terminals`, until it exits or
  is closed. `project` is the project the folder belongs to, which the toolchain
  choice is kept for; the repository the folder is a worktree of when it is
  missing, and the folder itself when it is not in one.
- `run_runnable` does the same for a place a language marks as runnable in the
  file `path`, such as a `main`, a test function or a service of a compose
  file. The client sends what its copy of the language's `runnables.scm` found:
  the place's `tags` (`kind` is the one tag of a client that sends none, as it was when kinds were fixed),
  what each of its captures matched as `variables` (`name` is the one
  `name` variable of a client that sends none) and its `position`, a line and a
  column from zero. The server runs the first task tied to any of the tags: the
  project's own task with a tag, when it has one, or else a template of an
  installed language's `tasks.json`; `task` names another of those tied to the
  place. The command has the variables of the file, the place and its captures
  put in, and its shell is started with them in its environment as
  `PM_CUSTOM_<name>` and the `PM_` variables of the extensions document. The
  nearest folder above the file that holds a template's `marker` file is the
  folder the command runs in.
- `runnable_tasks` lists the tasks that can run a place, as `run_runnable` would pick them, so that a client can let the reader choose one; the name of the chosen one is `run_runnable`'s `task`.
- `toolchains` lists what is installed on the server's machine, and the virtual
  environments the project keeps, for each language, in the places the
  `toolchain_source` tables of its installed language folders name. `system` is the one the machine's own path finds. `selected`
  marks the one the project's language is set to, which is `system` until one is
  chosen. `select_toolchain` chooses `toolchain` for `language` in `project`, or
  clears the choice when it is missing. The choice is kept by the server and used
  for the tasks it runs, including the one a debug scenario names, and for the
  language servers it starts for the project.
- A debug scenario that names a task, `preLaunchTask` in a `launch.json`, shows it as `task` in `debug_scenarios`. `debug_start`
  runs that task to its end first and fails, with what the task printed last,
  without starting the adapter when the task is not one of the project's or does
  not succeed.

## Debugging

A program is debugged by a debug adapter the server runs beside the folder, over
the Debug Adapter Protocol. The server keeps the session going when no client is
attached, and tells the connections watching it what it looks like after it
moves on. Which adapters there are, and what a file can be debugged as with
nothing written, is the `debug_adapter` and `debug_scenario` tables of the
installed languages' `config.toml` and the `debug_adapters` of the installed
extensions, described in [`docs/extensions.md`](extensions.md).

```json
{ "type": "debug", "debug": 1, "view": { "label": "Python: Current File", "standing": "stopped", "reason": "breakpoint", "threads": [], "thread": 1, "frames": [], "frame": 7, "scopes": [], "variables": [], "watches": [], "watched": [], "lines": [], "line_count": 0, "placed": [] }, "events": [{ "paused": { "id": 7, "name": "main", "path": "/home/me/app/main.py", "line": 11, "column": 0 } }] }
```

- `debug_scenarios` lists what a folder can be debugged as: the configurations
  of `.pandemonium.launch.json`, and `.vscode/launch.json`, and what the file in front can
  be run as without any written. `debug_start` chooses one by its place in that
  list, with `file` the same as it was asked for, and sets `files` as the
  breakpoints, lines counted from zero, before the program runs.
- `debug_attach` joins the process `pid` with the adapter `adapter` names, one of
  those `debug_processes` lists.
- `debug` (message): sent to the connection that started the session, and to
  those that `debug_watch` it, whenever the session moved on. Messages that come
  close together are told once. `view` is everything a window draws: `standing`
  (`starting`, `running`, `stopped` or `ended`), the threads, the stack of the
  thread paused with the frame being looked at, its scopes and the `variables`
  asked about as `[reference, [variable]]`, the watch expressions and their last
  results, the last 500 lines of the console and where the adapter put each
  file's breakpoints. `events` says what the window acts on: `paused` with the
  frame, `start_refused` with the adapter's reason, or `ended`.
- `debug_do` takes `action`: `continue`, `pause`, `step_over`, `step_in`,
  `step_out`, `stop`, `select_frame` with `frame`, `expand` with `reference`,
  `evaluate` with `expression`, `set_watches` with `watches` and
  `set_breakpoints` with `path` and `breakpoints`
  (`[{line, condition, hits, log}]`).
- A session ends when the program does or `stop` is sent; the last message has
  `standing: "ended"` and the server forgets it.
- `debugs` lists the sessions the server holds, for a client that attaches after
  a restart of the window; `debug_watch` subscribes to one and answers its view.

## Extensions

```json
{ "type": "diagnostics", "directory": "~/code/app", "path": "src/main.rs", "version": 4, "diagnostics": [{ "span": { "start": { "line": 3, "character": 8 }, "end": { "line": 3, "character": 12 } }, "severity": "error", "message": "…", "source": "rustc" }] }
{ "type": "extensions_changed", "extensions": [{ "id": "rust", "name": "Rust", "kind": "language", "installed": true, … }] }
{ "type": "folder_changed", "directory": "~/code/app" }
```

- `folder_changed`: a folder the connection watches with `watch_folders`
  changed, named as the connection named it. The window reads the git panel of
  the session shown again when it gets one.

- `diagnostics`: what a language server finds wrong with a file the connection
  opened, replacing what it found before. `severity` is `error`, `warning`,
  `information` or `hint`; `version` is the version of the text it is about,
  when the server says.
- `extensions_changed`: sent to every client when a language or an extension
  was installed, removed or allowed something, or installing one moved on,
  with every extension as `extensions` lists them. A client that uses what
  extensions bring asks for `extension_assets` again.

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
