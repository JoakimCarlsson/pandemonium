//! What a client on the same machine needs to reach the server.

use serde::{Deserialize, Serialize};

/// Where a running server listens and the token it wants.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Credential {
    /// The port it listens on.
    pub port: u16,
    /// The token a client sends in its first message.
    pub token: String,
    /// The process that runs it.
    pub pid: u32,
}

/// Where a server is and the token that lets a client in.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Target {
    /// The address, as `host:port`.
    pub address: String,
    /// The token to send in the first message.
    pub token: String,
}
