//! GitHub as a [`ReviewHost`]: its pull requests read and acted on through the `gh` program, on github.com or a GitHub Enterprise server, and its compare page offered when `gh` cannot open one.

use std::path::Path;

use serde::Deserialize;

use crate::github::gh_in;
use crate::pull_requests::{Check, CheckState, Draft, PullRequestStatus, ReviewDecision};
use crate::review_host::{MergeStrategy, RemoteAddress, ReviewHost, TemplatePlaces, encode};

/// The fields of a pull request `gh` is asked for.
const FIELDS: &str =
    "number,title,state,url,isDraft,reviewDecision,statusCheckRollup,headRefOid,baseRefName";

/// The longest the address of a compare page may grow to, in characters; GitHub refuses requests much longer than this.
const COMPARE_LENGTH: usize = 4000;

/// GitHub, on github.com or a GitHub Enterprise server, reached through `gh`.
pub(crate) struct GitHub;

/// A pull request as `gh` lists it.
#[derive(Deserialize)]
struct Listed {
    /// Its number.
    number: u64,
    /// Its title.
    #[serde(default)]
    title: String,
    /// How it stands.
    state: String,
    /// Its web address.
    url: String,
    /// Whether it is a draft.
    #[serde(default, rename = "isDraft")]
    draft: bool,
    /// What the reviewers decided, empty or missing when nothing is asked of them.
    #[serde(default, rename = "reviewDecision")]
    review: Option<String>,
    /// The check runs and commit statuses on its head.
    #[serde(default, rename = "statusCheckRollup")]
    rollup: Option<Vec<Rolled>>,
    /// The commit its head is at.
    #[serde(default, rename = "headRefOid")]
    head: String,
    /// The branch it is to be merged into.
    #[serde(default, rename = "baseRefName")]
    base: String,
}

/// A check run or a commit status as `gh` lists it: a check run has a name, a status and a conclusion; a commit status has a context and a state.
#[derive(Deserialize)]
struct Rolled {
    /// The name of a check run.
    #[serde(default)]
    name: Option<String>,
    /// The context of a commit status.
    #[serde(default)]
    context: Option<String>,
    /// Where a check run is: `QUEUED`, `IN_PROGRESS` or `COMPLETED`.
    #[serde(default)]
    status: Option<String>,
    /// How a completed check run ended.
    #[serde(default)]
    conclusion: Option<String>,
    /// How a commit status stands.
    #[serde(default)]
    state: Option<String>,
    /// The page of a check run.
    #[serde(default, rename = "detailsUrl")]
    details: Option<String>,
    /// The page of a commit status.
    #[serde(default, rename = "targetUrl")]
    target: Option<String>,
}

/// How `rolled` stands, from the status and conclusion of a check run or the state of a commit status.
fn check_state(rolled: &Rolled) -> CheckState {
    if let Some(status) = rolled.status.as_deref() {
        if status != "COMPLETED" {
            return CheckState::Pending;
        }
        return match rolled.conclusion.as_deref().unwrap_or_default() {
            "SUCCESS" => CheckState::Passed,
            "NEUTRAL" | "SKIPPED" => CheckState::Skipped,
            "" => CheckState::Pending,
            _ => CheckState::Failed,
        };
    }
    match rolled.state.as_deref().unwrap_or_default() {
        "SUCCESS" => CheckState::Passed,
        "PENDING" | "EXPECTED" | "" => CheckState::Pending,
        _ => CheckState::Failed,
    }
}

/// The decision GitHub writes as `review`.
fn review_of(review: Option<&str>) -> ReviewDecision {
    match review.unwrap_or_default() {
        "APPROVED" => ReviewDecision::Approved,
        "CHANGES_REQUESTED" => ReviewDecision::ChangesRequested,
        "REVIEW_REQUIRED" => ReviewDecision::Required,
        _ => ReviewDecision::None,
    }
}

/// The first pull request of what `gh pr list --json` wrote, or nothing when it listed none.
///
/// # Errors
///
/// Fails when `listed` is not a list of pull requests.
pub(crate) fn parse_list(listed: &str) -> Result<Option<PullRequestStatus>, String> {
    let found: Vec<Listed> = serde_json::from_str(listed)
        .map_err(|error| format!("gh answered with something unreadable: {error}"))?;
    Ok(found.into_iter().next().map(|listed| PullRequestStatus {
        checks_url: format!("{}/checks", listed.url),
        number: listed.number,
        title: listed.title,
        state: listed.state,
        url: listed.url,
        draft: listed.draft,
        review: review_of(listed.review.as_deref()),
        checks: listed
            .rollup
            .unwrap_or_default()
            .iter()
            .map(|rolled| Check {
                name: rolled
                    .name
                    .clone()
                    .or_else(|| rolled.context.clone())
                    .unwrap_or_default(),
                state: check_state(rolled),
                url: rolled
                    .details
                    .clone()
                    .or_else(|| rolled.target.clone())
                    .unwrap_or_default(),
            })
            .collect(),
        head: listed.head,
        base: listed.base,
    }))
}

