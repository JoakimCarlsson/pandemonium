//! What the window does about sessions: cutting one, going to it, ending it.
//!
//! A session is a worktree of a project with an agent turned loose in it, so
//! starting one is two things in one gesture: git cuts the worktree, and the
//! agent is started in it. Both go through here, and so does the sidebar's
//! reading of them — the rows under a project are this model, not a second
//! one kept beside it.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use pm_core::{FileTree, ProjectId, Scope, Session, SessionId};
use pm_ui::Theme;

use crate::agent::standing_color;
use crate::app::App;
use crate::config;
use crate::message::Message;
use crate::picker::{Choice, Kind, Row};
use crate::prompt::{Answer, Prompt};
use crate::workspace::{MenuTarget, SidebarProject, SidebarSession, counted};

impl App {
    /// Carries out the commands a session answers to.
    ///
    /// The answer says whether the message was one of them, so that the
    /// window can go on trying the rest.
    pub(super) fn session_command(&mut self, message: Message) -> bool {
        if let Message::SessionMenu(session) = message {
            self.open_menu(MenuTarget::Session(session));
            return true;
        }
        match message {
            Message::RunChecks(scope) => self.run_checks(scope, false),
            Message::ShowCheckOutput(scope) => self.show_check_output(scope),
            Message::NewSession => self.name_session_here(),
            Message::NewSessionFrom(project, place) => self.name_session(project, place),
            Message::SelectSession(session) => self.select_session(session),
            Message::FinishSession(session) => self.ask_finish_session(session),
            Message::EndSession(session) => self.end_session(session),
            _ => return false,
        }
        self.dismiss_menu();
        true
    }

    /// Opens the menu of what can be done to `project`, where the pointer is.
    ///
    /// The branches it offers to cut a session from are gathered once, away
    /// from the window, rather than while the menu is drawn: a menu is built
    /// every frame it is open, and asking git every frame for a list that
    /// cannot have changed is a subprocess a second for nothing. A project of
    /// several repositories offers none: a branch is one repository's, and a
    /// session of all of them is cut from what each has checked out. A branch
    /// whose remote branch was deleted is not offered, unless it is the one
    /// checked out: it is almost always one whose work has been merged.
    pub(super) fn open_project_menu(&mut self, project: ProjectId) {
        self.open.activate(project);
        self.session_bases = Vec::new();
        if let Some([only]) = self.open.get(project).map(pm_core::Project::repositories) {
            let root = only.root().to_path_buf();
            self.read_bases_later(project, move || {
                let branches = pm_core::branches(&root);
                let (checked_out, rest): (Vec<_>, Vec<_>) = branches
                    .iter()
                    .filter(|branch| branch.is_current() || !branch.is_gone())
                    .partition(|branch| branch.is_current());
                checked_out
                    .into_iter()
                    .chain(rest)
                    .map(|branch| branch.name().to_owned())
                    .collect()
            });
        }
        self.showing_bases = false;
        self.open_menu(MenuTarget::Project(project));
    }

    /// Offers `bases` to cut a session of `project` from, if its menu is
    /// still the one open.
    pub(super) fn take_bases(&mut self, project: ProjectId, bases: Vec<String>) {
        if self.menu.as_ref().map(|menu| menu.target) == Some(MenuTarget::Project(project)) {
            self.session_bases = bases;
        }
    }

    /// Asks what to call a session of the active project, cut from its head.
    ///
    /// This is what the sidebar's `+` and the keybinding mean: another
    /// session of what is already in front of the reader, from what that
    /// project has checked out. Cutting one from some other branch is the
    /// project's menu, where the branches are listed.
    pub(super) fn name_session_here(&mut self) {
        let Some(project) = self.open.active().map(pm_core::Project::id) else {
            return;
        };
        self.session_base = None;
        self.open.activate(project);
        self.open_picker_with(Kind::NewSession, Vec::new(), String::new());
    }

