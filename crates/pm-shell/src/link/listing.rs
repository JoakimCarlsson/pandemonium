//! The pick lists the quick input shows for the open file and the project:
//! the places the thing under the caret is used, the code actions offered
//! at the caret, the states the file was kept in when saved, and the
//! problems the language servers report; asked of the server, so they work
//! the same for remote projects.

use pm_protocol::{Request, Response};

use super::{Ask, Link};
use crate::quick::{Choosing, QuickItem, QuickKind, QuickPick};

/// The version the open file is handed to its language server at for one request.
const LISTED_VERSION: i32 = 1;

impl Link {
    /// Asks the server what the list of choices `choosing` holds, when it is one the server fills.
    pub(super) fn ask_listing(&mut self, choosing: Choosing) {
        if !matches!(
            choosing,
            Choosing::References | Choosing::Actions | Choosing::History
        ) {
            return;
        }
        let Some(directory) = self.browse_root() else {
            return;
        };
        let Some(file) = self.workspace.open_files.shown() else {
            self.workspace.status = Some("Open a file first.".to_owned());
            return;
        };
        let path = file.path.clone();
        let file_directory = file.directory.clone();
        let document = file.document.0.borrow();
        let text = document.field.text().to_owned();
        let caret = document.field.caret();
        let position = document.position_of(caret);
        let language = document.language.clone();
        let synced = document.synced.then_some(document.version);
        drop(document);
        if let Some(quick) = &mut self.workspace.quick {
            quick.waiting = Some(String::new());
        }
        if choosing == Choosing::History {
            let directory = if file_directory.is_empty() {
                directory
            } else {
                file_directory
            };
            self.ask_timeline(directory, path);
            return;
        }
        let directory = if synced.is_some() {
            file_directory
        } else {
            directory
        };
        let Some(language) = language else {
            self.workspace.status = Some("The open file has no language server.".to_owned());
            return;
        };
        let version = synced.unwrap_or(LISTED_VERSION);
        if synced.is_none() {
            self.ask(
                Request::OpenDocument {
                    directory: directory.clone(),
                    path: path.clone(),
                    language,
                    version,
                    text,
                },
                Ask::Nothing,
            );
        }
        let request = if choosing == Choosing::References {
            Request::Definition {
                directory: directory.clone(),
                path: path.clone(),
                version,
                position,
                seek: pm_lsp::Seek::References,
            }
        } else {
            Request::CodeActions {
                directory: directory.clone(),
                path: path.clone(),
                version,
                span: pm_buffer::Span {
                    start: position,
                    end: position,
                },
                only: None,
            }
        };
        self.ask(request, Ask::Listing(choosing));
    }

    /// The rows of the problems the language servers report, worst first.
    pub(super) fn problem_choices(&self) -> Vec<QuickItem> {
        self.workspace
            .problems
            .iter()
            .map(|problem| QuickItem {
                title: problem
                    .message
                    .lines()
                    .next()
                    .unwrap_or_default()
                    .to_owned(),
                detail: format!("{}:{}", problem.path, problem.line + 1),
                kind: QuickKind::Text,
                keys: None,
                matched: Vec::new(),
                pick: QuickPick::Open {
                    path: problem.path.clone(),
                    line: Some(problem.line + 1),
                },
            })
            .collect()
    }

    /// Takes the code action at `place` among those last offered for the open file.
    pub(super) fn take_action(&mut self, place: usize) {
        let (Some(directory), Some(file)) = (self.browse_root(), self.workspace.open_files.shown())
        else {
            return;
        };
        let path = file.path.clone();
        let synced = file.document.0.borrow().synced;
        self.ask(
            Request::TakeCodeAction {
                directory: directory.clone(),
                path: path.clone(),
                index: place,
            },
            Ask::Server(super::ServerFor::Edited(directory.clone(), path.clone())),
        );
        if !synced {
            self.ask(Request::CloseDocument { directory, path }, Ask::Nothing);
        }
    }

    /// Reads the state of the open file kept under `version`, to put it back in the editor.
    pub(super) fn restore_version(&mut self, version: u64) {
        let (Some(directory), Some(file)) = (self.browse_root(), self.workspace.open_files.shown())
        else {
            return;
        };
        let path = file.path.clone();
        self.ask(
            Request::ReadVersion {
                directory,
                path,
                version,
            },
            Ask::Listing(Choosing::History),
        );
    }

