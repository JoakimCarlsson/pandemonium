//! What the reader does to the changes with git: picking several files, the menu on them, discarding, ignoring, reading a file and changing branch.

use pm_input::FocusId;
use pm_text::Field;

use crate::change_list::Group;
use crate::git_menus::{MENU_ROW_BASE, MenuKind, OpenMenu};
use crate::git_prompts::Prompt;
use crate::sample::{Change, Session};
use crate::state::{Outgoing, Page, WorkspaceState};

/// The button that opens the list of branches.
pub const BRANCH_BUTTON: FocusId = FocusId(7020);

/// The panel listing the branches.
pub const BRANCH_BODY: FocusId = FocusId(7021);

/// The box branches are searched in and a new one is named in.
pub const BRANCH_FILTER: FocusId = FocusId(7022);

/// The row that makes a branch of the name typed.
pub const BRANCH_CREATE: FocusId = FocusId(7023);

/// What Enter does in the box branches are searched in.
pub const BRANCH_SUBMIT: FocusId = FocusId(7024);

/// The first of the rows that check a branch out; the rest follow it.
pub const BRANCH_PICK_BASE: u64 = 8300;

/// How many branches the list shows at once.
pub const BRANCH_ROWS: usize = 12;

/// The first of the rows of the list of repositories that pick the one the panel works on, numbered as [`WorkspaceState::repositories`] lists them; the rest follow it.
pub const REPOSITORY_BASE: u64 = 8320;

/// The first of the buttons on the rows of the list of repositories that open each one's branches; the rest follow it.
pub const REPOSITORY_BRANCH_BASE: u64 = 8400;

/// How many repositories the list of repositories shows at most: the folder's own and its submodules.
pub const REPOSITORY_ROWS: usize = 40;

/// The scrolling body of the file being read.
pub const VIEWER_SCROLL: FocusId = FocusId(7032);

/// Dismisses the banner saying what git refused to do.
pub const TROUBLE_DISMISS: FocusId = FocusId(7069);

/// Closes whatever of these is open.
pub const REVIEW_DISMISS: FocusId = FocusId(7040);

/// Stages or unstages every tracked file that is not staged.
pub const GROUP_TRACKED: FocusId = FocusId(7051);

/// Stages or unstages every file git does not track yet.
pub const GROUP_UNTRACKED: FocusId = FocusId(7052);

/// Folds or unfolds the changes section.
pub const CHANGES_TOGGLE: FocusId = FocusId(7053);

/// Folds or unfolds the graph.
pub const GRAPH_TOGGLE: FocusId = FocusId(7054);

/// Switches the graph between the checked out branch and every reference.
pub const GRAPH_REFS: FocusId = FocusId(7055);

/// Scrolls the graph back to the commit the checked out branch is at.
pub const GRAPH_TARGET: FocusId = FocusId(7056);

/// Fetches from every remote.
pub const GRAPH_FETCH: FocusId = FocusId(7057);

/// Pulls the checked out branch.
pub const GRAPH_PULL: FocusId = FocusId(7058);

/// Pushes the checked out branch.
pub const GRAPH_PUSH: FocusId = FocusId(7059);

/// Reads the changes and the graph again.
pub const CHANGES_REFRESH: FocusId = FocusId(7060);

/// Opens the menu of what else the button under the commit message can do.
pub const COMMIT_CHEVRON: FocusId = FocusId(7062);

/// Opens the menu over the panel's actions.
pub const CHANGES_MORE: FocusId = FocusId(7070);

/// The handle between the changes and the graph, which is dragged to change how tall the graph is.
pub const GRAPH_SASH: FocusId = FocusId(7071);

/// The scrolling body of the graph.
pub const GRAPH_SCROLL: FocusId = FocusId(7065);

/// The first of the arrows that open a changed file; the rest follow it.
pub const OPEN_ARROW_BASE: u64 = 8500;

/// How many commits the graph lists.
pub const GRAPH_LIMIT: usize = 200;

/// What the button under the commit message does.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Primary {
    /// Commits, under `title`, unless `stopped` says why it cannot.
    Commit {
        /// What the button calls the commit it would make.
        title: &'static str,
        /// Why it cannot commit, when it cannot.
        stopped: Option<&'static str>,
    },
    /// Pulls what the branch is behind by and pushes what it is ahead by.
    Sync {
        /// Commits this branch has that the one it follows has not.
        ahead: usize,
        /// Commits the branch it follows has that this one has not.
        behind: usize,
    },
    /// Pushes a branch that follows nothing, and makes it follow what it was pushed to.
    Publish,
    /// Waits on the remote, saying what is being `doing`.
    Busy(&'static str),
}

