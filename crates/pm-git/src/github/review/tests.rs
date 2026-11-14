//! Checks of the arguments given to `gh` for pull requests, of how what `gh` answers is read, and of which remotes are GitHub's, without a network or an account.

use super::{
    GitHub, compare_url, create_arguments, edit_arguments, github_slug, merge_arguments,
    parse_list, ready_arguments, reopen_arguments, status_arguments,
};
use crate::pull_requests::{CheckState, Draft, ReviewDecision};
use crate::review_host::{MergeStrategy, RemoteAddress, ReviewHost};

/// A draft with a short title and body.
fn sample_draft() -> Draft {
    Draft {
        title: "Add titles".to_owned(),
        body: "## Commits\n\n- Name sessions".to_owned(),
    }
}

/// The status of a branch is asked with its checks and review, and only the newest pull request.
#[test]
fn status_asks_for_checks_and_review() {
    let arguments = status_arguments("task/titles");
    assert_eq!(
        &arguments[..8],
        [
            "pr",
            "list",
            "--head",
            "task/titles",
            "--state",
            "all",
            "--limit",
            "1"
        ]
    );
    assert!(arguments[9].contains("statusCheckRollup"));
    assert!(arguments[9].contains("reviewDecision"));
}

/// Opening names the base, the head, the title and the body, and is a draft only when asked.
#[test]
fn create_arguments_name_base_and_head() {
    let draft = sample_draft();
    assert_eq!(
        create_arguments("task/titles", Some("main"), &draft, false),
        [
            "pr",
            "create",
            "--base",
            "main",
            "--head",
            "task/titles",
            "--title",
            "Add titles",
            "--body",
            "## Commits\n\n- Name sessions"
        ]
    );
    let drafted = create_arguments("task/titles", None, &draft, true);
    assert!(!drafted.contains(&"--base".to_owned()));
    assert_eq!(drafted.last().unwrap(), "--draft");
}

/// Updating, readying and reopening name the pull request by number, never by branch.
#[test]
fn actions_name_the_number() {
    assert_eq!(
        edit_arguments(7, "body"),
        ["pr", "edit", "7", "--body", "body"]
    );
    assert_eq!(ready_arguments(7), ["pr", "ready", "7"]);
    assert_eq!(reopen_arguments(7), ["pr", "reopen", "7"]);
}

/// Merging holds to the head that was seen, never deletes the branch through `gh`, and refuses unknown strategies.
#[test]
fn merge_holds_to_the_head() {
    assert_eq!(
        merge_arguments(7, MergeStrategy::Squash, "abc123"),
        [
            "pr",
            "merge",
            "7",
            "--squash",
            "--match-head-commit",
            "abc123"
        ]
    );
    let merged = merge_arguments(7, MergeStrategy::Merge, "");
    assert_eq!(merged, ["pr", "merge", "7", "--merge"]);
    assert!(!merged.iter().any(|argument| argument == "--delete-branch"));
    assert!(MergeStrategy::parse("octopus").is_err());
}

