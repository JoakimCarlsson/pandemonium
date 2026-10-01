//! The tool servers the reader wants every agent to have.

use std::sync::RwLock;

use serde_json::{Value, json};

/// How an agent reaches one tool server.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Reach {
    /// A program the agent starts and talks to over its pipes.
    Command {
        /// The program to run.
        program: String,
        /// The arguments to run it with.
        arguments: Vec<String>,
        /// The environment it is started with, over the one the agent has.
        env: Vec<(String, String)>,
    },
    /// A server spoken to over HTTP.
    Http {
        /// Where the server listens.
        url: String,
        /// The headers sent with every request.
        headers: Vec<(String, String)>,
    },
    /// A server spoken to over server-sent events.
    Events {
        /// Where the server listens.
        url: String,
        /// The headers sent with every request.
        headers: Vec<(String, String)>,
    },
}

/// One tool server an agent is told about when a conversation opens.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct McpServer {
    /// What the agent calls the server.
    pub name: String,
    /// How the agent reaches it.
    pub reach: Reach,
}

/// The tool servers every conversation is opened with.
static OFFERED: RwLock<Vec<McpServer>> = RwLock::new(Vec::new());

/// Offers `servers` to every conversation opened from now on.
pub fn install_mcp(servers: Vec<McpServer>) {
    if let Ok(mut offered) = OFFERED.write() {
        *offered = servers;
    }
}

/// What an agent said it can reach besides programs it starts.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Transports {
    /// Whether it can reach a server over HTTP.
    pub(crate) http: bool,
    /// Whether it can reach a server over server-sent events.
    pub(crate) events: bool,
}

impl Transports {
    /// Reads the `mcpCapabilities` of an agent's handshake.
    pub(crate) fn of(capabilities: &Value) -> Self {
        Self {
            http: capabilities["http"] == json!(true),
            events: capabilities["sse"] == json!(true),
        }
    }
}

/// The tool servers a conversation opens with, less those `transports` cannot reach.
pub(crate) fn offered(transports: Transports) -> Value {
    let offered = OFFERED.read().map(|servers| servers.clone());
    let servers = offered.unwrap_or_default();
    Value::Array(
        servers
            .iter()
            .filter(|server| match server.reach {
                Reach::Command { .. } => true,
                Reach::Http { .. } => transports.http,
                Reach::Events { .. } => transports.events,
            })
            .map(McpServer::wire)
            .collect(),
    )
}

impl McpServer {
    /// How the protocol writes this server down.
    fn wire(&self) -> Value {
        match &self.reach {
            Reach::Command {
                program,
                arguments,
                env,
            } => json!({
                "name": self.name,
                "command": program,
                "args": arguments,
                "env": pairs(env),
            }),
            Reach::Http { url, headers } => json!({
                "type": "http",
                "name": self.name,
                "url": url,
                "headers": pairs(headers),
            }),
            Reach::Events { url, headers } => json!({
                "type": "sse",
                "name": self.name,
                "url": url,
                "headers": pairs(headers),
            }),
        }
    }
}

/// `pairs` as the protocol's list of name and value objects.
fn pairs(pairs: &[(String, String)]) -> Vec<Value> {
    pairs
        .iter()
        .map(|(name, value)| json!({ "name": name, "value": value }))
        .collect()
}
