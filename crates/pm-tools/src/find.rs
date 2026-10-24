//! Looking for a program on the path and in the places tools install themselves.

use std::env;
use std::ffi::OsString;
use std::path::{Path, PathBuf};

/// The folders under the home folder where programs are usually installed without being on the path.
///
/// A window started from a desktop session inherits the path that session was
/// given, which is not the one a shell has: npm, bun, cargo and go each put
/// their programs somewhere only a shell profile hears about.
const TOOL_DIRECTORIES: [&str; 10] = [
    ".local/bin",
    ".grok/bin",
    ".bun/bin",
    ".deno/bin",
    ".npm-global/bin",
    "AppData/Roaming/npm",
    ".volta/bin",
    ".cargo/bin",
    "go/bin",
    "sdk/go/bin",
];

/// The folders outside the home folder where programs are usually installed without being on the path.
const SYSTEM_DIRECTORIES: [&str; 3] = ["/usr/local/bin", "/usr/local/go/bin", "/opt/homebrew/bin"];

/// The `bin` folder of every node version nvm has installed, newest name first.
fn nvm_directories(home: &Path) -> Vec<PathBuf> {
    let mut directories: Vec<PathBuf> = std::fs::read_dir(home.join(".nvm/versions/node"))
        .into_iter()
        .flatten()
        .flatten()
        .map(|version| version.path().join("bin"))
        .collect();
    directories.sort();
    directories.reverse();
    directories
}

/// The names a program called `name` may have on disk on this platform.
fn candidates(name: &str) -> Vec<String> {
    if cfg!(windows) {
        vec![
            format!("{name}.exe"),
            format!("{name}.cmd"),
            format!("{name}.bat"),
            name.to_owned(),
        ]
    } else {
        vec![name.to_owned()]
    }
}

/// The path the reader's login shell builds, which a window started from the desktop never inherits.
///
/// Found once; empty on Windows, when the shell does not answer within three seconds or when it names no path.
fn shell_path() -> &'static OsString {
    static PATH: std::sync::OnceLock<OsString> = std::sync::OnceLock::new();
    PATH.get_or_init(|| {
        if cfg!(windows) {
            return OsString::new();
        }
        let shell = env::var_os("SHELL").unwrap_or_else(|| "/bin/sh".into());
        let (sender, receiver) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let output = std::process::Command::new(shell)
                .args(["-ilc", "printf '%s' \"$PATH\""])
                .stdin(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .output();
            let _ = sender.send(output);
        });
        match receiver.recv_timeout(std::time::Duration::from_secs(3)) {
            Ok(Ok(output)) if output.status.success() => {
                String::from_utf8_lossy(&output.stdout).trim().into()
            }
            _ => OsString::new(),
        }
    })
}

/// The machine's path followed by the usual places programs install themselves, in the order they are searched.
fn search_directories() -> Vec<PathBuf> {
    let path = env::var_os("PATH").unwrap_or_default();
    let home = env::home_dir();
    env::split_paths(&path)
        .chain(env::split_paths(shell_path()))
        .chain(
            TOOL_DIRECTORIES
                .iter()
                .filter_map(|directory| Some(home.as_ref()?.join(directory))),
        )
        .chain(home.as_deref().map(nvm_directories).unwrap_or_default())
        .chain(SYSTEM_DIRECTORIES.iter().map(PathBuf::from))
        .collect()
}

/// Where `program` is installed, on the path or in the usual places beside it.
pub fn installed(program: &str) -> Option<PathBuf> {
    search_directories()
        .into_iter()
        .flat_map(|directory| {
            candidates(program)
                .into_iter()
                .map(move |name| directory.join(name))
        })
        .find(|candidate| candidate.is_file())
}

/// The machine's path with the folder holding `program` put first, so that what it starts is found beside it, and
/// the usual places programs install themselves that exist added after it, so that a program started from a
/// desktop session finds the tools a shell profile would have put on the path.
pub fn path_beside(program: &Path) -> OsString {
    let mut directories: Vec<PathBuf> = Vec::new();
    for directory in program
        .parent()
        .map(Path::to_path_buf)
        .into_iter()
        .chain(search_directories())
    {
        if directory.is_dir() && !directories.contains(&directory) {
            directories.push(directory);
        }
    }
    env::join_paths(directories).unwrap_or_default()
}
