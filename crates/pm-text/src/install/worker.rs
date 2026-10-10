//! Installing recipes into a partial directory and publishing completed servers.

use pm_host::Command;
use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use super::{Build, Recipe};
use crate::program;

/// The platform name used by release recipes.
pub fn platform() -> String {
    format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH)
}

/// Installs `command` from `recipe` under `servers`, then returns its executable.
///
/// The partial directory is removed on every failure and becomes the version
/// directory only after the executable and checksum have been verified.
pub fn install(servers: &Path, command: &str, recipe: Recipe) -> Result<PathBuf, String> {
    if command.is_empty()
        || command.contains(['/', '\\'])
        || [".", ".."].contains(&command)
        || recipe.version().contains(['/', '\\'])
        || [".", ".."].contains(&recipe.version())
    {
        return Err("Managed server names and versions cannot contain paths.".into());
    }
    let parent = servers.join(command);
    let directory_name = recipe.directory_name();
    let version = parent.join(&directory_name);
    if let Some(executable) = program::managed_in(&version) {
        return Ok(executable);
    }
    fs::create_dir_all(&parent).map_err(|error| error.to_string())?;
    let partial = parent.join(format!("{directory_name}.partial"));
    if partial.exists() {
        fs::remove_dir_all(&partial).map_err(|error| error.to_string())?;
    }
    fs::create_dir(&partial).map_err(|error| error.to_string())?;
    let installed = install_into(&partial, command, recipe);
    let result = installed.and_then(|executable| {
        let relative = executable
            .strip_prefix(&partial)
            .map_err(|error| error.to_string())?;
        fs::write(
            partial.join(".program"),
            relative.to_string_lossy().as_bytes(),
        )
        .map_err(|error| error.to_string())?;
        if version.exists() {
            fs::remove_dir_all(&version).map_err(|error| error.to_string())?;
        }
        fs::rename(&partial, &version).map_err(|error| error.to_string())?;
        program::managed_in(&version)
            .ok_or_else(|| format!("{command} was not found after installation"))
    });
    if result.is_err() {
        let _ = fs::remove_dir_all(&partial);
        if fs::read_dir(&parent).is_ok_and(|mut entries| entries.next().is_none()) {
            let _ = fs::remove_dir(&parent);
        }
    }
    result
}

/// Removes completed versions older than the one that has started successfully.
pub fn prune_older(servers: &Path, command: &str, current: &str) {
    let Ok(entries) = fs::read_dir(servers.join(command)) else {
        return;
    };
    for entry in entries.flatten() {
        if entry.file_name() != current
            && entry.path().is_dir()
            && entry.path().join(".program").is_file()
        {
            let _ = fs::remove_dir_all(entry.path());
        }
    }
}

/// Runs one recipe inside its temporary destination.
fn install_into(directory: &Path, command: &str, recipe: Recipe) -> Result<PathBuf, String> {
    match recipe {
        Recipe::Release { version, builds } => install_release(directory, command, version, builds),
        Recipe::Npm {
            package,
            version,
            extra,
        } => {
            let npm = tool("npm", command)?;
            let mut process = pm_host::Host::local().command(npm);
            process
                .args(["install", "--prefix"])
                .arg(directory)
                .args(["--no-save", &format!("{package}@{version}")])
                .args(extra);
            run(&mut process)?;
            executable(directory.join("node_modules/.bin"), command)
        }
        Recipe::Go { module, version } => {
            let go = tool("go", command)?;
            run(pm_host::Host::local()
                .command(go)
                .args(["install", &format!("{module}@{version}")])
                .env("GOBIN", directory))?;
            executable(directory.to_path_buf(), command)
        }
        Recipe::Pip { package, version } => {
            let python = program::installed("python3").or_else(|| program::installed("python")).ok_or_else(|| format!("Installing {command} needs Python, which was not found. Install Python, or install the server yourself."))?;
            let venv = directory.join("venv");
            run(pm_host::Host::local()
                .command(python)
                .args(["-m", "venv"])
                .arg(&venv))?;
            let bin = venv.join(if cfg!(windows) { "Scripts" } else { "bin" });
            let pip = executable(bin.clone(), "pip")?;
            run(pm_host::Host::local()
                .command(pip)
                .args(["install", &format!("{package}=={version}")]))?;
            executable(bin, command)?;
            let wrapper = directory.join(if cfg!(windows) {
                format!("{command}.cmd")
            } else {
                command.to_owned()
            });
            if cfg!(windows) {
                fs::write(
                    &wrapper,
                    format!("@echo off\r\n\"%~dp0venv\\Scripts\\{command}.exe\" %*\r\n"),
                )
                .map_err(|error| error.to_string())?;
            } else {
                fs::write(&wrapper, format!("#!/bin/sh\nhere=${{0%/*}}\nexec \"$here/venv/bin/python\" \"$here/venv/bin/{command}\" \"$@\"\n")).map_err(|error| error.to_string())?;
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    let mut permissions = fs::metadata(&wrapper)
                        .map_err(|error| error.to_string())?
                        .permissions();
                    permissions.set_mode(0o755);
                    fs::set_permissions(&wrapper, permissions)
                        .map_err(|error| error.to_string())?;
                }
            }
            Ok(wrapper)
        }
    }
}

