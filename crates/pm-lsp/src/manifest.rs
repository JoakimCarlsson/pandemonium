//! A language's `config.toml`: how its files are told apart, how they are edited, its grammar and the language server that reads them.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// A file fetched from an address and checked against the checksum the manifest pins for it.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct Pinned {
    /// Where it is fetched from.
    pub url: String,
    /// The SHA-256 of its bytes, in lowercase hex.
    pub sha256: String,
}

/// How a downloaded language server is packed.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Archive {
    /// The program itself.
    #[default]
    None,
    /// The program, gzipped.
    Gz,
    /// A zip archive holding the program.
    Zip,
}

/// The language server built for one platform.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct Download {
    /// Where it is fetched from.
    pub url: String,
    /// The SHA-256 of what is fetched, in lowercase hex.
    pub sha256: String,
    /// How it is packed.
    #[serde(default)]
    pub archive: Archive,
    /// The program's path inside a zip archive.
    #[serde(default)]
    pub member: Option<String>,
}

/// The way a language server is installed when the machine does not have it.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum InstallKind {
    /// Fetched as a program built for the platform.
    #[default]
    Download,
    /// Installed by a command the language folder names, run with the program it names found on the machine and without a shell: `go install`, `npm ci`, `gem install` or any other.
    Command,
}

/// How a language server is installed into the data folder.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct Install {
    /// Which way.
    pub kind: InstallKind,
    /// The version installed, which names the folder it goes in.
    pub version: String,
    /// The program for each platform, as `os-arch`: `linux-x86_64`, `macos-aarch64`, `windows-x86_64`.
    #[serde(default)]
    pub downloads: BTreeMap<String, Download>,
    /// The program, with its arguments, a `command` install runs, where `{folder}` stands for the folder the server is installed in and `{version}` for `version`; the program is found on the machine and run without a shell.
    #[serde(default)]
    pub command: Vec<String>,
    /// What the program of a `command` install needs, in words for the reader, when it is not on the machine.
    #[serde(default)]
    pub needs: Option<String>,
    /// The folder inside the install folder that a `command` install puts the server's program in; the install folder itself when nothing.
    #[serde(default)]
    pub bin: Option<String>,
    /// What the environment of the command holds besides, where `{folder}` and `{version}` stand as they do in the command.
    #[serde(default)]
    pub env: BTreeMap<String, String>,
    /// Variables taken out of the command's environment, so that what the machine has set does not change what is installed.
    #[serde(default)]
    pub env_remove: Vec<String>,
    /// Files of the language's folder copied into the install folder before the command runs, such as the lockfile that pins what it installs; `name:other` copies `name` as `other`.
    #[serde(default)]
    pub files: Vec<String>,
}

/// The language server that reads a language's files.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ServerSpec {
    /// What it is known by; the languages that name the same one share one server in a project.
    pub id: String,
    /// What the server calls the language.
    pub language_id: String,
    /// What the server calls the language for the files with an ending, by the ending, where it is not the same for them all, such as `jsx` for `javascriptreact`.
    #[serde(default)]
    pub language_ids: BTreeMap<String, String>,
    /// The program.
    pub command: String,
    /// The arguments it is started with.
    #[serde(default)]
    pub args: Vec<String>,
    /// The arguments it is run with to tell whether a program found on the machine works, which it does when it exits cleanly; none when finding it is enough.
    #[serde(default)]
    pub check: Vec<String>,
    /// Folders of the project, such as `.venv/bin`, looked in for the program before the machine's, and put first on the path it runs with.
    #[serde(default)]
    pub project: Vec<String>,
    /// What this client tells the server it can do besides what it always does, merged into the client capabilities of the `initialize` request, such as an extension of the protocol the server defines.
    #[serde(default)]
    pub capabilities: serde_json::Value,
    /// What it is told when it asks for its settings.
    #[serde(default)]
    pub settings: serde_json::Value,
    /// The options it is started with.
    #[serde(default)]
    pub init: serde_json::Value,
    /// How it is installed when the machine does not have it.
    #[serde(default)]
    pub install: Option<Install>,
    /// The folder of the installed language this was read from, which holds the lockfile an install through `npm` is pinned by.
    #[serde(skip)]
    pub origin: Option<std::path::PathBuf>,
}

