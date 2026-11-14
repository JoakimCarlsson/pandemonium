//! The review of a branch, a pull request on GitHub or its like elsewhere, whatever service hosts it: how it stands, with its checks and its decision, and what can be done to it. What every service shares lives here: drafting a title and body from the session's plan, the branch's commits and the repository's template, checking the branch is safe to act on, and pushing it. What differs is asked of the [`ReviewHost`] the repository's remote is on; when none is, only pushing is offered.

use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::review_host::{MergeStrategy, RemoteAddress, ReviewHost, TemplatePlaces, detect};

/// The most bytes of a template that are read.
const TEMPLATE_BYTES: usize = 16 * 1024;

/// What the failure says when the repository's remote is on a service no [`ReviewHost`] supports.
pub const UNSUPPORTED: &str = "this code host is not supported yet";

/// How one check run on the head of a pull request stands.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum CheckState {
    /// It is queued or running.
    Pending,
    /// It succeeded.
    Passed,
    /// It failed, timed out, was cancelled or needs action.
    Failed,
    /// It was skipped or ended neutral.
    Skipped,
}

/// One check run, or commit status, on the head of a pull request.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Check {
    /// What it is called.
    pub name: String,
    /// How it stands.
    pub state: CheckState,
    /// The page that tells about it, empty when the host gave none.
    #[serde(default)]
    pub url: String,
}

/// What the reviewers decided about a pull request.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub enum ReviewDecision {
    /// Nothing: the repository asks for no review, or the host did not say.
    #[default]
    None,
    /// A review is needed before it can be merged.
    Required,
    /// It was approved.
    Approved,
    /// A reviewer asked for changes.
    ChangesRequested,
}

/// A pull request of a branch, with what its checks and reviewers say.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PullRequestStatus {
    /// Its number in the repository.
    pub number: u64,
    /// Its title.
    pub title: String,
    /// How it stands, whatever the host: `OPEN`, `CLOSED` or `MERGED`, which each host maps its own states to.
    pub state: String,
    /// Its web address.
    pub url: String,
    /// Whether it is a draft.
    pub draft: bool,
    /// What the reviewers decided.
    pub review: ReviewDecision,
    /// The checks on its head, in the order the host lists them.
    pub checks: Vec<Check>,
    /// The commit its head is at.
    pub head: String,
    /// The branch it is to be merged into.
    pub base: String,
    /// The page that lists its checks, empty when the host has none apart from its own page.
    #[serde(default)]
    pub checks_url: String,
}

impl PullRequestStatus {
    /// Whether it is open.
    pub fn is_open(&self) -> bool {
        self.state == "OPEN"
    }

    /// Whether it was merged.
    pub fn is_merged(&self) -> bool {
        self.state == "MERGED"
    }

    /// Whether it was closed without being merged.
    pub fn is_closed(&self) -> bool {
        self.state == "CLOSED"
    }

    /// How many of its checks are in `state`.
    pub fn count(&self, state: CheckState) -> usize {
        self.checks
            .iter()
            .filter(|check| check.state == state)
            .count()
    }
}

/// What came of acting on the pull request of a branch.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct Outcome {
    /// How the pull request stands afterwards, or nothing when the branch has none.
    pub request: Option<PullRequestStatus>,
    /// The host's web page for opening a new review, filled in with the drafted title and body, to open in a browser when the host's program could not open it itself.
    pub compare: Option<String>,
}

/// The service a repository's reviews are on, as a client is told of it so it can name it and offer only what it supports.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CodeHost {
    /// The service's short, stable name, such as `github`; empty when it is not supported.
    #[serde(default)]
    pub kind: String,
    /// The service's name as the reader knows it, such as `GitHub`, or the machine the remote is on when it is not supported; empty when the repository has no remote.
    #[serde(default)]
    pub name: String,
    /// What the service calls a review, in lower case, such as `pull request`.
    #[serde(default)]
    pub review_noun: String,
    /// Whether its reviews can be read and acted on; when not, only pushing is offered.
    #[serde(default)]
    pub supported: bool,
}

impl CodeHost {
    /// What a client is told of `host`.
    pub fn of(host: &dyn ReviewHost) -> Self {
        Self {
            kind: host.kind().to_owned(),
            name: host.name().to_owned(),
            review_noun: host.review_noun().to_owned(),
            supported: true,
        }
    }

    /// A service no [`ReviewHost`] supports, named `name`, such as the machine its remote is on.
    pub fn unsupported(name: &str) -> Self {
        Self {
            kind: String::new(),
            name: name.to_owned(),
            review_noun: "pull request".to_owned(),
            supported: false,
        }
    }

    /// What the service calls a review, or `pull request` when it did not say.
    pub fn noun(&self) -> &str {
        if self.review_noun.is_empty() {
            "pull request"
        } else {
            &self.review_noun
        }
    }
}

