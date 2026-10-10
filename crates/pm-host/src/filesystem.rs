//! Filesystem operations on a named machine.

use crate::Host;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::io;
use std::path::{Path, PathBuf};

/// Portable file information returned by a machine.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Metadata {
    /// The byte length.
    length: u64,
    /// The last modification time, when supported by the filesystem.
    modified: Option<std::time::SystemTime>,
    /// The entry kind.
    kind: FileType,
    /// Whether an executable bit is set.
    pub executable: bool,
}
impl Metadata {
    /// The length in bytes.
    pub fn len(&self) -> u64 {
        self.length
    }
    /// The last modification time reported by the owning filesystem.
    pub fn modified(&self) -> Option<std::time::SystemTime> {
        self.modified
    }
    /// Whether the entry is empty.
    pub fn is_empty(&self) -> bool {
        self.length == 0
    }
    /// Whether this is a directory.
    pub fn is_dir(&self) -> bool {
        self.kind.directory
    }
    /// Whether this is a regular file.
    pub fn is_file(&self) -> bool {
        self.kind.file
    }
    /// The entry kind.
    pub fn file_type(&self) -> FileType {
        self.kind
    }
    /// Converts platform metadata without exposing platform handles.
    fn local(metadata: std::fs::Metadata) -> Self {
        #[cfg(unix)]
        let executable = {
            use std::os::unix::fs::PermissionsExt;
            metadata.permissions().mode() & 0o111 != 0
        };
        #[cfg(not(unix))]
        let executable = false;
        Self {
            length: metadata.len(),
            modified: metadata.modified().ok(),
            kind: FileType {
                directory: metadata.is_dir(),
                file: metadata.is_file(),
                symlink: metadata.file_type().is_symlink(),
            },
            executable,
        }
    }
}

/// An entry's portable kind, without following symlinks.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct FileType {
    /// Whether this entry is a directory.
    directory: bool,
    /// Whether this entry is a regular file.
    file: bool,
    /// Whether this entry is a symlink.
    symlink: bool,
}
impl FileType {
    /// Whether this is a directory.
    pub fn is_dir(&self) -> bool {
        self.directory
    }
    /// Whether this is a regular file.
    pub fn is_file(&self) -> bool {
        self.file
    }
    /// Whether this is a symlink.
    pub fn is_symlink(&self) -> bool {
        self.symlink
    }
}

/// An owned directory entry that can cross the wire.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DirEntry {
    /// The full entry path.
    path: PathBuf,
    /// The entry kind.
    kind: FileType,
}
impl DirEntry {
    /// The full entry path.
    pub fn path(&self) -> PathBuf {
        self.path.clone()
    }
    /// The last component of the path.
    pub fn file_name(&self) -> std::ffi::OsString {
        self.path.file_name().unwrap_or_default().to_owned()
    }
    /// The entry kind.
    pub fn file_type(&self) -> io::Result<FileType> {
        Ok(self.kind)
    }
}

/// Filesystem access through one machine handle.
#[derive(Clone)]
pub struct FileSystem(pub(crate) Host);

