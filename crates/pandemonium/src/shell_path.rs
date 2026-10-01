//! On macOS and Linux, an editor not started from a terminal takes its PATH
//! from the user's login shell, which is the one that knows where the user's
//! toolchains live. Call before any thread is spawned.

use std::env;
use std::ffi::OsString;
use std::io::{IsTerminal, Read};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

/// How long the login shell may take to print its PATH before it is given up on.
const SHELL_DEADLINE: Duration = Duration::from_secs(3);

/// How often the shell is looked at while it is waited for.
const SHELL_POLL: Duration = Duration::from_millis(10);

/// Replaces the PATH with the login shell's, keeping every inherited directory
/// the shell's does not name after it.
pub fn adopt() {
    let supported = cfg!(any(target_os = "macos", target_os = "linux"));
    if !supported || std::io::stdin().is_terminal() {
        return;
    }
    let Some(shell_path) = login_shell_path() else {
        return;
    };
    let inherited = env::var_os("PATH").unwrap_or_default();
    let adopted = merged(&shell_path, &inherited);
    unsafe { env::set_var("PATH", adopted) };
}

/// The PATH the login shell prints, if it prints one within [`SHELL_DEADLINE`].
fn login_shell_path() -> Option<OsString> {
    let shell = env::var("SHELL").unwrap_or_else(|_| default_shell().to_owned());
    let mut child = Command::new(shell)
        .args(["-l", "-i", "-c", "printenv PATH"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let exited = wait_for(&mut child, SHELL_DEADLINE);
    if !exited {
        let _ = child.kill();
        let _ = child.wait();
        return None;
    }
    let mut stdout = String::new();
    child.stdout.take()?.read_to_string(&mut stdout).ok()?;
    stdout
        .lines()
        .rev()
        .find(|line| !line.is_empty())
        .map(OsString::from)
}

/// The shell used when the session names none.
const fn default_shell() -> &'static str {
    if cfg!(target_os = "macos") {
        "/bin/zsh"
    } else {
        "/bin/sh"
    }
}

/// Whether `child` exited within `deadline`.
fn wait_for(child: &mut Child, deadline: Duration) -> bool {
    let started = Instant::now();
    while started.elapsed() < deadline {
        if !matches!(child.try_wait(), Ok(None)) {
            return true;
        }
        std::thread::sleep(SHELL_POLL);
    }
    false
}

/// The directories of `first`, then those of `second` that `first` lacks.
fn merged(first: &OsString, second: &OsString) -> OsString {
    let mut directories = env::split_paths(first).collect::<Vec<PathBuf>>();
    for directory in env::split_paths(second) {
        if !directories.contains(&directory) {
            directories.push(directory);
        }
    }
    env::join_paths(directories).unwrap_or_else(|_| first.clone())
}
