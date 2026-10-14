//! The accounts, organizations and repositories of the GitHub account the `gh` program is signed in to.

use std::process::{Command, Stdio};

use serde::{Deserialize, Serialize};

/// An account whose repositories can be listed: the signed-in one or an organization it belongs to.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Owner {
    /// The account's login.
    pub login: String,
    /// Whether it is an organization rather than the signed-in account itself.
    pub organization: bool,
}

/// An account as GitHub's API writes it.
#[derive(Deserialize)]
struct Account {
    /// The account's login.
    login: String,
}

/// A repository the signed-in GitHub account can reach.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Repository {
    /// The owner and the name, as `owner/name`.
    pub name: String,
    /// What its owner wrote about it, empty when nothing was.
    pub description: String,
    /// Whether it is private.
    pub private: bool,
    /// The address to clone it from: over ssh when `gh` is set to use ssh, over https otherwise.
    pub url: String,
}

/// One repository as GitHub's API writes it.
#[derive(Deserialize)]
struct Listed {
    /// The owner and the name.
    full_name: String,
    /// What its owner wrote about it.
    description: Option<String>,
    /// Whether it is private.
    private: bool,
    /// The https address.
    clone_url: String,
    /// The ssh address.
    ssh_url: String,
}

/// Runs `gh` with `arguments`, never asking a question, and returns what it wrote, or what it complained of.
fn gh(arguments: &[&str]) -> Result<String, String> {
    let output = Command::new("gh")
        .args(arguments)
        .env("GH_PROMPT_DISABLED", "1")
        .stdin(Stdio::null())
        .output()
        .map_err(|error| {
            format!("could not run gh: {error}; install GitHub CLI and run gh auth login")
        })?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).trim().to_owned())
    }
}

/// Whether `gh` is set to clone over ssh.
fn uses_ssh() -> bool {
    gh(&["config", "get", "git_protocol", "--host", "github.com"])
        .is_ok_and(|protocol| protocol.trim() == "ssh")
}

/// Runs `gh api` on `endpoint` and reads every page of what it returns as lists of `T`.
fn pages<T: serde::de::DeserializeOwned>(endpoint: &str) -> Result<Vec<T>, String> {
    let response = gh(&[
        "api",
        "--hostname",
        "github.com",
        "--method",
        "GET",
        endpoint,
        "--paginate",
        "--slurp",
    ])?;
    let pages: Vec<Vec<T>> = serde_json::from_str(&response)
        .map_err(|error| format!("GitHub answered with something unreadable: {error}"))?;
    Ok(pages.into_iter().flatten().collect())
}

/// The signed-in account followed by the organizations it belongs to, by login.
pub fn owners() -> Result<Vec<Owner>, String> {
    let user = gh(&["api", "--hostname", "github.com", "--method", "GET", "user"])?;
    let user: Account = serde_json::from_str(&user)
        .map_err(|error| format!("GitHub answered with something unreadable: {error}"))?;
    let mut organizations: Vec<Account> = pages("user/orgs?per_page=100")?;
    organizations.sort_by_key(|account| account.login.to_lowercase());
    let mut owners = vec![Owner {
        login: user.login,
        organization: false,
    }];
    owners.extend(organizations.into_iter().map(|account| Owner {
        login: account.login,
        organization: true,
    }));
    Ok(owners)
}

/// Every repository of `owner` the signed-in account can reach, by name.
pub fn repositories(owner: &Owner) -> Result<Vec<Repository>, String> {
    let endpoint = if owner.organization {
        format!("orgs/{}/repos?per_page=100&type=all", owner.login)
    } else {
        "user/repos?per_page=100&affiliation=owner".to_owned()
    };
    let ssh = uses_ssh();
    let mut found: Vec<Repository> = pages::<Listed>(&endpoint)?
        .into_iter()
        .map(|listed| Repository {
            name: listed.full_name,
            description: listed.description.unwrap_or_default(),
            private: listed.private,
            url: if ssh {
                listed.ssh_url
            } else {
                listed.clone_url
            },
        })
        .collect();
    found.sort_by_key(|repository| repository.name.to_lowercase());
    Ok(found)
}