impl FileSystem {
    /// Reads a file as raw bytes, with one remote round trip.
    pub fn read(&self, path: impl AsRef<Path>) -> io::Result<Vec<u8>> {
        match &self.0.0 {
            None => std::fs::read(path),
            Some(remote) => remote.read(path.as_ref()),
        }
    }
    /// Reads a UTF-8 file.
    pub fn read_to_string(&self, path: impl AsRef<Path>) -> io::Result<String> {
        String::from_utf8(self.read(path)?)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
    }
    /// Replaces a file's contents, with one remote round trip.
    pub fn write(&self, path: impl AsRef<Path>, bytes: impl AsRef<[u8]>) -> io::Result<()> {
        match &self.0.0 {
            None => std::fs::write(path, bytes),
            Some(remote) => remote.write(path.as_ref(), bytes.as_ref()),
        }
    }
    /// Creates a file exclusively, refusing an existing name.
    pub fn create_file(&self, path: impl AsRef<Path>) -> io::Result<()> {
        match &self.0.0 {
            None => std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(path)
                .map(drop),
            Some(_) => self.call("create_file", path.as_ref(), None).map(drop),
        }
    }
    /// Reads metadata, following links.
    pub fn metadata(&self, path: impl AsRef<Path>) -> io::Result<Metadata> {
        match &self.0.0 {
            None => std::fs::metadata(path).map(Metadata::local),
            Some(_) => serde_json::from_value(self.call("metadata", path.as_ref(), None)?)
                .map_err(io::Error::other),
        }
    }
    /// Reads metadata without following links.
    pub fn symlink_metadata(&self, path: impl AsRef<Path>) -> io::Result<Metadata> {
        match &self.0.0 {
            None => std::fs::symlink_metadata(path).map(Metadata::local),
            Some(_) => {
                serde_json::from_value(self.call("symlink_metadata", path.as_ref(), None)?)
                    .map_err(io::Error::other)
            }
        }
    }
    /// Lists one directory, returning owned entries.
    pub fn read_dir(
        &self,
        path: impl AsRef<Path>,
    ) -> io::Result<std::vec::IntoIter<io::Result<DirEntry>>> {
        let entries: Vec<DirEntry> = match &self.0.0 {
            None => std::fs::read_dir(path)?
                .map(|entry| {
                    let entry = entry?;
                    Ok(DirEntry {
                        path: entry.path(),
                        kind: Metadata::local(entry.metadata()?)
                            .kind
                            .with_link(entry.file_type()?.is_symlink()),
                    })
                })
                .collect::<io::Result<_>>()?,
            Some(_) => serde_json::from_value(self.call("read_dir", path.as_ref(), None)?)
                .map_err(io::Error::other)?,
        };
        Ok(entries.into_iter().map(Ok).collect::<Vec<_>>().into_iter())
    }
    /// Resolves a path on its own machine.
    pub fn canonicalize(&self, path: impl AsRef<Path>) -> io::Result<PathBuf> {
        match &self.0.0 {
            None => std::fs::canonicalize(path),
            Some(_) => serde_json::from_value(self.call("canonicalize", path.as_ref(), None)?)
                .map_err(io::Error::other),
        }
    }
    /// Creates a directory and every missing parent.
    pub fn create_dir_all(&self, path: impl AsRef<Path>) -> io::Result<()> {
        self.mutate("create_dir_all", path.as_ref(), None)
    }
    /// Creates one directory exclusively.
    pub fn create_dir(&self, path: impl AsRef<Path>) -> io::Result<()> {
        self.mutate("create_dir", path.as_ref(), None)
    }
    /// Renames a file or directory.
    pub fn rename(&self, from: impl AsRef<Path>, to: impl AsRef<Path>) -> io::Result<()> {
        self.mutate("rename", from.as_ref(), Some(to.as_ref()))
    }
    /// Removes an entry, recursively for directories and without following links.
    pub fn remove(&self, path: impl AsRef<Path>) -> io::Result<()> {
        self.mutate("remove", path.as_ref(), None)
    }
    /// Removes a file.
    pub fn remove_file(&self, path: impl AsRef<Path>) -> io::Result<()> {
        self.mutate("remove_file", path.as_ref(), None)
    }
    /// Removes a directory recursively.
    pub fn remove_dir_all(&self, path: impl AsRef<Path>) -> io::Result<()> {
        self.mutate("remove_dir_all", path.as_ref(), None)
    }
    /// Moves a local file to the desktop trash; remote trash is unavailable.
    pub fn trash(&self, path: impl AsRef<Path>) -> io::Result<()> {
        if !self.0.is_local() {
            return Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "This remote host has no trash; confirm permanent deletion",
            ));
        }
        trash::delete(path.as_ref()).map_err(io::Error::other)
    }
    /// Creates a symbolic link.
    pub fn symlink(&self, target: impl AsRef<Path>, to: impl AsRef<Path>) -> io::Result<()> {
        self.mutate("symlink", target.as_ref(), Some(to.as_ref()))
    }
    /// Copies a file and returns its byte length.
    pub fn copy(&self, from: impl AsRef<Path>, to: impl AsRef<Path>) -> io::Result<u64> {
        match &self.0.0 {
            None => std::fs::copy(from, to),
            Some(_) => {
                serde_json::from_value(self.call("copy", from.as_ref(), Some(to.as_ref()))?)
                    .map_err(io::Error::other)
            }
        }
    }
    /// Reads a link's destination.
    pub fn read_link(&self, path: impl AsRef<Path>) -> io::Result<PathBuf> {
        match &self.0.0 {
            None => std::fs::read_link(path),
            Some(_) => serde_json::from_value(self.call("read_link", path.as_ref(), None)?)
                .map_err(io::Error::other),
        }
    }
    /// Reports whether anything exists at a path, including dangling links.
    pub fn exists(&self, path: impl AsRef<Path>) -> bool {
        self.symlink_metadata(path).is_ok()
    }
    /// Reports whether a path is a directory.
    pub fn is_dir(&self, path: impl AsRef<Path>) -> bool {
        self.metadata(path).is_ok_and(|metadata| metadata.is_dir())
    }
    /// Reports whether a path is a regular file.
    pub fn is_file(&self, path: impl AsRef<Path>) -> bool {
        self.metadata(path).is_ok_and(|metadata| metadata.is_file())
    }
    /// Sends one filesystem control operation.
    fn call(&self, op: &str, path: &Path, to: Option<&Path>) -> io::Result<serde_json::Value> {
        self.0
            .0
            .as_ref()
            .unwrap()
            .request(op, json!({"path": path, "to": to}))
    }
    /// Dispatches an operation locally or across the connection.
    fn mutate(&self, op: &str, path: &Path, to: Option<&Path>) -> io::Result<()> {
        if self.0.is_local() {
            mutate(op, path, to)
        } else {
            self.call(op, path, to).map(drop)
        }
    }
}

