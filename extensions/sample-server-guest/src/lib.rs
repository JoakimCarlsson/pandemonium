//! The module of the Sample Server extension: it answers the hooks the host asks while it runs the
//! language server and the debug adapter of the extension's language. It finds the language server
//! on the path, or fetches the latest release of one into the extension's storage the first time
//! it is wanted, keeps that release, takes older ones away and says what it is doing.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

#[link(wasm_import_module = "pandemonium")]
unsafe extern "C" {
    /// Sends the JSON request at `pointer` of `length` bytes to the host, which answers with a
    /// pointer and a length packed into one `i64`, the pointer in the high half.
    #[link_name = "host"]
    fn host(pointer: *const u8, length: usize) -> i64;
}

/// The repository whose latest release holds the language server.
const REPOSITORY: &str = "docker/docker-language-server";

/// Hands the host `length` bytes of this module's memory to write into.
#[unsafe(no_mangle)]
pub extern "C" fn pm_alloc(length: usize) -> *mut u8 {
    let mut room = Vec::<u8>::with_capacity(length.max(1));
    let pointer = room.as_mut_ptr();
    std::mem::forget(room);
    pointer
}

/// The bytes a pointer and a length packed into `packed` name.
fn unpacked(packed: i64) -> Vec<u8> {
    let pointer = (packed >> 32) as u32 as usize as *const u8;
    let length = packed as u32 as usize;
    unsafe { std::slice::from_raw_parts(pointer, length) }.to_vec()
}

/// `bytes` left in memory for the host to read, as a packed pointer and length.
fn packed(bytes: Vec<u8>) -> i64 {
    let bytes = bytes.into_boxed_slice();
    let length = bytes.len();
    let pointer = Box::leak(bytes).as_ptr() as usize;
    ((pointer as i64) << 32) | length as i64
}

/// What the host answers a request with.
#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
enum Answer {
    /// It did it.
    Ok(Value),
    /// It refused or failed, for this reason.
    Error(String),
}

/// Asks the host for `request`, and gets what it came to or why not.
fn ask(request: Value) -> Result<Value, String> {
    let bytes = serde_json::to_vec(&request).map_err(|error| error.to_string())?;
    let reply = unpacked(unsafe { host(bytes.as_ptr(), bytes.len()) });
    match serde_json::from_slice(&reply).map_err(|error| error.to_string())? {
        Answer::Ok(value) => Ok(value),
        Answer::Error(reason) => Err(reason),
    }
}

/// Tells the reader what the module is doing.
fn say(text: &str) {
    let _ = ask(json!({"op": "status", "text": text}));
}

/// A program to start, as the host is told.
#[derive(Serialize)]
struct Command {
    /// The program.
    command: String,
    /// What it is started with.
    args: Vec<String>,
    /// Variables added to its environment.
    env: BTreeMap<String, String>,
}

/// The name of the file of the language server's release built for `platform`, without the version.
fn asset_stem(platform: &str) -> Option<&'static str> {
    Some(match platform {
        "linux-x86_64" => "docker-language-server-linux-amd64",
        "linux-aarch64" => "docker-language-server-linux-arm64",
        "macos-x86_64" => "docker-language-server-darwin-amd64",
        "macos-aarch64" => "docker-language-server-darwin-arm64",
        "windows-x86_64" => "docker-language-server-windows-amd64",
        "windows-aarch64" => "docker-language-server-windows-arm64",
        _ => return None,
    })
}

/// The path of the language server fetched into the storage for `version`, when it is there.
fn kept(version: &str, file: &str) -> Option<String> {
    let listed = ask(json!({"op": "storage-list", "dir": version})).ok()?;
    listed
        .as_array()?
        .iter()
        .find(|entry| entry["name"] == file)
        .and_then(|entry| entry["path"].as_str())
        .map(str::to_owned)
}

/// Takes every version of the language server but `version` out of the storage, and says how many went.
fn remove_older(version: &str) {
    let Ok(listed) = ask(json!({"op": "storage-list"})) else {
        return;
    };
    let mut removed = 0;
    for entry in listed.as_array().into_iter().flatten() {
        let name = entry["name"].as_str().unwrap_or_default();
        if entry["dir"] == true
            && name != version
            && ask(json!({"op": "storage-remove", "path": name})).is_ok()
        {
            removed += 1;
        }
    }
    if removed > 0 {
        say(&format!(
            "Docker language server {version} ready, {removed} older version(s) removed"
        ));
    }
}

/// The newest version already in the storage, when the latest cannot be asked for.
fn newest_kept(file_for: impl Fn(&str) -> String) -> Option<(String, String)> {
    let listed = ask(json!({"op": "storage-list"})).ok()?;
    let mut versions: Vec<String> = listed
        .as_array()?
        .iter()
        .filter(|entry| entry["dir"] == true)
        .filter_map(|entry| entry["name"].as_str().map(str::to_owned))
        .collect();
    versions.sort();
    versions
        .into_iter()
        .rev()
        .find_map(|version| kept(&version, &file_for(&version)).map(|path| (version, path)))
}

