//! What a worktree says to debug, read from where editors already write it.
//!
//! A reader who debugs in VS Code has a `.vscode/launch.json`, and one who
//! debugs in Zed has a `.zed/debug.json`; both are read as they are, so a
//! worktree that debugs somewhere else debugs here without being told twice.
//! Beside what the files say, a file that can be run as it stands — a Python
//! script, a Go package — offers to be debugged with nothing written at all.

use std::path::{Path, PathBuf};

use serde_json::{Map, Value, json};

use crate::adapter::Adapter;

/// Where VS Code writes a worktree's debug configurations.
const VS_CODE: &str = ".vscode/launch.json";

/// Where Zed writes them.
const ZED: &str = ".zed/debug.json";

/// The keys of a VS Code configuration that are about the configuration
/// rather than a word to the adapter.
const VS_CODE_OWN: [&str; 4] = ["name", "preLaunchTask", "postDebugTask", "presentation"];

/// The keys of a Zed scenario that are about the scenario rather than a word
/// to the adapter.
const ZED_OWN: [&str; 4] = ["label", "adapter", "build", "tcp_connection"];

/// Whether a scenario starts the program or joins one already running.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Request {
    /// The adapter starts the program.
    Launch,
    /// The adapter joins a program that is already running.
    Attach,
}

impl Request {
    /// The request as the protocol names it.
    pub fn command(self) -> &'static str {
        match self {
            Self::Launch => "launch",
            Self::Attach => "attach",
        }
    }

    /// The request a scenario names, launching where it names none.
    fn named(name: Option<&str>) -> Self {
        match name {
            Some("attach") => Self::Attach,
            _ => Self::Launch,
        }
    }
}

/// One thing a worktree can be debugged as.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Scenario {
    /// What the scenario is called.
    pub label: String,
    /// What the scenario called its adapter.
    pub kind: String,
    /// The adapter that stands for that, where the editor has one.
    pub adapter: Option<Adapter>,
    /// Whether it starts the program or joins it.
    pub request: Request,
    /// A task to finish successfully before starting the adapter.
    pub before: Option<String>,
    /// What the adapter is told about the program, variables filled in.
    pub config: Map<String, Value>,
    /// Where the scenario was read from, relative to the worktree, or
    /// nothing for one the editor offered on its own.
    pub source: Option<PathBuf>,
}

impl Scenario {
    /// What the adapter is told to launch or attach with.
    pub(crate) fn arguments(&self) -> Value {
        let mut config = self.config.clone();
        if let Some(adapter) = self.adapter {
            adapter.prepare(&mut config);
        }
        Value::Object(config)
    }
}

/// Everything `root` can be debugged as, with `file` the file in front.
///
/// The files are read every time they are asked about, because they are
/// short and are edited by hand between one debugging run and the next.
pub fn scenarios(root: &pm_host::Location, file: Option<&Path>) -> Vec<Scenario> {
    let words = Words::new(root, file);
    let mut found = read(root, VS_CODE)
        .map(|file| vs_code(&file, &words))
        .unwrap_or_default();
    found.extend(
        read(root, ZED)
            .map(|file| zed(&file, &words))
            .unwrap_or_default(),
    );
    found.extend(offered(file, &words));
    found
}

/// The file at `relative` under `root`, read as JSON with comments.
fn read(root: &pm_host::Location, relative: &str) -> Option<Value> {
    let written = root.host.fs().read_to_string(root.join(relative)).ok()?;
    serde_json::from_str(&plain_json(&written)).ok()
}

/// The scenarios a VS Code `launch.json` holds.
fn vs_code(file: &Value, words: &Words) -> Vec<Scenario> {
    let source = PathBuf::from(VS_CODE);
    file.get("configurations")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_object)
        .map(|configuration| {
            let kind = text(configuration, "type");
            Scenario {
                label: words.fill_text(&text(configuration, "name")),
                adapter: Adapter::find(&kind),
                kind,
                request: Request::named(configuration.get("request").and_then(Value::as_str)),
                before: configuration
                    .get("preLaunchTask")
                    .and_then(Value::as_str)
                    .map(str::to_owned),
                config: words.fill_map(without(configuration, &VS_CODE_OWN)),
                source: Some(source.clone()),
            }
        })
        .collect()
}