    /// Asks what to call a session of `project`, cut from the `place`-th base.
    ///
    /// The name is the whole of what a session is asked for: everything else
    /// — which project, which commit, where the worktree goes — is already
    /// known by the time the question is put, and nothing is started in the
    /// worktree that comes back. A session is somewhere to work; what runs
    /// there is opened in it afterwards, like anything else.
    pub(super) fn name_session(&mut self, project: ProjectId, place: usize) {
        self.open.activate(project);
        self.session_base = self.session_bases.get(place).cloned();
        self.open_picker_with(Kind::NewSession, Vec::new(), String::new());
    }

    /// Takes the name a session was given, and cuts it — or, for a project
    /// of several repositories, asks which of them it works in first.
    ///
    /// Every repository starts ticked: a session that works across all of
    /// them is the one a reader who just presses enter meant.
    pub(super) fn start_session(&mut self, name: &str) {
        let name = name.trim();
        let Some(project) = self.open.active().filter(|_| !name.is_empty()) else {
            return;
        };
        let roots = project
            .repositories()
            .iter()
            .map(|repository| repository.root().to_path_buf())
            .collect::<BTreeSet<_>>();

        self.session_name = name.to_owned();
        self.session_picks = roots;
        match self.session_picks.len() > 1 {
            true => self.open_picker(Kind::SessionRepositories),
            false => self.cut_session(),
        }
    }

    /// Ticks the repository at `root` for the session being picked for, or
    /// unticks it, and shows the list again where it was.
    pub(super) fn toggle_session_repository(&mut self, root: PathBuf, typed: String, place: usize) {
        if !self.session_picks.remove(&root) {
            self.session_picks.insert(root);
        }
        let rows = self.session_repository_rows();
        self.open_picker_with(Kind::SessionRepositories, rows, typed);
        if let Some(picker) = self.picker.as_mut() {
            picker.select(place);
        }
    }

    /// What the list of repositories to cut a session of offers: the line
    /// that cuts it, then a row per repository with its tick.
    pub(super) fn session_repository_rows(&self) -> Vec<Row> {
        let Some(project) = self.open.active() else {
            return Vec::new();
        };
        let total = project.repositories().len();
        let ticked = self.session_picks.len();
        let start = Row {
            section: None,
            label: format!("Start “{}”", self.session_name),
            detail: format!("{ticked} of {total} repositories"),
            choice: Choice::StartSession,
            enabled: ticked > 0,
        };
        let repositories = project.repositories().iter().map(|repository| {
            let root = repository.root();
            let tick = match self.session_picks.contains(root) {
                true => "✓",
                false => "  ",
            };
            Row {
                section: None,
                label: format!("{tick}  {}", within(project.root(), root)),
                detail: repository.branch().to_owned(),
                choice: Choice::SessionRepository(root.to_path_buf()),
                enabled: true,
            }
        });
        std::iter::once(start).chain(repositories).collect()
    }

    /// Cuts the session being named, of the repositories ticked for it, and
    /// points the window at it once git has cut it.
    pub(super) fn cut_session(&mut self) {
        let name = std::mem::take(&mut self.session_name);
        let chosen = std::mem::take(&mut self.session_picks)
            .into_iter()
            .collect::<Vec<_>>();
        let base = self.session_base.take().unwrap_or_default();
        let Some(under) = config::worktrees().filter(|_| !name.is_empty()) else {
            return;
        };
        let Some(project) = self.open.active().cloned() else {
            return;
        };

        self.cut_session_later(&project, &name, &base, &chosen, &under, None);
    }

    /// Points the window at `session`, bringing its agent forward if it has one.
    pub(super) fn select_session(&mut self, session: SessionId) {
        let Some(project) = self.sessions.get(session).map(Session::project) else {
            return;
        };
        let scope = Scope::of(project, session);
        self.point_at(scope);

        let Some(item) = self
            .agents
            .of_session(session)
            .map(|talk| crate::panes::Item::Agent(scope, talk))
        else {
            return;
        };
        let holder = self.panes.panes().into_iter().find(|pane| {
            self.panes
                .pane(*pane)
                .is_some_and(|pane| pane.items().any(|held| held == item))
        });
        match holder {
            Some(pane) => self.activate_tab(pane, item),
            None => self.show_item(self.panes.focus(), scope, item, false),
        }
    }

