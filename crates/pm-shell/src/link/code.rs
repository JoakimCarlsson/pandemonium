//! The link's part in the open files: reading a file from the server for
//! its tab, fetching the grammar its language is highlighted with, saving
//! it, and the tab strip's actions.

use pm_editor::ServerAsk;
use pm_protocol::{Request, Response};

use super::{Ask, Link, expand_home};
use crate::open_files::{FileBody, SessionTab, TabAction, Unsaved, is_picture};

/// What a request about the open files was for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum CodeAsk {
    /// The text of the file at this path in this folder.
    Text(String, String),
    /// The bytes of the picture at this path in this folder.
    Picture(String, String),
    /// The grammar of the language with this name.
    Grammar(String),
    /// Saving the file at this path in this folder.
    Save(String, String),
    /// What the last commit holds of the file at this path in this folder.
    Base(String, String),
    /// What the folder holds of the file at this path in this folder, to compare its editor with.
    Compare(String, String),
    /// What the folder holds of the open file at this path in this folder
    /// after something outside the editor changed the folder.
    Reload(String, String),
}

impl Link {
    /// Opens the file at `path` in the open session's folder in the preview
    /// tab, reading it from the server; a path inside that folder may be
    /// given whole.
    pub fn open_file(&mut self, path: String) {
        self.open_file_kept(path, false);
    }

    /// Opens the file at `path` as [`Link::open_file`] does, with the caret
    /// on `line`, counted from 1.
    pub fn open_location(&mut self, path: String, line: usize) {
        self.open_file(path);
        if let SessionTab::File(index) = self.workspace.open_files.tab
            && let Some(file) = self.workspace.open_files.files.get_mut(index)
        {
            file.go_to(line);
        }
    }

    /// Opens the file at `path` as [`Link::open_file`] does, in a tab of its
    /// own when `keep` rather than the preview tab.
    pub fn open_file_kept(&mut self, path: String, keep: bool) {
        let directory = expand_home(self.dock.place());
        let relative = path
            .strip_prefix(directory.trim_end_matches('/'))
            .map(|rest| rest.trim_start_matches('/').to_owned())
            .unwrap_or(path);
        let files = &mut self.workspace.open_files;
        files.vim = self.settings.editor.vim_mode;
        if files.open(&directory, &relative, keep).is_some() {
            self.read_file(directory, relative);
        }
    }

    /// Carries out `action` the tab strip or its menu asked for.
    pub fn file_tab(&mut self, mut action: TabAction) {
        if action == TabAction::Unsaved(Unsaved::Save) {
            let held = self.workspace.open_files.asking;
            for index in held
                .map(|close| self.workspace.open_files.unsaved_closing(close))
                .unwrap_or_default()
            {
                self.write_file(index);
            }
            action = TabAction::Unsaved(Unsaved::Discard);
        }
        let synced: Vec<(String, String)> = self
            .workspace
            .open_files
            .files
            .iter()
            .filter(|file| file.document.0.try_borrow().is_ok_and(|held| held.synced))
            .map(|file| (file.directory.clone(), file.path.clone()))
            .collect();
        if let Some((directory, path)) = self.workspace.open_files.act(action) {
            self.read_file(directory, path);
        }
        let files = &self.workspace.open_files;
        let closed = synced
            .into_iter()
            .filter(|(directory, path)| files.position(directory, path).is_none())
            .collect();
        self.close_for_server(closed);
        self.follow_diff_tab();
    }

    /// Keeps the Git panel's diff in step with the tab showing: the Diff tab
    /// shows the file chosen, or the first change when none was, and any
    /// other tab lets the diff go.
    fn follow_diff_tab(&mut self) {
        let git = &mut self.workspace.git;
        if self.workspace.open_files.tab != SessionTab::Diff {
            git.diff = None;
            return;
        }
        if git.diff.is_none() {
            git.diff = git.listed().first().map(|change| change.path.clone());
            self.git_load_diff();
        }
    }