/// What stops a commit when there is nothing to commit.
const NOTHING_TO_COMMIT: &str = "Nothing to commit";

/// How a press on a changed file adds to the files picked.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Pick {
    /// Adds the file, or takes it out when it is picked already.
    Toggle,
    /// Picks every file from the one picked last to this one, as they are listed.
    Range,
}

/// What the reader has picked and opened among the changes.
#[derive(Clone, Debug, PartialEq)]
pub struct GitControls {
    /// The changed files picked, by position, in the order they are in.
    pub marked: Vec<usize>,
    /// The file the keyboard is on.
    pub selected: Option<usize>,
    /// Where the range being marked was anchored, and what was marked before it began.
    pub gesture: Option<(usize, Vec<usize>)>,
    /// The menu that is open, if one is.
    pub menu: Option<OpenMenu>,
    /// The question or list that is open, if one is.
    pub prompt: Option<Prompt>,
    /// The box a stash is named in.
    pub stash_message: Field,
    /// The box a session is given a new title in.
    pub rename: Field,
    /// The box a new or renamed file or folder is named in, and what it is for, while the tree is asking.
    pub naming: Option<(crate::tree_actions::Naming, Field)>,
    /// The row of the tree last picked, where new files and folders go.
    pub tree_selected: Option<crate::tree_actions::TreeTarget>,
    /// The rows of the tree picked together, in the order they were picked.
    pub tree_marked: Vec<crate::tree_actions::TreeTarget>,
    /// Where the range of tree rows being marked was anchored, and what was marked before it began.
    pub tree_gesture: Option<(
        crate::tree_actions::TreeTarget,
        Vec<crate::tree_actions::TreeTarget>,
    )>,
    /// The file or folder of the tree the open menu or question is about.
    pub tree_target: Option<crate::tree_actions::TreeTarget>,
    /// What was cut or copied out of the tree.
    pub tree_clipboard: Option<crate::tree_actions::Clipboard>,
    /// The stashes of the open session's repository, newest first.
    pub stashes: Vec<pm_git::Stash>,
    /// The merge, rebase or cherry-pick the open session's folder is stopped in, if it is in one.
    pub operation: Option<pm_git::Operation>,
    /// How tall the graph is when it is open, in logical pixels.
    pub graph_height: f32,
    /// Whether the list of branches is open.
    pub branch_menu: bool,
    /// The branches of the repository the list shows, the one checked out first.
    pub branches: Vec<String>,
    /// The submodule of the open session's folder the panel works on, from the folder: its changes, commits, branches and remote; nothing for the folder's own repository.
    pub repository: Option<String>,
    /// What each checked out submodule of the open session's folder has checked out: its path and its branch, or `@` and the commit.
    pub submodule_heads: Vec<(String, String)>,
    /// What branches are searched for and a new one is named by.
    pub filter: Field,
    /// The files open in the window, one tab each, in the order the tabs are in.
    pub viewers: Vec<crate::open_files::Viewer>,
    /// The tab of the file shown when a file is what the main area shows.
    pub shown: Option<usize>,
    /// Whether the changes section is open.
    pub changes_open: bool,
    /// Whether the graph is open.
    pub graph_open: bool,
    /// Whether the graph lists every reference rather than what the checked out branch reaches.
    pub graph_all: bool,
    /// The commits of the graph, newest first.
    pub history: Vec<pm_git::Commit>,
    /// How far down the graph is scrolled, in logical pixels.
    pub graph_scroll: f32,
    /// What is being waited on from the remote, as the button words it.
    pub busy: Option<&'static str>,
    /// The session the graph was asked for, so that it is asked for once.
    pub logged: Option<usize>,
    /// Whether the server is busy with something asked about the folder: reading it, staging, committing, switching branch or talking to the remote.
    pub working: bool,
    /// What git refused to do, shown in the panel until something is asked of it again.
    pub trouble: Option<String>,
}

