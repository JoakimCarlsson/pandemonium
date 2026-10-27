//! Looking for a program on the path and in the places tools install themselves.

use std::env;
use std::ffi::OsString;
use std::path::{Path, PathBuf};

/// The folders under the home folder where programs are usually installed without being on the path, whatever language they are for: the folder a user's own scripts go in, and where agents install themselves.
///
/// A window started from a desktop session inherits the path that session was
/// given, which is not the one a shell has. The places a language's own tools
/// go, such as cargo's or go's, are that language's to say: see [`set_directories`].
const TOOL_DIRECTORIES: [&str; 2] = [".local/bin", ".grok/bin"];

/// The folders outside the home folder where programs are usually installed without being on the path.
const SYSTEM_DIRECTORIES: [&str; 2] = ["/usr/local/bin", "/opt/homebrew/bin"];

/// The places the installed languages say their tools go.
static LANGUAGE_DIRECTORIES: std::sync::RwLock<Vec<String>> = std::sync::RwLock::new(Vec::new());

/// Replaces the places the installed languages say their tools go with `places`: each may start with `~/` for the home folder, and any folder name in it may be `*` for every folder there.
pub fn set_directories(places: Vec<String>) {
    if let Ok(mut held) = LANGUAGE_DIRECTORIES.write() {
        *held = places;
    }
}

/// The folders `places` name, `~/` taken as the home folder and `*` as every folder at its place, in order.
fn expanded(places: &[String], home: Option<&Path>) -> Vec<PathBuf> {
    let mut found = Vec::new();
    for place in places {
        let (start, rest) = match place.strip_prefix("~/") {
            Some(rest) => match home {
                Some(home) => (home.to_path_buf(), rest),
                None => continue,
            },
            None => (PathBuf::from("/"), place.trim_start_matches('/')),
        };
        let mut current = vec![start];
        for part in rest.split('/').filter(|part| !part.is_empty()) {
            current = current
                .into_iter()
                .flat_map(|folder| {
                    if part == "*" {
                        let mut children: Vec<PathBuf> = std::fs::read_dir(&folder)
                            .into_iter()
                            .flatten()
                            .flatten()
                            .map(|entry| entry.path())
                            .collect();
                        children.sort();
                        children.reverse();
                        children
                    } else {
                        vec![folder.join(part)]
                    }
                })
                .collect();
        }
        found.extend(current);
    }
    found
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
        .chain(
            LANGUAGE_DIRECTORIES
                .read()
                .map(|places| expanded(&places, home.as_deref()))
                .unwrap_or_default(),
        )
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

/// The folders the installed languages say their tools generate, such as a build output or installed packages.
static GENERATED: std::sync::RwLock<Vec<String>> = std::sync::RwLock::new(Vec::new());

/// Replaces the folders the installed languages say their tools generate with `names`, each a folder's name.
pub fn set_generated(names: Vec<String>) {
    if let Ok(mut held) = GENERATED.write() {
        *held = names;
    }
}

/// The names of the folders the installed languages say their tools generate, which walks of a project leave out.
pub fn generated() -> Vec<String> {
    GENERATED
        .read()
        .map(|held| held.clone())
        .unwrap_or_default()
}