/// A named set of syntax nodes whose insides are edited differently from the rest of the file, such as the children of a JSX element or a string.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct ScopeSpec {
    /// The grammar's nodes the scope is made of, by kind; the caret is in the scope of the innermost of these around it.
    pub nodes: Vec<String>,
    /// What starts a comment that runs to the end of the line inside the scope in place of the language's, none written when the language's is kept and an empty one when there are no line comments inside it.
    #[serde(default)]
    pub line_comment: Option<String>,
    /// What opens and closes a comment inside the scope, such as `["{/*", "*/}"]`, used when it has no line comment.
    #[serde(default)]
    pub block_comment: Option<[String; 2]>,
    /// Whether Emmet abbreviations are expanded inside the scope; a language that sets it for any scope expands them only where it is set.
    #[serde(default)]
    pub emmet: Option<bool>,
}

/// The grammar nodes that make up a tag, which let the editor close the tags it opens; an opening tag with no name, such as `<>`, is closed with `</>`.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct TagSpec {
    /// The node of an element, an opening tag, what is inside it and a closing tag.
    pub element: String,
    /// The node of an opening tag.
    pub open: String,
    /// The node of a closing tag.
    pub close: String,
    /// The nodes that can be the name of a tag, such as an identifier or a member expression.
    pub names: Vec<String>,
}

/// What a project has to hold for a companion server to be started for it: any one of the files, or a file whose text a pattern matches.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct Detect {
    /// Pairs of a file at the root of the project and a pattern: the project holds what is asked for when the pattern matches the text of the file.
    #[serde(default)]
    pub text: Vec<[String; 2]>,
    /// File names at the root of the project, where `*` stands for any run of characters.
    #[serde(default)]
    pub files: Vec<String>,
}

/// A language server that reads a file alongside the one its language comes with, started only for the projects that need it.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Companion {
    /// The endings of the files it reads, without the dot; every file of the language when none.
    #[serde(default)]
    pub suffixes: Vec<String>,
    /// The scopes of the language its completions are offered in; everywhere when none.
    #[serde(default)]
    pub scopes: Vec<String>,
    /// What a project holds that needs it; every project when nothing.
    #[serde(default)]
    pub detect: Detect,
    /// The server.
    pub server: ServerSpec,
}

/// What running one kind of runnable place of a language's files comes to.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct RunnableSpec {
    /// The kind of place the language's `runnables.scm` marks, such as `main` or `test`.
    pub kind: String,
    /// What running it is called, with `{name}` standing for the place's name, `{file}` for the name of the file and `{path}` for its path from the folder the command runs in.
    pub label: String,
    /// The command that runs it, with `{name}`, `{file}` and `{path}` standing as they do in the label.
    pub command: String,
    /// The file that marks the folder the command runs in, such as `Cargo.toml`: the nearest folder above the file that holds it; the root of the project when nothing.
    #[serde(default)]
    pub marker: Option<String>,
}

/// A lens a language folder declares for the places its `lenses.scm` marks, which runs a command in a shell of the project when pressed.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct LensSpec {
    /// The kind of place the language's `lenses.scm` marks, such as `service`.
    pub kind: String,
    /// What the lens says, with `{name}` standing for the place's name.
    pub title: String,
    /// The command it runs, with `{name}` standing for the place's name and `{file}` for the name of the file.
    pub command: String,
    /// The names of the files it is shown above, where `*` stands for any run of characters; every file of the language when none.
    #[serde(default)]
    pub files: Vec<String>,
    /// The file that marks the folder the command runs in, such as `Cargo.toml`: the nearest folder above the file that holds it; the folder of the file when nothing.
    #[serde(default)]
    pub marker: Option<String>,
    /// The family of toolchain the command runs with, such as `javascript`; none when it needs no choice of toolchain.
    #[serde(default)]
    pub toolchain: Option<String>,
    /// Whether the lens is shown above the first place of its kind alone.
    #[serde(default)]
    pub once: bool,
}

