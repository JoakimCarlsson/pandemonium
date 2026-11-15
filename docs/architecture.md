# Architecture

A GPU-native editor for conducting agents. Several **projects** live in one
window; a **session** is an agent working in its own git worktree of one of
them; the window shows both as first-class items: files, worktrees, diffs and
the agent's own conversation, in panes, not in a terminal beside the editor.

**Stack:** Rust 2024 · [Rustle](https://github.com/JoakimCarlsson/rustle)
(windowing, layout, text and drawing, on winit and wgpu). Linux first; macOS
and Windows follow from Rustle, not from platform-specific code.

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

- **Text:** `pm-text` (text fields and editing models). Colours come from
  Rustle's own `Theme`; `pm-components` adds the few tones it lacks.
- **Shell:** `pm-shell`: the link to the server and the models the pages
  draw, with no drawing code.
- **Components:** `pm-components`: reusable Rustle composables (`Button`,
  `Tile`, `Card`, `Composer` and so on), each a builder with a variant and a
  size.
- **Pages:** `pm-pages`: the workspace, its panes and auxiliary windows, built
  only from components. A page is never a component.
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
  its commands). An extension lives on the server of its machine; its icon
  themes and command names are read by the window; colour themes are parsed
  but no longer applied. `docs/extensions.md`
  describes the package.
- **Binary:** `pandemonium`: the event loop, the window and the threads that
  talk to the servers. It wires the layers and implements none of them.
- **Not built yet** (one doc comment each): `pm-session`, `pm-review`.

## Rustle port

The window now runs on Rustle. The old interface framework (`pm-ui`,
`pm-renderer`, `pm-style` and its parts, and the old components and pages) is
deleted; the items below list what the old window did and name the old source
file, which stays readable in git history. `pandemonium --snapshot <png>
[WIDTHxHEIGHT] [dark|light] [live] [welcome|chat|history|extensions|overview]
[update|sign-in] [approve|inquiry|trust|continue] [code|diff]
[git-menu|git-source|git-confirm|branch-menu]
[files|search|file-menu|file-delete]
[quick|quick-commands|quick-sessions|quick-text|quick-theme]
[browser|browser-find|browser-menu|browser-tools|browser-ask|browser-cert|browser-clear|browser-notice|browser-pick]
[problems|tasks|debug] [selection]` draws the window over the sample
workspace, or over the local server's projects with the newest session open,
on the session view or the page named, with the dialog, card, tab, panel,
menu or quick input named open;
`settings-<section>[-narrow]` draws a section of the settings window over
sample settings.

An item is ticked once the Rustle window does it and it was checked in a
snapshot or the running window.

### Window & top bar