    /// Reads again the open files of `directory` that have no unsaved
    /// changes, after the server said something changed the folder, so
    /// that their editors follow edits made outside them.
    pub(super) fn reload_open_files(&mut self, directory: &str) {
        let stale: Vec<(String, String)> = self
            .workspace
            .open_files
            .files
            .iter()
            .filter(|file| {
                file.body == FileBody::Text
                    && !file.is_dirty()
                    && super::same_folder(&file.directory, directory)
            })
            .map(|file| (file.directory.clone(), file.path.clone()))
            .collect();
        for (directory, path) in stale {
            self.ask(
                Request::ReadFile {
                    directory: directory.clone(),
                    path: path.clone(),
                    head: false,
                    revision: None,
                },
                Ask::Code(CodeAsk::Reload(directory, path)),
            );
        }
    }

    /// Puts `text`, what the folder now holds of the open file at `index`,
    /// in its editor in place of the text it shows, keeping the caret where
    /// it can, unless the file was changed in the editor meanwhile or holds
    /// that text already; its language server is told of the change.
    fn take_reloaded(&mut self, index: usize, text: String) {
        let told = {
            let file = &self.workspace.open_files.files[index];
            if file.body != FileBody::Text || file.is_dirty() {
                return;
            }
            let mut document = file.document.0.borrow_mut();
            let text = text.replace("\r\n", "\n");
            if document.field.text() == text {
                return;
            }
            let whole = document.field.text().len();
            let caret = document.field.caret().min(text.len());
            let caret = (0..=caret)
                .rev()
                .find(|at| text.is_char_boundary(*at))
                .unwrap_or(0);
            let edited = document.edit(pm_text::Edit::Replace(0..whole, text, (caret, caret)));
            document.mark_saved();
            edited.and_then(|edited| edited.change)
        };
        if let Some((version, change)) = told {
            self.code(index, ServerAsk::Changed(version, change));
        }
    }

    /// Asks the server to write the open file at position `index` as its
    /// document holds it.
    pub fn save_file(&mut self, index: usize) {
        let Some(file) = self.workspace.open_files.files.get(index) else {
            return;
        };
        if file.body != FileBody::Text {
            return;
        }
        self.tidy_before_save(index);
        let Some(file) = self.workspace.open_files.files.get(index) else {
            return;
        };
        let synced = file.document.0.borrow().synced;
        if self.settings.editor.format_on_save && synced {
            let (directory, path) = (file.directory.clone(), file.path.clone());
            let version = file.document.0.borrow().version;
            self.ask(
                Request::Format {
                    directory: directory.clone(),
                    path: path.clone(),
                    version,
                    range: None,
                },
                Ask::Server(super::lsp::ServerFor::FormatThenSave(directory, path)),
            );
            return;
        }
        self.write_file(index);
    }

    /// Asks the server to write the open file at position `index` as its
    /// document holds it now, with no tidying or formatting first.
    pub(super) fn write_file(&mut self, index: usize) {
        let Some(file) = self.workspace.open_files.files.get(index) else {
            return;
        };
        let text = file.document.0.borrow().to_save();
        let (directory, path) = (file.directory.clone(), file.path.clone());
        self.ask(
            Request::WriteFile {
                directory: directory.clone(),
                path: path.clone(),
                text,
            },
            Ask::Code(CodeAsk::Save(directory, path)),
        );
    }

    /// Takes the spaces off the ends of the lines of the open file at `index`
    /// and ends it with a line break, as far as the editor settings ask,
    /// telling its language server of the change.
    fn tidy_before_save(&mut self, index: usize) {
        let editor = &self.settings.editor;
        let (trim, final_newline) = (
            editor.remove_trailing_whitespace_on_save,
            editor.ensure_final_newline_on_save,
        );
        let told = {
            let file = &self.workspace.open_files.files[index];
            let mut document = file.document.0.borrow_mut();
            document
                .tidy_edit(trim, final_newline)
                .and_then(|edit| document.edit(edit))
                .and_then(|edited| edited.change)
        };
        if let Some((version, change)) = told {
            self.code(index, ServerAsk::Changed(version, change));
        }
    }

