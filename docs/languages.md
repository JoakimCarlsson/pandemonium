# Language support

Open Settings → Languages, or run Manage Languages from the command palette, to search installed languages and the catalogue. Install Language Support… offers catalogue packages in the palette as well. Each package shows its publisher, source, version, supported server platforms and external prerequisites before installation. Install adds its grammar, highlighting and server declarations to already open files. Missing servers follow the existing Ask / Always / Never preference under Editor → Saving. Install Server explicitly installs a server regardless of that automatic policy.

Add Server and Edit accept an executable, a JSON array of arguments and a JSON object of initialization options. Save persists the effective server list in `settings.yaml`. Remove disables that entry; Restore Defaults removes the override. Executables on PATH or in the usual tool directories win over managed server copies. Each worktree runs its own server instance. Removing an extension stops its servers but retains user settings and managed binaries so reinstalling can restore the setup.

## Extension format

Extensions live under `$PANDEMONIUM_HOME/extensions/<id>` (default `~/.pandemonium/extensions/<id>`). Import Local Extension accepts an existing directory. Its name becomes the id: letters, digits, hyphens and underscores only. Assets and manifests are validated before the installed directory is replaced. Invalid updates preserve the working version. Paths must stay inside the package; symbolic links are not supported.

The complete WGSL example is in [`extensions/wgsl`](../extensions/wgsl/extension.yaml):

```yaml
name: WGSL
version: 0.1.0
publisher: Pandemonium
source: https://github.com/JoakimCarlsson/pandemonium/tree/main/extensions/wgsl
description: WGSL highlighting and wgsl-analyzer diagnostics and navigation.
languages:
  - name: WGSL
    language_id: wgsl
    grammar_name: wgsl_bevy
    grammar: grammars/wgsl_bevy.wasm
    highlights: [queries/highlights.scm]
    extensions: [wgsl]
    file_names: []
    line_comment: '//'
    servers:
      - command: wgsl-analyzer
        arguments: []
        options: {}
        install:
          kind: release
          version: '2026-09-30'
          builds:
            - platform: linux-x86_64
              url: https://github.com/wgsl-analyzer/wgsl-analyzer/releases/download/{version}/wgsl-analyzer-x86_64-unknown-linux-gnu.gz
              sha256: 4635d9d804c9609dc7639632c14d9b6cf888b21f3c95d41c1dc55a33dc76f98e
```

`grammar_name` defaults to `language_id`; set it when the grammar's exported name differs from the LSP identifier. A grammar must be compiled to WASM with a compatible tree-sitter ABI. Capture names use the existing theme vocabulary, including `keyword`, `type`, `attribute`, `function`, `property`, `variable`, `number`, `comment`, `string`, `operator` and `punctuation`.

Servers may still be bare command strings. The invocation format above also works in `settings.yaml`, including `install`. An editor form retains a declaration's recipe when the executable is unchanged. Changing the executable drops that recipe and uses the new command's built-in recipe if available. Arbitrary manually installed executables need no recipe.

Other recipes use the same installers as built-in servers:

```yaml
install: {kind: npm, package: example-language-server, version: 1.2.3, extra: [example-runtime@4.5.6]}
install: {kind: go, module: example.org/language-server, version: v1.2.3}
install: {kind: pip, package: example-language-server, version: 1.2.3}
```

Declare exact versions. Release assets require HTTPS and a SHA-256 per platform. Platform names come from Rust's OS and architecture names: `linux-x86_64`, `linux-aarch64`, `macos-aarch64`, `windows-x86_64`, etc. List only assets that actually exist. WGSL's pinned release has no Intel macOS build. The installer checks checksums before unpacking and activates completed server installs atomically under `servers/<command>/<version>`.

Themes and keymaps retain their existing manifest arrays. `indents`, `injections` and `folds` remain reserved. Built-in names, language ids and file associations cannot be replaced by extensions.

## Catalogue and publishing

The maintained index is [`extensions/catalogue.yaml`](../extensions/catalogue.yaml), fetched over HTTPS from this repository's default branch. `PANDEMONIUM_LANGUAGE_CATALOGUE` selects an alternative HTTPS index. Refresh Catalogue retrieves it again. The shipped index and packages provide a fallback when the default index is unreachable, so the first installation does not depend on publishing the current editor build. New catalogue entries and package updates are read at runtime and do not require rebuilding the editor.

The index is a YAML array:

```yaml
- id: wgsl
  name: WGSL
  version: 0.1.0
  publisher: Pandemonium
  source: https://github.com/JoakimCarlsson/pandemonium/tree/main/extensions/wgsl
  description: WGSL highlighting and wgsl-analyzer diagnostics and navigation.
  platforms: [linux-x86_64, linux-aarch64, macos-aarch64, windows-x86_64, windows-aarch64]
  prerequisites: []
  url: https://raw.githubusercontent.com/JoakimCarlsson/pandemonium/main/extensions/packages/wgsl-0.1.0.zip
  sha256: <SHA-256 of the ZIP>
```

1. Create a directory containing `extension.yaml`, the WASM grammar, queries and upstream licenses. Pin grammar sources to a tag or commit. Build the WASM grammar with tree-sitter 0.27: `tree-sitter build --wasm --output <extension>/grammars/<name>.wasm <grammar-repository>`. The CLI downloads its compiler tools into the user's cache when needed.
2. Import that directory in Settings → Languages and verify highlighting, diagnostics, hover and navigation in an already open file and in a second project/worktree.
3. Give the package a semver version. ZIP its contents with `extension.yaml` at the archive root. Do not include enclosing directories, links, tool caches or build output. Compute `sha256sum <package>.zip` and set the index checksum to that result. Compute release-asset checksums from the downloaded assets separately.
4. Publish the versioned ZIP at an HTTPS URL and add its index entry. Package name, version, publisher and source must match the manifest. `platforms` declares server availability; `prerequisites` lists required tools such as Node.js/npm, Go or Python before installation. Grammar-only extensions may leave `platforms` empty.
5. Submit the package, source and index change for review. For packages maintained here, use a new `extensions/packages/<id>-<version>.zip` for every version and retain old package URLs. Once the index is published, existing editor builds discover the package through Refresh Catalogue.

The WGSL grammar is built from `tree-sitter-grammars/tree-sitter-wgsl-bevy` tag `v0.1.4`, commit `d9306a798ede627001a8e5752f775858c8edd7e4`. Its MIT license is included. That tag has no highlight query; the packaged query uses the grammar's node names and Pandemonium's existing captures. No native WGSL grammar crate is linked into the editor.

The pinned wgsl-analyzer `2026-09-30` advertises hover but [its implementation returns no result](https://github.com/wgsl-analyzer/wgsl-analyzer/blob/2026-09-30/crates/ide/src/hover.rs). Struct-field type hover is therefore unavailable with this release; [upstream #362](https://github.com/wgsl-analyzer/wgsl-analyzer/issues/362) tracks hover work. Diagnostics and go-to-definition work. Update the extension's pinned server recipe once upstream supplies hover; the editor's existing hover path needs no language-specific change.

## Verification

Run `make lint`, `cargo build -p pandemonium`, then `make run` with a fresh `PANDEMONIUM_HOME` under `~/.cache/scratch/pandemonium/`. Open a shader before installing WGSL from Languages. Confirm highlighting, the WGSL status, install policy, diagnostics after a type mismatch, field hover and definition navigation without reopening the file. Verify a `wgsl` fence, persisted server edits, a second worktree, removal and reinstall. Import an invalid query or a package with an incorrect checksum and confirm the previous installation remains usable. Delete the scratch home after verification.
