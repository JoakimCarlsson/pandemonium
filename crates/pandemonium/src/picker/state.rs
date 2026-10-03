//! What the picker is offering, what has been typed, and what that narrows to.
//!
//! One picker stands behind every list the window asks a reader to choose
//! from — commands, files, projects, symbols, the matches of a search — and
//! behind the two prompts that ask for a line of text instead. They differ
//! in what fills the list and what choosing does, which is the caller's; the
//! typing, the filtering and the selection are the same every time and are
//! here.

use std::path::PathBuf;

use pm_core::{ProjectId, Scope, SessionId};
use pm_text::Position;

use crate::agent::TalkId;
use crate::config::FontSlot;
use crate::field::Field;
use crate::keymap::Action;
use crate::terminal::ShellId;

/// What a query starts with to ask the file picker for commands instead.
const COMMAND_PREFIX: char = '>';

/// What a query starts with to ask the file picker for workspace symbols.
const SYMBOL_PREFIX: char = '#';

/// How many rows the picker keeps after filtering.
const SHOWN: usize = 200;

/// What a picker is offering to pick.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Kind {
    /// Every command the window can carry out.
    Commands,
    /// Every server the editor can install.
    LanguageServers,
    /// Language packages offered by the maintained catalogue.
    LanguageExtensions,
    /// The language the Language Settings section shows.
    SettingsLanguage,
    /// The command line a language's files are piped through, by language name.
    LanguageFormatter(&'static str),
    /// Every file of the worktree the window is pointed at.
    Files,
    /// The projects the window holds open.
    Projects,
    /// Sessions in every open project.
    Sessions,
    /// The local branches of the active project.
    Branches,
    /// The active repository's saved stashes.
    Stashes,
    /// A message for saving a stash.
    StashMessage,
    /// A remote to fetch from.
    FetchRemotes,
    /// A remote to push to.
    PushRemotes,
    /// The symbols of the file the focused pane is showing.
    Symbols,
    /// The errors and warnings of every open file.
    Problems,
    /// Everywhere the symbol under the cursor is used.
    References,
    /// The symbols of the focused file's workspace whose names match the
    /// query, over the files of the worktree in front.
    WorkspaceSymbols,
    /// Whatever calls, or is called by, the symbol under the cursor.
    Calls,
    /// The logs of the language servers behind the focused file.
    ServerLogs,
    /// Every place a query was found in the worktree the window is pointed at.
    Search,
    /// A line number to go to, which is a prompt rather than a list.
    Line,
    /// A new name for the symbol under the cursor, which is also a prompt.
    Rename,
    /// What to call the terminal, which is a prompt too.
    RenameTerminal(ShellId),
    /// Create or rename a project group, optionally grouping one project.
    ProjectGroup(Option<usize>, Option<pm_core::ProjectId>),
    /// The name of a local branch to create and check out.
    NewBranch,
    /// The agents the editor can start in the active project's worktree.
    Agents,
    /// Saved conversations offered by the focused agent.
    AgentHistory(TalkId),
    /// Saved conversations offered by the focused agent, to have one forgotten.
    AgentDelete(TalkId),
    /// What to call the session about to be cut.
    NewSession,
    /// Which repositories of the active project the session about to be cut
    /// works in.
    SessionRepositories,
    /// The URL of a repository to clone and open.
    CloneUrl,
    /// A path to symlink into every new worktree.
    LinkedPath,
    /// A path to copy into every new worktree.
    CopiedPath,
    /// The variable a session's port is handed to its programs in.
    PortVariable,
    /// The family one kind of text is set in.
    Font(FontSlot),
    /// What to repaint one colour of the theme in, by index into its tokens.
    ThemeColor(usize),
    /// What to call the theme about to be written down.
    ThemeName,
    /// What to call the keymap about to be written down.
    KeymapName,
    /// The modes the agent of the session in hand can be put into.
    Modes,
    /// The values one of that agent's knobs takes: its models, say.
    Knob,
    /// What the worktree in front can be debugged as.
    Debug,
    /// Visible processes to attach to.
    Processes,
    /// Installed adapters able to attach to the selected process.
    AttachAdapters,
    /// Edit a breakpoint's condition.
    BreakpointCondition,
    /// Edit a breakpoint's hit count.
    BreakpointHits,
    /// Edit a breakpoint's log message.
    BreakpointLog,
    /// Add or edit a watch expression.
    Watch,
    /// Tasks offered by the worktree in front.
    Tasks,
}

