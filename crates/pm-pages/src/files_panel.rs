//! The right panel's Files tab: the folder's tree with change marks and file
//! icons, a menu of tree actions on each entry, names typed in place for new
//! and renamed entries, entries dragged onto folders and files dropped from
//! other applications imported; F2, Delete and the clipboard keys act on the
//! entries chosen, and Ctrl or Shift with a click chooses several. Only the
//! rows near the view are composed, and a secondary click on empty space
//! opens the menu of the folder's root.

use std::rc::Rc;

use rustle::prelude::*;
use rustle::{Key, KeyEvent, Modifiers, remember_focus};

use pm_components::{IconButton, Label, LabelVariant, Menu, Size, TreeItem, TreeItemVariant};
use pm_shell::{EditKind, FileTree, PickHow, TreeAction, TreeRow, Workspace, name_of};

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
        let list = remember_lazy_list_state();
        let rows = tree.rows(&workspace.git.changes);
        let top = new_entry_at(tree, "");
        let lead = usize::from(top.is_some());
        let count = lead + rows.len() + 1;
        let root_menu = actions.tree.clone();
        let import = actions.tree.clone();
        let moved = actions.tree.clone();
        let keys = actions.tree.clone();
        let focus = remember_focus();
        let naming = tree.editing.is_some();
        if tree.menu.as_deref() == Some("") {
            root_menu_popup(tree, actions);
        }
        lazy_list_keyed(
            Modifier::new()
                .fill_max_width()
                .weight(1.0)
                .focusable(&focus, move |event| !naming && tree_key(event, &keys))
                .on_context_menu(move |_| root_menu(TreeAction::Menu(Some(String::new()))))
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
            &list,
            count,
            |place| match place.checked_sub(lead).and_then(|at| rows.get(at)) {
                Some(row) => row.path.clone(),
                None if place < lead => "\u{0}new-entry".to_owned(),
                None => "\u{0}end".to_owned(),
            },
            |place| match place.checked_sub(lead).and_then(|at| rows.get(at)) {
                Some(row) => entry(tree, row, actions),
                None => match top.filter(|_| place < lead) {
                    Some(depth) => editor_row(tree, depth, actions),
                    None => spacer(Modifier::new().height(pm_components::spacing(24.0))),
                },
            },
        );
    });
}

/// Does what the key `event` asks of the entries chosen, saying whether it was a tree key: F2 renames, Delete deletes, and Ctrl+X, Ctrl+C and Ctrl+V cut, copy and paste.
fn tree_key(event: &KeyEvent, act: &Rc<dyn Fn(TreeAction)>) -> bool {
    let action = if event.is(Key::F(2), Modifiers::NONE) {
        TreeAction::RenameChosen
    } else if event.is(Key::Delete, Modifiers::NONE) {
        TreeAction::RemoveChosen
    } else if is_command(event, "x") {
        TreeAction::Hold { cut: true }
    } else if is_command(event, "c") {
        TreeAction::Hold { cut: false }
    } else if is_command(event, "v") {
        TreeAction::Paste
    } else {
        return false;
    };
    act(action);
    true
}

/// Whether `event` presses the letter `letter` with Ctrl, or Command where shortcuts use it.
fn is_command(event: &KeyEvent, letter: &str) -> bool {
    let key = || Key::Character(letter.to_owned());
    event.is(key(), Modifiers::CTRL) || event.is(key(), Modifiers::META)
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
                        parent: tree.target_folder(),
                        folder: false,
                    },
                ),
                (
                    phosphor::FOLDER_PLUS,
                    TreeAction::StartCreate {
                        parent: tree.target_folder(),
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
        entry_menu(&tree.root, row, actions);
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
    let pick = actions.tree.clone();
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
        .on_pick({
            let path = path.clone();
            move |held: Modifiers| {
                if held.shift {
                    pick(TreeAction::Pick(path.clone(), PickHow::Range));
                } else if held.command() {
                    pick(TreeAction::Pick(path.clone(), PickHow::Toggle));
                } else if folder {
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
fn entry_menu(root: &str, row: &TreeRow, actions: &Actions) {
    let close = actions.tree.clone();
    let root = root.to_owned();
    let copy = |text: String| {
        let tree = actions.tree.clone();
        move || {
            if let Some(board) = rustle::clipboard() {
                board.set_text(&text);
            }
            tree(TreeAction::Menu(None));
        }
    };
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
        .icon_item(
            phosphor::SCISSORS,
            "Cut",
            act(TreeAction::Hold { cut: true }),
        )
        .hint("Ctrl+X")
        .icon_item(
            phosphor::COPY_SIMPLE,
            "Copy",
            act(TreeAction::Hold { cut: false }),
        )
        .hint("Ctrl+C")
        .icon_item(phosphor::CLIPBOARD, "Paste", act(TreeAction::Paste))
        .hint("Ctrl+V")
        .separator()
        .icon_item(
            phosphor::LINK,
            "Copy path",
            copy(format!("{}/{path}", root.trim_end_matches('/'))),
        )
        .icon_item(
            phosphor::LINK_SIMPLE,
            "Copy relative path",
            copy(path.clone()),
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

/// The menu of the folder's root, opened by a secondary click on empty space: new entries at the root, paste, refresh, collapse and copying the folder's path.
fn root_menu_popup(tree: &FileTree, actions: &Actions) {
    let close = actions.tree.clone();
    let act = |action: TreeAction| {
        let tree = actions.tree.clone();
        move || tree(action.clone())
    };
    let root = tree.root.clone();
    let copied = actions.tree.clone();
    Menu::new()
        .offset(24.0, 28.0)
        .on_dismiss(move || close(TreeAction::Menu(None)))
        .icon_item(
            phosphor::FILE_PLUS,
            "New file",
            act(TreeAction::StartCreate {
                parent: String::new(),
                folder: false,
            }),
        )
        .icon_item(
            phosphor::FOLDER_PLUS,
            "New folder",
            act(TreeAction::StartCreate {
                parent: String::new(),
                folder: true,
            }),
        )
        .separator()
        .icon_item(phosphor::CLIPBOARD, "Paste", act(TreeAction::Paste))
        .hint("Ctrl+V")
        .separator()
        .icon_item(phosphor::LINK, "Copy path", move || {
            if let Some(board) = rustle::clipboard() {
                board.set_text(&root);
            }
            copied(TreeAction::Menu(None));
        })
        .icon_item(
            phosphor::ARROWS_CLOCKWISE,
            "Refresh",
            act(TreeAction::Refresh),
        )
        .icon_item(
            phosphor::ARROWS_IN_SIMPLE,
            "Collapse all",
            act(TreeAction::CollapseAll),
        )
        .show();
}

/// A muted line in place of the tree.
fn note(text: &str) {
    Label::new(text)
        .variant(LabelVariant::Subtle)
        .size(Size::Xs)
        .modifier(Modifier::new().padding_xy(12.0, 8.0))
        .show();
}
