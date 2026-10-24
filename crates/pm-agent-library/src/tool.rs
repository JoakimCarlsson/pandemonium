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

/// Claude Code's own tools, each with what it does.
const CLAUDE_TOOLS: [(&str, &str); 12] = [
    ("Agent", "Hands a task to an agent definition"),
    ("Bash", "Runs shell commands"),
    ("Edit", "Changes part of a file"),
    ("Glob", "Finds files by pattern"),
    ("Grep", "Searches the contents of files"),
    ("NotebookEdit", "Changes cells of a notebook"),
    ("Read", "Reads files"),
    ("Skill", "Loads a skill"),
    ("TodoWrite", "Keeps a list of the steps of a task"),
    ("WebFetch", "Fetches a web page"),
    ("WebSearch", "Searches the web"),
    ("Write", "Writes whole files"),
];

/// Codex's own tools, each with what it does.
const CODEX_TOOLS: [(&str, &str); 5] = [
    ("shell", "Runs shell commands"),
    ("apply_patch", "Changes files with a patch"),
    ("update_plan", "Keeps the plan of a task"),
    ("view_image", "Looks at a picture"),
    ("web_search", "Searches the web"),
];

impl ToolSet {
    /// The set of `product`'s own tools, when the app knows them.
    pub fn built_in(product: Option<Product>) -> Option<Self> {
        let tools: &[(&str, &str)] = match product? {
            Product::Claude => &CLAUDE_TOOLS,
            Product::Codex => &CODEX_TOOLS,
            Product::Grok | Product::Copilot => return None,
        };
        Some(Self {
            id: BUILT_IN.to_owned(),
            name: format!("{} tools", product?.name()),
            source: Source::BuiltIn,
            plugin: None,
            tools: tools
                .iter()
                .map(|(name, description)| ToolEntry {
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

    /// Whether `product` can be started without one tool of the set while it keeps the others: Claude Code can leave out any tool, every other product only a tool server whole.
    pub fn per_tool(&self, product: Option<Product>) -> bool {
        product == Some(Product::Claude)
    }

    /// Whether `product` can be started without the set: a tool server can always be left out, an agent's own tools only by Claude Code.
    pub fn switchable(&self, product: Option<Product>) -> bool {
        self.source != Source::BuiltIn || self.per_tool(product)
    }

    /// The name `product` gives the tool `tool` of the set, as it is turned off by.
    pub fn agent_name(&self, tool: &str) -> String {
        match self.source {
            Source::BuiltIn => tool.to_owned(),
            Source::Server | Source::Plugin => format!("mcp__{}__{tool}", normalized(&self.id)),
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

/// `name` as Claude Code writes a server's name into the names of its tools: every character that is not a letter, a digit, `_` or `-` made `_`.
fn normalized(name: &str) -> String {
    name.chars()
        .map(|character| match character {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '_' | '-' => character,
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
