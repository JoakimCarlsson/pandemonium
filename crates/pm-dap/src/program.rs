//! Finding the programs adapters are started as, and the port one of them listens on.

use std::env;
use std::ffi::OsString;
use std::io;
use std::net::TcpListener;
use std::path::{Path, PathBuf};

/// The folders under the home folder where programs are usually installed without being on the path.
const TOOL_DIRECTORIES: [&str; 5] = [
    ".cargo/bin",
    ".local/bin",
    "go/bin",
    ".bun/bin",
    ".volta/bin",
];

/// Where `program` is installed, on the path or in the usual places beside it.
pub(crate) fn installed(program: &str) -> Option<PathBuf> {
    let path = env::var_os("PATH").unwrap_or_default();
    let home = env::home_dir();
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
        .map(|directory| directory.join(program))
        .find(|candidate| candidate.is_file())
}

/// The machine's path with the folder holding `program` put first, so that what an adapter starts is found beside it.
pub(crate) fn path_beside(program: &Path) -> OsString {
    let first = program.parent().map(Path::to_path_buf);
    let machine = env::var_os("PATH")
        .map(|path| env::split_paths(&path).collect::<Vec<_>>())
        .unwrap_or_default();
    env::join_paths(first.into_iter().chain(machine)).unwrap_or_default()
}

/// A port on the loopback address nothing listens on now.
pub(crate) fn free_port() -> io::Result<u16> {
    Ok(TcpListener::bind(("127.0.0.1", 0))?.local_addr()?.port())
}
