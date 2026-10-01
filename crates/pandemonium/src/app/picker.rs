//! Filling the picker, and acting on what was picked.
//!
//! What every list offers is gathered here, because it is the window that
//! knows it: which projects are open, what is in their worktrees, which
//! commands apply where the keyboard is. [`crate::picker`] is the list
//! itself, which knows none of that and only narrows what it was given.

use std::path::{Path, PathBuf};

use pm_core::ProjectId;
use pm_text::Position;

use crate::app::places::Place;
use crate::app::{App, RemoteOperation, Wake};
use crate::config::WorktreePaths;
use crate::keymap::Action;
use crate::panes::Item;
use crate::picker::{Choice, Kind, Picker, Row};

/// How long the spinner holds each frame while a remote is waited on.
const SPIN_FRAME: std::time::Duration = std::time::Duration::from_millis(33);

/// How often a running agent's activity mark advances.
const AGENT_FRAME: std::time::Duration = std::time::Duration::from_millis(250);

impl App {
    /// Opens the picker of `kind`, gathering what it offers.
    ///
    /// What takes time to gather, a worktree's files, a search of them or
    /// what git says, is started here and arrives after the picker is open.
    pub(super) fn open_picker(&mut self, kind: Kind) {
        self.leave_listings();
        self.agent_picker_at = None;
        if !matches!(kind, Kind::Branches | Kind::NewBranch) {
            self.branch_picker_at = None;
        }
        let seeded = match kind {
            Kind::Search => self
                .with_buffer(pm_text::Buffer::selected_text)
                .unwrap_or_default(),
            kind => kind.seed(),
        };
        let rows = self.rows_for(kind);
        self.open_picker_with(kind, rows, seeded.clone());
        match kind {
            Kind::WorkspaceSymbols => self.ask_typed_symbols(),
            Kind::Search => self.search_later(&seeded),
            Kind::Branches => self.ask_branches(),
            Kind::FetchRemotes => self.ask_remotes(true),
            Kind::PushRemotes => self.ask_remotes(false),
            _ => {}
        }
    }

    /// Opens the picker of `kind` over `rows`, with `seeded` already typed.
    pub(super) fn open_picker_with(&mut self, kind: Kind, rows: Vec<Row>, seeded: String) {
        self.begin_opening();
        self.picker = Some(Picker::new(kind, rows, &seeded));
        self.completions = None;
        self.hint = None;
    }

    /// Opens the prompt `action` asks a line of text for.
    pub(super) fn open_prompt(&mut self, action: Action) {
        let kind = match action {
            Action::Rename => Kind::Rename,
            _ => Kind::Line,
        };
        let seeded = match kind {
            Kind::Rename => self
                .with_buffer(|buffer| {
                    let word = buffer.word_at(buffer.selection().head);
                    buffer.text_in(word)
                })
                .unwrap_or_default(),
            _ => String::new(),
        };
        self.open_picker_with(kind, Vec::new(), seeded);
    }

    /// Opens the prompt for what to call the shell `id` names, showing what
    /// the list calls it now.
    pub(super) fn open_terminal_rename(&mut self, id: crate::terminal::ShellId) {
        let Some(scope) = self.scope() else {
            return;
        };
        let seeded = self
            .terminals
            .list(scope)
            .into_iter()
            .find(|entry| entry.id == id)
            .map(|entry| entry.name)
            .unwrap_or_default();
        self.open_picker_with(Kind::RenameTerminal(id), Vec::new(), seeded);
    }

    /// Calls the shell `id` names `typed`, or by its program when `typed` is
    /// blank.
    fn rename_terminal(&mut self, id: crate::terminal::ShellId, typed: &str) {
        if let Some(scope) = self.scope() {
            self.terminals.rename(scope, id, typed);
        }
    }

    /// Puts the picker away, saying whether one was open.
    pub(super) fn dismiss_picker(&mut self) -> bool {
        self.branch_picker_at = None;
        self.agent_picker_at = None;
        self.leave_listings();
        self.picker.take().is_some()
    }

