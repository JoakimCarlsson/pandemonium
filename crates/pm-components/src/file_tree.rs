//! Every file of the project as a tree of folders, folders first, with the files git sees changed marked.

use std::collections::HashMap;

use pm_icon::IconName;
use pm_input::FocusId;
use pm_style::{
    Color, Role, bg, cursor_pointer, flex, flex_1, flex_col, font_mono, gap, h, items_center,
    min_w_px, overflow_hidden, pl, pr, truncate,
};
use pm_ui::{Node, node};

use crate::Input;
use crate::change_list::mark_color;
use crate::file_icon::{file_icon, folder_icon};
use crate::input::Size as InputSize;
use crate::parts::{glyph, line};
use crate::state::{TREE_BASE, WorkspaceState};
use crate::tree::{Entry, Layout, build, folders};
use crate::tree_actions::{ENTRY_FIELD, Naming, TREE_FILE_BASE};

/// How far, in steps, each level of the tree indents its rows.
const INDENT: f32 = 4.0;

/// The tree of `files`, folders before files, with `empty` among the folders.
fn tree(files: &[String], empty: &[String]) -> Vec<Entry> {
    let paths: Vec<String> = files
        .iter()
        .cloned()
        .chain(empty.iter().map(|folder| format!("{folder}/")))
        .collect();
    let paths: Vec<&str> = paths.iter().map(String::as_str).collect();
    build(
        &paths,
        Layout {
            compact: false,
            sorted: true,
        },
    )
}

/// The path of every folder of the tree of `files` and the `empty` folders, from the top down, open or not.
pub fn folder_paths(files: &[String], empty: &[String]) -> Vec<String> {
    folders(&tree(files, empty))
        .into_iter()
        .map(|folder| folder.path.clone())
        .collect()
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
    let mut styles = vec![
        flex(),
        items_center(),
        gap(2),
        h(8),
        pl(3.0 + depth as f32 * INDENT),
        pr(3),
        cursor_pointer(),
    ];
    styles.push(if selected {
        bg(Role::SurfaceSelected)
    } else {
        bg(Role::SurfaceHover).hover()
    });
    let mut row = node().styles(styles).child(chevron).child(icon).child(
        node()
            .styles([flex_1(), min_w_px(0), overflow_hidden()])
            .child(line(name, 13, 1.4, tone).styles([truncate()])),
    );
    if let Some(letter) = changed.letter {
        row = row.child(line(letter, 11, 1.4, tone).styles([font_mono()]));
    }
    row.focusable(id)
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
                h(8),
                pl(3.0 + depth as f32 * INDENT),
                pr(3),
            ])
            .child(node().styles([pm_style::size(3.5)]))
            .child(icon)
            .child(
                node().styles([flex_1(), min_w_px(0)]).child(
                    Input::new(field, ENTRY_FIELD)
                        .size(InputSize::Sm)
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

/// Whether `path` is the row last picked.
fn picked(state: &WorkspaceState, path: &str, folder: bool) -> bool {
    state
        .git
        .tree_selected
        .as_ref()
        .is_some_and(|target| target.path == path && target.folder == folder)
}

/// The rows of `entries` and of everything in their open folders, `depth` levels down.
fn rows_of(
    state: &WorkspaceState,
    marks: &HashMap<&str, &'static str>,
    entries: &[Entry],
    depth: usize,
) -> Vec<Node> {
    let mut rows = Vec::new();
    for entry in entries {
        match entry {
            Entry::File { name, index } => {
                let path = state.files.get(*index).map_or("", String::as_str);
                if renaming(state, path) {
                    rows.extend(name_lines(state, false, depth));
                    continue;
                }
                let changed = marks
                    .get(path)
                    .map_or_else(Changed::none, |letter| Changed::file(letter));
                rows.push(row(
                    file_icon(name, 4.0),
                    name,
                    depth,
                    None,
                    FocusId(TREE_FILE_BASE + *index as u64),
                    &changed,
                    picked(state, path, false),
                ));
            }
            Entry::Folder(folder) => {
                let open = !state.closed_tree.contains(&folder.path);
                let changed = if folder.files.iter().any(|index| {
                    state
                        .files
                        .get(*index)
                        .is_some_and(|path| marks.contains_key(path.as_str()))
                }) {
                    Changed::folder()
                } else {
                    Changed::none()
                };
                if renaming(state, &folder.path) {
                    rows.extend(name_lines(state, true, depth));
                } else {
                    rows.push(row(
                        folder_icon(open, 4.0),
                        &folder.name,
                        depth,
                        Some(open),
                        FocusId(TREE_BASE + folder.position as u64),
                        &changed,
                        picked(state, &folder.path, true),
                    ));
                }
                if open {
                    if let Some(is_folder) = new_in(state, &folder.path) {
                        rows.extend(name_lines(state, is_folder, depth + 1));
                    }
                    rows.extend(rows_of(state, marks, &folder.entries, depth + 1));
                }
            }
        }
    }
    rows
}

/// The rows of the files of the project the open session works in, with the field a name is typed in where it goes.
pub fn rows(state: &WorkspaceState) -> Vec<Node> {
    let mut rows = Vec::new();
    if let Some(is_folder) = new_in(state, "") {
        rows.extend(name_lines(state, is_folder, 0));
    }
    rows.extend(rows_of(
        state,
        &marks(state),
        &tree(&state.files, &state.tree_folders),
        0,
    ));
    rows
}
