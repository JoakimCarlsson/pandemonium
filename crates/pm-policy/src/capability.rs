//! The things an extension may be allowed to do, and the set of them the reader granted.

use std::collections::BTreeSet;
use std::fmt;

use serde::{Deserialize, Serialize};

/// Something an extension may do that needs the reader's say.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Capability {
    /// Run a program on the machine, as a language server, a debug adapter, a tool server or from its own code.
    Process,
    /// Fetch a file from the network.
    Download,
    /// Read the files of the project it is used in.
    ReadProject,
}

impl Capability {
    /// Every capability, in the order they are shown.
    pub const ALL: [Self; 3] = [Self::Process, Self::Download, Self::ReadProject];

    /// The name the manifest and the protocol write it as.
    pub fn id(self) -> &'static str {
        match self {
            Self::Process => "process",
            Self::Download => "download",
            Self::ReadProject => "read-project",
        }
    }

    /// What it lets an extension do, in words for the reader.
    pub fn describe(self) -> &'static str {
        match self {
            Self::Process => "Run programs on this machine",
            Self::Download => "Download files from the network",
            Self::ReadProject => "Read the files of a project",
        }
    }

    /// The capability written as `id`, if it is one.
    pub fn parse(id: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|capability| capability.id() == id)
    }
}

impl fmt::Display for Capability {
    /// Writes the name the manifest uses.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.id())
    }
}

/// The refusal of something an extension tried without being granted it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Denied {
    /// The extension that tried.
    pub extension: String,
    /// What it tried.
    pub capability: Capability,
}

impl fmt::Display for Denied {
    /// Says which extension was refused what, and where the reader grants it.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "denied: {} may not {}; grant it on the extensions page",
            self.extension,
            self.capability.describe().to_lowercase()
        )
    }
}

impl std::error::Error for Denied {}

/// The capabilities the reader granted one extension.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Grants {
    /// The extension the grants are for.
    extension: String,
    /// What it may do.
    held: BTreeSet<Capability>,
}

impl Grants {
    /// The grants of `extension`, holding the capabilities written as `ids`; names that are not capabilities are left out.
    pub fn new<'a>(extension: &str, ids: impl IntoIterator<Item = &'a str>) -> Self {
        Self {
            extension: extension.to_owned(),
            held: ids.into_iter().filter_map(Capability::parse).collect(),
        }
    }

    /// Whether `capability` was granted.
    pub fn allows(&self, capability: Capability) -> bool {
        self.held.contains(&capability)
    }

    /// Nothing when `capability` was granted, and the refusal otherwise.
    pub fn check(&self, capability: Capability) -> Result<(), Denied> {
        if self.allows(capability) {
            Ok(())
        } else {
            Err(Denied {
                extension: self.extension.clone(),
                capability,
            })
        }
    }

    /// The names of what was granted, in the order capabilities are shown.
    pub fn ids(&self) -> Vec<String> {
        self.held.iter().map(|held| held.id().to_owned()).collect()
    }
}