impl Default for GitControls {
    /// Nothing picked or open, with the changes and the graph unfolded.
    fn default() -> Self {
        Self {
            marked: Vec::new(),
            selected: None,
            gesture: None,
            menu: None,
            prompt: None,
            stash_message: Field::default(),
            rename: Field::default(),
            naming: None,
            tree_selected: None,
            tree_marked: Vec::new(),
            tree_gesture: None,
            tree_target: None,
            tree_clipboard: None,
            stashes: Vec::new(),
            operation: None,
            graph_height: crate::graph::HEIGHT,
            branch_menu: false,
            branches: Vec::new(),
            repository: None,
            submodule_heads: Vec::new(),
            filter: Field::default(),
            viewers: Vec::new(),
            shown: None,
            changes_open: true,
            graph_open: false,
            graph_all: false,
            history: Vec::new(),
            graph_scroll: 0.0,
            busy: None,
            logged: None,
            working: false,
            trouble: None,
        }
    }
}

impl GitControls {
    /// The branches whose name holds what was typed, at most as many as the list shows.
    pub fn matching(&self) -> Vec<&String> {
        let wanted = self.filter.text().trim().to_lowercase();
        self.branches
            .iter()
            .filter(|name| name.to_lowercase().contains(&wanted))
            .take(BRANCH_ROWS)
            .collect()
    }

    /// Whether a branch is called exactly what was typed.
    pub fn exists(&self) -> bool {
        let wanted = self.filter.text().trim();
        self.branches.iter().any(|name| name == wanted)
    }
}

impl WorkspaceState {
    /// Which changed file's row `id` is, if it is one.
    pub fn change_row_of(&self, id: FocusId) -> Option<usize> {
        id.0.checked_sub(crate::state::FILE_BASE)
            .map(|index| index as usize)
            .filter(|index| *index < self.change_count())
    }

    /// Which folder of the changed files' tree row `id` is, by its place among the folders, if it is one.
    pub fn change_folder_row_of(&self, id: FocusId) -> Option<usize> {
        id.0.checked_sub(crate::state::FOLDER_BASE)
            .map(|index| index as usize)
            .filter(|index| *index < self.change_count())
    }

    /// The state with the menu on changed files open at `at` over the folder at `position` of the tree, acting on every file in it.
    pub fn with_folder_change_menu(mut self, position: usize, at: (f32, f32)) -> Self {
        let files = self.review_session().and_then(|session| {
            crate::change_list::folder_table(&session.changes, self.git.repository.as_deref())
                .into_iter()
                .nth(position)
                .map(|folder| folder.files)
        });
        let Some(files) = files else {
            return self;
        };
        self.git.selected = files.first().copied();
        self.git.gesture = None;
        self.git.marked = files;
        self.with_git_menu(MenuKind::Change, at)
    }

    /// Whether pressing `id` leaves the list of branches open.
    pub fn keeps_branches(id: FocusId) -> bool {
        id == BRANCH_BUTTON
            || id == BRANCH_BODY
            || id == BRANCH_FILTER
            || id == BRANCH_CREATE
            || (BRANCH_PICK_BASE..BRANCH_PICK_BASE + BRANCH_ROWS as u64).contains(&id.0)
            || (REPOSITORY_BRANCH_BASE..REPOSITORY_BRANCH_BASE + REPOSITORY_ROWS as u64)
                .contains(&id.0)
    }

    /// Whether a menu of the panel, a question or the list of branches is open.
    pub fn review_overlay_open(&self) -> bool {
        self.git.menu.is_some() || self.git.prompt.is_some() || self.git.branch_menu
    }

    /// The state with the menus of the panel, the question and the list of branches closed.
    pub fn review_overlay_closed(mut self) -> Self {
        self.git.menu = None;
        self.git.prompt = None;
        self.git.branch_menu = false;
        self
    }

    /// The position of the session the review panel is about: the open one, or, on the page for starting work or a project's overview, the stand-in for the project's own folder.
    pub fn review_index(&self) -> Option<usize> {
        match self.page {
            Page::Session(index) => Some(index),
            Page::New(project) | Page::Project(project) => self
                .sessions
                .iter()
                .position(|session| session.home && session.project == project),
            Page::History | Page::Extensions => None,
        }
    }

    /// The session the review panel is about, if there is one.
    pub fn review_session(&self) -> Option<&Session> {
        self.review_index()
            .and_then(|index| self.sessions.get(index))
    }

