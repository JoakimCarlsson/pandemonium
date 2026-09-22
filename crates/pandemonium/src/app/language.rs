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
use crate::editor::{Completions, FileId, Shown};
use crate::keymap::Action;
use crate::picker::{Choice, Kind, Row};

/// How far under the pointer what is said about a place is drawn.
const HINT_DROP: f32 = 18.0;

/// Why a question was asked, for an answer that is not always acted on.
///
/// The same question serves two purposes: a reader asking to be taken
/// somewhere, and the editor asking whether there is anywhere to be taken —
/// which is what a name has to have before it is drawn as a link.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Purpose {
    /// The reader asked; act on the answer.
    Act,
    /// The pointer is resting on a name; only say whether it leads anywhere.
    Link,
}

/// The name the pointer is over, and what the server made of it.
#[derive(Clone, Debug)]
pub struct Link {
    /// The file the pointer is over.
    pub file: FileId,
    /// The word under it, which the question was asked about.
    pub about: std::ops::Range<Position>,
    /// What to underline, once a server has said the name leads somewhere.
    ///
    /// The server's own span is used when it gave one: it knows where the
    /// name it resolved begins and ends better than a walk over the
    /// characters around the pointer does.
    pub span: Option<std::ops::Range<Position>>,
}

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
    /// Why it was asked.
    purpose: Purpose,
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
        if self.say_unserved() {
            return;
        }
        self.ask(request);
    }

    /// Says that the file has no server, when it has none.
    ///
    /// A command that quietly does nothing is a command the reader retries.
    /// Nothing is running behind a file whose server is not installed, and
    /// saying so is the whole of what the editor can do about it.
    fn say_unserved(&mut self) -> bool {
        let Some(document) = self.active_file() else {
            return false;
        };
        if document.borrow().is_served() {
            return false;
        }
        let language = document
            .borrow()
            .buffer()
            .language()
            .map(|language| language.name().to_owned())
            .unwrap_or_else(|| String::from("this file"));

        let mut hint = Shown::at(self.cursor_point());
        hint.said = Some(format!("No language server is running for {language}."));
        self.hint = Some(hint);
        self.request_redraw();
        true
    }

    /// Asks the server behind the focused file `request`, about the cursor.
    pub(super) fn ask(&mut self, request: Request) {
        if matches!(request, Request::Hover | Request::Signature) {
            self.hint = Some(Shown::at(self.cursor_point()));
        }
        let Some(file) = self.active_file_id() else {
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
        self.ask_about_for(file, at, request, Purpose::Act);
    }

    /// Asks every server behind `file` `request`, for the reason `purpose`.
    pub(super) fn ask_about_for(
        &mut self,
        file: FileId,
        at: Position,
        request: Request,
        purpose: Purpose,
    ) {
        self.forget(file, &request, purpose);
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
                purpose,
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
        let Some(scope) = self.scope() else {
            return;
        };
        let showing = self
            .panes
            .panes()
            .into_iter()
            .filter_map(|pane| self.panes.pane(pane)?.active(scope)?.file())
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

    /// Says what the editor knows about the place the pointer has stopped on.
    ///
    /// A fault the editor already knows about goes up at once, because it
    /// has nothing to ask anybody, and the server is asked about the same
    /// place all the same: what a squiggle is on still has a type, and the
    /// two are read together.
    ///
    /// A word already being talked about is not asked about again, which is
    /// what lets the panel stay up while the pointer crosses the name it is
    /// about.
    pub(super) fn hover_at(&mut self, point: pm_gfx::Point) {
        let Some((file, at)) = self.place_under(point) else {
            return self.forget_hint(point);
        };
        let Some(document) = self.editor.get(file) else {
            return;
        };
        let about = (file, document.borrow().buffer().word_at(at));
        if self.hint.as_ref().and_then(|hint| hint.about.clone()) == Some(about.clone()) {
            return;
        }

        let fault = document
            .borrow()
            .buffer()
            .diagnostic_at(at)
            .map(|found| found.message.clone());

        self.hint = Some(Shown {
            at: pm_gfx::Point::new(point.x, point.y + HINT_DROP),
            fault,
            said: None,
            about: Some(about),
        });
        self.ask_about(file, at, Request::Hover);
        self.request_redraw();
    }

    /// The character in a file the pointer is over, when it is over one.
    ///
    /// The place is the character under the pointer rather than the gap a
    /// caret would fall into: what is being asked about is a name, not a
    /// place between two of them.
    pub(super) fn place_under(&self, point: pm_gfx::Point) -> Option<(FileId, Position)> {
        let (file, document) = self.document_at(point)?;
        let at = document.borrow().position_under(point)?;
        Some((file, at))
    }

    /// The word under `point`, and the file it is in.
    pub(super) fn word_under(
        &self,
        point: pm_gfx::Point,
    ) -> Option<(FileId, std::ops::Range<Position>)> {
        let (file, at) = self.place_under(point)?;
        let document = self.editor.get(file)?;
        let word = document.borrow().buffer().word_at(at);
        (word.start != word.end).then_some((file, word))
    }

    /// The name the panel is answering about, while it is up.
    ///
    /// It is lit on the text as well as answered for beside it, so a panel
    /// that opened over a crowded line still says which name it is about.
    pub(super) fn hovered_name(&self) -> Option<(FileId, std::ops::Range<Position>)> {
        let hint = self.hint.as_ref()?;
        (!hint.is_empty()).then_some(hint.about.clone()?)
    }

    /// The name to draw as a link, while the key that makes one is held.
    ///
    /// Nothing is underlined until a server has said the name leads
    /// somewhere: a link that goes nowhere is a promise the editor cannot
    /// keep.
    pub(super) fn link_target(&self) -> Option<(FileId, std::ops::Range<Position>)> {
        if !self.modifiers.control_key() {
            return None;
        }
        let link = self.link.as_ref()?;
        Some((link.file, link.span.clone()?))
    }

    /// Asks whether the name under the pointer leads anywhere, once.
    ///
    /// The question is asked again only when the pointer reaches another
    /// name, so sliding along one costs nothing after the first answer.
    pub(super) fn follow_pointer(&mut self, point: pm_gfx::Point) {
        if !self.modifiers.control_key() {
            return self.drop_link();
        }
        let Some((file, word)) = self.word_under(point) else {
            return self.drop_link();
        };
        if self
            .link
            .as_ref()
            .is_some_and(|link| link.file == file && link.about == word)
        {
            return;
        }
        if !self
            .editor
            .get(file)
            .is_some_and(|document| document.borrow().is_served())
        {
            return self.drop_link();
        }

        let at = word.start;
        self.link = Some(Link {
            file,
            about: word,
            span: None,
        });
        self.ask_about_for(file, at, Request::Definition, Purpose::Link);
    }

    /// Forgets the link the pointer was over, redrawing if one was drawn.
    pub(super) fn drop_link(&mut self) {
        if self.link.take().is_some() {
            self.request_redraw();
        }
    }

    /// Takes down what a server said about the name under the pointer.
    fn link_found(&mut self, pending: &Pending, found: &[Location]) {
        let Some(link) = self.link.as_mut() else {
            return;
        };
        if link.file != pending.file || !link.about.contains(&pending.at) {
            return;
        }
        link.span = Some(
            found
                .iter()
                .find_map(|location| location.origin.clone())
                .unwrap_or_else(|| link.about.clone()),
        );
        self.request_redraw();
    }

    /// Drops what was being said about a place the pointer has left.
    ///
    /// A panel about the word the pointer is still on stays up: reading a
    /// hover means moving across the name it is about, and one that went
    /// away at the first pixel of that would never be read at all.
    pub(super) fn forget_hint(&mut self, point: pm_gfx::Point) {
        let about = self.hint.as_ref().and_then(|hint| hint.about.clone());
        let Some(about) = about else {
            return;
        };
        let over = self
            .place_under(point)
            .filter(|(file, at)| *file == about.0 && about.1.contains(at));
        if over.is_none() {
            self.hint = None;
            self.request_redraw();
        }
    }

    /// The file drawn under `point`, whichever pane is showing it.
    pub(super) fn document_at(
        &self,
        point: pm_gfx::Point,
    ) -> Option<(FileId, crate::editor::OpenFile)> {
        let scope = self.scope()?;
        self.panes.panes().into_iter().find_map(|pane| {
            let file = self.panes.pane(pane)?.active(scope)?.file()?;
            let document = self.editor.get(file)?;
            let over = document.borrow().layout().text_area().contains(point);
            over.then_some((file, document))
        })
    }

    /// Gives up on every unanswered question of this kind and reason.
    ///
    /// The reason is part of it: the editor asking whether a name leads
    /// anywhere must not call off the reader asking to be taken there.
    fn forget(&mut self, file: FileId, request: &Request, purpose: Purpose) {
        let kind = std::mem::discriminant(request);
        self.asked.retain(|pending| {
            if pending.file != file
                || pending.purpose != purpose
                || std::mem::discriminant(&pending.request) != kind
            {
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
            Answer::Locations(found) if pending.purpose == Purpose::Link => {
                self.link_found(pending, &found);
            }
            Answer::Locations(found) if pending.request == Request::References => {
                self.show_references(found);
            }
            Answer::Locations(found) => self.go_to_first(&found),
            Answer::Hover(text) | Answer::Signature(text) => {
                if let Some(hint) = self.hint.as_mut() {
                    hint.said = Some(text);
                }
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
                            section: None,
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
                    section: None,
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
