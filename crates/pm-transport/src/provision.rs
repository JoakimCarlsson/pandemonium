//! What every transport that starts the server itself does the same way: finding the program, installing it, starting it and reading what it left.

use std::net::{SocketAddr, TcpStream};
use std::process::Command;
use std::time::{Duration, Instant};

use pm_protocol::Credential;

/// Where, under the reader's home folder on the machine the server runs on, the server leaves its credential.
pub(crate) const CREDENTIAL_PATH: &str =
    "${XDG_DATA_HOME:-$HOME/.local/share}/pandemonium/server.json";

/// The program the server is run as.
const PROGRAM: &str = "pandemonium-server";

/// Where the installer puts the server, which a non-interactive shell does not have on its path.
const PATH: &str = "export PATH=\"$HOME/.local/bin:$PATH\"";

/// How long a connection attempt to a server that may not be there may take.
const DIAL: Duration = Duration::from_millis(800);

/// How long a server that was just started has to leave its credential and listen.
const STARTUP: Duration = Duration::from_secs(60);

/// How long to wait between looks at a server that is starting.
const LOOK: Duration = Duration::from_millis(250);

/// The script that prints the credential the server left, and fails when it left none.
pub(crate) fn read_script() -> String {
    format!("cat \"{CREDENTIAL_PATH}\"")
}

/// The script that finds the server program and installs it from the release of `version` when it is missing, failing with a reason when it cannot.
pub(crate) fn install_script(version: &str) -> String {
    format!(
        "{PATH}; command -v {PROGRAM} >/dev/null 2>&1 || {{ \
         command -v curl >/dev/null 2>&1 || {{ echo 'curl is needed to install {PROGRAM}' >&2; exit 1; }}; \
         curl -fsSL https://raw.githubusercontent.com/JoakimCarlsson/pandemonium/main/install.sh \
         | PANDEMONIUM_COMPONENT=server PANDEMONIUM_VERSION='{version}' sh; }}"
    )
}

/// The script that forgets the old credential and becomes the server, listening on `listen`.
pub(crate) fn serve_script(listen: &str) -> String {
    format!("{PATH}; rm -f \"{CREDENTIAL_PATH}\"; exec {PROGRAM} --listen {listen}")
}

/// The version of this program, which the server it installs is asked to be.
pub(crate) fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// Keeps a program run for a transport from opening a console window of its own where the system would.
pub(crate) fn quiet(command: &mut Command) -> &mut Command {
    #[cfg(windows)]
    std::os::windows::process::CommandExt::creation_flags(command, 0x0800_0000);
    command
}

/// Keeps a program run for a transport apart from this process, so that it goes on when this process ends.
pub(crate) fn detached(command: &mut Command) -> &mut Command {
    #[cfg(unix)]
    std::os::unix::process::CommandExt::process_group(command, 0);
    #[cfg(windows)]
    std::os::windows::process::CommandExt::creation_flags(command, 0x0800_0008);
    command
}

/// What `command` printed when it succeeded, or the reason it did not.
pub(crate) fn output_of(command: &mut Command) -> Result<Vec<u8>, String> {
    let output = quiet(command)
        .stdin(std::process::Stdio::null())
        .output()
        .map_err(|error| format!("could not run {}: {error}", program_of(command)))?;
    if output.status.success() {
        return Ok(output.stdout);
    }
    let reason = String::from_utf8_lossy(&output.stderr);
    let reason = reason.trim();
    Err(if reason.is_empty() {
        format!("{} failed", program_of(command))
    } else {
        reason.to_owned()
    })
}

/// The name of the program `command` runs.
fn program_of(command: &Command) -> String {
    command.get_program().to_string_lossy().into_owned()
}

/// The credential in what a read script printed, when it is one.
pub(crate) fn credential_in(printed: &[u8]) -> Option<Credential> {
    serde_json::from_slice(printed).ok()
}

/// Whether something listens at `address`.
pub(crate) fn listening(address: &str) -> bool {
    address
        .parse::<SocketAddr>()
        .is_ok_and(|address| TcpStream::connect_timeout(&address, DIAL).is_ok())
}

/// Looks again and again, until [`STARTUP`] has passed, for what `look` finds, and gives what it found.
pub(crate) fn wait_for<T>(mut look: impl FnMut() -> Option<T>) -> Option<T> {
    let started = Instant::now();
    loop {
        if let Some(found) = look() {
            return Some(found);
        }
        if started.elapsed() >= STARTUP {
            return None;
        }
        std::thread::sleep(LOOK);
    }
}