impl FileType {
    /// Keeps link entries from being mistaken for directories during walks.
    fn with_link(mut self, link: bool) -> Self {
        if link {
            self.directory = false;
            self.file = false;
            self.symlink = true;
        }
        self
    }
}

/// Executes one local filesystem mutation.
pub(crate) fn mutate(op: &str, path: &Path, to: Option<&Path>) -> io::Result<()> {
    match op {
        "create_dir_all" => std::fs::create_dir_all(path),
        "create_dir" => std::fs::create_dir(path),
        "rename" => std::fs::rename(
            path,
            to.ok_or_else(|| io::Error::other("missing destination"))?,
        ),
        "remove_file" => std::fs::remove_file(path),
        "remove_dir_all" => std::fs::remove_dir_all(path),
        "remove" => {
            if std::fs::symlink_metadata(path)?.is_dir() {
                std::fs::remove_dir_all(path)
            } else {
                std::fs::remove_file(path)
            }
        }
        "symlink" => symlink(
            path,
            to.ok_or_else(|| io::Error::other("missing destination"))?,
        ),
        _ => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "unknown filesystem operation",
        )),
    }
}

/// Creates a local link using the platform's native operation.
fn symlink(target: &Path, to: &Path) -> io::Result<()> {
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(target, to)
    }
    #[cfg(windows)]
    {
        if target.is_dir() {
            std::os::windows::fs::symlink_dir(target, to)
        } else {
            std::os::windows::fs::symlink_file(target, to)
        }
    }
}

