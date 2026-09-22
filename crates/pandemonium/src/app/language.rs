//! What the window asks a language server, and what it does with the answer.
//!
//! Nothing here waits. A command sends a question and is over; the reply
//! arrives on the reader thread, wakes the window, and is acted on then —
//! which is why every question is written down with what it was about, and
//! why an answer to a question about a file that has since been closed is
//! dropped rather than applied to whatever is open now.

use std::path::PathBuf;
use std::sync::Arc;

use pm_text::{Answer, Asked, Client, FileEdit, Location, Position, Request};

use crate::app::App;
use crate::app::places::Place;
use crate::editor::{Completions, FileId};
use crate::keymap::Action;
use crate::picker::{Choice, Kind, Row};

/// How far under the pointer what is said about a place is drawn.
const HINT_DROP: f32 = 18.0;

/// One question asked of a server, for as long as it is unanswered.
pub struct Pending {
    /// The server it was asked of.
    client: Arc<Client>,
    /// What it was asked under.
    asked: Asked,
    /// The file it was asked about.
    file: FileId,
    /// Where in that file it was asked about.
    at: Position,
    /// What was asked.
    request: Request,
}

impl App {
    /// Carries out a command the language server behind the file answers.
    pub(super) fn act_on_language(&mut self, action: Action) {
        let request = match action {
            Action::GoToDefinition => Request::Definition,
            Action::GoToTypeDefinition => Request::TypeDefinition,
            Action::GoToImplementation => Request::Implementation,
            Action::GoToDeclaration => Request::Declaration,
            Action::FindReferences => Request::References,
            Action::ShowHover => Request::Hover,
            Action::ShowCompletions => Request::Completions,
            Action::ShowSignature => Request::Signature,
            Action::ShowCodeActions => Request::CodeActions,
            Action::Format => Request::Format,
            Action::ShowSymbols => Request::Symbols,
            Action::Rename => return self.open_prompt(Action::Rename),
            _ => return,
        };
        self.ask(request);
    }

    /// Asks the server behind the focused file `request`, about the cursor.
    pub(super) fn ask(&mut self, request: Request) {
        if matches!(request, Request::Hover | Request::Signature) {
            self.hint_at = self.cursor_point();
        }
        let Some(file) = self.active_tab() else {
            return;
        };
        let Some(at) = self
            .editor
            .get(file)
            .map(|document| document.borrow().buffer().selection().head)
        else {
            return;
        };
        self.ask_about(file, at, request);
    }

    /// Asks every server behind `file` `request`, about `at` in it.
    ///
    /// A question of the same kind that is still unanswered is given up on
    /// first: what the reader is asking about now is where the cursor is
    /// now, and an answer about where it was a keystroke ago is an answer to
    /// nothing.
    ///
    /// All of them are asked rather than one of them, because which has the
    /// answer is not knowable in advance: a linter has the fix and a type
    /// checker has the type, and the one with nothing to say says nothing.
    pub(super) fn ask_about(&mut self, file: FileId, at: Position, request: Request) {
        self.forget(file, &request);
        let Some(document) = self.editor.get(file) else {
            return;
        };
        let document = document.borrow();
        let clients = document.servers();
        let path = document.buffer().path().to_path_buf();
        drop(document);

        for client in clients {
            let asked = client.ask(request.clone(), &path, at);
            self.asked.push(Pending {
                client,
                asked,
                file,
                at,
                request: request.clone(),
            });
        }
    }

    /// Asks the servers what they know about the files on screen: what to
    /// write into their lines, and what their names are.
    ///
    /// The whole of each file is asked about rather than the part on screen:
    /// scrolling is not a question, and a file short enough to be open is
    /// short enough to be answered about in one go.
    pub(super) fn refresh_annotations(&mut self) {
        let showing = self
            .panes
            .panes()
            .into_iter()
            .filter_map(|pane| self.panes.pane(pane)?.active())
            .collect::<Vec<_>>();

        for file in showing {
            let Some(document) = self.editor.get(file) else {
                continue;
            };
            if document.borrow_mut().wants_semantics() {
                self.ask_about(file, Position::default(), Request::Semantics);
            }
            let wanted = document.borrow_mut().wants_hints();
            if !wanted {
                continue;
            }
            let last = document.borrow().buffer().line_count().saturating_sub(1);
            let span = Position::default()..Position::new(last, 0);
            self.ask_about(file, Position::default(), Request::Hints(span));
        }
    }

