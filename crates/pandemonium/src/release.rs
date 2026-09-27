//! Whether a newer release of the editor has been published.
//!
//! A release is a tag on the repository. This build carries the workspace
//! version, and a tag newer than that version is an update. Asking is a
//! network call made away from the window; a repository that cannot be
//! reached is one with nothing to offer.

use std::cmp::Ordering;
use std::process::{Command, Stdio};

/// The repository the editor is published from.
pub const REPOSITORY: &str = "https://github.com/JoakimCarlsson/pandemonium";

/// The git remote [`REPOSITORY`] is read from.
const REMOTE: &str = "https://github.com/JoakimCarlsson/pandemonium.git";

/// Whether a published tag is newer than the version this build carries.
pub fn available() -> bool {
    let Some(current) = Version::parse(env!("CARGO_PKG_VERSION")) else {
        return false;
    };
    tags().iter().any(|published| published > &current)
}

/// The version tags published on the repository.
///
/// A remote that does not answer, or stays quiet long enough that the check
/// is no longer a launch check, is a remote with nothing to offer.
fn tags() -> Vec<Version> {
    let Ok(output) = Command::new("git")
        .args([
            "-c",
            "credential.helper=",
            "-c",
            "http.lowSpeedLimit=1",
            "-c",
            "http.lowSpeedTime=20",
            "ls-remote",
            "--tags",
            REMOTE,
        ])
        .env("GIT_TERMINAL_PROMPT", "0")
        .stdin(Stdio::null())
        .output()
    else {
        return Vec::new();
    };
    if !output.status.success() {
        return Vec::new();
    }
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(tag)
        .collect()
}

/// The version one line of `git ls-remote --tags` names, when it names one.
fn tag(line: &str) -> Option<Version> {
    let name = line.split_whitespace().nth(1)?;
    let name = name.strip_prefix("refs/tags/")?;
    if name.ends_with("^{}") {
        return None;
    }
    Version::parse(name.strip_prefix('v')?)
}

/// A published version: its numbers, then the pre-release that precedes them.
///
/// Ordering is semantic version precedence. A pre-release is older than the
/// version it precedes, and a numeric identifier is older than a textual one.
#[derive(Clone, Debug, Eq, PartialEq)]
struct Version {
    /// The major number.
    major: u64,
    /// The minor number.
    minor: u64,
    /// The patch number.
    patch: u64,
    /// The pre-release identifiers, empty for a release.
    pre: Vec<Identifier>,
}

/// One dot-separated piece of a pre-release.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
enum Identifier {
    /// A numeric identifier, which sorts before a textual one.
    Number(u64),
    /// A textual identifier.
    Text(String),
}

impl Version {
    /// `text` read as a version, when it is one.
    fn parse(text: &str) -> Option<Self> {
        let (numbers, pre) = match text.split_once('-') {
            Some((numbers, pre)) => (numbers, Some(pre)),
            None => (text, None),
        };
        let mut numbers = numbers.split('.');
        let major = numbers.next()?.parse().ok()?;
        let minor = numbers.next()?.parse().ok()?;
        let patch = numbers.next()?.parse().ok()?;
        if numbers.next().is_some() {
            return None;
        }
        let pre = match pre {
            Some(pre) => Self::pre(pre)?,
            None => Vec::new(),
        };
        Some(Self {
            major,
            minor,
            patch,
            pre,
        })
    }

    /// The pre-release identifiers in `text`.
    fn pre(text: &str) -> Option<Vec<Identifier>> {
        if text.is_empty() {
            return None;
        }
        text.split('.').map(Identifier::parse).collect()
    }
}

impl Identifier {
    /// `text` read as one pre-release identifier.
    fn parse(text: &str) -> Option<Self> {
        if text.is_empty() || !text.bytes().all(|byte| byte.is_ascii_alphanumeric()) {
            return None;
        }
        let numeric = text.bytes().all(|byte| byte.is_ascii_digit());
        let bare = text == "0" || !text.starts_with('0');
        match numeric && bare {
            true => text.parse().ok().map(Self::Number),
            false => Some(Self::Text(text.to_owned())),
        }
    }
}

impl PartialOrd for Version {
    /// Semantic version precedence against `other`.
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Version {
    /// Semantic version precedence against `other`.
    fn cmp(&self, other: &Self) -> Ordering {
        let numbers =
            (self.major, self.minor, self.patch).cmp(&(other.major, other.minor, other.patch));
        if numbers != Ordering::Equal {
            return numbers;
        }
        match (self.pre.is_empty(), other.pre.is_empty()) {
            (true, false) => Ordering::Greater,
            (false, true) => Ordering::Less,
            _ => self.pre.cmp(&other.pre),
        }
    }
}
