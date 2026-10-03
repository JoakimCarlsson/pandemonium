//! Asking git about the worktrees, and having it change them, without
//! holding a frame up.
//!
//! An agent writes into its worktree and runs git in it several times a
//! second, and every one of those is news for the review of that worktree
//! and the drift of every session. The asking is a subprocess or more a
//! file, so it runs on a thread of its own and wakes the window with the
//! answer; one worktree is read at most once at a time, and news that comes
//! in while it is being read is read again once that reading is back.
//!
//! What the reader has git do to a worktree, staging to committing, is
//! carried out the same way: one piece of work at a time per worktree, never
//! beside a reading of it, in the order it was asked for, and the worktree is
//! read again once the last of it is back. Cutting and finishing a session,
//! and looking for the projects' repositories and sessions again, are the
//! rest of what is handed out from here.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use pm_core::{Cutting, Found, ProjectId, Repository, Scope, SessionId, StartError, Summary};

use crate::app::{App, Wake};
use crate::review::{Done, Reading, Work};

/// What one piece of work away from the window came back with.
enum Back {
    /// A worktree's review, read whole.
    Review(Scope, Reading),
    /// How far every session has drifted.
    Drift(Vec<(SessionId, Summary)>),
    /// What git said once it had done something to a worktree.
    Worked(Scope, Done),
    /// What the last commit holds for each file of a worktree's excerpts.
    Committed(Scope, Vec<(PathBuf, Option<String>)>),
    /// A worktree's review comments, written down.
    Remembered(Scope),
    /// The repositories of every project, and the sessions found on disk.
    Changes(Changes),
    /// A session's worktrees, cut or refused.
    Cut(Result<Cutting, StartError>),
    /// A session taken off disk, or why it could not be.
    Finished(SessionId, Result<(), StartError>),
    /// What git said once it had changed a project's branch.
    Branched(ProjectId, pm_core::Said),
    /// The branches a session of a project can be cut from, checked out
    /// first.
    Bases(ProjectId, Vec<String>),
}

/// The repositories of every open project, and the sessions they have on
/// disk that the window did not hold, as they were found.
struct Changes {
    /// Each project's repositories.
    repositories: Vec<(ProjectId, Vec<Repository>)>,
    /// The sessions found, when there is a home for them to be found in.
    sessions: Vec<Found>,
}

/// The work running away from the window, and what it came back with.
#[derive(Default)]
pub(super) struct Readings {
    /// What has come back and not been taken in yet.
    done: Arc<Mutex<Vec<Back>>>,
    /// The worktrees whose review is being read now.
    reading: BTreeSet<Scope>,
    /// The worktrees whose review was asked for again while it was read, or
    /// while git was doing something to them.
    again: BTreeSet<Scope>,
    /// The work waiting for its worktree, in the order it was asked for.
    queued: BTreeMap<Scope, VecDeque<Work>>,
    /// The worktrees git is doing something to now.
    working: BTreeSet<Scope>,
    /// The worktrees whose excerpts' last commit is being read now.
    excerpting: BTreeSet<Scope>,
    /// The worktrees whose excerpts were asked for again while they were read.
    excerpt_again: BTreeSet<Scope>,
    /// The worktrees whose review comments are being written down now.
    remembering: BTreeSet<Scope>,
    /// The worktrees whose comments changed again while they were written.
    remember_again: BTreeSet<Scope>,
    /// Whether the sessions' drift is being read now.
    drifting: bool,
    /// Whether it was asked for again while it was read.
    drift_again: bool,
    /// Whether the projects' repositories and sessions are being looked for.
    changing: bool,
    /// Whether they were asked for again while they were looked for.
    change_again: bool,
    /// The sessions being taken off disk now.
    finishing: BTreeSet<SessionId>,
}

impl App {
    /// Asks git again what `scope`'s worktree holds, and shows it once git
    /// has answered.
    pub(super) fn reread_review_later(&mut self, scope: Scope) {
        if self.readings.reading.contains(&scope) || self.readings.working.contains(&scope) {
            self.readings.again.insert(scope);
            return;
        }
        let Some(read) = self
            .reviews
            .get(&scope)
            .map(crate::review::Review::read_later)
        else {
            return;
        };
        self.readings.reading.insert(scope);
        self.spawn_read(move || Back::Review(scope, read()));
    }

