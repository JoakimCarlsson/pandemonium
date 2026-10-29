//! What the reader sets for one language server, whichever language it reads: where its program is, what it is started with and what it is told.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::manifest::ServerSpec;

/// What the reader set for the language server called `id`; each thing not set is as the language says.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ServerOverride {
    /// The program to start, a path or a name on the path, in place of the one the language names; empty for the language's own.
    pub binary: String,
    /// The arguments it is started with, in place of the language's; none leaves the language's.
    pub arguments: Option<Vec<String>>,
    /// Variables added to its environment, over those the language sets.
    pub env: BTreeMap<String, String>,
    /// The options it is told in its `initialize` request, in place of the language's.
    pub initialization_options: Option<Value>,
    /// The settings it is told when it asks for its configuration, in place of the language's.
    pub workspace_configuration: Option<Value>,
}

impl ServerOverride {
    /// Whether nothing is set.
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }

    /// `spec` with what is set here put over it.
    pub fn applied(&self, spec: &ServerSpec) -> ServerSpec {
        let mut spec = spec.clone();
        if !self.binary.trim().is_empty() {
            spec.command = self.binary.trim().to_owned();
            spec.install = None;
            spec.check = Vec::new();
            spec.project = Vec::new();
        }
        if let Some(arguments) = &self.arguments {
            spec.args = arguments.clone();
        }
        spec.env.extend(self.env.clone());
        if let Some(options) = &self.initialization_options {
            spec.init = options.clone();
        }
        if let Some(settings) = &self.workspace_configuration {
            spec.settings = settings.clone();
        }
        spec
    }
}
