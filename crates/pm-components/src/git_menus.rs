//! The menus of the source control panel: on changed files, under the commit button, over the panel's own actions, on the graph's references and on a commit.

use pm_input::FocusId;
use pm_ui::Node;

use crate::Menu;
use crate::git_controls::{BRANCH_BUTTON, CHANGES_MORE, COMMIT_CHEVRON, GRAPH_REFS};
use crate::git_prompts::{Prompt, StashAction};
use crate::state::{ADD_PROJECT, Outgoing, WorkspaceState};
use crate::tree_actions::TreeTarget;

/// The first of the rows of the menu that is open; the rest follow it.
pub const MENU_ROW_BASE: u64 = 9000;

/// How tall the menu on changed files is, in logical pixels, to keep it inside the window.
pub const CHANGE_MENU_HEIGHT: f32 = 11.0 * 32.0 + 4.0 * 9.0 + 10.0;

/// Which menu is open.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MenuKind {
    /// What can be done to the files marked.
    Change,
    /// What else the button under the commit message can do.
    Commit,
    /// What can be done from the panel, over everything it lists.
    Source,
    /// Which references the graph lists.
    Refs,
    /// What can be done to a commit of the graph, by its place in the list.
    History(usize),
    /// What can be done to the file or folder of the project's files the menu was opened on, or to the tree itself.
    Tree,
}

/// A menu that is open, and where.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OpenMenu {
    /// Which menu it is.
    pub kind: MenuKind,
    /// Where its corner is, in logical pixels.
    pub at: (f32, f32),
}

/// What a row of a menu does.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Command {
    /// Shows the diff of the file.
    OpenChanges,
    /// Opens the resolver on the file that is in conflict.
    Resolve,
    /// Shows the session diff.
    ReviewChanges,
    /// Opens every file the session's own commits changed as one view.
    ReviewBranch,
    /// Opens every file the commit changed as one view.
    ShowCommit,
    /// Reads the file as it is in the folder.
    OpenFile,
    /// Reads the file as it is in the last commit.
    OpenHead,
    /// Stages the files, or unstages them when all are staged.
    StageActing,
    /// Asks whether to discard the files.
    Discard,
    /// Stages every change.
    StageAll,
    /// Unstages every change.
    UnstageAll,
    /// Copies the path of the file on its machine.
    CopyPath,
    /// Copies the path of the file from the root of the repository.
    CopyRelativePath,
    /// Adds the files to `.gitignore`.
    Ignore,
    /// Commits, or finishes the merge, rebase or cherry-pick that stopped.
    Commit,
    /// Commits and pushes.
    CommitAndPush,
    /// Asks whether to rewrite the last commit.
    Amend,
    /// Asks whether to give up what stopped.
    Abort,
    /// Asks whether to leave out the commit it stopped on.
    Skip,
    /// Pulls.
    Pull,
    /// Pulls, replaying the branch's own commits on top.
    PullRebase,
    /// Pushes.
    Push,
    /// Pushes, overwriting only what was seen.
    ForcePush,
    /// Fetches from every remote.
    Fetch,
    /// Opens the panel that adds a project, which clones a repository.
    Clone,
    /// Opens the list of branches.
    Checkout,
    /// Reads the changes and the graph again.
    Refresh,
    /// Asks what to call a stash of the changes.
    StashPush,
    /// Asks which stash to take.
    Stashes(StashAction),
    /// Lists what the checked out branch reaches.
    Auto,
    /// Lists what every reference reaches.
    All,
    /// Applies the commit to the checked out branch.
    CherryPick,
    /// Copies the commit's name.
    CopyHash,
    /// Asks what to call a new file in the folder.
    TreeNewFile,
    /// Asks what to call a new folder in the folder.
    TreeNewFolder,
    /// Reads the file in the window.
    TreeOpen,
    /// Holds the entry to be moved by a paste.
    TreeCut,
    /// Holds the entry to be copied by a paste.
    TreeCopy,
    /// Moves or copies what is held into the folder.
    TreePaste,
    /// Copies the entry beside itself.
    TreeDuplicate,
    /// Copies the path of the entry on its machine.
    TreeCopyPath,
    /// Copies the path of the entry from the root of the project.
    TreeCopyRelativePath,
    /// Shows the entry in the file manager of this machine.
    TreeReveal,
    /// Asks what to rename the entry to.
    TreeRename,
    /// Asks whether to delete the entry.
    TreeDelete,
    /// Folds every folder of the tree.
    TreeCollapse,
    /// Reads the files and changes again.
    TreeRefresh,
}

