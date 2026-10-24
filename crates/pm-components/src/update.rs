//! Whether a release newer than the running build is out on the channel followed.

/// The version this build is.
pub const RUNNING: &str = env!("CARGO_PKG_VERSION");

/// What looking for a newer release on the channel followed found.
#[derive(Clone, Debug, PartialEq)]
pub enum Update {
    /// The lookup failed, so whether a newer release is out is not known.
    Unknown,
    /// Nothing newer than the running build is out.
    Current,
    /// A newer release is out.
    Newer {
        /// What it is called, as the reader is told.
        name: String,
        /// The page it is published on.
        url: String,
    },
}

impl Update {
    /// What the newest release found on the channel means for the running build: `version` is its version, nothing when none was found, and `url` its page.
    pub fn judged(version: Option<String>, url: String) -> Self {
        match version {
            Some(version) if !url.is_empty() && is_newer(&version, RUNNING) => Self::Newer {
                name: format!("Pandemonium {version}"),
                url,
            },
            _ => Self::Current,
        }
    }
}

/// One dot-separated part of what follows the `-` of a version: a number, or a word such as `rc`; a number comes before a word.
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Part {
    /// A part written in digits.
    Number(u64),
    /// A part written in anything else.
    Word(String),
}

/// What a version written `0.14.0` or `v0.14.0-rc.1` is ordered by: its numbers, whether it is the release those numbers name rather than one made before it, and the parts naming that earlier release.
fn order(version: &str) -> (Vec<u64>, bool, Vec<Part>) {
    let version = version.trim().trim_start_matches('v');
    let (numbers, before) = match version.split_once('-') {
        Some((numbers, before)) => (numbers, Some(before)),
        None => (version, None),
    };
    let parts = before
        .unwrap_or_default()
        .split('.')
        .filter(|part| !part.is_empty())
        .map(|part| {
            part.parse()
                .map_or_else(|_| Part::Word(part.to_owned()), Part::Number)
        })
        .collect();
    (
        numbers
            .split('.')
            .map(|part| part.parse().unwrap_or(0))
            .collect(),
        before.is_none(),
        parts,
    )
}

/// Whether the release `newest` comes after the version `running`.
fn is_newer(newest: &str, running: &str) -> bool {
    order(newest) > order(running)
}