/// The scenarios a Zed `debug.json` holds.
fn zed(file: &Value, words: &Words) -> Vec<Scenario> {
    let source = PathBuf::from(ZED);
    file.as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_object)
        .map(|scenario| {
            let kind = text(scenario, "adapter");
            Scenario {
                label: words.fill_text(&text(scenario, "label")),
                adapter: Adapter::find(&kind),
                kind,
                request: Request::named(scenario.get("request").and_then(Value::as_str)),
                before: scenario
                    .get("build")
                    .and_then(Value::as_str)
                    .map(str::to_owned),
                config: words.fill_map(without(scenario, &ZED_OWN)),
                source: Some(source.clone()),
            }
        })
        .collect()
}

/// What the file in front can be debugged as without a word written down.
fn offered(file: Option<&Path>, words: &Words) -> Vec<Scenario> {
    let Some(file) = file else {
        return Vec::new();
    };
    let named = |label: &str, kind: &str, config: Value| Scenario {
        label: label.to_owned(),
        kind: kind.to_owned(),
        adapter: Adapter::find(kind),
        request: Request::Launch,
        before: None,
        config: words.fill_map(config.as_object().cloned().unwrap_or_default()),
        source: None,
    };
    match file.extension().and_then(|extension| extension.to_str()) {
        Some("py") => vec![named(
            "Python: Current File",
            "debugpy",
            json!({ "program": "${file}", "cwd": "${workspaceFolder}" }),
        )],
        Some("go") => vec![named(
            "Go: Current Package",
            "go",
            json!({ "mode": "debug", "program": "${fileDirname}", "cwd": "${workspaceFolder}" }),
        )],
        _ => Vec::new(),
    }
}

/// The string `key` of `object` holds, or nothing.
fn text(object: &Map<String, Value>, key: &str) -> String {
    object
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned()
}

/// `object` without the keys in `own`.
fn without(object: &Map<String, Value>, own: &[&str]) -> Map<String, Value> {
    object
        .iter()
        .filter(|(key, _)| !own.contains(&key.as_str()))
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect()
}

/// The words a scenario may write in place of a path, and what each stands
/// for here.
struct Words {
    /// The machine supplying environment substitutions.
    host: pm_host::Host,
    /// Each word, with the value it is replaced by.
    known: Vec<(String, String)>,
}

impl Words {
    /// The words for a worktree at `root` with `file` in front.
    ///
    /// VS Code's `${...}` and Zed's `$ZED_...` say the same few things, and
    /// both are understood whichever file they were written in.
    fn new(root: &pm_host::Location, file: Option<&Path>) -> Self {
        let shown = |path: &Path| path.to_string_lossy().into_owned();
        let root_text = shown(root);
        let file_text = file.map(shown).unwrap_or_default();
        let relative = file
            .and_then(|file| file.strip_prefix(root).ok())
            .map(shown)
            .unwrap_or_default();
        let directory = file.and_then(Path::parent).map(shown).unwrap_or_default();
        let base = file
            .and_then(Path::file_name)
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        let stem = file
            .and_then(Path::file_stem)
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        let folder = root
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        let home = root
            .host
            .home()
            .map(|path| path.to_string_lossy().into_owned())
            .unwrap_or_default();
        let pairs = [
            ("${workspaceFolder}", root_text.clone()),
            ("${workspaceRoot}", root_text.clone()),
            ("${workspaceFolderBasename}", folder),
            ("${file}", file_text.clone()),
            ("${relativeFile}", relative.clone()),
            ("${fileDirname}", directory.clone()),
            ("${fileBasename}", base.clone()),
            ("${fileBasenameNoExtension}", stem.clone()),
            ("${userHome}", home),
            (
                "${pathSeparator}",
                if root.host.os() == "windows" {
                    "\\"
                } else {
                    "/"
                }
                .to_owned(),
            ),
            ("$ZED_WORKTREE_ROOT", root_text),
            ("$ZED_RELATIVE_FILE", relative),
            ("$ZED_FILENAME", base),
            ("$ZED_DIRNAME", directory),
            ("$ZED_STEM", stem),
            ("$ZED_FILE", file_text),
        ];
        Self {
            host: root.host.clone(),
            known: pairs
                .into_iter()
                .map(|(word, value)| (word.to_owned(), value))
                .collect(),
        }
    }

