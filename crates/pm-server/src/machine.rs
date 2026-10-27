//! What the reader set up for the work the server does on its machine: kept on disk, read where it is acted on, and the fetching it has the server do by itself.

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use pm_protocol::{MachineSettings, Response};

use crate::core::Core;

/// The file the settings are kept in.
const FILE: &str = "machine-settings";

/// How often the fetcher wakes to see whether fetching is due.
const FETCH_CHECK: Duration = Duration::from_secs(30);

/// The settings as they were last saved, or as they come when none were.
pub(crate) fn settings() -> MachineSettings {
    pm_store::load_text(FILE)
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

/// The answer to saving `settings`.
pub(crate) fn save(settings: &MachineSettings) -> Response {
    let text = match serde_json::to_string(settings) {
        Ok(text) => text,
        Err(error) => return crate::handler::failed(error.to_string()),
    };
    match pm_store::save_text(FILE, &text) {
        Ok(()) => Response::Done,
        Err(error) => crate::handler::failed(error.to_string()),
    }
}

/// Where new worktrees are made and the branch they are on, as the settings say, and, for a session that runs in a dev container, with the git-ignored dev container configuration the reader runs locally carried along.
pub(crate) fn placement_for(container: bool) -> pm_git::Placement {
    let mut placement = placement();
    if container {
        placement
            .include
            .extend([".devcontainer/", ".devcontainer.json"].map(str::to_owned));
    }
    placement
}

/// Where new worktrees are made and the branch they are on, as the settings say.
fn placement() -> pm_git::Placement {
    let git = settings().git;
    let root = git.worktree_root.trim();
    pm_git::Placement {
        root: (!root.is_empty()).then(|| crate::handler::place(root)),
        branch_template: Some(git.branch_template).filter(|template| !template.trim().is_empty()),
        include: git
            .worktree_include
            .split([',', '\n'])
            .map(str::trim)
            .filter(|path| !path.is_empty())
            .map(str::to_owned)
            .chain(
                (git.worktree_include == pm_protocol::GitPreferences::default().worktree_include)
                    .then(|| {
                        pm_tools::generated()
                            .into_iter()
                            .map(|name| format!("{name}/"))
                    })
                    .into_iter()
                    .flatten(),
            )
            .collect(),
    }
}

/// What a new session's agent is started with: `settings` as the reader changed them, with the mode the settings name when the reader did not pick one.
pub(crate) fn with_mode(settings: &[(String, String)]) -> Vec<(String, String)> {
    let mut held = settings.to_vec();
    if let Some(mode) = self::settings().agents.permission_mode
        && !held.iter().any(|(name, _)| name == "mode")
    {
        held.push(("mode".to_owned(), mode));
    }
    held
}

/// The repositories the server works for: those of the sessions it holds that are not closed, and the projects saved on its machine, each repository of a folder that holds several counted on its own.
fn repositories(core: &Core) -> BTreeSet<PathBuf> {
    let mut found: BTreeSet<PathBuf> = BTreeSet::new();
    if let Ok(sessions) = core.sessions.lock() {
        found.extend(
            sessions
                .values()
                .filter(|session| !session.meta.closed)
                .map(|session| crate::handler::place(&session.meta.repository)),
        );
    }
    found.extend(
        pm_store::load_projects()
            .unwrap_or_default()
            .iter()
            .filter(|path| pm_protocol::host_of(path).is_none())
            .map(|path| crate::handler::place(path)),
    );
    found
        .into_iter()
        .flat_map(|folder| match pm_git::layout(&folder) {
            pm_git::Layout::Repository => vec![folder],
            pm_git::Layout::Repositories(children) => {
                children.iter().map(|child| folder.join(child)).collect()
            }
            pm_git::Layout::Neither => Vec::new(),
        })
        .collect()
}

/// How often the states kept of saved files are swept for those that are too old or too many.
const SWEEP_EVERY: Duration = Duration::from_secs(10 * 60);

/// The folders the server works in: the folders of its open sessions and the repositories of the projects saved on its machine.
fn work_folders(core: &Core) -> BTreeSet<PathBuf> {
    let mut found = repositories(core);
    if let Ok(sessions) = core.sessions.lock() {
        found.extend(
            sessions
                .values()
                .filter(|session| !session.meta.closed)
                .map(|session| session.directory.clone()),
        );
    }
    found
}

/// Starts the thread that removes the states kept of saved files once they are older or take up more than the settings allow, as the server starts and then every few minutes.
pub(crate) fn start_sweeping(core: Arc<Core>) {
    std::thread::spawn(move || {
        loop {
            let history = settings().history;
            for folder in work_folders(&core) {
                let _ = crate::local_history::sweep(&folder, &history);
            }
            std::thread::sleep(SWEEP_EVERY);
        }
    });
}

/// Starts the thread that fetches every repository the server works for as often as the settings say, while they say to.
pub(crate) fn start_fetching(core: Arc<Core>) {
    std::thread::spawn(move || {
        let mut last = Instant::now();
        loop {
            std::thread::sleep(FETCH_CHECK);
            let git = settings().git;
            let every = Duration::from_secs(u64::from(git.fetch_minutes.max(1)) * 60);
            if !git.auto_fetch || last.elapsed() < every {
                continue;
            }
            last = Instant::now();
            for repository in repositories(&core) {
                let _ = pm_git::remote(&repository, "fetch");
            }
        }
    });
}
