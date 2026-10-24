//! A tool server reached at a web address, each message posted to it and answered in the response, as JSON or as a stream of events.

use std::time::Duration;

use serde_json::{Value, json};

use crate::listing::{Exchange, PROTOCOL, result_of};

/// How long a request to the server may take.
const PATIENCE: Duration = Duration::from_secs(20);

/// The header the server names the conversation in.
const SESSION: &str = "mcp-session-id";

/// A server at an address, and the conversation open with it.
pub(crate) struct Http {
    /// What sends the requests.
    agent: ureq::Agent,
    /// Where the server listens.
    url: String,
    /// The headers every request carries.
    headers: Vec<(String, String)>,
    /// The conversation, once the server named one.
    session: Option<String>,
    /// The id the next request is sent under.
    next: u64,
}

impl Http {
    /// The server at `url`, sent `headers` with every request.
    pub(crate) fn new(url: &str, headers: &[(String, String)]) -> Self {
        Self {
            agent: ureq::Agent::config_builder()
                .timeout_global(Some(PATIENCE))
                .build()
                .into(),
            url: url.to_owned(),
            headers: headers.to_vec(),
            session: None,
            next: 1,
        }
    }

    /// Posts `message` and returns what the response carries, keeping the conversation it names.
    fn post(&mut self, message: &Value) -> Result<String, String> {
        let request = self.headers.iter().fold(
            self.agent
                .post(&self.url)
                .header("Content-Type", "application/json")
                .header("Accept", "application/json, text/event-stream")
                .header("MCP-Protocol-Version", PROTOCOL),
            |request, (name, value)| request.header(name, value),
        );
        let request = match &self.session {
            Some(session) => request.header(SESSION, session),
            None => request,
        };
        let mut response = request
            .send(message.to_string())
            .map_err(|error| error.to_string())?;
        if let Some(session) = response
            .headers()
            .get(SESSION)
            .and_then(|value| value.to_str().ok())
        {
            self.session = Some(session.to_owned());
        }
        response
            .body_mut()
            .read_to_string()
            .map_err(|error| error.to_string())
    }
}

impl Exchange for Http {
    /// Posts the request and finds the answer that carries its id in what came back.
    fn ask(&mut self, method: &str, params: Value) -> Result<Value, String> {
        let id = self.next;
        self.next += 1;
        let body =
            self.post(&json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params }))?;
        std::iter::once(body.as_str())
            .chain(body.lines().filter_map(|line| line.strip_prefix("data:")))
            .filter_map(|text| serde_json::from_str::<Value>(text.trim()).ok())
            .find(|message| message["id"].as_u64() == Some(id))
            .ok_or_else(|| "the server did not answer".to_owned())
            .and_then(|message| result_of(&message))
    }

    /// Posts the notification.
    fn tell(&mut self, method: &str) -> Result<(), String> {
        self.post(&json!({ "jsonrpc": "2.0", "method": method }))
            .map(|_| ())
    }
}
