//! Checks of what every code host shares for pull requests: drafting, finding the template, naming the base and deleting the remote branch, and how a repository on an unsupported host is treated.

use std::fs;
use std::path::Path;
use std::process::Command;

use crate::pull_requests::{
    CodeHost, UNSUPPORTED, base_name, code_host, compose, delete_arguments, pull_request_action,
    pull_request_status, template,
};
use crate::review_host::TemplatePlaces;

/// GitHub's template places, as a host would give them.
const PLACES: TemplatePlaces = TemplatePlaces {
    file: "pull_request_template.md",
    folders: &[".github", "", "docs"],
    collection: ".github/PULL_REQUEST_TEMPLATE",
};

/// Deleting the remote branch names its full reference and leases it to the tip that was seen.
#[test]
fn delete_names_the_full_reference() {
    assert_eq!(
        delete_arguments("origin", "task/titles", Some("abc")),
        [
            "push",
            "--force-with-lease=refs/heads/task/titles:abc",
            "origin",
            "--delete",
            "refs/heads/task/titles"
        ]
    );
    assert_eq!(
        delete_arguments("origin", "x", None),
        ["push", "origin", "--delete", "refs/heads/x"]
    );
}

/// A base is named without its remote.
#[test]
fn base_without_remote() {
    let remotes = vec!["origin".to_owned(), "upstream".to_owned()];
    assert_eq!(base_name("origin/main", &remotes), "main");
    assert_eq!(base_name("release/1", &remotes), "release/1");
}

/// The draft takes its title from what was given or the first commit, and its body from the plan, the commits and the template.
#[test]
fn drafts_from_plan_commits_and_template() {
    let plan = vec!["Read the prompt".to_owned()];
    let commits = vec!["Name sessions".to_owned(), "Trim titles".to_owned()];
    let draft = compose("", &plan, &commits, Some("## Checklist\n- [ ] tests\n"));
    assert_eq!(draft.title, "Name sessions");
    assert_eq!(
        draft.body,
        "## Plan\n\n- Read the prompt\n\n## Commits\n\n- Name sessions\n- Trim titles\n\n## Checklist\n- [ ] tests"
    );
    assert_eq!(compose("  Mine ", &[], &[], None).title, "Mine");
    assert_eq!(compose("", &[], &[], None).title, "Changes");
}

/// The template is found in `.github` whatever its case, and in the folder of templates when there is no single one.
#[test]
fn finds_the_template() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../build")
        .join(format!("pull-template-{}", std::process::id()));
    fs::create_dir_all(root.join(".github/PULL_REQUEST_TEMPLATE")).unwrap();
    assert_eq!(template(&root, &PLACES), None);
    fs::write(root.join(".github/PULL_REQUEST_TEMPLATE/b.md"), "second").unwrap();
    fs::write(root.join(".github/PULL_REQUEST_TEMPLATE/a.md"), "first").unwrap();
    assert_eq!(template(&root, &PLACES).as_deref(), Some("first"));
    fs::write(root.join(".github/Pull_Request_Template.md"), "single").unwrap();
    assert_eq!(template(&root, &PLACES).as_deref(), Some("single"));
    fs::remove_dir_all(root).unwrap();
}

/// A template is not looked for in a folder of templates when the host has none.
#[test]
fn no_collection_no_template() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../build")
        .join(format!("pull-template-none-{}", std::process::id()));
    fs::create_dir_all(root.join(".gitlab/merge_request_templates")).unwrap();
    fs::write(root.join(".gitlab/merge_request_templates/a.md"), "first").unwrap();
    let places = TemplatePlaces {
        file: "merge_request_template.md",
        folders: &[""],
        collection: "",
    };
    assert_eq!(template(&root, &places), None);
    let places = TemplatePlaces {
        collection: ".gitlab/merge_request_templates",
        ..places
    };
    assert_eq!(template(&root, &places).as_deref(), Some("first"));
    fs::remove_dir_all(root).unwrap();
}

/// Runs `git` with `arguments` in `directory`, failing the test when it fails.
fn git(directory: &Path, arguments: &[&str]) {
    let status = Command::new("git")
        .current_dir(directory)
        .args(arguments)
        .output()
        .unwrap();
    assert!(status.status.success(), "{status:?}");
}

/// A repository on an unsupported host is said to be so, named after its machine, has no pull request, and refuses every action but pushing without running anything.
#[test]
fn unsupported_host_offers_only_push() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../build")
        .join(format!("pull-unsupported-{}", std::process::id()));
    fs::create_dir_all(&root).unwrap();
    git(&root, &["init", "-q", "-b", "task"]);
    git(
        &root,
        &["remote", "add", "origin", "git@gitlab.com:o/r.git"],
    );
    let host = code_host(&root);
    assert_eq!(host, CodeHost::unsupported("gitlab.com"));
    assert!(!host.supported);
    assert_eq!(pull_request_status(&root, "task"), None);
    for action in [
        "create",
        "create_draft",
        "update",
        "ready",
        "squash",
        "reopen",
        "delete_branch",
    ] {
        assert_eq!(
            pull_request_action(&root, "task", Some("main"), action, "", &[]).unwrap_err(),
            UNSUPPORTED,
            "{action}"
        );
    }
    fs::remove_dir_all(root).unwrap();
}

/// A repository with no remote is on no host at all.
#[test]
fn no_remote_is_unsupported() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../build")
        .join(format!("pull-no-remote-{}", std::process::id()));
    fs::create_dir_all(&root).unwrap();
    git(&root, &["init", "-q"]);
    assert_eq!(code_host(&root), CodeHost::unsupported(""));
    fs::remove_dir_all(root).unwrap();
}
