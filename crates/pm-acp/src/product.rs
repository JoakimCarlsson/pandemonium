//! The agents the app knows the files of, recognised by what runs them rather than by the name they are listed under.

use crate::agent::Agent;

/// An agent whose own files the app knows how to read.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
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
        let words = crate::mcp::command_words(command);
        let named = |needle: &str| {
            words
                .iter()
                .any(|word| word.to_lowercase().contains(needle))
        };
        if named("claude") {
            Some(Self::Claude)
        } else if named("codex") {
            Some(Self::Codex)
        } else if named("grok") {
            Some(Self::Grok)
        } else if named("copilot") {
            Some(Self::Copilot)
        } else {
            None
        }
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
}

impl Agent {
    /// The product the agent runs, when it is one the app knows.
    pub fn product(self) -> Option<Product> {
        let package = match self.source {
            crate::agent::Source::Package(package) => package,
            _ => "",
        };
        Product::of(&crate::mcp::command_line(
            std::iter::once(self.program)
                .chain(self.arguments.iter().copied())
                .chain(std::iter::once(package)),
        ))
    }
}
