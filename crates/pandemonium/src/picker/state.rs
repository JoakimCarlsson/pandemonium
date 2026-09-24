//! What the picker is offering, what has been typed, and what that narrows to.
//!
//! One picker stands behind every list the window asks a reader to choose
//! from — commands, files, projects, symbols, the matches of a search — and
//! behind the two prompts that ask for a line of text instead. They differ
//! in what fills the list and what choosing does, which is the caller's; the
//! typing, the filtering and the selection are the same every time and are
//! here.

use std::path::PathBuf;

use pm_core::{ProjectId, Scope};
use pm_text::Position;

use crate::agent::TalkId;
use crate::config::FontSlot;
use crate::field::Field;
use crate::keymap::Action;

/// How many rows the picker keeps after filtering.
const SHOWN: usize = 200;

/// What a picker is offering to pick.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Kind {
    /// Every command the window can carry out.
    Commands,
    /// Every file of every open project.
    Files,
    /// The projects the window holds open.
    Projects,
    /// The local branches of the active project.
    Branches,
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
    /// The symbols of the focused file's workspace whose names match the query.
    WorkspaceSymbols,
    /// Whatever calls, or is called by, the symbol under the cursor.
    Calls,
    /// Every place a query was found across the open projects.
    Search,
    /// A line number to go to, which is a prompt rather than a list.
    Line,
    /// A new name for the symbol under the cursor, which is also a prompt.
    Rename,
    /// The name of a local branch to create and check out.
    NewBranch,
    /// The agents the editor can start in the active project's worktree.
    Agents,
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
}

impl Kind {
    /// What the field says while nothing has been typed into it.
    pub fn placeholder(self) -> &'static str {
        match self {
            Self::Commands => "Run a command",
            Self::Files => "Open a file by name",
            Self::Projects => "Go to a project",
            Self::Branches => "Switch or type to create a branch…",
            Self::FetchRemotes => "Pick which remote to fetch",
            Self::PushRemotes => "Pick which remote to push to",
            Self::Symbols => "Go to a symbol",
            Self::Problems => "Go to a problem",
            Self::References => "Go to a use of this symbol",
            Self::WorkspaceSymbols => "Go to a symbol in the workspace",
            Self::Calls => "Go to a call",
            Self::Search => "Search every open project",
            Self::Line => "Go to line",
            Self::Rename => "New name",
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
            Self::Modes => "Put this agent into a mode",
            Self::Knob => "Set this to one of what it takes",
            Self::Debug => "Debug this worktree as",
        }
    }

    /// Whether the picker is a prompt for a line of text rather than a list.
    pub fn is_prompt(self) -> bool {
        matches!(
            self,
            Self::Line
                | Self::Rename
                | Self::NewBranch
                | Self::NewSession
                | Self::CloneUrl
                | Self::LinkedPath
                | Self::CopiedPath
                | Self::PortVariable
                | Self::ThemeColor(_)
                | Self::ThemeName
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
}

/// What choosing one row does.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Choice {
    /// Carry out this command.
    Act(Action),
    /// Open this file of this worktree.
    Open(Scope, PathBuf),
    /// Open this file of this worktree and go to this place in it.
    OpenAt(Scope, PathBuf, Position),
    /// Make this the project the window's files and commands apply to.
    Project(ProjectId),
    /// Check out this local branch of this project.
    Branch(ProjectId, String),
    /// Fetch this project's named remote.
    FetchRemote(ProjectId, String),
    /// Push this project to the named remote.
    PushRemote(ProjectId, String),
    /// Start this agent in the active project's worktree.
    Agent(pm_acp::Agent),
    /// Put this session into the mode this names.
    Mode(TalkId, String),
    /// Set this session's knob to the value this names.
    Knob(TalkId, String, String),
    /// Set this kind of text in this family, or in the editor's pick.
    Font(FontSlot, Option<String>),
    /// Debug this worktree as this scenario.
    Debug(Scope, Box<pm_dap::Scenario>),
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

    /// Puts the field through `edit` and narrows the rows to what is left.
    pub fn edit(&mut self, edit: impl FnOnce(&mut Field)) {
        edit(&mut self.field);
        self.filter();
    }

    /// Offers `rows` instead, narrowed by what has been typed.
    pub fn refill(&mut self, rows: Vec<Row>) {
        self.rows = rows;
        self.filter();
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
    fn filter(&mut self) {
        let query = self.field.value();
        if self.kind.is_prompt() || self.kind.is_queried() || query.is_empty() {
            self.matched = (0..self.rows.len()).take(SHOWN).collect();
            self.selected = 0;
            return;
        }

        let mut scored = self
            .rows
            .iter()
            .enumerate()
            .filter_map(|(index, row)| {
                let label = score(query, &row.label);
                let detail = score(query, &row.detail).map(|score| score / 2);
                label.or(detail).map(|score| (score, index))
            })
            .collect::<Vec<_>>();
        scored.sort_by(|left, right| right.0.cmp(&left.0).then(left.1.cmp(&right.1)));

        self.matched = scored
            .into_iter()
            .take(SHOWN)
            .map(|(_, index)| index)
            .collect();
        self.selected = 0;
    }
}

/// How well `haystack` matches `needle`, or nothing when it does not.
///
/// The score rewards what a reader typing a few letters is aiming at: the
/// letters in order, close together, and at the start of a word. It is a
/// ranking and not a distance, so only the order of the scores matters.
fn score(needle: &str, haystack: &str) -> Option<i32> {
    let subject = haystack.to_lowercase().chars().collect::<Vec<_>>();
    let pattern = needle.to_lowercase();
    let mut wanted = pattern.chars().filter(|ch| !ch.is_whitespace()).peekable();
    if wanted.peek().is_none() {
        return Some(0);
    }

    let mut score = 0;
    let mut last = None;
    for (index, ch) in subject.iter().enumerate() {
        let Some(want) = wanted.peek().copied() else {
            break;
        };
        if *ch != want {
            continue;
        }
        wanted.next();
        score += 10;
        if last == Some(index.wrapping_sub(1)) {
            score += 12;
        }
        let starts_word =
            index == 0 || matches!(subject[index - 1], ' ' | '_' | '-' | '/' | '.' | ':');
        if starts_word {
            score += 8;
        }
        last = Some(index);
    }

    if wanted.peek().is_some() {
        return None;
    }
    Some(score - haystack.chars().count() as i32 / 4)
}
