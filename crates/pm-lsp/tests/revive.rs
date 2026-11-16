//! A language server that is killed, as the out-of-memory killer kills one past its memory cap, is started again once with its open files, and left stopped when it is killed again soon after.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// The processes started in the folder `dir`, which a project's servers are started in, by id.
fn servers_in(dir: &Path) -> Vec<u32> {
    std::fs::read_dir("/proc")
        .map(|entries| {
            entries
                .flatten()
                .filter(|entry| {
                    std::fs::read_link(entry.path().join("cwd")).is_ok_and(|cwd| cwd == dir)
                })
                .filter_map(|entry| entry.file_name().to_str()?.parse().ok())
                .collect()
        })
        .unwrap_or_default()
}

/// The first value `found` gives within twenty seconds.
fn eventually<T>(found: impl Fn() -> Option<T>) -> Option<T> {
    let deadline = Instant::now() + Duration::from_secs(20);
    while Instant::now() < deadline {
        if let Some(value) = found() {
            return Some(value);
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    None
}

/// Kills the process `pid` outright.
fn kill(pid: u32) {
    std::process::Command::new("kill")
        .args(["-KILL", &pid.to_string()])
        .status()
        .unwrap();
}

/// Kills a real Markdown server twice: it comes back the first time, with the file still open on it, and stays stopped the second; skipped when Markdown is not installed in the app.
#[test]
fn a_killed_server_is_started_again_once() {
    if pm_lsp::installed_manifest("markdown").is_none() {
        eprintln!("skipped: Markdown is not installed in the app");
        return;
    }
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../build/tests/pm-lsp/revive");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("a.md"), "# Title\n").unwrap();
    let dir = dir.canonicalize().unwrap();
    let hub = pm_lsp::Hub::new(
        Arc::new(|_| {}),
        Arc::new(|_: &Path, _: &str, _: &pm_lsp::ServerSpec| std::collections::BTreeMap::new()),
    );
    hub.open(&dir, "a.md", "markdown", 1, "# Title\n").unwrap();
    let running = || {
        eventually(|| {
            hub.server_states(&dir)
                .iter()
                .any(|state| state.state == "running" || state.state == "loading")
                .then_some(())
        })
    };
    running().expect("the server did not come up");
    let first = eventually(|| servers_in(&dir).first().copied()).expect("no server started");
    kill(first);
    let second = eventually(|| servers_in(&dir).into_iter().find(|pid| *pid != first))
        .expect("the killed server was not started again");
    assert!(hub.is_open(&dir, "a.md"));
    running().expect("the server started again did not come up");
    kill(second);
    std::thread::sleep(Duration::from_secs(6));
    assert_eq!(
        servers_in(&dir),
        Vec::<u32>::new(),
        "it was started again a second time"
    );
    let states = hub.server_states(&dir);
    let stopped = states
        .iter()
        .find(|state| state.state == "stopped")
        .unwrap_or_else(|| panic!("not shown as stopped: {states:?}"));
    assert!(
        stopped
            .detail
            .as_deref()
            .is_some_and(|detail| detail.contains("killed")),
        "{stopped:?}"
    );
}
