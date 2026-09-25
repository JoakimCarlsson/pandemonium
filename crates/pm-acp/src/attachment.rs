//! Files and images a reader adds to an agent's next prompt.

use std::path::PathBuf;

use serde_json::{Value, json};

/// One piece of context attached to a prompt.
#[derive(Clone, Debug)]
pub enum Attachment {
    /// A file the local agent can read through a resource link.
    File(PathBuf),
    /// A base64 encoded image the agent has said it accepts.
    Image { data: String, mime_type: String },
}

impl Attachment {
    /// The name shown beside the prompt before it is sent.
    pub fn label(&self) -> String {
        match self {
            Self::File(path) => path.file_name().map_or_else(
                || path.display().to_string(),
                |name| name.to_string_lossy().into_owned(),
            ),
            Self::Image { .. } => "Pasted image".to_owned(),
        }
    }

    /// This attachment as an ACP prompt content block.
    pub(crate) fn content(&self) -> Value {
        match self {
            Self::File(path) => json!({
                "type": "resource_link",
                "uri": file_uri(path),
                "name": self.label(),
            }),
            Self::Image { data, mime_type } => json!({
                "type": "image",
                "data": data,
                "mimeType": mime_type,
            }),
        }
    }
}

/// A local file path expressed as a URI, with path bytes escaped.
fn file_uri(path: &std::path::Path) -> String {
    let path = path.to_string_lossy().replace('\\', "/");
    let mut uri = String::from("file://");
    if !path.starts_with('/') {
        uri.push('/');
    }
    for byte in path.bytes() {
        if byte.is_ascii_alphanumeric() || b"/-._~:".contains(&byte) {
            uri.push(char::from(byte));
        } else {
            uri.push_str(&format!("%{byte:02X}"));
        }
    }
    uri
}
