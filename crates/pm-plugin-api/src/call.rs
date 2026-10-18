//! What the host asks an extension's code to do, and what it answers.

use serde::{Deserialize, Serialize};

/// A command the reader ran, as the module is told of it.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct CommandCall {
    /// The command, by the id the manifest gives it.
    pub command: String,
    /// What the reader typed after the command's name.
    #[serde(default)]
    pub argument: String,
    /// The folder of the project the command runs in; nothing unless the extension may read the project.
    #[serde(default)]
    pub project: Option<String>,
}

/// What a command comes to.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct CommandReply {
    /// The text to put in the prompt box.
    #[serde(default)]
    pub prompt: Option<String>,
    /// A line to tell the reader instead, or besides.
    #[serde(default)]
    pub message: Option<String>,
}
