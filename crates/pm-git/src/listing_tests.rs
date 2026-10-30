//! Tests that listing a repository's files returns every name as it is on disk.

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

/// Names with non-ASCII characters, spaces, quotes and newlines come back verbatim, tracked, untracked and ignored alike.
#[test]
fn unusual_names_are_listed_verbatim() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../build")
        .join(format!("listing-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(root.join("samples/invoices")).unwrap();
    fs::create_dir_all(root.join("tårta/ignorerad")).unwrap();
    git(&root, &["init", "-q"]);
    fs::write(root.join(".gitignore"), "ignorerad/\n*.tmp\n").unwrap();
    fs::write(root.join("samples/invoices/Jämtkraft.pdf"), "").unwrap();
    fs::write(root.join("samples/invoices/Ellevio.pdf"), "").unwrap();
    fs::write(root.join("with space \"quoted\".txt"), "").unwrap();
    fs::write(root.join("line\nbreak.txt"), "").unwrap();
    fs::write(root.join("Ignorerad ö.tmp"), "").unwrap();
    git(
        &root,
        &["add", ".gitignore", "samples/invoices/Ellevio.pdf"],
    );
    git(&root, &["commit", "-q", "-m", "first"]);
    git(&root, &["add", "-f", "samples/invoices/Jämtkraft.pdf"]);
    git(&root, &["commit", "-q", "-m", "tracked"]);
    fs::write(root.join("samples/invoices/Fortum Ä.pdf"), "").unwrap();
    fs::write(root.join("tårta/ignorerad/x"), "").unwrap();
    fs::create_dir_all(root.join("tömt/ö")).unwrap();

    let found = files(&root, 100);
    for name in [
        "samples/invoices/Jämtkraft.pdf",
        "samples/invoices/Fortum Ä.pdf",
        "with space \"quoted\".txt",
        "line\nbreak.txt",
    ] {
        assert!(
            found.contains(&name.to_owned()),
            "{name} missing: {found:?}"
        );
    }
    assert_eq!(
        ignored(&root, 100),
        vec!["Ignorerad ö.tmp", "tårta/ignorerad/x"]
    );
    assert!(ignored_folders(&root, 100).contains(&"tårta/ignorerad".to_owned()));
    assert!(untracked_folders(&root, 100).contains(&"tömt".to_owned()));
    let _ = fs::remove_dir_all(&root);
}

/// A folder that holds repositories lists its own loose files and plain folders next to the files of each repository.
#[test]
fn folder_of_repositories_lists_loose_files() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../build")
        .join(format!("loose-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    for name in ["pyre", "rustle"] {
        fs::create_dir_all(root.join(name)).unwrap();
        git(&root.join(name), &["init", "-q"]);
        fs::write(root.join(name).join("lib.rs"), "").unwrap();
    }
    fs::create_dir_all(root.join("notes")).unwrap();
    fs::write(root.join("AGENTS.md"), "").unwrap();
    fs::write(root.join("notes/todo.md"), "").unwrap();

    let found = files(&root, 100);
    for name in ["pyre/lib.rs", "rustle/lib.rs", "AGENTS.md", "notes/todo.md"] {
        assert!(
            found.contains(&name.to_owned()),
            "{name} missing: {found:?}"
        );
    }
    assert_eq!(found.len(), 4, "{found:?}");
    let _ = fs::remove_dir_all(&root);
}
