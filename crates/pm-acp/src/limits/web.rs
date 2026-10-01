//! The services an agent's limits are read from when the agent will not say.
//!
//! A stored token goes out in the authorization header of a request and
//! nowhere else. A request that fails for any reason is no answer at all, so
//! nothing a failure carries, and so nothing of the token, is ever kept.

use std::time::Duration;

use serde_json::Value;
use ureq::http::Response;
use ureq::{Agent, Body};

/// How long a service has to answer before it is taken as not answering.
const PATIENCE: Duration = Duration::from_secs(10);

/// What the editor calls itself to a service.
const USER_AGENT: &str = concat!("pandemonium/", env!("CARGO_PKG_VERSION"));

/// The client every read goes through, giving up after [`PATIENCE`].
fn client() -> Agent {
    Agent::config_builder()
        .timeout_global(Some(PATIENCE))
        .build()
        .into()
}

/// What `url` answers a `GET` with, authorized as `authorization` and sent
/// with the `headers` the service asks for besides.
pub(super) fn get(url: &str, authorization: &str, headers: &[(&str, &str)]) -> Option<Value> {
    let request = client()
        .get(url)
        .header("Authorization", authorization)
        .header("Accept", "application/json")
        .header("User-Agent", USER_AGENT);
    json(
        headers
            .iter()
            .fold(request, |request, (name, value)| {
                request.header(*name, *value)
            })
            .call(),
    )
}

/// What `url` answers an empty JSON `POST` with, authorized as
/// `authorization`.
pub(super) fn post(url: &str, authorization: &str) -> Option<Value> {
    json(
        client()
            .post(url)
            .header("Authorization", authorization)
            .header("Accept", "application/json")
            .header("User-Agent", USER_AGENT)
            .header("Connect-Protocol-Version", "1")
            .content_type("application/json")
            .send("{}"),
    )
}

/// The JSON a service answered with, where it answered at all.
fn json(response: Result<Response<Body>, ureq::Error>) -> Option<Value> {
    let mut response = response.ok()?;
    serde_json::from_str(&response.body_mut().read_to_string().ok()?).ok()
}
