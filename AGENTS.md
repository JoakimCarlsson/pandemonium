# AGENTS.md — pandemonium

A GPU-native text editor for conducting agents. Several **projects** live in one
window; a **session** is an agent working in its own git worktree of one of
them; the editor shows both as first-class items — files, worktrees, diffs and
the agent's own CLI, in panes, not in a terminal beside the editor.

**Stack:** Rust 2024 · winit 0.30 (windowing) · wgpu 30 (rendering) · no UI
framework — `pm-ui` is ours. Linux first; macOS and Windows follow from winit
and wgpu, not from platform-specific code.

## Commands

```sh
make run     # cargo run -p pandemonium
make fmt     # cargo fmt --all
make lint    # cargo clippy --workspace --all-targets -- -D warnings
```

**Verify a change:** `make lint` clean, then `make run` and drive the flow you
touched. A change to `pm-gfx` is verified on screen, never by reasoning about
the draw list.

## Layout

One crate per layer, each depending only on the layers below it. `pm-core` is
the domain; nothing in it knows that a UI exists.

- `pm-host` — the bottom layer: local and SSH machine handles, filesystem
  operations, processes, ptys, walks, watches and the byte transport. It
  depends on nothing of ours; every other crate may depend on it.
- `pm-server` — the headless `pandemonium-server` endpoint on a remote host.
  Serves machine operations over standard input and output; knows `pm-host`
  alone and no editor features.
- `pm-core` — projects, sessions and worktrees: the model the whole window is
  drawn from. A project is a repository the window holds open; a session is an
  agent, its worktree and its branch. Everything is scoped by project id, from
  the first commit — a session, a buffer, a task or a language server that is
  not attached to a project id is a bug.
- `pm-acp` — agents over the Agent Client Protocol: which agents there are
  and how each is started, the process in its worktree, the protocol on its
  pipes, and what it says turned into events a window can draw. It speaks to
  one agent at a time and knows nothing of panes, projects or sessions in the
  editor's sense.
- `pm-gfx` — the GPU device, the surface, the glyph atlas and the draw list.
  The one place that talks to wgpu. Callers submit a draw list; they never see
  a queue, an encoder or a bind group.
- `pm-text` — rope storage, syntax trees and the language-server client. Text
  as data; it neither lays out nor draws.
- `pm-dap` — the debug adapter client: gdb, lldb-dap, debugpy and delve
  started beside a worktree, the Debug Adapter Protocol on their pipe or
  socket, and the scenarios a worktree's `.vscode/launch.json` or
  `.zed/debug.json` describe. It borrows `pm-text`'s frame and program
  lookup and knows nothing of panes or gutters.
- `pm-vt` — terminal emulation: escape-sequence parser, cell grid, scrollback.
  Drives a plain shell and an agent CLI alike; it knows nothing about either.
- `pm-vim` — modal editing over a `pm-text` buffer: a binding table written
  like Zed's `vim.json` (keys, action, `when`), and the motions, objects,
  operators, registers, `.`, macros and command line the actions name. It
  takes keystrokes of its own and answers with effects for the window —
  save, split, go to definition; it knows neither winit nor the window.
- `pm-ui` — the element tree, the layout pass, hit testing, focus and input
  routing, over `pm-gfx`'s draw list. Widgets are extracted from real screens
  as they repeat; there is no widget catalogue built ahead of them.
- `pandemonium` — the binary: the winit event loop, the window, the pane tree
  and the keymap. Wires the layers; implements none of them.

### Inside a crate

The same rule one level down: a module per concern, and a root that only says
which of them callers may name.

- **A crate root is a facade.** `lib.rs` holds the module declarations, the
  public re-exports and the crate's `//!` doc. A type declared in a crate root
  is a type that has not been given its module yet.
- **A module directory is its own facade.** `mod.rs` re-exports and holds the
  shape the submodules are variations of; the submodules hold the rest.
- **Files are cut along concerns, not along line counts.** Split when a file
  holds two jobs, not when it grows: eleven files of forty lines each, one per
  function, hide the screen as thoroughly as one file of a thousand. A handful
  of files at the top of a crate is the vocabulary of that layer, not clutter.
- **A screen keeps its parts.** The page, its header and the block it leaves
  behind are one screen and live in one file. A part that a second screen
  reaches for stops being part of the screen: it moves to `pm-ui`'s widgets,
  generic over the message type, and both screens call it there.
- **Names are reserved for the thing they mean.** `settings` is the surface
  that edits preferences, not the code that writes them down; that is
  `config`. Nothing is named for the first feature that happened to need it.

## Hard rules

- **A project is not a window.** Projects are peers inside one window, each with
  its own branch, file tree, language servers and tasks, while panes, tabs and
  the palette span all of them. Anything that assumes a single implicit project
  is wrong even when it works.
- **A session is a worktree you review**, not a terminal you supervise. Its
  files, its diff and its agent are browsable the way the working copy is.
- **Everything is a pane.** Files, diffs, agent sessions and terminals are the
  same kind of item in the same pane tree: splittable, tabbable, closable. No
  bespoke docks, no panel that only one feature can live in.
- **The layers point one way.** `pm-host` depends on nothing of ours and every
  other crate may depend on it; `pm-server`, `pm-core` and `pm-acp` know
  `pm-host` alone; `pm-vim` knows `pm-text`; `pm-dap` knows `pm-text` and
  `pm-host`; `pm-ui` knows `pm-gfx` and
  not the binary; the binary knows everyone. A layer never reaches back up.
- **One seam, one place.** Creating a session, resolving a project, tearing a
  worktree down: each has exactly one implementation, and every caller — the
  palette, a keybinding, a pane — goes through it.
- **State is read from one place and written through one seam.** Preferences
  live in one file under the editor's home, `config` is the only code that
  reads or writes it, and onboarding and a later settings pane are two editors
  of that one file, never two stores.
- Document every item in rustdoc (`///`, `//!` for a module), including private
  ones. Function bodies hold code only; the explanation lives in the name, the
  structure and the doc comment.

## Rendering

`pm-gfx` owns one surface per window and presents one frame per redraw. The
device, the surface and the frame are `renderer`; the instance layouts and the
pipelines that consume them are `pipeline`; a third kind of thing to draw is an
instance struct and a shader there, never a render pass in a caller. Text is
shaped once and cached; glyphs are rasterized into a shared atlas and drawn as
instanced quads. A frame is a draw list the UI builds and submits — adding a
second render pass or a bespoke pipeline for one widget is a `pm-gfx` change, not
a caller's.
