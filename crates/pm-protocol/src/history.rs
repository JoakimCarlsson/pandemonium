//! What is asked and said about the states a file was kept in and the sides of a file left in conflict.

use serde::{Deserialize, Serialize};

/// One state of a file that a server kept when the file was saved.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct LocalVersion {
    /// When it was kept, in milliseconds since the epoch, which is also what names it.
    pub id: u64,
    /// How many bytes the file held.
    pub size: u64,
}

/// The versions of a file that a merge, a rebase or a cherry-pick stopped on, and what the working folder holds of it.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct ConflictFiles {
    /// The file as the branch being worked on has it, nothing when it has none.
    pub ours: Option<String>,
    /// The file as the incoming side has it, nothing when it has none.
    pub theirs: Option<String>,
    /// The file as the two sides last had it in common, nothing when they had none.
    pub base: Option<String>,
    /// The file as the working folder holds it, with the markers git wrote where the sides disagree.
    pub merged: String,
    /// The installed language the file is written in, when it is one.
    #[serde(default)]
    pub language: Option<String>,
}
