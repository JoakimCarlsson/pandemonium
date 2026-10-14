//! The file that tells a client on this machine where the server is and how to get in.

use pm_protocol::Credential;

/// The name of the credential file under the app's data folder.
pub const FILE: &str = "server.json";

/// Writes `credential` where clients on this machine look for it, readable by the reader alone.
pub(crate) fn write_credential(credential: &Credential) -> std::io::Result<()> {
    pm_store::save_private(FILE, &serde_json::to_string(credential)?)
}
