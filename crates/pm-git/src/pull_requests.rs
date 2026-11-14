//! The pull request of a branch on GitHub: how it stands, with its checks and its review, and what can be done to it through the `gh` program. A new one is drafted from the session's plan, the branch's commits and the repository's template; when `gh` cannot open it, GitHub's compare page, filled in, is offered instead.

use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::github::gh_in;

/// The fields of a pull request `gh` is asked for.
const FIELDS: &str =
    "number,title,state,url,isDraft,reviewDecision,statusCheckRollup,headRefOid,baseRefName";

/// The most bytes of a template that are read.
const TEMPLATE_BYTES: usize = 16 * 1024;

/// The longest the address of a compare page may grow to, in characters; GitHub refuses requests much longer than this.
const COMPARE_LENGTH: usize = 4000;

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
    /// The page that tells about it, empty when GitHub gave none.
    #[serde(default)]
    pub url: String,
}

/// What the reviewers decided about a pull request.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub enum ReviewDecision {
    /// Nothing: the repository asks for no review, or GitHub did not say.
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
    /// How it stands, as GitHub writes it: `OPEN`, `CLOSED` or `MERGED`.
    pub state: String,
    /// Its web address.
    pub url: String,
    /// Whether it is a draft.
    pub draft: bool,
    /// What the reviewers decided.
    pub review: ReviewDecision,
    /// The checks on its head, in the order GitHub lists them.
    pub checks: Vec<Check>,
    /// The commit its head is at.
    pub head: String,
    /// The branch it is to be merged into.
    pub base: String,
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
    /// GitHub's compare page, filled in with the drafted title and body, to open in a browser when `gh` could not open the pull request itself.
    pub compare: Option<String>,
}

