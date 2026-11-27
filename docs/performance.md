# Interaction performance

The main window retains its node tree and solved layout between interaction
frames. Pointer, focus, selection, animations and theme roles are resolved at
paint time. Model changes, viewport changes, font changes and clock-second
changes rebuild the scene. The clock refresh keeps running tool durations and
relative ages current while frames are being drawn.

The file hierarchy is cached by its exact file and folder lists, including file
ordering, for up to four recently used projects or sessions. Building a new
hierarchy indexes children by name and kind and computes sorting keys once.
The tree constructs row contents only for the viewport plus a two-row margin.
Spacers preserve scroll height and offscreen keyboard identities. Tree rows
have a fixed height at the current density. Inline naming temporarily uses the
complete list because validation messages have variable heights.

Hover, press and focus styling is resolved when the interface is painted, so
moving the pointer repaints without composing the screen again; only a model
change composes it. Spinners read their angle when they are painted, so a
turning spinner paints the window again each frame without composing or
measuring it.

## Interactions on a heavy workspace

```sh
CARGO_TARGET_DIR=build cargo test -p pandemonium benchmark_interactions -- --ignored --nocapture --test-threads=1
```

The benchmarks in `crates/pandemonium/src/app/interactions/benchmarks.rs`
drive the app headless through the interaction rig, which feeds rustle's
`Embedded` interfaces as rustle's event loop feeds a window and wakes the app as
its external handler does, on a heavy workspace (`snapshot::heavy_workspace`): the file shown, the
conversation and the diff side by side in panes, 40 replies of 30 markdown
paragraphs each, and the review panel open on a tree of 400 files. They drag
the sidebar's handle, the review panel's handle and a seam between panes a
pixel a frame for 100 frames, move the pointer over the panes, scroll, type
into the prompt and stream 100 chunks of an agent's reply, and print for each
frame the main window's composing time and layout plus painting time (from
rustle's `Embedded::update_timing`) and the whole turn of the loop, as
p50/p95/max in milliseconds. Add `--release` for the release profile; the
numbers below are from the `dev` profile `make run` uses, which is optimized.

Three causes made every frame of these slow, measured with the benchmarks
and a sampling profile:

- Text was shaped again on every frame. Ellipsized labels, canvases and
  editable and selectable text asked rustle's text engine for a layout while
  composing and again while painting, and nothing kept it: shaping was half
  of a frame. rustle now keeps text layouts per thread by their runs, style,
  width and fonts, and text measured for editing goes through the same layouts.
- A spinner turning anywhere on the screen composed, measured and painted the
  whole window on every frame while an agent worked, and so on every pointer
  move. The spinner reads its angle while it is drawn; rustle now records
  state read while drawing (or measuring) and draws (or lays out) again for
  it without composing.
- Every change, including a pane's width, composed the whole screen. The
  workspace's regions (title bar, sidebar, each pane, the dock, the plan and
  review panels) became blocks keyed by their room, and a handle's drag
  changes only sizes, so the regions whose room did not change keep what they
  emitted. The title bar, the sidebar, the plan panel, the history page and the
  extensions page are now keyed by their own state compared by value, so a
  keystroke or a streamed reply leaves them too; the other regions read the
  screen's state themselves. Panel sizes are held as observable state apart
  from the screen, so a drag writes no screen, and nothing composes the whole
  tree by hand: fonts, ligatures, the theme, window sizes and whether the
  window floats are snapshot state their readers follow.

