# Extensions

A language is data. A folder of files, in `languages/` or inside an extension,
gives a language its colours, its indentation, its run buttons and tasks, its
text objects, its language servers and its debugger, and no code of the editor
names a language to do it. An extension is a folder with an `extension.toml`.
It adds languages, themes, icon themes, debug adapters, tool servers (MCP) and
commands to the machine it is installed on, and may carry a WebAssembly module
that runs its commands and answers questions about the language servers and
debug adapters it brings.

`extensions/hello` is a complete sample; `make extension-sample` builds it into
`build/extensions/hello`, and installing that folder from the extensions page
(Install Extension…) adds everything it carries. `extensions/sample` is a
language that uses every file and key a language folder may hold, and
`extensions/sample-server` is an extension written in Rust that fetches and
starts a language server; `make extension-sample` builds it too, into
`build/extensions/sample-server`.

An extension belongs to the server of the machine it is installed on, and
applies only to the projects of that server. The window reads its themes, icon
themes and command names from every server it is connected to.

## The manifest

```toml
name = "Hello"
version = "0.1.0"
description = "What it is for, in a line."
api = 2                          # the plugin API it was written for
wasm = "extension.wasm"          # optional: runs its commands and answers hooks
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
provided = false                        # true: the module says which program it is, see Hooks

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
- **Languages** are language folders, described next. An installed extension
  hands each to the language index as `<extension>-<folder>`, so it highlights,
  indents, runs and starts its language server as a built-in language does.

## A language folder

A folder holds a `config.toml` and any of the files below. Every file but
`config.toml` is optional, and a folder that has none of a query has none of
what it does. `make languages` lists the folders in `languages/languages.json`
with a checksum for each file; any file in a folder is listed, so the lockfile
that pins an npm install and a language's own grammar travel with it.

| File                        | What it gives the language                                                  |
| --------------------------- | --------------------------------------------------------------------------- |
| `config.toml`               | How its files are told apart and edited, its grammar and its servers        |
| `highlights.scm`            | The colour of each node                                                     |
| `indents.scm`               | Which nodes indent the lines inside them                                    |
| `outline.scm`               | The items the outline lists                                                 |
| `injections.scm`            | The stretches written in another language                                   |
| `brackets.scm`              | The brackets that pair up                                                   |
| `runnables.scm`             | The places that can be run, which get a run button                          |
| `tasks.json`                | The tasks it ships: what the run buttons run, and what a project lists      |
| `overrides.scm`             | The scopes whose settings differ, such as strings and comments              |
| `redactions.scm`            | The values to hide when the reader asks for private values to be covered    |
| `textobjects.scm`           | The functions, classes and comments to select whole                         |
| `debugger.scm`              | The names whose value is shown beside the lines while a program is paused   |
| `semantic_token_rules.json` | What the types of tokens its language server names look like                |
| `grammar.wasm`, `icon.svg`  | A grammar of its own, when `config.toml` pins none, and the icon of its files |
| `package.json`, `package-lock.json` | The pin of an npm install of its server, `<pin>.package.json` for more than one |

### `config.toml`

```toml
name = "Rust"
version = "0.1.0"
description = "What it is for, in a line."
icon = "icon.svg"
color = "#e8956a"
grammar = "rust"                  # the name the grammar was built under
inherits = "yaml"                 # optional: what this folder does not say is that language's
suffixes = ["rs"]                 # the endings of the file names written in it
names = ["Makefile"]              # whole file names
patterns = [".env.*"]             # file names with * for any run of characters
aliases = ["rs"]                  # other names a code fence may call it by
requires = ["json"]               # languages installed with it
hidden = false                    # installed only as part of another
first_line_pattern = '^#!.*\bsample\b'   # a file no name above reads is read as this when its first line matches

line_comment = "//"
indent = 4
tabs = false
toolchain = "javascript"          # the family of toolchains its files are built and run with; its own name when absent
toolchain_name = "Node.js"        # what the reader calls that family
emmet_dialect = "markup"          # `markup` or `style`: Emmet abbreviations are written in its files
pairs = ["()", "[]", "{}", "\"\""]
word_characters = ["-", "?"]      # part of a word besides letters, digits and the underscore

increase_indent_pattern = '\b(then|do)\s*$'    # a line that matches indents the line after it
decrease_indent_pattern = '^\s*end\b'          # a line that starts matching goes back a level
decrease_indent_patterns = [                   # a line that lines up with an earlier construct
  { pattern = '^\s*(else|elif)\b', valid_after = ["if"] },   # `@start.if` in indents.scm
]

