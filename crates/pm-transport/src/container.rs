//! Reaching a server inside the dev container of a folder, which is built, started and given the server first when it is not running.

use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::Path;
use std::process::{Command, Stdio};

use pm_protocol::Target;

use crate::bridge::Progress;
use crate::devcontainer::{Config, Source, read};
use crate::provision::{
    credential_in, detached, install_script, listening, output_of, quiet, read_script,
    serve_script, version, wait_for,
};

/// The program that builds and runs containers.
const ENGINE: &str = "docker";

/// The label a container carries, with the folder it was made for as its value, so that each folder owns one container.
const OWNER_LABEL: &str = "pandemonium.folder";

/// The port the server listens on inside the container.
const SERVER_PORT: u16 = 6868;

/// A command of the engine with these arguments.
fn engine<I, S>(arguments: I) -> Command
where
    I: IntoIterator<Item = S>,
    S: AsRef<std::ffi::OsStr>,
{
    let mut command = Command::new(ENGINE);
    command.args(arguments);
    command
}

/// What the engine printed for `arguments`, as text, or why it failed.
fn ask<I, S>(arguments: I) -> Result<String, String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<std::ffi::OsStr>,
{
    output_of(&mut engine(arguments))
        .map(|printed| String::from_utf8_lossy(&printed).trim().to_owned())
}

/// The name the image built for `folder` goes by.
fn image_name(folder: &str) -> String {
    let mut hasher = DefaultHasher::new();
    folder.hash(&mut hasher);
    format!("pandemonium-devcontainer-{:016x}", hasher.finish())
}

/// A container of the engine that was made for `folder`, with its identity and whether it is running, when there is one.
fn owned(folder: &str) -> Result<Option<(String, bool)>, String> {
    let listed = ask([
        "ps".to_owned(),
        "-a".to_owned(),
        "--filter".to_owned(),
        format!("label={OWNER_LABEL}={folder}"),
        "--format".to_owned(),
        "{{.ID}} {{.State}}".to_owned(),
    ])?;
    Ok(listed.lines().next().and_then(|line| {
        let (identity, state) = line.split_once(' ')?;
        Some((identity.to_owned(), state == "running"))
    }))
}

/// Makes the image `config` names available as `tag`, and returns the name to run.
fn prepare_image(config: &Config, tag: &str, progress: Progress) -> Result<String, String> {
    match &config.source {
        Source::Image(image) => {
            progress(&format!("Pulling {image}"));
            ask(["pull", image.as_str()])?;
            Ok(image.clone())
        }
        Source::Build {
            dockerfile,
            context,
            arguments,
            target,
        } => {
            progress("Building the container image");
            let mut command = engine(["build", "-t", tag, "-f"]);
            command.arg(dockerfile);
            for argument in arguments {
                command.args(["--build-arg", argument]);
            }
            if let Some(target) = target {
                command.args(["--target", target]);
            }
            command.arg(context);
            output_of(&mut command)?;
            Ok(tag.to_owned())
        }
    }
}

/// Makes and starts the container of `folder` from `image`, and returns its identity.
fn create(folder: &str, image: &str, config: &Config) -> Result<String, String> {
    let mut command = engine(["run", "-d", "--init"]);
    command
        .args(["--label", &format!("{OWNER_LABEL}={folder}")])
        .args(["-p", &format!("127.0.0.1::{SERVER_PORT}")])
        .args(["-w", &config.workspace])
        .args([
            "--mount",
            &format!("type=bind,source={folder},target={}", config.workspace),
        ]);
    for variable in &config.environment {
        command.args(["-e", variable]);
    }
    for mount in &config.mounts {
        command.args(["--mount", mount]);
    }
    command
        .args(&config.run_arguments)
        .args(["--entrypoint", "/bin/sh", image])
        .args([
            "-c",
            "trap 'exit 0' TERM; while sleep 1000 & wait $!; do :; done",
        ]);
    let printed = output_of(&mut command)?;
    Ok(String::from_utf8_lossy(&printed).trim().to_owned())
}

/// The address on this machine that the port of the server inside the container `identity` is published at.
fn published(identity: &str) -> Result<String, String> {
    let ports = ask(["port", identity, &format!("{SERVER_PORT}/tcp")])?;
    ports
        .lines()
        .find(|line| line.starts_with("127.0.0.1:"))
        .or_else(|| ports.lines().next())
        .map(str::to_owned)
        .ok_or_else(|| "the container publishes no port for the server".to_owned())
}

/// A command run in the container `identity` through `script`, which a shell there reads.
fn inside(identity: &str, script: &str) -> Command {
    engine(["exec", identity, "sh", "-c", script])
}

/// The target of the server inside the container `identity`, which is published at `address`, when it is running.
fn running(identity: &str, address: &str) -> Option<Target> {
    let printed = output_of(&mut inside(identity, &read_script())).ok()?;
    let held = credential_in(&printed)?;
    listening(address).then_some(Target {
        address: address.to_owned(),
        token: held.token,
    })
}

/// Starts the server inside the container `identity` as a process of the container's own.
fn start(identity: &str) -> Result<(), String> {
    let script = serve_script(&format!("0.0.0.0:{SERVER_PORT}"));
    let mut command = engine(["exec", "-d", identity, "sh", "-c", &script]);
    detached(quiet(&mut command))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|error| format!("could not run {ENGINE}: {error}"))
        .and_then(|status| {
            status
                .success()
                .then_some(())
                .ok_or_else(|| "the container did not take the server".to_owned())
        })
}

/// The container of `folder`, which is made when the folder has none, started when it is stopped, and given the server when that is not running.
fn ready(folder: &str, progress: Progress) -> Result<(String, String), String> {
    ask(["version", "--format", "{{.Server.Version}}"])
        .map_err(|reason| format!("{ENGINE} is not running: {reason}"))?;
    let identity = match owned(folder)? {
        Some((identity, true)) => identity,
        Some((identity, false)) => {
            progress("Starting the container");
            ask(["start", identity.as_str()])?;
            identity
        }
        None => {
            progress("Reading devcontainer.json");
            let config = read(Path::new(folder))?;
            let image = prepare_image(&config, &image_name(folder), progress)?;
            progress("Creating the container");
            let identity = create(folder, &image, &config)?;
            if let Some(command) = &config.after_create {
                progress("Running the post create command");
                ask([
                    "exec",
                    "-w",
                    config.workspace.as_str(),
                    identity.as_str(),
                    "sh",
                    "-c",
                    command.as_str(),
                ])?;
            }
            identity
        }
    };
    let address = published(&identity)?;
    Ok((identity, address))
}

/// The server inside the dev container of `folder` on this machine, with the container built, started and given the server first when it is not running.
///
/// Each folder owns one container, found again by the label it carries, so a
/// worktree opened a second time reaches the container it opened the first
/// time. The server keeps running in the container when the window closes.
pub fn container(folder: &str, progress: Progress) -> Result<Target, String> {
    let (identity, address) = ready(folder, progress)?;
    if let Some(target) = running(&identity, &address) {
        return Ok(target);
    }
    progress("Checking that the container has the server");
    output_of(&mut inside(&identity, &install_script(version())))
        .map_err(|reason| format!("could not install the server in the container: {reason}"))?;
    progress("Starting the server in the container");
    start(&identity)?;
    wait_for(|| running(&identity, &address))
        .ok_or_else(|| "the server in the container did not start".to_owned())
}
