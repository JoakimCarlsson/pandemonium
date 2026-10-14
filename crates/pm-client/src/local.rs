//! Finding the server on this machine, and starting it when it is not running.

use std::net::{SocketAddr, TcpStream};
use std::time::{Duration, Instant};

use pm_protocol::{Build, Credential, Target};

/// The name of the credential file the server leaves under the app's data folder.
const CREDENTIAL_FILE: &str = "server.json";

/// How long a connection attempt may take before it counts as refused.
const DIAL: Duration = Duration::from_millis(400);

/// How long a server that was just started has to begin listening.
const STARTUP: Duration = Duration::from_secs(8);

/// How long a server that was told to stop has to stop listening.
const SHUTDOWN: Duration = Duration::from_secs(4);

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

/// Stops the server on this machine when it runs an earlier build of this very program, or is too old to say what it runs, so that a rebuilt program is not served by the code it replaced. Its sessions are kept on disk and its agents start again on their next prompt.
fn retire_outdated() {
    let Some(held) = credential() else {
        return;
    };
    let address = format!("127.0.0.1:{}", held.port);
    let outdated = match (held.build, Build::current()) {
        (None, _) => true,
        (Some(running), Some(current)) => running.superseded_by(&current),
        (Some(_), None) => false,
    };
    if !outdated || !listening(&address) || !terminate(held.pid) {
        return;
    }
    let asked = Instant::now();
    while asked.elapsed() < SHUTDOWN && listening(&address) {
        std::thread::sleep(Duration::from_millis(100));
    }
}

/// Asks the process `pid` to end, and returns whether the request was delivered.
fn terminate(pid: u32) -> bool {
    let pid = pid.to_string();
    let mut command = if cfg!(windows) {
        let mut command = std::process::Command::new("taskkill");
        command.args(["/PID", &pid, "/F"]);
        command
    } else {
        let mut command = std::process::Command::new("kill");
        command.args(["-TERM", &pid]);
        command
    };
    command
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

/// Starts this program again in its server mode, on a port of the system's choosing, apart from this process so that it outlives it.
fn start() -> std::io::Result<()> {
    let mut command = std::process::Command::new(std::env::current_exe()?);
    command
        .args(["--server", "--listen", "127.0.0.1:0"])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    #[cfg(unix)]
    std::os::unix::process::CommandExt::process_group(&mut command, 0);
    command.spawn().map(|_| ())
}

/// The server on this machine, started first when it is not running.
pub fn local() -> Result<Target, String> {
    retire_outdated();
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