    /// Says what the editor knows about the place the pointer is resting on.
    ///
    /// A fault the editor already knows about is said at once, because it
    /// has nothing to ask anybody: a squiggle the reader is pointing at is
    /// one the server has already explained.
    pub(super) fn hover_at(&mut self, point: pm_gfx::Point) {
        let Some((file, document)) = self.document_at(point) else {
            return;
        };
        let at = document.borrow().position_at(point);
        self.hint_at = pm_gfx::Point::new(point.x, point.y + HINT_DROP);

        let fault = document
            .borrow()
            .buffer()
            .diagnostic_at(at)
            .map(|found| found.message.clone());
        if let Some(fault) = fault {
            self.hint = Some((self.hint_at, fault));
            return self.request_redraw();
        }
        self.ask_about(file, at, Request::Hover);
    }

    /// The file drawn under `point`, whichever pane is showing it.
    pub(super) fn document_at(
        &self,
        point: pm_gfx::Point,
    ) -> Option<(FileId, crate::editor::OpenFile)> {
        self.panes.panes().into_iter().find_map(|pane| {
            let file = self.panes.pane(pane)?.active()?;
            let document = self.editor.get(file)?;
            let over = document.borrow().layout().text_area().contains(point);
            over.then_some((file, document))
        })
    }

    /// Gives up on every unanswered question of this kind about this file.
    fn forget(&mut self, file: FileId, request: &Request) {
        let kind = std::mem::discriminant(request);
        self.asked.retain(|pending| {
            if pending.file != file || std::mem::discriminant(&pending.request) != kind {
                return true;
            }
            pending.client.forget(pending.asked);
            false
        });
    }

    /// Collects every answer that has come back, saying whether any had.
    pub(super) fn collect_answers(&mut self) -> bool {
        let ready = self
            .asked
            .iter()
            .enumerate()
            .filter_map(|(index, pending)| {
                let answer = pending.client.answer(pending.asked)?;
                Some((index, answer))
            })
            .collect::<Vec<_>>();
        if ready.is_empty() {
            return false;
        }

        for (index, answer) in ready.iter().rev() {
            let pending = self.asked.remove(*index);
            self.answered(&pending, answer.clone());
        }
        true
    }

    /// Acts on one answer, if the file it was about is still open.
    ///
    /// An empty answer is no answer: every server behind the file is asked,
    /// and the ones with nothing to say must not wipe out what the one with
    /// something to say has already put on the screen.
    fn answered(&mut self, pending: &Pending, answer: Answer) {
        if self.editor.get(pending.file).is_none() {
            return;
        }
        if answer.is_empty() {
            return self.save_once_formatted(pending);
        }
        match answer {
            Answer::Locations(found) if pending.request == Request::References => {
                self.show_references(found);
            }
            Answer::Locations(found) => self.go_to_first(&found),
            Answer::Hover(text) | Answer::Signature(text) => {
                self.hint = Some((self.hint_at, text));
            }
            Answer::Completions(items) => self.show_completions(pending, items),
            Answer::CodeActions(actions) => self.show_code_actions(actions),
            Answer::Edits(files) => {
                self.apply_edits(files);
                self.save_once_formatted(pending);
            }
            Answer::Hints(hints) => {
                if let Some(document) = self.editor.get(pending.file) {
                    document.borrow_mut().buffer_mut().set_hints(hints);
                }
            }
            Answer::Semantics(spans) => {
                if let Some(document) = self.editor.get(pending.file) {
                    document.borrow_mut().buffer_mut().set_semantics(spans);
                }
            }
            Answer::Symbols(symbols) => {
                let rows = symbols
                    .into_iter()
                    .filter_map(|symbol| {
                        let project = self.editor.project_of(pending.file)?;
                        let path = self.editor.path(pending.file)?;
                        Some(Row {
                            label: format!("{}{}", "  ".repeat(symbol.depth), symbol.name),
                            detail: if symbol.detail.is_empty() {
                                symbol.kind.to_owned()
                            } else {
                                symbol.detail
                            },
                            choice: Choice::OpenAt(project, path, symbol.position),
                            enabled: true,
                        })
                    })
                    .collect();
                self.open_picker_with(Kind::Symbols, rows, String::new());
            }
        }
        self.request_redraw();
    }

    /// Saves a file that was formatted on its way to being saved.
    ///
    /// The save waits for the last server to answer, not the first: a
    /// formatter that has changes to make must have made them before the
    /// file they are made to goes to disk.
    fn save_once_formatted(&mut self, pending: &Pending) {
        if pending.request != Request::Format || !self.saving {
            return;
        }
        let awaited = self
            .asked
            .iter()
            .any(|other| other.file == pending.file && other.request == Request::Format);
        if awaited {
            return;
        }
        self.saving = false;
        self.save_active();
    }