brackets = [                      # pairs with more to say than `pairs`
  { start = "'", end = "'", close = true, not_in = ["string", "comment"] },
]

[[debug_adapter]]                 # a debug adapter that runs programs written in it
id = "debugpy"
name = "Debugpy"
programs = ["python3", "python"]  # the programs it may be installed as, the first to prefer
arguments = ["-m", "debugpy.adapter"]   # {port} stands for the port of an adapter that listens on one
connect = "stdio"                 # or "tcp"
types = ["debugpy", "python"]     # what a launch scenario's type may be
attach_names = ["python"]         # the starts of the names of the programs it attaches to; "" for any
[debug_adapter.set]               # always told to the adapter, over what a scenario says
console = "internalConsole"
[debug_adapter.default]           # told when a scenario says nothing, or `auto`, for the key
mode = "debug"

[[debug_scenario]]                # what a file of the language can be debugged as with nothing written
suffixes = ["py"]
label = "Python: Current File"
adapter = "debugpy"               # as a scenario names an adapter
config = { program = "${file}", cwd = "${workspaceFolder}" }

[grammar_file]                    # the grammar, fetched and checked against its checksum
url = "https://…/tree-sitter-rust.wasm"
sha256 = "…"

[scope.string]                    # settings of a scope: overrides.scm names it, or `nodes` lists node kinds
nodes = ["string_literal"]
line_comment = ""                 # what starts a line comment inside it; empty for none
block_comment = ["{/*", "*/}"]    # what opens and closes a comment inside it
emmet = false                     # whether Emmet abbreviations are expanded inside it

[tags]                            # the nodes of a tag, which let the editor close tags it opens
element = "jsx_element"
open = "jsx_opening_element"
close = "jsx_closing_element"
names = ["identifier", "member_expression"]
```

- `inherits` lays what the folder writes over another language's `config.toml`:
  the grammar, the queries a folder does not hold, the editing rules and the
  language servers are the other language's unless this folder says its own. A
  `[server]` or `[[companion]]` written here replaces the other's whole. The
  keys that say what the language is and which files it reads (`name`,
  `version`, `description`, `icon`, `color`, `suffixes`, `names`, `patterns`,
  `aliases`, `hidden`, `requires`) are never taken from it. A folder that builds
  on another lists it in `requires` so it is installed too.
- A file is read as the language whose `names` or `patterns` match it before
  one whose `suffixes` do, so that `docker-compose.yml` is Docker Compose and
  not YAML. `first_line_pattern` is looked at last, for a file no language
  reads by its name.
- `increase_indent_pattern` and the two `decrease` keys are regular expressions
  matched against a line. `valid_after` names the `@start.<name>` captures of
  `indents.scm`; a line that matches lines up with the nearest of those earlier
  in the file that is at its indentation or less, and goes back one level when
  none is named.
- `not_in` names the scopes a pair is not closed in; a pair written under
  `brackets` is closed or not as its `not_in` says, and one only in `pairs` is
  not closed inside strings and comments. A scope is named by a capture of
  `overrides.scm` or a `[scope.<name>]` table.

### Language servers

```toml
[server]
id = "rust-analyzer"            # what it is known by; languages that name the same id share one server per project
language_id = "rust"            # what the server calls the language
command = "rust-analyzer"
args = ["--stdio"]
check = ["--version"]           # run to tell whether a program found on the machine works
project = [".venv/bin"]         # folders of the project looked in first
env = { RUST_LOG = "info" }     # added to the environment it starts with
settings = { … }                # what it is told when it asks for its configuration
init = { … }                    # the options of its `initialize` request
languages = ["docker-compose"]  # other languages it also reads, by folder name
opt_in_languages = ["docker-compose"]   # languages it does not read until the reader names it
provided = false                # true: the extension's module says the program and what it is told, see Hooks

[server.language_ids]           # what it calls a language, by language folder name or by file ending
docker-compose = "dockercompose"
jsx = "javascriptreact"

[server.install]                # how it is installed when the machine does not have it
kind = "download"               # or "npm", or "go"
version = "2026-10-12"
# download: [server.install.downloads.<os>-<arch>] with url, sha256, archive = "gz" | "zip", member
# npm: packages = ["name@version"], pinned by the folder's package.json and package-lock.json
# go: package = "example.com/tool@version"
# npm with more than one install in a folder: pin = "<name>" names <name>.package.json and its lockfile

[[companion]]                   # another server for the same files
suffixes = ["jsx"]              # the endings it reads; every file when none
scopes = ["string"]             # the scopes its completions are offered in; everywhere when none

