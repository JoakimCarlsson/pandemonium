//! Reading the files and folders of this machine that are brought into a session's folder, away from the window.

use std::fs;
use std::path::Path;

use pm_components::tree_actions::{EntryChange, joined};

/// The most bytes one file brought in may hold, so that the request carrying it, as base64 inside JSON, stays within the 16 MiB a frame may hold.
const FILE_BYTES: u64 = 10 * 1024 * 1024;

/// The most bytes brought in at once, which are all held in memory until they are sent.
const TOTAL_BYTES: u64 = 256 * 1024 * 1024;

/// The most files and empty folders brought in at once.
const MOST_ENTRIES: usize = 5000;

/// What was read to be brought in: what to ask of the server, and what was left out and why.
#[derive(Debug, Default)]
pub(super) struct Gathered {
    /// The files to write and the empty folders to make, in the order they are asked for.
    pub changes: Vec<EntryChange>,
    /// What was left out, each with the reason, worded for the reader.
    pub skipped: Vec<String>,
    /// How many bytes the files read so far hold together.
    bytes: u64,
}

impl Gathered {
    /// Whether another entry would go over what may be brought in at once; says so once when it would.
    fn full(&mut self, more: u64) -> bool {
        let over = self.changes.len() >= MOST_ENTRIES || self.bytes + more > TOTAL_BYTES;
        let limit = "the rest is more than can be brought in at once";
        if over && !self.skipped.iter().any(|reason| reason == limit) {
            self.skipped.push(limit.to_owned());
        }
        over
    }

    /// Reads the file at `source` to be written at `path` of the session's folder, or leaves it out when it is too large or cannot be read.
    fn file(&mut self, source: &Path, path: String, size: u64) {
        if size > FILE_BYTES {
            self.skipped
                .push(format!("{} is larger than 10 MB", source.display()));
            return;
        }
        if self.full(size) {
            return;
        }
        match fs::read(source) {
            Ok(data) => {
                self.bytes += data.len() as u64;
                self.changes.push(EntryChange::Import { path, data });
            }
            Err(error) => self
                .skipped
                .push(format!("{} could not be read: {error}", source.display())),
        }
    }

    /// Reads everything in the folder at `source` to be written under `path` of the session's folder; an empty folder is made as one, and links are left out.
    fn folder(&mut self, source: &Path, path: String) {
        let mut children: Vec<fs::DirEntry> = match fs::read_dir(source) {
            Ok(children) => children.flatten().collect(),
            Err(error) => {
                self.skipped
                    .push(format!("{} could not be read: {error}", source.display()));
                return;
            }
        };
        children.sort_by_key(fs::DirEntry::file_name);
        if children.is_empty() && !self.full(0) {
            self.changes
                .push(EntryChange::Create { path, folder: true });
            return;
        }
        for child in children {
            let Ok(metadata) = child.metadata() else {
                continue;
            };
            let below = joined(&path, &child.file_name().to_string_lossy());
            if metadata.is_dir() {
                self.folder(&child.path(), below);
            } else if metadata.is_file() {
                self.file(&child.path(), below, metadata.len());
            }
        }
    }
}

/// Reads each file or folder of this machine in `files`, named with where it goes from the root of the session's folder, with everything inside the folders.
pub(super) fn gather(files: Vec<(String, String)>) -> Gathered {
    let mut gathered = Gathered::default();
    for (source, path) in files {
        let source = Path::new(&source);
        match fs::metadata(source) {
            Ok(metadata) if metadata.is_dir() => gathered.folder(source, path),
            Ok(metadata) => gathered.file(source, path, metadata.len()),
            Err(error) => gathered
                .skipped
                .push(format!("{} could not be read: {error}", source.display())),
        }
    }
    gathered
}

/// Checks what is read from a dropped folder.
#[cfg(test)]
mod tests {
    use super::*;

    /// A folder is brought in with its files, the folders above them and its empty folders, and a missing source is reported.
    #[test]
    fn gathers_files_and_empty_folders() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../build")
            .join(format!("gather-{}", std::process::id()));
        fs::create_dir_all(root.join("art/empty")).unwrap();
        fs::write(root.join("art/logo.png"), [1, 2, 3]).unwrap();
        fs::write(root.join("notes.txt"), "hello").unwrap();
        let at = |name: &str| root.join(name).to_string_lossy().into_owned();
        let gathered = gather(vec![
            (at("art"), "assets/art".to_owned()),
            (at("notes.txt"), "assets/notes copy.txt".to_owned()),
            (at("missing"), "assets/missing".to_owned()),
        ]);
        assert_eq!(
            gathered.changes,
            vec![
                EntryChange::Create {
                    path: "assets/art/empty".to_owned(),
                    folder: true,
                },
                EntryChange::Import {
                    path: "assets/art/logo.png".to_owned(),
                    data: vec![1, 2, 3],
                },
                EntryChange::Import {
                    path: "assets/notes copy.txt".to_owned(),
                    data: b"hello".to_vec(),
                },
            ]
        );
        assert_eq!(gathered.skipped.len(), 1);
        fs::remove_dir_all(root).unwrap();
    }
}
