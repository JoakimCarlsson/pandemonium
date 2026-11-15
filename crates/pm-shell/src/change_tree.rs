//! The changed files of a group arranged as a folder tree: folders first
//! and then files, each by name, with a folder that holds only one folder
//! joined with it into one row, as `src/link`.

use crate::git::ChangedFile;

/// A folder of a tree of changed files, with the folders and files right under it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ChangeFolder<'a> {
    /// Where the folder is from the root of the worktree, empty at the root.
    pub path: String,
    /// What its row says: its name, or the names of the folders joined into it.
    pub name: String,
    /// The folders right under it, by name.
    pub folders: Vec<ChangeFolder<'a>>,
    /// The files right under it, by name.
    pub files: Vec<&'a ChangedFile>,
}

/// One row of a tree of changed files, as the list shows it.
#[derive(Clone, Debug, PartialEq)]
pub enum ChangeEntry<'a, 'b> {
    /// A folder, `depth` folders down.
    Folder {
        /// The folder.
        folder: &'b ChangeFolder<'a>,
        /// How many folders hold its row.
        depth: usize,
    },
    /// A file, `depth` folders down.
    File {
        /// The file.
        change: &'a ChangedFile,
        /// How many folders hold its row.
        depth: usize,
    },
}

impl<'a> ChangeFolder<'a> {
    /// The tree of `changes`, rooted at the worktree's root.
    pub fn of(changes: &[&'a ChangedFile]) -> Self {
        let mut root = ChangeFolder::default();
        for change in changes {
            let folders: Vec<&str> = change
                .folder()
                .split('/')
                .filter(|part| !part.is_empty())
                .collect();
            root.insert(&folders, change);
        }
        root.arrange();
        root
    }

    /// Puts `change` in the folder reached through `folders` below this one, making the folders it lacks.
    fn insert(&mut self, folders: &[&str], change: &'a ChangedFile) {
        let Some((first, rest)) = folders.split_first() else {
            self.files.push(change);
            return;
        };
        let place = match self.folders.iter().position(|folder| folder.name == *first) {
            Some(place) => place,
            None => {
                let path = if self.path.is_empty() {
                    (*first).to_owned()
                } else {
                    format!("{}/{first}", self.path)
                };
                self.folders.push(ChangeFolder {
                    path,
                    name: (*first).to_owned(),
                    ..ChangeFolder::default()
                });
                self.folders.len() - 1
            }
        };
        self.folders[place].insert(rest, change);
    }

    /// Joins each folder below that holds only one folder with it, and sorts every level by name.
    fn arrange(&mut self) {
        for folder in &mut self.folders {
            while folder.files.is_empty() && folder.folders.len() == 1 {
                let only = folder.folders.remove(0);
                folder.name = format!("{}/{}", folder.name, only.name);
                folder.path = only.path;
                folder.folders = only.folders;
                folder.files = only.files;
            }
            folder.arrange();
        }
        self.folders.sort_by(|one, other| one.name.cmp(&other.name));
        self.files
            .sort_by(|one, other| one.name().cmp(other.name()));
    }

    /// Every file under the folder, at any depth, in the order the list shows them.
    pub fn all_files(&self) -> Vec<&'a ChangedFile> {
        let mut found = Vec::new();
        for folder in &self.folders {
            found.extend(folder.all_files());
        }
        found.extend(self.files.iter().copied());
        found
    }

    /// The rows of the tree under the folder, top to bottom, leaving out
    /// what is under a folder for which `folded` says true.
    pub fn rows<'b>(&'b self, folded: &dyn Fn(&str) -> bool) -> Vec<ChangeEntry<'a, 'b>> {
        let mut rows = Vec::new();
        self.collect(0, folded, &mut rows);
        rows
    }

    /// Adds to `rows` the rows under the folder, the first ones `depth` folders down.
    fn collect<'b>(
        &'b self,
        depth: usize,
        folded: &dyn Fn(&str) -> bool,
        rows: &mut Vec<ChangeEntry<'a, 'b>>,
    ) {
        for folder in &self.folders {
            rows.push(ChangeEntry::Folder { folder, depth });
            if !folded(&folder.path) {
                folder.collect(depth + 1, folded, rows);
            }
        }
        for change in &self.files {
            rows.push(ChangeEntry::File { change, depth });
        }
    }

    /// Lines added and removed by every file under the folder.
    pub fn tally(&self) -> (usize, usize) {
        self.all_files()
            .iter()
            .fold((0, 0), |(added, removed), change| {
                (added + change.added, removed + change.removed)
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A changed file at `path` adding `added` lines.
    fn change(path: &str, added: usize) -> ChangedFile {
        ChangedFile {
            path: path.to_owned(),
            added,
            ..ChangedFile::default()
        }
    }

    /// Folders holding only a folder are joined, folders come before files, and folding hides what is under.
    #[test]
    fn builds_a_compact_sorted_tree() {
        let changes = [
            change("z.rs", 1),
            change("src/link/b.rs", 2),
            change("src/link/a.rs", 3),
            change("crates/x/src/lib.rs", 4),
        ];
        let listed: Vec<&ChangedFile> = changes.iter().collect();
        let tree = ChangeFolder::of(&listed);
        let names: Vec<&str> = tree.folders.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(names, ["crates/x/src", "src/link"]);
        assert_eq!(tree.folders[1].path, "src/link");
        assert_eq!(tree.folders[1].tally(), (5, 0));
        let order: Vec<&str> = tree
            .all_files()
            .iter()
            .map(|change| change.path.as_str())
            .collect();
        assert_eq!(
            order,
            [
                "crates/x/src/lib.rs",
                "src/link/a.rs",
                "src/link/b.rs",
                "z.rs"
            ]
        );
        assert_eq!(tree.rows(&|_| false).len(), 6);
        assert_eq!(tree.rows(&|path| path == "src/link").len(), 4);
    }
}
