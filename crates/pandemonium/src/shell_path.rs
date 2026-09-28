//! The login shell's PATH, for an editor not started from a terminal.
//!
//! An app opened from the Finder or the Dock inherits launchd's PATH, which
//! holds only the system directories; the agents, `npx`, git and the language
//! servers live on the PATH the user's shell profile builds.

use std::env;
use std::io::{IsTerminal, Read};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

/// How long the login shell may take to report its PATH before it is abandoned.
const TIMEOUT: Duration = Duration::from_secs(5);

/// How often the login shell is checked for having exited.
const POLL: Duration = Duration::from_millis(10);

/// Replaces PATH with the login shell's on macOS when stdin is not a terminal.
///
/// Called first in `main`, before any thread exists, which is what makes
/// writing the process environment sound. A shell that fails or does not
/// answer in time leaves PATH as it was.
pub fn adopt() {
    if !cfg!(target_os = "macos") || std::io::stdin().is_terminal() {
        return;
    }
    let Some(path) = login_path() else {
        return;
    };
    unsafe { env::set_var("PATH", path) };
}

/// Asks the user's interactive login shell for its PATH.
fn login_path() -> Option<String> {
    let shell = env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".to_owned());
    let mut child = Command::new(shell)
        .args(["-l", "-i", "-c", "/usr/bin/printenv PATH"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let started = Instant::now();
    loop {
        match child.try_wait().ok()? {
            Some(status) if status.success() => break,
            Some(_) => return None,
            None if started.elapsed() > TIMEOUT => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
            None => thread::sleep(POLL),
        }
    }
    let mut output = String::new();
    child.stdout.take()?.read_to_string(&mut output).ok()?;
    output
        .lines()
        .rev()
        .find(|line| !line.is_empty())
        .map(str::to_owned)
}
