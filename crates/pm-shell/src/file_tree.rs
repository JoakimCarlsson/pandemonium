//! The file tree of the right panel's Files tab: the entries the server
//! listed for a folder, which folders are unfolded, the entry chosen, the
//! menu and the name being typed for a new or renamed entry, and the rows
//! the tree shows with the marks of the changes git sees.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use crate::git::ChangedFile;

/// The mark a folder holding changed files shows.
pub const FOLDER_MARK: char = '•';

/// The file tree of one folder.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FileTree {
    /// The folder listed, as the server knows it.
    pub root: String,
    /// The files, from the root of the folder.
    pub paths: Vec<String>,
    /// The folders that hold no tracked file, from the root of the folder.
    pub folders: Vec<String>,
    /// Which of `paths` and `folders` git ignores.
    pub ignored: Vec<String>,
    /// The folders unfolded.
    pub expanded: BTreeSet<String>,
    /// The entry chosen.
    pub selected: Option<String>,
    /// The entries chosen beside `selected` with Ctrl or Shift.
    pub picked: BTreeSet<String>,
    /// The entries cut or copied, waiting to be pasted.
    pub held: Option<Held>,
    /// The entry whose menu is open; the empty path for the folder itself.
    pub menu: Option<String>,
    /// The name being typed for a new or renamed entry.
    pub editing: Option<TreeEdit>,
    /// The entry the reader is asked to confirm taking off the disk.
    pub removing: Option<String>,
    /// Whether the server answered for `root`.
    pub loaded: bool,
}

/// Entries cut or copied in the tree, waiting to be pasted.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Held {
    /// The entries, from the root of the folder.
    pub paths: Vec<String>,
    /// Whether pasting moves them rather than copying them.
    pub cut: bool,
}

/// How a click chooses an entry of the tree.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PickHow {
    /// Adds the entry to the entries chosen, or takes it out.
    Toggle,
    /// Chooses every row from the entry chosen last to this one.
    Range,
}

/// A name being typed in the tree.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TreeEdit {
    /// What the name is for.
    pub kind: EditKind,
    /// The name as typed.
    pub text: String,
}

/// What a name typed in the tree is for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EditKind {
    /// A new file or folder inside `parent`.
    Create {
        /// The folder it goes in; empty for the root.
        parent: String,
        /// Whether it is a folder.
        folder: bool,
    },
    /// A new name for the entry at `path`.
    Rename {
        /// The entry renamed.
        path: String,
    },
}

/// One row the tree shows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TreeRow {
    /// Its path from the root of the folder.
    pub path: String,
    /// Its name.
    pub name: String,
    /// How many folders hold it.
    pub depth: usize,
    /// Whether it is a folder.
    pub folder: bool,
    /// Whether it is a folder that is unfolded.
    pub expanded: bool,
    /// Whether git ignores it.
    pub ignored: bool,
    /// Its change mark: a file's status letter, or [`FOLDER_MARK`] for a folder holding changes.
    pub mark: Option<char>,
    /// Whether it is the entry chosen.
    pub selected: bool,
}

/// What the person does to the tree.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TreeAction {
    /// Folds or unfolds the folder.
    Toggle(String),
    /// Chooses the entry.
    Select(String),
    /// Chooses the entry along with others, as Ctrl or Shift and a click do.
    Pick(String, PickHow),
    /// Holds the entries chosen to be moved, when `cut`, or copied by the next paste.
    Hold {
        /// Whether pasting moves them.
        cut: bool,
    },
    /// Moves or copies the entries held into the folder chosen.
    Paste,
    /// Starts renaming the entry chosen.
    RenameChosen,
    /// Asks the reader to confirm taking the entries chosen off the disk.
    RemoveChosen,
    /// Opens the menu of the entry, or closes the open menu.
    Menu(Option<String>),
    /// Starts typing the name of a new entry.
    StartCreate {
        /// The folder it goes in; empty for the root.
        parent: String,
        /// Whether it is a folder.
        folder: bool,
    },
    /// Starts typing a new name for the entry.
    StartRename(String),
    /// Changes the name being typed.
    EditText(String),
    /// Makes the entry or renames it with the name typed.
    Commit,
    /// Stops typing a name.
    Cancel,
    /// Asks the reader to confirm taking the entry off the disk.
    AskRemove(String),
    /// Takes the entry off the disk.
    Remove(String),
    /// Copies the entry beside itself.
    Duplicate(String),
    /// Moves the entry into the folder.
    Move {
        /// The entry moved.
        from: String,
        /// The folder it goes into; empty for the root.
        into: String,
    },
    /// Brings a file or folder from the reader's machine into the folder.
    Import {
        /// The folder it goes into; empty for the root.
        into: String,
        /// The file on the reader's machine.
        file: PathBuf,
    },
    /// Lists the folder again.
    Refresh,
    /// Folds every folder.
    CollapseAll,
}