/// The program that starts the language server: the one on the path when there is one, or the
/// latest release, fetched into the storage when it is not there yet.
fn language_server_command(platform: &str) -> Result<Command, String> {
    if let Some(path) = ask(json!({"op": "which", "program": "sample-lsp"}))?.as_str() {
        say("Using sample-lsp from the path");
        return Ok(Command {
            command: path.to_owned(),
            args: vec!["--stdio".to_owned()],
            env: BTreeMap::new(),
        });
    }
    let stem = asset_stem(platform).ok_or_else(|| format!("no language server for {platform}"))?;
    let suffix = if platform.starts_with("windows") { ".exe" } else { "" };
    let file_for = |version: &str| format!("{stem}-{version}{suffix}");
    let program = |path: String| Command {
        command: path,
        args: vec!["start".to_owned(), "--stdio".to_owned()],
        env: BTreeMap::new(),
    };
    say("Asking GitHub for the latest release");
    let release = match ask(json!({"op": "latest-release", "repository": REPOSITORY})) {
        Ok(release) => release,
        Err(reason) => {
            return match newest_kept(file_for) {
                Some((version, path)) => {
                    say(&format!(
                        "Docker language server {version} (kept; GitHub said: {reason})"
                    ));
                    Ok(program(path))
                }
                None => Err(reason),
            };
        }
    };
    let version = release["version"].as_str().unwrap_or_default().to_owned();
    let file = file_for(&version);
    if let Some(path) = kept(&version, &file) {
        say(&format!("Docker language server {version} (kept)"));
        remove_older(&version);
        return Ok(program(path));
    }
    let asset = release["assets"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|asset| asset["name"] == file.as_str())
        .ok_or_else(|| format!("{REPOSITORY} {version} has no {file}"))?;
    say(&format!("Downloading Docker language server {version}"));
    let path = ask(json!({
        "op": "download-file",
        "url": asset["url"],
        "path": format!("{version}/{file}"),
        "sha256": asset["sha256"],
        "executable": true,
    }))?
    .as_str()
    .unwrap_or_default()
    .to_owned();
    say(&format!("Docker language server {version} ready"));
    remove_older(&version);
    Ok(program(path))
}

/// What the language server is told when it starts: where the project is and which shell the reader uses.
fn initialization_options(project: Option<&str>) -> Result<Value, String> {
    let environment = ask(json!({"op": "shell-env"}))?;
    Ok(json!({
        "sample": {
            "project": project.and_then(|folder| folder.rsplit('/').next()),
            "shell": environment["SHELL"],
        }
    }))
}

/// What the language server is told when it asks for its configuration.
fn workspace_configuration() -> Value {
    json!({"sample": {"strict": true, "source": "the Sample Server extension"}})
}

/// What the host asks of the module.
#[derive(Deserialize)]
#[serde(tag = "hook", rename_all = "snake_case")]
enum Call {
    /// Which program starts the language server.
    LanguageServerCommand {
        /// The platform the host runs on.
        #[serde(default)]
        platform: String,
    },
    /// What the language server is told when it starts.
    LanguageServerInitializationOptions {
        /// The folder of the project, when the module may know it.
        #[serde(default)]
        project: Option<String>,
    },
    /// What the language server is told when it asks for its configuration.
    LanguageServerWorkspaceConfiguration {},
    /// What the completions are called in a list.
    LabelForCompletions {
        /// The completions.
        items: Vec<Item>,
    },
    /// What the symbols are called in a list.
    LabelForSymbols {
        /// The symbols.
        items: Vec<Item>,
    },
    /// Which program is the debug adapter.
    DebugAdapterBinary {},
}

/// A completion or a symbol, as the host tells of it.
#[derive(Default, Deserialize)]
#[serde(default)]
struct Item {
    /// What it is called.
    label: String,
    /// What it is called, for a symbol.
    name: String,
    /// What kind of thing it is.
    kind: String,
    /// What the server says about it besides its name.
    detail: Option<String>,
    /// What holds it, for a symbol.
    container: Option<String>,
}

/// What the module answers the host.
fn answer(call: Call) -> Result<Value, String> {
    Ok(match call {
        Call::LanguageServerCommand { platform } => {
            json!({"command": language_server_command(&platform)?})
        }
        Call::LanguageServerInitializationOptions { project } => {
            json!({"json": initialization_options(project.as_deref())?})
        }
        Call::LanguageServerWorkspaceConfiguration {} => json!({"json": workspace_configuration()}),
        Call::LabelForCompletions { items } => json!({"labels": items
            .iter()
            .map(|item| Some(match &item.detail {
                Some(detail) => format!("{}: {} - {detail}", item.kind, item.label),
                None => format!("{}: {}", item.kind, item.label),
            }))
            .collect::<Vec<_>>()}),
        Call::LabelForSymbols { items } => json!({"labels": items
            .iter()
            .map(|item| Some(match &item.container {
                Some(container) => format!("{container}::{} ({})", item.name, item.kind),
                None => format!("{} ({})", item.name, item.kind),
            }))
            .collect::<Vec<_>>()}),
        Call::DebugAdapterBinary {} => {
            let (program, args) = match ask(json!({"op": "which", "program": "lldb-dap"}))?.as_str() {
                Some(path) => (path.to_owned(), Vec::new()),
                None => (
                    ask(json!({"op": "which", "program": "python3"}))?
                        .as_str()
                        .ok_or("neither lldb-dap nor python3 is on this machine")?
                        .to_owned(),
                    vec!["-m".to_owned(), "debugpy.adapter".to_owned()],
                ),
            };
            json!({"command": Command {
                command: program,
                args,
                env: BTreeMap::new(),
            }})
        }
    })
}

/// Answers a hook: reads the call at `pointer`, and leaves the answer in memory for the host.
#[unsafe(no_mangle)]
pub extern "C" fn pm_hook(pointer: *const u8, length: usize) -> i64 {
    let input = unsafe { std::slice::from_raw_parts(pointer, length) };
    let reply = match serde_json::from_slice::<Call>(input) {
        Ok(call) => answer(call).unwrap_or_else(|reason| json!({"error": reason})),
        Err(error) => json!({"error": format!("not a hook this module knows: {error}")}),
    };
    packed(serde_json::to_vec(&reply).unwrap_or_default())
}
