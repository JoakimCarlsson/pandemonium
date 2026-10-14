//! Every file of the project as a tree of folders, folders first.

use pm_icon::IconName;
use pm_input::FocusId;
use pm_style::{
    Role, bg, cursor_pointer, flex, flex_1, gap, h, items_center, min_w_px, overflow_hidden, pl,
    pr, truncate,
};
use pm_ui::{Node, node};

use crate::file_icon::{file_icon, folder_icon};
use crate::parts::{glyph, line};
use crate::state::{TREE_BASE, WorkspaceState};
use crate::tree::{Entry, Layout, build, folders};

/// How far, in steps, each level of the tree indents its rows.
const INDENT: f32 = 4.0;

/// The tree of `files`, folders before files.
fn tree(files: &[String]) -> Vec<Entry> {
    let paths: Vec<&str> = files.iter().map(String::as_str).collect();
    build(
        &paths,
        Layout {
            compact: false,
            sorted: true,
        },
    )
}

/// The path of every folder of the tree of `files`, from the top down, open or not.
pub fn folder_paths(files: &[String]) -> Vec<String> {
    folders(&tree(files))
        .into_iter()
        .map(|folder| folder.path.clone())
        .collect()
}

/// One row: a name beside the icon of what it is, `depth` levels down.
fn row(icon: Node, name: &str, depth: usize, fold: Option<(FocusId, bool)>) -> Node {
    let mut styles = vec![
        flex(),
        items_center(),
        gap(2),
        h(8),
        pl(3.0 + depth as f32 * INDENT),
        pr(3),
        bg(Role::SurfaceHover).hover(),
    ];
    if fold.is_some() {
        styles.push(cursor_pointer());
    }
    let chevron = match fold {
        Some((_, true)) => glyph(IconName::ChevronDown, 3.5, Role::Muted),
        Some((_, false)) => glyph(IconName::ChevronRight, 3.5, Role::Muted),
        None => node().styles([pm_style::size(3.5)]),
    };
    let row = node().styles(styles).child(chevron).child(icon).child(
        node()
            .styles([flex_1(), min_w_px(0), overflow_hidden()])
            .child(line(name, 13, 1.4, Role::Text).styles([truncate()])),
    );
    match fold {
        Some((id, _)) => row.focusable(id),
        None => row,
    }
}

/// The rows of `entries` and of everything in their open folders, `depth` levels down.
fn rows_of(state: &WorkspaceState, entries: &[Entry], depth: usize) -> Vec<Node> {
    let mut rows = Vec::new();
    for entry in entries {
        match entry {
            Entry::File { name, .. } => rows.push(row(file_icon(name, 4.0), name, depth, None)),
            Entry::Folder(folder) => {
                let open = !state.closed_tree.contains(&folder.path);
                rows.push(row(
                    folder_icon(open, 4.0),
                    &folder.name,
                    depth,
                    Some((FocusId(TREE_BASE + folder.position as u64), open)),
                ));
                if open {
                    rows.extend(rows_of(state, &folder.entries, depth + 1));
                }
            }
        }
    }
    rows
}

/// The rows of the files of the project the open session works in.
pub fn rows(state: &WorkspaceState) -> Vec<Node> {
    rows_of(state, &tree(&state.files), 0)
}
