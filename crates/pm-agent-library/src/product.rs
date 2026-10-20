//! The agents the app knows the files of, recognised by what runs them rather than by the name they are listed under.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// An agent whose own files the app knows how to read.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Product {
    /// Claude Code.
    Claude,
    /// Codex.
    Codex,
    /// Grok Build.
    Grok,
    /// GitHub Copilot.
    Copilot,
}

impl Product {
    /// The product that `command` starts, from the program and package it names, when it is one the app knows.
    pub fn of(command: &str) -> Option<Self> {
        let command = command.to_lowercase();
        [Self::Claude, Self::Codex, Self::Grok, Self::Copilot]
            .into_iter()
            .find(|product| command.contains(product.slug()))
    }

    /// The variable that moves the product's config and login to another folder.
    pub fn home_variable(self) -> &'static str {
        match self {
            Self::Claude => "CLAUDE_CONFIG_DIR",
            Self::Codex => "CODEX_HOME",
            Self::Grok => "GROK_HOME",
            Self::Copilot => "COPILOT_HOME",
        }
    }

    /// The name the product's folders and the accounts kept for it are filed under.
    pub fn slug(self) -> &'static str {
        match self {
            Self::Claude => "claude",
            Self::Codex => "codex",
            Self::Grok => "grok",
            Self::Copilot => "copilot",
        }
    }

    /// What the product is called.
    pub fn name(self) -> &'static str {
        match self {
            Self::Claude => "Claude Code",
            Self::Codex => "Codex",
            Self::Grok => "Grok Build",
            Self::Copilot => "GitHub Copilot",
        }
    }

    /// The folder the product keeps its config in when nothing moved it: the one its home variable names, or `~/.<slug>`.
    pub fn default_folder(self) -> Option<PathBuf> {
        std::env::var_os(self.home_variable())
            .map(PathBuf::from)
            .or_else(|| Some(std::env::home_dir()?.join(format!(".{}", self.slug()))))
    }
}