[companion.detect]              # started only for projects that hold any of these
dependencies = ["tailwindcss"]  # in a package.json
files = ["tailwind.config.*"]   # at the root

[companion.server]              # as [server]
id = "tailwindcss-language-server"
```

A language runs its `[server]` and every `[[companion]]` that reads the file,
that its project needs and that is not opt-in for it, and every server of
another language that lists it in `languages`. What the reader chooses in
Settings → Languages → Language servers (`servers` in a language's machine
preferences) replaces that: the servers named, by id and in order, the first
being the language's server and the rest running beside it. `...` stands for
the servers that run when nothing is chosen, and an opt-in server is started
only when it is named. What the reader sets for one server, by its id, is
`servers.<id>` in the machine settings and goes over the language's:

```json
{ "binary": "/opt/docker-language-server", "arguments": ["start", "--stdio"],
  "env": { "NAME": "value" }, "initialization_options": { … }, "workspace_configuration": { … } }
```

A server whose settings change is stopped and started again with them, and the
open files it reads move onto the new one.

### Queries

Every query is written in tree-sitter's query language, against the language's
grammar. A node that a pattern captures more than once takes the kind of the
rightmost capture the editor has a colour for; a capture whose first word is none of those below has no colour and the pattern's capture on its left is used instead.

| File | Captures | Meaning |
| ---- | -------- | ------- |
| `highlights.scm` | the first word of a capture decides: `keyword`, `conditional`, `repeat`, `include`, `exception`, `storageclass`, `tag` (keywords); `function`, `method`, `constructor`, `macro`; `type`, `namespace`, `module`; `string`, `character`, `escape`; `number`, `float`, `boolean`, `constant`, `label`; `comment`; `property`, `field`, `attribute`; `operator`; `punctuation`; `variable`, `parameter`; `markup`; any dotted tail follows | The colour of a node, by the first word of the name; `((identifier) @type @sample.type)` is coloured as a type, since `sample` is no colour |
| `indents.scm` | `@indent`, `@outdent`, `@start.<name>` | A node whose inside is indented; a token that closes a block; the start of a construct a `decrease_indent_patterns` rule lines up with |
| `outline.scm` | `@item`, `@name`, `@context` | The items of the outline |
| `injections.scm` | `@injection.content`, `@injection.language`, `injection.language`, `injection.combined` | A stretch written in another language |
| `brackets.scm` | `@open`, `@close` | A bracket and the one it pairs with, in one match |
| `runnables.scm` | `@run`, other captures, `(#set! tag …)` | See Runnables |
| `overrides.scm` | `@<scope>`, `@<scope>.inclusive` | The nodes of the scope named `<scope>`; a node holds the caret strictly inside it, and the `.inclusive` capture also at its start and end, as a line comment up to the line break |
| `redactions.scm` | `@redact` | A value hidden behind a block when Settings → Editor → Appearance → Hide private values is on |
| `textobjects.scm` | `@function.around`, `@function.inside`, `@class.around`, `@class.inside`, `@comment.around`, `@comment.inside` | A whole function, class or comment and what is inside it; Select Function, Select Function Body, Select Class, Select Class Body and Select Comment select the smallest around the caret, and `.around` stands for a missing `.inside` |
| `debugger.scm` | `@debug-variable`, `@debug-scope` | A name whose value is shown at the end of its line while the program is paused, looked up among the variables of the frame, and the nodes from whose start to the paused line the names are taken; used when the file's language server does not give inline values |

### Runnables

`runnables.scm` marks the places of a file that can be run; the editor draws a
run button in the gutter on the line each starts on.

- `@run` is the whole place. A pattern without it marks nothing.
- `(#set! tag <tag>)` tags the place; a pattern may set it more than once. A
  place with no tag runs nothing.
- Every other capture whose name does not start with an underscore is a
  variable of the place: its text reaches a task as `$PM_CUSTOM_<name>`.
  Captures such as `@_attribute` that only the query's own predicates use start
  with an underscore.

```scheme
(rule
  (targets
    (word) @run @target
    (#match? @target "^[A-Za-z0-9_/][A-Za-z0-9_./-]*$")
    (#set! tag make-target)))
```

The tags the built-in languages use are `rust-main`, `rust-test`, `rust-module`,
`make-target`, `package-json-script`, `pyproject-script`,
`docker-compose-service`, `dockerfile-stage`, `dockerfile-image`,
`go-mod-module`, `go-mod-require` and `go-work-file`.

When more than one task is tied to a place, pressing its run button lists them
for the reader to pick from; when there is one it runs at once. A task with
`"lens": true` is not behind a run button: it is a link at the end of the
line of each place that carries its tag, saying its label, and pressing it runs
that task. A place whose tasks are all links gets no run button.

### `tasks.json`

An array of task templates. A template is run by the places that carry one of
its `tags`, and listed for a project when it names `from` or `when`.

```json
[
  {
    "label": "docker compose up $PM_CUSTOM_service",
    "command": "docker compose -f $PM_FILE up $PM_CUSTOM_service",
    "group": "run",
    "tags": ["docker-compose-service"]
  },
  {
    "label": "make: $PM_CUSTOM_target",
    "command": "make $PM_CUSTOM_target",
    "folder": "$PM_DIRNAME",
    "groups": { "capture": "target", "build": ["all", "build"], "test": ["test"], "run": ["run"] },
    "source": "$PM_FILENAME",
    "tags": ["make-target"],
    "from": ["GNUmakefile", "makefile", "Makefile"],
    "order": 30
  }
]
```

| Key | Meaning |
| --- | ------- |
| `label` | What the task is called, with variables |
| `command` | What is typed into the shell, with variables; the value of a variable is quoted for the shell |
| `folder` | The folder it runs in, from the root of the project, with variables |
| `marker` | A file such as `Cargo.toml`: the task runs in the nearest folder above the file it is run from that holds it |
| `env` | Variables added to the environment, with variables |
| `group` | `build`, `test`, `run` or nothing |
| `groups` | `{"capture": "<name>", "<group>": ["text", …]}`: the group by what a capture matched, in place of `group` |
| `language` | The language whose toolchain it runs with, by family: `rust`, `javascript`, `python`, `go` |
| `source` | Where the task is said to come from, with variables |
| `lens` | Shows the task as a link at the end of the line of its places, in place of a run button; its `label` is what the link says and has no variables |
| `tags` | The tags of the places that run it |
| `files` | The file names it is run from; any when empty |
| `when` | A condition on the root of the project (below); with `from`, also what the manifest is looked for under |
| `from` | Manifests at the root of the project: the first that exists is read with its language's `runnables.scm`, and each place carrying one of the tags gives a listed task |
| `order` | Where the listed tasks come, the lowest first; a hundred when nothing |

A template that names neither `from` nor `when` is only run from a place, and
is not listed. A template whose `when` fails is skipped, for the list and for a
place alike, so that one folder holds a template for each package manager.

A condition is one key: `{"exists": "path"}`, `{"contains": {"file": "…",
"text": "…"}}`, `{"all": […]}`, `{"any": […]}` or `{"not": …}`.

Variables are written `$NAME`, `${NAME}` or `${NAME:fallback}`; a name that is
not one is left for the shell. They are also put in the environment of the
shell the task runs in.

| Variable | Value |
| -------- | ----- |
| `PM_ROOT` | The root of the project |
| `PM_FILE` | The file, as a path on the machine |
| `PM_RELATIVE_FILE` | The file, from the root of the project |
| `PM_FILENAME`, `PM_STEM`, `PM_DIRNAME` | Its name, its name without the ending, its folder from the root |
| `PM_LANGUAGE` | The language it is written in |
| `PM_ROW`, `PM_COLUMN` | Where the place starts, from one |
| `PM_CUSTOM_<name>` | What the capture `@<name>` of the place matched |

A project's own file, `.pandemonium.tasks.json`, is `{"tasks": [{"name": …,
"command": …, "tags": […]}]}`. A task of it with a tag is run from a place
with that tag in place of every template of the languages for the tag, and a
task that reads a variable of a place is not listed. Which task a button runs
is the first; the others tied to the place are run by name (`task` of
`run_runnable`).

### `semantic_token_rules.json`

An array of rules for the types of tokens the language's server names:
`{"token_type": "lifetime", "token_modifiers": ["deprecated"], "style": ["type"]}`.
A rule matches a token of its `token_type` (every type when omitted) that has
all its `token_modifiers`; `style` lists highlight capture names and the first
the editor knows is used. The first rule that matches decides. The reader's own
rules, `semantic_token_rules` in the editor settings, come first, then the
language's, then what the editor knows of the standard types.

## Capabilities

Everything is denied until the reader allows it. The manifest lists in
`capabilities` what the extension asks for, the extensions page shows them
before the install and again, as switches, once it is installed, and the
server keeps what was allowed in its machine settings.

| Capability     | Lets the extension                                                              |
| -------------- | ------------------------------------------------------------------------------- |
| `process`      | run programs: its language servers, debug adapters and tool servers, and from its module; find programs and read the shell's environment from its module |
| `download`     | download files: its languages' grammars and language servers, and from its module, and reach its tool servers by address; its module can fetch public addresses only and keep what it fetches in its own storage |
| `read-project` | read the files of the project a command runs in, from its module                |

A capability is checked where it is used. A language server, a debug adapter or
a tool server that needs `process` is refused with the reason; a language that
has to download is not installed; a module that asks the host for something it
was not allowed gets the refusal as its answer, and the reason is shown on the
extensions page and as the failure of opening the file.

## The module

The module is plain WebAssembly with no imports but one. It exports `memory`,
`pm_alloc(length) -> pointer` and, as it answers them, `pm_command(pointer,
length) -> packed` and `pm_hook(pointer, length) -> packed`, and imports
`pandemonium.host(pointer, length) -> packed`. Messages are JSON in the
module's memory; a `packed` result is the pointer in the high 32 bits and the
length in the low 32.

- `pm_command` is given `{"command", "argument", "project"}` (`project` only
  when `read-project` is allowed) and answers `{"prompt", "message"}`.
- `pm_hook` is given a hook call and answers a hook reply, below. A module
  without it answers no hook.
- `host` takes one of the calls below and answers `{"ok": value}` or
  `{"error": reason}`.

Each call runs in a fresh instance with a budget of work and a ceiling of
memory; the compiled module is kept between calls. The contract is in
`pm-plugin-api` and is append-only: a newer editor loads an older extension,
and an extension written for a newer API than the editor speaks is refused.
`extensions/sample-server-guest` is a module written in Rust.

### Host calls

| `op` | Fields | Needs | Answer |
| ---- | ------ | ----- | ------ |
| `process` | `program`, `args`, `cwd` | `process` | `{status, stdout, stderr}` |
| `download` | `url` | `download` | The text at the address, public addresses only, no redirects |
| `read-file` | `path` | `read-project` | The text of a file of the project |
| `which` | `program` | `process` | The path of the program, or `null` |
| `shell-env` | | `process` | The environment of the reader's login shell, an object |
| `latest-release` | `repository` as `owner/name` | `download` | `{version, assets: [{name, url, sha256}]}` of its latest GitHub release |
| `download-file` | `url`, `path`, `sha256`, `executable` | `download` | The path of the file written inside the extension's storage; follows redirects to public addresses, checks the checksum when given |
| `storage-list` | `dir` | `download` | `[{name, path, dir}]` of the storage or a folder of it |
| `storage-remove` | `path` | `download` | `null`; takes a file or folder out of the storage |
| `status` | `text` | nothing | `null`; the line shown as the extension's status on the extensions page, empty to clear it |

The storage of an extension is `extension-data/<name>` in the server's data
folder, kept when the extension is installed again.

### Hooks

A hook call is `{"hook": "<name>", …}`; the reply is `{"command": {command,
args, env}}`, `{"json": value}`, `{"labels": [string | null, …]}`, `"none"` or
`{"error": reason}`.

| Hook | Fields | Reply | Asked |
| ---- | ------ | ----- | ----- |
| `language_server_command` | `server`, `language`, `project`, `platform` | `command` | When a server with `provided = true` starts, unless the reader set a binary for it |
| `language_server_initialization_options` | `server`, `language`, `project` | `json` | Then, for its `initialize` request |
| `language_server_workspace_configuration` | `server`, `language`, `project` | `json` | Then, for the settings it asks for |
| `label_for_completions` | `server`, `items: [{label, kind, detail}]` | `labels` | For each list of completions of a provided server |
| `label_for_symbols` | `server`, `items: [{name, kind, container}]` | `labels` | For each list of symbols a provided server finds |
| `debug_adapter_binary` | `adapter`, `install`, `platform` | `command` | For an adapter with `provided = true`: to list it with `install` false, which only says where the program is, and to start it with `install` true, when it may be fetched |

`project` is the folder of the project, and only given when the extension may
read the project. `platform` is `os-arch`: `linux-x86_64`, `linux-aarch64`,
`macos-x86_64`, `macos-aarch64`, `windows-x86_64` or `windows-aarch64`. What
the reader sets for a server goes over what a hook answers. A program a hook
names is only started when the extension may run programs, and variables that
make a program load code of the extension's choosing (`LD_*`, `DYLD_*`,
`NODE_OPTIONS`, `BASH_ENV`, `ENV`, `PYTHONSTARTUP`, `PYTHONPATH`, `PERL5OPT`)
are left out of its environment.
