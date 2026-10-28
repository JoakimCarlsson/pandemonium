//! Finding a language server's program: in the project first, then on the machine, then in the data folder, installing it there when it is missing and falling back to a version installed before when that fails.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use crate::fetch::{fetch_checked, unpack, write_program};
use crate::manifest::{Install, InstallKind, ServerSpec};
use crate::paths::{platform, program_name, server_root};

/// How long a found program may take to say it works.
const CHECK: Duration = Duration::from_secs(10);

/// The names a program called `name` may have on disk on this platform.
fn candidates(name: &str) -> Vec<String> {
    if cfg!(windows) {
        vec![
            program_name(name),
            format!("{name}.cmd"),
            format!("{name}.bat"),
            name.to_owned(),
        ]
    } else {
        vec![name.to_owned()]
    }
}

/// The program called `name` inside `folder`, if it is there.
fn in_folder(folder: &Path, name: &str) -> Option<PathBuf> {
    candidates(name)
        .into_iter()
        .map(|candidate| folder.join(candidate))
        .find(|path| path.is_file())
}

/// Whether `program` run with `arguments` exits cleanly in time.
fn works(program: &Path, arguments: &[String]) -> bool {
    let Ok(mut child) = Command::new(program)
        .args(arguments)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    else {
        return false;
    };
    let started = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return status.success(),
            Ok(None) if started.elapsed() < CHECK => std::thread::sleep(Duration::from_millis(20)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return false;
            }
        }
    }
}

/// Where the program of `spec` is inside the folder `folder` it was installed in the way `install` says.
fn installed_program(spec: &ServerSpec, install: &Install, folder: &Path) -> Option<PathBuf> {
    let install = crate::legacy::command_form(install);
    let place = match install.bin.as_deref() {
        Some(bin) => folder.join(bin),
        None => folder.to_path_buf(),
    };
    in_folder(&place, &spec.command)
}

/// Runs `command`, saying what it needed, `needs`, when it cannot be started, and what it printed when it fails.
fn run(
    tool: &str,
    needs: &str,
    arguments: &[&str],
    prepare: impl FnOnce(&mut Command),
) -> Result<(), String> {
    let not_here =
        || format!("installing this language server needs {needs}, which is not on this machine");
    let program = pm_tools::installed(tool).ok_or_else(not_here)?;
    let mut command = Command::new(&program);
    command
        .args(arguments)
        .env("PATH", pm_tools::path_beside(&program));
    prepare(&mut command);
    let output = command
        .stdin(Stdio::null())
        .output()
        .map_err(|_| not_here())?;
    if output.status.success() {
        return Ok(());
    }
    let said = String::from_utf8_lossy(&output.stderr);
    let last = said
        .lines()
        .rev()
        .find(|line| !line.trim().is_empty())
        .unwrap_or("it failed");
    Err(format!(
        "installing this language server failed: {}",
        last.trim()
    ))
}

/// Installs the program of `spec` into `folder` the way `install` says.
fn install_into(spec: &ServerSpec, install: &Install, folder: &Path) -> Result<(), String> {
    std::fs::create_dir_all(folder).map_err(|error| error.to_string())?;
    match install.kind {
        InstallKind::Download => {
            let here = platform();
            let download = install
                .downloads
                .get(&here)
                .ok_or_else(|| format!("{} is not built for {here}", spec.command))?;
            let bytes = fetch_checked(&download.url, &download.sha256)?;
            let program = unpack(bytes, download.archive, download.member.as_deref())?;
            write_program(&folder.join(program_name(&spec.command)), &program)
        }
        InstallKind::Command | InstallKind::Go | InstallKind::Npm => {
            run_steps(spec, &crate::legacy::command_form(install), folder)
        }
    }
}

