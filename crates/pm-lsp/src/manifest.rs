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
    /// Built with `go install`, which needs Go on the machine.
    Go,
    /// Installed with `npm`, which needs Node.js on the machine.
    Npm,
    /// Installed by running a program the language names, with the arguments, environment and files it names.
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
    /// What `go install` builds, with its version.
    #[serde(default)]
    pub package: Option<String>,
    /// What `npm` installs, each with its version; the language folder's `package.json` and `package-lock.json` pin them and the checksum of every package.
    #[serde(default)]
    pub packages: Vec<String>,
    /// What the pin files of the language folder are named before `package.json` and `package-lock.json`, when the folder pins more than one install through `npm`; none when it pins one.
    #[serde(default)]
    pub pin: Option<String>,
    /// The program a `command` install runs, found on the machine: `cpanm`, `cargo`, `pip`.
    #[serde(default)]
    pub tool: Option<String>,
    /// What a reader calls that program, for saying it is missing; the program's name when nothing.
    #[serde(default)]
    pub needs: Option<String>,
    /// The arguments the program is run with, one run for each entry, in the install folder. `{folder}` and `{version}` stand for the folder and the version installed.
    #[serde(default)]
    pub steps: Vec<Vec<String>>,
    /// What the environment holds besides, where `{folder}` and `{version}` stand as in the steps.
    #[serde(default)]
    pub env: BTreeMap<String, String>,
    /// What the environment of the machine holds that the install must not see.
    #[serde(default)]
    pub env_remove: Vec<String>,
    /// The files of the language folder copied into the install folder before the steps run, named after `pin` when it is set.
    #[serde(default)]
    pub copy: Vec<String>,
    /// The folder inside the install folder the program ends up in; the install folder itself when nothing.
    #[serde(default)]
    pub bin: Option<String>,
}

/// The language server that reads a language's files.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ServerSpec {
    /// What it is known by; the languages that name the same one share one server in a project.
    pub id: String,
    /// What the server calls the language.
    pub language_id: String,
    /// What the server calls the language, by the language's folder name or, for the files with an ending, by the ending, where it is not the same for them all, such as `jsx` for `javascriptreact`; the language's folder name is looked for first.
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
    /// Whether the extension that brought the language says which program starts it, and what it is told, in place of `command` and the options here; the options and the program a reader sets still come first.
    #[serde(default)]
    pub provided: bool,
    /// The environment it is started with besides the machine's, by variable.
    #[serde(default)]
    pub env: BTreeMap<String, String>,
    /// The other languages it reads besides the one whose folder declares it, by folder name; a language does not have to name a server that serves it.
    #[serde(default)]
    pub languages: Vec<String>,
    /// The languages, by folder name, whose files it does not read until the reader names it among the servers of the language.
    #[serde(default)]
    pub opt_in_languages: Vec<String>,
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
    /// The grammar's nodes the scope is made of, by kind; the caret is in the scope of the innermost of these around it. A scope may instead, or as well, be made of what the language's `overrides.scm` captures under the scope's name.
    #[serde(default)]
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

/// A pair of characters the editor writes together as the opening one is typed, and where it does not.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct BracketSpec {
    /// The character that opens the pair.
    pub start: String,
    /// The character that closes it.
    pub end: String,
    /// Whether the closing character is written as the opening one is typed.
    #[serde(default = "yes")]
    pub close: bool,
    /// The scopes, by name, inside which the closing character is not written: the language's `[scope.<name>]` tables and the names its `overrides.scm` captures.
    #[serde(default)]
    pub not_in: Vec<String>,
}

/// A line pattern that takes the line back a level of indentation, and what it lines up with.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct DecreaseRule {
    /// The regular expression a line starts matching when it is typed.
    pub pattern: String,
    /// The names, as `@start.<name>` captures of `indents.scm`, of the constructs the line lines up with; the line goes back one level when there are none.
    #[serde(default)]
    pub valid_after: Vec<String>,
}

/// `true`, for the keys that are on unless a manifest turns them off.
fn yes() -> bool {
    true
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

/// What a project has to hold for a companion server to be started for it: any one of the dependencies in its `package.json`, or any one of the files.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct Detect {
    /// Dependencies a `package.json` at the root of the project names.
    #[serde(default)]
    pub dependencies: Vec<String>,
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

/// A debug adapter a language declares for the programs written in it.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct AdapterSpec {
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
    /// The starts of the names of the programs it attaches to, where an empty one stands for any program.
    #[serde(default)]
    pub attach_names: Vec<String>,
    /// What it is always told besides what a scenario says.
    #[serde(default)]
    pub set: serde_json::Map<String, serde_json::Value>,
    /// What it is told when a scenario says nothing, or `auto`, for the key.
    #[serde(default)]
    pub default: serde_json::Map<String, serde_json::Value>,
}