impl FileTree {
    /// The rows shown: the entries of the root and of every unfolded folder,
    /// folders first and each group by name, marked with `changes`.
    pub fn rows(&self, changes: &[ChangedFile]) -> Vec<TreeRow> {
        let mut children = Children::new();
        for folder in &self.folders {
            add_folder(&mut children, folder);
        }
        for path in &self.paths {
            let parent = parent_of(path);
            add_folder(&mut children, parent);
            children
                .entry(parent.to_owned())
                .or_default()
                .1
                .insert(path.clone());
        }
        let mut rows = Vec::new();
        self.walk(&children, "", 0, changes, &mut rows);
        rows
    }

    /// Adds the rows of the entries inside `folder`, `depth` deep, to `rows`.
    fn walk(
        &self,
        children: &Children,
        folder: &str,
        depth: usize,
        changes: &[ChangedFile],
        rows: &mut Vec<TreeRow>,
    ) {
        let Some((folders, files)) = children.get(folder) else {
            return;
        };
        for path in folders {
            let expanded = self.expanded.contains(path);
            let inside = format!("{path}/");
            rows.push(TreeRow {
                path: path.clone(),
                name: name_of(path).to_owned(),
                depth,
                folder: true,
                expanded,
                ignored: self.is_ignored(path),
                mark: changes
                    .iter()
                    .any(|change| change.path.starts_with(&inside))
                    .then_some(FOLDER_MARK),
                selected: self.is_chosen(path),
            });
            if expanded {
                self.walk(children, path, depth + 1, changes, rows);
            }
        }
        for path in files {
            rows.push(TreeRow {
                path: path.clone(),
                name: name_of(path).to_owned(),
                depth,
                folder: false,
                expanded: false,
                ignored: self.is_ignored(path),
                mark: changes
                    .iter()
                    .find(|change| &change.path == path)
                    .map(|change| change.status),
                selected: self.is_chosen(path),
            });
        }
    }

    /// Whether the entry at `path` is among those chosen.
    pub fn is_chosen(&self, path: &str) -> bool {
        self.selected.as_deref() == Some(path) || self.picked.contains(path)
    }

    /// The entries chosen, without those inside another chosen folder, in path order.
    pub fn chosen(&self) -> Vec<String> {
        let mut all: BTreeSet<String> = self.picked.clone();
        all.extend(self.selected.clone());
        let all: Vec<String> = all.into_iter().filter(|path| !path.is_empty()).collect();
        all.iter()
            .filter(|path| {
                !all.iter()
                    .any(|outer| outer != *path && path.starts_with(&format!("{outer}/")))
            })
            .cloned()
            .collect()
    }

    /// The entries an action on `path` covers: every entry chosen when `path` is one of them, else `path` alone.
    pub fn with_chosen(&self, path: String) -> Vec<String> {
        if self.is_chosen(&path) && !self.picked.is_empty() {
            self.chosen()
        } else {
            vec![path]
        }
    }

    /// Whether an entry, file or folder, is at `path`.
    pub fn has(&self, path: &str) -> bool {
        self.paths.iter().any(|file| file == path) || self.is_folder(path)
    }

    /// The first path for `name` inside `into` that neither the tree nor `taken` holds: its own, then `name copy.ext`, `name copy 2.ext` and so on.
    pub fn free_path(&self, into: &str, name: &str, taken: &[String]) -> String {
        let (stem, extension) = split_extension(name);
        (0..)
            .map(|count| {
                let name = match count {
                    0 => name.to_owned(),
                    1 => format!("{stem} copy{extension}"),
                    count => format!("{stem} copy {count}{extension}"),
                };
                join(into, &name)
            })
            .find(|path| !self.has(path) && !taken.contains(path))
            .unwrap_or_else(|| join(into, name))
    }

    /// The folder new and pasted entries go in: the folder chosen, the folder of the file chosen, or the root.
    pub fn target_folder(&self) -> String {
        match &self.selected {
            Some(path) if self.is_folder(path) => path.clone(),
            Some(path) => parent_of(path).to_owned(),
            None => String::new(),
        }
    }

    /// Whether git ignores the entry at `path` or a folder holding it.
    fn is_ignored(&self, path: &str) -> bool {
        self.ignored.iter().any(|ignored| {
            let ignored = ignored.trim_end_matches('/');
            path == ignored || path.starts_with(&format!("{ignored}/"))
        })
    }

    /// Whether the entry at `path` is a folder.
    pub fn is_folder(&self, path: &str) -> bool {
        let inside = format!("{path}/");
        self.folders
            .iter()
            .any(|folder| folder.trim_end_matches('/') == path)
            || self.paths.iter().any(|file| file.starts_with(&inside))
    }
}

/// The children of each folder, files and folders apart.
type Children = BTreeMap<String, (BTreeSet<String>, BTreeSet<String>)>;

