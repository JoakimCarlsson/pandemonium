# AGENTS.md — pandemonium

A GPU-native editor for conducting agents. Several **projects** live in one
window; a **session** is an agent working in its own git worktree of one of
them; the window shows both as first-class items: files, worktrees, diffs and
the agent's own conversation, in panes, not in a terminal beside the editor.

**Stack:** Rust 2024 · winit (windowing) · wgpu (rendering) · no UI framework:
`pm-ui` is ours. Linux first; macOS and Windows follow from winit and wgpu, not
from platform-specific code.

## Commands

```sh
make run     # cargo run -p pandemonium
make fmt     # cargo fmt --all
make lint    # cargo clippy --workspace --all-targets -- -D warnings
make install        # build and install the window as pandemonium-preview
make install-server # build and install only the headless pandemonium-server
make release VERSION=x.y.z  # bump, tag and push; CI builds and publishes
make languages      # write languages/languages.json after a language folder changes
make extension-sample # build the sample extension into build/extensions/hello
```

**Verify a change:** build, then draw the screen you touched. `pandemonium
--snapshot <path> [elapsed_ms [x y [focus [light|dark [screen]]]]]` writes one
frame to a PNG without a window; add a `Shot` in `crates/pandemonium/src/snapshot.rs`
for a state that has none. A change to rendering is verified on screen, never
by reasoning about the draw list. Build output goes in the git-ignored `build/`.
Every section of the settings window has a shot, `settings-<section>`, and
`settings-<section>-narrow` draws it at the window's smallest size, 640 by 420.

## Architecture

A **server owns the agents.** `pandemonium-server` runs on a machine, starts
the agents there, makes their worktrees, runs git, and keeps the sessions going
when no client is attached. Every client, the window included, is a client of
it: it connects over a WebSocket, sends `Request`s, and is told what the agents
do through events numbered per session. A client that reconnects asks for the
events after the last number it has. The window starts a server on its own
machine when none is running, apart from itself so that it outlives the window.

The protocol is the same whichever way the client reaches the server: a direct
TCP connection (a trusted network such as Tailscale), an ssh `-W` tunnel to a
server that is already running (nothing is installed or started over ssh), or,
later, an end-to-end encrypted relay for phones. A phone app later is only
another client. The protocol is documented in `docs/protocol.md`; a change to
`pm-protocol` changes that document in the same commit.

## Layout

One crate per layer, each depending only on the layers below it.

- **Styling:** `pm-primitives`, `pm-color`, `pm-layout`, `pm-typography`,
  `pm-decoration`, `pm-animation` and `pm-style`: small utility values a style
  is made of, and the one public styling API on top of them.
- **Drawing:** `pm-shaders` (WGSL), `pm-renderer` (the only crate that talks to
  wgpu; callers submit a draw list), `pm-text` (shaping and text fields),
  `pm-icon`, `pm-theme`.
- **Interface:** `pm-input` (pointer, keyboard, focus), `pm-state`, `pm-ui` (the
  element tree, layout pass, hit testing and painting into the draw list).
- **Components:** `pm-components`: reusable controls (`Button`, `Chip`,
  `Popover`, `Checkbox`, `Tally` and so on), each a builder with a variant and a
  size, and the state and sections the workspace is drawn from.
- **Pages:** `pm-pages`: onboarding, the workspace and the screen that switches
  between them. A page is never a component and never lives in `pm-components`.
- **Protocol:** `pm-protocol`: the messages between clients and a server, as
  serde types over JSON. Append-only: add fields and variants, never remove or
  repurpose them.
- **Server:** `pm-server`: the `pandemonium-server` program. Owns sessions,
  agents, worktrees and uploaded files; writes `server.json` (port and token,
  readable by the reader alone) under the data folder.
- **Client:** `pm-client`: one connection to a server that is dialled again when
  it drops, plus finding and starting the server on this machine.
- **Transport:** `pm-ssh`: reaches a server on another machine through ssh.
  `pm-transport`: the sibling transports, a distribution of Windows Subsystem
  for Linux, a dev container and a tunnel that opens no port, and the bridge
  that carries a local port through a program's standard streams; the first two
  start the server themselves.
- **Terminal:** `pm-terminal`: the emulator a client keeps for a shell the server
  runs: escape sequences parsed into a grid of cells with scrollback and
  selection. `pm-components` draws it as the terminal dock.
- **Markdown:** `pm-markdown`: markdown parsed into blocks of styled spans;
  `pm-components` lays them out as `Markdown`.
