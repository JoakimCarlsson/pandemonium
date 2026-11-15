//! The right panel's Files tab: the folder's tree with change marks and file
//! icons, a menu of tree actions on each entry, names typed in place for new
//! and renamed entries, entries dragged onto folders and files dropped from
//! other applications imported.

use rustle::prelude::*;

use pm_components::{IconButton, Label, LabelVariant, Menu, Size, TreeItem, TreeItemVariant};
use pm_shell::{EditKind, FileTree, TreeAction, TreeRow, Workspace, name_of};

use crate::actions::Actions;

/// Emits the Files tab for `workspace`.
pub fn files_panel(workspace: &Workspace, actions: &Actions) {
    let tree = &workspace.tree;
    column_with(Modifier::new().fill_max_size(), || {
        header(tree, actions);
        if tree.root.is_empty() {
            note("No folder open.");
            return;
        }
        if !tree.loaded {
            note("Listing files…");
            return;
        }
        let scroll = remember_scroll_state();
        let import = actions.tree.clone();
        let moved = actions.tree.clone();
        column_with(
            Modifier::new()
                .fill_max_width()
                .weight(1.0)
                .vertical_scroll(&scroll)
                .on_file_drop(move |file| {
                    import(TreeAction::Import {
                        into: String::new(),
                        file,
                    })
                })
                .drop_target(move |payload| {
                    if let Some(from) = payload.downcast_ref::<String>() {
                        moved(TreeAction::Move {
                            from: from.clone(),
                            into: String::new(),
                        });
                    }
                })
                .padding_xy(4.0, 0.0),
            || {
                if let Some(depth) = new_entry_at(tree, "") {
                    key("new-entry", || editor_row(tree, depth, actions));
                }
                for row in tree.rows(&workspace.git.changes) {
                    key(row.path.clone(), || entry(tree, &row, actions));
                }
                spacer(Modifier::new().height(pm_components::spacing(24.0)));
            },
        );
    });
}

/// The header: the folder's name and the buttons acting on the whole tree.
fn header(tree: &FileTree, actions: &Actions) {
    let name = name_of(tree.root.trim_end_matches('/')).to_uppercase();
    row_arranged(
        Modifier::new()
            .fill_max_width()
            .height(28.0)
            .padding_xy(10.0, 0.0),
        Arrangement::SpacedBy(pm_components::spacing(2.0)),
        Alignment::Center,
        || {
            Label::new(name)
                .variant(LabelVariant::Muted)
                .size(Size::Xs)
                .modifier(Modifier::new().weight(1.0))
                .show();
            for (glyph, action) in [
                (
                    phosphor::FILE_PLUS,
                    TreeAction::StartCreate {
                        parent: selected_folder(tree),
                        folder: false,
                    },
                ),
                (
                    phosphor::FOLDER_PLUS,
                    TreeAction::StartCreate {
                        parent: selected_folder(tree),
                        folder: true,
                    },
                ),
                (phosphor::ARROWS_CLOCKWISE, TreeAction::Refresh),
                (phosphor::ARROWS_IN_SIMPLE, TreeAction::CollapseAll),
            ] {
                let act = actions.tree.clone();
                key(glyph_name(&action), || {
                    IconButton::new(glyph)
                        .size(Size::Xs)
                        .on_click(move || act(action.clone()))
                        .show();
                });
            }
        },
    );
}

/// A stable name for the header button doing `action`.
fn glyph_name(action: &TreeAction) -> &'static str {
    match action {
        TreeAction::StartCreate { folder: false, .. } => "new-file",
        TreeAction::StartCreate { folder: true, .. } => "new-folder",
        TreeAction::Refresh => "refresh",
        _ => "collapse",
    }
}

/// The folder new entries go in: the folder chosen, the folder of the file chosen, or the root.
fn selected_folder(tree: &FileTree) -> String {
    match &tree.selected {
        Some(path) if tree.is_folder(path) => path.clone(),
        Some(path) => pm_shell::parent_of(path).to_owned(),
        None => String::new(),
    }
}

/// How deep the box for a new entry inside `folder` sits, when one is being named there.
fn new_entry_at(tree: &FileTree, folder: &str) -> Option<usize> {
    match &tree.editing {
        Some(edit) => match &edit.kind {
            EditKind::Create { parent, .. } if parent == folder => Some(if folder.is_empty() {
                0
            } else {
                folder.matches('/').count() + 1
            }),
            _ => None,
        },
        None => None,
    }
}

