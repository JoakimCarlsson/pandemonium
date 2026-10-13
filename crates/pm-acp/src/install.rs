//! Installing an agent that is not on the machine.

use std::process::{Command, Stdio};

use crate::agent::{Agent, Source};

/// The page that installs Node.js, which runs the agents that are packages.
const NODE: &str = "https://nodejs.org/en/download";

impl Agent {
    /// Installs the agent, and returns once that is over.
    ///
    /// An agent that comes from an installer runs the installer. One that is
    /// a package needs the runner, which is Node.js, so the page that
    /// installs it is opened.
    pub fn install(self) -> Result<(), String> {
        match self.source {
            Source::Installer { script } => {
                let status = Command::new("sh")
                    .args(["-c", script])
                    .stdin(Stdio::null())
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .status()
                    .map_err(|error| format!("could not run the installer: {error}"))?;
                if status.success() {
                    Ok(())
                } else {
                    Err("the installer failed".to_owned())
                }
            }
            Source::Package(_) => open(NODE),
        }
    }
}

/// Opens `address` in the reader's browser.
fn open(address: &str) -> Result<(), String> {
    let opener = if cfg!(target_os = "macos") {
        "open"
    } else {
        "xdg-open"
    };
    Command::new(opener)
        .arg(address)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map(|_| ())
        .map_err(|error| format!("could not open {address}: {error}"))
}