/// One entry of a menu.
enum Entry {
    /// A row, which does nothing while it has no command.
    Row(String, Option<Command>),
    /// A rule between two groups.
    Rule,
}

/// A row saying `label` that does `command`.
fn row(label: impl Into<String>, command: Command) -> Entry {
    Entry::Row(label.into(), Some(command))
}

/// A row saying `label` that does `command` while `enabled`, and nothing otherwise.
fn row_if(label: impl Into<String>, command: Command, enabled: bool) -> Entry {
    Entry::Row(label.into(), enabled.then_some(command))
}

impl WorkspaceState {
    /// The menu button `id` is, if it is one: a press on it opens that menu.
    pub fn menu_button_of(id: FocusId) -> Option<MenuKind> {
        match id {
            COMMIT_CHEVRON => Some(MenuKind::Commit),
            CHANGES_MORE => Some(MenuKind::Source),
            GRAPH_REFS => Some(MenuKind::Refs),
            _ => None,
        }
    }

    /// Whether `id` names a row of the menu that is open.
    pub fn is_menu_row(&self, id: FocusId) -> bool {
        (self.git.menu.is_some() && id.0.checked_sub(MENU_ROW_BASE).is_some_and(|row| row < 40))
            || self.settings_menu_row(id)
            || self.is_tab_menu_row(id)
            || self.is_terminal_menu_row(id)
            || self.is_browser_menu_row(id)
    }

    /// Which commit of the graph the row `id` is, if it is one.
    pub fn history_row_of(&self, id: FocusId) -> Option<usize> {
        id.0.checked_sub(crate::graph::HISTORY_ROW_BASE)
            .map(|row| row as usize)
            .filter(|row| *row < self.git.history.len())
    }

    /// The state with the menu `kind` open at `at`, and the others closed.
    pub fn with_git_menu(mut self, kind: MenuKind, at: (f32, f32)) -> Self {
        if self.review_session().is_none() {
            return self;
        }
        self.menu = None;
        self.project_menu = None;
        self.session_menu = None;
        self.message_menu = None;
        self.git.branch_menu = false;
        if kind == MenuKind::Source {
            self.ask_stashes();
        }
        self.git.menu = Some(OpenMenu { kind, at });
        self
    }

    /// Asks for the stashes, which the menu over the panel's actions offers to take.
    pub(crate) fn ask_stashes(&mut self) {
        if let Some(session) = self.review_index() {
            self.outbox.push(Outgoing::Stashes { session });
        }
    }

    /// The state after the server listed the stashes of the repository of the session at `session`.
    pub fn with_stashes(mut self, session: usize, stashes: Vec<pm_git::Stash>) -> Self {
        if self.review_index() == Some(session) {
            self.git.stashes = stashes;
        }
        self
    }

