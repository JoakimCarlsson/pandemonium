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

/// Most results a project-wide search gathers before it stops looking.
const SEARCH_LIMIT: usize = 500;

/// Longest a line of context beside a search result is drawn.
const CONTEXT: usize = 120;

/// How long the spinner holds each frame while a remote is waited on.
const SPIN_FRAME: std::time::Duration = std::time::Duration::from_millis(33);

/// How often a running agent's activity mark advances.
const AGENT_FRAME: std::time::Duration = std::time::Duration::from_millis(250);

impl App {
    /// Opens the picker of `kind`, gathering what it offers.
    pub(super) fn open_picker(&mut self, kind: Kind) {
        self.agent_picker_at = None;
        if !matches!(kind, Kind::Branches | Kind::NewBranch) {
            self.branch_picker_at = None;
        }
        let seeded = match kind {
            Kind::Search => self
                .with_buffer(pm_text::Buffer::selected_text)
                .unwrap_or_default(),
            _ => String::new(),
        };
        let rows = self.rows_for(kind, &seeded);
        self.open_picker_with(kind, rows, seeded);
    }

    /// Opens the picker of `kind` over `rows`, with `seeded` already typed.
    pub(super) fn open_picker_with(&mut self, kind: Kind, rows: Vec<Row>, seeded: String) {
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

    /// Puts the picker away, saying whether one was open.
    pub(super) fn dismiss_picker(&mut self) -> bool {
        self.branch_picker_at = None;
        self.agent_picker_at = None;
        self.picker.take().is_some()
    }

    /// Narrows the picker to what has been typed, re-gathering when it must.
    pub(super) fn refilter_picker(&mut self) {
        let Some(picker) = self.picker.as_ref() else {
            return;
        };
        if picker.kind() == Kind::WorkspaceSymbols {
            let query = picker.field().value().to_owned();
            return self.ask_workspace_symbols(query);
        }
        if !picker.kind().is_queried() {
            return;
        }
        let (kind, query) = (picker.kind(), picker.field().value().to_owned());
        let rows = self.rows_for(kind, &query);
        if let Some(picker) = self.picker.as_mut() {
            picker.refill(rows);
        }
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
        match (kind, chosen) {
            (Kind::Branches, _) if !typed.trim().is_empty() => self.create_branch(typed.trim()),
            (Kind::Line, _) => self.go_to_typed_line(&typed),
            (Kind::Rename, _) => self.rename_to(typed),
            (Kind::NewBranch, _) => self.create_branch(&typed),
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
            Choice::AgentHistory(session, saved) => self.open_agent_history(session, &saved),
            Choice::Mode(session, mode) => self.set_agent_mode(session, &mode),
            Choice::Knob(session, knob, value) => self.set_knob(session, &knob, &value),
            Choice::Font(slot, family) => self.set_font(slot, family),
            Choice::Debug(scope, scenario) => self.start_debugging(scope, *scenario),
            Choice::SessionRepository(_) => {}
            Choice::StartSession => self.cut_session(),
            Choice::Branch(project, branch) => self.switch_branch(project, &branch),
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

    /// What a picker of `kind` offers, given what has been typed so far.
    fn rows_for(&self, kind: Kind, query: &str) -> Vec<Row> {
        match kind {
            Kind::Commands => self.command_rows(),
            Kind::Files => self.file_rows(),
            Kind::Projects => self.project_rows(),
            Kind::Branches => self.branch_rows(),
            Kind::FetchRemotes => self.remote_rows(true),
            Kind::PushRemotes => self.remote_rows(false),
            Kind::Problems => self.problem_rows(),
            Kind::Agents => self.agent_rows(),
            Kind::AgentHistory(session) => self.agent_history_rows(session),
            Kind::Debug => self.debug_rows(),
            Kind::SessionRepositories => self.session_repository_rows(),
            Kind::Modes => self
                .focused_talk()
                .map_or_else(Vec::new, |session| self.mode_rows(session)),
            Kind::Knob
            | Kind::References
            | Kind::WorkspaceSymbols
            | Kind::Calls
            | Kind::Font(_) => Vec::new(),
            Kind::Search => self.search_rows(query),
            Kind::Symbols
            | Kind::Line
            | Kind::Rename
            | Kind::NewBranch
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

        Action::all()
            .map(|action| Row {
                section: None,
                label: action.title().to_owned(),
                detail: self
                    .resolver
                    .keymap()
                    .sequence_for(action, &context)
                    .map(ToString::to_string)
                    .unwrap_or_default(),
                choice: Choice::Act(action),
                enabled: has_buffer || !action.needs_buffer(),
            })
            .collect()
    }

    /// Every file of every worktree the window is holding.
    fn file_rows(&self) -> Vec<Row> {
        self.worktrees()
            .into_iter()
            .flat_map(|(id, root)| {
                pm_core::walk(&root).into_iter().map(move |path| {
                    let name = path
                        .file_name()
                        .map(|name| name.to_string_lossy().into_owned())
                        .unwrap_or_default();
                    Row {
                        section: None,
                        label: name,
                        detail: relative(&root, &path),
                        choice: Choice::Open(id, path),
                        enabled: true,
                    }
                })
            })
            .collect()
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

    /// Every local branch of the active project's active repository, current
    /// branch first.
    fn branch_rows(&self) -> Vec<Row> {
        let Some(id) = self.open.active().map(pm_core::Project::id) else {
            return Vec::new();
        };
        let Some(root) = self.repository_root(self.git_scope(id)) else {
            return Vec::new();
        };
        pm_core::branches(&root)
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
                    choice: Choice::Branch(id, branch.name().to_owned()),
                    enabled: !current,
                }
            })
            .collect()
    }

    /// Every configured remote of the active project's active repository, for
    /// fetching or pushing.
    fn remote_rows(&self, fetching: bool) -> Vec<Row> {
        let Some(id) = self.open.active().map(pm_core::Project::id) else {
            return Vec::new();
        };
        let Some(root) = self.repository_root(self.git_scope(id)) else {
            return Vec::new();
        };
        pm_core::remotes(&root)
            .into_iter()
            .map(|remote| Row {
                section: None,
                label: remote.clone(),
                detail: String::new(),
                choice: if fetching {
                    Choice::FetchRemote(id, remote)
                } else {
                    Choice::PushRemote(id, remote)
                },
                enabled: true,
            })
            .collect()
    }

    /// Checks out `branch` in `project` through the window's branch seam.
    fn switch_branch(&mut self, project: ProjectId, branch: &str) {
        self.change_branch(project, |root| pm_core::switch_branch(root, branch));
    }

    /// Creates and checks out `name` in the active project.
    pub(super) fn create_branch(&mut self, name: &str) {
        let Some(project) = self.open.active().map(pm_core::Project::id) else {
            return;
        };
        self.change_branch(project, |root| pm_core::create_branch(root, name));
    }

    /// Runs one branch-changing operation in the active repository of
    /// `project`, and refreshes every view of the project.
    fn change_branch(&mut self, project: ProjectId, change: impl FnOnce(&Path) -> pm_core::Said) {
        let scope = self.git_scope(project);
        let (Some(root), Some(repository)) = (self.root_of(scope), self.repository_root(scope))
        else {
            return;
        };
        let said = if self.editor.project_is_dirty(project) {
            Err("save or discard open editor changes before changing branch".to_owned())
        } else {
            change(&repository)
        };
        let changed = said.is_ok();
        if let Some(review) = self.reviews.get_mut(&scope) {
            review.report(said);
        }
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
                .any(|review| review.refresh_turn().is_some());
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

    /// Every place `query` appears in the worktrees the window is holding.
    fn search_rows(&self, query: &str) -> Vec<Row> {
        if query.len() < 2 {
            return Vec::new();
        }
        let needle = query.to_lowercase().chars().collect::<Vec<_>>();
        let mut rows = Vec::new();

        for (id, root) in self.worktrees() {
            for path in pm_core::walk(&root) {
                if rows.len() >= SEARCH_LIMIT {
                    return rows;
                }
                let Ok(text) = std::fs::read_to_string(&path) else {
                    continue;
                };
                for (line, content) in text.lines().enumerate() {
                    let Some(column) = found_at(content, &needle) else {
                        continue;
                    };
                    rows.push(Row {
                        section: None,
                        label: content.trim().chars().take(CONTEXT).collect(),
                        detail: format!("{}:{}", relative(&root, &path), line + 1),
                        choice: Choice::OpenAt(id, path.clone(), Position::new(line, column)),
                        enabled: true,
                    });
                    if rows.len() >= SEARCH_LIMIT {
                        return rows;
                    }
                }
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

/// Which character of `line` the lower-cased `needle` first appears at.
///
/// The comparison is made over characters rather than over bytes, because
/// lower-casing a line can change how many bytes it takes: an offset into
/// the folded copy is not an offset into the line it came from.
fn found_at(line: &str, needle: &[char]) -> Option<usize> {
    let folded = line.to_lowercase().chars().collect::<Vec<_>>();
    if needle.is_empty() || needle.len() > folded.len() {
        return None;
    }
    (0..=folded.len() - needle.len())
        .find(|start| folded[*start..start + needle.len()] == *needle)
        .filter(|start| *start <= line.chars().count())
}

/// `path` written from `root` down.
fn relative(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .display()
        .to_string()
}
