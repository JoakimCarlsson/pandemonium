//! The JSON-RPC envelope every language server message travels in.
//!
//! What a message says is the protocol's, and is typed by the method it is
//! sent under: a request's params and its result are the ones `lsp_types`
//! names for that method, and nothing here builds or reads them by hand.
//! What is left is the envelope itself — an id, a method, a result or an
//! error — which is the same for every method and is what this module is.

use lsp_types::notification::Notification;
use lsp_types::request::Request;
use serde_json::{Value, json};

/// The error a request is answered with when the editor does not know its method.
pub(super) const METHOD_NOT_FOUND: i64 = -32601;

/// The error a server answers with when the editor called a request off.
pub(super) const REQUEST_CANCELLED: i64 = -32800;

/// The error a server answers with when the file changed before it could answer.
pub(super) const CONTENT_MODIFIED: i64 = -32801;

/// The error a server answers with when it cancelled a request of its own accord.
pub(super) const SERVER_CANCELLED: i64 = -32802;

/// One message a server sent, taken out of its envelope.
pub(super) enum Incoming {
    /// An answer to a question the editor asked.
    Response {
        /// The id the question was asked under.
        id: i64,
        /// What the server answered, or why it did not.
        outcome: Result<Value, Failure>,
    },
    /// A question the server asks the editor, which is owed an answer.
    Request {
        /// The id to answer under, exactly as it was sent.
        id: Value,
        /// What is being asked.
        method: String,
        /// What it is asked with.
        params: Value,
    },
    /// Something the server says that is owed nothing.
    Notification {
        /// What is being said.
        method: String,
        /// What it is said with.
        params: Value,
    },
}

/// Why a server did not answer a question.
#[derive(Debug)]
pub(super) struct Failure {
    /// The error code it gave.
    pub code: i64,
    /// What it said about it.
    pub message: String,
    /// Whether the server asks for the question to be asked again.
    pub retrigger: bool,
}

impl Incoming {
    /// The message `value` holds, if it is one a server may send.
    pub(super) fn read(mut value: Value) -> Option<Self> {
        let method = value
            .get("method")
            .and_then(Value::as_str)
            .map(str::to_owned);
        let params = value
            .get_mut("params")
            .map(Value::take)
            .unwrap_or(Value::Null);
        match (method, value.get_mut("id").map(Value::take)) {
            (Some(method), Some(id)) => Some(Self::Request { id, method, params }),
            (Some(method), None) => Some(Self::Notification { method, params }),
            (None, Some(id)) => Some(Self::Response {
                id: id.as_i64()?,
                outcome: match value.get_mut("error").map(Value::take) {
                    Some(error) => Err(Failure::read(&error)),
                    None => Ok(value
                        .get_mut("result")
                        .map(Value::take)
                        .unwrap_or(Value::Null)),
                },
            }),
            (None, None) => None,
        }
    }
}

impl Failure {
    /// The failure a response's `error` object describes.
    fn read(error: &Value) -> Self {
        Self {
            code: error["code"].as_i64().unwrap_or_default(),
            message: error["message"].as_str().unwrap_or_default().to_owned(),
            retrigger: error["data"]["retriggerRequest"].as_bool().unwrap_or(false),
        }
    }
}

/// The request `R` asked under `id` with `params`.
///
/// A method that takes no params is sent without the member: a server that
/// expects none rejects a `null` as it would any other value.
pub(super) fn request<R: Request>(id: i64, params: R::Params) -> Value {
    with_params(
        json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": R::METHOD,
        }),
        serde_json::to_value(params).unwrap_or_default(),
    )
}

/// The notification `N` said with `params`, without the member when there
/// are none.
pub(super) fn notification<N: Notification>(params: N::Params) -> Value {
    with_params(
        json!({
            "jsonrpc": "2.0",
            "method": N::METHOD,
        }),
        serde_json::to_value(params).unwrap_or_default(),
    )
}

/// `message` with `params` as its `params` member, unless they are `null`.
fn with_params(mut message: Value, params: Value) -> Value {
    if !params.is_null() {
        message["params"] = params;
    }
    message
}

/// `message` with the members of its params named in `fields` taken out
/// where they are `null`.
///
/// A typed optional that is absent serializes as `null`, which a server that
/// reads the field as a string or an object rejects. Only the fields named
/// are touched: elsewhere `null` is a value the protocol gives meaning.
pub(super) fn without_null(mut message: Value, fields: &[&str]) -> Value {
    if let Some(params) = message.get_mut("params").and_then(Value::as_object_mut) {
        params.retain(|name, value| !(value.is_null() && fields.contains(&name.as_str())));
    }
    message
}

/// The answer to the server's request `id`, carrying `result`.
pub(super) fn response<R: Request>(id: Value, result: R::Result) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": result,
    })
}

/// The answer to the server's request `id` saying it could not be answered.
pub(super) fn error(id: Value, code: i64, message: &str) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": { "code": code, "message": message },
    })
}

/// The params of `R`, read out of what a server sent, if they are well formed.
pub(super) fn params<R: Request>(params: Value) -> Option<R::Params> {
    serde_json::from_value(params).ok()
}

/// The params of the notification `N`, read out of what a server sent.
pub(super) fn said<N: Notification>(params: Value) -> Option<N::Params> {
    serde_json::from_value(params).ok()
}

/// The result of `R`, read out of what a server answered, if it is well formed.
pub(super) fn result<R: Request>(result: Value) -> Option<R::Result> {
    serde_json::from_value(result).ok()
}