impl Kind {
    /// What the field says while nothing has been typed into it.
    pub fn placeholder(self) -> &'static str {
        match self {
            Self::Commands => "Run a command",
            Self::LanguageServers => "Install Language Server…",
            Self::LanguageExtensions => "Install Language Support…",
            Self::SettingsLanguage => "Choose a language",
            Self::LanguageFormatter(_) => {
                "Command that reads the file on stdin, e.g. prettier --stdin-filepath {path}"
            }
            Self::Files => "Search files by name, > for commands, # for symbols",
            Self::Sessions => "Go to a session",
            Self::Projects => "Go to a project",
            Self::Branches => "Switch or type to create a branch…",
            Self::Stashes => "Choose a stash",
            Self::StashMessage => "Stash message (optional)",
            Self::FetchRemotes => "Pick which remote to fetch",
            Self::PushRemotes => "Pick which remote to push to",
            Self::Symbols => "Go to a symbol",
            Self::Problems => "Go to a problem",
            Self::References => "Go to a use of this symbol",
            Self::WorkspaceSymbols => "Go to a symbol in the workspace",
            Self::Calls => "Go to a call",
            Self::ServerLogs => "Open a language server's log",
            Self::Search => "Search this worktree",
            Self::Line => "Go to line",
            Self::Rename => "New name",
            Self::RenameTerminal(_) => "What the terminal is called",
            Self::ProjectGroup(..) => "Group name",
            Self::NewBranch => "Name of the new branch",
            Self::NewSession => "What the session is called",
            Self::SessionRepositories => "Pick the repositories this session works in",
            Self::CloneUrl => "The repository to clone",
            Self::LinkedPath => "Path to link into new worktrees",
            Self::CopiedPath => "Path to copy into new worktrees",
            Self::PortVariable => "Variable to hand a session's port in",
            Self::Font(slot) => slot.placeholder(),
            Self::ThemeColor(_) => "#rrggbb, or #rrggbbaa",
            Self::ThemeName => "What the theme is called",
            Self::KeymapName => "What the keymap is called",
            Self::Agents => "Start an agent in this worktree",
            Self::AgentHistory(_) => "Search agent history",
            Self::AgentDelete(_) => "Choose a saved session to delete",
            Self::Modes => "Put this agent into a mode",
            Self::Knob => "Set this to one of what it takes",
            Self::Debug => "Debug this worktree as",
            Self::Processes => "Attach to a process",
            Self::AttachAdapters => "Attach using an adapter",
            Self::BreakpointCondition => "Condition as the adapter reads it",
            Self::BreakpointHits => "Hit count, e.g. 5 or >= 5",
            Self::BreakpointLog => "Log message, e.g. i is {i}",
            Self::Watch => "Watch expression",
            Self::Tasks => "Run a task in this worktree",
        }
    }

    /// Whether the picker is a prompt for a line of text rather than a list.
    pub fn is_prompt(self) -> bool {
        matches!(
            self,
            Self::Line
                | Self::BreakpointCondition
                | Self::BreakpointHits
                | Self::BreakpointLog
                | Self::Watch
                | Self::Rename
                | Self::RenameTerminal(_)
                | Self::ProjectGroup(..)
                | Self::NewBranch
                | Self::StashMessage
                | Self::NewSession
                | Self::CloneUrl
                | Self::LinkedPath
                | Self::CopiedPath
                | Self::PortVariable
                | Self::ThemeColor(_)
                | Self::ThemeName
                | Self::LanguageFormatter(_)
                | Self::KeymapName
        )
    }

    /// Whether what is typed changes the rows rather than filtering them.
    ///
    /// A search is run against the worktrees each time the query changes;
    /// every other list is gathered once when it opens and narrowed after.
    pub fn is_queried(self) -> bool {
        self == Self::Search
    }

    /// The character a query starts with to turn the file picker into this
    /// kind of list, for the kinds that are modes of it.
    fn prefix(self) -> Option<char> {
        match self {
            Self::Commands => Some(COMMAND_PREFIX),
            Self::WorkspaceSymbols => Some(SYMBOL_PREFIX),
            _ => None,
        }
    }

    /// Whether this kind is the file picker or one of its prefixed modes.
    fn is_quick_open(self) -> bool {
        self == Self::Files || self.prefix().is_some()
    }

    /// What is typed, less the prefix that chose this kind of list.
    pub fn query(self, typed: &str) -> &str {
        self.prefix()
            .and_then(|prefix| typed.strip_prefix(prefix))
            .unwrap_or(typed)
            .trim_start()
    }

    /// What is seeded into the field when a picker of this kind is opened.
    pub fn seed(self) -> String {
        self.prefix().map(String::from).unwrap_or_default()
    }
}

