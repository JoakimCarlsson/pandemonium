//! The tools an agent is given, in sets by where they come from, and which of them are turned off.

use serde::{Deserialize, Serialize};

use crate::product::Product;

/// Where a set of tools comes from.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Source {
    /// The agent's own tools.
    #[default]
    BuiltIn,
    /// A tool server the reader added.
    Server,
    /// A tool server an extension brings.
    Plugin,
}

/// One tool of a set.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct ToolEntry {
    /// What the agent calls it.
    pub name: String,
    /// What it does.
    #[serde(default)]
    pub description: String,
}

/// The tools that come from one place.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct ToolSet {
    /// What the set is kept under: `built-in` for the agent's own tools, and the server's name for a tool server.
    pub id: String,
    /// What a row calls it.
    pub name: String,
    /// Where it comes from.
    pub source: Source,
    /// For a set an extension brings, the extension, by folder name.
    #[serde(default)]
    pub plugin: Option<String>,
    /// Its tools.
    #[serde(default)]
    pub tools: Vec<ToolEntry>,
    /// Why its tools could not be listed, when they could not.
    #[serde(default)]
    pub error: Option<String>,
}

/// What the set of an agent's own tools is kept under.
pub const BUILT_IN: &str = "built-in";

/// How much of the machine a tool reaches, which decides when the harness asks before running it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Tier {
    /// It only reads.
    Read,
    /// It changes files.
    Write,
    /// It runs programs or other agents.
    Exec,
    /// It touches nothing; it talks to the reader or leaves a mode.
    Neither,
}

/// The harness's own tools, each with what it does and the tier it needs.
const HARNESS_TOOLS: [(&str, &str, Tier); 11] = [
    ("read", "Reads files", Tier::Read),
    ("grep", "Searches the contents of files", Tier::Read),
    ("glob", "Finds files by pattern", Tier::Read),
    ("bash", "Runs shell commands", Tier::Exec),
    ("edit", "Changes part of a file", Tier::Write),
    ("write", "Writes whole files", Tier::Write),
    ("todo", "Keeps a list of the steps of a task", Tier::Read),
    ("ask", "Asks the reader a question", Tier::Neither),
    ("task", "Hands a task to a subagent", Tier::Exec),
    ("skill", "Loads a skill", Tier::Read),
    (
        "exit_plan_mode",
        "Presents a plan and leaves plan mode",
        Tier::Neither,
    ),
];

/// The tier the harness's own tool `name` needs, when it is one of them.
pub fn tier(name: &str) -> Option<Tier> {
    HARNESS_TOOLS
        .iter()
        .find(|(tool, _, _)| *tool == name)
        .map(|(_, _, tier)| *tier)
}

/// The longest name a tool may have for the providers.
const NAME_LIMIT: usize = 64;

impl ToolSet {
    /// The set of the harness's own tools, which every product runs on.
    pub fn built_in(product: Option<Product>) -> Option<Self> {
        product?;
        Some(Self {
            id: BUILT_IN.to_owned(),
            name: "Built-in tools".to_owned(),
            source: Source::BuiltIn,
            plugin: None,
            tools: HARNESS_TOOLS
                .iter()
                .map(|(name, description, _)| ToolEntry {
                    name: (*name).to_owned(),
                    description: (*description).to_owned(),
                })
                .collect(),
            error: None,
        })
    }

    /// What the whole set is kept under when it is turned off.
    pub fn key(&self) -> String {
        format!("tools/{}", self.id)
    }

    /// What the tool `tool` of the set is kept under when it is turned off.
    pub fn tool_key(&self, tool: &str) -> String {
        format!("tool/{}/{tool}", self.id)
    }

    /// Whether the set is turned off whole.
    pub fn off(&self, withheld: &[String]) -> bool {
        withheld.contains(&self.key())
    }

    /// Whether the tool `tool` of the set is given to agents.
    pub fn tool_enabled(&self, tool: &str, withheld: &[String]) -> bool {
        !self.off(withheld) && !withheld.contains(&self.tool_key(tool))
    }

    /// How many of the set's tools are given to agents.
    pub fn enabled_count(&self, withheld: &[String]) -> usize {
        self.tools
            .iter()
            .filter(|tool| self.tool_enabled(&tool.name, withheld))
            .count()
    }

