# AGENTS.md — pandemonium

A GPU-native code editor for conducting agents. Several **projects** live in one
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

- `pm-core` — projects, sessions and worktrees: the model the whole window is
  drawn from. A project is a repository the window holds open; a session is an
  agent, its worktree and its branch. Everything is scoped by project id, from
  the first commit — a session, a buffer, a task or a language server that is
  not attached to a project id is a bug.
- `pm-gfx` — the GPU device, the surface, the glyph atlas and the draw list.
  The one place that talks to wgpu. Callers submit a draw list; they never see
  a queue, an encoder or a bind group.
- `pm-text` — rope storage, syntax trees and the language-server client. Text
  as data; it neither lays out nor draws.
- `pm-vt` — terminal emulation: escape-sequence parser, cell grid, scrollback.
  Drives a plain shell and an agent CLI alike; it knows nothing about either.
- `pm-ui` — the element tree, the layout pass, hit testing, focus and input
  routing, over `pm-gfx`'s draw list. Widgets are extracted from real screens
  as they repeat; there is no widget catalogue built ahead of them.
- `pandemonium` — the binary: the winit event loop, the window, the pane tree
  and the keymap. Wires the layers; implements none of them.

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
- **The layers point one way.** `pm-core` depends on nothing of ours; `pm-ui`
  knows `pm-gfx` and not the binary; the binary knows everyone. A layer never
  reaches back up.
- **One seam, one place.** Creating a session, resolving a project, tearing a
  worktree down: each has exactly one implementation, and every caller — the
  palette, a keybinding, a pane — goes through it.
- Document every item in rustdoc (`///`, `//!` for a module), including private
  ones. Function bodies hold code only; the explanation lives in the name, the
  structure and the doc comment.

## Rendering

`pm-gfx` owns one surface per window and presents one frame per redraw. Text is
shaped once and cached; glyphs are rasterized into a shared atlas and drawn as
instanced quads. A frame is a draw list the UI builds and submits — adding a
second render pass or a bespoke pipeline for one widget is a `pm-gfx` change, not
a caller's.
