//! Tests of naming a base reference for each repository of a folder of several.

use std::fs;
use std::path::Path;
use std::process::Command;

use super::*;

/// Runs `git` with `arguments` in `directory` and expects it to succeed.
fn git(directory: &Path, arguments: &[&str]) {
    let status = Command::new("git")
        .args(["-c", "user.name=t", "-c", "user.email=t@t"])
        .args(arguments)
        .current_dir(directory)
        .status()
        .unwrap();
    assert!(status.success());
}

/// A bare reference applies to every repository, a `name=reference` entry only to its own, and an unnamed repository without a bare reference gets the whole text.
#[test]
fn base_for_picks_the_entry_of_the_repository() {
    assert_eq!(base_for("a", "main"), "main");
    assert_eq!(base_for("a", "a=next,main"), "next");
    assert_eq!(base_for("b", "a=next,main"), "main");
    assert_eq!(base_for("b", "a=next"), "a=next");
}

/// A folder of two repositories on different branches gets one worktree each, made from the branch its entry names.
#[test]
fn worktree_of_several_repositories_uses_a_base_each() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../build")
        .join(format!("per-repo-base-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    for (name, branch) in [("one", "main"), ("two", "next")] {
        let folder = root.join(name);
        fs::create_dir_all(&folder).unwrap();
        git(&folder, &["init", "-q", "-b", branch]);
        git(&folder, &["commit", "-q", "--allow-empty", "-m", "start"]);
    }
    let made = add_worktree(
        &root,
        "per repo",
        "one=main,two=next",
        &Placement::default(),
    )
    .unwrap();
    assert!(made.path.join("one").exists() && made.path.join("two").exists());
    let refused = add_worktree(&root, "again", "main", &Placement::default());
    assert!(refused.unwrap_err().contains("two: main does not exist"));
    fs::remove_dir_all(&root).unwrap();
}