    /// Makes the stand-in for the folder of the project at `project` when there is none, and asks for what its folder holds the first time the page of that project is shown.
    pub(crate) fn read_home(&mut self, project: usize) {
        let Some(held) = self.projects.get(project) else {
            return;
        };
        let directory = held.directory().to_string_lossy().into_owned();
        let position = match self.review_index() {
            Some(position) => position,
            None => {
                self.sessions.push(Session::home(project, &directory));
                self.sessions.len() - 1
            }
        };
        if self.homed != Some(project) {
            self.homed = Some(project);
            self.outbox.push(Outgoing::Refresh { session: position });
        }
    }

    /// The files in the order the list shows them, group by group.
    fn listed(&self) -> Vec<usize> {
        self.review_session().map_or_else(Vec::new, |session| {
            crate::change_list::order_shown(
                &session.changes,
                &self.closed_folders,
                self.git.repository.as_deref(),
            )
        })
    }

    /// The files a command acts on, in the order the list shows them: those marked, or the row the keyboard is on.
    ///
    /// A single mark on the row the keyboard is already on is not a selection of one thing said twice, so it reads as the row itself: marking a row and then acting on it means no different than acting on it would have.
    pub fn acting_on(&self) -> Vec<usize> {
        let real = match self.git.marked.len() {
            0 => false,
            1 => self
                .git
                .selected
                .is_none_or(|selected| !self.git.marked.contains(&selected)),
            _ => true,
        };
        if !real {
            return self.git.selected.into_iter().collect();
        }
        self.review_session()
            .map_or_else(Vec::new, |session| {
                crate::change_list::order_shown(
                    &session.changes,
                    &std::collections::HashSet::new(),
                    self.git.repository.as_deref(),
                )
            })
            .into_iter()
            .filter(|row| self.git.marked.contains(row))
            .collect()
    }

    /// The changed files a command acts on, in the order the list shows them.
    pub fn marked_changes(&self) -> Vec<&Change> {
        let Some(session) = self.review_session() else {
            return Vec::new();
        };
        self.acting_on()
            .into_iter()
            .filter_map(|index| session.changes.get(index))
            .collect()
    }

    /// Puts the keyboard on the file at `index` alone, forgetting every mark.
    pub(crate) fn select_change(&mut self, index: usize) {
        self.git.marked.clear();
        self.git.gesture = None;
        self.git.selected = Some(index);
    }

    /// The state after the changed file at `index` was pressed with the modifier that marks several.
    pub fn change_picked(mut self, index: usize, how: Pick) -> Self {
        match how {
            Pick::Toggle => {
                self.git.gesture = None;
                if self.git.marked.is_empty()
                    && let Some(selected) = self.git.selected.filter(|selected| *selected != index)
                {
                    self.git.marked.push(selected);
                }
                if let Some(at) = self.git.marked.iter().position(|&held| held == index) {
                    self.git.marked.remove(at);
                } else {
                    self.git.marked.push(index);
                }
                self.git.marked.sort_unstable();
                self.git.selected = Some(index);
            }
            Pick::Range => self.mark_to(index),
        }
        self
    }

    /// Marks every file between the one the keyboard is on and `index`.
    ///
    /// The range is drawn from where the gesture was anchored rather than from wherever the keyboard has reached, so dragging one out and back again leaves behind only what it still covers.
    fn mark_to(&mut self, index: usize) {
        let (anchor, base) = match &self.git.gesture {
            Some((anchor, base)) => (*anchor, base.clone()),
            None => {
                let anchor = self.git.selected.unwrap_or(index);
                self.git.gesture = Some((anchor, self.git.marked.clone()));
                (anchor, self.git.marked.clone())
            }
        };
        let listed = self.listed();
        let (Some(from), Some(to)) = (
            listed.iter().position(|row| *row == anchor),
            listed.iter().position(|row| *row == index),
        ) else {
            return;
        };
        let (first, last) = (from.min(to), from.max(to));
        let mut marked = base;
        marked.extend(&listed[first..=last]);
        marked.sort_unstable();
        marked.dedup();
        self.git.marked = marked;
        self.git.selected = Some(index);
    }

    /// The state with every file the list shows picked.
    pub fn changes_all_picked(mut self) -> Self {
        self.git.gesture = None;
        self.git.marked = self.listed();
        self.git.marked.sort_unstable();
        self
    }

