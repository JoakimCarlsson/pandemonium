# Architecture

A GPU-native editor for conducting agents. Several **projects** live in one
window; a **session** is an agent working in its own git worktree of one of
them; the window shows both as first-class items: files, worktrees, diffs and
the agent's own conversation, in panes, not in a terminal beside the editor.

**Stack:** Rust 2024 · winit (windowing) · rustle (the declarative UI
framework, drawing through wgpu). Linux first; macOS and Windows follow from winit and wgpu, not
from platform-specific code.

## Client and server

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

## Crate responsibilities

One crate per layer, each depending only on the layers below it.

- **Interface:** rustle composables and rustle widgets themed through
  rustle's theme. `pm-components/src/theme` holds the theme values (roles,
  palette, shipped and extension families, light and dark, oklch) and
  provides them at each window root the way Compose builds a custom design
  system: rustle's `theme(ColorScheme, Typography, Shapes, ..)` carries the
  roles Compose has a slot for, the type scale and the corners, rustle's
  `LOCAL_SCROLLBAR_STYLE` the scrollbar look, and one composition local the
  tokens rustle has no slot for (every role, syntax colours, font stacks).
  Text is set in rustle `TextStyle`s (`theme::text_style`, `code_style`)
  through `select::styled_text`, which is rustle's text composables made
  part of the selectable text they are composed in (`select.rs` holds the
  conversation's selection). Boxes are plain rustle modifiers with theme
  values (`theme::sp`, `theme::color`, `Radius::px`); hover, press and focus
  looks are rustle's `interaction_background` and interactive drawing with
  the theme's `InteractionTones`; buttons, cards, badges and surfaces are
  rustle `surface_custom`/`outlined_card`, check boxes and switches rustle's
  `tri_state_checkbox`/`switch_with`, rules rustle's dividers, all in the
  theme's colours, sizes and corners. Icons are rustle's Phosphor set by name
  (`pm-components/src/icons.rs`), with artwork that arrives at run time
  rasterised there. What floats
  over the screen is rustle popups (`pm-components/src/popups.rs`): popovers
  placed against the box they are declared in, menus and code assists at a
  window point, kept inside the window's frame clear of its shadow band by a
  `PopupPositionProvider`; a press outside one that a press elsewhere closes
  calls its `on_dismiss_request`, which asks the app (`Operations::dismiss_requested`)
  to close what that press closes. Tooltips are rustle's `tooltip_area`
  (`pm-components/src/tooltip.rs`, `Button::tooltip`), shown after a rest on
  the frame clock, at once in snapshots.
  There is no frame report: what the binary needs of the geometry comes from
  the composables' own callbacks into holders the model owns (`Bounds`,
  `Places` for the few areas the app asks about by name, `TextLines`,
  `Selectables`), and what the pointer is over from the controls' hover.
  Scrolling areas are rustle's `vertical_scroll` over a
  `rustle::ScrollState` the model owns (hoisted state), styled with the
  theme's `scrollbar()`, provided as rustle's `LOCAL_SCROLLBAR_STYLE`; rustle handles the wheel and the bar, and the binary
  catches the model up with what the reader scrolled after each event
  (`follow_scrolls`). Where an area was drawn is kept by a `Bounds` the model
  owns, told by the area's `on_globally_positioned`.
  Controls take event lambdas, as composables do (`pm-components/src/events.rs`):
  `Interactive::control(id, on_click)` makes a control a focus target that runs
  `on_click` when clicked, or when Enter or Space reaches it while it holds
  rustle's focus; `control_with` hands the lambda the modifier keys (rows picked
  with Shift or Control), and `control_menu`/`pressable_menu` add the lambda of a
  secondary press (a row's, tab's, field's, shell's or page's menu; other controls
  open the menu of the conversation's text). `Button`, `Tile`, `Checkbox` and
  `Menu` take `on_click`/`on_pick`/`on_context` in their builders. The lambdas
  call the app's state holder, the `Operations` trait the binary implements
  (its `Controller`), provided once at each window's root: `update(change)` runs
  a reducer of the workspace, and most controls hand their press to the
  workspace's reducer with `events::reduce(id)` (`WorkspaceState::after(id)`,
  keyed by the control's `FocusId`); named operations do what only the app can
  (`open_command_palette`, `navigate`, `attach`, `take_listing`,
  `open_project_menu`, `open_git_menu`, `minimize_window`, …). After every
  change the binary does in one place (`App::settle`) what the change asks
  (`WorkspaceState::asked`: the focus to give, the shell taking the keyboard,
  the caret shown again), then saves and sends. Keymap commands call the same
  reducer or operations. What stays keyed by `FocusId` is the views' report of
  the pointer: the press that went down on a control (`Operations::pressed`, for
  fields, handles and the title bar), whether a click counts
  (`Operations::click_lands`: the press began there and dragged nothing), the
  pointer over a control (`hovered`) and drags. A panel's handle, the minimap
  and the numbers of a diff's lines hand their drags over with drag modifiers.
  Project headers, pane tabs and rows of the project's files are
  rustle drag-and-drop sources (`drag_and_drop_source`), and each window's
  root is the drag-and-drop target that tells the app a drag started, where
  the pointer carries it and that it ended (`events::carry_target`). The
  title bar moves the window through its control's press, and strips laid
  over a floating frame's edges resize it (`resize_edges`). Every control carries
  the rustle `test_tag` of its `FocusId`, by which headless tests and
  snapshots find it (`pm-components/src/tagged.rs`). Keyboard focus is
  rustle's: every control is a focus target Tab walks in paint order, and the
  binary keeps the `FocusId` of the control holding it as the domain key its
  logic asks about (`pm-components/src/focus.rs`): a control asks for rustle
  focus while the app keeps focus on it and tells the app when rustle moved
  focus (`on_focus_changed`). The file tree keeps its focused row composed and
  scrolls the next row in when Tab leaves the rows drawn. Each window's root
  hands every key to the app first (`on_preview_key_event`), which matches on
  rustle's `KeyEvent`s and turns them into the values of the crates below
  (`pm-field` edits, vim keystrokes, keymap chords) in `keys.rs`. The screen is hoisted in one
  rustle state (`mutable_state`) that every window reads; pictures are
  rustle's `ImageData`.
  Text inputs (search boxes, filters, the prompt, settings boxes, the commit
  message and the like) are rustle text fields (`basic_text_field_with`,
  `basic_secure_text_field` for keys and tokens) editing a
  `rustle::TextFieldState` the model owns, styled through rustle's parameters
  (text style, caret colour, selection colour, a decorator for the
  placeholder, line limits, a hoisted scroll state). Rustle edits them, keeps
  their undo history, composes input-method text in them and keeps their caret
  in view; the model catches up with what was typed after each event
  (`field_typed`). A field's rustle `Focus` follows the binary's focus
  (`focus::follow_focus`), and the window root's key handler sees its keys
  first, so the app's shortcuts, Enter and the clipboard keep working in it. `pm-field`'s `Field` stays the editing model of the code
  editor only (several carets, column selection, the editor's undo grouping,
  vim's programmatic edits): it is drawn line by line with rustle's
  `marked_text`, each line telling the state's `TextLines` its shaped layout
  and place (`on_text_layout`, `on_placed`), from which the binary finds the
  byte under the pointer, moves by visual line and places the input method's
  candidates. The code editor, the shell and the browser page hold rustle
  focus while the binary's focus is on them (`focus::keyboard_focus`), so the
  window keeps its input method on; the shell's cursor cell reports where
  candidates go with `ime_caret`.
- **Components:** `pm-components`: reusable controls (`Button`, `Chip`,
  `Popover`, `Checkbox`, `Tally` and so on), each a builder with a variant and a
  size, and the state and sections the workspace is drawn from.
- **Pages:** `pm-pages`: the workspace, its panes and auxiliary windows. A page
  is never a component and never lives in `pm-components`.
- **Protocol:** `pm-protocol`: the messages between clients and a server, as
  serde types over JSON. Append-only: add fields and variants, never remove or
  repurpose them.
- **Server:** `pm-server`: the `pandemonium-server` program. Owns sessions,
  agents, worktrees and uploaded files; writes `server.json` (port and token,
  readable by the reader alone) under the data folder. Offers agents tool
  servers on the loopback address: the browser's tools and the tools that start
  sessions (`docs/agent-sessions.md`).
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
  the last commit, formatting edits and modal editing; every rule returns the edits it calls for and
  `pm-components` draws it) and `pm-vim` (vim's keys: a binding table, motions,
  operators, text objects, registers, macros and the command line, carried out
  on a copy of the text that `pm-editor` hands in before a key and compares
  with after it; the window keeps undo and the settings). A language is a
  folder in `languages/`: a `config.toml` manifest and its tree-sitter queries,
  among them `runnables.scm`, which marks the places the gutter puts a run
  button beside, and `tasks.json`, which says what running each place and
  listing a project comes to; docs/extensions.md describes every file.
  `make languages` writes `languages/languages.json` with every file's checksum
  after a folder changes; a release build reads the folders from the repository
  at its own release tag, a debug build from its checkout, and either from
  `PANDEMONIUM_LANGUAGES` when it is set.
- **Domain:** `pm-harness` (the app's side of the harness: one agent per provider the harness lists,
  the conversation held with one over the harness's wire protocol, and its
  sign-in, and the answers to the harness's language requests, given from the
  server's own language hub so that the files an agent writes are formatted
  and checked by the servers already running; the harness is a library from the `oh-my-crab` repository that runs
  inside the server, one conversation to a thread, and the tools it offers are
  asked of it directly. The harness owns each conversation: its
  turns, title and chosen settings are saved by it, and the server reads them
  back from it. The server keeps only what is its own, such as
  worktrees, files kept per turn and queued prompts),
  `pm-agent-library` (the skills, instructions, agent definitions and tools
  each agent is given, as the crab harness finds them through `omc-context`,
  and which of them the reader turned off), `pm-mcp` (a client
  of tool servers, which lists the tools they offer), `pm-dap`
  (programs debugged through debug adapters over the Debug Adapter Protocol:
  the adapters, what a folder can be debugged as, and a running session),
  `pm-task` (what a project can be built, tested and run with: the task
  templates the languages ship in `tasks.json` made of its manifests and of the
  places of its files, its own file, and the toolchains installed on the
  machine),
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
- **Not built yet** (one doc comment each): `pm-session`, `pm-review`.

## Code hosts

The pull request bar works through `pm-git`, which keeps what every code host
shares apart from what differs between them:

- `pull_requests.rs` holds the shared workflow: drafting a title and body from
  the session's plan, the branch's commits and the repository's template,
  checking the branch is safe to act on, pushing, deleting the remote branch,
  and `CodeHost`, what a client is told of the host so it can name it.
- `review_host.rs` holds the `ReviewHost` trait, the `HOSTS` list and
  `detect`, which takes the publishing remote's address (`origin`, or the first
  remote) apart and asks each host whether it recognizes it. A remote no host
  recognizes is unsupported: the bar then offers only Push, and every other
  action fails with "this code host is not supported yet".
- `github/review.rs` is the one implementation today: GitHub on github.com,
  `*.ghe.com` and self-hosted GitHub Enterprise servers, through `gh`.

To add a host, such as GitLab or Azure DevOps:

1. Write a module, such as `gitlab/review.rs`, with a unit struct that
   implements `ReviewHost`: its `kind` (the stable name on the wire), `name`
   and `review_noun` (`merge request`, which the bar shows), `recognizes` for
   its remotes, `templates` for where it keeps description templates, `status`
   mapping its states to `OPEN`, `CLOSED` and `MERGED` and its checks and
   approvals to `CheckState` and `ReviewDecision`, the actions (`create`,
   `update_body`, `mark_ready`, `merge`, `reopen`), and `new_review_url` for its
   web page when its program is missing. `delete_remote_branch` has a default
   that pushes the deletion with `git`.
2. Add it to `HOSTS` in `review_host.rs`.
3. Give it tests beside it, like `github/review/tests.rs`.

Nothing in the protocol, the server or the window changes: they carry
`PullRequestStatus` and `CodeHost` whatever the host.
