//! On macOS, an editor not started from a terminal takes its PATH from the
//! user's login shell. Call before any thread is spawned.

use pm_host::Stdio;
use std::env;
use std::io::IsTerminal;

/// Adopts the login shell PATH for applications launched outside a macOS terminal.
pub fn adopt() {
    if !cfg!(target_os = "macos") || std::io::stdin().is_terminal() {
        return;
    }
    let shell = env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".to_owned());
    let Ok(output) = pm_host::Host::local()
        .command(shell)
        .args(["-l", "-i", "-c", "/usr/bin/printenv PATH"])
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
    else {
        return;
    };
    let stdout = String::from_utf8_lossy(&output.stdout);
    if let Some(path) = stdout.lines().rev().find(|line| !line.is_empty()) {
        unsafe { env::set_var("PATH", path) };
    }
}
