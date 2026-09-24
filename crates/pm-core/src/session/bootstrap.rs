//! What a fresh worktree is given before an agent is turned loose in it.
//!
//! A git worktree holds tracked files and nothing else, so everything a
//! project needs to actually run is missing from a session the moment it is
//! cut: the dependency trees nobody commits — `node_modules`, `.venv`,
//! `vendor` — and the local configuration that is deliberately ignored, like
//! `.env`. A session that cannot install, build or serve is a session the
//! agent spends its first turn repairing.
//!
//! So the paths are declared once and brought across at cut time: the large
//! regenerable trees as symlinks, because copying gigabytes per session is
//! not worth it, and the small editable files as copies, because a session
//! that edits `.env` must not be editing the project's. A path the repository
//! does not have is skipped, and one the worktree already tracks is left
//! alone — bringing across is for what git left out, never for overwriting
//! what it put there.
//!
//! A session is also given a port of its own, so two sessions serving the
//! same project do not fight over one.

use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::{fs, io};

/// How many times a port is asked for before the session goes without one.
const ATTEMPTS: usize = 8;

/// What a fresh worktree is given, and under what name its port arrives.
///
/// The declaration is the reader's, kept with the rest of the preferences:
/// which paths a project needs is a property of the project, not of the
/// session, and every session of it is cut the same way.
#[derive(Clone, Debug, PartialEq)]
pub struct Bootstrap {
    /// Paths symlinked into a fresh worktree, relative to the repository.
    pub link: Vec<PathBuf>,
    /// Paths copied into it, relative to the repository.
    pub copy: Vec<PathBuf>,
    /// The variable a session's own port is handed to a program in.
    pub port: Option<String>,
}

impl Default for Bootstrap {
    /// What a worktree is given when nothing has been declared.
    ///
    /// The defaults are the trees every ecosystem regenerates and the files
    /// every project keeps out of git. A build directory of a compiler that
    /// keys its cache to the source path — `target`, for one — is left out on
    /// purpose: sharing it between worktrees makes both of them rebuild.
    fn default() -> Self {
        Self {
            link: ["node_modules", ".venv", "venv", "vendor"]
                .map(PathBuf::from)
                .to_vec(),
            copy: [".env", ".env.local", ".npmrc"].map(PathBuf::from).to_vec(),
            port: Some("PORT".to_owned()),
        }
    }
}

impl Bootstrap {
    /// The environment a program started in a session with `port` is given.
    ///
    /// This is the one place the port becomes a variable, so a shell and an
    /// agent in the same worktree are handed the same thing under the same
    /// name.
    pub fn env(&self, port: Option<u16>) -> Vec<(String, String)> {
        match (self.port.as_deref(), port) {
            (Some(name), Some(port)) if !name.is_empty() => {
                vec![(name.to_owned(), port.to_string())]
            }
            _ => Vec::new(),
        }
    }
}

/// Brings what `wanted` declares from the repository at `origin` into `root`.
///
/// What could not be brought across comes back as one line apiece. A path the
/// repository does not have is not one of them: a declaration covers every
/// project the reader opens, and most projects have most of it missing.
pub fn apply(origin: &Path, root: &Path, wanted: &Bootstrap) -> Vec<String> {
    let linked = wanted
        .link
        .iter()
        .map(|path| (path, link(&origin.join(path), &root.join(path))));
    let copied = wanted
        .copy
        .iter()
        .map(|path| (path, copy(&origin.join(path), &root.join(path))));

    linked
        .chain(copied)
        .filter_map(|(path, brought)| {
            let trouble = brought.err()?;
            Some(format!(
                "{} could not be brought across: {trouble}",
                path.display()
            ))
        })
        .collect()
}

