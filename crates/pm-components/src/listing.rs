//! The list that hangs over the file shown to pick from: the places a search from the caret found, the changes the language server offers there, the commits that changed the file, or what the problems are; and the quick input over the command center, whose first character says what it lists: the files of the project, `>` the commands of the window, `#` the symbols of the project, `@` the items of the file's outline and `%` the lines of the project holding what is typed.

use pm_editor::Action;
use pm_input::FocusId;
use pm_protocol::{CodeAction, ExtensionCommand, Location, Seek};
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

/// How wide the list is, in logical pixels.
pub const LISTING_WIDTH: f32 = 600.0;

/// How far the field of the quick input is set in from its edges, in logical pixels, so that the field lies over the command center it opens from.
pub const QUICK_INSET: f32 = 6.0;

/// What the quick input lists, as the first character typed in it says.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Quick {
    /// The files of the project, typed with nothing before them.
    Files,
    /// The commands of the window, after `>`.
    Commands,
    /// The symbols of the project, after `#`.
    Symbols,
    /// The items of the outline of the file shown, after `@`.
    Outline,
    /// The lines of the files of the project holding what is typed, after `%`.
    Search,
    /// The sessions of every project and the projects to start one in, typed with nothing before them; opened only by its own keys.
    Sessions,
    /// The line of the file shown to go to, after `:`.
    Line,
    /// What can be typed first to list something else, after `?`.
    Help,
    /// One thing to pick from a list a command opened, typed with nothing before it.
    Choices,
}

impl Quick {
    /// What is typed first to list this.
    pub const fn prefix(self) -> &'static str {
        match self {
            Self::Files | Self::Sessions | Self::Choices => "",
            Self::Line => ":",
            Self::Help => "?",
            Self::Commands => ">",
            Self::Symbols => "#",
            Self::Outline => "@",
            Self::Search => "%",
        }
    }

    /// What the field says while nothing but the prefix is typed in it.
    pub const fn placeholder(self) -> &'static str {
        match self {
            Self::Files => "Search files by name (type ? to see what else can be typed first)",
            Self::Line => "Go to a line by its number",
            Self::Help => "Type a prefix, or select one",
            Self::Choices => "Select one",
            Self::Commands => "Run a command",
            Self::Symbols => "Go to a symbol in the project",
            Self::Outline => "Go to a symbol in the file",
            Self::Search => "Search the text of the project",
            Self::Sessions => "Go to a session, or a project to start one in",
        }
    }

    /// What the quick input lists once `text` is typed in it while it lists this: what the first character of `text` says, but the sessions stay listed while nothing says otherwise.
    fn after_typing(self, text: &str) -> Self {
        match Self::of(text) {
            Self::Files if matches!(self, Self::Sessions | Self::Choices) => self,
            other => other,
        }
    }

    /// What `text`, all that is typed in the quick input, lists.
    fn of(text: &str) -> Self {
        match text.chars().next() {
            Some('>') => Self::Commands,
            Some('#') => Self::Symbols,
            Some('@') => Self::Outline,
            Some('%') => Self::Search,
            Some(':') => Self::Line,
            Some('?') => Self::Help,
            _ => Self::Files,
        }
    }
}

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
        /// How many of `matched` were opened recently.
        recent: usize,
    },
    /// The commands that can be carried out where the keyboard was, narrowed by what is typed, to carry one out.
    Commands {
        /// Every command, with the keys it is bound to as they are shown.
        commands: Vec<(Action, Option<String>)>,
        /// The commands extensions bring for the machine the prompt is for, each with that machine; they follow `commands` in the places `matched` holds.
        extras: Vec<(Option<String>, ExtensionCommand)>,
        /// What the titles are narrowed to.
        filter: Field,
        /// The commands that match, the recently used first and then the best, by place among `commands`.
        matched: Vec<usize>,
        /// How many of `matched` were used recently.
        recent: usize,
    },
    /// The sessions of every project, the latest changed first, then the projects to start one in, narrowed by what is typed, to go to one.
    Sessions {
        /// Each place to go to, with its name and what is said beside it.
        sessions: Vec<(Destination, String, String)>,
        /// What the titles are narrowed to.
        filter: Field,
        /// The sessions that match, best first, by place among `sessions`.
        matched: Vec<usize>,
    },
    /// The line of the file shown to go to, by the number typed.
    Line {
        /// What is typed: `:` and the number.
        filter: Field,
        /// How many lines the file has.
        lines: usize,
        /// The line the caret is on, counted from one.
        current: usize,
    },
    /// Things to pick one of, narrowed by what is typed.
    Choices {
        /// What they are for.
        choosing: crate::quick_choices::Choosing,
        /// Every one of them, in the order they are listed.
        choices: Vec<crate::quick_choices::Choice>,
        /// What their names are narrowed to.
        filter: Field,
        /// The choices that match, best first, by place among `choices`.
        matched: Vec<usize>,
    },
}

