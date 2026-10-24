//! One customization given to an agent: what kind it is, where it comes from and what it says about itself.

use serde::{Deserialize, Serialize};

use crate::product::Product;

/// What a customization is.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    /// A skill the agent loads when a task calls for it.
    #[default]
    Skill,
    /// A file of instructions the agent reads for the folders it applies to.
    Instruction,
    /// An agent definition the agent can hand work to.
    Agent,
    /// A command the agent runs when something happens.
    Hook,
}

impl Kind {
    /// Every kind, in the order the settings list them.
    pub const ALL: [Kind; 4] = [Kind::Skill, Kind::Instruction, Kind::Agent, Kind::Hook];

    /// The word a key of the kind starts with.
    pub fn slug(self) -> &'static str {
        match self {
            Self::Skill => "skill",
            Self::Instruction => "instruction",
            Self::Agent => "agent",
            Self::Hook => "hook",
        }
    }

    /// Whether `product` can be started without the turned-off customizations of this kind; a product that reads them itself and cannot skip one cannot.
    pub fn switchable(self, product: Option<Product>) -> bool {
        matches!(
            (product, self),
            (
                Some(Product::Claude),
                Kind::Skill | Kind::Instruction | Kind::Agent
            ) | (Some(Product::Codex), Kind::Skill)
        )
    }
}

/// Where a customization comes from.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Scope {
    /// The project's own folder.
    #[default]
    Workspace,
    /// The reader's own folders, for every project.
    User,
    /// What came with the agent or a plugin installed for it.
    BuiltIn,
}

impl Scope {
    /// The word a key of the scope holds.
    pub fn slug(self) -> &'static str {
        match self {
            Self::Workspace => "workspace",
            Self::User => "user",
            Self::BuiltIn => "built-in",
        }
    }

    /// What a row calls it.
    pub fn label(self) -> &'static str {
        match self {
            Self::Workspace => "Workspace",
            Self::User => "User",
            Self::BuiltIn => "Built-in",
        }
    }
}

/// One customization found for a project.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct Item {
    /// What it is.
    pub kind: Kind,
    /// Where it comes from.
    pub scope: Scope,
    /// What it is called; for a hook, the event it runs on.
    pub name: String,
    /// What it says it is for; for a hook, the tools it matches.
    #[serde(default)]
    pub description: String,
    /// The file it is read from, on the machine of the project.
    pub path: String,
    /// For an instruction, the folders or patterns it applies to.
    #[serde(default)]
    pub applies: Vec<String>,
    /// For an agent definition, the model it runs on, when it names one.
    #[serde(default)]
    pub model: String,
    /// For an agent definition, the tools it may call, when it names them.
    #[serde(default)]
    pub tools: Vec<String>,
    /// For a hook, the command it runs.
    #[serde(default)]
    pub command: String,
}

impl Item {
    /// What the item is kept under when it is turned off: its kind, scope and name.
    pub fn key(&self) -> String {
        format!("{}/{}/{}", self.kind.slug(), self.scope.slug(), self.name)
    }

    /// Whether the item is given to agents, which it is unless `withheld` holds its key.
    pub fn enabled(&self, withheld: &[String]) -> bool {
        !withheld.contains(&self.key())
    }
}
