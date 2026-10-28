//! Finding one's way around code through its language server: the places a search from the caret finds, the thing at the caret renamed across its project, and the changes the server offers at the caret.

use pm_input::FocusId;
use pm_protocol::{CodeAction, FileEdit, Location, Position, Seek};
use pm_text::{Edit, Field};

use crate::listing::{Items, Listing};
use crate::state::{Outgoing, WorkspaceState};

/// The field the new name of the thing at the caret is typed in.
pub const RENAME_SYMBOL: FocusId = FocusId(7_401_900);

/// Hands in the new name typed.
pub const RENAME_SYMBOL_SUBMIT: FocusId = FocusId(7_401_901);

/// The box the new name is typed in, which keeps a press on it from counting as outside.
pub const RENAME_BOX: FocusId = FocusId(7_401_902);

/// The thing at the caret being given a new name.
#[derive(Clone, Debug, PartialEq)]
pub struct Renaming {
    /// The session of the file, by position.
    pub session: usize,
    /// The file.
    pub path: String,
    /// The version of the file the place is in.
    pub version: i32,
    /// The place.
    pub position: Position,
    /// The new name being typed.
    pub field: Field,
    /// Where the box hangs, in logical pixels of the window.
    pub at: (f32, f32),
    /// Whether the language server is renaming it now.
    pub busy: bool,
    /// Whether the language server is yet to say what the name is and whether it can be renamed.
    pub preparing: bool,
    /// Why the last rename failed, when it did.
    pub problem: Option<String>,
}

/// Whether finding the places `seek` looks for takes the reader straight to the place when there is just one.
fn goes_straight(seek: Seek) -> bool {
    matches!(
        seek,
        Seek::Definition | Seek::Declaration | Seek::TypeDefinition | Seek::Implementation
    )
}

/// Whether `character` is part of a name.
fn is_name(character: char) -> bool {
    character.is_alphanumeric() || character == '_'
}

impl WorkspaceState {
    /// The session the review panel is about and the file it shows, by path, when the file is read by a language server.
    fn coding_file(&self) -> Option<(usize, String)> {
        let session = self.review_index()?;
        let path = self.shown_viewer()?.path.clone();
        self.coding().then_some((session, path))
    }

    /// The name around the byte `offset` of the file shown, as a byte range.
    fn name_at(&self, offset: usize) -> Option<std::ops::Range<usize>> {
        let document = self.shown_document()?;
        let text = document.field.text();
        let start = document.word_start(offset);
        let end = text[offset.min(text.len())..]
            .char_indices()
            .find(|(_, character)| !is_name(*character))
            .map_or(text.len(), |(at, _)| offset + at);
        Some(start..end)
    }

    /// Whether the file shown is read by a language server, so it can be asked about.
    pub fn coding(&self) -> bool {
        self.shown_document()
            .is_some_and(|document| document.synced)
    }

    /// Whether the project of the session the review panel is about is on this machine.
    pub fn review_is_local(&self) -> bool {
        self.review_session()
            .and_then(|session| self.projects.get(session.project))
            .is_some_and(|project| project.host().is_none())
    }

    /// Looks for what `seek` names from the byte `offset` of the file shown; what is found is listed at `at` unless it is one place to go to.
    pub fn seek_requested(mut self, offset: usize, seek: Seek, at: (f32, f32)) -> Self {
        let Some((session, path)) = self.coding_file() else {
            return self;
        };
        let word = self
            .name_at(offset)
            .and_then(|range| {
                self.shown_document()
                    .map(|document| document.field.text()[range].to_owned())
            })
            .unwrap_or_default();
        let Some(document) = self.shown_document() else {
            return self;
        };
        let request = Outgoing::Definition {
            session,
            path,
            version: document.version,
            position: document.position_of(offset),
            seek,
        };
        self.outbox.push(request);
        self.listing = Some(Listing::waiting(
            session,
            Items::Places {
                seek,
                word,
                locations: Vec::new(),
            },
            at,
        ));
        self
    }

