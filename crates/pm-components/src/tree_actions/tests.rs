//! Coverage for moving entries of the tree by dragging and by the keyboard, and for bringing files in from outside.

use super::*;
use crate::state::REVIEW_FILES;

/// The sample workspace on the Files tab with a small tree listed.
fn listed() -> WorkspaceState {
    let state = WorkspaceState::sample().after(REVIEW_FILES);
    let key = state.files_of.clone().unwrap_or_default();
    let paths = [
        "Cargo.toml",
        "README.md",
        "docs/protocol.md",
        "crates/app/src/main.rs",
    ];
    let mut state = state.with_files(
        &key,
        paths.map(str::to_owned).to_vec(),
        Vec::new(),
        Vec::new(),
    );
    state.outbox.clear();
    state
}

/// The file at `path`.
fn file(path: &str) -> TreeTarget {
    TreeTarget {
        path: path.to_owned(),
        folder: false,
    }
}

/// The folder at `path`.
fn folder(path: &str) -> TreeTarget {
    TreeTarget {
        path: path.to_owned(),
        folder: true,
    }
}

/// What the tree asked to be done to the session's folder, in order.
fn changes(state: &WorkspaceState) -> Vec<EntryChange> {
    state
        .outbox
        .iter()
        .filter_map(|outgoing| match outgoing {
            Outgoing::Entry { change, .. } => Some(change.clone()),
            _ => None,
        })
        .collect()
}

/// Rows dragged onto a file land in the folder it is in, every picked row goes along, and letting go moves them.
#[test]
fn dragged_rows_move_into_the_folder_under_the_pointer() {
    let state = listed()
        .tree_row_picked(file("Cargo.toml"), Pick::Toggle)
        .tree_row_picked(file("README.md"), Pick::Toggle)
        .tree_dragged(
            file("README.md"),
            Some(file("docs/protocol.md")),
            (10.0, 10.0),
        );
    assert!(state.tree_dragging());
    assert_eq!(
        state.git.tree_drag.as_ref().unwrap().into.as_deref(),
        Some("docs")
    );
    let state = state.tree_dropped(false);
    assert!(!state.tree_dragging());
    assert!(state.open_tree.contains("docs"));
    assert_eq!(
        changes(&state),
        vec![
            EntryChange::Rename {
                from: "Cargo.toml".to_owned(),
                to: "docs/Cargo.toml".to_owned(),
            },
            EntryChange::Rename {
                from: "README.md".to_owned(),
                to: "docs/README.md".to_owned(),
            },
        ]
    );
}

/// A folder cannot land in itself or below itself, a row let go where it already is stays put, and a copy leaves the original.
#[test]
fn drops_that_move_nothing_ask_nothing() {
    let into_itself = listed().tree_dragged(
        folder("crates"),
        Some(file("crates/app/src/main.rs")),
        (0.0, 0.0),
    );
    assert_eq!(into_itself.git.tree_drag.as_ref().unwrap().into, None);
    assert!(changes(&into_itself.tree_dropped(false)).is_empty());

    let same_place = listed()
        .tree_dragged(file("README.md"), Some(TreeTarget::root()), (0.0, 0.0))
        .tree_dropped(false);
    assert!(changes(&same_place).is_empty());

    let nowhere = listed()
        .tree_dragged(file("README.md"), None, (0.0, 0.0))
        .tree_dropped(false);
    assert!(changes(&nowhere).is_empty());

    let copied = listed()
        .tree_dragged(file("README.md"), Some(folder("docs")), (0.0, 0.0))
        .tree_dropped(true);
    assert_eq!(
        changes(&copied),
        vec![EntryChange::Copy {
            from: "README.md".to_owned(),
            into: "docs".to_owned(),
        }]
    );
}

/// The keys cut the rows picked and paste them where the row last picked is, and Delete asks first.
#[test]
fn shortcuts_act_on_the_rows_picked() {
    let mut state = listed();
    state.select_tree(file("README.md"));
    let mut state = state.tree_shortcut(TreeShortcut::Cut);
    assert!(state.tree_holding());
    state.select_tree(file("docs/protocol.md"));
    let state = state.tree_shortcut(TreeShortcut::Paste);
    assert!(!state.tree_holding());
    assert_eq!(
        changes(&state),
        vec![EntryChange::Rename {
            from: "README.md".to_owned(),
            to: "docs/README.md".to_owned(),
        }]
    );
    let state = state.tree_shortcut(TreeShortcut::Delete);
    assert_eq!(
        state.git.prompt,
        Some(Prompt::Remove(vec![file("docs/protocol.md")]))
    );
    assert!(state.tree_shortcut(TreeShortcut::Rename).is_naming());
}

/// Files from outside go into the folder they are let go on, each under a name nothing there holds.
#[test]
fn files_brought_in_get_free_names() {
    let state = listed().tree_hovered(Some(TreeTarget::root()), (0.0, 0.0));
    assert!(!state.tree_dragging());
    let state = state.tree_brought_in(
        vec![
            "/home/me/README.md".to_owned(),
            "/tmp/other/README.md".to_owned(),
            "/home/me/art".to_owned(),
        ],
        Some(TreeTarget::root()),
    );
    assert_eq!(state.git.tree_drag, None);
    let imports: Vec<_> = state
        .outbox
        .iter()
        .filter_map(|outgoing| match outgoing {
            Outgoing::Import { files, .. } => Some(files.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(
        imports,
        vec![vec![
            ("/home/me/README.md".to_owned(), "README copy.md".to_owned()),
            (
                "/tmp/other/README.md".to_owned(),
                "README copy 2.md".to_owned()
            ),
            ("/home/me/art".to_owned(), "art".to_owned()),
        ]]
    );
}
