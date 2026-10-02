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

impl Reach {
    /// What the reader types to name where a server is: the command line it
    /// is started with, or the address it listens at.
    #[must_use]
    pub fn target(&self) -> String {
        match self {
            Self::Command {
                program, arguments, ..
            } => std::iter::once(program)
                .chain(arguments)
                .map(|word| quoted(word))
                .collect::<Vec<_>>()
                .join(" "),
            Self::Http { url, .. } | Self::Events { url, .. } => url.clone(),
        }
    }

    /// The environment of a program, or the headers of a server on the network.
    #[must_use]
    pub fn variables(&self) -> &[(String, String)] {
        match self {
            Self::Command { env, .. } => env,
            Self::Http { headers, .. } | Self::Events { headers, .. } => headers,
        }
    }

    /// This way of reaching a server, with `variables` for its environment or headers.
    #[must_use]
    pub fn with_variables(self, variables: Vec<(String, String)>) -> Self {
        match self {
            Self::Command {
                program, arguments, ..
            } => Self::Command {
                program,
                arguments,
                env: variables,
            },
            Self::Http { url, .. } => Self::Http {
                url,
                headers: variables,
            },
            Self::Events { url, .. } => Self::Events {
                url,
                headers: variables,
            },
        }
    }

    /// How a reader is told which way a server is reached.
    #[must_use]
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Command { .. } => "command",
            Self::Http { .. } => "http",
            Self::Events { .. } => "sse",
        }
    }
}

impl McpServer {
    /// The server a reader describes with a `name`, a `target` and some `variables`.
    ///
    /// A target that starts with `http://` or `https://` is a server on the
    /// network, and one ending in `/sse` speaks server-sent events; anything
    /// else is a command line, split at spaces where it is not quoted.
    ///
    /// # Errors
    ///
    /// Says what is missing when the name or the target is empty.
    pub fn described(
        name: &str,
        target: &str,
        variables: Vec<(String, String)>,
    ) -> Result<Self, String> {
        let name = name.trim();
        let target = target.trim();
        if name.is_empty() {
            return Err("A server needs a name".to_owned());
        }
        let reach = if target.starts_with("https://") || target.starts_with("http://") {
            match target.trim_end_matches('/').ends_with("/sse") {
                true => Reach::Events {
                    url: target.to_owned(),
                    headers: variables,
                },
                false => Reach::Http {
                    url: target.to_owned(),
                    headers: variables,
                },
            }
        } else {
            let mut words = words(target).into_iter();
            let program = words
                .next()
                .ok_or_else(|| "A server needs a command or an address".to_owned())?;
            Reach::Command {
                program,
                arguments: words.collect(),
                env: variables,
            }
        };
        Ok(Self {
            name: name.to_owned(),
            reach,
        })
    }
}

/// `line` split into words at spaces, keeping what is in double quotes whole.
fn words(line: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut word = String::new();
    let mut started = false;
    let mut quoted = false;
    for character in line.chars() {
        match (character, quoted) {
            ('"', _) => {
                quoted = !quoted;
                started = true;
            }
            (space, false) if space.is_whitespace() => {
                if started {
                    words.push(std::mem::take(&mut word));
                    started = false;
                }
            }
            (other, _) => {
                word.push(other);
                started = true;
            }
        }
    }
    if started {
        words.push(word);
    }
    words
}

/// `word` as [`words`] reads it back: in quotes when it holds a space or is empty.
fn quoted(word: &str) -> String {
    match word.is_empty() || word.contains(char::is_whitespace) {
        true => format!("\"{word}\""),
        false => word.to_owned(),
    }
}
