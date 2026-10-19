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

/// Where `program` is installed, on the path or in the usual places beside it.
pub fn installed(program: &str) -> Option<PathBuf> {
    let path = env::var_os("PATH").unwrap_or_default();
    let home = env::home_dir();
    env::split_paths(&path)
        .chain(
            TOOL_DIRECTORIES
                .iter()
                .filter_map(|directory| Some(home.as_ref()?.join(directory))),
        )
        .chain(home.as_deref().map(nvm_directories).unwrap_or_default())
        .chain(SYSTEM_DIRECTORIES.iter().map(PathBuf::from))
        .flat_map(|directory| {
            candidates(program)
                .into_iter()
                .map(move |name| directory.join(name))
        })
        .find(|candidate| candidate.is_file())
}

/// The machine's path with the folder holding `program` put first, so that what it starts is found beside it.
pub fn path_beside(program: &Path) -> OsString {
    let first = program.parent().map(Path::to_path_buf);
    let machine = env::var_os("PATH")
        .map(|path| env::split_paths(&path).collect::<Vec<_>>())
        .unwrap_or_default();
    env::join_paths(first.into_iter().chain(machine)).unwrap_or_default()
}
