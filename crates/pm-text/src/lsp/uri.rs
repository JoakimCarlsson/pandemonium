//! File paths as the protocol writes them, and back again.
//!
//! A language server speaks in `file://` URIs. Nothing outside this module
//! does, so a path stays a path everywhere else in the editor.

use std::path::{Path, PathBuf};

/// The characters a path keeps as they are inside a URI.
///
/// Everything else is written as a percent escape, which is what the
/// protocol asks for and what a server compares against.
fn is_plain(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~' | b'/')
}

/// `path` as a `file://` URI.
pub fn of(path: &Path) -> String {
    let mut uri = String::from("file://");
    for byte in path.to_string_lossy().as_bytes() {
        if is_plain(*byte) {
            uri.push(*byte as char);
        } else {
            uri.push_str(&format!("%{byte:02X}"));
        }
    }
    uri
}

/// `path` as the protocol's own URI type.
pub fn typed(path: &Path) -> lsp_types::Uri {
    of(path)
        .parse()
        .expect("a path with every other byte percent-escaped is a valid URI")
}

/// The path the protocol's URI `uri` names, if it names one.
pub fn path_of(uri: &lsp_types::Uri) -> Option<PathBuf> {
    path(uri.as_str())
}

/// The path a `file://` URI names, if it names one.
pub fn path(uri: &str) -> Option<PathBuf> {
    let escaped = uri.strip_prefix("file://")?;
    let bytes = escaped.as_bytes();
    let mut path = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'%' if index + 2 < bytes.len() => {
                let digits = std::str::from_utf8(&bytes[index + 1..index + 3]).ok()?;
                path.push(u8::from_str_radix(digits, 16).ok()?);
                index += 3;
            }
            byte => {
                path.push(byte);
                index += 1;
            }
        }
    }
    Some(PathBuf::from(String::from_utf8(path).ok()?))
}