    /// The commit and operation rows the menu under the commit button and the menu over the panel's actions share.
    fn commit_entries(&self) -> Vec<Entry> {
        let operation = self.git.operation.as_ref();
        let title = match operation {
            Some(pm_git::Operation::Merge { .. }) => "Commit Merge",
            Some(pm_git::Operation::Rebase { .. }) => "Continue Rebase",
            Some(pm_git::Operation::CherryPick { .. }) => "Continue Cherry-Pick",
            None => "Commit",
        };
        let mut entries = vec![
            row(title, Command::Commit),
            row_if(
                "Commit and Push",
                Command::CommitAndPush,
                operation.is_none(),
            ),
        ];
        match operation {
            Some(waiting) => {
                entries.push(row(format!("Abort {}", waiting.name()), Command::Abort));
                if !matches!(waiting, pm_git::Operation::Merge { .. }) {
                    entries.push(row("Skip Commit", Command::Skip));
                }
            }
            None => entries.push(row("Amend Last Commit", Command::Amend)),
        }
        entries
    }

    /// The entries of the menu `kind`.
    fn menu_entries(&self, kind: MenuKind) -> Vec<Entry> {
        match kind {
            MenuKind::Change => {
                let acting = self.marked_changes();
                let count = acting.len();
                let created = !acting.is_empty() && acting.iter().all(|change| change.is_created());
                let staged = self.marked_all_staged();
                let stage = match (count, staged) {
                    (1, true) => "Unstage File".to_owned(),
                    (1, false) => "Stage File".to_owned(),
                    (count, true) => format!("Unstage {count} Files"),
                    (count, false) => format!("Stage {count} Files"),
                };
                let discard = match (count, created) {
                    (1, true) => "Delete File".to_owned(),
                    (1, false) => "Discard Changes".to_owned(),
                    (count, true) => format!("Delete {count} Files"),
                    (count, false) => format!("Discard Changes to {count} Files"),
                };
                vec![
                    row_if("Open Changes", Command::OpenChanges, count == 1),
                    row_if(
                        "Resolve Conflict",
                        Command::Resolve,
                        count == 1 && acting.iter().all(|change| change.conflicted),
                    ),
                    row_if("Open File", Command::OpenFile, count == 1),
                    row_if(
                        "Open File (HEAD)",
                        Command::OpenHead,
                        self.marked_has_head(),
                    ),
                    Entry::Rule,
                    row(stage, Command::StageActing),
                    row(discard, Command::Discard),
                    Entry::Rule,
                    row("Stage All Changes", Command::StageAll),
                    row("Unstage All Changes", Command::UnstageAll),
                    Entry::Rule,
                    row_if("Copy Path", Command::CopyPath, count == 1),
                    row_if("Copy Relative Path", Command::CopyRelativePath, count == 1),
                    Entry::Rule,
                    row_if(
                        "Add to .gitignore",
                        Command::Ignore,
                        self.marked_ignorable(),
                    ),
                ]
            }
            MenuKind::Commit => self.commit_entries(),
            MenuKind::Source => {
                let stashed = !self.git.stashes.is_empty();
                let mut entries = vec![
                    row("Pull", Command::Pull),
                    row("Push", Command::Push),
                    row("Clone Repository…", Command::Clone),
                    row("Checkout…", Command::Checkout),
                    row("Fetch", Command::Fetch),
                    Entry::Rule,
                    row("Stage All Changes", Command::StageAll),
                    row("Unstage All Changes", Command::UnstageAll),
                    row("Pull (Rebase)", Command::PullRebase),
                    row("Force Push", Command::ForcePush),
                    Entry::Rule,
                    row("Refresh", Command::Refresh),
                    row_if(
                        "Review All Changes",
                        Command::ReviewChanges,
                        self.change_count() > 0,
                    ),
                    row_if(
                        "Review Branch Changes",
                        Command::ReviewBranch,
                        self.review_session()
                            .is_some_and(|session| session.base.is_some()),
                    ),
                ];
                entries.extend(self.commit_entries());
                entries.extend([
                    Entry::Rule,
                    row("Stash…", Command::StashPush),
                    row_if(
                        "Apply Stash…",
                        Command::Stashes(StashAction::Apply),
                        stashed,
                    ),
                    row_if("Pop Stash…", Command::Stashes(StashAction::Pop), stashed),
                    row_if("Drop Stash…", Command::Stashes(StashAction::Drop), stashed),
                ]);
                entries
            }
            MenuKind::Tree => self.tree_entries(),
            MenuKind::Refs => vec![row("Auto", Command::Auto), row("All", Command::All)],
            MenuKind::History(commit) => {
                let branch = self
                    .review_session()
                    .map(|session| session.branch.clone())
                    .unwrap_or_default();
                let known = self.git.history.get(commit).is_some();
                vec![
                    row_if("Show Changes", Command::ShowCommit, known),
                    row_if(
                        format!("Cherry-Pick into {branch}"),
                        Command::CherryPick,
                        known
                            && self.git.operation.is_none()
                            && self.git.graph_all
                            && !self.git.history[commit]
                                .refs
                                .iter()
                                .any(|name| name.starts_with("HEAD -> ")),
                    ),
                    row_if("Copy Commit Hash", Command::CopyHash, known),
                ]
            }
        }
    }