    /// The state with the menu on changed files open at `at` over the file at `index`: on the files marked when it is one of them, on it alone when it is not.
    pub fn with_change_menu(mut self, index: usize, at: (f32, f32)) -> Self {
        if index >= self.change_count() {
            return self;
        }
        if !self.acting_on().contains(&index) {
            self.select_change(index);
        }
        self.with_git_menu(MenuKind::Change, at)
    }

    /// Closes the menus that cannot be open together with the menu on changed files.
    fn close_other_menus(&mut self) {
        self.menu = None;
        self.project_menu = None;
        self.session_menu = None;
        self.message_menu = None;
        self.git.branch_menu = false;
    }

    /// Whether every file picked is staged.
    pub fn marked_all_staged(&self) -> bool {
        let marked = self.marked_changes();
        !marked.is_empty() && marked.iter().all(|change| change.fully_staged())
    }

    /// Whether the files picked can be added to `.gitignore`: all of them are new to git.
    pub fn marked_ignorable(&self) -> bool {
        let marked = self.marked_changes();
        !marked.is_empty() && marked.iter().all(|change| change.untracked)
    }

    /// Whether one file is picked and it has a version in the last commit.
    pub fn marked_has_head(&self) -> bool {
        matches!(self.marked_changes().as_slice(), [change] if !change.is_created())
    }

    /// What the button under the commit message does now: commits while there is something to commit, and once there is nothing, brings the branch level with the one it follows, or publishes it.
    pub fn primary(&self) -> Primary {
        if let Some(doing) = self.git.busy {
            return Primary::Busy(doing);
        }
        let Some(session) = self.review_session() else {
            return Primary::Commit {
                title: "Commit",
                stopped: Some(NOTHING_TO_COMMIT),
            };
        };
        let staged = session
            .changes
            .iter()
            .filter(|change| change.staged && !change.conflicted)
            .count();
        let tracked = session
            .changes
            .iter()
            .filter(|change| change.unstaged && !change.untracked && !change.conflicted)
            .count();
        let operation = self.git.operation.as_ref();
        let title = match operation {
            Some(pm_git::Operation::Merge { .. }) => "Commit Merge",
            Some(pm_git::Operation::Rebase { .. }) => "Continue Rebase",
            Some(pm_git::Operation::CherryPick { .. }) => "Continue Cherry-Pick",
            None if staged == 0 => "Commit Tracked",
            None => "Commit",
        };
        let unsaid = self.message.text().trim().is_empty();
        let stopped = if session.changes.iter().any(|change| change.conflicted) {
            Some(
                if matches!(operation, Some(pm_git::Operation::Merge { .. })) {
                    "Resolve the conflicts before committing"
                } else {
                    "Resolve the conflicts before continuing"
                },
            )
        } else if operation.is_none() && staged == 0 && tracked == 0 {
            Some(NOTHING_TO_COMMIT)
        } else if unsaid && !matches!(operation, Some(pm_git::Operation::Rebase { .. })) {
            Some("No commit message")
        } else {
            None
        };
        if stopped != Some(NOTHING_TO_COMMIT) {
            return Primary::Commit { title, stopped };
        }
        let tracking = &session.tracking;
        match (session.on_branch, &tracking.upstream) {
            (true, None) if !session.commits.is_empty() || !self.git.history.is_empty() => {
                Primary::Publish
            }
            (true, Some(_)) if tracking.ahead + tracking.behind > 0 => Primary::Sync {
                ahead: tracking.ahead,
                behind: tracking.behind,
            },
            _ => Primary::Commit { title, stopped },
        }
    }

    /// Carries out what the button under the commit message says.
    pub(crate) fn primary_pressed(&mut self) {
        match self.primary() {
            Primary::Commit { stopped: None, .. } => self.commit_pressed(false),
            Primary::Sync { .. } => self.remote("sync", "Syncing…"),
            Primary::Publish => self.remote("push", "Publishing…"),
            Primary::Commit { .. } | Primary::Busy(_) => {}
        }
    }

    /// Commits, or, while a merge, a rebase or a cherry-pick is stopped, takes its next step, finishing it under the message written; pushes afterwards when `push` is set.
    pub(crate) fn commit_pressed(&mut self, push: bool) {
        if self.git.operation.is_none() {
            self.commit_with(push, false);
            return;
        }
        let message = self.message.text().trim().to_owned();
        let message = (!message.is_empty()).then_some(message);
        self.message = crate::state::commit_field();
        self.step_with("continue", "Continuing…", message);
    }

