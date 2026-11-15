# Rustle port: parity gaps

An audit of the Rustle window (`task/replace-ui`, commit `e1f3503`) against the
old interface at `29b70e6`. The "Rustle port" checklist in
[architecture.md](architecture.md#rustle-port) is fully ticked, but it marks a
feature done once it *renders*, not once it *behaves* as before. This file
lists what is missing, stubbed, or wired to nothing.

**How this was checked.** Each old file in `crates/pm-components`,
`crates/pm-pages` and `crates/pandemonium/src/{app,work,view}*` at `29b70e6`
was read beside its new counterparts in `pm-components`, `pm-pages`,
`pm-shell` and `pandemonium/src/window.rs`. The new code was searched for
no-op closures, catch-all `_ => {}` arms, swallowed `let _ =` results, and
state that is written but never read. The old commit was built in a throwaway
worktree, and matching `--snapshot` states were rendered from both builds at
1280×800 in dark mode, with `HOME` pointed at a scratch folder. Nothing ran on
the desktop and no input was injected.

The renders are kept in `build/audit-shots/` (git-ignored):

| Pair | Shows |
|---|---|
| `old-live` / `new-live` | session view |
| `old-code` / `new-code` | editor |
| `old-diff` / `new-diff` | diff |
| `old-files` / `new-files` | files panel |
| `old-settings-appearance` / `new-settings-appearance` | Appearance settings |
| `icons-zoom.png` | title-bar icons at 4×: old on top, new below |

**References.** `old:` paths are at `29b70e6`. Unprefixed paths are in the
working tree. Line numbers are approximate wherever a subagent read the file
through the GitHub reader.

**Priority.** **P1** means a core feature is broken or missing, or user data or
input is lost. **P2** means a noticeable missing behaviour or visual. **P3** is
polish.

---

## 1. Top 30 by user impact

| # | Gap | Where (new) | Old reference |
|---|---|---|---|
| 1 | **FIXED** (`gap2/window`): every catalog setting now reaches the window, the editor, the terminal or the server; see the table in section 2. **Most settings are ignored.** Appearance (interface font and size, code font and size, line height, density), every Editor page, and Terminal font size, cursor, scrollback and copy-on-select are saved but never read. `window_theme` maps only `theme_mode`. `AppTheme::with_interface_font`, `with_code_font` and `with_density` have no callers. `pm_config::Density` is never converted to `pm_components::Density`. | `pandemonium/src/window.rs:185-191`, `pm-components/src/app_theme.rs:224-275` | `old: pandemonium/src/app.rs:330-355,713-725`, `scene.rs:18-39` |
| 2 | **FIXED** (`gap/panels`): Default and Dusk ship again, extension themes come from `ExtensionAssets`, the Light theme and Dark theme rows are back and the theme quick-pick lists the families. **Theme families are gone.** The Light theme and Dark theme rows were removed from the catalog, the Dusk theme and extension themes are gone, and the theme quick-pick lists only light, dark and system. | `pm-shell/src/settings_catalog.rs:266`, `link/quick_input.rs:123` | `old: settings_catalog.rs:55-58`, `pm-theme/src/builtin.rs:72-169` |
| 3 | **FIXED** (`gap/panels`): Rustle now rasterises outlines into area-coverage masks drawn on whole pixels, and Pandemonium draws glyphs at 12, 14 and 16 px in boxes of even size (title-bar icons 16 px in 28 px buttons). **Icons look pixelated and small.** Rustle tessellates icon paths with no MSAA and a 0.8 px fringe that only fades outward, so sub-pixel Phosphor strokes alias. Origins are not pixel-snapped. Pandemonium sizes glyphs off the 12/14/16 grid (9, 10, 11, 13, 15 px) and centres them on half pixels. The old build rasterised SVGs with resvg into anti-aliased masks at `size × scale`. | rustle `rustle-wgpu/src/path.rs:24,121,182,363`; `pm-components/src/icon_button.rs:88-123`, `checkbox.rs:92-119` | `old: pm-icon/src/svg.rs:56-72`, `pm-renderer/src/renderer.rs:876-889` |
| 4 | **FIXED.** `KeyBindings` in `pm-components/src/key_bindings.rs` hands the keymap to the editor and terminal; verified by unit tests (`own_bindings_reach_the_editor`, `shortcuts_are_caught`) and live: Ctrl+R in a shell reaches bash (reverse-i-search). **The editor and terminal ignore the keymap.** Only window-wide actions go through it. Editor keys are hard-coded in `code_keys`, so keymap presets and custom bindings do nothing in the editor or terminal. | `pm-components/src/code_keys.rs:96-175`, `terminal_keys.rs:102-127` | `old: app/code.rs:410-498,589-1006`, `app/terminal.rs:122-148` |
| 5 | **FIXED.** `link/keys.rs` does every window-wide action and the editor does or hands on the rest (section 3); verified by `window_actions_are_done` and the live Ctrl+Shift+P/Escape check. **About 55 window-wide keymap actions do nothing.** `run_action` ends in `_ => return None`. Affected: tabs (close, close others, pin, next/previous), panes (split, focus), zoom, ToggleTheme, git stash/amend/continue/abort/skip/create branch, agent mode/model/allow/reject, session rename/archive/copy path, Quit, FocusPrompt, and others. | `pm-shell/src/link/keys.rs:123` | `old: app/code.rs:829-1006` |
| 6 | **FIXED.** the root handler is `on_key_event`; verified live: Ctrl+R in a focused shell opened bash's reverse search, not the session picker. **The root key handler takes chords from the terminal and browser.** The keymap runs in `on_preview_key_event`, before the focused child, so Ctrl+R/N/P/T/E never reach a shell, and any binding overrides keys typed in a web page. | `pm-pages/src/workspace.rs:79,155-158` | `old: app.rs:2085-2097`, `app/browser.rs:667-702` |
| 7 | **FIXED.** `Workspace.status` shows in a dismissible status bar and every unhandled `Response::Failed` lands there; verified by `failures_reach_the_status_bar` and the `status` snapshot. **Error messages are never shown.** `Workspace.status` is written by about 25 failure paths (disconnect, start, save, LSP, file tree, listing, browser) and read by nothing. `answer_settings` and other answer handlers drop `Response::Failed` in catch-alls. Prompt, Cancel, Set, Answer, Trust, Rewind, Fork, SaveMcpServers, ModelServers and other failures vanish silently. | `pm-shell/src/model.rs:70`, `link.rs:153,916-919,981` | `old: work.rs:2080-2083,2374,2389` |
| 8 | **FIXED.** failures and halts end the turn and settle running steps; verified by `failures_end_the_turn`. **A failed turn leaves the session stuck.** `Event::Failed` never calls `end_turn`, so the session stays Working. Tool steps are never closed on Finished, Halted or Failed and keep their running mark. | `pm-shell/src/thread.rs:157-160,274-287` | `old: hearing.rs:80-84,252-260` |
| 9 | **FIXED.** **Closing a modified file discards it without asking.** This covers Close, Close others, Close all and Ctrl+W. Unsaved text, carets and open tabs are also no longer saved across restarts. *Closing now asks Save, Don't save or Cancel; open files, their caret lines and unsaved text are kept in `view.json` and reopened once the server lists its sessions.* | `pm-shell/src/open_files.rs:532-562`, `pm-shell/src/view.rs:53-72` | `old: open_files.rs:632-640`, `view.rs:47-65,588-617` |
| 10 | **FIXED.** Close goes through `set_close_handler`, which asks while agents work (confirm_close), shuts the browser and writes the view; verified by `closing_on_working_agents_asks_first`, the `quitting` snapshot, and live: the title-bar Close closed the window. **Closing the window doesn't ask, and the browser isn't shut down.** The title-bar Close calls `process::exit(0)`, and the confirm-close setting is ignored. A desktop close skips `shutdown_browser` and the view flush. | `pandemonium/src/window.rs:536-539` | `old: app/frame.rs:144-155`, `app.rs:2272-2276,2410` |
| 11 | **FIXED.** **Diagnostics never reach the editor.** There are no squiggles, no problem colour on line numbers, no problems in the hover card, no scrollbar marks and no next/previous problem. Problems only lists open files, and pressing a problem row does nothing. *Diagnostics now reach open and later-opened files: squiggles, coloured line numbers, scrollbar marks, F8/Shift+F8, and problem rows open their place. The server still reports only open files.* | `link.rs:798-803`, `code_editor.rs:1631-1825`, `pm-pages/src/problems_panel.rs:32-47` | `old: editor.rs:250-273`, `viewer.rs:257-267`, `dock.rs:253-343` |
| 12 | **PARTLY FIXED.** **Long lines are cut off.** There is no horizontal scroll and no word wrap in the editor or the diff (compare `new-code.png` and `new-diff.png`). The old build wrapped them. *The editor now scrolls across (Shift+wheel, following the caret) and wraps at the edge or the line length as Soft wrap says; the diff is still clipped.* | `code_editor.rs:1426-1458`, `diff_view.rs` | `old: viewer.rs:826-860`, `diff.rs` |
| 13 | **FIXED.** the dock asks `Terminals` on first connect, adopts the shells and replays them with `watch_terminal`; the client re-attaches on reconnect. Verified by `shells_of_the_server_are_taken_up`. **Shells are lost on reconnect or restart.** `Terminals` and `AttachTerminal` are never sent, `watch_terminal` is never called, shells are never adopted or closed, and missed output is never caught up. This breaks the AGENTS.md reconnect rule. | `pm-shell/src/dock.rs:136-141` | `old: dock.rs:509-520`, `work.rs:2121-2143` |
| 14 | **FIXED.** `link/remote.rs` passes every message on and `machines.rs` renumbers browser calls, diagnostics, folder changes, served pages and continued chats; unreachable machines' sessions show an out-of-reach mark. Verified by build and the existing remote tests. **Remote machines lose most server messages.** `link/remote.rs` passes on only six message kinds. BrowserCall (a remote agent's browser tool hangs), Diagnostics, FolderChanged, ExtensionsChanged and ChatContinued are dropped. Remote sessions also show no "out of reach" state; they keep spinning as Working. | `pm-shell/src/link/remote.rs:202`, `model.rs` (`SessionStatus`) | `old: work.rs:1752-1843`, `reach.rs:12-32` |
| 15 | **FIXED.** the clone form has a folder and machine, the machines form a token field, and the library's tool sets and withheld keys go to the open project's machine; verified by the `clone` and `settings-servers` snapshots. **Customizations, clone and the machines form ignore remote machines.** The library and `SaveWithheld` always go to the local server. Clone has no destination or machine. The machines form has no token field, so token-protected servers can't be reached. | `link.rs:359`, `settings_change.rs:110-119,240-274`, `link/projects.rs:80-98` | `old: settings_library.rs:137-174`, `settings_machines.rs:243-250`, `adder.rs:540-604` |
| 16 | **PARTLY FIXED** (`gap/conversation`): the sidebar's session and chat menus and the message right-click menu are back, and `Tile` has `on_menu`; the editor and text-field menus remain. **Context menus are missing.** The editor menu (18 commands), the text-field menu outside the composer, the session and chat menus in the sidebar (rename, close, copy path or branch, mark unread, open folder, remove chat), and the right-click menu on chat messages are all gone. `Tile` has no secondary-click hook. | `code_editor.rs`, `text_input.rs`, `pm-components/src/tile.rs:206-284` | `old: context_menu.rs:156-336`, `arrange.rs:17-36,142-204`, `message_menu.rs:45-102` |
| 17 | **FIXED.** the session menu in the sidebar renames, closes (asking before discarding uncommitted changes), copies and opens folders; forks open at once. Verified by `closing_with_changes_asks_first`, `forks_open_at_once`, `renaming_changes_the_title` and the `session-menu`, `rename`, `discard` snapshots. **Sessions can't be closed or renamed.** `CloseSession` and `RenameSession` are never sent. A fork made in this window doesn't appear until a reconnect, because the `Started` reply is dropped (`Ask::Nothing`). | `pm-pages/src/actions.rs`, `link/conversation_requests.rs:161` | `old: work.rs:819-852,998` |
| 18 | **PARTLY FIXED.** `IconButton::tooltip` exists and the title bar uses it; tooltips and menus are kept inside the window (`pm-components/src/window_area.rs`, `tip.rs`), verified live: Close's tooltip stays inside the right edge and the last project's menu opens upward. Tooltips elsewhere are still missing. **Tooltips are almost all gone.** The new code has one `tooltip(` call (`usage_ring.rs:94`), and `IconButton` has no tooltip API. The title bar, git panel, browser, find bar, file header, debug controls, sidebar status marks and agent chips all lost theirs. | `pm-components/src/icon_button.rs:22-37` | `old: pm-ui/src/tooltip.rs`; title bar alone had 11 |
| 19 | **FIXED** (`gap2/window`). Buttons, icon buttons, checkboxes, switches and selects take focus with Tab, show a ring and press on Enter/Space (selects also open on Down); steppers step on the arrows; menus take the keyboard as they open and move with the arrows, Home and End and choose on Enter; option lists move with the arrows and pick on Enter; the folder picker moves with the arrows, goes in on Enter, goes up on Backspace and Alt+Up, back and forward on Alt+Left/Right, and closes on Escape. Verified live: Space flips a switch reached by Tab, Down and Enter choose Overview in a project menu. **There is no keyboard focus ring and no keyboard operation of controls.** Buttons, checkboxes, switches, segmented controls, selects, steppers and menus can't be focused or driven by keys. `Dialog` and `ConfirmDialog` don't close on Escape. The folder picker and adder have no arrow or Enter keys. | `button.rs:131-144`, `menu.rs:269-298`, `dialog.rs:82-111`, `confirm_dialog.rs` | `old: button.rs:253-313`, `settings.rs:20-92`, `folder_picker.rs:308-348` |
| 20 | **PARTLY FIXED.** **Typing behaviour ignores its settings, and features are missing.** Typing ignores tab size, hard tabs, auto-close, auto-indent and comment extension. Format on save, code actions on save, auto save, snippets and Emmet on Tab, folding, multi-caret (Ctrl+D, Alt+click, column select), case and sort commands, and the line-copy fallback are gone. *Now honoured: tab size, hard tabs, auto-close, auto-surround, auto-indent, comment extension, format on save and on type, auto save, snippets and Emmet on Tab, folding, Ctrl+D, Ctrl+Shift+L, Ctrl+Alt+Up/Down and Alt+click. Still missing: code actions on save, column select, case and sort commands and the line-copy fallback.* | `code_editor.rs:669-855`, `link/code.rs:100-117`, `code_text.rs:14` | `old: editor.rs:353-684`, `line_edits.rs:20-157`, `app/code.rs:165-199` |
| 21 | **FIXED** (`gap/conversation`), except the account chip's menu. **The composer is reduced.** The `+`, `@` and `/` toolbar buttons do nothing. `@` file mentions are gone. A prompt made only of attachments can't be sent. The box doesn't grow past one line. Signed-out sessions show no "Sign in again" bar. Account and Goal chips are inert. Shift+Tab mode cycling is gone. | `pm-components/src/composer.rs:294-320,471-477`, `prompt_box.rs:116`, `thread.rs:156` | `old: composer.rs:353-412,592-631`, `input.rs:254-354`, `choices.rs:784-968` |
| 22 | **FIXED** (`gap/conversation`). **The start page drops what you chose.** `StartSession` sends empty `settings` and `attachments` and `base: "HEAD"`. The welcome composer can't take attachments. Agents aren't filtered to visible, signed-in ones. | `pm-shell/src/link.rs:524-537`, `pm-pages/src/welcome.rs:85,99-116` | `old: choices.rs:523-535,757-761`, `agent_list.rs:106-126` |
| 23 | **PARTLY FIXED.** **Folder watches are mostly lost, and diffs go stale or loop.** `WatchFolders` holds only the session folder. The tree and open files don't reload on outside edits. Kept diffs are never refetched. Empty or failed diffs re-request on every recomposition and show "Loading the diff…" forever. *Diffs are now refetched after each change and never re-requested once answered, and open files without unsaved changes follow outside edits. `WatchFolders` still holds only the session folder and the tree does not reload.* | `link/git.rs:61-66,193-223,522-529`, `changes_page.rs:99-109` | `old: work/watch.rs:39-114`, `lazy_diffs.rs:46-131` |
| 24 | **FIXED.** **Git workflows are missing.** You can't continue, skip or abort a merge, rebase or cherry-pick (Commit just sends a plain commit). Stash is gone. The submodule picker is gone. Commit readiness ignores conflicts. Partly staged files are mishandled, and conflicted rows can be staged. *Continue, Skip and Abort show while a merge, rebase or cherry-pick is stopped; stashes can be made, applied, popped and dropped; a submodule picker points the panel at a submodule; conflicts block committing; partly staged files list under both groups; conflicted rows have no stage box.* | `pm-pages/src/git_panel.rs:176-190,534-604`, `pm-shell/src/git.rs:89-145,394-398` | `old: git_controls.rs:543-616,815-905`, `git_menus.rs:239-265,414-424` |
| 25 | **FIXED** (`gap/conversation`), except "Show what this turn changed". **Restore files and Undo turn are gone.** The "Restore files to before this turn" action, Undo turn and turn changes are missing. `Restored`, `Undone`, `ToolSignIn`, `Limits`, `Goal`, `Slots` and `Container` events are dropped. | `pm-shell/src/thread.rs:254` | `old: chat.rs:63-123`, `hearing.rs:113-115,303-363` |
| 26 | **FIXED** (`gap/conversation`), except the duration in "Worked for Ns", which the events do not carry. **Subagents are flattened into ordinary rows.** The subagent card, per-subagent Stop (`StopSubagent`), the worktree/merge chip and nesting through `parent` are gone. Back-to-back thinking or text blocks overwrite each other. The "Worked for Ns · N steps" fold is gone. Edit steps lose their diff. | `pm-shell/src/translate.rs:24-65`, `thread.rs:71-96,285`, `pm-components/src/steps.rs` | `old: steps.rs:43-465,591-616`, `hearing.rs:185-220,407-418,541-594` |
| 27 | **PARTLY FIXED** (`gap/panels`): `>` lists every window-wide keymap action with the keys the keymap binds, the recently used first; `:` goes to a line, `?` lists the prefixes, recently opened files lead, and Run Task, Start Debugging and Select Toolchain open their pickers. Still missing: compare-with-file, chat search, Ctrl+Enter, outline without the language server. **Quick input is reduced.** It has a fixed table of 25 commands, with hard-coded key labels that ignore the keymap. `:` go to line, `?` help, recents, Compare-with-file mode, chat search, Ctrl+Enter to start a new session and the debug, task and toolchain pickers are gone. Outline needs the language server plus typing. | `pm-shell/src/quick.rs:179-244,291`, `link/quick_input.rs:173-252` | `old: listing.rs:166-1279`, `quick_choices.rs:148-446`, `quick_sessions.rs:10-188` |
| 28 | **PARTLY FIXED** (`gap/panels`): F2, Delete, Ctrl+X, Ctrl+C and Ctrl+V work on the entries chosen, Ctrl and Shift clicks choose several, dropped folders are imported whole, imports and moves take free names (`name copy.ext`), and names are validated. Still missing: the menu on empty space and per-frame row rebuilding. **The file tree is reduced.** There are no key shortcuts (the menu still shows F2 and Delete hints), no cut, copy or paste, and no multi-select. Dropping a folder fails, and imports overwrite files with the same name. Names aren't validated, there is no menu on empty space, and every row is rebuilt each frame. | `pm-pages/src/files_panel.rs:180-296`, `pm-shell/src/file_tree.rs:28-226`, `link/browse.rs:171-222` | `old: app/tree.rs:47-103`, `tree_actions.rs:239-748`, `work/import.rs:28-104` |
| 29 | **PARTLY FIXED** (`gap/panels`): the divider between panes drags, tabs can be pinned (kept first, left open by Close others and Close all), the session and dock tab strips scroll sideways, file tabs show their file-type icon, and the right pane keeps its file after a close. Still missing: up/down splits, drop-zone highlight, drag ghost, per-session open files. **Panes and tabs are simplified.** The pane tree became one fixed 50/50 left/right split with no draggable divider, no up/down splits, no drop-zone highlight and no drag ghost. Tabs have no pinning, a cut-down tab menu, generic file icons, and no overflow handling (shell tabs in the dock run off the edge in every new render). Open files are no longer per session. | `pm-shell/src/open_files.rs:276-343`, `pm-pages/src/session.rs:59-115`, `session_tabs.rs:28-254`, `pm-pages/src/dock.rs:99-127` | `old: pm-components/src/panes.rs:91-1026`, `pm-pages/src/panes.rs:75-585`, `open_files.rs:291-345` |
| 30 | **FIXED** (`gap/panels`): turning the browser on downloads it with no pane open, the settings row shows the download's progress, and the local and remote servers are told when it is available; file icons come from installed languages' `icon.svg` and icon-theme extensions again. **Undecided.** These are close for the last slot: the embedded browser no longer downloads when enabled in Settings, and agents are never told it is available (`web.rs:388-395`, `link/browser.rs:203-206`); and file-type icons no longer come from installed languages or icon themes (`file_icon.rs:84-127` vs `old: file_icon.rs:57-156`). | | |

