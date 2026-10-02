# Language support

Open Settings → Languages. Extensions lists the catalogue and what is installed, with an All / Installed / Not Installed choice and the built-in languages folded below; each card has Install, Upgrade or Uninstall and a link to the extension's repository. Install Language Support… offers catalogue packages in the palette as well. Install adds an extension's grammar, highlighting and server declarations to already open files. Missing servers follow the existing Ask / Always / Never preference under Editor → Saving.

Language Settings sets, per language, the tab size, hard tabs, format on save, formatter (language server, an external command or off), organize imports, fix on save, trailing whitespace and final newline. A setting follows the shared preference until it is changed, and the undo mark puts it back. It also lists the language's servers: Add Server and Edit accept an executable, a JSON array of arguments and a JSON object of initialization options, Remove drops an entry and Restore Defaults removes the override. Everything is saved in `settings.yaml`, the settings under `languages` and the servers under `language_servers`. An external formatter is a command line that reads the file on its standard input and writes the laid out file to its standard output; `{path}` stands for the file's path.

Executables on PATH or in the usual tool directories win over managed server copies. Each worktree runs its own server instance. Removing an extension stops its servers but retains user settings and managed binaries so reinstalling can restore the setup.

## Extension format

Extensions live under `$PANDEMONIUM_HOME/extensions/<id>` (default `~/.pandemonium/extensions/<id>`). Import Extension accepts an existing directory. Its name becomes the id: letters, digits, hyphens and underscores only. Assets and manifests are validated before the installed directory is replaced. Invalid updates preserve the working version. Paths must stay inside the package; symbolic links are not supported.

An extension is a directory with an `extension.yaml`:

```yaml
name: Example
version: 0.1.0
publisher: Example Publisher
source: https://example.org/example-language
description: Example highlighting and language server.
languages:
  - name: Example
    language_id: example
    grammar_name: example
    grammar: grammars/example.wasm
    highlights: [queries/highlights.scm]
    extensions: [example]
    file_names: []
    line_comment: '//'
    servers:
      - command: example-language-server
        arguments: []
        options: {}
        install:
          kind: release
          version: '1.2.3'
          builds:
            - platform: linux-x86_64
              url: https://example.org/releases/{version}/example-language-server-linux-x86_64.gz
              sha256: <SHA-256 of the asset>
```

`grammar_name` defaults to `language_id`; set it when the grammar's exported name differs from the LSP identifier. A grammar must be compiled to WASM with a compatible tree-sitter ABI. Capture names use the existing theme vocabulary, including `keyword`, `type`, `attribute`, `function`, `property`, `variable`, `number`, `comment`, `string`, `operator` and `punctuation`.

Servers may still be bare command strings. The invocation format above also works in `settings.yaml`, including `install`. An editor form retains a declaration's recipe when the executable is unchanged. Changing the executable drops that recipe and uses the new command's built-in recipe if available. Arbitrary manually installed executables need no recipe.

Other recipes use the same installers as built-in servers:

```yaml
install: {kind: npm, package: example-language-server, version: 1.2.3, extra: [example-runtime@4.5.6]}
install: {kind: go, module: example.org/language-server, version: v1.2.3}
install: {kind: pip, package: example-language-server, version: 1.2.3}
```

Declare exact versions. Release assets require HTTPS and a SHA-256 per platform. Platform names come from Rust's OS and architecture names: `linux-x86_64`, `linux-aarch64`, `macos-aarch64`, `windows-x86_64`, etc. List only assets that actually exist. The installer checks checksums before unpacking and activates completed server installs atomically under `servers/<command>/<version>`.

Themes and keymaps retain their existing manifest arrays. `indents`, `injections` and `folds` remain reserved. Built-in names, language ids and file associations cannot be replaced by extensions.

## Catalogue and publishing

The maintained index is [`extensions/catalogue.yaml`](../extensions/catalogue.yaml), fetched over HTTPS from this repository's default branch. `PANDEMONIUM_LANGUAGE_CATALOGUE` selects an alternative HTTPS index. Refresh retrieves it again. The client contains no bundled catalogue or extension packages; browsing and installing catalogue extensions requires a network connection. New catalogue entries and package updates are read at runtime and do not require rebuilding the editor.

The index is a YAML array:

```yaml
- id: example
  name: Example
  version: 0.1.0
  publisher: Example Publisher
  source: https://example.org/example-language
  description: Example highlighting and language server.
  platforms: [linux-x86_64, linux-aarch64, macos-aarch64, windows-x86_64, windows-aarch64]
  prerequisites: []
  url: https://example.org/packages/example-0.1.0.zip
  sha256: <SHA-256 of the ZIP>
```

1. Create a directory containing `extension.yaml`, the WASM grammar, queries and upstream licenses. Pin grammar sources to a tag or commit. Build the WASM grammar with tree-sitter 0.27: `tree-sitter build --wasm --output <extension>/grammars/<name>.wasm <grammar-repository>`. The CLI downloads its compiler tools into the user's cache when needed.
2. Import that directory in Settings → Languages and verify highlighting, diagnostics, hover and navigation in an already open file and in a second project/worktree.
3. Give the package a semver version. ZIP its contents with `extension.yaml` at the archive root. Do not include enclosing directories, links, tool caches or build output. Compute `sha256sum <package>.zip` and set the index checksum to that result. Compute release-asset checksums from the downloaded assets separately.
4. Publish the versioned ZIP at an HTTPS URL and add its index entry. Package name, version, publisher and source must match the manifest. `platforms` declares server availability; `prerequisites` lists required tools such as Node.js/npm, Go or Python before installation. Grammar-only extensions may leave `platforms` empty.
5. Submit the package, source and index change for review. For packages maintained here, use a new `extensions/packages/<id>-<version>.zip` for every version and retain old package URLs. Once the index is published, existing editor builds discover the package through Refresh.

## Verification

Run `make lint`, `cargo build -p pandemonium`, then `make run` with a fresh `PANDEMONIUM_HOME` under `~/.cache/scratch/pandemonium/`. Open a file of the language before installing its extension from Languages. Confirm highlighting, the language status, install policy, diagnostics after a type mismatch, definition navigation without reopening the file. Verify persisted server edits and per-language settings, a second worktree, removal and reinstall. Import an invalid query or a package with an incorrect checksum and confirm the previous installation remains usable. Delete the scratch home after verification.