    /// Asks whether to finish `session`, saying what finishing it takes away.
    pub(super) fn ask_finish_session(&mut self, session: SessionId) {
        let Some(held) = self.sessions.get(session) else {
            return;
        };
        let risk = held.work_at_risk();
        let mut detail = vec![format!("the worktree at {}", held.root().display())];
        if risk.uncommitted_files > 0 {
            detail.push(format!(
                "{} uncommitted",
                counted(risk.uncommitted_files, "file")
            ));
        }
        if risk.unpushed_commits > 0 {
            detail.push(format!(
                "{} not on any remote",
                counted(risk.unpushed_commits, "commit")
            ));
        }
        if detail.len() == 1 {
            detail.push("nothing would be lost".to_owned());
        }

        self.ask_first(Prompt::asking(
            format!("Finish “{}”?", held.name()),
            detail,
            vec![
                Answer::new("Finish session", Message::EndSession(session)),
                Answer::cancel(),
            ],
        ));
    }

    /// Takes `session` off disk, having been told to.
    ///
    /// The worktree goes, so everything reading it goes with it once git has
    /// taken it away. What is left of the session is what was pushed out of
    /// it, which is git's, not ours.
    pub(super) fn end_session(&mut self, session: SessionId) {
        if self.sessions.get(session).is_some() {
            self.finish_session_later(session);
        }
    }

    /// Takes `session`, whose worktree is off disk, out of the window: its
    /// tabs, its shells, its tree and its list of changes.
    pub(super) fn forget_session(&mut self, session: SessionId) {
        let Some((scope, roots)) = self.sessions.get(session).map(|held| {
            (
                Scope::of(held.project(), session),
                std::iter::once(held.root())
                    .chain(held.roots())
                    .map(Path::to_path_buf)
                    .collect::<Vec<_>>(),
            )
        }) else {
            return;
        };
        self.editor.close_scope(scope, &roots);
        self.sessions.forget(session);

        self.drop_tabs(&|held| held == scope);
        self.terminals.stop_all(scope);
        self.tasks.forget_scope(scope);
        self.testing.worktrees.remove(&scope);
        self.checks.forget(scope);
        self.advance_checks();
        self.pending_debug.retain(|_, (held, _)| *held != scope);
        self.debuggers.forget(|held| held == scope);
        self.files.remove(&scope);
        self.reviews.remove(&scope);
        self.sweep();
        if self.session == Some(session) {
            self.select_checkout();
        }
        self.store();
    }

    /// Points the window back at the active project's own checkout.
    pub(super) fn select_checkout(&mut self) {
        let Some(project) = self.open.active().map(pm_core::Project::id) else {
            return;
        };
        self.point_at(Scope::checkout(project));
    }

    /// Points the window at `scope`, reading its worktree if it is new here.
    ///
    /// This is the one seam the window changes worktree through: the project
    /// it belongs to becomes the active one, the tabs, shells and tree that
    /// belong to that worktree come forward, and its files and changes are
    /// read the first time it is pointed at and kept afterwards — so coming
    /// back to a worktree finds it as it was left, expanded folders and all.
    pub(super) fn point_at(&mut self, scope: Scope) {
        self.open.activate(scope.project());
        self.sync_layout();
        self.session = scope.session();
        self.panes.inherit(scope);

        let Some(root) = self.root_of(scope) else {
            return;
        };
        self.files
            .entry(scope)
            .or_insert_with(|| FileTree::new(&root));
        if let std::collections::btree_map::Entry::Vacant(vacant) = self.reviews.entry(scope) {
            vacant.insert(crate::review::Review::of(&root));
            self.reread_review_later(scope);
        }
    }

    /// Asks git again how far the sessions have drifted, on a turn boundary.
    ///
    /// A row states how much work is in a worktree, so the number has to move
    /// as the agent writes — but git is a subprocess a session, and an agent
    /// says something several times a second. A turn starting or ending is
    /// when the answer can have changed by anything worth reading, so that is
    /// when it is asked for.
    pub(super) fn reread_worked_sessions(&mut self) {
        let working = self.agents.working();
        if working == self.working {
            return;
        }
        self.working = working;
        self.reread_drift_later();
    }