/// How many files the finder lists at most.
const MOST_FILES: usize = 200;

/// How many files the quick input remembers as opened recently.
pub const RECENT_FILES: usize = 20;

/// How many commands the quick input remembers as used recently.
const RECENT_COMMANDS: usize = 8;

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

/// The files of `paths` that match `typed`, with how many of them lead as `recent`, those opened lately, the latest first; the rest best first, at most a limit of them.
fn matching_recent_files(paths: &[String], typed: &str, recent: &[String]) -> (Vec<usize>, usize) {
    let matched = matching_files(paths, typed);
    let used: Vec<usize> = recent
        .iter()
        .filter_map(|wanted| {
            matched
                .iter()
                .copied()
                .find(|place| paths[*place] == *wanted)
        })
        .collect();
    let count = used.len();
    let rest = matched.into_iter().filter(|place| !used.contains(place));
    (used.iter().copied().chain(rest).collect(), count)
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

/// The commands of `commands` whose titles match `typed`, with how many of them lead as `recent`, those used lately, the latest first; the rest best first: a title starting with it, then one holding it, then one holding its letters in order, each in the order the commands are listed; the commands of `extras` that match follow them, at places counted on from the last of `commands`.
fn matching_commands(
    commands: &[(Action, Option<String>)],
    extras: &[(Option<String>, ExtensionCommand)],
    typed: &str,
    recent: &[Action],
) -> (Vec<usize>, usize) {
    let typed = typed.trim().to_lowercase();
    let rank = |action: Action| {
        let title = action.palette_title().to_lowercase();
        if title.starts_with(&typed) {
            Some(0)
        } else if title.contains(&typed) {
            Some(1)
        } else if narrows_to(&title, &typed) {
            Some(2)
        } else if every_word_known(&title, action.keywords(), &typed) {
            Some(3)
        } else {
            None
        }
    };
    let used: Vec<usize> = recent
        .iter()
        .filter(|action| rank(**action).is_some())
        .filter_map(|action| commands.iter().position(|(held, _)| held == action))
        .collect();
    let mut scored: Vec<(u8, usize)> = commands
        .iter()
        .enumerate()
        .filter(|(place, _)| !used.contains(place))
        .filter_map(|(place, (action, _))| rank(*action).map(|rank| (rank, place)))
        .collect();
    scored.sort();
    let count = used.len();
    let mut brought: Vec<(u8, usize)> = extras
        .iter()
        .enumerate()
        .filter_map(|(place, (_, command))| {
            let title = command.title.to_lowercase();
            let rank = if title.starts_with(&typed) {
                0
            } else if title.contains(&typed) {
                1
            } else if narrows_to(&title, &typed) {
                2
            } else {
                return None;
            };
            Some((rank, commands.len() + place))
        })
        .collect();
    brought.sort();
    let matched = used
        .into_iter()
        .chain(scored.into_iter().map(|(_, place)| place))
        .chain(brought.into_iter().map(|(_, place)| place))
        .collect();
    (matched, count)
}

/// A place the list of sessions goes to.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Destination {
    /// A session, by its place among the workspace's.
    Session(usize),
    /// The page that starts a new session in a project, by its place among the workspace's.
    Project(usize),
}