/// The title and body a pull request is opened or updated with.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Draft {
    /// The title.
    pub title: String,
    /// The body, in markdown.
    pub body: String,
}

/// The arguments of `git` that delete `branch` from `remote`, naming the full reference so a tag of the same name is never the one deleted, and, when the remote branch was last seen at `tip`, only while it is still there.
pub fn delete_arguments(remote: &str, branch: &str, tip: Option<&str>) -> Vec<String> {
    let reference = format!("refs/heads/{branch}");
    let mut arguments = vec!["push".to_owned()];
    if let Some(tip) = tip {
        arguments.push(format!("--force-with-lease={reference}:{tip}"));
    }
    arguments.extend([remote.to_owned(), "--delete".to_owned(), reference]);
    arguments
}

/// The name on the remote of the branch `base`, without the remote's name when it starts with one of `remotes`, such as `main` for `origin/main`.
pub fn base_name(base: &str, remotes: &[String]) -> String {
    remotes
        .iter()
        .find_map(|remote| base.strip_prefix(&format!("{remote}/")))
        .unwrap_or(base)
        .to_owned()
}

/// The title and body of a pull request: `title` when it says anything, or the first commit's subject; the body lists the `plan`'s steps and the `commits`' subjects and ends with the repository's `template`.
pub fn compose(title: &str, plan: &[String], commits: &[String], template: Option<&str>) -> Draft {
    let title = Some(title.trim())
        .filter(|title| !title.is_empty())
        .or_else(|| commits.first().map(|subject| subject.trim()))
        .unwrap_or("Changes")
        .to_owned();
    let mut sections = Vec::new();
    if !plan.is_empty() {
        let steps: Vec<String> = plan.iter().map(|step| format!("- {step}")).collect();
        sections.push(format!("## Plan\n\n{}", steps.join("\n")));
    }
    if !commits.is_empty() {
        let subjects: Vec<String> = commits
            .iter()
            .map(|subject| format!("- {subject}"))
            .collect();
        sections.push(format!("## Commits\n\n{}", subjects.join("\n")));
    }
    if let Some(template) = template.map(str::trim).filter(|text| !text.is_empty()) {
        sections.push(template.to_owned());
    }
    Draft {
        title,
        body: sections.join("\n\n"),
    }
}

/// The text of the repository's review template, found in `places`: the single template, in any case, in the first of its folders that has one, or else the first markdown file by name in its folder of templates; nothing when it has none.
pub fn template(directory: &Path, places: &TemplatePlaces) -> Option<String> {
    let named = places.folders.iter().find_map(|folder| {
        fs::read_dir(directory.join(folder))
            .ok()?
            .flatten()
            .map(|entry| entry.path())
            .find(|path| {
                path.is_file()
                    && path.file_name().is_some_and(|name| {
                        name.to_string_lossy().to_lowercase() == places.file.to_lowercase()
                    })
            })
    });
    let path = named.or_else(|| {
        if places.collection.is_empty() {
            return None;
        }
        let mut listed: Vec<_> = fs::read_dir(directory.join(places.collection))
            .ok()?
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| path.extension().is_some_and(|extension| extension == "md"))
            .collect();
        listed.sort();
        listed.into_iter().next()
    })?;
    let bytes = fs::read(path).ok()?;
    let mut text = String::from_utf8_lossy(&bytes[..bytes.len().min(TEMPLATE_BYTES)]).into_owned();
    text.retain(|character| character != '\u{FFFD}');
    Some(text)
}

/// The draft of a review of the branch `directory` has checked out: from `title`, the `plan`'s steps, the commits since `base` and the template found in `places`.
pub fn draft(
    directory: &Path,
    base: Option<&str>,
    title: &str,
    plan: &[String],
    places: &TemplatePlaces,
) -> Draft {
    let commits = base
        .map(|base| crate::commits_since(directory, base))
        .unwrap_or_default();
    compose(
        title,
        plan,
        &commits,
        template(directory, places).as_deref(),
    )
}

/// The remote a branch is published to: `origin`, or the first remote.
fn publishing_remote(directory: &Path) -> Option<String> {
    let remotes = crate::branching::remotes(directory);
    remotes
        .iter()
        .find(|remote| *remote == "origin")
        .or_else(|| remotes.first())
        .cloned()
}

/// The address of the remote a branch of `directory` is published to, or nothing when it has no remote.
fn remote_url(directory: &Path) -> Option<String> {
    let remote = publishing_remote(directory)?;
    crate::run(directory, &["remote", "get-url", &remote])
        .ok()
        .map(|url| url.trim().to_owned())
        .filter(|url| !url.is_empty())
}

