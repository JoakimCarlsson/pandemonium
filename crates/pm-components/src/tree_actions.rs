//! What the reader does to the project's files from the Files tab: make a file or folder, rename, cut, copy, paste, duplicate, delete and copy a path.

use pm_input::FocusId;

use crate::git_prompts::Prompt;
use crate::state::{Outgoing, WorkspaceState};

/// Asks for a name for a new file at the root of the project.
pub const TREE_NEW_FILE: FocusId = FocusId(7077);

/// Asks for a name for a new folder at the root of the project.
pub const TREE_NEW_FOLDER: FocusId = FocusId(7078);

/// Reads the project's files and changes again.
pub const TREE_REFRESH: FocusId = FocusId(7079);

/// Folds every folder of the project's files.
pub const TREE_COLLAPSE: FocusId = FocusId(7080);

/// The empty space under the rows, which opens the menu of the tree itself.
pub const TREE_SPACE: FocusId = FocusId(7081);

/// The box the name of a new or renamed file or folder is typed in.
pub const ENTRY_FIELD: FocusId = FocusId(7076);

/// Makes the file or folder, or renames the entry, under the name typed in the tree.
pub const TREE_NAME_SUBMIT: FocusId = FocusId(7082);

/// The first of the rows of the project's files; the rest follow it.
pub const TREE_FILE_BASE: u64 = 3_000_000;

/// A file or folder of the tree, by its path from the root of the project; the root itself has an empty path.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TreeTarget {
    /// Where it is, from the root of the project.
    pub path: String,
    /// Whether it is a folder.
    pub folder: bool,
}

impl TreeTarget {
    /// The root of the project.
    pub fn root() -> Self {
        Self {
            path: String::new(),
            folder: true,
        }
    }

    /// Whether this is the root of the project.
    pub fn is_root(&self) -> bool {
        self.path.is_empty()
    }

    /// The folder new entries made from this one go in: itself when it is a folder, the folder it is in when it is a file.
    pub fn directory(&self) -> String {
        if self.folder {
            self.path.clone()
        } else {
            parent_of(&self.path).to_owned()
        }
    }
}

/// What a name typed into the tree is for.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Naming {
    /// A new file or folder inside a folder.
    New {
        /// The folder, from the root of the project; empty for the root.
        directory: String,
        /// Whether it is a folder.
        folder: bool,
    },
    /// A new name for an entry that is there.
    Rename(TreeTarget),
}

/// What was cut or copied out of the tree, waiting to be pasted.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Clipboard {
    /// What it is.
    pub target: TreeTarget,
    /// Whether pasting moves it rather than copies it.
    pub cut: bool,
}

/// What is done to the folder of a session on behalf of the tree.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EntryChange {
    /// Makes an empty file, or a folder, at the path.
    Create {
        /// Where it goes, from the root of the project.
        path: String,
        /// Whether to make a folder.
        folder: bool,
    },
    /// Moves an entry to a new path.
    Rename {
        /// Where it is.
        from: String,
        /// Where it goes.
        to: String,
    },
    /// Copies an entry into a folder.
    Copy {
        /// Where it is.
        from: String,
        /// The folder the copy goes in; empty for the root.
        into: String,
    },
    /// Takes an entry off the disk.
    Remove {
        /// Where it is.
        path: String,
    },
}

/// The folder `path` is in, empty for a path at the root.
pub fn parent_of(path: &str) -> &str {
    path.rsplit_once('/').map_or("", |(parent, _)| parent)
}

/// The last part of `path`.
pub fn name_of(path: &str) -> &str {
    path.rsplit_once('/').map_or(path, |(_, name)| name)
}

/// `name` inside the folder `directory`, which is empty at the root.
pub fn joined(directory: &str, name: &str) -> String {
    if directory.is_empty() {
        name.to_owned()
    } else {
        format!("{directory}/{name}")
    }
}

