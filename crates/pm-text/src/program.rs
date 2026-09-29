//! Finding the programs the editor runs beside itself, and running them.
//!
//! Language servers and debug adapters are programs the reader installed,
//! wherever their toolchain put them. This is the one place the editor looks
//! for one, so a server and an adapter that live side by side in `~/.cargo/bin`
//! are found the same way.

use std::env;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

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

/// Where `command` is installed, on the path or in the usual places beside it.
///
/// Nothing is started to find out: a program that is nowhere is one the
/// reader does not have, and the editor does not try to run it.
pub fn installed(command: &str) -> Option<PathBuf> {
    let path = env::var_os("PATH").unwrap_or_default();
    let home = env::home_dir();
    let names = file_names(command);

    env::split_paths(&path)
        .chain(
            TOOL_DIRECTORIES
                .iter()
                .filter_map(|directory| Some(home.as_ref()?.join(directory))),
        )
        .chain([
            PathBuf::from("/usr/local/bin"),
            PathBuf::from("/opt/homebrew/bin"),
        ])
        .flat_map(|directory| names.iter().map(move |name| directory.join(name)))
        .find(|program| program.is_file())
        .or_else(|| {
            let version = install::recipe(command)?.version();
            managed_in(&servers()?.join(command).join(version))
        })
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

/// The path a program at `program` runs with: its own directory first.
///
/// A server written in JavaScript starts through `env node`, and the node it
/// means is the one installed beside it, which a window started from a
/// desktop session is not told about the way a shell is.
pub fn path_beside(program: &Path) -> OsString {
    let inherited = env::var_os("PATH").unwrap_or_default();
    let beside = program.parent().map(Path::to_path_buf);
    env::join_paths(beside.into_iter().chain(env::split_paths(&inherited))).unwrap_or(inherited)
}
