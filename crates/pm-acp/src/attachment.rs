//! Files a reader adds to an agent's next prompt.

use std::path::Path;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use serde_json::{Value, json};

/// The largest file whose text goes into a prompt whole, in bytes.
const EMBEDDED: u64 = 256 * 1024;

/// The prompt content block for the file at `path`.
///
/// A picture goes as its bytes where the agent takes images, a text file as
/// its contents where the agent takes embedded context and is small enough,
/// and anything else as a link to where the file is.
pub(crate) fn content(path: &Path, images: bool, embeds: bool) -> Option<Value> {
    let name = path.file_name()?.to_string_lossy().into_owned();
    if let Some(mime_type) = image_type(path).filter(|_| images) {
        let bytes = std::fs::read(path).ok()?;
        return Some(json!({
            "type": "image",
            "data": STANDARD.encode(bytes),
            "mimeType": mime_type,
        }));
    }
    let embedded = embeds
        .then(|| text_of(path))
        .flatten()
        .map(|text| json!({ "type": "resource", "resource": { "uri": file_uri(path), "text": text } }));
    Some(embedded.unwrap_or_else(|| {
        json!({ "type": "resource_link", "uri": file_uri(path), "name": name })
    }))
}

/// The MIME type of the picture at `path` when its extension names a format agents take.
fn image_type(path: &Path) -> Option<&'static str> {
    match path.extension()?.to_str()?.to_ascii_lowercase().as_str() {
        "png" => Some("image/png"),
        "jpg" | "jpeg" => Some("image/jpeg"),
        "gif" => Some("image/gif"),
        "webp" => Some("image/webp"),
        _ => None,
    }
}

/// The text of the file at `path`, when it is text and small enough to send.
fn text_of(path: &Path) -> Option<String> {
    (std::fs::metadata(path).ok()?.len() <= EMBEDDED)
        .then(|| std::fs::read_to_string(path).ok())
        .flatten()
}

/// A local file path expressed as a URI, with path bytes escaped.
fn file_uri(path: &Path) -> String {
    let path = path.to_string_lossy();
    let mut uri = String::from("file://");
    for byte in path.bytes() {
        if byte.is_ascii_alphanumeric() || b"/-._~:".contains(&byte) {
            uri.push(char::from(byte));
        } else {
            uri.push_str(&format!("%{byte:02X}"));
        }
    }
    uri
}