    /// `written` with every word it knows filled in, and every
    /// `${env:NAME}` with what the environment says.
    fn fill_text(&self, written: &str) -> String {
        let filled = self
            .known
            .iter()
            .fold(written.to_owned(), |text, (word, value)| {
                text.replace(word, value)
            });
        environment(&self.host, &filled)
    }

    /// Every string in `value`, however deep, with its words filled in.
    fn fill(&self, value: Value) -> Value {
        match value {
            Value::String(text) => Value::String(self.fill_text(&text)),
            Value::Array(items) => {
                Value::Array(items.into_iter().map(|item| self.fill(item)).collect())
            }
            Value::Object(map) => Value::Object(self.fill_map(map)),
            other => other,
        }
    }

    /// Every string in `map`, however deep, with its words filled in.
    fn fill_map(&self, map: Map<String, Value>) -> Map<String, Value> {
        map.into_iter()
            .map(|(key, value)| (key, self.fill(value)))
            .collect()
    }
}

/// `text` with every `${env:NAME}` replaced by what the environment holds.
fn environment(host: &pm_host::Host, text: &str) -> String {
    const OPEN: &str = "${env:";
    let mut filled = String::new();
    let mut rest = text;
    while let Some(start) = rest.find(OPEN) {
        let Some(length) = rest[start..].find('}') else {
            break;
        };
        let name = &rest[start + OPEN.len()..start + length];
        filled.push_str(&rest[..start]);
        filled.push_str(&host.environment(name).unwrap_or_default());
        rest = &rest[start + length + 1..];
    }
    filled.push_str(rest);
    filled
}

/// `written` with its comments and trailing commas taken out, which is what
/// both editors allow in these files and JSON does not.
fn plain_json(written: &str) -> String {
    let mut plain = String::with_capacity(written.len());
    let mut chars = written.chars().peekable();
    let mut quoted = false;
    while let Some(ch) = chars.next() {
        match (ch, quoted, chars.peek().copied()) {
            ('\\', true, Some(next)) => {
                plain.push(ch);
                plain.push(next);
                chars.next();
            }
            ('"', _, _) => {
                quoted = !quoted;
                plain.push(ch);
            }
            ('/', false, Some('/')) => while chars.next_if(|next| *next != '\n').is_some() {},
            ('/', false, Some('*')) => {
                chars.next();
                let mut last = ' ';
                for next in chars.by_ref() {
                    if last == '*' && next == '/' {
                        break;
                    }
                    last = next;
                }
            }
            _ => plain.push(ch),
        }
    }
    without_trailing_commas(&plain)
}

/// `json` with every comma that closes a list or an object taken out.
fn without_trailing_commas(json: &str) -> String {
    let chars = json.chars().collect::<Vec<_>>();
    let mut plain = String::with_capacity(json.len());
    let mut quoted = false;
    let mut escaped = false;
    for (index, ch) in chars.iter().copied().enumerate() {
        if quoted {
            let closing = ch == '"' && !escaped;
            escaped = !escaped && ch == '\\';
            quoted = !closing;
            plain.push(ch);
            continue;
        }
        quoted = ch == '"';
        let closes = || {
            chars[index + 1..]
                .iter()
                .find(|next| !next.is_whitespace())
                .is_some_and(|next| matches!(next, ']' | '}'))
        };
        if ch == ',' && closes() {
            continue;
        }
        plain.push(ch);
    }
    plain
}