    /// The state after the language server named the places `seek` looks for from a place in a file of the session at `session`: the place itself is shown when there is just one to go to, and they are listed otherwise; nothing happens when the search was given up.
    pub fn with_places(mut self, session: usize, seek: Seek, locations: Vec<Location>) -> Self {
        let Some(listing) = self.listing.as_mut().filter(|held| {
            held.session == session
                && !held.answered
                && matches!(held.items, Items::Places { seek: wanted, .. } if wanted == seek)
        }) else {
            return self;
        };
        if goes_straight(seek) && locations.len() <= 1 {
            self.listing = None;
            return match locations.into_iter().next() {
                Some(location) => self.go_to_location(session, location),
                None => self,
            };
        }
        if let Items::Places {
            locations: held, ..
        } = &mut listing.items
        {
            *held = locations;
        }
        listing.answered = true;
        self
    }

    /// The list to draw over the file: any but a search for one place to go to that is still under way.
    pub fn listing_shown(&self) -> Option<&Listing> {
        self.listing.as_ref().filter(|listing| {
            listing.answered
                || !matches!(listing.items, Items::Places { seek, .. } if goes_straight(seek))
        })
    }

    /// Opens a box at `at` to give the thing at the byte `offset` of the file shown a new name, holding its name now, selected.
    pub fn rename_requested(mut self, offset: usize, at: (f32, f32)) -> Self {
        let Some((session, path)) = self.coding_file() else {
            return self;
        };
        let Some(range) = self.name_at(offset).filter(|range| !range.is_empty()) else {
            return self;
        };
        let Some(document) = self.shown_document() else {
            return self;
        };
        let mut field = Field::new(&document.field.text()[range.clone()]);
        field.apply(Edit::SelectAll);
        let (version, position) = (document.version, document.position_of(range.start));
        self.renaming = Some(Renaming {
            session,
            path: path.clone(),
            version,
            position,
            field,
            at,
            busy: false,
            preparing: true,
            problem: None,
        });
        let request = Outgoing::PrepareRename {
            session,
            path,
            version,
            position,
        };
        self.outbox.push(request);
        self
    }

    /// The state after the language server said what can be renamed where the box for a new name was opened: the name it says, or the word the box holds, is what is typed over; and the box says so when nothing there can be renamed.
    pub fn with_rename_target(
        mut self,
        session: usize,
        target: Option<pm_protocol::RenameTarget>,
    ) -> Self {
        let Some(renaming) = self
            .renaming
            .as_ref()
            .filter(|held| held.session == session && held.preparing)
        else {
            return self;
        };
        let Some(target) = target else {
            if let Some(renaming) = &mut self.renaming {
                renaming.problem = Some("nothing at this place can be renamed".to_owned());
            }
            return self;
        };
        let (path, position) = (renaming.path.clone(), renaming.position);
        let named = target.placeholder.or_else(|| {
            let document = self.shown_document().filter(|_| {
                self.shown_viewer()
                    .is_some_and(|viewer| viewer.path == path)
            })?;
            let span = target.span?;
            let text = document.field.text();
            let start = document.line_index().offset_of(span.start, text);
            let end = document.line_index().offset_of(span.end, text);
            text.get(start..end).map(str::to_owned)
        });
        let start = target.span.map_or(position, |span| span.start);
        if let Some(renaming) = &mut self.renaming {
            renaming.preparing = false;
            renaming.position = start;
            if let Some(name) = named {
                renaming.field = Field::new(name);
                renaming.field.apply(Edit::SelectAll);
            }
        }
        self
    }

    /// The state after asking the language server what can be renamed failed for `problem`: the box says why and the name can still be tried.
    pub fn rename_unprepared(mut self, problem: String) -> Self {
        if let Some(renaming) = &mut self.renaming {
            renaming.preparing = false;
            renaming.problem = Some(problem);
        }
        self
    }

    /// Asks the language server to rename the thing to the name typed in the box, which closes once it has answered.
    pub fn rename_submitted(mut self) -> Self {
        let Some(renaming) = self
            .renaming
            .as_mut()
            .filter(|held| !held.busy && !held.preparing)
        else {
            return self;
        };
        let name = renaming.field.text().trim().to_owned();
        if name.is_empty() {
            return self;
        }
        renaming.busy = true;
        let request = Outgoing::RenameSymbol {
            session: renaming.session,
            path: renaming.path.clone(),
            version: renaming.version,
            position: renaming.position,
            name,
        };
        self.outbox.push(request);
        self
    }