/// The service the remote at `url` is on, as a client is told of it: the supported one that recognizes it, or an unsupported one named after the machine the remote is on.
pub fn code_host_of(url: Option<&str>) -> CodeHost {
    match url.and_then(detect) {
        Some(host) => CodeHost::of(host),
        None => CodeHost::unsupported(
            &url.and_then(RemoteAddress::parse)
                .map(|address| address.host)
                .unwrap_or_default(),
        ),
    }
}

/// The service the reviews of the repository `directory` is a clone of are on, from its publishing remote's address.
pub fn code_host(directory: &Path) -> CodeHost {
    code_host_of(remote_url(directory).as_deref())
}

/// The newest review of `branch`, open or not, with its checks and decision, on the service the repository `directory` is a clone of is hosted on; nothing when it has none, when the service is not supported, or when it cannot say.
pub fn pull_request_status(directory: &Path, branch: &str) -> Option<PullRequestStatus> {
    let url = remote_url(directory)?;
    detect(&url)?.status(directory, branch)
}

/// Acts on the review of `branch`, which `directory` has checked out and which was made from `base`, on the service its remote is on: `push` publishes the branch; `create` and `create_draft` push it and open a review, or update the open one, drafted from `title`, the `plan`'s steps, the commits and the template, and offer the service's page for a new review when its program cannot open one; `update` pushes and redrafts the body of the open one; `ready` marks a draft ready for review; `merge`, `squash` and `rebase` merge the open one, as long as its head is where it was seen; `reopen` reopens a closed one; and `delete_branch` deletes the branch from its remote once no open review needs it. On a service no [`ReviewHost`] supports, only `push` is done.
///
/// # Errors
///
/// Fails when the branch name could be mistaken for an option, when the branch is its own base, when the service is not supported and the action is not `push`, when the review is not in a state the action needs, or with what `git` or the service complained of.
pub fn pull_request_action(
    directory: &Path,
    branch: &str,
    base: Option<&str>,
    action: &str,
    title: &str,
    plan: &[String],
) -> Result<Outcome, String> {
    if branch.is_empty() || branch.starts_with('-') {
        return Err(format!(
            "{branch} is not a branch a pull request can be made of"
        ));
    }
    let remotes = crate::branching::remotes(directory);
    let base = base.map(|base| base_name(base, &remotes));
    if base.as_deref() == Some(branch) {
        return Err(format!(
            "{branch} is the branch this one was made from; make a branch of its own first"
        ));
    }
    let url = remote_url(directory);
    let Some(host) = url.as_deref().and_then(detect) else {
        return match action {
            "push" => {
                crate::remote(directory, "push")?;
                Ok(Outcome::default())
            }
            _ => Err(UNSUPPORTED.to_owned()),
        };
    };
    let noun = host.review_noun();
    let no_open = || format!("this branch has no open {noun}");
    let current = host.status(directory, branch);
    let open = current.as_ref().filter(|request| request.is_open());
    let mut compare = None;
    match action {
        "push" => crate::remote(directory, "push")?,
        "create" | "create_draft" | "update" => {
            crate::remote(directory, "push")?;
            let draft = draft(directory, base.as_deref(), title, plan, &host.templates());
            match (open, action) {
                (Some(request), _) => host.update_body(directory, request, &draft.body)?,
                (None, "update") => return Err(no_open()),
                (None, _) => {
                    if let Err(failure) = host.create(
                        directory,
                        branch,
                        base.as_deref(),
                        &draft,
                        action == "create_draft",
                    ) && host
                        .status(directory, branch)
                        .is_none_or(|request| !request.is_open())
                    {
                        let page = url
                            .as_deref()
                            .and_then(RemoteAddress::parse)
                            .and_then(|address| {
                                host.new_review_url(&address, base.as_deref(), branch, &draft)
                            })
                            .ok_or(failure)?;
                        compare = Some(page);
                    }
                }
            }
        }
        "ready" => host.mark_ready(directory, open.ok_or_else(no_open)?)?,
        "merge" | "squash" | "rebase" => {
            let request = open.ok_or_else(no_open)?;
            host.merge(directory, request, MergeStrategy::parse(action)?)?;
        }
        "reopen" => {
            let request = current
                .as_ref()
                .filter(|request| request.is_closed())
                .ok_or_else(|| format!("this branch has no closed {noun}"))?;
            host.reopen(directory, request)?;
        }
        "delete_branch" => {
            if open.is_some() {
                return Err(format!(
                    "an open {noun} still needs this branch; deleting it would close the {noun}"
                ));
            }
            let remote = publishing_remote(directory)
                .ok_or("this repository has no remote to delete the branch from")?;
            let tip = crate::resolve(directory, &format!("refs/remotes/{remote}/{branch}"));
            host.delete_remote_branch(directory, &remote, branch, tip.as_deref())?;
        }
        other => return Err(format!("{other} is not something to do to a {noun}")),
    }
    Ok(Outcome {
        request: host.status(directory, branch),
        compare,
    })
}