    /// Takes the next step of what is stopped, `doing` being what the button says meanwhile.
    pub(crate) fn step(&mut self, action: &'static str, doing: &'static str) {
        self.step_with(action, doing, None);
    }

    /// Takes the next step of what is stopped, finishing it under `message` when there is one.
    fn step_with(&mut self, action: &'static str, doing: &'static str, message: Option<String>) {
        if let Some(session) = self.review_index() {
            self.git.busy = Some(doing);
            self.git.trouble = None;
            self.outbox.push(Outgoing::Step {
                session,
                action,
                message,
            });
        }
    }

    /// Asks the server to do `operation` with the remote of the open session's repository, and shows `doing` until it has.
    pub(crate) fn remote(&mut self, operation: &'static str, doing: &'static str) {
        if let Some(session) = self.review_index()
            && self.git.busy.is_none()
        {
            self.git.busy = Some(doing);
            self.git.trouble = None;
            self.outbox.push(Outgoing::Sync { session, operation });
        }
    }

    /// Asks for the commits of the graph the first time the graph is open for the open session.
    pub(crate) fn ask_log_once(&mut self) {
        if let Some(session) = self.review_index()
            && self.git.graph_open
            && self.git.logged != Some(session)
        {
            self.git.logged = Some(session);
            self.ask_log();
        }
    }

    /// Asks for the commits of the graph, of what every reference reaches or only the checked out branch.
    pub(crate) fn ask_log(&mut self) {
        if let Some(session) = self.review_index() {
            self.outbox.push(Outgoing::Log {
                session,
                all: self.git.graph_all,
            });
        }
    }

    /// Whether the commits of the open session's repository are wanted, for the graph or a project's overview, and whether they take in every reference.
    pub fn graph_wanted(&self) -> Option<bool> {
        (self.review_session().is_some()
            && (self.git.graph_open || matches!(self.page, Page::Project(_))))
        .then_some(self.git.graph_all)
    }

    /// The state after the server listed the `commits` of the repository of the session at `session`.
    pub fn with_history(mut self, session: usize, commits: Vec<pm_git::Commit>) -> Self {
        if self.review_index() == Some(session) {
            self.git.history = commits;
        }
        self
    }

    /// The state after the remote answered the request of the session at `session`, which failed for `failure` when it did.
    pub fn remote_done(mut self, _session: usize, failure: Option<String>) -> Self {
        self.git.busy = None;
        match failure {
            Some(message) => self.git_failed(&format!("Could not reach the remote: {message}")),
            None => self,
        }
    }

    /// The state after git refused something asked of the folder of the open session, for `reason`, which it says in the panel with git's own `fatal:` or `error:` prefix left off.
    pub fn git_failed(mut self, reason: &str) -> Self {
        let plain = |text: &str| {
            let text = text.trim();
            let text = text
                .strip_prefix("fatal: ")
                .or_else(|| text.strip_prefix("error: "))
                .unwrap_or(text);
            let mut letters = text.chars();
            letters
                .next()
                .map(|first| first.to_uppercase().chain(letters).collect::<String>())
                .unwrap_or_default()
        };
        let cleaned = match reason.split_once(": ") {
            Some((context, said)) => format!("{context}: {}", plain(said)),
            None => plain(reason),
        };
        self.git.trouble = Some(cleaned);
        self
    }

    /// The state after the graph is scrolled by `delta` logical pixels, downwards when positive, with `room` pixels to scroll.
    pub fn scrolled_graph(mut self, delta: f32, room: f32) -> Self {
        self.git.graph_scroll = (self.git.graph_scroll + delta).clamp(0.0, room);
        self
    }

    /// Opens the file at `index` of the changes as it is in the folder.
    pub(crate) fn open_changed_file(&mut self, index: usize) {
        if let Some(path) = self
            .review_session()
            .and_then(|session| session.changes.get(index))
            .map(|change| change.path.clone())
        {
            self.open_viewer(path, false);
        }
    }