- **Code:** `pm-buffer` (a text with its lines indexed, places as language
  servers and syntax trees count them, and the change an edit made),
  `pm-treesitter` (grammars compiled to WebAssembly and loaded at run time;
  highlighting with injections, indentation, bracket pairs and the outline from
  each language's queries; parsed in the window) and `pm-lsp` (the language
  index, installing languages and their servers, and the hub that runs language
  servers for the files clients have open; the server uses it), and `pm-editor`
  (an open file being edited: its syntax tree and language server version kept
  in step, completions, indentation as the language says, pairs, whole-line
  edits, several carets, find and replace, folding, how its lines differ from
  the last commit, formatting edits and jumps back and forward; every rule
  returns the edits it calls for and `pm-components` draws it). A language is a
  folder in `languages/`: a `config.toml` manifest and its tree-sitter queries,
  among them `runnables.scm`, which marks the places the gutter puts a run
  button beside.
  `make languages` writes `languages/languages.json` with every file's checksum
  after a folder changes; a release build reads the folders from the repository
  at its own release tag, a debug build from its checkout, and either from
  `PANDEMONIUM_LANGUAGES` when it is set.
- **Domain:** `pm-acp` (agents over the Agent Client Protocol), `pm-dap`
  (programs debugged through debug adapters over the Debug Adapter Protocol:
  the adapters, what a folder can be debugged as, and a running session),
  `pm-task` (what a project can be built, tested and run with: tasks read from
  its manifests and its own file, and the toolchains installed on the machine),
  `pm-git`,
  `pm-store` (persistence under the data directory). The server uses them; the
  window does not.
- **Tools:** `pm-tools`: finds a program on the path and in the places tools install
  themselves (nvm, Homebrew, go, cargo), for a window that was not started from a shell.
- **Settings:** `pm-config`: what the window keeps of its own (how it opens,
  looks and draws shells and git marks, what it does when an agent finishes and
  the servers it knows), read from and written to one file. What a server acts
  on is `pm_protocol::MachineSettings`, kept by each server; the settings window
  edits both, every setting a row of `pm-components`' catalogue.
- **Extensions:** `pm-policy` (the capabilities an extension may hold, denied
  until the reader grants them), `pm-plugin-api` (the versioned, append-only
  contract between the editor and an extension's module), `pm-wasm` (the
  sandbox that runs a module under a budget) and `pm-plugin` (the manifest, the
  installed extensions, installing one through the language index and running
  its commands). An extension lives on the server of its machine; its themes,
  icon themes and command names are read by the window. `docs/extensions.md`
  describes the package.
- **Binary:** `pandemonium`: the event loop, the window and the threads that
  talk to the servers. It wires the layers and implements none of them.
- **Not built yet** (one doc comment each): `pm-session`, `pm-review`,
  `pm-mcp`, `pm-agent-library`.

## Hard rules

- **Remote is built with every feature, not after it.** A feature that reads a
  file, runs a process, starts an agent or makes a worktree is a request the
  server carries out, so it works on a remote machine and for a phone from its
  first commit. A feature that only works on this computer is unfinished.
- **The window does no work of its own.** It never starts an agent, runs git or
  reads a project file. State is a value; it queues what must happen in its
  outbox and the binary sends it to the server as a request.
- **An agent outlives its client.** Closing the window, losing a connection or
  putting a phone to sleep never stops an agent.
- **A project is not a window.** Projects are peers inside one window, each with
  its own branch, file tree and sessions, and each on one machine. A project
  added as `ssh://host/folder` lives on `host`.
- **A session is a worktree you review**, not a terminal you supervise.
- **The layers point one way.** A layer never reaches back up. `pm-pages` knows
  `pm-components`; `pm-components` never knows `pm-pages`.
- **One seam, one place.** Creating a session, resolving a project, tearing a
  worktree down: each has exactly one implementation.
- **Extract a pattern when it repeats.** A control defined twice, or reached
  for by a second page, is a component in `pm-components` with a builder, a
  variant and a size, and both callers use it.
- **A crate root is a facade.** `lib.rs` holds module declarations, public
  re-exports and the crate's `//!` doc, nothing else.
- **Files are cut along concerns, not line counts.** Split when a file holds two
  jobs, not when it grows.
- Document every item in rustdoc (`///`, `//!` for a module), private ones
  included. Function bodies hold code only; the explanation lives in the name,
  the structure and the doc comment.
- A rename is done in one pass, every use included. Dependencies are added with
  `cargo add`, never by hand-editing a manifest.
