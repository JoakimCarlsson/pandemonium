//! Finding the programs the editor runs beside itself, and running them.
//!
//! Language servers and debug adapters are programs the reader installed,
//! wherever their toolchain put them. This is the one place the editor looks
//! for one, so a server and an adapter that live side by side in `~/.cargo/bin`
//! are found the same way.

use std::collections::HashMap;
use std::env;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Mutex, OnceLock};

use crate::install;

/// The directory where the editor installs servers.
static SERVERS: OnceLock<PathBuf> = OnceLock::new();

/// Registers the editor's server directory for program lookup.
pub fn set_servers(directory: PathBuf) {
    let _ = SERVERS.set(directory);
}

/// The directory where the editor installs servers, if registered.
pub fn servers() -> Option<&'static Path> {
    SERVERS.get().map(PathBuf::as_path)
}

/// The installed program named by a completed version directory.
pub fn managed_in(directory: &Path) -> Option<PathBuf> {
    let relative = std::fs::read_to_string(directory.join(".program")).ok()?;
    let program = directory.join(relative);
    program.is_file().then_some(program)
}

/// The directories a program is looked for in besides the path.
///
/// A window started from a desktop session inherits the path that session
/// was given, which is not the one a shell has: rustup, go and npm each put
/// their programs somewhere that only a shell profile ever hears about. A
/// program the reader has installed is a program the editor runs, whether or
/// not the session was told where it lives.
const TOOL_DIRECTORIES: [&str; 9] = [
    ".cargo/bin",
    ".local/bin",
    "go/bin",
    ".bun/bin",
    ".deno/bin",
    ".npm-global/bin",
    "AppData/Roaming/npm",
    ".volta/bin",
    ".local/share/fnm/aliases/default/bin",
];

/// The directories besides the path where a toolchain is usually installed:
/// the ones under the home directory, the Go toolchain's, whichever the
/// environment names as its own, and the package managers'.
fn usual_directories() -> Vec<PathBuf> {
    let home = env::home_dir();
    let named = ["GOROOT", "GOBIN"]
        .into_iter()
        .filter_map(env::var_os)
        .map(|root| {
            let root = PathBuf::from(root);
            match root.ends_with("bin") {
                true => root,
                false => root.join("bin"),
            }
        });
    named
        .chain(
            TOOL_DIRECTORIES
                .iter()
                .filter_map(|directory| Some(home.as_ref()?.join(directory))),
        )
        .chain(
            [
                "/usr/local/go/bin",
                "/usr/local/bin",
                "/opt/homebrew/bin",
                "/snap/bin",
            ]
            .map(PathBuf::from),
        )
        .collect()
}

/// Where `command` is installed, on the path or in the usual places beside it.
///
/// Nothing is started to find out: a program that is nowhere is one the
/// reader does not have, and the editor does not try to run it. The one
/// question asked of a program is a rustup proxy's `rustup which`, once.
pub fn installed(command: &str) -> Option<PathBuf> {
    installed_with_recipe(command, install::recipe(command))
}

/// Finds a custom executable first, then the configured recipe's managed version.
pub fn installed_with_recipe(command: &str, recipe: Option<install::Recipe>) -> Option<PathBuf> {
    let path = env::var_os("PATH").unwrap_or_default();
    let names = file_names(command);
    env::split_paths(&path)
        .chain(usual_directories())
        .flat_map(|directory| names.iter().map(move |name| directory.join(name)))
        .find(|program| program.is_file() && runs(command, program))
        .or_else(|| managed_in(&servers()?.join(command).join(recipe?.directory_name())))
}

/// Whether the program found for `command` at `program` runs `command`.
///
/// A rustup proxy in `.cargo/bin` stands in for every component rustup
/// knows, installed or not; a `rust-analyzer` proxy whose toolchain lacks the
/// component exits at once, and is no `rust-analyzer` at all.
fn runs(command: &str, program: &Path) -> bool {
    if command != "rust-analyzer" {
        return true;
    }
    match proxied_rustup(program) {
        Some(rustup) => rustup_has(&rustup, command),
        None => true,
    }
}

/// The `rustup` that `program` is a proxy of: the one beside it, when
/// `program` is that file linked under another name.
fn proxied_rustup(program: &Path) -> Option<PathBuf> {
    let directory = program.parent()?;
    file_names("rustup")
        .iter()
        .map(|name| directory.join(name))
        .find(|rustup| rustup.is_file() && same_file(program, rustup))
}

/// Whether `left` and `right` are one file, through a symbolic or a hard link.
fn same_file(left: &Path, right: &Path) -> bool {
    if let (Ok(left), Ok(right)) = (left.canonicalize(), right.canonicalize())
        && left == right
    {
        return true;
    }
    match (left.metadata(), right.metadata()) {
        (Ok(left), Ok(right)) => same_metadata(&left, &right),
        _ => false,
    }
}

/// Whether two files' metadata name one file on disk.
#[cfg(unix)]
fn same_metadata(left: &std::fs::Metadata, right: &std::fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt;
    left.dev() == right.dev() && left.ino() == right.ino()
}

/// Whether two files' metadata name one file on disk.
///
/// Rustup hard-links its proxies; without a stable file identity, two
/// executables of one length beside each other are taken to be one.
#[cfg(not(unix))]
fn same_metadata(left: &std::fs::Metadata, right: &std::fs::Metadata) -> bool {
    left.len() == right.len()
}

