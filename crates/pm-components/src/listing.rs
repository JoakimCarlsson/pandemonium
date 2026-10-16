//! The list that hangs over the file shown to pick from: the places a search from the caret found, the changes the language server offers there, the commits that changed the file, the items of its outline, or the files of the project.

use pm_input::FocusId;
use pm_protocol::{CodeAction, Location, Seek};
use pm_text::{Edit, Field};

use crate::state::{Outgoing, WorkspaceState};

/// The list itself, which keeps a press on it from counting as outside.
pub const LISTING_BODY: FocusId = FocusId(7_400_000);

/// The first of the rows of the list; the rest follow it.
pub const LISTING_ROW_BASE: u64 = 7_400_001;

/// One past the last row of the list.
pub const LISTING_ROW_END: u64 = 7_401_000;

/// How many rows the list shows at once.
pub const LISTING_SHOWN: usize = 12;

/// How tall the list is at most, in logical pixels: its title and the rows it shows at once.
pub const LISTING_HEIGHT: f32 = 28.0 + LISTING_SHOWN as f32 * 26.0 + 10.0;

/// What a list over the file holds.
#[derive(Clone, Debug, PartialEq)]
pub enum Items {
    /// The places a search from the caret found.
    Places {
        /// What was looked for.
        seek: Seek,
        /// What was searched from: the word at the caret.
        word: String,
        /// The places, in the order the server gave them.
        locations: Vec<Location>,
    },
    /// The changes the language server offers at the caret.
    Actions {
        /// The file they are for.
        path: String,
        /// The changes, in the order the server gave them.
        offered: Vec<CodeAction>,
    },
    /// The commits that changed a file.
    History {
        /// The file.
        path: String,
        /// The commits, newest first.
        commits: Vec<pm_git::FileCommit>,
    },
    /// The items of the file shown, as its outline lists them, narrowed by what is typed.
    Outline {
        /// The items, in the order they start.
        items: Vec<pm_treesitter::Item>,
        /// What the names are narrowed to.
        filter: Field,
    },
    /// The symbols of the project whose names match what is typed, as the language server of the file shown finds them.
    Symbols {
        /// The file whose language server is asked.
        path: String,
        /// What the names are matched against.
        filter: Field,
        /// The symbols, as the server last named them.
        locations: Vec<Location>,
    },
    /// What the language servers say is wrong with the files of the project.
    Problems {
        /// What is wrong, the worst first.
        problems: Vec<pm_protocol::Problem>,
    },
    /// The lines of the files of the project holding what is typed.
    Search {
        /// What is looked for.
        filter: Field,
        /// The lines, as the server last found them.
        locations: Vec<Location>,
    },
    /// The files of the project, narrowed by what is typed, to open one.
    Files {
        /// Every file, by path from the root of the project.
        paths: Vec<String>,
        /// What the paths are narrowed to.
        filter: Field,
        /// The files that match, best first, by place among `paths`.
        matched: Vec<usize>,
    },
}

/// How many files the finder lists at most.
const MOST_FILES: usize = 200;

/// How well `path` matches `typed`, lower being better: the letters must come in order, and a match within the file name, near its start and in a short path ranks first; nothing when they do not come in order.
fn path_score(path: &str, typed: &str) -> Option<(u8, usize, usize)> {
    if !narrows_to(path, typed) {
        return None;
    }
    let name = path.rsplit('/').next().unwrap_or(path).to_lowercase();
    let typed = typed.to_lowercase();
    let rank = if name.starts_with(&typed) {
        0
    } else if name.contains(&typed) {
        1
    } else if narrows_to(&name, &typed) {
        2
    } else {
        3
    };
    Some((rank, name.len(), path.len()))
}

/// The files of `paths` that match `typed`, best first, at most a limit of them.
fn matching_files(paths: &[String], typed: &str) -> Vec<usize> {
    let typed = typed.trim();
    let mut scored: Vec<((u8, usize, usize), usize)> = paths
        .iter()
        .enumerate()
        .filter_map(|(place, path)| path_score(path, typed).map(|score| (score, place)))
        .collect();
    scored.sort();
    scored
        .into_iter()
        .take(MOST_FILES)
        .map(|(_, place)| place)
        .collect()
}

