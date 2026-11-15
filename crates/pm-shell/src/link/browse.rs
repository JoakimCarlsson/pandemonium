//! The link's part in the right panel's Files and Search tabs: lists the
//! folder the open session works in, changes its entries through the
//! server, and searches its files.

use pm_protocol::{Request, Response};

use super::{Ask, Link, expand_home};
use crate::file_tree::{EditKind, FileTree, TreeAction, TreeEdit, join, name_of, parent_of};
use crate::model::RightTab;
use crate::search::{SearchAction, SearchHit};

/// The most files the tree asks the server for.
const FILES_LIMIT: usize = 20_000;

impl Link {
    /// The folder the Files and Search tabs work in: the open session's,
    /// or the first project's, as the server knows it.
    pub(super) fn browse_root(&self) -> Option<String> {
        let session = self
            .watched
            .and_then(|id| self.sessions.iter().find(|info| info.session == id))
            .filter(|info| !info.directory.is_empty())
            .map(|info| info.directory.clone());
        session
            .or_else(|| self.paths.first().cloned())
            .map(|path| expand_home(&path))
    }

    /// Chooses `tab` of the right panel, listing the folder when it is Files.
    pub fn choose_right_tab(&mut self, tab: RightTab) {
        self.workspace.right_tab = tab;
        self.refresh_browse();
    }

    /// Lists the folder again when the Files tab shows and the folder changed.
    pub(super) fn refresh_browse(&mut self) {
        let root = self.browse_root();
        let tree = &self.workspace.tree;
        let stale = root.as_deref() != Some(tree.root.as_str()) || !tree.loaded;
        if self.workspace.right_tab == RightTab::Files && stale {
            self.load_files();
        }
    }

    /// Asks the server for the files of the folder, keeping what is unfolded
    /// when the folder is the one already listed.
    pub(super) fn load_files(&mut self) {
        let Some(root) = self.browse_root() else {
            return;
        };
        if self.workspace.tree.root != root {
            self.workspace.tree = FileTree {
                root: root.clone(),
                ..FileTree::default()
            };
        }
        let tree = &self.workspace.tree;
        let expanded = tree
            .expanded
            .iter()
            .filter(|folder| {
                tree.ignored.iter().any(|ignored| {
                    let ignored = ignored.trim_end_matches('/');
                    *folder == ignored || folder.starts_with(&format!("{ignored}/"))
                })
            })
            .cloned()
            .collect();
        self.ask(
            Request::Files {
                directory: root.clone(),
                limit: FILES_LIMIT,
                expanded,
            },
            Ask::Files(root),
        );
    }

    /// Does what the person asked of the file tree.
    pub fn tree(&mut self, action: TreeAction) {
        let tree = &mut self.workspace.tree;
        match action {
            TreeAction::Toggle(path) => {
                if !tree.expanded.remove(&path) {
                    let ignored = tree.ignored.iter().any(|entry| {
                        entry.trim_end_matches('/') == path
                            || path.starts_with(&format!("{}/", entry.trim_end_matches('/')))
                    });
                    tree.expanded.insert(path.clone());
                    if ignored {
                        self.load_files();
                    }
                }
                self.workspace.tree.selected = Some(path);
            }
            TreeAction::Select(path) => tree.selected = Some(path),
            TreeAction::Menu(path) => {
                if let Some(path) = &path {
                    tree.selected = Some(path.clone()).filter(|path| !path.is_empty());
                }
                tree.menu = path;
            }
            TreeAction::StartCreate { parent, folder } => {
                tree.menu = None;
                if !parent.is_empty() {
                    tree.expanded.insert(parent.clone());
                }
                tree.editing = Some(TreeEdit {
                    kind: EditKind::Create { parent, folder },
                    text: String::new(),
                });
            }
            TreeAction::StartRename(path) => {
                tree.menu = None;
                tree.editing = Some(TreeEdit {
                    text: name_of(&path).to_owned(),
                    kind: EditKind::Rename { path },
                });
            }
            TreeAction::EditText(text) => {
                if let Some(editing) = &mut tree.editing {
                    editing.text = text;
                }
            }
            TreeAction::Commit => self.commit_edit(),
            TreeAction::Cancel => {
                tree.editing = None;
                tree.menu = None;
                tree.removing = None;
            }
            TreeAction::AskRemove(path) => {
                tree.menu = None;
                tree.removing = Some(path);
            }
            TreeAction::Remove(path) => {
                tree.removing = None;
                let directory = tree.root.clone();
                self.change_entry(Request::RemoveEntry { directory, path });
            }
            TreeAction::Duplicate(from) => {
                tree.menu = None;
                let directory = tree.root.clone();
                let into = parent_of(&from).to_owned();
                self.change_entry(Request::CopyEntry {
                    directory,
                    from,
                    into,
                });
            }
            TreeAction::Move { from, into } => {
                let inside = into == from || into.starts_with(&format!("{from}/"));
                if inside || parent_of(&from) == into {
                    return;
                }
                let directory = tree.root.clone();
                let to = join(&into, name_of(&from));
                self.change_entry(Request::RenameEntry {
                    directory,
                    from,
                    to,
                });
            }
            TreeAction::Import { into, file } => {
                let directory = tree.root.clone();
                let Some(name) = file
                    .file_name()
                    .map(|name| name.to_string_lossy().into_owned())
                else {
                    return;
                };
                match std::fs::read(&file) {
                    Ok(data) => self.change_entry(Request::ImportFile {
                        directory,
                        path: join(&into, &name),
                        data,
                    }),
                    Err(error) => {
                        self.workspace.status = Some(format!("Could not read {name}: {error}"))
                    }
                }
            }
            TreeAction::Refresh => self.load_files(),
            TreeAction::CollapseAll => tree.expanded.clear(),
        }
    }