    /// The entries of the menu on an entry of the tree, or on the tree itself.
    fn tree_entries(&self) -> Vec<Entry> {
        let target = self
            .git
            .tree_target
            .clone()
            .unwrap_or_else(TreeTarget::root);
        let pasting = self.git.tree_clipboard.is_some();
        let local = self.review_is_local();
        let targets = self.tree_targets();
        let one = targets.len() == 1;
        if target.is_root() {
            return vec![
                row("New File…", Command::TreeNewFile),
                row("New Folder…", Command::TreeNewFolder),
                Entry::Rule,
                row_if("Paste", Command::TreePaste, pasting),
                Entry::Rule,
                row("Copy Path", Command::TreeCopyPath),
                row("Copy Relative Path", Command::TreeCopyRelativePath),
                row_if("Reveal in File Manager", Command::TreeReveal, local),
                Entry::Rule,
                row("Collapse All", Command::TreeCollapse),
                row("Refresh", Command::TreeRefresh),
            ];
        }
        vec![
            row("New File…", Command::TreeNewFile),
            row("New Folder…", Command::TreeNewFolder),
            Entry::Rule,
            row_if(
                "Open",
                Command::TreeOpen,
                targets.iter().any(|target| !target.folder),
            ),
            Entry::Rule,
            row("Cut", Command::TreeCut),
            row("Copy", Command::TreeCopy),
            row_if("Paste", Command::TreePaste, pasting),
            row("Duplicate", Command::TreeDuplicate),
            Entry::Rule,
            row("Copy Path", Command::TreeCopyPath),
            row("Copy Relative Path", Command::TreeCopyRelativePath),
            row_if("Reveal in File Manager", Command::TreeReveal, local && one),
            Entry::Rule,
            row_if("Rename…", Command::TreeRename, one),
            row("Delete", Command::TreeDelete),
        ]
    }

    /// The node that draws the menu that is open.
    pub fn git_menu_node(&self) -> Option<Node> {
        let open = self.git.menu?;
        Some(self.git_menu(open.kind, open.at).build())
    }

    /// The point at which the menu `kind`, opened from the control occupying `trigger` as `(x, y, width, height)`, lies below it or above it inside a window `window` wide and tall.
    pub fn git_menu_below(
        &self,
        kind: MenuKind,
        trigger: (f32, f32, f32, f32),
        window: (f32, f32),
    ) -> (f32, f32) {
        self.git_menu(kind, (0.0, 0.0)).below(trigger, window).at()
    }

    /// The menu `kind` with its rows, its corner at `at`.
    fn git_menu(&self, kind: MenuKind, at: (f32, f32)) -> Menu {
        let mut menu = Menu::new(at.0, at.1);
        let mut position = 0;
        for entry in self.menu_entries(kind) {
            match entry {
                Entry::Rule => menu = menu.separator(),
                Entry::Row(label, command) => {
                    menu = menu.item(
                        &label,
                        None,
                        FocusId(MENU_ROW_BASE + position),
                        command.is_none(),
                    );
                    position += 1;
                }
            }
        }
        menu
    }

