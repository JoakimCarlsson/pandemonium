//! The work behind the screen: the agents that run, the worktrees they work in and the git operations on them.

use std::cell::Cell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};

use pm_acp::{AGENTS, Chat, Discovery, Event};
use pm_pages::{Outgoing, Screen};
use pm_renderer::Picture;
use pm_state::State;

/// What a thread that did some of the work reports back.
enum Job {
    /// The picture at this path was decoded, when it is one.
    Previewed(String, Option<Picture>),
    /// The agent at this position was signed in, or could not be.
    SignedIn(usize, Result<(), String>),
    /// What the agent at this position lets the reader change and offers behind a slash.
    Discovered(usize, Discovery),
    /// A worktree for a new session is ready, or could not be made; the agent is started in it with the words.
    Worktree {
        /// The session, by position.
        session: usize,
        /// What came of making the worktree: where it is and its branch.
        result: Result<(PathBuf, String), String>,
        /// What the reader wrote.
        text: String,
        /// The files attached, by path.
        files: Vec<String>,
        /// The settings the reader changed before starting.
        settings: Vec<(String, String)>,
    },
    /// The folder of a session was looked at again.
    Refreshed {
        /// The session, by position.
        session: usize,
        /// The files that differ from the last commit.
        changes: Vec<pm_git::Change>,
        /// The commits of the session's own branch, when it has one.
        commits: Option<Vec<String>>,
    },
    /// A commit was made in the folder of a session, by position.
    Committed(usize),
    /// Something the reader asked for could not be done.
    Failed {
        /// The session, by position.
        session: usize,
        /// What went wrong.
        reason: String,
    },
}

/// Where a session works and what it is made of.
struct Placement {
    /// The folder its agent works in.
    directory: PathBuf,
    /// Which agent runs it, by position.
    agent: usize,
    /// The branch its worktree was made from, when it has one of its own.
    base: Option<String>,
    /// The folder of the project it belongs to.
    repository: PathBuf,
    /// What it is called.
    title: String,
}

/// The agents that are running and the threads doing work for them.
pub struct Work {
    /// The running agent of each session, by the session's position.
    chats: HashMap<usize, Chat>,
    /// What the threads report.
    jobs: Receiver<Job>,
    /// Handed to each thread.
    reporter: Sender<Job>,
    /// Whether the agents have been asked what they let the reader change.
    discovered: bool,
    /// How many threads have yet to report.
    pending: Cell<usize>,
}

impl Work {
    /// No agents running and nothing in progress.
    pub fn new() -> Self {
        let (reporter, jobs) = mpsc::channel();
        Self {
            chats: HashMap::new(),
            jobs,
            reporter,
            discovered: false,
            pending: Cell::new(0),
        }
    }

    /// Asks each agent what it lets the reader change, once the workspace is showing.
    pub fn discover(&mut self, model: &State<Screen>) {
        if self.discovered || !model.get().is_workspace() {
            return;
        }
        self.discovered = true;
        for (index, agent) in AGENTS.iter().copied().enumerate() {
            self.spawn(move |reporter| {
                let found = pm_acp::discover(agent).unwrap_or_default();
                let _ = reporter.send(Job::Discovered(index, found));
            });
        }
    }

    /// Carries out what the reader asked of the agents and the folders since the last look, and returns what to copy.
    pub fn deliver(&mut self, model: &State<Screen>) -> Vec<String> {
        let mut outbox = Vec::new();
        model.update(|screen| outbox = screen.take_outbox());
        let mut copies = Vec::new();
        for outgoing in outbox {
            match outgoing {
                Outgoing::Start {
                    session,
                    text,
                    files,
                    settings,
                    base,
                } => self.start(model, session, text, files, settings, base),
                Outgoing::Prompt { session, text, files } => {
                    if !self.chats.contains_key(&session) {
                        self.begin(model, session, &[]);
                    }
                    if let Some(chat) = self.chats.get(&session) {
                        chat.prompt(&text, &files);
                    }
                }
                Outgoing::Choose {
                    session,
                    ticket,
                    option,
                } => {
                    if let Some(chat) = self.chats.get(&session) {
                        chat.choose(ticket, Some(option));
                    }
                }
                Outgoing::Cancel { session } => {
                    if let Some(chat) = self.chats.get(&session) {
                        chat.cancel();
                    }
                }
                Outgoing::Set {
                    session,
                    setting,
                    value,
                } => {
                    if let Some(chat) = self.chats.get(&session) {
                        chat.set(&setting, &value);
                    }
                }
                Outgoing::Stage {
                    session,
                    paths,
                    stage,
                } => self.stage(model, session, paths, stage),
                Outgoing::Commit { session, message } => self.commit(model, session, message),
                Outgoing::Accounts { logins, in_use } => {
                    let _ = pm_store::save_accounts(&logins, &in_use);
                    apply_accounts(&logins, &in_use);
                }
                Outgoing::SignIn { agent } => {
                    if let Some(held) = AGENTS.get(agent).copied() {
                        self.spawn(move |reporter| {
                            let _ = reporter.send(Job::SignedIn(agent, held.sign_in()));
                        });
                    }
                }
                Outgoing::Copy { text } => copies.push(text),
            }
        }
        copies
    }

