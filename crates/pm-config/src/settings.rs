//! Everything the window keeps of its own, read from and written to one file.

use serde::{Deserialize, Serialize};

use crate::{
    AgentSettings, AppearanceSettings, GeneralSettings, GitSettings, ServerList, TerminalSettings,
};

/// What the reader set up for the window itself.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct WindowSettings {
    /// How the window opens, closes and keeps itself up to date.
    pub general: GeneralSettings,
    /// How it looks.
    pub appearance: AppearanceSettings,
    /// How its shells are drawn.
    pub terminal: TerminalSettings,
    /// How it draws what git knows about lines.
    pub git: GitSettings,
    /// What it does when an agent finishes.
    pub agents: AgentSettings,
    /// The servers it knows.
    pub servers: ServerList,
}

impl WindowSettings {
    /// The settings written as `text`, with the defaults for what it does not say or cannot be read.
    pub fn read(text: &str) -> Self {
        serde_json::from_str(text).unwrap_or_default()
    }

    /// The settings written down.
    pub fn write(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_default()
    }
}