    /// Narrows the picker to what has been typed, re-gathering when it must.
    pub(super) fn refilter_picker(&mut self) {
        let Some(picker) = self.picker.as_ref() else {
            return;
        };
        if let Some(kind) = picker.switched() {
            let rows = self.rows_for(kind);
            if let Some(picker) = self.picker.as_mut() {
                picker.switch(kind, rows);
            }
            if kind == Kind::WorkspaceSymbols {
                self.ask_typed_symbols();
            }
            return;
        }
        if picker.kind() == Kind::WorkspaceSymbols {
            let query = Kind::WorkspaceSymbols
                .query(picker.field().value())
                .to_owned();
            return self.ask_workspace_symbols(query);
        }
        if !picker.kind().is_queried() {
            return;
        }
        let query = picker.field().value().to_owned();
        self.search_later(&query);
    }

    /// Takes what the picker has selected, and puts the picker away.
    pub(super) fn confirm_picker(&mut self) {
        let Some(picker) = self.picker.as_ref() else {
            return;
        };
        let kind = picker.kind();
        let typed = picker.field().value().to_owned();
        let chosen = picker.chosen().cloned();
        let place = picker.selected();

        self.picker = None;
        self.leave_listings();
        match (kind, chosen) {
            (Kind::Branches, _) if !typed.trim().is_empty() => self.create_branch(typed.trim()),
            (Kind::Line, _) => self.go_to_typed_line(&typed),
            (Kind::Rename, _) => self.rename_to(typed),
            (Kind::BreakpointCondition | Kind::BreakpointHits | Kind::BreakpointLog, _) => {
                self.set_breakpoint_field(kind, typed)
            }
            (Kind::RenameTerminal(id), _) => self.rename_terminal(id, &typed),
            (Kind::Watch, _) => self.save_watch(typed),
            (Kind::AnswerText, _) => self.type_answer(&typed),
            (Kind::AnswerOptions, Some(Choice::AnswerOption(option))) => self.choose_answer(option),
            (Kind::NewBranch, _) => self.create_branch(&typed),
            (Kind::StashMessage, _) => self.change_by(|review| review.stash_push(typed)),
            (Kind::NewSession, _) => self.start_session(&typed),
            (Kind::SessionRepositories, Some(Choice::SessionRepository(root))) => {
                self.toggle_session_repository(root, typed, place);
            }
            (Kind::CloneUrl, _) => self.clone_project(&typed),
            (Kind::LinkedPath, _) => self.add_worktree_path(WorktreePaths::Linked, &typed),
            (Kind::CopiedPath, _) => self.add_worktree_path(WorktreePaths::Copied, &typed),
            (Kind::PortVariable, _) => self.set_worktree_port(&typed),
            (Kind::ThemeColor(token), _) => self.set_theme_color(token, &typed),
            (Kind::ThemeName, _) => self.save_theme(&typed),
            (Kind::KeymapName, _) => self.save_keymap(&typed),
            (_, Some(choice)) => self.take(choice),
            (_, None) => {}
        }
    }

    /// Chooses the `place`-th row shown, and puts the picker away.
    pub(super) fn choose_picker(&mut self, place: usize) {
        if let Some(picker) = self.picker.as_mut() {
            picker.select(place);
        }
        self.confirm_picker();
    }

