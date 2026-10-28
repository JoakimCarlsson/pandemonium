//! When a template applies to a project: what its root holds.

use std::path::Path;

use serde::Deserialize;

/// A file at the root of a project and a text it must hold.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
pub struct Contains {
    /// The file, from the root.
    pub file: String,
    /// The text it must hold.
    pub text: String,
}

/// What a project's root has to hold for a template to apply, written as one key: `{"exists": "Cargo.toml"}`.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Condition {
    /// The file or folder, from the root, is there.
    Exists(String),
    /// The file is there and holds the text.
    Contains(Contains),
    /// Every condition holds.
    All(Vec<Condition>),
    /// At least one condition holds.
    Any(Vec<Condition>),
    /// The condition does not hold.
    Not(Box<Condition>),
}

impl Condition {
    /// Whether the project at `root` meets the condition.
    pub fn holds(&self, root: &Path) -> bool {
        match self {
            Self::Exists(path) => root.join(path).exists(),
            Self::Contains(contains) => std::fs::read_to_string(root.join(&contains.file))
                .is_ok_and(|text| text.contains(&contains.text)),
            Self::All(all) => all.iter().all(|condition| condition.holds(root)),
            Self::Any(any) => any.iter().any(|condition| condition.holds(root)),
            Self::Not(condition) => !condition.holds(root),
        }
    }
}
