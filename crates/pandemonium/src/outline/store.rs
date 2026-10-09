//! The followed file and its outline state, shared by every view of the worktree.

use std::collections::{HashMap, HashSet};

use pm_core::Scope;
use pm_text::{Position, Symbol};

use crate::editor::FileId;
use crate::input::Input;
use crate::panes::PaneId;

/// All open worktree outlines and their per-file state.
#[derive(Default)]
pub struct Store {
    /// The last file that held the keyboard in each worktree.
    followed: HashMap<Scope, FileId>,
    /// The tree state of each file shown in an outline.
    files: HashMap<FileId, Outline>,
    /// The last pane to show each file while it held the keyboard.
    panes: HashMap<FileId, PaneId>,
}

impl Store {
    /// Remembers `file` as the file `scope`'s outline follows.
    pub fn follow(&mut self, scope: Scope, file: FileId, pane: PaneId) {
        self.followed.insert(scope, file);
        self.files.entry(file).or_default();
        self.panes.insert(file, pane);
    }

    /// The file the worktree's outline follows.
    pub fn followed(&self, scope: Scope) -> Option<FileId> {
        self.followed.get(&scope).copied()
    }

    /// The most recent pane that held the keyboard for `file`.
    pub fn pane(&self, file: FileId) -> Option<PaneId> {
        self.panes.get(&file).copied()
    }

    /// The current tree state of `file`.
    pub fn get(&self, file: FileId) -> Option<&Outline> {
        self.files.get(&file)
    }

    /// The mutable tree state of `file`.
    pub fn get_mut(&mut self, file: FileId) -> &mut Outline {
        self.files.entry(file).or_default()
    }

    /// Forgets files no longer open, leaving their worktree's outline empty.
    pub fn retain(&mut self, open: impl Fn(FileId) -> bool) {
        self.followed.retain(|_, file| open(*file));
        self.files.retain(|file, _| open(*file));
        self.panes.retain(|file, _| open(*file));
    }
}

/// The symbols and interaction state of one followed file.
#[derive(Default)]
pub struct Outline {
    /// Symbols in source order, flattened from their tree.
    pub symbols: Vec<Symbol>,
    /// Where those symbols came from.
    pub source: String,
    /// The text the outline is narrowed by.
    pub filter: Input,
    /// The selected symbol's index in `symbols`.
    pub selected: Option<usize>,
    /// Name paths of folded symbols.
    collapsed: HashSet<String>,
    /// The first visible row shown in the pane.
    pub scroll: usize,
    /// The text version for which this outline has been requested.
    pub version: Option<i32>,
    /// The cursor position last applied to selection and scroll.
    cursor: Option<Position>,
}

impl Outline {
    /// Replaces symbols while preserving folded name paths.
    pub fn replace(&mut self, version: i32, symbols: Vec<Symbol>, source: String) {
        self.version = Some(version);
        self.symbols = symbols;
        self.source = source;
        self.cursor = None;
        self.selected = self.selected.filter(|index| *index < self.symbols.len());
        self.scroll = self.scroll.min(self.symbols.len().saturating_sub(1));
    }

    /// The source indices of visible rows, with whether each row matched directly.
    pub fn visible(&self) -> Vec<(usize, bool)> {
        let mut direct = Vec::new();
        let query = self.filter.value().to_lowercase();
        for symbol in &self.symbols {
            direct.push(query.is_empty() || subsequence(&symbol.name.to_lowercase(), &query));
        }
        let mut kept = direct.clone();
        if !query.is_empty() {
            for index in (0..self.symbols.len()).rev() {
                if !kept[index] {
                    continue;
                }
                let depth = self.symbols[index].depth;
                for parent in (0..index).rev() {
                    if self.symbols[parent].depth < depth {
                        kept[parent] = true;
                        break;
                    }
                }
            }
        }
        let mut hidden_depth = None;
        let mut rows = Vec::new();
        for (index, symbol) in self.symbols.iter().enumerate() {
            if hidden_depth.is_some_and(|depth| symbol.depth <= depth) {
                hidden_depth = None;
            }
            if hidden_depth.is_some() || !kept[index] {
                continue;
            }
            rows.push((index, direct[index]));
            if query.is_empty() && self.collapsed.contains(&self.path(index)) {
                hidden_depth = Some(symbol.depth);
            }
        }
        rows
    }

    /// Whether a symbol has nested children.
    pub fn has_children(&self, index: usize) -> bool {
        self.symbols
            .get(index + 1)
            .is_some_and(|next| next.depth > self.symbols[index].depth)
    }

    /// Whether `index` is folded.
    pub fn is_collapsed(&self, index: usize) -> bool {
        self.collapsed.contains(&self.path(index))
    }

    /// Folds or unfolds the symbol at `index`.
    pub fn toggle(&mut self, index: usize) {
        let path = self.path(index);
        if !self.collapsed.remove(&path) {
            self.collapsed.insert(path);
        }
    }

    /// Selects the deepest symbol holding `cursor`, unfolding its ancestors.
    pub fn follow_cursor(&mut self, cursor: Position, visible_rows: usize) {
        if self.cursor == Some(cursor) {
            return;
        }
        self.cursor = Some(cursor);
        let selected = self
            .symbols
            .iter()
            .enumerate()
            .filter(|(_, symbol)| symbol.range.start <= cursor && cursor < symbol.range.end)
            .max_by_key(|(_, symbol)| symbol.depth)
            .map(|(index, _)| index);
        self.selected = selected;
        let Some(index) = selected else { return };
        let path = self.path(index);
        for parent in (0..index).rev() {
            let candidate = self.path(parent);
            if path.starts_with(&format!("{candidate}/")) {
                self.collapsed.remove(&self.path(parent));
            }
        }
        if let Some(row) = self.visible().iter().position(|(item, _)| *item == index) {
            if row < self.scroll {
                self.scroll = row;
            } else if row >= self.scroll + visible_rows {
                self.scroll = row.saturating_sub(visible_rows.saturating_sub(1));
            }
        }
    }

    /// The name path of a symbol, stable across symbol refreshes.
    fn path(&self, index: usize) -> String {
        let mut parts = Vec::new();
        for symbol in &self.symbols[..=index] {
            parts.truncate(symbol.depth);
            parts.push(symbol.name.as_str());
        }
        parts.join("/")
    }
}

/// Whether every character of `query` appears in `name` in order.
fn subsequence(name: &str, query: &str) -> bool {
    let mut chars = name.chars();
    query
        .chars()
        .all(|wanted| chars.by_ref().any(|found| found == wanted))
}