/// Whether the letters of `typed` come in order in `name`, ignoring case.
fn narrows_to(name: &str, typed: &str) -> bool {
    let mut rest = name.chars().flat_map(char::to_lowercase);
    typed
        .chars()
        .flat_map(char::to_lowercase)
        .all(|wanted| rest.any(|held| held == wanted))
}

impl Items {
    /// The rows shown, each as its place among the items held: all of them, but for an outline narrowed by what is typed.
    pub fn rows(&self) -> Vec<usize> {
        match self {
            Self::Places { locations, .. } => (0..locations.len()).collect(),
            Self::Actions { offered, .. } => (0..offered.len()).collect(),
            Self::History { commits, .. } => (0..commits.len()).collect(),
            Self::Outline { items, filter } => items
                .iter()
                .enumerate()
                .filter(|(_, item)| narrows_to(&item.name, filter.text().trim()))
                .map(|(place, _)| place)
                .collect(),
            Self::Files { matched, .. } => matched.clone(),
            Self::Problems { problems } => (0..problems.len()).collect(),
            Self::Symbols { locations, .. } | Self::Search { locations, .. } => {
                (0..locations.len()).collect()
            }
        }
    }

    /// How many rows there are.
    pub fn len(&self) -> usize {
        self.rows().len()
    }

    /// Whether there are no rows.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// What the list is titled.
    pub fn title(&self) -> String {
        match self {
            Self::Places { seek, word, .. } if word.is_empty() => seek.title().to_owned(),
            Self::Places { seek, word, .. } => format!("{} of {word}", seek.title()),
            Self::Actions { .. } => "Code actions".to_owned(),
            Self::History { path, .. } => {
                let name = path.rsplit('/').next().unwrap_or(path);
                format!("History of {name}")
            }
            Self::Outline { .. } => "Outline".to_owned(),
            Self::Files { .. } => "Files".to_owned(),
            Self::Symbols { .. } => "Symbols".to_owned(),
            Self::Search { .. } => "Search".to_owned(),
            Self::Problems { .. } => "Problems".to_owned(),
        }
    }
}

/// A list hanging over the file shown.
#[derive(Clone, Debug, PartialEq)]
pub struct Listing {
    /// The session it is about, by position.
    pub session: usize,
    /// What it holds; empty while it is being asked for.
    pub items: Items,
    /// The row picked.
    pub picked: usize,
    /// Whether the server has answered.
    pub answered: bool,
    /// Why the server could not answer, when it could not.
    pub problem: Option<String>,
    /// Where it hangs, in logical pixels of the window.
    pub at: (f32, f32),
}

impl Listing {
    /// A list of `items` about the session at `session`, hanging at `at`, waiting for the server.
    pub fn waiting(session: usize, items: Items, at: (f32, f32)) -> Self {
        Self {
            session,
            items,
            picked: 0,
            answered: false,
            problem: None,
            at,
        }
    }
}

/// The field the names of an outline are narrowed by.
pub const LISTING_FILTER: FocusId = FocusId(7_401_950);

/// Takes the row picked in the list.
pub const LISTING_TAKE: FocusId = FocusId(7_401_951);

/// The row of the list that `id` names.
pub fn listing_row_of(id: FocusId) -> Option<usize> {
    (LISTING_ROW_BASE..LISTING_ROW_END)
        .contains(&id.0)
        .then(|| (id.0 - LISTING_ROW_BASE) as usize)
}

impl WorkspaceState {
    /// Whether a list hangs over the file.
    pub fn listing_open(&self) -> bool {
        self.listing.is_some()
    }

    /// Moves the pick of the list `delta` rows down, upwards when negative, wrapping around.
    pub fn listing_moved(mut self, delta: isize) -> Self {
        if let Some(listing) = &mut self.listing {
            let count = listing.items.len();
            if count > 0 {
                listing.picked =
                    (listing.picked as isize + delta).rem_euclid(count as isize) as usize;
            }
        }
        self
    }

    /// Closes the list.
    pub fn listing_closed(mut self) -> Self {
        self.listing = None;
        self
    }

