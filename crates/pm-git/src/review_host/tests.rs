//! Checks of how a remote's address is taken apart and which code host it is found to be on.

use super::{MergeStrategy, RemoteAddress, detect};
use crate::pull_requests::{CodeHost, code_host_of};

/// Remotes on github.com, over https and ssh, and on GitHub Enterprise servers are GitHub's.
#[test]
fn detects_github() {
    for url in [
        "https://github.com/o/r.git",
        "http://github.com/o/r",
        "https://token@github.com/o/r.git",
        "git@github.com:o/r.git",
        "ssh://git@github.com/o/r.git",
        "ssh://git@github.com:22/o/r",
        "https://github.example.com/team/app.git",
        "git@github.corp.internal:team/app.git",
        "https://octo.ghe.com/team/app",
    ] {
        assert_eq!(detect(url).map(|host| host.kind()), Some("github"), "{url}");
    }
}

/// Remotes on GitLab, Azure DevOps, Bitbucket, a plain server or a local path are on no supported host.
#[test]
fn others_are_unsupported() {
    for url in [
        "https://gitlab.com/o/r.git",
        "git@gitlab.com:o/r.git",
        "https://gitlab.example.com/group/sub/r.git",
        "https://dev.azure.com/org/project/_git/repo",
        "git@ssh.dev.azure.com:v3/org/project/repo",
        "https://bitbucket.org/o/r.git",
        "ssh://git@git.example.com/r.git",
        "/srv/git/r.git",
        "file:///srv/git/r.git",
        "",
    ] {
        assert!(detect(url).is_none(), "{url}");
    }
}

/// What a client is told names the supported host and its noun, and names an unsupported one after its machine.
#[test]
fn code_host_for_clients() {
    let github = code_host_of(Some("git@github.com:o/r.git"));
    assert!(github.supported);
    assert_eq!(github.kind, "github");
    assert_eq!(github.name, "GitHub");
    assert_eq!(github.noun(), "pull request");
    assert_eq!(
        code_host_of(Some("https://dev.azure.com/org/project/_git/repo")),
        CodeHost::unsupported("dev.azure.com")
    );
    assert_eq!(code_host_of(None), CodeHost::unsupported(""));
}

/// Addresses are taken apart into their machine, path and web start, whatever the form.
#[test]
fn parses_addresses() {
    let https = RemoteAddress::parse("https://user@Git.Example.com:8443/a/b.git/").unwrap();
    assert_eq!(https.host, "git.example.com");
    assert_eq!(https.path, "a/b");
    assert_eq!(https.web, "https://git.example.com:8443");
    let scp = RemoteAddress::parse("git@example.com:a/b.git").unwrap();
    assert_eq!(scp.host, "example.com");
    assert_eq!(scp.web, "https://example.com");
    assert_eq!(RemoteAddress::parse("C:\\repos\\r"), None);
}

/// Merge strategies are named as the actions name them.
#[test]
fn merge_strategies() {
    assert_eq!(MergeStrategy::parse("squash"), Ok(MergeStrategy::Squash));
    assert_eq!(MergeStrategy::parse("rebase"), Ok(MergeStrategy::Rebase));
    assert_eq!(MergeStrategy::parse("merge"), Ok(MergeStrategy::Merge));
    assert!(MergeStrategy::parse("octopus").is_err());
}