---

## 2. Settings: what is honoured

### Why Appearance "did nothing"

- **Theme mode works, live.** `window_theme(link)` runs on every rebuild of both
  windows, and changing a setting bumps `revision`
  (`window.rs:74,161,168-173`). The settings window shares the main window's
  `Link`, so the cause is not a separate process or state.
- **Nothing else on the page reaches the theme.** `AppTheme::system()`,
  `light()` and `dark()` hard-code `InterfaceFont::default()`,
  `CodeFont::default()` and `Density::default()` (`app_theme.rs:224-244`).
  `with_interface_font`, `with_code_font`, `with_density`,
  `InterfaceFont::named` and `CodeFont::named` have no callers. Every
  `code_font()` and `density` reader therefore sees the defaults:
  `code_editor.rs:241`, `terminal_view.rs:169`, `label.rs:87` and
  `diff_view.rs:189`.
- **Two `Density` types are never connected.** `pm_config::Density` is the one
  written (`settings_values.rs:459`); `pm_components::Density` is the one read.
- **Light theme and Dark theme were removed from the catalog.** They are still
  in `pm-config` (`appearance.rs:49-51`), but nothing reads them. In the old
  build, `chosen_family` built the theme family from them
  (`old: app.rs:126,355,528`).
- **No font stack is registered.** The old build asked for Inter or Noto Sans
  and JetBrains Mono or DejaVu Sans Mono (`old: pm-theme/src/fonts.rs:17-20`).
  New uses the generic `SansSerif` and `Monospace` families, and nothing calls
  `register_font`.
