//! What the window does to the worktree the file tree lists.
//!
//! Making a file, renaming one and taking one off the disk are the three
//! things the tree does that reach past the editor, so each of them asks
//! first: the name is typed into the same prompt everything else is, and the
//! tree is read again afterwards because what it lists is the disk.

use std::path::{Path, PathBuf};

use pm_core::EntryId;

use crate::app::App;
use crate::desktop;
use crate::message::Message;
use crate::picker::Kind;
use crate::prompt::{Answer, Prompt};

impl App {
    /// Reads the worktree the window is pointed at again, and what git makes of it.
    pub(super) fn reread_worktree(&mut self) {
        if let Some(scope) = self.scope()
            && let Some(tree) = self.files.get_mut(&scope)
        {
            tree.reload();
        }
        self.reread_changes();
    }

    /// Opens the menu of what can be done to the tree entry `id` names.
    pub(super) fn open_entry_menu(&mut self, id: EntryId) {
        self.open_menu(crate::workspace::MenuTarget::Entry(id));
    }

    /// Asks for a name, and makes what `kind` asks for beside `id`.
    pub(super) fn prompt_for_path(&mut self, kind: Kind, id: EntryId) {
        let Some((path, directory)) = self.entry_path(id) else {
            return;
        };
        let (target, seeded) = match kind {
            Kind::RenamePath => (
                path.clone(),
                path.file_name()
                    .map(|name| name.to_string_lossy().into_owned())
                    .unwrap_or_default(),
            ),
            _ => (
                match directory {
                    true => path.clone(),
                    false => path.parent().unwrap_or(&path).to_path_buf(),
                },
                String::new(),
            ),
        };

        self.path_target = Some(target);
        self.open_picker_with(kind, Vec::new(), seeded);
    }

    /// Asks whether the tree entry `id` names should come off the disk.
    pub(super) fn prompt_for_delete(&mut self, id: EntryId) {
        let Some((path, directory)) = self.entry_path(id) else {
            return;
        };
        let name = path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        let asked = match directory {
            true => "Are you sure you want to permanently delete this directory?",
            false => "Are you sure you want to permanently delete this file?",
        };

        self.path_target = Some(path);
        self.ask_first(Prompt::asking(
            asked,
            vec![name],
            vec![
                Answer::new("Delete", Message::ConfirmDelete),
                Answer::cancel(),
            ],
        ));
    }

    /// Makes what a path prompt was asking for, and opens it if it is a file.
    pub(super) fn make_path(&mut self, kind: Kind, name: &str) {
        let Some(parent) = self.path_target.take() else {
            return;
        };
        if name.is_empty() {
            return;
        }
        let path = parent.join(name);

        let made = match kind {
            Kind::NewFolder => std::fs::create_dir_all(&path).is_ok(),
            _ => {
                path.parent()
                    .is_none_or(|above| std::fs::create_dir_all(above).is_ok())
                    && std::fs::write(&path, "").is_ok()
            }
        };
        self.reread_worktree();
        if made && kind == Kind::NewFile {
            self.open_path(&path);
        }
    }

    /// Renames what a rename prompt was aimed at to `name`.
    pub(super) fn rename_path(&mut self, name: &str) {
        let Some(path) = self.path_target.take() else {
            return;
        };
        if name.is_empty() {
            return;
        }
        let Some(parent) = path.parent() else {
            return;
        };
        let renamed = parent.join(name);
        if std::fs::rename(&path, &renamed).is_err() {
            return;
        }

        self.close_tabs_of(&path);
        self.reread_worktree();
        if renamed.is_file() {
            self.open_path(&renamed);
        }
    }

    /// Takes off the disk what a delete prompt was aimed at.
    pub(super) fn delete_path(&mut self) {
        let Some(path) = self.path_target.take() else {
            return;
        };
        let removed = match path.is_dir() {
            true => std::fs::remove_dir_all(&path),
            false => std::fs::remove_file(&path),
        };
        if removed.is_err() {
            return;
        }

        self.close_tabs_of(&path);
        self.reread_worktree();
    }

    /// Closes every tab showing a file at or under `path`.
    fn close_tabs_of(&mut self, path: &Path) {
        let gone = self
            .panes
            .held()
            .into_iter()
            .filter(|item| {
                item.file()
                    .and_then(|file| self.editor.path(file))
                    .is_some_and(|open| open.starts_with(path))
            })
            .collect::<Vec<_>>();
        if gone.is_empty() {
            return;
        }
        self.panes.retain(|item| !gone.contains(&item));
        self.panes.close_empty();
        self.sweep();
        self.store();
    }

    /// Opens the file at `path` in the pane that has the keyboard.
    fn open_path(&mut self, path: &Path) {
        let Some((scope, root)) = self.worktree_holding(path) else {
            return;
        };
        if self.open_picture(self.panes.focus(), scope, path, false) {
            return;
        }
        if let Some(file) = self.editor.open(scope, &root, path, false) {
            self.show_file(self.panes.focus(), file, false);
        }
    }

    /// Shows the tree entry `id` names in the desktop's file manager.
    pub(super) fn reveal_entry(&mut self, id: EntryId) {
        if let Some((path, _)) = self.entry_path(id) {
            desktop::reveal(&path);
        }
    }

    /// Starts a shell in the directory the tree entry `id` names sits in.
    pub(super) fn open_entry_in_terminal(&mut self, id: EntryId) {
        let Some((path, directory)) = self.entry_path(id) else {
            return;
        };
        let Some((scope, _)) = self.worktree_holding(&path) else {
            return;
        };
        let directory = match directory {
            true => path.clone(),
            false => path.parent().unwrap_or(&path).to_path_buf(),
        };

        let env = self.worktree_env(scope);
        self.terminals.start(scope, &directory, &env);
        self.bottom_panel_open = true;
        self.terminal_focused = true;
        self.editor_focused = false;
    }

    /// Puts the path of the tree entry `id` names on the clipboard.
    pub(super) fn copy_entry_path(&mut self, id: EntryId, relative: bool) {
        let Some((path, _)) = self.entry_path(id) else {
            return;
        };
        let written = match self.worktree_holding(&path) {
            Some((_, root)) if relative => path
                .strip_prefix(&root)
                .unwrap_or(&path)
                .display()
                .to_string(),
            _ => path.display().to_string(),
        };
        desktop::copy(written);
    }

    /// Where the tree entry `id` names lives, and whether it holds others.
    pub(super) fn entry_path(&self, id: EntryId) -> Option<(PathBuf, bool)> {
        let tree = self.files.get(&self.scope()?)?;
        let entry = crate::tree::entry_of(tree, id)?;
        Some((entry.path().to_path_buf(), entry.is_directory()))
    }

    /// The worktree the window is holding that `path` lives in.
    fn worktree_holding(&self, path: &Path) -> Option<(pm_core::Scope, PathBuf)> {
        self.worktrees()
            .into_iter()
            .filter(|(_, root)| path.starts_with(root))
            .max_by_key(|(_, root)| root.as_os_str().len())
    }
}