/// The row of `row`, with its menu when open and the box for a new entry inside it when one is being named.
fn entry(tree: &FileTree, row: &TreeRow, actions: &Actions) {
    let renaming = matches!(
        tree.editing.as_ref().map(|edit| &edit.kind),
        Some(EditKind::Rename { path }) if *path == row.path
    );
    if renaming {
        editor_row(tree, row.depth, actions);
    } else {
        item(row, actions);
    }
    if tree.menu.as_deref() == Some(row.path.as_str()) {
        entry_menu(row, actions);
    }
    if row.folder
        && let Some(depth) = new_entry_at(tree, &row.path)
    {
        key("new-entry", || editor_row(tree, depth, actions));
    }
}

/// The tree row of `row`.
fn item(row: &TreeRow, actions: &Actions) {
    let path = row.path.clone();
    let folder = row.folder;
    let click = actions.tree.clone();
    let open = actions.open_file.clone();
    let menu = actions.tree.clone();
    let menu_path = path.clone();
    let mut item = TreeItem::new(row.name.clone(), row.depth)
        .mark(row.mark)
        .selected(row.selected)
        .variant(if row.ignored {
            TreeItemVariant::Dimmed
        } else {
            TreeItemVariant::Plain
        })
        .draggable(path.clone())
        .on_menu(move || menu(TreeAction::Menu(Some(menu_path.clone()))))
        .on_click({
            let path = path.clone();
            move || {
                if folder {
                    click(TreeAction::Toggle(path.clone()));
                } else {
                    click(TreeAction::Select(path.clone()));
                    open(path.clone());
                }
            }
        });
    if folder {
        item = item.folder(row.expanded);
        let moved = actions.tree.clone();
        let into = path.clone();
        item = item.on_drop(move |from| {
            moved(TreeAction::Move {
                from,
                into: into.clone(),
            })
        });
        let import = actions.tree.clone();
        let into = path.clone();
        item = item.on_file_drop(move |file| {
            import(TreeAction::Import {
                into: into.clone(),
                file,
            })
        });
    }
    item.show();
}

/// The box a name is typed in for the entry being named, `depth` folders deep.
fn editor_row(tree: &FileTree, depth: usize, actions: &Actions) {
    let Some(edit) = &tree.editing else {
        return;
    };
    let (name, folder) = match &edit.kind {
        EditKind::Create { folder, .. } => (edit.text.clone(), *folder),
        EditKind::Rename { path } => (name_of(path).to_owned(), tree.is_folder(path)),
    };
    let change = actions.tree.clone();
    let submit = actions.tree.clone();
    let cancel = actions.tree.clone();
    let mut item = TreeItem::new(name, depth).editor(
        edit.text.clone(),
        move |text| change(TreeAction::EditText(text)),
        move || submit(TreeAction::Commit),
        move || cancel(TreeAction::Cancel),
    );
    if folder {
        item = item.folder(false);
    }
    item.show();
}

/// The menu of tree actions on `row`, hung under it.
fn entry_menu(row: &TreeRow, actions: &Actions) {
    let close = actions.tree.clone();
    let act = |action: TreeAction| {
        let tree = actions.tree.clone();
        move || tree(action.clone())
    };
    let path = row.path.clone();
    let mut menu = Menu::new()
        .offset(24.0 + row.depth as f32 * 12.0, 0.0)
        .on_dismiss(move || close(TreeAction::Menu(None)));
    if row.folder {
        menu = menu
            .icon_item(
                phosphor::FILE_PLUS,
                "New file",
                act(TreeAction::StartCreate {
                    parent: path.clone(),
                    folder: false,
                }),
            )
            .icon_item(
                phosphor::FOLDER_PLUS,
                "New folder",
                act(TreeAction::StartCreate {
                    parent: path.clone(),
                    folder: true,
                }),
            );
    } else {
        let open = actions.open_file.clone();
        let tree = actions.tree.clone();
        let target = path.clone();
        menu = menu.icon_item(phosphor::FILE, "Open", move || {
            tree(TreeAction::Menu(None));
            open(target.clone());
        });
    }
    menu = menu
        .separator()
        .icon_item(
            phosphor::PENCIL_SIMPLE,
            "Rename",
            act(TreeAction::StartRename(path.clone())),
        )
        .hint("F2")
        .icon_item(
            phosphor::COPY,
            "Duplicate",
            act(TreeAction::Duplicate(path.clone())),
        )
        .separator()
        .destructive_item(
            phosphor::TRASH,
            "Delete",
            act(TreeAction::AskRemove(path.clone())),
        )
        .hint("Delete");
    menu.show();
}

/// A muted line in place of the tree.
fn note(text: &str) {
    Label::new(text)
        .variant(LabelVariant::Subtle)
        .size(Size::Xs)
        .modifier(Modifier::new().padding_xy(12.0, 8.0))
        .show();
}
