//! What the reader does to the project's files from the Files tab: make a file or folder, rename, cut, copy, paste, duplicate, delete, copy a path, drag entries into another folder and bring files in from outside the window.

use pm_input::FocusId;

use crate::git_controls::Pick;
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

/// How tall the menu on an entry of the tree is, in logical pixels, to keep it inside the window.
pub const TREE_MENU_HEIGHT: f32 = 12.0 * 32.0 + 5.0 * 9.0 + 10.0;

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
    /// What it is: one entry or several.
    pub targets: Vec<TreeTarget>,
    /// Whether pasting moves it rather than copies it.
    pub cut: bool,
}

/// Entries held by the pointer over the tree, on their way to another folder: rows of the tree, or files from outside the window.
#[derive(Clone, Debug, PartialEq)]
pub struct TreeDrag {
    /// The rows being dragged, outermost only; none when the files come from outside the window.
    pub targets: Vec<TreeTarget>,
    /// The folder they would land in, from the root of the project and empty for the root, while the pointer is over one that can take them.
    pub into: Option<String>,
    /// Where the pointer is, in logical pixels.
    pub at: (f32, f32),
}

/// What a key pressed over the tree asks of the rows picked.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TreeShortcut {
    /// Holds them to be moved by a paste.
    Cut,
    /// Holds them to be copied by a paste.
    Copy,
    /// Puts what is held into the folder of the row last picked.
    Paste,
    /// Asks for a new name for the row last picked.
    Rename,
    /// Asks whether to delete them.
    Delete,
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
    /// Writes a file of this machine into the folder.
    Import {
        /// Where it goes, from the root of the project.
        path: String,
        /// What it holds.
        data: Vec<u8>,
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

/// Splits `name` before its extension, keeping the dot with the extension; a leading dot starts no extension.
fn split_extension(name: &str) -> (&str, &str) {
    match name.rfind('.') {
        Some(0) | None => (name, ""),
        Some(at) => name.split_at(at),
    }
}

/// Whether `path` is `folder` or inside it.
fn within(path: &str, folder: &str) -> bool {
    path.strip_prefix(folder)
        .is_some_and(|rest| rest.is_empty() || rest.starts_with('/'))
}

/// `targets` without those inside a folder among them, which go with that folder.
fn outermost(targets: Vec<TreeTarget>) -> Vec<TreeTarget> {
    targets
        .iter()
        .filter(|target| {
            !target.is_root()
                && !targets.iter().any(|folder| {
                    folder.folder
                        && target
                            .path
                            .strip_prefix(&folder.path)
                            .is_some_and(|rest| rest.starts_with('/'))
                })
        })
        .cloned()
        .collect()
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

    /// The rows of the tree as they are shown, from the top down.
    fn tree_shown(&self) -> Vec<TreeTarget> {
        crate::file_tree::shown_rows(&self.files, &self.tree_folders, &self.open_tree)
    }

    /// The rows of the tree a command acts on, as they are shown: those marked, or the row last picked when none of them is shown.
    pub fn tree_picked(&self) -> Vec<TreeTarget> {
        if self.git.tree_marked.is_empty() {
            return self.git.tree_selected.clone().into_iter().collect();
        }
        let marked: Vec<TreeTarget> = self
            .tree_shown()
            .into_iter()
            .filter(|row| self.git.tree_marked.contains(row))
            .collect();
        if marked.is_empty() {
            self.git.tree_selected.clone().into_iter().collect()
        } else {
            marked
        }
    }

    /// Puts the tree's pick on `target` alone, forgetting every mark.
    pub(crate) fn select_tree(&mut self, target: TreeTarget) {
        self.git.tree_marked.clear();
        self.git.tree_gesture = None;
        self.git.tree_selected = Some(target);
    }

    /// The state after the tree row `target` was pressed with the modifier that picks several.
    ///
    /// A range is drawn from where the gesture was anchored, as the list of changes draws one.
    pub fn tree_row_picked(mut self, target: TreeTarget, how: Pick) -> Self {
        match how {
            Pick::Toggle => {
                self.git.tree_gesture = None;
                if self.git.tree_marked.is_empty()
                    && let Some(selected) = self
                        .git
                        .tree_selected
                        .clone()
                        .filter(|selected| *selected != target)
                {
                    self.git.tree_marked.push(selected);
                }
                if let Some(at) = self.git.tree_marked.iter().position(|held| *held == target) {
                    self.git.tree_marked.remove(at);
                } else {
                    self.git.tree_marked.push(target.clone());
                }
            }
            Pick::Range => {
                let (anchor, base) = self.git.tree_gesture.clone().unwrap_or_else(|| {
                    (
                        self.git
                            .tree_selected
                            .clone()
                            .unwrap_or_else(|| target.clone()),
                        self.git.tree_marked.clone(),
                    )
                });
                self.git.tree_gesture = Some((anchor.clone(), base.clone()));
                let shown = self.tree_shown();
                let (Some(from), Some(to)) = (
                    shown.iter().position(|row| *row == anchor),
                    shown.iter().position(|row| *row == target),
                ) else {
                    return self;
                };
                let mut marked = base;
                for row in &shown[from.min(to)..=from.max(to)] {
                    if !marked.contains(row) {
                        marked.push(row.clone());
                    }
                }
                self.git.tree_marked = marked;
            }
        }
        self.git.tree_selected = Some(target);
        self
    }

    /// The state with every row of the tree that is shown picked.
    pub fn tree_all_picked(mut self) -> Self {
        self.git.tree_gesture = None;
        self.git.tree_marked = self.tree_shown();
        self
    }

    /// The state with the menu on the tree open at `at` over `target`: on the rows picked when it is one of them, on it alone when it is not.
    pub fn with_tree_menu(mut self, target: TreeTarget, at: (f32, f32)) -> Self {
        if !target.is_root() && !self.tree_picked().contains(&target) {
            self.select_tree(target.clone());
        }
        self.git.tree_target = Some(target);
        self.with_git_menu(crate::git_menus::MenuKind::Tree, at)
    }

    /// The state after the files that were made, renamed, copied or removed are listed again.
    pub fn with_tree_stale(mut self) -> Self {
        self.reread_files();
        self
    }

    /// Asks for the files of the project again while keeping the current tree visible until the reply arrives.
    pub(crate) fn reread_files(&mut self) {
        let wanted = self.prompt_place();
        let key = wanted
            .as_ref()
            .map(|(host, directory)| format!("{host:?}:{directory}"));
        if self.files_of != key {
            self.refresh_files();
        } else if let (Some((host, directory)), Some(key)) = (wanted, key) {
            self.outbox.push(Outgoing::Files {
                key,
                host,
                directory,
                expanded: self.open_folders(),
            });
        }
    }

    /// The unfolded folders of the tree, among which the ignored ones have their contents listed only while they are open.
    pub(crate) fn open_folders(&self) -> Vec<String> {
        self.open_tree.iter().cloned().collect()
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
                self.select_tree(target.clone());
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

    /// The entries the open menu acts on: the rows picked when it was opened on one of them, the one it was opened on when not.
    pub(crate) fn tree_targets(&self) -> Vec<TreeTarget> {
        let Some(target) = self.tree_target() else {
            return Vec::new();
        };
        let picked = self.tree_picked();
        if picked.contains(&target) {
            picked
        } else {
            vec![target]
        }
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

    /// Asks whether to delete the entries the menu acts on.
    pub(crate) fn tree_delete(&mut self) {
        let targets = outermost(self.tree_targets());
        if !targets.is_empty() {
            self.ask(Prompt::Remove(targets));
        }
    }

    /// Opens the files the menu acts on, each kept in a tab of its own when there are several.
    pub(crate) fn tree_open(&mut self) {
        let files: Vec<TreeTarget> = self
            .tree_targets()
            .into_iter()
            .filter(|target| !target.folder)
            .collect();
        let several = files.len() > 1;
        for target in files {
            self.open_viewer(target.path, false);
            if several
                && let Some(viewer) = self
                    .git
                    .shown
                    .and_then(|shown| self.git.viewers.get_mut(shown))
            {
                viewer.preview = false;
            }
        }
    }

    /// Holds the entries the menu acts on, to be moved by a paste when `cut` and copied by it when not.
    pub(crate) fn tree_hold(&mut self, cut: bool) {
        let targets = outermost(self.tree_targets());
        if !targets.is_empty() {
            self.git.tree_clipboard = Some(Clipboard { targets, cut });
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
            self.git.tree_clipboard = None;
        }
        self.put_into(held.targets, into, !held.cut);
    }

    /// Moves `targets` into the folder `into`, or copies them there when `copy`, and unfolds it; an entry moved to where it already is stays put.
    fn put_into(&mut self, targets: Vec<TreeTarget>, into: String, copy: bool) {
        for target in targets {
            if copy {
                self.send_entry(EntryChange::Copy {
                    from: target.path,
                    into: into.clone(),
                });
            } else if parent_of(&target.path) != into {
                self.drop_viewer_of(&target.path);
                self.send_entry(EntryChange::Rename {
                    to: joined(&into, name_of(&target.path)),
                    from: target.path,
                });
            }
        }
        if !into.is_empty() {
            self.open_tree.insert(into);
        }
    }

    /// Whether something was cut or copied out of the tree and waits to be pasted.
    pub fn tree_holding(&self) -> bool {
        self.git.tree_clipboard.is_some()
    }

    /// The state after `shortcut` was pressed over the tree: it acts on the rows picked, as the menu on the row last picked does.
    pub fn tree_shortcut(mut self, shortcut: TreeShortcut) -> Self {
        self.git.tree_target = Some(
            self.git
                .tree_selected
                .clone()
                .unwrap_or_else(TreeTarget::root),
        );
        match shortcut {
            TreeShortcut::Cut => self.tree_hold(true),
            TreeShortcut::Copy => self.tree_hold(false),
            TreeShortcut::Paste => self.tree_paste(),
            TreeShortcut::Rename => self.tree_rename(),
            TreeShortcut::Delete => self.tree_delete(),
        }
        self
    }

    /// Whether the tree is known to hold something at `path`.
    fn tree_has(&self, path: &str) -> bool {
        self.files.iter().any(|file| within(file, path))
            || self.tree_folders.iter().any(|folder| within(folder, path))
    }

    /// The first path for `name` in the folder `into` that neither the tree nor `taken` holds: its own, then `name copy.ext`, `name copy 2.ext` and so on.
    fn free_path(&self, into: &str, name: &str, taken: &[String]) -> String {
        let (stem, extension) = split_extension(name);
        std::iter::once(name.to_owned())
            .chain((1..).map(|count| match count {
                1 => format!("{stem} copy{extension}"),
                count => format!("{stem} copy {count}{extension}"),
            }))
            .map(|candidate| joined(into, &candidate))
            .find(|path| !self.tree_has(path) && !taken.contains(path))
            .unwrap_or_else(|| joined(into, name))
    }

    /// The state after the files and folders of this machine at `paths` were dropped on, or pasted over, `over`: they are written into its folder, each under a name nothing there holds. Without `over` they go where the row last picked is.
    pub fn tree_brought_in(mut self, paths: Vec<String>, over: Option<TreeTarget>) -> Self {
        self.git.tree_drag = None;
        let Some(session) = self.review_index() else {
            return self;
        };
        let into = over
            .or_else(|| self.git.tree_selected.clone())
            .map(|target| target.directory())
            .unwrap_or_default();
        let mut taken = Vec::new();
        let mut files = Vec::new();
        for source in paths {
            let Some(name) = std::path::Path::new(&source)
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
            else {
                continue;
            };
            let path = self.free_path(&into, &name, &taken);
            taken.push(path.clone());
            files.push((source, path));
        }
        if files.is_empty() {
            return self;
        }
        if !into.is_empty() {
            self.open_tree.insert(into);
        }
        self.git.trouble = None;
        self.outbox.push(Outgoing::Import { session, files });
        self
    }

    /// The folder what is dragged would land in when the pointer is over `over`: nothing when it is over no row, or over a folder being dragged or something inside one.
    fn landing(targets: &[TreeTarget], over: Option<TreeTarget>) -> Option<String> {
        let into = over?.directory();
        let inside_dragged = targets
            .iter()
            .any(|target| target.folder && within(&into, &target.path));
        (!inside_dragged).then_some(into)
    }

    /// The state with the tree row `from` dragged to `at`, over the row `over` if it is over one: every row picked goes with it when it is one of them, and it goes alone when it is not.
    pub fn tree_dragged(
        mut self,
        from: TreeTarget,
        over: Option<TreeTarget>,
        at: (f32, f32),
    ) -> Self {
        let targets = match self.git.tree_drag.take() {
            Some(drag) if !drag.targets.is_empty() => drag.targets,
            _ => {
                if !self.tree_picked().contains(&from) {
                    self.select_tree(from.clone());
                }
                outermost(self.tree_picked())
            }
        };
        if targets.is_empty() {
            return self;
        }
        let into = Self::landing(&targets, over);
        self.git.menu = None;
        self.git.tree_drag = Some(TreeDrag { targets, into, at });
        self
    }

    /// The state with files from outside the window held at `at`, over the tree row `over` if they are over one.
    pub fn tree_hovered(mut self, over: Option<TreeTarget>, at: (f32, f32)) -> Self {
        self.git.tree_drag = Some(TreeDrag {
            targets: Vec::new(),
            into: Self::landing(&[], over),
            at,
        });
        self
    }

    /// Whether rows of the tree are being dragged.
    pub fn tree_dragging(&self) -> bool {
        self.git
            .tree_drag
            .as_ref()
            .is_some_and(|drag| !drag.targets.is_empty())
    }

    /// The state after the rows being dragged were let go: they move into the folder they were over, or are copied there when `copy`, and stay where they are when they were over none.
    pub fn tree_dropped(mut self, copy: bool) -> Self {
        if let Some(TreeDrag {
            targets,
            into: Some(into),
            ..
        }) = self.git.tree_drag.take()
        {
            self.put_into(targets, into, copy);
        }
        self
    }

    /// The state with nothing held over the tree any more, and nothing moved.
    pub fn without_tree_drag(mut self) -> Self {
        self.git.tree_drag = None;
        self
    }

    /// Copies each entry the menu acts on into the folder it is in, beside itself.
    pub(crate) fn tree_duplicate(&mut self) {
        for target in outermost(self.tree_targets()) {
            let into = parent_of(&target.path).to_owned();
            self.send_entry(EntryChange::Copy {
                from: target.path,
                into,
            });
        }
    }

    /// Puts the paths of the entries the menu acts on on the clipboard, one a line, from the root of the project or in full.
    pub(crate) fn tree_copy_path(&mut self, relative: bool) {
        let Some(session) = self.review_session() else {
            return;
        };
        let lines: Vec<String> = self
            .tree_targets()
            .into_iter()
            .map(|target| match (relative, target.is_root()) {
                (true, true) => ".".to_owned(),
                (true, false) => target.path,
                (false, true) => session.directory.clone(),
                (false, false) => joined(session.directory.trim_end_matches('/'), &target.path),
            })
            .collect();
        if !lines.is_empty() {
            self.outbox.push(Outgoing::Copy {
                text: lines.join("\n"),
            });
        }
    }

    /// Shows the entry the menu was opened on in the file manager of this machine.
    pub(crate) fn tree_reveal(&mut self) {
        if let (Some(target), Some(session)) = (self.tree_target(), self.review_index()) {
            self.outbox.push(Outgoing::RevealFile {
                session,
                path: target.path,
            });
        }
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

#[cfg(test)]
mod tests;