    /// Carries out `action` if it is one of this module's, and says whether it was.
    pub(crate) fn press_git(&mut self, action: FocusId) -> bool {
        if let Some(row) = action
            .0
            .checked_sub(MENU_ROW_BASE)
            .filter(|row| *row < 40 && self.git.menu.is_some())
        {
            self.press_menu_row(row as usize);
            return true;
        }
        if self.press_prompt(action) || self.press_tree(action) || self.press_files(action) {
            return true;
        }
        match action {
            REVIEW_DISMISS => {
                self.git.menu = None;
                self.git.prompt = None;
                self.git.branch_menu = false;
            }
            BRANCH_BODY | BRANCH_FILTER => {}
            GROUP_TRACKED | GROUP_UNTRACKED => {
                let group = match action {
                    GROUP_TRACKED => Group::Tracked,
                    _ => Group::Untracked,
                };
                let members = self.review_session().map_or_else(Vec::new, |session| {
                    crate::change_list::members(
                        &session.changes,
                        group,
                        self.git.repository.as_deref(),
                    )
                });
                self.stage_changes(&members);
            }
            TROUBLE_DISMISS => self.git.trouble = None,
            CHANGES_TOGGLE => self.git.changes_open = !self.git.changes_open,
            GRAPH_TOGGLE => self.git.graph_open = !self.git.graph_open,
            GRAPH_TARGET => {
                self.git.graph_all = false;
                self.git.graph_scroll = 0.0;
                self.ask_log();
            }
            GRAPH_FETCH => self.remote("fetch", "Fetching…"),
            GRAPH_PULL => self.remote("pull", "Pulling…"),
            GRAPH_PUSH => self.remote("push", "Pushing…"),
            CHANGES_REFRESH => {
                if let Some(session) = self.review_index() {
                    self.outbox.push(Outgoing::Refresh { session });
                }
            }
            BRANCH_BUTTON => self.toggle_branch_menu(),
            BRANCH_CREATE | BRANCH_SUBMIT => self.submit_branch(action == BRANCH_CREATE),
            _ => return self.press_branch_row(action),
        }
        true
    }

    /// Opens or closes the list of branches of the repository the panel works on, asking the server for them when it opens.
    fn toggle_branch_menu(&mut self) {
        if self.git.branch_menu {
            self.git.branch_menu = false;
            return;
        }
        let Some(open) = self.review_index() else {
            return;
        };
        self.close_other_menus();
        self.git.branch_menu = true;
        self.git.filter = Field::default();
        self.git.branches = Vec::new();
        self.outbox.push(Outgoing::SessionBranches {
            session: open,
            submodule: self.git.repository.clone(),
        });
    }

    /// The repositories of the open session's folder the panel can work on: its own, then each checked out submodule, nested ones included, as the server last said.
    pub fn repositories(&self) -> Vec<Option<String>> {
        std::iter::once(None)
            .chain(
                self.git
                    .submodule_heads
                    .iter()
                    .map(|(path, _)| Some(path.clone())),
            )
            .take(REPOSITORY_ROWS)
            .collect()
    }

    /// Makes the repository at `row` of [`Self::repositories`] the one the panel works on, reading its history again, and opens its branches when `branches` is set.
    fn pick_repository(&mut self, row: usize, branches: bool) {
        let Some(picked) = self.repositories().into_iter().nth(row) else {
            return;
        };
        let Some(open) = self.review_index() else {
            return;
        };
        if self.git.repository != picked {
            self.git.repository = picked;
            self.git.marked = Vec::new();
            self.git.selected = None;
            self.git.history = Vec::new();
            self.outbox.push(Outgoing::Refresh { session: open });
        }
        self.git.branch_menu = false;
        if branches {
            self.toggle_branch_menu();
        }
    }

    /// The submodule the panel works on in the folder of the session at `session`, or nothing for the folder's own repository or a session the panel does not show.
    pub fn repository_of(&self, session: usize) -> Option<String> {
        (self.review_index() == Some(session))
            .then(|| self.git.repository.clone())
            .flatten()
    }

    /// What the repository at `repository` of the open session's folder has checked out: the session's branch for its own, or the submodule's as the server last said.
    pub fn head_of(&self, repository: Option<&str>) -> Option<String> {
        match repository {
            Some(path) => self
                .git
                .submodule_heads
                .iter()
                .find(|(held, _)| held == path)
                .map(|(_, head)| head.clone()),
            None => self.review_session().map(|session| session.branch.clone()),
        }
    }

