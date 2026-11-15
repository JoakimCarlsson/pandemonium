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
    /// Who last changed this line, counted from 0, of the file at this path in this folder.
    Blame(String, String, usize),
    /// The address of a line of a file on its git host, opened when the flag is set and copied otherwise.
    Permalink(bool),
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
        if let Some(here) = self.caret_place() {
            self.jumped_from.push(here);
            self.jumped_back.clear();
        }
        self.open_place(path, line);
    }

    /// Where the caret of the file showing is, as its path and line counted from 1, when a file shows.
    fn caret_place(&self) -> Option<(String, usize)> {
        let SessionTab::File(index) = self.workspace.open_files.tab else {
            return None;
        };
        let file = self.workspace.open_files.files.get(index)?;
        let document = file.document.0.borrow();
        Some((
            file.path.clone(),
            document.line_of(document.field.caret()) + 1,
        ))
    }

    /// Goes back to where the caret was before it last jumped, or forward again to where it went back from when `forward`.
    pub(super) fn walk_jumps(&mut self, forward: bool) {
        let target = if forward {
            self.jumped_back.pop()
        } else {
            self.jumped_from.pop()
        };
        let Some((path, line)) = target else {
            return;
        };
        if let Some(here) = self.caret_place() {
            if forward {
                self.jumped_from.push(here);
            } else {
                self.jumped_back.push(here);
            }
        }
        self.open_place(path, line);
    }

    /// Opens the file at `path` with the caret on `line`, counted from 1, without noting where the caret jumped from.
    fn open_place(&mut self, path: String, line: usize) {
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
        let (synced, language) = {
            let document = file.document.0.borrow();
            (
                document.synced,
                document.language.clone().unwrap_or_default(),
            )
        };
        let editor = &self.settings.editor;
        let preferences = self.settings.machine.language(&language);
        let mut kinds = Vec::new();
        if preferences
            .organize_imports_on_save
            .unwrap_or(editor.organize_imports_on_save)
        {
            kinds.push(pm_editor::ORGANIZE_IMPORTS.to_owned());
        }
        if preferences
            .fix_all_on_save
            .unwrap_or(editor.fix_all_on_save)
        {
            kinds.push(pm_editor::FIX_ALL.to_owned());
        }
        if synced && !kinds.is_empty() {
            self.save_action(index, kinds);
            return;
        }
        self.format_then_write(index);
    }

    /// Takes the first change of the first kind of `kinds` the language server offers for the whole open file at `index`, then the next kind, and formats and writes the file once none are left.
    pub(super) fn save_action(&mut self, index: usize, mut kinds: Vec<String>) {
        let Some(file) = self.workspace.open_files.files.get(index) else {
            return;
        };
        if kinds.is_empty() {
            self.format_then_write(index);
            return;
        }
        let kind = kinds.remove(0);
        let (directory, path) = (file.directory.clone(), file.path.clone());
        let (version, span) = {
            let document = file.document.0.borrow();
            (document.version, document.whole_span())
        };
        self.ask(
            Request::CodeActions {
                directory: directory.clone(),
                path: path.clone(),
                version,
                span,
                only: Some(vec![kind]),
            },
            Ask::Server(super::lsp::ServerFor::SaveAction(directory, path, kinds)),
        );
    }

    /// Formats the open file at `index` when the settings, or those of its language, say saving formats, then writes it.
    pub(super) fn format_then_write(&mut self, index: usize) {
        let Some(file) = self.workspace.open_files.files.get(index) else {
            return;
        };
        let (synced, language) = {
            let document = file.document.0.borrow();
            (
                document.synced,
                document.language.clone().unwrap_or_default(),
            )
        };
        let format = self
            .settings
            .machine
            .language(&language)
            .format_on_save
            .unwrap_or(self.settings.editor.format_on_save);
        if format && synced {
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
                    let comparison = self.comparison("Saved", &saved, &edited);
                    self.show_comparison(index, comparison);
                }
            }
            (
                CodeAsk::Reload(directory, path),
                Response::File {
                    text: Some(text), ..
                },
            ) => {
                if let Some(index) = files.position(&directory, &path) {
                    self.take_reloaded(index, text);
                }
            }
            (CodeAsk::Permalink(open), Response::Address { url }) => {
                self.outcomes.push(if open {
                    crate::quick::QuickOutcome::OpenAddress(url)
                } else {
                    crate::quick::QuickOutcome::Copy(url)
                });
            }
            (CodeAsk::Permalink(_), Response::Failed { message }) => {
                self.workspace.status = Some(format!("No link to this line: {message}"));
            }
            (CodeAsk::Blame(directory, path, line), Response::Blamed { blame }) => {
                if let Some(index) = files.position(&directory, &path) {
                    files.files[index].document.0.borrow_mut().blame =
                        blame_line(&blame).map(|said| (line, said));
                }
            }
            (CodeAsk::Blame(directory, path, _), Response::Failed { .. }) => {
                if let Some(index) = files.position(&directory, &path) {
                    files.files[index].document.0.borrow_mut().blame = None;
                }
            }
            (CodeAsk::Save(_, path), Response::Failed { message }) => {
                self.workspace.status = Some(format!("Could not save {path}: {message}"));
            }
            _ => {}
        }
    }
}

