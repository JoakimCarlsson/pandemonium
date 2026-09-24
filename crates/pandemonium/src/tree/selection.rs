//! Which rows of the tree are selected, and which one the keyboard is on.
//!
//! A selection is kept by path rather than by entry, because reading the
//! disk again hands every entry a new id while the files stay where they
//! were. The order rows are walked in is the order the tree draws them, so
//! every gesture that spans rows is handed that order.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// The rows the reader has selected in one worktree's tree.
#[derive(Clone, Debug, Default)]
pub struct Selection {
    /// The row the keyboard is on.
    cursor: Option<PathBuf>,
    /// The rows selected, the cursor's among them unless it was unmarked.
    marked: BTreeSet<PathBuf>,
    /// Where a range swept with shift is measured from.
    anchor: Option<PathBuf>,
    /// What was marked before the range being swept began.
    base: BTreeSet<PathBuf>,
}

impl Selection {
    /// The row the keyboard is on.
    pub fn cursor(&self) -> Option<&Path> {
        self.cursor.as_deref()
    }

    /// Whether the row at `path` is selected.
    pub fn is_selected(&self, path: &Path) -> bool {
        self.marked.contains(path)
    }

    /// How many rows are selected.
    pub fn count(&self) -> usize {
        self.marked.len()
    }

    /// Selects the row at `path` alone, and puts the keyboard on it.
    pub fn select(&mut self, path: &Path) {
        self.marked.clear();
        self.marked.insert(path.to_path_buf());
        self.base.clear();
        self.cursor = Some(path.to_path_buf());
        self.anchor = Some(path.to_path_buf());
    }

    /// Puts the keyboard on `path`, leaving what is selected as it is.
    pub fn place(&mut self, path: &Path) {
        self.cursor = Some(path.to_path_buf());
    }

    /// Adds the row at `path` to what is selected, or takes it out.
    pub fn toggle(&mut self, path: &Path) {
        if !self.marked.remove(path) {
            self.marked.insert(path.to_path_buf());
        }
        self.base = self.marked.clone();
        self.cursor = Some(path.to_path_buf());
        self.anchor = Some(path.to_path_buf());
    }

    /// Selects every row from the anchor to `path`, in the order drawn.
    ///
    /// What was marked before the sweep began stays marked, so a range
    /// added with the secondary modifier held adds to what was there; a
    /// sweep drawn out and back again leaves only what it still covers.
    pub fn extend_to(&mut self, path: &Path, order: &[PathBuf]) {
        let anchor = self.anchor.clone().unwrap_or_else(|| path.to_path_buf());
        let place = |target: &Path| order.iter().position(|row| row == target);
        let (Some(from), Some(to)) = (place(&anchor), place(path)) else {
            return self.select(path);
        };
        let (low, high) = (from.min(to), from.max(to));
        self.marked = self.base.clone();
        self.marked.extend(order[low..=high].iter().cloned());
        self.cursor = Some(path.to_path_buf());
        self.anchor = Some(anchor);
    }

    /// Selects every row in `order`.
    pub fn select_all(&mut self, order: &[PathBuf]) {
        self.marked = order.iter().cloned().collect();
        self.base = self.marked.clone();
    }

    /// Forgets what is selected, keeping the keyboard where it is.
    pub fn clear(&mut self) {
        self.marked.clear();
        self.base.clear();
    }

    /// Moves the keyboard `steps` rows along `order`, sweeping if asked.
    ///
    /// Without a sweep the row reached is selected alone, which is how the
    /// arrows walk a tree; with one it is added to the range being swept.
    pub fn step(&mut self, order: &[PathBuf], steps: isize, sweeping: bool) {
        let Some(last) = order.len().checked_sub(1) else {
            return;
        };
        let at = self
            .cursor
            .as_ref()
            .and_then(|cursor| order.iter().position(|row| row == cursor));
        let reached = match at {
            Some(at) => at.saturating_add_signed(steps).min(last),
            None if steps < 0 => last,
            None => 0,
        };
        let path = order[reached].clone();
        match sweeping {
            true => self.extend_to(&path, order),
            false => self.select(&path),
        }
    }

    /// Forgets every row whose path `keep` turns down.
    pub fn retain(&mut self, keep: impl Fn(&Path) -> bool) {
        self.marked.retain(|path| keep(path));
        self.base.retain(|path| keep(path));
        if self.cursor.as_deref().is_some_and(|cursor| !keep(cursor)) {
            self.cursor = None;
        }
        if self.anchor.as_deref().is_some_and(|anchor| !keep(anchor)) {
            self.anchor = None;
        }
    }

    /// What a command given now acts on, in the order the tree draws it.
    ///
    /// That is what is selected, or the row the keyboard is on when nothing
    /// is; a row under another that is also being acted on is left out, so
    /// a directory and a file inside it are moved or deleted once.
    pub fn acting_on(&self, order: &[PathBuf]) -> Vec<PathBuf> {
        let mut chosen = match self.marked.is_empty() {
            true => self.cursor.iter().cloned().collect::<Vec<_>>(),
            false => self.marked.iter().cloned().collect(),
        };
        chosen.sort_by_key(|path| order.iter().position(|row| row == path));
        chosen
            .iter()
            .filter(|path| {
                !chosen
                    .iter()
                    .any(|other| other != *path && path.starts_with(other))
            })
            .cloned()
            .collect()
    }
}
