//! GitHub accounts and repositories discovered through the signed-in CLI.

use std::process::{Command, Stdio};

use serde::Deserialize;
use serde::de::DeserializeOwned;

/// An account whose repositories can be browsed.
#[derive(Clone, Debug, Deserialize)]
pub struct GithubOwner {
    /// The account's GitHub login.
    pub login: String,
    /// Whether this account is an organization rather than the signed-in user.
    #[serde(skip)]
    pub organization: bool,
}

/// A repository available to the signed-in GitHub account.
#[derive(Clone, Debug, Deserialize)]
pub struct GithubRepository {
    /// The owner and repository name used for searching.
    pub full_name: String,
    /// The repository's summary, when one has been written.
    pub description: Option<String>,
    /// Whether the repository is private.
    pub private: bool,
    /// The HTTPS address used by git clone.
    pub clone_url: String,
    /// The SSH address used when configured in gh.
    pub ssh_url: String,
}

/// Lists the signed-in account followed by all its organizations.
pub fn github_owners() -> Result<Vec<GithubOwner>, String> {
    let user = api::<GithubOwner>("user", false)?;
    let mut organizations = pages::<GithubOwner>("user/orgs?per_page=100")?;
    organizations.sort_by_key(|owner| owner.login.to_lowercase());
    for owner in &mut organizations {
        owner.organization = true;
    }
    let mut owners = vec![user];
    owners.extend(organizations);
    Ok(owners)
}

/// Lists every accessible repository owned by the selected account.
pub fn github_repositories(owner: &GithubOwner) -> Result<Vec<GithubRepository>, String> {
    let endpoint = if owner.organization {
        format!("orgs/{}/repos?per_page=100&type=all", owner.login)
    } else {
        "user/repos?per_page=100&affiliation=owner".to_owned()
    };
    let mut repositories = pages::<GithubRepository>(&endpoint)?;
    repositories.sort_by_key(|repository| repository.full_name.to_lowercase());
    Ok(repositories)
}

/// Whether gh is configured to clone GitHub repositories over SSH.
pub fn github_uses_ssh() -> bool {
    gh(&["config", "get", "git_protocol", "--host", "github.com"])
        .is_ok_and(|protocol| protocol.trim() == "ssh")
}

/// Collects all arrays returned by a paginated GitHub endpoint.
fn pages<T: DeserializeOwned>(endpoint: &str) -> Result<Vec<T>, String> {
    api::<Vec<Vec<T>>>(endpoint, true).map(|pages| pages.into_iter().flatten().collect())
}

/// Decodes one GitHub response, optionally collecting every page.
fn api<T: DeserializeOwned>(endpoint: &str, paginate: bool) -> Result<T, String> {
    let mut arguments = vec![
        "api",
        "--hostname",
        "github.com",
        "--method",
        "GET",
        endpoint,
    ];
    if paginate {
        arguments.extend(["--paginate", "--slurp"]);
    }
    let response = gh(&arguments)?;
    serde_json::from_str(&response).map_err(|error| format!("Invalid GitHub response: {error}"))
}

/// Runs gh without interactive input and preserves its failure message.
fn gh(arguments: &[&str]) -> Result<String, String> {
    let output = Command::new("gh")
        .args(arguments)
        .env("GH_PROMPT_DISABLED", "1")
        .stdin(Stdio::null())
        .output()
        .map_err(|error| {
            format!("Could not run gh: {error}. Install GitHub CLI and run gh auth login.")
        })?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).trim().to_owned())
    }
}
