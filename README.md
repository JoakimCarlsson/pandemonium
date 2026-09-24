# pandemonium

Yet another text editor you don't need.

There are already plenty of good ones. This one exists because I wanted an
editor shaped exactly like the way I work, and it is quite opinionated about
it. If your workflow looks like mine, you might like it. If it doesn't, Zed,
Neovim and VS Code are all excellent and considerably more finished.

## How I work, and therefore how this works

I rarely write code alone anymore. Most days I have a few agents running at
once, each in its own git worktree, across a couple of repositories, and my job
is less typing and more reading what they did, steering them, and deciding
what gets merged. Every editor I tried treated that as a terminal bolted to the
side of the real work. pandemonium treats it as the real work.

- **Several projects, one window.** Repositories are peers in the same window,
  each with its own branch, file tree, language servers and tasks. Panes, tabs
  and the palette span all of them.
- **A session is a worktree you review.** Starting an agent cuts a fresh
  worktree and branch. Its files, its diff and its conversation are browsable
  the same way your own working copy is. You review a session, you don't
  babysit a terminal.
- **Agents speak ACP.** Claude Code, Codex, Gemini, GitHub Copilot and Cursor
  are driven over the [Agent Client Protocol](https://agentclientprotocol.com),
  confined to their own worktree, asking permission through the editor.
- **Everything is a pane.** Files, diffs, agent sessions and terminals are the
  same kind of thing: splittable, tabbable, closable. No bespoke docks.
- **Vim, properly.** Modal editing modelled on Zed's vim mode, driven by a
  binding table: motions, text objects, operators, registers, `.`, macros,
  visual block, multi-cursor and the ex command line.
- **Source control that you can actually read.** A Source Control sidebar, a
  lane-coloured commit graph, and review diffs highlighted with tree-sitter and
  semantic tokens.
- **A debugger in the bottom panel.** Breakpoints in the gutter, a call
  stack, variables and a console beside Problems and Terminal, over the
  Debug Adapter Protocol with gdb, lldb-dap, debugpy or delve. Your existing
  `.vscode/launch.json` or `.zed/debug.json` is read as it is; F5 starts or
  continues.
- **GPU-native.** Rendered with wgpu, windowed with winit, with a UI layer
  written from scratch. No web view, no Electron, no UI framework.

## Status

Early, moving fast, and built for an audience of one. Things will break,
defaults reflect my preferences, and features appear in the order I need them.
Linux is the daily driver; macOS and Windows build in CI and should work, but
get less love.

## Installing

Prebuilt binaries for Linux, macOS and Windows are attached to each
[release](https://github.com/JoakimCarlsson/pandemonium/releases).

```sh
curl -fsSL https://raw.githubusercontent.com/JoakimCarlsson/pandemonium/main/install.sh | sh
```

```powershell
irm https://raw.githubusercontent.com/JoakimCarlsson/pandemonium/main/install.ps1 | iex
```

Set `PANDEMONIUM_VERSION` to pin a release. The script checks the download
against the release's `SHA256SUMS`, puts the binary in `~/.local/bin` and, on
Linux, adds a launcher entry.

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
make fmt     # cargo fmt --all
make lint    # cargo clippy --workspace --all-targets -- -D warnings
make release VERSION=0.2.0   # bump, tag and push; CI builds and publishes
```

## Layout

One crate per layer, each depending only on the ones below it.

| Crate         | What it is                                                              |
| ------------- | ----------------------------------------------------------------------- |
| `pm-core`     | Projects, sessions and worktrees: the domain, knowing nothing of a UI. |
| `pm-acp`      | Agents over the Agent Client Protocol: the process and its protocol.    |
| `pm-gfx`      | GPU device, surface, glyph atlas and draw list. The only wgpu code.     |
| `pm-text`     | Rope storage, syntax trees and the language-server client.              |
| `pm-vt`       | Terminal emulation: parser, cell grid, scrollback.                      |
| `pm-vim`      | Modal editing over a `pm-text` buffer.                                  |
| `pm-ui`       | Element tree, layout, hit testing, focus and input routing.             |
| `pandemonium` | The binary: event loop, window, pane tree and keymap.                   |

The rules the codebase lives by are in [`AGENTS.md`](AGENTS.md).

## Contributing

Issues, ideas and pull requests are all very welcome. It started as a personal
tool, so if a change heads somewhere I hadn't planned, let's talk it through in
an issue first. And if you'd rather take it in your own direction, forks are
more than welcome too.

## License

[MIT](LICENSE)
