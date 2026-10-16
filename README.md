# pandemonium

A GPU-native editor for conducting agents.

I rarely write code alone anymore. Most days I have a few agents running at
once, each in its own git worktree, across a couple of repositories, and my job
is less typing and more reading what they did, steering them, and deciding
what gets merged. Every editor I tried treated that as a terminal bolted to the
side of the real work. pandemonium treats it as the real work.

## Status

Early, being rewritten from scratch, and built for an audience of one. Most of
what is described below is the direction, not what runs today. Expect things to
break and features to appear in the order I need them. Linux is the daily
driver; macOS and Windows build and should work, but get less love.

## How it works

- **Several projects, one window.** Projects are peers in the same window, each
  with its own branch, file tree and sessions, and each on one machine.
- **A session is a worktree you review.** Starting an agent cuts a fresh
  worktree and branch. Its files, its diff and its conversation are shown in
  panes, not in a terminal beside the editor.
- **Agents speak ACP.** They are driven over the
  [Agent Client Protocol](https://agentclientprotocol.com) and confined to
  their own worktree.
- **A server owns the agents.** `pandemonium-server` starts the agents, makes
  their worktrees and runs git. The window is a client of it, so an agent keeps
  working when the window is closed, the laptop sleeps or the connection drops,
  and a client that reconnects catches up on what it missed.
- **Remote from the first commit.** The window reaches a server on this machine
  or on another one, over a direct connection on a trusted network such as
  Tailscale or through an ssh tunnel. A project added as `ssh://host/folder`
  lives on `host`. A phone is only another client.
- **GPU-native.** Rendered with wgpu, windowed with winit, with a UI layer
  written from scratch. No web view, no Electron, no UI framework.

The protocol between clients and a server is in [docs/protocol.md](docs/protocol.md).

## Agents and accounts

Any agent that speaks ACP can be added under Settings, Agents, Agent Servers:
a name and the command that starts it in protocol mode. Claude Code, Codex and
Grok Build are in the list from the start and can be edited or removed like any
other. Model and mode pickers, slash commands and sign-in come from what the
agent reports over ACP, so they need nothing from pandemonium.

Every agent can keep more than one login. An agent that has a variable for its
login folder, such as `CLAUDE_CONFIG_DIR` or `CODEX_HOME`, is started with that
variable pointing at the account's folder; the add form takes it as an optional
box. Any other agent is started with `HOME` set to a folder of its own for the
account. That folder links to everything in your home folder except the agent's
own folders, which are found by matching words from its name and command, so
the agent starts signed out while git, ssh and npm keep working. Skills, rules
and agents from the agent's own folder are linked in.

Limits:

- Showing who an agent is signed in as, and sharing a new account's setup,
  work only for the agents whose files pandemonium knows: Claude Code, Codex,
  Grok Build and, for sharing only, Copilot. Other agents show "Account N".
- An agent that keeps its config under a name unrelated to its own name will
  see your normal login folder. Give it its login folder variable in the add
  form.
- Separate accounts for agents without a login folder variable work on Linux
  and macOS only. On Windows they need `USERPROFILE` and `AppData` instead of
  `HOME`, junctions instead of symlinks and a Windows terminal for logins that
  need one, none of which is built. There the agents run, but every account
  shares the one login.
- Logins that need a terminal look for a Linux terminal emulator, so they do
  not open on macOS or Windows.

## Languages

Nothing language-specific ships in the binary. You pick languages on first
launch, or later under Settings, Languages, and the server installs each one
on the machine that holds the project: its tree-sitter grammar, its queries
and its language server. A file of an installed language gets syntax
highlighting, indentation, bracket pairs and an outline in the window, and
completion, diagnostics and hover from its language server.

A language is a folder in [`languages/`](languages): a `config.toml` manifest
and five queries (`highlights`, `indents`, `outline`, `injections`,
`brackets`). The manifest pins the grammar and says how the language server is
found and installed: a program you already have is used first, in the project
(`.venv/bin`, `node_modules/.bin`) and then on your path; otherwise it is
downloaded, built with `go install` or installed with `npm ci`, into a folder
per version. `languages/languages.json` lists the folders with a checksum for
every file, every download is checked against the checksum it is pinned to,
an npm install is pinned by the lockfile in the language's folder and a Go one
by Go's checksum database. A
build reads the list from this repository at its own release tag; point
`PANDEMONIUM_LANGUAGES` at a folder or address to read it from elsewhere, and
run `make languages` after changing a folder.

## Installing

Prebuilt binaries for Linux, macOS and Windows are attached to each
[release](https://github.com/JoakimCarlsson/pandemonium/releases).

On the machine you sit at, install the window. It starts a server of its own
on this machine when none is running, apart from itself so that it outlives it.

```sh
curl -fsSL https://raw.githubusercontent.com/JoakimCarlsson/pandemonium/main/install.sh | sh
```

```powershell
irm https://raw.githubusercontent.com/JoakimCarlsson/pandemonium/main/install.ps1 | iex
```

On a machine that should run agents while you are away, install only the
headless server, then start it with `pandemonium-server --listen ADDRESS`.

```sh
curl -fsSL https://raw.githubusercontent.com/JoakimCarlsson/pandemonium/main/install.sh | PANDEMONIUM_COMPONENT=server sh
```

Set `PANDEMONIUM_VERSION` to pin a release. The scripts check the download
against the release's `SHA256SUMS` and put the binary in `~/.local/bin`. The
window also adds a launcher entry on Linux or `Pandemonium.app` in
`~/Applications` on macOS.

### Nightly

Pre-releases and the nightly build are a separate channel, Pandemonium
Nightly. It installs beside the stable app as `pandemonium-nightly`, with its
own launcher entry, window class, server and data folder, so it never replaces
or reads from the stable one. Without `PANDEMONIUM_VERSION` it installs the
latest nightly build.

```sh
curl -fsSL https://raw.githubusercontent.com/JoakimCarlsson/pandemonium/main/install.sh | PANDEMONIUM_CHANNEL=nightly sh
```

```powershell
$env:PANDEMONIUM_CHANNEL = "nightly"; irm https://raw.githubusercontent.com/JoakimCarlsson/pandemonium/main/install.ps1 | iex
```

## Building

You need a recent stable Rust toolchain (edition 2024) and a GPU that wgpu is
happy with. Agents are fetched with `npx` on first run, so Node helps too.

```sh
git clone https://github.com/JoakimCarlsson/pandemonium.git
cd pandemonium
make run
```

Other targets:

```sh
make fmt                     # cargo fmt --all
make lint                    # cargo clippy --workspace --all-targets -- -D warnings
make install                 # build this checkout and install it as pandemonium-preview
make install-server          # build and install only pandemonium-server
make release VERSION=0.2.0   # bump, tag and push; CI builds and publishes
```

`pandemonium --snapshot <path>` draws one frame to a PNG without opening a
window; see `crates/pandemonium/src/snapshot.rs`.

## Layout

One crate per layer, each depending only on the layers below it. The window
implements none of them; it wires them together.

| Layer      | Crates                                                                                                  |
| ---------- | ------------------------------------------------------------------------------------------------------- |
| Styling    | `pm-primitives`, `pm-color`, `pm-layout`, `pm-typography`, `pm-decoration`, `pm-animation`, `pm-style`  |
| Drawing    | `pm-shaders`, `pm-renderer` (the only wgpu code), `pm-text`, `pm-icon`, `pm-theme`                      |
| Interface  | `pm-input`, `pm-state`, `pm-ui`                                                                         |
| Components | `pm-components`: reusable controls and the sections the workspace is drawn from                         |
| Pages      | `pm-pages`: onboarding, the workspace and the screen that switches between them                         |
| Protocol   | `pm-protocol`: the messages between clients and a server                                                |
| Server     | `pm-server`: the `pandemonium-server` program                                                           |
| Client     | `pm-client` (one reconnecting connection, and starting the local server), `pm-ssh` (ssh transport)      |
| Code       | `pm-buffer` (text and places), `pm-treesitter` (grammars, highlighting), `pm-lsp` (languages, language servers) |
| Domain     | `pm-acp` (agents over ACP), `pm-git`, `pm-store` (persistence)                                          |
| Binary     | `pandemonium`: the event loop, the window and the threads that talk to the servers                      |

Crates for later work, each holding only a doc comment so far: `pm-session`,
`pm-review`, `pm-terminal`, `pm-config`, `pm-markdown`, `pm-mcp`, `pm-policy`,
`pm-plugin`, `pm-plugin-api`, `pm-wasm` and `pm-agent-library`.

## License

MIT. See [LICENSE](LICENSE).
