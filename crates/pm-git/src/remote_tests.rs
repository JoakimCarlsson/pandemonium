//! Tests that pushing over a folder of repositories pushes only the repositories with something to push.

use super::*;

/// Runs `git` with `arguments` in `directory` as a test author on `main`, expects it to succeed and returns what it wrote.
fn git(directory: &Path, arguments: &[&str]) -> String {
    let output = Command::new("git")
        .args([
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@t",
            "-c",
            "init.defaultBranch=main",
        ])
        .args(arguments)
        .current_dir(directory)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "git {arguments:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).trim().to_owned()
}

/// Makes the bare repository `<name>.git` in `root` and the clone `folder/<name>` of it, with one commit on `main` pushed and followed.
fn cloned(root: &Path, name: &str) -> PathBuf {
    let bare = format!("{name}.git");
    git(root, &["init", "-q", "--bare", &bare]);
    let clone = root.join("folder").join(name);
    git(root, &["clone", "-q", &bare, clone.to_str().unwrap()]);
    commit(&clone, "first");
    git(&clone, &["push", "-q", "-u", "origin", "main"]);
    clone
}

/// Commits an empty commit with `message` in the repository `directory`.
fn commit(directory: &Path, message: &str) {
    git(directory, &["commit", "-q", "--allow-empty", "-m", message]);
}

/// Makes every push from the clone `clone` fail, even one with nothing to send, while fetching still works, as a repository the user may only read does: its pushes go to a remote that is not there.
fn refuse_pushes(clone: &Path) {
    let denied = clone.join("../../denied.git");
    git(
        clone,
        &[
            "remote",
            "set-url",
            "--push",
            "origin",
            denied.to_str().unwrap(),
        ],
    );
}

/// Push and sync over a folder holding one repository with commits to push and one level with a remote that refuses pushes succeed and push the first; a branch that follows nothing is still published, and a repository that is ahead and refused is named in the failure.
#[test]
fn folder_of_repositories_pushes_only_what_is_ahead() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../build")
        .join(format!("remote-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(root.join("folder")).unwrap();
    let owned = cloned(&root, "owned");
    let reference = cloned(&root, "reference");
    refuse_pushes(&reference);
    let folder = root.join("folder");
    let owned_remote = root.join("owned.git");

    commit(&owned, "pushed");
    assert_eq!(remote(&folder, "push"), Ok(()));
    assert_eq!(
        git(&owned_remote, &["rev-parse", "main"]),
        git(&owned, &["rev-parse", "HEAD"])
    );

    commit(&owned, "synced");
    assert_eq!(remote(&folder, "sync"), Ok(()));
    assert_eq!(
        git(&owned_remote, &["rev-parse", "main"]),
        git(&owned, &["rev-parse", "HEAD"])
    );

    commit(&owned, "forced");
    assert_eq!(remote(&folder, "force_push"), Ok(()));
    assert_eq!(
        git(&owned_remote, &["rev-parse", "main"]),
        git(&owned, &["rev-parse", "HEAD"])
    );

    assert_eq!(remote(&folder, "fetch"), Ok(()));
    assert_eq!(remote(&folder, "pull"), Ok(()));

    git(&owned, &["switch", "-q", "-c", "feature"]);
    assert_eq!(remote(&folder, "push"), Ok(()));
    assert_eq!(
        git(&owned_remote, &["rev-parse", "feature"]),
        git(&owned, &["rev-parse", "HEAD"])
    );

    commit(&reference, "refused");
    let refused = remote(&folder, "push").unwrap_err();
    assert!(refused.starts_with("reference: "), "{refused}");
    assert!(!refused.contains("owned: "), "{refused}");
    let _ = fs::remove_dir_all(&root);
}
