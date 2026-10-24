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

Moving inside the same unpressed tree row skips a redraw when neither the
model nor the scrollbar hover changes and no animation is running. Moving to
another row, dragging, crossing the scrollbar grip and other interactions
still request frames.

## Measure a running window

```sh
PANDEMONIUM_FRAME_TIMINGS=1 CARGO_TARGET_DIR=build cargo run -p pandemonium
```

Every 240 successfully submitted main-window frames, stderr reports p50, p95
and p99 in milliseconds for CPU scene work, surface acquisition plus encoding
and submission, pointer movement to successful submission, and key/IME handling
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