/// A pull request with check runs, commit statuses and a review decision is read whole.
#[test]
fn parses_checks_and_review() {
    let listed = r#"[{
        "number": 42, "title": "Add titles", "state": "OPEN",
        "url": "https://github.com/o/r/pull/42", "isDraft": true,
        "reviewDecision": "CHANGES_REQUESTED", "headRefOid": "abc", "baseRefName": "main",
        "statusCheckRollup": [
            {"__typename": "CheckRun", "name": "build", "status": "COMPLETED", "conclusion": "SUCCESS", "detailsUrl": "https://ci/1"},
            {"__typename": "CheckRun", "name": "test", "status": "IN_PROGRESS", "conclusion": ""},
            {"__typename": "CheckRun", "name": "lint", "status": "COMPLETED", "conclusion": "FAILURE"},
            {"__typename": "CheckRun", "name": "docs", "status": "COMPLETED", "conclusion": "SKIPPED"},
            {"__typename": "StatusContext", "context": "deploy", "state": "PENDING", "targetUrl": "https://ci/2"}
        ]
    }]"#;
    let request = parse_list(listed).unwrap().unwrap();
    assert_eq!(request.number, 42);
    assert!(request.draft && request.is_open());
    assert_eq!(request.review, ReviewDecision::ChangesRequested);
    assert_eq!(request.head, "abc");
    assert_eq!(request.checks_url, "https://github.com/o/r/pull/42/checks");
    let states: Vec<_> = request.checks.iter().map(|check| check.state).collect();
    assert_eq!(
        states,
        [
            CheckState::Passed,
            CheckState::Pending,
            CheckState::Failed,
            CheckState::Skipped,
            CheckState::Pending
        ]
    );
    assert_eq!(request.checks[4].name, "deploy");
    assert_eq!(request.checks[4].url, "https://ci/2");
    assert_eq!(request.count(CheckState::Pending), 2);
}

/// A branch without pull requests reads as none, one without checks as having none, and an unreadable answer fails.
#[test]
fn parses_empty_and_bare() {
    assert_eq!(parse_list("[]").unwrap(), None);
    let bare = parse_list(
        r#"[{"number":1,"state":"MERGED","url":"u","reviewDecision":"","statusCheckRollup":null}]"#,
    )
    .unwrap()
    .unwrap();
    assert!(bare.is_merged() && bare.checks.is_empty());
    assert_eq!(bare.review, ReviewDecision::None);
    assert!(parse_list("not json").is_err());
}

/// GitHub repositories are named from https and ssh addresses, and others are not.
#[test]
fn slugs_of_remotes() {
    for url in [
        "git@github.com:o/r.git",
        "ssh://git@github.com/o/r.git",
        "https://github.com/o/r",
        "https://github.com/o/r.git/",
        "https://token@github.com/o/r.git",
    ] {
        assert_eq!(github_slug(url).as_deref(), Some("o/r"), "{url}");
    }
    assert_eq!(github_slug("https://gitlab.com/o/r.git"), None);
    assert_eq!(github_slug("/srv/git/r.git"), None);
}

/// The compare page is filled in, encoded, and kept short however long the body is.
#[test]
fn compare_page_is_filled_and_short() {
    let url = compare_url(
        "https://github.com",
        "o/r",
        Some("main"),
        "task/titles",
        &sample_draft(),
    );
    assert!(url.starts_with(
        "https://github.com/o/r/compare/main...task%2Ftitles?quick_pull=1&title=Add%20titles&body=%23%23"
    ));
    let long = Draft {
        title: "t".to_owned(),
        body: "日本".repeat(2000),
    };
    let url = compare_url("https://github.com", "o/r", None, "b", &long);
    assert!(url.len() <= 4000);
    assert!(url.ends_with("%E6%9C%AC") || url.ends_with("%E6%97%A5"));
}

/// GitHub Enterprise servers are recognized, their compare page is on their own machine, and other services are not GitHub's.
#[test]
fn enterprise_and_others() {
    let enterprise = RemoteAddress::parse("git@github.example.com:team/app.git").unwrap();
    assert!(GitHub.recognizes(&enterprise));
    assert_eq!(
        github_slug("https://octo.ghe.com/team/app").as_deref(),
        Some("team/app")
    );
    let page = GitHub
        .new_review_url(&enterprise, None, "b", &sample_draft())
        .unwrap();
    assert!(page.starts_with("https://github.example.com/team/app/compare/b?"));
    for url in [
        "git@gitlab.com:o/r.git",
        "https://dev.azure.com/org/project/_git/repo",
        "git@ssh.dev.azure.com:v3/org/project/repo",
        "https://bitbucket.org/o/r.git",
    ] {
        assert_eq!(github_slug(url), None, "{url}");
    }
}