/// The arguments of `gh` that list the newest pull request of `branch`, open or not, with its checks and review.
pub(crate) fn status_arguments(branch: &str) -> Vec<String> {
    [
        "pr", "list", "--head", branch, "--state", "all", "--limit", "1", "--json", FIELDS,
    ]
    .map(str::to_owned)
    .to_vec()
}

/// The arguments of `gh` that open a pull request of `branch` into `base`, or into the repository's default branch when there is no base, with `draft`'s title and body, as a draft when `as_draft`.
pub(crate) fn create_arguments(
    branch: &str,
    base: Option<&str>,
    draft: &Draft,
    as_draft: bool,
) -> Vec<String> {
    let mut arguments = vec!["pr".to_owned(), "create".to_owned()];
    if let Some(base) = base {
        arguments.extend(["--base".to_owned(), base.to_owned()]);
    }
    arguments.extend([
        "--head".to_owned(),
        branch.to_owned(),
        "--title".to_owned(),
        draft.title.clone(),
        "--body".to_owned(),
        draft.body.clone(),
    ]);
    if as_draft {
        arguments.push("--draft".to_owned());
    }
    arguments
}

/// The arguments of `gh` that replace the body of the pull request `number` with `body`; its title is left alone, so a rename made on GitHub stays.
pub(crate) fn edit_arguments(number: u64, body: &str) -> Vec<String> {
    vec![
        "pr".to_owned(),
        "edit".to_owned(),
        number.to_string(),
        "--body".to_owned(),
        body.to_owned(),
    ]
}

/// The arguments of `gh` that mark the draft pull request `number` ready for review.
pub(crate) fn ready_arguments(number: u64) -> Vec<String> {
    vec!["pr".to_owned(), "ready".to_owned(), number.to_string()]
}

/// The arguments of `gh` that reopen the closed pull request `number`.
pub(crate) fn reopen_arguments(number: u64) -> Vec<String> {
    vec!["pr".to_owned(), "reopen".to_owned(), number.to_string()]
}

/// The arguments of `gh` that merge the pull request `number` by `strategy`, only while its head is still at `head`. The branch is never deleted by `gh`, which would also delete the local branch a worktree has checked out.
pub(crate) fn merge_arguments(number: u64, strategy: MergeStrategy, head: &str) -> Vec<String> {
    let flag = match strategy {
        MergeStrategy::Merge => "--merge",
        MergeStrategy::Squash => "--squash",
        MergeStrategy::Rebase => "--rebase",
    };
    let mut arguments = vec![
        "pr".to_owned(),
        "merge".to_owned(),
        number.to_string(),
        flag.to_owned(),
    ];
    if !head.is_empty() {
        arguments.extend(["--match-head-commit".to_owned(), head.to_owned()]);
    }
    arguments
}

/// Whether `host` is GitHub's: github.com, a GitHub Enterprise Cloud domain under `ghe.com`, or a self-hosted GitHub Enterprise server whose name has `github` as one of its labels, such as `github.example.com`.
pub(crate) fn is_github_host(host: &str) -> bool {
    host == "github.com"
        || host.ends_with(".ghe.com")
        || host.split('.').any(|label| label == "github")
}

/// The `owner/name` of the GitHub repository at `url`, over https or ssh, or nothing when it is not on GitHub.
#[cfg(test)]
pub(crate) fn github_slug(url: &str) -> Option<String> {
    let address = RemoteAddress::parse(url)?;
    slug_of(&address)
}

/// The `owner/name` of the repository at `address`, or nothing when it is not on GitHub or its path is not two parts.
fn slug_of(address: &RemoteAddress) -> Option<String> {
    if !is_github_host(&address.host) {
        return None;
    }
    let mut parts = address.path.split('/');
    match (parts.next(), parts.next(), parts.next()) {
        (Some(owner), Some(name), None) if !owner.is_empty() && !name.is_empty() => {
            Some(format!("{owner}/{name}"))
        }
        _ => None,
    }
}

