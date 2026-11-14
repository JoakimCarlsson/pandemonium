//! The services that host repositories and their reviews, such as GitHub. Each is one implementation of [`ReviewHost`], listed in [`HOSTS`]; the shared pull request workflow finds the one a repository's remote is on and asks it to read and act on reviews, so adding a service means writing one implementation and listing it.

use std::path::Path;

use crate::pull_requests::{Draft, PullRequestStatus};

/// Every service a repository's reviews can be read from and acted on through, tried in order against the remote's address.
pub(crate) const HOSTS: &[&dyn ReviewHost] = &[&crate::github::review::GitHub];

/// How a review is merged into its base.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MergeStrategy {
    /// With a merge commit.
    Merge,
    /// Squashed into one commit.
    Squash,
    /// With its commits rebased onto the base.
    Rebase,
}

impl MergeStrategy {
    /// The strategy `name` names: `merge`, `squash` or `rebase`.
    ///
    /// # Errors
    ///
    /// Fails when `name` is none of the three.
    pub fn parse(name: &str) -> Result<Self, String> {
        match name {
            "merge" => Ok(Self::Merge),
            "squash" => Ok(Self::Squash),
            "rebase" => Ok(Self::Rebase),
            other => Err(format!("{other} is not a way to merge a pull request")),
        }
    }
}

/// Where a service looks for the template a new review's description starts from, relative to the repository's root.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TemplatePlaces {
    /// The name of a single template, matched in any case.
    pub file: &'static str,
    /// The folders a single template is looked for in, in order; an empty name is the root.
    pub folders: &'static [&'static str],
    /// A folder of templates whose first markdown file by name is taken when there is no single one; empty when the service has none.
    pub collection: &'static str,
}

/// A remote's address taken apart: the machine it is on and the path of the repository there.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RemoteAddress {
    /// The machine's name, in lower case, without a user or a port.
    pub host: String,
    /// The repository's path on it, without slashes around it or a `.git` ending.
    pub path: String,
    /// The start of the service's web addresses: the scheme, the machine and, over http, its port.
    pub web: String,
}

impl RemoteAddress {
    /// `url` taken apart, over https, http, ssh or the short `user@host:path` form of ssh; nothing for a local path or anything else.
    pub fn parse(url: &str) -> Option<Self> {
        let url = url.trim();
        let (scheme, rest) = if let Some(rest) = url.strip_prefix("https://") {
            ("https", rest)
        } else if let Some(rest) = url.strip_prefix("http://") {
            ("http", rest)
        } else if let Some(rest) = url
            .strip_prefix("ssh://")
            .or_else(|| url.strip_prefix("git+ssh://"))
        {
            ("ssh", rest)
        } else {
            let (authority, path) = url.split_once(':')?;
            if authority.contains('/')
                || path.starts_with("//")
                || !(authority.contains('@') || authority.contains('.'))
            {
                return None;
            }
            return Self::assemble("ssh", authority, path);
        };
        let (authority, path) = rest.split_once('/')?;
        Self::assemble(scheme, authority, path)
    }

    /// The address on `authority`, with or without a user and a port, at `path`, reached over `scheme`.
    fn assemble(scheme: &str, authority: &str, path: &str) -> Option<Self> {
        let machine = authority
            .rsplit_once('@')
            .map_or(authority, |(_, machine)| machine);
        let host = machine.split(':').next().unwrap_or_default().to_lowercase();
        let path = path.trim_matches('/');
        let path = path.strip_suffix(".git").unwrap_or(path).to_owned();
        if host.is_empty() || path.is_empty() {
            return None;
        }
        let web = match scheme {
            "http" | "https" => format!("{scheme}://{}", machine.to_lowercase()),
            _ => format!("https://{host}"),
        };
        Some(Self { host, path, web })
    }
}

