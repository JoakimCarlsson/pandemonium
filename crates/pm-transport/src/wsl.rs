//! Reaching a server inside a Windows Subsystem for Linux distribution, started there when it is not running.

use std::process::{Command, Stdio};

use pm_protocol::{Target, is_destination};

use crate::bridge::Progress;
use crate::provision::{
    credential_in, detached, install_script, listening, output_of, quiet, read_script,
    serve_script, version, wait_for,
};

/// The program that runs commands in a distribution.
const LAUNCHER: &str = "wsl.exe";

/// A command run in `distro` through `script`, which a shell there reads.
fn inside(distro: &str, script: &str) -> Command {
    let mut command = Command::new(LAUNCHER);
    command.args(["-d", distro, "--exec", "sh", "-c", script]);
    command
}

/// The target of the server already running in `distro`, when one is.
fn running(distro: &str) -> Option<Target> {
    let printed = output_of(&mut inside(distro, &read_script())).ok()?;
    let held = credential_in(&printed)?;
    let address = format!("127.0.0.1:{}", held.port);
    listening(&address).then_some(Target {
        address,
        token: held.token,
    })
}

/// Starts the server in `distro` as a process of its own that keeps the distribution up and outlives this one.
fn start(distro: &str) -> Result<(), String> {
    let mut command = inside(distro, &serve_script("127.0.0.1:0"));
    detached(quiet(&mut command))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map(|_| ())
        .map_err(|error| format!("could not run {LAUNCHER}: {error}"))
}

/// The server inside the Windows Subsystem for Linux distribution `distro`, installed and started there first when it is not running.
///
/// The server keeps running when the window closes: it is a process of the
/// distribution's own, which the distribution is kept up for.
pub fn wsl(distro: &str, progress: Progress) -> Result<Target, String> {
    if !cfg!(windows) {
        return Err("Windows Subsystem for Linux is only reached from Windows".to_owned());
    }
    if !is_destination(distro) {
        return Err(format!("{distro} is not the name of a distribution"));
    }
    progress(&format!("Looking for a server in {distro}"));
    if let Some(target) = running(distro) {
        return Ok(target);
    }
    progress(&format!("Checking that {distro} has the server"));
    output_of(&mut inside(distro, &install_script(version())))
        .map_err(|reason| format!("could not install the server in {distro}: {reason}"))?;
    progress(&format!("Starting the server in {distro}"));
    start(distro)?;
    wait_for(|| running(distro)).ok_or_else(|| format!("the server in {distro} did not start"))
}