    /// The session the window is pointed at, if it is pointed at one.
    ///
    /// This is the one answer to "which worktree am I in": the file tree, the
    /// list of changes and anything started now all read it, and it changes
    /// only when the reader picks a row.
    pub(super) fn selected_session(&self) -> Option<SessionId> {
        self.session
    }

    /// The environment a program started in `scope`'s worktree is given.
    ///
    /// A session serves on a port of its own, and this is where that becomes
    /// a variable, so a shell and an agent in the same worktree are handed
    /// the same one. A project's own checkout is handed nothing: it is where
    /// the reader's own server runs, on whatever port the project says.
    pub(super) fn worktree_env(&self, scope: Scope) -> Vec<(String, String)> {
        let port = scope
            .session()
            .and_then(|session| self.sessions.get(session))
            .and_then(Session::port);
        self.preferences.bootstrap.env(port)
    }

    /// Every open project and the sessions hanging under it, for the sidebar.
    pub(super) fn sidebar_projects(&self) -> Vec<SidebarProject> {
        let theme = self.theme();
        let scope = self.scope();
        let selected = self.selected_session();

        self.open
            .iter()
            .map(|project| SidebarProject {
                project: project.id(),
                at_checkout: scope == Some(Scope::checkout(project.id())),
                sessions: self
                    .sessions
                    .of(project.id())
                    .map(|session| {
                        let summary = session.summary();
                        SidebarSession {
                            id: session.id(),
                            name: session.name().to_owned(),
                            parent: session.delegation().map(|parent| parent.name.clone()),
                            depth: session.delegation().map_or(0, |parent| parent.depth),
                            added: summary.added,
                            removed: summary.removed,
                            status_color: self.session_color(&theme, session.id()),
                            pending: self
                                .reviews
                                .get(&Scope::of(project.id(), session.id()))
                                .map_or(0, |review| review.comments().pending()),
                            selected: selected == Some(session.id()),
                        }
                    })
                    .collect(),
            })
            .collect()
    }

    /// What colour the dot beside `session` is drawn in.
    ///
    /// The state is the agent's, read off the conversation running in the
    /// worktree: stopped, waiting on the reader, working, or doing none of
    /// those. A session with no agent in it is one nobody is waiting for
    /// either way.
    fn session_color(&self, theme: &Theme, session: SessionId) -> pm_gfx::Rgba {
        self.agents
            .of_session(session)
            .and_then(|id| self.agents.get(id))
            .map_or(theme.colors.text_subtle, |talk| {
                standing_color(theme, talk.standing())
            })
    }

    /// Sessions across every project, with the same health as sidebar rows.
    pub(super) fn session_rows(&self) -> Vec<Row> {
        self.open
            .iter()
            .flat_map(|project| {
                self.sessions.of(project.id()).map(|session| {
                    let scope = Scope::of(project.id(), session.id());
                    Row {
                        section: None,
                        label: session.name().to_owned(),
                        detail: format!("{} · {}", project.name(), self.checks.detail(scope)),
                        choice: Choice::Session(session.id(), self.checks.health(scope)),
                        enabled: true,
                    }
                })
            })
            .collect()
    }

    /// Says what could not be brought into a worktree that was cut anyway.
    ///
    /// The session is already there and already selected: this is a reading
    /// of what is missing from it, so that an agent failing to install or
    /// serve is explained before it happens rather than after.
    pub(super) fn say_bootstrap_trouble(&mut self, trouble: &[String]) {
        if trouble.is_empty() {
            return;
        }
        self.ask_first(Prompt::asking(
            "The worktree was cut, but not everything came with it".to_owned(),
            trouble.to_vec(),
            vec![Answer::understood()],
        ));
    }

    /// Says that something could not be done, and what git made of it.
    pub(super) fn say_trouble(&mut self, asked: &str, trouble: &impl std::fmt::Display) {
        self.ask_first(Prompt::asking(
            asked.to_owned(),
            vec![trouble.to_string()],
            vec![Answer::understood()],
        ));
    }
}

/// Where `root` sits in the project at `project`, for a row that names it.
fn within(project: &Path, root: &Path) -> String {
    match root.strip_prefix(project) {
        Ok(relative) if !relative.as_os_str().is_empty() => relative.display().to_string(),
        _ => root
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default(),
    }
}
