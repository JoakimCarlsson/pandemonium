//! Paths arranged as a tree of folders and files.

/// A folder of the tree: the folders and files in it, and which of the paths it was built from lie beneath it.
#[derive(Clone, Debug, PartialEq)]
pub struct Folder {
    /// What the folder is called; a folder that held only another folder is called by both names, joined by a slash.
    pub name: String,
    /// Where the folder is, from the root.
    pub path: String,
    /// Its place among the folders of the tree when they are counted from the top down, whether open or not.
    pub position: usize,
    /// The folders and files in it.
    pub entries: Vec<Entry>,
    /// The positions, in the list the tree was built from, of every file beneath it.
    pub files: Vec<usize>,
}

/// One row of the tree.
#[derive(Clone, Debug, PartialEq)]
pub enum Entry {
    /// A folder and what is in it.
    Folder(Folder),
    /// A file: its name and its position in the list the tree was built from.
    File {
        /// What the file is called.
        name: String,
        /// Its position in the list the tree was built from.
        index: usize,
    },
}

/// How the tree is laid out.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Layout {
    /// Whether a folder holding only another folder is joined with it into one row.
    pub compact: bool,
    /// Whether folders come before files, each in alphabetical order, in place of the order the paths came in.
    pub sorted: bool,
}

/// A folder or file while the tree is being put together.
struct Draft {
    /// What it is called.
    name: String,
    /// Where it is.
    path: String,
    /// Its position in the list, when it is a file.
    file: Option<usize>,
    /// What is in it.
    children: Vec<Draft>,
}

impl Draft {
    /// A folder or file called `name` at `path`.
    fn new(name: &str, path: String, file: Option<usize>) -> Self {
        Self {
            name: name.to_owned(),
            path,
            file,
            children: Vec::new(),
        }
    }

    /// Every file beneath it, or itself when it is one.
    fn files(&self) -> Vec<usize> {
        match self.file {
            Some(index) => vec![index],
            None => self.children.iter().flat_map(Draft::files).collect(),
        }
    }

    /// Joins every folder that holds only another folder with that folder, unless it is one of `stops`, whose row always ends at it.
    fn compacted(mut self, stops: &[&str]) -> Self {
        self.children = self
            .children
            .into_iter()
            .map(|child| child.compacted(stops))
            .collect();
        if self.file.is_none()
            && !stops.contains(&self.path.as_str())
            && let [only] = self.children.as_slice()
            && only.file.is_none()
        {
            let mut only = self.children.remove(0);
            only.name = format!("{}/{}", self.name, only.name);
            return only;
        }
        self
    }

    /// Puts folders before files, each in alphabetical order, all the way down.
    fn sorted(mut self) -> Self {
        self.children = self.children.into_iter().map(Draft::sorted).collect();
        self.children.sort_by(|a, b| {
            a.file
                .is_some()
                .cmp(&b.file.is_some())
                .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
        });
        self
    }

    /// The finished entry, numbering the folders in `counter` from the top down.
    fn finish(self, counter: &mut usize) -> Entry {
        if let Some(index) = self.file {
            return Entry::File {
                name: self.name,
                index,
            };
        }
        let position = *counter;
        *counter += 1;
        let files = self.files();
        let entries = self
            .children
            .into_iter()
            .map(|child| child.finish(counter))
            .collect();
        Entry::Folder(Folder {
            name: self.name,
            path: self.path,
            position,
            entries,
            files,
        })
    }
}

/// The tree of `paths`, split at slashes, laid out as `layout` says; a path that ends in a slash is a folder with nothing in it.
pub fn build(paths: &[&str], layout: Layout) -> Vec<Entry> {
    build_stopping(paths, layout, &[])
}

/// The tree of `paths` laid out as `layout` says, no folder of `stops` joined with the one it holds.
pub fn build_stopping(paths: &[&str], layout: Layout, stops: &[&str]) -> Vec<Entry> {
    let mut root = Draft::new("", String::new(), None);
    for (index, path) in paths.iter().enumerate() {
        let parts: Vec<&str> = path.split('/').filter(|part| !part.is_empty()).collect();
        let mut at = &mut root;
        for (depth, part) in parts.iter().enumerate() {
            let here = parts[..=depth].join("/");
            let file = depth + 1 == parts.len() && !path.ends_with('/');
            let found = at
                .children
                .iter()
                .position(|child| child.path == here && child.file.is_some() == file);
            let found = found.unwrap_or_else(|| {
                at.children
                    .push(Draft::new(part, here, file.then_some(index)));
                at.children.len() - 1
            });
            at = &mut at.children[found];
        }
    }
    if layout.sorted {
        root = root.sorted();
    }
    let children = root
        .children
        .into_iter()
        .map(|child| {
            if layout.compact {
                child.compacted(stops)
            } else {
                child
            }
        })
        .collect::<Vec<_>>();
    let mut counter = 0;
    children
        .into_iter()
        .map(|child| child.finish(&mut counter))
        .collect()
}

/// Every folder of `entries`, open or not, from the top down.
pub fn folders(entries: &[Entry]) -> Vec<&Folder> {
    let mut found = Vec::new();
    for entry in entries {
        if let Entry::Folder(folder) = entry {
            found.push(folder);
            found.extend(folders(&folder.entries));
        }
    }
    found
}