    /// Whether the set reads as on: it is not turned off whole, and it has a tool on or lists none.
    pub fn enabled(&self, withheld: &[String]) -> bool {
        !self.off(withheld) && (self.tools.is_empty() || self.enabled_count(withheld) > 0)
    }

    /// Whether `product` can be started without one tool of the set while it keeps the others: the harness leaves out any tool by name.
    pub fn per_tool(&self, product: Option<Product>) -> bool {
        product.is_some()
    }

    /// Whether `product` can be started without the set.
    pub fn switchable(&self, product: Option<Product>) -> bool {
        product.is_some()
    }

    /// The name the harness gives the tool `tool` of the set, as it is turned off by.
    pub fn agent_name(&self, tool: &str) -> String {
        match self.source {
            Source::BuiltIn => tool.to_owned(),
            Source::Server | Source::Plugin => {
                let mut name = format!("mcp__{}__{}", normalized(&self.id), normalized(tool));
                name.truncate(NAME_LIMIT);
                name
            }
        }
    }

    /// The names, as [`Self::agent_name`] gives them, of the set's tools that are turned off.
    pub fn withheld_names(&self, withheld: &[String]) -> Vec<String> {
        self.tools
            .iter()
            .filter(|tool| !self.tool_enabled(&tool.name, withheld))
            .map(|tool| self.agent_name(&tool.name))
            .collect()
    }
}

/// `name` as the harness writes it into the name of a tool: every character that is not an ASCII letter, digit or `-` made `_`.
fn normalized(name: &str) -> String {
    name.chars()
        .map(|character| match character {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '-' => character,
            _ => '_',
        })
        .collect()
}

/// `withheld` with the set `set` turned on whole, or off whole when it reads as on.
pub fn toggled_set(set: &ToolSet, withheld: &[String]) -> Vec<String> {
    let on = set.enabled(withheld);
    let prefix = format!("tool/{}/", set.id);
    let mut kept: Vec<String> = withheld
        .iter()
        .filter(|key| **key != set.key() && !key.starts_with(&prefix))
        .cloned()
        .collect();
    if on {
        kept.push(set.key());
    }
    kept
}

/// `withheld` with the tool `tool` of `set` turned the other way; turning one tool of a set that is off whole on leaves its other tools off.
pub fn toggled_tool(set: &ToolSet, tool: &str, withheld: &[String]) -> Vec<String> {
    let key = set.tool_key(tool);
    let mut kept: Vec<String> = withheld.to_vec();
    if set.off(withheld) {
        kept.retain(|held| *held != set.key() && !held.starts_with(&format!("tool/{}/", set.id)));
        kept.extend(
            set.tools
                .iter()
                .filter(|other| other.name != tool)
                .map(|other| set.tool_key(&other.name)),
        );
    } else if kept.contains(&key) {
        kept.retain(|held| *held != key);
    } else {
        kept.push(key);
    }
    kept
}

#[cfg(test)]
/// Tests for the names the harness gives tools.
mod tests {
    use super::*;

    /// A tool server's tool is named as the harness names it: unsafe characters become `_` and the name is cut at 64.
    #[test]
    fn server_tools_are_named_like_the_harness() {
        let set = ToolSet {
            id: "my server".to_owned(),
            source: Source::Server,
            ..ToolSet::default()
        };
        assert_eq!(set.agent_name("do.it"), "mcp__my_server__do_it");
        assert_eq!(set.agent_name(&"x".repeat(100)).len(), 64);
    }

    /// The built-in set is the harness's tools, and the same for both products.
    #[test]
    fn built_in_is_the_harness_set() {
        let claude = ToolSet::built_in(Some(Product::Claude)).unwrap();
        let codex = ToolSet::built_in(Some(Product::Codex)).unwrap();
        assert_eq!(claude.tools, codex.tools);
        assert_eq!(claude.tools.len(), 11);
        assert_eq!(claude.agent_name("bash"), "bash");
        assert_eq!(tier("edit"), Some(Tier::Write));
        assert_eq!(tier("ask"), Some(Tier::Neither));
    }
}