    /// Has git carry `work` out in `scope`'s worktree once whatever it is
    /// doing there already is done, and reads the worktree again after it.
    ///
    /// Its repositories read as busy from the moment it is asked for, not
    /// from when it starts: work waiting behind a reading is work the reader
    /// has to see is under way.
    pub(super) fn work_later(&mut self, scope: Scope, work: Option<Work>) {
        let Some(work) = work else {
            return;
        };
        if let Some(review) = self.reviews.get_mut(&scope) {
            review.began(&work);
        }
        self.readings
            .queued
            .entry(scope)
            .or_default()
            .push_back(work);
        self.work_next(scope);
    }

    /// Writes `scope`'s review comments into its worktree's git directory,
    /// away from the window, when they have changed since they last were.
    ///
    /// One write is under way per worktree at a time, so that a later state
    /// is never overtaken by an earlier one still being written.
    pub(super) fn remember_comments_later(&mut self, scope: Scope) {
        if self.readings.remembering.contains(&scope) {
            self.readings.remember_again.insert(scope);
            return;
        }
        let Some(review) = self.reviews.get(&scope) else {
            return;
        };
        let Some(text) = review.comments().take_unsaved() else {
            return;
        };
        let root = review.root().to_path_buf();
        self.readings.remembering.insert(scope);
        self.spawn_read(move || {
            pm_core::remember_review(&root, &text);
            Back::Remembered(scope)
        });
    }

    /// Starts the next piece of work waiting for `scope`, unless git is
    /// busy with that worktree already.
    fn work_next(&mut self, scope: Scope) {
        if self.readings.reading.contains(&scope) || self.readings.working.contains(&scope) {
            return;
        }
        let Some(work) = self
            .readings
            .queued
            .get_mut(&scope)
            .and_then(VecDeque::pop_front)
        else {
            return;
        };
        self.readings.working.insert(scope);
        self.spun = std::time::Instant::now();
        self.spawn_read(move || Back::Worked(scope, work.run()));
    }

    /// Whether work is waiting for `scope`'s worktree.
    fn has_queued(&self, scope: Scope) -> bool {
        self.readings
            .queued
            .get(&scope)
            .is_some_and(|queued| !queued.is_empty())
    }

    /// Asks git again how far every session has drifted, and shows it once
    /// git has answered.
    pub(super) fn reread_drift_later(&mut self) {
        if self.readings.drifting {
            self.readings.drift_again = true;
            return;
        }
        let read = self.sessions.read_drift();
        self.readings.drifting = true;
        self.spawn_read(move || Back::Drift(read()));
    }

    /// Reads what the last commit holds for every changed file of `scope`'s
    /// excerpts, and puts the excerpts together once it is back.
    pub(super) fn reread_excerpts_later(&mut self, scope: Scope, paths: Vec<PathBuf>) {
        if self.readings.excerpting.contains(&scope) {
            self.readings.excerpt_again.insert(scope);
            return;
        }
        let Some(root) = self.root_of(scope) else {
            return;
        };
        self.readings.excerpting.insert(scope);
        self.spawn_read(move || {
            let committed = paths
                .into_iter()
                .map(|path| {
                    let held = pm_core::committed(&root, &path);
                    (path, held)
                })
                .collect();
            Back::Committed(scope, committed)
        });
    }

    /// Looks for every project's repositories and sessions again, and asks
    /// git again what every worktree the window is holding makes of itself,
    /// once what was found is back.
    ///
    /// Each project's repositories are looked for again first, so a session
    /// is cut from the repositories that are there now.
    pub(super) fn reread_changes(&mut self) {
        if self.readings.changing {
            self.readings.change_again = true;
            return;
        }
        let read = self.read_changes();
        self.readings.changing = true;
        self.spawn_read(move || Back::Changes(read()));
    }