/// The places of `sessions` whose names or what is said beside them match `typed`, the sessions before the projects and each best first: a name starting with it, then one holding it, then one holding its letters in order, then what is said beside it that does, each in the order they are listed.
fn matching_sessions(sessions: &[(Destination, String, String)], typed: &str) -> Vec<usize> {
    let typed = typed.trim().to_lowercase();
    let mut scored: Vec<(bool, u8, usize)> = sessions
        .iter()
        .enumerate()
        .filter_map(|(place, (destination, title, beside))| {
            let title = title.to_lowercase();
            let rank = if title.starts_with(&typed) {
                0
            } else if title.contains(&typed) {
                1
            } else if narrows_to(&title, &typed) {
                2
            } else if narrows_to(&beside.to_lowercase(), &typed) {
                3
            } else {
                return None;
            };
            let project = matches!(destination, Destination::Project(_));
            Some((project, rank, place))
        })
        .collect();
    scored.sort();
    scored.into_iter().map(|(.., place)| place).collect()
}

/// The choices of `choices` whose names or what is said beside them match `typed`, best first: a name starting with it, then one holding it, then one holding its letters in order, then what is said beside it that does, each in the order they are listed; the `?` of the help is not matched.
pub(crate) fn matching_choices(
    choices: &[crate::quick_choices::Choice],
    typed: &str,
) -> Vec<usize> {
    let typed = typed.trim_start_matches('?').trim().to_lowercase();
    let mut scored: Vec<(u8, usize)> = choices
        .iter()
        .enumerate()
        .filter_map(|(place, choice)| {
            let label = choice.label.to_lowercase();
            let rank = if label.starts_with(&typed) {
                0
            } else if label.contains(&typed) {
                1
            } else if narrows_to(&label, &typed) {
                2
            } else if narrows_to(&choice.detail.to_lowercase(), &typed) {
                3
            } else {
                return None;
            };
            Some((rank, place))
        })
        .collect();
    scored.sort();
    scored.into_iter().map(|(_, place)| place).collect()
}

/// The line `typed` names among `lines`, counted from one, when it is a number of one of them.
pub fn line_named(typed: &str, lines: usize) -> Option<usize> {
    typed
        .trim()
        .parse::<usize>()
        .ok()
        .filter(|line| (1..=lines).contains(line))
}

/// The characters of `name` that `typed` matches, by their place among the characters of `name`, ignoring case: the run that holds it when `name` holds it whole, and otherwise its letters where they first come in order; none when they do not.
pub fn matched_characters(name: &str, typed: &str) -> Vec<usize> {
    let name: Vec<char> = name.chars().flat_map(char::to_lowercase).collect();
    let typed: Vec<char> = typed
        .trim()
        .chars()
        .flat_map(char::to_lowercase)
        .filter(|character| !character.is_whitespace())
        .collect();
    if typed.is_empty() {
        return Vec::new();
    }
    let whole: String = typed.iter().collect();
    let joined: String = name.iter().collect();
    if let Some(start) = joined.find(&whole) {
        let first = joined[..start].chars().count();
        return (first..first + typed.len()).collect();
    }
    let mut places = Vec::new();
    let mut wanted = typed.iter().peekable();
    for (place, held) in name.iter().enumerate() {
        if wanted.peek() == Some(&held) {
            places.push(place);
            wanted.next();
        }
    }
    if wanted.peek().is_some() {
        return Vec::new();
    }
    places
}