    /// The state after the server said what the submodules of the folder of the session at `session` have checked out; a submodule picked that is no longer there gives the panel back to the folder's own repository.
    pub fn with_submodule_heads(mut self, session: usize, heads: Vec<(String, String)>) -> Self {
        if self.review_index() == Some(session) {
            if let Some(picked) = &self.git.repository
                && !heads.iter().any(|(path, _)| path == picked)
            {
                self.git.repository = None;
            }
            self.git.submodule_heads = heads;
        }
        self
    }

    /// Checks out the branch typed, or makes it when `create` is set or none is called that.
    fn submit_branch(&mut self, create: bool) {
        let Some(open) = self.review_index() else {
            return;
        };
        let typed = self.git.filter.text().trim().to_owned();
        if !create {
            let matching = self.git.matching();
            let picked = matching
                .iter()
                .find(|name| ***name == typed)
                .or_else(|| (matching.len() == 1).then(|| &matching[0]))
                .map(|name| (*name).clone());
            if let Some(branch) = picked {
                self.git.branch_menu = false;
                self.git.trouble = None;
                self.git.busy = Some("Switching branch…");
                self.outbox.push(Outgoing::Checkout {
                    session: open,
                    branch,
                    submodule: self.git.repository.clone(),
                });
                return;
            }
        }
        if !typed.is_empty() && !self.git.exists() {
            self.git.branch_menu = false;
            self.git.trouble = None;
            self.git.busy = Some("Making branch…");
            self.outbox.push(Outgoing::CreateBranch {
                session: open,
                name: typed,
                submodule: self.git.repository.clone(),
            });
        }
    }

    /// Checks out the branch of the row `action` if it is one, or picks the repository of a row of the list of repositories, opening its branches when `action` is the button on it, and says whether it was any.
    fn press_branch_row(&mut self, action: FocusId) -> bool {
        for (base, branches) in [(REPOSITORY_BASE, false), (REPOSITORY_BRANCH_BASE, true)] {
            if let Some(row) = action
                .0
                .checked_sub(base)
                .map(|row| row as usize)
                .filter(|row| *row < REPOSITORY_ROWS)
            {
                self.pick_repository(row, branches);
                return true;
            }
        }
        let Some(row) = action
            .0
            .checked_sub(BRANCH_PICK_BASE)
            .map(|row| row as usize)
            .filter(|row| *row < BRANCH_ROWS)
        else {
            return false;
        };
        let Some(open) = self.review_index() else {
            return true;
        };
        let name = self.git.matching().get(row).map(|name| (*name).clone());
        if let Some(branch) = name {
            self.git.branch_menu = false;
            self.git.trouble = None;
            self.git.busy = Some("Switching branch…");
            self.outbox.push(Outgoing::Checkout {
                session: open,
                branch,
                submodule: self.git.repository.clone(),
            });
        }
        true
    }

    /// The state after the server listed the branches of the folder of the session at `session`.
    pub fn with_session_branches(mut self, session: usize, names: Vec<String>) -> Self {
        if self.review_index() == Some(session) {
            self.git.branches = names;
            self.choices_renewed(crate::quick_choices::Choosing::Branch);
        }
        self
    }

    /// Keeps the files marked, and the one the keyboard is on, by path, after the changes were read again.
    pub(crate) fn remarked(&mut self, kept: &(Vec<String>, Option<String>)) {
        let (marked, selected) = match self.review_session() {
            Some(session) => (
                session
                    .changes
                    .iter()
                    .enumerate()
                    .filter(|(_, change)| kept.0.contains(&change.path))
                    .map(|(index, _)| index)
                    .collect(),
                kept.1.as_ref().and_then(|path| {
                    session
                        .changes
                        .iter()
                        .position(|change| &change.path == path)
                }),
            ),
            None => (Vec::new(), None),
        };
        self.git.marked = marked;
        self.git.selected = selected;
        self.git.gesture = None;
    }

    /// The paths of the files marked and of the one the keyboard is on, for [`WorkspaceState::remarked`].
    pub(crate) fn marked_by_path(&self) -> (Vec<String>, Option<String>) {
        let Some(session) = self.review_session() else {
            return (Vec::new(), None);
        };
        let path = |index: &usize| {
            session
                .changes
                .get(*index)
                .map(|change| change.path.clone())
        };
        (
            self.git.marked.iter().filter_map(path).collect(),
            self.git.selected.as_ref().and_then(path),
        )
    }
}
