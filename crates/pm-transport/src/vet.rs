//! Refuses the settings of a `devcontainer.json` that would let a session's container reach past its worktree.

use std::path::Path;

use crate::devcontainer::Config;

/// The `docker run` flags a session container accepts, each written as `--flag=value` or alone.
const ALLOWED_FLAGS: [&str; 4] = ["--init", "--label", "--hostname", "--env"];

/// The keys of a `--mount` a session container accepts.
const ALLOWED_MOUNT_KEYS: [&str; 7] = [
    "type",
    "source",
    "src",
    "target",
    "dst",
    "destination",
    "readonly",
];

/// Checks that the container `config` asks for stays inside the worktree `folder`, and names the first setting that does not.
pub(crate) fn vet(config: &Config, folder: &str) -> Result<(), String> {
    for argument in &config.run_arguments {
        let flag = argument.split('=').next().unwrap_or(argument);
        if !ALLOWED_FLAGS.contains(&flag) {
            return Err(refusal(argument));
        }
    }
    for mount in &config.mounts {
        refuse_mount(mount, folder)?;
    }
    Ok(())
}

/// The message for a setting that was refused.
fn refusal(setting: &str) -> String {
    format!(
        "the devcontainer.json asks for `{setting}`, which a session container does not allow because it could reach past the worktree"
    )
}

/// Refuses `mount`, as `docker run --mount` reads it, unless it is a volume, a tmpfs or a bind of a path inside `folder`.
fn refuse_mount(mount: &str, folder: &str) -> Result<(), String> {
    let mut kind = "volume".to_owned();
    let mut source = None;
    for part in mount.split(',') {
        let (key, value) = part.split_once('=').unwrap_or((part, ""));
        let key = key.trim().to_ascii_lowercase();
        if !ALLOWED_MOUNT_KEYS.contains(&key.as_str()) {
            return Err(refusal(mount));
        }
        match key.as_str() {
            "type" => kind = value.trim().to_ascii_lowercase(),
            "source" | "src" => source = Some(value),
            _ => {}
        }
    }
    match kind.as_str() {
        "volume" | "tmpfs" => Ok(()),
        "bind" if source.is_some_and(|path| inside(path, folder)) => Ok(()),
        _ => Err(refusal(mount)),
    }
}

/// Whether `path` resolves, links followed, to somewhere in `folder`; a path that cannot be resolved is not.
fn inside(path: &str, folder: &str) -> bool {
    match (
        Path::new(path).canonicalize(),
        Path::new(folder).canonicalize(),
    ) {
        (Ok(path), Ok(folder)) => path.starts_with(folder),
        _ => false,
    }
}
