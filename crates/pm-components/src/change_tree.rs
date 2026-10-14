//! The changed files as a tree of folders, each with a check box to stage it and the lines it changed.

use pm_icon::IconName;
use pm_input::FocusId;
use pm_style::{
    Color, Radius, Role, bg, border, border_color, cursor_pointer, flex, flex_1, gap, h,
    items_center, justify_center, leading_ratio, min_w_px, overflow_hidden, pl, pr, rounded, size,
    text_color, text_px, truncate,
};
use pm_ui::{Node, node, text};

use crate::Checkbox;
use crate::Tally;
use crate::file_icon::file_icon;
use crate::parts::{glyph, line};
use crate::sample::{Change, Fate, Session};
use crate::state::{FILE_BASE, FOLDER_BASE, FOLDER_STAGE_BASE, STAGE_BASE, WorkspaceState};
use crate::tree::{Entry, Folder, Layout, build, folders};

/// How far, in steps, each level of the tree indents its rows.
const INDENT: f32 = 3.5;

/// How far, in steps, a file is set in beyond the folder it is in, to line its icon up under the folder's name.
const FILE_OFFSET: f32 = 5.5;

/// The tree of `changes`, folders that hold only a folder joined into one row.
fn tree(changes: &[Change]) -> Vec<Entry> {
    let paths: Vec<&str> = changes.iter().map(|change| change.path.as_str()).collect();
    build(
        &paths,
        Layout {
            compact: true,
            sorted: false,
        },
    )
}

/// The path of every folder of the tree of `changes`, from the top down, open or not.
pub fn folder_paths(changes: &[Change]) -> Vec<String> {
    folders(&tree(changes))
        .into_iter()
        .map(|folder| folder.path.clone())
        .collect()
}

/// The positions in `changes` of every change under the folder at `position` of the tree, if there is one.
pub fn folder_files(changes: &[Change], position: usize) -> Option<Vec<usize>> {
    folders(&tree(changes))
        .get(position)
        .map(|folder| folder.files.clone())
}

/// The mark of how a file changed: a plus for a new one, a minus for one that is gone, a dot for the rest.
fn fate_mark(fate: Fate) -> Node {
    let (mark, tone) = match fate {
        Fate::Added => ("+", Role::Success),
        Fate::Deleted => ("−", Role::Danger),
        Fate::Modified => ("•", Role::Muted),
    };
    node()
        .styles([
            flex(),
            items_center(),
            justify_center(),
            size(4),
            pm_style::flex_none(),
            rounded(Radius::Sm),
            border(),
            border_color(Color::from(tone).alpha(0.6)),
        ])
        .child(text(mark).styles([text_px(11), leading_ratio(1.0), text_color(tone)]))
}

/// How much of `changes` the `indices` have staged: all, some or none.
fn staged_share(changes: &[Change], indices: &[usize]) -> (bool, bool) {
    let staged = indices
        .iter()
        .filter(|&&index| changes.get(index).is_some_and(|change| change.staged))
        .count();
    (
        staged > 0 && staged == indices.len(),
        staged > 0 && staged < indices.len(),
    )
}

/// The row of the file at `index`, `depth` levels down.
fn file_row(
    state: &WorkspaceState,
    changes: &[Change],
    name: &str,
    index: usize,
    depth: usize,
) -> Node {
    let change = &changes[index];
    let tone = if change.viewed {
        Role::Subtle
    } else {
        Role::Text
    };
    let mut styles = vec![
        flex(),
        items_center(),
        gap(2),
        h(8),
        pl(3.0 + depth as f32 * INDENT + FILE_OFFSET),
        pr(3),
        cursor_pointer(),
        bg(Role::SurfaceHover).hover(),
    ];
    if state.file == Some(index) {
        styles.push(bg(Role::SurfaceSelected));
    }
    node()
        .styles(styles)
        .child(Checkbox::new(change.staged, FocusId(STAGE_BASE + index as u64)).build())
        .child(file_icon(name, 4.0))
        .child(
            node()
                .styles([flex_1(), min_w_px(0), overflow_hidden()])
                .child(line(name, 13, 1.4, tone).styles([truncate()])),
        )
        .child(
            Tally::new(change.added(), change.removed())
                .size(11)
                .build(),
        )
        .child(fate_mark(change.fate()))
        .focusable(FocusId(FILE_BASE + index as u64))
}

/// The row of `folder`, `depth` levels down, open or folded.
fn folder_row(changes: &[Change], folder: &Folder, depth: usize, open: bool) -> Node {
    let (all, some) = staged_share(changes, &folder.files);
    let added = folder
        .files
        .iter()
        .map(|&index| changes[index].added())
        .sum();
    let removed = folder
        .files
        .iter()
        .map(|&index| changes[index].removed())
        .sum();
    node()
        .styles([
            flex(),
            items_center(),
            gap(2),
            h(8),
            pl(3.0 + depth as f32 * INDENT),
            pr(3),
            cursor_pointer(),
            bg(Role::SurfaceHover).hover(),
        ])
        .child(glyph(
            if open {
                IconName::ChevronDown
            } else {
                IconName::ChevronRight
            },
            3.5,
            Role::Muted,
        ))
        .child(
            Checkbox::new(all, FocusId(FOLDER_STAGE_BASE + folder.position as u64))
                .mixed(some)
                .build(),
        )
        .child(
            node()
                .styles([flex_1(), min_w_px(0), overflow_hidden()])
                .child(line(&folder.name, 13, 1.4, Role::Text).styles([truncate()])),
        )
        .child(Tally::new(added, removed).size(11).build())
        .focusable(FocusId(FOLDER_BASE + folder.position as u64))
}

/// The rows of `entries` and of everything in their open folders, `depth` levels down.
fn rows_of(
    state: &WorkspaceState,
    changes: &[Change],
    entries: &[Entry],
    depth: usize,
) -> Vec<Node> {
    let mut rows = Vec::new();
    for entry in entries {
        match entry {
            Entry::File { name, index } => {
                rows.push(file_row(state, changes, name, *index, depth));
            }
            Entry::Folder(folder) => {
                let open = !state.closed_folders.contains(&folder.path);
                rows.push(folder_row(changes, folder, depth, open));
                if open {
                    rows.extend(rows_of(state, changes, &folder.entries, depth + 1));
                }
            }
        }
    }
    rows
}

/// The rows of the changes of `session` as a tree.
pub fn rows(state: &WorkspaceState, session: &Session) -> Vec<Node> {
    rows_of(state, &session.changes, &tree(&session.changes), 0)
}