- [x] Self-drawn frame: drag to move, double-press to maximize, edge resize, min/max/close (checked on Wayland in a nested KWin) — `title_bar.rs`, `app/frame.rs`
- [x] Window shadow outside the frame (Rustle's `WindowShadow`: Wayland subsurface strips, checked in the running window; X11 `_GTK_FRAME_EXTENTS` with a band the window draws, not yet checked on X11) — `app/shadow*.rs`, `app/wayland.rs`
- [x] Back and forward through the navigation trail — `title_bar.rs`, `trail.rs`
- [x] Command centre field naming the current place — `title_bar.rs`
- [x] Newer-release button and settings button — `title_bar.rs`, `update.rs`
- [x] Reopen on the last page and layout — `view.rs`, `view/writer.rs`
- [x] Resizable panels with sashes and saved sizes (sizes kept in `view.json` and restored on the next start) — `sash.rs`
- [x] Separate windows for settings and the folder picker — `window.rs`, `app/asides.rs`

### Sidebar

- [x] Navigation, chats, projects with nested submodules and their sessions — `sidebar.rs`
- [x] Project avatar: colour and first letter — `avatar.rs`, `project.rs`
- [x] Project menu, drag to reorder, remove project (drag reorders through Rustle's drag source and drop target; the menu also moves up and down) — `arrange.rs`
- [x] Add project: local folder or clone from GitHub — `adder.rs`
- [x] Session ages — `age.rs`
- [x] Unread, working and needs-attention marks — `session_attention.rs`
- [x] Chats without a project — `chats.rs`
- [x] Project overview page — `overview.rs`
- [x] History page — `history.rs`
- [x] Extensions page — `extensions.rs`
- [x] Sample workspace before real projects exist — `sample.rs`

### Session view and conversation

- [x] Conversation of messages, kept scrolled to the end — `chat.rs`
- [x] Markdown: headings, lists, code blocks, quotes, fold-outs, tables, rules — `markdown.rs`
- [x] Mermaid diagrams, TeX and images — `figures.rs`
- [x] Agent steps, tools and thinking inline — `steps.rs`, `hearing.rs`
- [x] Activity chips above the prompt — `activity.rs`
- [x] Plan panel and approve band — `plan.rs`
- [x] Inquiry card — `inquiry.rs`, `draft.rs`
- [x] Trust-project card — `trust.rs`
- [x] Backup-model notice — `fallback.rs`
- [x] Spend strip — `spend.rs`
- [x] Pull request bar and its actions — `pull_request_bar.rs`
- [x] Select and copy conversation text, across blocks — `app/select.rs`
- [x] Message menu — `message_menu.rs`
- [x] Conversation commands: retry, afresh, hand over, side question — `conversation_commands.rs`
- [x] Continue a chat in a project — `continue_chat.rs`
- [x] Welcome page for a new session or chat, with the agent list — `welcome.rs`, `agent_list.rs`
- [x] Start page hands its browser and shells to the new session — `hand_over.rs`
- [x] Desktop notifications and sounds (`finish_notice` decides from the settings, tested) — `notify.rs`
- [x] Session header: title, branch, agent and account — `pm-pages/workspace.rs` (old)

### Composer

- [x] Prompt box that sends to the agent — `composer.rs`, `input.rs`
- [x] Agent setting pickers, slash commands, where work runs — `choices.rs`
- [x] Model picker with `@slot` choices — `slot_picks.rs`
- [x] Context and allowance rings — `usage.rs`
- [x] Queued prompts — `queued.rs`
- [x] Paste files and images — `app/paste.rs`
- [x] Drop files to attach (on Wayland through Rustle's own `wl_data_device`, since winit reports no drops there) — `app/drops.rs`
- [x] Browser element chips (snapshots `browser-pick` and `composer-attached`) — `elements.rs`
- [x] Screenshot and console capture chips (snapshot `composer-attached`) — `captures.rs`
- [x] Text field menu (Cut, Copy, Paste, Select all) — `context_menu.rs`

### Tabs and panes

- [x] Session tabs: Chat, Diff, File, Excerpts, Browser, Update — `session_tabs.rs`, `session_extras.rs`
- [x] Open-file tabs with a preview tab — `open_files.rs`, `tab_chip.rs`
- [x] Split panes and dragging tabs between them — `session.rs`, `open_files.rs`
- [x] Tab menu — `session_tabs.rs`
- [x] Next, previous and reopen closed tab — `session_tabs.rs`, `open_files.rs`

### Code editor

- [x] Viewer and editor with line numbers — `viewer.rs`, `editor.rs`
- [x] Syntax highlighting, syntax-aware edits, auto-indent — `editor.rs`, `work/code.rs`
- [x] Completions and hover card — `code_assist.rs`, `link/lsp.rs`
- [x] Semantic colours, inlay hints, code lenses — `code_editor.rs`, `link/lsp.rs`
- [x] Usage highlights, signature help, change gutter — `code_editor.rs`, `code_assist.rs`
- [x] Lightbulb — `code_editor.rs`, `code_assist.rs`
- [x] Definition, references, rename, code actions — `link/lsp.rs`, `link/listing.rs`
- [x] Organize imports and fix all — `file_view.rs`, `link/file_actions.rs`
- [x] Find and replace — `find.rs`
- [x] Line edits and go to line — `line_edits.rs`
- [x] Minimap — `minimap.rs`
- [x] Sticky scroll — `sticky.rs`
- [x] Scrollbar marks — `scroll_marks.rs`
- [x] Colour picker — `code_assist.rs`
- [x] Drag selected text — `code_editor.rs`
- [x] Right-to-left text — `code_editor.rs`
- [x] Markdown and image preview — `preview.rs`, `work/picture.rs`
- [x] Missing-language offer — `file_view.rs`, `extensions.rs`
- [x] File actions — `file_view.rs`, `link/file_actions.rs`
- [x] Compare with clipboard, saved or another file — `file_view.rs`, `link/file_actions.rs`
- [x] File icons — `file_icon.rs`

### Diff and review

- [x] Side-by-side diff with line notes (press a line number to leave a note; notes are sent to the agent as one prompt) — `diff_view.rs`, `diff_page.rs`
- [x] Syntax-coloured diff with word highlights — `diff_syntax.rs`
- [x] Lazily loaded diffs (the Diff tab lists every change and asks for diffs as files come into view) — `changes_page.rs`, `link/review.rs`
- [x] Commit and branch changes, editable excerpts (from the graph's commit menu and the panel menu's "Changes since") — `changes_page.rs`, `search_panel.rs`
- [x] Merge conflict resolver — `conflict_page.rs`, `review.rs`
- [x] Added and removed tally — `tally.rs`

### Git panel

- [x] Branch, commit box, commit button, graph section — `review.rs`
- [x] Changed files grouped with stage checkboxes — `change_list.rs`
- [x] Multi-select, discard, ignore, open, switch branch — `git_controls.rs`
- [x] Git menus — `git_menus.rs`
- [x] Git confirmations — `git_prompts.rs`
- [x] Branch menu — `branch_menu.rs`
- [x] Branch tasks (publish, rename, delete, merge, rebase, unset upstream; choosing an upstream not yet) — `branch_tasks.rs`
- [x] Commit graph — `graph.rs`
- [x] Live refresh through folder watches — `work/watch.rs`

### Files panel

- [x] File tree with change marks — `file_tree.rs`, `tree.rs`
- [x] Tree actions — `tree_actions.rs`, `app/tree.rs`
- [x] Drag entries and import dropped files — `tree_actions.rs`, `work/import.rs`

### Search

- [x] Project search — `project_search.rs`
- [x] Results as editable excerpts — `search_panel.rs`, `link/excerpts.rs`
- [x] Quick input: files, commands, symbols, outline, text — `listing.rs`, `quick_input.rs`
- [x] Quick session switcher — `quick_sessions.rs`
- [x] Quick choices: appearance, keymap, settings, rewind, branch — `quick_choices.rs`

### Dock

- [x] Terminal with shells per session folder as tabs, and tasks run in tabs of their own — `terminal.rs`, `dock.rs`
- [x] Terminal keys, selection, scrollback — `terminal_keys.rs`
- [x] Terminal menu (Copy, Paste, Select all, Clear on the secondary button) — `terminal_menu.rs`
- [x] Problems tab — `problems_panel.rs`
- [x] Tasks — `tasks.rs`, `pm-pages/tasking.rs` (old)
- [x] Debug tab — `debug_panel.rs`, `debug.rs`
- [x] Breakpoints and inline values (kept by the link, listed in the Debug tab, marked in the editor gutter and shown after the paused line) — `debug.rs`, `inline_values.rs`

### Browser

- [x] Browser pane with address bar and dev tools — `browser.rs`, `app/browser.rs`
- [x] Address bar search — `browser_address.rs`
- [x] Browser tabs per session — `browser_tabs.rs`, `browser_scope.rs`
- [x] Find in page — `browser_find.rs`
- [x] Page menu (the rows that add screenshots and the console to the prompt wait on the composer's attachments) — `browser_menu.rs`
- [x] Clear site data — `browser_clear.rs`
- [x] Permission, certificate and download prompts — `browser_safety.rs`
- [x] Browser shortcuts — `browser_keys.rs`
- [x] Local links open inside — `browser_links.rs`

### Settings

- [x] Settings window: sections, search, reset, narrow layout — `settings.rs`, `settings_row.rs`
- [x] General — `settings_pages.rs`
- [x] Appearance — `settings_pages.rs`
- [x] Editor pages: overview, appearance, editing, saving, intelligence, search, vim — `settings_editor.rs`
- [x] Languages — `settings_languages.rs`
- [x] Keymap — `settings_keymap.rs`
- [x] Terminal, Browser, Git and Worktrees — `settings_pages.rs`
- [x] Agents — `settings_agents.rs`
- [x] Agent servers and MCP servers: sign-in, keys, accounts, the registry and the server form — `settings_servers.rs`
- [x] Model servers, with the form declaring one — `settings_models.rs`
- [x] Model slots: editing a slot, inheriting again, applying a preset — `settings_slots.rs`
- [x] Customizations: skills, instructions, agents, hooks, tools, and new ones from a draft — `settings_library.rs`, `settings_drafts.rs`
- [x] Servers and machines — `settings_machines.rs`

### Dialogs and menus

- [x] Menu, popover and modal — `menu.rs`, `popover.rs`, `modal.rs`
- [x] Update dialog — `update_dialog.rs`
- [x] Sign-in code dialog — `sign_in_code.rs`
- [x] Folder picker — `folder_picker.rs`
- [x] Pick lists for references, actions, history, problems — `link/listing.rs`

### Keyboard and vim

- [x] Keymap (every chord pressed in the window goes through the settings' keymap; actions that work everywhere run on the link, tested) — `keys.rs`
- [x] Vim mode — `vim.rs`, `app/vim.rs`
- [x] Key caps in the quick input — `quick_input.rs`

### Remote projects and machines

- [x] Projects on other machines, reach state, reconnect replay (each machine is reached in the background, its sessions listed under its projects with numbers of the window's own, followed, opened and prompted through its own server; tested against a made-up server) — `reach.rs`, `work.rs`
- [x] Machine settings synced to every server (sent to each server reached when they change and when it connects; tested against a made-up server) — `work/machine.rs`
- [x] Remote folder picking, tasks and debugging (requests naming another machine's folders, shells or debugging sessions go to its server; tested against a made-up server) — `folder_picker.rs`, `work/tasks.rs`

### Other

- [x] Light, dark and system appearance — Rustle's `theme(...)` with Pandemonium's colour schemes and `ExtendedColors`, `app_theme.rs` in `pm-components`
- [x] Snapshot states for each page (`--snapshot`) — `snapshot.rs`
- [x] Frame performance measurement — `app/performance.rs`

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
