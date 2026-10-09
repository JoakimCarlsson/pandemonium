//! A name being typed into the tree, where the row it names will be.
//!
//! Making a file, making a directory and renaming one are the same gesture:
//! a line of the tree turns into a field, the reader types, and Enter makes
//! it so. The name is checked as it is typed, so a name that is already
//! taken is said to be taken before anything is asked of the disk.

use std::path::{Path, PathBuf};

use pm_core::ops;

use crate::input::Input;

/// What the name being typed is for.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EditKind {
    /// A new, empty file.
    NewFile,
    /// A new directory.
    NewFolder,
    /// A new name for what is already there.
    Rename,
}

/// A name being typed into a line of the tree.
#[derive(Clone, Debug)]
pub struct Edit {
    /// What the name is for.
    kind: EditKind,
    /// The directory a new entry goes in, or the entry being renamed.
    at: PathBuf,
    /// What has been typed.
    field: Input,
}

impl Edit {
    /// A name for a new entry of `kind` inside `directory`.
    pub fn creating(kind: EditKind, directory: &Path) -> Self {
        Self {
            kind,
            at: directory.to_path_buf(),
            field: Input::default(),
        }
    }

    /// A new name for the entry at `path`, starting from the one it has.
    ///
    /// The caret starts before the extension, which is the part of a name a
    /// rename is nearly always about.
    pub fn renaming(path: &Path) -> Self {
        let name = path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        let stem = match name.rfind('.') {
            Some(0) | None => name.chars().count(),
            Some(at) => name[..at].chars().count(),
        };
        let mut field = Input::filled(name);
        field.place(stem);
        Self {
            kind: EditKind::Rename,
            at: path.to_path_buf(),
            field,
        }
    }

    /// What the name is for.
    pub fn kind(&self) -> EditKind {
        self.kind
    }

    /// The directory a new entry goes in, or the entry being renamed.
    pub fn at(&self) -> &Path {
        &self.at
    }

    /// What has been typed.
    pub fn field(&self) -> &Input {
        &self.field
    }

    /// What has been typed, to be typed into.
    pub fn field_mut(&mut self) -> &mut Input {
        &mut self.field
    }

    /// The directory the named entry will be in.
    pub fn directory(&self) -> &Path {
        match self.kind {
            EditKind::Rename => self.at.parent().unwrap_or(&self.at),
            EditKind::NewFile | EditKind::NewFolder => &self.at,
        }
    }

    /// Where the entry will be once the name is taken.
    pub fn target(&self) -> PathBuf {
        self.directory().join(self.field.value().trim())
    }

    /// Whether taking the name now would change nothing at all.
    pub fn is_unchanged(&self) -> bool {
        self.kind == EditKind::Rename && self.target() == self.at
    }

    /// What is wrong with the name typed so far, if anything is.
    pub fn problem(&self) -> Option<String> {
        let value = self.field.value();
        let name = value.trim();
        if name.is_empty() {
            return Some("A file or folder name must be provided".to_owned());
        }
        if !ops::is_valid_name(name) {
            return Some(format!(
                "The name {name} is not valid as a file or folder name"
            ));
        }
        let target = self.target();
        let renaming_case = self.kind == EditKind::Rename
            && target.to_string_lossy().to_lowercase() == self.at.to_string_lossy().to_lowercase();
        if !renaming_case && std::fs::symlink_metadata(&target).is_ok() {
            return Some(format!(
                "A file or folder {name} already exists at this location"
            ));
        }
        None
    }
}