Measured on the same machine (`dev` profile, milliseconds, p50 / p95 of the
whole turn; before is the earlier interface with text sizes cached as a
window's renderer caches them):

| Interaction | Before | After |
| --- | --- | --- |
| Drag the sidebar's handle | 34.2 / 36.2 | 2.9 / 3.2 |
| Drag the review panel's handle | 38.1 / 56.2 | 5.2 / 6.8 |
| Drag a seam between panes | 55.0 / 57.7 | 5.8 / 9.3 |
| Move the pointer | 22.8 / 24.4 | 0.6 / 0.9 |
| Scroll | 34.1 / 35.7 | 3.2 / 3.3 |
| Type into the prompt | 56.8 / 60.6 | 4.1 / 4.2 |
| Stream an agent's reply | 91.1 / 124.0 | 11.6 / 15.0 |

Medians moved by up to a third between runs while other work loaded the
machine; compare before and after in the same session. The release profile
measured 27.2 ms before and 2.4 ms after for the
sidebar's handle, 41.8 ms and 5.8 ms for a seam, and 72.2 ms and 11.4 ms for
streaming. What costs most now is composing the panes whose width changed
(the conversation and the code view compose their visible rows again) and,
while streaming, composing again every region that reads the whole screen;
then measuring and painting the whole tree, which rustle
caches only per restartable scope.

## Scrolling large views and streaming

`benchmark_interactions_scroll_views` scrolls each large view alone in the
window, 300 frames of 8 pixels (a trackpad) and 100 of 400 (a flick) each
way, and counts the frames over a 120 Hz display's 8.3 ms: a session of 300
messages (`snapshot::long_session`), a diff of 300 files with one of 5,000
added lines (`large_diff`), a highlighted file of 20,000 lines
(`large_code`), the 10,000-row tree (`large_tree`) and the heavy workspace.
`benchmark_interactions_typing` and `benchmark_interactions_streaming` also
run on a long prompt, a long conversation and a long session. They run with
the command above.

The tests in `crates/pandemonium/src/app/interactions/scrolling_large.rs` run
with the ordinary test suite and check the same views, plus the conversation of
the heavy workspace, for correctness: they scroll each one frame by frame in
small steps and in flicks, then to the far end and back. Every frame must keep
the view covered in text with no blank stretch taller than 240 pixels. The text
must move, the end must be reached, and the start must look as it did, each
run within a pixel of where it was. A list places its items at fractions of a
pixel from where its estimated end puts them, so a run can round to the next
pixel once more items are measured; that was so before these changes too.

What made these slow, found with a sampling profile of the benchmarks:

- A composition local provided inside a skippable block (the selection
  container of the conversation, every lazy list item) was checked against the
  value outside it, so the block never skipped: every message and markdown
  block was composed again for each streamed word. A provider now keeps the
  reads of its own local from the blocks around it.
- A lazy list read its scroll offset while composing, so every scrolled
  pixel composed the pane holding it again. It now reads the offset untracked,
  composes a quarter of a viewport beyond each end, and composes again only when the
  viewport leaves what it composed. The code view follows its top row with
  `rustle::watched_state_of`, which changes only when the row does.
- Each scroll of the diff or the conversation wrote the screen, to remember
  where it was, which composed every region again. A scroll is now followed at
  once only when it must be (the diff is anchored to a file or has diffs to
  read) and otherwise once the lists rest for 150 ms.
- The diff worked out the rows of every file on each composition. Its rows
  and items are kept until a file, a fold or a note changes, its syntax
  colours with them, and each row is a `rustle::kept` block, whose layout is
  kept while it is skipped.
- Text set again searched the fonts for every run and shaped every word
  again. The weights found are kept, and shaped words are kept by cosmic-text's
  shape run cache, so a growing paragraph shapes only its new words.

Measured on the same machine (release profile, milliseconds, p50 / p95 of the
whole turn):

| Interaction | Before | After |
| --- | --- | --- |
| Scroll the large diff, trackpad | 7.6 / 10.2 | 0.8 / 1.2 |
| Scroll the large diff, flick | 10.1 / 16.2 | 2.9 / 6.1 |
| Scroll the large file, trackpad | 2.9 / 6.0 | 1.6 / 5.7 |
| Scroll the large file, flick | 4.6 / 7.8 | 4.3 / 7.5 |
| Scroll the long session, trackpad | 0.2 / 1.8 | 0.3 / 0.5 |
| Stream into the heavy workspace | 13.5 / 19.2 | 7.5 / 10.4 |
| Stream into a long conversation | 9.9 / 18.2 | 4.7 / 8.1 |
| Type beside a long conversation | 4.5 / 5.6 | 2.4 / 2.9 |

What costs most now while streaming is laying out and painting the whole
tree: the panes beside the conversation read the whole screen, so each
streamed word composes and measures them again, and painting walks every node.
The snapshot benchmarks below compose the screen from nothing each frame, so
they measure a first frame, not the retained interface.

## Frames within a native editor's budget

`benchmark_interactions_resize` resizes the main window three pixels a frame
both ways at once, as a desktop dragging its corner does, on the heavy
workspace and on a huge file. It runs with the command above.

What a sampling profile of every interaction benchmark found, and what
changed:

- The minimap emitted a line of marked text for each of its rows, a few
  hundred text nodes measured and painted on every scrolled frame, with a
  selection worked out per block. It now draws its blocks as rectangles on one
  canvas. rustle works out selection rectangles without counting the
  characters of clusters a selection covers whole.
- The title bar and the sidebar were keyed by the screen's generation, which
  every keystroke moves on, so typing composed both again. They are keyed by
  their own state, compared by value; the workspace still follows the chrome's
  generation, so a streamed word that changes them shows.
- The window was laid out on each size the desktop sent, and again after
  composing what the size changed. rustle now lays out once a frame, after
  composing, however many sizes arrived.
- Freeing the measured tree and display list a frame replaced was up to a
  fifth of laying out. rustle drops them once the frame is handed to the GPU.
- The window allocates through mimalloc, whose thread-local free lists make the
  short-lived allocations of composing, measuring and painting cheaper than the
  system allocator's. The benchmarks' counting allocator forwards to it, and
  forwards growing in place, so they time what the window does.
- Whether the browser helper is installed was looked up on disk, with the
  program's own path read, several times every turn of the loop; the answer is
  now kept for half a second.
- rustle draws text from glyph atlases of its own: a shaped text keeps where
  its glyphs are drawn and where they are in the atlas, so a frame copies them
  instead of looking each glyph up. Frames come out the same to the pixel.

Measured on the same machine (release profile, milliseconds, p50 / p95 of the
whole turn; the window's GPU work is not in these):

| Interaction | Before | After |
| --- | --- | --- |
| Drag the review panel's handle | 7.35 / 10.55 | 2.57 / 3.67 |
| Drag a seam between panes | 4.07 / 6.76 | 2.14 / 2.50 |
| Scroll the huge file, trackpad | 4.25 / 7.43 | 0.49 / 1.09 |
| Scroll the huge file, flick | 4.40 / 11.54 | 1.28 / 1.75 |
| Scroll the large file, flick | 3.86 / 4.71 | 1.54 / 2.48 |
| Stream into the heavy workspace | 3.33 / 4.54 | 2.06 / 3.03 |
| Stream into a long conversation | 2.32 / 3.22 | 1.34 / 1.72 |
| Type into the prompt | 3.47 / 4.45 | 2.28 / 2.79 |
| Type into a long prompt | 1.03 / 1.63 | 0.50 / 0.54 |
| Resize the window, heavy workspace | 6.78 / 7.87 | 5.40 / 6.06 |
| Resize the window, huge file | 3.22 / 3.97 | 2.34 / 2.88 |

The huge file's flick missed 42 of 200 frames of a 120 Hz display before and
none after. rustle's `frame` benchmark (`cargo bench -p rustle-bench --bench
frame` in rustle) measures recording a frame and the GPU drawing it: a screen
of highlighted code went from 0.90 ms to 0.12 ms a frame. Resizing still
composes the panes again, because a pane is keyed by its size.

## Measure a running window

```sh
PANDEMONIUM_FRAME_TIMINGS=1 CARGO_TARGET_DIR=build cargo run -p pandemonium
```

Every 240 composed main-window frames, stderr reports p50, p95 and p99 in
milliseconds for composing and laying out the screen, the remaining time to
the end of the frame, pointer movement to successful submission, and key/IME handling
to successful submission. Pointer timings
start at event handling and include coalesced events from the oldest pending
movement. They exclude OS event delivery, compositor queues, scanout and actual
display latency. Keyboard timings begin before applying the edit and include
coalesced input through the next successful submission. These are CPU
measurements, not GPU timestamp queries.

For a hardware-specific comparison, run the same workload twice:

```sh
PANDEMONIUM_FRAME_TIMINGS=1 PANDEMONIUM_FRAME_LATENCY=2 CARGO_TARGET_DIR=build cargo run -p pandemonium
PANDEMONIUM_FRAME_TIMINGS=1 PANDEMONIUM_FRAME_LATENCY=1 CARGO_TARGET_DIR=build cargo run -p pandemonium
```

The override accepts only `1` or `2`; the default remains `2`. Keep vsync on.
Compare slow-frame percentiles while moving across a large expanded tree,
scrolling, and receiving agent output. A smaller queue can reduce latency but
can also reduce throughput, so choose it from measurements on the target GPU
and compositor. Headless snapshots cannot establish this tradeoff.

## Streaming latency

Agent output reaches the window without timers. The agent's threads ring the
server's bell as soon as they queue an event, and the pump runs on the ring; its
500 ms wait is only a backstop. Each connection sleeps in a poll on its socket
and on a waker, on the server's side and on the client's. Queueing a message
for the client, or a request for the server, rings that waker, so it is written
at once instead of at the next 25 ms read timeout. The client's connection
thread calls a wake function after each report, rustle's `external_waker`, which
wakes the event loop to run the app's external handler, so incoming events are applied at once instead of at the
next 50 ms tick while an agent works.

Measured over a loopback WebSocket with an agent streaming one chunk every
10 ms, the delay from the agent's write to the client's read fell from p50 13 ms,
p95 24 ms and stalls of up to 276 ms to p50 1.4 ms, p95 2.8 ms and a maximum of
7 ms, with chunks arriving evenly spaced instead of in bursts.

## Reproducible CPU comparisons

```sh
CARGO_TARGET_DIR=build cargo test -p pm-components -p pandemonium benchmark_ -- --ignored --nocapture
```

The tree benchmark compares a complete 10,000-row list against virtual rows,
including construction, layout and painting. The screen benchmark compares
rebuilding a large-tree workspace against retaining its geometry, and prints
p50/p95/p99. Neither benchmark submits GPU work or moves the system pointer.

## Render fixtures

```sh
CARGO_TARGET_DIR=build cargo build -p pandemonium
mkdir -p build/performance
build/debug/pandemonium --snapshot build/performance/files-large.png 700 1200 230 0 dark files-large
build/debug/pandemonium --snapshot build/performance/files-scrolled.png 700 1200 230 0 dark files-scrolled
build/debug/pandemonium --snapshot build/performance/naming.png 700 1200 230 0 dark naming
```

`files-large` expands 100 folders with 100 files each; `files-scrolled` shows the
same tree halfway down. Ordinary `files`, `treepicked`, `treemenu`, `naming`,
`code`, and settings snapshots also exercise the shared painting path.


## Typing and large text

Completed message layouts are retained independently of the prompt. Streaming
text, tool state, expanded cards, markdown details, figures, attachments,
fonts and density invalidate the affected layout. Running tool timers refresh
on clock changes. Offscreen retained text replays compact selection and focus
metadata, preserving copying across the conversation without traversing all
of its styled nodes on every key.

Multiline inputs construct at most the eight visible lines plus a three-line
margin. Their complete wrapped byte ranges remain available for keyboard
navigation; moving vertically shapes only the current and destination lines.
Selection and IME composition stay in the field, and the viewport follows the
caret using the actual scaled font height.

Wrapping is cached per paragraph, width and font identity. An edit rewraps its
changed paragraph; caret motion reuses wrapping. Long unbroken tokens use
bounded prefix probes instead of repeatedly shaping large remaining suffixes.
The shaped-run and paragraph caches use bounded generations to retain hot
entries while old edits expire. Figure discovery also survives prompt edits.

Fields track text changes with a revision. Grouped typing no longer copies the
entire input twice per key for undo comparison, and unlimited fields no longer
count all Unicode characters for every insertion. Pointer, clipboard and
vertical-navigation queries borrow the needed state instead of cloning the
whole workspace.

Window-state capture waits for a 300 ms pause, with a two-second maximum during
continuous input. A single background worker serializes and writes it, keeping
only the latest queued view and preserving write order. Normal window shutdown
captures and flushes the final draft.

Run the four typing workloads independently of GPU presentation:

```sh
CARGO_TARGET_DIR=build cargo test -p pandemonium benchmark_typing -- --ignored --nocapture
```

The long prompt contains about 90 KB of Unicode text. The long conversation
contains 40 replies with 100 paragraphs each. A local optimized development
build measured these warm edit/build/layout/paint times (milliseconds):

| Workload | Before p50 / p95 | After p50 / p95 |
| --- | --- | --- |
| Short prompt, short conversation | 0.66 / 0.81 | 0.59 / 0.79 |
| Long prompt, short conversation | 4.14 / 4.43 | 0.54 / 0.65 |
| Short prompt, long conversation | 7.82 / 7.93 | 0.88 / 0.97 |
| Long prompt, long conversation | 11.06 / 11.58 | 0.93 / 1.04 |

These are local synthetic CPU comparisons; they exclude cold startup, OS event
delivery, GPU work, compositor queues and display scanout. Use the running
window's `key-to-submit` measurements for the real workload.

```sh
mkdir -p build/performance-input
build/debug/pandemonium --snapshot build/performance-input/input-stress.png 100 700 600 320 dark input-stress
build/debug/pandemonium --snapshot build/performance-input/input-scrolled.png 100 700 600 320 dark input-stress-scrolled
build/debug/pandemonium --snapshot build/performance-input/input-composing.png 100 700 600 320 dark input-stress-composing
```

## Startup

```sh
PANDEMONIUM_STARTUP_TIMINGS=1 CARGO_TARGET_DIR=build cargo run --release -p pandemonium
```

Stderr reports the milliseconds since the process began at each phase: the
saved settings, fonts and the model, the window, the renderer,
the first presented frame and the first listing of this machine's sessions.

The window opens on what this machine has saved (projects, choices, accounts,
the view it was left on). It no longer waits for the server on this machine.
The connection, starting the server when it is not running, the session
listing, the extension list and the agent list are all requests that run in
the background and update the window as they are answered. A server that cannot
be reached leaves the window as it opened. Projects on other machines are
reached only when something asks them for work, so an unreachable `ssh://`
project cannot delay the window.

Measured on a local release build with an isolated data folder, two local
projects, milliseconds from process start (three runs each):

| Phase | Before, cold server | After, cold server | Before, warm server | After, warm server |
| --- | --- | --- | --- | --- |
| Server answered | 122-147 | in background | 22-122 | in background |
| Window created | 150-193 | 31-46 | 72-152 | 34-47 |
| Renderer ready | 328-389 | 154-214 | 231-291 | 180-196 |
| First frame presented | 381-443 | 210-272 | 286-358 | 241-251 |

With two unreachable `ssh://` projects added, the warm first frame was 252-337
before and 231-246 after.

The GPU opens on its own thread, started before rustle's event loop. The window
builds its first scene while it opens, so the device is ready by the time the
scene is. The instance asks only for the primary graphics interfaces (Vulkan,
Metal, DX12), so no OpenGL, Mesa or LLVM driver is loaded. Together these took
the first frame from about 215 ms to 105-127 ms and resident memory from about
353 MB to about 212 MB on the test machine, where 77 MB of what remains is the
GPU vendor's shader compiler.

## Idle and streaming

An idle window draws nothing and the app sleeps: what waits on time is an
effect keyed on its deadline that waits with rustle's `delay`, a timer the loop
sleeps until without drawing, and polling (the browser's pump, an agent being
heard) runs only while that work is under way. A blinking caret asks for a frame at its next
change, not for continuous frames, and the input method is told where the caret
is only when it moves. Telling it every frame made the desktop answer with an
empty composition each time, which dirtied the model and drew another frame, at
about 210 frames and 520 wakeups a second with 49% of a core in use. Idle now
uses about 1% of a core and draws no frames.

Retained subtrees are released by an estimate of their bytes as well as their
count. A reply that grows by a word at a time builds a new subtree for each
word, and the old ones used to stay until 256 had piled up: 220 MB for 300
words of one reply, 15 MB now. Layout allocates its tree once.

```sh
CARGO_TARGET_DIR=build cargo test -p pandemonium benchmark_streaming -- --ignored --nocapture --test-threads=1
```

The benchmark streams 300 chunks into a short chat, a 40-reply chat and a chat
with paragraph breaks, and prints build-to-paint p50/p95/p99, the bytes
allocated per frame, and what the frames left retained.

A request sent after a quiet spell and its answer, measured over loopback, took
a median of 14.95 ms (up to 17 ms) while the client looked at its orders every
25 ms. It takes 0.11 ms against a stub server and 0.32 ms through the real
connection code. The tests are `a_request_after_a_quiet_spell_is_answered_quickly`
in `pm-client` and `answers_a_request_after_a_quiet_spell_quickly` in
`pm-server`.

## Memory on Linux

On Linux every heavy process the app starts runs in a memory-capped systemd
scope of its own, so that running out of memory stops the process that grew
and never the window:

| Scope | Holds | Cap | Past the cap |
| --- | --- | --- | --- |
| `pandemonium-server-<pid>-<n>.scope` | the server and what it starts: agents, their builds and tools, terminals, tasks | 85% of memory | the largest process the server started is killed; the server goes on |
| `pandemonium-<program>-<pid>-<n>.scope` | one language server, debugger or the browser helper | half of memory | that helper is killed |

Before the server scope reaches its cap, a guard thread in the server kills
the largest process it started at nine tenths of the cap, and marks every
process it started as preferred over the server by the kernel's
out-of-memory killer. A language server that is killed is started again with
its open files; one killed again within ten minutes is left stopped, and an
agent asking about it is told why.

An agent's conversation starts language servers only when it first asks
something of them, and lets go of them after ten minutes without asking, so
that a server no open file uses stops.

`systemctl --user list-units 'pandemonium-*'` lists the scopes, and
`journalctl -k | grep 'Killed process'` shows what the kernel stopped. Set
`PANDEMONIUM_UNCONFINED=1` to start everything without scopes. Machines
without a user systemd start everything as before.

## Browser frames

A page's paint used to be turned from BGRA to RGBA by a JavaScript loop over
every pixel, written whole into its shared file, copied out by the window into
a new buffer and copied again for the renderer. The helper now writes the
engine's bytes as they are and only the rows that changed (see
[browser.md](browser.md)). On one 1080p paint, measured by the last test of
`helpers/electron-browser/test/surface.test.js` with `npm test`, the helper's
side went from about 4.2 ms (swizzle and whole write) to about 0.7 ms for a
whole repaint and under 0.05 ms for a blinking caret.

On the window's side nothing reorders bytes any more: the renderer keeps a
page in a BGRA texture (`rustle::PixelFormat::Bgra8`). Each `Painted` report
carries the paint's dirty rectangle, the window copies only the rectangle the
paints since its spare buffer changed, and the renderer writes only the
rectangle that changed since the picture its texture holds
(`ImageData::changed_since`, `queue.write_texture` of that rectangle). Two
ignored tests measure it, on a machine busy with other builds, so the numbers
are ranges over runs:

```sh
CARGO_TARGET_DIR=build cargo test -p pm-browser --release -- --ignored --nocapture surface_read_cost
cargo test -p rustle-wgpu --release -- --ignored --nocapture live_picture_upload_cost   # in rustle
```

| Per paint, window side | 1080p before | 1080p after | 4K before | 4K after |
| ---------------------- | ------------ | ----------- | --------- | -------- |
| Copy out of the file   | 0.8-2.6 ms (whole, swapped) | 0.3-0.6 ms whole, 0.0001 ms caret | 5.4-7.8 ms | 2.6-3.8 ms whole, 0.0001 ms caret |
| Upload to the texture  | 1.0-1.3 ms (whole RGBA) | 1.0-1.4 ms whole BGRA, 0.03 ms caret, 0.14-0.18 ms a tenth of the rows | 4.1-4.8 ms | 4.0-5.0 ms whole, 0.02-0.06 ms caret, 0.44-0.53 ms a tenth |

A whole repaint now costs about the copy and the upload of the picture once
each (about 1.3-2 ms at 1080p, 6.6-9 ms at 4K, against 1.8-3.9 ms and
9.5-12.6 ms before); a caret blinking, a spinner or a hover costs a few
hundredths of a millisecond instead of a whole picture.

The window still copies the changed rectangle once out of the shared file
before the renderer uploads it. The renderer uploads later, on its own frame,
while the helper may already be writing the next paint into that file, so the
copy is what lets the reader check the file's counter and throw away a torn
picture; with only the changed rectangle copied it costs little.

### Shared GPU textures

Electron 45 can paint offscreen into a GPU texture instead of a bitmap
(`webPreferences.offscreen.useSharedTexture`, the `paint` event's
`texture.textureInfo.handle`). It is not used, for these reasons:

- The handle is local to the helper. On Linux it is a set of dmabuf file
  descriptors (`nativePixmap.planes[].fd`), which reach another process only
  over a Unix socket with `SCM_RIGHTS` or with `pidfd_getfd`; the window talks
  to the helper over loopback TCP, on every system alike. On macOS it is an
  `IOSurfaceRef` that Electron documents as not global, which reaches another
  process only through a Mach port, which Node cannot send without a native
  module of our own. On Windows it is an NT handle to be duplicated into the
  window's process.
- wgpu has no portable way to import such memory. It needs unsafe backend code
  per platform: Vulkan with `VK_EXT_external_memory_dma_buf` and DRM format
  modifiers (and something else again on the GL backend) on Linux, Metal
  textures made from an IOSurface on macOS, and D3D12 shared handles on
  Windows, each brought into wgpu with `create_texture_from_hal`, and rustle
  would have to expose that to its users.
- Every texture has to be given back to the engine (`texture.release()`) soon,
  or the engine runs out and stops painting; across two processes that is a
  round trip per paint, and the window would have to wait on a GPU fence the
  handle does not carry before sampling it.

The bitmap path now moves only what changed, so a page painting steadily
costs well under a millisecond a frame outside whole repaints. Shared
textures would remove the copies of whole repaints and of video, and are worth
it only with a native module in the helper and per-backend import code in
rustle, behind this path as the fallback.
