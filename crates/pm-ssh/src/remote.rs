//! Reaching a server on another machine through ssh, without anything but ssh on the way.

use std::process::{Command, Stdio};

use pm_protocol::{Credential, Target, is_destination};

/// Where, under the remote reader's home folder, the server leaves its credential.
const CREDENTIAL_PATH: &str = "${XDG_DATA_HOME:-$HOME/.local/share}/pandemonium/server.json";

/// How long ssh may take to connect, in seconds.
const CONNECT_SECONDS: u32 = 10;

/// The options every ssh command here is given: no questions, a short wait and no forwarding but the one asked for.
fn options() -> Vec<String> {
    vec![
        "-T".to_owned(),
        "-o".to_owned(),
        "BatchMode=yes".to_owned(),
        "-o".to_owned(),
        format!("ConnectTimeout={CONNECT_SECONDS}"),
        "-o".to_owned(),
        "ClearAllForwardings=yes".to_owned(),
        "-o".to_owned(),
        "ExitOnForwardFailure=yes".to_owned(),
    ]
}

/// The credential of the server running on `destination`, read over ssh.
fn credential(destination: &str) -> Result<Credential, String> {
    let output = Command::new("ssh")
        .args(options())
        .arg("--")
        .arg(destination)
        .arg(format!("cat \"{CREDENTIAL_PATH}\""))
        .stdin(Stdio::null())
        .output()
        .map_err(|error| format!("could not run ssh: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "no pandemonium-server is running on {destination}: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    serde_json::from_slice(&output.stdout)
        .map_err(|error| format!("{destination} left an unreadable credential: {error}"))
}

/// The server running on `destination`, reached through a local port that carries each connection over ssh.
///
/// Nothing is installed or started on the remote machine: the server has to
/// be running there already.
pub fn remote(destination: &str) -> Result<Target, String> {
    if !is_destination(destination) {
        return Err(format!(
            "{destination} is not a destination ssh can be given"
        ));
    }
    let held = credential(destination)?;
    let (destination, port) = (destination.to_owned(), held.port);
    let address = pm_transport::bridge(move || {
        let mut command = Command::new("ssh");
        command
            .args(options())
            .args(["-W", &format!("127.0.0.1:{port}"), "--", &destination]);
        command
    })?;
    Ok(Target {
        address,
        token: held.token,
    })
}