impl LensSpec {
    /// Whether the lens is shown above the file at `path`, by the name of the file.
    pub fn is_for(&self, path: &str) -> bool {
        let name = path.rsplit(['/', '\\']).next().unwrap_or(path);
        self.files.is_empty() || self.files.iter().any(|pattern| glob(pattern, name))
    }
}

/// The settings of the machine that make a dependency private, so that its name is not sent to a registry.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct PrivateSpec {
    /// A program, with its arguments, that prints the value of each variable named in `patterns` and `hosts` on a line of its own when they are put after it, such as `["go", "env"]`; the variables of the process are read when it cannot be run.
    #[serde(default)]
    pub read: Vec<String>,
    /// Variables that hold comma-separated names, or paths whose elements may be globs, of dependencies that are private.
    #[serde(default)]
    pub patterns: Vec<String>,
    /// Variables that list the hosts dependencies are fetched from: every dependency is private unless the host of the registry is the only one listed.
    #[serde(default)]
    pub hosts: Vec<String>,
    /// Files of `key=value` lines that can name another registry: one that starts with `~/` is in the home folder, one that starts with `$NAME/` in the folder the variable names, and any other is looked for in the folder of the manifest and every folder above it.
    #[serde(default)]
    pub ini: Vec<String>,
    /// The keys of that file that name another registry, where `{scope}` stands for the scope a name starts with, such as `@types`.
    #[serde(default)]
    pub ini_keys: Vec<String>,
}

/// How the dependencies of the manifests a language reads are checked for newer versions: its `dependencies.scm` says where a manifest names them, and this where their registry says what the latest is.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct DependencySpec {
    /// Whether the language's extension module says what the latest version of a dependency is and whether a requirement is behind it, which `url`, `latest` and `requirement` then need not say.
    #[serde(default)]
    pub hook: bool,
    /// The address that answers with what the registry knows of a dependency, where `{name}` stands for its name.
    #[serde(default)]
    pub url: String,
    /// The places in the answer, a JSON document, that hold the latest version, as keys joined by dots; the first that has one is used.
    #[serde(default)]
    pub latest: Vec<String>,
    /// How a name is written in the address: each pair has what is found in the name and what it is written as instead, such as `["/", "%2f"]`.
    #[serde(default)]
    pub escape: Vec<[String; 2]>,
    /// What is written before a capital letter, which is then written in lower case; capitals are left as they are when nothing.
    #[serde(default)]
    pub capitals: Option<String>,
    /// The pattern a version or requirement in a manifest is read with: its `version` group holds dot-separated numbers, its `prefix` group what stands before them and its `pre` group a pre-release tag; a requirement it does not match, such as a range or an address, is left alone.
    #[serde(default)]
    pub requirement: String,
    /// The names of the files it is for, where `*` stands for any run of characters; every file of the language when none.
    #[serde(default)]
    pub files: Vec<String>,
    /// When a dependency is not asked about.
    #[serde(default)]
    pub private: PrivateSpec,
}