    /// Looks for every project's repositories and sessions again, and takes
    /// in what was found before answering.
    ///
    /// This is for a launch, whose restored tabs name the sessions they were
    /// in and have to find them already held.
    pub(super) fn reread_changes_now(&mut self) {
        let read = self.read_changes();
        self.take_changes(read());
    }

    /// What looks for every project's repositories and sessions again, on
    /// whichever thread it is called on.
    fn read_changes(&self) -> impl FnOnce() -> Changes + Send + 'static {
        let repositories = self.open.read_later();
        let projects = self.open.iter().cloned().collect::<Vec<_>>();
        let sessions =
            crate::config::worktrees().map(|under| self.sessions.find_later(&projects, &under));
        move || Changes {
            repositories: repositories(),
            sessions: sessions.map(|find| find()).unwrap_or_default(),
        }
    }

    /// Cuts a session of `project`, as [`pm_core::Sessions::cut_later`]
    /// says, and points the window at it once it is cut.
    pub(super) fn cut_session_later(
        &mut self,
        project: &pm_core::Project,
        name: &str,
        base: &str,
        chosen: &[PathBuf],
        under: &std::path::Path,
    ) {
        let cut = self.sessions.cut_later(
            project,
            name,
            base,
            chosen,
            under,
            &self.preferences.bootstrap,
        );
        self.spawn_read(move || Back::Cut(cut()));
    }

    /// Takes `session`'s worktrees off disk, and everything reading them out
    /// of the window once they are gone.
    pub(super) fn finish_session_later(&mut self, session: SessionId) {
        if !self.readings.finishing.insert(session) {
            return;
        }
        let finish = self.sessions.finish_later(session);
        self.spawn_read(move || Back::Finished(session, finish()));
    }

    /// Changes `project`'s branch with `change`, and refreshes every view of
    /// the project once git has done it.
    pub(super) fn change_branch_later(
        &mut self,
        project: ProjectId,
        change: impl FnOnce() -> pm_core::Said + Send + 'static,
    ) {
        self.spawn_read(move || Back::Branched(project, change()));
    }

    /// Lists the branches a session of `project` can be cut from with
    /// `list`, and offers them once git has answered.
    pub(super) fn read_bases_later(
        &mut self,
        project: ProjectId,
        list: impl FnOnce() -> Vec<String> + Send + 'static,
    ) {
        self.spawn_read(move || Back::Bases(project, list()));
    }

    /// Runs `read` on a thread of its own, waking the window with what it
    /// came back with.
    fn spawn_read(&self, read: impl FnOnce() -> Back + Send + 'static) {
        let done = self.readings.done.clone();
        let wake = self.waker(Wake::Reading);
        std::thread::spawn(move || {
            let read = read();
            if let Ok(mut done) = done.lock() {
                done.push(read);
            }
            wake();
        });
    }

    /// Takes in every piece of work that has come back, answering whether
    /// any did, and starts again the ones asked for while they ran.
    pub(super) fn take_readings(&mut self) -> bool {
        let done = self
            .readings
            .done
            .lock()
            .map(|mut done| std::mem::take(&mut *done))
            .unwrap_or_default();
        let any = !done.is_empty();
        for back in done {
            match back {
                Back::Review(scope, reading) => self.take_review(scope, reading),
                Back::Drift(drifts) => self.take_drift(drifts),
                Back::Worked(scope, done) => self.take_worked(scope, done),
                Back::Committed(scope, committed) => self.take_committed(scope, committed),
                Back::Remembered(scope) => {
                    self.readings.remembering.remove(&scope);
                    if self.readings.remember_again.remove(&scope) {
                        self.remember_comments_later(scope);
                    }
                }
                Back::Changes(changes) => {
                    self.readings.changing = false;
                    self.take_changes(changes);
                    if std::mem::take(&mut self.readings.change_again) {
                        self.reread_changes();
                    }
                }
                Back::Cut(cut) => self.take_cut(cut),
                Back::Finished(session, finished) => self.take_finished(session, finished),
                Back::Branched(project, said) => self.branch_changed(project, said),
                Back::Bases(project, bases) => self.take_bases(project, bases),
            }
        }
        any
    }

    /// Puts `reading` into `scope`'s review and everything drawn from it.
    fn take_review(&mut self, scope: Scope, reading: Reading) {
        self.readings.reading.remove(&scope);
        if self
            .reviews
            .get_mut(&scope)
            .is_some_and(|review| review.take(reading))
        {
            self.open_reviewed_files_of(scope);
            self.repaint_reviews();
            self.refresh_excerpts_of(scope);
            self.remember_comments_later(scope);
        }
        if self.has_queued(scope) {
            self.work_next(scope);
        } else if self.readings.again.remove(&scope) {
            self.reread_review_later(scope);
        } else if let Some(review) = self.reviews.get_mut(&scope) {
            review.settle_work();
        }
    }

    /// Puts what git said into `scope`'s review, and goes on to the next
    /// piece of work there or reads the worktree again.
    fn take_worked(&mut self, scope: Scope, done: Done) {
        self.readings.working.remove(&scope);
        let push = done.wants_push();
        for trouble in done.troubles() {
            self.notices.trouble(
                trouble,
                Some(crate::message::Message::ShowTool(
                    crate::panes::Tool::Changes,
                )),
            );
        }
        if let Some(review) = self.reviews.get_mut(&scope) {
            review.finished(done);
        }
        if self.has_queued(scope) {
            self.readings.again.insert(scope);
            self.work_next(scope);
        } else {
            self.readings.again.remove(&scope);
            self.reread_review_later(scope);
        }
        if push && self.scope() == Some(scope) {
            self.push_branch();
        }
    }

    /// Puts how far the sessions have drifted into their rows.
    fn take_drift(&mut self, drifts: Vec<(SessionId, Summary)>) {
        self.readings.drifting = false;
        self.sessions.drifted(drifts);
        if std::mem::take(&mut self.readings.drift_again) {
            self.reread_drift_later();
        } else {
            self.check_changed_turns();
        }
    }

    /// Puts `scope`'s excerpts together from the files `committed` names
    /// and what the last commit holds for each.
    fn take_committed(&mut self, scope: Scope, committed: Vec<(PathBuf, Option<String>)>) {
        self.readings.excerpting.remove(&scope);
        self.set_excerpts(scope, committed);
        if self.readings.excerpt_again.remove(&scope) {
            self.refresh_excerpts_of(scope);
        }
    }

    /// Takes in the repositories and sessions `changes` found, and asks
    /// again what every worktree the window is holding makes of itself.
    ///
    /// A worktree the window is pointed at is read for the first time here;
    /// one whose project has closed is forgotten, because a review of a
    /// worktree nobody is looking at answers a question nobody asked.
    fn take_changes(&mut self, changes: Changes) {
        self.open.reread(changes.repositories);
        self.sessions.adopt(changes.sessions);
        self.reread_drift_later();
        if let Some(scope) = self.scope() {
            self.point_at(scope);
        }
        self.watch_worktrees();
        self.reviews
            .retain(|scope, _| self.open.get(scope.project()).is_some());
        let scopes = self.reviews.keys().copied().collect::<Vec<_>>();
        for scope in scopes {
            self.reread_review_later(scope);
        }
    }

    /// Points the window at a session that has been cut, or says why it
    /// could not be.
    fn take_cut(&mut self, cut: Result<Cutting, StartError>) {
        match cut {
            Ok(cutting) => {
                let started = self.sessions.took(cutting);
                self.select_session(started.id);
                self.say_bootstrap_trouble(&started.trouble);
            }
            Err(trouble) => self.say_trouble("The session could not be cut", &trouble),
        }
    }

    /// Takes a session that has gone off disk out of the window, or says why
    /// it could not be taken off.
    fn take_finished(&mut self, session: SessionId, finished: Result<(), StartError>) {
        self.readings.finishing.remove(&session);
        match finished {
            Ok(()) => self.forget_session(session),
            Err(trouble) => self.say_trouble("The session could not be finished", &trouble),
        }
    }
}