    /// Asks the server for what the file at `path` in `directory` holds, as
    /// bytes for a picture and as text otherwise.
    pub(super) fn read_file(&mut self, directory: String, path: String) {
        let (request, ask) = if is_picture(&path) {
            (
                Request::ReadBytes {
                    directory: directory.clone(),
                    path: path.clone(),
                    head: false,
                },
                CodeAsk::Picture(directory, path),
            )
        } else {
            (
                Request::ReadFile {
                    directory: directory.clone(),
                    path: path.clone(),
                    head: false,
                    revision: None,
                },
                CodeAsk::Text(directory, path),
            )
        };
        self.ask(request, Ask::Code(ask));
    }

    /// Handles the `response` to a request about the open files made for `ask`.
    pub(super) fn answer_code(&mut self, ask: CodeAsk, response: Response) {
        let files = &mut self.workspace.open_files;
        match (ask, response) {
            (
                CodeAsk::Text(directory, path),
                Response::File {
                    text,
                    truncated,
                    language,
                },
            ) => {
                let Some(index) = files.position(&directory, &path) else {
                    return;
                };
                let file = &mut files.files[index];
                match text {
                    Some(text) => file.take_text(&text, truncated, language.clone()),
                    None => file.body = FileBody::Failed("This file is not text.".to_owned()),
                }
                let pending_made = file.is_dirty();
                self.hand_diagnostics(index);
                self.open_for_server(index);
                if pending_made {
                    self.workspace.open_files.files[index].preview = false;
                }
                let file = &self.workspace.open_files.files[index];
                if file.body == FileBody::Text {
                    self.ask(
                        Request::ReadFile {
                            directory: directory.clone(),
                            path: path.clone(),
                            head: true,
                            revision: None,
                        },
                        Ask::Code(CodeAsk::Base(directory.clone(), path.clone())),
                    );
                }
                if let Some(language) =
                    language.filter(|name| pm_treesitter::registry::called(name).is_none())
                {
                    self.ask(
                        Request::Grammar {
                            language: language.clone(),
                        },
                        Ask::Code(CodeAsk::Grammar(language)),
                    );
                }
            }
            (CodeAsk::Picture(directory, path), Response::Bytes { bytes, .. }) => {
                if let Some(index) = files.position(&directory, &path) {
                    files.files[index].body = FileBody::Picture(std::rc::Rc::new(bytes));
                }
            }
            (
                CodeAsk::Text(directory, path) | CodeAsk::Picture(directory, path),
                Response::Failed { message },
            ) => {
                if let Some(index) = files.position(&directory, &path) {
                    files.files[index].body = FileBody::Failed(message);
                }
            }
            (CodeAsk::Grammar(_), Response::Grammar(bundle)) => {
                if let Ok(grammar) = crate::grammar::learn_grammar(&bundle) {
                    for file in &mut files.files {
                        file.highlight_with(&grammar);
                    }
                }
            }
            (CodeAsk::Base(directory, path), response) => {
                let base = match response {
                    Response::File { text, .. } => text,
                    _ => None,
                };
                if let Some(index) = files.position(&directory, &path) {
                    files.files[index].take_base(base);
                }
            }
            (CodeAsk::Save(directory, path), Response::Done) => {
                if let Some(index) = files.position(&directory, &path) {
                    files.files[index].document.0.borrow_mut().mark_saved();
                }
            }
            (CodeAsk::Compare(directory, path), Response::File { text, .. }) => {
                if let Some(index) = files.position(&directory, &path) {
                    let edited = files.files[index]
                        .document
                        .0
                        .borrow()
                        .field
                        .text()
                        .to_owned();
                    let saved = text.unwrap_or_default();
                    self.show_comparison(
                        index,
                        crate::open_files::Comparison::between("Saved", &saved, &edited),
                    );
                }
            }
            (CodeAsk::Reload(directory, path), Response::File { text: Some(text), .. }) => {
                if let Some(index) = files.position(&directory, &path) {
                    self.take_reloaded(index, text);
                }
            }
            (CodeAsk::Save(_, path), Response::Failed { message }) => {
                self.workspace.status = Some(format!("Could not save {path}: {message}"));
            }
            _ => {}
        }
    }
}