/// Copies what lies in the folder at `origin` outside every one of
/// `repositories` into the session folder at `root`.
///
/// `repositories` is every repository the folder holds, cut or not: one that
/// was left out of the session is left out, never copied in as loose files.
///
/// A folder of several repositories is more than the repositories: the
/// `Makefile` that builds them together, the compose file that runs them.
/// None of it is any repository's to cut, so it is copied the way `.env` is,
/// and a session that edits it edits its own. What `wanted` links is left to
/// be linked, and a directory holding a repository is walked into rather
/// than copied, since the worktree cut there already stands in for part of
/// it.
pub fn loose(
    origin: &Path,
    root: &Path,
    repositories: &[PathBuf],
    wanted: &Bootstrap,
) -> Vec<String> {
    let mut trouble = Vec::new();
    let Ok(entries) = fs::read_dir(origin) else {
        return trouble;
    };
    for entry in entries.flatten() {
        let source = entry.path();
        let destination = root.join(entry.file_name());
        let linked = wanted.link.iter().any(|path| origin.join(path) == source);
        if linked || repositories.contains(&source) {
            continue;
        }
        let holding = repositories
            .iter()
            .any(|repository| repository.starts_with(&source));
        let brought = match holding {
            true => fs::create_dir_all(&destination).map(|()| {
                trouble.extend(loose(&source, &destination, repositories, wanted));
            }),
            false => copy(&source, &destination),
        };
        if let Err(error) = brought {
            trouble.push(format!(
                "{} could not be brought across: {error}",
                source.display()
            ));
        }
    }
    trouble
}

/// Symlinks `source` into the worktree at `destination`.
///
/// The link is absolute, so it resolves however the worktree is reached.
fn link(source: &Path, destination: &Path) -> io::Result<()> {
    if !skippable(source, destination)? {
        return Ok(());
    }
    symlink(source, destination)
}

/// Copies `source` into the worktree at `destination`, directories and all.
fn copy(source: &Path, destination: &Path) -> io::Result<()> {
    if !skippable(source, destination)? {
        return Ok(());
    }
    let kind = fs::symlink_metadata(source)?.file_type();
    match () {
        () if kind.is_symlink() => symlink(&fs::read_link(source)?, destination),
        () if kind.is_dir() => copy_tree(source, destination),
        () => fs::copy(source, destination).map(|_| ()),
    }
}

/// Whether there is anything to bring across, having made room for it.
///
/// There is nothing to do when the repository has not got the path or the
/// worktree already has it, and either is the ordinary case rather than a
/// failure. The parent is made either way, because a path may name a file
/// inside a directory git left out.
fn skippable(source: &Path, destination: &Path) -> io::Result<bool> {
    if fs::symlink_metadata(source).is_err() || fs::symlink_metadata(destination).is_ok() {
        return Ok(false);
    }
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent)?;
    }
    Ok(true)
}

/// Copies the directory at `source` to `destination`, recursively.
fn copy_tree(source: &Path, destination: &Path) -> io::Result<()> {
    fs::create_dir_all(destination)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let destination = destination.join(entry.file_name());
        match entry.file_type()?.is_dir() {
            true => copy_tree(&entry.path(), &destination)?,
            false => {
                fs::copy(entry.path(), destination)?;
            }
        }
    }
    Ok(())
}

/// Symlinks `source` to `destination` the way the platform does it.
#[cfg(unix)]
fn symlink(source: &Path, destination: &Path) -> io::Result<()> {
    std::os::unix::fs::symlink(source, destination)
}

/// Symlinks `source` to `destination` the way the platform does it.
///
/// Windows tells the two apart, so what is being linked decides which call
/// is made; a link of either kind needs the privilege to make one.
#[cfg(windows)]
fn symlink(source: &Path, destination: &Path) -> io::Result<()> {
    match fs::symlink_metadata(source)?.is_dir() {
        true => std::os::windows::fs::symlink_dir(source, destination),
        false => std::os::windows::fs::symlink_file(source, destination),
    }
}

/// A local port nothing is listening on, and that `taken` has not been given.
///
/// The port is asked of the operating system and released again, so nothing
/// holds it between being handed out and the session's own server binding it.
/// Two sessions can be handed the same port only if the first has not bound
/// it yet, which is why the ones already handed out are passed in.
pub fn free_port(taken: &[u16]) -> Option<u16> {
    (0..ATTEMPTS).find_map(|_| {
        let port = TcpListener::bind(("127.0.0.1", 0))
            .ok()?
            .local_addr()
            .ok()?
            .port();
        (!taken.contains(&port)).then_some(port)
    })
}