    /// Goes to the first place a server named, taking down where the cursor was.
    fn go_to_first(&mut self, found: &[Location]) {
        let Some(location) = found.first() else {
            return;
        };
        let Some(place) = self.place_of(&location.path, location.range.start) else {
            return;
        };
        self.jump_to(&place);
    }

    /// Opens the picker over everywhere a symbol is used.
    fn show_references(&mut self, found: Vec<Location>) {
        let rows = found
            .into_iter()
            .filter_map(|location| {
                let place = self.place_of(&location.path, location.range.start)?;
                let name = location
                    .path
                    .file_name()
                    .map(|name| name.to_string_lossy().into_owned())
                    .unwrap_or_default();
                Some(Row {
                    label: format!("{name}:{}", place.position.line + 1),
                    detail: self.relative_to(place.project, &location.path),
                    choice: Choice::OpenAt(place.project, place.path, place.position),
                    enabled: true,
                })
            })
            .collect::<Vec<_>>();
        self.open_picker_with(Kind::References, rows, String::new());
    }

    /// Offers what could be written where the cursor is.
    fn show_completions(&mut self, pending: &Pending, items: Vec<pm_text::Completion>) {
        if items.is_empty() {
            return;
        }
        let Some(document) = self.editor.get(pending.file) else {
            return;
        };
        let document = document.borrow();
        let word = document.buffer().word_at(pending.at);
        let typed = document
            .buffer()
            .text_in(word.start..document.buffer().selection().head);
        let under = document.layout().cell.height;
        let at = document.point_of(word.start);
        let point = pm_gfx::Point::new(at.x, at.y + under);
        drop(document);

        let mut completions = Completions::new(items, word.start, point);
        completions.narrow(&typed);
        self.completions = (!completions.is_empty()).then_some(completions);
    }

    /// Opens the menu of fixes a server offers where the cursor is.
    fn show_code_actions(&mut self, actions: Vec<pm_text::CodeAction>) {
        self.code_actions = actions;
        if !self.code_actions.is_empty() {
            self.open_menu(crate::workspace::MenuTarget::CodeActions);
        }
    }

    /// Takes the `index`-th code action the server offered.
    pub(super) fn take_code_action(&mut self, index: usize) {
        let Some(action) = self.code_actions.get(index).cloned() else {
            return;
        };
        self.apply_edits(action.edits);
    }

    /// Makes the changes a rename, a formatter or a fix asked for.
    ///
    /// A file that is open takes its changes through the document it is open
    /// as, so the cursor, the undo history and the server all move with it; a
    /// file that is not open is rewritten on disk.
    pub(super) fn apply_edits(&mut self, files: Vec<FileEdit>) {
        for FileEdit { path, edits } in files {
            if edits.is_empty() {
                continue;
            }
            let opened = self
                .open
                .iter()
                .map(pm_core::Project::id)
                .find_map(|project| self.editor.opened(project, &path));

            match opened.and_then(|file| self.editor.get(file)) {
                Some(document) => document
                    .borrow_mut()
                    .edit(|buffer| buffer.apply_edits(edits)),
                None => write_through(&path, edits),
            }
        }
        self.store();
    }

    /// The place `at` in the file at `path` comes to, in whichever project holds it.
    pub(super) fn place_of(&self, path: &std::path::Path, at: Position) -> Option<Place> {
        let project = self
            .open
            .iter()
            .filter(|project| path.starts_with(project.root()))
            .max_by_key(|project| project.root().as_os_str().len())
            .or_else(|| self.open.active())?;
        Some(Place {
            project: project.id(),
            path: path.to_path_buf(),
            position: at,
        })
    }

    /// `path` written from the worktree of `project` down.
    fn relative_to(&self, project: pm_core::ProjectId, path: &std::path::Path) -> String {
        let Some(root) = self.open.get(project).map(pm_core::Project::root) else {
            return path.display().to_string();
        };
        path.strip_prefix(root)
            .unwrap_or(path)
            .display()
            .to_string()
    }
}

/// Makes `edits` to the file at `path`, which nothing has open.
fn write_through(path: &PathBuf, edits: Vec<(std::ops::Range<Position>, String)>) {
    let Ok(mut buffer) = pm_text::Buffer::open(path) else {
        return;
    };
    buffer.apply_edits(edits);
    let _ = buffer.save();
}
