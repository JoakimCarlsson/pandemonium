//! The agents the app offers: one for each provider the harness has.
//!
//! The harness is part of this program and holds the conversation and runs
//! the tools. It says which providers it has, and every one becomes an agent
//! here, so a provider added to the harness is offered without a change to
//! the app.

use std::sync::{PoisonError, RwLock};

use omc_wire::{ProviderInfo, SignIn};

use crate::product::Product;

/// The ids the app kept agents under before its list came from the harness, by the provider each stands for, so that saved sessions and logins still find their agent.
const KEPT_IDS: [(&str, &str); 1] = [("claude", "claude-code")];

/// What a provider needs set besides its API key before it can be reached, by provider id, in the words a settings page shows.
const SETUP_EXTRAS: [(&str, &str); 1] = [(
    "azure",
    "Set AZURE_OPENAI_ENDPOINT or AZURE_OPENAI_RESOURCE_NAME to the resource, and AZURE_OPENAI_DEPLOYMENT to the deployments to offer.",
)];

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
    /// Whether the provider is a model server the user declared rather than one the harness has built in.
    pub server: bool,
    /// The models a declared server was given to offer, in order; empty for a built-in provider and for a server whose models are asked of it.
    pub models: &'static [String],
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
            server: info.server,
            models: &info.models,
        }
    }

    /// What the reader sets up for the agent to be reached, in words a settings page shows: the environment variables its API key is read from, and anything else its provider needs; nothing for an agent that needs no setting up.
    pub fn setup_hint(&self) -> Option<String> {
        let key = match self.key_env {
            [] => None,
            [only] => Some(format!("Reads its key from {only}.")),
            [first @ .., last] => Some(format!(
                "Reads its key from {} or {last}.",
                first.join(", ")
            )),
        };
        let extra = SETUP_EXTRAS
            .iter()
            .find(|(provider, _)| *provider == self.provider)
            .map(|(_, extra)| (*extra).to_owned());
        match (key, extra) {
            (Some(key), Some(extra)) => Some(format!("{key} {extra}")),
            (key, extra) => key.or(extra),
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

/// The agents of this process as last read from the harness, or nothing before they are first asked for.
static AGENTS: RwLock<Option<&'static [Agent]>> = RwLock::new(None);

/// Every agent the app offers, one per provider of the harness, in the harness's order: the built-in providers, then the model servers the user declared.
pub fn agents() -> &'static [Agent] {
    let read = *AGENTS.read().unwrap_or_else(PoisonError::into_inner);
    read.unwrap_or_else(refresh_agents)
}

/// Reads the harness's providers again, so that a model server declared or taken out since is offered or left out from now on, and returns the agents as they now stand.
///
/// The list read before stays valid for whoever holds it: it is left in memory, which costs a few hundred bytes each time the declared servers change.
pub fn refresh_agents() -> &'static [Agent] {
    let providers: &'static [ProviderInfo] = Box::leak(omc_session::providers().into_boxed_slice());
    let listed: &'static [Agent] = Box::leak(
        providers
            .iter()
            .map(Agent::of)
            .collect::<Vec<_>>()
            .into_boxed_slice(),
    );
    *AGENTS.write().unwrap_or_else(PoisonError::into_inner) = Some(listed);
    listed
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

    /// Gemini, OpenAI and Azure are offered by the names the harness gives them, as built-in providers, each saying where its key is read from, and Azure what else it needs.
    #[test]
    fn cloud_model_providers_are_offered_with_hints() {
        let gemini = agent("gemini").unwrap();
        assert!(!gemini.server);
        assert_eq!(gemini.sign_in, SignIn::ApiKey);
        assert_eq!(
            gemini.setup_hint().as_deref(),
            Some("Reads its key from GEMINI_API_KEY or GOOGLE_API_KEY.")
        );
        assert_eq!(
            agent("openai").unwrap().setup_hint().as_deref(),
            Some("Reads its key from OPENAI_API_KEY.")
        );
        let azure = agent("azure").unwrap();
        assert!(
            azure
                .setup_hint()
                .is_some_and(|hint| hint.contains("AZURE_OPENAI_API_KEY")
                    && hint.contains("AZURE_OPENAI_ENDPOINT"))
        );
        assert_eq!(agent("ollama").unwrap().setup_hint(), None);
    }
}