- **Following a live system theme change is unconfirmed.** The old build
  handled `ThemeChanged` (`old: app.rs:2416-2419`); the new build only reads it
  when it rebuilds.
- **Text sizes differ visually.** In the paired renders, the new interface text
  is smaller than the old, and the new code text is larger than the old, even
  though both builds hold 13 px and 12 px.

### Every catalog setting

"Old WS" means the old renderer read the value from `WorkspaceState` every
frame; those consumers were not traced line by line. "srv" means the server
reads the value.

| Setting | Old consumer | New consumer outside the settings UI | Status |
|---|---|---|---|
| Launch | `app.rs:312` | `window.rs` restores the view only for Restore | live |
| ConfirmClose | `app.rs:2411` | `link/sessions.rs` `may_close` | live |
| CheckUpdates, Channel | `app.rs:326` | `link.rs` `recheck_release`, every 6 hours | live |
| OfferLanguages | old WS | `link.rs` `follow_settings` → `file_view.rs`; declined languages kept in `declined_languages` | live |
| EmbeddedBrowser | `app/browser.rs` | `web.rs` | live |
| OpenDevServers, OpenLocalLinks | `app/browser.rs` | `link/browser.rs` | live |
| RestrictAgentBrowsing, AgentOrigins | `app/browser.rs` | `web.rs`, `browser_agent.rs` | live |
| ThemeMode, LightTheme, DarkTheme | `app.rs:355,528` | `pm-pages/src/window_theme.rs` | live |
| InterfaceFont, InterfaceSize, CodeFont, CodeSize, LineHeight, Density | `app.rs:330-336,713` | `window_theme.rs` | live |
| LineNumbers, RelativeLineNumbers, CurrentLine, IndentGuides, WrapGuide, LineLength, GitGutter, Occurrences, InlayHints, CodeLens, Minimap, StickyScroll, CursorShape, CursorBlink, Ligatures, SoftWrap | old WS | `CodeLook` in `code_look.rs`, read by `code_editor.rs` | live |
| InlineBlame, GitBlame | `marks.rs:224-297` | `CodeLook::inline_blame` asks `Blame` for the caret's line (`ServerAsk::CaretLine`), drawn after the line | live |
| BracketColors | old WS | `code_editor.rs` colours `brackets_on` by depth | live |
| RedactPrivateValues | old WS | `code_editor.rs` `redacted` stars values off the caret's line | live |
| DependencyHints | old WS | `link/lsp.rs` asks lenses with `dependencies` | live |
| SemanticHighlighting, SemanticRangeLines, SemanticRangeMargin | `app.rs:708-711` | `link/lsp.rs` `ask_annotations`, a range around the caret first in long files, with the reader's token rules | live |
| TabSize, HardTabs, AutoClose, AutoSurround, AutoIndent, ExtendComments, FormatOnType, SnippetsOnTab, Emmet | `editor.rs`, `app/editing.rs` | `code_editor.rs` | live |
| LinkedEditing | `editor.rs` | `link/lsp.rs` asks `LinkedRanges` with each caret move | live |
| DragAndDrop | `editor.rs` | `code_editor.rs` `press_at` | live |
| VisualCaret | `app/editing.rs` | `code_editor.rs` `right_to_left_key` | live |
| FormatOnSave, TrimWhitespace, FinalNewline, OrganizeOnSave, FixAllOnSave | `editor.rs:647-684` | `link/code.rs` `save_file` → `save_action` → `format_then_write` | live |
| Autosave | `app/code.rs:165-199` | `code_editor.rs` | live |
| CompletionsOnInput, Hover, HoverDelay, SignatureHelp, Diagnostics | `app/code.rs:202-301` | `code_editor.rs` | live |
| CompletionDocs | `editor.rs:863-899` | `code_editor.rs` resolves only while on | live |
| Lightbulb | `editor.rs` | `code_editor.rs` `bulb_shown` | live |
| SearchWrap, SeedSearch, Smartcase, AutoFindInSelection | old WS | `code_bars.rs` `found`, `effective_query`; `open_find` | live |
| ScrollBeyond, ScrollMargin, ScrollbarMarks | old WS | `code_editor.rs` (`padded` rows, vim margin, minimap marks) | live |
| CompareIgnoreWhitespace | old WS | `Comparison::between_with` in `open_files.rs` | live |
| VimMode | `settings_state.rs:98` | `link.rs` `follow_settings` | live, also for files already open |
| VimClipboard, VimBindings, VimReplayTyping, VimSyntaxObjects | `components/vim.rs:73-135` | `code_editor.rs` `configure_vim`, `ModalView` | live |
| LanguageTabSize, LanguageIndent | `settings_state.rs:93-96` | `OpenFiles::indents` → `file_view.rs` `CodeLook::with_indent` | live |
| LanguageFormat, LanguageOrganize, LanguageFixAll | client | `link/code.rs` `save_file`, `format_then_write` | live |
| LanguageServer, LanguageServerCommand, LanguageServers, WorkspaceRepositories, WorkspaceFolders | srv | srv | live |
| BaseKeymap, custom keys | everywhere | `link/keys.rs`, `KeyBindings` | live |
| Shell, ShellArguments | srv | srv | new shells |
| TerminalFontSize, TerminalCursor, CopyOnSelect, Scrollback | `terminal::Look` | `window_theme.rs`, `window.rs`, `dock.rs` | live |
| AutoFetch, FetchInterval, LocalHistory, HistoryDays, HistorySize, GitHosts | srv | srv | live |
| GitGutterMarks, GitWordDiff | old WS, `diff.rs:788` | `CodeLook::with_git_gutter`, `with_git_marks`; `diff_view.rs` | live |
| BranchTemplate, WorktreeFolder, WorktreeInclude, PermissionMode, Effort, Memory, Checkpoints, PromptCache, CacheWarming, PlanAutosave | srv | srv | live |
| DefaultAgent | `settings_state.rs:140-149` | `start_requests.rs` `offer_agents`, after the agent last used | live |
| OnFinish, Notification | `notify.rs` | `window.rs`; `link.rs` `review_finished` opens the Diff tab | live |
| DefaultServer | old WS | `link/projects.rs`: the adder and its folder picker start on it | live |

