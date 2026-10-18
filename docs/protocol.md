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

A project on another machine is addressed `ssh://<destination>/<folder>` for
a server reached over ssh, and `tcp://<host>:<port>/<folder>` for one reached
directly; the window keeps the token of each direct server in its own
settings.

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
| `close_session`| `session`, `discard`?                                                                           | `done`          |
| `rename_session`| `session`, `title`                                                                              | `done`          |
| `set`          | `session`, `setting`, `value`                                                                    | `done`          |
| `attach`       | `session`, `after`                                                                               | `done`          |
| `inspect`      | `directory`, `base`                                                                              | `inspected`     |
| `projects`     | none                                                                                             | `projects`      |
| `save_projects`| `paths`                                                                                          | `done`          |
| `choices`      | none                                                                                             | `choices`       |
| `save_choices` | `choices`: `[[agent, setting, value]]`                                                           | `done`          |
| `agent_servers`| none                                                                                             | `agent_servers` |
| `save_agent_servers`| `servers`: `[{id, name, command, env, package, installer, home}]`, `env`: `[[name, value]]`                                | `done`          |
| `mcp_servers`  | none                                                                                             | `mcp_servers`   |
| `save_mcp_servers`| `servers`: `[{name, target, env, description, website, enabled}]`, `env`: `[[name, value]]`                                     | `done`          |
| `watch_folders`| `directories`: `[string]`                                                                       | `done`          |
| `machine_settings`| none                                                                                          | `machine_settings` |
| `save_machine_settings`| `settings`: `{git, agents, shell, languages}`                                            | `done`          |
| `latest_release`| `channel`: `stable` or `nightly`                                                                 | `latest_release` |
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
| `rename_entry` | `directory`, `from`, `to`                                                                        | `done`          |
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
| `debug_scenarios`| `directory`, `file` (optional)                                                                 | `debug_scenarios` |
| `debug_processes`| none                                                                                           | `debug_processes` |
| `debug_start`  | `directory`, `file` (optional), `scenario`, `files`: `[{path, breakpoints}]`                     | `debug`         |
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
| `install_extension` | `path`                                                                                      | `done`          |
| `grammar`      | `language`                                                                                       | `grammar`       |
| `open_document`| `directory`, `path`, `language`, `version`, `text`                                               | `opened`        |
| `change_document` | `directory`, `path`, `version`, `changes`: `[{span, text}]`                                   | `done`          |
| `close_document` | `directory`, `path`                                                                            | `done`          |
| `completion`   | `directory`, `path`, `version`, `position`, `trigger` (optional)                                 | `completions`   |
| `hover`        | `directory`, `path`, `version`, `position`                                                       | `hovered`       |
| `format`       | `directory`, `path`, `version`, `range` (optional)                                               | `formatted`     |
| `format_on_type` | `directory`, `path`, `version`, `position`, `character`                                        | `formatted`     |
| `folding_ranges` | `directory`, `path`, `version`                                                                 | `folding_ranges` |
| `linked_ranges` | `directory`, `path`, `version`, `position`                                                      | `linked_ranges` |
| `semantic_tokens` | `directory`, `path`, `version`                                                                | `semantic_tokens` |
| `inlay_hints`  | `directory`, `path`, `version`, `span`                                                           | `inlay_hints`   |
| `code_lenses`  | `directory`, `path`, `version`                                                                   | `code_lenses`   |
| `press_code_lens` | `directory`, `path`, `index`                                                                  | `lens_pressed`  |
| `definition`   | `directory`, `path`, `version`, `position`, `seek` (optional)                                    | `definitions`   |
| `workspace_symbols` | `directory`, `path`, `query`                                                                | `definitions`   |
| `search_files` | `directory`, `query`, `regex` (optional), `case` (optional)                                      | `definitions`   |
| `problems`     | `directory`                                                                                      | `problems`      |
| `blame`        | `directory`, `path`, `line`                                                                      | `blamed`        |
| `highlights`   | `directory`, `path`, `version`, `position`                                                       | `highlights`    |
| `signature`    | `directory`, `path`, `version`, `position`                                                       | `signature`     |
| `rename`       | `directory`, `path`, `version`, `position`, `name`                                               | `edited`        |
| `code_actions` | `directory`, `path`, `version`, `span`                                                           | `code_actions`  |
| `take_code_action` | `directory`, `path`, `index`                                                                 | `edited`        |
| `file_history` | `directory`, `path`                                                                              | `file_history`  |
| `permalink`    | `directory`, `path`, `line` (optional)                                                           | `address`       |