    /// Takes in what the running agents and the threads have reported since the last look.
    pub fn poll(&mut self, model: &State<Screen>) {
        let mut heard: Vec<(usize, Event)> = Vec::new();
        for (session, chat) in &self.chats {
            heard.extend(chat.events().into_iter().map(|event| (*session, event)));
        }
        for (session, event) in heard {
            let ended = matches!(event, Event::Finished(_) | Event::Failed(_));
            if matches!(event, Event::Failed(_)) {
                self.chats.remove(&session);
            }
            model.update(|screen| *screen = screen.clone().heard(session, event));
            if ended {
                self.refresh(model, session);
            }
        }
        while let Ok(job) = self.jobs.try_recv() {
            self.pending.set(self.pending.get().saturating_sub(1));
            self.finish(model, job);
        }
    }

    /// Whether a thread has yet to report, so the screen should keep looking for what it finds.
    pub fn busy(&self) -> bool {
        self.pending.get() > 0
    }

    /// Runs `work` on a thread of its own, which reports what it found through the sender it is given.
    fn spawn(&self, work: impl FnOnce(Sender<Job>) + Send + 'static) {
        self.pending.set(self.pending.get() + 1);
        let reporter = self.reporter.clone();
        std::thread::spawn(move || work(reporter));
    }

    /// Decodes the pictures among the files at `paths` away from the window, and reports each.
    pub fn preview(&self, paths: Vec<String>) {
        for path in paths {
            self.spawn(move |reporter| {
                let picture = decode_preview(&path);
                let _ = reporter.send(Job::Previewed(path, picture));
            });
        }
    }

    /// Takes in one report of a thread.
    fn finish(&mut self, model: &State<Screen>, job: Job) {
        match job {
            Job::Previewed(path, picture) => {
                if let Some(picture) = picture {
                    model.update(|screen| *screen = screen.clone().with_preview(&path, picture));
                }
            }
            Job::SignedIn(agent, result) => {
                model.update(|screen| *screen = screen.clone().signed_in(agent));
                if let Err(message) = result {
                    eprintln!("sign-in failed: {message}");
                } else if let Some(held) = AGENTS.get(agent).copied() {
                    self.spawn(move |reporter| {
                        let found = pm_acp::discover(held).unwrap_or_default();
                        let _ = reporter.send(Job::Discovered(agent, found));
                    });
                }
            }
            Job::Discovered(agent, found) => {
                if !found.settings.is_empty()
                    && let Some(held) = AGENTS.get(agent)
                {
                    let _ =
                        pm_store::save_text(&format!("discovery-{}", held.id), &found.to_text());
                }
                model.update(|screen| *screen = screen.clone().with_discovery(agent, found));
            }
            Job::Worktree {
                session,
                result,
                text,
                files,
                settings,
            } => match result {
                Ok((path, branch)) => {
                    let directory = path.to_string_lossy().into_owned();
                    model.update(|screen| {
                        *screen = screen.clone().worktree_ready(session, &directory, &branch);
                    });
                    self.begin(model, session, &settings);
                    if let Some(chat) = self.chats.get(&session) {
                        chat.prompt(&text, &files);
                    }
                }
                Err(reason) => model.update(|screen| {
                    *screen = screen.clone().heard(
                        session,
                        Event::Failed(format!("could not make a worktree: {reason}")),
                    );
                }),
            },
            Job::Refreshed {
                session,
                changes,
                commits,
            } => model.update(|screen| {
                *screen = screen.clone().refreshed(session, &changes, commits);
            }),
            Job::Committed(session) => self.refresh(model, session),
            Job::Failed { session, reason } => {
                model.update(|screen| *screen = screen.clone().noted(session, &reason));
                self.refresh(model, session);
            }
        }
    }

    /// Where the session at `session` works.
    fn session_of(model: &State<Screen>, session: usize) -> Option<Placement> {
        let screen = model.get();
        let Screen::Workspace(state) = &screen else {
            return None;
        };
        let held = state.sessions.get(session)?;
        let project = state.projects.get(held.project)?;
        Some(Placement {
            directory: PathBuf::from(&held.directory),
            agent: held.agent,
            base: held.base.clone(),
            repository: project.directory(),
            title: held.title.clone(),
        })
    }

