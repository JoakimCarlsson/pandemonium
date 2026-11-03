//! Tests of closing sessions.

use std::collections::BTreeMap;
use std::path::Path;
use std::process::Command;
use std::time::Duration;

use super::*;

/// Runs `git` with `arguments` in `folder` and panics when it fails.
fn git(folder: &Path, arguments: &[&str]) {
    let status = Command::new("git")
        .args(["-c", "user.name=test", "-c", "user.email=test@example.com"])
        .args(arguments)
        .current_dir(folder)
        .status()
        .expect("git");
    assert!(status.success(), "git {arguments:?} failed");
}

/// A repository with one commit and a worktree beside it, both under a fresh folder of `build/` named `name`; returns the two folders.
fn repository_with_worktree(name: &str) -> (PathBuf, PathBuf) {
    let base = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../build")
        .join(name);
    let _ = std::fs::remove_dir_all(&base);
    let repository = base.join("repository");
    std::fs::create_dir_all(&repository).expect("folder");
    let repository = repository.canonicalize().expect("folder");
    git(&repository, &["init", "-q"]);
    std::fs::write(repository.join("file"), "one").expect("file");
    git(&repository, &["add", "."]);
    git(&repository, &["commit", "-q", "-m", "first"]);
    let worktree = repository.parent().expect("parent").join("worktree");
    let path = worktree.to_string_lossy().into_owned();
    git(&repository, &["worktree", "add", "-q", "-b", "work", &path]);
    (repository, worktree)
}

/// A session of the first agent that works in `worktree`, a worktree of `repository`.
fn session_in(repository: &Path, worktree: &Path) -> Session {
    let meta = Meta {
        agent: pm_harness::AGENTS[0].id.to_owned(),
        repository: repository.to_string_lossy().into_owned(),
        directory: worktree.to_string_lossy().into_owned(),
        branch: Some("work".to_owned()),
        head: Some("abc".to_owned()),
        base: None,
        title: "test".to_owned(),
        closed: false,
        updated: 0,
        login: None,
        submodule: None,
        container: false,
    };
    Session::new(pm_harness::AGENTS[0], worktree.to_path_buf(), Vec::new(), meta)
}

/// Opens a shell in `folder` that the server knows by the same name.
fn shell_in(core: &Arc<Core>, folder: &Path) -> u64 {
    core.open_shell(
        folder,
        &folder.to_string_lossy(),
        80,
        24,
        &BTreeMap::new(),
        None,
    )
    .expect("shell")
}

/// Whether the server still holds the shell numbered `number`.
fn alive(core: &Core, number: u64) -> bool {
    core.terminal_infos()
        .iter()
        .any(|info| info.terminal == number)
}

/// Waits briefly for the shell numbered `number` to be gone and returns whether it is.
fn gone(core: &Core, number: u64) -> bool {
    (0..100).any(|_| {
        std::thread::sleep(Duration::from_millis(50));
        !alive(core, number)
    })
}

/// Removes what a test left of the session numbered `number` in the app's data folder.
fn forget(number: u64) {
    if let Some(root) = pm_store::data_directory() {
        let _ = std::fs::remove_dir_all(root.join("sessions").join(number.to_string()));
    }
}

/// Closing a session ends the shells in its worktree and no others, and leaves them all running when the close is refused or another open session shares the worktree.
#[test]
fn closing_a_session_ends_only_the_shells_in_its_worktree() {
    let (repository, worktree) = repository_with_worktree("close-session-test");
    let (first, second) = (4_000_000_001, 4_000_000_002);
    let core = Core::new();
    {
        let mut sessions = core.sessions.lock().expect("sessions");
        sessions.insert(first, session_in(&repository, &worktree));
        sessions.insert(second, session_in(&repository, &worktree));
    }
    let inside = shell_in(&core, &worktree);
    let project = shell_in(&core, &repository);

    assert!(matches!(close_session(&core, first, false), Response::Done));
    assert!(alive(&core, inside), "a shared worktree keeps its shells");

    std::fs::write(worktree.join("file"), "changed").expect("file");
    assert!(matches!(
        close_session(&core, second, false),
        Response::Uncommitted
    ));
    assert!(alive(&core, inside), "a refused close ends nothing");

    assert!(matches!(close_session(&core, second, true), Response::Done));
    assert!(gone(&core, inside), "the worktree's shell is ended");
    assert!(alive(&core, project), "the project's shell keeps running");

    core.close_terminal(project);
    forget(first);
    forget(second);
    let _ = std::fs::remove_dir_all(worktree.parent().expect("parent"));
}
