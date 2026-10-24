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
and submission, and pointer movement to successful submission. Pointer timings
start at event handling and include coalesced events from the oldest pending
movement. They exclude OS event delivery, compositor queues, scanout and actual
display latency. These are CPU measurements, not GPU timestamp queries.

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