/// A debug adapter a language brings, which is run to debug its programs.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct DebugAdapterSpec {
    /// The name it is known by in the protocol's handshake.
    pub id: String,
    /// What the reader calls it.
    pub name: String,
    /// The programs it may be installed as, the one to prefer first.
    pub programs: Vec<String>,
    /// What each of them is started with; `{port}` stands for the port it listens on.
    #[serde(default)]
    pub arguments: Vec<String>,
    /// `stdio`, or `tcp` for an adapter that listens on a port the editor picks.
    #[serde(default)]
    pub connect: String,
    /// The names a scenario may call it by.
    pub types: Vec<String>,
    /// Settings every scenario run with it gets, whatever it says.
    #[serde(default)]
    pub set: serde_json::Value,
    /// Settings a scenario gets only when it names none or says `auto`.
    #[serde(default)]
    pub default: serde_json::Value,
    /// The beginnings of the names of programs it attaches to, in lower case; `*` for any program no other adapter takes.
    #[serde(default)]
    pub attach: Vec<String>,
    /// How a build a scenario names in place of a program is run, for an adapter that knows only a program.
    #[serde(default)]
    pub build: Option<BuildSpec>,
}

/// How a build a scenario names in place of a program is run and read, for an adapter that knows only a program.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct BuildSpec {
    /// The key of a scenario's configuration that holds the build, an object.
    pub key: String,
    /// The program that builds.
    pub program: String,
    /// The key of the build that holds its arguments.
    #[serde(default = "args_key")]
    pub args: String,
    /// What is put after the arguments.
    #[serde(default)]
    pub append: Vec<String>,
    /// The key of the build that holds the folder it runs in.
    #[serde(default = "cwd_key")]
    pub cwd: String,
    /// The key of the build that holds the variables it runs with.
    #[serde(default = "env_key")]
    pub env: String,
    /// The key of the build that holds what narrows down the executables it made.
    #[serde(default = "filter_key")]
    pub filter: String,
    /// What a line of its output, a JSON object, must have to say that an executable was made: each key with the value it holds.
    #[serde(default, rename = "match")]
    pub matching: serde_json::Value,
    /// The key of such a line that holds the executable.
    pub take: String,
    /// For each key of the filter, where in such a line the value it is compared to stands, as keys joined by dots.
    #[serde(default)]
    pub compare: BTreeMap<String, String>,
}

/// The key of a build that holds its arguments unless the language says another.
fn args_key() -> String {
    "args".to_owned()
}

/// The key of a build that holds its folder unless the language says another.
fn cwd_key() -> String {
    "cwd".to_owned()
}

/// The key of a build that holds its variables unless the language says another.
fn env_key() -> String {
    "env".to_owned()
}

/// The key of a build that holds its filter unless the language says another.
fn filter_key() -> String {
    "filter".to_owned()
}

/// What a file of the language can be debugged as without a word written down.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct DebugScenarioSpec {
    /// What the scenario is called.
    pub label: String,
    /// The name of the adapter it runs with, as a scenario calls it.
    pub adapter: String,
    /// The endings of the files it is offered for, without the dot.
    pub suffixes: Vec<String>,
    /// What it is configured with, where `${file}`, `${fileDirname}` and `${workspaceFolder}` stand for what they say.
    #[serde(default)]
    pub config: serde_json::Value,
}