/// A service that hosts repositories and reviews of their branches: GitHub's pull requests, or another's merge requests. Every method that acts returns what the service complained of when it fails.
pub trait ReviewHost: Sync {
    /// Its short, stable name on the wire, such as `github`.
    fn kind(&self) -> &'static str;

    /// Its name as the reader knows it, such as `GitHub`.
    fn name(&self) -> &'static str;

    /// What it calls a review, in lower case, such as `pull request`.
    fn review_noun(&self) -> &'static str;

    /// Whether the remote at `address` is on this service.
    fn recognizes(&self, address: &RemoteAddress) -> bool;

    /// Where it looks for the template of a new review's description.
    fn templates(&self) -> TemplatePlaces;

    /// The newest review of `branch`, open or not, with its checks and decision, in the repository `directory` is a clone of; nothing when it has none, or when the service cannot say.
    fn status(&self, directory: &Path, branch: &str) -> Option<PullRequestStatus>;

    /// Opens a review of `branch` into `base`, or into the default branch when there is none, with `draft`'s title and body, as a draft when `as_draft`.
    ///
    /// # Errors
    ///
    /// Fails with what the service or its program complained of.
    fn create(
        &self,
        directory: &Path,
        branch: &str,
        base: Option<&str>,
        draft: &Draft,
        as_draft: bool,
    ) -> Result<(), String>;

    /// Replaces the description of `request` with `body`, leaving its title alone.
    ///
    /// # Errors
    ///
    /// Fails with what the service or its program complained of.
    fn update_body(
        &self,
        directory: &Path,
        request: &PullRequestStatus,
        body: &str,
    ) -> Result<(), String>;

    /// Marks the draft `request` ready for review.
    ///
    /// # Errors
    ///
    /// Fails with what the service or its program complained of.
    fn mark_ready(&self, directory: &Path, request: &PullRequestStatus) -> Result<(), String>;

    /// Merges `request` by `strategy`, only while its head is still the one it was seen at, and never deleting its branch.
    ///
    /// # Errors
    ///
    /// Fails with what the service or its program complained of.
    fn merge(
        &self,
        directory: &Path,
        request: &PullRequestStatus,
        strategy: MergeStrategy,
    ) -> Result<(), String>;

    /// Reopens the closed `request`.
    ///
    /// # Errors
    ///
    /// Fails with what the service or its program complained of.
    fn reopen(&self, directory: &Path, request: &PullRequestStatus) -> Result<(), String>;

    /// Deletes `branch` from `remote`, only while it is still at `tip` when that is known. By default it is pushed away with `git`, which works on every service.
    ///
    /// # Errors
    ///
    /// Fails with what `git` complained of.
    fn delete_remote_branch(
        &self,
        directory: &Path,
        remote: &str,
        branch: &str,
        tip: Option<&str>,
    ) -> Result<(), String> {
        let arguments = crate::pull_requests::delete_arguments(remote, branch, tip);
        let borrowed: Vec<&str> = arguments.iter().map(String::as_str).collect();
        crate::run(directory, &borrowed).map(|_| ())
    }

    /// The service's web page for opening a review of `branch` into `base`, filled in with `draft`, for the reader to open when the service's program could not open it; nothing when the service has no such page.
    fn new_review_url(
        &self,
        address: &RemoteAddress,
        base: Option<&str>,
        branch: &str,
        draft: &Draft,
    ) -> Option<String>;
}

/// The service the remote at `url` is on, or nothing when no supported one recognizes it.
pub fn detect(url: &str) -> Option<&'static dyn ReviewHost> {
    let address = RemoteAddress::parse(url)?;
    HOSTS.iter().copied().find(|host| host.recognizes(&address))
}

/// `text` with every byte but the unreserved ones of a URL percent-encoded.
pub(crate) fn encode(text: &str) -> String {
    text.bytes()
        .map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (byte as char).to_string()
            }
            _ => format!("%{byte:02X}"),
        })
        .collect()
}

#[cfg(test)]
mod tests;
