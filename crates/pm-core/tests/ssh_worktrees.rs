//! Worktree lifecycle checks against an explicitly supplied disposable SSH repository.

use pm_host::{Hosts, Location};

/// Removes the fixture's worktree registration and directory after the check.
struct Worktree {
    /// The repository that owns the linked checkout.
    origin: Location,
    /// The exclusively reserved checkout path.
    path: std::path::PathBuf,
}

impl Drop for Worktree {
    /// Cleans up even when a lifecycle assertion fails.
    fn drop(&mut self) {
        let _ = pm_core::remove_worktree(&self.origin, &self.path);
    }
}

/// Ensures remote removal forgets Git's registration and permits immediate reuse.
#[test]
#[ignore = "requires PANDEMONIUM_TEST_SSH_HOST and PANDEMONIUM_TEST_SSH_REPOSITORY"]
fn remote_worktree_removal_allows_reuse() {
    let alias = std::env::var("PANDEMONIUM_TEST_SSH_HOST").unwrap();
    let repository = std::env::var("PANDEMONIUM_TEST_SSH_REPOSITORY").unwrap();
    let mut hosts = Hosts::default();
    let host = hosts.connect(&alias).unwrap();
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let fixture = Worktree {
        path: host.home().unwrap().join(format!(
            ".cache/pandemonium/ssh-worktree-test-{}-{nonce}",
            std::process::id()
        )),
        origin: Location::new(host, repository),
    };
    assert!(!fixture.path.exists(), "Use an isolated remote filesystem");
    for _ in 0..2 {
        pm_core::add_worktree(&fixture.origin, &fixture.path, "HEAD").unwrap();
        assert!(pm_core::worktrees(&fixture.origin).contains(&fixture.path));
        pm_core::remove_worktree(&fixture.origin, &fixture.path).unwrap();
        assert!(!fixture.origin.host.fs().exists(&fixture.path));
        assert!(!pm_core::worktrees(&fixture.origin).contains(&fixture.path));
    }
}