/// GitHub's page at `web` comparing `branch` with `base` in the repository `slug`, or with its default branch when there is no base, set to open a pull request with `draft`'s title and as much of its body, cut between characters, as keeps the address within [`COMPARE_LENGTH`].
pub(crate) fn compare_url(
    web: &str,
    slug: &str,
    base: Option<&str>,
    branch: &str,
    draft: &Draft,
) -> String {
    let range = match base {
        Some(base) => format!("{}...{}", encode(base), encode(branch)),
        None => encode(branch),
    };
    let start = format!(
        "{web}/{slug}/compare/{range}?quick_pull=1&title={}",
        encode(&draft.title)
    );
    let room = COMPARE_LENGTH.saturating_sub(start.len() + "&body=".len());
    let mut body = String::new();
    for character in draft.body.chars() {
        let encoded = encode(&character.to_string());
        if body.len() + encoded.len() > room {
            break;
        }
        body.push_str(&encoded);
    }
    if body.is_empty() {
        start
    } else {
        format!("{start}&body={body}")
    }
}

/// Runs `gh` with `arguments` in `directory`, dropping what it wrote.
fn gh_owned(directory: &Path, arguments: &[String]) -> Result<(), String> {
    let borrowed: Vec<&str> = arguments.iter().map(String::as_str).collect();
    gh_in(Some(directory), &borrowed).map(|_| ())
}

impl ReviewHost for GitHub {
    /// `github`.
    fn kind(&self) -> &'static str {
        "github"
    }

    /// `GitHub`.
    fn name(&self) -> &'static str {
        "GitHub"
    }

    /// `pull request`.
    fn review_noun(&self) -> &'static str {
        "pull request"
    }

    /// Whether `address` is on github.com or a GitHub Enterprise server and names a repository as `owner/name`.
    fn recognizes(&self, address: &RemoteAddress) -> bool {
        slug_of(address).is_some()
    }

    /// `pull_request_template.md` in `.github`, the root or `docs`, else the first in `.github/PULL_REQUEST_TEMPLATE`.
    fn templates(&self) -> TemplatePlaces {
        TemplatePlaces {
            file: "pull_request_template.md",
            folders: &[".github", "", "docs"],
            collection: ".github/PULL_REQUEST_TEMPLATE",
        }
    }

    /// The newest pull request of `branch` as `gh pr list` writes it.
    fn status(&self, directory: &Path, branch: &str) -> Option<PullRequestStatus> {
        let borrowed = status_arguments(branch);
        let borrowed: Vec<&str> = borrowed.iter().map(String::as_str).collect();
        gh_in(Some(directory), &borrowed)
            .ok()
            .and_then(|listed| parse_list(&listed).ok().flatten())
    }

    /// Opens it with `gh pr create`.
    fn create(
        &self,
        directory: &Path,
        branch: &str,
        base: Option<&str>,
        draft: &Draft,
        as_draft: bool,
    ) -> Result<(), String> {
        gh_owned(directory, &create_arguments(branch, base, draft, as_draft))
    }

    /// Replaces the body with `gh pr edit`.
    fn update_body(
        &self,
        directory: &Path,
        request: &PullRequestStatus,
        body: &str,
    ) -> Result<(), String> {
        gh_owned(directory, &edit_arguments(request.number, body))
    }

    /// Marks it ready with `gh pr ready`.
    fn mark_ready(&self, directory: &Path, request: &PullRequestStatus) -> Result<(), String> {
        gh_owned(directory, &ready_arguments(request.number))
    }

    /// Merges it with `gh pr merge`, held to its head.
    fn merge(
        &self,
        directory: &Path,
        request: &PullRequestStatus,
        strategy: MergeStrategy,
    ) -> Result<(), String> {
        gh_owned(
            directory,
            &merge_arguments(request.number, strategy, &request.head),
        )
    }

    /// Reopens it with `gh pr reopen`.
    fn reopen(&self, directory: &Path, request: &PullRequestStatus) -> Result<(), String> {
        gh_owned(directory, &reopen_arguments(request.number))
    }

    /// The compare page of the repository, set to open a pull request filled in with `draft`.
    fn new_review_url(
        &self,
        address: &RemoteAddress,
        base: Option<&str>,
        branch: &str,
        draft: &Draft,
    ) -> Option<String> {
        let slug = slug_of(address)?;
        Some(compare_url(&address.web, &slug, base, branch, draft))
    }
}

#[cfg(test)]
mod tests;