/// Records `folder` and every folder above it in `children`.
fn add_folder(children: &mut Children, folder: &str) {
    let mut path = folder.trim_end_matches('/');
    while !path.is_empty() {
        let parent = parent_of(path);
        children
            .entry(parent.to_owned())
            .or_default()
            .0
            .insert(path.to_owned());
        path = parent;
    }
}

/// `name` split before its extension, the dot going with the extension; a leading dot is part of the stem.
fn split_extension(name: &str) -> (&str, &str) {
    match name.rfind('.') {
        Some(dot) if dot > 0 => name.split_at(dot),
        _ => (name, ""),
    }
}

/// Why `name` cannot name an entry, when it cannot.
pub fn name_problem(name: &str) -> Option<&'static str> {
    let name = name.trim();
    if name.is_empty() {
        Some("A name is needed.")
    } else if name == "." || name == ".." {
        Some("That name is reserved.")
    } else if name.contains('/') || name.contains('\\') {
        Some("A name can't hold a slash.")
    } else if name.chars().any(char::is_control) {
        Some("A name can't hold control characters.")
    } else {
        None
    }
}

/// The folder holding `path`, or the empty path for the root.
pub fn parent_of(path: &str) -> &str {
    path.rsplit_once('/').map_or("", |(parent, _)| parent)
}

/// The last part of `path`.
pub fn name_of(path: &str) -> &str {
    path.rsplit_once('/').map_or(path, |(_, name)| name)
}

/// `name` inside `folder`, or `name` alone for the root.
pub fn join(folder: &str, name: &str) -> String {
    if folder.is_empty() {
        name.to_owned()
    } else {
        format!("{folder}/{name}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A tree over a few files with `src` unfolded.
    fn tree() -> FileTree {
        FileTree {
            paths: vec![
                "Cargo.toml".into(),
                "src/main.rs".into(),
                "src/ui/view.rs".into(),
                "README.md".into(),
            ],
            folders: vec!["target/".into()],
            ignored: vec!["target/".into()],
            expanded: ["src".to_owned()].into(),
            ..FileTree::default()
        }
    }

    #[test]
    fn lists_folders_first_and_unfolds_only_expanded() {
        let names: Vec<_> = tree()
            .rows(&[])
            .into_iter()
            .map(|row| (row.depth, row.name))
            .collect();
        assert_eq!(
            names,
            vec![
                (0, "src".to_owned()),
                (1, "ui".to_owned()),
                (1, "main.rs".to_owned()),
                (0, "target".to_owned()),
                (0, "Cargo.toml".to_owned()),
                (0, "README.md".to_owned()),
            ]
        );
    }

    /// Free names skip the files already there, keeping the extension.
    #[test]
    fn free_paths_do_not_overwrite() {
        let tree = tree();
        assert_eq!(tree.free_path("src", "lib.rs", &[]), "src/lib.rs");
        assert_eq!(tree.free_path("src", "main.rs", &[]), "src/main copy.rs");
        assert_eq!(
            tree.free_path("src", "main.rs", &["src/main copy.rs".to_owned()]),
            "src/main copy 2.rs"
        );
        assert_eq!(tree.free_path("", "src", &[]), "src copy");
    }

    /// The entries chosen leave out those inside a chosen folder, and every one shows chosen.
    #[test]
    fn chosen_entries_are_outermost() {
        let mut tree = tree();
        tree.selected = Some("src/main.rs".into());
        tree.picked = ["src".to_owned(), "README.md".to_owned()].into();
        assert_eq!(
            tree.chosen(),
            vec!["README.md".to_owned(), "src".to_owned()]
        );
        assert!(tree.rows(&[]).iter().filter(|row| row.selected).count() == 3);
    }

    /// Names with slashes or nothing in them are refused.
    #[test]
    fn names_are_checked() {
        assert!(name_problem("ok.rs").is_none());
        assert!(name_problem(" ").is_some());
        assert!(name_problem("a/b").is_some());
        assert!(name_problem("..").is_some());
    }

    #[test]
    fn marks_changed_files_and_their_folders() {
        let changes = [ChangedFile {
            path: "src/ui/view.rs".into(),
            status: 'M',
            ..ChangedFile::default()
        }];
        let mut tree = tree();
        tree.expanded.insert("src/ui".into());
        let rows = tree.rows(&changes);
        let mark = |name: &str| rows.iter().find(|row| row.name == name).unwrap().mark;
        assert_eq!(mark("src"), Some(FOLDER_MARK));
        assert_eq!(mark("ui"), Some(FOLDER_MARK));
        assert_eq!(mark("view.rs"), Some('M'));
        assert_eq!(mark("main.rs"), None);
        assert!(
            rows.iter()
                .find(|row| row.name == "target")
                .unwrap()
                .ignored
        );
    }
}