`agent` is an agent's id: `claude-code`, `codex` or `grok`.

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
  `settings` is a list of `[setting, value]` pairs applied before the first
  prompt; when it names no `mode` and `agents.permission_mode` is set, that mode
  is asked for too. The response gives the session number, the folder the agent works in
  and, for a worktree, the abbreviated commit it has checked out (`head`). The connection is attached to the session already.
- **`close_session`:** stops the session's agent, drops its queued prompts and marks it `closed`. The
  log is kept, so the session still shows in `sessions` (with `closed: true`)
  and can be opened with `attach`. A session that has a worktree of its own
  has the worktree removed, unless another open session works in the same
  folder; while the worktree has changes that are not committed the request
  fails, unless `discard` is `true`. A later `prompt` opens a closed session
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
- **`agent_servers`, `mcp_servers`:** the agents the server can start and
  the tool servers every agent is started with, kept by the server and replaced
  whole by the matching `save_` request. The agents the app ships are in the list the first time it is asked for, and are changed or removed like any other; `package`, `installer` and `home` are optional. A tool server's `target` is an address
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
  branch_template, worktree_root}`: while `auto_fetch` is set the server runs
  `git fetch --all --prune` every `fetch_minutes` in the repositories of its
  open sessions and of the projects saved on its machine. `agents` is
  `{default_agent, permission_mode}`, each optional: the agent a client offers
  first for new work, by id, and the mode new sessions start in. `shell` is
  `{program, arguments}`, the shell `open_terminal` starts when `program` is not
  empty. `languages` maps a language's id to `{tab_size, hard_tabs,
  format_on_save, server_off, server_command}`: the first three are for clients
  to edit with and are optional; `server_off` keeps the language server from
  reading the language, and a `server_command` is started in place of the one
  the language comes with. Files already open move to the new server at once.
  The window sends the settings the server on its own machine keeps to every
  other server it reaches, as it reaches it and whenever they change, so a
  project on another machine works by the same settings.
- **`latest_release`:** the newest release of the app on `channel`, looked up
  on the project's releases: the latest made for everyone for `stable`, the
  build made every night for `nightly`.
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
- **`create_entry`:** makes an empty file, or a folder when `folder`, at
  `path` (from the root of `directory`) and the folders above it; a name that
  is taken is refused.
- **`rename_entry`:** renames or moves `from` to `to`, both from the root of
  `directory`; a name that is taken is refused.
- **`copy_entry`:** copies `from` into the folder `into` (empty for the root of
  `directory`), under `name copy.ext`, `name copy 2.ext` and so on when the name
  is taken.
- **`remove_entry`:** takes `path` and everything in it off the disk. Every
  path of these four must stay inside `directory`.
- **`write_file`:** replaces the contents of `path` (from the root of
  `directory`, which must stay inside it) with `text`, making the file when it is
  not there. A folder is refused.
- **`read_file`:** reads `path` from the working folder or, when `head`, from
  the last commit, or from the commit `revision` names by its object name. The response holds no text for a file that is not text, and
  names the installed language the file is written in, by the endings and names
  of files its manifest lists.
- **`extensions`:** every extension the list offers, then those
  installed on the server's machine that it does not, each as `{id, name,
  kind, version, description, icon, color, hidden, suffixes, installed, busy,
  problem, server, local}`. `kind` is what the extension is; `language` is the
  only kind so far. `icon` is the SVG an installed language's files are shown
  with and `color` its tint as `#rrggbb`; a client shows a file of no installed
  language as a plain file. `hidden` languages are
  installed only as part of another and left out of what a reader picks from;
  `busy` says what installing it is doing now, `problem` why installing it or
  its language server last failed, and `server` what the machine has of its
  language server, in words. The list is `languages.json` read from
  `https://raw.githubusercontent.com/JoakimCarlsson/pandemonium/<tag>/languages`,
  where the tag is the server's own release (`v<version>`) or `nightly`; a debug
  build reads the `languages` folder of its checkout instead, and
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
- **`install_extension`:** copies the files of the folder at `path` on the
  server's machine into `local-languages`, under the folder's name and in place
  of one of the same name, and installs it as `install_extensions` does. Fails
  when the folder holds no `config.toml` that reads as a language, the only kind
  of extension so far.
- **`remove_extension`:** takes the language off the machine; its language
  server stays, for other languages that use it.
