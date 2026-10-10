//! Installing a compatible endpoint after SSH authentication has succeeded.

use crate::remote::Remote;
use std::io::{self, Write};
use std::process::{Command, Stdio};

/// The release installer shared with manual server installation.
const INSTALLER: &str = include_str!("../../../install.sh");

/// The private directory containing this client's compatible endpoint.
fn directory() -> String {
    format!(
        "$HOME/.cache/pandemonium/server/{}-{}",
        env!("CARGO_PKG_VERSION"),
        crate::wire::PROTOCOL
    )
}

/// Starts the private endpoint when present, otherwise the user's installed server.
pub(crate) fn server_command() -> String {
    let directory = directory();
    shell_command(&format!(
        "if [ -x \"{directory}/pandemonium-server\" ]; then exec \"{directory}/pandemonium-server\" --stdio; else exec pandemonium-server --stdio; fi"
    ))
}

/// Runs POSIX setup commands without depending on the user's login shell syntax.
fn shell_command(script: &str) -> String {
    format!("sh -c '{}'", script.replace('\'', "'\\''"))
}

impl Remote {
    /// Installs a matching sibling binary or a checksum-verified pinned release.
    pub(crate) fn install_server(&self) -> io::Result<()> {
        let machine = self.execute("uname -s && uname -m", &[])?;
        let machine = String::from_utf8_lossy(&machine);
        let mut lines = machine.lines();
        let os = match lines.next() {
            Some("Linux") => "linux",
            Some("Darwin") => "macos",
            _ => {
                return Err(io::Error::other(
                    "Automatic server setup requires a Linux or macOS SSH host",
                ));
            }
        };
        let arch = match lines.next() {
            Some("x86_64" | "amd64") => "x86_64",
            Some("aarch64" | "arm64") => "aarch64",
            _ => return Err(io::Error::other("Unsupported remote architecture")),
        };
        let sibling = std::env::current_exe()?.with_file_name("pandemonium-server");
        if os == std::env::consts::OS && arch == std::env::consts::ARCH && sibling.is_file() {
            let version = Command::new(&sibling).arg("--version").output()?;
            if !version.status.success()
                || String::from_utf8_lossy(&version.stdout).trim()
                    != format!("pandemonium-server {}", env!("CARGO_PKG_VERSION"))
            {
                return Err(io::Error::other(
                    "The adjacent server binary does not match this editor",
                ));
            }
            let directory = directory();
            let script = format!(
                "set -eu; mkdir -p \"{directory}\"; destination=\"{directory}/pandemonium-server\"; staging=\"$destination.$$.new\"; trap 'rm -f \"$staging\"' EXIT HUP INT TERM; cat > \"$staging\"; chmod 755 \"$staging\"; \"$staging\" --version >/dev/null; mv -f \"$staging\" \"$destination\""
            );
            self.execute(&script, &std::fs::read(sibling)?)?;
        } else {
            let installer = format!(
                "export PANDEMONIUM_COMPONENT=server PANDEMONIUM_VERSION='{}' PANDEMONIUM_BIN_DIR=\"{}\";\n{}",
                env!("CARGO_PKG_VERSION"),
                directory(),
                INSTALLER
            );
            self.execute("sh -s", installer.as_bytes())?;
        }
        Ok(())
    }

    /// Runs a bootstrap command through the authenticated SSH connection.
    fn execute(&self, script: &str, bytes: &[u8]) -> io::Result<Vec<u8>> {
        let mut command = Command::new("ssh");
        command.args([
            "-T",
            "-o",
            "BatchMode=yes",
            "-o",
            "ConnectTimeout=10",
            "-o",
            "ServerAliveInterval=15",
            "-o",
            "ServerAliveCountMax=3",
        ]);
        if let Some(control) = self.control.lock().unwrap().as_ref() {
            command.arg("-S").arg(control);
        }
        let mut child = command
            .arg(&self.name)
            .arg(shell_command(script))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;
        let mut input = child
            .stdin
            .take()
            .ok_or_else(|| io::Error::other("SSH has no input"))?;
        let bytes = bytes.to_vec();
        let writing = std::thread::spawn(move || input.write_all(&bytes));
        let output = child.wait_with_output()?;
        writing
            .join()
            .map_err(|_| io::Error::other("Server upload interrupted"))??;
        if !output.status.success() {
            return Err(io::Error::other(
                String::from_utf8_lossy(&output.stderr).trim().to_owned(),
            ));
        }
        Ok(output.stdout)
    }
}
