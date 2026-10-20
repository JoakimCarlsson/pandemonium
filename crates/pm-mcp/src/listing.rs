//! Asking a tool server what tools it offers.

use serde_json::{Value, json};

/// The version of the protocol the client asks for.
pub(crate) const PROTOCOL: &str = "2025-06-18";

/// A tool server, as it is reached.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Server {
    /// A program started on this machine and spoken to over its standard streams.
    Command {
        /// The program.
        program: String,
        /// Its arguments.
        args: Vec<String>,
        /// The environment it is started with, over the one it inherits.
        env: Vec<(String, String)>,
    },
    /// A server listening at a web address.
    Address {
        /// Where it listens.
        url: String,
        /// The headers every request to it carries.
        headers: Vec<(String, String)>,
    },
}

/// One tool a server offers.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Tool {
    /// What the server calls it.
    pub name: String,
    /// What it does, as the server says.
    pub description: String,
}

/// A way of speaking to a server: a request answered by its result, and a notification answered by nothing.
pub(crate) trait Exchange {
    /// The result the server answers `method` with `params` with.
    fn ask(&mut self, method: &str, params: Value) -> Result<Value, String>;

    /// Tells the server `method`, which it does not answer.
    fn tell(&mut self, method: &str) -> Result<(), String>;
}

/// The tools `server` offers, every page of them, after the conversation with it is opened.
pub fn tools(server: &Server) -> Result<Vec<Tool>, String> {
    match server {
        Server::Command { program, args, env } => {
            listed(&mut crate::stdio::Stdio::start(program, args, env)?)
        }
        Server::Address { url, headers } => listed(&mut crate::http::Http::new(url, headers)),
    }
}

/// The tools the server spoken to through `link` offers.
fn listed(link: &mut impl Exchange) -> Result<Vec<Tool>, String> {
    link.ask(
        "initialize",
        json!({
            "protocolVersion": PROTOCOL,
            "capabilities": {},
            "clientInfo": { "name": "pandemonium", "version": env!("CARGO_PKG_VERSION") },
        }),
    )?;
    link.tell("notifications/initialized")?;
    let mut found = Vec::new();
    let mut cursor: Option<String> = None;
    loop {
        let params = match &cursor {
            Some(cursor) => json!({ "cursor": cursor }),
            None => json!({}),
        };
        let page = link.ask("tools/list", params)?;
        found.extend(
            page["tools"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|tool| {
                    Some(Tool {
                        name: tool["name"].as_str()?.to_owned(),
                        description: tool["description"].as_str().unwrap_or_default().to_owned(),
                    })
                }),
        );
        cursor = page["nextCursor"]
            .as_str()
            .filter(|next| !next.is_empty() && cursor.as_deref() != Some(*next))
            .map(str::to_owned);
        if cursor.is_none() {
            return Ok(found);
        }
    }
}

/// The result of `message`, the server's answer to a request, or why it failed.
pub(crate) fn result_of(message: &Value) -> Result<Value, String> {
    match message.get("error") {
        Some(error) => Err(error["message"]
            .as_str()
            .unwrap_or("the server answered with an error")
            .to_owned()),
        None => Ok(message["result"].clone()),
    }
}
