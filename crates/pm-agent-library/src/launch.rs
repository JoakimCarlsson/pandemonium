//! What an agent is started with so that the customizations and tools turned off are left out of what it is given.

use serde_json::{Value, json};

use crate::item::{Item, Kind};
use crate::product::Product;

/// What is added to an agent's start so that it leaves out what was turned off: the `_meta` of the conversation it opens, and its environment.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Launch {
    /// What the request that opens the conversation carries as `_meta`, when it carries anything.
    pub meta: Option<Value>,
    /// Variables to start the agent's program with.
    pub env: Vec<(String, String)>,
}

impl Launch {
    /// How `product` is started without `items`, the customizations turned off, and without `tools`, the tools turned off by the names it gives them; what it cannot leave out is given to it all the same.
    pub fn without(product: Option<Product>, items: &[Item], tools: &[String]) -> Self {
        let of = |kind: Kind| {
            items
                .iter()
                .filter(move |item| item.kind == kind && kind.switchable(product))
        };
        match product {
            Some(Product::Claude) => {
                let denied: Vec<String> = of(Kind::Skill)
                    .map(|item| format!("Skill({})", item.name))
                    .chain(of(Kind::Agent).map(|item| format!("Agent({})", item.name)))
                    .chain(tools.iter().cloned())
                    .collect();
                let excluded: Vec<&str> = of(Kind::Instruction)
                    .map(|item| item.path.as_str())
                    .collect();
                let mut options = serde_json::Map::new();
                if !denied.is_empty() {
                    options.insert("disallowedTools".to_owned(), json!(denied));
                }
                if !excluded.is_empty() {
                    options.insert(
                        "settings".to_owned(),
                        json!({ "claudeMdExcludes": excluded }),
                    );
                }
                Self {
                    meta: (!options.is_empty())
                        .then(|| json!({ "claudeCode": { "options": options } })),
                    env: Vec::new(),
                }
            }
            Some(Product::Codex) => {
                let skills: Vec<Value> = of(Kind::Skill)
                    .map(|item| json!({ "path": item.path, "enabled": false }))
                    .collect();
                Self {
                    meta: None,
                    env: (!skills.is_empty())
                        .then(|| {
                            (
                                "CODEX_CONFIG".to_owned(),
                                json!({ "skills": { "config": skills } }).to_string(),
                            )
                        })
                        .into_iter()
                        .collect(),
                }
            }
            Some(Product::Grok | Product::Copilot) | None => Self::default(),
        }
    }
}