    /// Lists the items of the outline of the file shown at `at`, to go to one by typing part of its name.
    pub fn outline_requested(mut self, at: (f32, f32)) -> Self {
        let Some(session) = self.review_index() else {
            return self;
        };
        let Some(document) = self.shown_document() else {
            return self;
        };
        let items = document
            .syntax
            .as_ref()
            .map(|syntax| syntax.outline(document.field.text()))
            .unwrap_or_default();
        let caret = document.field.caret();
        let picked = items
            .iter()
            .rposition(|item| item.range.start <= caret && caret <= item.range.end)
            .unwrap_or(0);
        self.listing = Some(Listing {
            session,
            items: Items::Outline {
                items,
                filter: Field::new(""),
            },
            picked,
            answered: true,
            problem: None,
            at,
        });
        self
    }

    /// Lists the files of the project the review panel is about at `at`, to open one by typing part of its path.
    pub fn file_finder_requested(mut self, at: (f32, f32)) -> Self {
        let Some(session) = self.review_index() else {
            return self;
        };
        let paths = self.files.clone();
        let matched = matching_files(&paths, "");
        self.listing = Some(Listing {
            session,
            items: Items::Files {
                paths,
                filter: Field::new(""),
                matched,
            },
            picked: 0,
            answered: true,
            problem: None,
            at,
        });
        self
    }

    /// The field narrowing the list, when the list has one.
    pub fn listing_filter(&self) -> Option<&Field> {
        match &self.listing.as_ref()?.items {
            Items::Outline { filter, .. }
            | Items::Files { filter, .. }
            | Items::Symbols { filter, .. }
            | Items::Search { filter, .. } => Some(filter),
            _ => None,
        }
    }

    /// The state after the field narrowing the list was changed by `edit`: the first row is picked.
    pub fn listing_filtered(mut self, edit: Edit) -> Self {
        let Some(listing) = self.listing.as_mut() else {
            return self;
        };
        match &mut listing.items {
            Items::Outline { filter, .. } => filter.apply(edit),
            Items::Files {
                paths,
                filter,
                matched,
            } => {
                filter.apply(edit);
                *matched = matching_files(paths, filter.text());
            }
            Items::Symbols { path, filter, .. } => {
                let before = filter.text().to_owned();
                filter.apply(edit);
                if filter.text() != before {
                    let request = Outgoing::WorkspaceSymbols {
                        session: listing.session,
                        path: path.clone(),
                        query: filter.text().to_owned(),
                    };
                    listing.answered = false;
                    self.outbox.push(request);
                }
            }
            Items::Search { filter, .. } => {
                let before = filter.text().to_owned();
                filter.apply(edit);
                if filter.text() != before {
                    let request = Outgoing::SearchFiles {
                        session: listing.session,
                        query: filter.text().to_owned(),
                    };
                    listing.answered = false;
                    self.outbox.push(request);
                }
            }
            _ => return self,
        }
        if let Some(listing) = self.listing.as_mut() {
            listing.picked = 0;
        }
        self
    }

    /// Lists the symbols of the project as the language server of the file shown finds them, at `at`, narrowed by what is typed.
    pub fn symbols_requested(mut self, at: (f32, f32)) -> Self {
        let Some(session) = self.review_index() else {
            return self;
        };
        let Some(path) = self
            .shown_viewer()
            .filter(|_| self.coding())
            .map(|viewer| viewer.path.clone())
        else {
            return self;
        };
        self.listing = Some(Listing {
            session,
            items: Items::Symbols {
                path,
                filter: Field::new(""),
                locations: Vec::new(),
            },
            picked: 0,
            answered: true,
            problem: None,
            at,
        });
        self
    }

    /// Lists what the language servers say is wrong with the files of the project the review panel is about, at `at`.
    pub fn problems_requested(mut self, at: (f32, f32)) -> Self {
        let Some(session) = self.review_index() else {
            return self;
        };
        self.outbox.push(Outgoing::Problems { session });
        self.listing = Some(Listing::waiting(
            session,
            Items::Problems {
                problems: Vec::new(),
            },
            at,
        ));
        self
    }