    /// Carries out what one row of the picker stood for.
    fn take(&mut self, choice: Choice) {
        match choice {
            Choice::Act(action) => self.act(action),
            Choice::InstallLanguageServer(command) => self.start_server_install(command, true),
            Choice::Open(scope, path) => {
                self.jump_to(&Place {
                    scope,
                    path,
                    position: Position::default(),
                });
            }
            Choice::OpenAt(scope, path, position) => {
                self.jump_to(&Place {
                    scope,
                    path,
                    position,
                });
            }
            Choice::Project(project) => {
                self.open.activate(project);
                self.store();
            }
            Choice::Agent(agent) => self.start_agent(agent),
            Choice::Session(session, _) => self.select_session(session),
            Choice::Task(scope, task) => {
                self.run_task(scope, &task, crate::tasks::Shown::Front);
            }
            Choice::AgentHistory(session, saved) => self.open_agent_history(session, &saved),
            Choice::Mode(session, mode) => self.set_agent_mode(session, &mode),
            Choice::Knob(session, knob, value) => self.set_knob(session, &knob, &value),
            Choice::Font(slot, family) => self.set_font(slot, family),
            Choice::Debug(scope, scenario) => self.start_debugging(scope, *scenario),
            Choice::Process(pid) => self.choose_attach_process(pid),
            Choice::SessionRepository(_) => {}
            Choice::StartSession => self.cut_session(),
            Choice::AnswerOption(option) => self.choose_answer(option),
            Choice::Branch(project, branch) => self.switch_branch(project, &branch),
            Choice::Stash(index) => {
                if let Some(action) = self.stash_action.take() {
                    if action == crate::review::StashAction::Drop {
                        self.apply(crate::message::Message::DropStash(index));
                    } else {
                        self.change_by(|review| review.stash_action(index, action));
                    }
                }
            }
            Choice::FetchRemote(project, remote) => {
                self.run_in(
                    self.git_scope(project),
                    RemoteOperation::Fetch,
                    move |root| pm_core::fetch_from(root, &remote),
                );
            }
            Choice::PushRemote(project, remote) => {
                self.run_in(
                    self.git_scope(project),
                    RemoteOperation::Push,
                    move |root| pm_core::push_to(root, &remote),
                );
            }
        }
    }

    /// Goes to the line, and the column, a go-to-line prompt was given.
    fn go_to_typed_line(&mut self, typed: &str) {
        let mut parts = typed.split(&[':', ','][..]).map(str::trim);
        let Some(line) = parts.next().and_then(|line| line.parse::<usize>().ok()) else {
            return;
        };
        let column = parts
            .next()
            .and_then(|column| column.parse::<usize>().ok())
            .unwrap_or(1);
        let at = Position::new(line.saturating_sub(1), column.saturating_sub(1));
        if let Some(from) = self.here() {
            self.trail.jumped(from);
        }
        self.place_cursor(at, false);
    }

    /// Asks the server to rename the symbol under the cursor to `name`.
    fn rename_to(&mut self, name: String) {
        if name.is_empty() {
            return;
        }
        self.ask(pm_text::Request::Rename(name));
    }

    /// What a picker of `kind` offers when it opens.
    ///
    /// The lists that take time to gather open empty, or with what has been
    /// gathered so far, and are filled as the rest arrives.
    fn rows_for(&mut self, kind: Kind) -> Vec<Row> {
        match kind {
            Kind::Commands => self.command_rows(),
            Kind::LanguageServers => self.language_server_rows(),
            Kind::Files | Kind::WorkspaceSymbols => self.listed_file_rows(),
            Kind::Projects => self.project_rows(),
            Kind::Problems => self.problem_rows(),
            Kind::Agents => self.agent_rows(),
            Kind::AgentHistory(session) => self.agent_history_rows(session),
            Kind::Debug => self.debug_rows(),
            Kind::Processes => self.process_rows(),
            Kind::AttachAdapters => self.attach_adapter_rows(),
            Kind::Sessions => self.session_rows(),
            Kind::Tasks => self.task_rows(),
            Kind::Stashes => self.review().map_or_else(Vec::new, |review| {
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map_or(0, |time| time.as_secs());
                review
                    .stashes()
                    .into_iter()
                    .map(|stash| {
                        let age = now.saturating_sub(stash.when.max(0) as u64);
                        let age = if age < 3600 {
                            format!("{}m ago", age / 60)
                        } else if age < 86400 {
                            format!("{}h ago", age / 3600)
                        } else {
                            format!("{}d ago", age / 86400)
                        };
                        Row {
                            section: None,
                            label: format!("stash@{{{}}} {}", stash.index, stash.message),
                            detail: age,
                            choice: Choice::Stash(stash.index),
                            enabled: true,
                        }
                    })
                    .collect()
            }),
            Kind::SessionRepositories => self.session_repository_rows(),
            Kind::Modes => self
                .focused_talk()
                .map_or_else(Vec::new, |session| self.mode_rows(session)),
            Kind::AnswerOptions
            | Kind::Knob
            | Kind::References
            | Kind::Calls
            | Kind::ServerLogs
            | Kind::Font(_) => Vec::new(),
            Kind::Branches
            | Kind::FetchRemotes
            | Kind::PushRemotes
            | Kind::Search
            | Kind::Symbols
            | Kind::Line
            | Kind::Rename
            | Kind::RenameTerminal(_)
            | Kind::AnswerText
            | Kind::BreakpointCondition
            | Kind::BreakpointHits
            | Kind::BreakpointLog
            | Kind::Watch
            | Kind::NewBranch
            | Kind::StashMessage
            | Kind::NewSession
            | Kind::CloneUrl
            | Kind::LinkedPath
            | Kind::CopiedPath
            | Kind::PortVariable
            | Kind::ThemeColor(_)
            | Kind::ThemeName
            | Kind::KeymapName => Vec::new(),
        }
    }