- **`grammar`:** the grammar and queries of an installed language, found by its
  folder name or a name a code fence calls it, as `{id, symbol, aliases, wasm,
  highlights, indents, outline, injections, brackets, indent, tabs,
  line_comment, pairs}`. `wasm` is the tree-sitter grammar compiled to
  WebAssembly, in base64; a client parses and highlights with it itself.
  `pairs` lists what the editor closes as it is opened, each the opening and
  the closing character, such as `"()"`.
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
  ask its language server. `completions` holds `[{label, detail, kind, text,
  span, filter, sort, documentation}]`, the best match for the word before
  `position` first, with snippets made plain text; `span`, when given, is what
  choosing it replaces, and `documentation` is markdown when the server sent
  it with the list. `hovered` holds `{text, span}` in markdown, or null.
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
  does not say.
- **`inlay_hints`:** what to show inside the lines of the open file at
  `version` within `span`, as the language server says; none when it does not
  say.
- **`code_lenses`:** what to show with stretches of the open file at
  `version`, as the language server says, each already filled in by the
  server. The server keeps the list as given, and `press_code_lens` takes one
  by its place in it.
- **`press_code_lens`:** presses one of the lenses last given for the open
  file: the places it points to when it counts uses, and otherwise the
  changes the language server made carrying out its command, as for
  `take_code_action`.
- **`definition`:** the places `seek` looks for from `position`: `definition`
  (the default), `declaration`, `type_definition`, `implementation`,
  `references`, `incoming_calls`, `outgoing_calls`, `supertypes` or `subtypes`. A file inside the folder
  is named by its path from the root; one outside it by its absolute path, with
  the whole text it holds for the first four, to be shown read only.
- **`rename`, `take_code_action`:** carry out an edit across the folder. Files
  no connection has open are changed on disk by the server; the changes to the
  open ones come back in `edited` for the client to make, each a stretch of
  the file as it is before any of them. `code_actions` lists what the language
  server offers for `span`, and `take_code_action` takes one by its place in
  the latest list for that file.
- **`search_files`:** the lines of the files of the folder's repository,
  tracked or not but not ignored, that hold `query`, as typed or as a regular
  expression when `regex`, matching case only when `case`; as `definitions`
  with `line` holding each line, at most 500 of them.
