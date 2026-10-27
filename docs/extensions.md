# Extensions

An extension is a folder with an `extension.toml`. It adds languages, themes,
icon themes, debug adapters, tool servers (MCP) and commands to the machine it
is installed on, and may carry a WebAssembly module that runs its commands.
`extensions/hello` is a complete sample; `make extension-sample` builds it into
`build/extensions/hello`, and installing that folder from the extensions page
(Install Extension…) adds everything it carries.

An extension belongs to the server of the machine it is installed on, and
applies only to the projects of that server. The window reads its themes, icon
themes and command names from every server it is connected to.

## The manifest

```toml
name = "Hello"
version = "0.1.0"
description = "What it is for, in a line."
api = 1                          # the plugin API it was written for
wasm = "extension.wasm"          # optional: runs the commands that have no fixed prompt
capabilities = ["process", "read-project", "download"]

[[languages]]
folder = "languages/hello"       # a language folder, as under languages/

[[themes]]
file = "themes/hello-dusk.toml"

[[icon_themes]]
file = "icons/hello.toml"

[[debug_adapters]]
id = "hello-dap"
name = "Hello"
programs = ["python3"]
arguments = ["-m", "debugpy.adapter"]   # {port} stands for the port of an adapter that listens on one
connect = "stdio"                       # or "tcp"
types = ["hello"]                       # what a launch scenario's type may be

[[mcp_servers]]
name = "hello-tools"
target = "npx -y @modelcontextprotocol/server-everything"   # or an http(s) address
env = { TOKEN = "…" }

[[commands]]
id = "hello"
title = "Say hello"
description = "Starts a prompt with a greeting"
prompt = "Say hello to {argument}."     # a command without `prompt` is run by the module
```

- **Languages** are language folders: a `config.toml` and five tree-sitter
  queries, the grammar named by `grammar_file` or carried as `grammar.wasm`. An
  installed extension hands each to the language index as `<extension>-<folder>`,
  so it highlights, indents and starts its language server as a built-in
  language does. A manifest may also hold `patterns`, file names with `*` for
  any run of characters such as `.env.*`; `[server.language_ids]`, the name the
  server is told for the files with an ending (`jsx = "javascriptreact"`);
  `[scope.<name>]`, a set of grammar `nodes` inside which `line_comment`,
  `block_comment` and `emmet` differ; `[tags]`, the node kinds of `element`,
  `open`, `close` and `names` of the tags that are closed as they are opened;
  and `[[companion]]`, a second language server for the files (`suffixes`,
  `scopes`, `detect` with `dependencies` and `files`, and its own `server`),
  whose npm install is pinned by `<pin>.package.json` and
  `<pin>.package-lock.json` in the folder.
- **Themes** name a colour, `#rrggbb` or `#rrggbbaa`, for each role they change,
  in `[dark]` and `[light]`; the rest is the default theme's. They appear in the
  theme settings.
- **Icon themes** list `[[icons]]` with a `suffix`, an SVG `file` and a `color`.
- **Debug adapters** are used by a launch scenario whose `type` is one of
  `types`. An adapter that is one the editor already knows is not replaced.
- **Tool servers** are listed in the agents settings and given to every agent
  that starts.
- **Commands** appear in the command palette and behind the slash of the prompt
  box as `<extension>:<id>`; running one puts what it comes to in the prompt box.

## Lenses

A language folder declares the lenses shown in a row above the lines of its
files with a tree-sitter query, `lenses.scm`, and `[[lens]]` entries in its
`config.toml`; no Rust is needed. The query marks places with
`@lens.<kind>` (the whole place, whose first line gets the lens) and,
optionally, `@name`:

```scheme
(rule (targets (word) @name) (#not-match? @name "^[.%]")) @lens.target
```

```toml
[[lens]]
kind = "target"                  # the <kind> of the capture
title = "Run Target"             # {name} stands for the captured name
command = "make {name}"          # {name} and {file} (the file's name) stand in it
files = ["Makefile"]             # optional: names or globs it is shown above
marker = "Cargo.toml"            # optional: the nearest folder above the file that holds it
toolchain = "javascript"         # optional: the family of toolchain it runs with
once = true                      # optional: only above the first place of its kind
```

Pressing the lens runs `command` in a shell of the project, as a task does, on
the machine the project is on. A `{name}` that is not made of letters, digits
and `_-.:/@+`, or that starts with `-`, gets no lens, so that a name read from a
file is never typed into a shell as more than a word. The server evaluates the
query, so lenses work for projects reached over `ssh://` as for local ones.
`languages/compose`, `json` (for `package.json`), `make` and `dockerfile` are
the examples. `lenses.scm` is listed in `languages.json` like the other
queries, so `make languages` must be run after changing it.

A language lists the languages it needs in `requires`, which are installed with
it and left out of the list when `hidden`.

## Capabilities

Everything is denied until the reader allows it. The manifest lists in
`capabilities` what the extension asks for, the extensions page shows them
before the install and again, as switches, once it is installed, and the
server keeps what was allowed in its machine settings.

| Capability     | Lets the extension                                                              |
| -------------- | ------------------------------------------------------------------------------- |
| `process`      | run programs: its language servers, debug adapters and tool servers, and from its module |
| `download`     | download files: its languages' grammars and language servers, and from its module, and reach its tool servers by address; its module can fetch public addresses only, without following redirects |
| `read-project` | read the files of the project a command runs in, from its module                |

A capability is checked where it is used. A language server, a debug adapter or
a tool server that needs `process` is refused with the reason; a language that
has to download is not installed; a module that asks the host for something it
was not allowed gets the refusal as its answer.

## The module

The module is plain WebAssembly with no imports but one. It exports `memory`,
`pm_alloc(length) -> pointer` and `pm_command(pointer, length) -> packed`, and
imports `pandemonium.host(pointer, length) -> packed`. Messages are JSON in the
module's memory; a `packed` result is the pointer in the high 32 bits and the
length in the low 32.

- `pm_command` is given `{"command", "argument", "project"}` (`project` only
  when `read-project` is allowed) and answers `{"prompt", "message"}`.
- `host` takes `{"op": "process", "program", "args", "cwd"}`,
  `{"op": "download", "url"}` or `{"op": "read-file", "path"}` and answers
  `{"ok": value}` or `{"error": reason}`.

Each call runs in a fresh instance with a budget of work and a ceiling of
memory. The contract is in `pm-plugin-api` and is append-only: a newer editor
loads an older extension, and an extension written for a newer API than the
editor speaks is refused.
