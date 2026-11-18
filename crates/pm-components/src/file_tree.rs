//! Every file of the project as a tree of folders, folders first, with the files git sees changed marked.

use std::collections::{HashMap, HashSet};

use rustle::Modifier;

use crate::Input;
use crate::change_list::mark_color;
use crate::file_icon::{file_icon, folder_icon};
use crate::kit::theme::step_scale;
use crate::kit::{
    Flex, FocusId, IconName, Interactive, Radius, Role, Side, Styled, Tone, Type, column, gap_box,
    icon as icon_of, label, label_with, row,
};
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
    tone: Option<Tone>,
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
            tone: Some(Tone::from(Role::Muted)),
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

/// How a row is drawn beyond what it names.
struct Look {
    /// What it says about the changes under it.
    changed: Changed,
    /// Whether it is among the rows picked.
    selected: bool,
    /// Whether what the pointer holds would land in it.
    landing: bool,
    /// Whether it was cut and waits to be pasted elsewhere.
    cut: bool,
}

/// How the row of the entry at `path` is drawn, given what it says about the changes under it.
fn look(
    state: &WorkspaceState,
    chosen: &[TreeTarget],
    path: &str,
    folder: bool,
    changed: Changed,
) -> Look {
    Look {
        changed,
        selected: picked(chosen, path, folder),
        landing: folder
            && state
                .git
                .tree_drag
                .as_ref()
                .is_some_and(|drag| drag.into.as_deref() == Some(path)),
        cut: state.git.tree_clipboard.as_ref().is_some_and(|held| {
            held.cut
                && held
                    .targets
                    .iter()
                    .any(|target| target.path == path && target.folder == folder)
        }),
    }
}

/// Emits one row: a name beside `icon`, the icon of what it is, `depth` levels down, with a chevron when `fold` says whether it is an open or closed folder, answering to `id`, drawn as `look` says, and the letter it is marked with.
#[track_caller]
fn entry_row(
    icon: impl FnOnce(),
    name: &str,
    depth: usize,
    fold: Option<bool>,
    id: FocusId,
    look: &Look,
) {
    let tone = match look.changed.tone {
        _ if look.cut => Tone::from(Role::Subtle),
        Some(tone) => tone,
        None => Tone::from(Role::Text),
    };
    let mut around = Modifier::new();
    if look.landing {
        around = around.fill_rounded(Tone::from(Role::Accent).alpha(0.24), Radius::Sm);
    }
    if look.cut {
        around = around.opacity(60.0);
    }
    let letter = look.changed.letter;
    Tile::new(id)
        .selected(look.selected)
        .height(6.0)
        .gap(1.5)
        .radius(Radius::Sm)
        .modifier(around)
        .inner(
            Modifier::new()
                .pad_side(Side::Left, 3.0 + depth as f32 * INDENT)
                .pad_side(Side::Right, 3),
        )
        .build_with(|| {
            match fold {
                Some(true) => icon_of(IconName::ChevronDown, 3.5, Role::Muted),
                Some(false) => icon_of(IconName::ChevronRight, 3.5, Role::Muted),
                None => gap_box(Modifier::new().square(3.5)),
            }
            icon();
            label_with(
                Modifier::new().grow(),
                name,
                Type::new(13).ratio(1.4).tone(tone).truncate(),
            );
            if let Some(letter) = letter {
                label(
                    letter,
                    Type::new(11).ratio(1.4).tone(tone).truncate().mono(),
                );
            }
        });
}

/// Emits what the pointer holds over the tree, named beside it: the row being dragged, or how many there are; nothing while no row is dragged.
#[track_caller]
pub fn drag_ghost(state: &WorkspaceState) {
    let Some(drag) = state.git.tree_drag.as_ref() else {
        return;
    };
    let Some(first) = drag.targets.first() else {
        return;
    };
    let count = drag.targets.len();
    let name = crate::tree_actions::name_of(&first.path);
    let words = if count == 1 {
        name.to_owned()
    } else {
        format!("{count} items")
    };
    row(
        Modifier::new()
            .at(drag.at.0 + 12.0, drag.at.1 + 10.0)
            .z_index_unclipped(60)
            .h(7)
            .shadow_md(Radius::Md)
            .fill_rounded(Role::Surface, Radius::Md)
            .stroke(Role::Border, Radius::Md)
            .pad_x(2),
        Flex::new().center().gap(1.5),
        || {
            if count != 1 {
                icon_of(IconName::Copy, 4.0, Role::Muted);
            } else if first.folder {
                folder_icon(false, 4.0);
            } else {
                file_icon(name, 4.0);
            }
            label(&words, Type::new(13).ratio(1.4).tone(Role::Text).truncate());
        },
    );
}

