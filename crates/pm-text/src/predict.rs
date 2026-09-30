//! Predictions supplied by a document's language servers.

use std::ops::Range;
use std::path::Path;
use std::sync::Arc;

use crate::lsp::Asked;
use crate::{Answer, Client, Indent, Position, Request};

/// Text proposed for a buffer version in place of a span.
#[derive(Clone, Debug)]
pub struct Prediction {
    /// The text this proposal replaces.
    pub range: Range<Position>,
    /// The proposed text.
    pub text: String,
    /// The version this proposal was requested against.
    pub version: i32,
}

/// One asynchronous request for a prediction.
#[derive(Clone)]
pub struct Ticket {
    /// The server answering the request, when one is available.
    client: Option<Arc<Client>>,
    /// The server's request identifier.
    asked: Option<Asked>,
    /// The buffer version the request was made against.
    version: i32,
    /// The cursor at which the request was made.
    at: Position,
}

impl Ticket {
    /// The buffer version and cursor this request belongs to.
    pub fn place(&self) -> (i32, Position) {
        (self.version, self.at)
    }
}

/// A provider that can answer predictions without waiting on the caller.
pub trait Predictor {
    /// Begins asking about `text` at `at` in `path` and `version`.
    fn ask(&self, path: &Path, version: i32, at: Position, text: &str) -> Ticket;
    /// Takes a finished answer, or returns none while it is pending.
    fn take(&self, ticket: Ticket) -> Option<Option<Prediction>>;
    /// Stops work on a request whose answer is no longer wanted.
    fn cancel(&self, ticket: Ticket);
}

/// Predictions from the first capable server opened on a document.
pub struct ServerPredictor {
    /// Servers opened on the document, in their configured order.
    clients: Vec<Arc<Client>>,
}

impl ServerPredictor {
    /// Uses the capable clients among `clients` for future questions.
    pub fn new(clients: Vec<Arc<Client>>) -> Self {
        Self { clients }
    }
}

/// Constructs a prediction provider over the servers opened on a document.
pub fn server_predictor(clients: Vec<Arc<Client>>) -> Box<dyn Predictor> {
    Box::new(ServerPredictor::new(clients))
}

impl Predictor for ServerPredictor {
    /// Sends one asynchronous inline completion request.
    fn ask(&self, path: &Path, version: i32, at: Position, _text: &str) -> Ticket {
        let client = self
            .clients
            .iter()
            .find(|client| client.offers(&Request::InlineCompletion, path))
            .cloned();
        let asked = client.as_ref().map(|client| {
            client.ask(
                Request::InlineCompletion,
                path,
                at,
                Indent::default(),
                at..at,
            )
        });
        Ticket {
            client,
            asked,
            version,
            at,
        }
    }

    /// Reads the first proposal from a completed reply.
    fn take(&self, ticket: Ticket) -> Option<Option<Prediction>> {
        let (Some(client), Some(asked)) = (&ticket.client, ticket.asked) else {
            return Some(None);
        };
        let answer = client.answer(asked)?;
        let Answer::Inline(items) = answer else {
            return Some(None);
        };
        Some(items.into_iter().find_map(|mut item| {
            if item.text.is_empty() {
                return None;
            }
            if item.range.start == Position::default() && item.range.end == Position::default() {
                item.range = ticket.at..ticket.at;
            }
            item.version = ticket.version;
            Some(item)
        }))
    }

    /// Cancels the server request when it is still outstanding.
    fn cancel(&self, ticket: Ticket) {
        if let (Some(client), Some(asked)) = (&ticket.client, ticket.asked) {
            client.cancel(asked);
        }
    }
}
