# AGENTS.md

## Architecture rules

- Read project files and run processes, git, agents and worktree operations on
  the server. Route client actions through state outboxes and protocol
  requests. Every feature must work with remote projects from its first
  implementation.
- Keep agents running when a client closes, sleeps or disconnects. Reconnecting
  clients must be able to catch up on missed session events.
- Treat projects as peers within one window, each belonging to one machine.
  Resolve `ssh://host/folder` on `host`. A session is an agent's worktree with
  files, a diff and a conversation to review.
- Keep dependencies pointing down the layers. Pages use components; components
  never depend on pages. Keep domain work on the server and the `pandemonium`
  binary limited to wiring the layers together.
- Keep wgpu calls in `pm-renderer`. Build interface elements with `pm-ui` and
  the existing styling and component APIs. Use winit and wgpu for portability.
- Give each operation one implementation. Reuse the same path for session
  creation, project resolution and worktree teardown wherever it is called.
- When a control is repeated or needed by a second page, extract it into
  `pm-components` with a builder, variant and size. Pages belong in `pm-pages`.

## Rust conventions

- Keep `lib.rs` as a facade: module declarations, public re-exports and the
  crate's `//!` documentation only.
- Split files by responsibility, not line count.
- Document every item, including private items, with rustdoc. Use `//!` for
  modules and `///` for other items. Put explanations in names, structure and
  documentation; keep function bodies code only.
- Complete renames in one pass, including every use.
- Add dependencies with `cargo add`; do not edit manifests by hand to add them.

## Contracts and generated files

- Keep `pm-protocol` and `pm-plugin-api` append-only. Add fields and variants;
  never remove or repurpose existing ones.
- Update [docs/protocol.md](docs/protocol.md) in the same change as
  `pm-protocol`. Update [docs/extensions.md](docs/extensions.md) when changing
  the extension package format.
- Run `make languages` after changing a language folder to regenerate
  `languages/languages.json` and its checksums.

## Verification

- Put build output and snapshots in the git-ignored `build/` directory. Set
  `CARGO_TARGET_DIR=build` for Cargo and make commands.
- For Rust changes, build the affected targets, run the relevant tests, and
  run `make fmt` and `make lint`. Fix failures caused by the change.
- For UI changes, build first, render the affected state with `--snapshot`,
  and inspect the resulting image. Add a `Shot` in
  `crates/pandemonium/src/snapshot.rs` when the state has none. A draw list or
  successful compilation alone does not verify rendering.
- For settings UI changes, inspect both `settings-<section>` and
  `settings-<section>-narrow`; the narrow window is 640 by 420.
- For documentation-only changes, check links and the diff; builds and UI
  snapshots are unnecessary.

## Reference docs

- Use [README.md](README.md) for setup, build commands and snapshot usage.
- Use [docs/architecture.md](docs/architecture.md) when choosing a crate or
  changing layer boundaries.
- Use [docs/protocol.md](docs/protocol.md) for requests, events and reconnects;
  [docs/extensions.md](docs/extensions.md) for extension packages;
  [docs/browser.md](docs/browser.md) for the embedded browser and its helper;
  and [docs/performance.md](docs/performance.md) for performance work.
- Keep project instructions in `AGENTS.md`. Keep project descriptions, crate
  inventories and long explanations in the reference docs.
