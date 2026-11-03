//! Web addresses: telling one from other text and opening it in the reader's browser.

use std::process::{Command, Stdio};

/// Whether `address` is a web page: it begins with `http://` or `https://`, has something after that and holds no whitespace.
pub fn is_web_address(address: &str) -> bool {
    let rest = address
        .strip_prefix("https://")
        .or_else(|| address.strip_prefix("http://"));
    rest.is_some_and(|rest| !rest.is_empty() && !address.contains(char::is_whitespace))
}

/// Opens `address` in the reader's browser; only a web address is opened.
///
/// # Errors
///
/// Fails with the reason when `address` is not a web address or no opener can be started.
pub fn open(address: &str) -> Result<(), String> {
    if !is_web_address(address) {
        return Err(format!("{address} is not a web address"));
    }
    let opener = if cfg!(target_os = "macos") {
        "open"
    } else {
        "xdg-open"
    };
    Command::new(opener)
        .arg(address)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map(|_| ())
        .map_err(|error| format!("could not open {address}: {error}"))
}