- **`workspace_symbols`:** the symbols of the folder whose names match
  `query`, as the language server of the open file `path` finds them, those
  defined inside the folder only, as `definitions` with `name` naming each.

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
| `files`      | `paths`: repository-relative, tracked or not ignored, `folders` (optional): untracked folders, which may hold no file yet |
| `branches`   | `names`: local branches, the current one first, then remote ones such as `origin/main`; read from git at each request |
| `stashes`    | `stashes`: `[{index, message, when}]`, newest first |
| `log`        | `commits`: `[{id, refs, summary, lanes}]`, newest first |
| `file`       | `text`: string or null for a file that is not text, `truncated`: whether it was cut short, `language` (optional): the installed language it is written in |
| `bytes`      | `bytes` (base64): what a file holds, `truncated`: whether it was cut short |
| `identity`   | `default`, `selected`: who the agent is signed in as, or null |
| `started`    | `session`, `directory`, `branch` (always null for now), `head` (null without a worktree) |
| `inspected`  | `changes`: `[{path, diff, staged, untracked, index, worktree, conflicted, from}]`, `commits`: `[string]` or null, `branch`: string or null, `head`: string or null, `tracking`: `{upstream, ahead, behind}`, `operation`: `Merge`, `Rebase` or `CherryPick` with what git prepared, or null |
| `projects`   | `paths`, `saved`                                          |
| `choices`    | `choices`: `[[agent, setting, value]]`                    |
| `agent_servers` | `servers`: `[{id, name, command, env, package, installer, home}]`                |
| `mcp_servers` | `servers`: `[{name, target, env, description, website, enabled}]`                     |
| `machine_settings` | `settings`: `{git, agents, shell, languages}`, as `save_machine_settings` takes them |
| `latest_release` | `version`: the version, or for `nightly` the day it was made, or null when none was found; `url`: its page |
| `registry_agents` | `agents`: `[{id, name, version, description, website, command, env}]` |
| `tool_listings` | `listings`: `[{id, title, description, name, target, env, inputs, asks, website}]` |
| `accounts`   | `logins`: `[{agent, id, name}]`, `in_use`: `[[agent, id]]` |
| `sessions`   | `sessions`: `[SessionInfo]`                               |
| `terminal`   | `terminal`: the number of the shell that was started      |
| `terminals`  | `terminals`: `[{terminal, directory, columns, rows}]`, oldest first; `directory` is the address `open_terminal` was given |
| `debug_scenarios` | `scenarios`: `[{label, adapter, installed, attach, source}]`, the order `debug_start` chooses by |
| `debug_processes` | `adapters`: `[{id, name}]` installed, `processes`: `[{pid, name, command}]`, newest first |
| `debug`      | `debug`: the number of the session, `view`: the [view](#debugging) of it now |
| `debugs`     | `debugs`: `[{debug, directory, label}]`, oldest first      |
| `owners`     | `owners`: `[{login, organization}]`                       |
| `repositories` | `repositories`: `[{name, description, private, url}]`   |
| `folders`    | `path`: absolute, `names`: the folders inside it           |
| `directory`  | `path`: a `clone` made   |
| `extensions` | `extensions`: `[{id, name, kind, version, description, icon, color, hidden, suffixes, installed, busy, problem, server, local}]` |
| `grammar`    | `{id, symbol, aliases, wasm, highlights, indents, outline, injections, brackets, indent, tabs, line_comment}` |
| `opened`     | `triggers`: the characters after which the server offers completions; `signature`: those after which it tells what the call the caret is in looks like; `format`: those after which it formats what was typed; `formats_ranges`, `folds`, `links`, `semantic_tokens`, `inlay_hints`, `code_lens`: whether it formats a stretch, says what can be folded, says what is edited together, says what each name is, says what to show inside lines and says what to show with stretches |
| `completions`| `items`: `[{label, detail, kind, text, span, filter, sort, documentation}]`, `kind` one of `function`, `variable`, `type`, `module`, `keyword`, `constant`, `snippet`, `other` |
| `hovered`    | `hover`: `{text, span}` or null |
| `formatted`  | `edits`: `[{span, text}]` |
| `folding_ranges` | `ranges`: `[{start, end}]`, the first and last line of each block, counted from 0 |
| `linked_ranges` | `spans`: the stretches edited together |
| `semantic_tokens` | `tokens`: `[{line, start, length, kind}]`, `start` and `length` in UTF-16 code units, `kind` one of `keyword`, `function`, `type`, `string`, `constant`, `comment`, `property`, `operator`, `variable` |
| `inlay_hints` | `hints`: `[{position, label, kind, pad_left, pad_right}]`, `kind` one of `type`, `parameter`, `other` |
| `code_lenses` | `lenses`: `[{span, title, pressable}]` |
| `lens_pressed` | `places`: as `locations` of `definitions`; `files`: as of `edited` |
| `definitions`| `locations`: `[{path, span, inside, text, language, name, line}]`; `name` names a calling or called function, `line` is the line the span starts on |
| `code_actions` | `actions`: `[{title, kind, preferred, disabled}]` |
| `highlights` | `spans`: the stretches of the file where the thing at `position` is used |
| `blamed`     | `blame`: `{id, author, when, summary}`, who last changed the line as the file is on disk; `id` is empty for a line not committed, `when` seconds since the Unix epoch |
| `problems`   | `problems`: `[{path, diagnostic}]`, what the language servers last said is wrong with each file of the folder, open or not, the worst first |
| `signature`  | `signature`: `{label, active, documentation, others}` or null; `active` is the byte range of `label` naming the parameter the place is at, `others` how many more signatures there are |
| `edited`     | `files`: `[{path, changes: [{span, text}]}]` |
| `file_history` | `commits`: `[{object, id, author, when, summary, path}]`, newest first, following the file back through moves; `path` is what it was called in that commit |
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
base, title, closed, updated, last_seq, running}`. `updated` is milliseconds since the epoch,
`last_seq` is the number of the session's latest event, and `running` says
whether its agent is running now.

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

## Debugging

A program is debugged by a debug adapter the server runs beside the folder, over
the Debug Adapter Protocol. The server keeps the session going when no client is
attached, and tells the connections watching it what it looks like after it
moves on.

```json
{ "type": "debug", "debug": 1, "view": { "label": "Python: Current File", "standing": "stopped", "reason": "breakpoint", "threads": [], "thread": 1, "frames": [], "frame": 7, "scopes": [], "variables": [], "watches": [], "watched": [], "lines": [], "line_count": 0, "placed": [] }, "events": [{ "paused": { "id": 7, "name": "main", "path": "/home/me/app/main.py", "line": 11, "column": 0 } }] }
```

- `debug_scenarios` lists what a folder can be debugged as: the configurations
  of `.pandemonium.launch.json`, `.vscode/launch.json` and `.zed/debug.json`, and what the file in front can
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
- `extensions_changed`: sent to every client when a language was installed or
  removed, or installing one moved on, with every extension as `extensions`
  lists them.

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
