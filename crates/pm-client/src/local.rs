//! Finding the server on this machine, and starting it when it is not running.

use std::net::{SocketAddr, TcpStream};
use std::time::{Duration, Instant};

use pm_protocol::{Credential, Target};

/// The name of the credential file the server leaves under the app's data folder.
const CREDENTIAL_FILE: &str = "server.json";

/// How long a connection attempt may take before it counts as refused.
const DIAL: Duration = Duration::from_millis(400);

/// How long a server that was just started has to begin listening.
const STARTUP: Duration = Duration::from_secs(8);

/// The credential the server on this machine left, if any.
fn credential() -> Option<Credential> {
    serde_json::from_str(&pm_store::load_text(CREDENTIAL_FILE)?).ok()
}

/// Whether something listens at `address`.
fn listening(address: &str) -> bool {
    address
        .parse::<SocketAddr>()
        .is_ok_and(|address| TcpStream::connect_timeout(&address, DIAL).is_ok())
}

/// The target of the server on this machine when it is running.
fn running() -> Option<Target> {
    let held = credential()?;
    let address = format!("127.0.0.1:{}", held.port);
    listening(&address).then_some(Target {
        address,
        token: held.token,
    })
}

/// Starts the server program that sits beside this one, on a port of the system's choosing, apart from this process so that it outlives it.
fn start() -> std::io::Result<()> {
    let program = std::env::current_exe()?
        .parent()
        .map(|folder| folder.join("pandemonium-server"))
        .ok_or_else(|| std::io::Error::other("this program has no folder"))?;
    let mut command = std::process::Command::new(program);
    command
        .args(["--listen", "127.0.0.1:0"])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    #[cfg(unix)]
    std::os::unix::process::CommandExt::process_group(&mut command, 0);
    command.spawn().map(|_| ())
}

/// The server on this machine, started first when it is not running.
pub fn local() -> Result<Target, String> {
    if let Some(target) = running() {
        return Ok(target);
    }
    start().map_err(|error| format!("could not start the server: {error}"))?;
    let started = Instant::now();
    while started.elapsed() < STARTUP {
        std::thread::sleep(Duration::from_millis(100));
        if let Some(target) = running() {
            return Ok(target);
        }
    }
    Err("the server did not start".to_owned())
}