    /// Whether the box for a new name is open.
    pub fn renaming_open(&self) -> bool {
        self.renaming.is_some()
    }

    /// Closes the box for a new name.
    pub fn rename_closed(mut self) -> Self {
        self.renaming = None;
        self
    }

    /// The state after renaming failed for `problem`: the box stays open and says why.
    pub fn rename_failed(mut self, problem: String) -> Self {
        if let Some(renaming) = &mut self.renaming {
            renaming.busy = false;
            renaming.problem = Some(problem);
        }
        self
    }

    /// The byte range of the link at the byte `offset` of the file shown, and where it leads.
    pub fn link_under(
        &self,
        offset: usize,
    ) -> Option<(std::ops::Range<usize>, pm_protocol::LinkTarget)> {
        let (range, target) = self.shown_document()?.link_at(offset)?;
        Some((range, target.clone()))
    }

    /// Follows a link of the file shown: a web address is opened in the browser, and a file of the project is opened at the place the link names, after remembering where the caret was so that going back returns there.
    pub fn link_followed(mut self, target: pm_protocol::LinkTarget) -> Self {
        match target {
            pm_protocol::LinkTarget::Url { address } => {
                if pm_acp::is_web_address(&address) {
                    self.outbox.push(Outgoing::Open { address });
                }
            }
            pm_protocol::LinkTarget::Unresolved { link } => {
                if let Some((session, path)) = self.coding_file() {
                    self.outbox.push(Outgoing::ResolveLink {
                        session,
                        path,
                        link,
                    });
                }
            }
            pm_protocol::LinkTarget::File { path, position } => {
                self.remember_spot();
                self.open_viewer(path, false);
                if let Some(viewer) = self
                    .git
                    .shown
                    .and_then(|index| self.git.viewers.get_mut(index))
                {
                    viewer.jump = position;
                }
            }
        }
        self
    }

    /// The state after the language server said where a link of the file shown at the session at `session` leads: it is followed, and when it leads nowhere the session is told so.
    pub fn with_link_resolved(
        self,
        session: usize,
        target: Option<pm_protocol::LinkTarget>,
    ) -> Self {
        match target {
            Some(target) => self.link_followed(target),
            None => self.noted(session, "That link leads nowhere the editor can open"),
        }
    }

    /// Asks the language server what changes it offers at the caret of the file shown, or for the selection, a list of them hanging at `at`.
    pub fn code_actions_requested(mut self, at: (f32, f32)) -> Self {
        let Some((session, path)) = self.coding_file() else {
            return self;
        };
        let Some(document) = self.shown_document() else {
            return self;
        };
        let caret = document.field.caret();
        let range = document.field.selection().unwrap_or(caret..caret);
        let span = pm_protocol::Span::new(
            document.position_of(range.start),
            document.position_of(range.end),
        );
        let request = Outgoing::CodeActions {
            session,
            path: path.clone(),
            version: document.version,
            span,
        };
        self.outbox.push(request);
        self.listing = Some(Listing::waiting(
            session,
            Items::Actions {
                path,
                offered: Vec::new(),
            },
            at,
        ));
        self
    }

    /// The state after the language server said what changes it offers for the file at `path` of the session at `session`; the one it prefers is picked.
    pub fn with_code_actions(
        mut self,
        session: usize,
        path: &str,
        offered: Vec<CodeAction>,
    ) -> Self {
        if let Some(listing) = self.listing.as_mut().filter(|held| {
            held.session == session
                && matches!(&held.items, Items::Actions { path: wanted, .. } if wanted == path)
        }) {
            listing.picked = offered
                .iter()
                .position(|action| action.preferred && action.disabled.is_none())
                .unwrap_or(0);
            listing.items = Items::Actions {
                path: path.to_owned(),
                offered,
            };
            listing.answered = true;
        }
        self
    }

    /// The state after an edit across the project of the session at `session` was made: the changes to its files open here, `files`, are made to them, and the box for a new name closes.
    pub fn with_edited_files(mut self, session: usize, files: Vec<FileEdit>) -> Self {
        self.renaming = None;
        for file in files {
            self.change_open_file(session, &file.path, file.changes);
        }
        self
    }
}
