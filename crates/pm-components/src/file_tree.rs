//! Every file of the project as a tree of folders, folders first, with the files git sees changed marked.

use std::collections::{HashMap, HashSet};

use pm_icon::IconName;
use pm_input::FocusId;
use pm_style::{
    Color, Radius, Role, flex, flex_1, flex_col, font_mono, gap, h, items_center, min_w_px,
    overflow_hidden, pl, pr, truncate,
};
use pm_ui::{Node, node};

use crate::Input;
use crate::change_list::mark_color;
use crate::file_icon::{file_icon, folder_icon};
use crate::parts::{glyph, line};
use crate::size::Size;
use crate::state::{TREE_BASE, WorkspaceState};
use crate::tile::Tile;
use crate::tree::{Entry, folders};

mod cache;

use crate::tree_actions::{ENTRY_FIELD, Naming, TREE_FILE_BASE, TreeTarget};
use cache::tree;

/// How far, in steps, each level of the tree indents its rows.
const INDENT: f32 = 4.0;

/// The path of every folder of the tree of `files` and the `empty` folders, from the top down, open or not.
pub fn folder_paths(files: &[String], empty: &[String]) -> Vec<String> {
    folders(&tree(files, empty))
        .into_iter()
        .map(|folder| folder.path.clone())
        .collect()
}

/// Every row of the tree of `files` and the `empty` folders shown while the folders in `open` are unfolded, from the top down.
pub fn shown_rows(files: &[String], empty: &[String], open: &HashSet<String>) -> Vec<TreeTarget> {
    /// Adds the rows of `entries`, and of what is in those of their folders that are open, to `rows`.
    fn collect(
        files: &[String],
        entries: &[Entry],
        open: &HashSet<String>,
        rows: &mut Vec<TreeTarget>,
    ) {
        for entry in entries {
            match entry {
                Entry::File { index, .. } => {
                    rows.extend(files.get(*index).map(|path| TreeTarget {
                        path: path.clone(),
                        folder: false,
                    }))
                }
                Entry::Folder(folder) => {
                    rows.push(TreeTarget {
                        path: folder.path.clone(),
                        folder: true,
                    });
                    if open.contains(&folder.path) {
                        collect(files, &folder.entries, open, rows);
                    }
                }
            }
        }
    }
    let mut rows = Vec::new();
    collect(files, &tree(files, empty), open, &mut rows);
    rows
}

/// The letter each changed file of the open session is marked with, by path.
fn marks(state: &WorkspaceState) -> HashMap<&str, &'static str> {
    state.review_session().map_or_else(HashMap::new, |session| {
        session
            .changes
            .iter()
            .map(|change| (change.path.as_str(), change.mark()))
            .collect()
    })
}

/// What a row says about the changes under it: the letter it is marked with, if it is a changed file, and whether it is changed or holds what is.
struct Changed {
    /// The letter git's status is written with, for a file.
    letter: Option<&'static str>,
    /// The colour the name is written in, when there is a change.
    tone: Option<Color>,
}

impl Changed {
    /// A row with no change under it.
    fn none() -> Self {
        Self {
            letter: None,
            tone: None,
        }
    }

    /// A file or folder git ignores.
    fn ignored() -> Self {
        Self {
            letter: None,
            tone: Some(Color::from(Role::Muted)),
        }
    }

    /// A file marked `letter`.
    fn file(letter: &'static str) -> Self {
        Self {
            letter: Some(letter),
            tone: Some(mark_color(letter)),
        }
    }

    /// A folder with a changed file somewhere beneath it.
    fn folder() -> Self {
        Self {
            letter: None,
            tone: Some(mark_color("M")),
        }
    }
}

