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

/// Finds a program on the local execution machine or in managed installations.
pub fn installed(command: &str) -> Option<PathBuf> {
    pm_host::Host::local().which(command).or_else(|| {
        let version = install::recipe(command)?.version();
        managed_in(&servers()?.join(command).join(version))
    })
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