### Settings window UX

- FIXED: **P2** Keyboard navigation is gone: no Tab order or focus IDs, and Enter in a
  picker filter no longer picks the first row (`old: settings.rs:20-92`).
  *Every control takes Tab; the option list moves with the arrows and Enter picks
  the row marked, or the first.* Verified live.
- FIXED: **P2** Unreadable list entries (origins, bindings) are dropped with no
  message, and the box is cleared anyway. *The box keeps the text and says it
  does not read (`line_reads`).*
- FIXED: **P3** The picker filter always shows and the list is uncapped. *The
  filter shows from 10 choices and the list scrolls past 240 px.*
- FIXED: **P3** Every change re-reads both settings files from disk, and save
  errors are swallowed. *`store_writer.rs` keeps what it wrote and writes off the
  UI thread, logging failures.*
- FIXED: settings failures (model servers, slots, MCP, library, tool sets) show
  in a dismissible notice at the top of the settings page and no longer leave
  "Asking the server…" up.
- **OK:** search, reset buttons, and the narrow layout breakpoint (760 px,
  rows stacking below 480 px).

### Agents, servers, models, customizations and machines

**P1**
- **Customizations always use the local server.** The library place has no
  host (`link.rs:359`), and `SaveWithheld` goes to the local server
  (`settings_change.rs:110-119,240-249`). An `ssh://` project's customizations
  are read and changed on this machine. Old:
  `old: settings_library.rs:137-174`.
- **The machines form has no Token field.** `KnownServer::default()` saves an
  empty token, so Direct and Tunnel servers that need one can't be reached
  (`settings_body.rs:354-418`, `settings_change.rs:260-274`).
- **Model slots lost most editing.** Missing: the provider view, User/Project
  scope, the model picker, effort, the backup picker with reordering, "Use
  suggested", custom slots, and saving or deleting presets. Only two free-text
  boxes remain, hard-coded to `SlotScope::User` and provider `None`
  (`settings_forms.rs:352-440`). Old: `old: settings_slots.rs:415-639`.
