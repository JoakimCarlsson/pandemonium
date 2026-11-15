//! What the dialogs over the window show: the offer of a newer release and the code a sign-in waits for.

/// How installing a newer release is going.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum UpdateRun {
    /// Nothing is being done yet.
    #[default]
    Idle,
    /// The server is downloading it.
    Downloading {
        /// How many bytes have arrived.
        received: u64,
        /// How many there are in all; zero when not known.
        total: u64,
    },
    /// It is downloaded and being installed, after which the window restarts.
    Installing,
    /// It could not be installed, for this reason.
    Failed(String),
}

impl UpdateRun {
    /// Whether a download or install is under way.
    pub fn busy(&self) -> bool {
        matches!(self, Self::Downloading { .. } | Self::Installing)
    }

    /// The line telling the reader how it is going, when anything is.
    pub fn status(&self) -> Option<String> {
        match self {
            Self::Idle => None,
            Self::Downloading { received, total } if *total > 0 => Some(format!(
                "Downloading… {}%",
                received.saturating_mul(100) / total
            )),
            Self::Downloading { .. } => Some("Downloading…".to_owned()),
            Self::Installing => Some("Installing and restarting…".to_owned()),
            Self::Failed(reason) => Some(format!("The update failed: {reason}")),
        }
    }
}

/// A release newer than the running build, offered to the reader.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct UpdateOffer {
    /// What it is called, as the reader is told.
    pub name: String,
    /// Its version, as the release files are named by.
    pub version: String,
    /// The page it is published on.
    pub url: String,
    /// The day it was published, when known.
    pub published: Option<String>,
    /// Its notes, in markdown, when it has any.
    pub notes: Option<String>,
    /// How installing it is going.
    pub run: UpdateRun,
    /// Whether the dialog offering it is showing.
    pub open: bool,
}

/// The code a sign-in waits for the reader to enter on a web page.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SignInCode {
    /// The agent being signed in to.
    pub agent: String,
    /// The page the code is entered on.
    pub url: String,
    /// The code.
    pub code: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The status line counts the download in percent when its size is known.
    #[test]
    fn tells_how_far_the_download_got() {
        let run = UpdateRun::Downloading {
            received: 25,
            total: 100,
        };
        assert_eq!(run.status().as_deref(), Some("Downloading… 25%"));
        assert!(run.busy());
        assert_eq!(UpdateRun::Idle.status(), None);
    }
}
