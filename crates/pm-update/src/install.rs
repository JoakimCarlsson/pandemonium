//! Replacing the running install with a staged update, keeping the old one until the new one has started.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// How long a relaunched program must keep running to count as started.
const STARTING: Duration = Duration::from_millis(1500);

/// What the end of the name of the folder or file kept aside is.
const PREVIOUS: &str = "previous";

/// An install that was replaced, and the old one kept beside it.
#[derive(Clone, Debug)]
pub struct Installed {
    /// What was replaced: the app bundle, or the program.
    target: PathBuf,
    /// Where the old install was moved to.
    previous: PathBuf,
    /// The program to start.
    launch: PathBuf,
}

/// What is replaced when this program is updated, and the program inside it that is started: the app bundle it runs from, or its own file.
fn running() -> Result<(PathBuf, PathBuf), String> {
    let program = std::env::current_exe().map_err(|error| error.to_string())?;
    let bundle = program.ancestors().skip(1).find(|folder| {
        cfg!(target_os = "macos")
            && folder
                .extension()
                .is_some_and(|extension| extension == "app")
    });
    Ok(match bundle {
        Some(bundle) => (bundle.to_owned(), program),
        None => (program.clone(), program),
    })
}

/// Where the old install of `target` is kept while the new one is tried.
fn previous_of(target: &Path) -> PathBuf {
    let mut name = target.file_name().unwrap_or_default().to_owned();
    name.push(format!(".{PREVIOUS}"));
    target.with_file_name(name)
}

/// Why the running install cannot be replaced, when it cannot: the folder holding it is not one this user may write to, which is so for a system package or a read-only volume.
pub fn blocker() -> Option<String> {
    let (target, _) = match running() {
        Ok(running) => running,
        Err(reason) => {
            return Some(format!(
                "Could not find where this app is installed: {reason}."
            ));
        }
    };
    let folder = target.parent()?;
    let probe = folder.join(".pandemonium-write-check");
    match std::fs::File::create(&probe) {
        Ok(_) => {
            let _ = std::fs::remove_file(probe);
            None
        }
        Err(_) => Some(format!(
            "This app is installed in {}, which this user cannot write to.",
            folder.display()
        )),
    }
}

/// Copies the file or folder `from` to `to`, folders with everything in them and links as links.
fn copy(from: &Path, to: &Path) -> std::io::Result<()> {
    let kind = std::fs::symlink_metadata(from)?.file_type();
    if kind.is_symlink() {
        #[cfg(unix)]
        return std::os::unix::fs::symlink(std::fs::read_link(from)?, to);
        #[cfg(not(unix))]
        return std::fs::copy(from, to).map(|_| ());
    }
    if kind.is_dir() {
        std::fs::create_dir_all(to)?;
        for entry in std::fs::read_dir(from)? {
            let entry = entry?;
            copy(&entry.path(), &to.join(entry.file_name()))?;
        }
        return Ok(());
    }
    std::fs::copy(from, to).map(|_| ())
}

/// Removes the file or folder at `path`, if there is one.
fn remove(path: &Path) {
    if path.is_dir() {
        let _ = std::fs::remove_dir_all(path);
    } else {
        let _ = std::fs::remove_file(path);
    }
}

/// Whether the program at `launch` starts and says its version.
fn runs(launch: &Path) -> Result<(), String> {
    let output = Command::new(launch)
        .arg("--version")
        .stdin(Stdio::null())
        .output()
        .map_err(|error| format!("the new version does not start: {error}"))?;
    if output.status.success() {
        Ok(())
    } else {
        Err(format!("the new version does not start: {}", output.status))
    }
}

/// Puts the `staged` update where the running install is, the old install kept beside it. The install is left as it was when anything fails, the new version being tried first.
pub fn install(staged: &Path) -> Result<Installed, String> {
    let (target, program) = running()?;
    let relative = program.strip_prefix(&target).unwrap_or(Path::new(""));
    let launch = if relative.as_os_str().is_empty() {
        target.clone()
    } else {
        target.join(relative)
    };
    let trial = if staged.is_dir() {
        staged.join(relative)
    } else {
        staged.to_owned()
    };
    runs(&trial)?;
    let previous = previous_of(&target);
    let incoming = target.with_file_name(format!(
        "{}.new",
        target.file_name().unwrap_or_default().to_string_lossy()
    ));
    remove(&previous);
    remove(&incoming);
    if let Err(error) = copy(staged, &incoming) {
        remove(&incoming);
        return Err(format!("could not copy the new version: {error}"));
    }
    if let Err(error) = std::fs::rename(&target, &previous) {
        remove(&incoming);
        return Err(format!("could not move the current version aside: {error}"));
    }
    if let Err(error) = std::fs::rename(&incoming, &target) {
        let restored = std::fs::rename(&previous, &target);
        remove(&incoming);
        return Err(match restored {
            Ok(()) => format!("could not put the new version in place: {error}"),
            Err(back) => format!(
                "could not put the new version in place: {error}; the old one is at {}: {back}",
                previous.display()
            ),
        });
    }
    Ok(Installed {
        target,
        previous,
        launch,
    })
}

/// Puts the old install back in place of the one `installed` put there.
fn restore(installed: &Installed) -> Result<(), String> {
    remove(&installed.target);
    std::fs::rename(&installed.previous, &installed.target).map_err(|error| {
        format!(
            "could not put the old version back from {}: {error}",
            installed.previous.display()
        )
    })
}

/// Starts the new version, apart from this process so that it outlives it, and waits until it has run for a moment; the old version is put back if it does not start. The server it finds running is kept.
pub fn relaunch(installed: &Installed) -> Result<(), String> {
    let mut command = Command::new(&installed.launch);
    command
        .env(crate::KEEP_SERVER, "1")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    #[cfg(unix)]
    std::os::unix::process::CommandExt::process_group(&mut command, 0);
    let mut child = command.spawn().map_err(|error| {
        let restored = restore(installed).err().unwrap_or_default();
        format!("could not start the new version: {error} {restored}")
    })?;
    let started = Instant::now();
    while started.elapsed() < STARTING {
        if let Ok(Some(status)) = child.try_wait() {
            let restored = restore(installed).err().unwrap_or_default();
            return Err(format!(
                "the new version stopped at once: {status} {restored}"
            ));
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    remove(&installed.previous);
    Ok(())
}

/// Removes the old install an update left beside this program, which a running program could not be deleted from under on Windows.
pub fn forget_previous() {
    if let Ok((target, _)) = running() {
        remove(&previous_of(&target));
    }
}