/// One row: a name beside the icon of what it is, `depth` levels down, and the letter it is marked with.
fn row(
    icon: Node,
    name: &str,
    depth: usize,
    fold: Option<bool>,
    id: FocusId,
    changed: &Changed,
    selected: bool,
) -> Node {
    let chevron = match fold {
        Some(true) => glyph(IconName::ChevronDown, 3.5, Role::Muted),
        Some(false) => glyph(IconName::ChevronRight, 3.5, Role::Muted),
        None => node().styles([pm_style::size(3.5)]),
    };
    let tone = changed.tone.unwrap_or_else(|| Color::from(Role::Text));
    let mut row = Tile::new(id)
        .selected(selected)
        .height(6.0)
        .gap(1.5)
        .radius(Radius::Sm)
        .build()
        .styles([
            pm_style::flex_none(),
            pl(3.0 + depth as f32 * INDENT),
            pr(3),
        ])
        .child(chevron)
        .child(icon)
        .child(
            node()
                .styles([flex_1(), min_w_px(0), overflow_hidden()])
                .child(line(name, 13, 1.4, tone).styles([truncate()])),
        );
    if let Some(letter) = changed.letter {
        row = row.child(line(letter, 11, 1.4, tone).styles([font_mono()]));
    }
    row
}

/// The field a name is typed into where its row will be, `depth` levels down, with what is wrong with the name under it.
fn name_lines(state: &WorkspaceState, folder: bool, depth: usize) -> Vec<Node> {
    let Some((_, field)) = state.git.naming.as_ref() else {
        return Vec::new();
    };
    let problem = state.naming_problem();
    let icon = if folder {
        folder_icon(false, 4.0)
    } else {
        file_icon(field.text(), 4.0)
    };
    let mut lines = vec![
        node()
            .styles([
                flex(),
                items_center(),
                gap(2),
                h(6),
                pl(3.0 + depth as f32 * INDENT),
                pr(3),
            ])
            .child(node().styles([pm_style::size(3.5)]))
            .child(icon)
            .child(
                node().styles([flex_1(), min_w_px(0)]).child(
                    Input::new(field, ENTRY_FIELD)
                        .size(Size::Sm)
                        .invalid(problem.is_some())
                        .build(),
                ),
            ),
    ];
    if let Some(problem) = problem {
        lines.push(
            node()
                .styles([
                    flex(),
                    flex_col(),
                    pl(3.0 + depth as f32 * INDENT + 8.0),
                    pr(3),
                    pm_style::pb(1),
                ])
                .child(line(problem, 12, 1.4, Role::Danger)),
        );
    }
    lines
}

/// What is being named in the tree, if it is a new entry in `directory`: whether it is a folder.
fn new_in(state: &WorkspaceState, directory: &str) -> Option<bool> {
    match state.git.naming.as_ref()? {
        (
            Naming::New {
                directory: at,
                folder,
            },
            _,
        ) if at == directory => Some(*folder),
        _ => None,
    }
}

/// Whether the entry at `path` is the one being renamed.
fn renaming(state: &WorkspaceState, path: &str) -> bool {
    matches!(state.git.naming.as_ref(), Some((Naming::Rename(target), _)) if target.path == path)
}

/// Whether `path` is among the rows `picked`.
fn picked(picked: &[TreeTarget], path: &str, folder: bool) -> bool {
    picked
        .iter()
        .any(|target| target.path == path && target.folder == folder)
}

/// Visible row nodes and compact spacers for entries outside the viewport.
struct Rows {
    /// The row indices built, with a small margin around the viewport.
    visible: std::ops::Range<usize>,
    /// The next row's position in the expanded tree.
    index: usize,
    /// The fixed row height at the current density.
    height: f32,
    /// Offscreen entries awaiting a single spacer.
    skipped: Vec<FocusId>,
    /// The visible nodes and spacers in tree order.
    nodes: Vec<Node>,
}

impl Rows {
    /// Adds a row, constructing its contents only when it can be seen.
    fn push(&mut self, id: FocusId, build: impl FnOnce() -> Node) {
        if self.visible.contains(&self.index) {
            self.flush();
            self.nodes.push(build());
        } else {
            self.skipped.push(id);
        }
        self.index += 1;
    }

