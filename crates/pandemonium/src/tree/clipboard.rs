//! What was cut or copied out of the tree, waiting to be pasted.
//!
//! The tree keeps its own clipboard of paths rather than writing them to the
//! desktop's: a paste is a copy or a move on the disk, and only the tree
//! knows which of the two it was asked for.

use std::path::{Path, PathBuf};

/// Paths cut or copied out of the tree.
#[derive(Clone, Debug)]
pub struct Clipboard {
    /// What was cut or copied.
    pub paths: Vec<PathBuf>,
    /// Whether pasting moves them rather than copying them.
    pub cut: bool,
}

impl Clipboard {
    /// Whether the row at `path` is waiting to be moved by a paste.
    pub fn is_cut(&self, path: &Path) -> bool {
        self.cut && self.paths.iter().any(|cut| path.starts_with(cut))
    }
}