impl WorkspaceState {
    /// The file or folder of the tree row `id` is, if it is one, or the root when it is the space under the rows.
    pub fn tree_row_of(&self, id: FocusId) -> Option<TreeTarget> {
        if id == TREE_SPACE {
            return Some(TreeTarget::root());
        }
        if let Some(index) =
            id.0.checked_sub(TREE_FILE_BASE)
                .map(|index| index as usize)
                .filter(|index| *index < self.files.len())
        {
            return Some(TreeTarget {
                path: self.files[index].clone(),
                folder: false,
            });
        }
        let position =
            id.0.checked_sub(crate::state::TREE_BASE)
                .map(|index| index as usize)
                .filter(|index| *index < self.files.len() + self.tree_folders.len())?;
        crate::file_tree::folder_paths(&self.files, &self.tree_folders)
            .into_iter()
            .nth(position)
            .map(|path| TreeTarget { path, folder: true })
    }

    /// The state with the menu on the tree open at `at` over `target`.
    pub fn with_tree_menu(mut self, target: TreeTarget, at: (f32, f32)) -> Self {
        if !target.is_root() {
            self.git.tree_selected = Some(target.clone());
        }
        self.git.tree_target = Some(target);
        self.with_git_menu(crate::git_menus::MenuKind::Tree, at)
    }

    /// The state after the files that were made, renamed, copied or removed are listed again.
    pub fn with_tree_stale(mut self) -> Self {
        self.reread_files();
        self
    }

    /// Asks for the files of the project again.
    pub(crate) fn reread_files(&mut self) {
        self.files_of = None;
        self.refresh_files();
    }

    /// Carries out `action` if it is a button or row of the tree, and says whether it was.
    pub(crate) fn press_tree(&mut self, action: FocusId) -> bool {
        match action {
            TREE_NEW_FILE | TREE_NEW_FOLDER => {
                let directory = self
                    .git
                    .tree_selected
                    .as_ref()
                    .map(TreeTarget::directory)
                    .unwrap_or_default();
                self.ask_name(Naming::New {
                    directory,
                    folder: action == TREE_NEW_FOLDER,
                });
            }
            TREE_NAME_SUBMIT => self.confirm_name(),
            ENTRY_FIELD => {}
            TREE_REFRESH => {
                self.reread_files();
                if let Some(session) = self.review_index() {
                    self.outbox.push(Outgoing::Refresh { session });
                }
            }
            TREE_COLLAPSE => {
                self.open_tree.clear();
            }
            TREE_SPACE => {}
            _ => {
                let Some(target) = self.tree_row_of(action).filter(|target| !target.folder) else {
                    return false;
                };
                self.git.tree_selected = Some(target.clone());
                self.open_viewer(target.path, false);
            }
        }
        true
    }

    /// Opens a field in the tree for the name `naming` calls for, unfolding the folder a new entry goes in.
    fn ask_name(&mut self, naming: Naming) {
        self.git.menu = None;
        self.git.prompt = None;
        let current = match &naming {
            Naming::Rename(target) => name_of(&target.path).to_owned(),
            Naming::New { directory, .. } => {
                self.open_tree.insert(directory.clone());
                String::new()
            }
        };
        self.git.naming = Some((naming, pm_text::Field::new(current)));
    }

    /// Whether the tree is asking for a name.
    pub fn is_naming(&self) -> bool {
        self.git.naming.is_some()
    }

    /// The state with the name being typed in the tree given up.
    pub fn without_naming(mut self) -> Self {
        self.git.naming = None;
        self
    }

