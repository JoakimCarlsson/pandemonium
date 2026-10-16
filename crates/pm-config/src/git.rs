//! How the window draws what git knows about the lines of a file.

use serde::{Deserialize, Serialize};

/// Whether a mark git gives a file is drawn.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GitMarks {
    /// As the editor's own setting says.
    #[default]
    FollowEditor,
    /// Never.
    Off,
}

impl GitMarks {
    /// Whether the mark is drawn when the editor's own setting is `editor`.
    pub fn shows(self, editor: bool) -> bool {
        match self {
            Self::FollowEditor => editor,
            Self::Off => false,
        }
    }
}

/// How the window draws what git knows about the lines of a file.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct GitSettings {
    /// Whether the caret's line says who last changed it.
    pub blame: GitMarks,
    /// Whether the lines changed since the last commit are marked beside their numbers.
    pub gutter: GitMarks,
}
