//! The agents the app ships.
//!
//! Every agent is one provider of the harness, which is part of this program
//! and holds the conversation and runs the tools; the record here says which provider and under which
//! name the app keeps it.

use crate::product::Product;

/// One agent the app knows how to run.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Agent {
    /// The name the app keeps the agent under.
    pub id: &'static str,
    /// The name a row calls it.
    pub name: &'static str,
    /// The product whose files and customizations the app knows.
    pub product: Product,
    /// The provider the harness reaches the model through.
    pub provider: &'static str,
}

/// Every agent the app ships, in the order the workspace lists them.
pub const AGENTS: [Agent; 2] = [
    Agent {
        id: "claude-code",
        name: "Claude",
        product: Product::Claude,
        provider: "claude",
    },
    Agent {
        id: "codex",
        name: "Codex",
        product: Product::Codex,
        provider: "codex",
    },
];
