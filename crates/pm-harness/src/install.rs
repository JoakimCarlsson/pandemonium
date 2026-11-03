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
            Source::Manual => Err("an agent added by hand has to be installed by hand".to_owned()),
        }
    }
}

/// Whether `address` is a web page: it begins with `http://` or `https://`, has something after that and holds no whitespace.
pub fn is_web_address(address: &str) -> bool {
    let rest = address
        .strip_prefix("https://")
        .or_else(|| address.strip_prefix("http://"));
    rest.is_some_and(|rest| !rest.is_empty() && !address.contains(char::is_whitespace))
}

/// Opens `address` in the reader's browser; only a web address is opened.
pub fn open(address: &str) -> Result<(), String> {
    if !is_web_address(address) {
        return Err(format!("{address} is not a web address"));
    }
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
