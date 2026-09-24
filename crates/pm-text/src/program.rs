//! Finding the programs the editor runs beside itself, and running them.
//!
//! Language servers and debug adapters are programs the reader installed,
//! wherever their toolchain put them. This is the one place the editor looks
//! for one, so a server and an adapter that live side by side in `~/.cargo/bin`
//! are found the same way.

use std::env;
use std::ffi::OsString;
use std::path::{Path, PathBuf};

/// The directories a program is looked for in besides the path.
///
/// A window started from a desktop session inherits the path that session
/// was given, which is not the one a shell has: rustup, go and npm each put
/// their programs somewhere that only a shell profile ever hears about. A
/// program the reader has installed is a program the editor runs, whether or
/// not the session was told where it lives.
const TOOL_DIRECTORIES: [&str; 8] = [
    ".cargo/bin",
    ".local/bin",
    "go/bin",
    ".bun/bin",
    ".deno/bin",
    ".npm-global/bin",
    ".volta/bin",
    ".local/share/fnm/aliases/default/bin",
];

/// Where `command` is installed, on the path or in the usual places beside it.
///
/// Nothing is started to find out: a program that is nowhere is one the
/// reader does not have, and the editor does not try to run it.
pub fn installed(command: &str) -> Option<PathBuf> {
    let path = env::var_os("PATH").unwrap_or_default();
    let home = env::var_os("HOME").map(PathBuf::from);

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
        .map(|directory| directory.join(command))
        .find(|program| program.is_file())
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