/// Whether every word of `typed` starts a word of `title` or of `keywords`, ignoring case, and `typed` has a word.
fn every_word_known(title: &str, keywords: &str, typed: &str) -> bool {
    let known: Vec<String> = title
        .split_whitespace()
        .chain(keywords.split_whitespace())
        .map(str::to_lowercase)
        .collect();
    let mut words = typed.split_whitespace().peekable();
    words.peek().is_some()
        && words.all(|word| {
            let word = word.to_lowercase();
            known.iter().any(|held| held.starts_with(&word))
        })
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
            Self::Outline { items, .. } => items
                .iter()
                .enumerate()
                .filter(|(_, item)| narrows_to(&item.name, self.query().trim()))
                .map(|(place, _)| place)
                .collect(),
            Self::Files { matched, .. }
            | Self::Commands { matched, .. }
            | Self::Sessions { matched, .. }
            | Self::Choices { matched, .. } => matched.clone(),
            Self::Line { lines, .. } => line_named(self.query(), *lines)
                .map(|_| vec![0])
                .unwrap_or_default(),
            Self::Problems { problems } => (0..problems.len()).collect(),
            Self::Symbols { locations, .. } | Self::Search { locations, .. } => {
                (0..locations.len()).collect()
            }
        }
    }

    /// What the quick input lists while it holds these items, when it can hold them.
    pub fn quick(&self) -> Option<Quick> {
        match self {
            Self::Files { .. } => Some(Quick::Files),
            Self::Commands { .. } => Some(Quick::Commands),
            Self::Symbols { .. } => Some(Quick::Symbols),
            Self::Outline { .. } => Some(Quick::Outline),
            Self::Search { .. } => Some(Quick::Search),
            Self::Sessions { .. } => Some(Quick::Sessions),
            Self::Line { .. } => Some(Quick::Line),
            Self::Choices {
                choosing: crate::quick_choices::Choosing::Help,
                ..
            } => Some(Quick::Help),
            Self::Choices { .. } => Some(Quick::Choices),
            _ => None,
        }
    }

    /// The field narrowing the items, when they have one.
    pub fn filter(&self) -> Option<&Field> {
        match self {
            Self::Outline { filter, .. }
            | Self::Files { filter, .. }
            | Self::Commands { filter, .. }
            | Self::Sessions { filter, .. }
            | Self::Line { filter, .. }
            | Self::Choices { filter, .. }
            | Self::Symbols { filter, .. }
            | Self::Search { filter, .. } => Some(filter),
            _ => None,
        }
    }

    /// What the field says while nothing but the prefix is typed in it.
    pub fn placeholder(&self) -> &'static str {
        match self {
            Self::Choices { choosing, .. } => choosing.placeholder(),
            items => items.quick().unwrap_or(Quick::Files).placeholder(),
        }
    }

    /// What is typed to narrow the items, without the character that says what the quick input lists.
    pub fn query(&self) -> &str {
        let Some(filter) = self.filter() else {
            return "";
        };
        let prefix = self.quick().map_or("", Quick::prefix);
        filter.text().strip_prefix(prefix).unwrap_or(filter.text())
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
            Self::Commands { .. } => "Commands".to_owned(),
            Self::Sessions { .. } => "Sessions".to_owned(),
            Self::Line { .. } => "Go to line".to_owned(),
            Self::Choices { .. } => "Choices".to_owned(),
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
    /// Whether it is the quick input over the command center, whose first character says what it lists.
    pub quick: bool,
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
            quick: false,
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

    /// Opens the quick input at `at` over the items of the outline of the file shown, to go to one by typing part of its name.
    pub fn outline_requested(self, at: (f32, f32)) -> Self {
        self.quick_opened(Field::new(Quick::Outline.prefix()), at)
    }

    /// Opens the quick input at `at` over the files of the project the review panel is about, to open one by typing part of its path.
    pub fn file_finder_requested(self, at: (f32, f32)) -> Self {
        self.quick_opened(Field::new(Quick::Files.prefix()), at)
    }

    /// Opens the quick input at `at` over the commands that can be carried out where the keyboard is, to carry one out by typing part of its name.
    pub fn commands_requested(self, at: (f32, f32)) -> Self {
        self.quick_opened(Field::new(Quick::Commands.prefix()), at)
    }

    /// Opens the quick input at `at` over the symbols of the project, as the language server of the file shown finds them.
    pub fn symbols_requested(self, at: (f32, f32)) -> Self {
        self.quick_opened(Field::new(Quick::Symbols.prefix()), at)
    }

    /// Opens the quick input at `at` over the lines of the files of the project holding what is typed, starting from what is selected in the file shown.
    pub fn search_requested(self, at: (f32, f32)) -> Self {
        let selected = self
            .shown_document()
            .map(|document| document.field.selected().to_owned())
            .filter(|selected| !selected.is_empty() && !selected.contains('\n'))
            .unwrap_or_default();
        let typed = format!("{}{selected}", Quick::Search.prefix());
        self.quick_opened(Field::new(typed), at)
    }

    /// Opens the quick input at `at` over the sessions of every project, the latest changed first, to go to one by typing part of its name.
    pub fn sessions_requested(self, at: (f32, f32)) -> Self {
        self.quick_as(Quick::Sessions, Field::new(""), at)
    }

    /// The quick input at `at` holding `typed`, listing what its first character says.
    pub(crate) fn quick_typed(self, typed: &str, at: (f32, f32)) -> Self {
        self.quick_opened(Field::new(typed), at)
    }

    /// The quick input at `at` holding `filter`, listing what its first character says, or saying why it cannot.
    fn quick_opened(self, filter: Field, at: (f32, f32)) -> Self {
        let quick = Quick::of(filter.text());
        self.quick_as(quick, filter, at)
    }

    /// The quick input at `at` holding `filter` and listing what `quick` says, or saying why it cannot.
    fn quick_as(mut self, quick: Quick, filter: Field, at: (f32, f32)) -> Self {
        let query = filter
            .text()
            .strip_prefix(quick.prefix())
            .unwrap_or_default()
            .to_owned();
        let session = self.review_index();
        let mut picked = 0;
        let mut problem = None;
        let mut answered = true;
        let items = match quick {
            Quick::Choices => return self,
            Quick::Help => {
                return self.choices_requested(crate::quick_choices::Choosing::Help, at);
            }
            Quick::Line => {
                let (lines, current) = match self.shown_document() {
                    Some(document) => (
                        document.lines(),
                        document.line_of(document.field.caret()) + 1,
                    ),
                    None => {
                        problem = Some("Open a file to go to one of its lines".to_owned());
                        (0, 0)
                    }
                };
                Items::Line {
                    filter,
                    lines,
                    current,
                }
            }
            Quick::Sessions => {
                let shown = match self.page {
                    crate::state::Page::Session(index) => Some(index),
                    _ => None,
                };
                let mut order: Vec<usize> = (0..self.sessions.len())
                    .filter(|index| Some(*index) != shown && !self.sessions[*index].closed)
                    .collect();
                order.sort_by_key(|index| std::cmp::Reverse(self.sessions[*index].updated_at));
                let projects = self.projects.iter().enumerate().map(|(index, project)| {
                    (
                        Destination::Project(index),
                        project.name.clone(),
                        String::new(),
                    )
                });
                let sessions: Vec<(Destination, String, String)> = order
                    .into_iter()
                    .map(|index| {
                        let session = &self.sessions[index];
                        let project = self
                            .projects
                            .get(session.project)
                            .map_or_else(String::new, |project| project.name.clone());
                        let beside = format!("{project} · {}", session.age());
                        (Destination::Session(index), session.title.clone(), beside)
                    })
                    .chain(projects)
                    .collect();
                if sessions.is_empty() {
                    problem = Some("Add a project to start a session".to_owned());
                }
                let matched = matching_sessions(&sessions, &query);
                Items::Sessions {
                    sessions,
                    filter,
                    matched,
                }
            }
            Quick::Commands => {
                let coding = self.shown_document().is_some();
                let keymap = self.editor.keymap();
                let commands: Vec<(Action, Option<String>)> = Action::ALL
                    .into_iter()
                    .filter(|action| *action != Action::CommandPalette && !action.in_terminal())
                    .filter(|action| coding || action.everywhere())
                    .map(|action| (action, keymap.label(action)))
                    .collect();
                let extras = self.extension_commands();
                let (matched, recent) =
                    matching_commands(&commands, &extras, &query, &self.recent_commands);
                Items::Commands {
                    commands,
                    extras,
                    filter,
                    matched,
                    recent,
                }
            }
            Quick::Files => {
                if session.is_none() {
                    problem = Some("Open a project to go to its files".to_owned());
                }
                let paths = if session.is_some() {
                    self.files.clone()
                } else {
                    Vec::new()
                };
                let (matched, recent) = matching_recent_files(&paths, &query, &self.recent_files);
                Items::Files {
                    paths,
                    filter,
                    matched,
                    recent,
                }
            }
            Quick::Outline => {
                let items = match self.shown_document() {
                    Some(document) => {
                        let items = document
                            .syntax
                            .as_ref()
                            .map(|syntax| syntax.outline(document.field.text()))
                            .unwrap_or_default();
                        let caret = document.field.caret();
                        picked = items
                            .iter()
                            .rposition(|item| item.range.start <= caret && caret <= item.range.end)
                            .filter(|_| query.is_empty())
                            .unwrap_or(0);
                        items
                    }
                    None => {
                        problem = Some("Open a file to go to its symbols".to_owned());
                        Vec::new()
                    }
                };
                Items::Outline { items, filter }
            }
            Quick::Symbols => {
                let path = self
                    .shown_viewer()
                    .filter(|_| self.coding())
                    .or_else(|| self.git.viewers.first())
                    .map(|viewer| viewer.path.clone())
                    .unwrap_or_default();
                if path.is_empty() || session.is_none() {
                    problem = Some("Open a file to go to the symbols of its project".to_owned());
                } else if !query.is_empty() {
                    answered = false;
                    self.outbox.push(Outgoing::WorkspaceSymbols {
                        session: session.unwrap_or_default(),
                        path: path.clone(),
                        query: query.clone(),
                    });
                }
                Items::Symbols {
                    path,
                    filter,
                    locations: Vec::new(),
                }
            }
            Quick::Search => {
                match session {
                    None => problem = Some("Open a project to search its files".to_owned()),
                    Some(session) if !query.is_empty() => {
                        answered = false;
                        self.outbox.push(Outgoing::SearchFiles {
                            session,
                            query: query.clone(),
                        });
                    }
                    Some(_) => {}
                }
                Items::Search {
                    filter,
                    locations: Vec::new(),
                }
            }
        };
        self.listing = Some(Listing {
            session: session.unwrap_or_default(),
            items,
            picked,
            answered,
            problem,
            at,
            quick: true,
        });
        self
    }

    /// The state after `action` was carried out from the quick input: it leads the commands used recently.
    pub fn command_used(mut self, action: Action) -> Self {
        self.recent_commands.retain(|held| *held != action);
        self.recent_commands.insert(0, action);
        self.recent_commands.truncate(RECENT_COMMANDS);
        self
    }

    /// The command that taking the row `row` of the list, or the row picked when there is none, carries out, when the list holds commands.
    pub fn command_taken(&self, row: Option<usize>) -> Option<Action> {
        let listing = self.listing.as_ref()?;
        let Items::Commands { commands, .. } = &listing.items else {
            return None;
        };
        let index = *listing.items.rows().get(row.unwrap_or(listing.picked))?;
        commands.get(index).map(|(action, _)| *action)
    }

    /// The command an extension brings that taking the row `row` of the list, or the row picked when there is none, runs, with the machine it runs on, when the list holds commands and that row is one of them.
    pub fn extension_command_taken(
        &self,
        row: Option<usize>,
    ) -> Option<(Option<String>, ExtensionCommand)> {
        let listing = self.listing.as_ref()?;
        let Items::Commands {
            commands, extras, ..
        } = &listing.items
        else {
            return None;
        };
        let index = *listing.items.rows().get(row.unwrap_or(listing.picked))?;
        extras.get(index.checked_sub(commands.len())?).cloned()
    }

    /// The field narrowing the list, when the list has one.
    pub fn listing_filter(&self) -> Option<&Field> {
        self.listing.as_ref()?.items.filter()
    }

    /// The state after the field narrowing the list was changed by `edit`: the first row is picked, and the quick input lists something else when its first character says to.
    pub fn listing_filtered(mut self, edit: Edit) -> Self {
        let has_session = self.review_index().is_some();
        let Some(listing) = self.listing.as_mut() else {
            return self;
        };
        if listing.quick
            && let Some(filter) = listing.items.filter()
        {
            let mut changed = filter.clone();
            changed.apply(edit.clone());
            let held = listing.items.quick().unwrap_or(Quick::Files);
            let wanted = held.after_typing(changed.text());
            if held != wanted {
                let at = listing.at;
                return self.quick_as(wanted, changed, at);
            }
        }
        let recent = self.recent_commands.clone();
        let recent_files = self.recent_files.clone();
        let session = listing.session;
        let items = &mut listing.items;
        let before = items.query().to_owned();
        match items {
            Items::Outline { filter, .. } => filter.apply(edit),
            Items::Files { filter, .. }
            | Items::Commands { filter, .. }
            | Items::Sessions { filter, .. }
            | Items::Line { filter, .. }
            | Items::Choices { filter, .. }
            | Items::Symbols { filter, .. }
            | Items::Search { filter, .. } => filter.apply(edit),
            _ => return self,
        }
        let query = items.query().to_owned();
        if query == before {
            return self;
        }
        match items {
            Items::Files {
                paths,
                matched,
                recent: count,
                ..
            } => (*matched, *count) = matching_recent_files(paths, &query, &recent_files),
            Items::Sessions {
                sessions, matched, ..
            } => *matched = matching_sessions(sessions, &query),
            Items::Choices {
                choosing,
                choices,
                matched,
                ..
            } => {
                *choices =
                    WorkspaceState::with_new_branch(*choosing, std::mem::take(choices), &query);
                *matched = matching_choices(choices, &query);
            }
            Items::Commands {
                commands,
                extras,
                matched,
                recent: count,
                ..
            } => (*matched, *count) = matching_commands(commands, extras, &query, &recent),
            Items::Symbols { path, .. } if !path.is_empty() => {
                let request = Outgoing::WorkspaceSymbols {
                    session,
                    path: path.clone(),
                    query,
                };
                listing.answered = false;
                listing.problem = None;
                self.outbox.push(request);
            }
            Items::Search { .. } if has_session && !query.is_empty() => {
                listing.answered = false;
                listing.problem = None;
                self.outbox.push(Outgoing::SearchFiles { session, query });
            }
            _ => {}
        }
        if let Some(listing) = self.listing.as_mut() {
            listing.picked = 0;
        }
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

    /// The state after the server named `locations`, the symbols or the lines of the project found for `query`, when that is still what is typed.
    pub fn with_symbols(mut self, session: usize, query: &str, locations: Vec<Location>) -> Self {
        if let Some(listing) = self.listing.as_mut()
            && listing.session == session
            && listing.items.query() == query
            && let Items::Symbols {
                locations: held, ..
            }
            | Items::Search {
                locations: held, ..
            } = &mut listing.items
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
        let typed = listing.items.query().to_owned();
        let at = listing.at;
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
            Items::Commands { .. } => self,
            Items::Line { lines, .. } => {
                let Some(number) = line_named(&typed, lines) else {
                    return self;
                };
                self.remember_spot();
                let edit = self
                    .shown_document()
                    .map(|document| document.go_to_line_edit(number));
                if let Some(edit) = edit {
                    self.file_edited(edit);
                }
                self
            }
            Items::Choices { choices, .. } => match choices.into_iter().nth(index) {
                Some(choice) => self.choice_taken(choice, at),
                None => self,
            },
            Items::Sessions { sessions, .. } => match sessions.get(index) {
                Some((Destination::Session(session), ..)) => {
                    self.after(FocusId(crate::state::SESSION_BASE + *session as u64))
                }
                Some((Destination::Project(project), ..)) => {
                    self.after(FocusId(crate::state::NEW_IN_BASE + *project as u64))
                }
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