    /// Carries out what the `position`-th row of the open menu does, and closes it.
    pub(crate) fn press_menu_row(&mut self, position: usize) {
        let Some(open) = self.git.menu.take() else {
            return;
        };
        let command = self
            .menu_entries(open.kind)
            .into_iter()
            .filter_map(|entry| match entry {
                Entry::Row(_, command) => Some(command),
                Entry::Rule => None,
            })
            .nth(position)
            .flatten();
        if let Some(command) = command {
            self.run(command, open.kind);
        }
    }

    /// The state after the source control `action` of the keymap, carried out as the panel's own menu does it; any other action leaves it as it is.
    pub fn source_control(mut self, action: pm_editor::Action) -> Self {
        use pm_editor::Action;
        let command = match action {
            Action::Commit => Command::Commit,
            Action::CommitAndPush => Command::CommitAndPush,
            Action::Pull => Command::Pull,
            Action::Push => Command::Push,
            Action::Fetch => Command::Fetch,
            Action::StashChanges => Command::StashPush,
            Action::CheckoutBranch => Command::Checkout,
            Action::CloneRepository => Command::Clone,
            Action::RefreshSourceControl => Command::Refresh,
            Action::PullRebase => Command::PullRebase,
            Action::ForcePush => Command::ForcePush,
            Action::StageAll => Command::StageAll,
            Action::UnstageAll => Command::UnstageAll,
            Action::CommitAmend => Command::Amend,
            Action::ContinueOperation => Command::Commit,
            Action::SkipCommit => Command::Skip,
            Action::AbortOperation => Command::Abort,
            Action::StashApply => Command::Stashes(StashAction::Apply),
            Action::StashPop => Command::Stashes(StashAction::Pop),
            Action::StashDrop => Command::Stashes(StashAction::Drop),
            Action::Sync => {
                self.remote("sync", "Syncing…");
                return self;
            }
            Action::ToggleGraph => {
                self.git.graph_open = !self.git.graph_open;
                return self;
            }
            _ => return self,
        };
        self.run(command, MenuKind::Source);
        self
    }

