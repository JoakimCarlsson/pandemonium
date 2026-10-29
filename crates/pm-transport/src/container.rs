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
pub(crate) fn engine<I, S>(arguments: I) -> Command
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

/// The name the container of `folder` goes by: the folder's own name, which a person can tell apart in a list, and a short hash, which keeps two folders with one name apart.
fn container_name(folder: &str) -> String {
    let mut hasher = DefaultHasher::new();
    folder.hash(&mut hasher);
    let readable: String = Path::new(folder)
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default()
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || character == '-' {
                character
            } else {
                '-'
            }
        })
        .collect();
    format!(
        "pandemonium-{}-{:08x}",
        readable.trim_matches('-'),
        hasher.finish() as u32
    )
}

/// Where the features of the container of `folder` are fetched to and built from.
fn features_scratch(folder: &str) -> std::path::PathBuf {
    let home = std::env::var_os("XDG_CACHE_HOME")
        .map(std::path::PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| Path::new(&home).join(".cache")))
        .unwrap_or_else(std::env::temp_dir);
    home.join("pandemonium").join(image_name(folder))
}

/// The label a container carries, with the worktree of the session it was made for as its value, so that each opted-in session owns one container.
const SESSION_LABEL: &str = "pandemonium.session";

/// A container of the engine that carries `label` with the value `folder`, with its identity and whether it is running, when there is one.
fn owned(label: &str, folder: &str) -> Result<Option<(String, bool)>, String> {
    let listed = ask([
        "ps".to_owned(),
        "-a".to_owned(),
        "--filter".to_owned(),
        format!("label={label}={folder}"),
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

/// Makes and starts the container of `folder` from `image`, marked with `label`, and returns its identity.
fn create(label: &str, folder: &str, image: &str, config: &Config) -> Result<String, String> {
    let mut command = engine(["run", "-d", "--init"]);
    command
        .args(["--name", &container_name(folder)])
        .args(["--label", &format!("{label}={folder}")])
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

/// The container of `folder` that carries `label`, which is made when the folder has none and started when it is stopped.
fn ready(label: &str, folder: &str, progress: Progress) -> Result<String, String> {
    ask(["version", "--format", "{{.Server.Version}}"])
        .map_err(|reason| format!("{ENGINE} is not running: {reason}"))?;
    let identity = match owned(label, folder)? {
        Some((identity, true)) => identity,
        Some((identity, false)) => {
            progress("Starting the container");
            ask(["start", identity.as_str()])?;
            identity
        }
        None => {
            progress("Reading devcontainer.json");
            let config = read(Path::new(folder))?;
            let mut image = prepare_image(&config, &image_name(folder), progress)?;
            if !config.features.is_empty() {
                let tag = format!("{}-features", image_name(folder));
                image = crate::features::install(
                    &config.features,
                    &image,
                    &tag,
                    &features_scratch(folder),
                    progress,
                )?;
            }
            progress("Creating the container");
            let identity = create(label, folder, &image, &config)?;
            if let Some(command) = &config.after_create {
                progress("Running the post create command");
                if let Err(reason) = ask([
                    "exec",
                    "-w",
                    config.workspace.as_str(),
                    identity.as_str(),
                    "sh",
                    "-c",
                    command.as_str(),
                ]) {
                    let _ = ask(["rm", "-f", identity.as_str()]);
                    return Err(reason);
                }
            }
            identity
        }
    };
    Ok(identity)
}

/// The server inside the dev container of `folder` on this machine, with the container built, started and given the server first when it is not running.
///
/// Each folder owns one container, found again by the label it carries, so a
/// worktree opened a second time reaches the container it opened the first
/// time. The server keeps running in the container when the window closes.
pub fn container(folder: &str, progress: Progress) -> Result<Target, String> {
    let identity = ready(OWNER_LABEL, folder, progress)?;
    let address = published(&identity)?;
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

/// How a command runs inside the container of a session, and where the session's worktree is in it.
pub struct Enclosed {
    /// The identity of the container.
    identity: String,
    /// Where the worktree is inside the container.
    pub workspace: String,
}

/// Whether the variable `name` may be handed to a command in the container: a plain name that is not `PATH`, which names places on this machine, and not one the dynamic loader or the engine's own client reads, because the value is also set on the `docker` process that runs on this machine.
fn forwarded(name: &std::ffi::OsStr) -> bool {
    let Some(name) = name.to_str() else {
        return false;
    };
    let plain = name
        .chars()
        .all(|character| character.is_ascii_alphanumeric() || character == '_')
        && !name.starts_with(|character: char| character.is_ascii_digit());
    let reserved = name == "PATH"
        || ["LD_", "DYLD_", "DOCKER_", "CONTAINER_", "BUILDKIT_"]
            .iter()
            .any(|prefix| name.starts_with(prefix));
    plain && !name.is_empty() && !reserved
}

impl Enclosed {
    /// `command` as `docker exec` runs it in the container, in the workspace and in the folder `command` is set to run in, or else the workspace, with the environment `command` was given except `PATH`, which names places on this machine; the program is found by its name on the container's own path. `terminal` gives the process one.
    pub fn wrap(&self, command: &Command, terminal: bool) -> Command {
        let mut wrapped = engine(["exec", if terminal { "-it" } else { "-i" }]);
        match command.get_current_dir() {
            Some(folder) => wrapped.arg("-w").arg(folder),
            None => wrapped.arg("-w").arg(&self.workspace),
        };
        for (name, value) in command.get_envs() {
            if let Some(value) = value.filter(|_| forwarded(name)) {
                wrapped.arg("-e").arg(name);
                wrapped.env(name, value);
            }
        }
        let program = Path::new(command.get_program());
        wrapped
            .arg(&self.identity)
            .arg(program.file_name().unwrap_or(program.as_os_str()));
        wrapped.args(command.get_args());
        wrapped
    }
}

/// Whether `folder` holds a `devcontainer.json`.
pub fn configured(folder: &Path) -> bool {
    crate::devcontainer::exists(folder)
}

/// The container of the session working in the worktree `folder`, made, started and prepared first when it is not running, with the worktree mounted at the workspace the `devcontainer.json` names.
///
/// Each worktree owns one container, found again by the label it carries, so a session that starts again reaches the container it had.
pub fn session_container(folder: &str, progress: Progress) -> Result<Enclosed, String> {
    static MAKING: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let _one_at_a_time = MAKING
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let config = read(Path::new(folder))?;
    crate::vet::vet(&config, folder)?;
    let identity = ready(SESSION_LABEL, folder, progress)?;
    Ok(Enclosed {
        identity,
        workspace: config.workspace,
    })
}

/// The container of the session working in the worktree `folder`, when it has one that is running.
pub fn running_session_container(folder: &str) -> Option<Enclosed> {
    let (identity, true) = owned(SESSION_LABEL, folder).ok()?? else {
        return None;
    };
    let config = read(Path::new(folder)).ok()?;
    Some(Enclosed {
        identity,
        workspace: config.workspace,
    })
}

/// Removes the container of the session working in the worktree `folder`, and the image built for it, when it has them.
pub fn remove_session_container(folder: &str) -> Result<(), String> {
    if let Some((identity, _)) = owned(SESSION_LABEL, folder)? {
        ask(["rm", "-f", identity.as_str()])?;
    }
    Ok(())
}
