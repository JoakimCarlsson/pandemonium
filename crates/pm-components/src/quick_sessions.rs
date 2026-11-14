//! The sessions the quick input lists: those waiting for the reader first while nothing is typed, the sessions and projects whose names match what is typed, and the chats whose conversations hold it, as the servers found them, with a preview of the message that matched; and turning what is typed into the prompt of a new session.

use pm_input::FocusId;
use pm_protocol::{ChatField, ChatHit};

use crate::listing::{Destination, Items};
use crate::sample::{Session, Status};
use crate::state::{Outgoing, WorkspaceState};

/// Pressed with Control or Command and Enter in the quick input: starts a new session whose prompt is what is typed.
pub const LISTING_PROMPT: FocusId = FocusId(7_401_952);

/// What the list says before the rest of what is said beside a session waiting for the reader, when it waits.
fn waiting_label(session: &Session) -> Option<&'static str> {
    match session.shown() {
        Status::NeedsInput => Some("Needs input"),
        Status::Unread => Some("Unread"),
        Status::Error => Some("Failed"),
        _ => None,
    }
}

/// Whether `session` waits for the reader: it needs an answer, or holds a response the reader has not read.
pub fn waits_for_reader(session: &Session) -> bool {
    waiting_label(session).is_some()
}

/// Places `hits` the servers found for the sessions at their positions into `matched`, the rows of `sessions`, and keeps a preview of each message that matched in `found`: a session already listed keeps its place, and one that is not follows the sessions listed and comes before the projects; a session not among `sessions`, such as a closed one, is added to them first with what `describe` says beside it.
pub(crate) fn merge_hits(
    sessions: &mut Vec<(Destination, String, String)>,
    matched: &mut Vec<usize>,
    found: &mut Vec<(usize, String)>,
    hits: Vec<(usize, ChatHit)>,
    describe: impl Fn(usize) -> Option<(String, String)>,
) {
    for (position, hit) in hits {
        let place = match sessions
            .iter()
            .position(|(destination, ..)| *destination == Destination::Session(position))
        {
            Some(place) => place,
            None => {
                let Some((title, beside)) = describe(position) else {
                    continue;
                };
                sessions.push((Destination::Session(position), title, beside));
                sessions.len() - 1
            }
        };
        if hit.field != ChatField::Title && !found.iter().any(|(held, _)| *held == place) {
            found.push((place, hit.preview));
        }
        if !matched.contains(&place) {
            let first_project = matched
                .iter()
                .position(|row| matches!(sessions[*row].0, Destination::Project(_)))
                .unwrap_or(matched.len());
            matched.insert(first_project, place);
        }
    }
}

impl WorkspaceState {
    /// What the list says beside the session at `index`: whether it waits for the reader, its project and branch, or that it is a chat, and how long ago it changed.
    fn described(&self, index: usize) -> Option<(String, String)> {
        let session = self.sessions.get(index)?;
        let mut parts: Vec<String> = Vec::new();
        parts.extend(waiting_label(session).map(str::to_owned));
        match self.project_of(session) {
            Some(project) => {
                parts.push(project.name.clone());
                if !session.branch.is_empty() {
                    parts.push(session.branch.clone());
                }
            }
            None => parts.push("Chat".to_owned()),
        }
        parts.push(session.age());
        Some((session.title.clone(), parts.join(" · ")))
    }

    /// The places the quick input lists sessions in, before anything is matched: the sessions not closed, but for the one shown, those waiting for the reader first and then the latest changed first; then the projects to start one in.
    pub(crate) fn session_destinations(&self) -> Vec<(Destination, String, String)> {
        let shown = match self.page {
            crate::state::Page::Session(index) => Some(index),
            _ => None,
        };
        let mut order: Vec<usize> = (0..self.sessions.len())
            .filter(|index| {
                Some(*index) != shown
                    && !self.sessions[*index].closed
                    && !self.sessions[*index].home
            })
            .collect();
        order.sort_by_key(|index| {
            let session = &self.sessions[*index];
            (
                !waits_for_reader(session),
                std::cmp::Reverse(session.updated_at),
            )
        });
        let projects = self.projects.iter().enumerate().map(|(index, project)| {
            (
                Destination::Project(index),
                project.name.clone(),
                String::new(),
            )
        });
        order
            .into_iter()
            .filter_map(|index| {
                self.described(index)
                    .map(|(title, beside)| (Destination::Session(index), title, beside))
            })
            .chain(projects)
            .collect()
    }

    /// Asks every server for the sessions whose conversations hold `query`, when it has a word.
    pub(crate) fn chats_asked(&mut self, query: &str) -> bool {
        if query.trim().is_empty() {
            return false;
        }
        self.outbox.push(Outgoing::SearchChats {
            query: query.to_owned(),
        });
        true
    }

    /// The state after a server found `hits`, the sessions at these positions whose titles or conversations hold `query`, when that is still what the quick input lists sessions for.
    pub fn with_chats(mut self, query: &str, hits: Vec<(usize, ChatHit)>) -> Self {
        let described: Vec<Option<(String, String)>> = (0..self.sessions.len())
            .map(|index| {
                self.sessions
                    .get(index)
                    .filter(|session| !session.home)
                    .and_then(|_| self.described(index))
            })
            .collect();
        let Some(listing) = self.listing.as_mut() else {
            return self;
        };
        if listing.items.query() != query {
            return self;
        }
        if let Items::Sessions {
            sessions,
            matched,
            found,
            ..
        } = &mut listing.items
        {
            merge_hits(sessions, matched, found, hits, |position| {
                described.get(position).cloned().flatten()
            });
            listing.answered = true;
        }
        self
    }

    /// Starts a new session whose prompt is what is typed in the quick input, when it lists sessions and something is typed, and otherwise takes the row picked as Enter does: in the project of the session shown or of the first project, or as a chat when there is no project; the list closes.
    pub fn listing_prompted(mut self) -> Self {
        let typed = match self.listing.as_ref().map(|listing| &listing.items) {
            Some(Items::Sessions { filter, .. }) => filter.text().trim().to_owned(),
            _ => return self.listing_taken(None),
        };
        if typed.is_empty() {
            return self.listing_taken(None);
        }
        self.listing = None;
        let project = match self.page {
            crate::state::Page::Session(index) => {
                self.sessions.get(index).and_then(|session| session.project)
            }
            crate::state::Page::New(project) | crate::state::Page::Project(project) => {
                Some(project)
            }
            _ => None,
        }
        .or_else(|| (!self.projects.is_empty()).then_some(0));
        match project {
            Some(project) => self.start_in(project, None),
            None => self.start_chat(),
        }
        self.prompt = crate::state::prompt_field(&typed);
        self.sync_context();
        self
    }
}

#[cfg(test)]
mod tests;