    /// Handles the `response` to a request made for the list of choices `choosing`.
    pub(super) fn answer_listing(&mut self, choosing: Choosing, response: Response) {
        if choosing == Choosing::History
            && let Some(found) = self.answer_timeline(&response)
        {
            self.show_listing(choosing, found);
            return;
        }
        if let (
            Choosing::History,
            Response::File {
                text: Some(text), ..
            },
        ) = (choosing, &response)
        {
            self.put_back(text);
            return;
        }
        let shown = self
            .workspace
            .open_files
            .shown()
            .map(|file| (file.directory.clone(), file.path.clone()));
        let synced = self
            .workspace
            .open_files
            .shown()
            .is_some_and(|file| file.document.0.borrow().synced);
        if choosing == Choosing::References
            && !synced
            && let (Some(directory), Some((_, path))) = (self.browse_root(), shown.clone())
        {
            self.ask(Request::CloseDocument { directory, path }, Ask::Nothing);
        }
        let mut trouble = None;
        let found = match response {
            Response::Definitions { locations } => location_items(locations),
            Response::CodeActions { actions } => actions
                .into_iter()
                .enumerate()
                .map(|(place, action)| QuickItem {
                    title: action.title,
                    detail: action.kind.unwrap_or_default(),
                    kind: if action.preferred {
                        QuickKind::Current
                    } else {
                        QuickKind::Command
                    },
                    keys: None,
                    matched: Vec::new(),
                    pick: QuickPick::Choose(Choosing::Actions, place),
                })
                .collect(),
            Response::Toolchains { toolchains } => toolchains
                .into_iter()
                .map(|toolchain| {
                    let language = if toolchain.language_name.is_empty() {
                        toolchain.language.clone()
                    } else {
                        toolchain.language_name.clone()
                    };
                    QuickItem {
                        title: toolchain.name,
                        detail: format!("{language} · {}", toolchain.detail),
                        kind: if toolchain.selected {
                            QuickKind::Current
                        } else {
                            QuickKind::Choice
                        },
                        keys: None,
                        matched: Vec::new(),
                        pick: QuickPick::Toolchain {
                            language: toolchain.language,
                            id: toolchain.id,
                        },
                    }
                })
                .collect(),
            Response::Failed { message } => {
                trouble = Some(message);
                Vec::new()
            }
            _ => return,
        };
        if trouble.is_some() {
            self.workspace.status = trouble;
        }
        self.show_listing(choosing, found);
    }

    /// Lists `found` in the quick input, when it still lists the choices `choosing`.
    fn show_listing(&mut self, choosing: Choosing, found: Vec<QuickItem>) {
        if let Some(quick) = self
            .workspace
            .quick
            .as_mut()
            .filter(|quick| quick.choosing == Some(choosing))
        {
            quick.waiting = None;
            quick.selected = 0;
            quick.found = found;
        }
    }

    /// Puts `text`, a saved state of the open file, in its editor, left unsaved.
    fn put_back(&mut self, text: &str) {
        let files = &mut self.workspace.open_files;
        let crate::open_files::SessionTab::File(index) = files.tab else {
            return;
        };
        let Some(file) = files.files.get_mut(index) else {
            return;
        };
        let language = file.document.0.borrow().language.clone();
        file.take_text(text, false, language);
        file.document.0.borrow_mut().mark_unsaved();
    }
}

/// The rows listing `locations`, places a language server found: each reads the name found there, or else its line, beside its path and line, and opens the file at that line.
pub(super) fn location_items(locations: Vec<pm_lsp::Location>) -> Vec<QuickItem> {
    locations
        .into_iter()
        .map(|location| {
            let line = location.span.start.line as usize + 1;
            let text = location
                .line
                .as_deref()
                .unwrap_or_default()
                .trim()
                .to_owned();
            QuickItem {
                title: location.name.clone().unwrap_or(text),
                detail: format!("{}:{line}", location.path),
                kind: QuickKind::Symbol,
                keys: None,
                matched: Vec::new(),
                pick: QuickPick::Open {
                    path: location.path,
                    line: Some(line),
                },
            }
        })
        .collect()
}
