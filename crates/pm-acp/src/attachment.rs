//! Files and images a reader adds to an agent's next prompt.

use std::path::{Path, PathBuf};

use serde_json::{Value, json};

/// The largest file whose text goes into a prompt whole, in bytes.
///
/// A file past this is still attached, as a link the agent reads for itself:
/// a prompt is sent in one message, and a lockfile pasted into it spends the
/// agent's context on what it would never have opened.
const EMBEDDED: u64 = 256 * 1024;

/// One piece of context attached to a prompt.
#[derive(Clone, Debug)]
pub enum Attachment {
    /// A file on its owning machine, readable through a resource link.
    File(pm_host::Location),
    /// A base64 encoded image the agent has said it accepts.
    Image {
        /// The image bytes encoded for the ACP prompt.
        data: String,
        /// The MIME type of those bytes.
        mime_type: String,
        /// The original file name, when the image came from a file.
        name: Option<String>,
    },
    /// Lines picked out of a file, as they read when they were picked.
    Selection {
        /// The file they are from.
        path: PathBuf,
        /// The first of them, counted from one.
        first: usize,
        /// The last of them, counted from one.
        last: usize,
        /// What they say.
        text: String,
    },
}

impl Attachment {
    /// The name shown beside the prompt before it is sent.
    pub fn label(&self) -> String {
        match self {
            Self::File(path) => path.file_name().map_or_else(
                || path.display().to_string(),
                |name| name.to_string_lossy().into_owned(),
            ),
            Self::Image { name, .. } => name.clone().unwrap_or_else(|| "Pasted image".to_owned()),
            Self::Selection {
                path, first, last, ..
            } => {
                let name = path.file_name().unwrap_or_default().to_string_lossy();
                match first == last {
                    true => format!("{name}:{first}"),
                    false => format!("{name}:{first}-{last}"),
                }
            }
        }
    }

    /// This attachment as an ACP prompt content block.
    ///
    /// An agent that `embeds` is sent a text file's contents with its name,
    /// so what it answers about is the file as it was when the reader sent
    /// it; anything else is sent as a link to where the file is. A selection
    /// is always sent whole, as a resource where the agent takes one and as
    /// a fenced passage under its place where it does not.
    pub(crate) fn content(&self, embeds: bool) -> Value {
        match self {
            Self::File(path) => match embeds.then(|| embedded(path)).flatten() {
                Some(text) => json!({
                    "type": "resource",
                    "resource": { "uri": file_uri(path), "text": text },
                }),
                None => json!({
                    "type": "resource_link",
                    "uri": file_uri(path),
                    "name": self.label(),
                }),
            },
            Self::Image {
                data, mime_type, ..
            } => json!({
                "type": "image",
                "data": data,
                "mimeType": mime_type,
            }),
            Self::Selection {
                path,
                first,
                last,
                text,
            } => match embeds {
                true => json!({
                    "type": "resource",
                    "resource": {
                        "uri": format!("{}#L{first}-L{last}", file_uri(path)),
                        "text": text,
                    },
                }),
                false => json!({
                    "type": "text",
                    "text": format!(
                        "{}:{first}-{last}\n```\n{text}\n```",
                        path.display()
                    ),
                }),
            },
        }
    }
}

/// The text of the file at `path`, when it is text and small enough to send.
fn embedded(path: &pm_host::Location) -> Option<String> {
    let size = path.host.fs().metadata(path).ok()?.len();
    (size <= EMBEDDED)
        .then(|| path.host.fs().read_to_string(path).ok())
        .flatten()
}

/// A local file path expressed as a URI, with path bytes escaped.
fn file_uri(path: &Path) -> String {
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