/// A language: how its files are told apart, how they are edited, its grammar and its language server.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Manifest {
    /// What the reader calls it.
    pub name: String,
    /// What kind of extension it is; `language` is the only kind so far.
    #[serde(default = "language_kind")]
    pub kind: String,
    /// Its version.
    #[serde(default)]
    pub version: String,
    /// What it is for, in a line.
    #[serde(default)]
    pub description: String,
    /// The file in its folder holding the icon its files are shown with, an SVG.
    #[serde(default)]
    pub icon: Option<String>,
    /// The colour its icon is tinted, as `#rrggbb`.
    #[serde(default)]
    pub color: Option<String>,
    /// The name the grammar was built under; none for a language with no files to read, such as one that only draws figures.
    #[serde(default)]
    pub grammar: String,
    /// What it draws, by the names a fence or a piece of syntax gives it, such as `mermaid`: its folder's `module.wasm` answers `render` calls for them with an SVG picture.
    #[serde(default)]
    pub renders: Vec<String>,
    /// The endings of the file names that are written in it, without the dot.
    #[serde(default)]
    pub suffixes: Vec<String>,
    /// Whole file names that are written in it.
    #[serde(default)]
    pub names: Vec<String>,
    /// File names that are written in it, where `*` stands for any run of characters, such as `.env.*`.
    #[serde(default)]
    pub patterns: Vec<String>,
    /// A pattern the first line of a file that is not told by its name is matched against, such as `^#!.*\\bperl\\b` for a script that names its interpreter.
    #[serde(default)]
    pub first_line_pattern: Option<String>,
    /// Other names a code fence may call it by.
    #[serde(default)]
    pub aliases: Vec<String>,
    /// What starts a comment that runs to the end of the line.
    #[serde(default)]
    pub line_comment: Option<String>,
    /// How many columns one level of indentation is.
    #[serde(default = "default_indent")]
    pub indent: usize,
    /// Whether a level of indentation is a tab rather than spaces.
    #[serde(default)]
    pub tabs: bool,
    /// The pairs the editor closes as the opening character is typed, each the opening and the closing character, such as `"()"`.
    #[serde(default = "default_pairs")]
    pub pairs: Vec<String>,
    /// The languages installed with it.
    #[serde(default)]
    pub requires: Vec<String>,
    /// Whether it is installed only as part of another, and so left out of the list the reader picks from.
    #[serde(default)]
    pub hidden: bool,
    /// The grammar, which a folder of your own may hold as `grammar.wasm` instead.
    #[serde(default)]
    pub grammar_file: Pinned,
    /// The language server, when it has one.
    #[serde(default)]
    pub server: Option<ServerSpec>,
    /// The language servers that read its files besides that one, each for the projects that need it.
    #[serde(default, rename = "companion")]
    pub companions: Vec<Companion>,
    /// The parts of a file edited differently from the rest, by name.
    #[serde(default, rename = "scope")]
    pub scopes: BTreeMap<String, ScopeSpec>,
    /// The kind of Emmet abbreviation its files take, `markup` or `style`, when they take any.
    #[serde(default)]
    pub emmet: Option<String>,
    /// How its tags are closed as they are opened, for a language that has them.
    #[serde(default)]
    pub tags: Option<TagSpec>,
    /// What running each kind of place its `runnables.scm` marks comes to.
    #[serde(default, rename = "runnable")]
    pub runnables: Vec<RunnableSpec>,
    /// The lenses its `lenses.scm` has shown above the places it marks, each with what pressing it runs.
    #[serde(default, rename = "lens")]
    pub lenses: Vec<LensSpec>,
    /// How the dependencies its manifests name are checked for newer versions, for a language that reads manifests.
    #[serde(default)]
    pub dependencies: Option<DependencySpec>,
    /// How files with an ending are shown rendered rather than as source, by the ending: `markdown` lays the text out as a page and `vector` draws it as a picture.
    #[serde(default)]
    pub previews: BTreeMap<String, String>,
    /// The names of folders its tools generate, such as installed packages or build output, which walks of a project leave out and a new worktree is given a copy of by default.
    #[serde(default)]
    pub generated: Vec<String>,
    /// Folders its tools are installed in without being on the path, which may start with `~/` and may have `*` for any folder name: where a program of its toolchain is looked for after the path.
    #[serde(default)]
    pub tool_directories: Vec<String>,
    /// The debug adapters it brings.
    #[serde(default, rename = "debug_adapter")]
    pub debug_adapters: Vec<DebugAdapterSpec>,
    /// What its files can be debugged as without a word written down.
    #[serde(default, rename = "debug_scenario")]
    pub debug_scenarios: Vec<DebugScenarioSpec>,
    /// The family of toolchains its files are built and run with, shared by the languages that use the same ones, such as `javascript` for TypeScript; its own folder name when nothing.
    #[serde(default)]
    pub family: Option<String>,
    /// The tasks a project has when its files say so, such as the scripts of a manifest.
    #[serde(default, rename = "task")]
    pub tasks: Vec<pm_task::TaskSpec>,
    /// How the toolchains of its family are found on a machine.
    #[serde(default, rename = "toolchain")]
    pub toolchains: Vec<pm_task::ToolchainSpec>,
}

