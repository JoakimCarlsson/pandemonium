//! The agents the app offers: one for each provider the harness has.
//!
//! The harness is part of this program and holds the conversation and runs
//! the tools. It says which providers it has, and every one becomes an agent
//! here, so a provider added to the harness is offered without a change to
//! the app.

use std::sync::OnceLock;

use omc_wire::{ProviderInfo, SignIn};

use crate::product::Product;

/// The ids the app kept agents under before its list came from the harness, by the provider each stands for, so that saved sessions and logins still find their agent.
const KEPT_IDS: [(&str, &str); 1] = [("claude", "claude-code")];

/// One agent the app knows how to run.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Agent {
    /// The name the app keeps the agent under.
    pub id: &'static str,
    /// The name a row calls it.
    pub name: &'static str,
    /// The product whose files and customizations the app knows, when it knows one for this agent.
    pub product: Option<Product>,
    /// The provider the harness reaches the model through.
    pub provider: &'static str,
    /// How the provider is signed in to.
    pub sign_in: SignIn,
    /// The environment variables the provider reads an API key from, first first; empty when it takes none.
    pub key_env: &'static [String],
    /// The cloud credentials the provider is reached with and where they come from, such as Google Cloud's, when it reads those instead of signing in here.
    pub credentials: Option<&'static str>,
}

impl Agent {
    /// The agent that stands for the harness's provider `info`.
    fn of(info: &'static ProviderInfo) -> Self {
        let provider = info.id.as_str();
        Self {
            id: kept_id(provider),
            name: info.name.as_str(),
            product: Product::of(provider),
            provider,
            sign_in: info.sign_in,
            key_env: &info.key_env,
            credentials: info.credentials.as_deref(),
        }
    }
}

/// The id the app keeps the agent for `provider` under: the provider's own, unless the app kept it under another before.
fn kept_id(provider: &'static str) -> &'static str {
    KEPT_IDS
        .iter()
        .find(|(kept, _)| *kept == provider)
        .map_or(provider, |(_, id)| id)
}

/// Every agent the app offers, one per provider of the harness, in the harness's order.
pub fn agents() -> &'static [Agent] {
    static PROVIDERS: OnceLock<Vec<ProviderInfo>> = OnceLock::new();
    static AGENTS: OnceLock<Vec<Agent>> = OnceLock::new();
    AGENTS.get_or_init(|| {
        PROVIDERS
            .get_or_init(omc_session::providers)
            .iter()
            .map(Agent::of)
            .collect()
    })
}

/// The agent the app keeps under `id`, when it offers one.
pub fn agent(id: &str) -> Option<Agent> {
    agents().iter().copied().find(|agent| agent.id == id)
}

#[cfg(test)]
/// Tests for the list of agents.
mod tests {
    use super::*;

    /// Every provider of the harness is an agent with the first ones in their old order, Claude keeps the id it was saved under, and only Claude and Codex have products.
    #[test]
    fn every_provider_is_an_agent() {
        let ids: Vec<&str> = agents().iter().map(|agent| agent.id).collect();
        assert_eq!(
            &ids[..5],
            ["claude-code", "codex", "ollama", "openrouter", "copilot"]
        );
        assert!(ids.contains(&"kimi-code"));
        let claude = agent("claude-code").unwrap();
        assert_eq!(claude.provider, "claude");
        assert_eq!(claude.product, Some(Product::Claude));
        assert_eq!(agent("codex").unwrap().product, Some(Product::Codex));
        let copilot = agent("copilot").unwrap();
        assert_eq!(copilot.name, "GitHub Copilot");
        assert_eq!(copilot.product, None);
        assert_eq!(copilot.sign_in, SignIn::DeviceCode);
        assert_eq!(copilot.credentials, None);
        let vertex = agent("vertex").unwrap();
        assert_eq!(vertex.sign_in, SignIn::None);
        assert!(
            vertex
                .credentials
                .is_some_and(|credentials| credentials.contains("Google Cloud"))
        );
        assert!(
            agent("bedrock")
                .unwrap()
                .credentials
                .is_some_and(|credentials| credentials.contains("AWS"))
        );
        assert_eq!(agent("ollama").unwrap().credentials, None);
        assert!(agent("nothing").is_none());
    }
}