impl Link {
    /// Asks git who last changed line `line`, counted from 0, of the open file at `index`, for the words drawn after the caret's line.
    pub(super) fn ask_blame(&mut self, index: usize, line: usize) {
        let Some(file) = self.workspace.open_files.files.get(index) else {
            return;
        };
        let (directory, path) = (file.directory.clone(), file.path.clone());
        file.document.0.borrow_mut().blame = None;
        self.ask(
            Request::Blame {
                directory: directory.clone(),
                path: path.clone(),
                line: line + 1,
            },
            Ask::Code(CodeAsk::Blame(directory, path, line)),
        );
    }
}

/// What the caret's line says of `blame`: who changed it, how long ago and the commit's first line, or nothing for a line not committed yet.
fn blame_line(blame: &pm_git::Blame) -> Option<String> {
    if blame.id.is_empty() {
        return None;
    }
    let then = u64::try_from(blame.when).unwrap_or_default() * 1000;
    let ago = super::age(super::now_ms().saturating_sub(then));
    Some(format!("{}, {ago} • {}", blame.author, blame.summary))
}

impl Link {
    /// Does the keymap's `action`, which the editor of the open file at `index` left to the window: goes back or forward through the places the caret jumped from, copies or opens the address of the caret's lines on the git host, lists the file's history or opens a shell in its folder.
    pub(super) fn editor_action(&mut self, index: usize, action: pm_editor::Action) {
        let Some(file) = self.workspace.open_files.files.get(index) else {
            return;
        };
        let (directory, path) = (file.directory.clone(), file.path.clone());
        let lines = {
            let document = file.document.0.borrow();
            let range = document
                .field
                .selection()
                .unwrap_or(document.field.caret()..document.field.caret());
            (
                document.line_of(range.start) + 1,
                document.line_of(range.end) + 1,
            )
        };
        match action {
            pm_editor::Action::GoBack => self.walk_jumps(false),
            pm_editor::Action::GoForward => self.walk_jumps(true),
            pm_editor::Action::CopyPermalink | pm_editor::Action::OpenPermalink => self.ask(
                Request::Permalink {
                    directory,
                    path,
                    line: Some(lines.0),
                    end_line: (lines.1 > lines.0).then_some(lines.1),
                },
                Ask::Code(CodeAsk::Permalink(
                    action == pm_editor::Action::OpenPermalink,
                )),
            ),
            pm_editor::Action::FileHistory => {
                self.workspace.open_files.tab = SessionTab::File(index);
                self.open_choices(crate::quick::Choosing::History);
            }
            pm_editor::Action::OpenTerminalHere => {
                let folder = match path.rsplit_once('/') {
                    Some((parent, _)) => format!("{}/{parent}", directory.trim_end_matches('/')),
                    None => directory,
                };
                self.workspace.dock_open = true;
                self.workspace.dock_tab = crate::model::DockTab::Terminal;
                self.change_dock(|dock| dock.new_shell_in(folder));
            }
            _ => {}
        }
    }
}
