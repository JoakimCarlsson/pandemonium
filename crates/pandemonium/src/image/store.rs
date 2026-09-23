//! The pictures the window has open, whichever pane is showing them.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use pm_core::{ProjectId, Scope};
use pm_gfx::Image;

/// The endings of the files opened as pictures rather than as text.
const PICTURES: &[&str] = &["png", "jpg", "jpeg", "gif", "webp", "bmp", "ico", "svg"];

/// An open picture's identity for as long as it is open.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ImageId(u64);

/// One open picture: where it came from and what it came to.
struct Entry {
    /// The worktree it was opened from.
    scope: Scope,
    /// Where it lives.
    path: PathBuf,
    /// The picture, or why it could not be read.
    picture: Result<Image, String>,
    /// How large the file is on disk.
    bytes: u64,
    /// Whether it is only being looked at, and gives its tab up to the next.
    preview: bool,
}

/// One open picture as its pane draws it.
pub struct Shown {
    /// Where it lives, from the worktree down.
    pub name: String,
    /// The picture, or why it could not be read.
    pub picture: Result<Image, String>,
    /// How large the file is on disk.
    pub bytes: u64,
}

/// Every picture the window has open.
#[derive(Default)]
pub struct Images {
    /// The open pictures, by the id the panes name them with.
    open: BTreeMap<ImageId, Entry>,
    /// The id the next picture opened will be given.
    next: ImageId,
}

impl Images {
    /// Whether `path` is a file the window opens as a picture.
    pub fn is_picture(path: &Path) -> bool {
        path.extension()
            .and_then(|ending| ending.to_str())
            .is_some_and(|ending| PICTURES.contains(&ending.to_ascii_lowercase().as_str()))
    }

    /// Opens `path` in `scope`, or hands back the picture if it is open.
    ///
    /// A picture that cannot be read still opens, saying why, because the
    /// reader asked for that file and a tab that does not appear says less.
    pub fn open(&mut self, scope: Scope, path: &Path, preview: bool) -> ImageId {
        if let Some(id) = self.opened(scope, path) {
            if !preview {
                self.keep(id);
            }
            return id;
        }
        let id = self.next;
        self.next = ImageId(id.0 + 1);
        let (picture, bytes) = read(path);
        self.open.insert(
            id,
            Entry {
                scope,
                path: path.to_path_buf(),
                picture,
                bytes,
                preview,
            },
        );
        id
    }

    /// The picture `path` is open as in `scope`, if it is open at all.
    pub fn opened(&self, scope: Scope, path: &Path) -> Option<ImageId> {
        self.open
            .iter()
            .find(|(_, entry)| entry.scope == scope && entry.path == path)
            .map(|(id, _)| *id)
    }

    /// The picture `id` names as its pane draws it, for a worktree at `root`.
    pub fn shown(&self, id: ImageId, root: &Path) -> Option<Shown> {
        let entry = self.open.get(&id)?;
        Some(Shown {
            name: entry
                .path
                .strip_prefix(root)
                .unwrap_or(&entry.path)
                .display()
                .to_string(),
            picture: entry.picture.clone(),
            bytes: entry.bytes,
        })
    }

    /// The worktree the picture `id` names was opened from.
    pub fn scope_of(&self, id: ImageId) -> Option<Scope> {
        self.open.get(&id).map(|entry| entry.scope)
    }

    /// Where the picture `id` names lives.
    pub fn path_of(&self, id: ImageId) -> Option<&Path> {
        self.open.get(&id).map(|entry| entry.path.as_path())
    }

    /// What a tab showing the picture `id` names calls it.
    pub fn name(&self, id: ImageId) -> Option<String> {
        let path = self.path_of(id)?;
        Some(path.file_name()?.to_string_lossy().into_owned())
    }

    /// Whether the picture `id` names is only being looked at.
    pub fn is_preview(&self, id: ImageId) -> bool {
        self.open.get(&id).is_some_and(|entry| entry.preview)
    }

    /// Keeps the picture `id` names open, so nothing else takes its tab.
    pub fn keep(&mut self, id: ImageId) {
        if let Some(entry) = self.open.get_mut(&id) {
            entry.preview = false;
        }
    }

    /// Reads the pictures of `scope` at `paths` from disk again, answering
    /// whether any of them was open.
    pub fn reread_paths(&mut self, scope: Scope, paths: &BTreeSet<PathBuf>) -> bool {
        let mut reread = false;
        for entry in self.open.values_mut() {
            if entry.scope == scope && paths.contains(&entry.path) {
                (entry.picture, entry.bytes) = read(&entry.path);
                reread = true;
            }
        }
        reread
    }

    /// Closes every picture no pane is holding open any more.
    pub fn retain(&mut self, held: &BTreeSet<ImageId>) {
        self.open.retain(|id, _| held.contains(id));
    }

    /// Closes every picture of `project`.
    pub fn close_project(&mut self, project: ProjectId) {
        self.open
            .retain(|_, entry| entry.scope.project() != project);
    }
}

/// The picture at `path`, or why it could not be read, and its size on disk.
fn read(path: &Path) -> (Result<Image, String>, u64) {
    let bytes = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) => return (Err(error.to_string()), 0),
    };
    let size = bytes.len() as u64;
    let picture = Image::decode(&bytes)
        .ok_or_else(|| "This file is not a picture the editor can read".to_owned());
    (picture, size)
}
