//! Reaching a server through a tunnel that it makes outward, so that no port is open on its machine.

use std::process::Command;

use pm_protocol::{Target, is_destination};

use crate::bridge::bridge;
use crate::provision::output_of;

/// The program that carries a connection through the tunnel.
const CARRIER: &str = "cloudflared";

/// The server behind the tunnel named `hostname`, reached through a local port that carries each connection through the tunnel, with the `token` it was started with.
///
/// The server's machine runs the same carrier as a named tunnel that points at
/// the port the server listens on, so it only ever connects outward.
pub fn tunnel(hostname: &str, token: &str) -> Result<Target, String> {
    if !is_destination(hostname) {
        return Err(format!("{hostname} is not a name a tunnel can be given"));
    }
    output_of(Command::new(CARRIER).arg("--version")).map_err(|reason| {
        format!("{CARRIER} is needed to reach {hostname} through a tunnel: {reason}")
    })?;
    let hostname = hostname.to_owned();
    let address = bridge(move || {
        let mut command = Command::new(CARRIER);
        command.args(["access", "tcp", "--hostname", &hostname]);
        command
    })?;
    Ok(Target {
        address,
        token: token.to_owned(),
    })
}