/// Whether `rustup` has `component` in its active toolchain, asked once per
/// rustup with `rustup which`.
fn rustup_has(rustup: &Path, component: &str) -> bool {
    static ANSWERS: OnceLock<Mutex<HashMap<PathBuf, bool>>> = OnceLock::new();
    let answers = ANSWERS.get_or_init(Mutex::default);
    if let Some(answer) = answers
        .lock()
        .ok()
        .and_then(|answers| answers.get(rustup).copied())
    {
        return answer;
    }
    let answer = Command::new(rustup)
        .args(["which", component])
        .env("PATH", path_beside(rustup))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success());
    if let Ok(mut answers) = answers.lock() {
        answers.insert(rustup.to_path_buf(), answer);
    }
    answer
}

/// Finds the last completed managed version while a newer recipe is unavailable.
pub fn managed_fallback(command: &str) -> Option<PathBuf> {
    let mut versions = std::fs::read_dir(servers()?.join(command))
        .ok()?
        .flatten()
        .filter_map(|entry| {
            Some((
                entry.metadata().ok()?.modified().ok()?,
                managed_in(&entry.path())?,
            ))
        })
        .collect::<Vec<_>>();
    versions.sort_by_key(|(written, _)| *written);
    versions.pop().map(|(_, program)| program)
}

/// The file names `program` is installed under on this platform.
#[cfg(not(windows))]
fn file_names(program: &str) -> Vec<String> {
    vec![program.to_owned()]
}

/// The file names `program` is installed under on this platform.
///
/// Windows runs a program by its extension, and `PATHEXT` lists the ones it
/// runs. The bare name is left out: npm installs a shell script beside each
/// `.cmd`, which Windows cannot start.
#[cfg(windows)]
fn file_names(program: &str) -> Vec<String> {
    env::var("PATHEXT")
        .unwrap_or_else(|_| ".COM;.EXE;.BAT;.CMD".to_owned())
        .split(';')
        .filter(|extension| !extension.is_empty())
        .map(|extension| format!("{program}{extension}"))
        .collect()
}

/// The path a program at `program` runs with: its own directory first, then
/// the inherited path, then the tools the editor installed, then the usual
/// places that exist and are not on it.
///
/// A server written in JavaScript starts through `env node`, and the node it
/// means is the one installed beside it, which a window started from a
/// desktop session is not told about the way a shell is. The same goes for
/// the tools a server runs in turn: gopls runs `go`, and
/// bash-language-server runs `shellcheck`, which the editor may have
/// installed into its own home.
pub fn path_beside(program: &Path) -> OsString {
    let inherited = env::var_os("PATH").unwrap_or_default();
    let beside = program.parent().map(Path::to_path_buf);
    let mut directories = beside
        .into_iter()
        .chain(env::split_paths(&inherited))
        .collect::<Vec<_>>();
    for extra in managed_tool_directories().chain(usual_directories()) {
        if extra.is_dir() && !directories.contains(&extra) {
            directories.push(extra);
        }
    }
    env::join_paths(directories).unwrap_or(inherited)
}

/// The directories holding the tools the editor installed for the servers
/// that run them.
fn managed_tool_directories() -> impl Iterator<Item = PathBuf> {
    NEEDS
        .iter()
        .filter_map(|need| {
            let recipe = install::recipe(need.program)?;
            managed_in(&servers()?.join(need.program).join(recipe.directory_name()))
                .or_else(|| managed_fallback(need.program))
        })
        .filter_map(|program| program.parent().map(Path::to_path_buf))
}

/// A program a server runs in turn, and what is lost without it.
#[derive(Debug, Eq, Hash, PartialEq)]
pub struct Need {
    /// The server that runs it.
    pub server: &'static str,
    /// The program it runs.
    pub program: &'static str,
    /// What the server cannot do without it.
    pub loss: &'static str,
}

impl Need {
    /// Whether the editor has a recipe to install the program itself.
    pub fn installable(&self) -> bool {
        install::recipe(self.program).is_some()
    }

    /// A line saying the program is missing and what that costs.
    pub fn explanation(&self) -> String {
        format!(
            "{} needs `{}`, which is not on the PATH or in the usual places: {}. Install it or add its directory to PATH.",
            self.server, self.program, self.loss
        )
    }
}

/// Every program a server runs in turn.
///
/// A server that starts but cannot find one of them fails somewhere else,
/// with a message about its own work rather than about the missing program.
const NEEDS: [Need; 2] = [
    Need {
        server: "gopls",
        program: "go",
        loss: "gopls cannot load a workspace without it",
    },
    Need {
        server: "bash-language-server",
        program: "shellcheck",
        loss: "shell scripts are not linted without it",
    },
];

/// The programs `command` runs in turn.
pub fn needs(command: &str) -> impl Iterator<Item = &'static Need> {
    NEEDS.iter().filter(move |need| need.server == command)
}

/// The servers that run `program` in turn.
pub fn needed_by(program: &str) -> impl Iterator<Item = &'static str> {
    NEEDS
        .iter()
        .filter(move |need| need.program == program)
        .map(|need| need.server)
}

/// The programs `command` runs in turn that are not installed.
pub fn missing_for(command: &str) -> Vec<&'static Need> {
    needs(command)
        .filter(|need| installed(need.program).is_none())
        .collect()
}