/// Emits the field a name is typed into where its row will be, `depth` levels down, with what is wrong with the name under it; returns how many lines it emitted, none while nothing is being named.
#[track_caller]
fn name_lines(state: &WorkspaceState, folder: bool, depth: usize) -> usize {
    let Some((_, field)) = state.git.naming.as_ref() else {
        return 0;
    };
    let problem = state.naming_problem();
    rustle::key(ENTRY_FIELD.0, || {
        row(
            Modifier::new()
                .h(6)
                .pad_side(Side::Left, 3.0 + depth as f32 * INDENT)
                .pad_side(Side::Right, 3),
            Flex::new().center().gap(2),
            || {
                gap_box(Modifier::new().square(3.5));
                if folder {
                    folder_icon(false, 4.0);
                } else {
                    file_icon(field.text(), 4.0);
                }
                Input::new(field, ENTRY_FIELD)
                    .size(Size::Sm)
                    .invalid(problem.is_some())
                    .modifier(Modifier::new().grow())
                    .build();
            },
        );
    });
    let Some(problem) = problem else {
        return 1;
    };
    column(
        Modifier::new()
            .pad_side(Side::Left, 3.0 + depth as f32 * INDENT + 8.0)
            .pad_side(Side::Right, 3)
            .pad_side(Side::Bottom, 1),
        Flex::new(),
        || {
            label(
                problem,
                Type::new(12).ratio(1.4).tone(Role::Danger).truncate(),
            )
        },
    );
    2
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

/// The rows of the tree being emitted: only those near the viewport are drawn, and runs of the others become one spacer each.
struct Rows {
    /// The row indices drawn, with a small margin around the viewport.
    visible: std::ops::Range<usize>,
    /// The next row's position in the expanded tree.
    index: usize,
    /// The fixed row height at the current density.
    height: f32,
    /// Offscreen entries awaiting a single spacer.
    skipped: Vec<FocusId>,
    /// How many rows, name lines and spacers have been emitted so far.
    emitted: usize,
}

impl Rows {
    /// Adds the row answering to `id`, emitting it with `build` only when it can be seen.
    #[track_caller]
    fn push(&mut self, id: FocusId, build: impl FnOnce()) {
        if self.visible.contains(&self.index) {
            self.flush();
            rustle::key(id.0, build);
            self.emitted += 1;
        } else {
            self.skipped.push(id);
        }
        self.index += 1;
    }

    /// Emits the consecutive offscreen rows held as one spacer of their total height that keeps their keyboard identities.
    #[track_caller]
    fn flush(&mut self) {
        if !self.skipped.is_empty() {
            gap_box(
                Modifier::new()
                    .height(self.skipped.len() as f32 * self.height)
                    .virtual_focus(std::mem::take(&mut self.skipped)),
            );
            self.emitted += 1;
        }
    }
}

/// Emits the entries in open folders, drawing only the rows inside `rows`' viewport.
#[track_caller]
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
                    rows.emitted += name_lines(state, false, depth);
                    continue;
                }
                let id = FocusId(TREE_FILE_BASE + *index as u64);
                rows.push(id, || {
                    let changed = match marks.get(path) {
                        Some(letter) => Changed::file(letter),
                        None if state.tree_ignored.contains(path) => Changed::ignored(),
                        None => Changed::none(),
                    };
                    entry_row(
                        || file_icon(name, 4.0),
                        name,
                        depth,
                        None,
                        id,
                        &look(state, chosen, path, false, changed),
                    );
                });
            }
            Entry::Folder(folder) => {
                let open = state.open_tree.contains(&folder.path);
                if renaming(state, &folder.path) {
                    rows.emitted += name_lines(state, true, depth);
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
                        entry_row(
                            || folder_icon(open, 4.0),
                            &folder.name,
                            depth,
                            Some(open),
                            id,
                            &look(state, chosen, &folder.path, true, changed),
                        );
                    });
                }
                if open {
                    if let Some(is_folder) = new_in(state, &folder.path) {
                        rows.emitted += name_lines(state, is_folder, depth + 1);
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

/// Emits the rows of the files of the project the open session works in, with the field a name is typed in where it goes, drawing only those that fall within `viewport` logical pixels of the list's scroll offset.
#[track_caller]
pub fn rows(state: &WorkspaceState, viewport: f32) {
    emit_rows(state, viewport);
}

/// Emits what [`rows`] emits and returns how many rows, name lines and spacers that was.
#[track_caller]
fn emit_rows(state: &WorkspaceState, viewport: f32) -> usize {
    let entries = tree(&state.files, &state.tree_folders);
    let height = 24.0 * step_scale();
    let count = row_count(&entries, &state.open_tree);
    let room = (count as f32 * height + 128.0 * step_scale() - viewport).max(0.0);
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
        emitted: 0,
    };
    if let Some(is_folder) = new_in(state, "") {
        rows.emitted += name_lines(state, is_folder, 0);
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
    rows.emitted
}

#[cfg(test)]
mod tests;