    /// Starts the new session at `session`: in a worktree off `base` when one is asked for, otherwise in its folder.
    fn start(
        &mut self,
        model: &State<Screen>,
        session: usize,
        text: String,
        files: Vec<String>,
        settings: Vec<(String, String)>,
        base: Option<String>,
    ) {
        let Some(placement) = Self::session_of(model, session) else {
            return;
        };
        let (repository, title) = (placement.repository, placement.title);
        match base {
            Some(base) => {
                self.spawn(move |reporter| {
                    let result = pm_git::add_worktree(&repository, &title, &base)
                        .map(|made| (made.path, made.branch));
                    let _ = reporter.send(Job::Worktree {
                        session,
                        result,
                        text,
                        files,
                        settings,
                    });
                });
            }
            None => {
                self.begin(model, session, &settings);
                if let Some(chat) = self.chats.get(&session) {
                    chat.prompt(&text, &files);
                }
            }
        }
    }

    /// Starts the agent of the session at `session` in its folder, with the settings the reader chose.
    fn begin(&mut self, model: &State<Screen>, session: usize, settings: &[(String, String)]) {
        let Some(placement) = Self::session_of(model, session) else {
            return;
        };
        let Some(agent) = AGENTS.get(placement.agent).copied() else {
            return;
        };
        let chat = Chat::start(agent, placement.directory);
        for (setting, value) in settings {
            chat.set(setting, value);
        }
        self.chats.insert(session, chat);
    }

    /// Looks at the folder of the session at `session` again, for its changes and commits.
    fn refresh(&self, model: &State<Screen>, session: usize) {
        let Some(placement) = Self::session_of(model, session) else {
            return;
        };
        self.spawn(move |reporter| {
            let _ = reporter.send(Self::look(
                session,
                &placement.directory,
                placement.base.as_deref(),
            ));
        });
    }

    /// What the folder at `directory` holds that differs from the last commit.
    fn look(session: usize, directory: &Path, base: Option<&str>) -> Job {
        if !pm_git::is_repository(directory) {
            return Job::Refreshed {
                session,
                changes: Vec::new(),
                commits: None,
            };
        }
        Job::Refreshed {
            session,
            changes: pm_git::changes(directory),
            commits: base.map(|base| pm_git::commits_since(directory, base)),
        }
    }

    /// Stages or unstages `paths` in the folder of the session at `session`.
    fn stage(&self, model: &State<Screen>, session: usize, paths: Vec<String>, stage: bool) {
        let Some(placement) = Self::session_of(model, session) else {
            return;
        };
        let (directory, base) = (placement.directory, placement.base);
        self.spawn(move |reporter| {
            let done = if stage {
                pm_git::stage(&directory, &paths)
            } else {
                pm_git::unstage(&directory, &paths)
            };
            let _ = reporter.send(match done {
                Ok(()) => Self::look(session, &directory, base.as_deref()),
                Err(reason) => Job::Failed {
                    session,
                    reason: format!("Could not stage the files: {reason}"),
                },
            });
        });
    }

    /// Commits what is staged in the folder of the session at `session`.
    fn commit(&self, model: &State<Screen>, session: usize, message: String) {
        let Some(placement) = Self::session_of(model, session) else {
            return;
        };
        let directory = placement.directory;
        self.spawn(move |reporter| {
            let _ = reporter.send(match pm_git::commit(&directory, &message) {
                Ok(()) => Job::Committed(session),
                Err(reason) => Job::Failed {
                    session,
                    reason: format!("The commit failed: {reason}"),
                },
            });
        });
    }
}

/// Points every agent at the folder of the login in use for it, or at its own when none is.
pub fn apply_accounts(logins: &[(String, String, String)], in_use: &[(String, String)]) {
    for agent in AGENTS {
        let home = in_use
            .iter()
            .find(|(owner, _)| owner == agent.id)
            .filter(|(owner, id)| logins.iter().any(|(held, name, _)| held == owner && name == id))
            .and_then(|(owner, id)| pm_store::account_directory(owner, id).ok());
        agent.use_home(home);
    }
}

/// The longest side of a decoded preview, in pixels.
const PREVIEW_SIDE: u32 = 512;

/// The picture in the file at `path`, shrunk to a preview, or nothing when the file is not a picture.
fn decode_preview(path: &str) -> Option<Picture> {
    use std::hash::{Hash, Hasher};
    let modified = std::fs::metadata(path).and_then(|data| data.modified()).ok();
    let decoded = image::open(path).ok()?.thumbnail(PREVIEW_SIDE, PREVIEW_SIDE);
    let rgba = decoded.to_rgba8();
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    (path, modified).hash(&mut hasher);
    Some(Picture {
        key: hasher.finish(),
        width: rgba.width(),
        height: rgba.height(),
        pixels: std::sync::Arc::new(rgba.into_raw()),
    })
}