    /// The state after the server said what is wrong with the files of the project of the session at `session`.
    pub fn with_problems(mut self, session: usize, problems: Vec<pm_protocol::Problem>) -> Self {
        if let Some(listing) = self.listing.as_mut()
            && listing.session == session
            && matches!(listing.items, Items::Problems { .. })
        {
            listing.items = Items::Problems { problems };
            listing.answered = true;
        }
        self
    }

    /// Lists the lines of the files of the project the review panel is about that hold what is typed, at `at`, starting from what is selected in the file shown.
    pub fn search_requested(mut self, at: (f32, f32)) -> Self {
        let Some(session) = self.review_index() else {
            return self;
        };
        let selected = self
            .shown_document()
            .map(|document| document.field.selected().to_owned())
            .filter(|selected| !selected.is_empty() && !selected.contains('\n'))
            .unwrap_or_default();
        let mut filter = Field::new(selected.clone());
        filter.apply(Edit::SelectAll);
        self.listing = Some(Listing {
            session,
            items: Items::Search {
                filter,
                locations: Vec::new(),
            },
            picked: 0,
            answered: selected.is_empty(),
            problem: None,
            at,
        });
        if !selected.is_empty() {
            self.outbox.push(Outgoing::SearchFiles {
                session,
                query: selected,
            });
        }
        self
    }

    /// The state after the server named `locations`, the symbols or the lines of the project found for `query`, when that is still what is typed.
    pub fn with_symbols(mut self, session: usize, query: &str, locations: Vec<Location>) -> Self {
        if let Some(listing) = self.listing.as_mut()
            && listing.session == session
            && let Items::Symbols {
                filter,
                locations: held,
                ..
            }
            | Items::Search {
                filter,
                locations: held,
            } = &mut listing.items
            && filter.text() == query
        {
            *held = locations;
            listing.answered = true;
            listing.picked = 0;
        }
        self
    }

    /// The state after the server could not fill the list, for `problem`.
    pub fn listing_failed(mut self, problem: String) -> Self {
        if let Some(listing) = &mut self.listing {
            listing.answered = true;
            listing.problem = Some(problem);
        }
        self
    }

    /// Takes the row picked, or the one at `row` when given: goes to the place, takes the change or opens the file as the commit left it; and closes the list.
    pub fn listing_taken(mut self, row: Option<usize>) -> Self {
        let Some(listing) = self.listing.take() else {
            return self;
        };
        let place = row.unwrap_or(listing.picked);
        let Some(index) = listing.items.rows().get(place).copied() else {
            return self;
        };
        let session = listing.session;
        match listing.items {
            Items::Outline { items, .. } => {
                let Some(item) = items.get(index) else {
                    return self;
                };
                self.remember_spot();
                self.file_edited(pm_text::Edit::Place(item.name_start, false));
                self
            }
            Items::Symbols { locations, .. } | Items::Search { locations, .. } => {
                match locations.into_iter().nth(index) {
                    Some(location) => self.go_to_location(session, location),
                    None => self,
                }
            }
            Items::Problems { problems } => match problems.into_iter().nth(index) {
                Some(problem) => self.go_to_location(
                    session,
                    Location {
                        path: problem.path,
                        span: problem.diagnostic.span,
                        inside: true,
                        ..Location::default()
                    },
                ),
                None => self,
            },
            Items::Files { paths, .. } => {
                if let Some(path) = paths.get(index) {
                    self.remember_spot();
                    self.open_viewer(path.clone(), false);
                }
                self
            }
            Items::Places { locations, .. } => match locations.into_iter().nth(index) {
                Some(location) => self.go_to_location(session, location),
                None => self,
            },
            Items::Actions { path, offered } => {
                let usable = offered
                    .get(index)
                    .is_some_and(|action| action.disabled.is_none());
                if usable {
                    self.outbox.push(Outgoing::TakeCodeAction {
                        session,
                        path,
                        index,
                    });
                }
                self
            }
            Items::History { commits, .. } => {
                if let Some(commit) = commits.into_iter().nth(index) {
                    self.outbox.push(Outgoing::ReadRevision {
                        session,
                        label: format!("{} @ {}", commit.path, commit.id),
                        path: commit.path,
                        revision: commit.object,
                    });
                }
                self
            }
        }
    }
}
