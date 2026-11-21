# Architecture

A GPU-native editor for conducting agents. Several **projects** live in one
window; a **session** is an agent working in its own git worktree of one of
them; the window shows both as first-class items: files, worktrees, diffs and
the agent's own conversation, in panes, not in a terminal beside the editor.

**Stack:** Rust 2024 · rustle (the declarative UI framework, which owns the
windows and the event loop, through winit, and draws through wgpu). Linux first;
macOS and Windows follow from rustle and wgpu, not from platform-specific code.

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
  rustle's theme. Colours are `rustle::Color`, whose maths (Oklab `lerp`,
  `copy`, `composite_over`, colour spaces) lives in rustle as Compose's
  `Color` has it. `pm-components/src/theme` holds only the theme values
  (roles, shipped and extension families, light and dark) and provides
  them at each window root the way Compose builds a custom design system:
  rustle's `theme(ColorScheme, Typography, Shapes, ..)` carries the
  roles Compose has a slot for, the type scale and the corners, rustle's
  `LOCAL_SCROLLBAR_STYLE` the scrollbar look, and one composition local the
  tokens rustle has no slot for (every role, syntax colours, font stacks).
  Views call rustle's rows, columns and text composables directly with theme
  values. The theme's own modules hold the rest of the design system:
  `theme/typography.rs` the text styles of interface text and code, and
  `theme/decoration.rs` the shadows and the `Frame` modifiers for outlined and
  panelled boxes. The button is `button.rs`; `parts.rs` holds the small pieces
  screens share (a line of text, a glyph, a check box, an empty list's note,
  a panel's top bar), `icons.rs` the icons and `events.rs` the event lambdas
  controls hand the app.
  Text is set in rustle `TextStyle`s (`theme::text_style`, `code_style`)
  with rustle's text composables; the conversation is selectable through
  rustle's `selection_container`, and the file tree, diff, conversation and
  code view are rustle lazy lists. Boxes are plain rustle modifiers with theme
  values (`theme::sp`, `theme::color`, `Radius::px`); hover, press and focus
  looks are rustle indications: the theme's `InteractionTones`, `FocusRing`
  and `FocusOutline` are `IndicationNodeFactory`s drawn from a remembered
  `MutableInteractionSource` that `theme::Indicated::indicated` feeds with
  hovers, primary presses and rustle's focus; shadows are
  rustle `Shadow` layers cast with `drop_shadow_with` (`theme::box_shadow`,
  `ShadowLayers`); spinners turn on an angle of rustle's
  `remember_infinite_transition`; buttons, cards, badges and surfaces are
  rustle `surface_custom`/`outlined_card`, check boxes and switches rustle's
  `tri_state_checkbox`/`switch_with`, rules rustle's dividers, all in the
  theme's colours, sizes and corners. Icons are rustle's Phosphor constants
  (`rustle::phosphor::X`, `phosphor::fill::X`) used directly, drawn by the
  helpers in `pm-components/src/icons.rs`, which also holds the few panel
  drawings made from Phosphor's own and rasterises artwork that arrives at
  run time. Opened `.svg` files, diagrams and formulas are drawn into pixels
  by rustle's `SvgPainter` (`pandemonium/src/work/picture.rs`), whose text is
  set with the interface's font database and whose embedded images are drawn. What floats
  over the screen is rustle popups (`pm-components/src/popups.rs`): popovers
  and the quick input placed against the box they are declared in, menus at a
  window point or hanging from the control that opened them as a dropdown
  menu does, and the code assists hanging from the line they are about,
  each placed by a `PopupPositionProvider` and kept inside the window's frame
  clear of its shadow band. Each popup closes itself: menus and popovers are
  focusable, so Escape or a press outside calls their own
  `on_dismiss_request`, and the boxes typed in over the code close on a press
  outside that goes on to what it landed on (`Dismissal`). Tooltips are rustle's `tooltip_area`
  (`pm-components/src/tooltip.rs`, `Button::tooltip`), shown after a rest on
  the frame clock, at once in snapshots.
  The screen is one rustle state the window roots read, so a change composes
  the root again. The regions of the workspace (title bar, sidebar, each pane,
  the dock, the plan and review panels) are `pm_components::revision::region`
  blocks keyed by the screen's revision and the room they are given; dragging
  the handle between panels changes sizes only (`Model::resize`), which leaves
  the revision, so only the regions whose room changed are composed again.
  Spinners read their angle while they are drawn, so they turn without
  composing (see `docs/performance.md`).
  There is no frame report: what the binary needs of the geometry comes from
  the composables' own callbacks into holders the model owns (`Bounds`,
  `Places` for the few areas the app asks about by name, `TextLines`,
  `Selectables`), and what the pointer is over from the controls' hover.
  `Places` keeps where an area is drawn (`bounds::placed`, its
  `on_globally_positioned`) and the size it is laid out at (`bounds::sized`,
  its `on_size_changed`): the shell and the browser's page and developer tools
  are sized from the latter, and the app takes window coordinates into the
  frame's from where the main window's frame told it is laid out
  (`WindowEvent::Frame`), not from the shadow band's width.
  The workspace is a row of the sidebar, the page and the plan and review
  panels, each panel at the width it was dragged to and the page taking the
  rest by weight; the handles between them are `Sash::between` boxes in that
  row (and in the column of the panes and the dock), no thicker than the edge
  they lie on, out of which the handle reaches to both sides. Panes are laid
  out by a custom layout (`Split` in `pm-pages/src/panes.rs`) that gives the
  first side its share of the split, the line between the sides and the
  handle on it, and the second side the rest; the highlight of where a carried
  tab would land is drawn inside the pane it is over. The browser stacks its
  strip, toolbar and bars over a row of the page, taking the rest by weight,
  and the developer tools, kept at their dragged width by a layout modifier;
  the find bar and the certificate page are laid out for the room they are
  given with `with_constraints`. Only the widths the views need to compose
  their content (each pane's, the plan panel's) are still worked out from the
  state, since content composed while measuring cannot borrow the screen.
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
  (its `Controller`), provided once at each window's root: `send(event)` hands
  an `events::Event` to the workspace's reducer, `WorkspaceState::handle`, and
  most controls send the event of their area, such as `Event::Sidebar(SidebarEvent::Session(index))` (`pm-components/src/areas.rs`), with `events::send`; the control is
  named by its typed `Target` (`pm-components/src/target.rs`). Each control is
  named once, as a variant of its area's enum in `areas.rs`; a `Target` wraps
  that variant in its area, such as `Target::Composer(ComposerEvent::Prompt)` or
  `Target::Sidebar(SidebarEvent::Session(index))`, so an event and its target
  are the same value under the same area (`Event::pressing` and each area's
  handler only rewrap it before the `pressed` reducer matches the nested
  target); fields send `Event::Field`
  (typed, changed, left, edited); `update(change)` runs any other change of the
  workspace; named operations do what only the app can
  (`open_command_palette`, `navigate`, `attach`, `take_listing`,
  `open_project_menu`, `open_git_menu`, `minimize_window`, …). After every
  change the binary does in one place (`App::settle`) what the change asks
  (`WorkspaceState::asked`: the focus to give, the shell taking the keyboard,
  the caret shown again), then saves and sends. Keymap commands call the same
  reducer or operations. What stays keyed by `Target` is the views' report of
  the pointer: the press that went down on a control (`Operations::pressed`, for
  fields, handles and the title bar), whether a click counts
  (`Operations::click_lands`: the press began there and dragged nothing), the
  pointer over a control (`hovered`) and drags. A panel's handle is
  `draggable_with` along its axis (`events::sash_drag`), the drag state adding
  up the distance since the drag began, and the minimap hands its press and
  drag over with `on_drag`, mapped through the height it is drawn at.
  Project headers, pane tabs and rows of the project's files are
  rustle drag-and-drop sources (`drag_and_drop_source`), and each window's
  root is the drag-and-drop target that tells the app a drag started, where
  the pointer carries it and that it ended (`events::carry_target`); the
  payload names the control carried all the way, and the drag itself lives
  in the screen's state (the project, tab or rows dragged), which the app
  reads instead of keeping presses of its own. The number of a diff's line
  is a drag-and-drop source and target of its own (`events::comment_drag`):
  a drag from it starts the lines for a comment, the numbers it comes over
  stretch them and its end opens the box, while a click comments on that
  line alone. Selected code is carried the same way
  (`code_pointing::text_drag`): the code editor's text is the source, picking
  the selection up as the drag starts from a press on it, and the target
  marking where it would land and putting it down as the drag ends. The
  title bar moves the window through its control's press, and strips laid
  over a floating frame's edges resize it (`resize_edges`). Every control carries
  the rustle `test_tag` of its `Target` (`Target::tag`), by which headless tests and
  snapshots find it (`pm-components/src/tagged.rs`). Keyboard focus is
  rustle's alone: every control is a focus target Tab walks in paint order,
  holding the rustle `Focus` of its `Target` from a registry the binary owns
  and provides to every window (`focus::FocusRequesters`, as a Compose screen
  holds its `FocusRequester`s, in `pm-components/src/focus.rs`). The binary
  asks which control holds focus by reading those focuses, asks for focus with
  `request_focus` and lets it go with `free_focus`; views draw focus from the
  same state. The file tree keeps its focused row composed and
  scrolls the next row in when Tab leaves the rows drawn. Each window's root
  hands every key to the app first (`on_preview_key_event`), which matches on
  rustle's `KeyEvent`s and turns them into the values of the crates below
  (`pm-editor` edits, vim keystrokes, keymap chords) in `keys.rs`. The screen is hoisted in one
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
  (`field_typed`). A field holds its control's rustle `Focus` from the
  registry (`focus::focus_of`), and the window root's key handler sees its keys
  first, so the app's shortcuts, Enter and the clipboard keep working in it. `pm-editor`'s `Field` is the editing model of the code
  editor only (several carets, column selection, the editor's undo grouping,
  vim's programmatic edits): it is drawn line by line with rustle's
  `marked_text`, each line telling the state's `TextLines` its shaped layout
  and place (`on_text_layout`, `on_placed`), from which the binary finds the
  byte under the pointer, moves by visual line and places the input method's
  candidates. The code editor, the shell and the browser page are focusables
  holding their control's rustle focus (`focus::keyboard_focus`), so the
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
  servers and syntax trees count them, the change an edit made, the word
  around a place and how much a press of one, two or three clicks takes hold
  of, which the code editor, the terminal and selections share),
  `pm-treesitter` (grammars compiled to WebAssembly and loaded at run time;
  highlighting with injections, indentation, bracket pairs and the outline from
  each language's queries; parsed in the window) and `pm-lsp` (the language
  index, installing languages and their servers, and the hub that runs language
  servers for the files clients have open; the server uses it), and `pm-editor`
  (an open file being edited: its text with several carets, selections and
  undo history as a `Field` changed by `Edit`s, its syntax tree and language server version kept
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
- **Binary:** `pandemonium`: the root composable of the windows, the app's
  handling of what happens in them and the threads that talk to the servers. It
  wires the layers and implements none of them.

## The windows and the app

The binary has no event loop of its own and does not depend on winit. `main`
makes the `App` and hands it to `app::run`, which calls `rustle::run_with` with
the main window's options and a root composable (`app/host.rs`). The root shows
the main window and opens the settings window and the folder picker's with
`rustle::window` while the screen says they are open; their `on_close_request`
closes what they show, and `rustle::set_close_handler` asks the app before the
main window closes.

What the app does with the windows goes through rustle's API:

- **Pointer.** Each control handles its own gestures with rustle modifiers in
  the view that draws it (`pm-components`, `pm-pages`) and tells the app what
  happened through a typed operation (`pm_components::pointing`): a control's
  press, click and secondary press with where they were and how many presses in
  a row rustle counted; the byte of code text or the cell of the shell under the
  pointer as it presses, drags, rests and turns the wheel; what a web page's
  window hands on to the page; the place a carried control or files are over.
  The bottom layer of each window's root takes only the presses nothing above
  took and the mouse's back and forward buttons. The first operation of an
  event notes what the keyboard holds and wakes the loop, so `App::on_wake`
  catches up with the event (`app/controller.rs`).
- **Keys and input methods.** The root's preview key handler hands every key to
  the app first. The code editor, the shell and a web page take text through a
  `PlatformTextInputSession` started by `pm_components::focus::keyboard_focus`:
  what the input method composes and commits goes to the app, which also says
  where the editor's caret is for the candidate window.
- **The window.** Its focus, size and modifier keys come from
  `LOCAL_WINDOW_INFO`, its scale from `LOCAL_DENSITY` and the desktop's
  appearance from `is_system_in_dark_theme`, each followed by a
  `disposable_effect` keyed on the value. Maximizing, minimizing, moving,
  resizing, bringing forward and asking for attention go through
  `WindowHandle` (`app/port.rs`); the frame's shadow is a `WindowShadow`.
- **Waking.** The app's work and the servers' connections wake the loop with
  `rustle::external_waker`, and `rustle::set_external_handler` runs
  `App::on_wake`, which only drains what woke it: the work's results and the
  outbox, an event the interface finished handling, what rustle scrolled by
  itself, and a frame drawn, which `rustle::set_frame_observer` reports.
- **Idle work.** Everything else is an effect of the main window's root
  (`idle_effects` in `app/host.rs`), each keyed on the state it follows: what
  follows a change of the screen (settings written, the outbox delivered, the
  view's save put off, the place recorded, the windows beside the main one
  followed) on the screen's generation; the card about a word, the highlight
  and marker requests, autosave, recording keys and the view's save each as a
  `launched_effect` keyed on what it waits on (the word under the pointer, the
  file, version and caret, the chords recorded, the screen's generation), so a
  change starts its wait again, waiting with rustle's `delay` on the frame
  clock, which sets a timer and draws nothing meanwhile, and then acting when
  it still waits on the same thing; the agents' work looked at, the browser's
  pages pumped and newer releases looked for at their intervals; the main
  window closed once the app quits; and the view saved when the root leaves
  the composition. The app says what each waits on after each turn
  (`App::publish_idle`) in one rustle state those effects read, and keeps no
  deadline of its own, so the headless rig sees each wait fire by moving the
  frame clock on.

The interaction tests and benchmarks (`app/interactions`) run the same views
headless through `rustle::Embedded` (`app/headless.rs`): the rig feeds each
window's embedded interface pointer, key, wheel, input-method and file events,
the roots hand the app its part as in a window, and the rig wakes the app as
the external handler does, with stand-ins recording what was asked of the
windows.
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
