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

## Language modules

Data covers most of a language. For what it cannot say, an extension that
carries languages ships a WebAssembly module (`wasm` in `extension.toml`, with
`api = 2`) that exports `pm_language` next to the command export. The host calls
it with a `LanguageCall` and it answers with a `LanguageReply`, both JSON, both
typed in `pm-plugin-api`; a module written in Rust uses those types directly, as
`extensions/hooks-guest` does. Every call names the language, by its folder's
name in the extension (`<extension>-<folder>`). A module answers `unsupported`
for a call it does not know, and the host falls back to the language's data.

| Hook | The host asks | The module answers |
|------|---------------|--------------------|
| `server-command` | how to start the language server in the project | `command`: the program, its arguments and environment; used in place of finding or installing the server |
| `latest-version` | the latest version of a dependency | `version`, or nothing |
| `behind` | whether a requirement is behind a version | `replacement`: what to write instead, or nothing |
| `tasks` | the tasks of the project | `tasks` |
| `toolchains` | the toolchains of the machine and project | `toolchains` |

`latest-version` and `behind` are used when `[dependencies]` says `hook = true`,
with `dependencies.scm` still saying where a manifest names its dependencies,
so a language with decimal versions, or a registry that does not answer JSON,
needs no change to Pandemonium. A module asks the host for what it needs with
`HostCall`s, each gated by a capability the reader grants: `process` runs a
program or finds one with `which`, `download` fetches a page, and `read-project`
reads a file; the project is named in a call only when the extension may read it.
Modules run in the same sandbox as commands, under the same limits.

What a language's data and module may make the machine run follows the same
grants. For a language an extension carries, `[[toolchain]]` entries (which run
programs to find versions), `private.read`, a `kind = "command"` install and the
`tasks`, `toolchains` and `server-command` hooks are all refused unless the
extension was granted `process`. What a module says the latest version is, or
what to write instead, is accepted only as a version (letters, digits and
`._+~^=<>-`, at most 64 characters), and a task's folder may not leave the
project. A lens or task a language declares is a command the reader presses to
run, as a runnable is, and is not gated.

A script that has no ending is told by its first line: `first_line_pattern` in
the language's `config.toml` is a pattern matched against it, such as
`^#!.*\bperl\b`. A language's name or ending is tried first. In `[[runnable]]`,
`{file}` stands for the name of the file and `{path}` for its path from the
folder the command runs in, besides `{name}`, so that `prove -v {path}` runs a
test file; a path that is not made of letters, digits and `_-./@+` is refused.

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

## Dependencies

A language that reads manifests declares how to check their dependencies for
newer versions: a query, `dependencies.scm`, and a `[dependencies]` table.
The query captures `@name` and `@version` of each dependency, and a place
captured as `@skip` takes the dependencies inside it out (a crate from git):

```scheme
(require_spec (module_path) @name (version) @version)
((require_spec) @skip (#match? @skip "//[ \t]*indirect"))
```

```toml
[dependencies]
url = "https://proxy.golang.org/{name}/@latest"   # {name} stands for the name
latest = ["Version"]             # where in the JSON answer the version stands, tried in order
escape = [["/", "%2f"]]          # optional: what is found in a name and what it is written as instead
capitals = "!"                   # optional: written before a capital letter, which is then lower case
requirement = '^(?<prefix>[\^~=v ]*)(?<version>[0-9]+(?:\.[0-9]+)*)(?<pre>-[0-9A-Za-z.-]+)?$'
files = ["go.mod"]               # optional: names or globs it is for

[dependencies.private]           # when a name is not sent to the registry
read = ["go", "env"]             # prints the value of each variable below, one to a line
patterns = ["GOPRIVATE", "GONOPROXY"]   # variables of comma-separated private names or path globs
hosts = ["GOPROXY"]              # variables of hosts: private unless the registry's is the only one
ini = ["~/.npmrc", ".npmrc"]     # key=value files that can name another registry; a relative one is looked for in the manifest's folder and every folder above it, ~/ is the home folder and $NAME/ the folder a variable names
ini_keys = ["registry", "{scope}:registry"]
```

`requirement` is how the language writes versions: its `version` group is
dot-separated numbers, `prefix` what stands before them (`^`, `~`, `v`) and
`pre` a pre-release tag. A requirement it does not match, such as a range, a
tag, a path or an address, is left alone, and the registry's answer is read
with the same pattern. A version is behind when the registry's is higher in the
numbers the manifest wrote, and the upgrade keeps the prefix. The answer must
be JSON. The server
asks, keeps what it learns for an hour and shows `Latest <version> · Upgrade`
above the line, which writes the version in place when pressed. Nothing about a
kind of manifest is in the server: `languages/gomod`, `cargo` and `package-json`
are the examples.

## Tasks and toolchains

A language declares the tasks a project has and the toolchains its machine
keeps; the server knows no language, so a language that is not installed adds
none. `family` names the toolchains the language's files are built and run with
(`javascript` for TypeScript, as the folder names of several languages may
share one); it is the language's own folder name when it is not given.

```toml
family = "javascript"

[[task]]
name = "npm: {name}"             # {name} stands for a generated task's name
command = "npm run {name}"
group = "run"                    # build, test or run
files = ["package.json"]         # at least one must exist in the project's folder
requires = []                    # all must exist
absent = ["pnpm-lock.yaml"]      # none may exist
text = [["package.json", "pytest"]]   # [file, pattern] the pattern must match
no_text = []                     # [file, pattern] the pattern must not match
groups = { build = ["build", "compile"] }   # group of a generated task, by name

[task.each]                      # one task for each name: json, toml or regex
json = "scripts"                 # the keys of this JSON object
# toml = ["project.scripts"]    # the keys of these TOML tables
# regex = '(?m)^(?<name>[a-z]+):' # the `name` group of every match

[[toolchain]]
system = [["node", "--version"]] # programs that print their version; used unless `list` worked
list = ["rustup", "toolchain", "list"]   # one toolchain a line: its id, then a description
children = ["~/.nvm/versions/node"]     # folders whose folders are each a toolchain
fixed = ["/usr/local/go"]        # folders that are each a toolchain
project = [".venv"]              # folders of the project, never run to ask a version
bin = ["bin"]                    # where a toolchain's programs are, tried in turn
program = "node"                 # what must be in that folder
name = "nvm {name}"
env = { VIRTUAL_ENV = "{folder}" }       # {id}, {name} and {folder} stand in it
```

A generated name that is not made of letters, digits and `_-.:/@+`, or that
starts with `-`, makes no task, so that a name read from a file is never typed
into a shell as more than a word. Tasks that appear more than once by name are
listed once, the first in the order the languages are read. `languages/cargo`,
`package-json`, `make`, `gomod`, `python`, `rust` and `go` are the examples.

## Installing a language server

A language server that is not on the machine is installed into the server's
data folder by the way `[server.install]` says. `kind` is `download`, `go`, `npm`
or `command`, which runs a program the manifest names, found on the machine and
run without a shell, so that a language can use `gem`, `cpan` or any other tool
without a change to Pandemonium:

```toml
[server.install]
kind = "command"
version = "0.58.0"
command = ["gem", "install", "--no-document", "--install-dir", "{folder}", "--bindir", "{folder}", "solargraph", "-v", "{version}"]
needs = "Ruby and RubyGems"      # said when the program is not on the machine
bin = "."                        # where the server's program ends up in {folder}
```

The command runs in the staging folder of the version and is moved into place
only when it succeeds, so a failed install leaves a working one in place. It is
as trusted as the manifest's `command`, which is run as well: a language of the
list is checked against `languages.json`, and a folder of your own is yours.

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