    /// Every command the window can carry out, with the chords it answers to.
    fn command_rows(&self) -> Vec<Row> {
        let context = self.context();
        let has_buffer = self.active_file().is_some();
        let has_shell = self
            .scope()
            .is_some_and(|scope| self.terminals.active_id(scope).is_some());

        Action::all()
            .map(|action| Row {
                section: None,
                label: action.title().to_owned(),
                detail: self.keys_for(action, &context).unwrap_or_default(),
                choice: Choice::Act(action),
                enabled: (has_buffer || !action.needs_buffer())
                    && (action != Action::RenameTerminal || has_shell),
            })
            .collect()
    }

    /// Every server the editor can install, with installed ones marked.
    fn language_server_rows(&self) -> Vec<Row> {
        let commands = pm_text::Language::all()
            .iter()
            .flat_map(|language| language.servers())
            .filter(|server| server.install.is_some())
            .map(|server| server.command)
            .collect::<std::collections::BTreeSet<_>>();
        commands
            .into_iter()
            .map(|command| Row {
                section: None,
                label: command.to_owned(),
                detail: if pm_text::program::installed(command).is_some() {
                    "Installed"
                } else {
                    "Not installed"
                }
                .to_owned(),
                choice: Choice::InstallLanguageServer(command),
                enabled: true,
            })
            .collect()
    }

    /// The worktree the window is pointed at, with where it sits on disk.
    ///
    /// A tab is drawn only in the worktree it was opened from, so the files
    /// listed for the pickers are this worktree's alone: a file of any other
    /// would open where it cannot be seen.
    pub(super) fn here_on_disk(&self) -> Option<(pm_core::Scope, PathBuf)> {
        let scope = self.scope()?;
        Some((scope, self.root_of(scope)?))
    }

    /// The projects the window holds open.
    fn project_rows(&self) -> Vec<Row> {
        self.open
            .iter()
            .map(|project| Row {
                section: None,
                label: project.name().to_owned(),
                detail: project.branch().unwrap_or_default().to_owned(),
                choice: Choice::Project(project.id()),
                enabled: true,
            })
            .collect()
    }

    /// Asks git away from the window for every branch of the active
    /// project's active repository, for the branch picker.
    fn ask_branches(&self) {
        let Some(id) = self.open.active().map(pm_core::Project::id) else {
            return;
        };
        let Some(root) = self.repository_root(self.git_scope(id)) else {
            return;
        };
        self.ask_git_later(Kind::Branches, move || {
            branch_rows(id, pm_core::branches(&root))
        });
    }

