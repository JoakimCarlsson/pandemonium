//! Merge state and completion in ordinary and linked worktrees.

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use pm_core::{Operation, Status, abort_merge, commit};

/// A disposable repository and its linked worktree.
struct Fixture {
    /// Directory containing both worktrees.
    home: PathBuf,
    /// The main worktree.
    root: PathBuf,
    /// The linked worktree.
    linked: PathBuf,
}

impl Fixture {
    /// Creates divergent branches with a conflict ready to merge.
    fn new() -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let home =
            std::env::temp_dir().join(format!("pandemonium-merge-{}-{nonce}", std::process::id()));
        let root = home.join("repo");
        let linked = home.join("linked");
        std::fs::create_dir_all(&root).unwrap();
        git(&root, &["init", "-q", "-b", "main"]);
        git(&root, &["config", "user.name", "Merge Test"]);
        git(&root, &["config", "user.email", "merge@example.invalid"]);
        std::fs::write(root.join("a.txt"), "base\n").unwrap();
        git(&root, &["add", "a.txt"]);
        git(&root, &["commit", "-qm", "base"]);
        git(&root, &["checkout", "-qb", "other"]);
        std::fs::write(root.join("a.txt"), "theirs\n").unwrap();
        git(&root, &["commit", "-qam", "theirs"]);
        git(&root, &["checkout", "-q", "main"]);
        std::fs::write(root.join("a.txt"), "ours\n").unwrap();
        git(&root, &["commit", "-qam", "ours"]);
        git(
            &root,
            &[
                "worktree",
                "add",
                "-qb",
                "session",
                linked.to_str().unwrap(),
            ],
        );
        Self { home, root, linked }
    }
}

impl Drop for Fixture {
    /// Removes the disposable repository after the test.
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.home);
    }
}

/// Runs a successful git command in `root`.
fn git(root: &Path, args: &[&str]) -> String {
    let output = pm_host::Host::local()
        .command("git")
        .args(args)
        .current_dir(root)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

/// Starts the expected conflicting merge.
fn start_merge(root: &Path) {
    let output = pm_host::Host::local()
        .command("git")
        .args(["merge", "other"])
        .current_dir(root)
        .output()
        .unwrap();
    assert!(!output.status.success());
}

/// A linked worktree exposes its own merge state and can commit an unchanged resolution.
#[test]
fn linked_merge_commits_two_parents() {
    let fixture = Fixture::new();
    start_merge(&fixture.linked);
    assert!(Status::of(&fixture.root).head().operation.is_none());
    let status = Status::of(&fixture.linked);
    let Some(Operation::Merge(merge)) = &status.head().operation else {
        panic!("merge missing")
    };
    assert!(merge.message.starts_with("Merge branch 'other'"));
    assert!(!merge.message.lines().any(|line| line.starts_with('#')));
    assert!(
        status
            .changed()
            .iter()
            .any(|changed| changed.is_conflicted())
    );
    std::fs::write(fixture.linked.join("a.txt"), "ours\n").unwrap();
    git(&fixture.linked, &["add", "a.txt"]);
    assert!(Status::of(&fixture.linked).changed().is_empty());
    commit(&fixture.linked, &merge.message, false).unwrap();
    assert!(Status::of(&fixture.linked).head().operation.is_none());
    assert_eq!(
        git(
            &fixture.linked,
            &["rev-list", "--parents", "-n", "1", "HEAD"]
        )
        .split_whitespace()
        .count(),
        3
    );
}

/// Aborting a linked merge restores the worktree and leaves its peer alone.
#[test]
fn linked_merge_aborts() {
    let fixture = Fixture::new();
    start_merge(&fixture.linked);
    abort_merge(&fixture.linked).unwrap();
    assert!(Status::of(&fixture.linked).head().operation.is_none());
    assert!(Status::of(&fixture.root).head().operation.is_none());
    assert_eq!(
        std::fs::read_to_string(fixture.linked.join("a.txt")).unwrap(),
        "ours\n"
    );
    assert!(git(&fixture.linked, &["status", "--porcelain"]).is_empty());
}