    /// Replaces consecutive offscreen rows with their total height and keyboard identities.
    fn flush(&mut self) {
        if !self.skipped.is_empty() {
            self.nodes.push(
                node()
                    .styles([
                        pm_style::h_px(self.skipped.len() as f32 * self.height),
                        pm_style::flex_none(),
                    ])
                    .virtual_focus(std::mem::take(&mut self.skipped)),
            );
        }
    }
}

/// Adds the entries in open folders, building only the rows inside `rows`' viewport.
fn rows_of(
    state: &WorkspaceState,
    marks: &HashMap<&str, &'static str>,
    chosen: &[TreeTarget],
    entries: &[Entry],
    depth: usize,
    rows: &mut Rows,
) {
    for entry in entries {
        match entry {
            Entry::File { name, index } => {
                let path = state.files.get(*index).map_or("", String::as_str);
                if renaming(state, path) {
                    rows.nodes.extend(name_lines(state, false, depth));
                    continue;
                }
                let id = FocusId(TREE_FILE_BASE + *index as u64);
                rows.push(id, || {
                    let changed = match marks.get(path) {
                        Some(letter) => Changed::file(letter),
                        None if state.tree_ignored.contains(path) => Changed::ignored(),
                        None => Changed::none(),
                    };
                    row(
                        file_icon(name, 4.0),
                        name,
                        depth,
                        None,
                        id,
                        &changed,
                        picked(chosen, path, false),
                    )
                });
            }
            Entry::Folder(folder) => {
                let open = state.open_tree.contains(&folder.path);
                if renaming(state, &folder.path) {
                    rows.nodes.extend(name_lines(state, true, depth));
                } else {
                    let id = FocusId(TREE_BASE + folder.position as u64);
                    rows.push(id, || {
                        let changed = if folder.files.iter().any(|index| {
                            state
                                .files
                                .get(*index)
                                .is_some_and(|path| marks.contains_key(path.as_str()))
                        }) {
                            Changed::folder()
                        } else if state.tree_ignored.contains(&folder.path) {
                            Changed::ignored()
                        } else {
                            Changed::none()
                        };
                        row(
                            folder_icon(open, 4.0),
                            &folder.name,
                            depth,
                            Some(open),
                            id,
                            &changed,
                            picked(chosen, &folder.path, true),
                        )
                    });
                }
                if open {
                    if let Some(is_folder) = new_in(state, &folder.path) {
                        rows.nodes.extend(name_lines(state, is_folder, depth + 1));
                    }
                    rows_of(state, marks, chosen, &folder.entries, depth + 1, rows);
                }
            }
        }
    }
}

/// Counts entries in expanded folders without building row descriptions.
fn row_count(entries: &[Entry], open: &HashSet<String>) -> usize {
    entries
        .iter()
        .map(|entry| {
            1 + match entry {
                Entry::Folder(folder) if open.contains(&folder.path) => {
                    row_count(&folder.entries, open)
                }
                _ => 0,
            }
        })
        .sum()
}

/// The rows of the files of the project the open session works in, with the field a name is typed in where it goes.
pub fn rows(state: &WorkspaceState, viewport: f32) -> Vec<Node> {
    let entries = tree(&state.files, &state.tree_folders);
    let height = 24.0 * pm_style::step_scale();
    let count = row_count(&entries, &state.open_tree);
    let room = (count as f32 * height + 128.0 * pm_style::step_scale() - viewport).max(0.0);
    let offset = state.tree_scroll.clamp(0.0, room);
    let first = (offset / height).floor() as usize;
    let visible = if state.git.naming.is_some() {
        0..usize::MAX
    } else {
        first.saturating_sub(2)..((offset + viewport.max(0.0)) / height).ceil() as usize + 2
    };
    let mut rows = Rows {
        visible,
        index: 0,
        height,
        skipped: Vec::new(),
        nodes: Vec::new(),
    };
    if let Some(is_folder) = new_in(state, "") {
        rows.nodes.extend(name_lines(state, is_folder, 0));
    }
    rows_of(
        state,
        &marks(state),
        &state.tree_picked(),
        &entries,
        0,
        &mut rows,
    );
    rows.flush();
    rows.nodes
}

#[cfg(test)]
mod tests;