    /// Carries out `command` from the menu `kind`.
    fn run(&mut self, command: Command, kind: MenuKind) {
        let acting = self.acting_on();
        match command {
            Command::OpenChanges => {
                if let [index] = acting.as_slice() {
                    self.open_change(*index);
                }
            }
            Command::Resolve => {
                if let [index] = acting.as_slice() {
                    self.request_resolver(*index);
                }
            }
            Command::ReviewChanges => {
                self.tab = crate::state::Tab::Diff;
            }
            Command::ReviewBranch => {
                if let (Some(session), Some(base)) = (
                    self.review_index(),
                    self.review_session()
                        .and_then(|session| session.base.clone()),
                ) {
                    self.outbox.push(Outgoing::RevisionChanges {
                        session,
                        label: format!("Changes since {base}"),
                        from: Some(base),
                        to: "HEAD".to_owned(),
                    });
                }
            }
            Command::ShowCommit => {
                if let (MenuKind::History(row), Some(session)) = (kind, self.review_index())
                    && let Some(commit) = self.git.history.get(row)
                {
                    self.outbox.push(Outgoing::RevisionChanges {
                        session,
                        label: format!("Commit {}", commit.id),
                        from: None,
                        to: commit.id.clone(),
                    });
                }
            }
            Command::OpenFile | Command::OpenHead => {
                if let [change] = self.marked_changes().as_slice() {
                    let path = change.path.clone();
                    self.open_viewer(path, command == Command::OpenHead);
                }
            }
            Command::StageActing => self.stage_changes(&acting),
            Command::Discard => {
                if !acting.is_empty() {
                    self.ask(Prompt::Discard(acting));
                }
            }
            Command::StageAll | Command::UnstageAll => {
                let all: Vec<usize> = (0..self.change_count()).collect();
                self.set_staged(&all, command == Command::StageAll);
            }
            Command::CopyPath | Command::CopyRelativePath => {
                let copied = self.review_session().and_then(|session| {
                    let [index] = acting.as_slice() else {
                        return None;
                    };
                    let change = session.changes.get(*index)?;
                    Some(if command == Command::CopyRelativePath {
                        change.path.clone()
                    } else {
                        format!(
                            "{}/{}",
                            session.directory.trim_end_matches('/'),
                            change.path
                        )
                    })
                });
                if let Some(text) = copied {
                    self.outbox.push(Outgoing::Copy { text });
                }
            }
            Command::Ignore => {
                self.git.busy = Some("Updating .gitignore…");
                self.git.trouble = None;
                let paths = self.paths_of(&acting);
                if let Some(session) = self.review_index() {
                    self.outbox.push(Outgoing::Ignore { session, paths });
                }
            }
            Command::Commit => self.commit_pressed(false),
            Command::CommitAndPush => self.commit_pressed(true),
            Command::Amend => self.ask(Prompt::Amend),
            Command::Abort => self.ask(Prompt::Abort),
            Command::Skip => self.ask(Prompt::Skip),
            Command::Pull => self.remote("pull", "Pulling…"),
            Command::PullRebase => self.remote("pull_rebase", "Pulling…"),
            Command::Push => self.remote("push", "Pushing…"),
            Command::ForcePush => self.remote("force_push", "Force pushing…"),
            Command::Fetch => self.remote("fetch", "Fetching…"),
            Command::Clone => {
                *self = std::mem::replace(self, WorkspaceState::new(&[])).after(ADD_PROJECT);
            }
            Command::Checkout => {
                *self = std::mem::replace(self, WorkspaceState::new(&[])).after(BRANCH_BUTTON);
            }
            Command::Refresh => {
                if let Some(session) = self.review_index() {
                    self.outbox.push(Outgoing::Refresh { session });
                }
            }
            Command::StashPush => self.ask(Prompt::StashMessage),
            Command::Stashes(action) => {
                self.ask_stashes();
                self.ask(Prompt::Stashes(action));
            }
            Command::Auto | Command::All => {
                self.git.graph_all = command == Command::All;
                self.git.graph_scroll = 0.0;
                self.ask_log();
            }
            Command::CherryPick => {
                if let (MenuKind::History(row), Some(session)) = (kind, self.review_index())
                    && let Some(commit) = self.git.history.get(row)
                {
                    let commit = commit.id.clone();
                    self.git.busy = Some("Cherry-picking…");
                    self.git.trouble = None;
                    self.outbox.push(Outgoing::CherryPick { session, commit });
                }
            }
            Command::TreeNewFile => self.tree_new(false),
            Command::TreeNewFolder => self.tree_new(true),
            Command::TreeOpen => self.tree_open(),
            Command::TreeCut => self.tree_hold(true),
            Command::TreeCopy => self.tree_hold(false),
            Command::TreePaste => self.tree_paste(),
            Command::TreeDuplicate => self.tree_duplicate(),
            Command::TreeCopyPath => self.tree_copy_path(false),
            Command::TreeCopyRelativePath => self.tree_copy_path(true),
            Command::TreeReveal => self.tree_reveal(),
            Command::TreeRename => self.tree_rename(),
            Command::TreeDelete => self.tree_delete(),
            Command::TreeCollapse => {
                self.press_tree(crate::tree_actions::TREE_COLLAPSE);
            }
            Command::TreeRefresh => {
                self.press_tree(crate::tree_actions::TREE_REFRESH);
            }
            Command::CopyHash => {
                if let MenuKind::History(row) = kind
                    && let Some(commit) = self.git.history.get(row)
                {
                    let text = commit.id.clone();
                    self.outbox.push(Outgoing::Copy { text });
                }
            }
        }
    }
}
