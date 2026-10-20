//! Whether a release newer than the running build is out on the channel followed.

/// The commit this build was made from, as `PANDEMONIUM_COMMIT` said when it was built.
const COMMIT: Option<&str> = option_env!("PANDEMONIUM_COMMIT");

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
    /// What the newest release found on the channel means for the running build: `version` is its version, or for the `nightly` build the day it was made, nothing when none was found; `commit` is the commit the nightly build was made from, and `url` its page.
    pub fn judged(
        nightly: bool,
        version: Option<String>,
        commit: Option<String>,
        url: String,
    ) -> Self {
        let newer = !url.is_empty()
            && match (&version, nightly) {
                (Some(_), true) => {
                    commit.is_some() && COMMIT.is_some() && commit.as_deref() != COMMIT
                }
                (Some(version), false) => pm_protocol::is_newer(version, RUNNING),
                (None, _) => false,
            };
        match version {
            Some(made) if newer && nightly => Self::Newer {
                name: format!("Pandemonium Nightly {made}"),
                url,
            },
            Some(version) if newer => Self::Newer {
                name: format!("Pandemonium {version}"),
                url,
            },
            _ => Self::Current,
        }
    }
}
