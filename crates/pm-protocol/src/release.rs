//! What a client is told about a published release of the app.

use serde::{Deserialize, Serialize};

/// One file attached to a release: what it is called, where it is downloaded from and how large it is.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct ReleaseAsset {
    /// Its name, as the release lists it.
    pub name: String,
    /// The address it is downloaded from.
    pub url: String,
    /// How many bytes it is.
    #[serde(default)]
    pub size: u64,
}