- **Model servers can't be edited.** There is no Edit, no API (wire) choice
  (Responses-only servers can't be declared), and no key-env box. The id must
  be typed and is not checked against existing agent or provider ids
  (`settings_forms.rs:333-338,504-520`).

**P2**
- **Accounts:**
  - No identity status ("Signed in as…" / "Checking…").
  - You can't switch back to the default account.
  - No per-account keys; `SaveKey` always sends `login: None`.
  - Sign-in and key buttons show for agents that can't use them.
  - No rename.
- FIXED: **The sign-in code dialog never closes by itself**, though its text says it
  does. *It closes when the server answers the sign-in (`Ask::SignIn`).*
- **Declared model servers are not listed as agents**
  (`settings_model.rs:28-39`).
- **"Find models" takes every model found.** There is no tick list to choose
  which to offer.
- **Failures are lost or leave the page stuck:**
  - Model server, slot and MCP save failures are dropped
    (`link.rs:139-153,916-924`). The form closes before the answer, so a
    failed save loses what was typed.
  - A library failure leaves "Asking the server…" up forever.
- **MCP:**
  - No Edit.
  - Listings that need input install unfilled, with no Configure form.
  - Extension servers show a switch that flips back on the next load.
  - The featured registry list isn't loaded on open.
- **Customizations:**
  - They need an open project, even for personal scope.
  - Switches don't respect `switchable` or `per_tool`.
  - Rows have no "Open" button.
  - A new draft doesn't open its file.
  - The SYSTEM.md / APPEND_SYSTEM.md writers are gone.
- FIXED: **Machines:** no Edit, and the address is not required. *Each server
  row has Edit, which fills the form; saving under a known name changes it, and
  the address is required.* Rendered in `settings-servers` and
  `settings-servers-narrow`.

**P3**
- Per-agent icons are gone; every agent shows `ROBOT`.
- "Saved" shows under every section.
- MCP lost "Copy configuration", "Open website", the Disabled badge and folding.
- Slot purpose lines and source names are gone.
- Library scope and Off badges are gone; overview counts show 0 instead of
  "Reading…".
- Draft names are checked by a local rule instead of `valid_name`.

---

## 3. Window, title bar and app loop

**P1**
- FIXED: **The root preview key handler takes chords before the terminal and the
  browser page** (top 30, #6). Verified live in a virtual KWin.
- FIXED (mostly): **About 55 keymap actions are unhandled** (top 30, #5). Still
  undone: ToggleReadOnly, CopyPermalink, OpenPermalink, FileHistory,
  OpenTerminalHere, OutgoingCalls, ShowSupertypes, ShowSubtypes,
  ToggleBreakpoint, CopyWithoutIndentation, CursorUndo/Redo and the editor's
  GoBack/GoForward; splits only make the fixed left/right split, and stash
  apply/pop/drop act on the newest stash. Verified by `window_actions_are_done`.
  Was: Handled ones:
  CommandPalette, FileFinder, ProjectSymbols, Outline, GoToSession,
  ChangeTheme, ChangeKeymap, Problems, CheckoutBranch, RewindConversation,
  OpenSettings, NewSession, NewChat, ShowHistory, ShowExtensions,
  WorkspaceBack, WorkspaceForward, ToggleTerminal, NewTerminal, the debugger
  stop/pause/step actions, OpenBrowser, StopAgent, RetryReply, StartFresh,
  HandOffConversation, AskOnTheSide, Commit, CommitAndPush, RefreshSourceControl,
  StageAll, UnstageAll, Fetch, Pull, PullRebase, Push, ForcePush and Sync.
  ToggleSourceControl, SearchProject, RunTask, RerunTask, StartDebugging and
  AttachDebugger only open a panel.
  *Now also done (`gap2/window`):* ToggleReadOnly, CopyWithoutIndentation,
  CursorUndo/Redo, ToggleBreakpoint, OutgoingCalls, ShowSupertypes and
  ShowSubtypes in the editor (`code_keys.rs`, `code_editor.rs`
  `run_beyond_text`); GoBack/GoForward, CopyPermalink, OpenPermalink,
  FileHistory and OpenTerminalHere handed to the link (`ServerAsk::Act`,
  `link/code.rs` `editor_action`); stash apply/pop/drop open the stash list when
  there are several. Splits are still left/right only.
- FIXED: **Close quits without asking** (top 30, #10). Verified by
  `closing_on_working_agents_asks_first` and live.
- PARTLY FIXED: **The saved view lost most of what it kept.** Open files with
  their carets and unsaved text, read receipts, recent files and commands, the
  folded projects and the split share are kept again (`Link::view`). Still gone:
  the prompt draft, the focused field, unfolded folders, graph and plan sizes,
  the maximised terminal and scroll positions.

**P2**
- FIXED: **The view file is read and written on the UI thread for every action.**
  *`store_writer.rs` compares with what it last wrote and writes on a thread of
  its own, settling bursts for 150 ms; the close writes at once.*
- PARTLY FIXED: **The trail holds only pages.** *Back and forward pass over
  closed sessions and removed projects, the editor's GoBack/GoForward step
  through the places the caret jumped from (`walk_jumps`), and mouse buttons
  4/5 are bound on the workspace.* Buttons 4/5 could not be checked live:
  KWin's fake input makes winit-wayland panic in `pointer_frame` on them.
- FIXED: **The window always opens at 1636×720.** *It opens at the size it was
  left, maximised when it was left so, clamped to 720×480–3840×2160, and has a
  minimum size (`frame_memory.rs`).*
- **macOS lost its native traffic lights.** The window is undecorated on every
  platform.
- FIXED: **The title bar has no tooltips** and no "Update languages" button.
  *The button shows while an installed language has an update and opens
  Extensions; tooltips are moved inside the window and above the control when
  there is no room below (`tip.rs`), verified live on Close.*
- FIXED: **Resize grips misbehave.** *They were laid out with `align` in a
  stack and covered the whole floating window, so every press started a resize
  (seen live: no click reached the page, and winit panicked). They are now rows
  and columns on the frame's edge only, inset to the shadow band, and absent
  while maximised.* Verified live: clicks reach the page.
- FIXED: **"Open on launch" is ignored.**

**P3**
- FIXED: Panel toggles don't show their filled state (see `icons-zoom.png`);
  they now show `active` while open (snapshots).
- FIXED: The command centre is fixed at 330 px and its text is wrong on welcome
  pages. *It grows between 180 and 420 px and names the project of a new session.*
- FIXED: No border around the floating frame. *A hairline in `outline_variant`.*
- FIXED: No 6-hour update recheck, and the update installs on the UI thread.
- The app id is hard-coded.
- FIXED: AltGr chords may be read as shortcuts. *A key typed with Alt that
  types a sign other than its own is typing.* Verified live on a Swedish layout:
  AltGr+2 types `@` in the quick input.
- FIXED: Chords are named by the character typed. *With Ctrl, Alt or the
  command key held, letters, digits and signs are named by their physical key
  as on a US layout.* Verified live on a Swedish layout: Ctrl with the key left
  of 1 toggles the terminal.
- FIXED: The keymap is rebuilt on every keystroke. *`Link::keymap` keeps it
  until the preset or bindings change.*
- Startup timings and `--snapshot-browser` are gone.

---

## 4. Sidebar, overview, history, extensions and welcome

**P1**
- **FIXED** (`gap/conversation`): **The session and chat context menu is gone** (top 30, #16).
- **There is no "Server out of reach" status.** Remote sessions keep spinning
  as Working (`link/machines.rs:165-180`).
- **FIXED** (`gap/conversation`; kept per local session in the saved view): **Read receipts are memory-only.** Replayed events are all marked read, so
  anything that finished while the window was away shows as Idle, not Unread
  (`link.rs:1154-1156`, `thread.rs:44`).
- **The adder can't clone from GitHub through the account and repository
  pickers.** It also has no clone destination and no clone onto another
  machine (`pm-pages/src/adder.rs:47-107`, `link/projects.rs:80-98`).
- **Extensions:** "Install Extension…" from a folder and the per-capability
  grant switches are gone. `InspectExtension`, `InstallExtension` and
  `GrantExtension` are never sent.
- **FIXED** (`gap/conversation`): **Welcome page agent chips are unfiltered and reset to the first agent**
  (`welcome.rs:85`, `start.rs:34,44`).

**P2**
- A session is marked read as soon as it opens, whatever the focus or scroll.
- **FIXED** (`gap/conversation`): "Mark unread" is gone.
- Status marks are plain coloured dots with no glyphs and no tooltips.
- The overview is missing its Repository section (branch, ahead/behind,
  uncommitted count) and its agent icons and tallies per row.
- **FIXED** (`gap/conversation`): "New chat" is never highlighted.
- The adder has no keyboard navigation and ignores a remote default machine.
- **FIXED** (`gap/conversation`), except the glyph: History groups days in UTC (`pm-shell/src/history.rs:35-41`) and always
  shows `SPARKLE` with the raw agent id.
- Extensions lost their own icons, matching by file ending, and the reason a
  folder install failed.
- Drag reordering has no live preview or dimming. Reordering sample projects
  is offered but does nothing.

**P3**
- Folded projects aren't saved.
- Ages are computed once per rebuild, so "just now" goes stale.
- Tiles have no pressed state.
- The submodule rail and parent-folder hint are gone.
- Avatar colours come from a small palette instead of an OKLCH hue.
- The welcome page lost its review-panel button and usage tooltips.

---

## 5. Conversation

**P1**
- **FIXED** (`gap/conversation`): **Restore files and Undo turn are gone**, and `Restored` and `Undone` are
  dropped (`thread.rs:254`).
- **FIXED** (`gap/conversation`): **`Event::Failed` leaves the session Working, and tool steps never close**
  (`thread.rs:157-160,274-287`).
- **FIXED** (`gap/conversation`): **`ToolSignIn` is ignored,** so a tool server's sign-in page is never shown.
- **FIXED** (`gap/conversation`): **Subagents are flattened:** no card, no Stop, no worktree chip, no nesting.

**P2**
- **FIXED** (`gap/conversation`): Back-to-back blocks of the same kind overwrite each other: two thinking
  blocks, or two text blocks, become one (`thread.rs:71-96,308-314`; the old
  test `each_thinking_block_is_one_thought` expected two).
- **PARTLY FIXED** (`gap/conversation`; it says Worked, Failed or Stopped with the step count): The turn footer always says "Worked", with no duration and no step fold.
  "Thought for Ns" became "Thought".
- **FIXED** (`gap/conversation`): Edit steps lose their diff, and the +/− counts are miscounted with a
  HashSet.
- Steps lost relative paths, file icons, a live timer, the spinner and output
  caps, so large outputs expand without limit.
- **PARTLY FIXED** (`gap/conversation`; the container step and `Event::Container` are shown): The "Thinking…" status line is gone, along with "Sending prompt…", the
  container step, "server out of reach" and the handling of `Event::Container`.
- **FIXED** (`gap/conversation`), except Select all: The right-click message menu is gone (Copy reply as Markdown, Select all).
  Multi-click and Shift-click selection are unverified.
- **PARTLY FIXED** (`gap/conversation`; rewind drops the cards): "Show what this turn changed" is gone. Rewind leaves the approval and
  inquiry cards up.
- **FIXED** (`gap/conversation`), for sent files: Sent files and element chips are missing from user messages.
  `Event::Answered` keeps no record of the answers.
- Markdown links can't be clicked. Images by URL or repository path don't
  load. Diagrams and formulas are drawn on the UI thread with an uncapped
  cache.
- **Inquiry:** "Skip" abandons the whole form. Number-key picks and stepping
  one question at a time are gone. "Send answers" is enabled even when nothing
  is answered.
- **Plan:** the resizable side panel and "Revise…" are gone.
- **Activity chips** are plain labels, with no popovers for approvals, tasks,
  subagents or shells.
- **The pull request bar** shows for any branch, including main. Its action
  set is wrong: it has Squash and Merge, and lacks Push, Update and Delete
  branch. It has no busy state or error reporting. Replies go to whichever
  session is open. It doesn't use the code host's own noun. It never works for
  remote sessions (`conversation_requests.rs:104-141`).
- **Continue chat** hard-codes base `HEAD`, offers remote projects, and isn't
  disabled while the agent is working.
- **The trust card** lost its per-standing wording and its hooks warning, and
  prints kinds with `{:?}`.
- **The session header status dot is static.**
- **Each streamed chunk re-parses the whole reply,** and every event clones
  every message (`thread.rs:84`, `link.rs:1183`).

**P3**
- Spend shows costs under a cent as $0.00 and lost its tooltip breakdown.
- The header doesn't show the model, and chats get a made-up "main" branch.
- The fallback notice wording is raw.
- Side answers are plain text instead of markdown.
- Message actions are always visible instead of on hover.

---

## 6. Composer

**P1**
- **FIXED** (`gap/conversation`): **The `+`, `@` and `/` toolbar buttons have no handlers**
  (`composer.rs:471-477`).
- **FIXED** (`gap/conversation`): **`@` file mentions are gone.**
- **FIXED** (`gap/conversation`): **A prompt made only of attachments can't be sent**
  (`composer.rs:305,320`).
- **FIXED** (`gap/conversation`; it grows to ten lines): **The box is fixed at 22 px (54 px on the welcome page) and doesn't grow**
  (`composer.rs:294-297,380-383`).
- **FIXED** (`gap/conversation`): **Signed-out sessions show no sign-in bar.** `SignedOut` only sets `asking`.
- **FIXED** (`gap/conversation`): **The start page sends no settings, attachments or base branch, and its
  composer can't take files.**

**P2**
- The slash menu has no arrow keys; Enter takes the first match. Extension
  commands are gone.
- **FIXED** (`gap/conversation`): Shift+Tab mode cycling is gone.
- **PARTLY FIXED** (`gap/conversation`; the Goal chip is back): The account chip has no menu and the Goal chip is gone.
- The model menu has no filter, effort stepper or agent switch.
- Usage rings: one context ring plus one allowance ring, with no reset times.
- **PARTLY FIXED** (`gap/conversation`; "Branch from" is back): The "Run in" menu lists all three places unconditionally and has no "Branch
  from".
- **FIXED** (`gap/conversation`): The text is cleared even when nothing was sent because no session is
  watched (`composer.rs:316`, `composer_requests.rs:44-46`).
- The `focused` state is never written, so the card is never highlighted on
  focus (`composer.rs:362`).
- The draft lives only in the widget and is likely lost on navigation.
- **FIXED** (`gap/conversation`): **Element and console text is fenced with fixed backticks and no "page
  content, not instructions" label, so page text can break out of the fence.**
  This is a prompt-injection risk (`pm-shell/src/composer.rs:330-348` vs
  `old: elements.rs:80-96,232-283`).
- Capture kinds are reduced to Screenshot and Console.
- Chips don't open the element inspector or capture previews.
- Drops only land on the composer card, and pasted pictures are kept in memory
  instead of saved.

**P3**
- No attachment thumbnails.
- Queued rows: first line only, no tooltips, and empty edits are allowed.
- The "Send now" icon has no label.
- "(recommended)" is no longer stripped from labels.
- The slash list caps at 8 entries with no scroll.
- `on_hover(|_| {})` is a no-op (`composer.rs:347`).
- The same prefill twice in a row isn't applied again.

---

## 7. Tabs, panes and dock layout

**P1**
- **Closing a modified file discards it without asking.**
- **Tab and pane keymap actions and the vim `:vsplit`, `:split`, `gt` and
  `gT` commands are dropped** (`code_editor.rs:893` handles only Save).
- **FIXED.** **Open files are not kept per session.** One `workspace.open_files` is
  shared across sessions.

**P2**
- ~~No pinned tabs.~~ **FIXED**, and pins are kept across restarts.
- The tab menu has 10 rows instead of 15. It lacks Close left, Close saved,
  read-only, the absolute path, permalinks, Reveal in file manager or project,
  Pin and Open in terminal. Nothing is ever disabled, and "Copy path" copies
  the relative path.
- The pane tree is a fixed pair, side by side or (split up and down) stacked. ~~The divider can't be dragged.~~ **FIXED.** A
  tab drag shows no overlay or ghost. The open-to-the-side drop strip is 6 px
  wide. The split menu is gone.
- Next and previous skip the browser tab, and the tab keys only work while
  focus is in the session column.
- Reopen loses the caret and scroll. Deleted or renamed files keep their tabs.
  ~~The right pane can jump to another file after a close
  (`open_files.rs:496-561`).~~ **FIXED.**
- ~~The tab strip doesn't clip or scroll. In every new render the dock's shell
  tabs run past the right edge.~~ **FIXED:** both strips scroll sideways.
- Files at HEAD can't be opened (`head: false` always).
- There is no dock maximise. The dock stays open after its last shell exits.
  The Debug tab always shows.
- The plan is no longer a resizable side column.

**P3**
- The Diff tab always shows.
- The page tab label ignores the page.
- ~~Every file tab gets `FILE_TEXT`.~~ **FIXED.**
- The Excerpts tab has a fixed title.
- No empty-pane state.
- `Tab` has no tooltip, agent variant or lock mark, and the close cross only
  shows on hover.

---

## 8. Code editor

**P1**
- **PARTLY FIXED.** **No editor setting is applied** (see section 2). Typing,
  wrap, blink, hover, completion, signature, format and auto-save settings now
  reach the editor.
- **FIXED.** **Diagnostics are never stored or drawn.**
- **FIXED.** **The lightbulb applies the wrong action.** Titles are filtered to enabled
  actions, but `TakeAction(place)` indexes the unfiltered list
  (`file_view.rs:158-163`, `code_assist.rs:361-364`, `lsp.rs:467`).
- **The keymap is ignored in the editor.**
- **FIXED.** **Line edits are missing:** add next occurrence, select all occurrences,
  carets above and below, column select, split selection, case changes, sort,
  reverse and unique, transpose, trim, delete to line end or start, block
  comment, text objects, next/previous problem and change, cursor undo.
  Column select also works by Shift+Alt or middle drag.
- **FIXED.** **No folding.** `FoldingRanges` is never requested. Ctrl+Shift+[
  and ] fold and unfold at the caret, Ctrl+Alt+[ and ] fold and unfold all,
  by the syntax tree or indentation; a folded line shows a ⋯ mark.
- **FIXED.** **Format on save, actions on save and auto save are gone.**
  Organize imports and fix all on save run before formatting and the write.
- **Documents aren't re-opened with the language server after a reconnect or
  when a language is installed.**
- **Vim:** settings are hard-coded, only `:w` is handled, `zz`/`zt`/`zb` and
  Ctrl-E/Ctrl-Y are dropped, and toggling vim mode doesn't reach open files.
- **FIXED.** **Typing ignores its settings,** and snippets and Emmet on Tab are gone.
- **FIXED.** **No horizontal scroll and no soft wrap.** Long lines are clipped (see
  `new-code.png`). Jumps to a line land on its row while lines wrap, and Page
  Up and Down move by rows.
- **FIXED.** **The language server's folding ranges are never asked for.**

**P2**
- **PARTLY FIXED** (the settings are honoured and hover waits for the hover
  delay). Completion, signature, hover and hover-delay settings are ignored. Hover,
  highlights and code actions are requested on every move with no rest delay.
  The completion scope is always `None`.
- Inlay hints and lenses are fetched even when switched off.
- **FIXED.** Line numbers are always shown and never relative. The cursor shape and
  blink settings are ignored outside vim.
- Minimap and sticky scroll are always on.
- **Find bar:** no find-in-selection, smartcase, wrap setting or autofocus,
  and Shift+Enter doesn't step back.
- **FIXED.** Copy with no selection copies nothing (the old build copied the line).
  Paste doesn't re-indent.
- **FIXED** (Alt+click adds a caret; the editor menu has the clipboard,
  language-server, comment, fold, find, case and sort commands). No Ctrl+click
  to go to definition, no Alt+click for a caret, and no editor context menu.
- Pressing selected text without moving leaves the drag state. Ctrl+K is bound
  directly to Hover, which blocks Ctrl+K chords (`code_keys.rs:111`).
- **FIXED.** Missing actions: Go back/forward, toggle breakpoint at the caret,
  permalinks, file history, terminal here, reveal, format selection, call
  hierarchy, declaration and type definition. Read-only toggling, copy
  without indentation, supertypes and subtypes work too.
- The colour picker doesn't list the server's presentations.
- **File header:** no file icon, HEAD or read-only labels, breadcrumbs, or
  problem counts (compare `old-code.png` and `new-code.png`).
- No notice that a file is truncated. SVGs have no preview. Links in rendered
  markdown can't be pressed.
- No conditional or unplaced breakpoint glyphs, no paused-line arrow, no run
  buttons or lenses, and no inline blame.

**P3**
- No indent guides, ruler, bracket colouring, matching-bracket box, private
  value redaction or scrolling past the end.
- Ligatures are hard-coded off.
- The vim status line doesn't show the recording register.
- Visual caret movement is always on.
- Popups aren't kept on screen.
- The hover card can't be scrolled or copied.
- Pictures are decoded on the UI thread with no size cap.
- No tooltips on the find, go-to or header buttons.
- **Visual:** the sticky header overlaps the first line (`new-code.png` shows
  "23 pub fn run()" above line 34 with no separator).

---

## 9. Diff, review and git

**P1**
- **FIXED.** **Folder watches cover only the session folder,** and the tree and open
  files never reload on outside edits.
- **FIXED.** **Kept diffs go stale, and empty or failed diffs loop forever with
  "Loading the diff…".**
- **FIXED.** **No continue, skip or abort for a merge, rebase or cherry-pick.**
- **FIXED.** **Commit readiness ignores conflicts and untracked-only states,** and the ✓
  button is never disabled.
- **FIXED.** **Stash is gone entirely.**
- **FIXED.** **The submodule repository picker is gone.**
- **FIXED.** **Partly staged files are mishandled** (one `staged` flag, first-match
  lookups).

**P2**
- **FIXED.** Changed files are a flat list instead of a folder tree with folder
  checkboxes and tallies (compare `old-files.png` and `new-files.png`).
- **FIXED.** The group checkbox state is wrong.
- **FIXED.** The stash keys always act on `stash@{0}`.
- **FIXED.** Conflicted rows show a stage box.
- Changed-file rows have no file icons.
- Moved submodules and side-by-side repositories are dropped.
- The change menu and source menu lost rows: Open file at HEAD, copy paths,
  Clone, Checkout, Set upstream, Stash, Amend, and the "(no remote)" reasons.
- **Branch tasks:** no remote choice, no "also on the remote" ticks, no
  unmerged warnings, merge and rebase limited to 6 branches, no Upstream task.
  Enter in the branch filter can create a branch when matches exist.
- **Graph:** the commit menu lost Cherry-pick and Copy hash. It is fixed at
  230 px with 60 commits and no "go to current". It is open by default and
  fetched even when folded.
- **PARTLY FIXED** (long lines wrap with Soft wrap on and scroll across
  otherwise). **Diff view:** one file at a time, no folding per file, no
  viewed marks, no comments over ranges or whole files. Long lines are
  clipped instead of wrapped (`new-diff.png`).
- The word-diff setting is ignored, and the server's word ranges are dropped.
- **FIXED.** Turn revisions (Undo turn) and the file timeline with Restore are gone.
- "Take all ours/theirs" ignores the server's blobs (so delete/modify
  conflicts break), and conflict choices can't be undone.
- No tooltips anywhere in the panel.

**P3**
- No busy spinner.
- Raw `fatal:` error text.
- A range pick replaces the earlier picks.
- `ChangeRow::on_open` and `GraphRow::on_click` are never set.
- Ref pills aren't capped.
- The Refs control is a plain toggle.
- The amend prompt isn't marked destructive.
- The resolver closes before the server answers.

---

## 10. Files panel, search, quick input and dialogs

**P1**
- ~~**Tree:** no keys (yet the menu shows F2 and Delete hints), no cut, copy or
  paste, no multi-select. A dropped folder fails, and imports overwrite files
  with the same name.~~ **FIXED.**
- **Project search needs Enter;** there is no search as you type.
- **Quick input:**
  - ~~A fixed list of 25 commands with hard-coded key labels.~~ **FIXED.**
  - Outline needs typing and the language server.
  - ~~`:` go to line and `?` help are gone.~~ **FIXED.**
  - Ctrl+Enter no longer starts a new session.
  - The agent-setting, restore, fork and runnable choosers are gone.
    ~~Debug, task and toolchain~~ **FIXED.**
- **The folder picker only adds projects** (no clone-into or extension
  purposes) and has no keyboard.
- **The editor and text-field context menus are gone.**

**P2**
- **Tree:**
  - No menu on empty space.
  - No copy path or reveal.
  - ~~Names aren't validated.~~ **FIXED.**
  - Rename leaves the viewer on the old path.
  - Weak drag feedback: no target highlight, ghost or Ctrl to copy.
  - Refresh doesn't refresh git.
  - Every row is rebuilt each frame (up to 20k).
- **Search:** ignores `seed_search_from_selection`, has no clear button, no
  tooltips, and lost its empty and error states.
- **Quick input:**
  - No group headings. ~~No recents.~~ **FIXED.**
  - Files are capped at 60.
  - No compare-with-file mode.
  - The `%` highlight length is wrong (`link/quick_input.rs:252`).
  - Chat search and session ordering by attention are gone, as are the "New
    session in…" rows.
  - No "Create branch".
  - The current choice isn't preselected.
  - Rewind lists oldest first.
  - Disabled code actions aren't filtered.
  - History has no commits.
- **Folder picker:** stale replies can show the wrong folder, and
  Back/Forward/Up are never disabled.
- **Popovers** aren't kept inside the window, and menus have no keys or
  pressed state.
- **Dialogs:** `Dialog` and `ConfirmDialog` ignore Escape and Enter.
- **The update dialog** only checks the blocker after Install is pressed.

**P3**
- Case-sensitive sort.
- The menu sits at a fixed offset instead of at the pointer.
- No wheel scrolling in the quick input.
- Per-chooser empty texts are gone.
- Places lost Desktop, Documents and Downloads.
- The update date is shown raw, and "Later" shows while installing.

---

## 11. Dock: terminal, problems, tasks and debugger

**P1**
- **Terminal font size, family, cursor, scrollback and copy on select are
  ignored.**
- **Shells aren't re-attached after a reconnect or restart,** and each launch
  leaves the last launch's shells running on the server.
- **Problem rows can't be pressed,** and Problems covers only open files and
  isn't scoped to the folder.
- **Watches can't be added or removed.** `SetWatches` is never sent.

**P2**
- **Debugger:**
  - Pausing doesn't reveal the line, and picking a frame doesn't either.
  - The paused line isn't marked in the gutter.
  - Breakpoints are keyed by relative path only, so they leak across projects.
  - The toggle-breakpoint key does nothing.
  - Inline values are a stand-in: the first six variables, with no
    `InlineValues` request.
  - No attach to process.
  - Starting doesn't switch to the Debug tab.
  - The console has no colours and doesn't stick to the end.
  - The controls ignore the program's state and have no tooltips.
- **Tasks:**
  - Rerun just opens the tab.
  - No run gutter marks, lenses or runnable picker.
  - No toolchain picker.
  - Tasks and debug starts send `project: None`.
- **Terminal menu:** "Add to chat" and "Kill" are gone, and there are no
  disabled states or key labels.
- **Terminal input:**
  - No Ctrl+click links.
  - No IME preedit.
  - ~~**Paste doesn't strip control characters, so a paste can escape bracketed
    paste mode** (`pm-shell/src/terminal.rs:170-177`).~~ **FIXED:** control
    characters other than tab and return are dropped, with a test.
  - A new or picked shell doesn't take focus.

**P3**
- Typing doesn't clear the selection.
- Shift+PgUp pages a full screen instead of rows−1.
- The Problems count includes hints.
- Problem rows show the full path and message.
- Expensive scopes can't be opened.
- Variable layout is flat.
- Debug starts ignore the open file.

---

## 12. Browser

**P1**
- ~~**Turning on the embedded browser doesn't download it until the pane is
  opened.** `install` is skipped when no browser is open, and `run_web`
  returns early. Agents are never told it is available, and the notice text
  is wrong (`web.rs:388-395,462`, `link/browser.rs:203-206`).~~ **FIXED.**
- ~~**Download progress (`download_note`) is written but never read.** The
  settings page shows no status.~~ **FIXED:** the Embedded browser row shows it.

**P2**
- The keymap preview handler beats the page's keys (#6).
- Mouse back and forward don't navigate.
- The camera button takes a screenshot instead of opening the capture menu.
- Area capture has no mode state, highlight or cancel.
- Switching tabs leaves the old tab in pick mode.
- The address bar freezes once you type without pressing Enter
  (`link/browser.rs:61-65`, `browser.rs:211`).
- The permission prompt's Allow button may stay greyed out: no recompose after
  `ALLOW_DELAY` (`web.rs:720-726`).
- The find bar has no Escape or Shift+Enter.
- The developer tools can squash the page (range up to 2000).
- The lock button has no tooltips or disabled state on agent tabs.
- Opening the browser doesn't focus the address bar.

**P3**
- The tab strip has no agent look, tooltip or scroll to the active tab.
- No toolbar tooltips.
- Right-click isn't forwarded to the page.
- No focus handoff after menu commands or element picks.
- Reopening find doesn't select the old query.
- The find bar doesn't fit narrow pages.
- The menu isn't clamped horizontally.
- "Open in default browser" errors are swallowed.
- Shortcut key-ups leak to the page.
- An agent's first navigate doesn't show the pane.
- Restore changes the agents' default tab.
- `supported()` is no longer checked.

**OK:** the engine pipeline, agent tools and origin restriction, safety
prompts, zoom, clear data, per-session scoping and restore.

---

## 13. Visual primitives, theme and icons

**P1**
- ~~**Icon rasterisation.**~~ **FIXED.** See top 30, #3, and `icons-zoom.png`: the new icons
  are about 12 px against the old 16 px, thinner, and the bottom-panel glyph
  shows hatching artefacts. The fix lives in both repositories:
  - **Rustle:** draw icons through an anti-aliased raster atlas at
    `size × scale`, or add MSAA or an inner fringe, and snap origins to
    physical pixels.
  - **Pandemonium:** go back to 12, 14 and 16 px glyphs with integer centring.
- ~~**File-type icons no longer come from `languages/*/icon.svg` or icon-theme
  extensions.**~~ **FIXED:** the SVGs are read into outlines once and tinted. A hard-coded Phosphor table replaced them, and the server's
  `icon_themes` go unused (`file_icon.rs:84-127`).
- **No focus ring or keyboard focus on any control.**
- **Tooltips are missing everywhere.**
- **Interface font, code font and density are never applied.**

**P2**
- No pressed state on any control.
- Disabled buttons keep their fill; the old ones dropped to 40% opacity with a
  not-allowed cursor.
- Button variants Outline, Link and Field, and the selected, round, monospace
  and width options, are gone.
- The AccentHover and Sidebar colour roles are gone, so primary buttons don't
  light up on hover.
- The Subtle and Comment colours changed (`#666666` → `#707070`,
  `#6f7378` → `#7a7e83`).
- ~~Folders are grey instead of gold (`#d9a95b`).~~ **FIXED.**
- Spinning icons (busy states) are gone everywhere.

**P3**
- No colour transitions.
- Figures are rasterised at a fixed 2× instead of the window scale
  (`figures.rs:22,328`).
- Button and icon-button heights changed (28/32/36/40 → 22/26/32/38).
- Radii and popover shadows differ.

---

## 14. Client link and protocol coverage

**P1**
- FIXED: **`Workspace.status` has no reader,** so about 25 error paths are
  invisible. The status bar shows it (`status` snapshot).
- FIXED: **Catch-alls drop `Response::Failed`** for every `Ask::Nothing` request
  (verified by `failures_reach_the_status_bar`); attachments are still cleared
  before the send. Was:
  Prompt, Cancel, Set, the queue actions, Choose, Answer, Trust, Rewind, Fork,
  ContinueChat, SaveProjects and WatchFolders (`link.rs:153,981`). Prompt
  attachments are also cleared before the send, so they are lost if it fails.
- **Requests never sent:**
  - FIXED: CloseSession, RenameSession (session tests and snapshots)
  - Restore, UndoTurn, TurnChanges, StopSubagent
  - SetSessionAccount, SignInSession, Identity, Discover, SaveChoices
  - FIXED: Stash, Step, Stashes and SubmoduleHeads are sent; CherryPick from the
    graph's commit menu; DevContainer by the welcome page, which offers the dev
    container only where one is set up
  - Owners, Repositories
  - FIXED: Terminals, AttachTerminal (`shells_of_the_server_are_taken_up`)
  - InspectExtension, InstallExtension, GrantExtension, RunExtensionCommand,
    ExtensionAssets
  - Blame, Problems, FoldingRanges, LinkedRanges, DocumentLinks, ResolveLink,
    InlineValues, FormatOnType, Permalink, FileHistory
  - RunnableTasks, RunRunnable, Toolchains, SelectToolchain
  - DebugProcesses, DebugAttach
  - SearchChats, CreatePromptFile
- FIXED: **Remote servers:** BrowserCall, Diagnostics, FolderChanged,
  TerminalServing, SessionServing and ChatContinued are passed on with their
  numbers and folders mapped; ExtensionsChanged of other machines is left out
  on purpose, since the window lists this machine's extensions. Was dropped
  (`link/remote.rs:202`).

**P2**
- Agent events dropped at `thread.rs:254`: ToolSignIn, Restored, Undone,
  Limits, Goal, Slots and Container.
- Git is never refreshed on Finished, Failed, Restored or Undone.
- FIXED: On reconnect, nothing re-opens LSP documents or re-asks extension
  assets. *`reopen_for_server` opens every local file again and
  `ExtensionAssets` is asked on each connect.*
- FIXED: Disconnect only sets the (unread) status, and requests made while
  disconnected are dropped. *The status bar shows it, and `pm-client` keeps up
  to 256 requests made while down and sends them once the connection is up.*
- `open_branch` searches local sessions only, so pull requests never work for
  remote sessions. The overview skips Log for remote projects.

**P3**
- No progress while a remote server starts, and a failed reach clears the
  backlog silently (`remote.rs:167-170`).
- **`let _ =` swallowing real errors:**
  - FIXED: `window.rs:214,218,254`: saving settings and the view
  - `web.rs:209`: open address
  - `notify.rs:75`
  - `link/quick_input.rs:357-363`: discarded `QuickOutcome`
  - FIXED: `window.rs`'s title-bar Close no longer calls `process::exit`
- **Unused fields:** `Workspace.draft` (`model.rs:44`) is never written or
  read.
- **No-op closures in shipped UI:**
  - `composer.rs:347`: `on_hover(|_| {})`
  - `menu.rs:127`: a disabled menu row
  - the `PageHeader::show(|| {})` empty slots
  - click swallowers in `modal.rs`, `popover.rs`, `dialog.rs`,
    `confirm_dialog.rs` and `browser_safety.rs:42` (intentional)

  The `|_| {}` closures in `pm-pages/src/actions.rs:191-245` and
  `settings_actions.rs:24-27` are `inert()` fixtures, which `window.rs`
  overrides. There are no TODO, `todo!` or `unimplemented!` markers.

---

## 15. What is verified working

- **Window:** the frame (drag, double-click to maximise, minimise, maximise,
  close), the shadow, panel toggles, sashes with saved sizes, the update
  dialog, and the agent-finished notification.
- **Sidebar:** project layout, the chats section, project menu, reorder and
  remove, the folder adder (local and remote), history and the extensions
  list.
- **Conversation:** retry, afresh, hand over, side question, rewind and fork
  requests; approve, inquiry and trust answers; plan progress; markdown with
  mermaid and TeX; a virtualised list kept scrolled to the end.
- **Composer:** Enter and Shift+Enter, Ctrl+Enter while working, pasting files
  and pictures, the field menu, setting picks with `@slot`, and queue actions.
- **Tabs:** preview tab behaviour, dirty dot, Ctrl+Tab and Ctrl+PgUp/PgDn,
  reopen, and splitting with Ctrl+\\.
- **Editor:** find and replace, go to line, the core line edits, the
  completion list, rename, signature help, lens presses, drag to move text,
  change bars, gutter breakpoints, sticky header, minimap, semantic tokens,
  inlay hints and lenses, the compare actions, and the language install offer.
- **Git:** stage and unstage, discard (with confirm), commit and commit-and-
  push, sync and publish, the branch menu, graph lanes, single-line diff notes
  sent to the agent, and the per-region conflict choices.
- **Dock:** terminal input encoding, selection, scrollback, resize, the
  Copy/Paste/Select all/Clear menu, the task list and runs, debug scenarios
  and stepping, and variable expansion.
- **Browser:** the engine, agent tools, safety prompts, find, zoom, the menu,
  tabs and restore.
- **Settings:** search, reset, the narrow layout, theme mode, and every setting
  the server reads.
- **Link:** local `ServerMessage` handling is exhaustive; session event
  replay, remote session numbering and machine settings sync all work.