/// What a file of a language can be debugged as with nothing written down.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct ScenarioSpec {
    /// The endings of the files it is offered for, without the dot.
    pub suffixes: Vec<String>,
    /// What the reader calls it.
    pub label: String,
    /// The adapter that runs it, as a scenario names an adapter.
    pub adapter: String,
    /// What the adapter is told; `${file}`, `${fileDirname}` and `${workspaceFolder}` are filled in.
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
    /// The name the grammar was built under.
    pub grammar: String,
    /// The endings of the file names that are written in it, without the dot.
    #[serde(default)]
    pub suffixes: Vec<String>,
    /// Whole file names that are written in it.
    #[serde(default)]
    pub names: Vec<String>,
    /// File names that are written in it, where `*` stands for any run of characters, such as `.env.*`.
    #[serde(default)]
    pub patterns: Vec<String>,
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
    /// Pairs with more to say than `pairs` can: whether they close, and the scopes they are not written in. A pair named here replaces the same pair of `pairs`.
    #[serde(default)]
    pub brackets: Vec<BracketSpec>,
    /// The characters besides letters, digits and the underscore that are part of a word, for a double click and for moving by words.
    #[serde(default)]
    pub word_characters: Vec<String>,
    /// A regular expression matched against the first line of a file whose name no suffix, name or pattern of any language reads, so that a shebang tells the language.
    #[serde(default)]
    pub first_line_pattern: Option<String>,
    /// A regular expression: a line that matches indents the line after it by one level.
    #[serde(default)]
    pub increase_indent_pattern: Option<String>,
    /// A regular expression: a line that starts matching goes back one level, whatever the syntax around it.
    #[serde(default)]
    pub decrease_indent_pattern: Option<String>,
    /// Line patterns that line up with an earlier construct, the first that matches deciding.
    #[serde(default)]
    pub decrease_indent_patterns: Vec<DecreaseRule>,
    /// The languages installed with it.
    #[serde(default)]
    pub requires: Vec<String>,
    /// Whether it is installed only as part of another, and so left out of the list the reader picks from.
    #[serde(default)]
    pub hidden: bool,
    /// The language it builds on: the grammar, queries, editing rules and language servers its own folder does not give are that language's.
    #[serde(default)]
    pub inherits: Option<String>,
    /// The grammar, which a folder of your own may hold as `grammar.wasm` instead.
    #[serde(default)]
    pub grammar_file: Pinned,
    /// The language server, when it has one.
    #[serde(default)]
    pub server: Option<ServerSpec>,
    /// The language servers that read its files besides that one, each for the projects that need it.
    #[serde(default, rename = "companion")]
    pub companions: Vec<Companion>,
    /// The family of toolchains its files are built and run with, such as `javascript` for TypeScript; its own folder's name when nothing.
    #[serde(default)]
    pub toolchain: Option<String>,
    /// What a reader calls that family of toolchains, such as `Node.js`; the family capitalised when nothing.
    #[serde(default)]
    pub toolchain_name: Option<String>,
    /// Where the toolchains of that family are kept on a machine, for the language that owns the family.
    #[serde(default, rename = "toolchain_source")]
    pub toolchain_sources: Vec<pm_task::ToolchainSource>,
    /// How Emmet abbreviations are written in its files: `markup` or `style`; none when it takes none.
    #[serde(default)]
    pub emmet_dialect: Option<String>,
    /// The debug adapters that run programs written in it.
    #[serde(default, rename = "debug_adapter")]
    pub debug_adapters: Vec<AdapterSpec>,
    /// What its files can be debugged as with nothing written down.
    #[serde(default, rename = "debug_scenario")]
    pub debug_scenarios: Vec<ScenarioSpec>,
    /// The parts of a file edited differently from the rest, by name.
    #[serde(default, rename = "scope")]
    pub scopes: BTreeMap<String, ScopeSpec>,
    /// How its tags are closed as they are opened, for a language that has them.
    #[serde(default)]
    pub tags: Option<TagSpec>,
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

    /// The manifest read from the language folder `folder`, with what it builds on laid under it, its language servers knowing the folder they came from.
    pub fn read(folder: &std::path::Path) -> Result<Self, String> {
        let (table, servers_from) = crate::inherit::read_table(folder, 0)?;
        let mut manifest: Self = toml::Value::Table(table)
            .try_into()
            .map_err(|error| format!("config.toml: {error}"))?;
        let origin = servers_from.unwrap_or_else(|| folder.to_path_buf());
        for server in manifest
            .server
            .iter_mut()
            .chain(manifest.companions.iter_mut().map(|held| &mut held.server))
        {
            server.origin = Some(origin.clone());
        }
        Ok(manifest)
    }

    /// Whether a file at `path` is written in the language by its whole name or a pattern, which is more than its ending says.
    pub fn matches_by_name(&self, path: &str) -> bool {
        let name = path.rsplit(['/', '\\']).next().unwrap_or(path);
        self.names.iter().any(|whole| whole == name)
            || self.patterns.iter().any(|pattern| glob(pattern, name))
    }

    /// Whether a file at `path` is written in the language, by its name or the ending of its name.
    pub fn matches(&self, path: &str) -> bool {
        let name = path.rsplit(['/', '\\']).next().unwrap_or(path);
        self.names.iter().any(|whole| whole == name)
            || self.patterns.iter().any(|pattern| glob(pattern, name))
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
    /// What the server calls the language of the file at `path`, written in the installed language `language`: by the language's folder name, then by the ending of the file's name when the spec names one for it, or else `language_id`.
    pub fn language_id_for(&self, language: &str, path: &str) -> &str {
        if let Some(id) = self.language_ids.get(language) {
            return id;
        }
        self.language_id_of(path)
    }

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

    /// Whether it does not read the files of `language` until the reader names it.
    pub fn is_opt_in_for(&self, language: &str) -> bool {
        self.opt_in_languages.iter().any(|held| held == language)
    }
}