/// What choosing one row does.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Choice {
    /// Points at an existing session worktree.
    Session(SessionId, crate::health::Health),
    /// Carry out this command.
    Act(Action),
    /// Install this language server.
    InstallLanguageServer(&'static str),
    /// Show the settings of this language.
    SettingsLanguage(&'static str),
    /// Install this language package after reviewing its catalogue metadata.
    InstallLanguageExtension(usize),
    /// Open this file of this worktree.
    Open(Scope, PathBuf),
    /// Open this file of this worktree and go to this place in it.
    OpenAt(Scope, PathBuf, Position),
    /// Make this the project the window's files and commands apply to.
    Project(ProjectId),
    /// Check out this local branch of this project.
    Branch(ProjectId, String),
    /// Act on a stash in the active repository.
    Stash(usize),
    /// Fetch this project's named remote.
    FetchRemote(ProjectId, String),
    /// Push this project to the named remote.
    PushRemote(ProjectId, String),
    /// Start this agent in the active project's worktree.
    Agent(pm_acp::Agent),
    /// Open a saved conversation from the named running agent.
    AgentHistory(TalkId, String),
    /// Have the named running agent forget a saved conversation.
    AgentDelete(TalkId, String),
    /// Put this session into the mode this names.
    Mode(TalkId, String),
    /// Set this session's knob to the value this names.
    Knob(TalkId, String, String),
    /// Set this kind of text in this family, or in the editor's pick.
    Font(FontSlot, Option<String>),
    /// Debug this worktree as this scenario.
    Debug(Scope, Box<pm_dap::Scenario>),
    /// A process selected for attaching.
    Process(u32),
    /// Run this task in its worktree.
    Task(Scope, Box<pm_core::Task>),
    /// Tick or untick this repository for the session about to be cut.
    SessionRepository(PathBuf),
    /// Cut the session about to be cut, of the repositories ticked.
    StartSession,
}

/// One thing the picker is offering.
#[derive(Clone, Debug)]
pub struct Row {
    /// The group this row belongs to, when a picker separates its choices.
    pub section: Option<&'static str>,
    /// What the row is called, and what the query is matched against.
    pub label: String,
    /// What is said beside it: a path, a keybinding, a line of context.
    pub detail: String,
    /// What choosing it does.
    pub choice: Choice,
    /// Whether it can be chosen at all.
    pub enabled: bool,
}

/// One picker: what it offers, what has been typed, and what is selected.
pub struct Picker {
    /// What it is picking.
    kind: Kind,
    /// What has been typed into it.
    field: Field,
    /// Everything it was given to offer.
    rows: Vec<Row>,
    /// Which of them the query leaves, best match first.
    matched: Vec<usize>,
    /// Every row the last query matched, with how well, in the order given.
    candidates: Vec<(i32, usize)>,
    /// The query `candidates` were narrowed by, while they stand for one.
    ///
    /// A query that only adds to this one can match no row this one did not,
    /// so typing onward scores the candidates rather than every row.
    narrowed_by: Option<String>,
    /// Which of those is selected.
    selected: usize,
}

impl Picker {
    /// A picker of `kind` offering `rows`, with `seeded` already typed in.
    pub fn new(kind: Kind, rows: Vec<Row>, seeded: &str) -> Self {
        let mut picker = Self {
            kind,
            field: Field::filled(seeded),
            rows,
            matched: Vec::new(),
            candidates: Vec::new(),
            narrowed_by: None,
            selected: 0,
        };
        picker.filter();
        picker
    }

    /// What it is picking.
    pub fn kind(&self) -> Kind {
        self.kind
    }

    /// What has been typed into it.
    pub fn field(&self) -> &Field {
        &self.field
    }

    /// What has been typed into it, to be typed into.
    pub fn field_mut(&mut self) -> &mut Field {
        &mut self.field
    }

    /// Puts the field through `edit` and narrows the rows to what is left.
    pub fn edit(&mut self, edit: impl FnOnce(&mut Field)) {
        edit(&mut self.field);
        self.filter();
    }

    /// Which list what has been typed asks for instead of this one, when it
    /// asks for another: files turn into commands or symbols at their prefix,
    /// and back into files once it is taken away.
    pub fn switched(&self) -> Option<Kind> {
        if !self.kind.is_quick_open() {
            return None;
        }
        let typed = self.field.value().chars().next();
        let wanted = [Kind::Commands, Kind::WorkspaceSymbols]
            .into_iter()
            .find(|kind| kind.prefix() == typed)
            .unwrap_or(Kind::Files);
        (wanted != self.kind).then_some(wanted)
    }

    /// Offers `rows` as a picker of `kind`, keeping what has been typed.
    pub fn switch(&mut self, kind: Kind, rows: Vec<Row>) {
        self.kind = kind;
        self.refill(rows);
    }

    /// Offers `rows` instead, narrowed by what has been typed.
    pub fn refill(&mut self, rows: Vec<Row>) {
        self.rows = rows;
        self.narrowed_by = None;
        self.filter();
    }

    /// Offers `rows` as well, after the rows already offered, narrowed by what
    /// has been typed and keeping the selection where it is.
    ///
    /// This is how a list that is still being gathered grows: only the rows
    /// that arrive are scored, not the ones already offered.
    pub fn extend(&mut self, rows: Vec<Row>) {
        let start = self.rows.len();
        self.rows.extend(rows);
        let limit = self.limit();
        match self.narrowed_by.as_deref() {
            Some(query) => {
                let pattern = pattern(query);
                let found = (start..self.rows.len())
                    .filter_map(|index| self.scored(&pattern, index))
                    .collect::<Vec<_>>();
                self.candidates.extend(found);
                self.matched = ranked(self.candidates.clone(), limit);
            }
            None => {
                let room = limit.saturating_sub(self.matched.len());
                self.matched.extend((start..self.rows.len()).take(room));
            }
        }
        self.selected = self.selected.min(self.matched.len().saturating_sub(1));
    }

    /// Replaces rows while keeping the selected choice when it is still shown.
    pub fn refill_preserving_selection(&mut self, rows: Vec<Row>) {
        let chosen = self.chosen().cloned();
        self.refill(rows);
        let selected = chosen.and_then(|chosen| {
            self.shown()
                .find(|(_, row)| row.choice == chosen)
                .map(|(place, _)| place)
        });
        if let Some(place) = selected {
            self.selected = place;
        }
    }

    /// The rows the query leaves, best match first.
    pub fn shown(&self) -> impl Iterator<Item = (usize, &Row)> {
        self.matched
            .iter()
            .enumerate()
            .map(|(place, index)| (place, &self.rows[*index]))
    }

    /// How many rows the current query leaves.
    pub fn shown_count(&self) -> usize {
        self.matched.len()
    }

    /// Every row offered before the query narrows them.
    pub fn rows(&self) -> impl Iterator<Item = &Row> {
        self.rows.iter()
    }

    /// Which of the shown rows is selected.
    pub fn selected(&self) -> usize {
        self.selected
    }

    /// Selects the `place`-th shown row.
    pub fn select(&mut self, place: usize) {
        self.selected = place.min(self.matched.len().saturating_sub(1));
    }

    /// Moves the selection `step` rows along, wrapping around at either end.
    pub fn step(&mut self, step: isize) {
        let count = self.matched.len();
        if count == 0 {
            return;
        }
        self.selected = (self.selected as isize + step).rem_euclid(count as isize) as usize;
    }

    /// What choosing the selected row would do, if it can be chosen.
    pub fn chosen(&self) -> Option<&Choice> {
        let row = self.rows.get(*self.matched.get(self.selected)?)?;
        row.enabled.then_some(&row.choice)
    }

    /// Narrows the rows to the ones the query matches, best match first.
    pub fn filter(&mut self) {
        let query = self.kind.query(self.field.value());
        let limit = self.limit();
        if self.kind.is_prompt() || self.kind.is_queried() || query.is_empty() {
            self.narrowed_by = None;
            self.candidates.clear();
            self.matched = (0..self.rows.len()).take(limit).collect();
            self.selected = 0;
            return;
        }

        let pattern = pattern(query);
        let narrowing = self
            .narrowed_by
            .as_deref()
            .is_some_and(|before| query.starts_with(before));
        let candidates = match narrowing {
            true => self
                .candidates
                .iter()
                .filter_map(|(_, index)| self.scored(&pattern, *index))
                .collect::<Vec<_>>(),
            false => (0..self.rows.len())
                .filter_map(|index| self.scored(&pattern, index))
                .collect(),
        };
        self.narrowed_by = Some(query.to_owned());
        self.matched = ranked(candidates.clone(), limit);
        self.candidates = candidates;
        self.selected = 0;
    }

    /// How well the `index`-th row matches `pattern`, by its label or else,
    /// at half the weight, by what is said beside it.
    fn scored(&self, pattern: &[char], index: usize) -> Option<(i32, usize)> {
        let row = &self.rows[index];
        score(pattern, &row.label)
            .or_else(|| score(pattern, &row.detail).map(|score| score / 2))
            .map(|score| (score, index))
    }

    /// How many rows the picker keeps after filtering.
    fn limit(&self) -> usize {
        match self.kind {
            Kind::AgentHistory(_) | Kind::AgentDelete(_) => usize::MAX,
            _ => SHOWN,
        }
    }
}

/// What `query` asks to be matched: its letters lower-cased, without the
/// spaces between them.
fn pattern(query: &str) -> Vec<char> {
    query
        .chars()
        .filter(|ch| !ch.is_whitespace())
        .flat_map(char::to_lowercase)
        .collect()
}

/// The first `limit` of `scored`'s rows, best score first and in the order
/// given among equals.
///
/// Only the rows kept are sorted: the rest are set aside by selection, which
/// is what keeps a first letter typed over a hundred thousand files cheap.
fn ranked(mut scored: Vec<(i32, usize)>, limit: usize) -> Vec<usize> {
    let order =
        |left: &(i32, usize), right: &(i32, usize)| right.0.cmp(&left.0).then(left.1.cmp(&right.1));
    if scored.len() > limit {
        scored.select_nth_unstable_by(limit, order);
        scored.truncate(limit);
    }
    scored.sort_unstable_by(order);
    scored.into_iter().map(|(_, index)| index).collect()
}

/// How well `haystack` matches `pattern`, or nothing when it does not.
///
/// The score rewards what a reader typing a few letters is aiming at: the
/// letters in order, close together, and at the start of a word. It is a
/// ranking and not a distance, so only the order of the scores matters.
/// `pattern` is already lower-cased, and `haystack` is lower-cased a letter
/// at a time as it is read, so scoring a row allocates nothing.
fn score(pattern: &[char], haystack: &str) -> Option<i32> {
    let mut wanted = pattern.iter().copied().peekable();
    if wanted.peek().is_none() {
        return Some(0);
    }

    let mut score = 0;
    let mut last = None;
    let mut before = None;
    let mut index = 0usize;
    let mut counted = 0;
    let mut letters = haystack.chars();
    for ch in letters.by_ref() {
        counted += 1;
        for lower in ch.to_lowercase() {
            if wanted.next_if_eq(&lower).is_some() {
                score += 10;
                if last == Some(index.wrapping_sub(1)) {
                    score += 12;
                }
                let starts_word =
                    before.is_none_or(|before| matches!(before, ' ' | '_' | '-' | '/' | '.' | ':'));
                if starts_word {
                    score += 8;
                }
                last = Some(index);
            }
            before = Some(lower);
            index += 1;
        }
        if wanted.peek().is_none() {
            break;
        }
    }

    if wanted.peek().is_some() {
        return None;
    }
    Some(score - (counted + letters.count()) as i32 / 4)
}