    /// What is wrong with the name typed in the tree, if anything is.
    pub fn naming_problem(&self) -> Option<&'static str> {
        let (naming, field) = self.git.naming.as_ref()?;
        let typed = field.text().trim();
        if typed.is_empty() || self.naming_unchanged() {
            return None;
        }
        if typed == "." || typed == ".." || typed.contains(['/', '\\', '\0']) {
            return Some("A file or folder name cannot be a path.");
        }
        let directory = match naming {
            Naming::New { directory, .. } => directory.clone(),
            Naming::Rename(target) => parent_of(&target.path).to_owned(),
        };
        let path = joined(&directory, typed);
        let taken = self.files.iter().any(|file| {
            file == &path
                || file
                    .strip_prefix(&path)
                    .is_some_and(|rest| rest.starts_with('/'))
        }) || self.tree_folders.iter().any(|folder| folder == &path);
        taken.then_some("A file or folder with this name already exists.")
    }

    /// Whether the name typed for a rename is the entry's own.
    fn naming_unchanged(&self) -> bool {
        matches!(&self.git.naming, Some((Naming::Rename(target), field))
            if field.text().trim() == name_of(&target.path))
    }

    /// The target the menu that is open was opened on.
    fn tree_target(&self) -> Option<TreeTarget> {
        self.git.tree_target.clone()
    }

    /// Asks for the name of a new file, or folder, in the folder the menu was opened on.
    pub(crate) fn tree_new(&mut self, folder: bool) {
        let directory = self
            .tree_target()
            .map(|target| target.directory())
            .unwrap_or_default();
        self.ask_name(Naming::New { directory, folder });
    }

    /// Asks for the new name of the entry the menu was opened on.
    pub(crate) fn tree_rename(&mut self) {
        if let Some(target) = self.tree_target().filter(|target| !target.is_root()) {
            self.ask_name(Naming::Rename(target));
        }
    }

    /// Asks whether to delete the entry the menu was opened on.
    pub(crate) fn tree_delete(&mut self) {
        if let Some(target) = self.tree_target().filter(|target| !target.is_root()) {
            self.ask(Prompt::Remove(target));
        }
    }

    /// Opens the file the menu was opened on.
    pub(crate) fn tree_open(&mut self) {
        if let Some(target) = self.tree_target().filter(|target| !target.folder) {
            self.open_viewer(target.path, false);
        }
    }

    /// Holds the entry the menu was opened on, to be moved by a paste when `cut` and copied by it when not.
    pub(crate) fn tree_hold(&mut self, cut: bool) {
        if let Some(target) = self.tree_target().filter(|target| !target.is_root()) {
            self.git.tree_clipboard = Some(Clipboard { target, cut });
        }
    }

    /// Puts what is held into the folder the menu was opened on.
    pub(crate) fn tree_paste(&mut self) {
        let Some(held) = self.git.tree_clipboard.clone() else {
            return;
        };
        let into = self
            .tree_target()
            .map(|target| target.directory())
            .unwrap_or_default();
        if held.cut {
            let to = joined(&into, name_of(&held.target.path));
            self.git.tree_clipboard = None;
            self.send_entry(EntryChange::Rename {
                from: held.target.path,
                to,
            });
        } else {
            self.send_entry(EntryChange::Copy {
                from: held.target.path,
                into,
            });
        }
    }

    /// Copies the entry the menu was opened on into the folder it is in, beside itself.
    pub(crate) fn tree_duplicate(&mut self) {
        if let Some(target) = self.tree_target().filter(|target| !target.is_root()) {
            let into = parent_of(&target.path).to_owned();
            self.send_entry(EntryChange::Copy {
                from: target.path,
                into,
            });
        }
    }

    /// Puts the path of the entry the menu was opened on on the clipboard, from the root of the project or in full.
    pub(crate) fn tree_copy_path(&mut self, relative: bool) {
        let Some(target) = self.tree_target() else {
            return;
        };
        let Some(session) = self.review_session() else {
            return;
        };
        let text = match (relative, target.is_root()) {
            (true, true) => ".".to_owned(),
            (true, false) => target.path,
            (false, true) => session.directory.clone(),
            (false, false) => joined(session.directory.trim_end_matches('/'), &target.path),
        };
        self.outbox.push(Outgoing::Copy { text });
    }

    /// Sends `change` to the server for the open session's folder, and says it is working.
    pub(crate) fn send_entry(&mut self, change: EntryChange) {
        let Some(session) = self.review_index() else {
            return;
        };
        self.git.trouble = None;
        self.outbox.push(Outgoing::Entry { session, change });
    }

    /// Does what the name typed in the tree asks: makes the file or folder, or renames the entry. A name with a problem is left open to be mended, and an unchanged one is let go.
    pub(crate) fn confirm_name(&mut self) {
        if self.naming_problem().is_some() {
            return;
        }
        let unchanged = self.naming_unchanged();
        let Some((naming, field)) = self.git.naming.take() else {
            return;
        };
        let typed = field.text().trim().to_owned();
        if typed.is_empty() || unchanged {
            return;
        }
        match naming {
            Naming::New { directory, folder } => self.send_entry(EntryChange::Create {
                path: joined(&directory, &typed),
                folder,
            }),
            Naming::Rename(target) => {
                let to = joined(parent_of(&target.path), &typed);
                self.drop_viewer_of(&target.path);
                self.send_entry(EntryChange::Rename {
                    from: target.path,
                    to,
                });
            }
        }
    }
}
