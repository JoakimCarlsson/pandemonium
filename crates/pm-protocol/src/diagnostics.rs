//! What a server tells about its own health, for a bug report.

use serde::{Deserialize, Serialize};

/// One file a server gives for a bug report, such as the facts about its build or its crash log.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct DiagnosticFile {
    /// Its name inside the report, such as `crashes.json`.
    pub name: String,
    /// What it holds.
    pub text: String,
}