/// The title and body a pull request is opened or updated with.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Draft {
    /// The title.
    pub title: String,
    /// The body, in markdown.
    pub body: String,
}

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
pub fn parse_list(listed: &str) -> Result<Option<PullRequestStatus>, String> {
    let found: Vec<Listed> = serde_json::from_str(listed)
        .map_err(|error| format!("gh answered with something unreadable: {error}"))?;
    Ok(found.into_iter().next().map(|listed| PullRequestStatus {
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
pub fn status_arguments(branch: &str) -> Vec<String> {
    [
        "pr", "list", "--head", branch, "--state", "all", "--limit", "1", "--json", FIELDS,
    ]
    .map(str::to_owned)
    .to_vec()
}

/// The arguments of `gh` that open a pull request of `branch` into `base`, or into the repository's default branch when there is no base, with `draft`'s title and body, as a draft when `as_draft`.
pub fn create_arguments(
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
pub fn edit_arguments(number: u64, body: &str) -> Vec<String> {
    vec![
        "pr".to_owned(),
        "edit".to_owned(),
        number.to_string(),
        "--body".to_owned(),
        body.to_owned(),
    ]
}

/// The arguments of `gh` that mark the draft pull request `number` ready for review.
pub fn ready_arguments(number: u64) -> Vec<String> {
    vec!["pr".to_owned(), "ready".to_owned(), number.to_string()]
}

/// The arguments of `gh` that reopen the closed pull request `number`.
pub fn reopen_arguments(number: u64) -> Vec<String> {
    vec!["pr".to_owned(), "reopen".to_owned(), number.to_string()]
}

/// The arguments of `gh` that merge the pull request `number` by `method`, `merge`, `squash` or `rebase`, only while its head is still at `head`. The branch is never deleted by `gh`, which would also delete the local branch a worktree has checked out.
///
/// # Errors
///
/// Fails when `method` is none of the three.
pub fn merge_arguments(number: u64, method: &str, head: &str) -> Result<Vec<String>, String> {
    if !matches!(method, "merge" | "squash" | "rebase") {
        return Err(format!("{method} is not a way to merge a pull request"));
    }
    let mut arguments = vec![
        "pr".to_owned(),
        "merge".to_owned(),
        number.to_string(),
        format!("--{method}"),
    ];
    if !head.is_empty() {
        arguments.extend(["--match-head-commit".to_owned(), head.to_owned()]);
    }
    Ok(arguments)
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

/// The `owner/name` of the GitHub repository at `url`, over https or ssh, or nothing when it is not on GitHub.
pub fn github_slug(url: &str) -> Option<String> {
    let url = url.trim();
    let path = url
        .strip_prefix("git@github.com:")
        .or_else(|| url.strip_prefix("ssh://git@github.com/"))
        .or_else(|| url.strip_prefix("https://github.com/"))
        .or_else(|| url.strip_prefix("http://github.com/"))
        .or_else(|| {
            url.strip_prefix("https://")
                .and_then(|rest| rest.split_once("@github.com/"))
                .map(|(_, path)| path)
        })?;
    let path = path.trim_end_matches('/');
    let path = path.strip_suffix(".git").unwrap_or(path);
    let mut parts = path.split('/');
    match (parts.next(), parts.next(), parts.next()) {
        (Some(owner), Some(name), None) if !owner.is_empty() && !name.is_empty() => {
            Some(format!("{owner}/{name}"))
        }
        _ => None,
    }
}

/// `text` with every byte but the unreserved ones of a URL percent-encoded.
fn encode(text: &str) -> String {
    text.bytes()
        .map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (byte as char).to_string()
            }
            _ => format!("%{byte:02X}"),
        })
        .collect()
}

/// GitHub's page comparing `branch` with `base` in the repository `slug`, or with its default branch when there is no base, set to open a pull request with `draft`'s title and as much of its body, cut between characters, as keeps the address within [`COMPARE_LENGTH`].
pub fn compare_url(slug: &str, base: Option<&str>, branch: &str, draft: &Draft) -> String {
    let range = match base {
        Some(base) => format!("{}...{}", encode(base), encode(branch)),
        None => encode(branch),
    };
    let start = format!(
        "https://github.com/{slug}/compare/{range}?quick_pull=1&title={}",
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

/// The text of the repository's pull request template: `pull_request_template.md`, in any case, in `.github`, the root or `docs`, or else the first by name in `.github/PULL_REQUEST_TEMPLATE`; nothing when it has none.
pub fn template(directory: &Path) -> Option<String> {
    let named = [".github", "", "docs"].iter().find_map(|folder| {
        fs::read_dir(directory.join(folder))
            .ok()?
            .flatten()
            .map(|entry| entry.path())
            .find(|path| {
                path.is_file()
                    && path.file_name().is_some_and(|name| {
                        name.to_string_lossy().to_lowercase() == "pull_request_template.md"
                    })
            })
    });
    let path = named.or_else(|| {
        let mut listed: Vec<_> = fs::read_dir(directory.join(".github/PULL_REQUEST_TEMPLATE"))
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

/// The draft of a pull request of the branch `directory` has checked out: from `title`, the `plan`'s steps, the commits since `base` and the repository's template.
pub fn draft(directory: &Path, base: Option<&str>, title: &str, plan: &[String]) -> Draft {
    let commits = base
        .map(|base| crate::commits_since(directory, base))
        .unwrap_or_default();
    compose(title, plan, &commits, template(directory).as_deref())
}

/// Runs `gh` with `arguments` in `directory`.
fn gh_owned(directory: &Path, arguments: &[String]) -> Result<String, String> {
    let borrowed: Vec<&str> = arguments.iter().map(String::as_str).collect();
    gh_in(Some(directory), &borrowed)
}

/// The newest pull request of `branch`, open or not, with its checks and review, in the GitHub repository `directory` is a clone of; nothing when it has none, or when `gh` cannot say.
pub fn pull_request_status(directory: &Path, branch: &str) -> Option<PullRequestStatus> {
    gh_owned(directory, &status_arguments(branch))
        .ok()
        .and_then(|listed| parse_list(&listed).ok().flatten())
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

/// Acts on the pull request of `branch`, which `directory` has checked out and which was made from `base`: `push` publishes the branch; `create` and `create_draft` push it and open a pull request, or update the open one, drafted from `title`, the `plan`'s steps, the commits and the template, and offer GitHub's compare page when `gh` cannot; `update` pushes and redrafts the body of the open one; `ready` marks a draft ready for review; `merge`, `squash` and `rebase` merge the open one, as long as its head is where it was seen; `reopen` reopens a closed one; and `delete_branch` deletes the branch from its remote once no open pull request needs it.
///
/// # Errors
///
/// Fails when the branch name could be mistaken for an option, when the branch is its own base, when the pull request is not in a state the action needs, or with what `git` or `gh` complained of.
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
    let current = pull_request_status(directory, branch);
    let open = current.as_ref().filter(|request| request.is_open());
    let mut compare = None;
    match action {
        "push" => crate::remote(directory, "push")?,
        "create" | "create_draft" | "update" => {
            crate::remote(directory, "push")?;
            let draft = draft(directory, base.as_deref(), title, plan);
            match (open, action) {
                (Some(request), _) => {
                    gh_owned(directory, &edit_arguments(request.number, &draft.body))?;
                }
                (None, "update") => return Err("this branch has no open pull request".to_owned()),
                (None, _) => {
                    let arguments =
                        create_arguments(branch, base.as_deref(), &draft, action == "create_draft");
                    if let Err(failure) = gh_owned(directory, &arguments)
                        && pull_request_status(directory, branch)
                            .is_none_or(|request| !request.is_open())
                    {
                        let slug = publishing_remote(directory)
                            .and_then(|remote| {
                                crate::run(directory, &["remote", "get-url", "--push", &remote])
                                    .ok()
                            })
                            .and_then(|url| github_slug(&url))
                            .ok_or(failure)?;
                        compare = Some(compare_url(&slug, base.as_deref(), branch, &draft));
                    }
                }
            }
        }
        "ready" => {
            let request = open.ok_or("this branch has no open pull request")?;
            gh_owned(directory, &ready_arguments(request.number))?;
        }
        "merge" | "squash" | "rebase" => {
            let request = open.ok_or("this branch has no open pull request")?;
            gh_owned(
                directory,
                &merge_arguments(request.number, action, &request.head)?,
            )?;
        }
        "reopen" => {
            let request = current
                .as_ref()
                .filter(|request| request.is_closed())
                .ok_or("this branch has no closed pull request")?;
            gh_owned(directory, &reopen_arguments(request.number))?;
        }
        "delete_branch" => {
            if open.is_some() {
                return Err(
                    "an open pull request still needs this branch; deleting it would close the pull request"
                        .to_owned(),
                );
            }
            let remote = publishing_remote(directory)
                .ok_or("this repository has no remote to delete the branch from")?;
            let tip = crate::resolve(directory, &format!("refs/remotes/{remote}/{branch}"));
            let arguments = delete_arguments(&remote, branch, tip.as_deref());
            let borrowed: Vec<&str> = arguments.iter().map(String::as_str).collect();
            crate::run(directory, &borrowed)?;
        }
        other => return Err(format!("{other} is not something to do to a pull request")),
    }
    Ok(Outcome {
        request: pull_request_status(directory, branch),
        compare,
    })
}