/// Runs the steps of the `command` install `install` of `spec` in `folder`, after copying the pin files its language folder holds.
fn run_steps(spec: &ServerSpec, install: &Install, folder: &Path) -> Result<(), String> {
    let tool = install
        .tool
        .as_deref()
        .ok_or("the manifest names no program to install it with")?;
    for name in &install.copy {
        let held = match &install.pin {
            Some(prefix) => format!("{prefix}.{name}"),
            None => name.clone(),
        };
        let origin = spec
            .origin
            .as_deref()
            .ok_or("an install that copies files needs the language folder that holds them")?;
        std::fs::copy(origin.join(&held), folder.join(name))
            .map_err(|error| format!("the language has no {held} for its install: {error}"))?;
    }
    let fill = |text: &str| {
        text.replace("{folder}", &folder.to_string_lossy())
            .replace("{version}", &install.version)
    };
    for step in &install.steps {
        let arguments: Vec<String> = step.iter().map(|argument| fill(argument)).collect();
        let arguments: Vec<&str> = arguments.iter().map(String::as_str).collect();
        run(
            tool,
            install.needs.as_deref().unwrap_or(tool),
            &arguments,
            |command| {
                let env = pm_task::safe_env(install.env.clone());
                for (name, value) in &env {
                    command.env(name, fill(value));
                }
                for name in &install.env_remove {
                    command.env_remove(name);
                }
                command.current_dir(folder);
            },
        )?;
    }
    Ok(())
}

/// The program of `spec` in a version installed before, the newest by name first, for when the wanted version cannot be installed.
fn earlier_install(spec: &ServerSpec, install: &Install) -> Option<PathBuf> {
    let root = server_root(&spec.id).ok()?;
    let mut versions: Vec<PathBuf> = std::fs::read_dir(root)
        .ok()?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.is_dir()
                && !path
                    .file_name()
                    .is_some_and(|name| name.to_string_lossy().ends_with(".partial"))
        })
        .collect();
    versions.sort();
    versions
        .iter()
        .rev()
        .find_map(|folder| installed_program(spec, install, folder))
}

/// The program of `spec` found in the project at `root` or on the machine, without installing anything.
pub(crate) fn found(spec: &ServerSpec, root: Option<&Path>) -> Option<PathBuf> {
    let in_project = root.and_then(|root| {
        spec.project
            .iter()
            .find_map(|folder| in_folder(&root.join(folder), &spec.command))
    });
    in_project.or_else(|| {
        pm_tools::installed(&spec.command)
            .filter(|program| spec.check.is_empty() || works(program, &spec.check))
    })
}

/// The program of `spec` installed in the data folder, installing it first when it is missing and using one installed before when that fails.
pub(crate) fn managed(spec: &ServerSpec) -> Result<PathBuf, String> {
    let install = spec
        .install
        .as_ref()
        .ok_or_else(|| format!("{} is not on this machine", spec.command))?;
    let server = server_root(&spec.id)?;
    let folder = server.join(&install.version);
    if let Some(program) = installed_program(spec, install, &folder) {
        return Ok(program);
    }
    let staging = server.join(format!("{}.partial", install.version));
    let _ = std::fs::remove_dir_all(&staging);
    let outcome = install_into(spec, install, &staging).and_then(|()| {
        let _ = std::fs::remove_dir_all(&folder);
        std::fs::rename(&staging, &folder).map_err(|error| error.to_string())
    });
    match outcome {
        Ok(()) => installed_program(spec, install, &folder)
            .ok_or_else(|| format!("{} was installed but cannot be found", spec.command)),
        Err(reason) => {
            let _ = std::fs::remove_dir_all(&staging);
            earlier_install(spec, install).ok_or(reason)
        }
    }
}

/// The program of `spec`: the project's own when the project at `root` has one, the machine's when it works, or the one installed in the data folder.
pub(crate) fn ensure_server(spec: &ServerSpec, root: Option<&Path>) -> Result<PathBuf, String> {
    match found(spec, root) {
        Some(program) => Ok(program),
        None => managed(spec),
    }
}

/// What the machine has of the language server of `spec`, without installing anything: where it is or which version is installed, or nothing.
fn server_found(spec: &ServerSpec) -> Option<String> {
    if let Some(program) = pm_tools::installed(&spec.command) {
        return Some(format!("Uses {}", program.display()));
    }
    let install = spec.install.as_ref()?;
    let folder = server_root(&spec.id).ok()?.join(&install.version);
    installed_program(spec, install, &folder)
        .map(|_| format!("{} {}", spec.command, install.version))
}

/// What the machine has of the language server of `spec`, in words for the reader, without installing anything.
pub fn server_state(spec: &ServerSpec) -> String {
    if spec.provided {
        return "Provided by its extension".to_owned();
    }
    server_found(spec).unwrap_or_else(|| format!("{} is not installed", spec.command))
}

/// Whether the machine has the language server of `spec`.
pub fn server_ready(spec: &ServerSpec) -> bool {
    spec.provided || server_found(spec).is_some()
}