    /// Asks git away from the window for every configured remote of the
    /// active project's active repository, for fetching or pushing.
    fn ask_remotes(&self, fetching: bool) {
        let Some(id) = self.open.active().map(pm_core::Project::id) else {
            return;
        };
        let Some(root) = self.repository_root(self.git_scope(id)) else {
            return;
        };
        let kind = match fetching {
            true => Kind::FetchRemotes,
            false => Kind::PushRemotes,
        };
        self.ask_git_later(kind, move || {
            remote_rows(id, pm_core::remotes(&root), fetching)
        });
    }

    /// Checks out `branch` in `project` through the window's branch seam.
    fn switch_branch(&mut self, project: ProjectId, branch: &str) {
        let branch = branch.to_owned();
        self.change_branch(project, move |root| pm_core::switch_branch(root, &branch));
    }

    /// Creates and checks out `name` in the active project.
    pub(super) fn create_branch(&mut self, name: &str) {
        let Some(project) = self.open.active().map(pm_core::Project::id) else {
            return;
        };
        let name = name.to_owned();
        self.change_branch(project, move |root| pm_core::create_branch(root, &name));
    }

    /// Runs one branch-changing operation in the active repository of
    /// `project`, and refreshes every view of the project.
    fn change_branch(
        &mut self,
        project: ProjectId,
        change: impl FnOnce(&Path) -> pm_core::Said + Send + 'static,
    ) {
        let scope = self.git_scope(project);
        let Some(repository) = self.repository_root(scope) else {
            return;
        };
        if self.editor.project_is_dirty(project) {
            self.branch_changed(
                project,
                Err("save or discard open editor changes before changing branch".to_owned()),
            );
            return;
        }
        self.change_branch_later(project, move || change(&repository));
    }

    /// Takes in what changing `project`'s branch came to, and refreshes
    /// every view of the project once it has changed.
    pub(super) fn branch_changed(&mut self, project: ProjectId, said: pm_core::Said) {
        let scope = self.git_scope(project);
        let Some(root) = self.root_of(scope) else {
            return;
        };
        let changed = said.is_ok();
        if let Some(review) = self.reviews.get_mut(&scope) {
            review.report(said);
        }
        self.reread_review_later(scope);
        if !changed {
            self.secondary_sidebar_view = crate::workspace::SidebarView::Changes;
            self.secondary_sidebar_open = true;
            return;
        }

        self.open.refresh(project);
        self.editor.reload_project(scope, &root);
        if let Some(tree) = self.files.get_mut(&scope) {
            tree.reload();
        }
        self.reread_changes();
        self.store();
    }

    /// Pushes the active branch, establishing its upstream when necessary.
    pub(super) fn push_branch(&mut self) {
        let Some(scope) = self.scope() else {
            return;
        };
        let upstream = self
            .reviews
            .get(&scope)
            .and_then(|review| review.head()?.upstream.as_ref())
            .is_some();
        self.remote_operation(RemoteOperation::Push, move |root| {
            pm_core::push_branch(root, upstream)
        });
    }

    /// Whether a spinner is owed its next frame, taking it up if so.
    pub(super) fn spun(&mut self) -> bool {
        if self
            .next_spin()
            .is_none_or(|next| std::time::Instant::now() < next)
        {
            return false;
        }
        self.spun = std::time::Instant::now();
        true
    }

    /// When a spinner next turns, while a remote is being waited on or a
    /// refresh control is turning.
    pub(super) fn next_spin(&self) -> Option<std::time::Instant> {
        let turning = self.remote_operation.is_some()
            || self
                .reviews
                .values()
                .any(|review| review.refresh_turn().is_some() || review.is_working());
        if turning {
            Some(self.spun + SPIN_FRAME)
        } else if self.agents.working() > 0 {
            Some(self.spun + AGENT_FRAME)
        } else {
            None
        }
    }