/// An extension is a language unless its manifest says otherwise.
fn language_kind() -> String {
    "language".to_owned()
}

/// One level of indentation is four columns unless a manifest says otherwise.
fn default_indent() -> usize {
    4
}

/// The pairs closed as they are opened when a manifest names none: brackets and double quotes.
fn default_pairs() -> Vec<String> {
    ["()", "[]", "{}", "\"\""].map(str::to_owned).to_vec()
}

impl Manifest {
    /// The manifest written in `text`, or why it cannot be read.
    pub fn parse(text: &str) -> Result<Self, String> {
        toml::from_str(text).map_err(|error| format!("config.toml: {error}"))
    }

    /// The manifest read from the language folder `folder`, its language server knowing the folder it came from.
    pub fn read(folder: &std::path::Path) -> Result<Self, String> {
        let text = std::fs::read_to_string(folder.join("config.toml"))
            .map_err(|error| format!("{} has no config.toml: {error}", folder.display()))?;
        let mut manifest = Self::parse(&text)?;
        for server in manifest
            .server
            .iter_mut()
            .chain(manifest.companions.iter_mut().map(|held| &mut held.server))
        {
            server.origin = Some(folder.to_path_buf());
        }
        Ok(manifest)
    }

    /// Whether a file at `path` is written in the language by its whole name or a pattern its name matches, which tells it apart from the languages that read files by their ending.
    pub fn matches_name(&self, path: &str) -> bool {
        let name = path.rsplit(['/', '\\']).next().unwrap_or(path);
        self.names.iter().any(|whole| whole == name)
            || self.patterns.iter().any(|pattern| glob(pattern, name))
    }

    /// Whether a file whose first line is `line` is written in the language, by the pattern it gives for the first line.
    pub fn matches_first_line(&self, line: &str) -> bool {
        self.first_line_pattern
            .as_deref()
            .and_then(|pattern| regex::Regex::new(pattern).ok())
            .is_some_and(|pattern| pattern.is_match(line))
    }

    /// Whether a file at `path` is written in the language, by its name or the ending of its name.
    pub fn matches(&self, path: &str) -> bool {
        let name = path.rsplit(['/', '\\']).next().unwrap_or(path);
        self.matches_name(path)
            || name.rsplit_once('.').is_some_and(|(stem, suffix)| {
                !stem.is_empty()
                    && self
                        .suffixes
                        .iter()
                        .any(|held| held.eq_ignore_ascii_case(suffix))
            })
    }
}

/// Whether `name` is what `pattern` describes, where `*` stands for any run of characters and everything else stands for itself.
pub fn glob(pattern: &str, name: &str) -> bool {
    let mut parts = pattern.split('*');
    let first = parts.next().unwrap_or_default();
    let Some(mut rest) = name.strip_prefix(first) else {
        return false;
    };
    let mut parts: Vec<&str> = parts.collect();
    let Some(last) = parts.pop() else {
        return rest.is_empty();
    };
    for part in parts {
        let Some(at) = rest.find(part) else {
            return false;
        };
        rest = &rest[at + part.len()..];
    }
    rest.ends_with(last)
}

impl ServerSpec {
    /// What the server calls the language of the file at `path`: by its ending when the spec names one for it, or the language's name.
    pub fn language_id_of(&self, path: &str) -> &str {
        let name = path.rsplit(['/', '\\']).next().unwrap_or(path);
        name.rsplit_once('.')
            .and_then(|(_, suffix)| {
                self.language_ids
                    .iter()
                    .find(|(held, _)| held.eq_ignore_ascii_case(suffix))
            })
            .map_or(self.language_id.as_str(), |(_, id)| id.as_str())
    }
}