/// User toolchain directories searched beside the inherited executable path.
const TOOL_DIRECTORIES: &[&str] = &[
    ".cargo/bin",
    ".grok/bin",
    ".local/bin",
    "go/bin",
    ".bun/bin",
    ".deno/bin",
    ".npm-global/bin",
    "AppData/Roaming/npm",
    ".volta/bin",
    ".local/share/fnm/aliases/default/bin",
];
/// Locates a program on the path and in customary tool directories.
pub(crate) fn which(program: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH").unwrap_or_default();
    let home = std::env::home_dir();
    #[cfg(windows)]
    let names = std::env::var("PATHEXT")
        .unwrap_or_else(|_| ".COM;.EXE;.BAT;.CMD".into())
        .split(';')
        .map(|extension| format!("{program}{extension}"))
        .collect::<Vec<_>>();
    #[cfg(not(windows))]
    let names = [program.to_owned()];
    std::env::split_paths(&path)
        .chain(
            TOOL_DIRECTORIES
                .iter()
                .filter_map(|directory| Some(home.as_ref()?.join(directory))),
        )
        .chain([
            PathBuf::from("/usr/local/bin"),
            PathBuf::from("/opt/homebrew/bin"),
        ])
        .flat_map(|directory| names.iter().map(move |name| directory.join(name)))
        .find(|path| {
            path.metadata().is_ok_and(|metadata| {
                let metadata = Metadata::local(metadata);
                metadata.is_file() && metadata.executable_or_windows()
            })
        })
}

/// Finds repository roots for the shared ignore rules, without domain types.
pub(crate) fn repositories(root: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let mut pending = vec![(root.to_path_buf(), 2)];
    while let Some((path, depth)) = pending.pop() {
        if path.join(".git").exists() {
            found.push(path.clone());
        }
        if depth == 0 {
            continue;
        }
        if let Ok(entries) = std::fs::read_dir(path) {
            for entry in entries.flatten() {
                let name = entry.file_name();
                let name = name.to_string_lossy();
                if entry.file_type().is_ok_and(|kind| kind.is_dir())
                    && !name.starts_with('.')
                    && !["node_modules", "target", "vendor", "venv", "dist", "build"]
                        .contains(&name.as_ref())
                {
                    pending.push((entry.path(), depth - 1));
                }
            }
        }
    }
    found
}

/// Replaces a complete remote upload atomically while retaining file permissions.
pub(crate) fn replace(
    path: &Path,
    receive: impl FnOnce(&mut std::fs::File) -> io::Result<()>,
) -> io::Result<()> {
    let path = if path.exists() {
        std::fs::canonicalize(path)?
    } else {
        path.to_path_buf()
    };
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::other("The file has no parent directory"))?;
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(io::Error::other)?
        .as_nanos();
    let staging = parent.join(format!(
        ".pandemonium-save-{}-{nonce:x}",
        std::process::id()
    ));
    let result = (|| {
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&staging)?;
        if let Ok(metadata) = std::fs::metadata(&path) {
            file.set_permissions(metadata.permissions())?;
        }
        receive(&mut file)?;
        file.sync_all()?;
        drop(file);
        std::fs::rename(&staging, &path)
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(staging);
    }
    result
}

impl Metadata {
    /// Whether a regular file can be executed on this endpoint platform.
    fn executable_or_windows(&self) -> bool {
        cfg!(windows) || self.executable
    }
}

/// The endpoint's executable path with installed toolchains available to child programs.
pub(crate) fn executable_path(program: &str) -> std::ffi::OsString {
    let inherited = std::env::var_os("PATH").unwrap_or_default();
    let beside = Path::new(program)
        .parent()
        .filter(|path| !path.as_os_str().is_empty());
    let directories = beside
        .map(Path::to_path_buf)
        .into_iter()
        .chain(std::env::split_paths(&inherited))
        .chain(
            TOOL_DIRECTORIES
                .iter()
                .filter_map(|directory| Some(std::env::home_dir()?.join(directory))),
        )
        .chain([
            PathBuf::from("/usr/local/bin"),
            PathBuf::from("/opt/homebrew/bin"),
        ]);
    std::env::join_paths(directories).unwrap_or(inherited)
}