    /// Runs a remote Git operation away from the UI thread and wakes on completion.
    pub(super) fn remote_operation(
        &mut self,
        kind: RemoteOperation,
        operation: impl FnOnce(&Path) -> pm_core::Said + Send + 'static,
    ) {
        if self.remote_operation.is_some() {
            return;
        }
        let Some(scope) = self.scope() else {
            return;
        };
        self.run_in(scope, kind, operation);
    }

    /// Runs a remote Git operation in one explicitly named worktree.
    fn run_in(
        &mut self,
        scope: pm_core::Scope,
        kind: RemoteOperation,
        operation: impl FnOnce(&Path) -> pm_core::Said + Send + 'static,
    ) {
        if self.remote_operation.is_some() {
            return;
        }
        let Some(root) = self.repository_root(scope) else {
            return;
        };
        let results = self.git_results.clone();
        let wake = self.waker(Wake::Git);
        self.remote_operation = Some(kind);
        if let Some(review) = self.reviews.get_mut(&scope) {
            review.begin(kind.doing());
        }
        std::thread::spawn(move || {
            let said = operation(&root);
            if let Ok(mut results) = results.lock() {
                results.push((scope, said));
            }
            wake();
        });
    }

    /// Every error and warning a server has reported in an open file.
    fn problem_rows(&self) -> Vec<Row> {
        let mut rows = Vec::new();
        for (scope, path, file) in self.open_files() {
            let Some(document) = self.editor.get(file) else {
                continue;
            };
            let document = document.borrow();
            for found in document.buffer().diagnostics() {
                rows.push(Row {
                    section: None,
                    label: found.message.lines().next().unwrap_or_default().to_owned(),
                    detail: format!(
                        "{}:{}",
                        path.file_name().unwrap_or_default().to_string_lossy(),
                        found.range.start.line + 1
                    ),
                    choice: Choice::OpenAt(scope, path.clone(), found.range.start),
                    enabled: true,
                });
            }
        }
        rows
    }

    /// Every file the window has open, with the worktree and path it is in.
    pub(super) fn open_files(&self) -> Vec<(pm_core::Scope, PathBuf, crate::editor::FileId)> {
        self.panes
            .held()
            .into_iter()
            .filter_map(Item::file)
            .filter_map(|file| Some((self.editor.scope_of(file)?, self.editor.path(file)?, file)))
            .collect()
    }
}

/// The rows of `project`'s branches, local ones first and the current
/// branch marked and not to be chosen.
fn branch_rows(project: ProjectId, branches: Vec<pm_core::Branch>) -> Vec<Row> {
    branches
        .into_iter()
        .map(|branch| {
            let remote = branch.is_remote();
            let current = branch.is_current();
            Row {
                section: Some(if remote {
                    "Remote Branches"
                } else {
                    "Local Branches"
                }),
                label: match current {
                    true => format!("✓  {}", branch.name()),
                    false => branch.name().to_owned(),
                },
                detail: branch.detail().to_owned(),
                choice: Choice::Branch(project, branch.name().to_owned()),
                enabled: !current,
            }
        })
        .collect()
}

/// The rows of `project`'s remotes, for fetching from or pushing to.
fn remote_rows(project: ProjectId, remotes: Vec<String>, fetching: bool) -> Vec<Row> {
    remotes
        .into_iter()
        .map(|remote| Row {
            section: None,
            label: remote.clone(),
            detail: String::new(),
            choice: if fetching {
                Choice::FetchRemote(project, remote)
            } else {
                Choice::PushRemote(project, remote)
            },
            enabled: true,
        })
        .collect()
}

/// The row of the file at `path` in the worktree `scope` at `root`: its
/// name, and where it sits from the top of the worktree.
pub(super) fn file_row(scope: pm_core::Scope, root: &Path, path: PathBuf) -> Row {
    Row {
        section: None,
        label: path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default(),
        detail: relative(root, &path),
        choice: Choice::Open(scope, path),
        enabled: true,
    }
}

/// `path` written from `root` down.
pub(super) fn relative(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .display()
        .to_string()
}