/// Downloads and verifies a release asset before unpacking it.
fn install_release(
    directory: &Path,
    command: &str,
    version: &str,
    builds: &[Build],
) -> Result<PathBuf, String> {
    let platform = platform();
    let build = builds
        .iter()
        .find(|build| build.platform == platform)
        .ok_or_else(|| format!("{command} has no release for {platform}."))?;
    let url = build.url.replace("{version}", version);
    let bytes = super::download_checked(&url, build.sha256)?;
    if url.ends_with(".tar.gz") {
        let mut archive = tar::Archive::new(flate2::read::GzDecoder::new(bytes.as_slice()));
        archive
            .unpack(directory)
            .map_err(|error| error.to_string())?;
    } else if url.ends_with(".zip") {
        super::unpack_zip(&bytes, directory)?;
    } else {
        let target = directory.join(if cfg!(windows) {
            format!("{command}.exe")
        } else {
            command.to_owned()
        });
        let mut file = File::create(&target).map_err(|error| error.to_string())?;
        if url.ends_with(".gz") {
            io::copy(
                &mut flate2::read::GzDecoder::new(bytes.as_slice()),
                &mut file,
            )
            .map_err(|error| error.to_string())?;
        } else {
            file.write_all(&bytes).map_err(|error| error.to_string())?;
        }
    }
    let found = find_executable(directory, command)?
        .ok_or_else(|| format!("{command} was not found in its release archive"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = fs::metadata(&found)
            .map_err(|error| error.to_string())?
            .permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&found, permissions).map_err(|error| error.to_string())?;
    }
    Ok(found)
}

/// Finds `command` within a release archive's directory tree.
fn find_executable(directory: &Path, command: &str) -> Result<Option<PathBuf>, String> {
    let entries = fs::read_dir(directory).map_err(|error| error.to_string())?;
    for entry in entries {
        let path = entry.map_err(|error| error.to_string())?.path();
        if path.is_dir() {
            if let Some(found) = find_executable(&path, command)? {
                return Ok(Some(found));
            }
        } else if path.file_name().is_some_and(|name| {
            name == command || name.to_string_lossy() == format!("{command}.exe")
        }) {
            return Ok(Some(path));
        }
    }
    Ok(None)
}

/// Finds a required installation tool or explains its absence.
fn tool(name: &str, command: &str) -> Result<PathBuf, String> {
    program::installed(name).ok_or_else(|| format!("Installing {command} needs {name}, which was not found. Install {}, or install the server yourself.", match name {
        "npm" => "Node.js",
        "go" => "Go",
        name => name,
    }))
}

/// Runs an installer and includes its own stderr in a failure.
fn run(process: &mut Command) -> Result<(), String> {
    let output = process.output().map_err(|error| error.to_string())?;
    if output.status.success() {
        Ok(())
    } else {
        let error = String::from_utf8_lossy(&output.stderr).trim().to_owned();
        Err(if error.is_empty() {
            format!("installer exited with {}", output.status)
        } else {
            error
        })
    }
}

/// Finds a generated executable in an installer's output directory.
fn executable(directory: PathBuf, command: &str) -> Result<PathBuf, String> {
    let path = if cfg!(windows) {
        [format!("{command}.cmd"), format!("{command}.exe")]
            .into_iter()
            .map(|name| directory.join(name))
            .find(|path| path.is_file())
            .unwrap_or_else(|| directory.join(command))
    } else {
        directory.join(command)
    };
    path.is_file()
        .then_some(path)
        .ok_or_else(|| format!("{command} was not installed in {}", directory.display()))
}