    /// Makes or renames the entry the name typed is for.
    fn commit_edit(&mut self) {
        let tree = &mut self.workspace.tree;
        let Some(editing) = tree.editing.take() else {
            return;
        };
        let name = editing.text.trim().to_owned();
        if name.is_empty() {
            return;
        }
        let directory = tree.root.clone();
        let request = match editing.kind {
            EditKind::Create { parent, folder } => {
                let path = join(&parent, &name);
                tree.selected = Some(path.clone());
                Request::CreateEntry {
                    directory,
                    path,
                    folder,
                }
            }
            EditKind::Rename { path } => {
                let to = join(parent_of(&path), &name);
                if to == path {
                    return;
                }
                tree.selected = Some(to.clone());
                Request::RenameEntry {
                    directory,
                    from: path,
                    to,
                }
            }
        };
        self.change_entry(request);
    }

    /// Sends `request`, which changes an entry, and lists the folder again once it is done.
    fn change_entry(&mut self, request: Request) {
        if self.workspace.tree.root.is_empty() {
            return;
        }
        self.ask(request, Ask::Entry);
    }

    /// Does what the person asked of the Search tab.
    pub fn search(&mut self, action: SearchAction) {
        let search = &mut self.workspace.search;
        match action {
            SearchAction::Query(query) => {
                search.query = query;
                return;
            }
            SearchAction::Run => {}
            SearchAction::Regex => search.regex = !search.regex,
            SearchAction::Case => search.case = !search.case,
            SearchAction::Edit => {
                self.search_edit();
                return;
            }
            SearchAction::Save(path) => {
                self.save_excerpts(path);
                return;
            }
        }
        let query = search.query.clone();
        if query.trim().is_empty() {
            search.hits.clear();
            search.searched = None;
            search.error = None;
            return;
        }
        let Some(directory) = self.browse_root() else {
            return;
        };
        let search = &mut self.workspace.search;
        search.searching = Some(query.clone());
        let (regex, case) = (search.regex, search.case);
        self.ask(
            Request::SearchFiles {
                directory,
                query: query.clone(),
                regex,
                case,
            },
            Ask::Search(query),
        );
    }

    /// Handles the `response` to a request the Files or Search tab or the quick input made for `ask`.
    pub(super) fn answer_browse(&mut self, ask: Ask, response: Response) {
        match (ask, response) {
            (
                Ask::Files(root),
                Response::Files {
                    paths,
                    folders,
                    ignored,
                },
            ) if root == self.workspace.tree.root => {
                let tree = &mut self.workspace.tree;
                tree.paths = paths;
                tree.folders = folders;
                tree.ignored = ignored;
                tree.loaded = true;
            }
            (Ask::Entry, Response::Failed { message }) => {
                self.workspace.status = Some(message);
                self.load_files();
            }
            (Ask::Entry, _) => self.load_files(),
            (Ask::Search(query), Response::Definitions { locations }) => {
                let search = &mut self.workspace.search;
                if search.searching.as_deref() != Some(query.as_str()) {
                    return;
                }
                search.searching = None;
                search.error = None;
                search.hits = locations
                    .into_iter()
                    .map(|location| {
                        let text = location.line.unwrap_or_default();
                        let column = char_column(&text, location.span.start.character);
                        SearchHit {
                            path: location.path,
                            line: location.span.start.line as usize + 1,
                            column,
                            length: 0,
                            text,
                        }
                    })
                    .collect();
                let measured: Vec<usize> = search
                    .hits
                    .iter()
                    .map(|hit| search.match_length(&hit.text, hit.column))
                    .collect();
                for (hit, length) in search.hits.iter_mut().zip(measured) {
                    hit.length = length;
                }
                search.searched = Some(query);
                if search.editing {
                    self.read_excerpts();
                }
            }
            (Ask::Search(query), Response::Failed { message }) => {
                let search = &mut self.workspace.search;
                if search.searching.as_deref() == Some(query.as_str()) {
                    search.searching = None;
                    search.hits.clear();
                    search.error = Some(message);
                }
            }
            (ask @ (Ask::Quick(_) | Ask::Branches), response) => self.answer_quick(ask, response),
            _ => {}
        }
    }
}

/// The place in characters of the UTF-16 offset `units` into `text`.
pub(super) fn char_column(text: &str, units: u32) -> usize {
    let mut counted = 0_u32;
    for (place, letter) in text.chars().enumerate() {
        if counted >= units {
            return place;
        }
        counted += letter.len_utf16() as u32;
    }
    text.chars().count()
}
